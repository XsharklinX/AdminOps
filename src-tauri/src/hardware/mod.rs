//! Hardware: inventario completo, sensores (LibreHardwareMonitor), SMART y
//! resultado de la prueba de memoria.

pub mod sensors;
pub mod smart;

use crate::ps;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoryModule {
    pub slot: String,
    pub bank: String,
    pub capacity: u64,
    pub speed: Option<u32>,
    pub configured_speed: Option<u32>,
    pub manufacturer: String,
    pub part_number: String,
    pub kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Gpu {
    pub name: String,
    pub driver_version: String,
    /// ISO 8601
    pub driver_date: Option<String>,
    pub vram: Option<u64>,
    pub resolution: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Monitor {
    pub name: String,
    pub manufacturer: String,
    pub year: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Inventory {
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub board_manufacturer: String,
    pub board_product: String,
    pub bios_vendor: String,
    pub bios_version: String,
    /// ISO 8601
    pub bios_date: Option<String>,
    /// UEFI o Legacy
    pub firmware: String,
    pub cpu: String,
    pub cores: u32,
    pub threads: u32,
    pub max_mhz: u32,
    pub socket: String,
    pub virtualization: Option<bool>,
    pub ram_total: u64,
    pub ram_slots: u32,
    pub ram_max: Option<u64>,
    pub modules: Vec<MemoryModule>,
    pub gpus: Vec<Gpu>,
    pub monitors: Vec<Monitor>,
    pub os: String,
    pub os_version: String,
    pub os_build: String,
    pub architecture: String,
    /// Clave de Windows grabada en el firmware (OEM). No va en el informe.
    pub product_key: Option<String>,
}

impl Inventory {
    /// Resumen en una línea para la ficha del cliente y detectar cambios entre visitas.
    pub fn summary(&self) -> String {
        let gb = self.ram_total as f64 / 1024f64.powi(3);
        let gpu = self.gpus.iter().map(|g| g.name.as_str()).collect::<Vec<_>>().join(" + ");
        format!("{} · {:.0} GB RAM ({} módulos) · {} · {} {}", self.cpu, gb, self.modules.len(), gpu, self.board_manufacturer, self.board_product)
    }
}

const INVENTORY_SCRIPT: &str = r#"
$cs = Get-CimInstance Win32_ComputerSystem
$bios = Get-CimInstance Win32_BIOS
$bb = Get-CimInstance Win32_BaseBoard
$cpu = @(Get-CimInstance Win32_Processor)[0]
$os = Get-CimInstance Win32_OperatingSystem
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$types = @{ 20 = 'DDR'; 21 = 'DDR2'; 24 = 'DDR3'; 26 = 'DDR4'; 30 = 'LPDDR4'; 34 = 'DDR5'; 35 = 'LPDDR5' }
$modules = @(Get-CimInstance Win32_PhysicalMemory | ForEach-Object {
  [pscustomobject]@{
    slot = "$($_.DeviceLocator)".Trim(); bank = "$($_.BankLabel)".Trim(); capacity = [uint64]$_.Capacity
    speed = if ($_.Speed) { [uint32]$_.Speed } else { $null }
    configuredSpeed = if ($_.ConfiguredClockSpeed) { [uint32]$_.ConfiguredClockSpeed } else { $null }
    manufacturer = "$($_.Manufacturer)".Trim(); partNumber = "$($_.PartNumber)".Trim()
    kind = if ($types[[int]$_.SMBIOSMemoryType]) { $types[[int]$_.SMBIOSMemoryType] } else { '' }
  }
})
$arrays = @(Get-CimInstance Win32_PhysicalMemoryArray | Where-Object Use -eq 3)
$slots = ($arrays | Measure-Object MemoryDevices -Sum).Sum
$maxKb = ($arrays | Measure-Object MaxCapacityEx -Sum).Sum
# VRAM real (Win32_VideoController.AdapterRAM se desborda por encima de 4 GB).
$vram = @{}
Get-ChildItem 'HKLM:\SYSTEM\ControlSet001\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}' -ErrorAction SilentlyContinue | ForEach-Object {
  $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
  if ($p.DriverDesc -and $p.'HardwareInformation.qwMemorySize') { $vram[$p.DriverDesc] = [uint64]$p.'HardwareInformation.qwMemorySize' }
}
$gpus = @(Get-CimInstance Win32_VideoController | Where-Object { $_.Name -notmatch 'Basic Display|Remote|Virtual|Parsec|Meta' } | ForEach-Object {
  [pscustomobject]@{
    name = "$($_.Name)"; driverVersion = "$($_.DriverVersion)"
    driverDate = if ($_.DriverDate) { $_.DriverDate.ToString('o') } else { $null }
    vram = if ($vram[$_.Name]) { $vram[$_.Name] } elseif ($_.AdapterRAM) { [uint64][uint32]$_.AdapterRAM } else { $null }
    resolution = if ($_.CurrentHorizontalResolution) { "$($_.CurrentHorizontalResolution)×$($_.CurrentVerticalResolution) @ $($_.CurrentRefreshRate) Hz" } else { $null }
  }
})
$dec = { param($a) (($a | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join '').Trim() }
$monitors = @()
try {
  $monitors = @(Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction Stop | ForEach-Object {
    [pscustomobject]@{ name = & $dec $_.UserFriendlyName; manufacturer = & $dec $_.ManufacturerName; year = if ($_.YearOfManufacture) { [uint32]$_.YearOfManufacture } else { $null } }
  })
} catch {}
$key = $null
try { $key = (Get-CimInstance SoftwareLicensingService -ErrorAction Stop).OA3xOriginalProductKey } catch {}
$fw = if ($env:firmware_type) { $env:firmware_type } elseif (Test-Path 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State') { 'UEFI' } else { 'Legacy' }
ConvertTo-Json -Depth 4 -Compress -InputObject ([pscustomobject]@{
  manufacturer = "$($cs.Manufacturer)".Trim(); model = "$($cs.Model)".Trim(); serial = "$($bios.SerialNumber)".Trim()
  boardManufacturer = "$($bb.Manufacturer)".Trim(); boardProduct = "$($bb.Product)".Trim()
  biosVendor = "$($bios.Manufacturer)".Trim(); biosVersion = "$($bios.SMBIOSBIOSVersion)".Trim()
  biosDate = if ($bios.ReleaseDate) { $bios.ReleaseDate.ToString('o') } else { $null }
  firmware = "$fw"
  cpu = "$($cpu.Name)".Trim(); cores = [uint32]$cpu.NumberOfCores; threads = [uint32]$cpu.NumberOfLogicalProcessors
  maxMhz = [uint32]$cpu.MaxClockSpeed; socket = "$($cpu.SocketDesignation)"
  virtualization = if ($null -ne $cpu.VirtualizationFirmwareEnabled) { [bool]$cpu.VirtualizationFirmwareEnabled } else { $null }
  ramTotal = [uint64]$cs.TotalPhysicalMemory; ramSlots = [uint32]$slots
  ramMax = if ($maxKb) { [uint64]$maxKb * 1024 } else { $null }
  modules = $modules; gpus = $gpus; monitors = $monitors
  os = "$($os.Caption)".Trim(); osVersion = "$($cv.DisplayVersion)"; osBuild = "$($cv.CurrentBuild).$($cv.UBR)"
  architecture = "$($os.OSArchitecture)"; productKey = if ($key) { "$key" } else { $null }
})
"#;

pub fn inventory() -> Result<Inventory, String> {
    let out = ps::powershell_opts(INVENTORY_SCRIPT, ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada del inventario: {e}"))
}

#[tauri::command(async)]
pub fn hardware_inventory() -> Result<Inventory, String> {
    inventory()
}

// ---------- Prueba de memoria ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTest {
    /// ISO 8601
    pub time: String,
    /// `true` si no encontró errores.
    pub passed: bool,
    pub message: String,
}

/// Último veredicto del Diagnóstico de memoria de Windows (mdsched).
pub fn memory_test() -> Result<Option<MemoryTest>, String> {
    let out = ps::powershell(
        r#"
try {
  $e = Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-MemoryDiagnostics-Results' } -MaxEvents 1 -ErrorAction Stop
  # 1101: sin errores · 1102: errores de hardware detectados
  ConvertTo-Json -Compress -InputObject ([pscustomobject]@{ time = $e.TimeCreated.ToString('o'); passed = ($e.Id -ne 1102); message = "$($e.Message)".Trim() })
} catch { if ($_.FullyQualifiedErrorId -match 'NoMatchingEventsFound') { 'null' } else { throw } }
"#,
    )?;
    serde_json::from_str(if out.is_empty() { "null" } else { &out }).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn memory_test_result() -> Result<Option<MemoryTest>, String> {
    memory_test()
}

#[cfg(test)]
mod tests {
    /// Inventario real (solo lectura): `cargo test inventory_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn inventory_real() {
        let i = super::inventory().unwrap();
        println!("{i:#?}\n{}", i.summary());
        println!("{:?}", super::memory_test());
    }
}
