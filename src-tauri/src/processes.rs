//! Procesos: listado tipo Administrador de tareas y finalizar proceso/árbol.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};
use tauri::State;

/// Finalizarlos provoca un pantallazo azul o cierra la sesión: no se permite.
const CRITICAL: &[&str] = &[
    "system",
    "registry",
    "memory compression",
    "secure system",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
];

/// Se pueden finalizar, pero con consecuencias que conviene avisar.
const SENSITIVE: &[(&str, &str)] = &[
    ("svchost.exe", "Aloja servicios de Windows: finalizarlo puede detener la red, el audio u otras funciones."),
    ("explorer.exe", "Es el escritorio y la barra de tareas: desaparecerán unos segundos hasta que Windows lo relance."),
    ("dwm.exe", "Compone la pantalla: parpadeará mientras Windows lo reinicia."),
    ("sihost.exe", "Gestiona la sesión del escritorio; pueden dejar de responder el menú Inicio y las notificaciones."),
    ("spoolsv.exe", "Es la cola de impresión: se cancelarán los trabajos en curso."),
    ("audiodg.exe", "Motor de audio: el sonido se cortará un momento."),
    ("fontdrvhost.exe", "Carga las fuentes: algunas apps pueden mostrar texto incorrecto."),
    ("ctfmon.exe", "Controla el teclado e idiomas de entrada."),
];

pub struct ProcessState {
    inner: Mutex<Collector>,
}

struct Collector {
    sys: System,
    users: Users,
    last: Instant,
}

impl ProcessState {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Collector { sys: System::new(), users: Users::new_with_refreshed_list(), last: Instant::now() }) }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessView {
    pid: u32,
    parent: Option<u32>,
    name: String,
    exe: Option<String>,
    user: Option<String>,
    /// % del total del equipo (como el Administrador de tareas).
    cpu: f32,
    memory: u64,
    disk_read_per_sec: u64,
    disk_write_per_sec: u64,
    /// Segundos epoch.
    started: u64,
    /// none | sensitive | critical | self
    protection: &'static str,
    note: Option<&'static str>,
}

fn classify(name: &str, pid: u32, own: &HashSet<u32>) -> (&'static str, Option<&'static str>) {
    let lower = name.to_lowercase();
    if pid == 0 || pid == 4 || CRITICAL.contains(&lower.as_str()) {
        return ("critical", Some("Proceso crítico de Windows: finalizarlo provocaría un pantallazo azul o cerraría la sesión."));
    }
    if own.contains(&pid) {
        return ("self", Some("Es parte de AdminOps."));
    }
    match SENSITIVE.iter().find(|(n, _)| *n == lower) {
        Some((_, why)) => ("sensitive", Some(why)),
        None => ("none", None),
    }
}

/// PIDs de AdminOps y de todo lo que cuelga de él (WebView2, PowerShell…).
fn own_tree(sys: &System) -> HashSet<u32> {
    let me = std::process::id();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, p) in sys.processes() {
        if let Some(parent) = p.parent() {
            children.entry(parent.as_u32()).or_default().push(pid.as_u32());
        }
    }
    let mut own = HashSet::from([me]);
    let mut stack = vec![me];
    while let Some(p) = stack.pop() {
        for c in children.get(&p).into_iter().flatten() {
            if own.insert(*c) {
                stack.push(*c);
            }
        }
    }
    own
}

