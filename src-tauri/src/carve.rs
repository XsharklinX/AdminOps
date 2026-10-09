//! Recuperar archivos por su firma (como PhotoRec): para discos formateados, con
//! el sistema de archivos dañado, tarjetas de cámara o imágenes de disco, donde
//! ya no hay índice que diga dónde está cada archivo.
//!
//! Se recorre el disco buscando el principio de archivos conocidos (JPEG, PNG,
//! PDF, ZIP/Office…) y, en cada uno, se mide dónde acaba siguiendo su propia
//! estructura (los bloques de un PNG, los marcadores de un JPEG, el final de un ZIP).
//! Lo que acaba bien es «completo»; lo que se corta o no se puede comprobar,
//! «parcial». Un archivo que estaba troceado en el disco (fragmentado) no se puede
//! reconstruir por este método: sale parcial o no sale.
//!
//! Solo lee. Los archivos se copian a otro disco después (`carveio.rs`).

use crate::rawdisk::{Aligned, BlockSource};
use serde::{Deserialize, Serialize};

const MIB: u64 = 1024 * 1024;
/// Se lee de 4 MB en 4 MB.
const WIN: usize = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub id: u32,
    pub ext: String,
    /// photos · documents · videos · music · archives · databases
    pub group: String,
    /// «Foto JPEG», «Documento de Word»…
    pub label: String,
    /// Desde el principio de lo que se escaneó.
    pub offset: u64,
    pub length: u64,
    /// full · partial
    pub quality: String,
}

/// Lectura con ventana: pide al disco trozos alineados y deja leer sin preocuparse de los bordes.
pub struct Cur<'a, S: BlockSource> {
    src: &'a mut S,
    base: u64,
    buf: Vec<u8>,
    limit: u64,
}

impl<'a, S: BlockSource> Cur<'a, S> {
    pub fn new(src: &'a mut S, limit: u64) -> Self {
        Cur { src, base: u64::MAX, buf: Vec::new(), limit }
    }

    /// Lo que hay desde `abs` hasta el final de la ventana (vacío si se acabó el disco o no se pudo leer).
    pub fn tail(&mut self, abs: u64) -> &[u8] {
        if abs >= self.limit {
            return &[];
        }
        let inside = self.base != u64::MAX && abs >= self.base && abs < self.base + self.buf.len() as u64;
        if !inside {
            let sector = self.src.sector().max(512) as u64;
            let start = abs / sector * sector;
            let want = ((self.limit - start).min(WIN as u64) / sector * sector).max(sector);
            let mut a = Aligned::new(want as usize);
            match self.src.read_at(start, a.slice(want as usize)) {
                Ok(n) => {
                    self.buf = a.slice(want as usize)[..n].to_vec();
                    self.base = start;
                }
                Err(_) => {
                    self.buf.clear();
                    self.base = u64::MAX;
                    return &[];
                }
            }
            if self.buf.is_empty() {
                return &[];
            }
        }
        let i = (abs - self.base) as usize;
        if i >= self.buf.len() {
            return &[];
        }
        let end = ((self.limit - self.base).min(self.buf.len() as u64)) as usize;
        &self.buf[i..end.max(i)]
    }

    pub fn byte(&mut self, abs: u64) -> Option<u8> {
        self.tail(abs).first().copied()
    }

