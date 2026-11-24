//! Uso de la red (Red → Uso de la red): qué programa sube y baja cuánto, en vivo.
//!
//! Se escucha el proveedor `Microsoft-Windows-Kernel-Network` de ETW con una sesión en tiempo real
//! llamada «AdminOps-UsoRed»: cada envío y cada recepción de TCP y UDP (UDP importa por QUIC y
//! Teams) llega con el número del proceso, los bytes y la dirección remota. Cada segundo se agrupa
//! por programa (nombre amigable, ruta para el icono), se calcula la media móvil de 3 s en Mbit/s,
//! los 3 destinos con más tráfico y los bytes totales. Si ETW no se puede (otro fallo que no sea
//! falta de permiso), se cae a `GetPerTcpConnectionEStats`, solo TCP y solo con administrador, y la
//! pantalla lo avisa. Sin permiso para ninguna de las dos, la pantalla ofrece reabrir con administrador.
//!
//! La sesión corre solo mientras alguien la necesita: la pantalla (que debe seguir preguntando, si no
//! se suelta sola a los 15 s) o el vigilante de la conexión. Se para al salir de la pantalla, al cerrar
//! AdminOps por cualquier camino (`shutdown`) y, si AdminOps murió sin poder hacerlo, la siguiente vez
//! que se abre (`start` para la sesión que quedó con ese nombre).
//!
//! El total del día es lo que AdminOps midió mientras miraba, guardado por día en los datos de este equipo.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Nombre fijo de la sesión de ETW.
pub const SESSION_NAME: &str = "AdminOps-UsoRed";
/// Segundos de la media móvil.
pub const WINDOW: usize = 3;
/// Un programa con tráfico en los últimos tantos segundos va arriba.
pub const ACTIVE_SECS: u64 = 10;
/// Si la pantalla deja de preguntar durante tanto, la sesión se suelta sola.
const LEASE: Duration = Duration::from_secs(15);
/// Días que se guardan.
const KEEP_DAYS: usize = 14;
/// Destinos que se recuerdan por programa y por proceso.
const MAX_DESTS: usize = 128;
const MAX_PIDS: usize = 4096;

// ---------- Eventos de Kernel-Network (puro, con pruebas) ----------

/// Un envío o una recepción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetEvent {
    pub pid: u32,
    pub bytes: u32,
    /// Direcciones tal como las trae el evento: «de `src` a `dst`», el sentido en que va el paquete.
    pub dst: IpAddr,
    pub src: IpAddr,
    /// Subida (envío) o bajada (recepción).
    pub upload: bool,
    pub udp: bool,
}

fn v4(b: &[u8]) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(b[0], b[1], b[2], b[3]))
}

fn v6(b: &[u8]) -> IpAddr {
    let mut o = [0u8; 16];
    o.copy_from_slice(&b[..16]);
    IpAddr::V6(Ipv6Addr::from(o))
}

/// Lee un evento de datos de `Microsoft-Windows-Kernel-Network`. Los números de evento son:
/// TCP IPv4 enviado 10, recibido 11; TCP IPv6 26 y 27; UDP IPv4 42 y 43; UDP IPv6 58 y 59.
/// En todos: PID (4 bytes), tamaño (4), dirección de destino y de origen y los puertos (comprobado
/// con la plantilla del manifiesto de Windows, `Get-WinEvent -ListProvider`).
pub fn parse_event(id: u16, data: &[u8]) -> Option<NetEvent> {
    let (upload, udp, ipv6) = match id {
        10 => (true, false, false),
        11 => (false, false, false),
        26 => (true, false, true),
        27 => (false, false, true),
        42 => (true, true, false),
        43 => (false, true, false),
        58 => (true, true, true),
        59 => (false, true, true),
        _ => return None,
    };
    let need = if ipv6 { 8 + 16 + 16 + 4 } else { 8 + 4 + 4 + 4 };
    if data.len() < need {
        return None;
    }
    let pid = u32::from_le_bytes(data[0..4].try_into().ok()?);
    let bytes = u32::from_le_bytes(data[4..8].try_into().ok()?);
    let (dst, src) = if ipv6 { (v6(&data[8..24]), v6(&data[24..40])) } else { (v4(&data[8..12]), v4(&data[12..16])) };
    Some(NetEvent { pid, bytes, dst, src, upload, udp })
}

impl NetEvent {
    /// La dirección del otro extremo. Según el manifiesto del proveedor, el evento dice «de origen a destino»:
    /// en un envío el otro extremo es el destino y en una recepción es el origen. Si esa resulta ser una
    /// dirección de este equipo y la otra no, se cambia (por si Windows rellena los campos al revés).
    pub fn remote(&self, locals: &HashSet<IpAddr>) -> IpAddr {
        let (first, other) = if self.upload { (self.dst, self.src) } else { (self.src, self.dst) };
        if locals.contains(&first) && !locals.contains(&other) {
            other
        } else {
            first
        }
    }
}

/// El tráfico de dentro del propio equipo no es «red».
pub fn is_local_only(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => a.is_loopback() || a.is_unspecified(),
        IpAddr::V6(a) => a.is_loopback() || a.is_unspecified(),
    }
}

// ---------- Nombres de programa (puro, con pruebas) ----------

/// «Google Chrome» → «Chrome»; «Microsoft OneDrive» → «OneDrive». Sin descripción, el nombre del archivo.
pub fn friendly_name(description: Option<&str>, file: &str) -> String {
    let stem = file.rsplit(['\\', '/']).next().unwrap_or(file);
    let stem = stem.strip_suffix(".exe").or_else(|| stem.strip_suffix(".EXE")).unwrap_or(stem);
    let mut d = description.map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
    for prefix in ["Microsoft ", "Google ", "Mozilla "] {
        if let Some(rest) = d.strip_prefix(prefix) {
            if !rest.is_empty() {
                d = rest.to_string();
            }
            break;
        }
    }
    if d.is_empty() || d.chars().count() > 32 {
        // Sin descripción útil: el nombre del archivo con la primera en mayúscula.
        let mut c = stem.chars();
        return match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => "Programa".to_string(),
        };
    }
    d
}

/// Los servicios que lleva un `svchost`: si alguno es de Windows Update o de la Optimización de
/// distribución, se cuenta como «Windows Update»; si no, «Servicios de Windows».
pub fn service_group(services: &[String]) -> (&'static str, &'static str) {
    if services.iter().any(|s| s.eq_ignore_ascii_case("wuauserv") || s.eq_ignore_ascii_case("DoSvc")) {
        ("svchost:wu", "Windows Update")
    } else {
        ("svchost", "Servicios de Windows")
    }
}

// ---------- Conteo por programa (puro, con pruebas) ----------

/// Mbit/s de una ventana de bytes por segundo (media de las muestras que hay).
pub fn mbps(window: &VecDeque<u64>) -> f64 {
    if window.is_empty() {
        return 0.0;
    }
    let sum: u64 = window.iter().sum();
    (sum as f64) * 8.0 / 1_000_000.0 / (window.len() as f64)
}

fn push_window(w: &mut VecDeque<u64>, v: u64) {
    w.push_back(v);
    while w.len() > WINDOW {
        w.pop_front();
    }
}

/// De número de proceso a programa con su nombre amigable (también lo usa «Grabar el fallo»).
pub(crate) use win::Idents;

/// Quién es un programa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// Identificador estable (el nombre del archivo en minúsculas, o `svchost:wu`).
    pub key: String,
    pub name: String,
    pub path: Option<String>,
}

/// Lo que un programa movió en un segundo.
#[derive(Debug, Clone, Default)]
pub struct Tick {
    pub up: u64,
    pub down: u64,
    pub dests: Vec<(IpAddr, u64)>,
}

#[derive(Default)]
struct Prog {
    name: String,
    path: Option<String>,
    up_win: VecDeque<u64>,
    down_win: VecDeque<u64>,
    total_up: u64,
    total_down: u64,
    dests: HashMap<IpAddr, u64>,
    /// Milisegundos desde 1970 del último tráfico (0: ninguno).
    last_traffic_ms: u64,
    first_ms: u64,
}

/// Lo medido desde que se empezó a mirar.
#[derive(Default)]
pub struct Live {
    pub started_ms: u64,
    progs: HashMap<String, Prog>,
}

impl Live {
    pub fn new(now_ms: u64) -> Self {
        Live { started_ms: now_ms, progs: HashMap::new() }
    }

    /// Un segundo más: lo que movió cada programa (los que no salen, mueven 0).
    /// `dt_ms` es el tiempo real que pasó desde el latido anterior: los Mbit/s se dividen por él y no por 1 s fijo.
    pub fn apply(&mut self, now_ms: u64, dt_ms: u64, ticks: Vec<(Ident, Tick)>) {
        let dt = dt_ms.max(1);
        let per_sec = |bytes: u64| bytes.saturating_mul(1000) / dt;
        let mut seen = HashSet::new();
        for (id, t) in ticks {
            seen.insert(id.key.clone());
            let p = self.progs.entry(id.key).or_default();
            if p.first_ms == 0 {
                p.first_ms = now_ms;
            }
            p.name = id.name;
            if id.path.is_some() {
                p.path = id.path;
            }
            push_window(&mut p.up_win, per_sec(t.up));
            push_window(&mut p.down_win, per_sec(t.down));
            p.total_up += t.up;
            p.total_down += t.down;
            if t.up + t.down > 0 {
                p.last_traffic_ms = now_ms;
            }
            for (ip, b) in t.dests {
                if let Some(v) = p.dests.get_mut(&ip) {
                    *v += b;
                } else if p.dests.len() < MAX_DESTS {
                    p.dests.insert(ip, b);
                } else if let Some((&small, &sb)) = p.dests.iter().min_by_key(|(_, b)| **b) {
                    // Lleno: el más pequeño deja su sitio al nuevo si este ya lo supera.
                    if b > sb {
                        p.dests.remove(&small);
                        p.dests.insert(ip, b);
                    }
                }
            }
        }
        for (k, p) in self.progs.iter_mut() {
            if !seen.contains(k) {
                push_window(&mut p.up_win, 0);
                push_window(&mut p.down_win, 0);
            }
        }
    }
}

