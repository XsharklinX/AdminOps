//! Recolectores de diagnóstico. Cada uno es un script de PowerShell que
//! devuelve JSON con claves camelCase; si uno falla, los demás siguen.

use crate::ps;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

fn run<T: DeserializeOwned>(script: &str) -> Result<T, String> {
    let out = ps::powershell(script)?;
    serde_json::from_str(if out.is_empty() { "null" } else { &out })
        .map_err(|e| format!("Respuesta inesperada: {e}"))
}

// ---------- Discos físicos ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalDisk {
    pub name: String,
    pub media_type: String,
    pub bus_type: String,
    pub health: String,
    pub operational: String,
    pub size: u64,
    /// Grados C. Muchos discos no lo reportan (null o 0).
    pub temperature: Option<u32>,
    /// % de vida consumida (solo SSD/NVMe que lo reportan).
    pub wear: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub read_errors: Option<u64>,
    pub write_errors: Option<u64>,
}

pub fn disks() -> Result<Vec<PhysicalDisk>, String> {
    run(r#"
$out = foreach ($d in @(Get-PhysicalDisk | Sort-Object DeviceId)) {
  $r = $null
  try { $r = $d | Get-StorageReliabilityCounter -ErrorAction Stop } catch {}
  [pscustomobject]@{
    name = "$($d.FriendlyName)".Trim(); mediaType = "$($d.MediaType)"; busType = "$($d.BusType)"
    health = "$($d.HealthStatus)"; operational = ($d.OperationalStatus -join ', '); size = [uint64]$d.Size
    temperature = if ($r.Temperature) { [uint32]$r.Temperature } else { $null }
    wear = if ($null -ne $r.Wear) { [uint32]$r.Wear } else { $null }
    powerOnHours = $r.PowerOnHours; readErrors = $r.ReadErrorsTotal; writeErrors = $r.WriteErrorsTotal
  }
}
ConvertTo-Json -InputObject @($out) -Compress
"#)
}

// ---------- Estabilidad ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BugCheck {
    pub time: String,
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppCrash {
    pub app: String,
    pub count: u32,
    pub last: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dump {
    pub name: String,
    pub time: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootTime {
    pub time: String,
    pub ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stability {
    pub days: u32,
    pub bugchecks: Vec<BugCheck>,
    pub unexpected_shutdowns: Vec<String>,
    pub crashes: Vec<AppCrash>,
    /// `None` si no se pudo leer la carpeta (requiere administrador).
    pub minidumps: Option<Vec<Dump>>,
    /// `None` si no se pudo leer el registro (requiere administrador).
    pub boot_times: Option<Vec<BootTime>>,
}

pub const STABILITY_DAYS: u32 = 30;

pub fn stability() -> Result<Stability, String> {
    let mut s: Stability = run(&format!(
        r#"
$since = (Get-Date).AddDays(-{STABILITY_DAYS})
$sys = @(Get-WinEvent -FilterHashtable @{{ LogName = 'System'; Id = 41, 1001; StartTime = $since }} -ErrorAction SilentlyContinue)
$bug = @($sys | Where-Object {{ $_.Id -eq 1001 -and ($_.ProviderName -like '*WER-SystemErrorReporting*' -or $_.ProviderName -eq 'BugCheck') }} | ForEach-Object {{
  [pscustomobject]@{{ time = $_.TimeCreated.ToString('o'); code = [regex]::Match("$($_.Properties[0].Value) $($_.Message)", '0x[0-9a-fA-F]{{1,8}}').Value }}
}})
$kp = @($sys | Where-Object {{ $_.Id -eq 41 -and $_.ProviderName -eq 'Microsoft-Windows-Kernel-Power' }} | ForEach-Object {{ $_.TimeCreated.ToString('o') }})
$app = @(Get-WinEvent -FilterHashtable @{{ LogName = 'Application'; ProviderName = 'Application Error', 'Application Hang'; StartTime = $since }} -ErrorAction SilentlyContinue)
$crashes = @($app | Where-Object {{ $_.Id -in 1000, 1002 }} | Group-Object {{ "$($_.Properties[0].Value)" }} | Sort-Object Count -Descending | Select-Object -First 10 | ForEach-Object {{
  [pscustomobject]@{{ app = $_.Name; count = $_.Count; last = ($_.Group | Sort-Object TimeCreated -Descending)[0].TimeCreated.ToString('o') }}
}})
$dumps = $null
try {{
  $dumps = @(Get-ChildItem (Join-Path $env:SystemRoot 'Minidump') -Filter *.dmp -ErrorAction Stop | Sort-Object LastWriteTime -Descending | ForEach-Object {{
    [pscustomobject]@{{ name = $_.Name; time = $_.LastWriteTime.ToString('o'); size = $_.Length }}
  }})
}} catch [System.Management.Automation.ItemNotFoundException] {{ $dumps = @() }} catch {{}}
$boots = $null
try {{
  $boots = @(Get-WinEvent -FilterHashtable @{{ LogName = 'Microsoft-Windows-Diagnostics-Performance/Operational'; Id = 100 }} -MaxEvents 5 -ErrorAction Stop | ForEach-Object {{
    $ms = ([xml]$_.ToXml()).Event.EventData.Data | Where-Object Name -eq 'BootTime' | ForEach-Object '#text'
    [pscustomobject]@{{ time = $_.TimeCreated.ToString('o'); ms = [uint64]$ms }}
  }})
}} catch {{ if ($_.FullyQualifiedErrorId -match 'NoMatchingEventsFound') {{ $boots = @() }} }}
ConvertTo-Json -Depth 4 -Compress -InputObject ([pscustomobject]@{{
  days = {STABILITY_DAYS}; bugchecks = $bug; unexpectedShutdowns = $kp; crashes = $crashes; minidumps = $dumps; bootTimes = $boots
}})
"#
    ))?;
    for b in &mut s.bugchecks {
        let (name, hint) = bugcheck_info(&b.code);
        b.name = name.map(str::to_string);
        b.hint = hint.map(str::to_string);
    }
    Ok(s)
}

/// Nombre y pista de diagnóstico de los códigos de pantallazo azul más comunes.
fn bugcheck_info(code: &str) -> (Option<&'static str>, Option<&'static str>) {
    let n = u32::from_str_radix(code.trim_start_matches("0x").trim_start_matches("0X"), 16).unwrap_or(0);
    let (name, hint) = match n {
        0x0A => ("IRQL_NOT_LESS_OR_EQUAL", "Driver defectuoso o RAM inestable."),
        0x19 => ("BAD_POOL_HEADER", "Corrupción de memoria por un driver; probar la RAM."),
        0x1A => ("MEMORY_MANAGEMENT", "Muy ligado a RAM defectuosa: ejecutar diagnóstico de memoria."),
        0x1E => ("KMODE_EXCEPTION_NOT_HANDLED", "Driver con fallo; revisar drivers recientes."),
        0x3B => ("SYSTEM_SERVICE_EXCEPTION", "Driver (a menudo gráfico o antivirus)."),
        0x50 => ("PAGE_FAULT_IN_NONPAGED_AREA", "RAM, disco o driver; ejecutar SFC y probar RAM."),
        0x7E => ("SYSTEM_THREAD_EXCEPTION_NOT_HANDLED", "Driver incompatible, a menudo tras una actualización."),
        0x7F => ("UNEXPECTED_KERNEL_MODE_TRAP", "Hardware: RAM, CPU u overclock."),
        0x9F => ("DRIVER_POWER_STATE_FAILURE", "Driver que falla al suspender/reanudar; actualizar drivers de chipset y red."),
        0xC2 => ("BAD_POOL_CALLER", "Driver defectuoso."),
        0xD1 => ("DRIVER_IRQL_NOT_LESS_OR_EQUAL", "Driver defectuoso, muy a menudo el de red o Wi-Fi."),
        0xEF => ("CRITICAL_PROCESS_DIED", "Archivos del sistema dañados o disco con fallos: SFC, DISM y revisar disco."),
        0xF4 => ("CRITICAL_OBJECT_TERMINATION", "Disco o controlador de almacenamiento con fallos."),
        0x101 => ("CLOCK_WATCHDOG_TIMEOUT", "CPU: temperatura, overclock o BIOS desactualizada."),
        0x116 => ("VIDEO_TDR_FAILURE", "Driver o GPU: reinstalar driver gráfico (DDU) y revisar temperatura."),
        0x117 => ("VIDEO_TDR_TIMEOUT_DETECTED", "Driver gráfico que deja de responder."),
        0x124 => ("WHEA_UNCORRECTABLE_ERROR", "Error de hardware: temperatura, fuente, overclock o CPU."),
        0x12B => ("FAULTY_HARDWARE_CORRUPTED_PAGE", "RAM defectuosa."),
        0x133 => ("DPC_WATCHDOG_VIOLATION", "Driver o firmware de almacenamiento (SSD); actualizar firmware y drivers."),
        0x139 => ("KERNEL_SECURITY_CHECK_FAILURE", "Driver incompatible o memoria dañada."),
        0x154 => ("UNEXPECTED_STORE_EXCEPTION", "Disco (SSD) o su driver; revisar salud del disco."),
        _ => return (None, None),
    };
    (Some(name), Some(hint))
}

// ---------- Drivers ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProblem {
    pub name: String,
    pub class: Option<String>,
    pub code: u32,
    pub device_id: String,
    #[serde(default)]
    pub problem: String,
}

pub fn drivers() -> Result<Vec<DeviceProblem>, String> {
    // 45 = dispositivo desconectado (fantasma): normal y muy frecuente, no es un problema.
    let mut v: Vec<DeviceProblem> = run(r#"
$d = @(Get-CimInstance Win32_PnPEntity -Filter 'ConfigManagerErrorCode <> 0 AND ConfigManagerErrorCode <> 45' | ForEach-Object {
  [pscustomobject]@{ name = if ($_.Name) { $_.Name } else { $_.DeviceID }; class = $_.PNPClass; code = [uint32]$_.ConfigManagerErrorCode; deviceId = $_.DeviceID }
})
ConvertTo-Json -InputObject $d -Compress
"#)?;
    for d in &mut v {
        d.problem = device_problem(d.code).to_string();
    }
    Ok(v)
}

pub fn device_problem(code: u32) -> &'static str {
    match code {
        1 => "No está configurado correctamente",
        3 => "Driver dañado o sin memoria suficiente",
        10 => "El dispositivo no puede iniciarse",
        12 => "No hay recursos libres suficientes",
        14 => "Requiere reiniciar el equipo",
        18 => "Hay que reinstalar los drivers",
        19 => "Configuración del registro dañada",
        21 => "Windows está quitando el dispositivo",
        22 => "Deshabilitado por el usuario",
        24 => "No presente o faltan drivers",
        28 => "Drivers no instalados",
        29 => "Deshabilitado por el firmware (BIOS/UEFI)",
        31 => "No funciona correctamente",
        32 => "El servicio del driver está deshabilitado",
        37 => "El driver devolvió un error al iniciar",
        39 => "Driver dañado o ausente",
        43 => "El dispositivo informó de un problema (código 43)",
        48 => "Driver bloqueado por incompatibilidad",
        52 => "No se pudo verificar la firma digital del driver",
        _ => "Error del dispositivo",
    }
}

// ---------- Batería ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub name: String,
    pub manufacturer: String,
    pub chemistry: String,
    /// mWh
    pub design: u64,
    pub full: u64,
    pub cycles: Option<u32>,
}

impl Battery {
    /// Capacidad actual respecto a la de fábrica, en %.
    pub fn health(&self) -> f64 {
        if self.design == 0 { 0.0 } else { self.full as f64 * 100.0 / self.design as f64 }
    }
}

pub fn battery() -> Result<Option<Battery>, String> {
    run(r#"
if (-not (Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue)) { 'null'; return }
$f = Join-Path $env:TEMP ("adminops-battery-{0}.xml" -f $PID)
powercfg.exe /batteryreport /xml /output $f | Out-Null
if (-not (Test-Path $f)) { 'null'; return }
[xml]$x = Get-Content -LiteralPath $f -Raw
Remove-Item -LiteralPath $f -ErrorAction SilentlyContinue
$b = @($x.BatteryReport.Batteries.Battery)[0]
if (-not $b) { 'null'; return }
ConvertTo-Json -Compress -InputObject ([pscustomobject]@{
  name = "$($b.Id)".Trim(); manufacturer = "$($b.Manufacturer)".Trim(); chemistry = "$($b.Chemistry)"
  design = [uint64]$b.DesignCapacity; full = [uint64]$b.FullChargeCapacity
  cycles = if ("$($b.CycleCount)" -match '^\d+$') { [uint32]$b.CycleCount } else { $null }
})
"#)
}

// ---------- Sistema y seguridad ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub last_boot: String,
    pub install_date: String,
    pub pending_reboot: bool,
    pub last_update: Option<String>,
    pub last_update_id: Option<String>,
    pub defender_realtime: Option<bool>,
    pub signature_age_days: Option<u32>,
    pub antivirus: Vec<String>,
    pub activated: Option<bool>,
    pub secure_boot: Option<bool>,
    pub tpm_ready: Option<bool>,
    #[serde(default)]
    pub quick_scan_age_days: Option<u32>,
}

pub fn system() -> Result<SystemHealth, String> {
    run(r#"
$os = Get-CimInstance Win32_OperatingSystem
$cbs = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending'
$wu = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired'
$pfr = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager' -Name PendingFileRenameOperations -ErrorAction SilentlyContinue).PendingFileRenameOperations
$hf = Get-HotFix -ErrorAction SilentlyContinue | Where-Object InstalledOn | Sort-Object InstalledOn -Descending | Select-Object -First 1
$mp = try { Get-MpComputerStatus -ErrorAction Stop } catch { $null }
$av = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntivirusProduct -ErrorAction SilentlyContinue | ForEach-Object displayName | Select-Object -Unique)
$lic = Get-CimInstance SoftwareLicensingProduct -Filter "PartialProductKey IS NOT NULL AND ApplicationId = '55c92734-d682-4d71-983e-d6ec3f16059f'" -ErrorAction SilentlyContinue | Select-Object -First 1
# Ambos requieren administrador: sin él Get-Tpm informa "no listo" aunque esté bien.
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole('Administrators')
$sb = if ($admin) { try { [bool](Confirm-SecureBootUEFI -ErrorAction Stop) } catch { $null } } else { $null }
$tpm = if ($admin) { try { [bool](Get-Tpm -ErrorAction Stop).TpmReady } catch { $null } } else { $null }
ConvertTo-Json -Compress -InputObject ([pscustomobject]@{
  lastBoot = $os.LastBootUpTime.ToString('o'); installDate = $os.InstallDate.ToString('o')
  pendingReboot = [bool]((Test-Path $cbs) -or (Test-Path $wu) -or $pfr)
  lastUpdate = if ($hf) { $hf.InstalledOn.ToString('o') } else { $null }; lastUpdateId = $hf.HotFixID
  defenderRealtime = if ($mp) { [bool]$mp.RealTimeProtectionEnabled } else { $null }
  signatureAgeDays = if ($mp -and $mp.AntivirusSignatureAge -lt 10000) { [uint32]$mp.AntivirusSignatureAge } else { $null }
  antivirus = $av
  activated = if ($lic) { $lic.LicenseStatus -eq 1 } else { $null }
  secureBoot = $sb; tpmReady = $tpm
  quickScanAgeDays = if ($mp -and $mp.QuickScanAge -lt 10000) { [uint32]$mp.QuickScanAge } else { $null }
})
"#)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bugcheck_codes_parse_with_and_without_padding() {
        assert_eq!(super::bugcheck_info("0x00000124").0, Some("WHEA_UNCORRECTABLE_ERROR"));
        assert_eq!(super::bugcheck_info("0x9f").0, Some("DRIVER_POWER_STATE_FAILURE"));
        assert_eq!(super::bugcheck_info("").0, None);
    }

    /// Ejecuta todos los recolectores en este equipo (solo lectura).
    #[test]
    #[ignore]
    fn collect_real_system() {
        println!("DISKS {:#?}", super::disks());
        println!("STABILITY {:#?}", super::stability());
        println!("DRIVERS {:#?}", super::drivers());
        println!("BATTERY {:#?}", super::battery());
        println!("SYSTEM {:#?}", super::system());
    }
}
