//! ¿Por qué no responde? Mira a qué está esperando un programa con la cadena de
//! espera de Windows (Wait Chain Traversal, la que usa el Monitor de recursos)
//! y la junta con lo que se ve desde fuera: las unidades de red que no
//! contestan, a las que apunta la línea de comandos del programa, y sus
//! conexiones. Solo lee: nada se cambia; cerrar o reconectar se hace con los
//! comandos que ya existen (`kill_process`, `reconnect_network_drive`).

use serde::Serialize;

/// Tipo de lo que hay en un eslabón de la cadena.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeKind {
    CriticalSection,
    SendMessage,
    Mutex,
    Alpc,
    Com,
    ThreadWait,
    ProcessWait,
    Thread,
    Unknown,
    Network,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub blocked: bool,
    pub pid: u32,
}

/// La cadena de espera de un hilo.
#[derive(Clone, Debug, Default)]
pub struct Chain {
    pub nodes: Vec<Node>,
    pub cycle: bool,
}

/// Una razón por la que el programa espera, en palabras.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reason {
    /// network · deadlock · antivirus · program · service · lock
    pub kind: &'static str,
    pub text: String,
    pub threads: usize,
}

/// Programas de seguridad: si el programa espera a uno de ellos, suele ser que lo está analizando.
const SECURITY: &[&str] = &["msmpeng", "mssense", "sense", "avast", "avgsvc", "sophos", "savservice", "mcshield", "ccsvchst", "bdagent", "eset", "ekrn", "cylance", "csfalcon", "mbamservice", "wrsa", "cb"];

fn is_security(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let n = n.trim_end_matches(".exe");
    SECURITY.iter().any(|s| n == *s || (s.len() > 4 && n.contains(s)))
}

/// Junta las cadenas de todos los hilos en pocas razones. `name_of` da el nombre del programa de un PID.
pub fn reasons(chains: &[Chain], own_pid: u32, name_of: &dyn Fn(u32) -> String) -> Vec<Reason> {
    let mut out: Vec<Reason> = Vec::new();
    let mut add = |kind: &'static str, text: String| {
        match out.iter_mut().find(|r| r.kind == kind && r.text == text) {
            Some(r) => r.threads += 1,
            None => out.push(Reason { kind, text, threads: 1 }),
        }
    };
    for c in chains {
        // Una cadena de un solo eslabón (o cuyo hilo corre) no espera a nada.
        if c.nodes.len() < 2 || !c.nodes.iter().any(|n| n.blocked) {
            continue;
        }
        if c.cycle {
            add("deadlock", "Dos partes del programa se esperan una a la otra (interbloqueo): esperar no lo arregla.".into());
            continue;
        }
        let other = c.nodes.iter().skip(1).find(|n| n.kind == NodeKind::Thread && n.pid != own_pid && n.pid != 0);
        if c.nodes.iter().any(|n| n.kind == NodeKind::Network) {
            add("network", "Espera una respuesta de la red (un archivo o un servidor).".into());
        } else if let Some(o) = other {
            let name = name_of(o.pid);
            if is_security(&name) {
                add("antivirus", format!("Espera al antivirus ({name}), que debe de estar analizando algo."));
            } else if c.nodes.iter().any(|n| n.kind == NodeKind::SendMessage) {
                add("program", format!("Espera a que conteste «{name}», que no responde."));
            } else {
                add("program", format!("Espera a «{name}»."));
            }
        } else if c.nodes.iter().any(|n| n.kind == NodeKind::ProcessWait) {
            add("program", "Espera a que termine otro proceso.".into());
        } else if c.nodes.iter().any(|n| matches!(n.kind, NodeKind::Com | NodeKind::Alpc)) {
            add("service", "Espera a un servicio de Windows (COM o ALPC).".into());
        } else if c.nodes.iter().any(|n| matches!(n.kind, NodeKind::CriticalSection | NodeKind::Mutex)) {
            add("lock", "Espera un bloqueo interno: otra parte del propio programa lo tiene.".into());
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.threads));
    out
}