// ---------- Lo que se enseña ----------

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    /// El nombre (DNS inverso) o, si no hay, la IP.
    pub host: String,
    pub ip: String,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgramUsage {
    pub key: String,
    pub name: String,
    /// Ruta del programa, para pedir su icono.
    pub path: Option<String>,
    pub up_mbps: f64,
    pub down_mbps: f64,
    /// Desde que se empezó a mirar.
    pub total_up: u64,
    pub total_down: u64,
    /// Hoy, mientras AdminOps miraba.
    pub today_up: u64,
    pub today_down: u64,
    /// Segundos que se lleva viendo este programa.
    pub watched_secs: u64,
    pub destinations: Vec<Destination>,
    /// Con tráfico en los últimos 10 s.
    pub active: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageError {
    /// `needsAdmin` o `failed`.
    pub kind: String,
    pub message: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OneDrivePause {
    pub until_ms: u64,
    pub entry_id: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub running: bool,
    /// `etw` (TCP y UDP), `estats` (solo TCP) o `none`.
    pub method: String,
    pub tcp_only: bool,
    pub error: Option<UsageError>,
    pub programs: Vec<ProgramUsage>,
    pub total_up_mbps: f64,
    pub total_down_mbps: f64,
    pub started_ms: u64,
    pub now_ms: u64,
    pub onedrive_pause: Option<OneDrivePause>,
    /// Eventos que Windows no pudo entregar (si hay, las cifras se quedan cortas).
    pub lost_events: u64,
}

/// Los 3 destinos con más tráfico.
pub fn top_destinations(dests: &HashMap<IpAddr, u64>, name_of: &dyn Fn(&IpAddr) -> Option<String>) -> Vec<Destination> {
    let mut v: Vec<(&IpAddr, &u64)> = dests.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    v.into_iter()
        .take(3)
        .map(|(ip, b)| Destination { host: name_of(ip).unwrap_or_else(|| ip.to_string()), ip: ip.to_string(), bytes: *b })
        .collect()
}

impl Live {
    fn programs(&self, now_ms: u64, today: &BTreeMap<String, DayEntry>, name_of: &dyn Fn(&IpAddr) -> Option<String>) -> Vec<ProgramUsage> {
        let mut out: Vec<ProgramUsage> = self
            .progs
            .iter()
            .map(|(key, p)| {
                let day = today.get(key);
                ProgramUsage {
                    key: key.clone(),
                    name: p.name.clone(),
                    path: p.path.clone(),
                    up_mbps: mbps(&p.up_win),
                    down_mbps: mbps(&p.down_win),
                    total_up: p.total_up,
                    total_down: p.total_down,
                    today_up: day.map_or(p.total_up, |d| d.up),
                    today_down: day.map_or(p.total_down, |d| d.down),
                    watched_secs: now_ms.saturating_sub(p.first_ms) / 1000,
                    destinations: top_destinations(&p.dests, name_of),
                    active: p.last_traffic_ms > 0 && now_ms.saturating_sub(p.last_traffic_ms) <= ACTIVE_SECS * 1000,
                }
            })
            .collect();
        // Lo que hoy movió algo pero ahora no está (otra vez abierta AdminOps, o ya sin tráfico en esta sesión).
        for (key, d) in today {
            if !self.progs.contains_key(key) {
                out.push(ProgramUsage {
                    key: key.clone(),
                    name: d.name.clone(),
                    path: d.path.clone(),
                    up_mbps: 0.0,
                    down_mbps: 0.0,
                    total_up: 0,
                    total_down: 0,
                    today_up: d.up,
                    today_down: d.down,
                    watched_secs: 0,
                    destinations: Vec::new(),
                    active: false,
                });
            }
        }
        out.sort_by(|a, b| {
            let (x, y) = (a.up_mbps + a.down_mbps, b.up_mbps + b.down_mbps);
            y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal).then_with(|| (b.today_up + b.today_down).cmp(&(a.today_up + a.today_down))).then_with(|| a.name.cmp(&b.name))
        });
        out
    }
}

// ---------- Total del día (puro, con pruebas) ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DayEntry {
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
    pub up: u64,
    pub down: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DayFile {
    pub days: BTreeMap<String, BTreeMap<String, DayEntry>>,
}

impl DayFile {
    pub fn add(&mut self, date: &str, id: &Ident, up: u64, down: u64) {
        if up == 0 && down == 0 {
            return;
        }
        let e = self.days.entry(date.to_string()).or_default().entry(id.key.clone()).or_default();
        e.name = id.name.clone();
        if id.path.is_some() {
            e.path = id.path.clone();
        }
        e.up += up;
        e.down += down;
        while self.days.len() > KEEP_DAYS {
            let Some(first) = self.days.keys().next().cloned() else { break };
            self.days.remove(&first);
        }
    }

    pub fn today(&self, date: &str) -> BTreeMap<String, DayEntry> {
        self.days.get(date).cloned().unwrap_or_default()
    }
}

fn today_key() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

// ---------- Estado compartido ----------

/// Lo que llega de ETW entre dos latidos: por proceso.
#[derive(Default)]
struct PidAcc {
    up: u64,
    down: u64,
    dests: HashMap<IpAddr, u64>,
}

static ACC: Mutex<Option<HashMap<u32, PidAcc>>> = Mutex::new(None);

/// Eventos de otros proveedores que ha visto la sesión de ensayo (solo pruebas).
#[cfg(test)]
static TEST_EVENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Las direcciones de este equipo (se renuevan cada 30 s).
static LOCALS: Mutex<Option<HashSet<IpAddr>>> = Mutex::new(None);

fn refresh_locals() {
    let nets = sysinfo::Networks::new_with_refreshed_list();
    let set: HashSet<IpAddr> = nets.iter().flat_map(|(_, n)| n.ip_networks().iter().map(|i| i.addr)).collect();
    *LOCALS.lock().unwrap_or_else(|e| e.into_inner()) = Some(set);
}

fn push_event(ev: &NetEvent) {
    let remote = {
        let g = LOCALS.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_ref() {
            Some(l) => ev.remote(l),
            None => ev.remote(&HashSet::new()),
        }
    };
    if is_local_only(&remote) {
        return;
    }
    let mut g = ACC.lock().unwrap_or_else(|e| e.into_inner());
    let map = g.get_or_insert_with(HashMap::new);
    if !map.contains_key(&ev.pid) && map.len() >= MAX_PIDS {
        return;
    }
    let a = map.entry(ev.pid).or_default();
    let b = u64::from(ev.bytes);
    if ev.upload {
        a.up += b;
    } else {
        a.down += b;
    }
    if let Some(v) = a.dests.get_mut(&remote) {
        *v += b;
    } else if a.dests.len() < MAX_DESTS {
        a.dests.insert(remote, b);
    }
}

fn take_acc() -> HashMap<u32, PidAcc> {
    ACC.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default()
}

struct Shared {
    live: Live,
    method: String,
    tcp_only: bool,
    running: bool,
    error: Option<UsageError>,
    days: DayFile,
    days_loaded: bool,
    days_dirty: bool,
    lost: u64,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

fn shared() -> std::sync::MutexGuard<'static, Option<Shared>> {
    SHARED.lock().unwrap_or_else(|e| e.into_inner())
}

fn days_file() -> Option<std::path::PathBuf> {
    APP.get().map(|app| crate::paths::machine_data_dir(app).join("netusage.json"))
}

fn load_days() -> DayFile {
    days_file().map(|p| crate::paths::read_json::<DayFile>(&p)).unwrap_or_default()
}

fn save_days(days: &DayFile) {
    if let Some(p) = days_file() {
        if let Err(e) = crate::paths::write_json(&p, days) {
            log::warn!("Uso de la red: no se pudo guardar el total del día: {e}");
        }
    }
}

// ---------- Control: quién necesita la sesión ----------

struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[derive(Default)]
struct Control {
    /// Quién la necesita y hasta cuándo (la pantalla debe seguir preguntando; el vigilante no caduca).
    users: HashMap<String, Option<Instant>>,
    running: Option<Running>,
}

static CTL: Mutex<Option<Control>> = Mutex::new(None);

fn ctl() -> std::sync::MutexGuard<'static, Option<Control>> {
    CTL.lock().unwrap_or_else(|e| e.into_inner())
}

/// Alguien necesita medir (la pantalla con `lease`, el vigilante sin él). Arranca la sesión si no está.
pub fn hold(user: &str, lease: bool) {
    // Si la anterior se está parando, se espera a que acabe (el nombre de la sesión es único).
    for _ in 0..60 {
        let stopping = ctl().as_ref().and_then(|c| c.running.as_ref()).is_some_and(|r| r.stop.load(Ordering::SeqCst));
        if !stopping {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut g = ctl();
    let c = g.get_or_insert_with(Control::default);
    c.users.insert(user.to_string(), lease.then(|| Instant::now() + LEASE));
    if c.running.is_none() {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new().name("netusage".into()).spawn(move || run(flag, false)).ok();
        c.running = Some(Running { stop, thread });
    }
}

/// Esa persona ya no necesita medir. Si no queda nadie, la sesión se para (sin esperar).
pub fn release(user: &str) {
    let mut g = ctl();
    let Some(c) = g.as_mut() else { return };
    c.users.remove(user);
    if c.users.is_empty() {
        if let Some(r) = &c.running {
            r.stop.store(true, Ordering::SeqCst);
        }
    }
}

/// Renueva el plazo de la pantalla (cada vez que pregunta).
fn renew(user: &str) {
    if let Some(c) = ctl().as_mut() {
        if let Some(l) = c.users.get_mut(user) {
            if l.is_some() {
                *l = Some(Instant::now() + LEASE);
            }
        }
    }
}

/// ¿Sigue haciendo falta? Quita a quien dejó de preguntar.
fn still_needed() -> bool {
    let mut g = ctl();
    let Some(c) = g.as_mut() else { return false };
    let now = Instant::now();
    c.users.retain(|_, until| until.is_none_or(|u| u > now));
    !c.users.is_empty()
}

/// Para todo y espera a que la sesión de ETW esté cerrada. Al cerrar AdminOps por cualquier camino.
pub fn shutdown() {
    let handle = {
        let mut g = ctl();
        let Some(c) = g.as_mut() else { return };
        c.users.clear();
        c.running.as_mut().and_then(|r| {
            r.stop.store(true, Ordering::SeqCst);
            r.thread.take()
        })
    };
    if let Some(h) = handle {
        let t0 = Instant::now();
        while !h.is_finished() && t0.elapsed() < Duration::from_secs(4) {
            std::thread::sleep(Duration::from_millis(25));
        }
        if h.is_finished() {
            let _ = h.join();
        }
    }
    // Por si el hilo no llegó a tiempo: la sesión no se queda abierta.
    win::stop_session_by_name();
}

/// Guarda la app y para la sesión que quedó de una ejecución anterior que murió sin cerrarla.
pub fn start(app: tauri::AppHandle) {
    let _ = APP.set(app);
    // Síncrono: es una llamada rápida y así no puede cruzarse con una sesión nueva (que además, si encuentra una vieja,
    // la para ella misma antes de `StartTrace`).
    if win::stop_session_by_name() {
        log::info!("Uso de la red: se paró la sesión «{SESSION_NAME}» que quedó de otra ejecución");
    }
}

// ---------- El hilo que mide ----------

/// Cuánto se espera antes de volver a medir tras un pánico del hilo.
const PANIC_RETRY_WAIT: Duration = Duration::from_secs(5);

/// El hilo de medir. Si hace pánico, igual se deja todo cerrado y `CTL.running` vuelve a quedar libre.
/// `retry` es que este hilo ya es el reintento tras un pánico: si vuelve a pasar, no se insiste.
fn run(stop: Arc<AtomicBool>, retry: bool) {
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_inner(&stop))).is_err();
    if panicked {
        log::error!("Uso de la red: el hilo de medida hizo pánico; se cierra la sesión");
    }
    finish(&stop, panicked, retry);
}

