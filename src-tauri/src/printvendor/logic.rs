//! Lógica pura de impresoras y escáneres RICOH y EPSON: versiones, marca y
//! modelo, tipo de conexión y estado del driver. Sin PowerShell ni disco, para
//! poder probarla entera.

use crate::fixes::printdeep::{brand, generic_driver};
use serde::Serialize;
use std::cmp::Ordering;

// ---------- Versiones ----------

/// «4.42.0.0» → `[4, 42, 0, 0]`. Acepta de 1 a 4 números (los que faltan valen 0).
pub fn parse_version(s: &str) -> Option<[u32; 4]> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut out = [0u32; 4];
    for (i, part) in s.split('.').enumerate() {
        *out.get_mut(i)? = part.trim().parse().ok()?;
    }
    Some(out)
}

/// Compara dos versiones de cuatro números. `None` si alguna no se entiende.
pub fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    Some(parse_version(a)?.cmp(&parse_version(b)?))
}

/// Una versión que se pueda comparar y que no sea la «0.0.0.0» de «no se sabe».
pub fn usable_version(s: &str) -> bool {
    parse_version(s).is_some_and(|v| v != [0; 4])
}

/// La última versión con forma `1.2.3` o `1.2.3.4` que aparezca en un texto
/// (títulos de Windows Update como «RICOH - Printer - 4.42.0.0»).
pub fn extract_version(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|t| t.trim_matches('.'))
        .rfind(|t| t.matches('.').count() >= 2 && parse_version(t).is_some())
        .map(String::from)
}

// ---------- Marca y modelo ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vendor {
    Ricoh,
    Epson,
    Other,
}

impl Vendor {
    pub fn id(self) -> &'static str {
        match self {
            Vendor::Ricoh => "ricoh",
            Vendor::Epson => "epson",
            Vendor::Other => "otra",
        }
    }

    pub fn from_brand(b: Option<&str>) -> Vendor {
        match b {
            Some("ricoh") => Vendor::Ricoh,
            Some("epson") => Vendor::Epson,
            _ => Vendor::Other,
        }
    }
}

/// Marca por el ID de hardware USB: VID_05CA es RICOH, VID_04B8 es EPSON y
/// VID_04C5 es Fujitsu/PFU (los fi y ScanSnap, ahora RICOH).
pub fn brand_from_hwid(id: &str) -> Option<&'static str> {
    let u = id.to_ascii_uppercase();
    if u.contains("VID_05CA") || u.contains("VID_04C5") {
        Some("ricoh")
    } else if u.contains("VID_04B8") {
        Some("epson")
    } else {
        None
    }
}

/// «VID_04B8&PID_1234» → «04B8:1234».
pub fn usb_id(hwid: &str) -> Option<String> {
    let u = hwid.to_ascii_uppercase();
    let vid = u.split("VID_").nth(1)?.get(..4)?;
    let pid = u.split("PID_").nth(1)?.get(..4)?;
    (vid.chars().all(|c| c.is_ascii_hexdigit()) && pid.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("{vid}:{pid}"))
}

/// Marca que aparece en un texto. PFU, Fujitsu (fi-7160), ScanSnap y PaperStream
/// cuentan como RICOH; el resto de marcas las conoce `printdeep::brand`.
pub fn brand_key(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    let tokens: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).collect();
    for (i, t) in tokens.iter().enumerate() {
        if t.starts_with("ricoh") || t.starts_with("scansnap") || t.starts_with("paperstream") || *t == "pfu" || t.starts_with("fujitsu") {
            return Some("ricoh");
        }
        // fi-7160, fi 7300NX…
        if *t == "fi" && tokens.get(i + 1).is_some_and(|n| n.chars().next().is_some_and(|c| c.is_ascii_digit())) {
            return Some("ricoh");
        }
        if t.starts_with("epson") {
            return Some("epson");
        }
    }
    brand(text)
}

