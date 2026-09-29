//! Borrado seguro: sobrescribir archivos antes de borrarlos, vaciar el espacio
//! libre de una unidad (cipher /w) y preparar el equipo para venderlo o donarlo.
//!
//! En discos SSD el controlador puede conservar copias de los bloques
//! (nivelación de desgaste): sobrescribir reduce mucho el riesgo pero no lo
//! elimina del todo. Para un SSD, lo seguro es BitLocker o el restablecimiento
//! de Windows con "Limpiar la unidad". La interfaz lo explica.

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::State;

/// Datos pseudoaleatorios rápidos (xorshift): basta para sobrescribir, no es criptografía.
struct Noise(u64);

impl Noise {
    fn new() -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9E37_79B9, |d| d.as_nanos() as u64);
        Noise(seed | 1)
    }

    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            let bytes = self.0.to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
}

/// Rutas que nunca se borran: raíz de una unidad, Windows, Archivos de programa,
/// ProgramData, la carpeta personal entera o los datos de AdminOps.
pub fn forbidden(p: &Path) -> bool {
    let s = p.display().to_string().trim_end_matches('\\').to_lowercase();
    let env = |k: &str| std::env::var(k).unwrap_or_default().trim_end_matches('\\').to_lowercase();
    let windows = env("SystemRoot");
    let profile = env("USERPROFILE");
    let users = std::path::Path::new(&profile).parent().map(|p| p.display().to_string().to_lowercase()).unwrap_or_default();
    p.parent().is_none()
        || s.len() <= 3
        || (!windows.is_empty() && s.starts_with(&windows))
        || s.contains(r"\program files")
        || s.ends_with(r"\programdata")
        || s.contains(r"\programdata\microsoft")
        || s == profile
        || s == users
        || s.contains(r"\appdata\roaming\com.adminops.app")
        || s.contains(r"\appdata\local\com.adminops.app")
}

fn wipe_file(path: &Path, noise: &mut Noise) -> std::io::Result<()> {
    let meta = std::fs::metadata(path)?;
    if meta.permissions().readonly() {
        let mut perm = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        std::fs::set_permissions(path, perm)?;
    }
    let len = meta.len();
    {
        let mut f = std::fs::OpenOptions::new().write(true).open(path)?;
        let mut buf = vec![0u8; 1 << 20];
        let mut left = len;
        while left > 0 {
            let n = left.min(buf.len() as u64) as usize;
            noise.fill(&mut buf[..n]);
            f.write_all(&buf[..n])?;
            left -= n as u64;
        }
        f.sync_all()?;
    }
    // Renombrar antes de borrar: el nombre original tampoco queda en la tabla de archivos.
    let hidden = path.with_file_name(format!("{:016x}.tmp", noise.0));
    let target = if std::fs::rename(path, &hidden).is_ok() { hidden } else { path.to_path_buf() };
    std::fs::remove_file(target)
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WipeResult {
    pub files: u64,
    pub bytes: u64,
    /// Archivos que no se pudieron borrar (en uso, sin permiso…).
    pub failed: Vec<String>,
}

fn collect(p: &Path, files: &mut Vec<PathBuf>, dirs: &mut Vec<PathBuf>) {
    let Ok(meta) = std::fs::symlink_metadata(p) else { return };
    if meta.file_type().is_symlink() {
        // El enlace se quita, pero nunca se sigue.
        files.push(p.to_path_buf());
    } else if meta.is_dir() {
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                collect(&e.path(), files, dirs);
            }
        }
        dirs.push(p.to_path_buf());
    } else {
        files.push(p.to_path_buf());
    }
}

/// Borra un archivo o carpeta sobrescribiendo su contenido.
pub fn wipe_path(p: &Path, task: &Task) -> Result<WipeResult, String> {
    if forbidden(p) {
        return Err(format!("«{}» es del sistema o demasiado general y no se borra.", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string())));
    }
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    collect(p, &mut files, &mut dirs);
    let total = files.len();
    let mut noise = Noise::new();
    let mut r = WipeResult::default();
    for (i, f) in files.iter().enumerate() {
        if task.cancelled() {
            return Err(crate::ps::CANCELLED_MSG.into());
        }
        let size = std::fs::symlink_metadata(f).map(|m| if m.file_type().is_symlink() { 0 } else { m.len() }).unwrap_or(0);
        let res = if size == 0 { std::fs::remove_file(f).or_else(|_| std::fs::remove_dir(f)) } else { wipe_file(f, &mut noise) };
        match res {
            Ok(()) => {
                r.files += 1;
                r.bytes += size;
            }
            Err(e) => r.failed.push(format!("{}: {e}", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
        }
        if i % 25 == 0 || i + 1 == total {
            task.step(format!("Borrando de forma segura… {} de {total} archivos", i + 1));
        }
    }
    // Carpetas de dentro hacia fuera (collect las deja en ese orden).
    for d in dirs {
        let _ = std::fs::remove_dir(&d);
    }
    Ok(r)
}

#[tauri::command(async)]
pub fn wipe_pick(app: tauri::AppHandle, folders: bool) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    let d = app.dialog().file();
    let picked = if folders { d.blocking_pick_folders() } else { d.blocking_pick_files() };
    picked.unwrap_or_default().into_iter().filter_map(|p| p.into_path().ok()).map(|p| p.display().to_string()).collect()
}