fn run_inner(stop: &Arc<AtomicBool>) {
    let app_ok = APP.get().is_some();
    {
        let mut g = shared();
        let days = match g.as_mut() {
            Some(s) if s.days_loaded => std::mem::take(&mut s.days),
            _ => {
                if app_ok {
                    load_days()
                } else {
                    DayFile::default()
                }
            }
        };
        *g = Some(Shared { live: Live::new(now_ms()), method: "none".into(), tcp_only: false, running: false, error: None, days, days_loaded: true, days_dirty: false, lost: 0 });
    }
    let _ = take_acc();
    rdns::begin();
    refresh_locals();
    let mut locals_at = Instant::now();
    // Primero ETW; si falla y se puede, el plan B.
    let mut session = None;
    let mut estats = None;
    match win::EtwSession::start() {
        Ok(s) => {
            session = Some(s);
            set_method("etw", false, None);
        }
        Err(win::StartError::Denied(step)) => {
            log::info!("Uso de la red: ETW sin permiso en {step}");
            // Sin permiso, el plan B tampoco puede (pide administrador).
            if crate::elevation::is_elevated() {
                estats = win::Estats::new().ok();
            }
            match estats {
                Some(_) => set_method("estats", true, None),
                None => set_method("none", false, Some(UsageError { kind: "needsAdmin".into(), message: "Medir el uso de la red necesita administrador.".into() })),
            }
        }
        Err(win::StartError::Other(m)) => {
            log::warn!("Uso de la red: ETW no arrancó ({m}); se prueba el plan B");
            estats = if crate::elevation::is_elevated() { win::Estats::new().ok() } else { None };
            match estats {
                Some(_) => set_method("estats", true, None),
                None => set_method("none", false, Some(UsageError { kind: "failed".into(), message: format!("No se pudo medir el tráfico de la red: {m}") })),
            }
        }
    }
    let measuring = session.is_some() || estats.is_some();
    let mut idents = win::Idents::new();
    let mut last_save = Instant::now();
    let mut last_tick = Instant::now();
    let mut next = Instant::now() + Duration::from_secs(1);
    let mut ticks_seen = 0u32;
    while !stop.load(Ordering::SeqCst) {
        // Duerme a trozos para parar enseguida.
        while Instant::now() < next && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(40));
        }
        if stop.load(Ordering::SeqCst) {
            break;
        }
        next += Duration::from_secs(1);
        if next < Instant::now() {
            next = Instant::now() + Duration::from_secs(1);
        }
        if !still_needed() {
            break;
        }
        if !measuring {
            continue;
        }
        if locals_at.elapsed() >= Duration::from_secs(30) {
            refresh_locals();
            locals_at = Instant::now();
        }
        if let Some(e) = estats.as_mut() {
            e.poll();
        }
        // Cada 5 s se pregunta a ETW cuántos eventos no pudo entregar.
        ticks_seen += 1;
        if ticks_seen.is_multiple_of(5) {
            if let Some(l) = session.as_ref().and_then(|x| x.lost()) {
                if let Some(sh) = shared().as_mut() {
                    sh.lost = l;
                }
            }
        }
        let acc = take_acc();
        // De procesos a programas.
        let mut by_prog: HashMap<String, (Ident, Tick)> = HashMap::new();
        let mut dests: HashMap<String, HashMap<IpAddr, u64>> = HashMap::new();
        for (pid, a) in acc {
            let id = idents.identify(pid);
            let slot = by_prog.entry(id.key.clone()).or_insert_with(|| (id.clone(), Tick::default()));
            slot.1.up += a.up;
            slot.1.down += a.down;
            let d = dests.entry(id.key).or_default();
            for (ip, b) in a.dests {
                *d.entry(ip).or_insert(0) += b;
            }
        }
        let date = today_key();
        let now = now_ms();
        let dt_ms = last_tick.elapsed().as_millis().clamp(1, 60_000) as u64;
        last_tick = Instant::now();
        let mut ticks = Vec::with_capacity(by_prog.len());
        for (key, (id, mut t)) in by_prog {
            t.dests = dests.remove(&key).map(|m| m.into_iter().collect()).unwrap_or_default();
            ticks.push((id, t));
        }
        // Los nombres de los destinos que aún no se conocen se piden sin esperar.
        for (_, t) in &ticks {
            for (ip, _) in &t.dests {
                rdns::request(ip);
            }
        }
        let mut g = shared();
        if let Some(s) = g.as_mut() {
            for (id, t) in &ticks {
                s.days.add(&date, id, t.up, t.down);
                if t.up + t.down > 0 {
                    s.days_dirty = true;
                }
            }
            s.live.apply(now, dt_ms, ticks);
            if s.days_dirty && last_save.elapsed() >= Duration::from_secs(30) {
                save_days(&s.days);
                s.days_dirty = false;
                last_save = Instant::now();
            }
        }
    }
    // Cierre: se para la sesión de ETW (y se apaga lo que el plan B encendió).
    drop(session);
    drop(estats);
}

/// Se guarda lo del día y se deja todo como estaba (también tras un pánico). Tras un pánico, un solo reintento pasados unos
/// segundos; si el reintento también hace pánico se para y la pantalla muestra el error (nunca un bucle de reinicios).
fn finish(stop: &Arc<AtomicBool>, panicked: bool, retry: bool) {
    {
        let mut g = shared();
        if let Some(s) = g.as_mut() {
            if s.days_dirty {
                save_days(&s.days);
                s.days_dirty = false;
            }
            s.running = false;
            if panicked && retry {
                s.method = "none".into();
                s.error = Some(UsageError { kind: "failed".into(), message: "La medición del uso de la red se detuvo por un fallo interno. Vuelve a abrir esta pantalla para intentarlo otra vez.".into() });
            }
        }
    }
    rdns::stop();
    let mut g = ctl();
    if let Some(c) = g.as_mut() {
        c.running = None;
        // Alguien pidió medir justo mientras esto se cerraba (o hubo un pánico y toca el único reintento): se empieza otra vez.
        let give_up = panicked && retry;
        let again = !c.users.is_empty() && !stop.load(Ordering::SeqCst) && !give_up;
        if again {
            let new_stop = Arc::new(AtomicBool::new(false));
            let flag = new_stop.clone();
            let thread = std::thread::Builder::new()
                .name("netusage".into())
                .spawn(move || {
                    if panicked {
                        // Se espera a trozos para poder parar enseguida.
                        let t0 = Instant::now();
                        while t0.elapsed() < PANIC_RETRY_WAIT && !flag.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(40));
                        }
                        if flag.load(Ordering::SeqCst) {
                            finish(&flag, false, true);
                            return;
                        }
                    }
                    run(flag, panicked)
                })
                .ok();
            c.running = Some(Running { stop: new_stop, thread });
        }
    }
}

fn set_method(method: &str, tcp_only: bool, error: Option<UsageError>) {
    if let Some(s) = shared().as_mut() {
        s.method = method.to_string();
        s.tcp_only = tcp_only;
        s.running = error.is_none();
        s.error = error;
    }
}

// ---------- DNS inverso con caché y tiempo límite ----------

mod rdns {
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::net::IpAddr;
    use std::sync::Mutex;
    use std::time::Duration;

    const MAX_CACHE: usize = 2000;
    const MAX_QUEUE: usize = 64;
    const LIMIT: Duration = Duration::from_millis(1500);
    /// El hilo se va solo si pasa este rato sin nada que resolver.
    const IDLE: Duration = Duration::from_secs(20);

    #[derive(Default)]
    struct State {
        cache: HashMap<IpAddr, Option<String>>,
        asked: HashSet<IpAddr>,
        queue: VecDeque<IpAddr>,
        /// Hay un hilo trabajando.
        worker: bool,
        /// Hay que parar (se acabó la medición).
        stop: bool,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    fn lock() -> std::sync::MutexGuard<'static, Option<State>> {
        STATE.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// El nombre ya sabido (nada si falta o no tiene).
    pub fn cached(ip: &IpAddr) -> Option<String> {
        lock().as_ref().and_then(|s| s.cache.get(ip).cloned().flatten())
    }

    /// Una medición nueva empieza: lo pendiente de antes ya no vale.
    pub fn begin() {
        let mut g = lock();
        let s = g.get_or_insert_with(State::default);
        s.stop = false;
    }

    /// Pide un nombre sin esperarlo.
    pub fn request(ip: &IpAddr) {
        let spawn = {
            let mut g = lock();
            let s = g.get_or_insert_with(State::default);
            if s.stop || s.cache.contains_key(ip) || s.asked.contains(ip) || s.queue.len() >= MAX_QUEUE {
                return;
            }
            s.asked.insert(*ip);
            s.queue.push_back(*ip);
            !std::mem::replace(&mut s.worker, true)
        };
        if spawn && std::thread::Builder::new().name("netusage-dns".into()).spawn(work).is_err() {
            if let Some(s) = lock().as_mut() {
                s.worker = false;
            }
        }
    }

    /// Se acabó la medición: el hilo para y lo que estaba en cola se olvida.
    pub fn stop() {
        if let Some(s) = lock().as_mut() {
            s.stop = true;
            s.queue.clear();
            s.asked.clear();
        }
    }

    fn work() {
        let mut idle = Duration::ZERO;
        loop {
            let ip = {
                let mut g = lock();
                let Some(s) = g.as_mut() else { return };
                if s.stop || idle >= IDLE {
                    s.worker = false;
                    return;
                }
                s.queue.pop_front()
            };
            let Some(ip) = ip else {
                std::thread::sleep(Duration::from_millis(100));
                idle += Duration::from_millis(100);
                continue;
            };
            idle = Duration::ZERO;
            // Cada consulta en su hilo, para poder dejar de esperarla.
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(super::win::reverse_lookup(&ip));
            });
            let name = rx.recv_timeout(LIMIT).ok().flatten().filter(|n| n.parse::<IpAddr>().is_err());
            let mut g = lock();
            if let Some(s) = g.as_mut() {
                if s.cache.len() >= MAX_CACHE {
                    s.cache.clear();
                    s.asked.clear();
                }
                s.cache.insert(ip, name);
            }
        }
    }
}

