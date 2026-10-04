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

/// Tipos de archivo, para ver de qué está lleno un disco. El último es «otros».
const KINDS: [&str; 9] = ["video", "image", "audio", "doc", "mail", "archive", "disk", "program", "other"];
const OTHER: usize = KINDS.len() - 1;
/// Archivos que se guardan de cada tipo (los más grandes), y a partir de qué tamaño.
const KIND_TOP: usize = 60;
const KIND_MIN: u64 = 1024 * 1024;
/// Archivos que se enseñan de una carpeta.
const FOLDER_FILES: usize = 300;

/// A qué tipo pertenece un archivo, por su extensión.
fn kind_of(name: &str) -> usize {
    let Some((_, ext)) = name.rsplit_once('.') else { return OTHER };
    if ext.len() > 6 {
        return OTHER;
    }
    match ext.to_ascii_lowercase().as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" | "vob" | "mts" | "m2ts" | "3gp" => 0,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tif" | "tiff" | "heic" | "webp" | "raw" | "cr2" | "nef" | "arw" | "dng" | "psd" | "svg" | "ico" => 1,
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "aiff" => 2,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "xlsm" | "ppt" | "pptx" | "odt" | "ods" | "odp" | "txt" | "rtf" | "csv" | "one" | "mdb" | "accdb" | "pub" | "vsdx" => 3,
        "pst" | "ost" | "msg" | "eml" | "mbox" | "nst" => 4,
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "cab" | "tgz" => 5,
        "iso" | "img" | "msi" | "msp" | "msu" | "vhd" | "vhdx" | "vmdk" | "vdi" | "wim" | "esd" | "dmg" => 6,
        "exe" | "dll" | "sys" | "ocx" | "drv" | "mui" | "pak" | "bin" | "dat" => 7,
        _ => OTHER,
    }
}

struct Node {
    name: String,
    size: u64,
    files: u64,
    /// Lo que ocupa cada tipo de archivo dentro (mismo orden que `KINDS`).
    kinds: [u64; KINDS.len()],
    /// El cambio más reciente de cualquier archivo de dentro (segundos desde 1970).
    modified: u64,
    children: Vec<Node>,
}

/// Lo que un recorrido va apartando: los archivos más grandes, en general y por tipo.
#[derive(Default)]
struct Tops {
    all: Vec<TopFile>,
    by_kind: [Vec<TopFile>; KINDS.len()],
}

fn trim(list: &mut Vec<TopFile>, keep: usize) {
    if list.len() > keep * 2 {
        list.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
        list.truncate(keep);
    }
}

impl Tops {
    fn absorb(&mut self, other: Tops) {
        self.all.extend(other.all);
        trim(&mut self.all, TOP_FILES);
        for (mine, theirs) in self.by_kind.iter_mut().zip(other.by_kind) {
            if !theirs.is_empty() {
                mine.extend(theirs);
                trim(mine, KIND_TOP);
            }
        }
    }

    /// Ordena y deja solo lo que se enseña.
    fn finish(&mut self) {
        self.all.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
        self.all.truncate(TOP_FILES);
        for k in &mut self.by_kind {
            k.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
            k.truncate(KIND_TOP);
        }
    }

    /// Olvida lo que estaba en `path` (un archivo, o una carpeta con todo lo suyo).
    fn forget(&mut self, path: &Path) {
        self.all.retain(|(_, p, _)| !p.starts_with(path));
        for k in &mut self.by_kind {
            k.retain(|(_, p, _)| !p.starts_with(path));
        }
    }
}

