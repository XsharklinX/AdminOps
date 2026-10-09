//! Recuperación por firmas: los comandos de la interfaz (buscar archivos en un
//! disco o en una imagen, ver una miniatura y copiar los elegidos). El
//! algoritmo está en `carve.rs`.
//!
//! Los archivos recuperados se copian siempre a otra carpeta, y nunca al disco
//! que se está recuperando: escribir en él puede pisar justo lo que se busca.

use crate::carve::{self, Item};
use crate::rawdisk::{Aligned, BlockSource, FileSource, Mode, RawDisk};
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceSpec {
    /// disk · image
    pub kind: String,
    pub number: u32,
    pub path: String,
    /// Para limitar a una partición.
    pub offset: u64,
    pub length: u64,
}

impl SourceSpec {
    fn key(&self) -> String {
        if self.kind == "image" {
            format!("image:{}", self.path)
        } else {
            format!("disk:{}", self.number)
        }
    }
}

static FOUND: Mutex<Option<HashMap<String, Vec<Item>>>> = Mutex::new(None);

fn remember(key: &str, items: Vec<Item>) {
    FOUND.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(key.to_string(), items);
}

fn recall(key: &str) -> Vec<Item> {
    FOUND.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(key).cloned()).unwrap_or_default()
}

fn open(spec: &SourceSpec) -> Result<Box<dyn BlockSource + Send>, String> {
    if spec.kind == "image" {
        let src = FileSource::open(std::path::Path::new(&spec.path), 512).map_err(|e| format!("No se pudo abrir la imagen: {e}"))?;
        Ok(Box::new(src))
    } else {
        if !crate::elevation::is_elevated() {
            return Err("Requiere ejecutar AdminOps como administrador.".into());
        }
        Ok(Box::new(RawDisk::open(spec.number, Mode::Read)?))
    }
}

fn range(spec: &SourceSpec, total: u64) -> (u64, u64) {
    let from = spec.offset.min(total);
    let to = if spec.length > 0 { (from + spec.length).min(total) } else { total };
    (from, to)
}

/// Busca archivos por su firma. `groups`: photos, documents, videos, music, archives, databases (vacío: todos).
#[tauri::command(async)]
pub fn carve_scan(app: tauri::AppHandle, source: SourceSpec, groups: Vec<String>) -> Result<Vec<Item>, String> {
    let mut src = open(&source)?;
    let (from, to) = range(&source, src.len());
    let key = source.key();
    let task = crate::task::Task::new(&app, format!("carve:{key}")).named("Búsqueda de archivos por firma");
    task.step("Recorriendo el disco…");
    let mut last = 101;
    let began = std::time::Instant::now();
    let items = carve::carve(&mut src, from, to, &groups, |pos, total, found| {
        let pct = (pos * 100 / total.max(1)) as i32;
        if pct != last {
            last = pct;
            let eta = if pos > 0 { (began.elapsed().as_secs_f64() * (total - pos) as f64 / pos as f64) as u64 / 60 } else { 0 };
            task.step(format!("Buscando archivos… {pct} % · {} encontrados · quedan {eta} min", found.len()));
        }
        !task.cancelled()
    });
    remember(&key, items.clone());
    Ok(items)
}

/// Los de la última búsqueda (para volver a la lista sin repetirla).
#[tauri::command]
pub fn carve_found(source: SourceSpec) -> Vec<Item> {
    recall(&source.key())
}

