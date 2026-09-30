//! Descubrir impresoras en la red por su nombre: mDNS (Bonjour/AirPrint) y
//! WS-Discovery (lo que usa Windows para «Agregar impresora»).
//!
//! Las impresoras se anuncian solas cuando alguien pregunta: por mDNS dicen su
//! nombre («HP LaserJet M404 [A1B2C3]») y su modelo; por WS-Discovery dicen que
//! son una impresora. Así salen también las que este equipo aún no ha visto
//! nunca (no están en su tabla de vecinos) y se pueden listar con su nombre en
//! vez de solo con la dirección.
//!
//! Solo se pregunta en la red local, con un mensaje de cada tipo, y se escucha
//! lo que contesten durante un par de segundos. Si el cortafuegos de Windows no
//! deja pasar las respuestas, no pasa nada: la búsqueda por puertos sigue igual.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Lo que una impresora dijo de sí misma.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Announced {
    pub name: String,
    pub model: String,
    /// "mdns" | "wsd"
    pub via: &'static str,
}

// ---------- mDNS ----------

/// Servicios de impresión que se preguntan: IPP (AirPrint), RAW 9100 y LPD.
const SERVICES: &[&str] = &["_ipp._tcp.local", "_pdl-datastream._tcp.local", "_printer._tcp.local"];

fn push_name(out: &mut Vec<u8>, name: &str) {
    for label in name.split('.').filter(|l| !l.is_empty()) {
        out.push(label.len().min(63) as u8);
        out.extend_from_slice(&label.as_bytes()[..label.len().min(63)]);
    }
    out.push(0);
}

/// Pregunta PTR por cada servicio, pidiendo respuesta directa (bit QU).
pub fn mdns_query() -> Vec<u8> {
    let mut m = vec![0, 0, 0, 0, 0, SERVICES.len() as u8, 0, 0, 0, 0, 0, 0];
    for s in SERVICES {
        push_name(&mut m, s);
        m.extend([0x00, 0x0C, 0x80, 0x01]);
    }
    m
}

/// Lee un nombre DNS con compresión. Devuelve las etiquetas y dónde sigue.
fn read_name(buf: &[u8], mut pos: usize) -> Option<(Vec<String>, usize)> {
    let mut labels = Vec::new();
    let mut next = None;
    for _ in 0..64 {
        let len = *buf.get(pos)? as usize;
        if len == 0 {
            return Some((labels, next.unwrap_or(pos + 1)));
        }
        if len & 0xC0 == 0xC0 {
            let ptr = ((len & 0x3F) << 8) | *buf.get(pos + 1)? as usize;
            next.get_or_insert(pos + 2);
            pos = ptr;
            continue;
        }
        labels.push(String::from_utf8_lossy(buf.get(pos + 1..pos + 1 + len)?).into_owned());
        pos += 1 + len;
    }
    None
}

fn u16_at(buf: &[u8], pos: usize) -> Option<usize> {
    Some(u16::from_be_bytes([*buf.get(pos)?, *buf.get(pos + 1)?]) as usize)
}