/// Marca de un aparato: primero el ID de hardware y, si no, el primer texto
/// (nombre del SNMP, de la impresora, del driver) que diga una marca. Esa marca
/// manda aunque no sea RICOH ni EPSON: un nombre «HP LaserJet» no se convierte
/// en RICOH porque su driver sea de RICOH.
pub fn identify(hwids: &[String], texts: &[&str]) -> Option<&'static str> {
    hwids.iter().find_map(|h| brand_from_hwid(h)).or_else(|| texts.iter().find_map(|t| brand_key(t)))
}

const NOISE: &[&str] = &[
    "series", "pcl", "pcl5", "pcl5e", "pcl5c", "pcl6", "pcl6xl", "ps", "ps3", "ps2", "postscript", "escp", "esc/p", "esc/p-r", "escpr", "driver", "for", "universal", "print", "printer",
    "class", "ipp", "wsd", "usb", "scanner", "wia", "twain",
];
const BRAND_WORDS: &[&str] = &["ricoh", "epson", "pfu", "fujitsu"];

/// «RICOH IM C3000 PCL 6» → «IM C3000»; «EPSON ET-2820 Series» → «ET-2820».
/// Quita la marca, lo que pone Windows al duplicar («(Copia 2)») y el sufijo del
/// lenguaje de impresión. Si todo era ruido (driver universal), queda vacío.
pub fn clean_model(name: &str) -> String {
    // Impresora de un servidor: \\SERVIDOR\Recepcion → Recepcion.
    let name = if name.starts_with("\\\\") { name.rsplit('\\').next().unwrap_or(name) } else { name };
    let mut plain = String::new();
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let mut tokens: Vec<&str> = plain.split_whitespace().collect();
    // Lo que va después de la marca.
    if let Some(i) = tokens.iter().position(|t| BRAND_WORDS.contains(&t.trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase().as_str())) {
        if i + 1 < tokens.len() {
            tokens.drain(..=i);
        }
    }
    while let Some(last) = tokens.last().map(|t| t.to_ascii_lowercase()) {
        let prev = tokens.len().checked_sub(2).map(|i| tokens[i].to_ascii_lowercase());
        let level = matches!(last.as_str(), "6" | "5" | "5e" | "5c" | "3" | "2") && prev.as_deref().is_some_and(|p| matches!(p, "pcl" | "ps"));
        let dash = last == "-" || last == "–";
        if NOISE.contains(&last.as_str()) || level || dash {
            tokens.pop();
        } else {
            break;
        }
    }
    tokens.retain(|t| !BRAND_WORDS.contains(&t.to_ascii_lowercase().as_str()));
    tokens.join(" ")
}

/// El modelo en minúsculas y solo letras y números, para compararlo.
pub fn norm_model(s: &str) -> String {
    clean_model(s).chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// ¿Es el mismo modelo? Igualdad exacta del nombre limpio: «C300» no es «C3000».
pub fn same_model(a: &str, b: &str) -> bool {
    let (na, nb) = (norm_model(a), norm_model(b));
    na.len() >= 2 && na == nb
}

/// Elige de la lista de modelos de un inf el que hay que instalar: el del mismo
/// modelo, luego el mismo driver que ya tiene (actualizarlo en el sitio) y, por
/// último, el driver universal. Devuelve el nombre y por qué.
pub fn pick_model(models: &[String], device_model: &str, current_driver: &str) -> Option<(String, &'static str)> {
    if !device_model.trim().is_empty() {
        if let Some(m) = models.iter().find(|m| same_model(m, device_model)) {
            return Some((m.clone(), "modelo"));
        }
    }
    if !current_driver.trim().is_empty() {
        if let Some(m) = models.iter().find(|m| m.eq_ignore_ascii_case(current_driver.trim())) {
            return Some((m.clone(), "mismoDriver"));
        }
    }
    models.iter().find(|m| m.to_ascii_lowercase().contains("universal")).map(|m| (m.clone(), "universal"))
}

/// ¿Coincide algún ID de hardware (sin mirar mayúsculas)?
pub fn hw_match(device: &[String], other: &str) -> bool {
    let o = other.trim();
    !o.is_empty() && device.iter().any(|d| d.trim().eq_ignore_ascii_case(o))
}

// ---------- Conexión ----------

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    /// "usb" | "red" | "wsd" | "servidor" | "otra"
    pub kind: String,
    /// Dirección del puerto, si es de red.
    pub ip: Option<String>,
    /// Nombre del puerto de Windows (USB001, IP_192.168.1.30…).
    pub port: String,
}

