//! Instalador de AdminOps con interfaz propia.
//!
//! Lleva dentro el instalador NSIS estándar de Tauri y lo ejecuta en silencio
//! (`/S /D=carpeta`): así se conserva todo lo que hace bien (desinstalador,
//! entrada en "Aplicaciones instaladas", accesos del menú Inicio) con una
//! presentación a la altura de la app. Si el equipo no tiene WebView2, se abre
//! directamente el instalador clásico, que lo instala.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod fingerprint;

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
    if EXE_SHA256.is_empty() {
        return true;
    }
    std::fs::read(dir.join("adminops.exe")).is_ok_and(|b| fingerprint::fingerprint(&b) == EXE_SHA256)
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
    /// Abierto por AdminOps con `--volver` («Volver a la versión anterior»): sí se deja
    /// instalar esta versión, más antigua, encima de la instalada.
    rollback: bool,
    /// Abierto por AdminOps con `--actualizar` (la ventana de «Versión nueva»): actualiza la
    /// instalación que hay, sin preguntar nada, y vuelve a abrir AdminOps al terminar.
    auto: bool,
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
    let older = current.as_ref().is_some_and(|(v, _)| is_downgrade(v, VERSION));
    let volver = std::env::args().any(|a| a == "--volver");
    let actualizar = std::env::args().any(|a| a == "--actualizar");
    // `--volver --automatico`: lo abre una actualización que no arrancó para deshacerse sola, sin preguntar.
    let volver_solo = volver && older && std::env::args().any(|a| a == "--automatico");
    SetupInfo {
        // Solo encima de una instalación que ya existe y más antigua o igual: nunca instala de cero ni baja de versión.
        auto: (actualizar && !volver && current.is_some() && !older) || volver_solo,
        version: VERSION.into(),
        downgrade: older && !volver,
        rollback: older && volver,
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
    // Abierto por AdminOps (`--actualizar`, `--volver`), este instalador es parte de su árbol de
    // procesos: con /T se cerraría a sí mismo. Primero se le da tiempo a cerrarse sola (lo hace en
    // cuanto ve el instalador en marcha) y, si hay que forzarla, solo a ella.
    let from_app = std::env::args().any(|a| a == "--actualizar" || a == "--volver");
    if from_app {
        let start = Instant::now();
        while app_running() && start.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    let tree: &[&str] = if from_app { &["/F", "/IM", "adminops.exe"] } else { &["/F", "/T", "/IM", "adminops.exe"] };
    let _ = std::process::Command::new("taskkill.exe").args(tree).creation_flags(NO_WINDOW).status();
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
    // La versión que había antes de instalar: si la nueva no abre, es a la que se vuelve.
    let before = installed().map(|(v, _)| v);
    let _ = PREVIOUS_VERSION.set(before.clone());

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
            // Copia de este instalador en una carpeta que solo pueden cambiar los administradores: así se puede volver
            // a esta versión más adelante sin descargar nada y sin depender del modo en que se abra AdminOps.
            keep_own_copy(before.as_deref());
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

/// Lo que pongan los administradores en ProgramData: `%ProgramData%\AdminOps`.
fn program_data() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("AdminOps"))
}

/// Carpeta de las marcas de «arrancó bien» y del aviso de actualización fallida: `%ProgramData%\AdminOps\arranque`.
/// La crea el instalador con «Usuarios» en modificar, porque AdminOps escribe en ella sin administrador (aunque
/// «Como en la oficina» haya dejado `%ProgramData%\AdminOps` en solo lectura). Es la misma que `boot_dir` de la app.
fn boot_dir() -> Option<PathBuf> {
    program_data().map(|d| d.join("arranque"))
}

/// Carpeta de las copias del instalador (la versión instalada y la anterior): solo la cambian los administradores.
fn setups_dir() -> Option<PathBuf> {
    program_data().map(|d| d.join("instalador"))
}

/// Donde se apunta la huella y el formato de datos de cada copia (solo lo escribe un administrador).
const SETUPS_KEY: &str = r"SOFTWARE\AdminOps\Instaladores";