/// Lo que dice una respuesta mDNS: el nombre del servicio (la primera etiqueta
/// del PTR) y el modelo (TXT «ty=» o «product=»).
pub fn parse_mdns(buf: &[u8]) -> Option<Announced> {
    if buf.len() < 12 || buf[2] & 0x80 == 0 {
        return None; // no es una respuesta
    }
    let qd = u16_at(buf, 4)?;
    let records = u16_at(buf, 6)? + u16_at(buf, 8)? + u16_at(buf, 10)?;
    let mut pos = 12;
    for _ in 0..qd {
        pos = read_name(buf, pos)?.1 + 4;
    }
    let mut out = Announced { via: "mdns", ..Default::default() };
    for _ in 0..records {
        let (owner, p) = read_name(buf, pos)?;
        let kind = u16_at(buf, p)?;
        let len = u16_at(buf, p + 8)?;
        let data = p + 10;
        buf.get(data..data + len)?;
        let is_print = owner.iter().any(|l| l == "_ipp" || l == "_pdl-datastream" || l == "_printer");
        match kind {
            12 if is_print && out.name.is_empty() => {
                if let Some((target, _)) = read_name(buf, data) {
                    out.name = target.first().cloned().unwrap_or_default();
                }
            }
            16 if out.model.is_empty() => {
                let mut i = data;
                while i < data + len {
                    let n = buf[i] as usize;
                    let s = String::from_utf8_lossy(buf.get(i + 1..(i + 1 + n).min(data + len))?).into_owned();
                    if let Some(v) = s.strip_prefix("ty=").or_else(|| s.strip_prefix("product=")) {
                        out.model = v.trim_matches(|c| c == '(' || c == ')').trim().to_string();
                        break;
                    }
                    i += 1 + n;
                }
            }
            _ => {}
        }
        pos = data + len;
    }
    (!out.name.is_empty() || !out.model.is_empty()).then_some(out)
}

// ---------- WS-Discovery ----------

fn message_id() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) ^ (u128::from(std::process::id()) << 64);
    let h = format!("{n:032x}");
    format!("urn:uuid:{}-{}-4{}-8{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[17..20], &h[20..32])
}

/// Probe de WS-Discovery: «¿quién es un dispositivo?». Las impresoras dicen
/// en sus Types que son de impresión.
pub fn wsd_probe() -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:wsdp="http://schemas.xmlsoap.org/ws/2006/02/devprof">"#,
            r#"<soap:Header><wsa:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</wsa:To>"#,
            r#"<wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>"#,
            r#"<wsa:MessageID>{}</wsa:MessageID></soap:Header>"#,
            r#"<soap:Body><wsd:Probe><wsd:Types>wsdp:Device</wsd:Types></wsd:Probe></soap:Body></soap:Envelope>"#
        ),
        message_id()
    )
}

/// ¿La respuesta es de una impresora? Se mira solo lo que dice en Types.
pub fn wsd_is_printer(xml: &str) -> bool {
    let lower = xml.to_lowercase();
    lower.contains("probematches") && lower.split("types>").skip(1).step_by(2).any(|t| t.contains("print"))
}

// ---------- Escuchar ----------

/// Pregunta por mDNS y WS-Discovery a la vez y junta lo que contesten, por dirección.
pub fn discover(wait: Duration) -> HashMap<IpAddr, Announced> {
    let mdns = std::thread::spawn(move || listen(SocketAddr::new(Ipv4Addr::new(224, 0, 0, 251).into(), 5353), mdns_query(), wait, parse_mdns));
    let wsd = std::thread::spawn(move || {
        listen(SocketAddr::new(Ipv4Addr::new(239, 255, 255, 250).into(), 3702), wsd_probe().into_bytes(), wait, |b| {
            wsd_is_printer(&String::from_utf8_lossy(b)).then_some(Announced { via: "wsd", ..Default::default() })
        })
    });
    let mut out: HashMap<IpAddr, Announced> = wsd.join().unwrap_or_default();
    // mDNS trae nombre y modelo: manda sobre WS-Discovery.
    for (ip, a) in mdns.join().unwrap_or_default() {
        out.insert(ip, a);
    }
    out
}

