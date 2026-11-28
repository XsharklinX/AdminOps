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

/// Por qué es portable: "marker" (archivo junto al exe), "removable" (el
/// programa está en un pendrive) o "" (instalado en el equipo).
fn portable_state() -> &'static (Option<PathBuf>, &'static str) {
    static STATE: OnceLock<(Option<PathBuf>, &'static str)> = OnceLock::new();
    STATE.get_or_init(|| {
        let Some(dir) = exe_dir() else { return (None, "") };
        if dir.join(MARKER).is_file() {
            (Some(dir.join("AdminOps-data")), "marker")
        } else if crate::storage::on_removable_drive(&dir) {
            // Instalado en un pendrive: los datos viajan con él (ver storage.rs).
            (Some(dir.join("AdminOps-data")), "removable")
        } else {
            (None, "")
        }
    })
}

/// Carpeta portable, si el marcador existe junto al ejecutable o está en un pendrive.
fn portable_root() -> Option<&'static PathBuf> {
    portable_state().0.as_ref()
}

pub fn portable_reason() -> &'static str {
    portable_state().1
}

/// El marcador pide traer los datos de este equipo la próxima vez que arranque.
const MIGRATE_REQUEST: &str = "migrar";

pub fn marker_requests_migration() -> bool {
    exe_dir().and_then(|d| std::fs::read_to_string(d.join(MARKER)).ok()).is_some_and(|t| t.trim() == MIGRATE_REQUEST)
}

pub fn clear_migration_request() {
    if marker_requests_migration() {
        if let Some(d) = exe_dir() {
            let _ = std::fs::write(d.join(MARKER), "");
        }
    }
}

/// ¿Se puede escribir junto al programa? (En Archivos de programa, solo como administrador.)
pub fn exe_dir_writable() -> bool {
    exe_dir().is_some_and(|d| {
        let probe = d.join(".adminops-write-test");
        let ok = std::fs::write(&probe, b"ok").is_ok();
        let _ = std::fs::remove_file(&probe);
        ok
    })
}

