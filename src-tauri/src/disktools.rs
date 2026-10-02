//! Herramientas de disco: velocidad, capacidad real de un pendrive, expulsar y formatear.
//!
//! - **Velocidad**: escribe y lee un archivo de prueba de 256 MB sin pasar por
//!   la caché de Windows (si no, se mediría la memoria, no el disco), y lecturas
//!   aleatorias durante unos segundos. Dice si es normal para ese tipo de disco.
//! - **Capacidad real** (como H2testw): llena el espacio libre de un pendrive con
//!   datos que se pueden comprobar y los vuelve a leer. Los pendrives falsos
//!   dicen tener más capacidad de la real: al pasar el límite, lo que se escribe
//!   se pierde o pisa lo anterior. No toca lo que ya hay y borra la prueba al acabar.
//! - **Expulsar** y **formatear** unidades extraíbles, nunca la de Windows ni
//!   aquella desde la que se ejecuta AdminOps.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::State;

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;
/// Bloque de lectura/escritura: múltiplo del sector, como exige la E/S sin caché.
const BLOCK: usize = MB as usize;
const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;

fn letter_ok(letter: &str) -> Result<char, String> {
    let mut c = letter.trim().trim_end_matches(['\\', ':']).chars();
    match (c.next(), c.next()) {
        (Some(l), None) if l.is_ascii_alphabetic() => Ok(l.to_ascii_uppercase()),
        _ => Err("Unidad no válida.".into()),
    }
}

fn root(l: char) -> PathBuf {
    PathBuf::from(format!("{l}:\\"))
}

fn system_letter() -> char {
    std::env::var("SystemDrive").ok().and_then(|s| s.chars().next()).unwrap_or('C').to_ascii_uppercase()
}

/// Unidad desde la que se ejecuta AdminOps (no se expulsa ni se formatea).
fn own_letter() -> Option<char> {
    std::env::current_exe().ok()?.to_str()?.chars().next().map(|c| c.to_ascii_uppercase())
}

#[cfg(windows)]
fn is_removable(l: char) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let wide: Vec<u16> = format!("{l}:\\").encode_utf16().chain(Some(0)).collect();
    // SAFETY: cadena terminada en cero; solo consulta el tipo de unidad.
    unsafe { GetDriveTypeW(wide.as_ptr()) == 2 }
}

#[cfg(not(windows))]
fn is_removable(_: char) -> bool {
    false
}

#[cfg(windows)]
fn free_bytes(l: char) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = format!("{l}:\\").encode_utf16().chain(Some(0)).collect();
    let mut free = 0u64;
    // SAFETY: punteros válidos a variables locales; los que no interesan, nulos.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(free)
}

#[cfg(not(windows))]
fn free_bytes(_: char) -> Option<u64> {
    None
}

/// Búfer alineado a 4 KB (la E/S sin caché lo exige).
struct Aligned {
    raw: Vec<u8>,
    off: usize,
}

impl Aligned {
    fn new(len: usize) -> Self {
        let raw = vec![0u8; len + 4096];
        let off = (4096 - (raw.as_ptr() as usize % 4096)) % 4096;
        Aligned { raw, off }
    }
    fn buf(&mut self, len: usize) -> &mut [u8] {
        &mut self.raw[self.off..self.off + len]
    }
}

fn open_direct(path: &Path, write: bool) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    if write {
        o.write(true).create(true).truncate(true);
    } else {
        o.read(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.custom_flags(FILE_FLAG_NO_BUFFERING | if write { FILE_FLAG_WRITE_THROUGH } else { 0 });
    }
    o.open(path)
}

/// Datos que dependen de su posición: si el pendrive devuelve otro trozo (o
/// ceros), no coinciden. splitmix64 de (semilla, desplazamiento).
pub fn fill_pattern(buf: &mut [u8], seed: u64, offset: u64) {
    for (i, chunk) in buf.as_chunks_mut::<8>().0.iter_mut().enumerate() {
        let mut z = seed ^ (offset / 8 + i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        chunk.copy_from_slice(&z.to_le_bytes());
    }
}

fn mbps(bytes: u64, t: Duration) -> f64 {
    let s = t.as_secs_f64().max(0.001);
    bytes as f64 / MB as f64 / s
}

// ---------- Velocidad ----------

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    pub write_mbps: f64,
    pub read_mbps: f64,
    /// Lecturas aleatorias de 4 KB por segundo.
    pub random_iops: f64,
    /// ok | warn | bad
    pub level: String,
    pub text: String,
}

