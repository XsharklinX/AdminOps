//! Atributos SMART de discos ATA/SATA vía WMI (requiere administrador).
//! Los NVMe no exponen la tabla SMART clásica: su salud viene de
//! Get-StorageReliabilityCounter (ver diagnóstico) y de los sensores.

use crate::ps;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SmartAttribute {
    pub id: u8,
    pub name: String,
    pub current: u8,
    pub worst: u8,
    pub raw: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SmartDisk {
    pub model: String,
    /// El propio disco anuncia que va a fallar.
    pub predict_failure: bool,
    pub reallocated: Option<u64>,
    pub pending: Option<u64>,
    pub uncorrectable: Option<u64>,
    pub crc_errors: Option<u64>,
    pub power_on_hours: Option<u64>,
    pub temperature: Option<u64>,
    pub attributes: Vec<SmartAttribute>,
}

fn attr_name(id: u8) -> &'static str {
    match id {
        1 => "Tasa de errores de lectura",
        3 => "Tiempo de arranque del motor",
        4 => "Arranques/paradas",
        5 => "Sectores reasignados",
        7 => "Errores de búsqueda",
        9 => "Horas encendido",
        10 => "Reintentos de giro",
        12 => "Ciclos de encendido",
        170 => "Bloques de reserva",
        171 => "Fallos de programación",
        172 => "Fallos de borrado",
        173 | 177 => "Desgaste (nivelación)",
        174 | 192 => "Apagados inesperados",
        184 => "Errores de extremo a extremo",
        187 => "Errores no corregibles",
        188 => "Tiempos de espera de comandos",
        190 | 194 => "Temperatura",
        195 => "ECC corregidos por hardware",
        196 => "Eventos de reasignación",
        197 => "Sectores pendientes",
        198 => "Sectores no corregibles",
        199 => "Errores CRC (cable/conector)",
        202 | 231 => "Vida restante",
        233 => "Desgaste del medio",
        241 => "Total escrito",
        242 => "Total leído",
        _ => "Atributo del fabricante",
    }
}

/// Tabla SMART en bruto: 2 bytes de versión y 30 entradas de 12 bytes
/// (id, flags×2, actual, peor, raw×6, reservado).
pub fn parse_vendor_specific(bytes: &[u8]) -> Vec<SmartAttribute> {
    bytes
        .get(2..)
        .unwrap_or_default()
        .as_chunks::<12>()
        .0
        .iter()
        .filter(|e| e[0] != 0)
        .map(|e| {
            let mut raw = [0u8; 8];
            raw[..6].copy_from_slice(&e[5..11]);
            SmartAttribute { id: e[0], name: attr_name(e[0]).into(), current: e[3], worst: e[4], raw: u64::from_le_bytes(raw) }
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDisk {
    model: String,
    predict_failure: bool,
    raw: Vec<u8>,
}

/// Windows dice así que la clase de SMART no existe para los discos del equipo.
fn not_supported(e: &str) -> bool {
    let l = e.to_lowercase();
    ["incompatible", "not supported", "no compatible", "no se admite", "no admitid", "0x8004100c"].iter().any(|k| l.contains(k))
}

pub fn read() -> Result<Vec<SmartDisk>, String> {
    let out = ps::powershell(
        r#"
$drives = @(Get-CimInstance Win32_DiskDrive)
$status = @(Get-CimInstance -Namespace root\wmi -ClassName MSStorageDriver_FailurePredictStatus -ErrorAction Stop)
$data = @(Get-CimInstance -Namespace root\wmi -ClassName MSStorageDriver_FailurePredictData -ErrorAction SilentlyContinue)
$r = @(foreach ($s in $status) {
  $pnp = $s.InstanceName -replace '_\d+$', ''
  $d = $drives | Where-Object { $_.PNPDeviceID -eq $pnp } | Select-Object -First 1
  $raw = ($data | Where-Object InstanceName -eq $s.InstanceName | Select-Object -First 1).VendorSpecific
  [pscustomobject]@{ model = if ($d) { "$($d.Model)".Trim() } else { $pnp }; predictFailure = [bool]$s.PredictFailure; raw = @($raw | ForEach-Object { [int]$_ }) }
})
ConvertTo-Json -InputObject $r -Depth 3 -Compress
"#,
    )
    .map_err(|e| {
        if e.to_lowercase().contains("denied") || e.to_lowercase().contains("denegado") {
            "Requiere ejecutar AdminOps como administrador.".into()
        } else {
            e
        }
    });
    let out = match out {
        Ok(o) => o,
        // «Incompatible» / «Not supported»: ningún disco de este equipo da SMART
        // por esta vía (NVMe, RAID, USB). No es un fallo: no hay nada que listar.
        Err(e) if not_supported(&e) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let raw: Vec<RawDisk> = if out.is_empty() { vec![] } else { serde_json::from_str(&out).map_err(|e| e.to_string())? };
    Ok(raw
        .into_iter()
        .map(|d| {
            let attributes = parse_vendor_specific(&d.raw);
            let get = |ids: &[u8]| attributes.iter().find(|a| ids.contains(&a.id)).map(|a| a.raw);
            SmartDisk {
                model: d.model,
                predict_failure: d.predict_failure,
                reallocated: get(&[5]),
                pending: get(&[197]),
                uncorrectable: get(&[198]),
                crc_errors: get(&[199]),
                power_on_hours: get(&[9]).map(|h| h & 0xFFFF_FFFF),
                temperature: get(&[194, 190]).map(|t| t & 0xFF),
                attributes,
            }
        })
        .collect())
}

#[tauri::command(async)]
pub fn smart_status() -> Result<Vec<SmartDisk>, String> {
    read()
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_smart_table() {
        let mut t = vec![0u8; 2 + 30 * 12];
        // Sectores reasignados: actual 100, peor 100, raw 8
        t[2..14].copy_from_slice(&[5, 0x33, 0, 100, 100, 8, 0, 0, 0, 0, 0, 0]);
        // Temperatura 41 °C (raw con mín/máx en los bytes altos)
        t[14..26].copy_from_slice(&[194, 0x22, 0, 41, 60, 41, 0, 20, 0, 55, 0, 0]);
        let a = super::parse_vendor_specific(&t);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].raw, 8);
        assert_eq!(a[0].name, "Sectores reasignados");
        assert_eq!(a[1].raw & 0xFF, 41);
    }
}
