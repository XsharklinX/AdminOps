//! Programas desactualizados con fallos conocidos: se cruzan los programas
//! instalados y sus versiones con vulnerabilidades públicas (CVE) y con los que
//! ya no tienen soporte. Se prioriza lo que se está explotando de verdad, no
//! cualquier aviso. La base viene con AdminOps y se actualiza con cada versión.

use serde::Serialize;

/// Hasta cuándo llega la base (se dice en pantalla para no dar falsa seguridad).
pub const DB_DATE: &str = "septiembre de 2025";

struct Rule {
    /// Trozo del nombre del programa (en minúsculas).
    name: &'static str,
    /// Si el nombre contiene esto, no es este programa.
    not: &'static [&'static str],
    /// Primera versión sin el fallo.
    fixed: &'static str,
    cve: &'static str,
    /// Se está explotando (catálogo KEV de CISA o ataques conocidos).
    exploited: bool,
    what: &'static str,
    winget: &'static str,
}

const RULES: &[Rule] = &[
    Rule { name: "7-zip", not: &[], fixed: "24.09", cve: "CVE-2025-0411", exploited: true, what: "Un archivo comprimido puede saltarse el aviso de «descargado de Internet» y ejecutar lo que lleva dentro.", winget: "7zip.7zip" },
    Rule { name: "winrar", not: &[], fixed: "7.13", cve: "CVE-2025-8088", exploited: true, what: "Un .rar preparado puede dejar programas en carpetas del sistema (se usó en ataques reales).", winget: "RARLab.WinRAR" },
    Rule { name: "putty", not: &["winscp", "filezilla"], fixed: "0.81", cve: "CVE-2024-31497", exploited: false, what: "Las claves NIST P-521 usadas con PuTTY pueden recuperarse: hay que actualizar y cambiar esas claves.", winget: "PuTTY.PuTTY" },
    Rule { name: "winscp", not: &[], fixed: "6.3.3", cve: "CVE-2024-31497", exploited: false, what: "Lleva el mismo fallo de PuTTY con las claves P-521.", winget: "WinSCP.WinSCP" },
    Rule { name: "filezilla", not: &["server"], fixed: "3.67.0", cve: "CVE-2024-31497", exploited: false, what: "Lleva el mismo fallo de PuTTY con las claves P-521.", winget: "TimKosse.FileZilla.Client" },
    Rule { name: "vlc media player", not: &[], fixed: "3.0.21", cve: "CVE-2024-46461", exploited: false, what: "Un vídeo o una emisión preparados pueden colgar VLC o ejecutar código.", winget: "VideoLAN.VLC" },
    Rule { name: "notepad++", not: &[], fixed: "8.5.7", cve: "CVE-2023-40031", exploited: false, what: "Abrir un archivo preparado puede ejecutar código.", winget: "Notepad++.Notepad++" },
    Rule { name: "keepass", not: &["keepassxc"], fixed: "2.54", cve: "CVE-2023-32784", exploited: false, what: "La contraseña maestra se puede recuperar de la memoria del equipo.", winget: "DominikReichl.KeePass" },
    Rule { name: "openvpn", not: &["connect"], fixed: "2.6.10", cve: "CVE-2024-27459", exploited: false, what: "Un usuario del equipo puede conseguir permisos de administrador a través de OpenVPN.", winget: "OpenVPNTechnologies.OpenVPN" },
    Rule { name: "git", not: &["github", "gitkraken", "tortoisegit", "digital"], fixed: "2.45.1", cve: "CVE-2024-32002", exploited: false, what: "Clonar un repositorio preparado puede ejecutar código.", winget: "Git.Git" },
    Rule { name: "teamviewer", not: &[], fixed: "15.58.4", cve: "CVE-2024-7479", exploited: false, what: "Un usuario del equipo puede conseguir permisos de administrador.", winget: "TeamViewer.TeamViewer" },
    Rule { name: "anydesk", not: &[], fixed: "8.0.8", cve: "Certificado revocado (2024)", exploited: true, what: "Las versiones anteriores van firmadas con un certificado robado a AnyDesk y revocado.", winget: "AnyDesk.AnyDesk" },
    Rule { name: "zoom", not: &["zoomit"], fixed: "5.16.5", cve: "CVE-2024-24691", exploited: false, what: "Un usuario del equipo puede conseguir permisos de administrador.", winget: "Zoom.Zoom" },
    Rule { name: "libreoffice", not: &[], fixed: "24.2.5", cve: "CVE-2024-6472", exploited: false, what: "Un documento con macros mal firmadas puede ejecutarlas.", winget: "TheDocumentFoundation.LibreOffice" },
    Rule { name: "mozilla firefox", not: &["esr"], fixed: "131.0.2", cve: "CVE-2024-9680", exploited: true, what: "Una web preparada puede ejecutar código (se usó en ataques reales).", winget: "Mozilla.Firefox" },
    Rule { name: "google chrome", not: &[], fixed: "140.0.7339.185", cve: "CVE-2025-10585", exploited: true, what: "Una web preparada puede ejecutar código (fallo de día cero explotado).", winget: "Google.Chrome" },
];

