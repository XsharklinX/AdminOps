//! Lo que el cohete hace además de «Optimizar ya» (ver `rocket`):
//!
//! - **A fondo** (mantenerlo pulsado): la caché de descargas de Windows Update, la de Optimización
//!   de distribución, lo que lleva más de 30 días en la papelera y las miniaturas. Primero dice
//!   cuánto ocupa cada cosa y luego borra lo que se elija. También la caché de Chrome, Edge, Brave y
//!   Firefox, solo si ese navegador está cerrado.
//! - **Ctrl+Alt+O** desde cualquier sitio.
//! - **Programa que no responde**: si el de delante lleva un rato «No responde», la burbuja se pone
//!   roja y ofrece esperar o cerrarlo y volver a abrirlo.
//! - **Preparar una llamada**: aligera, pausa OneDrive y Windows Update 2 horas y dice qué micrófono
//!   usarán las llamadas.
//! - **Despega solo** (opcional): memoria por encima del 90 % dos minutos, o el equipo sin usar un rato
//!   con la memoria alta. Nunca con una presentación, un vídeo a pantalla completa o el micrófono en uso.
//! - **Solo el cohete**: `--rocket` arranca únicamente la ventanita, para tenerlo con AdminOps cerrada.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tauri::{Emitter, Manager};

pub const HUNG_EVENT: &str = "rocket://hung";
pub const AUTO_EVENT: &str = "rocket://auto";
pub const LAUNCH_EVENT: &str = "rocket://launch";
static BRAIN: AtomicBool = AtomicBool::new(false);
/// Este proceso es «solo el cohete» (`--rocket`).
static ONLY: AtomicBool = AtomicBool::new(false);
const MUTEX_FULL: &str = "Local\\AdminOps-ventana-principal";
pub const TASK: &str = "AdminOps cohete al iniciar sesión";

pub fn rocket_only() -> bool {
    ONLY.load(Ordering::SeqCst)
}

// ---------- A fondo ----------

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeepItem {
    pub id: &'static str,
    pub label: &'static str,
    pub bytes: u64,
}

fn win_dir() -> Option<PathBuf> {
    std::env::var_os("SystemRoot").map(PathBuf::from)
}

fn update_dir() -> Option<PathBuf> {
    win_dir().map(|w| w.join(r"SoftwareDistribution\Download"))
}

fn delivery_dir() -> Option<PathBuf> {
    win_dir().map(|w| w.join(r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"))
}

fn thumbs_dir() -> Option<PathBuf> {
    crate::target_user::user_dirs().map(|u| u.local_app_data.join(r"Microsoft\Windows\Explorer"))
}

/// Los navegadores cuya caché entra en «a fondo»: (id, programa, etiqueta).
const BROWSER_CACHES: [(&str, &str, &str); 4] = [
    ("chrome", "chrome.exe", "Caché de Chrome"),
    ("edge", "msedge.exe", "Caché de Edge"),
    ("brave", "brave.exe", "Caché de Brave"),
    ("firefox", "firefox.exe", "Caché de Firefox"),
];

/// Las carpetas de caché de un navegador, de todos sus perfiles (solo la caché: no cookies ni sesiones).
fn browser_cache_dirs(id: &str) -> Vec<PathBuf> {
    if id == "firefox" {
        let Some(u) = crate::target_user::user_dirs() else { return vec![] };
        let Ok(rd) = std::fs::read_dir(u.local_app_data.join(r"Mozilla\Firefox\Profiles")) else { return vec![] };
        return rd.flatten().map(|e| e.path().join("cache2")).filter(|p| p.is_dir()).collect();
    }
    let Some(b) = crate::webnotif::CHROMIUM.iter().find(|b| b.id == id) else { return vec![] };
    b.profiles().into_iter().flat_map(|(_, dir)| ["Cache", "Code Cache", "GPUCache"].map(|c| dir.join(c))).filter(|p| p.is_dir()).collect()
}

/// Lo que ocupa la caché de ese navegador (sin edad mínima: la caché se puede borrar entera).
fn browser_cache_bytes(id: &str, deadline: Instant) -> u64 {
    browser_cache_dirs(id).iter().map(|d| dir_size(d, Duration::ZERO, 0, deadline)).sum()
}

/// Lo que ocupan los archivos de más de `age` (sin seguir enlaces).
pub fn dir_size(dir: &Path, age: Duration, depth: u32, deadline: Instant) -> u64 {
    if depth > 16 || Instant::now() > deadline {
        return 0;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let now = SystemTime::now();
    let mut n = 0;
    for e in rd.flatten() {
        let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
        if crate::rocket::is_link(&m) {
            continue;
        }
        if m.is_dir() {
            n += dir_size(&e.path(), age, depth + 1, deadline);
        } else if m.modified().ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|d| d >= age) {
            n += m.len();
        }
    }
    n
}

fn thumb_files() -> Vec<PathBuf> {
    let Some(d) = thumbs_dir() else { return vec![] };
    std::fs::read_dir(d)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.to_ascii_lowercase().starts_with("thumbcache_") && n.to_ascii_lowercase().ends_with(".db"))).collect())
        .unwrap_or_default()
}

/// Un elemento de la papelera: su `$I` (con la fecha de borrado) y su `$R` (el archivo o carpeta).
struct Trashed {
    info: PathBuf,
    data: PathBuf,
    bytes: u64,
}

