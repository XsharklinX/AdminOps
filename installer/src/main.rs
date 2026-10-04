//! Instalador de AdminOps con interfaz propia.
//!
//! Lleva dentro el instalador NSIS estándar de Tauri y lo ejecuta en silencio
//! (`/S /D=carpeta`): así se conserva todo lo que hace bien (desinstalador,
//! entrada en "Aplicaciones instaladas", accesos del menú Inicio) con una
//! presentación a la altura de la app. Si el equipo no tiene WebView2, se abre
//! directamente el instalador clásico, que lo instala.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::Emitter;

const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.exe"));
const VERSION: &str = env!("APP_VERSION");
const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AdminOps";
/// Huella del adminops.exe de este instalador ("" en desarrollo).
const EXE_SHA256: &str = env!("APP_EXE_SHA256");

/// ¿El adminops.exe de esa carpeta es exactamente el que trae este instalador?
/// NSIS en silencio puede no poder sobrescribirlo (algo lo tenía abierto) y
/// terminar sin error; como las dos builds tienen la misma versión, eso pasaba
/// por «actualizado» y se seguía usando la anterior.
fn exe_is_current(dir: &Path) -> bool {
    use sha2::Digest;
    if EXE_SHA256.is_empty() {
        return true;
    }
    std::fs::read(dir.join("adminops.exe")).is_ok_and(|b| sha2::Sha256::digest(&b).iter().map(|x| format!("{x:02x}")).collect::<String>() == EXE_SHA256)
}

fn parse_version(v: &str) -> Vec<u32> {
    v.split(['.', '-']).map(|p| p.parse().unwrap_or(0)).collect()
}

/// ¿La versión instalada es más nueva que la de este instalador?
///
/// Hubo una build con el número 2.0.0 por un salto de numeración: en realidad es
/// anterior a la 1.2.2 (la serie sigue en 1.2.x). Quien la tenga instalada debe
/// poder actualizar; cualquier otra versión mayor sí bloquea, como siempre.
fn is_downgrade(installed: &str, ours: &str) -> bool {
    parse_version(installed) != [2, 0, 0] && parse_version(installed) > parse_version(ours)
}

/// Versión instalada y carpeta, leídas de la entrada que crea el instalador NSIS.
#[cfg(windows)]
fn installed() -> Option<(String, PathBuf)> {
    use winreg::enums::*;
    let key = winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(UNINSTALL_KEY, KEY_READ | KEY_WOW64_64KEY).ok()?;
    let version: String = key.get_value("DisplayVersion").ok()?;
    let uninstall: String = key.get_value("UninstallString").ok()?;
    let exe = PathBuf::from(uninstall.trim().trim_matches('"'));
    Some((version, exe.parent()?.to_path_buf()))
}

#[cfg(not(windows))]
fn installed() -> Option<(String, PathBuf)> {
    None
}

/// AdminOps instalado en otra unidad (el pendrive) que este equipo no tiene
/// registrado: en cada PC el pendrive puede tener otra letra y Windows solo
/// recuerda la instalación del equipo donde se hizo. Primero la unidad desde la
/// que se abre este instalador.
fn installed_on_other_drive() -> Option<PathBuf> {
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    let own = std::env::current_exe().ok().and_then(|e| e.to_str().and_then(|s| s.chars().next())).map(|c| c.to_ascii_uppercase());
    let letters = own.into_iter().chain('D'..='Z');
    for c in letters {
        if format!("{c}:") == system {
            continue;
        }
        let dir = PathBuf::from(format!("{c}:\\AdminOps"));
        if dir.join("adminops.exe").is_file() {
            return Some(dir);
        }
    }
    None
}

fn default_dir() -> PathBuf {
    PathBuf::from(std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into())).join("AdminOps")
}

fn app_running() -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| p.name().eq_ignore_ascii_case("adminops.exe"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupInfo {
    version: String,
    installed_version: Option<String>,
    install_dir: String,
    /// La versión instalada es más nueva: no se permite "actualizar" hacia atrás.
    downgrade: bool,
    /// Se encontró AdminOps en otra unidad (el pendrive) sin registrar en este equipo.
    found_on_drive: bool,
    app_running: bool,
}

#[tauri::command]
fn setup_info() -> SetupInfo {
    // La instalación que recuerda Windows, si su carpeta sigue ahí (la de un
    // pendrive puede haber cambiado de letra).
    let current = installed().filter(|(_, d)| d.join("adminops.exe").is_file());
    let other = if current.is_none() { installed_on_other_drive() } else { None };
    SetupInfo {
        version: VERSION.into(),
        downgrade: current.as_ref().is_some_and(|(v, _)| is_downgrade(v, VERSION)),
        install_dir: current.as_ref().map(|(_, d)| d.clone()).or_else(|| other.clone()).unwrap_or_else(default_dir).display().to_string(),
        installed_version: current.map(|(v, _)| v),
        found_on_drive: other.is_some(),
        app_running: app_running(),
    }
}

#[tauri::command(async)]
fn pick_dir(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.join("AdminOps").display().to_string())
}

