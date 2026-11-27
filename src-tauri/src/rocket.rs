//! El cohete de «Optimizar ya»: una burbuja redonda que se puede mover a cualquier sitio de la
//! pantalla y que, al tocarla, aligera el equipo sin cortar lo que el usuario está haciendo:
//!
//! - Memoria: vacía el conjunto de trabajo de los procesos (lo que no usan vuelve a la lista de
//!   espera y Windows se lo devuelve al momento si lo piden) y la parte de baja prioridad de la caché
//!   en espera. Nunca toca el programa que está delante ni los del sistema que mueven el sonido o la
//!   pantalla.
//! - Temporales: lo que tiene más de un día en la carpeta temporal del usuario y en la de Windows.
//!   Lo que está abierto se salta.
//! - Informes de errores de Windows con más de una semana.
//! - La caché de los navegadores (Chrome, Edge, Brave, Opera, Firefox), también abiertos: lo que está en uso
//!   se salta; nunca contraseñas, cookies ni historial (pedido por David en la 1.5.1).
//! - Caché DNS.
//!
//! Lo que podría notarse (vaciar la papelera, reiniciar el Explorador) no está aquí a propósito.
//!
//! Tres modos (Ajustes → General): `desktop`, una ventanita sin marco y siempre encima que sigue ahí
//! con AdminOps minimizada o escondida junto al reloj; `app`, la misma burbuja dentro de la ventana
//! de AdminOps (la pinta la interfaz); `off`. En el escritorio se esconde sola mientras haya algo a
//! pantalla completa (una presentación, un juego, un vídeo).

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tauri::{Emitter, Manager};

pub const LABEL: &str = "rocket";
/// Lado de la ventanita plegada (px lógicos). La burbuja mide menos: el resto es para el halo.
const SIZE: f64 = 68.0;
/// Separación del borde de la pantalla al colocarla la primera vez.
const MARGIN: i32 = 24;
pub const MODE_EVENT: &str = "rocket://mode";

static BUSY: AtomicBool = AtomicBool::new(false);
/// Desplegada: mientras tanto sus movimientos no son la posición de la burbuja.
static WIDE: AtomicBool = AtomicBool::new(false);
/// Dónde estaba la burbuja plegada antes de desplegarse.
static ANCHOR: Mutex<Option<(i32, i32)>> = Mutex::new(None);
/// La escondió una pantalla completa (y hay que volver a enseñarla después).
static HIDDEN_FULLSCREEN: AtomicBool = AtomicBool::new(false);
/// Para guardar la posición una vez que se deja de arrastrar y no en cada píxel.
static MOVE_GEN: AtomicU64 = AtomicU64::new(0);
static SAVER: AtomicBool = AtomicBool::new(false);
static WATCHER: AtomicBool = AtomicBool::new(false);

pub fn valid_mode(m: &str) -> bool {
    matches!(m, "desktop" | "app" | "off")
}

// ---------- La optimización ----------

/// Lo que hizo una pasada, para la frase de después.
#[derive(Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Boost {
    /// Memoria que quedó libre (diferencia de memoria en uso antes y después), en bytes.
    pub ram_freed: u64,
    pub ram_used_before: u64,
    pub ram_used_after: u64,
    pub ram_total: u64,
    /// Procesos a los que se les vació la memoria que no usaban.
    pub trimmed: u32,
    /// Se vació la parte de baja prioridad de la caché en espera.
    pub standby: bool,
    /// Temporales e informes de errores borrados.
    pub temp_freed: u64,
    pub temp_files: u32,
    /// Archivos que estaban abiertos y se dejaron.
    pub temp_skipped: u32,
    pub dns: bool,
    pub ms: u64,
    /// Qué se llevó: la memoria que soltó cada programa (los que más) y lo borrado por carpeta.
    pub apps: Vec<Freed>,
    pub folders: Vec<Freed>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Freed {
    pub name: String,
    pub bytes: u64,
}

/// «chrome.exe» → «Chrome»: el nombre que el usuario reconoce.
pub fn app_label(exe: &str) -> String {
    let stem = exe.trim().trim_end_matches(".exe").trim_end_matches(".EXE");
    let known = [
        ("chrome", "Chrome"), ("msedge", "Edge"), ("firefox", "Firefox"), ("ms-teams", "Teams"), ("teams", "Teams"),
        ("outlook", "Outlook"), ("olk", "Outlook"), ("excel", "Excel"), ("winword", "Word"), ("powerpnt", "PowerPoint"),
        ("explorer", "Explorador"), ("onedrive", "OneDrive"), ("msedgewebview2", "Componentes web"), ("searchhost", "Búsqueda de Windows"),
        ("acrobat", "Acrobat"), ("acrord32", "Acrobat Reader"), ("anydesk", "AnyDesk"), ("whatsapp", "WhatsApp"), ("spotify", "Spotify"),
        ("adminops", "AdminOps"), ("code", "VS Code"), ("zoom", "Zoom"),
    ];
    let low = stem.to_ascii_lowercase();
    if let Some((_, l)) = known.iter().find(|(k, _)| *k == low) {
        return (*l).to_string();
    }
    let mut c = stem.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Suma por programa y se queda con los que más soltaron (al menos 5 MB).
pub fn top_apps(per_proc: &[(String, u64)], n: usize) -> Vec<Freed> {
    let mut sum: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for (exe, b) in per_proc {
        *sum.entry(app_label(exe)).or_default() += b;
    }
    let mut v: Vec<Freed> = sum.into_iter().filter(|(_, b)| *b >= 5 * 1_048_576).map(|(name, bytes)| Freed { name, bytes }).collect();
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.name.cmp(&b.name)));
    v.truncate(n);
    v
}

/// Optimiza. Si ya hay una pasada en marcha (dos clics seguidos, o el cohete y la interfaz a la vez),
/// se dice en vez de lanzar otra encima.
#[tauri::command]
pub async fn rocket_boost(app: tauri::AppHandle) -> Result<Boost, String> {
    boost(&app, "Optimizar ya (cohete)").await
}