/// Formato de los datos de esta versión (`DATA_FORMAT` de la app).
const DATA_FORMAT: &str = env!("APP_DATA_FORMAT");

fn setup_file_name(version: &str) -> String {
    format!("AdminOps-{version}-Setup.exe")
}

fn version_ok(v: &str) -> bool {
    !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// «AdminOps-1.5.3-Setup.exe» → «1.5.3» (solo el nombre exacto de una copia).
fn setup_version_of(name: &str) -> Option<String> {
    let v = name.strip_prefix("AdminOps-")?.strip_suffix("-Setup.exe")?;
    version_ok(v).then(|| v.to_string())
}

/// ¿Es un enlace (junction o enlace simbólico) y no una carpeta de verdad?
fn is_reparse_point(p: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        std::fs::symlink_metadata(p).is_ok_and(|m| m.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(windows))]
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

/// ¿El dueño de la carpeta es Administradores o SYSTEM? (lo que crea este instalador, que corre elevado). Una que
/// creó un usuario antes que nosotros la controla él: no sirve para guardar nada que se ejecute como administrador.
/// `allow_self`: también vale la cuenta de este proceso, solo para una carpeta que acaba de crear él (con la directiva
/// «Creador del objeto» como dueño, lo que crea el técnico elevado queda a su nombre y no al de Administradores).
#[cfg(windows)]
fn owned_by(p: &Path, allow_self: bool) -> bool {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{IsWellKnownSid, WinBuiltinAdministratorsSid, WinLocalSystemSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID};
    let w: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut owner: PSID = std::ptr::null_mut();
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `w` acaba en 0; los punteros de salida son locales; `sd` se libera con LocalFree.
    let r = unsafe { GetNamedSecurityInfoW(w.as_ptr(), SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION, &mut owner, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &mut sd) };
    if r != 0 || owner.is_null() {
        return false;
    }
    // SAFETY: `owner` apunta dentro de `sd`, que sigue vivo hasta LocalFree.
    let ok = unsafe { IsWellKnownSid(owner, WinBuiltinAdministratorsSid) != 0 || IsWellKnownSid(owner, WinLocalSystemSid) != 0 } || (allow_self && is_own_sid(owner));
    // SAFETY: `sd` lo reservó GetNamedSecurityInfoW.
    unsafe { LocalFree(sd as _) };
    ok
}

/// ¿Es `sid` la cuenta de este proceso?
#[cfg(windows)]
fn is_own_sid(sid: windows_sys::Win32::Security::PSID) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{EqualSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    let mut tok: HANDLE = std::ptr::null_mut();
    // SAFETY: el pseudo-handle del proceso actual siempre es válido; `tok` se cierra abajo.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut tok) } == 0 {
        return false;
    }
    let mut buf = vec![0u64; 64]; // 512 bytes alineados: de sobra para TOKEN_USER y su SID.
    let mut len = 0u32;
    // SAFETY: `buf` mide lo que se le dice; `tok` es un token abierto con TOKEN_QUERY.
    let got = unsafe { GetTokenInformation(tok, TokenUser, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32, &mut len) } != 0;
    // SAFETY: `tok` lo abrió OpenProcessToken.
    unsafe { CloseHandle(tok) };
    if !got {
        return false;
    }
    // SAFETY: GetTokenInformation(TokenUser) deja un TOKEN_USER al principio de `buf`, que sigue vivo.
    let user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
    // SAFETY: los dos SID son válidos mientras viven `buf` y el descriptor del que sale `sid`.
    unsafe { EqualSid(user.User.Sid, sid) != 0 }
}

#[cfg(not(windows))]
fn owned_by(_: &Path, _: bool) -> bool {
    true
}

/// ¿Hay algún enlace (junction, enlace simbólico) dentro? Se mira sin seguirlos.
fn has_links_inside(dir: &Path) -> bool {
    let Ok(list) = std::fs::read_dir(dir) else { return false };
    list.flatten().any(|e| {
        let p = e.path();
        is_reparse_point(&p) || (p.is_dir() && has_links_inside(&p))
    })
}