    pub fn bytes(&mut self, abs: u64, n: usize) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let t = self.tail(abs + out.len() as u64);
            if t.is_empty() {
                return None;
            }
            let take = t.len().min(n - out.len());
            out.extend_from_slice(&t[..take]);
        }
        Some(out)
    }

    fn be16(&mut self, abs: u64) -> Option<u32> {
        let b = self.bytes(abs, 2)?;
        Some(u16::from_be_bytes([b[0], b[1]]) as u32)
    }
    fn be32(&mut self, abs: u64) -> Option<u32> {
        let b = self.bytes(abs, 4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn le32(&mut self, abs: u64) -> Option<u32> {
        let b = self.bytes(abs, 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn le64(&mut self, abs: u64) -> Option<u64> {
        let b = self.bytes(abs, 8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(&b);
        Some(u64::from_le_bytes(a))
    }
}

const FULL: &str = "full";
const PARTIAL: &str = "partial";

type Measured = Option<(u64, &'static str)>;

fn jpeg<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let mut pos = off + 2;
    loop {
        if pos - off > max {
            return Some((max, PARTIAL));
        }
        if c.byte(pos) != Some(0xFF) {
            // Se rompió la estructura: lo que hay hasta aquí se puede abrir en parte.
            return (pos - off > 4096).then_some((pos - off, PARTIAL));
        }
        let mut m = c.byte(pos + 1)?;
        while m == 0xFF {
            pos += 1;
            m = c.byte(pos + 1)?;
        }
        match m {
            0xD9 => return Some((pos + 2 - off, FULL)),
            0x01 | 0xD0..=0xD7 => pos += 2,
            0xD8 | 0x00 => return (pos - off > 4096).then_some((pos - off, PARTIAL)),
            0xDA => {
                let len = c.be16(pos + 2)? as u64;
                pos += 2 + len;
                // Datos comprimidos: hasta el siguiente marcador que no sea relleno (FF 00) ni reinicio (FF D0..D7).
                loop {
                    let t = c.tail(pos);
                    if t.is_empty() {
                        return Some((pos - off, PARTIAL));
                    }
                    let mut found = None;
                    let mut i = 0;
                    while i + 1 < t.len() {
                        if t[i] == 0xFF && t[i + 1] != 0x00 && !(0xD0..=0xD7).contains(&t[i + 1]) {
                            found = Some(i);
                            break;
                        }
                        i += 1;
                    }
                    match found {
                        Some(i) => {
                            pos += i as u64;
                            break;
                        }
                        None => pos += (t.len() - 1).max(1) as u64,
                    }
                    if pos - off > max {
                        return Some((max, PARTIAL));
                    }
                }
            }
            _ => {
                let len = c.be16(pos + 2)? as u64;
                if len < 2 {
                    return None;
                }
                pos += 2 + len;
            }
        }
    }
}

fn png<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let mut pos = off + 8;
    let mut first = true;
    loop {
        let (Some(len), Some(kind)) = (c.be32(pos), c.bytes(pos + 4, 4)) else {
            return (pos - off > 1000).then_some((pos - off, PARTIAL));
        };
        if !kind.iter().all(|b| b.is_ascii_alphabetic()) || len as u64 > max {
            return (pos - off > 1000).then_some((pos - off, PARTIAL));
        }
        if first {
            // El primer bloque tiene que ser IHDR, de 13 bytes y con su suma de control bien.
            if &kind != b"IHDR" || len != 13 {
                return None;
            }
            let body = c.bytes(pos + 4, 4 + 13)?;
            if c.be32(pos + 8 + 13)? != crate::partitions::crc32(&body) {
                return None;
            }
            first = false;
        }
        pos += 12 + len as u64;
        if &kind == b"IEND" {
            return Some((pos - off, FULL));
        }
        if pos - off > max {
            return Some((max, PARTIAL));
        }
    }
}

fn skip_sub_blocks<S: BlockSource>(c: &mut Cur<S>, mut pos: u64) -> Option<u64> {
    loop {
        let n = c.byte(pos)? as u64;
        pos += 1 + n;
        if n == 0 {
            return Some(pos);
        }
    }
}

fn gif<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let flags = c.byte(off + 10)?;
    let mut pos = off + 13 + if flags & 0x80 != 0 { 3 << ((flags & 7) + 1) } else { 0 };
    loop {
        if pos - off > max {
            return Some((max, PARTIAL));
        }
        match c.byte(pos) {
            Some(0x3B) => return Some((pos + 1 - off, FULL)),
            Some(0x21) => match skip_sub_blocks(c, pos + 2) {
                Some(p) => pos = p,
                None => return (pos - off > 100).then_some((pos - off, PARTIAL)),
            },
            Some(0x2C) => {
                let f = c.byte(pos + 9)?;
                pos += 10 + if f & 0x80 != 0 { 3 << ((f & 7) + 1) } else { 0 } + 1;
                match skip_sub_blocks(c, pos) {
                    Some(p) => pos = p,
                    None => return (pos - off > 100).then_some((pos - off, PARTIAL)),
                }
            }
            _ => return (pos - off > 100).then_some((pos - off, PARTIAL)),
        }
    }
}

/// Posición del primer `needle` desde `from` (hasta `to`), o None.
fn find<S: BlockSource>(c: &mut Cur<S>, from: u64, to: u64, needle: &[u8]) -> Option<u64> {
    let mut pos = from;
    while pos < to {
        let t = c.tail(pos);
        if t.len() < needle.len() {
            return None;
        }
        let limit = ((to - pos) as usize).min(t.len());
        if let Some(i) = t[..limit].windows(needle.len()).position(|w| w == needle) {
            return Some(pos + i as u64);
        }
        pos += (limit.saturating_sub(needle.len() - 1)).max(1) as u64;
    }
    None
}

fn pdf<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    // Los PDF «linealizados» dicen su tamaño exacto en /L.
    if let Some(head) = c.bytes(off, 1024.min(max as usize)) {
        if let Some(p) = head.windows(11).position(|w| w == b"/Linearized") {
            if let Some(l) = head[p..].windows(3).position(|w| w == b"/L ") {
                let digits: String = head[p + l + 3..].iter().take_while(|b| b.is_ascii_digit()).map(|b| *b as char).collect();
                if let Ok(n) = digits.parse::<u64>() {
                    if n > 1024 && n <= max {
                        return Some((n, FULL));
                    }
                }
            }
        }
    }
    let mut from = off + 5;
    let mut last_end = None;
    for _ in 0..16 {
        let Some(eof) = find(c, from, off + max, b"%%EOF") else { break };
        let mut end = eof + 5;
        // Fin de línea tras el %%EOF.
        for _ in 0..2 {
            if matches!(c.byte(end), Some(b'\r' | b'\n')) {
                end += 1;
            }
        }
        last_end = Some(end);
        // Una actualización incremental continúa con «N N obj».
        let next = c.bytes(end, 24).unwrap_or_default();
        let starts_obj = {
            let s: String = next.iter().take(24).map(|b| *b as char).collect();
            let mut parts = s.split_whitespace();
            matches!((parts.next(), parts.next(), parts.next()), (Some(a), Some(b), Some(o)) if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) && o.starts_with("obj"))
        };
        if !starts_obj {
            break;
        }
        from = end;
    }
    match last_end {
        Some(e) => Some((e - off, FULL)),
        None => None,
    }
}