// ---------- Comandos ----------

fn snapshot_now() -> UsageSnapshot {
    let now = now_ms();
    let pause = crate::onedrive::pause_info();
    let mut g = shared();
    let Some(s) = g.as_mut() else {
        return UsageSnapshot { method: "none".into(), now_ms: now, onedrive_pause: pause, ..Default::default() };
    };
    let date = today_key();
    let today = s.days.today(&date);
    let name_of = |ip: &IpAddr| rdns::cached(ip);
    let programs = s.live.programs(now, &today, &name_of);
    let total_up_mbps = programs.iter().fold(0.0, |a, p| a + p.up_mbps);
    let total_down_mbps = programs.iter().fold(0.0, |a, p| a + p.down_mbps);
    UsageSnapshot {
        running: s.running,
        method: s.method.clone(),
        tcp_only: s.tcp_only,
        error: s.error.clone(),
        programs,
        total_up_mbps,
        total_down_mbps,
        started_ms: s.live.started_ms,
        now_ms: now,
        onedrive_pause: pause,
        lost_events: s.lost,
    }
}

/// La pantalla empieza a medir.
#[tauri::command(async)]
pub fn netusage_start() -> UsageSnapshot {
    hold("page", true);
    // Un momento para que la sesión diga si arrancó o no.
    for _ in 0..25 {
        let ready = shared().as_ref().is_some_and(|s| s.method != "none" || s.error.is_some());
        if ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    snapshot_now()
}

/// Lo medido hasta ahora (cada vez que se pregunta se renueva el plazo de la pantalla).
#[tauri::command(async)]
pub fn netusage_snapshot() -> UsageSnapshot {
    renew("page");
    snapshot_now()
}

/// La pantalla ya no mide.
#[tauri::command]
pub fn netusage_stop() {
    release("page");
}

/// El icono de un programa (PNG en base 64) si la ruta es de uno que se ha visto.
#[tauri::command(async)]
pub fn netusage_icon(path: String) -> Option<String> {
    win::icon_data_url(&path)
}

/// Para el vigilante de la conexión: mide mientras corre.
pub fn watch_started() {
    hold("watch", false);
}

pub fn watch_stopped() {
    release("watch");
}

// ---------- PNG mínimo (para los iconos) ----------

/// CRC-32 de un trozo de PNG.
pub fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// PNG RGBA sin comprimir (bloques «stored» de zlib) de una imagen pequeña. `None` si no cuadran los tamaños.
pub fn png_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > 256 || height > 256 || rgba.len() != (width * height * 4) as usize {
        return None;
    }
    let mut raw = Vec::with_capacity(rgba.len() + height as usize);
    for row in rgba.chunks((width * 4) as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    while let Some(b) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    Some(out)
}

// ---------- Windows ----------

#[cfg(windows)]
mod win {
    use super::{Ident, NetEvent};
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::net::IpAddr;
    use std::ptr::{null, null_mut};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA};
    use windows_sys::Win32::System::Diagnostics::Etw::*;

    /// `Microsoft-Windows-Kernel-Network`.
    const KERNEL_NETWORK: GUID = GUID { data1: 0x7DD4_2A49, data2: 0x5329, data3: 0x4832, data4: [0x8D, 0xFD, 0x43, 0xD9, 0x79, 0x15, 0x3A, 0x88] };
    /// Palabras clave: IPv4 (0x10) e IPv6 (0x20).
    const KEYWORDS: u64 = 0x30;
    const INVALID_TRACE: u64 = u64::MAX;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    pub enum StartError {
        /// Sin permiso, y en qué paso.
        Denied(&'static str),
        Other(String),
    }

    /// Memoria para `EVENT_TRACE_PROPERTIES` más el nombre de la sesión detrás (alineada a 8 bytes).
    fn props_buffer(name: &[u16]) -> Vec<u64> {
        let head = std::mem::size_of::<EVENT_TRACE_PROPERTIES>();
        let total = head + name.len() * 2;
        let mut buf = vec![0u64; total.div_ceil(8)];
        let p = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        // SAFETY: el búfer tiene sitio para la estructura y el nombre detrás, y está alineado a 8.
        unsafe {
            (*p).Wnode.BufferSize = (buf.len() * 8) as u32;
            (*p).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*p).Wnode.ClientContext = 1;
            (*p).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
            (*p).BufferSize = 64;
            (*p).MinimumBuffers = 2;
            (*p).MaximumBuffers = 16;
            (*p).FlushTimer = 1;
            (*p).LoggerNameOffset = head as u32;
            let dst = (buf.as_mut_ptr() as *mut u8).add(head) as *mut u16;
            std::ptr::copy_nonoverlapping(name.as_ptr(), dst, name.len());
        }
        buf
    }

    /// Para la sesión con ese nombre si existe. Devuelve si había una.
    pub fn stop_session_by_name() -> bool {
        stop_session(super::SESSION_NAME)
    }

    /// Para una sesión por su nombre. Devuelve si había una.
    pub fn stop_session(session: &str) -> bool {
        let name = wide(session);
        let mut buf = props_buffer(&name);
        // SAFETY: `buf` es una `EVENT_TRACE_PROPERTIES` con su nombre detrás; el nombre acaba en 0.
        let r = unsafe { ControlTraceW(CONTROLTRACE_HANDLE { Value: 0 }, name.as_ptr(), buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_STOP) };
        r == 0
    }

    unsafe extern "system" fn on_event(rec: *mut EVENT_RECORD) {
        if rec.is_null() {
            return;
        }
        // SAFETY: Windows da un registro válido durante la llamada.
        let r = unsafe { &*rec };
        let guid = r.EventHeader.ProviderId;
        if guid.data1 != KERNEL_NETWORK.data1 || guid.data2 != KERNEL_NETWORK.data2 || guid.data3 != KERNEL_NETWORK.data3 || guid.data4 != KERNEL_NETWORK.data4 {
            // Solo en las pruebas: se cuentan los eventos de otros proveedores (el de prueba de la sesión de ensayo).
            #[cfg(test)]
            super::TEST_EVENTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return;
        }
        let len = r.UserDataLength as usize;
        if r.UserData.is_null() || len == 0 {
            return;
        }
        // SAFETY: `UserData` apunta a `UserDataLength` bytes que viven durante la llamada.
        let data = unsafe { std::slice::from_raw_parts(r.UserData as *const u8, len) };
        let _ = std::panic::catch_unwind(|| {
            if let Some(ev) = super::parse_event(r.EventHeader.EventDescriptor.Id, data) {
                super::push_event(&ev);
            }
        });
    }

    /// Una sesión de ETW en marcha. Al soltarla se para.
    pub struct EtwSession {
        control: CONTROLTRACE_HANDLE,
        name: Vec<u16>,
        consumer: Option<std::thread::JoinHandle<()>>,
        trace: PROCESSTRACE_HANDLE,
    }

    impl EtwSession {
        pub fn start() -> Result<EtwSession, StartError> {
            EtwSession::start_with(super::SESSION_NAME, &KERNEL_NETWORK, KEYWORDS)
        }

        /// Una sesión con ese nombre que escucha ese proveedor (en uso, solo el de la red del kernel; las pruebas usan otro).
        pub fn start_with(session: &str, provider: &GUID, keywords: u64) -> Result<EtwSession, StartError> {
            let name = wide(session);
            let mut control = CONTROLTRACE_HANDLE { Value: 0 };
            let mut buf = props_buffer(&name);
            // SAFETY: llamadas de ETW con búferes propios y vivos durante la llamada.
            let mut r = unsafe { StartTraceW(&mut control, name.as_ptr(), buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES) };
            if r == ERROR_ALREADY_EXISTS {
                // Quedó una de otra ejecución: se para y se vuelve a intentar una vez.
                stop_session(session);
                buf = props_buffer(&name);
                // SAFETY: igual que arriba.
                r = unsafe { StartTraceW(&mut control, name.as_ptr(), buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES) };
            }
            if r == ERROR_ACCESS_DENIED {
                return Err(StartError::Denied("StartTrace"));
            }
            if r != 0 {
                return Err(StartError::Other(format!("StartTrace dio el error {r}")));
            }
            let mut session = EtwSession { control, name: name.clone(), consumer: None, trace: PROCESSTRACE_HANDLE { Value: INVALID_TRACE } };
            // SAFETY: `control` es la sesión recién creada.
            let r = unsafe { EnableTraceEx2(control, provider, EVENT_CONTROL_CODE_ENABLE_PROVIDER, TRACE_LEVEL_VERBOSE as u8, keywords, 0, 0, null()) };
            if r == ERROR_ACCESS_DENIED {
                return Err(StartError::Denied("EnableTrace"));
            }
            if r != 0 {
                return Err(StartError::Other(format!("EnableTrace dio el error {r}")));
            }
            let mut logfile: EVENT_TRACE_LOGFILEW = unsafe { std::mem::zeroed() };
            let mut logger = name.clone();
            logfile.LoggerName = logger.as_mut_ptr();
            logfile.Anonymous1.ProcessTraceMode = PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
            logfile.Anonymous2.EventRecordCallback = Some(on_event);
            // SAFETY: `logfile` y `logger` viven durante la llamada; ETW copia lo que necesita.
            let trace = unsafe { OpenTraceW(&mut logfile) };
            if trace.Value == INVALID_TRACE {
                return Err(StartError::Other("OpenTrace falló".into()));
            }
            session.trace = trace;
            session.consumer = std::thread::Builder::new()
                .name("netusage-etw".into())
                .spawn(move || {
                    // SAFETY: `trace` es un manejador abierto; ProcessTrace vuelve cuando se para la sesión.
                    let r = unsafe { ProcessTrace(&trace, 1, null(), null()) };
                    if r != 0 {
                        log::warn!("Uso de la red: ProcessTrace terminó con el error {r}");
                    }
                })
                .ok();
            if session.consumer.is_none() {
                return Err(StartError::Other("no se pudo crear el hilo de ETW".into()));
            }
            Ok(session)
        }
    }

    impl EtwSession {
        /// Eventos y búferes que ETW perdió desde que empezó la sesión.
        pub fn lost(&self) -> Option<u64> {
            let mut buf = props_buffer(&self.name);
            // SAFETY: igual que en `start`; consultar no cambia la sesión.
            let r = unsafe { ControlTraceW(self.control, self.name.as_ptr(), buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_QUERY) };
            if r != 0 {
                return None;
            }
            let p = buf.as_ptr() as *const EVENT_TRACE_PROPERTIES;
            // SAFETY: ETW rellenó la estructura que está al principio del búfer.
            Some(unsafe { u64::from((*p).EventsLost) + u64::from((*p).RealTimeBuffersLost) })
        }
    }

    impl Drop for EtwSession {
        fn drop(&mut self) {
            let mut buf = props_buffer(&self.name);
            // SAFETY: igual que en `start`; parar la sesión hace volver a `ProcessTrace`.
            unsafe {
                ControlTraceW(self.control, self.name.as_ptr(), buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_STOP);
            }
            if let Some(h) = self.consumer.take() {
                let t0 = Instant::now();
                while !h.is_finished() && t0.elapsed() < Duration::from_secs(3) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                if h.is_finished() {
                    let _ = h.join();
                }
            }
            if self.trace.Value != INVALID_TRACE {
                // SAFETY: el manejador se abrió en `start` y no se ha cerrado.
                unsafe { CloseTrace(self.trace) };
            }
        }
    }

    // ----- Plan B: estadísticas de las conexiones TCP -----

    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, GetPerTcpConnectionEStats, SetPerTcpConnectionEStats, MIB_TCPROW_LH, MIB_TCPROW_OWNER_PID, TcpConnectionEstatsData, TCP_ESTATS_DATA_ROD_v0, TCP_ESTATS_DATA_RW_v0,
        TCP_TABLE_OWNER_PID_CONNECTIONS,
    };

    /// Solo TCP IPv4: pide a Windows contar los bytes de cada conexión y los lee cada segundo.
    pub struct Estats {
        last: HashMap<(u32, u32, u32, u32), (u64, u64)>,
        /// Las conexiones en las que se encendió la cuenta: al acabar se apaga.
        enabled: HashMap<(u32, u32, u32, u32), MIB_TCPROW_LH>,
    }

    const AF_INET: u32 = 2;
    const MAX_ROWS: usize = 65_536;

    impl Estats {
        pub fn new() -> Result<Estats, String> {
            let mut e = Estats { last: HashMap::new(), enabled: HashMap::new() };
            // Una primera lectura: si no se puede ni leer la tabla ni activar una conexión, no hay plan B.
            let rows = table()?;
            if let Some(r) = rows.first() {
                match enable(r, true) {
                    // La conexión que se activó aquí también se anota, para que `Drop` la apague.
                    Ok(()) => {
                        let key = (r.dwLocalAddr, r.dwLocalPort, r.dwRemoteAddr, r.dwRemotePort);
                        let row = MIB_TCPROW_LH { Anonymous: windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCPROW_LH_0 { dwState: r.dwState }, dwLocalAddr: r.dwLocalAddr, dwLocalPort: r.dwLocalPort, dwRemoteAddr: r.dwRemoteAddr, dwRemotePort: r.dwRemotePort };
                        e.enabled.insert(key, row);
                    }
                    Err(c) if c == ERROR_ACCESS_DENIED => return Err("sin permiso".into()),
                    Err(_) => {}
                }
            }
            e.last.clear();
            Ok(e)
        }

        /// Un latido: los bytes nuevos de cada conexión pasan a los contadores por proceso.
        pub fn poll(&mut self) {
            let Ok(rows) = table() else { return };
            let mut seen = HashMap::new();
            for r in rows.iter().take(MAX_ROWS) {
                let key = (r.dwLocalAddr, r.dwLocalPort, r.dwRemoteAddr, r.dwRemotePort);
                let remote = IpAddr::V4(std::net::Ipv4Addr::from(u32::from_be(r.dwRemoteAddr)));
                let local = IpAddr::V4(std::net::Ipv4Addr::from(u32::from_be(r.dwLocalAddr)));
                if remote.is_loopback() || remote.is_unspecified() {
                    continue;
                }
                let row = MIB_TCPROW_LH { Anonymous: windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCPROW_LH_0 { dwState: r.dwState }, dwLocalAddr: r.dwLocalAddr, dwLocalPort: r.dwLocalPort, dwRemoteAddr: r.dwRemoteAddr, dwRemotePort: r.dwRemotePort };
                let known = self.last.contains_key(&key);
                if !known {
                    if enable(&row, true).is_err() {
                        continue;
                    }
                    if self.enabled.len() < MAX_ROWS {
                        self.enabled.insert(key, row);
                    }
                }
                let Some((out, inn)) = read(&row) else { continue };
                if let Some(&(po, pi)) = self.last.get(&key) {
                    let (du, dd) = (out.saturating_sub(po), inn.saturating_sub(pi));
                    if du + dd > 0 {
                        for (up, bytes) in [(true, du), (false, dd)] {
                            let mut left = bytes;
                            while left > 0 {
                                let b = left.min(u64::from(u32::MAX)) as u32;
                                // Lo que llega va «del otro extremo a este equipo»: el destino es la dirección propia.
                                let (dst, src) = if up { (remote, local) } else { (local, remote) };
                                super::push_event(&NetEvent { pid: r.dwOwningPid, bytes: b, dst, src, upload: up, udp: false });
                                left -= u64::from(b);
                            }
                        }
                    }
                }
                seen.insert(key, (out, inn));
            }
            self.last = seen;
            // Las conexiones que ya no están se olvidan (Windows las libera solo).
            let gone: Vec<_> = self.enabled.keys().filter(|k| !self.last.contains_key(*k)).copied().collect();
            for k in gone {
                self.enabled.remove(&k);
            }
        }
    }

    impl Drop for Estats {
        fn drop(&mut self) {
            for row in self.enabled.values() {
                let _ = enable(row, false);
            }
        }
    }

    fn table() -> Result<Vec<MIB_TCPROW_OWNER_PID>, String> {
        let mut size = 0u32;
        // SAFETY: primera llamada sin búfer, solo para saber el tamaño.
        let r = unsafe { GetExtendedTcpTable(null_mut(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_CONNECTIONS, 0) };
        if r != ERROR_INSUFFICIENT_BUFFER && r != 0 {
            return Err(format!("GetExtendedTcpTable dio el error {r}"));
        }
        for _ in 0..4 {
            let size_usize = (size as usize).clamp(4, 64 * 1024 * 1024) + 4096;
            let mut buf = vec![0u8; size_usize.div_ceil(4) * 4];
            let mut size2 = buf.len() as u32;
            // SAFETY: `buf` mide `size2` bytes.
            let r = unsafe { GetExtendedTcpTable(buf.as_mut_ptr() as *mut c_void, &mut size2, 0, AF_INET, TCP_TABLE_OWNER_PID_CONNECTIONS, 0) };
            if r == ERROR_INSUFFICIENT_BUFFER {
                size = size2;
                continue;
            }
            if r != 0 {
                return Err(format!("GetExtendedTcpTable dio el error {r}"));
            }
            // La tabla es un contador de 4 bytes y las filas.
            if buf.len() < 4 {
                return Ok(Vec::new());
            }
            let n = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            let row = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
            if n > MAX_ROWS || 4 + n * row > buf.len() {
                return Err("la tabla de conexiones no cuadra".into());
            }
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                // SAFETY: la fila `i` está dentro del búfer (comprobado arriba); se lee sin alinear.
                out.push(unsafe { std::ptr::read_unaligned(buf.as_ptr().add(4 + i * row) as *const MIB_TCPROW_OWNER_PID) });
            }
            return Ok(out);
        }
        Err("la tabla de conexiones cambia demasiado deprisa".into())
    }

    fn enable(row: &impl RowRef, on: bool) -> Result<(), u32> {
        let rw = TCP_ESTATS_DATA_RW_v0 { EnableCollection: on };
        // SAFETY: `row` y `rw` son estructuras completas del tamaño que se indica.
        let r = unsafe { SetPerTcpConnectionEStats(row.as_row(), TcpConnectionEstatsData, &rw as *const _ as *const u8, 0, std::mem::size_of::<TCP_ESTATS_DATA_RW_v0>() as u32, 0) };
        if r == 0 {
            Ok(())
        } else {
            Err(r)
        }
    }

    fn read(row: &MIB_TCPROW_LH) -> Option<(u64, u64)> {
        let mut rod: TCP_ESTATS_DATA_ROD_v0 = unsafe { std::mem::zeroed() };
        // SAFETY: `rod` es una estructura completa del tamaño que se indica.
        let r = unsafe { GetPerTcpConnectionEStats(row, TcpConnectionEstatsData, null_mut(), 0, 0, null_mut(), 0, 0, &mut rod as *mut _ as *mut u8, 0, std::mem::size_of::<TCP_ESTATS_DATA_ROD_v0>() as u32) };
        (r == 0).then_some((rod.DataBytesOut, rod.DataBytesIn))
    }

    trait RowRef {
        fn as_row(&self) -> *const MIB_TCPROW_LH;
    }

    impl RowRef for MIB_TCPROW_LH {
        fn as_row(&self) -> *const MIB_TCPROW_LH {
            self
        }
    }

    impl RowRef for MIB_TCPROW_OWNER_PID {
        fn as_row(&self) -> *const MIB_TCPROW_LH {
            // El prefijo de la fila con dueño es la fila normal: estado, dirección y puerto local y remoto.
            self as *const MIB_TCPROW_OWNER_PID as *const MIB_TCPROW_LH
        }
    }

    // ----- Quién es cada proceso -----

    use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, ENUM_SERVICE_STATUS_PROCESSW, SC_ENUM_PROCESS_INFO, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_STATE_ALL, SERVICE_WIN32,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};

    fn process_path(pid: u32) -> Option<String> {
        // SAFETY: se abre con el permiso mínimo y se cierra siempre.
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            (ok != 0 && (len as usize) <= buf.len()).then(|| String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    /// La descripción del archivo (campo «FileDescription» de su versión).
    fn file_description(path: &str) -> Option<String> {
        let w = wide(path);
        // SAFETY: el búfer se reserva con el tamaño que dice Windows (acotado) y las lecturas se comprueban dentro de él.
        unsafe {
            let mut handle = 0u32;
            let size = GetFileVersionInfoSizeW(w.as_ptr(), &mut handle);
            if size == 0 || size > (1 << 20) {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            if GetFileVersionInfoW(w.as_ptr(), 0, size, buf.as_mut_ptr() as *mut c_void) == 0 {
                return None;
            }
            let start = buf.as_ptr() as usize;
            let end = start + buf.len();
            let query = |sub: &str| -> Option<(*const u16, usize)> {
                let q = wide(sub);
                let mut p: *mut c_void = null_mut();
                let mut len = 0u32;
                if VerQueryValueW(buf.as_ptr() as *const c_void, q.as_ptr(), &mut p, &mut len) == 0 || p.is_null() || len == 0 {
                    return None;
                }
                let (a, n) = (p as usize, len as usize);
                (a >= start && a.checked_add(n.checked_mul(2)?)? <= end).then_some((p as *const u16, n))
            };
            let mut langs: Vec<(u16, u16)> = Vec::new();
            if let Some((p, n)) = query("\\VarFileInfo\\Translation") {
                // Cada traducción son 4 bytes (idioma y página de códigos): aquí `n` cuenta bytes.
                for i in 0..(n / 4).min(4) {
                    langs.push((*p.add(i * 2), *p.add(i * 2 + 1)));
                }
            }
            langs.push((0x0409, 0x04B0));
            for (l, c) in langs {
                if let Some((p, n)) = query(&format!("\\StringFileInfo\\{l:04x}{c:04x}\\FileDescription")) {
                    let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                    let s = s.trim_end_matches('\0').trim().to_string();
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
            None
        }
    }

    /// Servicios de cada proceso (los de un `svchost`).
    fn service_names() -> HashMap<u32, Vec<String>> {
        let mut out: HashMap<u32, Vec<String>> = HashMap::new();
        // SAFETY: lista de servicios con búfer propio; los textos se leen dentro de él.
        unsafe {
            let scm = OpenSCManagerW(null(), null(), SC_MANAGER_ENUMERATE_SERVICE);
            if scm.is_null() {
                return out;
            }
            let mut needed = 0u32;
            let mut count = 0u32;
            let mut resume = 0u32;
            EnumServicesStatusExW(scm, SC_ENUM_PROCESS_INFO, SERVICE_WIN32, SERVICE_STATE_ALL, null_mut(), 0, &mut needed, &mut count, &mut resume, null());
            let mut size = (needed as usize).clamp(1024, 8 * 1024 * 1024);
            for _ in 0..4 {
                let mut buf = vec![0u64; size.div_ceil(8)];
                let mut needed = 0u32;
                let mut count = 0u32;
                let mut resume = 0u32;
                let bytes = (buf.len() * 8) as u32;
                let ok = EnumServicesStatusExW(scm, SC_ENUM_PROCESS_INFO, SERVICE_WIN32, SERVICE_STATE_ALL, buf.as_mut_ptr() as *mut u8, bytes, &mut needed, &mut count, &mut resume, null());
                let err = if ok == 0 { windows_sys::Win32::Foundation::GetLastError() } else { 0 };
                if ok == 0 && err == ERROR_MORE_DATA {
                    size = (needed as usize + 4096).min(16 * 1024 * 1024);
                    continue;
                }
                if ok != 0 {
                    let start = buf.as_ptr() as usize;
                    let end = start + buf.len() * 8;
                    let each = std::mem::size_of::<ENUM_SERVICE_STATUS_PROCESSW>();
                    let count = (count as usize).min((end - start) / each);
                    let entries = buf.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW;
                    for i in 0..count {
                        let e = std::ptr::read_unaligned(entries.add(i));
                        let p = e.lpServiceName as usize;
                        if e.lpServiceName.is_null() || p < start || p >= end {
                            continue;
                        }
                        // Texto acabado en 0 dentro del búfer.
                        let max = (end - p) / 2;
                        let mut n = 0;
                        while n < max && *e.lpServiceName.add(n) != 0 {
                            n += 1;
                        }
                        let name = String::from_utf16_lossy(std::slice::from_raw_parts(e.lpServiceName, n));
                        let pid = e.ServiceStatusProcess.dwProcessId;
                        if pid != 0 {
                            out.entry(pid).or_default().push(name);
                        }
                    }
                }
                break;
            }
            CloseServiceHandle(scm);
        }
        out
    }

    /// De número de proceso a programa, con caché.
    pub struct Idents {
        by_pid: HashMap<u32, (Ident, Instant)>,
        desc: HashMap<String, String>,
        services: HashMap<u32, Vec<String>>,
        services_at: Option<Instant>,
    }

    impl Idents {
        pub fn new() -> Idents {
            Idents { by_pid: HashMap::new(), desc: HashMap::new(), services: HashMap::new(), services_at: None }
        }

        pub fn identify(&mut self, pid: u32) -> Ident {
            if let Some((id, at)) = self.by_pid.get(&pid) {
                if at.elapsed() < Duration::from_secs(30) {
                    return id.clone();
                }
            }
            let id = self.resolve(pid);
            if let Some(p) = &id.path {
                remember_path(p);
            }
            if self.by_pid.len() > 4096 {
                self.by_pid.clear();
            }
            self.by_pid.insert(pid, (id.clone(), Instant::now()));
            id
        }

        fn resolve(&mut self, pid: u32) -> Ident {
            if pid == 0 || pid == 4 {
                return Ident { key: "system".into(), name: "Windows (sistema)".into(), path: None };
            }
            let Some(path) = process_path(pid) else {
                return Ident { key: "otros".into(), name: "Otros procesos".into(), path: None };
            };
            let file = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
            if file.eq_ignore_ascii_case("svchost.exe") {
                if self.services_at.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) || !self.services.contains_key(&pid) {
                    // Un servicio nuevo puede no estar en la lista de hace unos segundos.
                    if self.services_at.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
                        self.services = service_names();
                        self.services_at = Some(Instant::now());
                    }
                }
                let names = self.services.get(&pid).cloned().unwrap_or_default();
                let (key, name) = super::service_group(&names);
                return Ident { key: key.into(), name: name.into(), path: Some(path) };
            }
            let desc = self.desc.entry(path.clone()).or_insert_with(|| file_description(&path).unwrap_or_default()).clone();
            let name = super::friendly_name((!desc.is_empty()).then_some(desc.as_str()), &file);
            Ident { key: file.to_lowercase(), name, path: Some(path) }
        }
    }

    // ----- Iconos -----

    static KNOWN: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ICONS: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

    pub fn remember_path(p: &str) {
        let mut g = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
        if !g.iter().any(|k| k.eq_ignore_ascii_case(p)) {
            if g.len() > 1024 {
                g.clear();
            }
            g.push(p.to_string());
        }
    }

    pub fn icon_data_url(path: &str) -> Option<String> {
        if !KNOWN.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|k| k.eq_ignore_ascii_case(path)) {
            return None;
        }
        if let Some(hit) = ICONS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(path)) {
            return hit.clone();
        }
        let png = icon_png(path);
        let url = png.map(|b| {
            use base64::Engine;
            format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))
        });
        ICONS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(path.to_string(), url.clone());
        url
    }

    use windows_sys::Win32::Graphics::Gdi::{CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    /// Los píxeles (BGRA de arriba abajo) de un mapa de bits, o `None`.
    unsafe fn bitmap_pixels(dc: *mut c_void, bmp: *mut c_void, w: i32, h: i32) -> Option<Vec<u8>> {
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader = BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB, biSizeImage: 0, biXPelsPerMeter: 0, biYPelsPerMeter: 0, biClrUsed: 0, biClrImportant: 0 };
        let mut px = vec![0u8; (w as usize) * (h as usize) * 4];
        let got = GetDIBits(dc, bmp, 0, h as u32, px.as_mut_ptr() as *mut c_void, &mut bi, DIB_RGB_COLORS);
        (got != 0).then_some(px)
    }

    fn icon_png(path: &str) -> Option<Vec<u8>> {
        // Los iconos de algunos programas los da una extensión del Explorador que necesita COM en este hilo.
        // SAFETY: se inicia y, si se consiguió, se cierra al salir.
        let com = unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED) }.is_ok();
        let out = icon_png_inner(path);
        if com {
            // SAFETY: empareja el `CoInitializeEx` de arriba.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
        out
    }

    fn icon_png_inner(path: &str) -> Option<Vec<u8>> {
        let w = wide(path);
        // SAFETY: se recorre la API de iconos de Windows; todo lo que se abre se cierra antes de salir.
        unsafe {
            let mut info: SHFILEINFOW = std::mem::zeroed();
            let r = SHGetFileInfoW(w.as_ptr(), 0, &mut info, std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
            if r == 0 || info.hIcon.is_null() {
                return None;
            }
            let mut ii: ICONINFO = std::mem::zeroed();
            let mut out = None;
            if GetIconInfo(info.hIcon, &mut ii) != 0 {
                let mut bm: BITMAP = std::mem::zeroed();
                if !ii.hbmColor.is_null() && GetObjectW(ii.hbmColor, std::mem::size_of::<BITMAP>() as i32, &mut bm as *mut _ as *mut c_void) != 0 && bm.bmWidth > 0 && bm.bmHeight > 0 && bm.bmWidth <= 256 && bm.bmHeight <= 256 {
                    let dc = CreateCompatibleDC(null_mut());
                    if !dc.is_null() {
                        let (bw, bh) = (bm.bmWidth, bm.bmHeight);
                        if let Some(mut color) = bitmap_pixels(dc, ii.hbmColor, bw, bh) {
                            // Los iconos viejos no traen transparencia en el color: la da la máscara.
                            let has_alpha = color.chunks(4).any(|p| p[3] != 0);
                            let mask = if has_alpha || ii.hbmMask.is_null() { None } else { bitmap_pixels(dc, ii.hbmMask, bw, bh) };
                            for (i, p) in color.chunks_mut(4).enumerate() {
                                let a = if has_alpha {
                                    p[3]
                                } else if let Some(m) = &mask {
                                    // En la máscara, blanco es transparente.
                                    if m[i * 4] > 127 { 0 } else { 255 }
                                } else {
                                    255
                                };
                                p.swap(0, 2);
                                p[3] = a;
                            }
                            out = super::png_rgba(bw as u32, bh as u32, &color);
                        }
                        DeleteDC(dc);
                    }
                }
                if !ii.hbmColor.is_null() {
                    DeleteObject(ii.hbmColor);
                }
                if !ii.hbmMask.is_null() {
                    DeleteObject(ii.hbmMask);
                }
            }
            DestroyIcon(info.hIcon);
            out
        }
    }

    // ----- DNS inverso -----

    use windows_sys::Win32::Networking::WinSock::{GetNameInfoW, AF_INET as WS_AF_INET, AF_INET6 as WS_AF_INET6, NI_NAMEREQD, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6};

    pub fn reverse_lookup(ip: &IpAddr) -> Option<String> {
        // Que Winsock esté iniciado (la biblioteca estándar lo hace al abrir el primer socket).
        let _ = std::net::UdpSocket::bind(("127.0.0.1", 0));
        let mut host = [0u16; 1025];
        // SAFETY: las estructuras de dirección son completas y del tamaño que se pasa; el búfer de salida mide lo que se indica.
        let r = unsafe {
            match ip {
                IpAddr::V4(a) => {
                    let mut sa: SOCKADDR_IN = std::mem::zeroed();
                    sa.sin_family = WS_AF_INET;
                    sa.sin_addr.S_un.S_addr = u32::from_ne_bytes(a.octets());
                    GetNameInfoW(&sa as *const _ as *const SOCKADDR, std::mem::size_of::<SOCKADDR_IN>() as i32, host.as_mut_ptr(), host.len() as u32, null_mut(), 0, NI_NAMEREQD as i32)
                }
                IpAddr::V6(a) => {
                    let mut sa: SOCKADDR_IN6 = std::mem::zeroed();
                    sa.sin6_family = WS_AF_INET6;
                    sa.sin6_addr.u.Byte = a.octets();
                    GetNameInfoW(&sa as *const _ as *const SOCKADDR, std::mem::size_of::<SOCKADDR_IN6>() as i32, host.as_mut_ptr(), host.len() as u32, null_mut(), 0, NI_NAMEREQD as i32)
                }
            }
        };
        if r != 0 {
            return None;
        }
        let n = host.iter().position(|&c| c == 0).unwrap_or(host.len());
        let s = String::from_utf16_lossy(&host[..n]);
        (!s.is_empty()).then_some(s)
    }
}

