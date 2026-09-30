//! Acceso remoto a fondo: agenda de conexiones, Escritorio remoto con opciones
//! y contraseña guardada en Windows, prueba de conexión, y las herramientas de
//! asistencia más usadas (AnyDesk, RustDesk, TeamViewer) con el ID de este equipo.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

static FILE_LOCK: Mutex<()> = Mutex::new(());

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub fn valid_host(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || ".-_:".contains(c))
}

// ---------- Agenda de conexiones ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RdpOptions {
    pub fullscreen: bool,
    pub multimon: bool,
    pub clipboard: bool,
    pub drives: bool,
    pub printers: bool,
    /// Sonido en este equipo.
    pub audio: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Connection {
    pub id: String,
    pub name: String,
    /// rdp | anydesk | rustdesk | teamviewer
    pub kind: String,
    /// Nombre o IP (RDP) o ID de la herramienta.
    pub target: String,
    pub username: String,
    pub client: String,
    pub notes: String,
    pub options: RdpOptions,
    /// Hay contraseña guardada en el Administrador de credenciales de Windows.
    pub saved_password: bool,
    pub last_used: u64,
}

fn book_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("remote-connections.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Connection> {
    crate::paths::read_json(&book_path(app))
}

#[tauri::command]
pub fn list_connections(app: tauri::AppHandle) -> Vec<Connection> {
    let mut v = load(&app);
    v.sort_by_key(|c| std::cmp::Reverse(c.last_used));
    v
}

fn validate(c: &Connection) -> Result<(), String> {
    if c.name.trim().is_empty() || c.name.chars().count() > 60 {
        return Err("Ponle un nombre (máximo 60 caracteres).".into());
    }
    match c.kind.as_str() {
        "rdp" if !valid_host(c.target.trim()) => Err("Escribe el nombre del equipo o su IP (sin espacios).".into()),
        "anydesk" | "rustdesk" | "teamviewer" if c.target.trim().is_empty() || !c.target.trim().chars().all(|ch| ch.is_ascii_alphanumeric() || " -@._".contains(ch)) => {
            Err("Escribe el ID de la herramienta (solo números y letras).".into())
        }
        "rdp" | "anydesk" | "rustdesk" | "teamviewer" => Ok(()),
        _ => Err("Tipo de conexión no válido.".into()),
    }
}