/// Lee un `$I…`: versión (8 bytes), tamaño (8) y fecha de borrado (FILETIME, 8). Devuelve (tamaño, días).
pub fn parse_recycle_info(raw: &[u8], now_filetime: u64) -> Option<(u64, u64)> {
    if raw.len() < 24 {
        return None;
    }
    let size = u64::from_le_bytes(raw[8..16].try_into().ok()?);
    let deleted = u64::from_le_bytes(raw[16..24].try_into().ok()?);
    let days = now_filetime.saturating_sub(deleted) / (10_000_000 * 86_400);
    Some((size, days))
}

fn now_filetime() -> u64 {
    // FILETIME: intervalos de 100 ns desde 1601.
    const EPOCH_DIFF: u64 = 11_644_473_600;
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| (d.as_secs() + EPOCH_DIFF) * 10_000_000).unwrap_or(0)
}

/// Lo que lleva más de 30 días en la papelera del usuario con la sesión abierta, en todas las unidades.
fn old_trash() -> Vec<Trashed> {
    let Some(sid) = crate::target_user::get().map(|t| t.sid.clone()) else { return vec![] };
    let now = now_filetime();
    let mut out = Vec::new();
    for letter in b'C'..=b'Z' {
        let bin = PathBuf::from(format!("{}:\\$Recycle.Bin\\{sid}", letter as char));
        let Ok(rd) = std::fs::read_dir(&bin) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(rest) = name.strip_prefix("$I") else { continue };
            let Ok(raw) = std::fs::read(e.path()) else { continue };
            if let Some((bytes, days)) = parse_recycle_info(&raw, now) {
                if days >= 30 {
                    out.push(Trashed { info: e.path(), data: bin.join(format!("$R{rest}")), bytes });
                }
            }
        }
    }
    out
}

