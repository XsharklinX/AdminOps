//! Vigilante de la conexión: se deja en marcha (minutos u horas) y apunta cada
//! corte, cuándo empezó, cuánto duró y de quién era la culpa: del router (no
//! contesta la puerta de enlace) o de Internet (el router contesta, fuera no).
//! Es la prueba que pide el proveedor de Internet ante «se me corta a ratos».
//!
//! Cada 5 s: un ping al router y otro a 1.1.1.1 (o a 8.8.8.8 si ese no
//! contesta). Un corte empieza tras dos comprobaciones fallidas seguidas (un
//! ping perdido suelto no es un corte) y acaba con la primera que vuelve a ir.
//! Solo funciona mientras el técnico lo tiene en marcha y AdminOps abierto.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

const EVERY: Duration = Duration::from_secs(5);
/// Lo que se guarda para la gráfica: la última hora y media.
const KEEP_POINTS: usize = 1080;
const MAX_OUTAGES: usize = 500;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outage {
    pub start: u64,
    /// None mientras dura.
    pub end: Option<u64>,
    /// "router": no contesta la puerta de enlace · "internet": el router sí, fuera no · "network": sin red.
    pub kind: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub t: u64,
    /// Lo que tardó Internet en contestar; None si no contestó.
    pub ms: Option<u32>,
    pub router: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchStatus {
    pub running: bool,
    pub started_at: Option<u64>,
    pub gateway: Option<String>,
    pub checks: u64,
    pub failed: u64,
    pub points: Vec<Point>,
    pub outages: Vec<Outage>,
}

#[derive(Default)]
struct State {
    status: WatchStatus,
    /// Comprobaciones fallidas seguidas (el corte se abre a la segunda).
    streak: u32,
    streak_start: u64,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Cada puesta en marcha tiene su número: un hilo de una vuelta anterior que aún
/// no ha terminado de dormir se da cuenta y sale, en vez de seguir en paralelo.
static ROUND: AtomicU64 = AtomicU64::new(0);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// La puerta de enlace por la que sale el tráfico a Internet (sin PowerShell).
pub(crate) fn gateway() -> Option<Ipv4Addr> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetBestRoute, MIB_IPFORWARDROW};
    let mut row: MIB_IPFORWARDROW = unsafe { std::mem::zeroed() };
    let dest = u32::from_ne_bytes([1, 1, 1, 1]);
    if unsafe { GetBestRoute(dest, 0, &mut row) } != 0 {
        return None;
    }
    let ip = Ipv4Addr::from(row.dwForwardNextHop.to_ne_bytes());
    (!ip.is_unspecified()).then_some(ip)
}

/// Un ping; Some(ms) si contestó a tiempo.
fn ping(addr: Ipv4Addr, timeout_ms: u32) -> Option<u32> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY};
    unsafe {
        let h = IcmpCreateFile();
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let data = [0x61u8; 16];
        let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8];
        let n = IcmpSendEcho(h, u32::from_ne_bytes(addr.octets()), data.as_ptr().cast(), data.len() as u16, std::ptr::null(), reply.as_mut_ptr().cast(), reply.len() as u32, timeout_ms);
        IcmpCloseHandle(h);
        if n == 0 {
            return None;
        }
        let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
        (r.Status == 0).then_some(r.RoundTripTime)
    }
}

/// Anota una comprobación. Separado del bucle para poder probarlo.
fn record(s: &mut State, t: u64, gateway: Option<Ipv4Addr>, router: bool, internet: Option<u32>) {
    let st = &mut s.status;
    st.checks += 1;
    st.gateway = gateway.map(|g| g.to_string());
    st.points.push(Point { t, ms: internet, router });
    if st.points.len() > KEEP_POINTS {
        let extra = st.points.len() - KEEP_POINTS;
        st.points.drain(..extra);
    }
    if internet.is_some() {
        s.streak = 0;
        if let Some(open) = st.outages.last_mut().filter(|o| o.end.is_none()) {
            open.end = Some(t);
        }
        return;
    }
    st.failed += 1;
    s.streak += 1;
    if s.streak == 1 {
        s.streak_start = t;
    }
    let kind = if gateway.is_none() {
        "network"
    } else if router {
        "internet"
    } else {
        "router"
    };
    match st.outages.last_mut().filter(|o| o.end.is_none()) {
        // Si el corte cambia de causa (vuelve el router pero no Internet), se dice la peor.
        Some(open) => {
            if open.kind == "internet" && kind != "internet" {
                open.kind = kind.into();
            }
        }
        None if s.streak >= 2 => {
            st.outages.push(Outage { start: s.streak_start, end: None, kind: kind.into() });
            if st.outages.len() > MAX_OUTAGES {
                st.outages.remove(0);
            }
        }
        None => {}
    }
}

fn file(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("netwatch.json")
}

fn save(app: &tauri::AppHandle, outages: &[Outage]) {
    if let Ok(json) = serde_json::to_string(outages) {
        let tmp = file(app).with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, file(app));
        }
    }
}

