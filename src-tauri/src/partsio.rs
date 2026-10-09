//! Repuestos compatibles: leer del equipo lo que hace falta y devolver el consejo. La lógica
//! está en `partsadvice.rs`.

use crate::partsadvice::{advise, Advice, Input};
use std::time::Duration;

const SCRIPT: &str = r#"
$mem = @(Get-CimInstance Win32_PhysicalMemory -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ capacity = [uint64]$_.Capacity; speed = [uint32]$_.Speed; configured = [uint32]$_.ConfiguredClockSpeed; kind = [uint32]$_.SMBIOSMemoryType; form = [uint32]$_.FormFactor; slot = "$($_.DeviceLocator)"; part = "$($_.PartNumber)".Trim() } })
$arr = Get-CimInstance Win32_PhysicalMemoryArray -ErrorAction SilentlyContinue | Select-Object -First 1
$cs = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue
$bb = Get-CimInstance Win32_BaseBoard -ErrorAction SilentlyContinue
$enc = @((Get-CimInstance Win32_SystemEnclosure -ErrorAction SilentlyContinue).ChassisTypes)
$laptop = [bool](($enc | Where-Object { $_ -in 8, 9, 10, 11, 12, 14, 18, 21, 30, 31, 32 }) -or ($cs -and $cs.PCSystemType -eq 2))
$disks = @(Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ name = "$($_.FriendlyName)".Trim(); bus = "$($_.BusType)"; media = "$($_.MediaType)"; size = [uint64]$_.Size } })
$maxKb = if ($arr) { [uint64]$arr.MaxCapacity } else { 0 }
$maxEx = if ($arr -and $arr.MaxCapacityEx) { [uint64]$arr.MaxCapacityEx } else { 0 }
$o = [pscustomobject]@{
  maker = "$($cs.Manufacturer)".Trim(); model = "$($cs.Model)".Trim(); board = "$($bb.Product)".Trim(); laptop = $laptop
  modules = $mem; slots = if ($arr) { [uint32]$arr.MemoryDevices } else { 0 }
  maxMemory = if ($maxEx -gt 0) { $maxEx * 1024 } else { $maxKb * 1024 }; disks = $disks }
ConvertTo-Json -InputObject $o -Depth 4 -Compress
"#;

/// Qué se puede comprar para ampliar este equipo y qué no encaja.
#[tauri::command(async)]
pub fn parts_advice() -> Result<Advice, String> {
    let out = crate::pspool::query(SCRIPT, Some(Duration::from_secs(30)), "Repuestos compatibles")?;
    let input: Input = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada de Windows: {e}"))?;
    Ok(advise(&input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