fn listen(to: SocketAddr, msg: Vec<u8>, wait: Duration, parse: fn(&[u8]) -> Option<Announced>) -> HashMap<IpAddr, Announced> {
    let mut out = HashMap::new();
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else { return out };
    let _ = sock.set_multicast_ttl_v4(1);
    if sock.set_read_timeout(Some(Duration::from_millis(250))).is_err() || sock.send_to(&msg, to).is_err() {
        return out;
    }
    let end = Instant::now() + wait;
    let mut buf = [0u8; 9000];
    let mut resent = false;
    while Instant::now() < end {
        // Una segunda pregunta a mitad: el UDP multicast se pierde a veces.
        if !resent && Instant::now() + wait / 2 > end {
            let _ = sock.send_to(&msg, to);
            resent = true;
        }
        match sock.recv_from(&mut buf) {
            Ok((n, from)) => {
                if let Some(a) = parse(&buf[..n]) {
                    out.entry(from.ip()).or_insert(a);
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            // Otros errores (Windows devuelve al instante los ICMP de «puerto cerrado»): sin girar en vacío.
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rr(name: &[u8], kind: u16, data: &[u8]) -> Vec<u8> {
        let mut v = name.to_vec();
        v.extend(kind.to_be_bytes());
        v.extend([0x80, 0x01, 0, 0, 0x11, 0x94]);
        v.extend((data.len() as u16).to_be_bytes());
        v.extend_from_slice(data);
        v
    }

    /// Una respuesta como la de una HP por AirPrint, con compresión de nombres.
    #[test]
    fn mdns_answer_gives_name_and_model() {
        let mut m = vec![0, 0, 0x84, 0, 0, 0, 0, 2, 0, 0, 0, 0];
        // _ipp._tcp.local en el byte 12; el PTR apunta a «HP LaserJet M404 [A1B2C3]» + puntero.
        let mut owner = Vec::new();
        push_name(&mut owner, "_ipp._tcp.local");
        let mut target = vec![25];
        target.extend(b"HP LaserJet M404 [A1B2C3]");
        target.extend([0xC0, 12]);
        m.extend(rr(&owner, 12, &target));
        let txt: Vec<u8> = [b"txtvers=1".as_slice(), b"ty=HP LaserJet Pro M404dn", b"note=Recepcion"]
            .iter()
            .flat_map(|s| std::iter::once(s.len() as u8).chain(s.iter().copied()))
            .collect();
        m.extend(rr(&[0xC0, 12], 16, &txt));
        let a = parse_mdns(&m).unwrap();
        assert_eq!(a.name, "HP LaserJet M404 [A1B2C3]");
        assert_eq!(a.model, "HP LaserJet Pro M404dn");
        assert_eq!(a.via, "mdns");
    }

    #[test]
    fn mdns_ignores_queries_and_garbage() {
        assert!(parse_mdns(&mdns_query()).is_none(), "nuestra propia pregunta no es una impresora");
        assert!(parse_mdns(&[]).is_none());
        assert!(parse_mdns(&[0, 0, 0x84, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0xC0]).is_none());
        // Bucle de punteros: no se cuelga.
        let mut m = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        m.extend([0xC0, 12]);
        assert!(parse_mdns(&m).is_none());
    }

    #[test]
    fn query_asks_for_the_print_services() {
        let q = mdns_query();
        assert_eq!(q[5] as usize, SERVICES.len());
        assert!(q.windows(4).any(|w| w == b"_ipp"));
        assert!(q.windows(15).any(|w| w == b"_pdl-datastream"));
    }

    #[test]
    fn wsd_recognises_printers() {
        let p = wsd_probe();
        assert!(p.contains("<wsd:Probe>") && p.contains("urn:uuid:"));
        let impresora = "<s:Envelope><s:Body><d:ProbeMatches><d:ProbeMatch><d:Types>wsdp:Device wprt:PrintDeviceType</d:Types></d:ProbeMatch></d:ProbeMatches></s:Body></s:Envelope>";
        let nas = "<s:Envelope><s:Body><d:ProbeMatches><d:ProbeMatch><d:Types>wsdp:Device pub:Computer</d:Types><d:XAddrs>http://print-server/</d:XAddrs></d:ProbeMatch></d:ProbeMatches></s:Body></s:Envelope>";
        assert!(wsd_is_printer(impresora));
        assert!(!wsd_is_printer(nas), "«print» fuera de Types no cuenta");
        assert!(!wsd_is_printer(&p), "nuestro propio Probe no es una respuesta");
    }
}
