//! «Lo que no se guardó» (Datos → Recuperar archivos → Sin guardar de Office).
//!
//! Solo lee: busca lo que Word, Excel y PowerPoint dejan por si se cierran mal (las carpetas de
//! autorrecuperación que dice el registro del usuario, los «sin guardar», los temporales de
//! `%TEMP%`), enseña una vista previa y, si Windows tiene copias de «Versiones anteriores»
//! (instantáneas VSS), dice en cuáles está un archivo. Lo único que escribe es «Abrir una copia»:
//! copia el archivo al Escritorio con otro nombre y lo abre. El original no se toca nunca.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

const MAX_FILES: usize = 200;
const MAX_DEPTH: usize = 2;
/// Lo que más se lee de un archivo para la vista previa (un documento de Office es un zip).
const MAX_PREVIEW_BYTES: u64 = 8 * 1024 * 1024;
const PREVIEW_LINES: usize = 8;
const PREVIEW_WIDTH: usize = 140;

// ---------- Lo que se enseña ----------

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnsavedFile {
    /// Ruta completa: la interfaz la devuelve para pedir la vista previa o la copia.
    pub path: String,
    /// El nombre limpio («Presupuesto 2027.xlsx»).
    pub name: String,
    /// Word, Excel, PowerPoint u Office (si no se sabe).
    pub app: String,
    /// Autorrecuperación, Sin guardar, Copia de seguridad o Temporal.
    pub origin: String,
    pub size: u64,
    /// Segundos desde 1970.
    pub modified: u64,
    /// Extensión en minúsculas, sin punto.
    pub ext: String,
    /// La extensión con la que se abrirá la copia (`docx`, `xlsx`…), si se sabe.
    pub copy_ext: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentDoc {
    pub name: String,
    pub path: String,
    pub app: String,
    /// Cuándo se abrió por última vez (segundos desde 1970).
    pub opened: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UnsavedList {
    pub files: Vec<UnsavedFile>,
    pub recent: Vec<RecentDoc>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// Identificador de la instantánea.
    pub id: String,
    /// Cuándo se hizo la instantánea (segundos desde 1970).
    pub when: u64,
    pub size: u64,
    /// Cuándo se modificó el archivo en esa instantánea.
    pub modified: u64,
    #[serde(skip)]
    device: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VersionList {
    pub versions: Vec<Version>,
    /// Cuántas instantáneas tiene esa unidad (0: no hay «Versiones anteriores»).
    pub snapshots: usize,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub lines: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    /// El nombre de la copia en el Escritorio.
    pub name: String,
    /// Si se abrió con su programa (si no, se enseña en el Explorador).
    pub opened: bool,
    pub message: String,
}

// ---------- Puro, con pruebas ----------

/// Expande `%NOMBRE%` con lo que devuelve `lookup`; `None` si alguna no existe.
pub fn expand_env_with(s: &str, lookup: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let j = after.find('%')?;
        let name = &after[..j];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        out.push_str(&lookup(name)?);
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// Expande `%NOMBRE%` como lo vería el usuario con la sesión abierta: las carpetas de su perfil salen de
/// `dirs` (si AdminOps se elevó con otra cuenta, las variables del proceso serían las de esa cuenta).
pub fn expand_env_for_user(s: &str, dirs: Option<&crate::target_user::UserDirs>) -> Option<String> {
    expand_env_with(s, &|name| {
        if let Some(d) = dirs {
            let p = match name.to_ascii_uppercase().as_str() {
                "APPDATA" => Some(&d.app_data),
                "LOCALAPPDATA" => Some(&d.local_app_data),
                "TEMP" | "TMP" => Some(&d.temp),
                "USERPROFILE" => Some(&d.profile),
                _ => None,
            };
            if let Some(p) = p {
                return Some(p.to_string_lossy().into_owned());
            }
        }
        std::env::var(name).ok()
    })
}

/// Ruta local de disco (`C:\…`), sin `..` ni caracteres que no puede tener un nombre de archivo.
pub fn valid_local_path(p: &str) -> bool {
    let b = p.as_bytes();
    p.len() <= 520
        && b.len() >= 4
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && !p.chars().any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
        && !p.split('\\').any(|seg| seg == ".." || seg == ".")
}

/// El nombre sin los prefijos con que Office renombra sus archivos de recuperación.
pub fn clean_name(name: &str) -> String {
    let mut n = name.trim().trim_start_matches('~').trim().to_string();
    for prefix in ["Autorrecuperación de ", "AutoRecovery save of ", "Copia de seguridad de ", "Backup of "] {
        if n.len() > prefix.len() && n.is_char_boundary(prefix.len()) && n[..prefix.len()].eq_ignore_ascii_case(prefix) {
            n = n[prefix.len()..].trim().to_string();
            break;
        }
    }
    if n.is_empty() {
        name.to_string()
    } else {
        n
    }
}

/// Qué programa es, por la carpeta en la que estaba y la extensión.
pub fn infer_app(folder_app: &str, ext: &str) -> &'static str {
    match ext {
        "docx" | "doc" | "docm" | "wbk" => "Word",
        "xlsx" | "xls" | "xlsb" | "xlsm" | "xar" => "Excel",
        "pptx" | "ppt" | "pptm" => "PowerPoint",
        // Un .asd suelto (en «Sin guardar», que no es de un programa) casi siempre es de Word.
        "asd" if !APPS.contains(&folder_app) => "Word",
        _ => match folder_app {
            "Word" => "Word",
            "Excel" => "Excel",
            "PowerPoint" => "PowerPoint",
            _ => "Office",
        },
    }
}

/// La extensión de la copia. Un `.asd` (autorrecuperación) y un `.xar` (Excel) son formatos propios de
/// Office, no un `.docx` ni un `.xlsx` con otro nombre: conservan su extensión y se abren con su programa.
/// Un `.wbk` (copia de seguridad de Word) sí es un `.doc`.
pub fn copy_ext(app: &str, ext: &str) -> Option<&'static str> {
    Some(match ext {
        "asd" => "asd",
        "xar" => "xar",
        "wbk" => "doc",
        "tmp" => match app {
            "Word" => "docx",
            "Excel" => "xlsx",
            "PowerPoint" => "pptx",
            _ => return None,
        },
        "docx" => "docx",
        "doc" => "doc",
        "docm" => "docm",
        "xlsx" => "xlsx",
        "xls" => "xls",
        "xlsb" => "xlsb",
        "xlsm" => "xlsm",
        "pptx" => "pptx",
        "ppt" => "ppt",
        "pptm" => "pptm",
        _ => return None,
    })
}

/// ¿Es un formato propio de Office que no abre el programa asociado de Windows (hay que pedírselo a Word o Excel)?
pub fn needs_office_app(ext: &str) -> bool {
    matches!(ext, "asd" | "xar")
}

/// El programa de Office que abre ese archivo, con su ejecutable en `App Paths`.
pub fn office_exe_name(app: &str) -> Option<&'static str> {
    match app {
        "Word" => Some("WINWORD.EXE"),
        "Excel" => Some("EXCEL.EXE"),
        "PowerPoint" => Some("POWERPNT.EXE"),
        _ => None,
    }
}

/// La ruta del programa de Office instalado (Word, Excel, PowerPoint), si está.
fn office_exe(app: &str) -> Option<PathBuf> {
    let exe = office_exe_name(app)?;
    for base in [r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths", r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths"] {
        let Some(v) = crate::tweaks::registry::read_string(&format!(r"{base}\{exe}"), "") else { continue };
        let p = PathBuf::from(v.trim().trim_matches('"'));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Extensiones que tienen sentido en las carpetas de autorrecuperación.
fn recover_ext(ext: &str) -> bool {
    matches!(ext, "asd" | "xar" | "wbk" | "tmp" | "docx" | "doc" | "docm" | "xlsx" | "xls" | "xlsb" | "xlsm" | "pptx" | "ppt" | "pptm")
}

/// ¿Es un temporal de Word dentro de `%TEMP%`? (`~WRD0000.tmp` o un `~$…` con contenido.) Los `~DF….tmp` no
/// cuentan: son temporales genéricos de Windows y llenarían la lista de basura.
pub fn is_office_temp(name: &str, size: u64) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("~$") {
        // Los de bloqueo miden 162 bytes y solo dicen quién tiene el archivo abierto.
        return size > 1024;
    }
    size > 0 && lower.ends_with(".tmp") && lower.starts_with("~wr")
}

/// El nombre de la copia: «Presupuesto (recuperado 11-52).xlsx». Sin caracteres que Windows no admite.
pub fn copy_name(name: &str, ext: &str, hhmm: &str, n: u32) -> String {
    // Sin pasar por `Path`: en Windows «a:b.asd» se tomaría por una unidad.
    let stem = match name.rsplit_once('.') {
        Some((head, _)) if !head.is_empty() => head,
        _ => name,
    };
    let stem: String = stem.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c }).collect();
    let stem: String = stem.trim().trim_end_matches('.').chars().take(120).collect();
    let stem = if stem.is_empty() { "Documento".to_string() } else { stem };
    let extra = if n > 1 { format!(" {n}") } else { String::new() };
    let tail = if ext.is_empty() { String::new() } else { format!(".{ext}") };
    format!("{stem} (recuperado {hhmm}){extra}{tail}")
}

fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Los textos de un XML de Office: lo que hay dentro de las etiquetas `open` (`<w:t`), agrupado por
/// párrafos (cada `end`, `</w:p>`, cierra una línea).
pub fn xml_texts(xml: &str, open: &str, end: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let t = cur.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
        cur.clear();
    };
    while i < xml.len() {
        let Some(lt) = xml[i..].find('<') else { break };
        let tag = &xml[i + lt..];
        if tag.starts_with(end) {
            flush(&mut cur, &mut out);
            i += lt + end.len();
            continue;
        }
        if tag.starts_with(open) && matches!(tag[open.len()..].chars().next(), Some('>') | Some(' ')) {
            let Some(gt) = tag.find('>') else { break };
            if tag[..gt].ends_with('/') {
                i += lt + gt + 1;
                continue;
            }
            let start = gt + 1;
            let Some(close) = tag[start..].find('<') else { break };
            cur.push_str(&decode_entities(&tag[start..start + close]));
            i += lt + start + close;
            continue;
        }
        i += lt + 1;
    }
    flush(&mut cur, &mut out);
    out
}

fn shorten(s: &str) -> String {
    if s.chars().count() <= PREVIEW_WIDTH {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(PREVIEW_WIDTH).collect::<String>().trim_end())
    }
}

/// Del texto de un `.docx`, `.xlsx` o `.pptx` (un zip), las primeras líneas. Vacío si no es de esos.
pub fn zip_preview<R: Read + std::io::Seek>(reader: R) -> Vec<String> {
    let Ok(mut zip) = zip::ZipArchive::new(reader) else { return Vec::new() };
    for (entry, open, end) in [("word/document.xml", "<w:t", "</w:p>"), ("xl/sharedStrings.xml", "<t", "</si>"), ("ppt/slides/slide1.xml", "<a:t", "</a:p>")] {
        let Ok(file) = zip.by_name(entry) else { continue };
        let mut xml = String::new();
        if file.take(MAX_PREVIEW_BYTES).read_to_string(&mut xml).is_err() {
            continue;
        }
        let lines: Vec<String> = xml_texts(&xml, open, end).iter().take(PREVIEW_LINES).map(|l| shorten(l)).collect();
        if !lines.is_empty() {
            return lines;
        }
    }
    Vec::new()
}