#[tauri::command]
pub fn netwatch_start(app: tauri::AppHandle) -> WatchStatus {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return netwatch_status(app);
    }
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        // Los cortes de antes (de esta sesión o de la última vez) se conservan.
        if s.status.outages.is_empty() {
            if let Ok(v) = std::fs::read_to_string(file(&app)).map(|t| serde_json::from_str::<Vec<Outage>>(&t).unwrap_or_default()) {
                s.status.outages = v;
            }
        }
        s.status.running = true;
        s.status.started_at = Some(now());
        s.status.checks = 0;
        s.status.failed = 0;
        s.status.points.clear();
        s.streak = 0;
    }
    log::info!("Vigilante de la conexión: en marcha");
    let handle = app.clone();
    let round = ROUND.fetch_add(1, Ordering::SeqCst) + 1;
    let alive = move || RUNNING.load(Ordering::SeqCst) && ROUND.load(Ordering::SeqCst) == round;
    std::thread::spawn(move || {
        while alive() {
            let gw = gateway();
            let router = gw.is_some_and(|g| ping(g, 1000).is_some());
            let internet = ping(Ipv4Addr::new(1, 1, 1, 1), 1500).or_else(|| ping(Ipv4Addr::new(8, 8, 8, 8), 1500));
            let closed = {
                let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
                let before = s.status.outages.clone();
                record(&mut s, now(), gw, router, internet);
                (s.status.outages != before).then(|| s.status.outages.clone())
            };
            if let Some(outages) = closed {
                save(&handle, &outages);
            }
            // Duerme a trozos para que «Detener» responda enseguida.
            for _ in 0..10 {
                if !alive() {
                    break;
                }
                std::thread::sleep(EVERY / 10);
            }
        }
        // Otra vuelta ya en marcha: lo suyo no se toca.
        if ROUND.load(Ordering::SeqCst) != round {
            return;
        }
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        // Un corte abierto al parar se cierra ahí: no se sabe cuánto más duró.
        let t = now();
        if let Some(open) = s.status.outages.last_mut().filter(|o| o.end.is_none()) {
            open.end = Some(t);
        }
        s.status.running = false;
        let outages = s.status.outages.clone();
        drop(s);
        save(&handle, &outages);
        log::info!("Vigilante de la conexión: detenido");
    });
    netwatch_status(app)
}

#[tauri::command]
pub fn netwatch_stop() {
    RUNNING.store(false, Ordering::SeqCst);
}

#[tauri::command]
pub fn netwatch_status(app: tauri::AppHandle) -> WatchStatus {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if s.status.outages.is_empty() && !s.status.running {
        if let Ok(v) = std::fs::read_to_string(file(&app)).map(|t| serde_json::from_str::<Vec<Outage>>(&t).unwrap_or_default()) {
            s.status.outages = v;
        }
    }
    s.status.running = RUNNING.load(Ordering::SeqCst) || s.status.running;
    s.status.clone()
}

/// Borra los cortes apuntados (no cambia nada del equipo).
#[tauri::command]
pub fn netwatch_clear(app: tauri::AppHandle) {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.status.outages.clear();
    s.status.points.clear();
    s.status.checks = 0;
    s.status.failed = 0;
    drop(s);
    save(&app, &[]);
}

#[cfg(test)]
mod tests {
    use super::*;

    const GW: Option<Ipv4Addr> = Some(Ipv4Addr::new(192, 168, 1, 1));

    #[test]
    fn one_lost_ping_is_not_an_outage() {
        let mut s = State::default();
        record(&mut s, 0, GW, true, Some(20));
        record(&mut s, 5, GW, true, None);
        record(&mut s, 10, GW, true, Some(22));
        assert!(s.status.outages.is_empty());
        assert_eq!((s.status.checks, s.status.failed), (3, 1));
    }

    #[test]
    fn outage_starts_at_first_failure_and_blames_the_right_side() {
        let mut s = State::default();
        record(&mut s, 0, GW, true, Some(20));
        record(&mut s, 5, GW, true, None);
        record(&mut s, 10, GW, true, None);
        assert_eq!(s.status.outages, vec![Outage { start: 5, end: None, kind: "internet".into() }]);
        // El router deja de contestar: la culpa pasa a ser del router.
        record(&mut s, 15, GW, false, None);
        record(&mut s, 20, GW, true, Some(30));
        assert_eq!(s.status.outages, vec![Outage { start: 5, end: Some(20), kind: "router".into() }]);
        // Sin puerta de enlace: sin red.
        record(&mut s, 25, None, false, None);
        record(&mut s, 30, None, false, None);
        assert_eq!(s.status.outages.last().unwrap().kind, "network");
    }

    #[test]
    fn keeps_a_bounded_chart() {
        let mut s = State::default();
        for i in 0..(KEEP_POINTS as u64 + 50) {
            record(&mut s, i * 5, GW, true, Some(10));
        }
        assert_eq!(s.status.points.len(), KEEP_POINTS);
        assert_eq!(s.status.points[0].t, 50 * 5);
    }
}
