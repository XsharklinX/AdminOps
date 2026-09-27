//! Diagnóstico de red: adaptadores, ping (ICMP nativo), DNS y salida a Internet.

use crate::ps;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub description: String,
    pub kind: String,
    pub link_speed: String,
    pub mac: String,
    pub ipv4: Vec<String>,
    pub gateway: Vec<String>,
    pub dns: Vec<String>,
    pub dhcp: Option<bool>,
    #[serde(default)]
    pub ssid: Option<String>,
    #[serde(default)]
    pub signal: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PingResult {
    pub label: String,
    pub target: String,
    pub sent: u32,
    pub received: u32,
    pub avg_ms: Option<f64>,
    pub min_ms: Option<u32>,
    pub max_ms: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DnsResult {
    pub host: String,
    pub ok: bool,
    pub ms: f64,
    pub addresses: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReport {
    pub adapters: Vec<Adapter>,
    pub pings: Vec<PingResult>,
    pub dns: Vec<DnsResult>,
    /// Salida a Internet comprobada con la prueba de conectividad de Windows.
    pub internet: bool,
    /// Hay respuesta HTTP pero no la esperada: probablemente un portal cautivo (hotel, aeropuerto…).
    pub captive_portal: bool,
}

fn adapters() -> Result<Vec<Adapter>, String> {
    let out = ps::powershell(
        r#"
$wlan = (netsh.exe wlan show interfaces) -join "`n"
$ssid = if ($wlan -match '(?m)^\s*SSID\s*:\s*(.+)$') { $Matches[1].Trim() } else { $null }
$signal = if ($wlan -match '(?m)(\d+)\s*%') { [int]$Matches[1] } else { $null }
$r = @(Get-NetIPConfiguration -ErrorAction SilentlyContinue | Where-Object { $_.NetAdapter.Status -eq 'Up' } | ForEach-Object {
  $a = $_.NetAdapter
  $wifi = "$($a.PhysicalMediaType)" -match '802\.11' -or $a.InterfaceDescription -match 'Wi-?Fi|Wireless|WLAN'
  $ifc = Get-NetIPInterface -InterfaceIndex $_.InterfaceIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue
  [pscustomobject]@{
    name = "$($_.InterfaceAlias)"; description = "$($_.InterfaceDescription)"
    kind = if ($wifi) { 'Wi-Fi' } elseif ($a.InterfaceDescription -match 'Virtual|Hyper-V|VMware|VirtualBox|TAP|WireGuard|VPN') { 'Virtual/VPN' } else { 'Ethernet' }
    linkSpeed = "$($a.LinkSpeed)"; mac = "$($a.MacAddress)"
    ipv4 = @($_.IPv4Address | ForEach-Object IPAddress | Where-Object { $_ })
    gateway = @($_.IPv4DefaultGateway | ForEach-Object NextHop | Where-Object { $_ })
    dns = @($_.DNSServer | Where-Object AddressFamily -eq 2 | ForEach-Object ServerAddresses | Where-Object { $_ })
    dhcp = if ($ifc) { "$($ifc.Dhcp)" -eq 'Enabled' } else { $null }
    ssid = if ($wifi) { $ssid } else { $null }
    signal = if ($wifi) { $signal } else { $null }
  }
})
ConvertTo-Json -InputObject $r -Depth 3 -Compress
"#,
    )?;
    if out.is_empty() {
        return Ok(vec![]);
    }
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))
}

