//! Identificar a fondo los dispositivos de la red local: qué son y qué modelo.
//!
//! Todo por la red local, sin Internet:
//! - SSDP/UPnP: televisores, routers, consolas, reproductores (fabricante y modelo en su XML).
//! - mDNS/Bonjour (consulta "legacy unicast"): Chromecast, Apple, impresoras, altavoces.
//! - NetBIOS: nombre y grupo de trabajo de los PC con Windows.
//! - Puertos TCP abiertos y título de su página web.
//! - TTL del ping: familia del sistema (Windows, Linux/Android/Apple, equipo de red).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceInfo {
    pub ip: String,
    /// router | pc | laptop | phone | tablet | tv | streaming | printer | camera | nas | console | speaker | iot | network | unknown
    pub kind: String,
    pub manufacturer: String,
    pub model: String,
    /// Nombre que el propio dispositivo anuncia ("TV del salón", "HP LaserJet de recepción").
    pub friendly: String,
    /// Windows | Linux, Android o Apple | Equipo de red
    pub os: String,
    pub netbios: String,
    pub workgroup: String,
    pub http_title: String,
    pub services: Vec<String>,
    pub ports: Vec<u16>,
}

// ---------- TTL ----------

#[cfg(windows)]
fn ping_ttl(addr: Ipv4Addr) -> Option<u8> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY};
    unsafe {
        let h = IcmpCreateFile();
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let data = [0x61u8; 16];
        let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + data.len() + 8];
        let n = IcmpSendEcho(h, u32::from_ne_bytes(addr.octets()), data.as_ptr().cast(), data.len() as u16, std::ptr::null(), reply.as_mut_ptr().cast(), reply.len() as u32, 700);
        IcmpCloseHandle(h);
        if n == 0 {
            return None;
        }
        let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
        (r.Status == 0).then_some(r.Options.Ttl)
    }
}

#[cfg(not(windows))]
fn ping_ttl(_: Ipv4Addr) -> Option<u8> {
    None
}

fn os_from_ttl(ttl: u8) -> &'static str {
    match ttl {
        0..=64 => "Linux, Android o Apple",
        65..=128 => "Windows",
        _ => "Equipo de red",
    }
}

// ---------- Puertos ----------

const PORTS: &[u16] = &[22, 23, 80, 443, 445, 515, 548, 554, 631, 1883, 3389, 5000, 5001, 8008, 8009, 8080, 9100, 62078];

fn open_ports(ip: Ipv4Addr) -> Vec<u16> {
    PORTS.iter().copied().filter(|p| TcpStream::connect_timeout(&SocketAddr::from((ip, *p)), Duration::from_millis(300)).is_ok()).collect()
}

// ---------- NetBIOS ----------

/// Consulta NBSTAT: devuelve (nombre del equipo, grupo de trabajo).
fn netbios(ip: Ipv4Addr) -> Option<(String, String)> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(Duration::from_millis(700))).ok()?;
    // Cabecera + nombre "*" codificado (CK + 30 A) + tipo NBSTAT (0x21), clase IN.
    let mut q = vec![0x13, 0x37, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, b'C', b'K'];
    q.extend(std::iter::repeat_n(b'A', 30));
    q.extend_from_slice(&[0x00, 0x00, 0x21, 0x00, 0x01]);
    sock.send_to(&q, (ip, 137)).ok()?;
    let mut buf = [0u8; 1024];
    let (n, _) = sock.recv_from(&mut buf).ok()?;
    parse_nbstat(&buf[..n])
}

fn parse_nbstat(b: &[u8]) -> Option<(String, String)> {
    // Tras la cabecera (12) y el nombre de la pregunta repetido (34) vienen tipo, clase, TTL y longitud (10).
    let start = 12 + 34 + 10;
    let count = *b.get(start)? as usize;
    let mut name = String::new();
    let mut group = String::new();
    for i in 0..count {
        let off = start + 1 + i * 18;
        let entry = b.get(off..off + 18)?;
        let text = String::from_utf8_lossy(&entry[..15]).trim().to_string();
        let suffix = entry[15];
        let is_group = entry[16] & 0x80 != 0;
        if suffix == 0x00 && !is_group && name.is_empty() {
            name = text;
        } else if suffix == 0x00 && is_group && group.is_empty() {
            group = text;
        }
    }
    (!name.is_empty()).then_some((name, group))
}