#[tauri::command]
pub fn save_connection(app: tauri::AppHandle, connection: Connection) -> Result<Connection, String> {
    validate(&connection)?;
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let mut c = connection;
    c.name = c.name.trim().into();
    c.target = c.target.trim().into();
    c.username = c.username.trim().into();
    match list.iter_mut().find(|x| !c.id.is_empty() && x.id == c.id) {
        Some(x) => {
            c.saved_password = x.saved_password && x.target.eq_ignore_ascii_case(&c.target);
            c.last_used = x.last_used;
            *x = c.clone();
        }
        None => {
            c.id = format!("c{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
            list.push(c.clone());
        }
    }
    crate::paths::write_json(&book_path(&app), &list)?;
    Ok(c)
}

#[tauri::command]
pub fn delete_connection(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    if let Some(c) = list.iter().find(|c| c.id == id) {
        if c.kind == "rdp" && c.saved_password {
            let _ = forget_password(&c.target);
        }
    }
    list.retain(|c| c.id != id);
    crate::paths::write_json(&book_path(&app), &list)
}

fn touch(app: &tauri::AppHandle, id: &str) {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(app);
    if let Some(c) = list.iter_mut().find(|c| c.id == id) {
        c.last_used = now();
        let _ = crate::paths::write_json(&book_path(app), &list);
    }
}

// ---------- Contraseña de Escritorio remoto (Administrador de credenciales) ----------

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[cfg(windows)]
fn store_password(host: &str, user: &str, password: &str) -> Result<(), String> {
    use windows_sys::Win32::Security::Credentials::{CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_DOMAIN_PASSWORD};
    let mut target = wide(&format!("TERMSRV/{host}"));
    let mut username = wide(user);
    let blob: Vec<u8> = password.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let cred = CREDENTIALW {
        Type: CRED_TYPE_DOMAIN_PASSWORD,
        TargetName: target.as_mut_ptr(),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: username.as_mut_ptr(),
        ..Default::default()
    };
    if unsafe { CredWriteW(&cred, 0) } == 0 {
        return Err(format!("Windows no guardó la contraseña: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(windows))]
fn store_password(_: &str, _: &str, _: &str) -> Result<(), String> {
    Err("Solo en Windows.".into())
}

#[cfg(windows)]
fn forget_password(host: &str) -> Result<(), String> {
    use windows_sys::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_DOMAIN_PASSWORD};
    let target = wide(&format!("TERMSRV/{host}"));
    unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_DOMAIN_PASSWORD, 0) };
    Ok(())
}

#[cfg(not(windows))]
fn forget_password(_: &str) -> Result<(), String> {
    Ok(())
}

/// Guarda (o quita, con contraseña vacía) la contraseña de una conexión de Escritorio remoto.
#[tauri::command]
pub fn set_connection_password(app: tauri::AppHandle, id: String, password: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let c = list.iter_mut().find(|c| c.id == id).ok_or("La conexión ya no existe.")?;
    if c.kind != "rdp" {
        return Err("Solo las conexiones de Escritorio remoto guardan contraseña.".into());
    }
    if password.is_empty() {
        forget_password(&c.target)?;
        c.saved_password = false;
    } else {
        if c.username.is_empty() {
            return Err("Pon antes el usuario de la conexión.".into());
        }
        store_password(&c.target, &c.username, &password)?;
        c.saved_password = true;
    }
    crate::paths::write_json(&book_path(&app), &list)
}

// ---------- Conectar ----------

/// Archivo .rdp con las opciones elegidas (lo que mstsc guarda al pulsar «Guardar como»).
fn rdp_file(target: &str, user: &str, o: &RdpOptions) -> String {
    let b = |v: bool| if v { 1 } else { 0 };
    let mut lines = vec![
        format!("full address:s:{target}"),
        format!("screen mode id:i:{}", if o.fullscreen { 2 } else { 1 }),
        format!("use multimon:i:{}", b(o.multimon)),
        format!("redirectclipboard:i:{}", b(o.clipboard)),
        format!("redirectprinters:i:{}", b(o.printers)),
        format!("drivestoredirect:s:{}", if o.drives { "*" } else { "" }),
        format!("audiomode:i:{}", if o.audio { 0 } else { 2 }),
        "smart sizing:i:1".into(),
        "dynamic resolution:i:1".into(),
        "prompt for credentials:i:0".into(),
        "authentication level:i:2".into(),
    ];
    if !o.fullscreen {
        lines.push("desktopwidth:i:1600".into());
        lines.push("desktopheight:i:900".into());
    }
    if !user.is_empty() {
        lines.push(format!("username:s:{user}"));
    }
    lines.join("\r\n") + "\r\n"
}

fn launch_rdp(app: &tauri::AppHandle, target: &str, user: &str, o: &RdpOptions) -> Result<(), String> {
    if !valid_host(target) {
        return Err("Escribe un nombre de equipo o una IP.".into());
    }
    if user.contains(['\r', '\n']) {
        return Err("Usuario no válido.".into());
    }
    let dir = crate::paths::machine_data_dir(app).join("rdp");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = target.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect();
    let file = dir.join(format!("{safe}.rdp"));
    // mstsc lee los .rdp en UTF-16.
    let content: Vec<u8> = [0xFFu8, 0xFE].into_iter().chain(rdp_file(target, user, o).encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
    std::fs::write(&file, content).map_err(|e| e.to_string())?;
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    std::process::Command::new(format!("{system}\\System32\\mstsc.exe")).arg(&file).spawn().map(|_| ()).map_err(|e| format!("No se pudo abrir Escritorio remoto: {e}"))
}

/// Conexión rápida por Escritorio remoto (sin guardar).
#[tauri::command]
pub fn connect_rdp(app: tauri::AppHandle, target: String, username: String, options: RdpOptions) -> Result<(), String> {
    launch_rdp(&app, target.trim(), username.trim(), &options)
}

#[tauri::command]
pub fn connect_saved(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let c = load(&app).into_iter().find(|c| c.id == id).ok_or("La conexión ya no existe.")?;
    touch(&app, &id);
    match c.kind.as_str() {
        "rdp" => launch_rdp(&app, &c.target, &c.username, &c.options),
        tool => connect_tool(tool, &c.target),
    }
}

// ---------- Prueba de conexión ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reach {
    pub resolved: Option<String>,
    pub ping_ms: Option<u32>,
    pub rdp_open: bool,
    /// Explicación de lo que falla (vacía si todo va bien).
    pub hint: String,
}

#[tauri::command(async)]
pub fn test_connection(target: String) -> Result<Reach, String> {
    let host = target.trim();
    if !valid_host(host) {
        return Err("Escribe un nombre de equipo o una IP.".into());
    }
    let ip = (host, 3389).to_socket_addrs().ok().and_then(|mut a| {
        a.find_map(|x| match x {
            SocketAddr::V4(v4) => Some(*v4.ip()),
            SocketAddr::V6(_) => None,
        })
    });
    let Some(ip) = ip else {
        return Ok(Reach {
            resolved: None,
            ping_ms: None,
            rdp_open: false,
            hint: "No se encuentra ese nombre en la red. Prueba con la IP, o comprueba que los dos equipos están en la misma red (o conectados por VPN).".into(),
        });
    };
    let (status, _, ms) = crate::network::tools::echo(ip, 64, 1000);
    let ping_ms = (status == 0).then_some(ms);
    let rdp_open = TcpStream::connect_timeout(&SocketAddr::from((ip, 3389)), Duration::from_millis(1500)).is_ok();
    let hint = match (ping_ms.is_some(), rdp_open) {
        (_, true) => String::new(),
        (true, false) => "El equipo responde pero no acepta Escritorio remoto: puede estar desactivado, bloqueado por el firewall o ser Windows Home (que no puede recibirlo). En ese caso usa AnyDesk o RustDesk.".into(),
        (false, false) => "El equipo no responde: puede estar apagado (prueba a encenderlo por la red), en otra red o con el firewall bloqueando todo.".into(),
    };
    Ok(Reach { resolved: Some(ip.to_string()), ping_ms, rdp_open, hint })
}

// ---------- Herramientas de asistencia ----------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// anydesk | rustdesk | teamviewer
    pub id: String,
    pub name: String,
    pub installed: bool,
    /// ID de este equipo en la herramienta (para dárselo a quien te asiste).
    pub this_id: Option<String>,
}

fn find_exe(candidates: &[&str]) -> Option<PathBuf> {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    candidates
        .iter()
        .map(|c| PathBuf::from(c.replace("{PF}", &env("ProgramFiles")).replace("{PF86}", &env("ProgramFiles(x86)")).replace("{PD}", &env("ProgramData")).replace("{LAD}", &env("LOCALAPPDATA"))))
        .find(|p| p.is_file())
}

fn anydesk() -> Option<PathBuf> {
    find_exe(&[r"{PF86}\AnyDesk\AnyDesk.exe", r"{PF}\AnyDesk\AnyDesk.exe", r"{PD}\AnyDesk\AnyDesk.exe", r"{LAD}\AnyDesk\AnyDesk.exe"])
}

fn rustdesk() -> Option<PathBuf> {
    find_exe(&[r"{PF}\RustDesk\rustdesk.exe", r"{PF86}\RustDesk\rustdesk.exe", r"{LAD}\RustDesk\rustdesk.exe"])
}

fn teamviewer() -> Option<PathBuf> {
    find_exe(&[r"{PF}\TeamViewer\TeamViewer.exe", r"{PF86}\TeamViewer\TeamViewer.exe"])
}

fn run_for_id(exe: &std::path::Path, arg: &str) -> Option<String> {
    let out = crate::ps::exec_opts(&exe.display().to_string(), &[arg], crate::ps::Opts { timeout: Some(Duration::from_secs(8)), task: None }).ok()?;
    let id: String = out.trim().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    (id.len() >= 6).then_some(id)
}

fn teamviewer_id() -> Option<String> {
    [r"HKLM\SOFTWARE\TeamViewer", r"HKLM\SOFTWARE\WOW6432Node\TeamViewer"]
        .iter()
        .find_map(|k| crate::tweaks::registry::read_u32(k, "ClientID"))
        .filter(|id| *id > 0)
        .map(|id| id.to_string())
}

#[tauri::command(async)]
pub fn remote_tools() -> Vec<Tool> {
    let any = anydesk();
    let rust = rustdesk();
    vec![
        Tool { id: "anydesk".into(), name: "AnyDesk".into(), installed: any.is_some(), this_id: any.as_ref().and_then(|e| run_for_id(e, "--get-id")) },
        Tool { id: "rustdesk".into(), name: "RustDesk".into(), installed: rust.is_some(), this_id: rust.as_ref().and_then(|e| run_for_id(e, "--get-id")) },
        Tool { id: "teamviewer".into(), name: "TeamViewer".into(), installed: teamviewer().is_some(), this_id: teamviewer_id() },
    ]
}

fn connect_tool(tool: &str, target: &str) -> Result<(), String> {
    let id = target.trim().replace(' ', "");
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || "-@._".contains(c)) {
        return Err("ID no válido.".into());
    }
    let (exe, args): (Option<PathBuf>, Vec<String>) = match tool {
        "anydesk" => (anydesk(), vec![id]),
        "rustdesk" => (rustdesk(), vec!["--connect".into(), id]),
        "teamviewer" => (teamviewer(), vec!["-i".into(), id]),
        _ => return Err("Herramienta desconocida.".into()),
    };
    let exe = exe.ok_or("Esa herramienta no está instalada en este equipo.")?;
    std::process::Command::new(exe).args(args).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn connect_tool_id(tool: String, target: String) -> Result<(), String> {
    connect_tool(&tool, &target)
}

#[tauri::command]
pub fn open_remote_tool(tool: String) -> Result<(), String> {
    let exe = match tool.as_str() {
        "anydesk" => anydesk(),
        "rustdesk" => rustdesk(),
        "teamviewer" => teamviewer(),
        _ => None,
    }
    .ok_or("Esa herramienta no está instalada en este equipo.")?;
    std::process::Command::new(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn install_remote_tool(app: tauri::AppHandle, tweaks: State<'_, TweakState>, tool: String) -> Result<(), String> {
    let (id, name) = match tool.as_str() {
        "anydesk" => ("AnyDesk.AnyDesk", "AnyDesk"),
        "rustdesk" => ("RustDesk.RustDesk", "RustDesk"),
        "teamviewer" => ("TeamViewer.TeamViewer", "TeamViewer"),
        _ => return Err("Herramienta desconocida.".into()),
    };
    let task = crate::task::Task::new(&app, "remote-install").named(format!("Instalar {name}"));
    task.step(format!("Instalando {name}…"));
    let r = crate::ps::exec_opts(
        "winget",
        &["install", "--id", id, "-e", "--source", "winget", "--accept-package-agreements", "--accept-source-agreements", "--silent", "--disable-interactivity"],
        task.opts(Some(Duration::from_secs(600))),
    )
    .map(|_| ());
    tweaks.record(Op::Run, &format!("Instalar {name}"), &r);
    r
}

// ---------- Escritorio remoto de este equipo ----------

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RdpServer {
    pub port: u32,
    pub nla: bool,
    /// Miembros del grupo «Usuarios de escritorio remoto» (los administradores entran siempre).
    pub users: Vec<String>,
}

const RDP_USERS_SCRIPT: &str = r#"
$g = Get-LocalGroup -SID 'S-1-5-32-555' -ErrorAction SilentlyContinue
$users = if ($g) { @(Get-LocalGroupMember -Group $g -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Name)" }) } else { @() }
ConvertTo-Json -InputObject @($users) -Compress
"#;

#[tauri::command(async)]
pub fn rdp_server() -> Result<RdpServer, String> {
    use crate::tweaks::registry::read_u32;
    let out = crate::pspool::query(RDP_USERS_SCRIPT, Some(Duration::from_secs(20)), "Escritorio remoto: usuarios")?;
    Ok(RdpServer {
        port: read_u32(r"HKLM\SYSTEM\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp", "PortNumber").unwrap_or(3389),
        nla: read_u32(r"HKLM\SYSTEM\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp", "UserAuthentication").unwrap_or(1) == 1,
        users: serde_json::from_str(out.trim()).unwrap_or_default(),
    })
}

/// Da o quita permiso de Escritorio remoto a un usuario local.
#[tauri::command(async)]
pub fn set_rdp_user(tweaks: State<'_, TweakState>, user: String, allow: bool) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n{}$g = Get-LocalGroup -SID 'S-1-5-32-555'\n{}-LocalGroupMember -Group $g -Member $user\n'ok'",
        crate::ps::text_var("user", user.trim()),
        if allow { "Add" } else { "Remove" }
    );
    let r = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, &format!("Escritorio remoto: {} a «{}»", if allow { "permitir" } else { "quitar" }, user.trim()), &r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rdp_files_and_validation() {
        let f = rdp_file("192.168.1.20", "ANA\\admin", &RdpOptions { fullscreen: true, clipboard: true, ..Default::default() });
        assert!(f.contains("full address:s:192.168.1.20"));
        assert!(f.contains("screen mode id:i:2"));
        assert!(f.contains("redirectclipboard:i:1"));
        assert!(f.contains("username:s:ANA\\admin"));
        assert!(!f.contains("desktopwidth"));
        let ok = |kind: &str, target: &str| validate(&Connection { name: "x".into(), kind: kind.into(), target: target.into(), ..Default::default() }).is_ok();
        assert!(ok("rdp", "PC-RECEPCION"));
        assert!(!ok("rdp", "a b"));
        assert!(ok("anydesk", "123 456 789"));
        assert!(!ok("anydesk", "1;calc"));
        assert!(!ok("ftp", "x"));
    }

    /// Equipo real: `cargo test remote_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn remote_real() {
        for t in remote_tools() {
            println!("{} instalado={} id={:?}", t.name, t.installed, t.this_id);
        }
        let r = test_connection("10.0.0.1".into()).unwrap();
        println!("router: ping {:?} rdp {} · {}", r.ping_ms, r.rdp_open, r.hint);
        let r = test_connection("no-existe-este-equipo".into()).unwrap();
        println!("inexistente: {}", r.hint);
    }

    #[test]
    fn script_parses() {
        assert!(crate::ps::parse_errors(RDP_USERS_SCRIPT).is_empty());
    }
}