fn validate_dir(dir: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(dir.trim());
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()).to_lowercase();
    let lower = p.display().to_string().to_lowercase();
    if !p.is_absolute() || p.components().count() < 2 || p.parent().is_none() {
        return Err("Elige una carpeta, no la raíz de un disco.".into());
    }
    if lower.starts_with(&windows) || dir.contains(['"', '\n', '\r']) {
        return Err("Esa carpeta no es válida para instalar programas.".into());
    }
    Ok(p)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    percent: u32,
    step: String,
}

/// Cierra AdminOps y todo lo que retiene sus archivos: el árbol de procesos
/// (sus PowerShell en segundo plano) y cualquier PowerShell que tenga cargadas
/// las DLL de sensores de la carpeta de instalación. Si no, el instalador
/// silencioso no puede reemplazarlas y aborta sin decir nada.
fn stop_app(dir: &Path) {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("taskkill.exe").args(["/F", "/T", "/IM", "adminops.exe"]).creation_flags(NO_WINDOW).status();
    let d = dir.display().to_string().replace('\'', "''");
    // También el navegador interno de AdminOps (msedgewebview2 con su perfil):
    // si queda vivo, el AdminOps nuevo se engancha a él y Correo y Teams no cargan.
    let script = format!(
        r#"Get-Process powershell, pwsh -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne $PID -and ($_.Modules.FileName -like '{d}\*') }} | Stop-Process -Force -ErrorAction SilentlyContinue
Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" -ErrorAction SilentlyContinue | Where-Object {{ $_.CommandLine -like '*\AdminOps\WebView*' -or $_.CommandLine -like '*\AdminOps-data\*' -or $_.CommandLine -like '*\com.adminops.app\*' }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }}"#
    );
    let _ = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(NO_WINDOW)
        .status();
    let start = Instant::now();
    while app_running() && start.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(300));
    }
    std::thread::sleep(Duration::from_millis(500));
    // Y lo que quede con algún archivo de la carpeta abierto, sea lo que sea.
    let left = free_files(dir);
    if !left.is_empty() {
        eprintln!("Siguen con archivos abiertos: {}", names(&left));
    }
}

/// Un programa que tiene abierto algún archivo de la carpeta de AdminOps.
struct Locker {
    pid: u32,
    name: String,
    /// Servicio o parte de Windows: no se cierra a la fuerza, se dice.
    protected: bool,
}

/// Todos los archivos de la carpeta (hasta un tope: con el programa, sus DLL y el
/// desinstalador sobra).
fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(d) = pending.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                pending.push(p);
            } else {
                out.push(p);
            }
            if out.len() >= 2000 {
                return out;
            }
        }
    }
    out
}

/// Pregunta a Windows (Restart Manager, lo que usan los instaladores de Windows)
/// qué programas tienen abiertos los archivos de la carpeta. Así se cierra lo
/// que de verdad bloquea, sea lo que sea, y no solo lo que se adivina por nombre.
#[cfg(windows)]
fn lockers(dir: &Path) -> Vec<Locker> {
    use windows_sys::Win32::System::RestartManager::{RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY, RM_PROCESS_INFO};
    let files: Vec<Vec<u16>> = files_in(dir).iter().map(|p| p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()).collect();
    if files.is_empty() {
        return Vec::new();
    }
    let ptrs: Vec<*const u16> = files.iter().map(|f| f.as_ptr()).collect();
    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    unsafe {
        if RmStartSession(&mut session, 0, key.as_mut_ptr()) != 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        if RmRegisterResources(session, ptrs.len() as u32, ptrs.as_ptr(), 0, std::ptr::null(), 0, std::ptr::null()) == 0 {
            let (mut needed, mut count, mut reasons) = (0u32, 0u32, 0u32);
            // Primera vez: cuántos son. Segunda: la lista.
            let _ = RmGetList(session, &mut needed, &mut count, std::ptr::null_mut(), &mut reasons);
            if needed > 0 {
                let mut info: Vec<RM_PROCESS_INFO> = vec![std::mem::zeroed(); needed as usize];
                count = needed;
                if RmGetList(session, &mut needed, &mut count, info.as_mut_ptr(), &mut reasons) == 0 {
                    for i in &info[..count as usize] {
                        let len = i.strAppName.iter().position(|&c| c == 0).unwrap_or(i.strAppName.len());
                        // 3 = servicio, 4 = el Explorador, 1000 = crítico para Windows.
                        let protected = matches!(i.ApplicationType, 3 | 4 | 1000);
                        out.push(Locker { pid: i.Process.dwProcessId, name: String::from_utf16_lossy(&i.strAppName[..len]), protected });
                    }
                }
            }
        }
        RmEndSession(session);
        out
    }
}

