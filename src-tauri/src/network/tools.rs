//! Red avanzada: ping y traza de ruta en vivo (ICMP nativo, no depende del
//! idioma de ping.exe/tracert.exe), puertos en uso, DNS por adaptador y el
//! archivo hosts.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};

static STOP: AtomicBool = AtomicBool::new(false);

const IP_SUCCESS: u32 = 0;
const IP_TTL_EXPIRED_TRANSIT: u32 = 11013;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    /// Número de ping o salto.
    seq: u32,
    /// Quién respondió (en una traza, el router de ese salto).
    from: Option<String>,
    ms: Option<u32>,
    /// ok | timeout | unreachable | error
    status: &'static str,
    /// Solo en la traza: se llegó al destino.
    reached: bool,
}

/// Resuelve un nombre o IP a IPv4 (el ICMP de Windows usado aquí es IPv4).
fn resolve(host: &str) -> Result<Ipv4Addr, String> {
    let h = host.trim();
    if h.is_empty() || h.len() > 253 || h.contains(char::is_whitespace) {
        return Err("Escribe un nombre (google.com) o una IP (8.8.8.8).".into());
    }
    if let Ok(ip) = h.parse::<Ipv4Addr>() {
        return Ok(ip);
    }
    (h, 0)
        .to_socket_addrs()
        .map_err(|_| format!("No se pudo resolver «{h}»: revisa el nombre o el DNS."))?
        .find_map(|a| match a.ip() {
            std::net::IpAddr::V4(v4) => Some(v4),
            _ => None,
        })
        .ok_or_else(|| format!("«{h}» no tiene dirección IPv4."))
}

/// Un eco ICMP con el TTL indicado. Devuelve (estado, quién respondió, ms).
#[cfg(windows)]
pub(crate) fn echo(addr: Ipv4Addr, ttl: u8, timeout_ms: u32) -> (u32, Option<Ipv4Addr>, u32) {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY, IP_OPTION_INFORMATION,
    };
    unsafe {
        let h = IcmpCreateFile();
        if h == INVALID_HANDLE_VALUE {
            return (u32::MAX, None, 0);
        }
        let data = [0x61u8; 32];
        let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8];
        let opts = IP_OPTION_INFORMATION { Ttl: ttl, ..Default::default() };
        let n = IcmpSendEcho(
            h,
            u32::from_ne_bytes(addr.octets()),
            data.as_ptr().cast(),
            data.len() as u16,
            &opts,
            reply.as_mut_ptr().cast(),
            reply.len() as u32,
            timeout_ms,
        );
        IcmpCloseHandle(h);
        if n == 0 {
            // Sin respuesta: el estado detallado lo da GetLastError, pero para
            // el técnico basta con "sin respuesta".
            return (11010, None, timeout_ms);
        }
        let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
        (r.Status, Some(Ipv4Addr::from(r.Address.to_ne_bytes())), r.RoundTripTime)
    }
}

#[cfg(not(windows))]
pub(crate) fn echo(_: Ipv4Addr, _: u8, _: u32) -> (u32, Option<Ipv4Addr>, u32) {
    (u32::MAX, None, 0)
}

