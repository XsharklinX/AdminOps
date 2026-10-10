//! Licencias de Windows y Office, de verdad: qué tipo es cada una (OEM de la
//! placa, retail, por volumen, KMS), si la clave grabada en la placa coincide con
//! la edición instalada, cuándo caduca una activación KMS y qué pasa si se cambia
//! la placa o el disco. Las activaciones dudosas se dicen con tacto.

use super::parse;
use crate::troubleshoot::{finding, Finding};
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Product {
    pub name: String,
    pub description: String,
    pub channel: String,
    pub office: bool,
    /// 0 sin licencia · 1 con licencia · 2-6 periodos de gracia o notificación.
    pub status: u32,
    /// Minutos que quedan de gracia (KMS: hasta que caduca si no renueva).
    pub grace_minutes: u32,
    pub kms_host: String,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct LicRaw {
    pub edition: String,
    pub bios_key_desc: String,
    pub products: Vec<Product>,
    pub domain: bool,
    pub kms_tasks: u32,
    pub office_c2r: String,
    pub office_license_files: u32,
}

const SCRIPT: &str = r#"
$svc = Get-CimInstance SoftwareLicensingService -ErrorAction SilentlyContinue
$ps = @(Get-CimInstance SoftwareLicensingProduct -Filter "PartialProductKey IS NOT NULL" -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ name = "$($_.Name)"; description = "$($_.Description)"; channel = "$($_.ProductKeyChannel)"; office = ("$($_.ApplicationId)" -eq '0ff1ce15-a989-479d-af46-f275c6370663')
    status = [int]$_.LicenseStatus; graceMinutes = [int]$_.GracePeriodRemaining
    kmsHost = "$(if ($_.KeyManagementServiceMachine) { $_.KeyManagementServiceMachine } else { $_.DiscoveredKeyManagementServiceMachineName })" }
})
$tasks = @(Get-ScheduledTask -ErrorAction SilentlyContinue | Where-Object { $_.TaskName -match 'KMS' }).Count + @(Get-Service -ErrorAction SilentlyContinue | Where-Object { $_.Name -match 'KMS' }).Count
$c2r = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Office\ClickToRun\Configuration' -ErrorAction SilentlyContinue
[pscustomobject]@{
  edition = "$((Get-CimInstance Win32_OperatingSystem).Caption)"; biosKeyDesc = "$($svc.OA3xOriginalProductKeyDescription)"
  products = $ps; domain = [bool](Get-CimInstance Win32_ComputerSystem).PartOfDomain; kmsTasks = $tasks
  officeC2r = "$($c2r.ProductReleaseIds)"; officeLicenseFiles = @(Get-ChildItem "$UserLocalAppData\Microsoft\Office\Licenses" -Recurse -File -ErrorAction SilentlyContinue).Count
} | ConvertTo-Json -Depth 4 -Compress
"#;

/// El tipo de licencia en palabras.
pub fn kind_of(p: &Product) -> &'static str {
    let c = format!("{} {}", p.channel, p.description).to_ascii_uppercase();
    if c.contains("OEM:DM") {
        "OEM (grabada en la placa)"
    } else if c.contains("OEM") {
        "OEM (vino con el equipo)"
    } else if c.contains("RETAIL") {
        "Retail (comprada aparte)"
    } else if c.contains("MAK") {
        "Por volumen (MAK)"
    } else if c.contains("GVLK") || c.contains("KMS") {
        "Por volumen (KMS)"
    } else if c.contains("SUBSCRIPTION") || c.contains("TIMEBASED") {
        "Suscripción"
    } else {
        "Desconocido"
    }
}

/// Edición de Windows (Home, Pro, Enterprise…) en un texto cualquiera.
pub fn edition_of(text: &str) -> Option<&'static str> {
    let t = text.to_ascii_lowercase();
    // Del más concreto al más general: «Professional Education» antes que «Professional».
    for (needle, ed) in [("enterprise", "Enterprise"), ("education", "Education"), ("professional", "Pro"), (" pro", "Pro"), ("core", "Home"), ("home", "Home"), ("hogar", "Home")] {
        if t.contains(needle) {
            return Some(ed);
        }
    }
    None
}

/// Hosts de activación que no son de ninguna empresa (servidores públicos conocidos).
const PUBLIC_KMS: &[&str] = &["kms8.msguides.com", "kms.digiboy.ir", "kms.03k.org", "kms.loli.beer", "kms.srv.crsoo.com", "kms9.msguides.com", "kms.chinancce.com"];

/// ¿La activación KMS parece de fuera de una empresa?
pub fn suspicious_kms(p: &Product, domain: bool, kms_tasks: u32) -> bool {
    if !kind_of(p).contains("KMS") {
        return false;
    }
    let h = p.kms_host.to_ascii_lowercase();
    let loopback = h.starts_with("127.") || h == "localhost" || h.starts_with("::1") || h.starts_with("0.0.0.0");
    PUBLIC_KMS.iter().any(|k| h.starts_with(k)) || loopback || (!domain && kms_tasks > 0) || (!domain && h.is_empty() && p.status == 1)
}

fn status_label(s: u32) -> &'static str {
    match s {
        1 => "Activado",
        0 => "Sin licencia",
        2..=4 => "En periodo de gracia",
        5 => "En modo de aviso",
        6 => "Gracia ampliada",
        _ => "Estado desconocido",
    }
}

