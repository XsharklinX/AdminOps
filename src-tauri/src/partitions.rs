//! Particiones: leer la tabla de un disco (MBR o GPT), detectar lo que está roto,
//! encontrar particiones perdidas y preparar su restauración. Es lo que se hacía
//! con TestDisk.
//!
//! Este archivo no toca el disco: trabaja sobre un `BlockSource` y devuelve
//! datos y «planes de escritura» (qué sectores se cambiarían y por qué). Quien
//! escribe de verdad es `partitionsio.rs`, después de guardar una copia de la
//! tabla y de que el técnico lo confirme.

use crate::rawdisk::{Aligned, BlockSource};
use serde::{Deserialize, Serialize};

pub const GPT_SIG: &[u8; 8] = b"EFI PART";
const MIB: u64 = 1024 * 1024;

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for b in data {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn le64(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}

/// GUID en su forma de texto (los tres primeros campos van en little-endian).
pub fn guid_str(b: &[u8]) -> String {
    format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        le32(b, 0),
        le16(b, 4),
        le16(b, 6),
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    )
}

pub fn guid_bytes(s: &str) -> Option<[u8; 16]> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let raw: Vec<u8> = (0..16).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()).collect::<Option<_>>()?;
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&u32::from_str_radix(&hex[0..8], 16).ok()?.to_le_bytes());
    out[4..6].copy_from_slice(&u16::from_str_radix(&hex[8..12], 16).ok()?.to_le_bytes());
    out[6..8].copy_from_slice(&u16::from_str_radix(&hex[12..16], 16).ok()?.to_le_bytes());
    out[8..16].copy_from_slice(&raw[8..16]);
    Some(out)
}

pub const GUID_EFI: &str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
pub const GUID_MSR: &str = "E3C9E316-0B5C-4DB8-817D-F92DF00215AE";
pub const GUID_DATA: &str = "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7";
const GUID_RECOVERY: &str = "DE94BBA4-06D1-4D40-A16A-BFD50179D6AC";

fn gpt_kind(guid: &str) -> &'static str {
    match guid {
        GUID_EFI => "Sistema EFI",
        GUID_MSR => "Reservada de Microsoft",
        GUID_DATA => "Datos",
        GUID_RECOVERY => "Recuperación de Windows",
        "0FC63DAF-8483-4772-8E79-3D69D8477DE4" => "Linux",
        "0657FD6D-A4AB-43C4-84E5-0933C84B4F4F" => "Linux (intercambio)",
        "E75CAF8F-F680-4CEE-AFA3-B001E56EFC2D" => "Espacios de almacenamiento",
        "48465300-0000-11AA-AA11-00306543ECC3" => "Apple HFS+",
        _ => "Otra",
    }
}