/// Una pasada, apuntada en el diario con su antes y después de memoria: así sale en el caso abierto
/// y en el informe del cliente.
pub async fn boost(app: &tauri::AppHandle, title: &str) -> Result<Boost, String> {
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("Ya está optimizando.".into());
    }
    // Cada paso se avisa a la interfaz (la memoria bajando en vivo y la lista con su ✓).
    let em = app.clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        run(&move |step: &str, text: String| {
            let _ = em.emit(PROGRESS_EVENT, Progress { step: step.to_string(), mem_pct: rocket_memory(), text });
        })
    })
    .await
    .map_err(|e| e.to_string());
    BUSY.store(false, Ordering::SeqCst);
    let b = r?;
    record(app, title, &b);
    log::info!(
        "Cohete: {} MB de memoria ({} procesos, espera baja: {}), {} MB en {} temporales ({} en uso), DNS: {}, {} ms",
        b.ram_freed / 1_048_576,
        b.trimmed,
        b.standby,
        b.temp_freed / 1_048_576,
        b.temp_files,
        b.temp_skipped,
        b.dns,
        b.ms
    );
    Ok(b)
}

/// «Optimizar ya: 1,6 GB de memoria y 340 MB de temporales», con la memoria en uso antes y después.
fn record(app: &tauri::AppHandle, title: &str, b: &Boost) {
    let Some(state) = app.try_state::<crate::tweaks::TweakState>() else { return };
    let pct = |used: u64| if b.ram_total == 0 { 0.0 } else { (used as f64 / b.ram_total as f64 * 100.0).round() };
    let gb = |n: u64| crate::measure::fmt_num((n as f64 / 1_073_741_824.0 * 10.0).round() / 10.0);
    let mb = |n: u64| (n / 1_048_576).to_string();
    let text = format!("{title}: {} GB de memoria y {} MB de temporales", gb(b.ram_freed), mb(b.temp_freed));
    let measure = crate::measure::Measure { label: "Memoria en uso".into(), before: pct(b.ram_used_before), after: pct(b.ram_used_after), unit: "%".into() };
    state.record_measured(crate::tweaks::journal::Op::Run, &text, &Ok::<(), String>(()), Some(measure));
}

pub const PROGRESS_EVENT: &str = "rocket://progress";

/// Un paso de la pasada: memory | temp | browser | dns, con la memoria en uso en ese momento.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    step: String,
    mem_pct: u32,
    text: String,
}

/// Las cachés de los navegadores del usuario (Chrome, Edge, Brave, Opera y Firefox), en todos sus perfiles. Con el
/// navegador abierto lo que esté en uso no se puede borrar y se salta; contraseñas, cookies e historial no están aquí.
fn browser_caches() -> Vec<PathBuf> {
    let Some(u) = crate::target_user::user_dirs() else { return vec![] };
    let mut out = Vec::new();
    for base in [r"Google\Chrome\User Data", r"Microsoft\Edge\User Data", r"BraveSoftware\Brave-Browser\User Data"] {
        let root = u.local_app_data.join(base);
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "Default" || name.starts_with("Profile ") {
                for c in [r"Cache\Cache_Data", "Code Cache", "GPUCache"] {
                    out.push(e.path().join(c));
                }
            }
        }
    }
    let opera = u.local_app_data.join(r"Opera Software\Opera Stable");
    for c in [r"Cache\Cache_Data", "Code Cache", "GPUCache"] {
        out.push(opera.join(c));
    }
    if let Ok(rd) = std::fs::read_dir(u.local_app_data.join(r"Mozilla\Firefox\Profiles")) {
        for e in rd.flatten() {
            out.push(e.path().join("cache2"));
        }
    }
    out.retain(|p| p.is_dir());
    out
}

fn run(on: &dyn Fn(&str, String)) -> Boost {
    let t0 = Instant::now();
    let mut b = Boost::default();
    let (total, before) = memory();
    b.ram_total = total;
    b.ram_used_before = before;

    on("memory", "Liberando memoria".into());
    let per_proc = trim_working_sets();
    b.trimmed = per_proc.len() as u32;
    b.apps = top_apps(&per_proc, 5);
    b.standby = purge_low_priority_standby();
    let t_mem = t0.elapsed().as_millis();
    let freed: u64 = per_proc.iter().map(|(_, b)| b).sum();
    on("memory_done", format!("Memoria · {} MB", freed / 1_048_576));

    // Los temporales del usuario con la sesión abierta (no los de la cuenta que elevó AdminOps).
    let day = Duration::from_secs(24 * 3600);
    let mut dirs: Vec<(PathBuf, Duration, &str)> = Vec::new();
    if let Some(u) = crate::target_user::user_dirs() {
        dirs.push((u.temp, day, "Temporales del usuario"));
    }
    if let Some(root) = std::env::var_os("SystemRoot") {
        dirs.push((Path::new(&root).join("Temp"), day * 7, "Temporales de Windows"));
    }
    if let Some(pd) = std::env::var_os("ProgramData") {
        let wer = Path::new(&pd).join(r"Microsoft\Windows\WER");
        dirs.push((wer.join("ReportArchive"), day * 7, "Informes de errores"));
        dirs.push((wer.join("ReportQueue"), day * 7, "Informes de errores"));
    }
    on("temp", "Borrando temporales".into());
    // Que el cohete no se quede minutos girando en una carpeta temporal enorme: lo que no dé
    // tiempo se hace la próxima vez.
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut seen = std::collections::HashSet::new();
    for (dir, age, label) in dirs {
        let key = dir.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let mut c = Clean::default();
        clean_dir(&dir, age, 0, deadline, &mut c);
        b.temp_freed += c.bytes;
        b.temp_files += c.files;
        b.temp_skipped += c.skipped;
        if c.bytes > 0 {
            match b.folders.iter_mut().find(|f| f.name == label) {
                Some(f) => f.bytes += c.bytes,
                None => b.folders.push(Freed { name: label.into(), bytes: c.bytes }),
            }
        }
    }

    on("temp_done", format!("Temporales · {} MB", b.temp_freed / 1_048_576));
    // La caché de los navegadores, también abiertos (sin edad mínima: es caché).
    on("browser", "Limpiando la caché de los navegadores".into());
    let mut cache = Clean::default();
    let cache_deadline = Instant::now() + Duration::from_secs(4);
    for dir in browser_caches() {
        clean_dir(&dir, Duration::ZERO, 0, cache_deadline, &mut cache);
    }
    if cache.bytes > 0 {
        b.temp_freed += cache.bytes;
        b.temp_files += cache.files;
        b.folders.push(Freed { name: "Caché de los navegadores".into(), bytes: cache.bytes });
    }
    on("browser_done", format!("Caché de navegadores · {} MB", cache.bytes / 1_048_576));
    let t_tmp = t0.elapsed().as_millis();
    log::info!("Cohete: memoria en {t_mem} ms, temporales en {} ms", t_tmp - t_mem);
    on("dns", "Vaciando la caché DNS".into());
    b.dns = crate::ps::hidden("ipconfig").arg("/flushdns").output().map(|o| o.status.success()).unwrap_or(false);
    on("dns_done", if b.dns { "Caché DNS vaciada".into() } else { "Caché DNS: no se pudo".into() });

    // Un respiro para que Windows apunte lo liberado antes de medir otra vez.
    std::thread::sleep(Duration::from_millis(400));
    let (_, after) = memory();
    b.ram_used_after = after;
    b.ram_freed = before.saturating_sub(after);
    b.ms = t0.elapsed().as_millis() as u64;
    b
}