/// Si la velocidad es normal para ese tipo de disco.
pub fn speed_verdict(media: &str, bus: &str, read: f64, iops: f64) -> (String, String) {
    let (kind, expected, min_iops) = match (bus.to_uppercase().as_str(), media.to_uppercase().as_str()) {
        ("NVME", _) => ("un SSD NVMe", 800.0, 5000.0),
        ("USB", _) => ("un disco o pendrive USB", 25.0, 0.0),
        ("SD", _) | ("MMC", _) => ("una tarjeta de memoria", 15.0, 0.0),
        (_, "SSD") => ("un SSD SATA", 300.0, 3000.0),
        (_, "HDD") => ("un disco mecánico", 60.0, 40.0),
        _ => ("este tipo de disco", 50.0, 0.0),
    };
    let slow = read < expected * 0.5;
    let few = min_iops > 0.0 && iops < min_iops * 0.3;
    if bus.eq_ignore_ascii_case("USB") && read < 45.0 {
        ("warn".into(), format!("Lee a {read:.0} MB/s: velocidad de USB 2.0. Si el disco o el puerto son USB 3 (azules), prueba en otro puerto o con otro cable."))
    } else if slow || few {
        ("warn".into(), format!("Más lento de lo normal para {kind} (lo normal es desde {expected:.0} MB/s). Puede estar gastado, casi lleno, o conectado a un puerto lento."))
    } else if media.eq_ignore_ascii_case("HDD") && !bus.eq_ignore_ascii_case("USB") {
        ("ok".into(), format!("Normal para {kind}. Si el equipo va lento, cambiarlo por un SSD es la mejora más notable (unas 5 veces más rápido)."))
    } else {
        ("ok".into(), format!("Normal para {kind}."))
    }
}

/// Mide la velocidad de una unidad con un archivo de prueba de 256 MB.
#[tauri::command(async)]
pub fn disk_speed_test(app: tauri::AppHandle, letter: String, media: String, bus: String) -> Result<SpeedResult, String> {
    let l = letter_ok(&letter)?;
    let size: u64 = 256 * MB;
    if free_bytes(l).is_some_and(|f| f < size + 64 * MB) {
        return Err(format!("En {l}: no hay espacio para la prueba (hacen falta unos 320 MB libres)."));
    }
    let task = crate::task::Task::new(&app, format!("disk-speed:{l}")).named(format!("Velocidad de {l}:"));
    let file = root(l).join(".adminops-velocidad.tmp");
    let result = (|| -> Result<SpeedResult, String> {
        let mut buf = Aligned::new(4 * BLOCK);
        let seed = 0xAD_0105;
        task.step("Escribiendo…");
        let mut f = open_direct(&file, true).map_err(|e| format!("No se pudo escribir en {l}: {e}"))?;
        let t = Instant::now();
        let mut done = 0;
        while done < size {
            fill_pattern(buf.buf(4 * BLOCK), seed, done);
            f.write_all(buf.buf(4 * BLOCK)).map_err(|e| format!("Error al escribir: {e}"))?;
            done += 4 * MB;
            if task.cancelled() {
                return Err("Cancelado.".into());
            }
        }
        let write = mbps(size, t.elapsed());
        drop(f);
        task.step("Leyendo…");
        let mut f = open_direct(&file, false).map_err(|e| e.to_string())?;
        let t = Instant::now();
        let mut done = 0;
        while done < size {
            f.read_exact(buf.buf(4 * BLOCK)).map_err(|e| format!("Error al leer: {e}"))?;
            done += 4 * MB;
        }
        let read = mbps(size, t.elapsed());
        task.step("Lecturas aleatorias…");
        let t = Instant::now();
        let mut n = 0u64;
        let mut x: u64 = 0x1234_5678;
        while t.elapsed() < Duration::from_secs(3) {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let pos = (x % (size / 4096)) * 4096;
            f.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
            f.read_exact(buf.buf(4096)).map_err(|e| e.to_string())?;
            n += 1;
        }
        let iops = n as f64 / t.elapsed().as_secs_f64();
        let (level, text) = speed_verdict(&media, &bus, read, iops);
        Ok(SpeedResult { write_mbps: write, read_mbps: read, random_iops: iops, level, text })
    })();
    let _ = std::fs::remove_file(&file);
    result
}

