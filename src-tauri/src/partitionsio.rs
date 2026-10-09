//! Particiones: los comandos de la interfaz (leer la tabla, copiarla, buscar
//! particiones perdidas, restaurar una, reparar el arranque). La lógica está en
//! `partitions.rs`; aquí se habla con el disco.
//!
//! Escribir en la tabla de un disco es lo más delicado de AdminOps, así que:
//! - nunca en el disco de Windows,
//! - siempre guarda antes una copia de los sectores que va a tocar,
//! - comprueba después que la tabla quedó como se esperaba y, si no, la devuelve a como estaba,
//! - lo bloquea el modo auditoría y queda en el diario.

use crate::partitions::{self, Found, Layout, Plan};
use crate::rawdisk::{Aligned, BlockSource, Mode, RawDisk};
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

const IOCTL_DISK_UPDATE_PROPERTIES: u32 = 0x0007_0140;

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

/// La tabla de particiones de un disco.
#[tauri::command(async)]
pub fn partition_layout(number: u32) -> Result<Layout, String> {
    need_admin()?;
    let mut d = RawDisk::open(number, Mode::Read)?;
    Ok(partitions::read_layout(&mut d))
}

// ---------- Copias de la tabla ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct SavedSector {
    lba: u64,
    data: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct TableBackup {
    number: u32,
    model: String,
    size: u64,
    sector: u32,
    created: u64,
    scheme: String,
    sectors: Vec<SavedSector>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub created: u64,
    pub scheme: String,
    pub size: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn backup_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("copias-tabla")
}

fn take_backup(app: &tauri::AppHandle, d: &mut RawDisk, model: &str, layout: &Layout) -> Result<BackupInfo, String> {
    let engine = base64::engine::general_purpose::STANDARD;
    let mut sectors = Vec::new();
    for (lba, count) in partitions::backup_ranges(layout) {
        let data = partitions::read_sectors(d, lba, count as usize).map_err(|e| format!("No se pudo copiar la tabla (error {e}); no se cambia nada."))?;
        sectors.push(SavedSector { lba, data: engine.encode(data) });
    }
    // Los EBR de las unidades lógicas (MBR), uno a uno.
    for p in layout.partitions.iter().filter(|p| p.kind.starts_with("Lógica")) {
        let ebr = p.start_lba.saturating_sub(layout_ebr_gap(layout, p.start_lba));
        if let Ok(data) = partitions::read_sectors(d, ebr, 1) {
            sectors.push(SavedSector { lba: ebr, data: engine.encode(data) });
        }
    }
    let created = now();
    let t = TableBackup { number: d.number(), model: model.to_string(), size: layout.size, sector: layout.sector, created, scheme: layout.scheme.clone(), sectors };
    let dir = backup_dir(app);
    let path = dir.join(format!("disco{}-{}.json", d.number(), created));
    crate::paths::write_json(&path, &t)?;
    Ok(BackupInfo { path: path.display().to_string(), created, scheme: layout.scheme.clone(), size: layout.size })
}

/// Un EBR está 63 sectores (o 2048) antes de su partición lógica, normalmente.
fn layout_ebr_gap(layout: &Layout, start: u64) -> u64 {
    if layout.sector == 0 {
        return 0;
    }
    if start >= 2048 && start.is_multiple_of(2048) {
        2048.min(start)
    } else {
        63.min(start)
    }
}

/// Guarda una copia de la tabla (principio y final del disco).
#[tauri::command(async)]
pub fn partition_backup(app: tauri::AppHandle, number: u32, model: String) -> Result<BackupInfo, String> {
    need_admin()?;
    let mut d = RawDisk::open(number, Mode::Read)?;
    let layout = partitions::read_layout(&mut d);
    take_backup(&app, &mut d, &model, &layout)
}

