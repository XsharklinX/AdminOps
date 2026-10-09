//! Clonar o hacer una imagen de un disco que falla, a la manera de ddrescue.
//!
//! Con un disco enfermo, leer sin cuidado lo empeora: cada error hace que el
//! disco insista durante segundos y arrastra al resto. Por eso se copia en tres
//! pasadas:
//!
//! 1. **Rápida**: se copia de corrido; al primer error se salta una zona (cada vez
//!    más grande) y se sigue. Recupera lo bueno cuanto antes.
//! 2. **Zonas saltadas**: se vuelve a las zonas que se saltaron, en bloques de 64 KB.
//! 3. **Reintentos**: los bloques que siguen dando error se leen sector a sector,
//!    con varios intentos, y solo se dan por perdidos los sectores que no hay manera.
//!
//! Lo ilegible queda a ceros en el destino y apuntado. El avance se guarda: se
//! puede parar y seguir otro día. El algoritmo trabaja sobre `BlockSource` y
//! `BlockSink`, así que se prueba con discos en memoria.

use crate::rawdisk::{Aligned, BlockSource};
use serde::{Deserialize, Serialize};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
/// Granularidad del mapa.
pub const BLOCK: u64 = 64 * KIB;
const FAST_CHUNK: u64 = MIB;
const MAX_SKIP: u64 = 64 * MIB;

/// Estados de cada bloque.
pub const TODO: u8 = b'?';
pub const GOOD: u8 = b'+';
pub const SKIPPED: u8 = b's';
/// Dio error en 64 KB: pendiente de partir en sectores.
pub const BAD_BLOCK: u8 = b'b';
/// Algún sector se perdió, el resto se salvó.
pub const PARTIAL: u8 = b'p';
/// Todo el bloque se perdió.
pub const LOST: u8 = b'x';

pub trait BlockSink {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u32>;
}

impl BlockSink for Box<dyn BlockSink + Send> {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u32> {
        (**self).write_at(offset, data)
    }
}

/// Un archivo de imagen.
pub struct FileSink {
    file: std::fs::File,
}

impl FileSink {
    pub fn create(path: &std::path::Path, size: u64) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)?;
        if file.metadata()?.len() < size {
            file.set_len(size)?;
        }
        Ok(FileSink { file })
    }
}

impl BlockSink for FileSink {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u32> {
        use std::io::{Seek, SeekFrom, Write};
        self.file.seek(SeekFrom::Start(offset)).map_err(|e| e.raw_os_error().unwrap_or(0) as u32)?;
        self.file.write_all(data).map_err(|e| e.raw_os_error().unwrap_or(0) as u32)
    }
}

#[cfg(test)]
pub struct MemSink(pub Vec<u8>);

#[cfg(test)]
impl BlockSink for MemSink {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u32> {
        self.0[offset as usize..offset as usize + data.len()].copy_from_slice(data);
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CloneState {
    pub size: u64,
    pub sector: u32,
    /// Un carácter por bloque de 64 KB.
    pub states: String,
    /// Sectores que no hubo manera de leer: (desplazamiento, longitud).
    pub lost: Vec<(u64, u64)>,
    /// 1 rápida · 2 zonas saltadas · 3 reintentos · 4 terminado
    pub pass: u8,
    /// Dónde va la pasada actual (para seguir).
    pub cursor: u64,
}

impl CloneState {
    pub fn new(size: u64, sector: u32) -> Self {
        let blocks = size.div_ceil(BLOCK) as usize;
        CloneState { size, sector: sector.max(512), states: "?".repeat(blocks), lost: Vec::new(), pass: 1, cursor: 0 }
    }

    fn set(&mut self, first: usize, last: usize, s: u8) {
        let mut v = std::mem::take(&mut self.states).into_bytes();
        if let Some(max) = v.len().checked_sub(1) {
            for b in &mut v[first.min(max)..=last.min(max)] {
                *b = s;
            }
        }
        self.states = String::from_utf8(v).unwrap_or_default();
    }

    fn get(&self, i: usize) -> u8 {
        self.states.as_bytes().get(i).copied().unwrap_or(GOOD)
    }

    /// (bien, perdido, pendiente) en bytes.
    pub fn totals(&self) -> (u64, u64, u64) {
        let lost: u64 = self.lost.iter().map(|(_, l)| *l).sum();
        let pending: u64 = self
            .states
            .bytes()
            .enumerate()
            .filter(|(_, s)| matches!(*s, TODO | SKIPPED | BAD_BLOCK))
            .map(|(i, _)| (self.size - i as u64 * BLOCK).min(BLOCK))
            .sum();
        (self.size.saturating_sub(lost + pending), lost, pending)
    }