fn looks_ipv4(s: &str) -> bool {
    s.parse::<std::net::Ipv4Addr>().is_ok()
}

/// Cómo está conectada una impresora, según su puerto.
pub fn connection_of(port: &str, host_address: &str, server_connection: bool) -> Connection {
    let port = port.trim();
    let upper = port.to_ascii_uppercase();
    let host = host_address.trim();
    let (kind, ip) = if server_connection || port.starts_with("\\\\") {
        ("servidor", None)
    } else if upper.starts_with("WSD") {
        ("wsd", None)
    } else if !host.is_empty() {
        ("red", Some(host.to_string()))
    } else if looks_ipv4(port) {
        ("red", Some(port.to_string()))
    } else if let Some(rest) = upper.strip_prefix("IP_").filter(|r| looks_ipv4(r)) {
        ("red", Some(rest.to_string()))
    } else if upper.starts_with("USB") || upper.starts_with("DOT4") || upper.contains("USBPRINT") {
        ("usb", None)
    } else {
        ("otra", None)
    };
    Connection { kind: kind.into(), ip, port: port.to_string() }
}

// ---------- Estado del driver ----------

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    AlDia,
    Viejo,
    Generico,
    Falta,
    OtraMarca,
    SinComparar,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::AlDia => "Al día",
            Status::Viejo => "Viejo",
            Status::Generico => "Genérico",
            Status::Falta => "Falta",
            Status::OtraMarca => "De otra marca",
            Status::SinComparar => "Sin comparar",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverClass {
    /// No hay driver.
    None,
    /// De Windows o genérico (clase IPP, «Generic / Text Only»…).
    Generic,
    /// De una marca distinta a la del aparato.
    OtherBrand,
    /// Del fabricante (incluye su driver universal).
    Vendor,
}

impl DriverClass {
    pub fn id(self) -> &'static str {
        match self {
            DriverClass::None => "ninguno",
            DriverClass::Generic => "generico",
            DriverClass::OtherBrand => "otraMarca",
            DriverClass::Vendor => "fabricante",
        }
    }
}

/// Qué clase de driver es. El «Universal» de la propia marca (RICOH PCL6 Driver
/// for Universal Print) es del fabricante: es justo el que RICOH recomienda.
pub fn classify_driver(device_brand: Option<&str>, driver_name: &str, manufacturer: &str) -> DriverClass {
    let name = driver_name.trim();
    if name.is_empty() {
        return DriverClass::None;
    }
    let driver_brand = brand_key(name).or_else(|| brand_key(manufacturer));
    if let (Some(d), Some(b)) = (device_brand, driver_brand) {
        if d != b {
            return DriverClass::OtherBrand;
        }
    }
    let lower = name.to_ascii_lowercase();
    let microsoft = manufacturer.trim().eq_ignore_ascii_case("microsoft") || lower.contains("microsoft");
    let generic = generic_driver(name) && !(lower.contains("universal") && driver_brand.is_some());
    if microsoft || generic {
        DriverClass::Generic
    } else {
        DriverClass::Vendor
    }
}

/// Estado del driver instalado frente a la mejor versión que se conoce.
/// Sin nada con qué comparar es «sin comparar», nunca «al día».
pub fn driver_status(device_brand: Option<&str>, driver_name: &str, manufacturer: &str, installed_version: &str, best_candidate: Option<&str>) -> Status {
    match classify_driver(device_brand, driver_name, manufacturer) {
        DriverClass::None => Status::Falta,
        DriverClass::OtherBrand => Status::OtraMarca,
        DriverClass::Generic => Status::Generico,
        DriverClass::Vendor => {
            let (Some(best), true) = (best_candidate, usable_version(installed_version)) else { return Status::SinComparar };
            match compare_versions(best, installed_version) {
                Some(Ordering::Greater) => Status::Viejo,
                Some(_) => Status::AlDia,
                None => Status::SinComparar,
            }
        }
    }
}