fn mbr_kind(t: u8) -> &'static str {
    match t {
        0x01 | 0x04 | 0x06 | 0x0E => "FAT16",
        0x07 => "NTFS / exFAT",
        0x0B | 0x0C => "FAT32",
        0x05 | 0x0F | 0x85 => "Extendida",
        0x27 => "Recuperación de Windows",
        0x82 => "Linux (intercambio)",
        0x83 => "Linux",
        0xAF => "Apple HFS+",
        0xEE => "GPT protectora",
        _ => "Otra",
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Part {
    pub index: u32,
    pub start_lba: u64,
    pub sectors: u64,
    pub offset: u64,
    pub size: u64,
    pub kind: String,
    /// NTFS · FAT32 · exFAT · BitLocker… (lo que hay dentro).
    pub fs: String,
    /// Partición activa (MBR).
    pub boot: bool,
    pub type_guid: String,
    pub name: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// warn | bad
    pub level: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Gap {
    pub offset: u64,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// gpt · mbr · none
    pub scheme: String,
    pub sector: u32,
    pub size: u64,
    pub disk_guid: String,
    pub partitions: Vec<Part>,
    /// Espacio sin asignar (≥ 8 MB).
    pub free: Vec<Gap>,
    pub issues: Vec<Issue>,
    /// Hay una partición EFI (GPT) o activa (MBR) de la que arrancar.
    pub bootable: bool,
}

fn issue(level: &str, text: impl Into<String>) -> Issue {
    Issue { level: level.into(), text: text.into() }
}

pub fn read_sectors<S: BlockSource>(src: &mut S, lba: u64, count: usize) -> Result<Vec<u8>, u32> {
    let sector = src.sector() as usize;
    let mut buf = Aligned::new(sector * count);
    let n = src.read_at(lba * sector as u64, buf.slice(sector * count))?;
    if n < sector * count {
        return Err(0);
    }
    Ok(buf.slice(sector * count).to_vec())
}

/// Qué sistema de archivos hay en el primer sector de una partición.
pub fn detect_fs(boot: &[u8]) -> &'static str {
    if boot.len() < 512 {
        return "";
    }
    if &boot[3..11] == b"NTFS    " {
        "NTFS"
    } else if &boot[3..11] == b"EXFAT   " {
        "exFAT"
    } else if &boot[3..11] == b"-FVE-FS-" {
        "BitLocker"
    } else if &boot[82..90] == b"FAT32   " {
        "FAT32"
    } else if &boot[54..59] == b"FAT16" || &boot[54..59] == b"FAT12" {
        "FAT"
    } else {
        ""
    }
}

pub struct GptHeader {
    pub alt_lba: u64,
    pub first_usable: u64,
    pub last_usable: u64,
    pub disk_guid: [u8; 16],
    pub entries_lba: u64,
    pub num_entries: u32,
    pub entry_size: u32,
    pub header_crc_ok: bool,
    pub entries_crc: u32,
}

pub fn parse_gpt_header(b: &[u8]) -> Option<GptHeader> {
    if b.len() < 92 || &b[0..8] != GPT_SIG {
        return None;
    }
    let size = le32(b, 12) as usize;
    if !(92..=b.len()).contains(&size) {
        return None;
    }
    let stored = le32(b, 16);
    let mut copy = b[..size].to_vec();
    copy[16..20].fill(0);
    let mut guid = [0u8; 16];
    guid.copy_from_slice(&b[56..72]);
    Some(GptHeader {
        alt_lba: le64(b, 32),
        first_usable: le64(b, 40),
        last_usable: le64(b, 48),
        disk_guid: guid,
        entries_lba: le64(b, 72),
        num_entries: le32(b, 80),
        entry_size: le32(b, 84),
        header_crc_ok: crc32(&copy) == stored,
        entries_crc: le32(b, 88),
    })
}

fn utf16_name(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    String::from_utf16_lossy(&units)
}

fn read_gpt_entries<S: BlockSource>(src: &mut S, h: &GptHeader) -> Option<Vec<u8>> {
    if h.num_entries == 0 || h.num_entries > 1024 || !(128..=1024).contains(&h.entry_size) {
        return None;
    }
    let total = h.num_entries as usize * h.entry_size as usize;
    let sector = src.sector() as usize;
    let sectors = total.div_ceil(sector);
    let mut raw = read_sectors(src, h.entries_lba, sectors).ok()?;
    raw.truncate(total);
    Some(raw)
}

fn fs_of<S: BlockSource>(src: &mut S, offset: u64) -> String {
    let sector = src.sector() as u64;
    read_sectors(src, offset / sector, 1).map(|b| detect_fs(&b).to_string()).unwrap_or_default()
}

fn mbr_partitions<S: BlockSource>(src: &mut S, mbr: &[u8], out: &mut Vec<Part>, issues: &mut Vec<Issue>) {
    let sector = src.sector() as u64;
    let mut ext_start = 0u64;
    for i in 0..4 {
        let e = &mbr[446 + i * 16..446 + i * 16 + 16];
        let (t, lba, n) = (e[4], le32(e, 8) as u64, le32(e, 12) as u64);
        if t == 0 || n == 0 {
            continue;
        }
        if matches!(t, 0x05 | 0x0F | 0x85) {
            ext_start = lba;
            out.push(Part { index: out.len() as u32 + 1, start_lba: lba, sectors: n, offset: lba * sector, size: n * sector, kind: "Extendida".into(), ..Default::default() });
            continue;
        }
        let offset = lba * sector;
        out.push(Part { index: out.len() as u32 + 1, start_lba: lba, sectors: n, offset, size: n * sector, kind: mbr_kind(t).into(), boot: e[0] == 0x80, fs: fs_of(src, offset), ..Default::default() });
    }
    // Unidades lógicas: una cadena de EBR dentro de la extendida.
    if ext_start > 0 {
        let mut ebr_lba = ext_start;
        for _ in 0..128 {
            let Ok(b) = read_sectors(src, ebr_lba, 1) else {
                issues.push(issue("warn", "No se pudo leer una de las unidades lógicas de la partición extendida."));
                break;
            };
            if le16(&b, 510) != 0xAA55 {
                break;
            }
            let (t, rel, n) = (b[446 + 4], le32(&b, 446 + 8) as u64, le32(&b, 446 + 12) as u64);
            if t != 0 && n > 0 {
                let lba = ebr_lba + rel;
                let offset = lba * sector;
                out.push(Part { index: out.len() as u32 + 1, start_lba: lba, sectors: n, offset, size: n * sector, kind: format!("Lógica · {}", mbr_kind(t)), fs: fs_of(src, offset), ..Default::default() });
            }
            let (nt, next) = (b[462 + 4], le32(&b, 462 + 8) as u64);
            if nt == 0 || next == 0 {
                break;
            }
            ebr_lba = ext_start + next;
        }
    }
}

/// Lee la tabla de particiones de un disco y dice qué le pasa.
pub fn read_layout<S: BlockSource>(src: &mut S) -> Layout {
    let sector = src.sector();
    let size = src.len();
    let mut l = Layout { scheme: "none".into(), sector, size, ..Default::default() };
    let Ok(mbr) = read_sectors(src, 0, 1) else {
        l.issues.push(issue("bad", "No se puede leer el primer sector del disco: sin él no hay tabla de particiones que mirar."));
        return l;
    };
    let has_mbr = le16(&mbr, 510) == 0xAA55;
    let protective = has_mbr && (0..4).any(|i| mbr[446 + i * 16 + 4] == 0xEE);
    let gpt = read_sectors(src, 1, 1).ok().and_then(|b| parse_gpt_header(&b));

    if let Some(h) = gpt {
        l.scheme = "gpt".into();
        l.disk_guid = guid_str(&h.disk_guid);
        if !protective {
            l.issues.push(issue("warn", "Falta la tabla MBR protectora: algunos programas viejos podrían ver el disco como vacío."));
        }
        if !h.header_crc_ok {
            l.issues.push(issue("bad", "La cabecera de la tabla GPT está dañada (su suma de control no cuadra)."));
        }
        match read_gpt_entries(src, &h) {
            Some(raw) => {
                if crc32(&raw) != h.entries_crc {
                    l.issues.push(issue("bad", "La lista de particiones GPT está dañada (su suma de control no cuadra)."));
                }
                for (i, e) in raw.chunks_exact(h.entry_size as usize).enumerate() {
                    if e[..16].iter().all(|b| *b == 0) {
                        continue;
                    }
                    let (first, last) = (le64(e, 32), le64(e, 40));
                    if last < first {
                        l.issues.push(issue("bad", format!("La partición {} tiene un final anterior a su principio.", i + 1)));
                        continue;
                    }
                    let guid = guid_str(&e[..16]);
                    let offset = first * sector as u64;
                    l.partitions.push(Part {
                        index: i as u32 + 1,
                        start_lba: first,
                        sectors: last - first + 1,
                        offset,
                        size: (last - first + 1) * sector as u64,
                        kind: gpt_kind(&guid).into(),
                        fs: fs_of(src, offset),
                        type_guid: guid,
                        name: utf16_name(&e[56..e.len().min(128)]),
                        ..Default::default()
                    });
                }
            }
            None => l.issues.push(issue("bad", "No se puede leer la lista de particiones GPT.")),
        }
        // La copia de seguridad de la GPT, al final del disco.
        let last_lba = size / sector as u64 - 1;
        match read_sectors(src, last_lba, 1).ok().and_then(|b| parse_gpt_header(&b)) {
            Some(bk) if bk.header_crc_ok && bk.entries_crc == h.entries_crc => {}
            Some(_) => l.issues.push(issue("warn", "La copia de seguridad de la tabla GPT (al final del disco) no coincide con la principal.")),
            None => l.issues.push(issue("warn", "Falta la copia de seguridad de la tabla GPT al final del disco.")),
        }
        if h.alt_lba != last_lba {
            l.issues.push(issue("warn", "La tabla GPT cree que el disco tiene otro tamaño: puede haberse clonado a un disco distinto."));
        }
        l.bootable = l.partitions.iter().any(|p| p.type_guid == GUID_EFI);
    } else if has_mbr && !protective {
        l.scheme = "mbr".into();
        let mut issues = Vec::new();
        let mut parts = Vec::new();
        mbr_partitions(src, &mbr, &mut parts, &mut issues);
        l.issues.extend(issues);
        l.bootable = parts.iter().any(|p| p.boot);
        if !l.bootable && !parts.is_empty() {
            l.issues.push(issue("warn", "Ninguna partición está marcada como activa: este disco no arrancaría en un equipo con BIOS clásica."));
        }
        l.partitions = parts;
    } else if protective {
        l.scheme = "gpt".into();
        l.issues.push(issue("bad", "El disco dice ser GPT pero su cabecera está dañada o ha desaparecido: las particiones no se ven."));
    } else {
        l.issues.push(issue("bad", "El disco no tiene tabla de particiones (aparece «sin formato» o sin asignar). Si tenía datos, puede que sigan ahí: busca particiones perdidas."));
    }

    // Solapes y particiones que se salen del disco.
    let mut sorted: Vec<&Part> = l.partitions.iter().filter(|p| p.kind != "Extendida").collect();
    sorted.sort_by_key(|p| p.offset);
    for w in sorted.windows(2) {
        if w[0].offset + w[0].size > w[1].offset {
            l.issues.push(issue("bad", format!("Las particiones {} y {} se solapan.", w[0].index, w[1].index)));
        }
    }
    for p in &sorted {
        if p.offset + p.size > size {
            l.issues.push(issue("bad", format!("La partición {} termina más allá del final del disco ({} bytes de más).", p.index, p.offset + p.size - size)));
        }
    }
    l.free = gaps(&l, &sorted);
    l
}

fn gaps(l: &Layout, sorted: &[&Part]) -> Vec<Gap> {
    let mut out = Vec::new();
    let mut cursor = MIB;
    let end = l.size.saturating_sub(MIB);
    for p in sorted {
        if p.offset > cursor + 8 * MIB {
            out.push(Gap { offset: cursor, size: p.offset - cursor });
        }
        cursor = cursor.max(p.offset + p.size);
    }
    if end > cursor + 8 * MIB {
        out.push(Gap { offset: cursor, size: end - cursor });
    }
    out
}

// ---------- Buscar particiones perdidas ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Found {
    pub offset: u64,
    pub size: u64,
    pub fs: String,
    /// alta · media · baja
    pub confidence: String,
    pub detail: String,
    pub label: String,
}