struct Scan {
    root: PathBuf,
    tree: Node,
    tops: Tops,
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

fn walk(path: &Path, c: &Counters) -> (Node, Tops) {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let mut node = Node { name, size: 0, files: 0, kinds: [0; KINDS.len()], modified: 0, children: vec![] };
    let mut top = Tops::default();
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
            let kind = kind_of(&e.name);
            node.size += len;
            node.files += 1;
            node.kinds[kind] += len;
            node.modified = node.modified.max(e.modified);
            let file = path.join(&e.name);
            if len >= KIND_MIN {
                top.by_kind[kind].push((len, file.clone(), e.modified));
            }
            top.all.push((len, file, e.modified));
            c.files.fetch_add(1, Ordering::Relaxed);
            c.bytes.fetch_add(len, Ordering::Relaxed);
        }
    }
    if top.all.len() > TOP_FILES {
        top.all.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
        top.all.truncate(TOP_FILES);
    }
    let results: Vec<(Node, Tops)> = subdirs.par_iter().map(|d| walk(d, c)).collect();
    for (child, child_top) in results {
        node.size += child.size;
        node.files += child.files;
        node.modified = node.modified.max(child.modified);
        for (mine, theirs) in node.kinds.iter_mut().zip(child.kinds) {
            *mine += theirs;
        }
        top.absorb(child_top);
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
    /// Última modificación: la del archivo o, en una carpeta, la más reciente de dentro.
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<u64>,
}

/// Lo que ocupa un tipo de archivo.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindSize {
    id: &'static str,
    size: u64,
}

/// Una carpeta del análisis: sus subcarpetas y de qué está llena.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceFolder {
    path: String,
    size: u64,
    files: u64,
    children: Vec<SpaceEntry>,
    kinds: Vec<KindSize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceView {
    root: String,
    size: u64,
    files: u64,
    denied: u64,
    seconds: f64,
    folder: SpaceFolder,
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
            modified: Some(c.modified).filter(|m| *m > 0),
        })
        .collect()
}

fn folder_of(node: &Node, path: &Path) -> SpaceFolder {
    SpaceFolder {
        path: path.display().to_string(),
        size: node.size,
        files: node.files,
        children: entries_of(node, path),
        kinds: KINDS.iter().zip(node.kinds).filter(|(_, size)| *size > 0).map(|(id, size)| KindSize { id, size }).collect(),
    }
}

/// Las partes de `path` por debajo de la raíz del análisis.
fn parts_under(root: &Path, path: &Path) -> Result<Vec<String>, String> {
    let rel = path.strip_prefix(root).map_err(|_| "Ruta fuera del análisis".to_string())?;
    Ok(rel.components().map(|p| p.as_os_str().to_string_lossy().into_owned()).collect())
}

fn node_at<'a>(scan: &'a Scan, path: &Path) -> Result<&'a Node, String> {
    let mut node = &scan.tree;
    for name in parts_under(&scan.root, path)? {
        node = node.children.iter().find(|c| c.name == name).ok_or("Carpeta no encontrada en el análisis")?;
    }
    Ok(node)
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
    let (tree, mut tops) = walk(&root, &counters);
    done.store(true, Ordering::SeqCst);
    crate::task::notify_if_long(&app, "Análisis de espacio", start.elapsed(), CANCEL.load(Ordering::SeqCst));
    if CANCEL.load(Ordering::SeqCst) {
        return Err("Análisis cancelado.".into());
    }
    tops.finish();
    let top = &tops.all;
    let seconds = start.elapsed().as_secs_f64();
    log::info!("Análisis de espacio de {path}: {} archivos, {} bytes en {seconds:.1} s", tree.files, tree.size);

    let view = SpaceView {
        root: root.display().to_string(),
        size: tree.size,
        files: tree.files,
        denied: counters.denied.load(Ordering::Relaxed),
        seconds,
        folder: folder_of(&tree, &root),
        largest_files: top.iter().take(50).map(file_entry).collect(),
        // Grandes y sin tocar en más de un año: lo primero que se ofrece borrar.
        old_files: {
            let year_ago = now().saturating_sub(365 * 24 * 3600);
            top.iter().filter(|(_, _, m)| *m > 0 && *m < year_ago).take(50).map(file_entry).collect()
        },
    };
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(Scan { root, tree, tops });
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