pub fn findings(raw: &LicRaw) -> Vec<Finding> {
    let mut out = Vec::new();
    let windows: Vec<&Product> = raw.products.iter().filter(|p| !p.office && p.description.to_ascii_lowercase().contains("windows")).collect();
    for p in &windows {
        let kind = kind_of(p);
        let level = if p.status == 1 { "ok" } else { "warn" };
        let mut detail = format!("{} · {}.", kind, status_label(p.status));
        if kind.contains("OEM (grabada") {
            detail.push_str(" Va ligada a la placa: con la misma placa se reactiva sola aunque se cambie el disco o se reinstale; con otra placa deja de valer.");
        } else if kind.starts_with("Retail") {
            detail.push_str(" Se puede pasar a otro equipo (desactivándola en este).");
        } else if kind.contains("KMS") && p.grace_minutes > 0 {
            detail.push_str(&format!(" Renueva contra el servidor de la empresa; si no lo encuentra, caduca en {} días.", p.grace_minutes / 1440));
        }
        out.push(finding(level, format!("{} — {}", raw.edition, status_label(p.status)), detail));
        if suspicious_kms(p, raw.domain, raw.kms_tasks) {
            out.push(finding("warn", "La activación de Windows no parece de una empresa", "Es una activación por volumen (KMS) en un equipo que no está en un dominio, o contra un servidor público. Suele ser una activación no oficial: conviene regularizarla con una licencia OEM o retail."));
        }
    }
    if let (Some(bios), Some(installed)) = (edition_of(&raw.bios_key_desc), edition_of(&raw.edition)) {
        if bios != installed {
            out.push(finding("warn", format!("La placa trae licencia de Windows {bios} y está instalado {installed}"), format!("La clave grabada en la placa ({}) es de otra edición. Si se reinstala, Windows se activará como {bios}; para {installed} hace falta su propia licencia.", raw.bios_key_desc)));
        } else {
            out.push(finding("info", "Clave de la placa", format!("La placa trae una licencia {} que coincide con la edición instalada.", raw.bios_key_desc)));
        }
    }
    let office: Vec<&Product> = raw.products.iter().filter(|p| p.office).collect();
    for p in &office {
        let name = p.name.split(',').next().unwrap_or(&p.name).trim();
        out.push(finding(if p.status == 1 { "ok" } else { "warn" }, format!("{name} — {}", status_label(p.status)), format!("{}.", kind_of(p))));
        if suspicious_kms(p, raw.domain, raw.kms_tasks) {
            out.push(finding("warn", format!("La activación de {name} no parece de una empresa"), "Office por volumen (KMS) fuera de un dominio o contra un servidor público. Conviene regularizarla."));
        }
    }
    if office.is_empty() && !raw.office_c2r.is_empty() {
        let ok = raw.office_license_files > 0;
        out.push(finding(if ok { "ok" } else { "warn" }, format!("Office: {}", raw.office_c2r), if ok { "Licencia de suscripción (Microsoft 365) del usuario: va con su cuenta, no con el equipo." } else { "No se ve la licencia de la suscripción en este usuario: inicia sesión en Office con la cuenta que la tiene (Archivo → Cuenta)." }));
    }
    if windows.is_empty() && office.is_empty() && raw.office_c2r.is_empty() {
        out.push(finding("warn", "No se pudo leer ninguna licencia", "Windows no contestó a la consulta de licencias. Prueba de nuevo como administrador."));
    }
    out
}

pub fn check() -> Result<Vec<Finding>, String> {
    let script = format!("{}{SCRIPT}", crate::target_user::script_prelude());
    let out = crate::pspool::query(&script, Some(Duration::from_secs(60)), "Licencias")?;
    Ok(findings(&parse::<LicRaw>(&out)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(channel: &str, status: u32, host: &str) -> Product {
        Product { name: "Windows(R), Professional edition".into(), description: format!("Windows(R) Operating System, {channel} channel"), channel: channel.into(), office: false, status, grace_minutes: 259_200, kms_host: host.into() }
    }

    #[test]
    fn tipos_de_licencia() {
        assert!(kind_of(&win("OEM:DM", 1, "")).contains("placa"));
        assert!(kind_of(&win("Retail", 1, "")).starts_with("Retail"));
        assert!(kind_of(&win("Volume:GVLK", 1, "")).contains("KMS"));
        assert!(kind_of(&win("Volume:MAK", 1, "")).contains("MAK"));
    }

    #[test]
    fn ediciones() {
        assert_eq!(edition_of("[4.0] Core OEM:DM"), Some("Home"));
        assert_eq!(edition_of("[4.0] Professional OEM:DM"), Some("Pro"));
        assert_eq!(edition_of("Microsoft Windows 11 Pro"), Some("Pro"));
        assert_eq!(edition_of("Microsoft Windows 11 Home"), Some("Home"));
        assert_eq!(edition_of("Microsoft Windows 11 Enterprise"), Some("Enterprise"));
    }

    #[test]
    fn kms_dudoso() {
        assert!(suspicious_kms(&win("Volume:GVLK", 1, "kms8.msguides.com"), false, 0));
        assert!(suspicious_kms(&win("Volume:GVLK", 1, "127.0.0.2"), false, 0));
        assert!(suspicious_kms(&win("Volume:GVLK", 1, "kms.empresa.local"), false, 1));
        assert!(!suspicious_kms(&win("Volume:GVLK", 1, "kms.empresa.local"), true, 0));
        assert!(!suspicious_kms(&win("OEM:DM", 1, ""), false, 0));
    }

    #[test]
    fn clave_de_la_placa_de_otra_edicion() {
        let raw = LicRaw { edition: "Microsoft Windows 11 Pro".into(), bios_key_desc: "[4.0] Core OEM:DM".into(), products: vec![win("Retail", 1, "")], ..Default::default() };
        let f = findings(&raw);
        assert!(f.iter().any(|x| x.level == "warn" && x.title.contains("Home") && x.title.contains("Pro")), "{f:?}");
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
