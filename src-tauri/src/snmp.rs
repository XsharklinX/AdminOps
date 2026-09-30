//! SNMP mínimo para preguntar a las impresoras de red cómo están.
//!
//! Casi todas las impresoras de oficina publican su estado por SNMP con la
//! Printer-MIB (RFC 3805): nivel de cada consumible, contador de páginas y
//! errores del aparato (sin papel, atasco, puerta abierta, sin tóner). Con eso
//! «Revisar» puede decir «tóner negro al 8 %» antes de que alguien llame.
//!
//! Solo lo necesario: SNMP v2c (y v1 de reserva) con GET y GETNEXT, la
//! comunidad de solo lectura de siempre («public»), por UDP en la red local.
//! Nada sale de la red. Se escribe aquí en vez de añadir una biblioteca porque
//! son cuatro mensajes y así se puede probar byte a byte.

use serde::Serialize;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Duration;

// ---------- BER ----------

fn push_len(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        out.push(len as u8);
    } else if len <= 0xFF {
        out.extend([0x81, len as u8]);
    } else {
        out.extend([0x82, (len >> 8) as u8, len as u8]);
    }
}

fn tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    push_len(&mut out, value.len());
    out.extend_from_slice(value);
    out
}

fn int(v: i64) -> Vec<u8> {
    let mut bytes = v.to_be_bytes().to_vec();
    // Quitar los bytes sobrantes sin cambiar el signo.
    while bytes.len() > 1 && ((bytes[0] == 0 && bytes[1] & 0x80 == 0) || (bytes[0] == 0xFF && bytes[1] & 0x80 != 0)) {
        bytes.remove(0);
    }
    tlv(0x02, &bytes)
}

/// Codifica un OID («1.3.6.1.2.1.1.5.0»).
pub fn encode_oid(oid: &[u32]) -> Vec<u8> {
    let mut v = Vec::new();
    if oid.len() >= 2 {
        v.push((oid[0] * 40 + oid[1]) as u8);
    }
    for &arc in oid.iter().skip(2) {
        let mut chunk = vec![(arc & 0x7F) as u8];
        let mut rest = arc >> 7;
        while rest > 0 {
            chunk.push(((rest & 0x7F) as u8) | 0x80);
            rest >>= 7;
        }
        chunk.reverse();
        v.extend(chunk);
    }
    tlv(0x06, &v)
}

pub fn parse_oid(s: &str) -> Vec<u32> {
    s.split('.').filter_map(|p| p.parse().ok()).collect()
}