/// (total, en uso) en bytes.
#[cfg(windows)]
fn memory() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: estructura propia con su tamaño puesto, como pide la API.
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return (0, 0);
    }
    (m.ullTotalPhys, m.ullTotalPhys.saturating_sub(m.ullAvailPhys))
}

#[cfg(not(windows))]
fn memory() -> (u64, u64) {
    (0, 0)
}

/// Procesos que no se tocan: el núcleo y los que, al volver a cargar su memoria, se notarían
/// (el sonido daría un chasquido, la pantalla un tirón).
pub fn protected(name: &str) -> bool {
    const KEEP: [&str; 12] = [
        "system",
        "idle",
        "registry",
        "memory compression",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "winlogon.exe",
        "lsass.exe",
        "dwm.exe",
        "audiodg.exe",
        "msmpeng.exe",
    ];
    let n = name.to_ascii_lowercase();
    KEEP.contains(&n.as_str())
}

/// Vacía el conjunto de trabajo de cada proceso que se puede y devuelve (ejecutable, bytes que soltó).
#[cfg(windows)]
fn trim_working_sets() -> Vec<(String, u64)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::{OpenProcess, SetProcessWorkingSetSize, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    // El programa que el usuario tiene delante se queda como está.
    let mut front = 0u32;
    // SAFETY: llamadas de solo lectura sobre la ventana activa; `front` es nuestra.
    unsafe {
        let w = GetForegroundWindow();
        if !w.is_null() {
            GetWindowThreadProcessId(w, &mut front);
        }
    }
    let mut out = Vec::new();
    /// Memoria en uso (conjunto de trabajo) de un proceso abierto.
    unsafe fn ws(h: windows_sys::Win32::Foundation::HANDLE) -> u64 {
        // SAFETY: estructura propia con su tamaño; `h` es un proceso abierto por quien llama.
        unsafe {
            let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            if K32GetProcessMemoryInfo(h, &mut c, c.cb) != 0 {
                c.WorkingSetSize as u64
            } else {
                0
            }
        }
    }
    // SAFETY: instantánea propia, recorrida con la estructura de su tamaño y cerrada al final.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let pid = e.th32ProcessID;
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]);
            if pid > 4 && pid != front && !protected(&name) {
                let h = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if !h.is_null() {
                    // (-1, -1): «vacía lo que no esté usando», lo mismo que EmptyWorkingSet.
                    let before = ws(h);
                    if SetProcessWorkingSetSize(h, usize::MAX, usize::MAX) != 0 {
                        out.push((name.clone(), before.saturating_sub(ws(h))));
                    }
                    CloseHandle(h);
                }
            }
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
fn trim_working_sets() -> Vec<(String, u64)> {
    Vec::new()
}

/// Vacía solo la parte de baja prioridad de la caché en espera (lo que Windows precargó «por si
/// acaso»): la caché de lo que se usa de verdad se queda. Necesita administrador.
#[cfg(windows)]
fn purge_low_priority_standby() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows_sys::Win32::Security::{AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY};
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    if !crate::elevation::is_elevated() {
        return false;
    }
    const SYSTEM_MEMORY_LIST_INFORMATION: i32 = 80;
    const MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST: i32 = 5;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    // SAFETY: el token es el de este proceso y se cierra; las estructuras son propias.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        let name = wide("SeProfileSingleProcessPrivilege");
        let ok = LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) != 0 && {
            let tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
            AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut()) != 0
                && windows_sys::Win32::Foundation::GetLastError() == 0
        };
        CloseHandle(token);
        if !ok {
            return false;
        }
        let ntdll = wide("ntdll.dll");
        let module = GetModuleHandleW(ntdll.as_ptr());
        if module.is_null() {
            return false;
        }
        let Some(f) = GetProcAddress(module, c"NtSetSystemInformation".as_ptr().cast()) else {
            return false;
        };
        type NtSet = unsafe extern "system" fn(i32, *mut core::ffi::c_void, u32) -> i32;
        let nt_set: NtSet = std::mem::transmute(f);
        let mut cmd: i32 = MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST;
        nt_set(SYSTEM_MEMORY_LIST_INFORMATION, (&mut cmd as *mut i32).cast(), std::mem::size_of::<i32>() as u32) >= 0
    }
}