#[tauri::command]
pub async fn rocket_deep_scan() -> Result<Vec<DeepItem>, String> {
    if !crate::elevation::is_elevated() {
        return Err("La limpieza a fondo necesita AdminOps abierta como administrador.".into());
    }
    tauri::async_runtime::spawn_blocking(|| {
        let deadline = Instant::now() + Duration::from_secs(20);
        let day = Duration::from_secs(86_400);
        let mut v = vec![
            DeepItem { id: "update", label: "Descargas de Windows Update", bytes: update_dir().map_or(0, |d| dir_size(&d, day, 0, deadline)) },
            DeepItem { id: "delivery", label: "Optimización de distribución", bytes: delivery_dir().map_or(0, |d| dir_size(&d, Duration::ZERO, 0, deadline)) },
            DeepItem { id: "recycle", label: "Papelera de más de 30 días", bytes: old_trash().iter().map(|t| t.bytes).sum() },
            DeepItem { id: "thumbs", label: "Miniaturas de imágenes", bytes: thumb_files().iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum() },
        ];
        // Las cachés de los navegadores, solo si el navegador está cerrado (abierto, la tiene en uso y se
        // volvería a llenar al momento): si está abierto, no aparece en la lista.
        let running = crate::webnotif::running_exes();
        for (id, exe, label) in BROWSER_CACHES {
            if !running.contains(exe) {
                v.push(DeepItem { id, label, bytes: browser_cache_bytes(id, deadline) });
            }
        }
        v.retain(|i| i.bytes >= 1_048_576);
        Ok(v)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Borra lo elegido de la lista de «a fondo». Devuelve lo liberado de verdad (lo abierto se salta).
#[tauri::command]
pub async fn rocket_deep_clean(app: tauri::AppHandle, ids: Vec<String>) -> Result<u64, String> {
    if !crate::elevation::is_elevated() {
        return Err("La limpieza a fondo necesita AdminOps abierta como administrador.".into());
    }
    let freed = tauri::async_runtime::spawn_blocking(move || {
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut freed = 0u64;
        for id in &ids {
            match id.as_str() {
                "update" => {
                    if let Some(d) = update_dir() {
                        let mut c = crate::rocket::Clean::default();
                        crate::rocket::clean_dir(&d, Duration::from_secs(86_400), 0, deadline, &mut c);
                        freed += c.bytes;
                    }
                }
                "delivery" => {
                    let before = delivery_dir().map_or(0, |d| dir_size(&d, Duration::ZERO, 0, deadline));
                    let _ = crate::ps::powershell_opts("Delete-DeliveryOptimizationCache -Force -ErrorAction Stop", crate::ps::Opts { timeout: Some(Duration::from_secs(120)), task: None });
                    let after = delivery_dir().map_or(0, |d| dir_size(&d, Duration::ZERO, 0, deadline));
                    freed += before.saturating_sub(after);
                }
                "recycle" => {
                    for t in old_trash() {
                        let gone = if t.data.is_dir() { std::fs::remove_dir_all(&t.data).is_ok() } else { std::fs::remove_file(&t.data).is_ok() || !t.data.exists() };
                        if gone {
                            let _ = std::fs::remove_file(&t.info);
                            freed += t.bytes;
                        }
                    }
                }
                "thumbs" => {
                    for p in thumb_files() {
                        let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                        if std::fs::remove_file(&p).is_ok() {
                            freed += len;
                        }
                    }
                }
                other => {
                    // Caché de un navegador: solo si sigue cerrado (pudo abrirse desde que se midió).
                    let Some((id, exe, _)) = BROWSER_CACHES.iter().find(|(i, _, _)| *i == other) else { continue };
                    if crate::webnotif::running_exes().contains(*exe) {
                        continue;
                    }
                    for d in browser_cache_dirs(id) {
                        let mut c = crate::rocket::Clean::default();
                        crate::rocket::clean_dir(&d, Duration::ZERO, 0, deadline, &mut c);
                        freed += c.bytes;
                    }
                }
            }
        }
        freed
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(state) = app.try_state::<crate::tweaks::TweakState>() {
        let text = format!("Optimizar a fondo (cohete): {} MB liberados", freed / 1_048_576);
        state.record(crate::tweaks::journal::Op::Run, &text, &Ok::<(), String>(()));
    }
    Ok(freed)
}

// ---------- Ctrl+Alt+O ----------

/// Registra Ctrl+Alt+O: con el cohete en el escritorio, despega a la vista (aunque haya una
/// presentación); si no, optimiza y avisa con una notificación.
pub fn start_hotkey(app: tauri::AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
        // Si lo tiene el proceso «solo el cohete», lo suelta al ver abierta esta AdminOps: se reintenta un rato.
        let mut ok = false;
        for _ in 0..15 {
            if RegisterHotKey(std::ptr::null_mut(), 4, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'O')) != 0 {
                ok = true;
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        if !ok {
            log::warn!("No se pudo registrar Ctrl+Alt+O para el cohete: lo usa otro programa.");
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                launch(&app);
            }
        }
    });
    #[cfg(not(windows))]
    let _ = app;
}

/// «Optimizar ya» desde el atajo o desde el menú del reloj.
pub fn launch(app: &tauri::AppHandle) {
    // En modo auditoría no se cambia nada del equipo, tampoco desde el atajo o el menú del reloj.
    if crate::audit::active() {
        crate::toast::show(app, "Modo auditoría", "El cohete no optimiza mientras el modo auditoría está activo.", &[], |_| {});
        return;
    }
    if crate::rocket::force_show(app) {
        let _ = app.emit(LAUNCH_EVENT, ());
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (title, body) = match crate::rocket::boost(&app, "Optimizar ya (atajo)").await {
            Ok(b) => ("Optimizado".to_string(), format!("{} MB de memoria y {} MB de temporales, sin cerrar nada.", b.ram_freed / 1_048_576, b.temp_freed / 1_048_576)),
            Err(e) => ("No se pudo optimizar".to_string(), e),
        };
        crate::toast::show(&app, &title, &body, &[], |_| {});
    });
}

// ---------- El vigilante: colgados y despegue automático ----------

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hung {
    pub pid: u32,
    pub name: String,
    pub secs: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AutoNote {
    /// «15:02».
    pub at: String,
    pub before_pct: u32,
    pub after_pct: u32,
    pub reason: &'static str,
}

/// El programa de delante y su nombre, si hay ventana activa.
#[cfg(windows)]
fn foreground() -> Option<(u32, bool)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId};
    // SAFETY: lecturas sobre la ventana activa; búferes propios.
    unsafe {
        let w = GetForegroundWindow();
        if w.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(w, &mut pid);
        let mut buf = [0u16; 16];
        let n = GetClassNameW(w, buf.as_mut_ptr(), buf.len() as i32);
        let ghost = n == 5 && String::from_utf16_lossy(&buf[..5]) == "Ghost";
        Some((pid, ghost))
    }
}

#[cfg(not(windows))]
fn foreground() -> Option<(u32, bool)> {
    None
}

/// Minutos sin tocar ratón ni teclado.
#[cfg(windows)]
fn idle_secs() -> u64 {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut li = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: estructura propia con su tamaño.
    unsafe {
        if GetLastInputInfo(&mut li) == 0 {
            return 0;
        }
        u64::from(GetTickCount().wrapping_sub(li.dwTime)) / 1000
    }
}

#[cfg(not(windows))]
fn idle_secs() -> u64 {
    0
}

fn proc_name(pid: u32) -> String {
    let mut sys = sysinfo::System::new();
    let p = sysinfo::Pid::from_u32(pid);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
    sys.process(p).map(|x| crate::rocket::app_label(&x.name().to_string_lossy())).unwrap_or_else(|| "El programa".into())
}

/// ¿Toca despegar solo? Memoria ≥ 90 % durante 2 min, o ≥ 80 % con 15 min sin usar el equipo; como
/// mucho una vez cada 30 min. Pura, para las pruebas.
pub fn auto_due(high_for: Duration, idle: Duration, pct: u32, since_last: Option<Duration>) -> Option<&'static str> {
    if since_last.is_some_and(|d| d < Duration::from_secs(30 * 60)) {
        return None;
    }
    if pct >= 90 && high_for >= Duration::from_secs(120) {
        Some("memoria")
    } else if pct >= 80 && idle >= Duration::from_secs(15 * 60) {
        Some("sin usar")
    } else {
        None
    }
}