/// Mensaje GET (0xA0) o GETNEXT (0xA1) para varios OID.
pub fn build_request(version: i64, community: &str, pdu: u8, request_id: i64, oids: &[Vec<u32>]) -> Vec<u8> {
    let binds: Vec<u8> = oids.iter().flat_map(|o| tlv(0x30, &[encode_oid(o), vec![0x05, 0x00]].concat())).collect();
    let pdu_body = [int(request_id), int(0), int(0), tlv(0x30, &binds)].concat();
    tlv(0x30, &[int(version), tlv(0x04, community.as_bytes()), tlv(pdu, &pdu_body)].concat())
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bytes(Vec<u8>),
    Oid(Vec<u32>),
    /// Null, noSuchObject, noSuchInstance, endOfMibView.
    Missing,
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<String> {
        match self {
            // Solo lo imprimible: algunas impresoras rellenan con ceros.
            Value::Bytes(b) => Some(String::from_utf8_lossy(b).chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string()),
            _ => None,
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn tlv(&mut self) -> Option<(u8, &'a [u8])> {
        let tag = *self.b.get(self.i)?;
        let mut len = *self.b.get(self.i + 1)? as usize;
        let mut pos = self.i + 2;
        if len & 0x80 != 0 {
            let n = len & 0x7F;
            if n == 0 || n > 3 {
                return None;
            }
            len = 0;
            for k in 0..n {
                len = (len << 8) | *self.b.get(pos + k)? as usize;
            }
            pos += n;
        }
        let v = self.b.get(pos..pos + len)?;
        self.i = pos + len;
        Some((tag, v))
    }
}

fn decode_int(v: &[u8]) -> i64 {
    if v.is_empty() || v.len() > 8 {
        return 0;
    }
    let mut n: i64 = if v[0] & 0x80 != 0 { -1 } else { 0 };
    for &b in v {
        n = (n << 8) | b as i64;
    }
    n
}

fn decode_uint(v: &[u8]) -> i64 {
    v.iter().take(8).fold(0i64, |n, &b| (n << 8) | b as i64)
}

fn decode_oid(v: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    if let Some(&first) = v.first() {
        out.push(u32::from(first / 40));
        out.push(u32::from(first % 40));
    }
    let mut acc: u32 = 0;
    for &b in v.iter().skip(1) {
        acc = (acc << 7) | u32::from(b & 0x7F);
        if b & 0x80 == 0 {
            out.push(acc);
            acc = 0;
        }
    }
    out
}

/// Pares OID → valor de una respuesta.
pub type Binds = Vec<(Vec<u32>, Value)>;

/// Lee una respuesta: (id de petición, estado de error, pares OID → valor).
pub fn parse_response(buf: &[u8]) -> Option<(i64, i64, Binds)> {
    let mut r = Reader { b: buf, i: 0 };
    let (t, msg) = r.tlv()?;
    if t != 0x30 {
        return None;
    }
    let mut m = Reader { b: msg, i: 0 };
    m.tlv()?; // versión
    m.tlv()?; // comunidad
    let (pdu_tag, pdu) = m.tlv()?;
    if pdu_tag != 0xA2 {
        return None;
    }
    let mut p = Reader { b: pdu, i: 0 };
    let (_, id) = p.tlv()?;
    let (_, err) = p.tlv()?;
    p.tlv()?; // índice del error
    let (_, binds) = p.tlv()?;
    let mut b = Reader { b: binds, i: 0 };
    let mut out = Vec::new();
    while b.i < binds.len() {
        let (_, pair) = b.tlv()?;
        let mut q = Reader { b: pair, i: 0 };
        let (_, oid) = q.tlv()?;
        let (vt, vv) = q.tlv()?;
        let value = match vt {
            0x02 => Value::Int(decode_int(vv)),
            0x41 | 0x42 | 0x43 | 0x46 => Value::Int(decode_uint(vv)),
            0x04 | 0x40 => Value::Bytes(vv.to_vec()),
            0x06 => Value::Oid(decode_oid(vv)),
            _ => Value::Missing,
        };
        out.push((decode_oid(oid), value));
    }
    Some((decode_int(id), decode_int(err), out))
}

// ---------- Cliente ----------

const COMMUNITY: &str = "public";

struct Session {
    sock: UdpSocket,
    to: SocketAddr,
    version: i64,
    next_id: i64,
}

impl Session {
    fn open(ip: IpAddr, timeout: Duration) -> Option<Session> {
        let sock = UdpSocket::bind(if ip.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).ok()?;
        sock.set_read_timeout(Some(timeout)).ok()?;
        let mut s = Session { sock, to: SocketAddr::new(ip, 161), version: 1, next_id: 0x4144 };
        // v2c y, si no contesta, v1 (algunas impresoras viejas solo saben esa).
        let probe = [parse_oid("1.3.6.1.2.1.1.5.0")];
        if s.request(0xA0, &probe).is_some() {
            return Some(s);
        }
        s.version = 0;
        s.request(0xA0, &probe).map(|_| s)
    }

    fn request(&mut self, pdu: u8, oids: &[Vec<u32>]) -> Option<Vec<(Vec<u32>, Value)>> {
        self.next_id += 1;
        let msg = build_request(self.version, COMMUNITY, pdu, self.next_id, oids);
        let mut buf = [0u8; 4096];
        for _ in 0..2 {
            self.sock.send_to(&msg, self.to).ok()?;
            while let Ok((n, from)) = self.sock.recv_from(&mut buf) {
                if from.ip() != self.to.ip() {
                    continue;
                }
                if let Some((id, err, binds)) = parse_response(&buf[..n]) {
                    if id == self.next_id {
                        return (err == 0).then_some(binds);
                    }
                }
            }
        }
        None
    }

    fn get(&mut self, oid: &str) -> Option<Value> {
        self.request(0xA0, &[parse_oid(oid)])?.into_iter().next().map(|(_, v)| v).filter(|v| *v != Value::Missing)
    }

    /// Recorre una columna de una tabla: (sufijo del índice, valor).
    fn walk(&mut self, base: &str) -> Vec<(Vec<u32>, Value)> {
        let base = parse_oid(base);
        let mut cur = base.clone();
        let mut out = Vec::new();
        for _ in 0..64 {
            let Some(mut r) = self.request(0xA1, std::slice::from_ref(&cur)) else { break };
            let Some((oid, v)) = r.pop() else { break };
            if !oid.starts_with(&base) || v == Value::Missing || oid <= cur {
                break;
            }
            out.push((oid[base.len()..].to_vec(), v));
            cur = oid;
        }
        out
    }
}

// ---------- Impresoras ----------

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Supply {
    pub name: String,
    /// 0-100, o None si la impresora no lo sabe.
    pub percent: Option<u8>,
    /// Es tóner o tinta (lo que se cambia a menudo), no un fusor o un tambor.
    pub consumable: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrinterStatus {
    /// Contestó por SNMP.
    pub reachable: bool,
    pub name: String,
    pub model: String,
    pub pages: Option<u64>,
    pub supplies: Vec<Supply>,
    /// Lo que la impresora dice que le pasa, en español.
    pub alerts: Vec<String>,
}

/// Los errores que publica la impresora (hrPrinterDetectedErrorState), en
/// español. El bit 0 es el más alto del primer byte.
pub fn decode_alerts(bits: &[u8]) -> Vec<String> {
    const NOMBRES: [&str; 15] = [
        "Queda poco papel",
        "Sin papel",
        "Queda poco tóner",
        "Sin tóner",
        "Puerta o tapa abierta",
        "Atasco de papel",
        "Sin conexión",
        "Pide servicio técnico",
        "Falta una bandeja de entrada",
        "Falta la bandeja de salida",
        "Falta un consumible",
        "La bandeja de salida está casi llena",
        "La bandeja de salida está llena",
        "Una bandeja de entrada está vacía",
        "Toca el mantenimiento preventivo",
    ];
    let mut out = Vec::new();
    for (i, nombre) in NOMBRES.iter().enumerate() {
        let (byte, bit) = (i / 8, 7 - (i % 8));
        if bits.get(byte).is_some_and(|b| b & (1 << bit) != 0) {
            out.push((*nombre).to_string());
        }
    }
    out
}

/// Porcentaje de un consumible. La MIB usa negativos para «no se sabe» (-1,
/// -2) y -3 para «queda algo, pero no se sabe cuánto».
pub fn supply_percent(level: i64, max: i64) -> Option<u8> {
    if level < 0 || max <= 0 {
        return None;
    }
    Some(((level * 100) / max).clamp(0, 100) as u8)
}

/// Tipos de consumible de la MIB que se cambian a menudo: tóner (3, 21) y tinta (5, 6).
fn is_consumable(kind: i64, name: &str) -> bool {
    matches!(kind, 3 | 5 | 6 | 21) || {
        let n = name.to_lowercase();
        n.contains("toner") || n.contains("tóner") || n.contains("ink") || n.contains("tinta") || n.contains("cartridge") || n.contains("cartucho")
    }
}

fn identity(s: &mut Session) -> (String, String) {
    let text = |v: Option<Value>| v.and_then(|v| v.as_text()).unwrap_or_default();
    let name = text(s.get("1.3.6.1.2.1.1.5.0"));
    let mut model = text(s.get("1.3.6.1.2.1.25.3.2.1.3.1"));
    if model.is_empty() {
        model = text(s.get("1.3.6.1.2.1.1.1.0")).chars().take(80).collect();
    }
    (name, model)
}

/// Solo nombre y modelo (para la búsqueda en la red, que pregunta a muchas).
pub fn printer_identity(ip: IpAddr) -> Option<(String, String)> {
    Session::open(ip, Duration::from_millis(800)).map(|mut s| identity(&mut s))
}

/// Lo que se sabe de una impresora por SNMP. Si no contesta, tarda unos 5 s en rendirse.
pub fn printer_status(ip: IpAddr) -> PrinterStatus {
    let Some(mut s) = Session::open(ip, Duration::from_millis(1200)) else { return PrinterStatus::default() };
    let (name, model) = identity(&mut s);
    let pages = s.get("1.3.6.1.2.1.43.10.2.1.4.1.1").and_then(|v| v.as_i64()).filter(|p| *p >= 0).map(|p| p as u64);
    let alerts = match s.get("1.3.6.1.2.1.25.3.5.1.2.1") {
        Some(Value::Bytes(b)) => decode_alerts(&b),
        _ => Vec::new(),
    };
    let names = s.walk("1.3.6.1.2.1.43.11.1.1.6.1");
    let levels = s.walk("1.3.6.1.2.1.43.11.1.1.9.1");
    let maxes = s.walk("1.3.6.1.2.1.43.11.1.1.8.1");
    let kinds = s.walk("1.3.6.1.2.1.43.11.1.1.5.1");
    let find = |list: &[(Vec<u32>, Value)], idx: &[u32]| list.iter().find(|(i, _)| i == idx).and_then(|(_, v)| v.as_i64());
    let supplies = names
        .iter()
        .filter_map(|(idx, v)| {
            let name = v.as_text()?;
            let percent = supply_percent(find(&levels, idx).unwrap_or(-2), find(&maxes, idx).unwrap_or(-2));
            let consumable = is_consumable(find(&kinds, idx).unwrap_or(0), &name);
            (!name.is_empty()).then_some(Supply { name, percent, consumable })
        })
        .collect();
    PrinterStatus { reachable: true, name, model, pages, supplies, alerts }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OID codificado como en la RFC (1.3.6.1.2.1.1.5.0 → 06 08 2B 06 01 02 01 01 05 00),
    /// y con arcos de varios bytes.
    #[test]
    fn oids_encode_and_decode() {
        assert_eq!(encode_oid(&parse_oid("1.3.6.1.2.1.1.5.0")), [0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x05, 0x00]);
        let big = parse_oid("1.3.6.1.4.1.11.2.3.9.4.2.1.1.3.3.0");
        let enc = encode_oid(&big);
        assert_eq!(decode_oid(&enc[2..]), big);
        assert_eq!(decode_oid(&encode_oid(&[1, 3, 6, 1, 2, 1, 200_000])[2..]), [1, 3, 6, 1, 2, 1, 200_000]);
    }

    #[test]
    fn integers_keep_their_sign() {
        for n in [0i64, 1, 127, 128, 255, 256, -1, -2, -3, 65_536, -129] {
            let e = int(n);
            assert_eq!(decode_int(&e[2..]), n, "{n}");
        }
        assert_eq!(int(128), [0x02, 0x02, 0x00, 0x80]);
    }

    /// Una respuesta como la de una impresora de verdad: sysName = "HP-RECEPCION".
    #[test]
    fn a_real_looking_response_is_parsed() {
        let oid = parse_oid("1.3.6.1.2.1.1.5.0");
        let bind = tlv(0x30, &[encode_oid(&oid), tlv(0x04, b"HP-RECEPCION")].concat());
        let pdu = [int(42), int(0), int(0), tlv(0x30, &bind)].concat();
        let msg = tlv(0x30, &[int(1), tlv(0x04, b"public"), tlv(0xA2, &pdu)].concat());
        let (id, err, binds) = parse_response(&msg).unwrap();
        assert_eq!((id, err), (42, 0));
        assert_eq!(binds[0].0, oid);
        assert_eq!(binds[0].1.as_text().as_deref(), Some("HP-RECEPCION"));
        // Contadores (Counter32) sin signo.
        let bind = tlv(0x30, &[encode_oid(&oid), tlv(0x41, &[0x00, 0xC3, 0x50])].concat());
        let pdu = [int(7), int(0), int(0), tlv(0x30, &bind)].concat();
        let msg = tlv(0x30, &[int(1), tlv(0x04, b"public"), tlv(0xA2, &pdu)].concat());
        assert_eq!(parse_response(&msg).unwrap().2[0].1, Value::Int(50_000));
        // Basura: no revienta.
        assert!(parse_response(&[0x30, 0x82, 0xFF]).is_none());
        assert!(parse_response(&[]).is_none());
    }

    #[test]
    fn requests_are_well_formed() {
        let m = build_request(1, "public", 0xA0, 5, &[parse_oid("1.3.6.1.2.1.1.5.0")]);
        assert_eq!(m[0], 0x30);
        assert!(m.windows(6).any(|w| w == b"public"));
        assert!(m.contains(&0xA0));
    }

    /// hrPrinterDetectedErrorState: el bit 0 es el más alto del primer byte.
    #[test]
    fn printer_alerts_are_decoded() {
        assert_eq!(decode_alerts(&[0b0000_0100]), ["Atasco de papel"]);
        assert_eq!(decode_alerts(&[0b0101_0000]), ["Sin papel", "Sin tóner"]);
        assert_eq!(decode_alerts(&[0x00, 0b0000_1000]), ["La bandeja de salida está llena"]);
        assert!(decode_alerts(&[0x00]).is_empty());
        assert!(decode_alerts(&[]).is_empty());
    }

    #[test]
    fn supply_levels() {
        assert_eq!(supply_percent(800, 10_000), Some(8));
        assert_eq!(supply_percent(-3, 100), None, "queda algo, no se sabe cuánto");
        assert_eq!(supply_percent(50, -2), None);
        assert_eq!(supply_percent(150, 100), Some(100));
        assert!(is_consumable(3, "Black Toner"));
        assert!(is_consumable(0, "Cartucho de tinta cian"));
        assert!(!is_consumable(15, "Fuser Kit"));
    }
}