/// Una unidad o ruta de red que no contesta y a la que el programa apunta.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetTarget {
    /// «Z:» si es una unidad.
    pub drive: String,
    /// `\\servidor\recurso`
    pub unc: String,
    pub host: String,
    /// El servidor contesta ahora mismo.
    pub reachable: bool,
    /// El programa tiene un archivo o una carpeta de ahí en su línea de comandos.
    pub used: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    /// network · deadlock · antivirus · program · service · lock · busy · hidden
    pub kind: &'static str,
    pub text: String,
}

/// La frase que se enseña, a partir de lo que se ha visto.
pub fn verdict(name: &str, hung: bool, reasons: &[Reason], nets: &[NetTarget], seen: usize) -> Verdict {
    let program = if name.to_ascii_lowercase().ends_with(".exe") { &name[..name.len() - 4] } else { name };
    let down: Vec<&NetTarget> = nets.iter().filter(|n| !n.reachable).collect();
    let used: Vec<&&NetTarget> = down.iter().filter(|n| n.used).collect();
    let label = |n: &NetTarget| if n.drive.is_empty() { n.unc.clone() } else { format!("la unidad {} ({})", n.drive, n.unc) };
    if let Some(n) = used.first() {
        return Verdict { kind: "network", text: format!("{program} espera a {}, que no contesta. No está roto: está esperando.", label(n)) };
    }
    if reasons.iter().any(|r| r.kind == "network") {
        if let Some(n) = down.first() {
            return Verdict { kind: "network", text: format!("{program} espera una respuesta de la red y {} no contesta: es lo más probable.", label(n)) };
        }
        return Verdict { kind: "network", text: format!("{program} espera una respuesta de la red (un archivo o un servidor) y no llega.") };
    }
    if let Some(r) = reasons.first() {
        let kind = r.kind;
        let text = if kind == "deadlock" { format!("{program} está bloqueado por dentro. {}", r.text) } else { format!("{program} — {}", r.text) };
        return Verdict { kind, text };
    }
    if let Some(n) = down.first() {
        return Verdict { kind: "network", text: format!("{program} no parece esperar a nada concreto, pero {} no contesta: puede ser la causa si el programa usa esa unidad.", label(n)) };
    }
    if seen == 0 {
        return Verdict { kind: "hidden", text: "Windows no deja ver este programa. Abre AdminOps como administrador y vuelve a mirar.".into() };
    }
    let busy = if hung { "Está ocupado: no se ve que espere a nada. Dale un minuto; si sigue igual, ciérralo." } else { "Ahora no se ve que espere a nada: responde con normalidad." };
    Verdict { kind: "busy", text: busy.into() }
}

/// Rutas con letra de unidad o de red dentro de una línea de comandos.
pub fn paths_in(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        // También `/open:"Z:\x"` o `--file=Z:\x`: se busca desde el primer «X:\» o «\\».
        let b = a.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let drive = i + 2 < b.len() && b[i].is_ascii_alphabetic() && b[i + 1] == b':' && b[i + 2] == b'\\' && (i == 0 || !b[i - 1].is_ascii_alphanumeric());
            // Solo bytes: `a[i..]` en mitad de una letra con tilde haría saltar el programa.
            // Las comprobaciones son de bytes ASCII, que nunca caen dentro de un carácter UTF-8,
            // así que cuando son ciertas, `i` es siempre un límite de carácter válido.
            let unc = b[i] == b'\\' && b.get(i + 1) == Some(&b'\\') && (i == 0 || !b[i - 1].is_ascii_alphanumeric());
            if drive || unc {
                out.push(a[i..].trim_matches('"').to_string());
                break;
            }
            i += 1;
        }
    }
    out
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub remote: String,
    pub port: u16,
    pub state: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WaitReport {
    pub pid: u32,
    pub name: String,
    /// Windows marca su ventana como «No responde».
    pub hung: bool,
    pub threads: usize,
    /// Hilos cuya cadena se pudo leer.
    pub readable: usize,
    pub reasons: Vec<Reason>,
    pub nets: Vec<NetTarget>,
    pub connections: Vec<Connection>,
    pub verdict: Verdict,
    /// Letra que se puede reconectar con un clic (la primera unidad caída que usa el programa).
    pub reconnect: Option<String>,
}

