//! Analizador de espacio: tamaño por carpeta y archivos más grandes.

use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Emitter;

const TOP_FILES: usize = 100;
/// Carpetas más pequeñas se suman al padre pero no se guardan como nodo.
const MIN_NODE: u64 = 1024 * 1024;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

static CANCEL: AtomicBool = AtomicBool::new(false);
static LAST: Mutex<Option<Scan>> = Mutex::new(None);

struct Node {
    name: String,
    size: u64,
    files: u64,
    children: Vec<Node>,
}

struct Scan {
    root: PathBuf,
    tree: Node,
    top: Vec<TopFile>,
}

#[derive(Default)]
struct Counters {
    files: AtomicU64,
    bytes: AtomicU64,
    denied: AtomicU64,
    /// No lo detiene el botón Cancelar del análisis (p. ej. tamaño de un perfil).
    independent: bool,
}

struct RawEntry {
    name: String,
    is_dir: bool,
    is_reparse: bool,
    size: u64,
    /// Última modificación (segundos desde 1970). Viene en el mismo listado,
    /// así que saber la antigüedad de un archivo no cuesta nada.
    modified: u64,
}

/// FILETIME (100 ns desde 1601) a segundos desde 1970.
#[cfg(windows)]
fn to_epoch(ft: windows_sys::Win32::Foundation::FILETIME) -> u64 {
    let t = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    t.saturating_sub(116_444_736_000_000_000) / 10_000_000
}

/// Lista una carpeta con FindFirstFileExW: nombre, atributos y tamaño llegan en
/// el propio listado. (`DirEntry::metadata()` de std abre cada archivo en las
/// versiones recientes de Rust: 124 s frente a unos segundos en C:\Windows.)
#[cfg(windows)]
fn list_dir(path: &Path) -> Option<Vec<RawEntry>> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        FindClose, FindExInfoBasic, FindExSearchNameMatch, FindFirstFileExW, FindNextFileW, FIND_FIRST_EX_LARGE_FETCH,
        WIN32_FIND_DATAW,
    };
    const DIRECTORY: u32 = 0x10;

    // Prefijo \\?\ para rutas de más de 260 caracteres.
    let mut pattern: Vec<u16> = std::ffi::OsStr::new(r"\\?\").encode_wide().collect();
    pattern.extend(path.join("*").as_os_str().encode_wide());
    pattern.push(0);
    let mut out = Vec::new();
    unsafe {
        let mut data: WIN32_FIND_DATAW = std::mem::zeroed();
        let h = FindFirstFileExW(
            pattern.as_ptr(),
            FindExInfoBasic,
            (&mut data as *mut WIN32_FIND_DATAW).cast(),
            FindExSearchNameMatch,
            std::ptr::null(),
            FIND_FIRST_EX_LARGE_FETCH,
        );
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        loop {
            let len = data.cFileName.iter().position(|&c| c == 0).unwrap_or(data.cFileName.len());
            let name = String::from_utf16_lossy(&data.cFileName[..len]);
            if name != "." && name != ".." {
                out.push(RawEntry {
                    is_dir: data.dwFileAttributes & DIRECTORY != 0,
                    is_reparse: data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
                    size: ((data.nFileSizeHigh as u64) << 32) | data.nFileSizeLow as u64,
                    modified: to_epoch(data.ftLastWriteTime),
                    name,
                });
            }
            if FindNextFileW(h, &mut data) == 0 {
                break;
            }
        }
        FindClose(h);
    }
    Some(out)
}

/// Archivo grande: tamaño, ruta y última modificación.
type TopFile = (u64, PathBuf, u64);

fn merge_top(a: &mut Vec<TopFile>, b: Vec<TopFile>) {
    a.extend(b);
    if a.len() > TOP_FILES * 2 {
        a.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
        a.truncate(TOP_FILES);
    }
}