fn zip<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let mut from = off + 30;
    loop {
        let e = find(c, from, off + max, b"PK\x05\x06")?;
        let rec = c.bytes(e, 22)?;
        let cd_size = u32::from_le_bytes([rec[12], rec[13], rec[14], rec[15]]) as u64;
        let cd_off = u32::from_le_bytes([rec[16], rec[17], rec[18], rec[19]]) as u64;
        let comment = u16::from_le_bytes([rec[20], rec[21]]) as u64;
        // El directorio central acaba justo donde empieza este final de archivo.
        if cd_off + cd_size == e - off {
            return Some((e + 22 + comment - off, FULL));
        }
        from = e + 4;
    }
}

fn sevenzip<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let next_off = c.le64(off + 12)?;
    let next_size = c.le64(off + 20)?;
    let total = 32u64.checked_add(next_off)?.checked_add(next_size)?;
    (total > 32 && total <= max).then_some((total, FULL))
}

fn riff<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let n = c.le32(off + 4)? as u64 + 8;
    (n > 44 && n <= max).then_some((n + (n & 1), FULL))
}

fn bmp<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let h = c.bytes(off, 30)?;
    let size = u32::from_le_bytes([h[2], h[3], h[4], h[5]]) as u64;
    let dib = u32::from_le_bytes([h[14], h[15], h[16], h[17]]);
    let pixels_at = u32::from_le_bytes([h[10], h[11], h[12], h[13]]) as u64;
    let ok = h[6..10] == [0, 0, 0, 0] && matches!(dib, 12 | 40 | 52 | 56 | 64 | 108 | 124) && size >= 54 && size <= max && pixels_at >= 14 + dib as u64 && pixels_at < size;
    ok.then_some((size, FULL))
}

fn sqlite<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let h = c.bytes(off, 32)?;
    let page = match u16::from_be_bytes([h[16], h[17]]) {
        1 => 65536u64,
        p => p as u64,
    };
    let pages = u32::from_be_bytes([h[28], h[29], h[30], h[31]]) as u64;
    let ok = page.is_power_of_two() && page >= 512 && pages > 0 && page * pages <= max;
    ok.then_some((page * pages, FULL))
}

