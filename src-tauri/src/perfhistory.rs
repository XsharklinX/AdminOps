//! Historial de rendimiento de los últimos 7 días: una muestra por minuto de
//! procesador, memoria y disco del sistema, y el programa que más procesador
//! usaba en ese momento. Sirve para «va lento desde el martes»: se ve cuándo
//! empezó y qué coincidía.
//!
//! Solo se mide mientras AdminOps está abierto (no instala nada que corra
//! aparte), y se dice así en la pantalla. Las muestras se guardan en un archivo
//! pequeño de este equipo (`perf-history.json`, unos 400 KB a la semana).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const EVERY: Duration = Duration::from_secs(60);
const KEEP_SECS: u64 = 7 * 24 * 3600;
/// Se guarda en disco cada tantas muestras (y al cerrar se pierde como mucho eso).
const SAVE_EVERY: usize = 5;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Sample {
    /// Segundos desde 1970.
    pub t: u64,
    /// Procesador, %.
    pub cpu: f32,
    /// Memoria en uso, %.
    pub ram: f32,
    /// Disco del sistema ocupado, %.
    pub disk: f32,
    /// Programa que más procesador usaba (si pasaba del 5 %).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<String>,
    /// Los tres programas que más memoria usaban (nombre, MB). Para ver fugas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mem: Vec<(String, u32)>,
    /// Los tres que más leían o escribían en disco ese minuto (nombre, KB/s).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub io: Vec<(String, u32)>,
    /// Disco físico ocupado (media del minuto, %): el «disco al 100 %».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy: Option<f32>,
}

/// Lo ocupado que está el disco: el contador de Windows «% de tiempo inactivo» (al revés).
#[cfg(windows)]
struct DiskBusy {
    query: windows_sys::Win32::System::Performance::PDH_HQUERY,
    counter: windows_sys::Win32::System::Performance::PDH_HCOUNTER,
}

#[cfg(windows)]
impl DiskBusy {
    fn new() -> Option<Self> {
        use windows_sys::Win32::System::Performance::*;
        unsafe {
            let mut query = std::ptr::null_mut();
            if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                return None;
            }
            let path: Vec<u16> = r"\PhysicalDisk(_Total)\% Idle Time".encode_utf16().chain(std::iter::once(0)).collect();
            let mut counter = std::ptr::null_mut();
            if PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) != 0 {
                return None;
            }
            PdhCollectQueryData(query);
            Some(Self { query, counter })
        }
    }

    /// Media desde la lectura anterior.
    fn read(&self) -> Option<f32> {
        use windows_sys::Win32::System::Performance::*;
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return None;
            }
            let mut v = PDH_FMT_COUNTERVALUE::default();
            if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, std::ptr::null_mut(), &mut v) != 0 {
                return None;
            }
            Some((100.0 - v.Anonymous.doubleValue).clamp(0.0, 100.0) as f32)
        }
    }
}

#[cfg(not(windows))]
struct DiskBusy;

#[cfg(not(windows))]
impl DiskBusy {
    fn new() -> Option<Self> {
        None
    }
    fn read(&self) -> Option<f32> {
        None
    }
}

/// Los `n` mayores por nombre de programa (sumando las ventanas de un mismo programa).
fn top_by_name(items: impl Iterator<Item = (String, u64)>, n: usize, min: u64) -> Vec<(String, u32)> {
    let mut by: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for (name, v) in items {
        *by.entry(name).or_default() += v;
    }
    let mut v: Vec<(String, u64)> = by.into_iter().filter(|(_, x)| *x >= min).collect();
    v.sort_by_key(|(_, x)| std::cmp::Reverse(*x));
    v.into_iter().take(n).map(|(name, x)| (name, x.min(u32::MAX as u64) as u32)).collect()
}

static SAMPLES: LazyLock<Mutex<Vec<Sample>>> = LazyLock::new(Default::default);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn file(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("perf-history.json")
}

fn trim(v: &mut Vec<Sample>, now: u64) {
    v.retain(|s| s.t + KEEP_SECS >= now && s.t <= now + 60);
}

fn system_disk_used() -> f32 {
    let c = crate::context::read();
    if c.total_bytes == 0 {
        0.0
    } else {
        (100.0 * (c.total_bytes - c.free_bytes) as f64 / c.total_bytes as f64) as f32
    }
}

fn round1(x: f32) -> f32 {
    (x * 10.0).round() / 10.0
}

