//! Lectura directa de SMART: la tabla con sus umbrales (SATA), el registro de
//! salud de los NVMe y las autopruebas del propio disco. La interpretación está
//! en `smartx.rs`; aquí solo se habla con el disco.
//!
//! - SATA por `SMART_RCV_DRIVE_DATA` / `SMART_SEND_DRIVE_COMMAND`, que sirven
//!   con controladoras AHCI. Si el disco está tras una caja USB o una RAID
//!   y no contesta, se cae a la lectura de Windows (WMI), sin umbrales.
//! - NVMe por `IOCTL_STORAGE_QUERY_PROPERTY` con la página 2 del registro.
//!
//! Todo es de solo lectura salvo iniciar o cancelar una autoprueba, que el disco
//! hace por su cuenta sin tocar los datos.

use crate::rawdisk::{Mode, RawDisk};
use crate::smartx::{self, NvmeHealth, SelfTest, SmartRow};
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use tauri::State;

const SMART_RCV_DRIVE_DATA: u32 = 0x0007_C088;
const SMART_SEND_DRIVE_COMMAND: u32 = 0x0007_C084;
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;

const SMART_READ_DATA: u8 = 0xD0;
const SMART_READ_THRESHOLDS: u8 = 0xD1;
const SMART_EXECUTE_SELFTEST: u8 = 0xD4;
const SMART_ENABLE: u8 = 0xD8;

/// Una orden SMART de ATA. Con `read`, devuelve los 512 bytes de datos.
fn ata_smart(d: &RawDisk, feature: u8, lba_low: u8, read: bool) -> Result<Vec<u8>, u32> {
    // SENDCMDINPARAMS: tamaño del búfer, 8 registros ATA, número de unidad y reservados.
    let mut input = [0u8; 36];
    input[0..4].copy_from_slice(&(if read { 512u32 } else { 0 }).to_le_bytes());
    input[4] = feature; // bFeaturesReg
    input[5] = 1; // bSectorCountReg
    // Como en el ejemplo de Microsoft (DFP): para leer y para encender SMART el registro vale 1; para las autopruebas, el subcomando.
    input[6] = if lba_low == 0 { 1 } else { lba_low }; // bSectorNumberReg (LBA bajo)
    input[7] = 0x4F; // bCylLowReg (firma de SMART)
    input[8] = 0xC2; // bCylHighReg (firma de SMART)
    input[9] = 0xA0; // bDriveHeadReg
    input[10] = 0xB0; // bCommandReg: SMART
    input[12] = d.number() as u8;
    // SENDCMDOUTPARAMS: 4 de tamaño + 12 de estado del controlador + datos.
    let mut out = vec![0u8; 16 + 512 + 4];
    // La cabecera de la respuesta son 16 bytes; el controlador pide además unos pocos de margen.
    let (code, out_len) = if read { (SMART_RCV_DRIVE_DATA, 16 + 512 + 4) } else { (SMART_SEND_DRIVE_COMMAND, 20) };
    d.ioctl(code, &input, &mut out[..out_len])?;
    if read {
        Ok(out[16..16 + 512].to_vec())
    } else {
        Ok(Vec::new())
    }
}

/// Registro de salud de un NVMe (512 bytes), o None si el disco no lo da.
fn nvme_log(d: &RawDisk) -> Option<Vec<u8>> {
    // Dos formas de pedir «todos los espacios de nombres»; según el controlador vale una u otra.
    for nsid in [0xFFFF_FFFFu32, 0] {
        let mut buf = vec![0u8; 8 + 40 + 512];
        buf[0..4].copy_from_slice(&50u32.to_le_bytes()); // StorageDeviceProtocolSpecificProperty
        buf[4..8].copy_from_slice(&0u32.to_le_bytes()); // PropertyStandardQuery
        let p = 8;
        buf[p..p + 4].copy_from_slice(&3u32.to_le_bytes()); // ProtocolTypeNvme
        buf[p + 4..p + 8].copy_from_slice(&2u32.to_le_bytes()); // NVMeDataTypeLogPage
        buf[p + 8..p + 12].copy_from_slice(&2u32.to_le_bytes()); // página 2: salud
        buf[p + 12..p + 16].copy_from_slice(&nsid.to_le_bytes());
        buf[p + 16..p + 20].copy_from_slice(&40u32.to_le_bytes()); // dónde empiezan los datos
        buf[p + 20..p + 24].copy_from_slice(&512u32.to_le_bytes()); // cuántos
        let mut out = buf.clone();
        if d.ioctl(IOCTL_STORAGE_QUERY_PROPERTY, &buf, &mut out).is_ok() && out.len() >= 8 + 40 + 512 {
            let data = out[48..48 + 512].to_vec();
            if data.iter().any(|b| *b != 0) {
                return Some(data);
            }
        }
    }
    None
}