/// Ejecuta icacls y dice si terminó bien (código 0). Un antivirus o AppLocker pueden impedirlo: entonces la carpeta
/// no tiene los permisos que se esperan y no se debe dar por lista.
#[cfg(windows)]
#[must_use]
fn icacls(path: &Path, args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("icacls.exe").arg(path).args(args).arg("/Q").creation_flags(0x0800_0000).status().is_ok_and(|s| s.success())
}

#[cfg(not(windows))]
#[must_use]
fn icacls(_: &Path, _: &[&str]) -> bool {
    true
}

/// Crea una carpeta dentro de `%ProgramData%\AdminOps` que sea de verdad nuestra (cualquier usuario puede crear cosas en
/// ProgramData y este instalador corre como administrador):
/// - `%ProgramData%\AdminOps` tiene que ser una carpeta real de los administradores; si no, no se usa (None).
/// - Si `dir` es un enlace, la creó un usuario o tiene enlaces dentro, se borra entera (remove_dir_all no sigue
///   enlaces) y se crea de nuevo. Los permisos se ponen después sin /T: nunca se recorre nada que no sea nuestro.
fn real_dir(dir: &Path) -> Option<()> {
    let parent = dir.parent()?;
    let new_parent = !parent.exists();
    std::fs::create_dir_all(parent).ok()?;
    if new_parent && !is_reparse_point(parent) && owned_by(parent, true) {
        // Recién creada por nosotros: a nombre de Administradores, como si no hubiera directiva «Creador del objeto».
        let _ = icacls(parent, &["/setowner", "*S-1-5-32-544"]);
    }
    if is_reparse_point(parent) || !owned_by(parent, new_parent) {
        return None;
    }
    if is_reparse_point(dir) {
        // remove_dir quita el enlace, no lo que hay detrás.
        std::fs::remove_dir(dir).ok()?;
    } else if dir.exists() && (!owned_by(dir, false) || has_links_inside(dir)) {
        std::fs::remove_dir_all(dir).ok()?;
    }
    std::fs::create_dir_all(dir).ok()?;
    // Recién creada (o recreada) por este proceso: vale también si queda a su nombre; el /setowner de después la pasa
    // a Administradores.
    (!is_reparse_point(dir) && owned_by(dir, true)).then_some(())
}

/// La carpeta de las marcas. «Usuarios» puede crear archivos dentro y cambiar los suyos (AdminOps escribe su marca sin
/// administrador), pero no borrar ni renombrar la carpeta: así no la pueden cambiar por un enlace a otro sitio. El
/// instalador solo lee en ella (nunca borra ni escribe), así que lo que pongan dentro no le afecta.
fn prepare_boot_dir() -> Option<PathBuf> {
    let dir = boot_dir()?;
    real_dir(&dir)?;
    // Las tres tienen que salir bien: si no, AdminOps sin administrador quizá no pueda escribir su marca, y una
    // actualización que abrió bien se daría por fallida.
    let ok = icacls(&dir, &["/setowner", "*S-1-5-32-544"])
        && icacls(&dir, &["/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F", "*S-1-5-32-545:(RX,WD,AD)"])
        // Aparte y sin «:r»: en la misma orden, un segundo /grant:r del mismo SID sustituiría al primero. Comprobado
        // con icacls: deja dos entradas separadas, «(RX,WD,AD)» en la carpeta y «(OI)(CI)(IO)(M)» para lo de dentro.
        && icacls(&dir, &["/grant", "*S-1-5-32-545:(OI)(CI)(IO)M"]);
    ok.then_some(dir)
}

/// La carpeta de las copias y del aviso de actualización fallida: dueño Administradores; control total para SYSTEM y
/// Administradores y solo lectura para «Usuarios», sin heredar nada de ProgramData (donde los usuarios pueden crear).
fn prepare_setups_dir() -> Option<PathBuf> {
    let dir = setups_dir()?;
    real_dir(&dir)?;
    // Sin estos permisos la carpeta podría seguir dejando escribir a los usuarios: no se usa.
    let ok = icacls(&dir, &["/setowner", "*S-1-5-32-544"])
        && icacls(&dir, &["/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F", "*S-1-5-32-545:(OI)(CI)RX"]);
    (ok && owned_by(&dir, false)).then_some(dir)
}

