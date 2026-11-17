//! ¿Quién tiene ese archivo? «No se puede expulsar el USB», «el archivo está
//! abierto en otro programa», «no deja borrar la carpeta».
//!
//! - En este equipo: el gestor de reinicio de Windows (Restart Manager, el que
//!   usan los instaladores) dice qué programas tienen un archivo abierto. Para
//!   una carpeta o una unidad se le pasan los archivos de dentro (con tope) y se
//!   suman los programas que trabajan o se ejecutan desde ahí.
//! - Por la red: `Get-SmbOpenFile` dice quién tiene abierto un archivo que este
//!   equipo comparte, y `Close-SmbOpenFile` cierra esa conexión.
//!
//! Mirar no cambia nada. Cerrar un programa o una conexión sí: van en `BLOCKED`
//! y quedan en el diario.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::State;

/// Archivos de una carpeta que se le enseñan al gestor de reinicio, y el tiempo que se dedica a buscarlos.
const MAX_FILES: usize = 3000;
const SCAN_TIME: Duration = Duration::from_secs(4);

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Holder {
    pub pid: u32,
    /// Nombre para mostrar («Microsoft Excel»).
    pub name: String,
    /// Nombre del proceso («EXCEL.EXE»): con él se comprueba al cerrar.
    pub exe: String,
    /// app · service · explorer · system
    pub kind: &'static str,
    pub closable: bool,
    /// Qué hace con la ubicación.
    pub note: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetHolder {
    pub file_id: String,
    pub user: String,
    pub client: String,
    pub path: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HolderReport {
    /// La ruta, sin el nombre de usuario.
    pub path: String,
    /// file · folder · drive
    pub kind: &'static str,
    pub local: Vec<Holder>,
    pub network: Vec<NetHolder>,
    /// Por qué no se pudo mirar la red (o vacío).
    pub net_note: String,
    /// Archivos que se le enseñaron al gestor de reinicio (0 si era uno solo).
    pub scanned: usize,
    /// La carpeta tiene más archivos de los que se revisaron.
    pub truncated: bool,
}

/// La ruta tal como la entiende Windows: `E:` pasa a `E:\`. Solo rutas absolutas, locales o de red.
pub fn normalize(input: &str) -> Result<String, String> {
    let t = input.trim().trim_matches('"').trim();
    if t.is_empty() {
        return Err("Elige un archivo, una carpeta o una unidad.".into());
    }
    if t.chars().any(|c| c.is_control() || "<>|\"*?".contains(c)) || t.chars().count() > 400 {
        return Err("Esa ruta no es válida.".into());
    }
    let b = t.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    if drive && (b.len() == 2 || b[2] == b'\\' || b[2] == b'/') {
        let rest = t[2..].replace('/', "\\");
        return Ok(format!("{}:{}", (b[0] as char).to_ascii_uppercase(), if rest.is_empty() { "\\".to_string() } else { rest }));
    }
    if t.starts_with("\\\\") && t.len() > 4 {
        return Ok(t.to_string());
    }
    Err("Escribe una ruta completa, como E:\\ o C:\\Carpeta\\archivo.xlsx.".into())
}

fn is_root(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() == 3 && b[1] == b':' && b[2] == b'\\'
}

/// ¿`inner` está dentro de `root` (o es `root`)? Sin distinguir mayúsculas.
pub fn under(inner: &str, root: &str) -> bool {
    let (i, r) = (inner.to_ascii_lowercase(), root.trim_end_matches('\\').to_ascii_lowercase());
    i == r || i.starts_with(&format!("{r}\\"))
}

/// Archivos de una carpeta, de fuera hacia dentro, con tope de cantidad y de tiempo.
fn collect_files(root: &Path) -> (Vec<String>, bool) {
    let start = Instant::now();
    let mut files = Vec::new();
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            if files.len() >= MAX_FILES || start.elapsed() > SCAN_TIME {
                return (files, true);
            }
            let Ok(kind) = entry.file_type() else { continue };
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if kind.is_dir() {
                if !kind.is_symlink() && name != "$recycle.bin" && name != "system volume information" {
                    queue.push_back(entry.path());
                }
            } else {
                files.push(entry.path().display().to_string());
            }
        }
    }
    (files, false)
}