fn mp4<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    const KNOWN: &[&[u8; 4]] = &[b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide", b"uuid", b"pnot", b"meta", b"moof", b"mfra", b"styp", b"sidx", b"junk"];
    let mut pos = off;
    let (mut boxes, mut moov, mut mdat) = (0, false, false);
    while let (Some(size32), Some(kind)) = (c.be32(pos), c.bytes(pos + 4, 4)) {
        if !KNOWN.iter().any(|k| &k[..] == kind.as_slice()) {
            break;
        }
        let size = match size32 {
            0 => break, // «hasta el final del archivo»: no se puede medir
            1 => match c.bytes(pos + 8, 8) {
                Some(b) => u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
                None => break,
            },
            n => n as u64,
        };
        if size < 8 || pos - off + size > max {
            break;
        }
        moov |= kind == b"moov";
        mdat |= kind == b"mdat";
        boxes += 1;
        pos += size;
    }
    (boxes >= 2).then_some((pos - off, if moov && mdat { FULL } else { PARTIAL }))
}

/// Documento antiguo de Office (OLE): el final lo marca el último sector que usa la tabla FAT.
fn ole<S: BlockSource>(c: &mut Cur<S>, off: u64, max: u64) -> Measured {
    let h = c.bytes(off, 512)?;
    let shift = u16::from_le_bytes([h[0x1E], h[0x1F]]) as u32;
    if !(9..=12).contains(&shift) {
        return None;
    }
    let ssize = 1u64 << shift;
    let n_fat = u32::from_le_bytes([h[0x2C], h[0x2D], h[0x2E], h[0x2F]]) as u64;
    if n_fat == 0 || n_fat > 109 {
        return None; // más de 109 hojas de FAT exige la cadena DIFAT: no se mide
    }
    let mut last = 0u64;
    for i in 0..n_fat as usize {
        let sect = u32::from_le_bytes([h[0x4C + i * 4], h[0x4D + i * 4], h[0x4E + i * 4], h[0x4F + i * 4]]) as u64;
        let fat = c.bytes(off + (sect + 1) * ssize, ssize as usize)?;
        for (j, e) in fat.chunks_exact(4).enumerate() {
            if u32::from_le_bytes([e[0], e[1], e[2], e[3]]) != 0xFFFF_FFFF {
                last = last.max(i as u64 * (ssize / 4) + j as u64);
            }
        }
    }
    let total = (last + 2) * ssize;
    (total <= max).then_some((total, FULL))
}

// ---------- Firmas ----------

#[derive(Clone, Copy)]
enum Kind {
    Jpeg,
    Png,
    Gif,
    Bmp,
    Pdf,
    Zip,
    SevenZip,
    Riff,
    Sqlite,
    Mp4,
    Ole,
}

struct Sig {
    kind: Kind,
    magic: &'static [u8],
    at: usize,
    max: u64,
    min: u64,
}

const SIGS: &[Sig] = &[
    Sig { kind: Kind::Jpeg, magic: &[0xFF, 0xD8, 0xFF], at: 0, max: 80 * MIB, min: 4096 },
    Sig { kind: Kind::Png, magic: &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], at: 0, max: 80 * MIB, min: 300 },
    Sig { kind: Kind::Gif, magic: b"GIF8", at: 0, max: 40 * MIB, min: 100 },
    Sig { kind: Kind::Bmp, magic: b"BM", at: 0, max: 100 * MIB, min: 1000 },
    Sig { kind: Kind::Pdf, magic: b"%PDF-", at: 0, max: 400 * MIB, min: 1024 },
    Sig { kind: Kind::Zip, magic: b"PK\x03\x04", at: 0, max: 800 * MIB, min: 100 },
    Sig { kind: Kind::SevenZip, magic: &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], at: 0, max: 2048 * MIB, min: 64 },
    Sig { kind: Kind::Riff, magic: b"RIFF", at: 0, max: 4096 * MIB, min: 1000 },
    Sig { kind: Kind::Sqlite, magic: b"SQLite format 3\0", at: 0, max: 4096 * MIB, min: 512 },
    Sig { kind: Kind::Mp4, magic: b"ftyp", at: 4, max: 8192 * MIB, min: 1000 },
    Sig { kind: Kind::Ole, magic: &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], at: 0, max: 200 * MIB, min: 2048 },
];