/// ¿Es este sector el arranque de un sistema de archivos que cabe en el disco?
/// `start` es el desplazamiento en bytes del sector; `disk` el tamaño del disco.
pub fn probe_boot_sector<S: BlockSource>(src: &mut S, start: u64, boot: &[u8]) -> Option<Found> {
    if boot.len() < 512 || le16(boot, 510) != 0xAA55 {
        return None;
    }
    let disk = src.len();
    let bps = le16(boot, 0x0B) as u64;
    let spc = boot[0x0D] as u64;
    let valid_bps = matches!(bps, 512 | 1024 | 2048 | 4096);
    match detect_fs(boot) {
        "NTFS" => {
            if !valid_bps || !spc.is_power_of_two() || spc > 128 {
                return None;
            }
            let total = le64(boot, 0x28);
            let size = (total + 1) * bps;
            if total == 0 || start + size > disk + bps {
                return None;
            }
            let sector = src.sector() as u64;
            let mut conf = 0;
            let mut notes = Vec::new();
            // $MFT: su primer registro empieza por «FILE».
            let mft = le64(boot, 0x30) * spc * bps + start;
            if mft + 1024 <= disk && read_sectors(src, mft / sector, (1024 / sector as usize).max(1)).is_ok_and(|b| &b[0..4] == b"FILE") {
                conf += 1;
                notes.push("la tabla de archivos (MFT) se lee");
            }
            // Copia del sector de arranque en el último sector de la partición.
            let end = start + size;
            if end <= disk && end >= sector && read_sectors(src, (end - sector) / sector, 1).is_ok_and(|b| &b[3..11] == b"NTFS    ") {
                conf += 1;
                notes.push("la copia de arranque del final coincide");
            }
            Some(Found { offset: start, size, fs: "NTFS".into(), confidence: ["baja", "media", "alta"][conf].into(), detail: if notes.is_empty() { "Solo el sector de arranque es válido.".into() } else { notes.join(" · ") }, ..Default::default() })
        }
        "FAT32" => {
            if !valid_bps || !spc.is_power_of_two() || spc > 128 || boot[0x10] == 0 {
                return None;
            }
            let total = le32(boot, 0x20) as u64;
            let size = total * bps;
            if total == 0 || start + size > disk + bps {
                return None;
            }
            let reserved = le16(boot, 0x0E) as u64;
            let sector = src.sector() as u64;
            // La FAT empieza justo tras los sectores reservados.
            let fat_ok = read_sectors(src, (start + reserved * bps) / sector, 1).is_ok_and(|b| matches!(b[0], 0xF8..=0xFF) && b[1] == 0xFF && b[2] == 0xFF);
            Some(Found {
                offset: start,
                size,
                fs: "FAT32".into(),
                confidence: if fat_ok { "alta" } else { "baja" }.into(),
                detail: if fat_ok { "la tabla de asignación (FAT) se lee".into() } else { "Solo el sector de arranque es válido.".into() },
                ..Default::default()
            })
        }
        "exFAT" => {
            let shift = boot[0x6C] as u32;
            if !(9..=12).contains(&shift) {
                return None;
            }
            let (part_off, len) = (le64(boot, 0x40), le64(boot, 0x48));
            let sector = src.sector() as u64;
            // El propio sector de arranque dice en qué sector empieza la partición: si coincide, es esta.
            let matches_pos = part_off == start / sector || part_off == 0;
            if len == 0 || !matches_pos {
                return None;
            }
            let size = len << shift;
            if start + size > disk + (1 << shift) {
                return None;
            }
            Some(Found { offset: start, size, fs: "exFAT".into(), confidence: if part_off == start / sector { "alta" } else { "media" }.into(), detail: "el sector de arranque coincide con su posición".into(), ..Default::default() })
        }
        _ => None,
    }
}