/// Empieza a medir (una vez, al abrir AdminOps).
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        if let Ok(text) = std::fs::read_to_string(file(&app)) {
            if let Ok(mut v) = serde_json::from_str::<Vec<Sample>>(&text) {
                trim(&mut v, now());
                *SAMPLES.lock().unwrap_or_else(|e| e.into_inner()) = v;
            }
        }
        let mut sys = System::new();
        let refresh = |sys: &mut System| {
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cpu().with_memory().with_disk_usage().with_exe(UpdateKind::Never));
        };
        // Primera lectura de referencia: el % de procesador sale de comparar con la anterior.
        refresh(&mut sys);
        // El % de cada programa es de un núcleo: se divide entre los núcleos.
        let cores = sys.cpus().len().max(1) as f32;
        let busy = DiskBusy::new();
        let mut unsaved = 0;
        loop {
            std::thread::sleep(EVERY);
            refresh(&mut sys);
            let top = sys
                .processes()
                .values()
                .map(|p| (p.cpu_usage() / cores, p.name().to_string_lossy().into_owned()))
                .filter(|(c, n)| *c >= 5.0 && n != "System Idle Process")
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, n)| n);
            let total = sys.total_memory().max(1) as f64;
            let name = |p: &sysinfo::Process| p.name().to_string_lossy().into_owned();
            let mem = top_by_name(sys.processes().values().map(|p| (name(p), p.memory() / (1024 * 1024))), 3, 200);
            // Lo leído y escrito desde la muestra anterior (un minuto), en KB/s.
            let io = top_by_name(sys.processes().values().map(|p| {
                let d = p.disk_usage();
                (name(p), (d.read_bytes + d.written_bytes) / 1024 / EVERY.as_secs())
            }), 3, 256);
            let s = Sample {
                t: now(),
                cpu: round1(sys.global_cpu_usage()),
                ram: round1((100.0 * sys.used_memory() as f64 / total) as f32),
                disk: round1(system_disk_used()),
                top,
                mem,
                io,
                busy: busy.as_ref().and_then(DiskBusy::read).map(round1),
            };
            let snapshot = {
                let mut v = SAMPLES.lock().unwrap_or_else(|e| e.into_inner());
                v.push(s);
                trim(&mut v, now());
                unsaved += 1;
                (unsaved >= SAVE_EVERY).then(|| v.clone())
            };
            if let Some(v) = snapshot {
                unsaved = 0;
                if let Ok(json) = serde_json::to_string(&v) {
                    // Se escribe aparte y se cambia el nombre: un corte a medias no deja el archivo roto.
                    let (path, tmp) = (file(&app), file(&app).with_extension("json.tmp"));
                    if std::fs::write(&tmp, json).is_ok() {
                        let _ = std::fs::rename(&tmp, &path);
                    }
                }
            }
        }
    });
}

/// Cómo de justa va la memoria según lo medido: media de uso y qué parte del
/// tiempo pasó del 90 %. None si hay menos de dos horas de muestras.
pub fn memory_pressure() -> Option<(f32, f32, u32)> {
    let v = SAMPLES.lock().unwrap_or_else(|e| e.into_inner());
    pressure_of(&v)
}

fn pressure_of(v: &[Sample]) -> Option<(f32, f32, u32)> {
    if v.len() < 120 {
        return None;
    }
    let n = v.len() as f32;
    let avg = v.iter().map(|s| s.ram).sum::<f32>() / n;
    let high = v.iter().filter(|s| s.ram >= 90.0).count() as f32 * 100.0 / n;
    Some((round1(avg), round1(high), v.len() as u32))
}

/// Copia de las muestras (para el análisis de fugas y de disco).
pub fn samples() -> Vec<Sample> {
    SAMPLES.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
pub fn perf_history() -> Vec<Sample> {
    SAMPLES.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_pressure_needs_two_hours_of_samples() {
        let s = |ram: f32| Sample { t: 0, cpu: 0.0, ram, disk: 0.0, ..Default::default() };
        assert_eq!(pressure_of(&vec![s(95.0); 60]), None);
        let mut v = vec![s(60.0); 150];
        v.extend(vec![s(95.0); 50]);
        let (avg, high, n) = pressure_of(&v).unwrap();
        assert_eq!(n, 200);
        assert_eq!(high, 25.0);
        assert!((avg - 68.8).abs() < 0.2, "{avg}");
    }

    #[test]
    fn keeps_seven_days() {
        let now = 1_800_000_000;
        let mut v: Vec<Sample> = [now - KEEP_SECS - 60, now - KEEP_SECS + 60, now - 60, now + 3600]
            .iter()
            .map(|&t| Sample { t, cpu: 1.0, ram: 1.0, disk: 1.0, ..Default::default() })
            .collect();
        trim(&mut v, now);
        assert_eq!(v.iter().map(|s| s.t).collect::<Vec<_>>(), vec![now - KEEP_SECS + 60, now - 60]);
    }
}