/// «[F00000000][T01D8A1B2C3D4E5F6][O00000000]*C:\ruta\archivo.xlsx» → (ruta, segundos desde 1970).
pub fn parse_mru(value: &str) -> Option<(String, u64)> {
    let (head, path) = value.split_once('*')?;
    let path = path.trim().to_string();
    let opened = head
        .split_once("[T")
        .and_then(|(_, r)| r.split_once(']'))
        .and_then(|(hex, _)| u64::from_str_radix(hex, 16).ok())
        .map(|ft| (ft / 10_000_000).saturating_sub(11_644_473_600))
        .unwrap_or(0);
    Some((path, opened))
}

// ---------- Leer carpetas ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    AutoRecover,
    Unsaved,
    Temp,
}

struct Root {
    dir: PathBuf,
    app: &'static str,
    kind: Kind,
}

const APPS: [&str; 3] = ["Word", "Excel", "PowerPoint"];
const OFFICE_VERSIONS: [&str; 5] = ["16.0", "15.0", "14.0", "12.0", "11.0"];

fn roots() -> Vec<Root> {
    let dirs = crate::target_user::user_dirs();
    let mut out: Vec<Root> = Vec::new();
    let mut add = |dir: PathBuf, app: &'static str, kind: Kind| {
        let key = dir.to_string_lossy().to_lowercase();
        if !out.iter().any(|r| r.dir.to_string_lossy().to_lowercase() == key) {
            out.push(Root { dir, app, kind });
        }
    };
    for app in APPS {
        for ver in OFFICE_VERSIONS {
            // El registro del cliente: `registry` redirige HKCU a su colmena si AdminOps se elevó con otra cuenta.
            let key = format!(r"HKCU\Software\Microsoft\Office\{ver}\{app}\Options");
            let value = crate::tweaks::registry::read_raw(&key, "AutoRecoverPath").and_then(|r| crate::tweaks::registry::raw_to_string(&r));
            if let Some(dir) = value.and_then(|v| expand_env_for_user(v.trim(), dirs.as_ref())).filter(|d| !d.trim().is_empty()) {
                add(PathBuf::from(dir), app, Kind::AutoRecover);
            }
        }
        if let Some(d) = &dirs {
            add(d.app_data.join("Microsoft").join(app), app, Kind::AutoRecover);
        }
    }
    if let Some(d) = &dirs {
        add(d.local_app_data.join("Microsoft").join("Office").join("UnsavedFiles"), "Office", Kind::Unsaved);
        add(d.temp.clone(), "Office", Kind::Temp);
    }
    out
}