fn sha256_hex(bytes: &[u8]) -> String {
    sha2::Digest::finalize(<sha2::Sha256 as sha2::Digest>::new_with_prefix(bytes)).iter().map(|x| format!("{x:02x}")).collect()
}

/// Guarda una copia de este instalador en la carpeta protegida y apunta su huella en HKLM. Deja solo la de esta
/// versión y la de `keep` (la que había antes de instalar); las demás se borran.
fn keep_own_copy(keep: Option<&str>) {
    #[cfg(windows)]
    {
        let Some(dir) = prepare_setups_dir() else { return };
        let Ok(me) = std::env::current_exe() else { return };
        let dst = dir.join(setup_file_name(VERSION));
        let same = std::fs::canonicalize(&me).ok().zip(std::fs::canonicalize(&dst).ok()).is_some_and(|(a, b)| a == b);
        let Ok(bytes) = std::fs::read(&me) else { return };
        let sha = sha256_hex(&bytes);
        if !same {
            let tmp = dir.join(format!("{}.nuevo", setup_file_name(VERSION)));
            if std::fs::write(&tmp, &bytes).is_err() {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
            let _ = std::fs::remove_file(&dst);
            if std::fs::rename(&tmp, &dst).is_err() {
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        }
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ};
        let root = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok((k, _)) = root.create_subkey(format!(r"{SETUPS_KEY}\{VERSION}")) {
            let _ = k.set_value("Sha256", &sha);
            let _ = k.set_value("DataFormat", &DATA_FORMAT.parse::<u32>().unwrap_or(0));
        }
        // Fuera las demás: la de esta versión y la anterior bastan. Al reinstalar la misma versión, «la anterior» es la
        // copia más nueva de una versión más antigua (si no, «Volver a la versión anterior» desaparecería).
        let copies: Vec<String> = std::fs::read_dir(&dir)
            .map(|l| l.flatten().filter_map(|e| setup_version_of(&e.file_name().to_string_lossy())).collect())
            .unwrap_or_default();
        let previous = match keep {
            Some(k) if k == VERSION => copies.iter().filter(|v| parse_version(v) < parse_version(VERSION)).max_by_key(|v| parse_version(v)).cloned(),
            other => other.map(str::to_string),
        };
        let wanted = |v: &str| v == VERSION || previous.as_deref() == Some(v);
        if let Ok(list) = std::fs::read_dir(&dir) {
            for e in list.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                // Solo copias del instalador (y las a medias): el aviso de actualización fallida se queda.
                let stale = match setup_version_of(&name) {
                    Some(v) => !wanted(&v),
                    None => name.ends_with(".nuevo"),
                };
                if stale {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        if let Ok(k) = root.open_subkey_with_flags(SETUPS_KEY, KEY_READ | KEY_ALL_ACCESS) {
            let old: Vec<String> = k.enum_keys().flatten().filter(|v| !wanted(v)).collect();
            for v in old {
                let _ = k.delete_subkey_all(&v);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = keep;
}

/// Abre un archivo sin dejar que nadie lo cambie ni lo borre mientras siga abierto (solo leer) y devuelve su huella.
#[cfg(windows)]
fn open_locked(path: &Path) -> Option<(std::fs::File, String)> {
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().read(true).share_mode(1).open(path).ok()?; // FILE_SHARE_READ
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &buf[..n]);
    }
    let sha: String = sha2::Digest::finalize(hasher).iter().map(|x| format!("{x:02x}")).collect();
    Some((f, sha))
}

/// La versión instalada antes de esta instalación (la apunta `install`).
static PREVIOUS_VERSION: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// El instalador de la versión anterior, abierto y bloqueado (nadie puede cambiarlo hasta que se suelte), con su versión.
struct PreviousSetup {
    path: PathBuf,
    version: String,
    _lock: std::fs::File,
}

/// Por qué no se puede volver sola a la anterior (se apunta en el aviso).
type NoPrevious = String;

/// La versión que había antes de instalar, si es más antigua que esta (a la que se puede volver).
fn version_before() -> Result<String, NoPrevious> {
    let Some(Some(v)) = PREVIOUS_VERSION.get() else { return Err("no había otra versión instalada antes".into()) };
    if !version_ok(v) || parse_version(v) >= parse_version(VERSION) {
        return Err(format!("la versión de antes ({v}) no es más antigua que esta"));
    }
    Ok(v.clone())
}

/// Los datos que deja esta versión los sabe leer la de antes. Si no, volver sola podría dejarla sin poder abrir.
fn same_data_format(format: u32) -> Result<(), NoPrevious> {
    if Some(format) == DATA_FORMAT.parse::<u32>().ok() {
        Ok(())
    } else {
        Err("la versión anterior guarda los datos de otra forma y no se vuelve sin preguntar".into())
    }
}

/// La copia protegida de la versión que había antes (`%ProgramData%\AdminOps\instalador`), si su huella es la apuntada
/// en HKLM y guarda los datos igual que esta. `Ok(None)`: no hay copia protegida de esa versión.
#[cfg(windows)]
fn protected_previous(version: &str) -> Result<Option<PreviousSetup>, NoPrevious> {
    let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(format!(r"{SETUPS_KEY}\{version}")) else { return Ok(None) };
    let Some(dir) = setups_dir() else { return Ok(None) };
    // La carpeta tiene que seguir siendo real y de los administradores al ejecutar la copia: junto al .exe nadie más
    // ha podido dejar una DLL que el Setup cargaría elevado.
    if is_reparse_point(&dir) || !owned_by(&dir, false) {
        return Ok(None);
    }
    let path = dir.join(setup_file_name(version));
    let Ok(trusted) = key.get_value::<String, _>("Sha256") else { return Ok(None) };
    same_data_format(key.get_value("DataFormat").unwrap_or(0))?;
    let Some((lock, sha)) = open_locked(&path) else { return Ok(None) };
    if sha != trusted {
        return Err(format!("la copia guardada de la {version} está dañada (su huella no coincide)"));
    }
    Ok(Some(PreviousSetup { path, version: version.to_string(), _lock: lock }))
}

/// Versiones de antes de la 1.5.3: la copia que guardaba AdminOps antes de actualizar (`--anterior`), si sirve: su
/// nombre es el que genera la build, es justo la versión que había antes de instalar, guarda los datos igual que esta
/// y su huella coincide con la apuntada en HKLM.
#[cfg(windows)]
fn legacy_previous(version: &str) -> Result<PreviousSetup, NoPrevious> {
    let none = || format!("no hay una copia guardada de la {version}");
    let dir = match arg_value("--anterior") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(std::env::var_os("APPDATA").ok_or_else(none)?).join("com.adminops.app").join("version-anterior"),
    };
    let meta: serde_json::Value = std::fs::read(dir.join("version.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).ok_or_else(none)?;
    let saved = meta.get("version").and_then(|v| v.as_str()).unwrap_or("");
    let file = meta.get("file").and_then(|v| v.as_str()).unwrap_or("");
    if saved != version || file != setup_file_name(version) {
        return Err(none());
    }
    let key = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\AdminOps\VersionAnterior").map_err(|_| none())?;
    let (trusted_version, trusted_sha): (String, String) = (key.get_value("Version").map_err(|_| none())?, key.get_value("Sha256").map_err(|_| none())?);
    if trusted_version != version {
        return Err(none());
    }
    same_data_format(meta.get("dataFormat").and_then(|v| v.as_u64()).unwrap_or(0) as u32)?;
    let path = dir.join(file);
    // Abierta y bloqueada desde que se comprueba hasta que arranca: nadie puede cambiarla entre medias.
    let (lock, sha) = open_locked(&path).ok_or_else(none)?;
    if sha != trusted_sha {
        return Err(format!("la copia guardada de la {version} está dañada (su huella no coincide)"));
    }
    Ok(PreviousSetup { path, version: version.to_string(), _lock: lock })
}

/// El instalador de la versión que había antes de instalar, listo para volver sola a ella; o por qué no se puede.
#[cfg(windows)]
fn previous_setup() -> Result<PreviousSetup, NoPrevious> {
    let version = version_before()?;
    match protected_previous(&version)? {
        Some(p) => Ok(p),
        None => legacy_previous(&version),
    }
}

#[cfg(not(windows))]
fn previous_setup() -> Result<PreviousSetup, NoPrevious> {
    Err("solo en Windows".into())
}

fn boot_mark_name() -> String {
    format!("arranque-{VERSION}.ok")
}

/// Cuánto espera el instalador a que la versión nueva diga que abrió bien: al menos 60 s y, mientras AdminOps siga
/// abierta (un PC lento, el aviso del administrador de «Abrir en modo técnico»), hasta 180 s.
const BOOT_WAIT: Duration = Duration::from_secs(60);
const BOOT_WAIT_MAX: Duration = Duration::from_secs(180);

/// Valor que sigue a un argumento (`--anterior "C:\ruta"`).
fn arg_value(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// Fecha y hora locales («27/11/2026 14:05»).
#[cfg(windows)]
fn local_time() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: SYSTEMTIME es un struct de enteros; GetLocalTime lo rellena entero.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!("{:02}/{:02}/{} {:02}:{:02}", t.wDay, t.wMonth, t.wYear, t.wHour, t.wMinute)
}

#[cfg(not(windows))]
fn local_time() -> String {
    String::new()
}

/// Deja `actualizacion-fallida.txt` en la carpeta de las copias (solo la cambian los administradores: aquí no se
/// escribe nunca donde puede tocar un usuario). AdminOps lo lee al abrirse y apunta en sus datos que ya lo enseñó.
/// `result`: «vuelta» (volvió sola), «manual» (se abrió el instalador de la anterior, que pide confirmar) o «sin-copia».
fn write_failure(result: &str, previous: Option<&str>, reason: &str) {
    let Some(dir) = prepare_setups_dir() else { return };
    let previous = previous.map(|v| format!("Anterior: {v}\r\n")).unwrap_or_default();
    let text = format!("Version: {VERSION}\r\nHora: {}\r\nResultado: {result}\r\n{previous}Motivo: {reason}\r\n", local_time());
    let tmp = dir.join("actualizacion-fallida.txt.nuevo");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join("actualizacion-fallida.txt"));
    }
}

/// La actualización abrió bien: un aviso de fallo que quedara de antes (p. ej. de un intento anterior que volvió a la
/// 1.5.2, que no lo lee) ya no es verdad.
fn clear_failure() {
    let Some(dir) = setups_dir() else { return };
    // Solo si la carpeta sigue siendo nuestra (real y de Administradores): aquí se borra como administrador.
    if is_reparse_point(&dir) || !owned_by(&dir, false) {
        return;
    }
    let f = dir.join("actualizacion-fallida.txt");
    if std::fs::symlink_metadata(&f).is_ok_and(|m| m.is_file()) {
        let _ = std::fs::remove_file(&f);
    }
}

/// Cuándo se abrió la versión nueva (`launch`): solo vale una marca escrita después.
static LAUNCHED_AT: std::sync::OnceLock<std::time::SystemTime> = std::sync::OnceLock::new();
/// La carpeta de las marcas quedó lista: si no, AdminOps no puede escribir la suya y no se puede comprobar nada.
static BOOT_DIR_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// ¿Ya escribió la versión nueva su marca, después de abrirla? (solo se lee: la carpeta la pueden tocar los usuarios).
fn boot_marked() -> bool {
    let Some(mark) = boot_dir().map(|d| d.join(boot_mark_name())) else { return false };
    let Some(since) = LAUNCHED_AT.get() else { return false };
    let Ok(m) = std::fs::symlink_metadata(&mark) else { return false };
    // Un margen de 2 s por la precisión de la hora del archivo.
    m.is_file() && m.modified().is_ok_and(|t| t + Duration::from_secs(2) >= *since)
}

/// Los Setup de antes de la 1.5.3 no entienden `--automatico`: abren su pantalla de «Volver» y esperan a que alguien
/// lo confirme.
fn understands_automatic(version: &str) -> bool {
    parse_version(version) >= parse_version("1.5.3")
}

/// Tras `launch` en una actualización: espera a que la app nueva marque que arrancó bien (60 s; hasta 180 s mientras
/// siga abierta). Si no lo hace, vuelve a la versión anterior (la misma ruta que «Volver a la versión anterior»: su
/// Setup con `--volver`, ahora con `--automatico` para que no pregunte) y lo deja apuntado para avisar. Devuelve «ok»,
/// «vuelta» (se abrió el Setup de la anterior, este instalador debe cerrarse) o «sin-copia» (no hay a dónde volver).
#[tauri::command(async)]
fn verify_launch() -> String {
    // Sin la carpeta de las marcas la versión nueva no puede decir nada: no se deshace una que quizá abrió bien.
    if !BOOT_DIR_READY.load(std::sync::atomic::Ordering::SeqCst) {
        return "ok".into();
    }
    let start = Instant::now();
    let waited = loop {
        if boot_marked() {
            clear_failure();
            return "ok".into();
        }
        let t = start.elapsed();
        if t >= BOOT_WAIT_MAX || (t >= BOOT_WAIT && !app_running()) {
            break t.as_secs();
        }
        std::thread::sleep(Duration::from_millis(if t >= BOOT_WAIT { 1000 } else { 500 }));
    };
    let why = if waited >= BOOT_WAIT_MAX.as_secs() { format!("no confirmó que abría en {waited} s") } else { format!("se cerró sin confirmar que abría ({waited} s)") };
    let prev = match previous_setup() {
        Ok(p) => p,
        Err(no) => {
            write_failure("sin-copia", None, &format!("La versión nueva {why} y no se volvió sola: {no}"));
            return "sin-copia".into();
        }
    };
    let automatic = understands_automatic(&prev.version);
    let result = if automatic { "vuelta" } else { "manual" };
    write_failure(result, Some(&prev.version), &format!("La versión nueva {why}"));
    // El archivo sigue bloqueado (prev) hasta que el Setup anterior ha arrancado. Sin la carpeta de WebView2 de este
    // instalador (que sigue abierto unos segundos): el Setup anterior se pondría la suya y podría borrar esta.
    let r = std::process::Command::new(&prev.path).args(["--volver", "--automatico"]).env_remove("WEBVIEW2_USER_DATA_FOLDER").spawn();
    match r {
        Ok(_) => result.into(),
        Err(e) => {
            write_failure("sin-copia", None, &format!("La versión nueva {why} y no se pudo abrir la copia de la {}: {e}", prev.version));
            "sin-copia".into()
        }
    }
}

#[tauri::command]
fn launch(dir: String) -> Result<(), String> {
    // La carpeta de las marcas, escribible por AdminOps sin administrador. Aquí no se borra nada (la pueden tocar los
    // usuarios): una marca de antes no vale porque solo cuentan las escritas desde ahora (LAUNCHED_AT).
    let ready = prepare_boot_dir().is_some();
    BOOT_DIR_READY.store(ready, std::sync::atomic::Ordering::SeqCst);
    let _ = LAUNCHED_AT.set(std::time::SystemTime::now());
    let exe = Path::new(&dir).join("adminops.exe");
    // El instalador corre como administrador (a veces con la cuenta del técnico en el PC de otro usuario). Abierto
    // desde aquí, AdminOps heredaría ese administrador y esa cuenta: no encontraría la carpeta de datos del usuario
    // de la sesión (WebView2: «can't read and write to its data directory»). Se le pide al Explorador, que corre
    // como el usuario de la sesión y sin elevar: es lo mismo que abrirla con doble clic.
    std::process::Command::new("explorer.exe").arg(&exe).spawn().map(|_| ()).map_err(|e| format!("No se pudo abrir AdminOps: {e}"))
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

/// ¿Sigue vivo ese proceso?
#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: OpenProcess devuelve null si no existe; el handle se cierra.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code) != 0 && code == STILL_ACTIVE as u32;
        CloseHandle(h);
        ok
    }
}

#[cfg(not(windows))]
fn process_alive(_: u32) -> bool {
    true
}

/// Las carpetas de WebView2 de instaladores que ya se cerraron (y la «wv» de la 1.5.3 en desarrollo): fuera. Las de un
/// instalador que sigue abierto no se tocan.
fn clean_old_webview_dirs(base: &Path) {
    let Ok(list) = std::fs::read_dir(base) else { return };
    for e in list.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let stale = match name.strip_prefix("wv-").map(str::parse::<u32>) {
            Some(Ok(pid)) => pid != std::process::id() && !process_alive(pid),
            _ => name == "wv",
        };
        if stale && !is_reparse_point(&e.path()) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

fn main() {
    // Abierto por AdminOps (actualizar, volver), hereda su WEBVIEW2_USER_DATA_FOLDER («…\AdminOps\WebView»). Con esa
    // carpeta, al cerrar el navegador interno de AdminOps antes de instalar (stop_app) se cerraba también su propia
    // ventana: un cuadro negro y todo se iba. Siempre una carpeta propia, que no coincide con las que se cierran.
    // Una por proceso: el Setup de la versión anterior (al volver sola) no comparte ni borra la de este.
    let base = std::env::temp_dir().join("adminops-instalador");
    clean_old_webview_dirs(&base);
    let own = base.join(format!("wv-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&own);
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &own);
    std::env::remove_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS");
    // Sin WebView2 esta ventana no puede mostrarse: el instalador clásico lo instala.
    if tauri::webview_version().is_err() {
        let _ = run_classic();
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![setup_info, pick_dir, install, launch, verify_launch, classic, terms, quit])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir el instalador");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una carpeta creada por este proceso es suya: vale con `allow_self` y, sin elevar, no es de Administradores.
    #[cfg(windows)]
    #[test]
    fn el_dueno_de_una_carpeta_propia_es_este_proceso() {
        let d = std::env::temp_dir().join(format!("adminops-dueno-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(owned_by(&d, true));
        let _ = std::fs::remove_dir(&d);
    }

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

    /// El ejecutable instalado y el de la carpeta de compilación solo se
    /// diferencian en la marca del tipo de instalador: tienen que dar la misma huella.
    #[test]
    fn the_installed_exe_matches_the_built_one() {
        let exe = |kind: &str, tail: &str| format!("cabecera__TAURI_BUNDLE_TYPE_VAR_{kind}resto__TAURI_BUNDLE_TYPE_VAR_{kind}{tail}").into_bytes();
        let built = fingerprint::fingerprint(&exe("UNK", "fin"));
        assert_eq!(built, fingerprint::fingerprint(&exe("NSS", "fin")));
        // Cualquier otra diferencia sí cuenta: es otra build.
        assert_ne!(built, fingerprint::fingerprint(&exe("NSS", "fim")));
        // Sin marca (o cortado justo en ella) no se rompe.
        assert_eq!(fingerprint::fingerprint(b"nada"), fingerprint::fingerprint(b"nada"));
        assert_ne!(fingerprint::fingerprint(b"__TAURI_BUNDLE_TYPE_VAR_N"), fingerprint::fingerprint(b"otra cosa"));
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
    fn setup_copies_by_name() {
        assert_eq!(setup_version_of("AdminOps-1.5.3-Setup.exe").as_deref(), Some("1.5.3"));
        assert_eq!(setup_version_of("actualizacion-fallida.txt"), None);
        assert_eq!(setup_version_of("AdminOps-..\\x-Setup.exe"), None);
        assert!(understands_automatic("1.5.3") && understands_automatic("1.6.0"));
        assert!(!understands_automatic("1.5.2"));
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