// ---------- SSDP / UPnP ----------

#[derive(Default, Debug)]
struct Upnp {
    friendly: String,
    manufacturer: String,
    model: String,
    device_type: String,
}

fn xml_tag(xml: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let Some(s) = xml.find(&open) else { return String::new() };
    let from = s + open.len();
    let to = xml[from..].find(&format!("</{tag}>")).map_or(from, |e| from + e);
    xml[from..to].replace("&amp;", "&").replace("&apos;", "'").replace("&quot;", "\"").trim().chars().take(80).collect()
}

/// LOCATION de cada IP que responde a una búsqueda SSDP.
fn ssdp_locations(local: Ipv4Addr, wait: Duration) -> HashMap<Ipv4Addr, String> {
    let mut out = HashMap::new();
    // Atado a la IP de la red actual: si no, el multicast puede salir por otro adaptador.
    let Ok(sock) = UdpSocket::bind((local, 0)) else { return out };
    let _ = sock.set_multicast_ttl_v4(2);
    let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
    let msg = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: ssdp:all\r\n\r\n";
    for _ in 0..2 {
        let _ = sock.send_to(msg.as_bytes(), "239.255.255.250:1900");
    }
    let end = Instant::now() + wait;
    let mut buf = [0u8; 2048];
    while Instant::now() < end {
        if let Ok((n, SocketAddr::V4(from))) = sock.recv_from(&mut buf) {
            let text = String::from_utf8_lossy(&buf[..n]);
            if let Some(loc) = text.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("location")).map(|(_, v)| v.trim().to_string())) {
                // Una por equipo: la del dispositivo raíz suele llegar primero.
                out.entry(*from.ip()).or_insert(loc);
            }
        }
    }
    out
}

async fn upnp_describe(location: &str) -> Option<Upnp> {
    let url = tauri::Url::parse(location).ok()?;
    // Solo direcciones de la red local.
    if !url.host_str().is_some_and(super::lan::is_private_host) {
        return None;
    }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(3)).build().ok()?;
    let xml = client.get(url).send().await.ok()?.text().await.ok()?;
    let d = Upnp {
        friendly: xml_tag(&xml, "friendlyName"),
        manufacturer: xml_tag(&xml, "manufacturer"),
        model: [xml_tag(&xml, "modelName"), xml_tag(&xml, "modelNumber")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" "),
        device_type: xml_tag(&xml, "deviceType"),
    };
    (!d.friendly.is_empty() || !d.model.is_empty()).then_some(d)
}

// ---------- mDNS (DNS-SD) ----------

const MDNS_SERVICES: &[&str] = &[
    "_googlecast._tcp.local",
    "_airplay._tcp.local",
    "_raop._tcp.local",
    "_ipp._tcp.local",
    "_ipps._tcp.local",
    "_printer._tcp.local",
    "_pdl-datastream._tcp.local",
    "_device-info._tcp.local",
    "_companion-link._tcp.local",
    "_hap._tcp.local",
    "_spotify-connect._tcp.local",
    "_smb._tcp.local",
    "_androidtvremote2._tcp.local",
    "_amzn-wplay._tcp.local",
    "_sonos._tcp.local",
    "_workstation._tcp.local",
];