#[cfg(not(windows))]
mod win {
    use super::Ident;
    use std::net::IpAddr;

    pub enum StartError {
        Denied(&'static str),
        Other(String),
    }
    pub struct EtwSession;
    impl EtwSession {
        pub fn lost(&self) -> Option<u64> {
            None
        }
        pub fn start() -> Result<EtwSession, StartError> {
            Err(StartError::Other("solo en Windows".into()))
        }
    }
    pub struct Estats;
    impl Estats {
        pub fn new() -> Result<Estats, String> {
            Err("solo en Windows".into())
        }
        pub fn poll(&mut self) {}
    }
    pub struct Idents;
    impl Idents {
        pub fn new() -> Idents {
            Idents
        }
        pub fn identify(&mut self, _pid: u32) -> Ident {
            Ident { key: "otros".into(), name: "Otros procesos".into(), path: None }
        }
    }
    pub fn stop_session_by_name() -> bool {
        false
    }
    pub fn icon_data_url(_path: &str) -> Option<String> {
        None
    }
    pub fn remember_path(_p: &str) {}
    pub fn reverse_lookup(_ip: &IpAddr) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn v4_event(id: u16, pid: u32, size: u32, daddr: [u8; 4]) -> (u16, Vec<u8>) {
        let mut d = Vec::new();
        d.extend_from_slice(&pid.to_le_bytes());
        d.extend_from_slice(&size.to_le_bytes());
        d.extend_from_slice(&daddr);
        d.extend_from_slice(&[192, 168, 1, 20]);
        d.extend_from_slice(&[0x01, 0xBB, 0xC3, 0x50]);
        d.extend_from_slice(&[0u8; 16]);
        (id, d)
    }