/// Pasa a portable al volver a abrir AdminOps, trayendo los datos de este equipo.
pub fn request_portable_with_migration() -> Result<(), String> {
    let dir = exe_dir().ok_or("No se encuentra la carpeta del programa.")?;
    std::fs::write(dir.join(MARKER), MIGRATE_REQUEST).map_err(|_| {
        "No se puede escribir en la carpeta del programa. Si está en Archivos de programa, abre AdminOps como administrador (o instálalo en el pendrive).".to_string()
    })
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
/// Dónde va el perfil del navegador interno en portable: «pendrive» o «equipo».
const BROWSER_PLACE: &str = "navegador.txt";

/// ¿El navegador interno (sesiones de Correo, Teams, Tickets) se guarda en el
/// pendrive? Por defecto:
/// - AdminOps **instalado en el pendrive**: no, en el disco de cada equipo.
///   Teams y Outlook escriben miles de archivos pequeños y en un pendrive van
///   lentísimos (páginas en blanco, arranque de 40 s); y sus sesiones solo
///   valen en el equipo donde se iniciaron, así que en el pendrive no viajaban.
/// - **El portable** (el .zip, pensado para equipos de clientes): sí, para no
///   dejar rastro.
///
/// Se cambia en Ajustes → Datos.
pub fn browser_on_usb() -> bool {
    let Some(root) = portable_root() else { return false };
    match std::fs::read_to_string(root.join(BROWSER_PLACE)).map(|s| s.trim().to_lowercase()) {
        Ok(v) if v == "pendrive" => true,
        Ok(v) if v == "equipo" => false,
        _ => portable_reason() == "marker",
    }
}

pub fn set_browser_on_usb(on: bool) -> Result<(), String> {
    let root = portable_root().ok_or("Solo cuando AdminOps guarda los datos junto al programa.")?;
    std::fs::write(root.join(BROWSER_PLACE), if on { "pendrive" } else { "equipo" }).map_err(|e| e.to_string())
}

pub fn portable_webview_dir() -> Option<PathBuf> {
    if !browser_on_usb() {
        return None;
    }
    let root = portable_root()?;
    // Una por equipo: las sesiones de los portales van cifradas por Windows para
    // cada equipo, y una sola carpeta compartida obligaba a volver a entrar en
    // todo al cambiar de PC (y otra vez al volver). Así, una vez en cada PC.
    let dir = root.join("equipos").join(host()).join("webview");
    let legacy = root.join("webview");
    if !dir.exists() && legacy.is_dir() {
        let _ = std::fs::create_dir_all(dir.parent().unwrap_or(root));
        let _ = std::fs::rename(&legacy, &dir);
    }
    Some(dir).filter(|d| is_writable(d))
}

/// Al arrancar en portable, antes de WebView2: traer los datos de este equipo
/// si el pendrive aún no tiene (ver storage.rs).
pub fn migrate_to_portable_if_needed() -> Option<crate::storage::Migrated> {
    let root = portable_root()?;
    crate::storage::migrate_if_needed(root, &host())
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
    // Modo técnico con la cuenta de otro (el técnico puso su contraseña en el PC de un usuario): esa cuenta puede no
    // tener su perfil cargado del todo en este equipo y WebView2 no abre su carpeta («can't read and write to its data
    // directory»). Se usa una carpeta de este equipo para esa cuenta, en ProgramData, que el administrador siempre
    // puede escribir.
    let elevated = crate::elevation::is_elevated();
    if elevated && elevated_as_other_user() {
        if let (Some(pd), Some(own)) = (std::env::var_os("ProgramData").map(PathBuf::from), crate::target_user::own_sid()) {
            let dir = pd.join("AdminOps").join("WebView").join(own);
            // Cerrada: solo SYSTEM, los administradores y esta cuenta (ProgramData deja leer y crear a cualquiera, y
            // ahí van la caché y los datos de los portales del técnico). Si alguien la creó antes, deja de ser suya.
            if is_writable(&dir) && lock_to_admins(&dir, own) && is_writable(&dir.join("EBWebView")) {
                return Some(dir);
            }
        }
    }
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    // Modo técnico en una carpeta aparte: la de modo usuario puede seguir abierta un momento por la instancia que se
    // cierra, y WebView2 no comparte su carpeta entre un proceso elevado y otro que no lo está.
    let preferred = local.join("AdminOps").join(if elevated { "WebView-tecnico" } else { "WebView" });
    // También la subcarpeta que crea WebView2: si quedó de otra cuenta, la de arriba se puede escribir y esta no.
    if is_writable(&preferred) && is_writable(&preferred.join("EBWebView")) {
        return Some(preferred);
    }
    let fallback = std::env::temp_dir().join("AdminOps").join("WebView");
    if is_writable(&fallback) && is_writable(&fallback.join("EBWebView")) {
        return Some(fallback);
    }
    // Última salida: una carpeta nueva que no puede estar heredada de nadie.
    let fresh = std::env::temp_dir().join("AdminOps").join(format!("WebView-{}", std::process::id()));
    is_writable(&fresh).then_some(fresh)
}

/// Deja la carpeta solo para SYSTEM, los administradores y `sid`, sin herencia, con los administradores de dueños.
fn lock_to_admins(dir: &std::path::Path, sid: &str) -> bool {
    let d = dir.to_string_lossy().into_owned();
    let own = crate::ps::exec("icacls", &[&d, "/setowner", "*S-1-5-32-544", "/T", "/C", "/Q"]).is_ok();
    let grant = format!("*{sid}:(OI)(CI)F");
    own && crate::ps::exec("icacls", &[&d, "/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F", &grant, "/T", "/C", "/Q"]).is_ok()
}

/// ¿Esta AdminOps se elevó con una cuenta distinta de la del usuario de la sesión?
fn elevated_as_other_user() -> bool {
    matches!((crate::target_user::own_sid(), crate::target_user::get()), (Some(own), Some(t)) if own != t.sid)
}

pub fn host() -> String {
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
    // Los datos cifrados (Ajustes → Seguridad) se abren aquí; sin la clave se leen como vacíos.
    std::fs::read(path).ok().and_then(crate::datacrypt::read).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Escribe un JSON de forma atómica (archivo temporal + renombrado).
pub fn write_json<T: Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Temporal único por proceso e hilo: la app y el cohete pueden escribir el mismo JSON a la vez.
    static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{}-{seq}.tmp", std::process::id()));
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    // Cifrado si toca; y con lo cifrado bloqueado, un error en lugar de pisarlo con lo vacío.
    let bytes = crate::datacrypt::for_write(path, json.into_bytes())?;
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Bloqueo entre procesos (la app y el cohete) con un mutex con nombre de Windows, para las lecturas-modificaciones-
/// escrituras de los JSON compartidos. Espera 5 s como mucho; si no lo consigue (o no puede crearlo), sigue sin él:
/// el mutex en memoria de cada módulo protege los hilos del mismo proceso. Fuera de Windows no hace nada.
pub struct CrossProcessLock {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

impl CrossProcessLock {
    /// `name` sin prefijo, p. ej. "AdminOpsNotas" (se crea como `Local\AdminOpsNotas`).
    #[cfg(windows)]
    pub fn acquire(name: &str) -> Self {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, LocalFree, ERROR_ACCESS_DENIED, WAIT_ABANDONED, WAIT_OBJECT_0};
        use windows_sys::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
        use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
        use windows_sys::Win32::System::Threading::{CreateMutexW, OpenMutexW, WaitForSingleObject, MUTEX_MODIFY_STATE};
        const SYNCHRONIZE: u32 = 0x0010_0000;
        let wide: Vec<u16> = format!("Local\\{name}").encode_utf16().chain(std::iter::once(0)).collect();
        // El cohete (sin elevar) y la app en modo técnico (elevada) comparten el mutex. Con los permisos por defecto,
        // el que crea uno elevado solo lo abren los administradores y el otro se quedaría sin bloqueo justo cuando hay
        // competencia. Por eso: control total para SYSTEM y Administradores, esperar y soltar para cualquier usuario
        // autenticado, y etiqueta de integridad media (sin ella el de un proceso elevado sería «alta» y no se podría
        // soltar desde uno normal).
        let sddl: Vec<u16> = "D:(A;;0x1F0001;;;SY)(A;;0x1F0001;;;BA)(A;;0x100001;;;AU)S:(ML;;NW;;;ME)".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` y `sddl` terminan en 0 y viven durante las llamadas; `sd` se libera con LocalFree; el handle se
        // cierra en Drop o aquí.
        unsafe {
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            let have_sd = ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut()) != 0;
            let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd, bInheritHandle: 0 };
            let mut handle = CreateMutexW(if have_sd { &sa } else { std::ptr::null() }, 0, wide.as_ptr());
            // Ya existe y lo creó otro con menos permisos para nosotros (CreateMutexW pide todos): se abre con lo justo.
            if handle.is_null() && GetLastError() == ERROR_ACCESS_DENIED {
                handle = OpenMutexW(SYNCHRONIZE | MUTEX_MODIFY_STATE, 0, wide.as_ptr());
            }
            if have_sd {
                LocalFree(sd as _);
            }
            if handle.is_null() {
                return Self { handle: std::ptr::null_mut() };
            }
            let r = WaitForSingleObject(handle, 5000);
            if r == WAIT_OBJECT_0 || r == WAIT_ABANDONED {
                Self { handle }
            } else {
                CloseHandle(handle);
                Self { handle: std::ptr::null_mut() }
            }
        }
    }

    #[cfg(not(windows))]
    pub fn acquire(_name: &str) -> Self {
        Self {}
    }
}

#[cfg(windows)]
impl Drop for CrossProcessLock {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::Threading::ReleaseMutex;
            // SAFETY: el handle es nuestro y el mutex lo tiene este mismo hilo (se adquiere y se suelta en él).
            unsafe {
                ReleaseMutex(self.handle);
                CloseHandle(self.handle);
            }
        }
    }
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
pub fn log_frontend_error(app: tauri::AppHandle, message: String) {
    log::error!("Interfaz: {}", message.chars().take(4000).collect::<String>());
    // Y en el registro de errores de la app (agrupado, sin datos personales).
    crate::errorlog::note(&app, "pantalla", "interfaz", &message);
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

/// Nombre temporal único por proceso y escritura, para que el cohete y la app no se pisen al guardar el mismo archivo.
pub fn tmp_for(path: &std::path::Path) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("dat");
    path.with_extension(format!("{ext}.{}-{seq}.tmp", std::process::id()))
}

#[cfg(all(test, windows))]
mod tests {
    use super::CrossProcessLock;

    #[test]
    fn el_mutex_entre_procesos_se_toma_y_se_suelta() {
        let name = format!("AdminOpsPrueba{}", std::process::id());
        let a = CrossProcessLock::acquire(&name);
        // El descriptor (SDDL) es válido: si no, no habría mutex.
        assert!(!a.handle.is_null());
        drop(a);
        // Otro hilo lo vuelve a tomar enseguida: se soltó bien.
        let n = name.clone();
        let ok = std::thread::spawn(move || !CrossProcessLock::acquire(&n).handle.is_null()).join().unwrap();
        assert!(ok);
    }
}
