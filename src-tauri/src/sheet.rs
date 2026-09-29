//! Ficha del equipo para el inventario de la empresa: modelo, número de serie,
//! hardware, Windows y licencia, red, usuario y enlace a la garantía del
//! fabricante. Rápida (una consulta), sin pasar por el diagnóstico completo.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Sheet {
    pub host: String,
    pub user: String,
    pub domain: String,
    pub part_of_domain: bool,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    /// Portátil | Sobremesa | Todo en uno | Mini PC | Servidor | Virtual
    pub chassis: String,
    pub cpu: String,
    pub cores: u32,
    pub ram_gb: f64,
    pub disks: String,
    pub gpu: String,
    pub os: String,
    pub os_version: String,
    pub installed: String,
    /// Activado | Sin activar | Desconocido
    pub license: String,
    pub bios: String,
    pub tpm: Option<bool>,
    pub secure_boot: Option<bool>,
    pub ip: String,
    pub mac: String,
    pub warranty_url: Option<String>,
}

const SCRIPT: &str = r#"
$cs = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue
$os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
$bios = Get-CimInstance Win32_BIOS -ErrorAction SilentlyContinue
$cpu = Get-CimInstance Win32_Processor -ErrorAction SilentlyContinue | Select-Object -First 1
$enc = Get-CimInstance Win32_SystemEnclosure -ErrorAction SilentlyContinue | Select-Object -First 1
$v = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction SilentlyContinue
$disks = @(Get-PhysicalDisk -ErrorAction SilentlyContinue | Sort-Object DeviceId | ForEach-Object { $t = if ("$($_.MediaType)" -eq 'SSD') { if ("$($_.BusType)" -eq 'NVMe') { 'SSD NVMe' } else { 'SSD' } } elseif ("$($_.MediaType)" -eq 'HDD') { 'HDD' } else { 'Disco' }; "$t $([math]::Round($_.Size / 1GB)) GB" }) -join ' + '
$lic = $null
try { $lic = Get-CimInstance SoftwareLicensingProduct -Filter "ApplicationID='55c92734-d682-4d71-983e-d6ec3f16059f' and PartialProductKey is not null" -ErrorAction Stop | Select-Object -First 1 } catch { }
$tpm = $null; try { $t = Get-CimInstance -Namespace 'root\cimv2\Security\MicrosoftTpm' -ClassName Win32_Tpm -ErrorAction Stop; $tpm = [bool]$t.IsEnabled_InitialValue } catch { }
$sb = $null; try { $sb = [bool](Confirm-SecureBootUEFI -ErrorAction Stop) } catch { }
$ipc = Get-NetIPConfiguration -ErrorAction SilentlyContinue | Where-Object { $_.IPv4DefaultGateway -and $_.NetAdapter.Status -eq 'Up' } | Select-Object -First 1
[pscustomobject]@{
  host = "$env:COMPUTERNAME"; user = "$($cs.UserName)"; domain = "$($cs.Domain)"; partOfDomain = [bool]$cs.PartOfDomain
  manufacturer = "$($cs.Manufacturer)".Trim(); model = "$($cs.Model)".Trim(); serial = "$($bios.SerialNumber)".Trim()
  chassis = "$(@($enc.ChassisTypes)[0])"; cpu = "$($cpu.Name)".Trim(); cores = [int]$cpu.NumberOfCores
  ramGb = [math]::Round($cs.TotalPhysicalMemory / 1GB, 1); disks = $disks
  gpu = (@(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Name)" }) -join ', ')
  os = "$($os.Caption)".Trim(); osVersion = "$($v.DisplayVersion) (compilación $($v.CurrentBuild).$($v.UBR))"
  installed = if ($os.InstallDate) { $os.InstallDate.ToString('yyyy-MM-dd') } else { '' }
  license = if (-not $lic) { 'Desconocido' } elseif ([int]$lic.LicenseStatus -eq 1) { 'Activado' } else { 'Sin activar' }
  bios = "$($bios.SMBIOSBIOSVersion) · $(if ($bios.ReleaseDate) { $bios.ReleaseDate.ToString('yyyy-MM-dd') })"
  tpm = $tpm; secureBoot = $sb
  ip = if ($ipc) { "$(@($ipc.IPv4Address)[0].IPAddress)" } else { '' }; mac = if ($ipc) { "$($ipc.NetAdapter.MacAddress)" } else { '' }
} | ConvertTo-Json -Compress
"#;

fn chassis(code: &str, model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("virtual") || m.contains("vmware") || m.contains("kvm") {
        return "Virtual".into();
    }
    match code.parse::<u32>().unwrap_or(0) {
        8..=10 | 14 | 30..=32 => "Portátil",
        3..=7 | 15 | 16 => "Sobremesa",
        13 => "Todo en uno",
        35 | 36 => "Mini PC",
        17 | 23 | 28 => "Servidor",
        _ => "Equipo",
    }
    .into()
}

/// Número de serie de relleno (placas base genéricas o montadas por piezas).
pub fn generic_serial(serial: &str) -> bool {
    let s: String = serial.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    s.len() < 4 || ["tobefilled", "defaultstring", "systemserialnumber", "0123456789", "none", "notapplicable", "chassisserial"].iter().any(|g| s.contains(g)) || s.chars().all(|c| c == '0')
}

/// Página de garantía del fabricante (con el número de serie cuando la web lo admite).
pub fn warranty_url(manufacturer: &str, serial: &str) -> Option<String> {
    let m = manufacturer.to_lowercase();
    let s: String = serial.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if generic_serial(serial) {
        return None;
    }
    Some(if m.contains("dell") {
        format!("https://www.dell.com/support/home/es-es/product-support/servicetag/{s}/overview")
    } else if m.contains("lenovo") {
        format!("https://pcsupport.lenovo.com/es/es/products/{s}/warranty")
    } else if m.contains("hp") || m.contains("hewlett") {
        "https://support.hp.com/es-es/check-warranty".into()
    } else if m.contains("asus") {
        "https://www.asus.com/es/support/warranty-status-inquiry/".into()
    } else if m.contains("acer") {
        format!("https://www.acer.com/es-es/support/product-support/{s}")
    } else if m.contains("microsoft") {
        "https://mybusinessservice.surface.com/".into()
    } else {
        return None;
    })
}

#[tauri::command(async)]
pub fn machine_sheet() -> Result<Sheet, String> {
    let out = crate::pspool::query(SCRIPT, Some(Duration::from_secs(60)), "Ficha del equipo")?;
    let mut s: Sheet = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    s.chassis = chassis(&s.chassis, &s.model);
    s.warranty_url = warranty_url(&s.manufacturer, &s.serial);
    if generic_serial(&s.serial) {
        s.serial = "No disponible (el fabricante no lo grabó)".into();
    }
    s.mac = s.mac.replace('-', ":");
    Ok(s)
}

/// Abre la web de garantía (solo las de fabricantes conocidos).
#[tauri::command]
pub fn open_warranty(url: String) -> Result<(), String> {
    const ALLOWED: &[&str] = &[
        "https://www.dell.com/",
        "https://pcsupport.lenovo.com/",
        "https://support.hp.com/",
        "https://www.asus.com/",
        "https://www.acer.com/",
        "https://mybusinessservice.surface.com/",
    ];
    if !ALLOWED.iter().any(|a| url.starts_with(a)) || url.contains(['"', ' ', '\n']) {
        return Err("Dirección no permitida.".into());
    }
    crate::shellopen::open(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }

    #[test]
    fn warranty_links() {
        assert!(warranty_url("Dell Inc.", "ABC1234").unwrap().contains("ABC1234"));
        assert!(warranty_url("LENOVO", "PF12AB34").unwrap().contains("PF12AB34"));
        assert!(warranty_url("Dell Inc.", "To be filled by O.E.M.").is_none());
        assert!(warranty_url("Genérico", "XYZ12345").is_none());
        assert!(generic_serial("Default string"));
        assert!(generic_serial("00000000"));
        assert!(!generic_serial("PF12AB34"));
        assert!(open_warranty("https://evil.com/x".into()).is_err());
    }

    #[test]
    fn chassis_names() {
        assert_eq!(chassis("10", "Latitude"), "Portátil");
        assert_eq!(chassis("3", "OptiPlex"), "Sobremesa");
        assert_eq!(chassis("1", "VMware Virtual Platform"), "Virtual");
    }

    /// Equipo real: `cargo test sheet_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sheet_real() {
        println!("{:#?}", machine_sheet().unwrap());
    }
}