fn walk(path: &Path, c: &Counters) -> (Node, Vec<TopFile>) {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let mut node = Node { name, size: 0, files: 0, children: vec![] };
    let mut top = Vec::new();
    if !c.independent && CANCEL.load(Ordering::Relaxed) {
        return (node, top);
    }
    let Some(entries) = list_dir(path) else {
        c.denied.fetch_add(1, Ordering::Relaxed);
        return (node, top);
    };
    let mut subdirs = Vec::new();
    for e in entries {
        if e.is_reparse {
            continue; // uniones/enlaces: evitaría contar dos veces o entrar en bucles
        }
        if e.is_dir {
            subdirs.push(path.join(&e.name));
        } else {
            let len = e.size;
            node.size += len;
            node.files += 1;
            top.push((len, path.join(&e.name), e.modified));
            c.files.fetch_add(1, Ordering::Relaxed);
            c.bytes.fetch_add(len, Ordering::Relaxed);
        }
    }
    if top.len() > TOP_FILES {
        top.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
        top.truncate(TOP_FILES);
    }
    let results: Vec<(Node, Vec<TopFile>)> = subdirs.par_iter().map(|d| walk(d, c)).collect();
    for (child, child_top) in results {
        node.size += child.size;
        node.files += child.files;
        merge_top(&mut top, child_top);
        if child.size >= MIN_NODE {
            node.children.push(child);
        }
    }
    node.children.sort_unstable_by_key(|a| std::cmp::Reverse(a.size));
    (node, top)
}

/// Tamaño total y número de archivos de una carpeta (sin seguir enlaces).
pub fn folder_size(path: &Path) -> (u64, u64) {
    let c = Counters { independent: true, ..Default::default() };
    let (node, _) = walk(path, &c);
    (node.size, node.files)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceEntry {
    name: String,
    path: String,
    size: u64,
    files: u64,
    has_children: bool,
    /// Última modificación (solo en archivos; `None` en carpetas).
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceView {
    root: String,
    size: u64,
    files: u64,
    denied: u64,
    seconds: f64,
    children: Vec<SpaceEntry>,
    largest_files: Vec<SpaceEntry>,
    /// Grandes y sin tocar en más de un año: lo que de verdad se puede ofrecer borrar.
    old_files: Vec<SpaceEntry>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    files: u64,
    bytes: u64,
}

fn entries_of(node: &Node, base: &Path) -> Vec<SpaceEntry> {
    node.children
        .iter()
        .map(|c| SpaceEntry {
            path: base.join(&c.name).display().to_string(),
            name: c.name.clone(),
            size: c.size,
            files: c.files,
            has_children: !c.children.is_empty(),
            modified: None,
        })
        .collect()
}

fn file_entry((size, path, modified): &TopFile) -> SpaceEntry {
    SpaceEntry {
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        path: path.display().to_string(),
        size: *size,
        files: 1,
        has_children: false,
        modified: Some(*modified),
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[tauri::command(async)]
pub fn scan_space(app: tauri::AppHandle, path: String) -> Result<SpaceView, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("No existe la carpeta {path}"));
    }
    CANCEL.store(false, Ordering::SeqCst);
    let counters = Arc::new(Counters::default());
    let done = Arc::new(AtomicBool::new(false));
    {
        let (counters, done, app) = (counters.clone(), done.clone(), app.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(250));
                let _ = app.emit(
                    "space-progress",
                    Progress { files: counters.files.load(Ordering::Relaxed), bytes: counters.bytes.load(Ordering::Relaxed) },
                );
            }
        });
    }
    let start = Instant::now();
    let (tree, mut top) = walk(&root, &counters);
    done.store(true, Ordering::SeqCst);
    if CANCEL.load(Ordering::SeqCst) {
        return Err("Análisis cancelado.".into());
    }
    top.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
    top.truncate(TOP_FILES);
    let seconds = start.elapsed().as_secs_f64();
    log::info!("Análisis de espacio de {path}: {} archivos, {} bytes en {seconds:.1} s", tree.files, tree.size);

    let view = SpaceView {
        root: root.display().to_string(),
        size: tree.size,
        files: tree.files,
        denied: counters.denied.load(Ordering::Relaxed),
        seconds,
        children: entries_of(&tree, &root),
        largest_files: top.iter().take(50).map(file_entry).collect(),
        // Grandes y sin tocar en más de un año: lo primero que se ofrece borrar.
        old_files: {
            let year_ago = now().saturating_sub(365 * 24 * 3600);
            top.iter().filter(|(_, _, m)| *m > 0 && *m < year_ago).take(50).map(file_entry).collect()
        },
    };
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(Scan { root, tree, top });
    Ok(view)
}