/// Programas que ya no reciben parches de seguridad. «a|b» = contiene «a» y «b»
/// («Microsoft Office Profesional Plus 2016»).
const EOL: &[(&str, &[&str], &str)] = &[
    ("microsoft office|2010", &[], "sin parches desde octubre de 2020"),
    ("microsoft office|2013", &[], "sin parches desde abril de 2023"),
    ("microsoft office|2016", &[], "sin parches desde octubre de 2025"),
    ("microsoft office|2019", &[], "sin parches desde octubre de 2025"),
    ("adobe reader xi", &[], "sin parches desde 2017"),
    ("adobe acrobat reader 2017", &[], "sin parches desde 2022"),
    ("adobe flash player", &[], "retirado en 2020"),
    ("microsoft silverlight", &[], "retirado en 2021"),
    ("quicktime", &[], "Apple dejó de publicar parches para Windows en 2016"),
    ("python 2.", &[], "sin soporte desde 2020"),
    ("java 7", &[], "sin parches públicos desde 2015"),
    ("java(tm) 7", &[], "sin parches públicos desde 2015"),
];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub name: String,
    pub version: String,
    /// Primera versión sin el fallo («» si es fin de soporte).
    pub fixed: String,
    pub cve: String,
    pub exploited: bool,
    pub what: String,
    /// Id de winget para actualizarlo («» si no se actualiza así).
    pub winget: String,
    pub eol: bool,
}

/// Las cifras de una versión («24.08 (x64)» → [24, 8]).
pub fn parts(v: &str) -> Vec<u64> {
    let start = v.find(|c: char| c.is_ascii_digit()).unwrap_or(v.len());
    v[start..].split(|c: char| !c.is_ascii_digit() && c != '.').next().unwrap_or("").split('.').filter(|p| !p.is_empty()).map(|p| p.parse().unwrap_or(0)).collect()
}

/// ¿`v` es anterior a `fixed`?
pub fn older(v: &str, fixed: &str) -> bool {
    let (a, b) = (parts(v), parts(fixed));
    if a.is_empty() {
        return false;
    }
    let n = a.len().max(b.len());
    for i in 0..n {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x < y;
        }
    }
    false
}

/// Lo que tiene fallos conocidos entre los programas instalados (nombre, versión).
pub fn check(programs: &[(String, String)]) -> Vec<Hit> {
    let mut out = Vec::new();
    for (name, version) in programs {
        let n = name.to_ascii_lowercase();
        if let Some(r) = RULES.iter().find(|r| n.contains(r.name) && !r.not.iter().any(|x| n.contains(x))) {
            // «Git» es una palabra corta: solo «Git» o «Git version…», no cualquier cosa que la contenga.
            if r.name == "git" && !(n == "git" || n.starts_with("git ") || n.starts_with("git version")) {
                continue;
            }
            if older(version, r.fixed) {
                out.push(Hit { name: name.clone(), version: version.clone(), fixed: r.fixed.into(), cve: r.cve.into(), exploited: r.exploited, what: r.what.into(), winget: r.winget.into(), eol: false });
            }
            continue;
        }
        if let Some((_, _, why)) = EOL.iter().find(|(p, not, _)| p.split('|').all(|x| n.contains(x)) && !not.iter().any(|x| n.contains(x))) {
            out.push(Hit { name: name.clone(), version: version.clone(), fixed: String::new(), cve: String::new(), exploited: false, what: format!("Ya no tiene soporte: {why}. Cualquier fallo nuevo queda sin arreglar."), winget: String::new(), eol: true });
        }
    }
    out.sort_by_key(|h| (!h.exploited, h.eol, h.name.to_ascii_lowercase()));
    out.dedup_by(|a, b| a.name == b.name && a.version == b.version);
    out
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VulnReport {
    pub hits: Vec<Hit>,
    pub checked: usize,
    pub db_date: &'static str,
}

#[tauri::command(async)]
pub fn vulnerable_programs() -> VulnReport {
    let list: Vec<(String, String)> = crate::programs::list().into_iter().filter_map(|p| p.version.clone().map(|v| (p.name.clone(), v))).collect();
    VulnReport { checked: list.len(), hits: check(&list), db_date: DB_DATE }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versiones() {
        assert_eq!(parts("24.08 (x64)"), vec![24, 8]);
        assert_eq!(parts("v3.0.20"), vec![3, 0, 20]);
        assert!(older("24.08", "24.09"));
        assert!(!older("24.09", "24.09"));
        assert!(!older("25.01", "24.09"));
        assert!(older("0.80", "0.81"));
        assert!(older("3.0.20.0", "3.0.21"));
        assert!(!older("", "1.0"));
    }

    #[test]
    fn encuentra_lo_explotado_primero() {
        let p = |n: &str, v: &str| (n.to_string(), v.to_string());
        let hits = check(&[p("7-Zip 19.00 (x64)", "19.00"), p("VLC media player", "3.0.20"), p("Microsoft Office Profesional Plus 2016", "16.0.4266.1001"), p("Git", "2.47.0"), p("GitHub Desktop", "3.0"), p("Google Chrome", "141.0.7390.55"), p("KeePassXC", "2.7.0")]);
        assert_eq!(hits[0].name, "7-Zip 19.00 (x64)");
        assert!(hits[0].exploited);
        assert!(hits.iter().any(|h| h.name.contains("VLC")));
        assert!(hits.iter().any(|h| h.eol && h.name.contains("2016")));
        assert!(!hits.iter().any(|h| h.name.contains("Git")));
        assert!(!hits.iter().any(|h| h.name.contains("Chrome")));
        assert!(!hits.iter().any(|h| h.name.contains("KeePass")));
    }
}
