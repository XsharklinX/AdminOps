//! Instalador de AdminOps con interfaz propia.
//!
//! Lleva dentro el instalador NSIS estándar de Tauri y lo ejecuta en silencio
//! (`/S /D=carpeta`): así se conserva todo lo que hace bien (desinstalador,
//! entrada en "Aplicaciones instaladas", accesos del menú Inicio) con una
//! presentación a la altura de la app. Si el equipo no tiene WebView2, se abre
//! directamente el instalador clásico, que lo instala.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::Emitter;

const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.exe"));
const VERSION: &str = env!("APP_VERSION");
const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AdminOps";

fn parse_version(v: &str) -> Vec<u32> {
    v.split(['.', '-']).map(|p| p.parse().unwrap_or(0)).collect()
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
    app_running: bool,
}

#[tauri::command]
fn setup_info() -> SetupInfo {
    let current = installed();
    SetupInfo {
        version: VERSION.into(),
        downgrade: current.as_ref().is_some_and(|(v, _)| parse_version(v) > parse_version(VERSION)),
        install_dir: current.as_ref().map_or_else(default_dir, |(_, d)| d.clone()).display().to_string(),
        installed_version: current.map(|(v, _)| v),
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
    let script = format!(
        r"Get-Process powershell, pwsh -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne $PID -and ($_.Modules.FileName -like '{d}\*') }} | Stop-Process -Force -ErrorAction SilentlyContinue"
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
}

/// Windows guarda en caché los iconos (barra de tareas, accesos): así muestra el nuevo.
fn refresh_icon_cache() {
    use std::os::windows::process::CommandExt;
    let exe = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\ie4uinit.exe");
    let _ = std::process::Command::new(exe).arg("-show").creation_flags(0x0800_0000).status();
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
    if code != 0 && code != -2 {
        // Archivos aún bloqueados (un proceso que tardó en cerrarse): cerrar de nuevo y reintentar.
        emit(8, "Reintentando…");
        stop_app(&dir);
        code = run_payload(&tmp, &dir, &emit);
    }
    let _ = std::fs::remove_file(&tmp);
    if code != 0 {
        return Err(match code {
            -1 => "No se pudo iniciar la instalación.".into(),
            -2 => "La instalación tardó demasiado y se detuvo.".into(),
            2 => "No se pudieron reemplazar los archivos de AdminOps: algún programa los tiene abiertos. Reinicia el equipo y vuelve a intentarlo, o usa el instalador clásico.".into(),
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
        .invoke_handler(tauri::generate_handler![setup_info, pick_dir, install, launch, classic, quit])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir el instalador");
}

#[cfg(test)]
mod tests {
    use super::*;

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
