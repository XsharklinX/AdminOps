//! Impresoras a fondo: lo que más tiempo quita después de la cola y la red.
//! Drivers universales cuando hay uno específico, drivers de otra marca, la
//! directiva «Point and Print» que pide administrador al conectar una impresora
//! de un servidor, y la reinstalación limpia (quitar cola y driver, reiniciar el
//! servicio y volver a poner la impresora con el mismo puerto).

use super::parse;
use crate::troubleshoot::{finding, fix_confirm, Finding};
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Driver {
    pub name: String,
    pub major: u32,
    pub manufacturer: String,
    pub version: u64,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Printer {
    pub name: String,
    pub driver: String,
    pub port: String,
    /// Conectada desde un servidor de impresión (\\servidor\impresora).
    pub connection: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct PrintRaw {
    pub drivers: Vec<Driver>,
    pub printers: Vec<Printer>,
    /// RestrictDriverInstallationToAdministrators: None si no está (de fábrica, 1 desde 2021).
    pub restrict_admin: Option<u32>,
}

const SCRIPT: &str = r#"
$drv = @(Get-PrinterDriver -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ name = "$($_.Name)"; major = [int]$_.MajorVersion; manufacturer = "$($_.Manufacturer)"; version = [uint64]$_.DriverVersion } })
$prn = @(Get-Printer -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ name = "$($_.Name)"; driver = "$($_.DriverName)"; port = "$($_.PortName)"; connection = ("$($_.Type)" -eq 'Connection') } })
$pp = Get-ItemProperty 'HKLM:\Software\Policies\Microsoft\Windows NT\Printers\PointAndPrint' -ErrorAction SilentlyContinue
[pscustomobject]@{ drivers = $drv; printers = $prn; restrictAdmin = if ($pp -and $null -ne $pp.RestrictDriverInstallationToAdministrators) { [int]$pp.RestrictDriverInstallationToAdministrators } else { $null } } | ConvertTo-Json -Depth 4 -Compress
"#;

/// Versión de driver empaquetada en 64 bits → «6.1.7601.17514».
pub fn version_text(v: u64) -> String {
    if v == 0 {
        return String::new();
    }
    format!("{}.{}.{}.{}", v >> 48, (v >> 32) & 0xFFFF, (v >> 16) & 0xFFFF, v & 0xFFFF)
}

/// Driver genérico (sirve para muchas impresoras, pero pierde funciones: bandejas, grapado, nivel de tóner).
pub fn generic_driver(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("universal") || n.contains("class driver") || n.contains("ipp class") || n.contains("ps class") || n.contains("pcl6 class") || n.contains("generic") || n.contains("genérico")
}

const BRANDS: &[&str] = &["hp", "epson", "brother", "canon", "kyocera", "ricoh", "xerox", "lexmark", "konica", "samsung", "sharp", "oki", "toshiba", "dell", "pantum"];

/// Marca que aparece en un texto («HP LaserJet 400» → «hp»).
pub fn brand(text: &str) -> Option<&'static str> {
    let t = format!(" {} ", text.to_ascii_lowercase().replace(['-', '_'], " "));
    BRANDS.iter().copied().find(|b| t.contains(&format!(" {b} ")) || t.contains(&format!(" {b}")))
}

fn is_virtual(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["pdf", "xps", "onenote", "fax", "send to", "enviar a"].iter().any(|v| n.contains(v))
}

pub fn findings(raw: &PrintRaw) -> Vec<Finding> {
    let mut out = Vec::new();
    for p in raw.printers.iter().filter(|p| !is_virtual(&p.name)) {
        let reinstall = fix_confirm(format!("pr.reinstall:{}", p.name), "Reinstalación limpia", true, "Se guarda la configuración de la impresora, se quita junto con su driver, se reinicia la cola de impresión y se vuelve a poner con el mismo puerto. Tarda un minuto.");
        let (pb, db) = (brand(&p.name), brand(&p.driver));
        if let (Some(pb), Some(db)) = (pb, db) {
            if pb != db {
                out.push(finding("warn", format!("{} usa un driver de otra marca", p.name), format!("Driver: «{}». Con el de otra marca suele imprimir mal o no imprimir. Instala el del fabricante y haz una reinstalación limpia.", p.driver)).fixes(vec![reinstall.clone()]).page("printers"));
                continue;
            }
        }
        if generic_driver(&p.driver) && !p.connection {
            let v = raw.drivers.iter().find(|d| d.name == p.driver).map(|d| version_text(d.version)).unwrap_or_default();
            out.push(finding("info", format!("{} usa un driver genérico", p.name), format!("«{}»{}. Imprime, pero sin las funciones propias del modelo (bandejas, dúplex, nivel de tóner). Si fallan, instala el driver específico del fabricante.", p.driver, if v.is_empty() { String::new() } else { format!(" ({v})") })).page("printers"));
        }
    }
    let connections = raw.printers.iter().filter(|p| p.connection).count();
    let restrict = raw.restrict_admin.unwrap_or(1) != 0;
    if restrict && connections > 0 {
        out.push(finding("info", "Point and Print: instalar desde el servidor pide administrador", "Desde 2021 (PrintNightmare) Windows pide administrador para instalar el driver de una impresora de un servidor. Es la causa de «necesitas permisos» al conectar una impresora de red. Instálala una vez como administrador, o que el administrador del dominio despliegue los drivers.").page("printers"));
    } else if raw.restrict_admin == Some(0) {
        out.push(finding("warn", "Point and Print permite instalar drivers sin administrador", "La directiva está relajada: cualquier usuario puede instalar drivers de impresora desde un servidor, una vía conocida de ataque (PrintNightmare). Déjala como viene de fábrica salvo que el dominio lo necesite."));
    }
    let used: std::collections::HashSet<&str> = raw.printers.iter().map(|p| p.driver.as_str()).collect();
    let orphans: Vec<&Driver> = raw.drivers.iter().filter(|d| !used.contains(d.name.as_str()) && !d.manufacturer.eq_ignore_ascii_case("Microsoft") && !is_virtual(&d.name)).collect();
    if orphans.len() >= 3 {
        let names: Vec<&str> = orphans.iter().take(5).map(|d| d.name.as_str()).collect();
        out.push(finding("info", format!("{} driver(s) de impresora sin usar", orphans.len()), format!("{}. Restos de impresoras antiguas: no molestan, pero si se mezclan versiones puede elegirse el que no toca.", names.join(", "))));
    }
    out
}