fn status_name(code: u32) -> &'static str {
    match code {
        IP_SUCCESS => "ok",
        11010 => "timeout",
        11002..=11005 => "unreachable",
        _ => "error",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeStart {
    target: String,
    ip: String,
}

/// Ping continuo (hasta `count` o hasta pulsar Detener). Emite `net-ping` por cada respuesta.
#[tauri::command(async)]
pub fn start_ping(app: tauri::AppHandle, host: String, count: u32) -> Result<ProbeStart, String> {
    let ip = resolve(&host)?;
    STOP.store(false, Ordering::SeqCst);
    let count = count.clamp(1, 1000);
    std::thread::spawn(move || {
        for seq in 1..=count {
            if STOP.load(Ordering::SeqCst) {
                break;
            }
            let start = std::time::Instant::now();
            let (code, from, ms) = echo(ip, 128, 2000);
            let p = Probe { seq, from: from.map(|f| f.to_string()), ms: (code == IP_SUCCESS).then_some(ms), status: status_name(code), reached: code == IP_SUCCESS };
            let _ = app.emit("net-ping", p);
            // Un ping por segundo, como ping.exe.
            if let Some(rest) = std::time::Duration::from_secs(1).checked_sub(start.elapsed()) {
                std::thread::sleep(rest);
            }
        }
        let _ = app.emit("net-ping-done", ());
    });
    Ok(ProbeStart { target: host.trim().into(), ip: ip.to_string() })
}

/// Traza de ruta: hasta 30 saltos, tres intentos por salto. Emite `net-trace` por salto.
#[tauri::command(async)]
pub fn start_trace(app: tauri::AppHandle, host: String) -> Result<ProbeStart, String> {
    let ip = resolve(&host)?;
    STOP.store(false, Ordering::SeqCst);
    std::thread::spawn(move || {
        for ttl in 1..=30u8 {
            if STOP.load(Ordering::SeqCst) {
                break;
            }
            let mut best: Option<(u32, Option<Ipv4Addr>, u32)> = None;
            for _ in 0..3 {
                let r = echo(ip, ttl, 1500);
                if r.0 == IP_SUCCESS || r.0 == IP_TTL_EXPIRED_TRANSIT {
                    best = Some(r);
                    break;
                }
                best.get_or_insert(r);
            }
            let (code, from, ms) = best.unwrap_or((11010, None, 0));
            let answered = code == IP_SUCCESS || code == IP_TTL_EXPIRED_TRANSIT;
            let reached = code == IP_SUCCESS;
            let p = Probe {
                seq: ttl as u32,
                from: from.filter(|_| answered).map(|f| f.to_string()),
                ms: answered.then_some(ms),
                status: if answered { "ok" } else { status_name(code) },
                reached,
            };
            let _ = app.emit("net-trace", p);
            if reached {
                break;
            }
        }
        let _ = app.emit("net-trace-done", ());
    });
    Ok(ProbeStart { target: host.trim().into(), ip: ip.to_string() })
}

#[tauri::command]
pub fn stop_probe() {
    STOP.store(true, Ordering::SeqCst);
}

// ---------- Puertos en uso ----------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PortEntry {
    protocol: String,
    local_address: String,
    local_port: u32,
    remote_address: Option<String>,
    remote_port: Option<u32>,
    state: String,
    pid: u32,
    process: Option<String>,
}

const PORTS_SCRIPT: &str = r#"
$names = @{}
Get-Process -ErrorAction SilentlyContinue | ForEach-Object { $names[[int]$_.Id] = $_.ProcessName }
$tcp = Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object { $_.State -in 'Listen', 'Established' } | ForEach-Object {
  [pscustomobject]@{ protocol = 'TCP'; localAddress = "$($_.LocalAddress)"; localPort = [int]$_.LocalPort
    remoteAddress = if ($_.State -eq 'Established') { "$($_.RemoteAddress)" } else { $null }
    remotePort = if ($_.State -eq 'Established') { [int]$_.RemotePort } else { $null }
    state = "$($_.State)"; pid = [int]$_.OwningProcess; process = $names[[int]$_.OwningProcess] }
}
$udp = Get-NetUDPEndpoint -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ protocol = 'UDP'; localAddress = "$($_.LocalAddress)"; localPort = [int]$_.LocalPort
    remoteAddress = $null; remotePort = $null; state = 'Listen'; pid = [int]$_.OwningProcess; process = $names[[int]$_.OwningProcess] }
}
ConvertTo-Json -InputObject @(@($tcp) + @($udp)) -Compress
"#;

#[tauri::command(async)]
pub fn list_ports() -> Result<Vec<PortEntry>, String> {
    let out = crate::ps::powershell(PORTS_SCRIPT)?;
    let mut v: Vec<PortEntry> = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    v.sort_by(|a, b| (a.state != "Listen", a.local_port, &a.protocol).cmp(&(b.state != "Listen", b.local_port, &b.protocol)));
    Ok(v)
}

// ---------- DNS por adaptador ----------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DnsAdapter {
    pub index: u32,
    pub name: String,
    pub description: String,
    #[serde(rename = "virtual")]
    pub is_virtual: bool,
    pub dns: Vec<String>,
    /// DNS escritos a mano (si no, los da el router por DHCP).
    pub manual: bool,
}

const DNS_SCRIPT: &str = r#"
$r = @(Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq 'Up' } | ForEach-Object {
  $key = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$($_.InterfaceGuid)"
  $ns = (Get-ItemProperty $key -Name NameServer -ErrorAction SilentlyContinue).NameServer
  [pscustomobject]@{
    index = [int]$_.ifIndex; name = "$($_.Name)"; description = "$($_.InterfaceDescription)"
    virtual = [bool]($_.InterfaceDescription -match 'Virtual|Hyper-V|VMware|VirtualBox|TAP|WireGuard|VPN|Loopback')
    dns = @((Get-DnsClientServerAddress -InterfaceIndex $_.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue).ServerAddresses | Where-Object { $_ })
    manual = [bool]$ns
  }
})
ConvertTo-Json -InputObject $r -Depth 3 -Compress
"#;