#[cfg(not(windows))]
fn lockers(_: &Path) -> Vec<Locker> {
    Vec::new()
}

/// Cierra lo que tiene abiertos los archivos (menos este instalador y lo que es
/// de Windows). Devuelve lo que no se pudo o no se debía cerrar.
#[cfg(windows)]
fn free_files(dir: &Path) -> Vec<Locker> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    let me = std::process::id();
    let mut left = Vec::new();
    for l in lockers(dir) {
        if l.pid == me {
            continue;
        }
        if l.protected {
            left.push(l);
            continue;
        }
        let done = unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, 0, l.pid);
            if h.is_null() {
                false
            } else {
                let ok = TerminateProcess(h, 1) != 0;
                CloseHandle(h);
                ok
            }
        };
        if !done {
            left.push(l);
        }
    }
    if !left.is_empty() || !lockers(dir).is_empty() {
        std::thread::sleep(Duration::from_millis(800));
    }
    left
}

#[cfg(not(windows))]
fn free_files(_: &Path) -> Vec<Locker> {
    Vec::new()
}

/// «AdminOps (1234), Explorador de Windows (5678)» para el mensaje de error.
fn names(list: &[Locker]) -> String {
    list.iter().map(|l| if l.name.is_empty() { format!("proceso {}", l.pid) } else { format!("{} ({})", l.name, l.pid) }).collect::<Vec<_>>().join(", ")
}


/// Windows guarda en caché los iconos (barra de tareas, accesos del menú Inicio):
/// sin vaciarla, la barra de tareas sigue mostrando el icono de una versión anterior.
fn refresh_icon_cache() {
    use std::os::windows::process::CommandExt;
    let exe = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\ie4uinit.exe");
    for arg in ["-ClearIconCache", "-show"] {
        let _ = std::process::Command::new(&exe).arg(arg).creation_flags(0x0800_0000).status();
    }
}

fn remove_desktop_shortcuts() {
    let public = std::env::var("PUBLIC").map(|p| PathBuf::from(p).join("Desktop"));
    let user = std::env::var("USERPROFILE").map(|p| PathBuf::from(p).join("Desktop"));
    for d in [public, user].into_iter().flatten() {
        let _ = std::fs::remove_file(d.join("AdminOps.lnk"));
    }
}

#[tauri::command(async)]
fn install(app: tauri::AppHandle, dir: String, desktop: bool, close_app: bool) -> Result<String, String> {
    if PAYLOAD.is_empty() {
        return Err("Este instalador se compiló sin AdminOps dentro.".into());
    }
    let dir = validate_dir(&dir)?;
    let emit = |percent: u32, step: &str| {
        let _ = app.emit("progress", Progress { percent, step: step.into() });
    };
    emit(2, "Preparando…");
    if app_running() && !close_app {
        return Err("AdminOps está abierto: ciérralo para continuar.".into());
    }
    emit(4, "Cerrando AdminOps…");
    stop_app(&dir);

    let tmp = std::env::temp_dir().join(format!("adminops-setup-{}.exe", std::process::id()));
    std::fs::write(&tmp, PAYLOAD).map_err(|e| format!("No se pudo preparar la instalación: {e}"))?;
    let mut code = run_payload(&tmp, &dir, &emit);
    if code == 0 && !exe_is_current(&dir) {
        // Terminó «bien» pero el programa sigue siendo el anterior: algo lo tenía abierto.
        code = 2;
    }
    if code != 0 && code != -2 {
        // Archivos aún bloqueados (un proceso que tardó en cerrarse): cerrar de nuevo y reintentar.
        emit(8, "Reintentando…");
        stop_app(&dir);
        code = run_payload(&tmp, &dir, &emit);
        if code == 0 && !exe_is_current(&dir) {
            code = 2;
        }
    }
    let _ = std::fs::remove_file(&tmp);
    if code != 0 {
        return Err(match code {
            -1 => "No se pudo iniciar la instalación.".into(),
            -2 => "La instalación tardó demasiado y se detuvo.".into(),
            2 => {
                let left = lockers(&dir);
                if left.is_empty() {
                    "No se pudieron reemplazar los archivos de AdminOps. Vuelve a intentarlo; si se repite, usa el instalador clásico.".into()
                } else {
                    format!("No se pudieron reemplazar los archivos de AdminOps: los tiene abiertos {}. Ciérralo (o pulsa «Instalar» otra vez para que el instalador lo cierre) y vuelve a intentarlo.", names(&left))
                }
            }
            c => format!("La instalación falló (código {c})."),
        });
    }
    if !desktop {
        remove_desktop_shortcuts();
    }
    refresh_icon_cache();
    emit(97, "Comprobando…");
    match installed() {
        Some((v, d)) if v == VERSION => {
            emit(100, "Listo");
            Ok(d.display().to_string())
        }
        _ => Err("La instalación terminó pero Windows no registra AdminOps. Prueba con el instalador clásico.".into()),
    }
}