struct RmApp {
    pid: u32,
    name: String,
    service: String,
    /// RM_APP_TYPE: 3 servicio, 4 Explorador, 1000 crítico.
    typ: i32,
}

#[cfg(windows)]
fn rm_holders(files: &[String]) -> Result<Vec<RmApp>, String> {
    use windows_sys::Win32::System::RestartManager::*;
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let text = |w: &[u16]| String::from_utf16_lossy(&w[..w.iter().position(|c| *c == 0).unwrap_or(w.len())]);
    // SAFETY: sesión del gestor de reinicio abierta y cerrada aquí; los punteros a
    // los nombres (`wide`) y al arreglo de resultados viven mientras dura cada llamada.
    unsafe {
        let mut session = 0u32;
        let mut key = [0u16; 33];
        let r = RmStartSession(&mut session, 0, key.as_mut_ptr());
        if r != 0 {
            return Err(format!("El gestor de reinicio de Windows no arrancó (error {r})."));
        }
        let wide: Vec<Vec<u16>> = files.iter().map(|f| f.encode_utf16().chain(std::iter::once(0)).collect()).collect();
        let ptrs: Vec<*const u16> = wide.iter().map(|w| w.as_ptr()).collect();
        let out: Result<Vec<RmApp>, String>;
        let r = RmRegisterResources(session, ptrs.len() as u32, ptrs.as_ptr(), 0, std::ptr::null(), 0, std::ptr::null());
        if r != 0 {
            out = Err(format!("Windows no pudo mirar esa ubicación (error {r})."));
        } else {
            let (mut needed, mut count, mut reasons) = (0u32, 0u32, 0u32);
            let mut r = RmGetList(session, &mut needed, &mut count, std::ptr::null_mut(), &mut reasons);
            let mut buf: Vec<RM_PROCESS_INFO> = Vec::new();
            // 234 = ERROR_MORE_DATA: la lista cambia entre llamadas, se reintenta con el tamaño nuevo.
            for _ in 0..3 {
                if r != 234 {
                    break;
                }
                buf = vec![RM_PROCESS_INFO::default(); needed as usize];
                count = needed;
                r = RmGetList(session, &mut needed, &mut count, buf.as_mut_ptr(), &mut reasons);
            }
            if r != 0 {
                out = Err(format!("Windows no pudo listar los programas (error {r})."));
            } else {
                out = Ok(buf
                    .iter()
                    .take(count as usize)
                    .map(|p| RmApp { pid: p.Process.dwProcessId, name: text(&p.strAppName), service: text(&p.strServiceShortName), typ: p.ApplicationType })
                    .collect());
            }
        }
        RmEndSession(session);
        out
    }
}

#[cfg(not(windows))]
fn rm_holders(_: &[String]) -> Result<Vec<RmApp>, String> {
    Ok(Vec::new())
}

/// Qué se puede hacer con un programa que tiene algo agarrado.
fn describe(pid: u32, exe: &str, rm_type: i32, service: &str, own: &std::collections::HashSet<u32>) -> (&'static str, bool, String) {
    let protection = crate::processes::classify(exe, pid, own).0;
    if protection == "self" {
        return ("system", false, "Es parte de AdminOps.".into());
    }
    if protection == "critical" || rm_type == 1000 {
        return ("system", false, "Es una parte esencial de Windows: no se cierra desde aquí.".into());
    }
    if rm_type == 4 || exe.eq_ignore_ascii_case("explorer.exe") {
        return ("explorer", false, "Es el Explorador de Windows: cierra la ventana que tenga abierta esa ubicación.".into());
    }
    if rm_type == 3 {
        let s = if service.is_empty() { String::new() } else { format!(" ({service})") };
        return ("service", false, format!("Es un servicio de Windows{s}: se detiene desde Servicios, no desde aquí."));
    }
    ("app", true, String::new())
}