    #[test]
    fn un_evento_de_tcp_ipv4_se_lee() {
        let (id, d) = v4_event(10, 4321, 1460, [52, 96, 12, 5]);
        let e = parse_event(id, &d).unwrap();
        assert_eq!(e, NetEvent { pid: 4321, bytes: 1460, dst: "52.96.12.5".parse().unwrap(), src: "192.168.1.20".parse().unwrap(), upload: true, udp: false });
        // En un envío, el otro extremo es el destino.
        assert_eq!(e.remote(&HashSet::new()), "52.96.12.5".parse::<IpAddr>().unwrap());
        let (id, d) = v4_event(11, 7, 100, [8, 8, 8, 8]);
        assert!(!parse_event(id, &d).unwrap().upload);
    }

    #[test]
    fn udp_y_ipv6_tambien() {
        let (id, d) = v4_event(42, 9, 1200, [142, 250, 1, 1]);
        let e = parse_event(id, &d).unwrap();
        assert!(e.udp && e.upload);
        let (id, d) = v4_event(43, 9, 1200, [142, 250, 1, 1]);
        assert!(parse_event(id, &d).unwrap().udp);
        let mut d = Vec::new();
        d.extend_from_slice(&5u32.to_le_bytes());
        d.extend_from_slice(&900u32.to_le_bytes());
        d.extend_from_slice(&"2a00:1450::1".parse::<Ipv6Addr>().unwrap().octets());
        d.extend_from_slice(&[0u8; 16]);
        d.extend_from_slice(&[0u8; 8]);
        let e = parse_event(27, &d).unwrap();
        assert_eq!(e.dst, "2a00:1450::1".parse::<IpAddr>().unwrap());
        assert!(!e.upload && !e.udp);
        assert!(parse_event(59, &d).unwrap().udp);
    }