#[cfg(windows)]
mod win {
    use super::{Chain, Node, NodeKind};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::Debug::*;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32};

    pub fn thread_ids(pid: u32) -> Vec<u32> {
        let mut out = Vec::new();
        // SAFETY: instantánea de hilos; se recorre con la estructura inicializada y se cierra.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut e = THREADENTRY32 { dwSize: std::mem::size_of::<THREADENTRY32>() as u32, ..Default::default() };
            if Thread32First(snap, &mut e) != 0 {
                loop {
                    if e.th32OwnerProcessID == pid {
                        out.push(e.th32ThreadID);
                    }
                    if Thread32Next(snap, &mut e) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
        out
    }

    fn kind_of(t: i32) -> NodeKind {
        // Se compara con `==`: son constantes de windows-sys, no patrones.
        let table = [
            (WctCriticalSectionType, NodeKind::CriticalSection),
            (WctSendMessageType, NodeKind::SendMessage),
            (WctMutexType, NodeKind::Mutex),
            (WctAlpcType, NodeKind::Alpc),
            (WctComType, NodeKind::Com),
            (WctComActivationType, NodeKind::Com),
            (WctThreadWaitType, NodeKind::ThreadWait),
            (WctProcessWaitType, NodeKind::ProcessWait),
            (WctThreadType, NodeKind::Thread),
            (WctSocketIoType, NodeKind::Network),
            (WctSmbIoType, NodeKind::Network),
        ];
        table.iter().find(|(k, _)| *k == t).map_or(NodeKind::Unknown, |(_, v)| *v)
    }

    /// Las cadenas de espera de todos los hilos del proceso (las que Windows deja leer).
    pub fn chains(pid: u32) -> (usize, Vec<Chain>) {
        let tids = thread_ids(pid);
        let mut out = Vec::new();
        // SAFETY: sesión síncrona de la cadena de espera; cada llamada recibe un
        // arreglo con la capacidad que declara y la sesión se cierra al terminar.
        unsafe {
            let session = OpenThreadWaitChainSession(0, None);
            if session.is_null() {
                return (tids.len(), out);
            }
            for tid in &tids {
                let mut count = WCT_MAX_NODE_COUNT;
                let mut nodes: [WAITCHAIN_NODE_INFO; WCT_MAX_NODE_COUNT as usize] = std::mem::zeroed();
                let mut cycle = 0;
                // WCT_OUT_OF_PROC_FLAG: sigue la espera hasta otros procesos (el servidor, el antivirus).
                let ok = GetThreadWaitChain(session, 0, WCT_OUT_OF_PROC_FLAG | WCT_OUT_OF_PROC_COM_FLAG | WCT_OUT_OF_PROC_CS_FLAG | WCT_NETWORK_IO_FLAG, *tid, &mut count, nodes.as_mut_ptr(), &mut cycle);
                if ok == 0 {
                    continue;
                }
                let list = nodes[..(count as usize).min(nodes.len())]
                    .iter()
                    .map(|n| {
                        let kind = kind_of(n.ObjectType);
                        let blocked = n.ObjectStatus == WctStatusBlocked;
                        let pid = if kind == NodeKind::Thread { n.Anonymous.ThreadObject.ProcessId } else { 0 };
                        Node { kind, blocked, pid }
                    })
                    .collect();
                out.push(Chain { nodes: list, cycle: cycle != 0 });
            }
            CloseThreadWaitChainSession(session);
        }
        (tids.len(), out)
    }

    /// ¿Alguna ventana de este proceso está marcada «No responde»?
    pub fn is_hung(pid: u32) -> bool {
        use windows_sys::Win32::Foundation::{HWND, LPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsHungAppWindow, IsWindowVisible};
        struct Probe {
            pid: u32,
            hung: bool,
        }
        unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> i32 {
            // SAFETY: lparam es el puntero al `Probe` de la llamada, vivo durante EnumWindows.
            unsafe {
                let probe = &mut *(lparam as *mut Probe);
                let mut owner = 0u32;
                GetWindowThreadProcessId(hwnd, &mut owner);
                if owner == probe.pid && IsWindowVisible(hwnd) != 0 && IsHungAppWindow(hwnd) != 0 {
                    probe.hung = true;
                    return 0;
                }
            }
            1
        }
        let mut probe = Probe { pid, hung: false };
        // SAFETY: el puntero apunta a `probe`, que sigue vivo durante la llamada.
        unsafe {
            EnumWindows(Some(each), &mut probe as *mut Probe as LPARAM);
        }
        probe.hung
    }
}

#[cfg(not(windows))]
mod win {
    use super::Chain;
    pub fn chains(_: u32) -> (usize, Vec<Chain>) {
        (0, Vec::new())
    }
    pub fn is_hung(_: u32) -> bool {
        false
    }
}

/// Conexiones del proceso con otro equipo (las que se quedan a medias son la pista).
fn connections(pid: u32) -> Vec<Connection> {
    let script = format!(
        "$r = @(Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.RemoteAddress -notin @('127.0.0.1','::1','0.0.0.0','::') -and \"$($_.State)\" -ne 'Listen' }} | Select-Object -First 12 | ForEach-Object {{ [pscustomobject]@{{ remote = \"$($_.RemoteAddress)\"; port = [int]$_.RemotePort; state = \"$($_.State)\" }} }})\nConvertTo-Json -InputObject $r -Compress"
    );
    let Ok(out) = crate::ps::powershell(&script) else { return Vec::new() };
    parse_connections(&out)
}

pub fn parse_connections(out: &str) -> Vec<Connection> {
    let t = out.trim();
    if t.is_empty() || t == "null" {
        return Vec::new();
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        remote: String,
        port: u16,
        state: String,
    }
    let list: Vec<Raw> = serde_json::from_str::<Vec<Raw>>(t).or_else(|_| serde_json::from_str::<Raw>(t).map(|r| vec![r])).unwrap_or_default();
    list.into_iter().map(|r| Connection { remote: r.remote, port: r.port, state: r.state }).collect()
}

/// Mira a qué espera el programa. `name` debe ser el del PID ahora (los PID se reutilizan).
#[tauri::command(async)]
pub fn why_not_responding(pid: u32, name: String) -> Result<WaitReport, String> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always));
    let p = sys.process(Pid::from_u32(pid)).ok_or("El programa ya no existe.")?;
    let current = p.name().to_string_lossy().into_owned();
    if !current.eq_ignore_ascii_case(&name) {
        return Err(format!("El PID {pid} ahora es «{current}», no «{name}». Actualiza la lista."));
    }
    let args: Vec<String> = p.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let names: std::collections::HashMap<u32, String> = sys.processes().iter().map(|(k, v)| (k.as_u32(), v.name().to_string_lossy().into_owned())).collect();

    let (threads, chains) = win::chains(pid);
    let why = reasons(&chains, pid, &|x| names.get(&x).cloned().unwrap_or_else(|| format!("PID {x}")));
    let hung = win::is_hung(pid);

    // Unidades de red que Windows recuerda y si su servidor contesta.
    let used_paths = paths_in(&args);
    let known = crate::shares::remembered();
    let nets: Vec<NetTarget> = std::thread::scope(|s| {
        let jobs: Vec<_> = known
            .iter()
            .map(|(letter, unc)| {
                let used_paths = &used_paths;
                s.spawn(move || {
                    let host = crate::shares::parse_unc(unc).map(|(h, _)| h.to_string()).unwrap_or_default();
                    let reachable = host.is_empty() || crate::shares::answers(&host);
                    let prefix = format!("{letter}:\\").to_ascii_lowercase();
                    let used = used_paths.iter().any(|p| p.to_ascii_lowercase().starts_with(&prefix) || p.to_ascii_lowercase().starts_with(&unc.to_ascii_lowercase()));
                    NetTarget { drive: format!("{letter}:"), unc: unc.clone(), host, reachable, used }
                })
            })
            .collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    });
    // Solo interesan las que no contestan (o las que usa el programa).
    let nets: Vec<NetTarget> = nets.into_iter().filter(|n| !n.reachable || n.used).collect();

    let verdict = verdict(&current, hung, &why, &nets, chains.len());
    let reconnect = nets.iter().find(|n| !n.reachable && n.used).or_else(|| nets.iter().find(|n| !n.reachable)).map(|n| n.drive.clone());
    let report = WaitReport { pid, name: current, hung, threads, readable: chains.len(), reasons: why, nets, connections: connections(pid), verdict, reconnect };
    // Ni el nombre del usuario ni sus carpetas salen en pantalla.
    Ok(WaitReport {
        nets: report.nets.into_iter().map(|n| NetTarget { unc: crate::escalate::scrub(&n.unc), ..n }).collect(),
        ..report
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(kind: NodeKind, blocked: bool, pid: u32) -> Node {
        Node { kind, blocked, pid }
    }
    fn names(p: u32) -> String {
        match p {
            10 => "MsMpEng.exe".into(),
            20 => "servidor.exe".into(),
            _ => format!("PID {p}"),
        }
    }

    #[test]
    fn un_hilo_que_corre_no_espera() {
        let c = Chain { nodes: vec![n(NodeKind::Thread, false, 1)], cycle: false };
        assert!(reasons(&[c], 1, &names).is_empty());
    }

    #[test]
    fn espera_al_antivirus_o_a_otro_programa() {
        let av = Chain { nodes: vec![n(NodeKind::Thread, true, 1), n(NodeKind::Alpc, true, 0), n(NodeKind::Thread, true, 10)], cycle: false };
        let other = Chain { nodes: vec![n(NodeKind::Thread, true, 1), n(NodeKind::SendMessage, true, 0), n(NodeKind::Thread, true, 20)], cycle: false };
        let r = reasons(&[av.clone(), av, other], 1, &names);
        assert_eq!(r[0].kind, "antivirus");
        assert_eq!(r[0].threads, 2);
        assert!(r[1].text.contains("servidor.exe"));
    }

    #[test]
    fn red_e_interbloqueo() {
        let net = Chain { nodes: vec![n(NodeKind::Thread, true, 1), n(NodeKind::Network, true, 0)], cycle: false };
        let dead = Chain { nodes: vec![n(NodeKind::Thread, true, 1), n(NodeKind::Mutex, true, 0), n(NodeKind::Thread, true, 1)], cycle: true };
        let r = reasons(&[net, dead], 1, &names);
        assert!(r.iter().any(|x| x.kind == "network") && r.iter().any(|x| x.kind == "deadlock"));
    }

    #[test]
    fn la_frase_nombra_la_unidad_caida() {
        let net = NetTarget { drive: "Z:".into(), unc: r"\\servidor\conta".into(), host: "servidor".into(), reachable: false, used: true };
        let v = verdict("EXCEL.EXE", true, &[], &[net], 12);
        assert_eq!(v.kind, "network");
        assert!(v.text.contains("EXCEL espera a la unidad Z:") && v.text.contains(r"\\servidor\conta"));
    }

    #[test]
    fn sin_pistas_dice_que_esta_ocupado_o_que_no_se_ve() {
        assert_eq!(verdict("a.exe", true, &[], &[], 5).kind, "busy");
        assert_eq!(verdict("a.exe", true, &[], &[], 0).kind, "hidden");
        let r = vec![Reason { kind: "program", text: "Espera a «x.exe».".into(), threads: 1 }];
        assert!(verdict("a.exe", true, &r, &[], 5).text.starts_with("a — Espera a"));
    }

    #[test]
    fn rutas_de_la_linea_de_comandos() {
        let args = vec!["EXCEL.EXE".into(), r#"/e Z:\Conta\Presu.xlsx"#.into(), r"\\srv\conta\a.xlsx".into(), "--flag=1".into()];
        let p = paths_in(&args);
        assert_eq!(p.len(), 2);
        assert!(p[0].starts_with(r"Z:\Conta"));
    }

    #[test]
    fn lee_las_conexiones() {
        assert!(parse_connections("").is_empty());
        assert_eq!(parse_connections(r#"{"remote":"10.0.0.5","port":445,"state":"SynSent"}"#).len(), 1);
        assert_eq!(parse_connections(r#"[{"remote":"10.0.0.5","port":445,"state":"Established"},{"remote":"1.1.1.1","port":443,"state":"CloseWait"}]"#)[1].port, 443);
    }
}