/// Quién tiene esa ubicación en este equipo.
fn local_holders(path: &str, kind: &'static str) -> Result<(Vec<Holder>, usize, bool), String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let (files, truncated) = if kind == "file" { (vec![path.to_string()], false) } else { collect_files(Path::new(path)) };
    let apps = rm_holders(&files)?;
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::Always).with_cwd(UpdateKind::Always));
    let own = crate::processes::own_tree(&sys);
    let mut out: Vec<Holder> = Vec::new();
    for a in apps {
        if own.contains(&a.pid) || out.iter().any(|h| h.pid == a.pid) {
            continue;
        }
        let exe = sys.process(sysinfo::Pid::from_u32(a.pid)).map(|p| p.name().to_string_lossy().into_owned()).unwrap_or_else(|| a.name.clone());
        let (k, closable, note) = describe(a.pid, &exe, a.typ, &a.service, &own);
        let note = if note.is_empty() { if kind == "file" { "Tiene el archivo abierto.".to_string() } else { "Tiene archivos de ahí abiertos.".to_string() } } else { note };
        out.push(Holder { pid: a.pid, name: if a.name.is_empty() { exe.clone() } else { a.name }, exe, kind: k, closable, note });
    }
    // Carpeta o unidad: también los programas que se ejecutan desde ahí o trabajan en ella.
    if kind != "file" {
        for (pid, p) in sys.processes() {
            let pid = pid.as_u32();
            if pid == 0 || own.contains(&pid) || out.iter().any(|h| h.pid == pid) {
                continue;
            }
            let in_exe = p.exe().is_some_and(|e| under(&e.display().to_string(), path));
            let in_cwd = p.cwd().is_some_and(|c| under(&c.display().to_string(), path));
            if !in_exe && !in_cwd {
                continue;
            }
            let exe = p.name().to_string_lossy().into_owned();
            let (k, closable, note) = describe(pid, &exe, 0, "", &own);
            let note = if note.is_empty() { if in_exe { "Se está ejecutando desde ahí.".to_string() } else { "Tiene ahí su carpeta de trabajo.".to_string() } } else { note };
            out.push(Holder { pid, name: exe.clone(), exe, kind: k, closable, note });
        }
    }
    out.sort_by(|a, b| b.closable.cmp(&a.closable).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok((out, files.len(), truncated))
}

/// Quién tiene abierto, por la red, algo de lo que este equipo comparte.
fn network_holders(path: &str) -> (Vec<NetHolder>, String) {
    if path.starts_with("\\\\") {
        return (Vec::new(), "Es una ruta de otro equipo: quién la tiene abierta se ve en ese equipo.".into());
    }
    if !crate::elevation::is_elevated() {
        return (Vec::new(), "Para ver quién lo tiene abierto por la red hace falta ejecutar AdminOps como administrador.".into());
    }
    let script = format!(
        "{}{}",
        crate::ps::text_var("p", path.trim_end_matches('\\')),
        r#"
$rows = @(Get-SmbOpenFile -ErrorAction SilentlyContinue | Where-Object { $_.Path -and ($_.Path -ieq $p -or $_.Path.StartsWith($p + '\', 'OrdinalIgnoreCase')) } | Select-Object -First 40 | ForEach-Object {
  [pscustomobject]@{ fileId = "$($_.FileId)"; user = "$($_.ClientUserName)"; client = "$($_.ClientComputerName)"; path = "$($_.Path)" }
})
ConvertTo-Json -InputObject $rows -Compress
"#
    );
    match crate::ps::powershell(&script) {
        Ok(out) => (parse_smb(&out), String::new()),
        Err(e) => (Vec::new(), format!("No se pudo mirar la red: {}", crate::escalate::scrub(&e))),
    }
}

pub fn parse_smb(out: &str) -> Vec<NetHolder> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        file_id: String,
        user: String,
        client: String,
        path: String,
    }
    let t = out.trim();
    if t.is_empty() || t == "null" {
        return Vec::new();
    }
    let list: Vec<Raw> = serde_json::from_str::<Vec<Raw>>(t).or_else(|_| serde_json::from_str::<Raw>(t).map(|r| vec![r])).unwrap_or_default();
    list.into_iter()
        .filter(|r| !r.file_id.is_empty() && r.file_id.len() <= 20 && r.file_id.chars().all(|c| c.is_ascii_digit()))
        .map(|r| NetHolder { file_id: r.file_id, user: r.user, client: r.client, path: crate::escalate::scrub(&r.path) })
        .collect()
}