/// Una carpeta del último análisis: sus subcarpetas y sus tipos de archivo.
#[tauri::command(async)]
pub fn space_folder(path: String) -> Result<SpaceFolder, String> {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
    let path = Path::new(&path);
    Ok(folder_of(node_at(scan, path)?, path))
}

/// Los archivos sueltos de una carpeta del análisis, del más grande al más
/// pequeño. Se leen en el momento: el análisis solo guarda carpetas.
#[tauri::command(async)]
pub fn space_files(path: String) -> Result<Vec<SpaceEntry>, String> {
    in_last_scan(&path)?;
    let dir = Path::new(&path);
    let mut files: Vec<TopFile> = list_dir(dir)
        .ok_or("No se puede leer esa carpeta.")?
        .into_iter()
        .filter(|e| !e.is_dir && !e.is_reparse)
        .map(|e| (e.size, dir.join(&e.name), e.modified))
        .collect();
    files.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));
    Ok(files.iter().take(FOLDER_FILES).map(file_entry).collect())
}

/// Los archivos más grandes de un tipo (vídeo, correo…) en el último análisis.
#[tauri::command(async)]
pub fn space_kind_files(kind: String) -> Result<Vec<SpaceEntry>, String> {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
    let i = KINDS.iter().position(|k| *k == kind).ok_or("Tipo de archivo desconocido")?;
    Ok(scan.tops.by_kind[i].iter().map(file_entry).collect())
}

/// ¿Está esta ruta dentro del último análisis? Lo que la interfaz envía solo
/// puede ser algo que el propio análisis encontró: nunca una ruta cualquiera.
fn in_last_scan(path: &str) -> Result<(), String> {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
    if Path::new(path).starts_with(&scan.root) { Ok(()) } else { Err("Ruta fuera del análisis".into()) }
}

/// Las carpetas de las que depende Windows, en minúsculas y sin barra final.
struct Protected {
    windows: String,
    /// Archivos de programa y datos de programa: ni ellas ni lo que cuelga directamente.
    programs: Vec<String>,
    /// La carpeta de los perfiles: ni ella ni un perfil entero.
    users: String,
}

impl Protected {
    fn here() -> Self {
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.into()).trim_end_matches('\\').to_lowercase();
        let drive = var("SystemDrive", "C:");
        Protected {
            windows: var("SystemRoot", r"C:\Windows"),
            programs: vec![var("ProgramFiles", r"C:\Program Files"), var("ProgramFiles(x86)", r"C:\Program Files (x86)"), var("ProgramData", r"C:\ProgramData")],
            users: format!(r"{drive}\users"),
        }
    }

    /// Por qué no se puede mandar esto a la papelera desde aquí, si no se puede.
    fn reason(&self, path: &str) -> Option<&'static str> {
        let p = path.trim_end_matches('\\').to_lowercase();
        let inside = |base: &str| p.strip_prefix(base).is_some_and(|rest| rest.starts_with('\\'));
        // Lo que cuelga directamente de `base` (un solo nivel).
        let child_of = |base: &str| p.strip_prefix(base).and_then(|rest| rest.strip_prefix('\\')).is_some_and(|rest| !rest.contains('\\'));
        if p.len() <= 2 {
            return Some("Es una unidad entera.");
        }
        if p == self.windows || inside(&self.windows) {
            return Some("Es de Windows: lo que sobra ahí se quita desde Limpieza.");
        }
        if matches!(p.rsplit('\\').next(), Some("pagefile.sys" | "hiberfil.sys" | "swapfile.sys")) && p.len() <= 16 {
            return Some("Es un archivo del sistema: se cambia desde los ajustes de Windows, no borrándolo.");
        }
        if self.programs.iter().any(|d| p == *d || child_of(d)) {
            return Some("Es un programa instalado: se quita desde Programas, desinstalándolo.");
        }
        if p == self.users || child_of(&self.users) {
            return Some("Es un perfil de usuario entero: se quita desde Usuarios y cuentas.");
        }
        None
    }
}