/// Un sitio del que se puede recuperar espacio.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Freeable {
    /// Clave estable (la usa la interfaz para saber qué ajuste lo limpia).
    key: &'static str,
    name: String,
    /// Qué es y si se puede borrar sin pensarlo.
    detail: &'static str,
    size: u64,
    /// Se puede borrar sin consecuencias para el usuario.
    safe: bool,
}

/// Tamaño de un archivo suelto (hiberfil.sys y compañía).
fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// Dónde se acumula el espacio recuperable en un Windows normal. Se mide de
/// verdad (no se estima) y en paralelo, porque son carpetas grandes.
#[tauri::command(async)]
pub fn space_freeable() -> Vec<Freeable> {
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let user = crate::target_user::user_dirs();

    let mut places: Vec<(&'static str, String, &'static str, PathBuf, bool)> = vec![
        ("windows-temp", "Temporales de Windows".into(), "Archivos de trabajo del sistema. Se pueden borrar.", PathBuf::from(&windows).join("Temp"), true),
        (
            "update-cache",
            "Caché de Windows Update".into(),
            "Instaladores ya aplicados. Se vuelven a descargar si hacen falta.",
            PathBuf::from(&windows).join("SoftwareDistribution").join("Download"),
            true,
        ),
        ("recycle-bin", "Papelera de reciclaje".into(), "Lo que se borró y aún se puede recuperar. Míralo antes.", PathBuf::from(format!("{drive}\\$Recycle.Bin")), false),
        (
            "windows-old",
            "Windows.old".into(),
            "La versión anterior de Windows, para volver atrás. Se borra sola a los 10 días.",
            PathBuf::from(format!("{drive}\\Windows.old")),
            false,
        ),
    ];
    if let Some(u) = &user {
        places.push(("user-temp", "Temporales de tu usuario".into(), "Archivos de trabajo de los programas. Se pueden borrar.", u.temp.clone(), true));
        places.push(("downloads", "Descargas".into(), "Lo que se ha descargado. Revísalo antes de borrar.", u.profile.join("Downloads"), false));
    }

    let mut out: Vec<Freeable> = places
        .par_iter()
        .filter(|(_, _, _, p, _)| p.is_dir())
        .map(|(key, name, detail, p, safe)| Freeable {
            key,
            name: name.clone(),
            detail,
            size: folder_size(p).0,
            safe: *safe,
        })
        .filter(|f| f.size > 0)
        .collect();

    // La hibernación es un archivo suelto del tamaño de la memoria.
    let hiber = PathBuf::from(format!("{drive}\\hiberfil.sys"));
    let size = file_size(&hiber);
    if size > 0 {
        out.push(Freeable {
            key: "hibernation",
            name: "Archivo de hibernación".into(),
            detail: "Ocupa tanto como la memoria RAM. Quitarlo desactiva la hibernación y el inicio rápido.",
            size,
            safe: false,
        });
    }
    out.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));
    out
}

#[tauri::command]
pub fn cancel_space_scan() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Subcarpetas de `path` dentro del último análisis.
#[tauri::command]
pub fn space_children(path: String) -> Result<Vec<SpaceEntry>, String> {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
    let rel = Path::new(&path).strip_prefix(&scan.root).map_err(|_| "Ruta fuera del análisis".to_string())?;
    let mut node = &scan.tree;
    for part in rel.components() {
        let name = part.as_os_str().to_string_lossy();
        node = node.children.iter().find(|c| c.name == name).ok_or("Carpeta no encontrada en el análisis")?;
    }
    Ok(entries_of(node, Path::new(&path)))
}