// ---------- Capacidad real (pendrive falso) ----------

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapacityResult {
    /// Lo que se escribió.
    pub tested: u64,
    /// Lo que se volvió a leer bien.
    pub good: u64,
    /// Dónde empezó a fallar (bytes desde el principio de la prueba), si falló.
    pub first_error: Option<u64>,
    pub write_mbps: f64,
    pub read_mbps: f64,
    pub fake: bool,
    pub text: String,
}

/// Qué quiere decir el resultado.
pub fn capacity_verdict(tested: u64, good: u64, first_error: Option<u64>) -> (bool, String) {
    let g = |b: u64| format!("{:.1} GB", b as f64 / GB as f64);
    match first_error {
        None => (false, format!("Todo bien: se escribieron y se leyeron sin errores {} (todo el espacio libre). La capacidad es real.", g(tested))),
        Some(at) if at > 0 && good + GB / 2 < tested && good as f64 / tested as f64 > 0.02 => (
            true,
            format!(
                "Pendrive falso o dañado: a partir de {} lo que se escribe se pierde. Solo {} de {} se leen bien. No lo uses para nada importante; se puede reformatear a su tamaño real con herramientas del fabricante o tirar.",
                g(at),
                g(good),
                g(tested)
            ),
        ),
        Some(_) => (true, format!("Errores al leer lo escrito: solo {} de {} se leen bien. El pendrive está dañado.", g(good), g(tested))),
    }
}

/// Llena el espacio libre y lo comprueba. Tarda: minutos por cada GB en pendrives lentos.
#[tauri::command(async)]
pub fn disk_capacity_test(app: tauri::AppHandle, letter: String, tweaks: State<'_, TweakState>) -> Result<CapacityResult, String> {
    let l = letter_ok(&letter)?;
    if !is_removable(l) {
        return Err("Solo para pendrives y tarjetas de memoria (unidades extraíbles).".into());
    }
    if own_letter() == Some(l) {
        return Err("AdminOps se está ejecutando desde esta unidad: prueba otra, o copia AdminOps a otro sitio para probar esta.".into());
    }
    let free = free_bytes(l).ok_or("No se puede leer el espacio libre.")?;
    // Se deja un margen para que el sistema de archivos no se ahogue.
    let usable = free.saturating_sub(32 * MB) / MB * MB;
    if usable < 64 * MB {
        return Err("Hay muy poco espacio libre para probar. La prueba usa el espacio libre sin tocar lo que ya hay: libera algo o vacíalo antes.".into());
    }
    let dir = root(l).join("AdminOps-prueba-capacidad");
    let task = crate::task::Task::new(&app, format!("disk-capacity:{l}")).named(format!("Capacidad real de {l}:"));
    let result = (|| -> Result<CapacityResult, String> {
        std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo escribir en {l}: {e}"))?;
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos() as u64);
        let mut buf = Aligned::new(BLOCK);
        let mut files = Vec::new();
        let (mut written, t) = (0u64, Instant::now());
        // Escribir: archivos de 1 GB hasta llenar.
        'files: for i in 0.. {
            if written >= usable {
                break;
            }
            let len = (usable - written).min(GB);
            let path = dir.join(format!("{i:04}.bin"));
            let Ok(mut f) = open_direct(&path, true) else { break };
            files.push((path, written, 0u64));
            let mut n = 0;
            while n < len {
                fill_pattern(buf.buf(BLOCK), seed, written + n);
                if f.write_all(buf.buf(BLOCK)).is_err() {
                    // Lleno antes de lo previsto (o error de escritura): se prueba lo escrito.
                    files.last_mut().unwrap().2 = n;
                    written += n;
                    break 'files;
                }
                n += MB;
                if n % (64 * MB) == 0 {
                    task.step(format!("Escribiendo… {:.1} de {:.1} GB ({:.0} MB/s)", (written + n) as f64 / GB as f64, usable as f64 / GB as f64, mbps(written + n, t.elapsed())));
                }
                if task.cancelled() {
                    return Err("Cancelado.".into());
                }
            }
            files.last_mut().unwrap().2 = n;
            written += n;
        }
        let write_mbps = mbps(written, t.elapsed());
        // Leer y comprobar (sin caché: si no, se leería la memoria).
        let mut expect = vec![0u8; BLOCK];
        let (mut good, mut first_error, t) = (0u64, None, Instant::now());
        for (path, start, len) in &files {
            let mut f = match open_direct(path, false) {
                Ok(f) => f,
                Err(_) => {
                    first_error.get_or_insert(*start);
                    continue;
                }
            };
            let mut n = 0;
            while n < *len {
                let ok = f.read_exact(buf.buf(BLOCK)).is_ok() && {
                    fill_pattern(&mut expect, seed, start + n);
                    buf.buf(BLOCK) == expect.as_slice()
                };
                if ok {
                    good += MB;
                } else {
                    first_error.get_or_insert(start + n);
                }
                n += MB;
                if n % (64 * MB) == 0 {
                    task.step(format!("Comprobando… {:.1} de {:.1} GB", (start + n) as f64 / GB as f64, written as f64 / GB as f64));
                }
                if task.cancelled() {
                    return Err("Cancelado.".into());
                }
            }
        }
        let (fake, text) = capacity_verdict(written, good, first_error);
        Ok(CapacityResult { tested: written, good, first_error, write_mbps, read_mbps: mbps(good, t.elapsed()), fake, text })
    })();
    task.step("Borrando los archivos de prueba…");
    let _ = std::fs::remove_dir_all(&dir);
    let journal = result.as_ref().map(|_| ()).map_err(Clone::clone);
    tweaks.record(Op::Run, &format!("Comprobar la capacidad real de {l}:"), &journal);
    result
}