#[cfg(not(windows))]
fn purge_low_priority_standby() -> bool {
    false
}

#[derive(Default, Debug)]
pub struct Clean {
    pub bytes: u64,
    pub files: u32,
    pub skipped: u32,
}

/// Borra lo que tenga más de `age` sin cambios. No sigue enlaces ni uniones (podrían llevar fuera de
/// la carpeta) y deja las carpetas que no queden vacías. Lo que está abierto falla al borrar y se cuenta
/// como saltado.
pub fn clean_dir(dir: &Path, age: Duration, depth: u32, deadline: Instant, c: &mut Clean) {
    // AdminOps corre como administrador y la carpeta Temp es del usuario: si la cambió por un enlace o una
    // unión a otra carpeta (Documentos, Archivos de programa…), no se toca nada.
    let Ok(meta) = std::fs::symlink_metadata(dir) else { return };
    if is_link(&meta) || !meta.is_dir() {
        return;
    }
    let Ok(root) = dir.canonicalize() else { return };
    walk(dir, &root, age, depth, deadline, c);
}

fn walk(dir: &Path, root: &Path, age: Duration, depth: u32, deadline: Instant, c: &mut Clean) {
    if depth > 16 || Instant::now() > deadline {
        return;
    }
    // Entre mirar la subcarpeta y entrar en ella podría haberse cambiado por un enlace: tiene que seguir
    // dentro de la carpeta de partida.
    if !dir.canonicalize().is_ok_and(|d| d.starts_with(root)) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let now = SystemTime::now();
    let old = |m: &std::fs::Metadata| m.modified().ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|d| d >= age);
    for entry in rd.flatten() {
        if Instant::now() > deadline {
            return;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if is_link(&meta) {
            continue;
        }
        if meta.is_dir() {
            walk(&path, root, age, depth + 1, deadline, c);
            // Solo si se quedó vacía y es vieja; si no, remove_dir falla y se queda.
            if old(&meta) {
                let _ = std::fs::remove_dir(&path);
            }
        } else if old(&meta) {
            let len = meta.len();
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    c.bytes += len;
                    c.files += 1;
                }
                Err(_) => c.skipped += 1,
            }
        }
    }
}

#[cfg(windows)]
pub(crate) fn is_link(m: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    m.file_type().is_symlink() || m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(crate) fn is_link(m: &std::fs::Metadata) -> bool {
    m.file_type().is_symlink()
}

// ---------- La ventanita del escritorio ----------

#[derive(Serialize, serde::Deserialize, Default, Clone, Copy)]
struct Pos {
    x: i32,
    y: i32,
}

fn pos_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("rocket.json")
}

/// Las posiciones por combinación de pantallas (portátil solo, en su base con dos monitores…).
fn layouts_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("rocket-pantallas.json")
}

/// La «firma» de las pantallas conectadas: posición, tamaño y escala de cada una. Cambia al conectar o quitar la
/// base, un monitor o al cambiar la escala.
fn screens_key(w: &tauri::WebviewWindow) -> String {
    let Ok(mons) = w.available_monitors() else { return String::new() };
    let mut parts: Vec<String> = mons
        .iter()
        .map(|m| format!("{},{},{}x{}@{}", m.position().x, m.position().y, m.size().width, m.size().height, (m.scale_factor() * 100.0).round() as i64))
        .collect();
    parts.sort();
    parts.join("|")
}

/// Dónde se dejó con estas pantallas; si nunca se movió con ellas, la posición de antes (versiones anteriores).
fn load_pos(app: &tauri::AppHandle, key: &str) -> Option<Pos> {
    let map: std::collections::HashMap<String, Pos> = crate::paths::read_json(&layouts_path(app));
    map.get(key).copied().or_else(|| std::fs::read(pos_path(app)).ok().and_then(|b| serde_json::from_slice(&b).ok()))
}

/// Guarda dónde se dejó, para estas pantallas.
fn save_pos(app: &tauri::AppHandle, w: &tauri::WebviewWindow, pos: Pos) {
    let key = screens_key(w);
    let _ = crate::paths::write_json(&pos_path(app), &pos);
    if key.is_empty() {
        return;
    }
    let path = layouts_path(app);
    let mut map: std::collections::HashMap<String, Pos> = crate::paths::read_json(&path);
    // Pocas combinaciones de verdad; si crece mucho (monitores de salas, proyectores), se empieza de nuevo.
    if map.len() > 24 {
        map.clear();
    }
    map.insert(key, pos);
    let _ = crate::paths::write_json(&path, &map);
}

/// Cambiaron las pantallas (se conectó o quitó la base, un monitor, la escala): la barra se recoge y el cohete vuelve
/// a donde se dejó con estas pantallas, o a su sitio de siempre; y se vuelve a pegar a su borde.
fn screens_changed(app: &tauri::AppHandle, w: &tauri::WebviewWindow, key: &str) {
    WIDE.store(false, Ordering::SeqCst);
    BAR_Y.store(i32::MIN, Ordering::SeqCst);
    let (x, y) = match load_pos(app, key).filter(|p| on_screen(w, p.x, p.y)) {
        Some(p) => (p.x, p.y),
        None => default_pos(w),
    };
    quiet();
    let _ = w.set_size(tauri::LogicalSize::new(size(), size()));
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    no_frame(w);
    dock_check(app, w, false);
    // La interfaz cierra lo que tuviera abierto y se coloca recogida en su sitio nuevo.
    let _ = app.emit(SCREENS_EVENT, ());
}

pub const SCREENS_EVENT: &str = "rocket://screens";

/// Pone el modo: abre o cierra la ventanita del escritorio y avisa a la interfaz (que pinta la
/// burbuja dentro de AdminOps en modo `app`). Crear ventanas desde un comando síncrono puede
/// bloquear WebView2: se hace en otro hilo.
pub fn apply_mode(app: &tauri::AppHandle, mode: &str) {
    let app2 = app.clone();
    let desktop = mode == "desktop";
    std::thread::spawn(move || {
        if desktop {
            if let Err(e) = open(&app2) {
                log::warn!("Cohete: {e}");
            }
        } else {
            close(&app2);
        }
    });
    let _ = app.emit(MODE_EVENT, mode);
}