/// La versión más alta de una lista (ignora las que no se entienden).
pub fn best_version<'a>(versions: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    versions.into_iter().filter(|v| usable_version(v)).max_by(|a, b| compare_versions(a, b).unwrap_or(Ordering::Equal))
}

/// Programa de escaneo por su nombre en «Programas y características».
pub fn scan_software_id(display_name: &str) -> Option<&'static str> {
    let n = display_name.to_ascii_lowercase();
    if n.contains("epson scan 2") || n.contains("epsonscan2") || n.contains("epson scan2") {
        Some("epsonscan2")
    } else if n.contains("paperstream capture") {
        Some("paperstream-capture")
    } else if n.contains("paperstream ip") {
        Some("paperstream-ip")
    } else if n.contains("scansnap home") {
        Some("scansnap-home")
    } else {
        None
    }
}

/// Mensaje en español para un código de error de WIA (HRESULT en hexadecimal).
pub fn wia_error(code: &str) -> String {
    let text = match code.trim_start_matches("0x").to_ascii_uppercase().as_str() {
        "80210015" => "No hay ningún escáner disponible. Revisa que esté encendido y conectado.",
        "80210001" => "El escáner dio un error. Apágalo y vuelve a encenderlo.",
        "80210002" => "Hay un atasco de papel en el escáner.",
        "80210003" => "No hay papel en el escáner.",
        "80210004" => "No se puede hablar con el escáner. Revisa el cable o la red.",
        "80210005" => "El escáner está ocupado con otro trabajo. Espera un momento y reinténtalo.",
        "80210006" => "El escáner no está listo. Espera a que termine de calentarse.",
        "80210007" => "El escáner está calentándose. Reinténtalo en unos segundos.",
        "80210016" => "La tapa del escáner está abierta.",
        "80210017" => "El escáner tiene la lámpara apagada o dañada.",
        "80210009" | "8021000A" => "El escáner está sin conexión o no responde.",
        "80210064" => "El escaneo se canceló.",
        _ => "El escáner no respondió a la prueba.",
    };
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_four_numbers() {
        assert_eq!(compare_versions("4.42.0.0", "4.9.0.0"), Some(Ordering::Greater), "42 > 9, no es texto");
        assert_eq!(compare_versions("1.0", "1.0.0.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("10.0.19041.1", "10.0.19041.2"), Some(Ordering::Less));
        assert_eq!(compare_versions("1.2.3.4.5", "1.2.3.4"), None, "más de cuatro números no se entiende");
        assert_eq!(compare_versions("", "1.0.0.0"), None);
        assert_eq!(compare_versions("a.b", "1.0"), None);
        assert!(!usable_version("0.0.0.0"));
        assert!(usable_version("2.0"));
        assert_eq!(extract_version("RICOH - Printer - 4.42.0.0"), Some("4.42.0.0".into()));
        assert_eq!(extract_version("Epson - Printer - 9/15/2023 12:00:00 AM - 1.2.3.4"), Some("1.2.3.4".into()));
        assert_eq!(extract_version("Sin versión 9/9/2022"), None);
        assert_eq!(best_version(["1.0.0.0", "bad", "2.1.0.0", "0.0.0.0"]), Some("2.1.0.0"));
        assert_eq!(best_version(Vec::<&str>::new()), None);
    }

    #[test]
    fn brand_comes_from_hardware_id_then_names() {
        assert_eq!(brand_from_hwid(r"USB\VID_05CA&PID_0418"), Some("ricoh"));
        assert_eq!(brand_from_hwid(r"usb\vid_04b8&pid_1100"), Some("epson"));
        assert_eq!(brand_from_hwid(r"USB\VID_04C5&PID_132B"), Some("ricoh"), "PFU/Fujitsu es RICOH");
        assert_eq!(brand_from_hwid(r"USB\VID_03F0&PID_0001"), None);
        assert_eq!(usb_id(r"USB\VID_04B8&PID_1100&REV_0100"), Some("04B8:1100".into()));
        assert_eq!(usb_id("nada"), None);

        assert_eq!(brand_key("RICOH IM C3000 PCL 6"), Some("ricoh"));
        assert_eq!(brand_key("ScanSnap iX1600"), Some("ricoh"));
        assert_eq!(brand_key("fi-7160"), Some("ricoh"));
        assert_eq!(brand_key("PFU ScanSnap S1300i"), Some("ricoh"));
        assert_eq!(brand_key("EPSON ET-2820 Series"), Some("epson"));
        assert_eq!(brand_key("HP LaserJet Pro M404"), Some("hp"));
        assert_eq!(brand_key("Recepción"), None);
        assert_eq!(brand_key("Wifi 7160"), None, "«fi» solo cuenta suelto y seguido de cifras");

        // El ID de hardware manda; después, el primer texto con marca.
        assert_eq!(identify(&[r"USB\VID_04B8&PID_1".to_string()], &["Recepción"]), Some("epson"));
        assert_eq!(identify(&[], &["Recepción", "EPSON L3250 Series", "RICOH PCL6"]), Some("epson"));
        assert_eq!(identify(&[], &["HP LaserJet", "RICOH PCL6 Driver"]), Some("hp"), "el driver no cambia la marca del aparato");
        assert_eq!(identify(&[], &["Recepción", "Microsoft IPP Class Driver"]), None);
        assert_eq!(Vendor::from_brand(Some("hp")), Vendor::Other);
        assert_eq!(Vendor::from_brand(Some("ricoh")).id(), "ricoh");
    }

    #[test]
    fn model_is_cleaned_and_compared_exactly() {
        assert_eq!(clean_model("RICOH IM C3000 PCL 6"), "IM C3000");
        assert_eq!(clean_model("Ricoh MP C307 PS"), "MP C307");
        assert_eq!(clean_model("EPSON ET-2820 Series"), "ET-2820");
        assert_eq!(clean_model("EPSON L3250 Series (Copiar 2)"), "L3250");
        assert_eq!(clean_model("Recepción RICOH IM C3000"), "IM C3000");
        assert_eq!(clean_model(r"\\SRV\RICOH IM 2500"), "IM 2500");
        assert_eq!(clean_model("RICOH PCL6 Driver for Universal Print"), "");
        assert_eq!(clean_model("ScanSnap iX1600"), "ScanSnap iX1600");
        assert_eq!(clean_model("Microsoft IPP Class Driver"), "Microsoft");
        assert!(same_model("RICOH IM C3000 PCL 6", "IM C3000"));
        assert!(same_model("EPSON ET-2820 Series", "Epson ET 2820"));
        assert!(!same_model("RICOH IM C300", "RICOH IM C3000"));
        assert!(!same_model("", ""));

        let models = ["RICOH PCL6 Driver for Universal Print".to_string(), "RICOH IM C3000 PCL 6".to_string()];
        assert_eq!(pick_model(&models, "IM C3000", ""), Some(("RICOH IM C3000 PCL 6".into(), "modelo")));
        assert_eq!(pick_model(&models, "IM C4500", "ricoh pcl6 driver for universal print"), Some((models[0].clone(), "mismoDriver")));
        assert_eq!(pick_model(&models, "IM C4500", "otro"), Some((models[0].clone(), "universal")));
        assert_eq!(pick_model(&models[1..], "IM C4500", ""), None);
        assert!(hw_match(&["USBPRINT\\RICOHIM_C3000".into()], "usbprint\\ricohim_c3000"));
        assert!(!hw_match(&[], "x"));
    }

    #[test]
    fn connection_is_read_from_the_port() {
        let c = connection_of("IP_192.168.1.30", "", false);
        assert_eq!((c.kind.as_str(), c.ip.as_deref()), ("red", Some("192.168.1.30")));
        assert_eq!(connection_of("192.168.1.31", "", false).ip.as_deref(), Some("192.168.1.31"));
        assert_eq!(connection_of("Puerto1", "10.0.0.5", false).ip.as_deref(), Some("10.0.0.5"));
        assert_eq!(connection_of("USB001", "", false).kind, "usb");
        assert_eq!(connection_of("WSD-1234-5678", "", false).kind, "wsd");
        assert_eq!(connection_of(r"\\SRV\Cola", "", true).kind, "servidor");
        assert_eq!(connection_of("LPT1:", "", false).kind, "otra");
    }

    #[test]
    fn driver_status_never_says_up_to_date_without_comparing() {
        fn s(brand: Option<&str>, name: &str, mfr: &str, inst: &str, best: Option<&str>) -> Status {
            driver_status(brand, name, mfr, inst, best)
        }
        // Sin candidato: sin comparar, no «al día».
        assert_eq!(s(Some("ricoh"), "RICOH IM C3000 PCL 6", "RICOH", "4.30.0.0", None), Status::SinComparar);
        // Versión instalada que no se entiende (o 0.0.0.0): tampoco.
        assert_eq!(s(Some("ricoh"), "RICOH IM C3000 PCL 6", "RICOH", "", Some("4.42.0.0")), Status::SinComparar);
        assert_eq!(s(Some("ricoh"), "RICOH IM C3000 PCL 6", "RICOH", "0.0.0.0", Some("4.42.0.0")), Status::SinComparar);
        assert_eq!(s(Some("ricoh"), "RICOH IM C3000 PCL 6", "RICOH", "4.30.0.0", Some("4.42.0.0")), Status::Viejo);
        assert_eq!(s(Some("ricoh"), "RICOH IM C3000 PCL 6", "RICOH", "4.42.0.0", Some("4.42.0.0")), Status::AlDia);
        assert_eq!(s(Some("ricoh"), "RICOH IM C3000 PCL 6", "RICOH", "5.0.0.0", Some("4.42.0.0")), Status::AlDia);
        assert_eq!(s(Some("epson"), "Microsoft IPP Class Driver", "Microsoft", "10.0.1.1", Some("9.0.0.0")), Status::Generico);
        assert_eq!(s(Some("epson"), "EPSON Universal Print Driver", "SEIKO EPSON", "1.0.0.0", Some("2.0.0.0")), Status::Viejo, "el universal de la marca es del fabricante");
        assert_eq!(s(Some("ricoh"), "EPSON ET-2820 Series", "EPSON", "1.0.0.0", Some("2.0.0.0")), Status::OtraMarca);
        assert_eq!(s(Some("ricoh"), "HP Universal Printing PCL 6", "HP", "1.0.0.0", None), Status::OtraMarca);
        assert_eq!(s(Some("ricoh"), "", "", "", None), Status::Falta);
        // Sin marca conocida en el aparato, un driver del fabricante se compara.
        assert_eq!(s(None, "RICOH PCL6 Driver for Universal Print", "RICOH", "4.0.0.0", Some("4.42.0.0")), Status::Viejo);
        assert_eq!(classify_driver(Some("ricoh"), "Generic / Text Only", "Microsoft").id(), "generico");
    }

    #[test]
    fn software_and_wia_errors() {
        assert_eq!(scan_software_id("EPSON Scan 2"), Some("epsonscan2"));
        assert_eq!(scan_software_id("PaperStream Capture"), Some("paperstream-capture"));
        assert_eq!(scan_software_id("PaperStream IP (TWAIN)"), Some("paperstream-ip"));
        assert_eq!(scan_software_id("ScanSnap Home"), Some("scansnap-home"));
        assert_eq!(scan_software_id("Epson Printer Connect"), None);
        assert!(wia_error("0x80210003").contains("papel"));
        assert!(wia_error("80210015").contains("disponible"));
        assert!(wia_error("zzz").contains("no respondió"));
    }
}