/// Quién tiene un archivo, una carpeta o una unidad. Solo mira.
#[tauri::command(async)]
pub fn file_holders(path: String) -> Result<HolderReport, String> {
    let path = normalize(&path)?;
    let meta = std::fs::metadata(&path).map_err(|_| "No se encuentra esa ubicación: ¿está conectada la unidad?".to_string())?;
    let kind = if is_root(&path) { "drive" } else if meta.is_dir() { "folder" } else { "file" };
    let (local, scanned, truncated) = local_holders(&path, kind)?;
    let (network, net_note) = network_holders(&path);
    Ok(HolderReport { path: crate::escalate::scrub(&path), kind, local, network, net_note, scanned: if kind == "file" { 0 } else { scanned }, truncated })
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub letter: String,
    /// removable · fixed · network · other
    pub kind: &'static str,
}

/// Unidades de este equipo (las extraíbles primero), para soltar o elegir una.
#[tauri::command]
pub fn holder_drives() -> Vec<DriveInfo> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
        // SAFETY: GetLogicalDrives no tiene argumentos; GetDriveTypeW recibe una ruta terminada en cero.
        let mask = unsafe { GetLogicalDrives() };
        let mut out: Vec<DriveInfo> = ('A'..='Z')
            .filter(|c| mask & (1 << (*c as u32 - 'A' as u32)) != 0)
            .map(|c| {
                let root: Vec<u16> = format!("{c}:\\").encode_utf16().chain(std::iter::once(0)).collect();
                let kind = match unsafe { GetDriveTypeW(root.as_ptr()) } {
                    2 => "removable",
                    3 => "fixed",
                    4 => "network",
                    _ => "other",
                };
                DriveInfo { letter: c.to_string(), kind }
            })
            .collect();
        out.sort_by_key(|d| match d.kind {
            "removable" => 0,
            "fixed" => 1,
            _ => 2,
        });
        out
    }
    #[cfg(not(windows))]
    Vec::new()
}

/// Deja elegir un archivo o una carpeta. `None`: se canceló.
#[tauri::command(async)]
pub fn holder_pick(app: tauri::AppHandle, folder: bool) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let d = app.dialog().file();
    let picked = if folder { d.blocking_pick_folder() } else { d.blocking_pick_file() };
    picked.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CloseResult {
    pub closed: bool,
    pub message: String,
}

#[cfg(windows)]
fn post_close(pid: u32) -> usize {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE};
    struct Job {
        pid: u32,
        sent: usize,
    }
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: lparam es el `Job` de la llamada, vivo durante EnumWindows.
        unsafe {
            let job = &mut *(lparam as *mut Job);
            let mut owner = 0u32;
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == job.pid && IsWindowVisible(hwnd) != 0 && PostMessageW(hwnd, WM_CLOSE, 0, 0) != 0 {
                job.sent += 1;
            }
        }
        1
    }
    let mut job = Job { pid, sent: 0 };
    // SAFETY: el puntero apunta a `job`, que sigue vivo durante la llamada.
    unsafe {
        EnumWindows(Some(each), &mut job as *mut Job as LPARAM);
    }
    job.sent
}

#[cfg(not(windows))]
fn post_close(_: u32) -> usize {
    0
}

/// Cierra un programa que tiene algo agarrado. Primero se lo pide con educación
/// (así pregunta si guarda); con `force`, lo termina y se pierde lo no guardado.
#[tauri::command(async)]
pub fn close_holder(pid: u32, name: String, force: bool, tweaks: State<'_, TweakState>) -> Result<CloseResult, String> {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let own = crate::processes::own_tree(&sys);
    let Some(p) = sys.process(Pid::from_u32(pid)) else {
        return Ok(CloseResult { closed: true, message: "El programa ya estaba cerrado.".into() });
    };
    let current = p.name().to_string_lossy().into_owned();
    if !current.eq_ignore_ascii_case(&name) {
        return Err(format!("El PID {pid} ahora es «{current}», no «{name}». Vuelve a mirar."));
    }
    if !describe(pid, &current, 0, "", &own).1 || current.eq_ignore_ascii_case("explorer.exe") {
        return Err(format!("«{current}» no se puede cerrar desde aquí."));
    }
    let title = format!("Cerrar {current} para liberar un archivo{}", if force { " (a la fuerza)" } else { "" });
    let result = (|| {
        if !force {
            post_close(pid);
            let until = Instant::now() + Duration::from_secs(10);
            while Instant::now() < until {
                std::thread::sleep(Duration::from_millis(300));
                sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
                if sys.process(Pid::from_u32(pid)).is_none() {
                    return Ok(CloseResult { closed: true, message: format!("{current} se cerró.") });
                }
            }
            return Ok(CloseResult { closed: false, message: format!("{current} sigue abierto: puede estar preguntando si guarda. Guarda y vuelve a mirar, o ciérralo a la fuerza.") });
        }
        sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        match sys.process(Pid::from_u32(pid)) {
            Some(p) if p.kill() => Ok(CloseResult { closed: true, message: format!("{current} se cerró a la fuerza.") }),
            Some(_) => Err(if crate::elevation::is_elevated() { "Windows no permitió cerrarlo: es un proceso protegido.".to_string() } else { "Acceso denegado: ejecuta AdminOps como administrador para cerrar este programa.".to_string() }),
            None => Ok(CloseResult { closed: true, message: "El programa ya estaba cerrado.".into() }),
        }
    })();
    tweaks.record(Op::Run, &title, &result);
    result
}