/// Cambió el modo o el tamaño en Ajustes: se cierra y se vuelve a abrir con lo nuevo.
pub fn reopen(app: &tauri::AppHandle, mode: &str) {
    let app2 = app.clone();
    let desktop = mode == "desktop";
    std::thread::spawn(move || {
        close(&app2);
        if desktop {
            if let Err(e) = open(&app2) {
                log::warn!("Cohete: {e}");
            }
        }
    });
    let _ = app.emit(MODE_EVENT, mode);
}

/// Cambia el modo desde el propio cohete (su menú) o desde Ajustes, y lo guarda.
#[tauri::command]
pub fn rocket_set_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    if !valid_mode(&mode) {
        return Err("Modo del cohete no válido.".into());
    }
    crate::workflow::set_rocket_mode(&app, &mode)?;
    apply_mode(&app, &mode);
    Ok(())
}

/// Abre AdminOps desde el menú del cohete.
#[tauri::command]
pub fn rocket_open_main(app: tauri::AppHandle) -> Result<(), String> {
    if crate::rocketplus::rocket_only() {
        return crate::rocketplus::open_full(&app);
    }
    crate::tray::show_main(&app);
    Ok(())
}

pub fn open(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.show();
        return Ok(());
    }
    let zoom = zoom_of(&crate::workflow::settings(app).rocket_size);
    ZOOM_PCT.store((zoom * 100.0) as u32, Ordering::SeqCst);
    let side = size();
    let mut b = tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("index.html".into()))
        .title("AdminOps · optimizar")
        .inner_size(side, side)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .disable_drag_drop_handler()
        .initialization_script(format!("window.__ADMINOPS_ROCKET__ = true; window.__ADMINOPS_ROCKET_ZOOM__ = {zoom};"));
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    let w = b.build().map_err(|e| format!("No se pudo abrir el cohete: {e}"))?;
    WIDE.store(false, Ordering::SeqCst);
    let saved = load_pos(app, &screens_key(&w));
    let (x, y) = match saved.filter(|p| on_screen(&w, p.x, p.y)) {
        Some(p) => (p.x, p.y),
        None => default_pos(&w),
    };
    quiet();
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    // Ya en su monitor, con la escala de ese monitor.
    let _ = w.set_size(tauri::LogicalSize::new(side, side));
    w.show().map_err(|e| e.to_string())?;
    no_frame(&w);
    // Si se guardó pegada a un borde, sigue pegada.
    dock_check(app, &w, false);
    watch_fullscreen(app.clone());
    Ok(())
}

pub fn close(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
    WIDE.store(false, Ordering::SeqCst);
    HIDDEN_FULLSCREEN.store(false, Ordering::SeqCst);
    if let Ok(mut d) = DOCK.lock() {
        *d = None;
    }
    if let Ok(mut a) = ANCHOR.lock() {
        *a = None;
    }
}

/// Si está pegada a un borde y a cuál, para la interfaz al montarse (el aviso de abrir llega antes).
#[tauri::command]
pub fn rocket_dock_state() -> Option<String> {
    DOCK.lock().ok().and_then(|d| *d).map(|d| if d.right { "right".into() } else { "left".into() })
}

/// Windows a veces le devuelve el marco (barra de título y X) a una ventana sin marco al enseñarla o cambiarle el
/// tamaño. Esta nunca debe tenerlo: se le quita cada vez.
pub(crate) fn no_frame(w: &tauri::WebviewWindow) {
    let _ = w.set_decorations(false);
    let _ = w.set_shadow(false);
    let _ = w.set_always_on_top(true);
}

/// Los tres tamaños de Ajustes: pequeño, normal y grande.
pub fn zoom_of(size: &str) -> f64 {
    match size {
        "s" => 0.8,
        "l" => 1.25,
        _ => 1.0,
    }
}

/// Lado de la ventanita plegada, en px lógicos, con el tamaño elegido.
fn size() -> f64 {
    SIZE * f64::from(ZOOM_PCT.load(Ordering::SeqCst)) / 100.0
}

/// Los movimientos que hace el propio AdminOps (pegar, asomar, desplegar) no son arrastres del
/// usuario: durante un momento no se guardan ni se vuelve a mirar si se pega al borde.
fn quiet() {
    let gen = QUIET_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    QUIET.store(true, Ordering::SeqCst);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(350));
        if QUIET_GEN.load(Ordering::SeqCst) == gen {
            QUIET.store(false, Ordering::SeqCst);
        }
    });
}

/// ¿Cae la burbuja dentro de algún monitor? Pegada a un borde está medio fuera, y vale.
/// (Si se desenchufó el monitor donde estaba, vuelve a su sitio de siempre.)
fn on_screen(w: &tauri::WebviewWindow, x: i32, y: i32) -> bool {
    let Ok(mons) = w.available_monitors() else { return false };
    mons.iter().any(|m| {
        let p = m.position();
        let s = m.size();
        let edge = (size() * m.scale_factor()) as i32 / 2 + 2;
        x >= p.x - edge && y >= p.y - edge && x + edge <= p.x + s.width as i32 && y + edge <= p.y + s.height as i32
    })
}

/// Abajo a la derecha del monitor principal, por encima de la barra de tareas.
fn default_pos(w: &tauri::WebviewWindow) -> (i32, i32) {
    let Ok(Some(m)) = w.primary_monitor() else { return (200, 200) };
    let scale = m.scale_factor();
    let side = (size() * scale) as i32;
    let margin = (f64::from(MARGIN) * scale) as i32;
    let area = m.work_area();
    let _ = margin;
    (area.position.x + area.size.width as i32 - side, area.position.y + area.size.height as i32 / 4)
}

/// Hacia dónde creció al desplegarse, para que la interfaz ponga la burbuja en su esquina.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Grow {
    pub left: bool,
    pub up: bool,
}