pub fn dns_adapters() -> Result<Vec<DnsAdapter>, String> {
    let out = crate::ps::powershell(DNS_SCRIPT)?;
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))
}

#[tauri::command(async)]
pub fn list_dns_adapters() -> Result<Vec<DnsAdapter>, String> {
    dns_adapters()
}

/// Cambia los DNS IPv4 de un adaptador. `servers` vacío = automáticos (DHCP).
#[tauri::command(async)]
pub fn set_dns(index: u32, servers: Vec<String>, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let parsed: Vec<Ipv4Addr> = servers
        .iter()
        .map(|s| s.trim().parse::<Ipv4Addr>().map_err(|_| format!("«{s}» no es una dirección IPv4 válida.")))
        .collect::<Result<_, _>>()?;
    if parsed.len() > 4 {
        return Err("Como máximo 4 servidores DNS.".into());
    }
    let adapter = dns_adapters()?.into_iter().find(|a| a.index == index).ok_or("El adaptador ya no está conectado.")?;
    let list = parsed.iter().map(|ip| format!("'{ip}'")).collect::<Vec<_>>().join(",");
    let script = if parsed.is_empty() {
        format!("Set-DnsClientServerAddress -InterfaceIndex {index} -ResetServerAddresses\nClear-DnsClientCache\n'ok'")
    } else {
        format!("Set-DnsClientServerAddress -InterfaceIndex {index} -ServerAddresses @({list})\nClear-DnsClientCache\n'ok'")
    };
    let result = crate::ps::powershell(&script).map(|_| ());
    let before = if adapter.manual { adapter.dns.join(", ") } else { "automáticos".into() };
    let after = if parsed.is_empty() { "automáticos".to_string() } else { parsed.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ") };
    tweaks.record(Op::Run, &format!("DNS de «{}»: {before} → {after}", adapter.name), &result);
    result
}

// ---------- Archivo hosts ----------

pub(crate) fn hosts_path() -> PathBuf {
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    PathBuf::from(windows).join(r"System32\drivers\etc\hosts")
}

#[tauri::command(async)]
pub fn read_hosts() -> Result<String, String> {
    let bytes = std::fs::read(hosts_path()).map_err(|e| format!("No se pudo leer el archivo hosts: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).replace("\r\n", "\n"))
}

/// Guarda el archivo hosts. Antes copia la versión actual a `hosts.adminops.bak`.
#[tauri::command(async)]
pub fn save_hosts(content: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if content.len() > 1_000_000 {
        return Err("El archivo hosts es demasiado grande (más de 1 MB).".into());
    }
    let path = hosts_path();
    let result = (|| {
        std::fs::copy(&path, path.with_file_name("hosts.adminops.bak")).map_err(|e| format!("No se pudo hacer la copia de seguridad: {e}"))?;
        let text = content.replace("\r\n", "\n").replace('\n', "\r\n");
        std::fs::write(&path, text).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                "Windows bloqueó el cambio. Algunos antivirus protegen el archivo hosts: revisa sus avisos.".to_string()
            } else {
                format!("No se pudo guardar: {e}")
            }
        })?;
        let _ = crate::ps::exec("ipconfig.exe", &["/flushdns"]);
        Ok(())
    })();
    tweaks.record(Op::Run, "Editar el archivo hosts (copia en hosts.adminops.bak)", &result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_and_rejects() {
        assert_eq!(resolve("8.8.8.8").unwrap(), Ipv4Addr::new(8, 8, 8, 8));
        assert_eq!(resolve("localhost").unwrap(), Ipv4Addr::LOCALHOST);
        assert!(resolve("").is_err());
        assert!(resolve("a b").is_err());
    }

    #[test]
    fn echo_localhost() {
        let (code, from, _) = echo(Ipv4Addr::LOCALHOST, 128, 1000);
        assert_eq!(code, IP_SUCCESS);
        assert_eq!(from, Some(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn reads_real_state() {
        assert!(read_hosts().is_ok());
        let dns = dns_adapters().unwrap();
        println!("{dns:?}");
        let ports = list_ports().unwrap();
        assert!(ports.iter().any(|p| p.state == "Listen"));
    }

    /// Usa la red: `cargo test trace_first_hops -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn trace_first_hops() {
        for ttl in 1..=3 {
            println!("salto {ttl}: {:?}", echo(Ipv4Addr::new(1, 1, 1, 1), ttl, 1500));
        }
        println!("destino: {:?}", echo(Ipv4Addr::new(1, 1, 1, 1), 128, 1500));
    }

    #[test]
    fn embedded_scripts_parse() {
        for (name, script) in [("PORTS_SCRIPT", PORTS_SCRIPT), ("DNS_SCRIPT", DNS_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }
}