fn secs(t: std::io::Result<std::time::SystemTime>) -> u64 {
    t.ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

/// Carpetas de arranque de Excel y Word: lo que hay dentro se abre solo, no es una recuperación.
pub fn is_startup_folder(name: &str) -> bool {
    name.eq_ignore_ascii_case("XLSTART") || name.eq_ignore_ascii_case("STARTUP")
}

fn walk(dir: &Path, depth: usize, root: &Root, out: &mut Vec<UnsavedFile>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let path = entry.path();
        if ft.is_dir() {
            // XLSTART y STARTUP guardan libros que Office abre solo (PERSONAL.XLSB…): no son nada sin guardar.
            if is_startup_folder(&entry.file_name().to_string_lossy()) {
                continue;
            }
            // El temporal de Windows tiene miles de carpetas: ahí solo se mira el primer nivel.
            if depth < MAX_DEPTH && root.kind != Kind::Temp {
                walk(&path, depth + 1, root, out);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
        let Ok(meta) = entry.metadata() else { continue };
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let keep = match root.kind {
            Kind::Temp => is_office_temp(&file_name, meta.len()),
            _ => recover_ext(&ext) && !(file_name.starts_with("~$") && meta.len() <= 1024),
        };
        if !keep {
            continue;
        }
        let app = infer_app(root.app, &ext);
        let origin = if ext == "wbk" {
            "Copia de seguridad"
        } else {
            match root.kind {
                Kind::AutoRecover => "Autorrecuperación",
                Kind::Unsaved => "Sin guardar",
                Kind::Temp => "Temporal",
            }
        };
        out.push(UnsavedFile {
            path: path.to_string_lossy().into_owned(),
            name: clean_name(&file_name),
            app: app.to_string(),
            origin: origin.to_string(),
            size: meta.len(),
            modified: secs(meta.modified()),
            copy_ext: copy_ext(app, &ext).map(str::to_string),
            ext,
        });
    }
}

/// Todo lo que Office dejó por si se cerraba mal, lo más reciente primero.
pub fn scan() -> Vec<UnsavedFile> {
    let mut files = Vec::new();
    for root in roots() {
        walk(&root.dir, 0, &root, &mut files);
    }
    files.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    let mut seen = std::collections::HashSet::new();
    files.retain(|f| seen.insert(f.path.to_lowercase()));
    files.truncate(MAX_FILES);
    files
}

/// Los documentos que Office dice haber abierto hace poco (para buscar sus versiones anteriores).
pub fn recent_docs() -> Vec<RecentDoc> {
    let mut docs: Vec<RecentDoc> = Vec::new();
    for app in APPS {
        for ver in OFFICE_VERSIONS {
            let key = format!(r"HKCU\Software\Microsoft\Office\{ver}\{app}\File MRU");
            for (name, raw) in crate::tweaks::registry::values(&key) {
                if !name.to_ascii_lowercase().starts_with("item") {
                    continue;
                }
                let Some((path, opened)) = crate::tweaks::registry::raw_to_string(&raw).and_then(|v| parse_mru(&v)) else { continue };
                if !valid_local_path(&path) || docs.iter().any(|d| d.path.eq_ignore_ascii_case(&path)) {
                    continue;
                }
                let name = Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                docs.push(RecentDoc { name, path, app: app.to_string(), opened });
            }
        }
    }
    docs.sort_by_key(|d| std::cmp::Reverse(d.opened));
    docs.truncate(12);
    docs
}

#[tauri::command(async)]
pub fn unsaved_list() -> UnsavedList {
    UnsavedList { files: scan(), recent: recent_docs() }
}

/// Un archivo de la lista de ahora (no se acepta una ruta cualquiera de la interfaz).
fn listed(path: &str) -> Result<UnsavedFile, String> {
    scan().into_iter().find(|f| f.path.eq_ignore_ascii_case(path)).ok_or_else(|| "Ese archivo ya no está en la lista.".to_string())
}

/// Abre un archivo para leerlo sin impedir que Office lo siga usando.
#[cfg(windows)]
fn open_shared(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
    std::fs::OpenOptions::new().read(true).share_mode(0x7).open(path)
}

#[cfg(not(windows))]
fn open_shared(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}

#[tauri::command(async)]
pub fn unsaved_preview(path: String) -> Result<Preview, String> {
    let f = listed(&path)?;
    let file = open_shared(Path::new(&f.path)).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    Ok(Preview { lines: zip_preview(file) })
}

// ---------- Versiones anteriores (instantáneas VSS) ----------

#[derive(serde::Deserialize)]
struct RawShadow {
    id: String,
    device: String,
    when: i64,
}

const SHADOWS_SCRIPT_BODY: &str = r#"# La unidad del archivo y sus instantáneas (Versiones anteriores)
$r = @()
try {
    $volumen = (Get-Volume -DriveLetter $unidad -ErrorAction Stop).UniqueId
    $r = @(Get-CimInstance -ClassName Win32_ShadowCopy -ErrorAction Stop | Where-Object { $_.VolumeName -eq $volumen } | Sort-Object InstallDate -Descending | ForEach-Object {
        [pscustomobject]@{ id = "$($_.ID)"; device = "$($_.DeviceObject)"; when = [int64]([DateTimeOffset]$_.InstallDate).ToUnixTimeSeconds() }
    })
} catch {
    # Que no se pudo mirar no es lo mismo que «no hay copias»: se dice con su motivo.
    throw "No se pudo mirar las copias de Windows: $($_.Exception.Message)"
}
ConvertTo-Json -Compress -InputObject $r
"#;

fn shadows_script(drive: char) -> String {
    format!("{}{SHADOWS_SCRIPT_BODY}", crate::ps::text_var("unidad", &drive.to_string()))
}

/// ¿Es un dispositivo de instantánea de verdad (`\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopyN`)?
pub fn valid_shadow_device(d: &str) -> bool {
    d.strip_prefix(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy").is_some_and(|n| !n.is_empty() && n.len() <= 6 && n.chars().all(|c| c.is_ascii_digit()))
}

/// En qué instantáneas está el archivo `path` (una ruta de disco local).
pub fn versions_of(path: &str) -> Result<VersionList, String> {
    if !valid_local_path(path) {
        return Err("Esa ruta no es la de un archivo de este equipo.".into());
    }
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let drive = path.chars().next().unwrap_or('C').to_ascii_uppercase();
    let out = crate::ps::powershell_opts(&shadows_script(drive), crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
    let shadows: Vec<RawShadow> = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada de Windows: {e}"))?;
    let rel = &path[2..];
    let mut versions = Vec::new();
    for s in &shadows {
        if !valid_shadow_device(&s.device) {
            continue;
        }
        let inside = PathBuf::from(format!("{}{}", s.device, rel));
        let Ok(meta) = std::fs::metadata(&inside) else { continue };
        if !meta.is_file() {
            continue;
        }
        versions.push(Version { id: s.id.clone(), when: s.when.max(0) as u64, size: meta.len(), modified: secs(meta.modified()), device: s.device.clone() });
    }
    Ok(VersionList { versions, snapshots: shadows.len() })
}

#[tauri::command(async)]
pub fn unsaved_versions(path: String) -> Result<VersionList, String> {
    versions_of(&path)
}

/// Deja elegir un archivo cualquiera para buscar sus versiones anteriores.
#[tauri::command(async)]
pub fn unsaved_pick(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file().blocking_pick_file().and_then(|p| p.into_path().ok())?;
    let s = path.to_string_lossy().into_owned();
    valid_local_path(&s).then_some(s)
}

// ---------- Abrir una copia ----------

/// El Escritorio del usuario con la sesión abierta (puede estar redirigido a OneDrive).
fn desktop() -> Result<PathBuf, String> {
    let dirs = crate::target_user::user_dirs();
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
    let from_reg = crate::tweaks::registry::read_raw(key, "Desktop").and_then(|r| crate::tweaks::registry::raw_to_string(&r)).and_then(|v| expand_env_for_user(&v, dirs.as_ref()));
    let dir = from_reg.map(PathBuf::from).or_else(|| dirs.map(|d| d.profile.join("Desktop")));
    dir.filter(|d| d.is_dir()).ok_or_else(|| "No se encontró el Escritorio.".to_string())
}

/// Copia `src` al Escritorio sin pisar nada. Devuelve la ruta de la copia.
fn copy_to_desktop(src: &Path, name: &str, ext: &str) -> Result<PathBuf, String> {
    let dir = desktop()?;
    let hhmm = chrono::Local::now().format("%H-%M").to_string();
    let mut input = open_shared(src).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    for n in 1..=50u32 {
        let dest = dir.join(copy_name(name, ext, &hhmm, n));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&dest) {
            Ok(mut out) => {
                return match std::io::copy(&mut input, &mut out) {
                    Ok(_) => Ok(dest),
                    Err(e) => {
                        drop(out);
                        let _ = std::fs::remove_file(&dest);
                        Err(format!("No se pudo copiar el archivo: {e}"))
                    }
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("No se pudo crear la copia en el Escritorio: {e}")),
        }
    }
    Err("Ya hay demasiadas copias con ese nombre en el Escritorio.".into())
}

/// Abre la copia. Los `.asd` y `.xar` los abre Word o Excel (si están); el resto, su programa asociado.
/// Devuelve `true` si se abrió con un programa; si no, enseña dónde quedó la copia.
fn open_copy(dest: &Path, app: &str, ext: &str, can_open: bool) -> bool {
    if can_open {
        if needs_office_app(ext) {
            if let Some(exe) = office_exe(app) {
                if crate::shellopen::run_as_user(&exe.to_string_lossy(), &format!("\"{}\"", dest.display())).is_ok() {
                    return true;
                }
            }
        } else if crate::shellopen::open(&dest.to_string_lossy()).is_ok() {
            return true;
        }
    }
    // Sin programa que lo abra: se enseña dónde quedó.
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", dest.display())).spawn();
    false
}

/// Lo que se le dice al técnico según se abriera o no la copia.
pub fn copy_message(opened: bool, app: &str, ext: &str) -> String {
    if opened {
        "Abierta una copia en el Escritorio · el original sigue igual".into()
    } else if needs_office_app(ext) {
        let program = if app == "Office" { "Word, Excel o PowerPoint".to_string() } else { app.to_string() };
        format!("Copia en el Escritorio · no se encontró {program} para abrirla: ábrela desde Archivo > Abrir en ese programa · el original sigue igual")
    } else {
        "Copia en el Escritorio, sin programa que la abra · el original sigue igual".into()
    }
}

/// «Abrir una copia»: copia el archivo (o su versión de una instantánea) al Escritorio y lo abre.
/// El original no se toca.
#[tauri::command(async)]
pub fn unsaved_copy(path: String, version: Option<String>, state: State<'_, TweakState>) -> Result<CopyResult, String> {
    let result = (|| -> Result<CopyResult, String> {
        let (source, name, ext, can_open, app) = match &version {
            None => {
                let f = listed(&path)?;
                let ext = f.copy_ext.clone().unwrap_or_else(|| f.ext.clone());
                (PathBuf::from(&f.path), f.name.clone(), ext, f.copy_ext.is_some(), f.app.clone())
            }
            Some(id) => {
                let list = versions_of(&path)?;
                let v = list.versions.iter().find(|v| &v.id == id).ok_or("Esa versión ya no está.")?;
                let name = Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let ext = Path::new(&path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                let rel = &path[2..];
                let app = infer_app("Office", &ext).to_string();
                (PathBuf::from(format!("{}{}", v.device, rel)), name, ext, true, app)
            }
        };
        let dest = copy_to_desktop(&source, &name, &ext)?;
        let opened = open_copy(&dest, &app, &ext, can_open);
        let copy = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(CopyResult { name: copy, opened, message: copy_message(opened, &app, &ext) })
    })();
    let title = match &result {
        Ok(r) => format!("Recuperar sin guardar: copia «{}» en el Escritorio", r.name),
        Err(_) => "Recuperar sin guardar: abrir una copia".to_string(),
    };
    state.record(Op::Run, &title, &result);
    result
}

// ---------- «Ver el comando» ----------

/// La ruta como expresión de PowerShell sin el nombre del usuario: `(Join-Path $env:APPDATA '…')`.
/// `roots`: las variables de entorno que valen como inicio, de la más concreta a la más general.
fn ps_path_expr_in(path: &str, roots: &[(&str, String)]) -> String {
    for (var, root) in roots {
        let root = root.trim_end_matches('\\');
        let starts = path.get(..root.len()).is_some_and(|head| head.eq_ignore_ascii_case(root));
        if root.len() >= 3 && starts && path.as_bytes().get(root.len()) == Some(&b'\\') && path.len() > root.len() + 1 {
            return format!("(Join-Path $env:{var} {})", crate::ps::ps_literal(&path[root.len() + 1..]));
        }
    }
    crate::ps::ps_literal(path)
}

pub fn ps_path_expr(path: &str) -> String {
    // Las carpetas del usuario con la sesión abierta (no las de la cuenta que elevó AdminOps).
    let roots: Vec<(&str, String)> = match crate::target_user::user_dirs() {
        Some(d) => vec![
            ("LOCALAPPDATA", d.local_app_data.to_string_lossy().into_owned()),
            ("APPDATA", d.app_data.to_string_lossy().into_owned()),
            ("TEMP", d.temp.to_string_lossy().into_owned()),
            ("USERPROFILE", d.profile.to_string_lossy().into_owned()),
        ],
        None => Vec::new(),
    };
    ps_path_expr_in(path, &roots)
}

/// El comando equivalente a «Abrir una copia» de un archivo sin guardar.
pub fn action_script(context: &str) -> Result<(&'static str, String), String> {
    if !valid_local_path(context) {
        return Err("Falta el archivo.".into());
    }
    let p = Path::new(context);
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let folder_app = APPS.iter().find(|a| context.split('\\').any(|seg| seg.eq_ignore_ascii_case(a))).copied().unwrap_or("Office");
    let app = infer_app(folder_app, &ext);
    let new_ext = copy_ext(app, &ext).map(str::to_string).unwrap_or_else(|| ext.clone());
    // Un .asd o .xar no lo abre el programa asociado: se lo pedimos a Word o Excel (si no están, se deja la copia).
    let open = match (needs_office_app(&ext), office_exe_name(app)) {
        (true, Some(exe)) => format!(
            "# Y se abre con {app} (un .{ext} es un formato propio de Office)\n\
             $programa = (Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{exe}' -ErrorAction SilentlyContinue).'(default)'\n\
             if ($programa) {{ Start-Process -FilePath $programa -ArgumentList \"`\"$destino`\"\" }} else {{ Write-Output \"No se encontró {app}: abre la copia desde Archivo > Abrir.\" }}\n"
        ),
        _ => "# Y se abre con su programa\nStart-Process -FilePath $destino\n".to_string(),
    };
    let script = format!(
        "# Abrir una copia de un archivo sin guardar de Office (el original no se toca)\n\n\
         # El archivo de autorrecuperación, sin guardar o temporal\n$origen = {}\n\
         # El Escritorio del usuario\n$escritorio = [Environment]::GetFolderPath('Desktop')\n\
         # Nombre de la copia: «(recuperado HH-MM)» y la extensión que corresponde a su tipo\n\
         $hora = Get-Date -Format 'HH-mm'\n\
         $nombre = [IO.Path]::GetFileNameWithoutExtension($origen) -replace '^(~|Autorrecuperación de |AutoRecovery save of )', ''\n\
         $destino = Join-Path $escritorio \"$nombre (recuperado $hora).{new_ext}\"\n\
         # Se copia sin pisar nada\n\
         if (Test-Path -LiteralPath $destino) {{ throw 'Ya existe una copia con ese nombre.' }}\n\
         Copy-Item -LiteralPath $origen -Destination $destino\n\
         {open}",
        ps_path_expr(context)
    );
    Ok(("Abrir una copia de un archivo sin guardar", script))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn expand_env(s: &str) -> Option<String> {
        expand_env_with(s, &|name| std::env::var(name).ok())
    }

    #[test]
    fn las_variables_de_entorno_se_expanden_o_se_rechaza() {
        std::env::set_var("ADMINOPS_TEST_DIR", r"C:\Datos");
        assert_eq!(expand_env(r"%ADMINOPS_TEST_DIR%\Microsoft\Word").as_deref(), Some(r"C:\Datos\Microsoft\Word"));
        assert_eq!(expand_env(r"D:\sin\variables").as_deref(), Some(r"D:\sin\variables"));
        assert_eq!(expand_env(r"%NO_EXISTE_ADMINOPS%\x"), None);
        assert_eq!(expand_env(r"50%\x"), None);
        assert_eq!(expand_env(r"%%\x"), None);
    }

    #[test]
    fn solo_se_aceptan_rutas_de_disco_locales_sin_trucos() {
        assert!(valid_local_path(r"C:\Users\Ana\Documents\Presupuesto 2027.xlsx"));
        assert!(!valid_local_path(r"\\servidor\recurso\x.docx"));
        assert!(!valid_local_path(r"C:\Users\..\Windows\x"));
        assert!(!valid_local_path(r"C:Users\x"));
        assert!(!valid_local_path(r"C:\Users\a|b.docx"));
        assert!(!valid_local_path("C:\\Users\\a\nb.docx"));
        assert!(!valid_local_path(r"C:\*.docx"));
        assert!(!valid_local_path("C:\\"));
        assert!(!valid_local_path(&format!("C:\\{}", "a".repeat(600))));
    }

    #[test]
    fn los_nombres_de_office_se_limpian() {
        assert_eq!(clean_name("Autorrecuperación de Presupuesto 2027.asd"), "Presupuesto 2027.asd");
        assert_eq!(clean_name("AutoRecovery save of Libro1.xlsb"), "Libro1.xlsb");
        assert_eq!(clean_name("~WRD0001.tmp"), "WRD0001.tmp");
        assert_eq!(clean_name("Informe.docx"), "Informe.docx");
        assert_eq!(clean_name("Copia de seguridad de Acta.wbk"), "Acta.wbk");
        // Si no queda nada, el nombre de siempre.
        assert_eq!(clean_name("~"), "~");
    }

    #[test]
    fn cada_extension_tiene_su_programa_y_su_copia() {
        assert_eq!(infer_app("Word", "asd"), "Word");
        assert_eq!(infer_app("PowerPoint", "asd"), "PowerPoint");
        assert_eq!(infer_app("Office", "asd"), "Word");
        assert_eq!(infer_app("Office", "xar"), "Excel");
        assert_eq!(infer_app("Office", "tmp"), "Office");
        assert_eq!(infer_app("Excel", "wbk"), "Word");
        // .asd y .xar son formatos propios: conservan su extensión. .wbk sí es un .doc.
        assert_eq!(copy_ext("Word", "asd"), Some("asd"));
        assert_eq!(copy_ext("Excel", "asd"), Some("asd"));
        assert_eq!(copy_ext("PowerPoint", "asd"), Some("asd"));
        assert_eq!(copy_ext("Excel", "xar"), Some("xar"));
        assert_eq!(copy_ext("Word", "wbk"), Some("doc"));
        assert!(needs_office_app("asd") && needs_office_app("xar") && !needs_office_app("wbk") && !needs_office_app("docx"));
        assert_eq!(office_exe_name("Word"), Some("WINWORD.EXE"));
        assert_eq!(office_exe_name("Excel"), Some("EXCEL.EXE"));
        assert_eq!(office_exe_name("Office"), None);
        assert_eq!(copy_ext("Excel", "tmp"), Some("xlsx"));
        assert_eq!(copy_ext("Office", "tmp"), None);
        assert_eq!(copy_ext("Excel", "xlsb"), Some("xlsb"));
        assert_eq!(copy_ext("Office", "exe"), None);
    }

    #[test]
    fn los_temporales_de_office_se_reconocen() {
        assert!(is_office_temp("~WRD0000.tmp", 20_000));
        assert!(is_office_temp("~WRL3E4F.TMP", 500));
        assert!(!is_office_temp("~DF1A2B.TMP", 20_000));
        assert!(!is_office_temp("~WRD0000.tmp", 0));
        assert!(!is_office_temp("tmp8F3A.tmp", 20_000));
        assert!(!is_office_temp("~$Informe.docx", 162));
        assert!(is_office_temp("~$Informe.docx", 4096));
        assert!(!is_office_temp("notas.txt", 100));
    }

    #[test]
    fn el_nombre_de_la_copia_no_pisa_ni_lleva_caracteres_prohibidos() {
        assert_eq!(copy_name("Presupuesto 2027.xlsx", "xlsx", "11-52", 1), "Presupuesto 2027 (recuperado 11-52).xlsx");
        assert_eq!(copy_name("Presupuesto 2027.xlsx", "xlsx", "11-52", 2), "Presupuesto 2027 (recuperado 11-52) 2.xlsx");
        assert_eq!(copy_name("a:b?c.asd", "docx", "09-05", 1), "a_b_c (recuperado 09-05).docx");
        assert_eq!(copy_name("", "docx", "09-05", 1), "Documento (recuperado 09-05).docx");
        assert_eq!(copy_name("x.tmp", "", "09-05", 1), "x (recuperado 09-05)");
        assert!(copy_name(&"n".repeat(300), "docx", "09-05", 1).chars().count() < 160);
    }

    #[test]
    fn el_texto_de_un_xml_de_office_sale_por_parrafos() {
        let docx = r#"<w:body><w:p><w:r><w:t>Hola </w:t></w:r><w:r><w:t xml:space="preserve">mundo &amp; más</w:t></w:r></w:p><w:p><w:tab/><w:r><w:t/></w:r></w:p><w:p><w:r><w:t>Segunda línea</w:t></w:r></w:p></w:body>"#;
        assert_eq!(xml_texts(docx, "<w:t", "</w:p>"), vec!["Hola mundo & más", "Segunda línea"]);
        let xlsx = "<sst><si><t>Concepto</t></si><si><r><t>Total</t></r><r><t> 2027</t></r></si></sst>";
        assert_eq!(xml_texts(xlsx, "<t", "</si>"), vec!["Concepto", "Total 2027"]);
        // `<tbl>` y `<tab/>` no son texto.
        assert!(xml_texts("<w:tbl><w:tab/></w:tbl>", "<w:t", "</w:p>").is_empty());
        assert!(xml_texts("", "<w:t", "</w:p>").is_empty());
        // XML cortado: no se cuelga ni falla.
        assert_eq!(xml_texts("<w:p><w:t>sin cerrar", "<w:t", "</w:p>"), Vec::<String>::new());
    }

    #[test]
    fn la_vista_previa_lee_un_docx_y_no_se_atraganta_con_otra_cosa() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"<w:p><w:r><w:t>Primera</w:t></w:r></w:p><w:p><w:r><w:t>Segunda</w:t></w:r></w:p>").unwrap();
            zip.finish().unwrap();
        }
        buf.set_position(0);
        assert_eq!(zip_preview(buf), vec!["Primera", "Segunda"]);
        assert!(zip_preview(std::io::Cursor::new(b"esto no es un zip".to_vec())).is_empty());
        assert!(zip_preview(std::io::Cursor::new(Vec::new())).is_empty());
    }

    #[test]
    fn las_lineas_largas_se_acortan() {
        let l = shorten(&"palabra ".repeat(60));
        assert!(l.chars().count() <= PREVIEW_WIDTH + 1 && l.ends_with('…'));
        assert_eq!(shorten("corta"), "corta");
    }

    #[test]
    fn un_reciente_de_office_trae_ruta_y_fecha() {
        let (p, t) = parse_mru(r"[F00000000][T01D8A1B2C3D4E5F6][O00000000]*C:\Datos\Cuentas.xlsx").unwrap();
        assert_eq!(p, r"C:\Datos\Cuentas.xlsx");
        assert!(t > 1_600_000_000, "{t}");
        let (p, t) = parse_mru(r"*D:\x.docx").unwrap();
        assert_eq!((p.as_str(), t), (r"D:\x.docx", 0));
        assert!(parse_mru("sin asterisco").is_none());
    }

    #[test]
    fn solo_valen_los_dispositivos_de_instantanea_de_verdad() {
        assert!(valid_shadow_device(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy12"));
        assert!(!valid_shadow_device(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy"));
        assert!(!valid_shadow_device(r"\\?\GLOBALROOT\Device\HarddiskVolume3"));
        assert!(!valid_shadow_device(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1\..\x"));
        assert!(!valid_shadow_device(r"C:\x"));
    }

    #[test]
    fn la_ruta_del_comando_no_lleva_el_nombre_de_usuario() {
        let roots = [("LOCALAPPDATA", r"C:\Users\Ana\AppData\Local".to_string()), ("APPDATA", r"C:\Users\Ana\AppData\Roaming".to_string())];
        let e = ps_path_expr_in(r"C:\Users\Ana\AppData\Roaming\Microsoft\Excel\Libro1.xlsb", &roots);
        assert_eq!(e, r"(Join-Path $env:APPDATA 'Microsoft\Excel\Libro1.xlsb')");
        assert!(!e.contains("Ana"));
        assert_eq!(ps_path_expr_in(r"D:\Otros\x.docx", &roots), r"'D:\Otros\x.docx'");
        // Una carpeta que solo empieza igual no cuenta.
        assert_eq!(ps_path_expr_in(r"C:\Users\Ana\AppData\RoamingX\a.docx", &roots), r"'C:\Users\Ana\AppData\RoamingX\a.docx'");
    }

    #[test]
    fn los_comandos_de_copia_son_powershell_valido() {
        for ctx in [r"C:\Users\Ana\AppData\Roaming\Microsoft\Excel\Libro1.xar", r"C:\Users\Ana\AppData\Local\Microsoft\Office\UnsavedFiles\Informe.asd", r"D:\Con 'comillas'\a.tmp"] {
            let (title, script) = action_script(ctx).unwrap();
            assert!(!title.is_empty());
            let errors = crate::ps::parse_errors(&script);
            assert!(errors.trim().is_empty(), "{ctx}: {errors}");
        }
        assert!(action_script("").is_err());
        assert!(action_script(r"..\x").is_err());
        let (_, s) = action_script(r"C:\Users\Ana\AppData\Roaming\Microsoft\Excel\Libro1.xar").unwrap();
        assert!(s.contains("(recuperado $hora).xar"), "{s}");
        assert!(s.contains("EXCEL.EXE"), "{s}");
        let (_, w) = action_script(r"C:\Users\Ana\AppData\Local\Microsoft\Office\UnsavedFiles\Informe.asd").unwrap();
        assert!(w.contains("(recuperado $hora).asd") && w.contains("WINWORD.EXE"), "{w}");
        // Un .wbk se copia como .doc y lo abre el programa asociado.
        let (_, b) = action_script(r"C:\Users\Ana\Documents\Acta.wbk").unwrap();
        assert!(b.contains("(recuperado $hora).doc") && b.contains("Start-Process -FilePath $destino"), "{b}");
    }

    #[test]
    fn la_frase_de_la_copia_dice_con_que_abrirla() {
        assert!(copy_message(true, "Word", "asd").starts_with("Abierta una copia"));
        let m = copy_message(false, "Word", "asd");
        assert!(m.contains("no se encontró Word") && m.contains("Archivo > Abrir") && m.contains("el original sigue igual"), "{m}");
        assert!(copy_message(false, "Office", "asd").contains("Word, Excel o PowerPoint"));
        assert!(copy_message(false, "Word", "docx").contains("sin programa"));
    }

    #[test]
    fn las_variables_se_expanden_con_las_carpetas_del_cliente() {
        let d = crate::target_user::UserDirs {
            profile: PathBuf::from(r"C:\Users\Cliente"),
            app_data: PathBuf::from(r"C:\Users\Cliente\AppData\Roaming"),
            local_app_data: PathBuf::from(r"C:\Users\Cliente\AppData\Local"),
            temp: PathBuf::from(r"C:\Users\Cliente\AppData\Local\Temp"),
        };
        assert_eq!(expand_env_for_user(r"%AppData%\Microsoft\Word", Some(&d)).as_deref(), Some(r"C:\Users\Cliente\AppData\Roaming\Microsoft\Word"));
        assert_eq!(expand_env_for_user(r"%LOCALAPPDATA%\x", Some(&d)).as_deref(), Some(r"C:\Users\Cliente\AppData\Local\x"));
        assert_eq!(expand_env_for_user(r"%TEMP%", Some(&d)).as_deref(), Some(r"C:\Users\Cliente\AppData\Local\Temp"));
        assert_eq!(expand_env_for_user(r"%USERPROFILE%\Documents", Some(&d)).as_deref(), Some(r"C:\Users\Cliente\Documents"));
        assert_eq!(expand_env_for_user(r"%NO_EXISTE_ADMINOPS%\x", Some(&d)), None);
        assert_eq!(expand_env_with(r"%A%\b", &|n| (n == "A").then(|| "Z".to_string())).as_deref(), Some(r"Z\b"));
    }

    #[test]
    fn xlstart_y_startup_no_son_recuperaciones() {
        assert!(is_startup_folder("XLSTART") && is_startup_folder("xlstart") && is_startup_folder("Startup"));
        assert!(!is_startup_folder("Excel") && !is_startup_folder("XLSTARTX"));
        let base = std::env::temp_dir().join(format!("adminops_unsaved_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("XLSTART")).unwrap();
        std::fs::create_dir_all(base.join("STARTUP")).unwrap();
        std::fs::write(base.join("XLSTART").join("PERSONAL.XLSB"), b"libro de macros").unwrap();
        std::fs::write(base.join("STARTUP").join("plantilla.docx"), b"plantilla").unwrap();
        std::fs::write(base.join("Libro1.xlsb"), b"recuperado").unwrap();
        let root = Root { dir: base.clone(), app: "Excel", kind: Kind::AutoRecover };
        let mut out = Vec::new();
        walk(&base, 0, &root, &mut out);
        let _ = std::fs::remove_dir_all(&base);
        let names: Vec<&str> = out.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["Libro1.xlsb"], "{names:?}");
    }

    /// Equipo real (solo lee): `cargo test --lib sin_guardar_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sin_guardar_real() {
        println!("Carpetas que se miran:");
        for r in roots() {
            println!("  [{:?}] {} · existe: {}", r.kind, r.dir.display().to_string().replace(&std::env::var("USERNAME").unwrap_or_default(), "<usuario>"), r.dir.is_dir());
        }
        let files = scan();
        println!("Sin guardar encontrados: {}", files.len());
        for f in files.iter().take(10) {
            println!("  {} · {} · {} · {} bytes · copia .{:?}", f.name, f.app, f.origin, f.size, f.copy_ext);
        }
        let recent = recent_docs();
        println!("Recientes de Office: {}", recent.len());
        for d in recent.iter().take(5) {
            println!("  {} · {}", d.name, d.app);
        }
    }
}