/// Ping ICMP con la API de Windows (no depende del idioma de ping.exe).
#[cfg(windows)]
pub fn ping(label: &str, target: &str, count: u32) -> PingResult {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY};

    let mut times = Vec::new();
    let addr: Option<Ipv4Addr> = target.parse().ok();
    if let Some(addr) = addr {
        unsafe {
            let h = IcmpCreateFile();
            if h != INVALID_HANDLE_VALUE {
                let data = [0x61u8; 32];
                let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8];
                for _ in 0..count {
                    let n = IcmpSendEcho(
                        h,
                        u32::from_ne_bytes(addr.octets()),
                        data.as_ptr().cast(),
                        data.len() as u16,
                        std::ptr::null(),
                        reply.as_mut_ptr().cast(),
                        reply.len() as u32,
                        1500,
                    );
                    if n > 0 {
                        let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
                        if r.Status == 0 {
                            times.push(r.RoundTripTime);
                        }
                    }
                }
                IcmpCloseHandle(h);
            }
        }
    }
    PingResult {
        label: label.into(),
        target: target.into(),
        sent: count,
        received: times.len() as u32,
        avg_ms: (!times.is_empty()).then(|| times.iter().sum::<u32>() as f64 / times.len() as f64),
        min_ms: times.iter().min().copied(),
        max_ms: times.iter().max().copied(),
    }
}

pub fn resolve(host: &str) -> DnsResult {
    let start = Instant::now();
    let r = (host, 443).to_socket_addrs();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    match r {
        Ok(addrs) => {
            let mut addresses: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
            addresses.dedup();
            DnsResult { host: host.into(), ok: !addresses.is_empty(), ms, addresses }
        }
        Err(_) => DnsResult { host: host.into(), ok: false, ms, addresses: vec![] },
    }
}

/// Prueba de conectividad de Windows (NCSI): (hay Internet, portal cautivo).
async fn internet_check() -> (bool, bool) {
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(6)).build() {
        Ok(c) => c,
        Err(_) => return (false, false),
    };
    match client.get("http://www.msftconnecttest.com/connecttest.txt").send().await {
        Ok(r) => {
            let body = r.text().await.unwrap_or_default();
            let ok = body.trim() == "Microsoft Connect Test";
            (ok, !ok)
        }
        Err(_) => (false, false),
    }
}

#[tauri::command]
pub async fn network_diagnostics() -> Result<NetworkReport, String> {
    let adapters = tauri::async_runtime::spawn_blocking(adapters).await.map_err(|e| e.to_string())??;

    // Destinos: puerta de enlace y DNS de cada adaptador + dos DNS públicos.
    let mut targets: Vec<(String, String)> = Vec::new();
    for a in adapters.iter().filter(|a| a.kind != "Virtual/VPN") {
        for g in a.gateway.iter().filter(|g| g.parse::<Ipv4Addr>().is_ok()) {
            targets.push((format!("Router ({})", a.name), g.clone()));
        }
        if let Some(d) = a.dns.iter().find(|d| d.parse::<Ipv4Addr>().is_ok()) {
            targets.push((format!("DNS ({})", a.name), d.clone()));
        }
    }
    targets.push(("Internet · Cloudflare".into(), "1.1.1.1".into()));
    targets.push(("Internet · Google".into(), "8.8.8.8".into()));
    targets.dedup_by(|a, b| a.1 == b.1);

    let pings = tauri::async_runtime::spawn_blocking(move || {
        std::thread::scope(|s| {
            let handles: Vec<_> = targets.iter().map(|(l, t)| s.spawn(move || ping(l, t, 4))).collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect::<Vec<_>>()
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    let dns = tauri::async_runtime::spawn_blocking(|| ["www.microsoft.com", "www.google.com"].map(resolve).to_vec())
        .await
        .map_err(|e| e.to_string())?;
    let (internet, captive_portal) = internet_check().await;
    Ok(NetworkReport { adapters, pings, dns, internet, captive_portal })
}

#[cfg(test)]
mod tests {
    /// Diagnóstico real (solo lectura): `cargo test network_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn network_real() {
        let r = tauri::async_runtime::block_on(super::network_diagnostics()).unwrap();
        println!("{r:#?}");
    }

    #[test]
    fn ping_localhost() {
        let r = super::ping("local", "127.0.0.1", 2);
        assert_eq!(r.received, 2, "{r:?}");
    }

    #[test]
    fn invalid_target_is_not_a_crash() {
        let r = super::ping("x", "no-es-ip", 1);
        assert_eq!(r.received, 0);
    }
}