fn service_label(svc: &str) -> Option<&'static str> {
    Some(match svc {
        "_googlecast._tcp" => "Chromecast / Google Cast",
        "_airplay._tcp" => "AirPlay",
        "_raop._tcp" => "Altavoz AirPlay",
        "_ipp._tcp" | "_ipps._tcp" | "_printer._tcp" | "_pdl-datastream._tcp" => "Impresora",
        "_companion-link._tcp" => "Dispositivo Apple",
        "_hap._tcp" => "Accesorio HomeKit",
        "_spotify-connect._tcp" => "Spotify Connect",
        "_smb._tcp" => "Carpetas compartidas",
        "_androidtvremote2._tcp" => "Android TV",
        "_amzn-wplay._tcp" => "Fire TV",
        "_sonos._tcp" => "Sonos",
        "_workstation._tcp" => "Equipo con Linux o NAS",
        _ => return None,
    })
}

fn dns_name(q: &mut Vec<u8>, name: &str) {
    for label in name.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
}

/// Una consulta PTR por servicio, pidiendo respuesta unicast (bit QU).
fn mdns_query() -> Vec<u8> {
    let mut q = vec![0, 0, 0, 0, 0, MDNS_SERVICES.len() as u8, 0, 0, 0, 0, 0, 0];
    for s in MDNS_SERVICES {
        dns_name(&mut q, s);
        q.extend_from_slice(&[0x00, 0x0C, 0x80, 0x01]);
    }
    q
}

/// Lee un nombre DNS (con compresión) desde `pos`. Devuelve (nombre, posición siguiente).
fn read_name(b: &[u8], mut pos: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut next = None;
    let mut jumps = 0;
    loop {
        let len = *b.get(pos)? as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        if len & 0xC0 == 0xC0 {
            let ptr = ((len & 0x3F) << 8) | *b.get(pos + 1)? as usize;
            next.get_or_insert(pos + 2);
            pos = ptr;
            jumps += 1;
            if jumps > 20 {
                return None;
            }
            continue;
        }
        labels.push(String::from_utf8_lossy(b.get(pos + 1..pos + 1 + len)?).to_string());
        pos += 1 + len;
    }
    Some((labels.join("."), next.unwrap_or(pos)))
}

#[derive(Default, Debug)]
struct Mdns {
    services: HashSet<String>,
    /// Nombre de la instancia ("Salón._googlecast._tcp.local" → "Salón").
    instance: String,
    txt: HashMap<String, String>,
}

/// Registros de una respuesta mDNS: servicios, nombre de instancia y TXT (md=, ty=, model=…).
fn parse_mdns(b: &[u8], into: &mut Mdns) -> Option<()> {
    let qd = u16::from_be_bytes([*b.get(4)?, *b.get(5)?]) as usize;
    let rr = u16::from_be_bytes([*b.get(6)?, *b.get(7)?]) as usize + u16::from_be_bytes([*b.get(8)?, *b.get(9)?]) as usize + u16::from_be_bytes([*b.get(10)?, *b.get(11)?]) as usize;
    let mut pos = 12;
    for _ in 0..qd {
        pos = read_name(b, pos)?.1 + 4;
    }
    for _ in 0..rr {
        let (name, p) = read_name(b, pos)?;
        let rtype = u16::from_be_bytes([*b.get(p)?, *b.get(p + 1)?]);
        let len = u16::from_be_bytes([*b.get(p + 8)?, *b.get(p + 9)?]) as usize;
        let data = p + 10;
        match rtype {
            // PTR: servicio → instancia
            12 => {
                if let Some((target, _)) = read_name(b, data) {
                    let svc = name.trim_end_matches(".local").to_string();
                    if svc.starts_with('_') {
                        into.services.insert(svc.clone());
                        if into.instance.is_empty() {
                            into.instance = target.split("._").next().unwrap_or("").to_string();
                        }
                    }
                }
            }
            // TXT: cadenas "clave=valor"
            16 => {
                let mut i = data;
                while i < data + len {
                    let l = *b.get(i)? as usize;
                    if let Some(kv) = b.get(i + 1..i + 1 + l) {
                        let kv = String::from_utf8_lossy(kv);
                        if let Some((k, v)) = kv.split_once('=') {
                            into.txt.entry(k.to_ascii_lowercase()).or_insert_with(|| v.chars().take(80).collect());
                        }
                    }
                    i += 1 + l;
                }
            }
            _ => {}
        }
        pos = data + len;
    }
    Some(())
}