/// Quita del análisis algo que ya no está en el disco, para que las cifras
/// sigan cuadrando sin volver a analizar. `fallback` es lo que medía si no era
/// una carpeta guardada (un archivo, o una carpeta de menos de 1 MB).
fn forget(scan: &mut Scan, path: &Path, fallback: (u64, u64)) {
    let Ok(parts) = parts_under(&scan.root, path) else { return };
    if parts.is_empty() {
        return;
    }
    scan.tops.forget(path);
    // Lo que se va: si era una carpeta guardada, sus propias cifras.
    let mut gone = (fallback.0, fallback.1, [0u64; KINDS.len()]);
    match node_at(scan, path) {
        Ok(n) => gone = (n.size, n.files, n.kinds),
        Err(_) => gone.2[kind_of(parts.last().map_or("", String::as_str))] = fallback.0,
    }
    let mut node = &mut scan.tree;
    for (i, name) in parts.iter().enumerate() {
        node.size = node.size.saturating_sub(gone.0);
        node.files = node.files.saturating_sub(gone.1);
        for (mine, theirs) in node.kinds.iter_mut().zip(gone.2) {
            *mine = mine.saturating_sub(theirs);
        }
        if i == parts.len() - 1 {
            node.children.retain(|c| c.name != *name);
            return;
        }
        let Some(next) = node.children.iter_mut().find(|c| c.name == *name) else { return };
        node = next;
    }
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
    let protected = Protected::here();
    for p in &paths {
        in_last_scan(p)?;
        if let Some(why) = protected.reason(p) {
            let name = Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
            return Err(format!("«{name}» no se manda a la papelera desde aquí. {why}"));
        }
    }
    // El tamaño se mide antes de borrar, para poder decir cuánto se liberó.
    let sizes: Vec<(u64, u64)> = paths
        .iter()
        .map(|p| {
            let path = Path::new(p);
            if path.is_dir() { folder_size(path) } else { (file_size(path), 1) }
        })
        .collect();
    let script = format!(
        "{}Add-Type -AssemblyName Microsoft.VisualBasic\nforeach ($p in @(ConvertFrom-Json $json)) {{\n  if (Test-Path -LiteralPath $p -PathType Container) {{ [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($p, 'OnlyErrorDialogs', 'SendToRecycleBin') }}\n  elseif (Test-Path -LiteralPath $p) {{ [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($p, 'OnlyErrorDialogs', 'SendToRecycleBin') }}\n}}\n'ok'",
        crate::ps::text_var("json", &serde_json::to_string(&paths).unwrap_or_default())
    );
    let result = crate::pspool::query(&script, Some(Duration::from_secs(300)), "Espacio: a la papelera").map(|_| ());
    let n = paths.len();
    state.record(crate::tweaks::journal::Op::Run, &format!("Espacio: {n} elemento(s) a la papelera"), &result);
    // Lo que de verdad se fue (Windows puede dejar algo en uso) sale del análisis.
    let mut freed = 0;
    if let Some(scan) = LAST.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        for (p, size) in paths.iter().zip(&sizes) {
            let path = Path::new(p);
            if !path.exists() {
                freed += size.0;
                forget(scan, path, *size);
            }
        }
    }
    result?;
    Ok(freed)
}