// ---------- Expulsar y formatear ----------

fn removable_target(l: char) -> Result<(), String> {
    if l == system_letter() {
        return Err("Es el disco de Windows.".into());
    }
    if own_letter() == Some(l) {
        return Err("AdminOps se está ejecutando desde esta unidad.".into());
    }
    if !is_removable(l) && !usb_disk(l) {
        return Err("Solo para pendrives, tarjetas y discos externos USB.".into());
    }
    Ok(())
}

/// ¿Es un disco USB (los discos externos se ven como fijos, no como extraíbles)?
fn usb_disk(l: char) -> bool {
    crate::pspool::query(&format!("\"$((Get-Partition -DriveLetter {l} -ErrorAction SilentlyContinue | Get-Disk -ErrorAction SilentlyContinue).BusType)\""), Some(Duration::from_secs(15)), "Tipo de disco")
        .is_ok_and(|o| o.trim().eq_ignore_ascii_case("USB"))
}

/// Programas abiertos desde esa unidad (lo típico que impide expulsarla).
fn programs_on(l: char) -> Vec<String> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let prefix = format!("{l}:\\").to_lowercase();
    let mut names: Vec<String> = sys
        .processes()
        .values()
        .filter(|p| p.exe().is_some_and(|e| e.to_string_lossy().to_lowercase().starts_with(&prefix)))
        .map(|p| p.name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// «Expulsar» de Windows, y comprueba que de verdad se fue.
#[tauri::command(async)]
pub fn disk_eject(letter: String) -> Result<(), String> {
    let l = letter_ok(&letter)?;
    removable_target(l)?;
    let script = format!("$s = New-Object -ComObject Shell.Application\n$i = $s.Namespace(17).ParseName('{l}:')\nif ($i) {{ $i.InvokeVerb('Eject') }}\nStart-Sleep -Milliseconds 2500\nTest-Path '{l}:\\'");
    let still = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(30)), task: None })?;
    if still.trim().eq_ignore_ascii_case("true") {
        let who = programs_on(l);
        return Err(if who.is_empty() {
            format!("Windows no pudo expulsar {l}: algún programa o ventana del Explorador tiene algo abierto ahí. Ciérralos y vuelve a intentarlo.")
        } else {
            format!("Windows no pudo expulsar {l}: lo están usando {}. Ciérralos y vuelve a intentarlo.", who.join(", "))
        });
    }
    Ok(())
}

/// Nombre válido para un volumen (FAT: 11 caracteres; NTFS y exFAT: 32 / 15).
pub fn clean_label(label: &str, fs: &str) -> String {
    let max = if fs.eq_ignore_ascii_case("FAT32") { 11 } else if fs.eq_ignore_ascii_case("exFAT") { 15 } else { 32 };
    label.chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_').take(max).collect::<String>().trim().to_string()
}