/// Posiciones donde mirar: cada MB (el alineado actual) y los puntos históricos (sector 63 y múltiplos de 63 sectores en los primeros GB).
pub fn candidate_offsets(sector: u32, from: u64, to: u64) -> impl Iterator<Item = u64> {
    let step = MIB.max(sector as u64);
    let first = from.div_ceil(step) * step;
    let legacy = (0..4096u64).map(move |i| 63 * 512 * (i + 1)).filter(move |o| *o >= from && *o < to && !o.is_multiple_of(step));
    (first..to).step_by(step as usize).chain(legacy)
}

/// Mira en los huecos del disco (o en todo él, si no hay tabla) y devuelve lo que parece una partición.
/// `tick` recibe (posición, total) y devuelve false para parar.
pub fn find_lost<S: BlockSource>(src: &mut S, ranges: &[(u64, u64)], mut tick: impl FnMut(u64, u64) -> bool) -> Vec<Found> {
    let sector = src.sector() as u64;
    let total: u64 = ranges.iter().map(|(a, b)| b - a).sum();
    let mut done = 0u64;
    let mut out: Vec<Found> = Vec::new();
    for &(from, to) in ranges {
        let mut last_tick = from;
        for off in candidate_offsets(src.sector(), from, to) {
            if off - last_tick.min(off) >= 64 * MIB || off == from {
                last_tick = off;
                if !tick(done + off.saturating_sub(from), total) {
                    return out;
                }
            }
            let Ok(b) = read_sectors(src, off / sector, 1) else { continue };
            if let Some(f) = probe_boot_sector(src, off, &b) {
                // Dentro de otra ya encontrada no puede haber una nueva.
                if !out.iter().any(|o| off >= o.offset && off < o.offset + o.size) {
                    out.push(f);
                }
            }
        }
        done += to - from;
    }
    out.sort_by_key(|f| f.offset);
    out
}

