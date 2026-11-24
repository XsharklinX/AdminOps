//! El hilo que apunta, cada segundo y mientras se graba, cómo va el equipo: procesador, disco y
//! programas que no responden. Reutiliza lo que ya hay: el contador de disco de `perfhistory`, la
//! lista de ventanas colgadas de `waitchain` y el reconocedor de programas de `netusage`.
//!
//! Nunca se guarda el título de una ventana (puede llevar nombres o documentos): solo el nombre del programa.
//!
//! La medida (`Sampler`) no sabe de dónde se la llama: la grabación normal la guarda en una lista que crece hasta
//! parar (`run`) y Vigilar la guarda en un búfer circular y además mira si pasó algo (`vigil`).

use super::analysis::Tick;
use super::Started;
use crate::netusage::Idents;
use crate::perfhistory::DiskBusy;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Solo se averigua quién es el programa cuando el valor va cerca del umbral (cuesta una llamada por proceso).
const LOOK_AT_CPU: f32 = 70.0;
const LOOK_AT_DISK: f32 = 80.0;
/// Menos que esto de lectura y escritura en un segundo no es «el que más escribía».
const MIN_IO_BYTES: u64 = 128 * 1024;

fn refresh(sys: &mut System) {
    sys.refresh_cpu_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cpu().with_disk_usage().with_exe(UpdateKind::Never));
}

/// Un programa con una ventana «No responde».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HungApp {
    /// El nombre del archivo en minúsculas (`excel.exe`): lo que identifica al programa.
    pub key: String,
    /// El nombre que se le enseña al técnico («Microsoft Excel»).
    pub name: String,
}

/// Lo medido en un segundo: lo que se guarda y los programas colgados con su clave.
pub struct Sampled {
    pub tick: Tick,
    pub hung: Vec<HungApp>,
}

/// Mide un segundo tras otro.
pub struct Sampler {
    sys: System,
    nets: sysinfo::Networks,
    last_net: Instant,
    cores: f32,
    busy: Option<DiskBusy>,
    idents: Idents,
    own: u32,
}

impl Sampler {
    /// Toma la primera lectura de referencia: los porcentajes salen de comparar con la anterior.
    pub fn new() -> Sampler {
        let mut sys = System::new();
        refresh(&mut sys);
        let cores = sys.cpus().len().max(1) as f32;
        let busy = DiskBusy::new();
        let _ = busy.as_ref().and_then(DiskBusy::read);
        let nets = sysinfo::Networks::new_with_refreshed_list();
        Sampler { sys, nets, last_net: Instant::now(), cores, busy, idents: Idents::new(), own: std::process::id() }
    }

    /// Mide cada segundo a partir de `t0` y llama a `each` con cada medida (la primera, un segundo después de
    /// `t0`, con `t` = 0) hasta que se pida parar. El segundo a medias no se apunta.
    pub fn run_loop(&mut self, t0: Instant, stop: &AtomicBool, mut each: impl FnMut(Sampled)) {
        refresh(&mut self.sys);
        let mut n: u64 = 0;
        loop {
            n += 1;
            let deadline = t0 + Duration::from_secs(n);
            loop {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                std::thread::sleep((deadline - now).min(Duration::from_millis(50)));
            }
            each(self.sample((n - 1) as u32));
        }
    }

    /// Mide ahora: lo ocurrido desde la medida anterior.
    pub fn sample(&mut self, t: u32) -> Sampled {
        let (own, cores) = (self.own, self.cores);
        refresh(&mut self.sys);
        let cpu = self.sys.global_cpu_usage();
        let disk = self.busy.as_ref().and_then(DiskBusy::read);
        self.sys.refresh_memory();
        let total = self.sys.total_memory();
        let mem = (total > 0).then(|| (self.sys.used_memory() as f64 * 100.0 / total as f64) as f32);
        // Lo que pasó por todas las tarjetas de red desde la medida anterior, por segundo.
        self.nets.refresh(true);
        let secs = self.last_net.elapsed().as_secs_f64().max(0.2);
        self.last_net = Instant::now();
        let bytes: u64 = self.nets.values().map(|n| n.received() + n.transmitted()).sum();
        let net = Some((bytes as f64 / secs) as u64);

        let cpu_top = (cpu >= LOOK_AT_CPU)
            .then(|| {
                self.sys
                    .processes()
                    .iter()
                    .filter(|(pid, p)| pid.as_u32() != own && pid.as_u32() != 0 && p.name().to_string_lossy() != "System Idle Process")
                    .map(|(pid, p)| (p.cpu_usage() / cores, pid.as_u32()))
                    .filter(|(c, _)| *c >= 5.0)
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, pid)| self.idents.identify(pid).name)
            })
            .flatten();
        let io_top = disk.filter(|d| *d >= LOOK_AT_DISK).and_then(|_| {
            self.sys
                .processes()
                .iter()
                .filter(|(pid, _)| pid.as_u32() != own && pid.as_u32() != 0)
                .map(|(pid, p)| {
                    let d = p.disk_usage();
                    (d.read_bytes + d.written_bytes, pid.as_u32())
                })
                .filter(|(b, _)| *b >= MIN_IO_BYTES)
                .max_by_key(|(b, _)| *b)
                .map(|(_, pid)| self.idents.identify(pid).name)
        });

        let mut hung_apps: Vec<HungApp> = crate::waitchain::hung_pids()
            .into_iter()
            .filter(|pid| *pid != own)
            .map(|pid| {
                let id = self.idents.identify(pid);
                HungApp { key: id.key, name: id.name }
            })
            .collect();
        hung_apps.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.key.cmp(&b.key)));
        hung_apps.dedup_by(|a, b| a.name == b.name);
        let hung: Vec<String> = hung_apps.iter().map(|h| h.name.clone()).collect();
        Sampled { tick: Tick { t, cpu, disk, cpu_top, io_top, hung, mem, net }, hung: hung_apps }
    }
}

/// Mide hasta que se pida parar. Los segundos se cuentan desde el primer fotograma del vídeo.
pub fn run(first: Arc<Mutex<Option<Started>>>, stop: Arc<AtomicBool>) -> Vec<Tick> {
    // COM propio: los contadores de disco y lo que lee de los procesos pueden usarlo por dentro. Sin
    // iniciarlo aquí, este hilo se apoyaba en el de la captura, que lo cierra al terminar de grabar, y lo
    // que el vigilante aún tenía abierto se quedaba colgando (cierre con 0xC0000005 al parar).
    let _com = ComGuard::new();
    run_inner(first, stop)
}

/// COM multihilo para este hilo mientras vive; se cierra al soltarlo.
pub struct ComGuard(bool);
impl ComGuard {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            // SAFETY: se empareja con CoUninitialize en `drop`, en este mismo hilo.
            ComGuard(unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED) }.is_ok())
        }
        #[cfg(not(windows))]
        ComGuard(false)
    }
}
impl Drop for ComGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.0 {
            // SAFETY: CoInitializeEx dio bien en este hilo.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

fn run_inner(first: Arc<Mutex<Option<Started>>>, stop: Arc<AtomicBool>) -> Vec<Tick> {
    let mut ticks: Vec<Tick> = Vec::new();
    let mut sampler = Sampler::new();
    let t0 = loop {
        if stop.load(Ordering::SeqCst) {
            return ticks;
        }
        if let Some(s) = *first.lock().unwrap_or_else(|e| e.into_inner()) {
            break s.at;
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    sampler.run_loop(t0, &stop, |s| ticks.push(s.tick));
    ticks
}