/// Formatea una unidad extraíble (borra todo lo que tiene).
#[tauri::command(async)]
pub fn disk_format(letter: String, fs: String, label: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    let l = letter_ok(&letter)?;
    removable_target(l)?;
    let fs = match fs.as_str() {
        "exFAT" | "FAT32" | "NTFS" => fs,
        _ => return Err("Formato no válido.".into()),
    };
    let label = clean_label(&label, &fs);
    let script = format!("Format-Volume -DriveLetter {l} -FileSystem {fs} -NewFileSystemLabel {} -Confirm:$false -Force -ErrorAction Stop | Out-Null\n'ok'", crate::ps::ps_literal(&label));
    let r = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(600)), task: None }).map(|_| ()).map_err(|e| {
        if e.to_lowercase().contains("fat32") || e.contains("too large") || e.contains("demasiado grande") {
            "FAT32 no admite unidades de más de 32 GB en Windows: usa exFAT.".to_string()
        } else {
            e
        }
    });
    tweaks.record(Op::Run, &format!("Formatear {l}: como {fs}"), &r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_depends_on_position() {
        let (mut a, mut b) = (vec![0u8; 64], vec![0u8; 64]);
        fill_pattern(&mut a, 1, 0);
        fill_pattern(&mut b, 1, 0);
        assert_eq!(a, b, "misma posición, mismos datos");
        fill_pattern(&mut b, 1, GB);
        assert_ne!(a, b, "un pendrive que devuelve otro trozo se detecta");
        fill_pattern(&mut b, 2, 0);
        assert_ne!(a, b, "cada prueba con su semilla");
        assert!(a.iter().any(|x| *x != 0));
    }

    #[test]
    fn capacity_verdicts() {
        let (fake, t) = capacity_verdict(60 * GB, 60 * GB, None);
        assert!(!fake && t.contains("real"));
        // Un «64 GB» que en realidad tiene 8.
        let (fake, t) = capacity_verdict(60 * GB, 8 * GB, Some(8 * GB));
        assert!(fake && t.contains("falso") && t.contains("8.0 GB"), "{t}");
        let (fake, t) = capacity_verdict(10 * GB, 0, Some(0));
        assert!(fake && t.contains("dañado"), "{t}");
    }

    #[test]
    fn speed_verdicts() {
        assert_eq!(speed_verdict("SSD", "NVMe", 2500.0, 20000.0).0, "ok");
        assert_eq!(speed_verdict("SSD", "NVMe", 300.0, 20000.0).0, "warn");
        assert!(speed_verdict("", "USB", 30.0, 0.0).1.contains("USB 2.0"));
        assert!(speed_verdict("HDD", "SATA", 120.0, 90.0).1.contains("SSD"));
        assert_eq!(speed_verdict("SSD", "SATA", 520.0, 8000.0).0, "ok");
    }

    #[test]
    fn labels_are_valid() {
        assert_eq!(clean_label("Mis fotos <2026>", "FAT32"), "Mis fotos 2");
        assert_eq!(clean_label("DATOS", "NTFS"), "DATOS");
        assert_eq!(clean_label("   ", "exFAT"), "");
    }

    /// Escritura y lectura sin caché de verdad (alineación y tamaños que exige Windows).
    #[test]
    fn direct_io_round_trip() {
        let path = std::env::temp_dir().join(format!("adminops-direct-{}.bin", std::process::id()));
        let mut buf = Aligned::new(BLOCK);
        {
            let mut f = open_direct(&path, true).unwrap();
            for i in 0..4u64 {
                fill_pattern(buf.buf(BLOCK), 9, i * MB);
                f.write_all(buf.buf(BLOCK)).unwrap();
            }
        }
        let mut f = open_direct(&path, false).unwrap();
        let mut expect = vec![0u8; BLOCK];
        for i in 0..4u64 {
            f.read_exact(buf.buf(BLOCK)).unwrap();
            fill_pattern(&mut expect, 9, i * MB);
            assert!(buf.buf(BLOCK) == expect.as_slice(), "bloque {i}");
        }
        drop(f);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn aligned_buffers() {
        let mut a = Aligned::new(8192);
        assert_eq!(a.buf(8192).as_ptr() as usize % 4096, 0);
    }
}