/// La burbuja se queda quieta y la ventana crece hacia el lado con sitio: a la izquierda si está en
/// la mitad derecha del monitor, hacia arriba si está en la mitad de abajo.
pub fn grow_dir(bubble_x: i32, bubble_y: i32, side: i32, mon: (i32, i32, i32, i32)) -> Grow {
    let (mx, my, mw, mh) = mon;
    Grow { left: bubble_x + side / 2 > mx + mw / 2, up: bubble_y + side / 2 > my + mh / 2 }
}

/// Despliega la ventanita a `width`×`height` (px lógicos) para el resultado, el desglose, la lista
/// de «a fondo» o el menú, o la pliega si cabe en la burbuja.
#[tauri::command]
pub fn rocket_size(app: tauri::AppHandle, width: f64, height: f64) -> Result<Grow, String> {
    let Some(w) = app.get_webview_window(LABEL) else { return Ok(Grow { left: false, up: false }) };
    let scale = w.scale_factor().unwrap_or(1.0);
    let s = size();
    let collapse = width <= s + 0.5 && height <= s + 0.5;
    if collapse {
        if !WIDE.load(Ordering::SeqCst) {
            return Ok(Grow { left: false, up: false });
        }
        quiet();
        let _ = w.set_size(tauri::LogicalSize::new(s, s));
        if let Some((x, y)) = ANCHOR.lock().ok().and_then(|mut a| a.take()) {
            let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
        }
        WIDE.store(false, Ordering::SeqCst);
        // Si estaba pegada al borde, vuelve a esconderse un poco después (la interfaz decide si el ratón sigue encima).
        return Ok(Grow { left: false, up: false });
    }
    // Dónde está la burbuja: la de antes de desplegar, o la de ahora si aún no se desplegó.
    let anchor = match ANCHOR.lock().ok().and_then(|a| *a) {
        Some(a) => a,
        None => {
            let pos = w.outer_position().map_err(|e| e.to_string())?;
            // Pegada al borde y medio escondida: se despliega desde donde asoma entera.
            let a = DOCK.lock().ok().and_then(|d| *d).map_or((pos.x, pos.y), |d| (d.peek_x, pos.y));
            if let Ok(mut slot) = ANCHOR.lock() {
                *slot = Some(a);
            }
            a
        }
    };
    WIDE.store(true, Ordering::SeqCst);
    let side = (s * scale) as i32;
    let mon = w.current_monitor().ok().flatten().map_or((0, 0, 1920, 1080), |m| (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32));
    let g = grow_dir(anchor.0, anchor.1, side, mon);
    let x = if g.left { anchor.0 - ((width - s) * scale) as i32 } else { anchor.0 };
    let y = if g.up { anchor.1 - ((height - s) * scale) as i32 } else { anchor.1 };
    quiet();
    let _ = w.set_size(tauri::LogicalSize::new(width, height));
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    Ok(g)
}

/// Pegada a un borde: `docked_x` medio escondida, `peek_x` asomando entera.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dock {
    right: bool,
    docked_x: i32,
    peek_x: i32,
}

/// A menos de esto del borde al soltarla, se pega (px lógicos).
const SNAP: f64 = 40.0;
pub const DOCK_EVENT: &str = "rocket://dock";

/// ¿Se pega a un borde? `x` es la esquina de la burbuja; `area` el área de trabajo (x, ancho).
pub fn dock_for(x: i32, side: i32, snap: i32, area: (i32, i32)) -> Option<(bool, i32, i32)> {
    let (ax, aw) = area;
    let gap = side / 10;
    if x <= ax + snap {
        Some((false, ax - side / 2, ax + gap))
    } else if x + side >= ax + aw - snap {
        Some((true, ax + aw - side / 2, ax + aw - side - gap))
    } else {
        None
    }
}

/// Tras soltarla (o al abrirla): si quedó cerca de un borde, se pega medio escondida; si no, se despega.
fn dock_check(app: &tauri::AppHandle, w: &tauri::WebviewWindow, save: bool) {
    // Abierta no se recoloca nunca: solo la pestaña escondida se pega al borde.
    if WIDE.load(Ordering::SeqCst) {
        return;
    }
    let Ok(pos) = w.outer_position() else { return };
    if let (Ok(Some(m)), Ok(sz)) = (w.current_monitor(), w.outer_size()) {
        let area = m.work_area();
        let (ax, ay, aw, ah) = (area.position.x, area.position.y, area.size.width as i32, area.size.height as i32);
        let ww = sz.width as i32;
        let right = pos.x + ww / 2 > ax + aw / 2;
        let x = if right { ax + aw - ww } else { ax };
        let y = pos.y.clamp(ay, (ay + ah - sz.height as i32).max(ay));
        // El centro de la pestaña: ahí se abrirá la barra.
        BAR_Y.store(y + sz.height as i32 / 2, Ordering::SeqCst);
        if let Ok(mut d) = DOCK.lock() {
            *d = Some(Dock { right, docked_x: x, peek_x: x });
        }
        if (x, y) != (pos.x, pos.y) {
            quiet();
            let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
        }
        if save {
            save_pos(app, w, Pos { x, y });
        }
        let _ = app.emit(DOCK_EVENT, Some(if right { "right" } else { "left" }));
        return;
    }
    let Ok(Some(m)) = w.current_monitor() else { return };
    let scale = m.scale_factor();
    let side = (size() * scale) as i32;
    let area = m.work_area();
    // La barra vive en un borde: al soltarla se pega al más cercano, a la altura donde se dejó.
    let snap = (area.size.width as i32 - side) / 2;
    let dock = dock_for(pos.x, side, snap.max((SNAP * scale) as i32), (area.position.x, area.size.width as i32)).map(|(right, docked_x, peek_x)| Dock { right, docked_x, peek_x });
    BAR_Y.store(pos.y.clamp(area.position.y, area.position.y + area.size.height as i32 - side), Ordering::SeqCst);
    if let Ok(mut d) = DOCK.lock() {
        *d = dock;
    }
    if let Some(d) = dock {
        if pos.x != d.docked_x {
            quiet();
            let _ = w.set_position(tauri::PhysicalPosition::new(d.docked_x, pos.y));
        }
        if save {
            save_pos(app, w, Pos { x: d.docked_x, y: pos.y });
        }
    } else if save {
        save_pos(app, w, Pos { x: pos.x, y: pos.y });
    }
    let _ = app.emit(DOCK_EVENT, dock.map(|d| if d.right { "right" } else { "left" }));
}