/// Cierra la conexión de otro equipo con un archivo que este comparte.
#[tauri::command(async)]
pub fn close_smb_file(file_id: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if file_id.is_empty() || file_id.len() > 20 || !file_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("Identificador de archivo no válido.".into());
    }
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let script = format!("{}Close-SmbOpenFile -FileId ([uint64]$id) -Force -ErrorAction Stop\n'ok'", crate::ps::text_var("id", &file_id));
    let result = crate::ps::powershell(&script).map(|_| ()).map_err(|e| crate::escalate::scrub(&e));
    tweaks.record(Op::Run, "Cerrar la conexión de otro equipo con un archivo de este", &result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_las_rutas() {
        assert_eq!(normalize("e:").unwrap(), "E:\\");
        assert_eq!(normalize("\"d:/Docs/a.txt\"").unwrap(), "D:\\Docs\\a.txt");
        assert_eq!(normalize(r"\\srv\conta").unwrap(), r"\\srv\conta");
        assert!(normalize("").is_err());
        assert!(normalize("relativa\\a.txt").is_err());
        assert!(normalize("C:\\a|b").is_err());
    }

    #[test]
    fn dentro_de_una_carpeta() {
        assert!(under(r"E:\Datos\a.txt", r"e:\datos"));
        assert!(under(r"E:\Datos", r"E:\Datos\"));
        assert!(!under(r"E:\Datos2\a.txt", r"E:\Datos"));
        assert!(under(r"E:\a", "E:\\"));
    }

    #[test]
    fn la_raiz_es_una_unidad() {
        assert!(is_root("E:\\"));
        assert!(!is_root("E:\\x"));
    }

    #[test]
    fn lee_los_archivos_abiertos_por_red() {
        assert!(parse_smb("[]").is_empty());
        let one = r#"{"fileId":"412316860417","user":"DOM\\m.perez","client":"10.0.0.8","path":"C:\\Users\\ana\\x.xlsx"}"#;
        let r = parse_smb(one);
        assert_eq!(r.len(), 1);
        assert!(r[0].path.contains("[usuario]"));
        // Un identificador que no es un número no pasa (va a un comando que cambia cosas).
        assert!(parse_smb(r#"[{"fileId":"1; calc","user":"a","client":"b","path":"c"}]"#).is_empty());
    }

    #[test]
    fn el_explorador_y_windows_no_se_cierran() {
        let own = std::collections::HashSet::new();
        assert!(!describe(4, "System", 0, "", &own).1);
        assert!(!describe(900, "explorer.exe", 4, "", &own).1);
        assert!(!describe(901, "SearchIndexer.exe", 3, "WSearch", &own).1);
        assert!(describe(902, "EXCEL.EXE", 1, "", &own).1);
    }

    #[test]
    fn un_archivo_abierto_se_encuentra_de_verdad() {
        // Este mismo proceso es «el programa que lo tiene»: se espera que no salga (es de AdminOps) y que no falle.
        let dir = std::env::temp_dir().join("adminops-filelocks-test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("abierto.txt");
        let _keep = std::fs::File::create(&f).unwrap();
        let r = file_holders(f.display().to_string()).unwrap();
        assert_eq!(r.kind, "file");
        assert!(r.local.iter().all(|h| h.pid != std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
    }
}