fn mdns_scan(local: Ipv4Addr, wait: Duration) -> HashMap<Ipv4Addr, Mdns> {
    let mut out: HashMap<Ipv4Addr, Mdns> = HashMap::new();
    let Ok(sock) = UdpSocket::bind((local, 0)) else { return out };
    let _ = sock.set_multicast_ttl_v4(255);
    let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
    let q = mdns_query();
    for _ in 0..2 {
        let _ = sock.send_to(&q, "224.0.0.251:5353");
    }
    let end = Instant::now() + wait;
    let mut buf = [0u8; 9000];
    while Instant::now() < end {
        if let Ok((n, SocketAddr::V4(from))) = sock.recv_from(&mut buf) {
            let _ = parse_mdns(&buf[..n], out.entry(*from.ip()).or_default());
        }
    }
    out
}

// ---------- Clasificación ----------

fn has(hay: &str, words: &[&str]) -> bool {
    let h = hay.to_lowercase();
    words.iter().any(|w| h.contains(w))
}

pub struct Hints<'a> {
    pub gateway: bool,
    pub this_pc: bool,
    pub private_mac: bool,
    pub vendor: &'a str,
    pub name: &'a str,
}

fn classify(d: &DeviceInfo, h: &Hints) -> &'static str {
    let text = format!("{} {} {} {} {} {} {} {}", d.manufacturer, d.model, d.friendly, d.http_title, d.netbios, h.vendor, h.name, d.services.join(" "));
    let p = |port: u16| d.ports.contains(&port);
    if h.gateway {
        return "router";
    }
    if has(&text, &["xbox", "playstation", "nintendo", "ps4", "ps5"]) {
        return "console";
    }
    if d.services.iter().any(|s| s == "Impresora") || p(9100) || p(631) || p(515) || has(&text, &["printer", "laserjet", "deskjet", "officejet", "epson", "brother", "canon", "impresora"]) {
        return "printer";
    }
    if p(554) || has(&text, &["camera", "cámara", "ipc", "hikvision", "dahua", "ezviz", "reolink", "nvr", "dvr", "imou", "tapo c"]) {
        return "camera";
    }
    if has(&text, &["chromecast", "google cast", "android tv", "fire tv", "roku", "apple tv", "appletv"]) {
        return "streaming";
    }
    if has(&text, &["tv", "bravia", "webos", "tizen", "smart tv", "televis"]) || has(&d.model, &["un55", "un65", "qn", "oled"]) {
        return "tv";
    }
    if d.services.iter().any(|s| s.contains("Altavoz") || s == "Sonos" || s == "Spotify Connect") || has(&text, &["sonos", "echo", "home mini", "nest mini", "homepod", "speaker"]) {
        return "speaker";
    }
    if p(62078) || has(&text, &["iphone", "android", "galaxy", "redmi", "pixel", "moto "]) {
        return "phone";
    }
    if has(&text, &["ipad", "tablet", "tab "]) {
        return "tablet";
    }
    if p(5000) || p(5001) || has(&text, &["synology", "qnap", "nas", "diskstation", "truenas"]) {
        return "nas";
    }
    if p(1883) || has(&text, &["espressif", "tuya", "shelly", "tasmota", "sonoff", "smart plug", "bombilla", "broadlink"]) {
        return "iot";
    }
    if h.this_pc || p(3389) || p(445) || !d.netbios.is_empty() || d.os == "Windows" || d.services.iter().any(|s| s == "Dispositivo Apple") {
        return if has(&text, &["macbook", "laptop", "portátil", "notebook"]) { "laptop" } else { "pc" };
    }
    if h.private_mac {
        return "phone";
    }
    if d.os == "Equipo de red" || has(&text, &["switch", "access point", "tp-link", "ubiquiti", "unifi", "mikrotik", "repeater", "mesh", "deco"]) {
        return "network";
    }
    "unknown"
}