/// «Empezar el día ligero». `since` es lo que lleva el cohete en marcha hoy: desde que arrancó (la tarea lo abre al
/// iniciar sesión) o desde que cambió el día con él abierto (el portátil despierta por la mañana). No se usa el
/// tiempo encendido de Windows: con el inicio rápido o la suspensión no vuelve a cero. `None`: aún es pronto
/// (Windows sigue cargando lo suyo, minuto y medio); `Some(true)`: toca; `Some(false)`: hoy ya no (más de 3 h, o
/// pasada la mañana: abrir AdminOps a las 15:00 no es empezar el día). De madrugada (antes de las 6:00) aún no: un
/// equipo que no se apaga, o que Windows despierta a las 3:00 para su mantenimiento, no gasta ahí el día; se espera
/// a las 6:00 contando desde entonces.
pub fn morning_window(since: Duration, hour: u32) -> Option<bool> {
    if hour >= 13 {
        Some(false)
    } else if hour < 6 || since < Duration::from_secs(90) {
        None
    } else {
        Some(since <= Duration::from_secs(3 * 3600))
    }
}

/// Arranca el vigilante una sola vez: cada 2 s mira si el programa de delante está colgado (solo con
/// el cohete en el escritorio) y, si está activado, si toca despegar solo.
pub fn start_brain(app: tauri::AppHandle) {
    if BRAIN.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut hung_since: Option<(u32, Instant)> = None;
        let mut told: Option<u32> = None;
        let mut high_since: Option<Instant> = None;
        let mut last_auto: Option<Instant> = None;
        let mut morning_checked = false;
        let mut morning_day = chrono::Local::now().date_naive();
        let mut morning_from = Instant::now();
        let mut tick = 0u64;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            tick += 1;
            let settings = crate::workflow::settings(&app);
            if settings.rocket == "off" {
                continue;
            }
            // Programa colgado (con la burbuja a la vista, en el escritorio).
            if app.get_webview_window(crate::rocket::LABEL).is_some() {
                let hung_pid = foreground().and_then(|(pid, ghost)| {
                    let set = crate::waitchain::hung_pids();
                    if set.contains(&pid) {
                        Some(pid)
                    } else if ghost && set.len() == 1 {
                        set.into_iter().next()
                    } else {
                        None
                    }
                });
                match (hung_pid, hung_since) {
                    (Some(pid), Some((p, t))) if p == pid => {
                        let secs = t.elapsed().as_secs();
                        if secs >= 10 && (told != Some(pid) || tick.is_multiple_of(5)) {
                            told = Some(pid);
                            let _ = app.emit(HUNG_EVENT, Some(Hung { pid, name: proc_name(pid), secs }));
                        }
                    }
                    (Some(pid), _) => hung_since = Some((pid, Instant::now())),
                    (None, _) => {
                        hung_since = None;
                        if told.take().is_some() {
                            let _ = app.emit(HUNG_EVENT, Option::<Hung>::None);
                        }
                    }
                }
            }
            // Empezar el día ligero: una vez al día, por la mañana, pasado minuto y medio de empezar (ver
            // `morning_window`). Con AdminOps abierta varios días, cada día nuevo vuelve a contar. Con una llamada o
            // pantalla completa se espera, sin marcar el día.
            let local = chrono::Local::now();
            if local.date_naive() != morning_day {
                morning_day = local.date_naive();
                morning_from = Instant::now();
                morning_checked = false;
            }
            // Encendido desde la madrugada: el día empieza a contar a las 6:00.
            if chrono::Timelike::hour(&local) < 6 {
                morning_from = Instant::now();
            }
            let window = morning_window(morning_from.elapsed(), chrono::Timelike::hour(&local));
            if !morning_checked && settings.rocket_morning && !crate::audit::active() && window == Some(false) {
                morning_checked = true;
            }
            if !morning_checked && settings.rocket_morning && !crate::audit::active() && window == Some(true) && !crate::rocket::fullscreen_now() && !crate::uxio::media_in_use() {
                morning_checked = true;
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let mark = crate::paths::shared_data_dir(&app).join("cohete-dia.txt");
                if std::fs::read_to_string(&mark).ok().as_deref() != Some(today.as_str()) {
                    let _ = std::fs::write(&mark, &today);
                    let app2 = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Ok(b) = crate::rocket::boost(&app2, "Empezar el día ligero (cohete)").await {
                            let pct = |u: u64| if b.ram_total == 0 { 0 } else { (u as f64 / b.ram_total as f64 * 100.0).round() as u32 };
                            let at = chrono::Local::now().format("%H:%M").to_string();
                            let _ = app2.emit(AUTO_EVENT, AutoNote { at, before_pct: pct(b.ram_used_before), after_pct: pct(b.ram_used_after), reason: "mañana" });
                        }
                    });
                }
            }
            // Despegar solo (nunca en modo auditoría).
            if !settings.rocket_auto || !tick.is_multiple_of(3) || crate::audit::active() {
                continue;
            }
            let pct = crate::rocket::rocket_memory();
            if pct >= 90 {
                high_since.get_or_insert_with(Instant::now);
            } else {
                high_since = None;
            }
            let high_for = high_since.map_or(Duration::ZERO, |t| t.elapsed());
            let Some(reason) = auto_due(high_for, Duration::from_secs(idle_secs()), pct, last_auto.map(|t| t.elapsed())) else { continue };
            // Nunca con una presentación, un vídeo a pantalla completa o una llamada.
            if crate::rocket::fullscreen_now() || crate::uxio::media_in_use() {
                continue;
            }
            last_auto = Some(Instant::now());
            high_since = None;
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                let title = if reason == "memoria" { "Optimizado solo (memoria alta)" } else { "Optimizado solo (equipo sin usar)" };
                if let Ok(b) = crate::rocket::boost(&app2, title).await {
                    let pct = |u: u64| if b.ram_total == 0 { 0 } else { (u as f64 / b.ram_total as f64 * 100.0).round() as u32 };
                    let at = chrono::Local::now().format("%H:%M").to_string();
                    let _ = app2.emit(AUTO_EVENT, AutoNote { at, before_pct: pct(b.ram_used_before), after_pct: pct(b.ram_used_after), reason });
                }
            });
        }
    });
}