fn mime(ext: &str) -> Option<&'static str> {
    match ext {
        "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Lee `len` bytes desde `abs` (alineando a sector).
fn read_range(src: &mut Box<dyn BlockSource + Send>, abs: u64, len: u64) -> Option<Vec<u8>> {
    let sector = src.sector().max(512) as u64;
    let start = abs / sector * sector;
    let end = (abs + len).div_ceil(sector) * sector;
    let end = end.min(src.len() / sector * sector);
    if end <= start {
        return None;
    }
    let mut buf = Aligned::new((end - start) as usize);
    let n = src.read_at(start, buf.slice((end - start) as usize)).ok()?;
    let skip = (abs - start) as usize;
    let all = &buf.slice((end - start) as usize)[..n];
    (all.len() > skip).then(|| all[skip..all.len().min(skip + len as usize)].to_vec())
}

/// Miniatura de una foto encontrada (hasta 3 MB), como dirección de datos.
#[tauri::command(async)]
pub fn carve_preview(source: SourceSpec, id: u32) -> Option<String> {
    let item = recall(&source.key()).into_iter().find(|i| i.id == id)?;
    let mime = mime(&item.ext)?;
    let mut src = open(&source).ok()?;
    let (from, _) = range(&source, src.len());
    let bytes = read_range(&mut src, from + item.offset, item.length.min(3 * 1024 * 1024))?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecoverSummary {
    pub folder: String,
    pub copied: u32,
    pub bytes: u64,
    pub failed: u32,
}

/// Número del disco físico en el que está una ruta (para no recuperar sobre el mismo disco).
fn disk_of_path(path: &str) -> Option<u32> {
    let l = path.trim().chars().next().filter(|c| c.is_ascii_alphabetic())?;
    crate::ps::powershell(&format!("(Get-Partition -DriveLetter {l} -ErrorAction Stop).DiskNumber")).ok()?.trim().parse().ok()
}

/// Copia los archivos elegidos a una carpeta de otro disco.
#[tauri::command(async)]
pub fn carve_recover(app: tauri::AppHandle, source: SourceSpec, ids: Vec<u32>, dest: String, tweaks: tauri::State<'_, TweakState>) -> Result<RecoverSummary, String> {
    let dest_dir = std::path::PathBuf::from(dest.trim());
    if dest.trim().is_empty() || !dest_dir.is_dir() {
        return Err("Elige una carpeta de destino que exista.".into());
    }
    if source.kind == "disk" && disk_of_path(&dest) == Some(source.number) {
        return Err("El destino está en el mismo disco que se está recuperando. Elige otro disco: escribir en él puede pisar justo lo que buscas.".into());
    }
    let items: Vec<Item> = recall(&source.key()).into_iter().filter(|i| ids.contains(&i.id)).collect();
    if items.is_empty() {
        return Err("No hay archivos elegidos.".into());
    }
    let mut src = open(&source)?;
    let (from, _) = range(&source, src.len());
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let folder = dest_dir.join(format!("Recuperado-{stamp}"));
    let task = crate::task::Task::new(&app, "carve-recover").named("Copiar archivos recuperados");
    let mut out = RecoverSummary { folder: folder.display().to_string(), ..Default::default() };
    for (n, it) in items.iter().enumerate() {
        if task.cancelled() {
            break;
        }
        task.step(format!("Copiando {} de {}…", n + 1, items.len()));
        let dir = folder.join(&it.group);
        let name = format!("{}{:06}.{}", if it.quality == "partial" { "parcial-" } else { "" }, it.id, it.ext);
        let res = std::fs::create_dir_all(&dir).and_then(|_| {
            let mut f = std::fs::File::create(dir.join(&name))?;
            // De 4 MB en 4 MB, para no cargar un vídeo entero en memoria.
            let mut done = 0u64;
            while done < it.length {
                let n = (it.length - done).min(4 * 1024 * 1024);
                let chunk = read_range(&mut src, from + it.offset + done, n).ok_or_else(|| std::io::Error::other("lectura"))?;
                f.write_all(&chunk)?;
                done += chunk.len() as u64;
                if chunk.is_empty() {
                    break;
                }
            }
            Ok(done)
        });
        match res {
            Ok(b) => {
                out.copied += 1;
                out.bytes += b;
            }
            Err(_) => out.failed += 1,
        }
    }
    let journal: Result<(), String> = Ok(());
    tweaks.record(Op::Run, &format!("Recuperar {} archivos por firma a {}", out.copied, out.folder), &journal);
    Ok(out)
}