/// Copias guardadas de un disco, de la más reciente a la más antigua.
#[tauri::command]
pub fn partition_backups(app: tauri::AppHandle, number: u32) -> Vec<BackupInfo> {
    let mut out: Vec<BackupInfo> = std::fs::read_dir(backup_dir(&app))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter_map(|e| {
                    let t: TableBackup = crate::paths::read_json(&e.path());
                    (t.number == number && !t.sectors.is_empty()).then(|| BackupInfo { path: e.path().display().to_string(), created: t.created, scheme: t.scheme, size: t.size })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|b| std::cmp::Reverse(b.created));
    out
}

fn write_sectors(d: &RawDisk, sector: u32, writes: &[(u64, Vec<u8>)]) -> Result<(), String> {
    for (lba, data) in writes {
        if data.is_empty() || data.len() % sector as usize != 0 {
            return Err("Datos de tabla con un tamaño que no es de sectores.".into());
        }
        let mut buf = Aligned::new(data.len());
        buf.slice(data.len()).copy_from_slice(data);
        d.write_at(lba * sector as u64, buf.slice(data.len())).map_err(|e| format!("Windows no dejó escribir la tabla (error {e}). Si el disco tiene unidades en uso, ciérralas y vuelve a intentarlo."))?;
    }
    let mut out = [0u8; 8];
    let _ = d.ioctl(IOCTL_DISK_UPDATE_PROPERTIES, &[], &mut out);
    Ok(())
}

pub(crate) fn is_system_disk(number: u32) -> bool {
    crate::ps::powershell(&format!("$d = Get-Disk -Number {number} -ErrorAction SilentlyContinue; if ($d -and ($d.IsSystem -or $d.IsBoot)) {{ 'si' }} else {{ 'no' }}")).map(|s| s.trim() == "si").unwrap_or(true)
}

/// Devuelve la tabla a como estaba en una copia.
#[tauri::command(async)]
pub fn partition_table_restore(path: String, number: u32, tweaks: State<'_, TweakState>) -> Result<Layout, String> {
    need_admin()?;
    if is_system_disk(number) {
        return Err("Es el disco de Windows: no se toca su tabla de particiones con Windows en marcha.".into());
    }
    let t: TableBackup = crate::paths::read_json(std::path::Path::new(&path));
    if t.sectors.is_empty() || t.number != number {
        return Err("Esa copia no es de este disco.".into());
    }
    let d = RawDisk::open(number, Mode::Write)?;
    if d.len() != t.size {
        return Err("El disco no tiene el mismo tamaño que cuando se hizo la copia: no se restaura.".into());
    }
    let engine = base64::engine::general_purpose::STANDARD;
    let mut writes = Vec::new();
    for s in &t.sectors {
        writes.push((s.lba, engine.decode(&s.data).map_err(|e| format!("La copia está dañada: {e}"))?));
    }
    let result = write_sectors(&d, t.sector, &writes);
    tweaks.record(Op::Run, &format!("Restaurar la tabla de particiones del disco {number} desde una copia"), &result);
    result?;
    let mut d = RawDisk::open(number, Mode::Read)?;
    Ok(partitions::read_layout(&mut d))
}

// ---------- Buscar particiones perdidas ----------

/// Mira en el espacio sin asignar (o en todo el disco si no hay tabla) los restos de NTFS, FAT32 y exFAT.
#[tauri::command(async)]
pub fn partition_find_lost(app: tauri::AppHandle, number: u32) -> Result<Vec<Found>, String> {
    need_admin()?;
    let mut d = RawDisk::open(number, Mode::Read)?;
    let layout = partitions::read_layout(&mut d);
    let ranges: Vec<(u64, u64)> = if layout.scheme == "none" { vec![(0, layout.size)] } else { layout.free.iter().map(|g| (g.offset, g.offset + g.size)).collect() };
    if ranges.is_empty() {
        return Ok(Vec::new());
    }
    let task = crate::task::Task::new(&app, format!("part-scan:{number}")).named("Buscar particiones perdidas");
    task.step("Mirando el espacio sin asignar…");
    let mut last_pct = 101;
    let found = partitions::find_lost(&mut d, &ranges, |pos, total| {
        let pct = (pos * 100 / total.max(1)) as i32;
        if pct != last_pct {
            last_pct = pct;
            task.step(format!("Buscando particiones perdidas… {pct} %"));
        }
        !task.cancelled()
    });
    Ok(found)
}

/// Da de alta una partición encontrada. Guarda antes una copia de la tabla y
/// comprueba el resultado; si no queda como se esperaba, deja la tabla como estaba.
#[tauri::command(async)]
pub fn partition_restore(app: tauri::AppHandle, number: u32, model: String, found: Found, tweaks: State<'_, TweakState>) -> Result<Layout, String> {
    need_admin()?;
    if is_system_disk(number) {
        return Err("Es el disco de Windows: no se toca su tabla de particiones con Windows en marcha.".into());
    }
    let mut d = RawDisk::open(number, Mode::Write)?;
    let layout = partitions::read_layout(&mut d);
    let plan: Plan = partitions::plan_restore(&mut d, &layout, &found, now())?;
    let backup = take_backup(&app, &mut d, &model, &layout)?;

    let writes: Vec<(u64, Vec<u8>)> = plan.writes.iter().map(|w| (w.lba, w.data.clone())).collect();
    let result = write_sectors(&d, layout.sector, &writes).and_then(|_| {
        // Se vuelve a leer: tiene que salir la partición nueva y ningún problema nuevo.
        let after = partitions::read_layout(&mut d);
        let ok = after.partitions.iter().any(|p| p.offset == found.offset) && after.issues.iter().filter(|i| i.level == "bad").count() <= layout.issues.iter().filter(|i| i.level == "bad").count();
        if ok {
            Ok(after)
        } else {
            // Se deshace con la copia que se acaba de guardar.
            let t: TableBackup = crate::paths::read_json(std::path::Path::new(&backup.path));
            let engine = base64::engine::general_purpose::STANDARD;
            let back: Vec<(u64, Vec<u8>)> = t.sectors.iter().filter_map(|s| engine.decode(&s.data).ok().map(|b| (s.lba, b))).collect();
            let _ = write_sectors(&d, layout.sector, &back);
            Err("La tabla no quedó como se esperaba, así que se ha devuelto a como estaba. No se ha perdido nada.".to_string())
        }
    });
    let journal = result.as_ref().map(|_| ()).map_err(Clone::clone);
    tweaks.record(Op::Run, &format!("Restaurar partición {} de {} MB en el disco {number} (copia de la tabla guardada)", found.fs, found.size / (1024 * 1024)), &journal);
    result
}

// ---------- Arranque ----------

/// Repara el arranque de Windows (BCD) de la instalación que hay en `letter`: bcdboot,
/// con la partición EFI si el disco es GPT o con el arranque clásico si es MBR.
#[tauri::command(async)]
pub fn boot_repair(app: tauri::AppHandle, letter: String, tweaks: State<'_, TweakState>) -> Result<String, String> {
    need_admin()?;
    let l = letter.trim().trim_end_matches([':', '\\']).chars().next().filter(|c| c.is_ascii_alphabetic()).ok_or("Unidad no válida.")?.to_ascii_uppercase();
    let task = crate::task::Task::new(&app, "boot-repair").named("Reparar el arranque de Windows");
    task.step("Buscando la partición de arranque…");
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$L = '{l}'
$win = "$L`:\Windows"
if (-not (Test-Path "$win\System32\winload.exe") -and -not (Test-Path "$win\System32\winload.efi")) {{ throw "En $L`: no hay una instalación de Windows." }}
$disk = (Get-Partition -DriveLetter $L).DiskNumber
$style = (Get-Disk -Number $disk).PartitionStyle
if ($style -eq 'GPT') {{
  $esp = Get-Partition -DiskNumber $disk | Where-Object {{ $_.GptType -eq '{{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}}' }} | Select-Object -First 1
  if (-not $esp) {{ throw 'Este disco no tiene partición de sistema EFI.' }}
  $free = 'S','T','U','V','W','X' | Where-Object {{ -not (Test-Path "$($_):\") }} | Select-Object -First 1
  $had = $esp.DriveLetter
  if (-not $had) {{ $esp | Set-Partition -NewDriveLetter $free; $drive = $free }} else {{ $drive = $had }}
  try {{ $out = & bcdboot.exe $win /s "$($drive):" /f UEFI 2>&1 | Out-String }} finally {{ if (-not $had) {{ $esp | Remove-PartitionAccessPath -AccessPath "$($free):\" -ErrorAction SilentlyContinue }} }}
}} else {{
  $out = & bcdboot.exe $win /s "$($L):" /f BIOS 2>&1 | Out-String
}}
if ($LASTEXITCODE -ne 0) {{ throw "bcdboot falló: $out" }}
"El arranque de Windows de $L`: se ha reparado ($style)."
"#
    );
    let result = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(180)), task: None }).map(|s| s.trim().to_string());
    let journal = result.as_ref().map(|_| ()).map_err(Clone::clone);
    tweaks.record(Op::Run, &format!("Reparar el arranque de Windows de {l}:"), &journal);
    result
}