/// Pegada a un borde: asoma entera al acercar el ratón (`out`) o vuelve a esconderse.
#[tauri::command]
pub fn rocket_peek(app: tauri::AppHandle, out: bool) {
    let Some(w) = app.get_webview_window(LABEL) else { return };
    if WIDE.load(Ordering::SeqCst) {
        return;
    }
    let Some(d) = DOCK.lock().ok().and_then(|d| *d) else { return };
    let Ok(pos) = w.outer_position() else { return };
    let x = if out { d.peek_x } else { d.docked_x };
    if pos.x != x {
        quiet();
        let _ = w.set_position(tauri::PhysicalPosition::new(x, pos.y));
    }
}

/// La ventanita se movió: un rato después de soltarla se mira si se pega a un borde y se guarda dónde.
pub fn on_moved(app: &tauri::AppHandle, x: i32, y: i32) {
    if WIDE.load(Ordering::SeqCst) || QUIET.load(Ordering::SeqCst) {
        return;
    }
    let gen = MOVE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut a) = LAST.lock() {
        *a = Some((x, y));
    }
    if SAVER.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut seen = gen;
        loop {
            std::thread::sleep(Duration::from_millis(600));
            let now = MOVE_GEN.load(Ordering::SeqCst);
            if now == seen {
                break;
            }
            seen = now;
        }
        SAVER.store(false, Ordering::SeqCst);
        if let Some(w) = app.get_webview_window(LABEL) {
            dock_check(&app, &w, true);
        }
    });
}

/// Memoria en uso (%), para el anillo de la burbuja.
#[tauri::command]
pub fn rocket_memory() -> u32 {
    let (total, used) = memory();
    if total == 0 {
        0
    } else {
        ((used as f64 / total as f64) * 100.0).round() as u32
    }
}

static LAST: Mutex<Option<(i32, i32)>> = Mutex::new(None);
static DOCK: Mutex<Option<Dock>> = Mutex::new(None);
static QUIET: AtomicBool = AtomicBool::new(false);
static QUIET_GEN: AtomicU64 = AtomicU64::new(0);
static ZOOM_PCT: AtomicU32 = AtomicU32::new(100);
/// Altura (arriba del cohete) donde se dejó la barra en su borde.
static BAR_Y: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);

/// La barra de herramientas: abierta (`width`×`height`, entera dentro de la pantalla, pegada a su borde y
/// con el cohete a la altura donde se dejó, o lo más cerca que quepa) o escondida (medio cohete asomando).
/// Devuelve si está en el borde derecho.
#[tauri::command]
pub fn rocket_bar(app: tauri::AppHandle, open: bool, width: f64, height: f64) -> Result<bool, String> {
    let Some(w) = app.get_webview_window(LABEL) else { return Ok(true) };
    let Ok(Some(m)) = w.current_monitor() else { return Ok(true) };
    let scale = m.scale_factor();
    let s = size();
    let side = (s * scale) as i32;
    let area = m.work_area();
    let (ax, ay, aw, ah) = (area.position.x, area.position.y, area.size.width as i32, area.size.height as i32);
    let pos = w.outer_position().map_err(|e| e.to_string())?;
    let right = DOCK.lock().ok().and_then(|d| *d).map_or(pos.x + side / 2 > ax + aw / 2, |d| d.right);
    // La altura de la pestaña (su centro): la barra se abre ahí, la mueva donde la mueva.
    let mut center = BAR_Y.load(Ordering::SeqCst);
    if center == i32::MIN {
        center = pos.y + w.outer_size().map_or(side, |s| s.height as i32) / 2;
        BAR_Y.store(center, Ordering::SeqCst);
    }
    let (ww, hh) = ((width * scale) as i32, (height * scale) as i32);
    let x = if right { ax + aw - ww } else { ax };
    // Abierta: el cohete (arriba) a la altura de la pestaña, o lo más cerca que quepa toda la barra.
    let y = if open { center - (side / 2) } else { center - hh / 2 }.clamp(ay, (ay + ah - hh).max(ay));
    let _ = s;
    WIDE.store(open, Ordering::SeqCst);
    quiet();
    let _ = w.set_size(tauri::LogicalSize::new(width, height));
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    no_frame(&w);
    if let Ok(mut d) = DOCK.lock() {
        *d = Some(Dock { right, docked_x: x, peek_x: x });
    }
    Ok(right)
}


/// Mientras haya una presentación, un juego o un vídeo a pantalla completa, el cohete se esconde.
static FORCED_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

/// Ctrl+Alt+O o el menú del reloj: con el cohete en el escritorio, se enseña (aunque esté escondido
/// por una pantalla completa) unos segundos, para que se vea despegar. Devuelve si está en el escritorio.
pub fn force_show(app: &tauri::AppHandle) -> bool {
    let Some(w) = app.get_webview_window(LABEL) else { return false };
    if let Ok(mut f) = FORCED_UNTIL.lock() {
        *f = Some(Instant::now() + Duration::from_secs(15));
    }
    HIDDEN_FULLSCREEN.store(false, Ordering::SeqCst);
    let _ = w.show();
    no_frame(&w);
    true
}