// ---------- Escribir una tabla ----------

/// Cambio que se haría en el disco: sector inicial y contenido.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Write {
    pub lba: u64,
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub scheme: String,
    pub summary: String,
    pub writes: Vec<Write>,
}

fn put16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn put64(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}

/// Un identificador que no se repite (no hace falta que sea aleatorio de verdad).
pub fn unique_guid(seed: u64) -> [u8; 16] {
    let mut g = [0u8; 16];
    let mut z = seed ^ 0x9E37_79B9_7F4A_7C15;
    for chunk in g.chunks_exact_mut(8) {
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        chunk.copy_from_slice(&z.to_le_bytes());
    }
    g[7] = (g[7] & 0x0F) | 0x40; // versión 4
    g[8] = (g[8] & 0x3F) | 0x80;
    g
}

/// Tabla GPT completa (MBR protectora + cabecera + lista) y su copia del final.
pub fn build_gpt(sector: u32, disk_size: u64, disk_guid: [u8; 16], entries: &[Vec<u8>], num_entries: u32, entry_size: u32) -> Vec<Write> {
    let sec = sector as usize;
    let last_lba = disk_size / sector as u64 - 1;
    let list_bytes = (num_entries * entry_size) as usize;
    let list_sectors = list_bytes.div_ceil(sec);
    let mut list = vec![0u8; list_sectors * sec];
    for (i, e) in entries.iter().take(num_entries as usize).enumerate() {
        let n = e.len().min(entry_size as usize);
        list[i * entry_size as usize..i * entry_size as usize + n].copy_from_slice(&e[..n]);
    }
    let entries_crc = crc32(&list[..list_bytes]);
    let first_usable = 2 + list_sectors as u64;
    let last_usable = last_lba - 1 - list_sectors as u64;

    let header = |my: u64, alt: u64, entries_lba: u64| {
        let mut h = vec![0u8; sec];
        h[0..8].copy_from_slice(GPT_SIG);
        put32(&mut h, 8, 0x0001_0000);
        put32(&mut h, 12, 92);
        put64(&mut h, 24, my);
        put64(&mut h, 32, alt);
        put64(&mut h, 40, first_usable);
        put64(&mut h, 48, last_usable);
        h[56..72].copy_from_slice(&disk_guid);
        put64(&mut h, 72, entries_lba);
        put32(&mut h, 80, num_entries);
        put32(&mut h, 84, entry_size);
        put32(&mut h, 88, entries_crc);
        let crc = crc32(&h[..92]);
        put32(&mut h, 16, crc);
        h
    };

    // MBR protectora: una partición tipo EE que cubre todo el disco (hasta 2 TB).
    let mut mbr = vec![0u8; sec];
    mbr[446 + 2] = 0x02;
    mbr[446 + 4] = 0xEE;
    mbr[446 + 5..446 + 8].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
    put32(&mut mbr, 446 + 8, 1);
    put32(&mut mbr, 446 + 12, last_lba.min(0xFFFF_FFFF) as u32);
    put16(&mut mbr, 510, 0xAA55);

    let mut head = mbr;
    head.extend(header(1, last_lba, 2));
    head.extend(&list);
    let mut tail = list;
    tail.extend(header(last_lba, 1, last_lba - list_sectors as u64));
    vec![Write { lba: 0, data: head }, Write { lba: last_lba - list_sectors as u64, data: tail }]
}

