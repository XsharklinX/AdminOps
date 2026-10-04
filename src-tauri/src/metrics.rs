//! Métricas del sistema en vivo (CPU, RAM, discos, red, procesos) vía `sysinfo`.
//!
//! Se mantiene una única instancia de `System` en el estado de Tauri: sysinfo
//! calcula el % de CPU comparando con la lectura anterior, así que recrearla en
//! cada llamada devolvería siempre 0.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;
use sysinfo::{Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub struct MetricsState {
    inner: Mutex<Collector>,
}

struct Collector {
    sys: System,
    disks: Disks,
    networks: Networks,
    last_net: Instant,
    /// Los discos cambian poco: se releen cada pocos segundos, no en cada lectura.
    last_disks: Instant,
    /// Última vez que se recorrió la lista de procesos, y lo que se sacó de ella.
    last_procs: Option<Instant>,
    top_processes: Vec<ProcessInfo>,
    process_count: usize,
}

/// Cada cuánto se vuelve a leer el espacio de los discos.
const DISKS_EVERY: std::time::Duration = std::time::Duration::from_secs(10);
/// Cada cuánto se recorre la lista de procesos.
///
/// Es, de largo, la parte cara de esta lectura: recorre los 250 procesos del
/// equipo para enseñar los 10 que más consumen. Hacerlo en cada tic (cada 2 s
/// por defecto) era un coste fijo permanente que en un equipo viejo se notaba
/// en el propio Administrador de tareas. CPU, memoria y red —que son lo que se
/// ve moverse en la gráfica— se siguen leyendo en cada tic, que es barato.
const PROCS_EVERY: std::time::Duration = std::time::Duration::from_secs(6);

impl MetricsState {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Self {
            inner: Mutex::new(Collector {
                sys,
                disks: Disks::new_with_refreshed_list(),
                networks: Networks::new_with_refreshed_list(),
                last_net: Instant::now(),
                last_disks: Instant::now(),
                last_procs: None,
                top_processes: Vec::new(),
                process_count: 0,
            }),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    host_name: String,
    os_name: String,
    os_version: String,
    kernel_version: String,
    cpu_brand: String,
    physical_cores: usize,
    logical_cores: usize,
    total_memory: u64,
    boot_time: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    mount: String,
    name: String,
    file_system: String,
    kind: String,
    total: u64,
    available: u64,
    removable: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pid: u32,
    name: String,
    cpu: f32,
    memory: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMetrics {
    cpu_total: f32,
    cpu_per_core: Vec<f32>,
    memory_used: u64,
    memory_total: u64,
    swap_used: u64,
    swap_total: u64,
    uptime: u64,
    process_count: usize,
    net_rx_per_sec: u64,
    net_tx_per_sec: u64,
    disks: Vec<DiskInfo>,
    top_processes: Vec<ProcessInfo>,
}

// Fuera del hilo de la ventana: leer los procesos no debe congelarla ni un instante.
#[tauri::command(async)]
pub fn get_system_info(state: tauri::State<MetricsState>) -> SystemInfo {
    let c = state.inner.lock().unwrap_or_else(|e| e.into_inner());
    let cpus = c.sys.cpus();
    SystemInfo {
        host_name: System::host_name().unwrap_or_default(),
        os_name: System::long_os_version().unwrap_or_else(|| "Windows".into()),
        os_version: System::os_version().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        cpu_brand: cpus
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default(),
        physical_cores: System::physical_core_count().unwrap_or(0),
        logical_cores: cpus.len(),
        total_memory: c.sys.total_memory(),
        boot_time: System::boot_time(),
    }
}

// Fuera del hilo de la ventana: leer los procesos no debe congelarla ni un instante.
#[tauri::command(async)]
pub fn get_live_metrics(state: tauri::State<MetricsState>) -> LiveMetrics {
    let mut guard = state.inner.lock().unwrap_or_else(|e| e.into_inner());
    collect(&mut guard)
}

fn collect(c: &mut Collector) -> LiveMetrics {
    c.sys.refresh_cpu_usage();
    c.sys.refresh_memory();
    // La lista de procesos, solo de vez en cuando (ver PROCS_EVERY). Y de cada
    // uno solo CPU y memoria: el nombre se lee una vez, y la ruta y la línea de
    // comandos, que son lo caro, no se usan aquí.
    if c.last_procs.is_none_or(|t| t.elapsed() >= PROCS_EVERY) {
        c.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cpu().with_memory().with_exe(UpdateKind::Never));
        c.last_procs = Some(Instant::now());
        (c.top_processes, c.process_count) = top_processes(&c.sys);
    }
    if c.last_disks.elapsed() >= DISKS_EVERY {
        c.disks.refresh(true);
        c.last_disks = Instant::now();
    }
    c.networks.refresh(true);

    let elapsed = c.last_net.elapsed().as_secs_f64().max(0.001);
    c.last_net = Instant::now();
    let (rx, tx) = c
        .networks
        .iter()
        .fold((0u64, 0u64), |(rx, tx), (_, n)| (rx + n.received(), tx + n.transmitted()));

    LiveMetrics {
        cpu_total: c.sys.global_cpu_usage(),
        cpu_per_core: c.sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
        memory_used: c.sys.used_memory(),
        memory_total: c.sys.total_memory(),
        swap_used: c.sys.used_swap(),
        swap_total: c.sys.total_swap(),
        uptime: System::uptime(),
        process_count: c.process_count,
        net_rx_per_sec: (rx as f64 / elapsed) as u64,
        net_tx_per_sec: (tx as f64 / elapsed) as u64,
        disks: c
            .disks
            .iter()
            .map(|d| DiskInfo {
                mount: d.mount_point().to_string_lossy().into_owned(),
                name: d.name().to_string_lossy().into_owned(),
                file_system: d.file_system().to_string_lossy().into_owned(),
                kind: format!("{:?}", d.kind()),
                total: d.total_space(),
                available: d.available_space(),
                removable: d.is_removable(),
            })
            .collect(),
        top_processes: c.top_processes.clone(),
    }
}

/// Los 10 procesos que más consumen, y cuántos hay en total.
fn top_processes(sys: &System) -> (Vec<ProcessInfo>, usize) {
    // El % de CPU por proceso de sysinfo es relativo a un núcleo (puede pasar de
    // 100), lo normalizamos al total de la máquina como hace el Administrador de tareas.
    let cores = sys.cpus().len().max(1) as f32;
    let mut procs: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .filter(|(pid, _)| pid.as_u32() != 0) // "System Idle Process"
        .map(|(pid, p)| ProcessInfo { pid: pid.as_u32(), name: p.name().to_string_lossy().into_owned(), cpu: p.cpu_usage() / cores, memory: p.memory() })
        .collect();
    let total = procs.len();
    procs.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal).then(b.memory.cmp(&a.memory)));
    procs.truncate(10);
    (procs, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Coste de cada lectura del Panel: `cargo test --release metrics_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn metrics_cost() {
        let state = MetricsState::new();
        let mut c = state.inner.lock().unwrap_or_else(|e| e.into_inner());
        collect(&mut c);
        let n = 20;
        let t = Instant::now();
        for _ in 0..n {
            let m = collect(&mut c);
            assert!(m.memory_total > 0 && m.process_count > 0);
        }
        println!("media por lectura: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }
}