/// Lo que añade a la comprobación de impresoras de «Solucionar problemas».
pub fn check() -> Vec<Finding> {
    crate::pspool::query(SCRIPT, Some(Duration::from_secs(40)), "Solucionar: impresoras a fondo").ok().and_then(|o| parse::<PrintRaw>(&o).ok()).map(|r| findings(&r)).unwrap_or_default()
}

const REINSTALL: &str = r#"
$ErrorActionPreference = 'Stop'
$p = Get-Printer -Name $name
$conn = "$($p.Type)" -eq 'Connection'
$cfg = [pscustomobject]@{ name = $p.Name; driver = $p.DriverName; port = $p.PortName; shared = $p.Shared; share = $p.ShareName; location = $p.Location; comment = $p.Comment }
Remove-Printer -Name $name
if (-not $conn -and -not (Get-Printer -ErrorAction SilentlyContinue | Where-Object { $_.DriverName -eq $cfg.driver })) { Remove-PrinterDriver -Name $cfg.driver -ErrorAction SilentlyContinue }
Restart-Service Spooler -Force
Start-Sleep -Seconds 2
if ($conn) {
  Add-Printer -ConnectionName $cfg.name
} else {
  if (-not (Get-PrinterDriver -Name $cfg.driver -ErrorAction SilentlyContinue)) { Add-PrinterDriver -Name $cfg.driver }
  $opts = @{ Name = $cfg.name; DriverName = $cfg.driver; PortName = $cfg.port }
  if ($cfg.location) { $opts.Location = $cfg.location }
  if ($cfg.comment) { $opts.Comment = $cfg.comment }
  Add-Printer @opts
  if ($cfg.shared) { Set-Printer -Name $cfg.name -Shared $true -ShareName $cfg.share }
}
'ok'
"#;

fn reinstall(name: &str) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if name.trim().is_empty() || name.len() > 200 {
        return Err("Nombre de impresora no válido.".into());
    }
    crate::ps::powershell(&format!("{}{REINSTALL}", crate::ps::text_var("name", name))).map_err(|e| format!("No se pudo reinstalar «{name}»: {e}. Si el driver ya no está en el equipo, instálalo del fabricante y vuelve a añadir la impresora."))?;
    Ok(format!("«{name}» reinstalada con su driver y su puerto. Imprime una página de prueba."))
}

pub fn run(kind: &str, arg: &str) -> Option<Result<String, String>> {
    (kind == "pr.reinstall").then(|| reinstall(arg))
}

pub fn title(kind: &str) -> Option<&'static str> {
    (kind == "pr.reinstall").then_some("Reinstalación limpia de una impresora")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_y_marcas() {
        assert_eq!(version_text((6u64 << 48) | (1 << 32) | (7601 << 16) | 17514), "6.1.7601.17514");
        assert_eq!(brand("HP LaserJet 400 M401"), Some("hp"));
        assert_eq!(brand("Kyocera ECOSYS P2135d KX"), Some("kyocera"));
        assert_eq!(brand("Impresora de recepción"), None);
        assert!(generic_driver("HP Universal Printing PCL 6"));
        assert!(generic_driver("Microsoft IPP Class Driver"));
        assert!(!generic_driver("HP LaserJet 400 M401 PCL 6"));
    }

    #[test]
    fn driver_de_otra_marca_y_point_and_print() {
        let raw = PrintRaw {
            drivers: vec![Driver { name: "Kyocera KX".into(), major: 3, manufacturer: "Kyocera".into(), version: 0 }],
            printers: vec![Printer { name: "HP LaserJet 400".into(), driver: "Kyocera KX".into(), port: "IP_192.168.1.50".into(), connection: false }, Printer { name: r"\\SRV\Recepcion".into(), driver: "Canon Generic".into(), port: String::new(), connection: true }],
            restrict_admin: None,
        };
        let f = findings(&raw);
        assert!(f.iter().any(|x| x.level == "warn" && x.title.contains("otra marca") && x.fixes[0].id == "pr.reinstall:HP LaserJet 400"), "{f:?}");
        assert!(f.iter().any(|x| x.title.contains("Point and Print")));
    }

    #[test]
    fn scripts_parse() {
        for s in [SCRIPT, REINSTALL] {
            let e = crate::ps::parse_errors(s);
            assert!(e.is_empty(), "{e}");
        }
    }
}