/// «Cerrar y volver a abrir» el programa colgado: se cierra a la fuerza y se abre otra vez como el
/// usuario. Office, al volver a abrirse, enseña los documentos que puede recuperar.
#[tauri::command]
pub async fn rocket_restart_hung(app: tauri::AppHandle, pid: u32, expected: String) -> Result<String, String> {
    let r = tauri::async_runtime::spawn_blocking(move || {
        let mut sys = sysinfo::System::new();
        let p = sysinfo::Pid::from_u32(pid);
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
        let proc_ = sys.process(p).ok_or("Ese programa ya se cerró.")?;
        let exe = proc_.exe().map(Path::to_path_buf).ok_or("No se sabe dónde está ese programa.")?;
        let name = crate::rocket::app_label(&proc_.name().to_string_lossy());
        // Que sea el mismo programa y siga colgado: el número de proceso pudo pasar a otro.
        if name != expected {
            return Err("Ese programa ya se cerró.".into());
        }
        if !crate::waitchain::hung_pids().contains(&pid) {
            return Err(format!("{name} ya responde: no hace falta cerrarlo."));
        }
        if !proc_.kill() {
            return Err(format!("No se pudo cerrar {name}."));
        }
        std::thread::sleep(Duration::from_millis(1200));
        crate::shellopen::run_as_user(&exe.to_string_lossy(), "")?;
        Ok(name)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let (Ok(name), Some(state)) = (&r, app.try_state::<crate::tweaks::TweakState>()) {
        state.record(crate::tweaks::journal::Op::Run, &format!("Cerrar y volver a abrir {name} (no respondía)"), &Ok::<(), String>(()));
    }
    let _ = app.emit(HUNG_EVENT, Option::<Hung>::None);
    r.map(|name| format!("{name} se cerró y se está volviendo a abrir."))
}

// ---------- Preparar una llamada ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CallStep {
    pub ok: bool,
    pub text: String,
}

/// Antes de una reunión: aligera, pausa OneDrive y Windows Update 2 horas y dice qué micrófono usarán
/// las llamadas (y si tiene permiso).
#[tauri::command]
pub async fn rocket_prepare_call(app: tauri::AppHandle) -> Result<Vec<CallStep>, String> {
    let mut steps = Vec::new();
    match crate::rocket::boost(&app, "Preparar llamada (cohete)").await {
        Ok(b) => steps.push(CallStep { ok: true, text: format!("{} MB de memoria liberados", b.ram_freed / 1_048_576) }),
        Err(e) => steps.push(CallStep { ok: false, text: e }),
    }
    let app2 = app.clone();
    let more = tauri::async_runtime::spawn_blocking(move || {
        let mut s = Vec::new();
        let state = app2.state::<crate::tweaks::TweakState>();
        match crate::onedrive::onedrive_pause(state.clone()) {
            Ok(_) => s.push(CallStep { ok: true, text: "OneDrive en pausa 2 horas".into() }),
            Err(e) if e.contains("no está instalado") => {}
            Err(e) => s.push(CallStep { ok: false, text: format!("OneDrive: {e}") }),
        }
        match crate::winupdate::pause_hours(2, &state) {
            Ok(()) => s.push(CallStep { ok: true, text: "Windows Update en pausa 2 horas".into() }),
            Err(e) => s.push(CallStep { ok: false, text: format!("Windows Update: {e}") }),
        }
        s.push(crate::audiofix::call_mic_step());
        s
    })
    .await
    .map_err(|e| e.to_string())?;
    steps.extend(more);
    Ok(steps)
}

// ---------- Solo el cohete (con AdminOps cerrada) ----------

/// `--rocket`: este proceso es solo la ventanita del cohete.
pub fn mark_rocket_only() {
    ONLY.store(true, Ordering::SeqCst);
}

/// La ventana principal de AdminOps está abierta: lo marca para que un «solo el cohete» se retire.
pub fn mark_full_running() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::{InitializeSecurityDescriptor, SetSecurityDescriptorDacl, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        let name: Vec<u16> = MUTEX_FULL.encode_utf16().chain(Some(0)).collect();
        // SAFETY: nombre propio; el mutex vive lo que el proceso (no se cierra a propósito). Sin lista de permisos
        // (DACL nula): en modo técnico (elevada) el «solo el cohete» sin elevar también tiene que poder verlo.
        unsafe {
            let mut sd: SECURITY_DESCRIPTOR = std::mem::zeroed();
            let sd_ptr = (&mut sd as *mut SECURITY_DESCRIPTOR).cast();
            let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd_ptr, bInheritHandle: 0 };
            let ok = InitializeSecurityDescriptor(sd_ptr, 1) != 0 && SetSecurityDescriptorDacl(sd_ptr, 1, std::ptr::null(), 0) != 0;
            CreateMutexW(if ok { &sa } else { std::ptr::null() }, 0, name.as_ptr());
        }
    }
}