/// Modelo legible a partir de lo que cada protocolo anuncia.
fn model_from(m: &Mdns) -> (String, String) {
    let t = |k: &str| m.txt.get(k).cloned().unwrap_or_default();
    // Chromecast: md; impresoras: ty / product; Apple: model / am / rpMd; genérico: model.
    let model = [t("md"), t("ty"), t("product").trim_matches(['(', ')']).to_string(), t("model"), t("am"), t("rpmd"), t("usb_mdl")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
    let manufacturer = [t("usb_mfg"), t("manufacturer"), t("mfg")].into_iter().find(|s| !s.is_empty()).unwrap_or_default();
    (manufacturer, model)
}

/// Identifica a fondo las IP indicadas (todas en paralelo).
pub fn identify(local: Ipv4Addr, targets: &[(Ipv4Addr, Hints)]) -> Vec<DeviceInfo> {
    let wait = Duration::from_millis(3000);
    let (ssdp, mdns, per_ip) = std::thread::scope(|s| {
        let ssdp = s.spawn(|| ssdp_locations(local, wait));
        let mdns = s.spawn(|| mdns_scan(local, wait));
        let per_ip: Vec<_> = targets
            .iter()
            .map(|(ip, h)| {
                let ip = *ip;
                let skip_ports = h.this_pc;
                s.spawn(move || {
                    let ports = if skip_ports { vec![] } else { open_ports(ip) };
                    (ip, ports, netbios(ip), ping_ttl(ip))
                })
            })
            .collect();
        let per_ip: Vec<_> = per_ip.into_iter().filter_map(|h| h.join().ok()).collect();
        (ssdp.join().unwrap_or_default(), mdns.join().unwrap_or_default(), per_ip)
    });

    // Descripciones UPnP y títulos web, en paralelo (red local, 3 s como mucho cada una).
    // En un hilo con su propio runtime: esperar un futuro desde dentro del runtime
    // de Tauri (donde se ejecutan los comandos) provoca un pánico y cierra la app.
    let wanted: HashSet<Ipv4Addr> = targets.iter().map(|(ip, _)| *ip).collect();
    let web = std::thread::scope(|s| {
        s.spawn(|| {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return (vec![], vec![]) };
            rt.block_on(async {
        let upnp = futures_util::future::join_all(ssdp.iter().filter(|(ip, _)| wanted.contains(ip)).map(|(ip, loc)| async move { (*ip, upnp_describe(loc).await) })).await;
        let titles = futures_util::future::join_all(per_ip.iter().filter(|(_, ports, _, _)| ports.iter().any(|p| [80, 443, 8080].contains(p))).map(|(ip, ports, _, _)| {
            let url = if ports.contains(&80) { format!("http://{ip}/") } else if ports.contains(&443) { format!("https://{ip}/") } else { format!("http://{ip}:8080/") };
            async move { (*ip, super::lan::page_title(&url).await) }
        }))
        .await;
        (upnp, titles)
            })
        })
        .join()
        .unwrap_or_default()
    });
    let (upnp, titles) = web;
    let upnp: HashMap<Ipv4Addr, Upnp> = upnp.into_iter().filter_map(|(ip, u)| u.map(|u| (ip, u))).collect();
    let titles: HashMap<Ipv4Addr, String> = titles.into_iter().filter(|(_, t)| !t.is_empty()).collect();

    targets
        .iter()
        .map(|(ip, h)| {
            let (_, ports, nb, ttl) = per_ip.iter().find(|x| x.0 == *ip).cloned().unwrap_or((*ip, vec![], None, None));
            let mut d = DeviceInfo { ip: ip.to_string(), ports, os: ttl.map(os_from_ttl).unwrap_or("").to_string(), ..Default::default() };
            if let Some((name, group)) = nb {
                d.netbios = name;
                d.workgroup = group;
            }
            if let Some(m) = mdns.get(ip) {
                let (mfg, model) = model_from(m);
                d.manufacturer = mfg;
                d.model = model;
                d.friendly = m.txt.get("fn").cloned().unwrap_or_else(|| m.instance.clone());
                let mut labels: Vec<String> = m.services.iter().filter_map(|s| service_label(s)).map(String::from).collect();
                labels.sort();
                labels.dedup();
                d.services = labels;
            }
            if let Some(u) = upnp.get(ip) {
                if d.manufacturer.is_empty() {
                    d.manufacturer = u.manufacturer.clone();
                }
                if d.model.is_empty() {
                    d.model = u.model.clone();
                }
                if d.friendly.is_empty() {
                    d.friendly = u.friendly.clone();
                }
                let kind = u.device_type.split(':').nth(3).unwrap_or("");
                let label = match kind {
                    "MediaRenderer" => "Reproductor multimedia (DLNA)",
                    "MediaServer" => "Servidor multimedia (DLNA)",
                    "InternetGatewayDevice" => "Router UPnP",
                    "Printer" => "Impresora",
                    _ => "",
                };
                if !label.is_empty() && !d.services.iter().any(|s| s == label) {
                    d.services.push(label.into());
                }
            }
            d.http_title = titles.get(ip).cloned().unwrap_or_default();
            if d.manufacturer.is_empty() {
                if let Some(b) = super::lan::brand_of(&d.http_title) {
                    d.manufacturer = b.into();
                }
            }
            d.kind = classify(&d, h).into();
            d
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Red real: `cargo test identify_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn identify_real() {
        let info = crate::network::lan::current().unwrap().unwrap();
        let out = crate::ps::powershell(&format!("$i = {}
@(Get-NetNeighbor -AddressFamily IPv4 -InterfaceIndex $i | Where-Object {{ $_.State -notin 'Unreachable','Incomplete' -and $_.LinkLayerAddress -and $_.LinkLayerAddress -ne 'FF-FF-FF-FF-FF-FF' }} | ForEach-Object {{ \"$($_.IPAddress)\" }}) -join ','", info.index)).unwrap();
        let gw: Ipv4Addr = info.gateway.parse().unwrap();
        let ips: Vec<Ipv4Addr> = out.trim().split(',').filter_map(|s| s.parse().ok()).filter(|ip: &Ipv4Addr| ip.is_private()).collect();
        let targets: Vec<(Ipv4Addr, Hints)> = ips.iter().map(|ip| (*ip, Hints { gateway: *ip == gw, this_pc: false, private_mac: false, vendor: "", name: "" })).collect();
        let t = Instant::now();
        for d in identify(info.ip.parse().unwrap(), &targets) {
            println!("{:15} {:10} os={:22} fab={} modelo={} nombre={} nb={} web={} serv={:?} puertos={:?}", d.ip, d.kind, d.os, d.manufacturer, d.model, d.friendly, d.netbios, d.http_title, d.services, d.ports);
        }
        println!("{} dispositivos en {:.1} s", targets.len(), t.elapsed().as_secs_f64());
    }

    /// Como en la app: llamada desde un hilo del runtime de Tauri (antes: pánico y cierre).
    /// `cargo test identify_inside_runtime -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn identify_inside_runtime() {
        let info = crate::network::lan::current().unwrap().unwrap();
        let local: Ipv4Addr = info.ip.parse().unwrap();
        let gw: Ipv4Addr = info.gateway.parse().unwrap();
        let out = tauri::async_runtime::block_on(tauri::async_runtime::spawn(async move {
            identify(local, &[(gw, Hints { gateway: true, this_pc: false, private_mac: false, vendor: "", name: "" })])
        }))
        .expect("la identificación no debe entrar en pánico dentro del runtime");
        println!("ok: {} → {}", out[0].ip, out[0].kind);
    }

    #[test]
    fn dns_names_with_compression() {
        // "a.local" en 12, y en 21 un puntero a él precedido de "_x".
        let mut b = vec![0u8; 12];
        dns_name(&mut b, "a.local");
        b.extend_from_slice(&[2, b'_', b'x', 0xC0, 12]);
        assert_eq!(read_name(&b, 12).unwrap().0, "a.local");
        let (n, next) = read_name(&b, 21).unwrap();
        assert_eq!(n, "_x.a.local");
        assert_eq!(next, b.len());
        // Bucle de punteros: no se cuelga.
        assert!(read_name(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xC0, 12], 12).is_none());
    }

    #[test]
    fn parses_mdns_answers() {
        // Respuesta con un PTR (_googlecast → "Salón") y un TXT (md=Chromecast, fn=TV Salón).
        let mut b = vec![0, 0, 0x84, 0, 0, 0, 0, 2, 0, 0, 0, 0];
        dns_name(&mut b, "_googlecast._tcp.local");
        let mut target = Vec::new();
        dns_name(&mut target, "Salon._googlecast._tcp.local");
        b.extend_from_slice(&[0, 12, 0, 1, 0, 0, 0, 120, 0, target.len() as u8]);
        b.extend_from_slice(&target);
        dns_name(&mut b, "Salon._googlecast._tcp.local");
        let txt: Vec<u8> = [&b"md=Chromecast"[..], &b"fn=TV Salon"[..]].iter().flat_map(|s| std::iter::once(s.len() as u8).chain(s.iter().copied())).collect();
        b.extend_from_slice(&[0, 16, 0x80, 1, 0, 0, 0, 120, 0, txt.len() as u8]);
        b.extend_from_slice(&txt);
        let mut m = Mdns::default();
        parse_mdns(&b, &mut m).unwrap();
        assert!(m.services.contains("_googlecast._tcp"));
        assert_eq!(m.instance, "Salon");
        assert_eq!(m.txt.get("md").map(String::as_str), Some("Chromecast"));
        assert_eq!(model_from(&m).1, "Chromecast");
    }

    #[test]
    fn nbstat_and_xml() {
        let mut b = vec![0u8; 56];
        b.push(2);
        let entry = |name: &str, group: bool| {
            let mut e = format!("{name:<15}").into_bytes();
            e.push(0x00);
            e.push(if group { 0x84 } else { 0x04 });
            e.push(0);
            e
        };
        b.extend(entry("RECEPCION", false));
        b.extend(entry("OFICINA", true));
        assert_eq!(parse_nbstat(&b), Some(("RECEPCION".into(), "OFICINA".into())));
        assert_eq!(xml_tag("<root><modelName>UN55 &amp; más</modelName></root>", "modelName"), "UN55 & más");
        assert_eq!(os_from_ttl(128), "Windows");
        assert_eq!(os_from_ttl(64), "Linux, Android o Apple");
    }

    #[test]
    fn classifies_common_devices() {
        let h = |gateway, private_mac| Hints { gateway, this_pc: false, private_mac, vendor: "", name: "" };
        let d = |ports: Vec<u16>, model: &str, services: Vec<&str>| DeviceInfo { ports, model: model.into(), services: services.into_iter().map(String::from).collect(), ..Default::default() };
        assert_eq!(classify(&d(vec![9100, 80], "", vec![]), &h(false, false)), "printer");
        assert_eq!(classify(&d(vec![], "Chromecast", vec!["Chromecast / Google Cast"]), &h(false, false)), "streaming");
        assert_eq!(classify(&d(vec![554], "", vec![]), &h(false, false)), "camera");
        assert_eq!(classify(&d(vec![445, 3389], "", vec![]), &h(false, false)), "pc");
        assert_eq!(classify(&d(vec![62078], "", vec![]), &h(false, true)), "phone");
        assert_eq!(classify(&d(vec![80], "", vec![]), &h(true, false)), "router");
        assert_eq!(classify(&d(vec![], "", vec![]), &h(false, false)), "unknown");
    }
}