    #[test]
    fn un_evento_corto_o_de_otro_tipo_se_ignora() {
        assert!(parse_event(10, &[1, 2, 3]).is_none());
        assert!(parse_event(12, &[0u8; 64]).is_none());
        // IPv6 con los datos de IPv4: faltan bytes.
        assert!(parse_event(26, &[0u8; 20]).is_none());
        assert!(parse_event(10, &[]).is_none());
    }

    #[test]
    fn el_otro_extremo_se_elige_por_el_sentido_y_por_las_direcciones_propias() {
        let mk = |upload: bool, dst: &str, src: &str| NetEvent { pid: 1, bytes: 1, dst: dst.parse().unwrap(), src: src.parse().unwrap(), upload, udp: false };
        let locals: HashSet<IpAddr> = ["192.168.1.20".parse().unwrap()].into_iter().collect();
        let nube: IpAddr = "52.1.1.1".parse().unwrap();
        // Como dice el manifiesto: envío de mi equipo a la nube; recepción de la nube a mi equipo.
        assert_eq!(mk(true, "52.1.1.1", "192.168.1.20").remote(&locals), nube);
        assert_eq!(mk(false, "192.168.1.20", "52.1.1.1").remote(&locals), nube);
        // Si Windows rellenara los campos al revés, las direcciones propias lo corrigen.
        assert_eq!(mk(false, "52.1.1.1", "192.168.1.20").remote(&locals), nube);
        assert_eq!(mk(true, "192.168.1.20", "52.1.1.1").remote(&locals), nube);
        // Sin saber cuáles son las propias, manda el sentido.
        assert_eq!(mk(true, "52.1.1.1", "192.168.1.20").remote(&HashSet::new()), nube);
    }