#[cfg(windows)]
pub fn full_running() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
    let name: Vec<u16> = MUTEX_FULL.encode_utf16().chain(Some(0)).collect();
    // SAFETY: solo se abre y se cierra el mutex con nombre.
    unsafe {
        let h = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if h.is_null() {
            false
        } else {
            CloseHandle(h);
            true
        }
    }
}

#[cfg(not(windows))]
pub fn full_running() -> bool {
    false
}

/// En «solo el cohete»: cuando se abre AdminOps entera (que trae su propio cohete), este se retira.
pub fn watch_full(app: tauri::AppHandle) {
    // Solo cuenta una AdminOps que se abre DESPUÉS: la que dejó este cohete al cerrarse puede tardar unos segundos en
    // terminar (cerrando una grabación…), y verla todavía no es motivo para irse.
    std::thread::spawn(move || {
        let mut seen_gone = !full_running();
        let started = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let running = full_running();
            // Pasados 15 s, una AdminOps entera que sigue ahí no se está cerrando: está abierta, y trae su cohete.
            if running && (seen_gone || started.elapsed() >= Duration::from_secs(15)) {
                log::info!("Solo el cohete: AdminOps se abrió entera; este proceso se cierra.");
                app.exit(0);
                break;
            }
            if !running {
                seen_gone = true;
            }
        }
    });
}

/// «Abrir AdminOps» desde «solo el cohete»: abre la entera y este proceso se cierra (ella trae su cohete).
pub fn open_full(app: &tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // AdminOps se abre sin administrador («Modo usuario»), igual que con doble clic; el administrador se pide dentro,
    // con «Pasar a modo técnico». Este proceso se cierra: la entera trae su propio cohete.
    std::process::Command::new(exe).spawn().map_err(|e| format!("No se pudo abrir AdminOps: {e}"))?;
    app.exit(0);
    Ok(())
}

/// Al cerrar AdminOps entera, el cohete (con su barra) se queda en el escritorio: para cualquier usuario, instalada o
/// portable, haya o no arranque con Windows. Nunca elevado: en modo técnico se abre como el usuario de la sesión y sin
/// administrador (con la cuenta de otro, no puede heredarla; con la misma, no se queda elevado sin AdminOps delante).
pub fn leave_rocket_behind(app: &tauri::AppHandle) {
    // Una vez por cierre (se llama al destruirse la ventana y al salir); nunca desde «solo el cohete», al relanzar
    // (modo técnico/usuario), al salir sin dejar rastro ni en la prueba del .exe.
    if rocket_only() || SKIP_BEHIND.load(Ordering::SeqCst) || std::env::var_os("ADMINOPS_NO_ROCKET_BEHIND").is_some() || LEFT_BEHIND.swap(true, Ordering::SeqCst) {
        return;
    }
    let s = crate::workflow::settings(app);
    if s.rocket != "desktop" || crate::appcare::updating() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    std::thread::spawn(move || {
        // Portable: el cohete no se queda corriendo desde la memoria USB (Windows no dejaría expulsarla y, al sacarla,
        // se caería). Se copia a una carpeta de este equipo y corre desde ahí.
        let exe = if crate::paths::is_portable() { local_copy(&exe).unwrap_or(exe) } else { exe };
        let exe = exe.to_string_lossy().into_owned();
        if crate::elevation::is_elevated() {
            if let Err(e) = crate::shellopen::run_as_user_visible(&exe, "--rocket") {
                log::warn!("No se pudo dejar el cohete como el usuario: {e}");
            }
        } else if let Err(e) = std::process::Command::new(&exe).arg("--rocket").spawn() {
            log::warn!("No se pudo dejar el cohete: {e}");
        }
    });
}

static SKIP_BEHIND: AtomicBool = AtomicBool::new(false);
static LEFT_BEHIND: AtomicBool = AtomicBool::new(false);

/// Esta salida no deja el cohete (se relanza AdminOps, o se sale sin dejar rastro).
pub fn skip_rocket_behind() {
    SKIP_BEHIND.store(true, Ordering::SeqCst);
}

/// Copia el .exe portable a %LOCALAPPDATA%\AdminOps\cohete del usuario de la sesión (si no está ya igual) para que el
/// cohete corra desde el equipo y no desde la memoria USB. Corriendo desde ahí ya no es «portable»: usa los ajustes de
/// ese usuario. En modo técnico con la cuenta de otro, la carpeta es la del usuario de la sesión, no la del técnico:
/// el cohete se abre como él y no podría leer el perfil del técnico.
fn local_copy(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let base = crate::target_user::user_dirs()?.local_app_data.join("AdminOps");
    let dir = base.join("cohete");
    std::fs::create_dir_all(&dir).ok()?;
    // Elevada, AdminOps escribe en una carpeta del usuario: si alguna es un enlace (junction) a otro sitio, no se copia.
    if [&base, &dir].iter().any(|d| is_reparse_point(d)) {
        log::warn!("La carpeta del cohete es un enlace a otro sitio: no se copia ahí.");
        return None;
    }
    let dst = dir.join("AdminOps.exe");
    let same = std::fs::metadata(&dst).ok().zip(std::fs::metadata(exe).ok()).is_some_and(|(a, b)| a.len() == b.len() && a.modified().ok() >= b.modified().ok());
    if !same {
        // Si el de la vez anterior sigue abierto, no se puede pisar: se usa el que hay.
        if std::fs::copy(exe, &dst).is_err() && !dst.exists() {
            return None;
        }
    }
    Some(dst)
}