/// Extensión, grupo y nombre en español.
fn describe<S: BlockSource>(c: &mut Cur<S>, kind: Kind, off: u64) -> (&'static str, &'static str, &'static str) {
    match kind {
        Kind::Jpeg => ("jpg", "photos", "Foto JPEG"),
        Kind::Png => ("png", "photos", "Imagen PNG"),
        Kind::Gif => ("gif", "photos", "Imagen GIF"),
        Kind::Bmp => ("bmp", "photos", "Imagen BMP"),
        Kind::Pdf => ("pdf", "documents", "Documento PDF"),
        Kind::Zip => {
            let head = c.bytes(off, 2048).unwrap_or_default();
            let has = |s: &[u8]| head.windows(s.len()).any(|w| w == s);
            if has(b"word/") {
                ("docx", "documents", "Documento de Word")
            } else if has(b"xl/") {
                ("xlsx", "documents", "Hoja de Excel")
            } else if has(b"ppt/") {
                ("pptx", "documents", "Presentación de PowerPoint")
            } else if has(b"mimetype") {
                ("odt", "documents", "Documento de OpenDocument")
            } else {
                ("zip", "archives", "Archivo ZIP")
            }
        }
        Kind::SevenZip => ("7z", "archives", "Archivo 7-Zip"),
        Kind::Riff => match c.bytes(off + 8, 4).as_deref() {
            Some(b"AVI ") => ("avi", "videos", "Vídeo AVI"),
            Some(b"WAVE") => ("wav", "music", "Audio WAV"),
            Some(b"WEBP") => ("webp", "photos", "Imagen WebP"),
            _ => ("riff", "archives", "Archivo RIFF"),
        },
        Kind::Sqlite => ("sqlite", "databases", "Base de datos SQLite"),
        Kind::Mp4 => match c.bytes(off + 8, 4).as_deref() {
            Some(b"heic" | b"heix" | b"mif1" | b"hevc") => ("heic", "photos", "Foto HEIC"),
            Some(b"qt  ") => ("mov", "videos", "Vídeo QuickTime"),
            Some(b"M4A " | b"M4B ") => ("m4a", "music", "Audio M4A"),
            _ => ("mp4", "videos", "Vídeo MP4"),
        },
        Kind::Ole => ("doc", "documents", "Documento antiguo de Office (Word, Excel…)"),
    }
}

fn measure<S: BlockSource>(c: &mut Cur<S>, s: &Sig, off: u64, max: u64) -> Measured {
    match s.kind {
        Kind::Jpeg => jpeg(c, off, max),
        Kind::Png => png(c, off, max),
        Kind::Gif => gif(c, off, max),
        Kind::Bmp => bmp(c, off, max),
        Kind::Pdf => pdf(c, off, max),
        Kind::Zip => zip(c, off, max),
        Kind::SevenZip => sevenzip(c, off, max),
        Kind::Riff => riff(c, off, max),
        Kind::Sqlite => sqlite(c, off, max),
        Kind::Mp4 => mp4(c, off, max),
        Kind::Ole => ole(c, off, max),
    }
}