/// Ejecuta el instalador NSIS en silencio y va informando del avance estimado. Devuelve su código.
fn run_payload(tmp: &Path, dir: &Path, emit: &dyn Fn(u32, &str)) -> i32 {
    use std::os::windows::process::CommandExt;
    // /D debe ir el último y sin comillas aunque tenga espacios (regla de NSIS).
    let Ok(mut child) = std::process::Command::new(tmp).arg("/S").raw_arg(format!("/D={}", dir.display())).spawn() else { return -1 };
    // NSIS en silencio no informa del progreso: avance estimado hasta que termina.
    let start = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return status.code().unwrap_or(-1);
        }
        let t = start.elapsed().as_secs_f64();
        let percent = (10.0 + 82.0 * (1.0 - (-t / 5.0).exp())) as u32;
        let step = match percent {
            0..=30 => "Copiando archivos…",
            31..=65 => "Instalando componentes…",
            66..=85 => "Registrando AdminOps en Windows…",
            _ => "Creando accesos directos…",
        };
        emit(percent, step);
        if start.elapsed() > Duration::from_secs(600) {
            let _ = child.kill();
            return -2;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

#[tauri::command]
fn launch(dir: String) -> Result<(), String> {
    let exe = Path::new(&dir).join("adminops.exe");
    std::process::Command::new(&exe).current_dir(&dir).spawn().map(|_| ()).map_err(|e| format!("No se pudo abrir AdminOps: {e}"))
}

/// Abre el instalador de siempre (con sus ventanas) y cierra este.
#[tauri::command]
fn classic(app: tauri::AppHandle) -> Result<(), String> {
    run_classic()?;
    app.exit(0);
    Ok(())
}

fn run_classic() -> Result<(), String> {
    if PAYLOAD.is_empty() {
        return Err("Este instalador se compiló sin AdminOps dentro.".into());
    }
    let tmp = std::env::temp_dir().join(format!("AdminOps_{VERSION}_x64-setup.exe"));
    std::fs::write(&tmp, PAYLOAD).map_err(|e| e.to_string())?;
    std::process::Command::new(&tmp).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Los términos de uso: el mismo texto que enseña AdminOps en «Acerca de».
#[tauri::command]
fn terms() -> &'static str {
    include_str!("../../src-tauri/terminos.txt")
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    // Sin WebView2 esta ventana no puede mostrarse: el instalador clásico lo instala.
    if tauri::webview_version().is_err() {
        let _ = run_classic();
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![setup_info, pick_dir, install, launch, classic, terms, quit])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir el instalador");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows dice de verdad quién tiene abierto un archivo de la carpeta.
    #[cfg(windows)]
    #[test]
    fn finds_who_holds_a_file() {
        let dir = std::env::temp_dir().join(format!("adminops-rm-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bloqueado.dll");
        let held = std::fs::File::create(&path).unwrap();
        let list = lockers(&dir);
        assert!(list.iter().any(|l| l.pid == std::process::id()), "debería verse a sí mismo: {:?}", list.iter().map(|l| l.pid).collect::<Vec<_>>());
        // Este proceso no se cierra a sí mismo.
        assert!(free_files(&dir).iter().all(|l| l.pid != std::process::id()));
        drop(held);
        assert!(lockers(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_mislabelled_build_can_be_updated() {
        // La 2.0.0 fue una build de la serie 1.2 con el número equivocado.
        assert!(!is_downgrade("2.0.0", "1.2.2"));
        // El resto, como siempre.
        assert!(is_downgrade("1.3.0", "1.2.2"));
        assert!(is_downgrade("2.0.1", "1.2.2"));
        assert!(!is_downgrade("1.2.2", "1.2.2"));
        assert!(!is_downgrade("1.1.11", "1.2.2"));
    }

    #[test]
    fn versions_and_dirs() {
        assert!(parse_version("0.17.0") > parse_version("0.16.3"));
        assert!(parse_version("1.0.0") > parse_version("0.99.0"));
        assert!(validate_dir(r"C:\Program Files\AdminOps").is_ok());
        assert!(validate_dir(r"D:\Apps\AdminOps").is_ok());
        assert!(validate_dir(r"C:\").is_err());
        assert!(validate_dir(r"C:\Windows\AdminOps").is_err());
        assert!(validate_dir("AdminOps").is_err());
    }
}