/// ¿Es un enlace (junction o enlace simbólico) y no una carpeta de verdad?
fn is_reparse_point(p: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT
        std::fs::symlink_metadata(p).is_ok_and(|m| m.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(windows))]
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

/// Un solo «solo el cohete» por sesión: el del inicio de sesión, el que deja AdminOps al cerrarse y el de una tarea
/// antigua no se juntan. Devuelve `false` si ya hay otro (este se cierra).
pub fn claim_rocket_only() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        let name: Vec<u16> = "Local\\AdminOpsCoheteSolo".encode_utf16().chain(Some(0)).collect();
        // SAFETY: nombre propio; el mutex vive lo que el proceso (no se cierra a propósito).
        unsafe {
            let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            !(h.is_null() || GetLastError() == ERROR_ALREADY_EXISTS)
        }
    }
    #[cfg(not(windows))]
    true
}

/// Dónde se apunta el arranque del cohete: el «Ejecutar al iniciar sesión» del usuario (HKCU\…\Run). No necesita
/// administrador, así que cada usuario del equipo lo tiene aunque no lo sea.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "AdminOps (cohete)";

/// La clave Run del usuario de la sesión: la propia; en modo técnico con la cuenta de otro, la suya en HKU.
#[cfg(windows)]
fn session_run_key(write: bool) -> Result<winreg::RegKey, String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_USERS, KEY_READ, KEY_WRITE};
    let flags = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
    let other = crate::elevation::is_elevated() && matches!((crate::target_user::own_sid(), crate::target_user::get()), (Some(own), Some(t)) if own != t.sid);
    let r = if other {
        let sid = crate::target_user::get().map(|t| t.sid.clone()).ok_or("No se sabe qué usuario tiene la sesión abierta.")?;
        let path = format!(r"{sid}\{RUN_KEY}");
        if write { winreg::RegKey::predef(HKEY_USERS).create_subkey_with_flags(&path, flags).map(|(k, _)| k) } else { winreg::RegKey::predef(HKEY_USERS).open_subkey_with_flags(&path, flags) }
    } else if write {
        winreg::RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(RUN_KEY, flags).map(|(k, _)| k)
    } else {
        winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, flags)
    };
    r.map_err(|e| e.to_string())
}

fn run_value_set() -> bool {
    #[cfg(windows)]
    {
        session_run_key(false).and_then(|k| k.get_value::<String, _>(RUN_VALUE).map_err(|e| e.to_string())).is_ok()
    }
    #[cfg(not(windows))]
    false
}

/// La tarea programada de las versiones anteriores (solo la quita un administrador).
fn legacy_task_exists() -> bool {
    let ps = format!("if (Get-ScheduledTask -TaskName {} -ErrorAction SilentlyContinue) {{ 'si' }}", crate::ps::ps_literal(TASK));
    crate::ps::powershell(&ps).map(|s| s.trim() == "si").unwrap_or(false)
}

fn remove_legacy_task() {
    if crate::elevation::is_elevated() {
        let _ = crate::ps::powershell(&format!("Unregister-ScheduledTask -TaskName {} -Confirm:$false -ErrorAction SilentlyContinue", crate::ps::ps_literal(TASK)));
    }
}

/// Tras actualizar o mover AdminOps de carpeta: si el arranque del cohete apunta a otro .exe, se corrige.
/// `Some(true)` si lo corrigió, `Some(false)` si estaba bien, `None` si no hay arranque puesto.
pub fn fix_startup_path() -> Option<bool> {
    if cfg!(debug_assertions) {
        return None;
    }
    #[cfg(windows)]
    {
        let key = session_run_key(false).ok()?;
        let now: String = key.get_value(RUN_VALUE).ok()?;
        let exe = std::env::current_exe().ok()?;
        let want = format!("\"{}\" --rocket", exe.to_string_lossy());
        if now.eq_ignore_ascii_case(&want) {
            return Some(false);
        }
        Some(set_startup(true).is_ok())
    }
    #[cfg(not(windows))]
    None
}

/// Queda la tarea de una versión anterior: la quita en modo técnico. `Some(true)` quitada, `Some(false)` queda.
pub fn fix_legacy_task() -> Option<bool> {
    if !legacy_task_exists() {
        return None;
    }
    remove_legacy_task();
    Some(!legacy_task_exists())
}

fn startup_enabled() -> bool {
    run_value_set() || legacy_task_exists()
}

#[tauri::command(async)]
pub fn rocket_startup_enabled() -> bool {
    startup_enabled()
}

/// Pone o quita el arranque del cohete al iniciar sesión, para el usuario de la sesión. Sin administrador.
fn set_startup(enabled: bool) -> Result<(), String> {
    if crate::paths::is_portable() {
        return Err("En la versión portable no se puede: arrancaría desde la memoria USB, que no siempre está. Al cerrar AdminOps el cohete se queda igualmente.".into());
    }
    #[cfg(windows)]
    {
        let key = session_run_key(true)?;
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            key.set_value(RUN_VALUE, &format!("\"{}\" --rocket", exe.to_string_lossy())).map_err(|e| format!("No se pudo poner el arranque: {e}"))?;
        } else {
            match key.delete_value(RUN_VALUE) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("No se pudo quitar el arranque: {e}")),
            }
        }
    }
    // La tarea de antes arrancaría otro cohete (o lo seguiría arrancando al quitarlo): fuera, si se puede.
    remove_legacy_task();
    Ok(())
}