/// Recorre `[from, to)` buscando archivos de los `groups` pedidos.
/// `tick` recibe (posición, total, archivos hasta ahora) y devuelve false para parar.
pub fn carve<S: BlockSource>(src: &mut S, from: u64, to: u64, groups: &[String], mut tick: impl FnMut(u64, u64, &[Item]) -> bool) -> Vec<Item> {
    let sector = src.sector().max(512) as u64;
    let to = to.min(src.len());
    let from = from.div_ceil(sector) * sector;
    let mut c = Cur::new(src, to);
    let mut items: Vec<Item> = Vec::new();
    let mut pos = from;
    while pos < to {
        if !tick(pos - from, to - from, &items) {
            break;
        }
        let window = c.tail(pos);
        if window.is_empty() {
            // Un trozo ilegible: se salta al siguiente MB.
            pos = (pos / MIB + 1) * MIB;
            continue;
        }
        let wlen = window.len();
        // Candidatos: sectores cuyo principio coincide con alguna firma.
        let mut cands: Vec<(u64, usize)> = Vec::new();
        let mut i = 0;
        while i + 32 <= wlen || (i < wlen && pos + wlen as u64 >= to) {
            for (si, s) in SIGS.iter().enumerate() {
                let end = s.at + s.magic.len();
                if i + end <= wlen && &window[i + s.at..i + end] == s.magic {
                    cands.push((pos + i as u64, si));
                    break;
                }
            }
            i += sector as usize;
        }
        let consumed = i.min(wlen) as u64;
        let mut skip_to = pos;
        for (off, si) in cands {
            if off < skip_to {
                continue;
            }
            let s = &SIGS[si];
            let max = s.max.min(to - off);
            if let Some((len, quality)) = measure(&mut c, s, off, max) {
                if len >= s.min {
                    let (ext, group, label) = describe(&mut c, s.kind, off);
                    if groups.is_empty() || groups.iter().any(|g| g == group) {
                        items.push(Item { id: items.len() as u32 + 1, ext: ext.into(), group: group.into(), label: label.into(), offset: off - from, length: len, quality: quality.into() });
                    }
                    skip_to = off + len.div_ceil(sector) * sector;
                }
            }
        }
        pos = skip_to.max(pos + consumed.max(sector));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partitions::crc32;
    use crate::rawdisk::MemSource;

    fn jpeg_file(body: usize) -> Vec<u8> {
        let mut f = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        f.extend(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        f.extend([0xFF, 0xDB, 0x00, 0x05, 1, 2, 3]); // tabla
        f.extend([0xFF, 0xDA, 0x00, 0x04, 0x01, 0x00]); // SOS
        for i in 0..body {
            let b = (i * 7 % 250) as u8;
            f.push(b);
            if b == 0xFF {
                f.push(0);
            }
        }
        f.extend([0xFF, 0x00, 0xFF, 0xD9]);
        f
    }

    fn png_file(extra: usize) -> Vec<u8> {
        let mut f = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let chunk = |kind: &[u8], data: &[u8]| {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            let mut body = kind.to_vec();
            body.extend(data);
            c.extend(&body);
            c.extend(crc32(&body).to_be_bytes());
            c
        };
        let mut ihdr = vec![0u8; 13];
        ihdr[3] = 8;
        ihdr[7] = 8;
        f.extend(chunk(b"IHDR", &ihdr));
        f.extend(chunk(b"IDAT", &vec![9u8; extra]));
        f.extend(chunk(b"IEND", &[]));
        f
    }

    fn zip_file() -> Vec<u8> {
        let mut f = b"PK\x03\x04".to_vec();
        f.extend(vec![0u8; 26]);
        f.extend(b"word/document.xml");
        f.extend(vec![5u8; 600]);
        let cd_off = f.len() as u32;
        f.extend(b"PK\x01\x02");
        f.extend(vec![0u8; 42]);
        let cd_size = f.len() as u32 - cd_off;
        f.extend(b"PK\x05\x06");
        f.extend([0u8; 8]);
        f.extend(cd_size.to_le_bytes());
        f.extend(cd_off.to_le_bytes());
        f.extend([0u8, 0]);
        f
    }

    fn place(data: &mut [u8], at: usize, f: &[u8]) {
        data[at..at + f.len()].copy_from_slice(f);
    }

    fn noise(n: usize) -> Vec<u8> {
        // Ruido determinista que no forma firmas.
        (0..n).map(|i| ((i * 31 + 7) % 200 + 1) as u8).collect()
    }

    #[test]
    fn finds_jpeg_png_zip_and_measures_them() {
        let mut data = noise(2 * MIB as usize);
        let j = jpeg_file(20_000);
        let p = png_file(2_000);
        let z = zip_file();
        place(&mut data, 4096, &j);
        place(&mut data, 100 * 512, &p);
        place(&mut data, 300 * 512, &z);
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        let items = carve(&mut d, 0, 2 * MIB, &[], |_, _, _| true);
        let by = |ext: &str| items.iter().find(|i| i.ext == ext).unwrap_or_else(|| panic!("falta {ext}: {items:?}"));
        assert_eq!(by("jpg").length, j.len() as u64);
        assert_eq!(by("jpg").offset, 4096);
        assert_eq!(by("jpg").quality, "full");
        assert_eq!(by("png").length, p.len() as u64);
        assert_eq!(by("docx").length, z.len() as u64);
        assert_eq!(by("docx").group, "documents");
        assert_eq!(items.len(), 3, "{items:?}");
    }

    #[test]
    fn truncated_jpeg_is_partial() {
        let mut data = noise(MIB as usize);
        let mut j = jpeg_file(20_000);
        j.truncate(j.len() - 4); // sin el FF D9
        place(&mut data, 8192, &j);
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        let items = carve(&mut d, 0, MIB, &[], |_, _, _| true);
        // Sigue habiendo ruido sin marcador: se corta donde la estructura se rompe.
        let jpg = items.iter().find(|i| i.ext == "jpg").expect("jpg");
        assert_eq!(jpg.quality, "partial");
    }

    #[test]
    fn filter_by_group() {
        let mut data = noise(MIB as usize);
        place(&mut data, 4096, &jpeg_file(10_000));
        place(&mut data, 100 * 512, &zip_file());
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        let items = carve(&mut d, 0, MIB, &["documents".to_string()], |_, _, _| true);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].ext, "docx");
    }

    #[test]
    fn nested_signature_inside_a_file_is_skipped() {
        let mut data = noise(MIB as usize);
        let mut inner = jpeg_file(6_000);
        // Una miniatura JPEG dentro del cuerpo de un PNG grande.
        let mut p = png_file(0);
        let idat_at = p.len() - 12; // antes del IEND
        let mut big = p[..idat_at].to_vec();
        let mut body = b"IDAT".to_vec();
        body.extend(vec![1u8; 512]);
        body.extend(&inner);
        body.resize(body.len().div_ceil(512) * 512 + 4, 1);
        let mut chunk = ((body.len() - 4) as u32).to_be_bytes().to_vec();
        chunk.extend(&body);
        chunk.extend(crc32(&body).to_be_bytes());
        big.extend(chunk);
        big.extend(&p[idat_at..]);
        inner.clear();
        p.clear();
        place(&mut data, 8192, &big);
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        let items = carve(&mut d, 0, MIB, &[], |_, _, _| true);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].ext, "png");
    }

    #[test]
    fn pdf_with_incremental_update_and_linearized() {
        let mut pdf = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n".to_vec();
        pdf.extend(vec![b'x'; 2000]);
        pdf.extend(b"\ntrailer\n%%EOF\n");
        pdf.extend(b"5 0 obj\n<<>>\nendobj\ntrailer\n%%EOF\n");
        let len = pdf.len();
        let mut data = noise(MIB as usize);
        place(&mut data, 2048, &pdf);
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        let items = carve(&mut d, 0, MIB, &[], |_, _, _| true);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].length, len as u64);
        // Linealizado: dice /L.
        let mut lin = b"%PDF-1.5\n1 0 obj\n<</Linearized 1/L 4000/O 3>>\nendobj\n%%EOF\n".to_vec();
        lin.resize(4000, b'y');
        let mut data = noise(MIB as usize);
        place(&mut data, 2048, &lin);
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        let items = carve(&mut d, 0, MIB, &[], |_, _, _| true);
        assert_eq!(items[0].length, 4000);
    }

    #[test]
    fn mp4_box_walk() {
        let boxed = |kind: &[u8; 4], n: usize| {
            let mut b = (n as u32).to_be_bytes().to_vec();
            b.extend(kind);
            b.extend(vec![0u8; n - 8]);
            b
        };
        let mut f = boxed(b"ftyp", 24);
        f[8..12].copy_from_slice(b"isom");
        f.extend(boxed(b"mdat", 5000));
        f.extend(boxed(b"moov", 1200));
        let mut data = noise(MIB as usize);
        place(&mut data, 4096, &f);
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        let items = carve(&mut d, 0, MIB, &[], |_, _, _| true);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].ext, "mp4");
        assert_eq!(items[0].length, f.len() as u64);
        assert_eq!(items[0].quality, "full");
    }

    #[test]
    fn stops_when_asked_and_reports_progress() {
        let mut d = MemSource { data: noise(8 * MIB as usize), sector: 512, bad: vec![] };
        let mut calls = 0;
        carve(&mut d, 0, 8 * MIB, &[], |pos, total, _| {
            calls += 1;
            assert!(pos <= total);
            calls < 2
        });
        assert_eq!(calls, 2);
    }

    #[test]
    fn unreadable_regions_do_not_stop_the_scan() {
        let mut data = noise(8 * MIB as usize);
        place(&mut data, 6 * MIB as usize + 4096, &jpeg_file(9_000));
        let mut d = MemSource { data, sector: 512, bad: vec![(MIB, 3 * MIB)] };
        let items = carve(&mut d, 0, 8 * MIB, &[], |_, _, _| true);
        assert!(items.iter().any(|i| i.ext == "jpg"), "{items:?}");
    }

    #[test]
    fn false_bm_header_is_rejected() {
        let mut data = noise(MIB as usize);
        place(&mut data, 4096, b"BM");
        let mut d = MemSource { data, sector: 512, bad: vec![] };
        assert!(carve(&mut d, 0, MIB, &[], |_, _, _| true).is_empty());
    }
}
