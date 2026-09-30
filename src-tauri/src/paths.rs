//! Dónde guarda AdminOps sus datos.
//!
//! - **Instalado**: `%APPDATA%\com.adminops.app\` y los informes en `Documentos\AdminOps\Informes`.
//! - **Portable** (hay un archivo `AdminOps.portable` junto al .exe): todo en
//!   `<carpeta del exe>\AdminOps-data\`, sin dejar rastro en el equipo del cliente.
//!   El diario y los análisis se separan por equipo, porque las copias de
//!   seguridad del diario solo sirven para el PC donde se hicieron.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::Manager;

const MARKER: &str = "AdminOps.portable";

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}

/// Carpeta portable, si el marcador existe junto al ejecutable.
fn portable_root() -> Option<&'static PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        let dir = exe_dir()?;
        dir.join(MARKER).is_file().then(|| dir.join("AdminOps-data"))
    })
    .as_ref()
}

/// Carpeta de datos del USB (solo en portable).
pub fn portable_data_root() -> Option<&'static PathBuf> {
    portable_root()
}

pub fn is_portable() -> bool {
    portable_root().is_some()
}

/// En portable, la caché de WebView2 (cookies, sesiones de Tickets…) también va
/// al USB para no dejar rastro en el equipo del cliente.
///
/// La carpeta se crea y se comprueba aquí, antes de dársela a WebView2: si el
/// USB viene protegido contra escritura (o el antivirus del cliente bloquea la
/// primera creación), WebView2 no arrancaría y la ventana se quedaría en negro.
/// En ese caso no se devuelve nada y se usa la carpeta del equipo.
pub fn portable_webview_dir() -> Option<PathBuf> {
    portable_root().map(|root| root.join("webview")).filter(|d| is_writable(d))
}

/// ¿Se puede crear y escribir en esta carpeta? (Una carpeta heredada de otro
/// usuario, o bajo acceso controlado, da «acceso denegado» y WebView2 no abre.)
fn is_writable(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".adminops-write-test");
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Carpeta de datos de WebView2 en modo instalado. WebView2 no arranca si su
/// carpeta por defecto quedó de otro usuario (p. ej. de una instalación como
/// administrador): por eso se fija una propia bajo el perfil de ESTE usuario, y
/// si no se puede escribir, se cae a la carpeta temporal. Portable la lleva al USB.
pub fn installed_webview_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let preferred = local.join("AdminOps").join("WebView");
    if is_writable(&preferred) {
        return Some(preferred);
    }
    let fallback = std::env::temp_dir().join("AdminOps").join("WebView");
    is_writable(&fallback).then_some(fallback)
}

fn host() -> String {
    sysinfo::System::host_name()
        .unwrap_or_else(|| "equipo".into())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

/// En portable, la carpeta de este equipo dentro del USB.
fn portable_machine_dir() -> Option<PathBuf> {
    portable_root().map(|root| root.join("equipos").join(host()))
}

/// Datos propios de ESTE equipo (diario de cambios, análisis).
pub fn machine_data_dir(app: &tauri::AppHandle) -> PathBuf {
    match portable_machine_dir() {
        Some(dir) => dir,
        None => app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("AdminOps")),
    }
}

/// Datos compartidos entre equipos: ajustes del técnico, clientes y perfiles
/// propios. En portable viajan en el USB; instalado, en %APPDATA%.
pub fn shared_data_dir(app: &tauri::AppHandle) -> PathBuf {
    match portable_root() {
        Some(root) => root.to_path_buf(),
        None => app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("AdminOps")),
    }
}

/// Lo mismo que `shared_data_dir`, antes de que exista la app (al arrancar).
/// Instalado: %APPDATA%\<identificador>, igual que la ruta que usa Tauri.
pub fn shared_data_dir_early() -> Option<PathBuf> {
    match portable_root() {
        Some(root) => Some(root.to_path_buf()),
        None => std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("com.adminops.app")),
    }
}

/// Copias de seguridad de drivers, junto a los informes.
pub fn drivers_backup_dir(app: &tauri::AppHandle) -> PathBuf {
    reports_dir(app).parent().map_or_else(|| reports_dir(app), PathBuf::from).join("Drivers")
}

/// Lee un JSON (o el valor por defecto si no existe o está dañado).
pub fn read_json<T: serde::de::DeserializeOwned + Default>(path: &std::path::Path) -> T {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Escribe un JSON de forma atómica (archivo temporal + renombrado).
pub fn write_json<T: Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Informes PDF. En portable se juntan los de todos los equipos.
pub fn reports_dir(app: &tauri::AppHandle) -> PathBuf {
    match portable_root() {
        Some(root) => root.join("Informes"),
        None => app
            .path()
            .document_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("AdminOps")
            .join("Informes"),
    }
}

pub const LOG_FILE: &str = "adminops";

/// Destino del registro de actividad para el plugin de logs. Se decide antes
/// de crear la app, así que en modo instalado usa la carpeta estándar de logs.
pub fn log_target() -> tauri_plugin_log::TargetKind {
    match portable_machine_dir() {
        Some(dir) => tauri_plugin_log::TargetKind::Folder { path: dir.join("logs"), file_name: Some(LOG_FILE.into()) },
        None => tauri_plugin_log::TargetKind::LogDir { file_name: Some(LOG_FILE.into()) },
    }
}

pub fn logs_dir(app: &tauri::AppHandle) -> PathBuf {
    match portable_machine_dir() {
        Some(dir) => dir.join("logs"),
        None => app.path().app_log_dir().unwrap_or_else(|_| std::env::temp_dir().join("AdminOps").join("logs")),
    }
}

/// Errores de la interfaz (página que falla, promesas sin capturar) al registro técnico.
#[tauri::command]
pub fn log_frontend_error(message: String) {
    log::error!("Interfaz: {}", message.chars().take(4000).collect::<String>());
}

/// Últimas `lines` líneas del registro de actividad.
#[tauri::command(async)]
pub fn read_log(app: tauri::AppHandle, lines: usize) -> String {
    let path = logs_dir(&app).join(format!("{LOG_FILE}.log"));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines.clamp(1, 5000))..].join("\n")
}

/// Abre una carpeta de AdminOps sin mostrar su ruta en la interfaz (contiene
/// el nombre de usuario de Windows del equipo).
#[tauri::command]
pub fn open_app_folder(app: tauri::AppHandle, kind: String) -> Result<(), String> {
    let dir = match kind.as_str() {
        "data" => machine_data_dir(&app),
        "reports" => reports_dir(&app),
        "logs" => logs_dir(&app),
        _ => return Err("Carpeta desconocida".into()),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_logs_folder(app: tauri::AppHandle) -> Result<(), String> {
    let dir = logs_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    portable: bool,
    data_dir: String,
    reports_dir: String,
    /// Página inicial para capturas y mediciones (`ADMINOPS_START_PAGE`, ver scripts/bench.ps1).
    start_page: Option<String>,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        portable: is_portable(),
        data_dir: machine_data_dir(&app).display().to_string(),
        reports_dir: reports_dir(&app).display().to_string(),
        start_page: std::env::var("ADMINOPS_START_PAGE").ok().filter(|p| p.len() < 200),
    }
}