#[tauri::command(async)]
pub fn list_processes(state: State<'_, ProcessState>) -> Vec<ProcessView> {
    let mut guard = state.inner.lock().unwrap();
    let c = &mut *guard;
    c.sys.refresh_cpu_usage();
    c.sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_disk_usage()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_user(UpdateKind::OnlyIfNotSet),
    );
    let secs = c.last.elapsed().as_secs_f64().max(0.001);
    c.last = Instant::now();
    let cores = c.sys.cpus().len().max(1) as f32;
    let own = own_tree(&c.sys);

    c.sys
        .processes()
        .iter()
        .filter(|(pid, _)| pid.as_u32() != 0)
        .map(|(pid, p)| {
            let name = p.name().to_string_lossy().into_owned();
            let (protection, note) = classify(&name, pid.as_u32(), &own);
            let disk = p.disk_usage();
            ProcessView {
                pid: pid.as_u32(),
                parent: p.parent().map(|x| x.as_u32()),
                exe: p.exe().map(|e| e.display().to_string()),
                user: p.user_id().and_then(|u| c.users.get_user_by_id(u)).map(|u| u.name().to_string()),
                cpu: p.cpu_usage() / cores,
                memory: p.memory(),
                disk_read_per_sec: (disk.read_bytes as f64 / secs) as u64,
                disk_write_per_sec: (disk.written_bytes as f64 / secs) as u64,
                started: p.start_time(),
                protection,
                note,
                name,
            }
        })
        .collect()
}

/// Finaliza un proceso (o su árbol). `name` debe coincidir con el proceso
/// actual de ese PID: Windows reutiliza los PID y podríamos cerrar otro.
#[tauri::command(async)]
pub fn kill_process(
    pid: u32,
    name: String,
    tree: bool,
    procs: State<'_, ProcessState>,
    tweaks: State<'_, TweakState>,
) -> Result<(), String> {
    let (current, protection) = {
        let mut c = procs.inner.lock().unwrap();
        c.sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        let own = own_tree(&c.sys);
        let p = c.sys.process(Pid::from_u32(pid)).ok_or("El proceso ya no existe.")?;
        let current = p.name().to_string_lossy().into_owned();
        let protection = classify(&current, pid, &own).0;
        (current, protection)
    };
    if !current.eq_ignore_ascii_case(&name) {
        return Err(format!("El PID {pid} ahora es «{current}», no «{name}». Actualiza la lista."));
    }
    match protection {
        "critical" => return Err(format!("{name} es un proceso crítico de Windows y no se puede finalizar.")),
        "self" => return Err("Ese proceso es parte de AdminOps.".into()),
        _ => {}
    }

    let title = format!("Finalizar {}: {name} (PID {pid})", if tree { "árbol" } else { "proceso" });
    let result = if tree {
        crate::ps::exec("taskkill.exe", &["/T", "/F", "/PID", &pid.to_string()]).map(|_| ())
    } else {
        let c = procs.inner.lock().unwrap();
        match c.sys.process(Pid::from_u32(pid)) {
            Some(p) if p.kill() => Ok(()),
            Some(_) => Err(if crate::elevation::is_elevated() {
                "Windows no permitió finalizarlo: es un proceso protegido.".into()
            } else {
                "Acceso denegado: ejecuta AdminOps como administrador para finalizar este proceso.".into()
            }),
            None => Ok(()), // terminó mientras tanto
        }
    };
    tweaks.record(Op::Run, &title, &result);
    log::info!("{title}: {result:?}");
    result
}

/// Abre la carpeta del ejecutable con el archivo seleccionado.
#[tauri::command]
pub fn open_process_location(pid: u32, procs: State<'_, ProcessState>) -> Result<(), String> {
    let exe = {
        let mut c = procs.inner.lock().unwrap();
        c.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        c.sys.process(Pid::from_u32(pid)).and_then(|p| p.exe().map(|e| e.to_path_buf()))
    }
    .ok_or("No se conoce la ruta de este proceso (suele requerir administrador).")?;
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", exe.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_processes() {
        let own = HashSet::from([1234]);
        assert_eq!(classify("csrss.exe", 700, &own).0, "critical");
        assert_eq!(classify("System", 4, &own).0, "critical");
        assert_eq!(classify("svchost.exe", 900, &own).0, "sensitive");
        assert_eq!(classify("adminops.exe", 1234, &own).0, "self");
        assert_eq!(classify("chrome.exe", 5000, &own).0, "none");
    }

    #[test]
    fn kills_a_real_child_process() {
        // Proceso de prueba propio, finalizado con la misma API que usa la app.
        let mut child = std::process::Command::new("ping.exe")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(child.id())]), true);
        assert!(sys.process(Pid::from_u32(child.id())).unwrap().kill());
        let status = child.wait().unwrap();
        assert!(!status.success());
    }
}
