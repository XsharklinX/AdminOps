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
    top: Vec<(u64, PathBuf)>,
}

#[derive(Default)]
struct Counters {
    files: AtomicU64,
    bytes: AtomicU64,
    denied: AtomicU64,
}

struct RawEntry {
    name: String,
    is_dir: bool,
    is_reparse: bool,
    size: u64,
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

fn merge_top(a: &mut Vec<(u64, PathBuf)>, b: Vec<(u64, PathBuf)>) {
    a.extend(b);
    if a.len() > TOP_FILES * 2 {
        a.sort_unstable_by(|x, y| y.0.cmp(&x.0));
        a.truncate(TOP_FILES);
    }
}

fn walk(path: &Path, c: &Counters) -> (Node, Vec<(u64, PathBuf)>) {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let mut node = Node { name, size: 0, files: 0, children: vec![] };
    let mut top = Vec::new();
    if CANCEL.load(Ordering::Relaxed) {
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
            top.push((len, path.join(&e.name)));
            c.files.fetch_add(1, Ordering::Relaxed);
            c.bytes.fetch_add(len, Ordering::Relaxed);
        }
    }
    if top.len() > TOP_FILES {
        top.sort_unstable_by(|x, y| y.0.cmp(&x.0));
        top.truncate(TOP_FILES);
    }
    let results: Vec<(Node, Vec<(u64, PathBuf)>)> = subdirs.par_iter().map(|d| walk(d, c)).collect();
    for (child, child_top) in results {
        node.size += child.size;
        node.files += child.files;
        merge_top(&mut top, child_top);
        if child.size >= MIN_NODE {
            node.children.push(child);
        }
    }
    node.children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    (node, top)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceEntry {
    name: String,
    path: String,
    size: u64,
    files: u64,
    has_children: bool,
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
        })
        .collect()
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
    top.sort_unstable_by(|x, y| y.0.cmp(&x.0));
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
        largest_files: top
            .iter()
            .take(50)
            .map(|(size, p)| SpaceEntry {
                name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                path: p.display().to_string(),
                size: *size,
                files: 1,
                has_children: false,
            })
            .collect(),
    };
    *LAST.lock().unwrap() = Some(Scan { root, tree, top });
    Ok(view)
}

#[tauri::command]
pub fn cancel_space_scan() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Subcarpetas de `path` dentro del último análisis.
#[tauri::command]
pub fn space_children(path: String) -> Result<Vec<SpaceEntry>, String> {
    let last = LAST.lock().unwrap();
    let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
    let rel = Path::new(&path).strip_prefix(&scan.root).map_err(|_| "Ruta fuera del análisis".to_string())?;
    let mut node = &scan.tree;
    for part in rel.components() {
        let name = part.as_os_str().to_string_lossy();
        node = node.children.iter().find(|c| c.name == name).ok_or("Carpeta no encontrada en el análisis")?;
    }
    Ok(entries_of(node, Path::new(&path)))
}

/// Muestra un archivo o carpeta del último análisis en el Explorador.
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    {
        let last = LAST.lock().unwrap();
        let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
        let in_scan = Path::new(&path).starts_with(&scan.root) || scan.top.iter().any(|(_, p)| p.as_os_str() == path.as_str());
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
        top.sort_unstable_by(|x, y| y.0.cmp(&x.0));
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