#[tauri::command(async)]
pub fn wipe_items(app: tauri::AppHandle, tweaks: State<'_, TweakState>, paths: Vec<String>) -> Result<WipeResult, String> {
    if paths.is_empty() {
        return Err("Elige qué borrar.".into());
    }
    let task = Task::new(&app, "wipe").named("Borrado seguro");
    let mut total = WipeResult::default();
    for p in &paths {
        let r = wipe_path(Path::new(p), &task)?;
        total.files += r.files;
        total.bytes += r.bytes;
        total.failed.extend(r.failed);
    }
    let outcome = if total.failed.is_empty() { Ok(()) } else { Err(format!("{} no se pudieron borrar", total.failed.len())) };
    tweaks.record(Op::Run, &format!("Borrado seguro: {} archivos", total.files), &outcome);
    Ok(total)
}

/// Sobrescribe el espacio libre de una unidad (lo ya borrado deja de poder recuperarse).
#[tauri::command(async)]
pub fn wipe_free_space(app: tauri::AppHandle, tweaks: State<'_, TweakState>, drive: String) -> Result<(), String> {
    let letter = drive.trim().trim_end_matches(['\\', ':']).to_ascii_uppercase();
    if letter.len() != 1 || !letter.chars().all(|c| c.is_ascii_uppercase()) {
        return Err("Unidad no válida.".into());
    }
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let task = Task::new(&app, "wipe-free").named(format!("Vaciar el espacio libre de {letter}:"));
    task.step(format!("Sobrescribiendo el espacio libre de {letter}: (puede tardar horas en discos grandes)…"));
    let arg = format!("/w:{letter}:\\");
    let r = crate::ps::exec_opts("cipher.exe", &[&arg], task.opts(None)).map(|_| ());
    tweaks.record(Op::Run, &format!("Espacio libre de {letter}: sobrescrito"), &r);
    r
}

/// Abre una página de Configuración de Windows (solo direcciones ms-settings:).
#[tauri::command]
pub fn open_ms_settings(uri: String) -> Result<(), String> {
    let ok = uri.starts_with("ms-settings:") && uri.len() < 80 && uri.chars().all(|c| c.is_ascii_alphanumeric() || "-:".contains(c));
    if !ok {
        return Err("Dirección no permitida.".into());
    }
    crate::shellopen::open(&uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_wipes_system_or_broad_paths() {
        assert!(forbidden(Path::new(r"C:\")));
        assert!(forbidden(Path::new(r"D:\")));
        assert!(forbidden(Path::new(r"C:\Windows\System32\drivers")));
        assert!(forbidden(Path::new(r"C:\Program Files (x86)\App")));
        assert!(forbidden(Path::new(r"C:\ProgramData")));
        if let Ok(p) = std::env::var("USERPROFILE") {
            assert!(forbidden(Path::new(&p)));
            assert!(!forbidden(&Path::new(&p).join("Documents").join("viejo.docx")));
        }
        assert!(!forbidden(Path::new(r"D:\Clientes\borrar")));
    }

    #[test]
    fn wipes_a_folder() {
        let base = std::env::temp_dir().join(format!("adminops-wipe-{}", std::process::id()));
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::write(base.join("a.txt"), "secreto").unwrap();
        std::fs::write(base.join("sub").join("b.bin"), vec![1u8; 3_000_000]).unwrap();
        std::fs::write(base.join("vacio.txt"), "").unwrap();
        let r = wipe_path(&base, &Task::detached("wipe-test")).unwrap();
        assert_eq!(r.files, 3);
        assert_eq!(r.bytes, 3_000_007);
        assert!(r.failed.is_empty());
        assert!(!base.exists());
        assert!(open_ms_settings("https://evil".into()).is_err());
    }

    #[test]
    fn noise_is_not_constant() {
        let mut n = Noise::new();
        let mut a = [0u8; 64];
        n.fill(&mut a);
        assert!(a.iter().any(|&b| b != a[0]));
    }
}