/// Entrada GPT de una partición de datos.
pub fn gpt_entry(entry_size: u32, type_guid: &str, unique: [u8; 16], first: u64, last: u64, name: &str) -> Vec<u8> {
    let mut e = vec![0u8; entry_size as usize];
    e[0..16].copy_from_slice(&guid_bytes(type_guid).unwrap_or([0; 16]));
    e[16..32].copy_from_slice(&unique);
    put64(&mut e, 32, first);
    put64(&mut e, 40, last);
    for (i, u) in name.encode_utf16().take(36).enumerate() {
        put16(&mut e, 56 + i * 2, u);
    }
    e
}

/// Qué escribir para dar de alta una partición encontrada. No toca el disco.
pub fn plan_restore<S: BlockSource>(src: &mut S, layout: &Layout, found: &Found, seed: u64) -> Result<Plan, String> {
    let sector = layout.sector as u64;
    if !found.offset.is_multiple_of(sector) || !found.size.is_multiple_of(sector) {
        return Err("La partición encontrada no está alineada a sectores.".into());
    }
    let (first, count) = (found.offset / sector, found.size / sector);
    let last = first + count - 1;
    if layout.partitions.iter().any(|p| p.kind != "Extendida" && found.offset < p.offset + p.size && found.offset + found.size > p.offset) {
        return Err("La partición encontrada se solapa con una que ya existe.".into());
    }
    let summary = format!("Dar de alta una partición {} de {} MB que empieza en el sector {first}.", found.fs, found.size / MIB);

    match layout.scheme.as_str() {
        "gpt" | "none" => {
            // Se reconstruye la tabla entera con las particiones que hay más la nueva.
            let list_sectors = |num: u32, esize: u32| (num * esize).div_ceil(layout.sector) as u64;
            let (mut entries, num, esize, guid, usable) = if layout.scheme == "gpt" {
                let h = read_sectors(src, 1, 1).ok().and_then(|b| parse_gpt_header(&b)).ok_or("No se puede leer la cabecera GPT: repárala antes.")?;
                let raw = read_gpt_entries(src, &h).ok_or("No se puede leer la lista de particiones GPT.")?;
                let list: Vec<Vec<u8>> = raw.chunks_exact(h.entry_size as usize).filter(|e| e[..16].iter().any(|b| *b != 0)).map(|e| e.to_vec()).collect();
                (list, h.num_entries, h.entry_size, h.disk_guid, (h.first_usable, h.last_usable))
            } else {
                let ls = list_sectors(128, 128);
                (Vec::new(), 128, 128, unique_guid(seed ^ 0xD15C), (2 + ls, layout.size / sector - 2 - ls))
            };
            if entries.len() as u32 >= num {
                return Err("La tabla GPT está llena: no cabe otra partición.".into());
            }
            if first < usable.0 || last > usable.1 {
                return Err("La partición encontrada cae fuera de la zona que una tabla GPT puede usar.".into());
            }
            entries.push(gpt_entry(esize, GUID_DATA, unique_guid(seed), first, last, "Recuperada"));
            entries.sort_by_key(|e| le64(e, 32));
            Ok(Plan { scheme: "gpt".into(), summary, writes: build_gpt(layout.sector, layout.size, guid, &entries, num, esize) })
        }
        "mbr" => {
            if first > 0xFFFF_FFFF || count > 0xFFFF_FFFF {
                return Err("Esta partición es demasiado grande para una tabla MBR.".into());
            }
            let mut mbr = read_sectors(src, 0, 1).map_err(|e| format!("No se pudo leer el MBR (error {e})."))?;
            let slot = (0..4).find(|i| mbr[446 + i * 16 + 4] == 0).ok_or("Las cuatro entradas del MBR están ocupadas.")?;
            let o = 446 + slot * 16;
            mbr[o..o + 16].fill(0);
            mbr[o + 1..o + 4].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
            mbr[o + 4] = match found.fs.as_str() {
                "FAT32" => 0x0C,
                _ => 0x07,
            };
            mbr[o + 5..o + 8].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
            put32(&mut mbr, o + 8, first as u32);
            put32(&mut mbr, o + 12, count as u32);
            Ok(Plan { scheme: "mbr".into(), summary, writes: vec![Write { lba: 0, data: mbr }] })
        }
        _ => Err("Tabla desconocida.".into()),
    }
}

/// Sectores que se guardan antes de tocar nada: el principio, el final y los EBR.
pub fn backup_ranges(layout: &Layout) -> Vec<(u64, u64)> {
    let last = layout.size / layout.sector as u64;
    let mut v = vec![(0, 34.min(last))];
    if layout.scheme != "mbr" {
        v.push((last.saturating_sub(33), 33.min(last)));
    }
    v
}