/// «El cohete arranca con Windows» (Ajustes → General): para el usuario de la sesión, sin administrador.
#[tauri::command(async)]
pub fn rocket_set_startup(enabled: bool) -> Result<(), String> {
    set_startup(enabled)?;
    if !enabled && legacy_task_exists() {
        return Err("Queda la tarea de una versión anterior, que solo se puede quitar en modo técnico.".into());
    }
    Ok(())
}

/// La primera vez que un usuario abre AdminOps (instalada), el cohete queda arrancando con Windows: la barra está
/// siempre a mano. Se apunta que ya se hizo, para respetar que luego lo quite en Ajustes.
pub fn ensure_default_startup(app: &tauri::AppHandle) {
    // Con el modo auditoría no se escribe nada en el equipo; tampoco en la prueba del .exe.
    if crate::paths::is_portable() || crate::audit::active() || std::env::var_os("ADMINOPS_NO_ROCKET_BEHIND").is_some() || crate::workflow::settings(app).rocket != "desktop" {
        return;
    }
    // La marca va con el usuario de la sesión (como la clave Run): si la quitó él, no se la vuelve a poner nadie.
    if default_startup_marked() {
        return;
    }
    match set_startup(true) {
        Ok(()) => mark_default_startup(),
        Err(e) => log::warn!("Arranque del cohete por defecto: {e}"),
    }
}

const MARK_KEY: &str = r"Software\AdminOps";
const MARK_VALUE: &str = "CoheteArranquePuesto";

#[cfg(windows)]
fn session_key(path: &str, write: bool) -> Result<winreg::RegKey, String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_USERS, KEY_READ, KEY_WRITE};
    let flags = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
    let other = crate::elevation::is_elevated() && matches!((crate::target_user::own_sid(), crate::target_user::get()), (Some(own), Some(t)) if own != t.sid);
    let (root, full) = if other {
        let sid = crate::target_user::get().map(|t| t.sid.clone()).ok_or("No se sabe qué usuario tiene la sesión abierta.")?;
        (winreg::RegKey::predef(HKEY_USERS), format!(r"{sid}\{path}"))
    } else {
        (winreg::RegKey::predef(HKEY_CURRENT_USER), path.to_string())
    };
    let r = if write { root.create_subkey_with_flags(&full, flags).map(|(k, _)| k) } else { root.open_subkey_with_flags(&full, flags) };
    r.map_err(|e| e.to_string())
}

fn default_startup_marked() -> bool {
    #[cfg(windows)]
    {
        session_key(MARK_KEY, false).and_then(|k| k.get_value::<u32, _>(MARK_VALUE).map_err(|e| e.to_string())).is_ok()
    }
    #[cfg(not(windows))]
    true
}

fn mark_default_startup() {
    #[cfg(windows)]
    if let Ok(k) = session_key(MARK_KEY, true) {
        let _ = k.set_value(MARK_VALUE, &1u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empezar_el_dia_cuenta_desde_que_empieza_y_solo_por_la_manana() {
        assert_eq!(morning_window(Duration::from_secs(30), 8), None);
        assert_eq!(morning_window(Duration::from_secs(95), 8), Some(true));
        assert_eq!(morning_window(Duration::from_secs(2 * 3600), 10), Some(true));
        assert_eq!(morning_window(Duration::from_secs(7 * 3600), 11), Some(false));
        // Abrir AdminOps (o despertar el portátil) por la tarde no es empezar el día.
        assert_eq!(morning_window(Duration::from_secs(95), 15), Some(false));
        // De madrugada (medianoche, mantenimiento de las 3:00) se espera, sin gastar el día.
        assert_eq!(morning_window(Duration::from_secs(95), 0), None);
        assert_eq!(morning_window(Duration::from_secs(4 * 3600), 3), None);
    }

    #[test]
    fn papelera() {
        let day = 10_000_000u64 * 86_400;
        let now = 200 * day;
        let mut raw = vec![2u8, 0, 0, 0, 0, 0, 0, 0];
        raw.extend_from_slice(&5_000u64.to_le_bytes());
        raw.extend_from_slice(&(now - 31 * day).to_le_bytes());
        assert_eq!(parse_recycle_info(&raw, now), Some((5_000, 31)));
        assert_eq!(parse_recycle_info(&raw[..10], now), None);
    }

    #[test]
    fn cuando_despega_solo() {
        let m = Duration::from_secs(60);
        assert_eq!(auto_due(m * 3, Duration::ZERO, 92, None), Some("memoria"));
        assert_eq!(auto_due(m, Duration::ZERO, 92, None), None);
        assert_eq!(auto_due(Duration::ZERO, m * 20, 82, None), Some("sin usar"));
        assert_eq!(auto_due(Duration::ZERO, m * 20, 60, None), None);
        // Hace 10 min que despegó: espera.
        assert_eq!(auto_due(m * 3, Duration::ZERO, 95, Some(m * 10)), None);
    }

    #[test]
    fn tamano_sin_enlaces() {
        let dir = std::env::temp_dir().join(format!("adminops-deep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::write(dir.join("a").join("x.bin"), [0u8; 300]).unwrap();
        assert_eq!(dir_size(&dir, Duration::ZERO, 0, Instant::now() + Duration::from_secs(5)), 300);
        assert_eq!(dir_size(&dir, Duration::from_secs(86_400), 0, Instant::now() + Duration::from_secs(5)), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