    #[test]
    fn lo_local_no_cuenta() {
        assert!(is_local_only(&"127.0.0.1".parse().unwrap()));
        assert!(is_local_only(&"::1".parse().unwrap()));
        assert!(is_local_only(&"0.0.0.0".parse().unwrap()));
        assert!(!is_local_only(&"192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn nombres_amigables() {
        assert_eq!(friendly_name(Some("Google Chrome"), r"C:\Program Files\Google\Chrome\Application\chrome.exe"), "Chrome");
        assert_eq!(friendly_name(Some("Microsoft OneDrive"), "OneDrive.exe"), "OneDrive");
        assert_eq!(friendly_name(Some("Microsoft Teams"), "ms-teams.exe"), "Teams");
        assert_eq!(friendly_name(Some("  Microsoft   Edge "), "msedge.exe"), "Edge");
        assert_eq!(friendly_name(None, r"C:\x\zoom.exe"), "Zoom");
        assert_eq!(friendly_name(Some(""), "steam.EXE"), "Steam");
        // Una descripción larguísima no cabe en la fila.
        assert_eq!(friendly_name(Some("Host Process for Windows Services and other long text"), "svchost.exe"), "Svchost");
        // «Microsoft» a secas se queda como está: no se deja la descripción vacía.
        assert_eq!(friendly_name(Some("Microsoft "), "app.exe"), "Microsoft");
    }

    #[test]
    fn svchost_de_windows_update_y_el_resto() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(service_group(&s(&["wuauserv"])), ("svchost:wu", "Windows Update"));
        assert_eq!(service_group(&s(&["BITS", "DoSvc"])), ("svchost:wu", "Windows Update"));
        assert_eq!(service_group(&s(&["Dnscache", "WinHttpAutoProxySvc"])), ("svchost", "Servicios de Windows"));
        assert_eq!(service_group(&[]), ("svchost", "Servicios de Windows"));
    }

    #[test]
    fn la_media_de_tres_segundos_en_mbit() {
        let mut w = VecDeque::new();
        assert_eq!(mbps(&w), 0.0);
        // 2 300 000 bytes por segundo son 18,4 Mbit/s.
        for _ in 0..5 {
            push_window(&mut w, 2_300_000);
        }
        assert_eq!(w.len(), WINDOW);
        assert!((mbps(&w) - 18.4).abs() < 1e-9);
        // Una muestra a cero baja la media a dos tercios.
        push_window(&mut w, 0);
        assert!((mbps(&w) - 18.4 * 2.0 / 3.0).abs() < 1e-9);
        // Con una sola muestra, es esa.
        let mut one = VecDeque::new();
        push_window(&mut one, 125_000);
        assert!((mbps(&one) - 1.0).abs() < 1e-9);
    }

    fn id(key: &str, name: &str) -> Ident {
        Ident { key: key.into(), name: name.into(), path: Some(format!(r"C:\x\{key}")) }
    }

    #[test]
    fn el_agrupado_por_programa_suma_procesos_y_marca_los_activos() {
        let ip: IpAddr = "52.1.1.1".parse().unwrap();
        let ip2: IpAddr = "52.2.2.2".parse().unwrap();
        let mut live = Live::new(1_000);
        live.apply(2_000, 1_000, vec![(id("onedrive.exe", "OneDrive"), Tick { up: 2_300_000, down: 0, dests: vec![(ip, 2_000_000), (ip2, 300_000)] }), (id("chrome.exe", "Chrome"), Tick { up: 10_000, down: 262_500, dests: vec![] })]);
        live.apply(3_000, 1_000, vec![(id("onedrive.exe", "OneDrive"), Tick { up: 2_300_000, down: 0, dests: vec![(ip, 2_300_000)] })]);
        let today = BTreeMap::new();
        let none = |_: &IpAddr| None;
        let p = live.programs(3_000, &today, &none);
        let one = p.iter().find(|x| x.key == "onedrive.exe").unwrap();
        assert!((one.up_mbps - 18.4).abs() < 1e-9);
        assert_eq!(one.total_up, 4_600_000);
        assert_eq!(one.destinations[0].ip, "52.1.1.1");
        assert_eq!(one.destinations[0].bytes, 4_300_000);
        assert_eq!(one.destinations.len(), 2);
        assert!(one.active);
        let ch = p.iter().find(|x| x.key == "chrome.exe").unwrap();
        // Una muestra con datos y otra sin: media de dos.
        assert!((ch.down_mbps - 262_500.0 * 8.0 / 1e6 / 2.0).abs() < 1e-9);
        // El que más mueve, primero.
        assert_eq!(p[0].key, "onedrive.exe");
        // Doce segundos después de su último tráfico, ya no está activo.
        let later = live.programs(3_000 + 11_000, &today, &none);
        assert!(!later.iter().find(|x| x.key == "chrome.exe").unwrap().active);
        assert!(!later.iter().find(|x| x.key == "onedrive.exe").unwrap().active);
    }

    /// Un latido que tarda 2 s en llegar no cuenta como 1 s: los Mbit/s salen de los bytes entre el tiempo real.
    #[test]
    fn los_mbit_se_dividen_por_el_tiempo_real() {
        let mut live = Live::new(0);
        // 4 600 000 bytes en 2 s son 2 300 000 por segundo: 18,4 Mbit/s.
        live.apply(2_000, 2_000, vec![(id("a.exe", "A"), Tick { up: 4_600_000, down: 0, dests: vec![] })]);
        let p = live.programs(2_000, &BTreeMap::new(), &|_| None);
        assert!((p[0].up_mbps - 18.4).abs() < 1e-9, "{}", p[0].up_mbps);
        // El total sí es lo que pasó de verdad.
        assert_eq!(p[0].total_up, 4_600_000);
        // Un tiempo a cero no divide por cero.
        live.apply(2_001, 0, vec![(id("a.exe", "A"), Tick { up: 1_000, down: 0, dests: vec![] })]);
        assert!(live.programs(2_001, &BTreeMap::new(), &|_| None)[0].up_mbps.is_finite());
    }

    #[test]
    fn los_destinos_se_ordenan_y_usan_el_nombre_si_lo_hay() {
        let mut m = HashMap::new();
        m.insert("1.1.1.1".parse::<IpAddr>().unwrap(), 10u64);
        m.insert("2.2.2.2".parse::<IpAddr>().unwrap(), 300);
        m.insert("3.3.3.3".parse::<IpAddr>().unwrap(), 200);
        m.insert("4.4.4.4".parse::<IpAddr>().unwrap(), 100);
        let names = |ip: &IpAddr| (ip.to_string() == "2.2.2.2").then(|| "onedrive.live.com".to_string());
        let t = top_destinations(&m, &names);
        assert_eq!(t.iter().map(|d| d.host.as_str()).collect::<Vec<_>>(), vec!["onedrive.live.com", "3.3.3.3", "4.4.4.4"]);
        assert_eq!(t[0].ip, "2.2.2.2");
    }

    #[test]
    fn el_total_del_dia_se_guarda_por_dia_y_se_limita() {
        let mut f = DayFile::default();
        f.add("2026-11-22", &id("a.exe", "A"), 100, 50);
        f.add("2026-11-22", &id("a.exe", "A"), 20, 0);
        f.add("2026-11-22", &id("b.exe", "B"), 0, 0);
        let t = f.today("2026-11-22");
        assert_eq!(t.len(), 1);
        assert_eq!((t["a.exe"].up, t["a.exe"].down), (120, 50));
        assert!(f.today("2026-11-21").is_empty());
        for d in 1..=20 {
            f.add(&format!("2026-10-{d:02}"), &id("a.exe", "A"), 1, 1);
        }
        assert_eq!(f.days.len(), KEEP_DAYS);
        assert!(!f.days.contains_key("2026-10-01"));
        // Va y vuelve por JSON.
        let s = serde_json::to_string(&f).unwrap();
        let back: DayFile = serde_json::from_str(&s).unwrap();
        assert_eq!(back.days.len(), f.days.len());
    }

    #[test]
    fn lo_de_hoy_sin_trafico_ahora_sale_en_la_lista() {
        let live = Live::new(0);
        let mut today = BTreeMap::new();
        today.insert("steam.exe".to_string(), DayEntry { name: "Steam".into(), path: None, up: 1, down: 5_000_000 });
        let p = live.programs(10_000, &today, &|_| None);
        assert_eq!(p.len(), 1);
        assert!(!p[0].active);
        assert_eq!(p[0].today_down, 5_000_000);
        assert_eq!(p[0].down_mbps, 0.0);
    }

    #[test]
    fn el_png_tiene_trozos_y_sumas_correctos() {
        let px: Vec<u8> = (0..4 * 4 * 4).map(|i| i as u8).collect();
        let png = png_rgba(4, 4, &px).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // IEND al final con su CRC conocido.
        assert_eq!(&png[png.len() - 12..], &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
        // Recorre los trozos y comprueba cada CRC.
        let mut i = 8;
        let mut kinds = Vec::new();
        while i < png.len() {
            let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
            let body = &png[i + 4..i + 8 + len];
            let crc = u32::from_be_bytes(png[i + 8 + len..i + 12 + len].try_into().unwrap());
            assert_eq!(crc32(body), crc);
            kinds.push(String::from_utf8_lossy(&body[..4]).to_string());
            i += 12 + len;
        }
        assert_eq!(kinds, vec!["IHDR", "IDAT", "IEND"]);
        assert!(png_rgba(4, 4, &px[..10]).is_none());
        assert!(png_rgba(0, 4, &[]).is_none());
        assert!(png_rgba(300, 1, &vec![0; 1200]).is_none());
        // Más de un bloque de zlib (imagen grande para el tamaño de bloque).
        assert!(png_rgba(256, 256, &vec![7u8; 256 * 256 * 4]).is_some());
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    /// Ensayo de la fontanería de ETW sin ser administrador: una sesión con otro nombre escucha un proveedor propio
    /// de este proceso, se escriben 5 eventos y la función de retorno los recibe; al soltarla, la sesión se para.
    /// `cargo test --lib fontaneria_de_etw -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn fontaneria_de_etw() {
        use windows_sys::Win32::System::Diagnostics::Etw::{EventRegister, EventUnregister, EventWriteString};
        let prueba = "AdminOps-UsoRed-Prueba";
        let guid = windows_sys::core::GUID::from_u128(0x6d2a_4c1e_83b0_4f6a_9a52_1c7e_3b90_a4d1);
        win::stop_session(prueba);
        TEST_EVENTS.store(0, Ordering::SeqCst);
        let mut reg = 0i64;
        // SAFETY: registro de un proveedor propio; se da de baja al final.
        let r = unsafe { EventRegister(&guid, None, std::ptr::null(), &mut reg) };
        assert_eq!(r, 0, "EventRegister");
        let sesion = win::EtwSession::start_with(prueba, &guid, 0).map_err(|e| match e {
            win::StartError::Denied(p) => format!("sin permiso en {p}"),
            win::StartError::Other(m) => m,
        });
        let sesion = sesion.expect("la sesión de ensayo arranca");
        let texto: Vec<u16> = "hola\0".encode_utf16().collect();
        for _ in 0..5 {
            // SAFETY: `reg` es el manejador registrado y el texto acaba en 0.
            unsafe { EventWriteString(reg, 4, 0, texto.as_ptr()) };
        }
        std::thread::sleep(Duration::from_millis(1500));
        drop(sesion);
        // SAFETY: se da de baja el proveedor registrado arriba.
        unsafe { EventUnregister(reg) };
        let vistos = TEST_EVENTS.load(Ordering::SeqCst);
        println!("eventos recibidos por la función de retorno: {vistos}");
        assert!(vistos >= 5, "llegaron {vistos}");
        assert!(!win::stop_session(prueba), "la sesión de ensayo seguía abierta");
    }

    /// Icono real de un programa del sistema: se guarda como PNG en la carpeta que diga `ADMINOPS_ICON_OUT` (si está) para verlo.
    /// `cargo test --lib icono_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn icono_real() {
        let windir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        for exe in ["explorer.exe", "notepad.exe", "System32\\cmd.exe"] {
            let path = format!("{windir}\\{exe}");
            win::remember_path(&path);
            let url = win::icon_data_url(&path);
            println!("{path}: {}", url.as_ref().map_or("sin icono".to_string(), |u| format!("{} caracteres", u.len())));
            if let (Some(u), Ok(dir)) = (url, std::env::var("ADMINOPS_ICON_OUT")) {
                use base64::Engine;
                let b64 = u.trim_start_matches("data:image/png;base64,");
                let png = base64::engine::general_purpose::STANDARD.decode(b64).expect("base 64");
                let name = exe.rsplit('\\').next().unwrap_or(exe);
                std::fs::write(std::path::Path::new(&dir).join(format!("{name}.png")), png).expect("escribir");
            }
        }
        // Una ruta que no se ha visto no devuelve nada.
        assert!(win::icon_data_url(&format!("{windir}\\regedit.exe")).is_none());
    }

    /// Equipo real, solo lectura: 20 s de ETW (o el plan B) con la salida por programa.
    /// `cargo test --lib uso_de_red_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn uso_de_red_real() {
        if let Err(e) = win::EtwSession::start().map(drop) {
            match e {
                win::StartError::Denied(step) => println!("ETW: sin permiso en {step} (administrador: {})", crate::elevation::is_elevated()),
                win::StartError::Other(m) => println!("ETW: {m}"),
            }
        }
        hold("test", false);
        // Algo de tráfico propio para que haya qué ver.
        let gen = std::thread::spawn(|| {
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_secs(12) {
                let _ = std::net::TcpStream::connect_timeout(&"1.1.1.1:443".parse().unwrap(), Duration::from_millis(800));
                std::thread::sleep(Duration::from_millis(300));
            }
        });
        for s in 1..=20 {
            std::thread::sleep(Duration::from_secs(1));
            if s % 5 == 0 {
                let snap = snapshot_now();
                println!("--- {s} s · método {} · solo TCP {} · error {:?} · total ↑ {:.3} ↓ {:.3} Mbit/s", snap.method, snap.tcp_only, snap.error, snap.total_up_mbps, snap.total_down_mbps);
                for p in snap.programs.iter().take(8) {
                    println!(
                        "  {:<24} ↑ {:>8.3} ↓ {:>8.3} Mbit/s · total ↑ {:>9} ↓ {:>9} B · activo {} · destinos {:?}",
                        p.name,
                        p.up_mbps,
                        p.down_mbps,
                        p.total_up,
                        p.total_down,
                        p.active,
                        p.destinations.iter().map(|d| d.host.clone()).collect::<Vec<_>>()
                    );
                }
            }
        }
        let _ = gen.join();
        let snap = snapshot_now();
        let paths: Vec<String> = snap.programs.iter().filter_map(|p| p.path.clone()).take(3).collect();
        for p in paths {
            let url = win::icon_data_url(&p);
            println!("icono de {p}: {}", url.as_ref().map_or("ninguno".to_string(), |u| format!("{} bytes de datos", u.len())));
        }
        shutdown();
        let queda = win::stop_session_by_name();
        println!("sesión parada al soltar: {}", !queda);
        assert!(!queda, "la sesión «{SESSION_NAME}» no debería seguir abierta");
    }
}