/// Aplica un plan sobre una imagen en memoria (para probar que sale la tabla esperada).
#[cfg(test)]
pub fn apply_to(data: &mut [u8], sector: u32, plan: &Plan) {
    for w in &plan.writes {
        let o = (w.lba * sector as u64) as usize;
        data[o..o + w.data.len()].copy_from_slice(&w.data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rawdisk::MemSource;

    const SEC: u32 = 512;

    fn ntfs_boot(total_sectors: u64) -> Vec<u8> {
        let mut b = vec![0u8; 512];
        b[3..11].copy_from_slice(b"NTFS    ");
        put16(&mut b, 0x0B, 512);
        b[0x0D] = 8;
        put64(&mut b, 0x28, total_sectors);
        put64(&mut b, 0x30, 4); // MFT en el cluster 4
        put16(&mut b, 510, 0xAA55);
        b
    }

    /// Un disco de `mb` MB con una NTFS de `part_mb` en el MB 1 (tabla GPT).
    fn gpt_disk(mb: u64, part_mb: u64) -> MemSource {
        let size = mb * MIB;
        let mut data = vec![0u8; size as usize];
        let first = 2048u64;
        let last = first + part_mb * MIB / 512 - 1;
        let entries = vec![gpt_entry(128, GUID_DATA, unique_guid(1), first, last, "Datos")];
        let plan = build_gpt(SEC, size, unique_guid(99), &entries, 128, 128);
        for w in &plan {
            let o = (w.lba * 512) as usize;
            data[o..o + w.data.len()].copy_from_slice(&w.data);
        }
        plant_ntfs(&mut data, first * 512, part_mb * MIB / 512 - 1);
        MemSource { data, sector: SEC, bad: vec![] }
    }

    /// Escribe un NTFS falso: arranque, un registro MFT y la copia final.
    fn plant_ntfs(data: &mut [u8], offset: u64, total: u64) {
        let boot = ntfs_boot(total);
        let o = offset as usize;
        data[o..o + 512].copy_from_slice(&boot);
        let mft = o + 4 * 8 * 512;
        data[mft..mft + 4].copy_from_slice(b"FILE");
        let end = o + (total as usize + 1) * 512 - 512;
        data[end..end + 512].copy_from_slice(&boot);
    }

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn guid_roundtrip() {
        let b = guid_bytes(GUID_EFI).unwrap();
        assert_eq!(guid_str(&b), GUID_EFI);
        assert!(guid_bytes("nope").is_none());
    }

    #[test]
    fn reads_a_healthy_gpt_disk() {
        let mut d = gpt_disk(64, 16);
        let l = read_layout(&mut d);
        assert_eq!(l.scheme, "gpt");
        assert_eq!(l.partitions.len(), 1);
        assert_eq!(l.partitions[0].kind, "Datos");
        assert_eq!(l.partitions[0].fs, "NTFS");
        assert_eq!(l.partitions[0].size, 16 * MIB);
        assert!(l.issues.is_empty(), "{:?}", l.issues);
        assert_eq!(l.free.len(), 1);
    }

    #[test]
    fn detects_corrupt_and_missing_backup() {
        let mut d = gpt_disk(64, 16);
        // Se estropea la copia del final.
        let n = d.data.len();
        d.data[n - 512..n].fill(0);
        let l = read_layout(&mut d);
        assert!(l.issues.iter().any(|i| i.text.contains("Falta la copia de seguridad")));
        // Se estropea la lista principal.
        let mut d = gpt_disk(64, 16);
        d.data[1024 + 40] ^= 0xFF;
        let l = read_layout(&mut d);
        assert!(l.issues.iter().any(|i| i.level == "bad" && i.text.contains("lista de particiones")), "{:?}", l.issues);
    }

    #[test]
    fn empty_disk_has_no_table() {
        let mut d = MemSource { data: vec![0u8; 8 * MIB as usize], sector: SEC, bad: vec![] };
        let l = read_layout(&mut d);
        assert_eq!(l.scheme, "none");
        assert!(l.issues[0].text.contains("sin formato"));
    }

    #[test]
    fn mbr_with_overlap_is_flagged() {
        let mut data = vec![0u8; 32 * MIB as usize];
        let mut mbr = vec![0u8; 512];
        let mut entry = |slot: usize, lba: u32, n: u32| {
            let o = 446 + slot * 16;
            mbr[o] = if slot == 0 { 0x80 } else { 0 };
            mbr[o + 4] = 0x07;
            mbr[o + 8..o + 12].copy_from_slice(&lba.to_le_bytes());
            mbr[o + 12..o + 16].copy_from_slice(&n.to_le_bytes());
        };
        entry(0, 2048, 20_000);
        entry(1, 10_000, 20_000);
        mbr[510] = 0x55;
        mbr[511] = 0xAA;
        data[..512].copy_from_slice(&mbr);
        let mut d = MemSource { data, sector: SEC, bad: vec![] };
        let l = read_layout(&mut d);
        assert_eq!(l.scheme, "mbr");
        assert!(l.bootable);
        assert!(l.issues.iter().any(|i| i.text.contains("se solapan")));
    }

    #[test]
    fn finds_a_lost_partition_and_restores_it() {
        // Disco con tabla GPT vacía y un NTFS "perdido" en 20 MB.
        let size = 128 * MIB;
        let mut data = vec![0u8; size as usize];
        let plan = build_gpt(SEC, size, unique_guid(5), &[], 128, 128);
        for w in &plan {
            let o = (w.lba * 512) as usize;
            data[o..o + w.data.len()].copy_from_slice(&w.data);
        }
        let start = 20 * MIB;
        let total = 30 * MIB / 512 - 1;
        plant_ntfs(&mut data, start, total);
        let mut d = MemSource { data, sector: SEC, bad: vec![] };
        let layout = read_layout(&mut d);
        assert!(layout.partitions.is_empty());
        let ranges: Vec<(u64, u64)> = layout.free.iter().map(|g| (g.offset, g.offset + g.size)).collect();
        let found = find_lost(&mut d, &ranges, |_, _| true);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].offset, start);
        assert_eq!(found[0].fs, "NTFS");
        assert_eq!(found[0].confidence, "alta");

        let plan = plan_restore(&mut d, &layout, &found[0], 7).unwrap();
        let mut after = d.data.clone();
        apply_to(&mut after, SEC, &plan);
        let mut d2 = MemSource { data: after, sector: SEC, bad: vec![] };
        let l2 = read_layout(&mut d2);
        assert!(l2.issues.is_empty(), "{:?}", l2.issues);
        assert_eq!(l2.partitions.len(), 1);
        assert_eq!(l2.partitions[0].offset, start);
        assert_eq!(l2.partitions[0].fs, "NTFS");
    }

    #[test]
    fn restore_on_blank_disk_creates_gpt() {
        let size = 64 * MIB;
        let mut data = vec![0u8; size as usize];
        plant_ntfs(&mut data, 8 * MIB, 16 * MIB / 512 - 1);
        let mut d = MemSource { data, sector: SEC, bad: vec![] };
        let layout = read_layout(&mut d);
        assert_eq!(layout.scheme, "none");
        let found = find_lost(&mut d, &[(MIB, size - MIB)], |_, _| true);
        assert_eq!(found.len(), 1);
        let plan = plan_restore(&mut d, &layout, &found[0], 3).unwrap();
        assert_eq!(plan.scheme, "gpt");
        let mut after = d.data.clone();
        apply_to(&mut after, SEC, &plan);
        let mut d2 = MemSource { data: after, sector: SEC, bad: vec![] };
        let l2 = read_layout(&mut d2);
        assert_eq!(l2.scheme, "gpt");
        assert!(l2.issues.is_empty(), "{:?}", l2.issues);
        assert_eq!(l2.partitions[0].offset, 8 * MIB);
    }

    #[test]
    fn restore_refuses_overlap() {
        let mut d = gpt_disk(64, 16);
        let layout = read_layout(&mut d);
        let found = Found { offset: 4 * MIB, size: 8 * MIB, fs: "NTFS".into(), ..Default::default() };
        assert!(plan_restore(&mut d, &layout, &found, 1).unwrap_err().contains("solapa"));
    }

    #[test]
    fn mbr_restore_uses_a_free_slot() {
        let mut data = vec![0u8; 64 * MIB as usize];
        data[510] = 0x55;
        data[511] = 0xAA;
        // Una entrada ocupada para que sea MBR.
        data[446 + 4] = 0x07;
        data[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
        data[446 + 12..446 + 16].copy_from_slice(&2048u32.to_le_bytes());
        plant_ntfs(&mut data, 20 * MIB, 8 * MIB / 512 - 1);
        let mut d = MemSource { data, sector: SEC, bad: vec![] };
        let layout = read_layout(&mut d);
        assert_eq!(layout.scheme, "mbr");
        let found = find_lost(&mut d, &[(10 * MIB, 60 * MIB)], |_, _| true);
        let plan = plan_restore(&mut d, &layout, &found[0], 1).unwrap();
        assert_eq!(plan.scheme, "mbr");
        let mut after = d.data.clone();
        apply_to(&mut after, SEC, &plan);
        let mut d2 = MemSource { data: after, sector: SEC, bad: vec![] };
        assert_eq!(read_layout(&mut d2).partitions.len(), 2);
    }

    #[test]
    fn exfat_checks_its_own_position() {
        let mut b = vec![0u8; 512];
        b[3..11].copy_from_slice(b"EXFAT   ");
        put64(&mut b, 0x40, 2048);
        put64(&mut b, 0x48, 20_480);
        b[0x6C] = 9;
        put16(&mut b, 510, 0xAA55);
        let mut d = MemSource { data: vec![0u8; 64 * MIB as usize], sector: SEC, bad: vec![] };
        assert!(probe_boot_sector(&mut d, 2048 * 512, &b).is_some());
        assert!(probe_boot_sector(&mut d, 4096 * 512, &b).is_none());
    }

    #[test]
    fn candidates_include_legacy_start() {
        let c: Vec<u64> = candidate_offsets(512, 0, 4 * MIB).collect();
        assert!(c.contains(&MIB) && c.contains(&(63 * 512)));
    }
}