/// Salud de un NVMe, para la nota y el veredicto de la tarjeta de cada disco.
pub fn nvme_health(number: u32) -> Option<NvmeHealth> {
    let d = RawDisk::open(number, Mode::Query).ok()?;
    smartx::parse_nvme_log(&nvme_log(&d)?)
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SmartFull {
    pub number: u32,
    pub model: String,
    /// ata · nvme · wmi (sin umbrales) · none
    pub kind: String,
    pub rows: Vec<SmartRow>,
    pub nvme: Option<NvmeHealth>,
    pub self_test: Option<SelfTest>,
    /// Qué decirle al técnico (por qué no hay datos, de dónde salen…).
    pub note: String,
    /// El disco deja lanzar autopruebas.
    pub can_self_test: bool,
}

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

/// La tabla SMART completa de un disco.
#[tauri::command(async)]
pub fn smart_full(number: u32, bus: String, model: String) -> Result<SmartFull, String> {
    need_admin()?;
    let mut out = SmartFull { number, model: model.clone(), kind: "none".into(), ..Default::default() };
    let d = RawDisk::open(number, Mode::Query)?;

    if bus.eq_ignore_ascii_case("NVMe") {
        if let Some(h) = nvme_log(&d).as_deref().and_then(smartx::parse_nvme_log) {
            out.kind = "nvme".into();
            out.rows = smartx::nvme_rows(&h);
            out.nvme = Some(h);
            out.note = "Registro de salud del propio NVMe. Los NVMe no tienen la tabla de atributos de los discos SATA.".into();
            return Ok(out);
        }
        out.note = "Este NVMe no deja leer su registro de salud con el controlador actual. Prueba con el controlador de NVMe de Windows o el del fabricante.".into();
        return Ok(out);
    }

    // SATA: primero se enciende SMART (por si está apagado) y se lee la tabla y los umbrales.
    let _ = ata_smart(&d, SMART_ENABLE, 0, false);
    if let Ok(table) = ata_smart(&d, SMART_READ_DATA, 0, true) {
        let thresholds = ata_smart(&d, SMART_READ_THRESHOLDS, 0, true).map(|t| smartx::parse_thresholds(&t)).unwrap_or_default();
        let entries = smartx::parse_ata_table(&table);
        if !entries.is_empty() {
            out.kind = "ata".into();
            out.rows = smartx::ata_rows(&entries, &thresholds);
            out.self_test = Some(smartx::parse_self_test(&table));
            out.can_self_test = true;
            out.note = if thresholds.is_empty() { "No se pudieron leer los umbrales de este disco.".into() } else { String::new() };
            return Ok(out);
        }
    }

    // Caja USB, RAID o controlador que no deja pasar las órdenes: lo que da Windows.
    if let Some(s) = crate::hardware::smart::read().unwrap_or_default().into_iter().find(|s| crate::disks::same_model(&s.model, &model)) {
        let entries: Vec<smartx::AtaEntry> = s.attributes.iter().map(|a| smartx::AtaEntry { id: a.id, current: a.current, worst: a.worst, raw: a.raw }).collect();
        out.kind = "wmi".into();
        out.rows = smartx::ata_rows(&entries, &[]);
        out.note = "Datos leídos a través de Windows: este disco no deja leer los umbrales ni lanzar autopruebas por este camino.".into();
        return Ok(out);
    }
    out.note = if bus.eq_ignore_ascii_case("USB") {
        "Las cajas USB suelen esconder el SMART del disco. Si abres la caja y lo conectas por SATA directo, sí se ve.".into()
    } else {
        "Este disco no ofrece datos SMART por ningún camino (puede estar tras una controladora RAID).".into()
    };
    Ok(out)
}

/// Estado de la autoprueba (para ir consultándolo mientras corre).
#[tauri::command(async)]
pub fn smart_selftest_status(number: u32) -> Result<SelfTest, String> {
    need_admin()?;
    let d = RawDisk::open(number, Mode::Query)?;
    let table = ata_smart(&d, SMART_READ_DATA, 0, true).map_err(|e| format!("El disco no contesta a SMART (error {e}). Las cajas USB y los NVMe no dejan hacer autopruebas."))?;
    Ok(smartx::parse_self_test(&table))
}

/// Lanza la autoprueba corta o extendida del propio disco, o la cancela.
/// "short" · "extended" · "abort". El disco la hace por su cuenta; sigue funcionando.
#[tauri::command(async)]
pub fn smart_selftest(number: u32, action: String, tweaks: State<'_, TweakState>) -> Result<SelfTest, String> {
    need_admin()?;
    let (sub, label) = match action.as_str() {
        "short" => (1u8, "Autoprueba corta del disco"),
        "extended" => (2, "Autoprueba extendida del disco"),
        "abort" => (127, "Cancelar la autoprueba del disco"),
        _ => return Err("Acción no válida.".into()),
    };
    let d = RawDisk::open(number, Mode::Query)?;
    let result = ata_smart(&d, SMART_EXECUTE_SELFTEST, sub, false)
        .map(|_| ())
        .map_err(|e| format!("El disco no aceptó la orden (error {e}). Las cajas USB y los NVMe no dejan hacer autopruebas."));
    tweaks.record(Op::Run, &format!("{label} (disco {number})"), &result);
    result?;
    std::thread::sleep(std::time::Duration::from_millis(800));
    let table = ata_smart(&d, SMART_READ_DATA, 0, true).map_err(|e| format!("No se pudo leer el estado (error {e})."))?;
    Ok(smartx::parse_self_test(&table))
}