/// ¿Está esta ruta dentro del último análisis? Lo que la interfaz envía solo
/// puede ser algo que el propio análisis encontró: nunca una ruta cualquiera.
fn in_last_scan(path: &str) -> Result<(), String> {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
    let inside = Path::new(path).starts_with(&scan.root) || scan.top.iter().any(|(_, p, _)| p.as_os_str() == path);
    if inside { Ok(()) } else { Err("Ruta fuera del análisis".into()) }
}

/// Manda a la papelera lo que el técnico haya marcado en el análisis. A la
/// papelera, no borrado directo: delante de un cliente hay que poder volver atrás.
#[tauri::command(async)]
pub fn space_recycle(paths: Vec<String>, state: tauri::State<'_, crate::tweaks::TweakState>) -> Result<u64, String> {
    if paths.is_empty() {
        return Ok(0);
    }
    if paths.len() > 500 {
        return Err("Como mucho 500 elementos a la vez.".into());
    }
    for p in &paths {
        in_last_scan(p)?;
    }
    // El tamaño se mide antes de borrar, para poder decir cuánto se liberó.
    let freed: u64 = paths
        .iter()
        .map(|p| {
            let path = Path::new(p);
            if path.is_dir() { folder_size(path).0 } else { file_size(path) }
        })
        .sum();
    let script = format!(
        "{}Add-Type -AssemblyName Microsoft.VisualBasic\nforeach ($p in @(ConvertFrom-Json $json)) {{\n  if (Test-Path -LiteralPath $p -PathType Container) {{ [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($p, 'OnlyErrorDialogs', 'SendToRecycleBin') }}\n  elseif (Test-Path -LiteralPath $p) {{ [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($p, 'OnlyErrorDialogs', 'SendToRecycleBin') }}\n}}\n'ok'",
        crate::ps::text_var("json", &serde_json::to_string(&paths).unwrap_or_default())
    );
    let result = crate::pspool::query(&script, Some(Duration::from_secs(300)), "Espacio: a la papelera").map(|_| ());
    let n = paths.len();
    state.record(crate::tweaks::journal::Op::Run, &format!("Espacio: {n} elemento(s) a la papelera"), &result);
    result?;
    Ok(freed)
}

/// Muestra un archivo o carpeta del último análisis en el Explorador.
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    {
        let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
        let in_scan = Path::new(&path).starts_with(&scan.root) || scan.top.iter().any(|(_, p, _)| p.as_os_str() == path.as_str());
        if !in_scan {
            return Err("Ruta fuera del análisis".into());
        }
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_add_up() {
        let dir = std::env::temp_dir().join(format!("adminops-space-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("x.bin"), vec![0u8; 1000]).unwrap();
        std::fs::write(dir.join("a/y.bin"), vec![0u8; 2_000_000]).unwrap();
        std::fs::write(dir.join("a/b/z.bin"), vec![0u8; 300]).unwrap();
        let c = Counters::default();
        let (tree, mut top) = walk(&dir, &c);
        top.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
        assert_eq!(tree.size, 2_001_300);
        assert_eq!(tree.files, 3);
        assert_eq!(tree.children.len(), 1, "solo 'a' supera 1 MB");
        assert_eq!(top[0].0, 2_000_000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Mide un análisis real: `cargo test --release space_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn space_real() {
        let c = Counters::default();
        let t = Instant::now();
        let (tree, _) = walk(Path::new("C:\\Windows"), &c);
        println!("C:\\Windows: {} archivos, {:.1} GB en {:.1} s", tree.files, tree.size as f64 / 1e9, t.elapsed().as_secs_f64());
    }
}