fn watch_fullscreen(app: tauri::AppHandle) {
    if WATCHER.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut screens = String::new();
        let mut tick = 0u64;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            tick += 1;
            let Some(w) = app.get_webview_window(LABEL) else { break };
            // ¿Se conectó o quitó la base o un monitor? (La primera vuelta solo apunta las de ahora.)
            let key = screens_key(&w);
            if !key.is_empty() && key != screens {
                if !screens.is_empty() {
                    // Windows tarda un momento en recolocar las ventanas y las áreas de trabajo.
                    std::thread::sleep(Duration::from_millis(800));
                    screens_changed(&app, &w, &screens_key(&w));
                    // Los iconos del Escritorio: el Explorador los recoloca a su manera; cuando termina, se reponen como
                    // estaban con estas pantallas.
                    let (app2, key2) = (app.clone(), screens_key(&w));
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(2500));
                        let n = crate::desktopicons::restore(&app2, &key2);
                        if n > 0 {
                            log::info!("Iconos del Escritorio: {n} repuestos para estas pantallas");
                        }
                    });
                }
                screens = screens_key(&w);
                tick = 0;
            }
            // Cada minuto con las mismas pantallas: dónde están los iconos ahora (para reponerlos al volver a ellas).
            if tick % 30 == 15 && !screens.is_empty() {
                crate::desktopicons::save(&app, &screens);
            }
            // Tras Ctrl+Alt+O se deja a la vista un rato aunque haya una presentación.
            let forced = FORCED_UNTIL.lock().ok().and_then(|f| *f).is_some_and(|t| Instant::now() < t);
            let busy = fullscreen_now() && !forced;
            if busy && !HIDDEN_FULLSCREEN.load(Ordering::SeqCst) && w.is_visible().unwrap_or(false) {
                HIDDEN_FULLSCREEN.store(true, Ordering::SeqCst);
                let _ = w.hide();
            } else if !busy && HIDDEN_FULLSCREEN.swap(false, Ordering::SeqCst) {
                let _ = w.show();
                no_frame(&w);
            }
        }
        WATCHER.store(false, Ordering::SeqCst);
    });
}

#[cfg(windows)]
pub(crate) fn fullscreen_now() -> bool {
    use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
    let mut s = 0;
    // SAFETY: solo escribe en `s`.
    let hr = unsafe { SHQueryUserNotificationState(&mut s) };
    hr >= 0 && matches!(s, QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
}

#[cfg(not(windows))]
pub(crate) fn fullscreen_now() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modos() {
        assert!(valid_mode("desktop") && valid_mode("app") && valid_mode("off"));
        assert!(!valid_mode("") && !valid_mode("Desktop"));
    }

    #[test]
    fn no_entra_en_una_union() {
        let base = std::env::temp_dir().join(format!("adminops-junction-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let victim = base.join("documentos");
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(victim.join("importante.docx"), b"no tocar").unwrap();
        let link = base.join("Temp");
        let made = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&link).arg(&victim).output().map(|o| o.status.success()).unwrap_or(false);
        if made {
            let mut c = Clean::default();
            clean_dir(&link, Duration::ZERO, 0, Instant::now() + Duration::from_secs(5), &mut c);
            assert_eq!(c.files, 0);
            assert!(victim.join("importante.docx").exists());
            let _ = std::fs::remove_dir(&link);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn crece_hacia_donde_hay_sitio() {
        let mon = (0, 0, 1920, 1080);
        assert_eq!(grow_dir(1800, 950, 68, mon), Grow { left: true, up: true });
        assert_eq!(grow_dir(40, 40, 68, mon), Grow { left: false, up: false });
    }

    #[test]
    fn se_pega_al_borde() {
        // Área de trabajo de 0 a 1920; burbuja de 68 px; a menos de 40 px se pega.
        assert_eq!(dock_for(1830, 68, 40, (0, 1920)), Some((true, 1886, 1846)));
        assert_eq!(dock_for(20, 68, 40, (0, 1920)), Some((false, -34, 6)));
        assert_eq!(dock_for(900, 68, 40, (0, 1920)), None);
        // Segundo monitor a la derecha.
        assert_eq!(dock_for(1930, 68, 40, (1920, 1920)), Some((false, 1886, 1926)));
    }

    #[test]
    fn nombres_que_se_reconocen() {
        assert_eq!(app_label("chrome.exe"), "Chrome");
        assert_eq!(app_label("ms-teams.exe"), "Teams");
        assert_eq!(app_label("contaplus.exe"), "Contaplus");
        let mb = 1_048_576;
        let top = top_apps(&[("chrome.exe".into(), 300 * mb), ("chrome.exe".into(), 340 * mb), ("notepad.exe".into(), mb), ("excel.exe".into(), 150 * mb)], 5);
        assert_eq!(top, vec![Freed { name: "Chrome".into(), bytes: 640 * mb }, Freed { name: "Excel".into(), bytes: 150 * mb }]);
    }

    #[test]
    fn no_toca_el_sistema() {
        assert!(protected("dwm.exe") && protected("AudioDG.exe") && protected("System"));
        assert!(!protected("chrome.exe") && !protected("Teams.exe"));
    }

    #[test]
    fn borra_solo_lo_viejo_y_no_sigue_enlaces() {
        let dir = std::env::temp_dir().join(format!("adminops-rocket-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("nuevo.tmp"), b"hola").unwrap();
        std::fs::write(dir.join("sub").join("viejo.tmp"), b"12345").unwrap();
        // Edad 0: todo cuenta como viejo; se borran los archivos y la subcarpeta vacía.
        let mut c = Clean::default();
        clean_dir(&dir, Duration::ZERO, 0, Instant::now() + Duration::from_secs(5), &mut c);
        assert_eq!(c.files, 2);
        assert_eq!(c.bytes, 9);
        assert!(!dir.join("sub").exists());
        // Con una edad de un día, lo recién creado se queda.
        std::fs::write(dir.join("hoy.tmp"), b"x").unwrap();
        let mut c = Clean::default();
        clean_dir(&dir, Duration::from_secs(86_400), 0, Instant::now() + Duration::from_secs(5), &mut c);
        assert_eq!(c.files, 0);
        assert!(dir.join("hoy.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