    /// El mapa en `n` casillas: . bien · ? pendiente · x con pérdida.
    pub fn cells(&self, n: usize) -> String {
        let blocks = self.states.len();
        if blocks == 0 {
            return String::new();
        }
        let n = n.min(blocks);
        let s = self.states.as_bytes();
        (0..n)
            .map(|c| {
                let (a, b) = (c * blocks / n, ((c + 1) * blocks / n).max(c * blocks / n + 1).min(blocks));
                let slice = &s[a..b];
                if slice.iter().any(|x| matches!(*x, LOST | PARTIAL | BAD_BLOCK)) {
                    'x'
                } else if slice.iter().any(|x| matches!(*x, TODO | SKIPPED)) {
                    '?'
                } else {
                    '.'
                }
            })
            .collect()
    }
}

fn merge_lost(lost: &mut Vec<(u64, u64)>, off: u64, len: u64) {
    if let Some(last) = lost.last_mut() {
        if last.0 + last.1 == off {
            last.1 += len;
            return;
        }
    }
    lost.push((off, len));
}

/// Copia lo que se pueda. `retries`: intentos por sector en la última pasada.
/// `tick` recibe el estado y devuelve false para parar. True si llegó al final.
pub fn clone<S: BlockSource, K: BlockSink>(src: &mut S, dst: &mut K, st: &mut CloneState, retries: u32, mut tick: impl FnMut(&CloneState) -> bool) -> bool {
    let size = st.size.min(src.len());
    let sector = st.sector as u64;
    let mut buf = Aligned::new(FAST_CHUNK as usize);
    let blocks = st.states.len();

    // ---- Pasada 1: de corrido, saltando al primer error ----
    if st.pass == 1 {
        let mut skip = MIB;
        let mut pos = st.cursor / BLOCK * BLOCK;
        while pos < size {
            let b = (pos / BLOCK) as usize;
            if st.get(b) != TODO {
                pos += BLOCK;
                continue;
            }
            let len = FAST_CHUNK.min(size - pos);
            let len = len.div_ceil(sector) * sector;
            let len = len.min(src.len().saturating_sub(pos));
            match src.read_at(pos, buf.slice(len as usize)) {
                Ok(n) if n as u64 == len => {
                    if dst.write_at(pos, buf.slice(len as usize)).is_err() {
                        st.cursor = pos;
                        return false;
                    }
                    let last = ((pos + len - 1) / BLOCK) as usize;
                    st.set(b, last, GOOD);
                    pos += len;
                    skip = MIB;
                }
                _ => {
                    // Se salta una zona cada vez mayor; se volverá a ella en la pasada 2.
                    let jump = skip.clamp(FAST_CHUNK, MAX_SKIP);
                    let end = (pos + jump).min(size);
                    let last = ((end - 1) / BLOCK) as usize;
                    st.set(b, last.max(b), SKIPPED);
                    pos = end.div_ceil(BLOCK) * BLOCK;
                    skip = (skip * 2).min(MAX_SKIP);
                }
            }
            st.cursor = pos;
            if !tick(st) {
                return false;
            }
        }
        st.pass = 2;
        st.cursor = 0;
    }

    // ---- Pasada 2: las zonas saltadas, en bloques de 64 KB ----
    if st.pass == 2 {
        let mut b = (st.cursor / BLOCK) as usize;
        while b < blocks {
            if st.get(b) == SKIPPED {
                let pos = b as u64 * BLOCK;
                let len = (size - pos).min(BLOCK).div_ceil(sector) * sector;
                match src.read_at(pos, buf.slice(len as usize)) {
                    Ok(n) if n as u64 == len => {
                        if dst.write_at(pos, buf.slice(len as usize)).is_err() {
                            st.cursor = pos;
                            return false;
                        }
                        st.set(b, b, GOOD);
                    }
                    _ => st.set(b, b, BAD_BLOCK),
                }
                st.cursor = pos;
                if !tick(st) {
                    return false;
                }
            }
            b += 1;
        }
        st.pass = 3;
        st.cursor = 0;
    }

    // ---- Pasada 3: sector a sector con reintentos ----
    if st.pass == 3 {
        let mut b = (st.cursor / BLOCK) as usize;
        while b < blocks {
            if st.get(b) == BAD_BLOCK {
                let base = b as u64 * BLOCK;
                let end = (base + BLOCK).min(size);
                let mut lost_here = 0u64;
                let mut got_any = false;
                let mut off = base;
                while off < end {
                    let n = sector.min(end - off);
                    let mut ok = false;
                    for _ in 0..retries.max(1) {
                        if let Ok(r) = src.read_at(off, buf.slice(n as usize)) {
                            if r as u64 == n {
                                ok = true;
                                break;
                            }
                        }
                    }
                    if ok {
                        got_any = true;
                        let _ = dst.write_at(off, buf.slice(n as usize));
                    } else {
                        lost_here += n;
                        merge_lost(&mut st.lost, off, n);
                        // Ceros, para no dejar basura de lo que hubiera antes en el destino.
                        let mut z = Aligned::new(n as usize);
                        z.slice(n as usize).fill(0);
                        let _ = dst.write_at(off, z.slice(n as usize));
                    }
                    off += n;
                }
                st.set(b, b, if lost_here == 0 { GOOD } else if got_any { PARTIAL } else { LOST });
                st.cursor = base;
                if !tick(st) {
                    return false;
                }
            }
            b += 1;
        }
        st.pass = 4;
    }
    tick(st);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rawdisk::MemSource;

    fn pattern(n: usize) -> Vec<u8> {
        (0..n).map(|i| ((i * 13 + i / 512) % 251) as u8).collect()
    }

    #[test]
    fn healthy_disk_is_copied_exactly() {
        let data = pattern(8 * MIB as usize);
        let mut src = MemSource { data: data.clone(), sector: 512, bad: vec![] };
        let mut dst = MemSink(vec![0u8; data.len()]);
        let mut st = CloneState::new(data.len() as u64, 512);
        assert!(clone(&mut src, &mut dst, &mut st, 2, |_| true));
        assert_eq!(dst.0, data);
        assert_eq!(st.pass, 4);
        assert!(st.lost.is_empty());
        assert!(st.states.bytes().all(|b| b == GOOD));
    }

    #[test]
    fn bad_sectors_are_isolated_and_the_rest_salvaged() {
        let data = pattern(8 * MIB as usize);
        // 10 sectores malos en medio de un bloque de 64 KB.
        let bad_start = 5 * MIB + 3 * BLOCK + 20 * 512;
        let bad = (bad_start, bad_start + 10 * 512);
        let mut src = MemSource { data: data.clone(), sector: 512, bad: vec![bad] };
        let mut dst = MemSink(vec![0xAA; data.len()]);
        let mut st = CloneState::new(data.len() as u64, 512);
        assert!(clone(&mut src, &mut dst, &mut st, 2, |_| true));
        assert_eq!(st.pass, 4);
        // Solo se pierden esos 10 sectores.
        assert_eq!(st.lost, vec![(bad.0, 10 * 512)]);
        for (i, (a, b)) in data.iter().zip(&dst.0).enumerate() {
            if (i as u64) >= bad.0 && (i as u64) < bad.1 {
                assert_eq!(*b, 0, "los sectores perdidos van a ceros");
            } else {
                assert_eq!(a, b, "difiere en {i}");
            }
        }
        let (good, lost, pending) = st.totals();
        assert_eq!(lost, 10 * 512);
        assert_eq!(pending, 0);
        assert_eq!(good + lost, data.len() as u64);
        assert!(st.cells(100).contains('x'));
    }

    #[test]
    fn stop_and_resume_from_saved_state() {
        let data = pattern(8 * MIB as usize);
        let mut src = MemSource { data: data.clone(), sector: 512, bad: vec![(2 * MIB, 2 * MIB + 4096)] };
        let mut dst = MemSink(vec![0u8; data.len()]);
        let mut st = CloneState::new(data.len() as u64, 512);
        let mut n = 0;
        assert!(!clone(&mut src, &mut dst, &mut st, 2, |_| {
            n += 1;
            n < 5
        }));
        // Se guarda y se recupera como si se hubiera cerrado la app.
        let saved = serde_json::to_string(&st).unwrap();
        let mut st: CloneState = serde_json::from_str(&saved).unwrap();
        assert!(clone(&mut src, &mut dst, &mut st, 2, |_| true));
        assert_eq!(st.pass, 4);
        assert_eq!(st.lost.len(), 1);
        assert_eq!(&dst.0[..2 * MIB as usize], &data[..2 * MIB as usize]);
        assert_eq!(&dst.0[3 * MIB as usize..], &data[3 * MIB as usize..]);
    }

    #[test]
    fn a_wholly_unreadable_block_is_lost() {
        let data = pattern(4 * MIB as usize);
        let mut src = MemSource { data: data.clone(), sector: 512, bad: vec![(MIB, MIB + BLOCK)] };
        let mut dst = MemSink(vec![0u8; data.len()]);
        let mut st = CloneState::new(data.len() as u64, 512);
        assert!(clone(&mut src, &mut dst, &mut st, 1, |_| true));
        assert_eq!(st.get((MIB / BLOCK) as usize), LOST);
        assert_eq!(st.lost, vec![(MIB, BLOCK)]);
        let (good, lost, _) = st.totals();
        assert_eq!(lost, BLOCK);
        assert_eq!(good, data.len() as u64 - BLOCK);
    }

    #[test]
    fn image_file_sink_roundtrip() {
        let dir = std::env::temp_dir().join(format!("adminops-clone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.img");
        let data = pattern(2 * MIB as usize);
        let mut src = MemSource { data: data.clone(), sector: 512, bad: vec![] };
        let mut sink = FileSink::create(&path, data.len() as u64).unwrap();
        let mut st = CloneState::new(data.len() as u64, 512);
        assert!(clone(&mut src, &mut sink, &mut st, 1, |_| true));
        assert_eq!(std::fs::read(&path).unwrap(), data);
        std::fs::remove_dir_all(&dir).ok();
    }
}