/// Muestra un archivo o carpeta del último análisis en el Explorador.
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    {
        let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        let scan = last.as_ref().ok_or("No hay ningún análisis.")?;
        if !Path::new(&path).starts_with(&scan.root) {
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
        let (tree, mut tops) = walk(&dir, &c);
        tops.finish();
        assert_eq!(tree.size, 2_001_300);
        assert_eq!(tree.files, 3);
        assert_eq!(tree.children.len(), 1, "solo 'a' supera 1 MB");
        assert_eq!(tops.all[0].0, 2_000_000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn knows_what_a_folder_is_full_of() {
        assert_eq!(KINDS[kind_of("Boda.MP4")], "video");
        assert_eq!(KINDS[kind_of("archivo.pst")], "mail");
        assert_eq!(KINDS[kind_of("Windows11.iso")], "disk");
        assert_eq!(KINDS[kind_of("sin-extension")], "other");
        assert_eq!(KINDS[kind_of("raro.extensionlarga")], "other");

        let dir = std::env::temp_dir().join(format!("adminops-kinds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("fotos")).unwrap();
        std::fs::write(dir.join("fotos/a.jpg"), vec![0u8; 1_500_000]).unwrap();
        std::fs::write(dir.join("fotos/b.mp4"), vec![0u8; 3_000_000]).unwrap();
        std::fs::write(dir.join("c.pdf"), vec![0u8; 500]).unwrap();
        let (tree, mut tops) = walk(&dir, &Counters::default());
        tops.finish();
        let of = |n: &Node, k: &str| n.kinds[KINDS.iter().position(|x| *x == k).unwrap()];
        assert_eq!(of(&tree, "video"), 3_000_000);
        assert_eq!(of(&tree, "image"), 1_500_000);
        assert_eq!(of(&tree, "doc"), 500);
        assert_eq!(tree.kinds.iter().sum::<u64>(), tree.size, "los tipos suman el total");
        assert!(tree.modified > 0 && tree.children[0].modified > 0);
        // Por tipo solo se apartan los de 1 MB o más.
        assert_eq!(tops.by_kind[0].len(), 1);
        assert!(tops.by_kind[3].is_empty());

        // Se borra el vídeo: las cifras de toda la rama bajan sin volver a analizar.
        let mut scan = Scan { root: dir.clone(), tree, tops };
        forget(&mut scan, &dir.join("fotos/b.mp4"), (3_000_000, 1));
        assert_eq!(scan.tree.size, 1_500_500);
        assert_eq!(scan.tree.files, 2);
        assert_eq!(of(&scan.tree, "video"), 0);
        assert_eq!(scan.tree.children[0].size, 1_500_000);
        assert!(scan.tops.all.iter().all(|(_, p, _)| !p.ends_with("b.mp4")));
        // Se borra la carpeta entera: desaparece con lo suyo.
        forget(&mut scan, &dir.join("fotos"), (0, 0));
        assert_eq!(scan.tree.size, 500);
        assert!(scan.tree.children.is_empty());
        assert_eq!(of(&scan.tree, "image"), 0);
        assert_eq!(scan.tops.all.len(), 1);
        // Algo de fuera del análisis no toca nada.
        forget(&mut scan, Path::new(r"Z:\otra\cosa"), (999, 9));
        assert_eq!(scan.tree.size, 500);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_what_windows_depends_on() {
        let p = Protected {
            windows: r"c:\windows".into(),
            programs: vec![r"c:\program files".into(), r"c:\programdata".into()],
            users: r"c:\users".into(),
        };
        for no in [
            r"C:\",
            "C:",
            r"C:\Windows",
            r"C:\Windows\System32\drivers",
            r"C:\pagefile.sys",
            r"C:\Program Files",
            r"C:\Program Files\Mozilla Firefox",
            r"C:\ProgramData\Microsoft",
            r"C:\Users",
            r"C:\Users\ana",
        ] {
            assert!(p.reason(no).is_some(), "{no}");
        }
        for yes in [
            r"C:\Users\ana\Downloads",
            r"C:\Users\ana\Videos\boda.mp4",
            r"C:\Windows.old",
            r"C:\Program Files\Juego\capturas\viejas",
            r"D:\Copias\2019",
            r"C:\Datos\pagefile.sys",
        ] {
            assert!(p.reason(yes).is_none(), "{yes}");
        }
    }

    /// Mide un análisis real: `cargo test --release space_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn space_real() {
        let c = Counters::default();
        let t = Instant::now();
        let (tree, _) = walk(Path::new("C:\\Windows"), &c);
        // Con los tipos de archivo: mismo tiempo que antes, dentro del margen.
        println!("C:\\Windows: {} archivos, {:.1} GB en {:.1} s", tree.files, tree.size as f64 / 1e9, t.elapsed().as_secs_f64());
    }
}
