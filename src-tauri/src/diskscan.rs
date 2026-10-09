//! Mapa de superficie y curva de velocidad.
//!
//! Recorre un disco en solo lectura, por bloques de 1 MB, y apunta para cada
//! zona si leyó bien, lento o con error. Con eso se dibuja el mapa (verde,
//! ámbar, rojo) y la curva de velocidad a lo largo del disco: una caída brusca
//! señala una zona enferma, y un SSD que se ralentiza se ve en la curva.
//!
//! No mueve ni arregla nada: es lo contrario de chkdsk /r. Dos modos:
//! - **rápido**: unos pocos MB de cada zona (minutos), para la curva y para ver si hay algo.
//! - **completo**: lee todo el disco (horas en uno grande), se puede parar y seguir otro día.
//!
//! El algoritmo trabaja sobre `BlockSource`, así que se prueba con discos en memoria.

use crate::rawdisk::{Aligned, BlockSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MB: u64 = 1024 * 1024;
/// Zonas del mapa (50 × 40).
pub const MAX_CELLS: usize = 2000;
/// En el modo rápido se lee esto de cada zona.
const QUICK_BYTES: u64 = 4 * MB;
/// Tras tantos bloques seguidos con error se salta a la siguiente zona (un disco muy enfermo tarda segundos por error).
const MAX_CONSECUTIVE_ERRORS: u32 = 3;

pub const OK: u8 = b'.';
pub const SLOW: u8 = b's';
pub const VERY_SLOW: u8 = b'v';
pub const BAD: u8 = b'x';
pub const TODO: u8 = b'?';

/// Estado de un bloque de 1 MB según lo que tardó en leerse.
pub fn classify(ms: f64, hdd: bool) -> u8 {
    let (slow, very) = if hdd { (100.0, 600.0) } else { (40.0, 300.0) };
    if ms >= very {
        VERY_SLOW
    } else if ms >= slow {
        SLOW
    } else {
        OK
    }
}

fn worse(a: u8, b: u8) -> u8 {
    let rank = |c: u8| match c {
        BAD => 4,
        VERY_SLOW => 3,
        SLOW => 2,
        OK => 1,
        _ => 0,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Summary {
    pub finished: u64,
    pub slow: u32,
    pub very_slow: u32,
    pub bad: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanResult {
    pub number: u32,
    pub model: String,
    pub size: u64,
    /// quick | full
    pub mode: String,
    /// Una letra por zona: . bien · s lento · v muy lento · x error · ? sin mirar.
    pub cells: String,
    pub done: usize,
    pub total: usize,
    pub cell_bytes: u64,
    /// MB/s a lo largo del disco (hasta 100 puntos).
    pub curve: Vec<f32>,
    pub avg_mbps: f32,
    pub min_mbps: f32,
    pub max_mbps: f32,
    pub slow: u32,
    pub very_slow: u32,
    pub bad: u32,
    pub started: u64,
    /// 0 mientras no ha terminado (se puede seguir otro día).
    pub finished: u64,
    /// Letras de los volúmenes que tienen zonas con error.
    pub volumes_hit: Vec<String>,
    /// El mismo análisis la última vez que se completó.
    pub previous: Option<Summary>,
    /// ok | warn | bad
    pub level: String,
    pub text: String,
    #[serde(skip_serializing)]
    pub speeds: Vec<f32>,
}

/// Lo que se va recorriendo.
pub struct ScanState {
    pub cells: Vec<u8>,
    pub speeds: Vec<f32>,
    pub cell_bytes: u64,
    pub done: usize,
    pub quick: bool,
    pub hdd: bool,
}

impl ScanState {
    pub fn new(size: u64, quick: bool, hdd: bool) -> Self {
        let size = size.max(MB);
        // Zonas múltiplo de 1 MB; entre 1 y MAX_CELLS.
        let cell_bytes = (size.div_ceil(MAX_CELLS as u64)).div_ceil(MB).max(1) * MB;
        let total = size.div_ceil(cell_bytes) as usize;
        ScanState { cells: vec![TODO; total], speeds: vec![0.0; total], cell_bytes, done: 0, quick, hdd }
    }
}

/// Recorre las zonas que faltan. `tick` se llama a menudo; si devuelve false, se para
/// (el progreso queda en `st` para seguir luego). Devuelve true si llegó al final.
pub fn scan<S: BlockSource>(src: &mut S, st: &mut ScanState, mut tick: impl FnMut(&ScanState) -> bool) -> bool {
    let sector = src.sector().max(512) as u64;
    let len = src.len() / sector * sector;
    let mut buf = Aligned::new(MB as usize);
    let total = st.cells.len();
    while st.done < total {
        let i = st.done;
        let start = i as u64 * st.cell_bytes;
        let end = (start + st.cell_bytes).min(len);
        if start >= end {
            st.cells[i] = OK;
            st.done += 1;
            continue;
        }
        let want = if st.quick { (end - start).min(QUICK_BYTES) } else { end - start };
        let mut state = OK;
        let mut read = 0u64;
        let mut spent = Duration::ZERO;
        let mut errors = 0u32;
        let mut off = start;
        while off < start + want {
            let n = (start + want - off).min(MB);
            let n = n / sector * sector;
            if n == 0 {
                break;
            }
            let t = Instant::now();
            let r = src.read_at(off, buf.slice(n as usize));
            let dt = t.elapsed();
            match r {
                Ok(got) if got as u64 == n => {
                    read += n;
                    spent += dt;
                    state = worse(state, classify(dt.as_secs_f64() * 1000.0, st.hdd));
                    errors = 0;
                }
                _ => {
                    state = BAD;
                    errors += 1;
                    if errors >= MAX_CONSECUTIVE_ERRORS {
                        break;
                    }
                }
            }
            off += n;
            if !tick(st) {
                // Se para a mitad de zona: esa zona se vuelve a hacer entera al seguir.
                return false;
            }
        }
        st.cells[i] = state;
        st.speeds[i] = if spent.as_secs_f64() > 0.0 { (read as f64 / MB as f64 / spent.as_secs_f64()) as f32 } else { 0.0 };
        st.done += 1;
        if !tick(st) {
            return st.done >= total;
        }
    }
    true
}

/// Curva de hasta `points` valores (media de MB/s por tramo) con las zonas leídas.
pub fn curve(cells: &[u8], speeds: &[f32], points: usize) -> Vec<f32> {
    let n = cells.len();
    if n == 0 || points == 0 {
        return Vec::new();
    }
    let points = points.min(n);
    (0..points)
        .filter_map(|p| {
            let (a, b) = (p * n / points, ((p + 1) * n / points).max(p * n / points + 1));
            let vals: Vec<f32> = (a..b.min(n)).filter(|&i| cells[i] != TODO && cells[i] != BAD && speeds[i] > 0.0).map(|i| speeds[i]).collect();
            (!vals.is_empty()).then(|| vals.iter().sum::<f32>() / vals.len() as f32)
        })
        .collect()
}

pub fn count(cells: &[u8], what: u8) -> u32 {
    cells.iter().filter(|c| **c == what).count() as u32
}

/// Qué volúmenes (letra, desplazamiento, tamaño) contienen zonas con error.
pub fn volumes_hit(cells: &[u8], cell_bytes: u64, parts: &[(String, u64, u64)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, c) in cells.iter().enumerate() {
        if *c != BAD {
            continue;
        }
        let (a, b) = (i as u64 * cell_bytes, (i as u64 + 1) * cell_bytes);
        for (letter, off, size) in parts {
            if !letter.is_empty() && a < off + size && b > *off && !out.contains(letter) {
                out.push(letter.clone());
            }
        }
    }
    out
}

/// El veredicto en palabras, comparando con la última vez.
pub fn judge(r: &ScanResult) -> (String, String) {
    let grew = r.previous.as_ref().is_some_and(|p| r.bad > p.bad);
    let mut text;
    let level;
    if r.bad > 0 {
        level = "bad";
        text = format!("{} zonas del disco no se pueden leer.", r.bad);
        if !r.volumes_hit.is_empty() {
            let v: Vec<String> = r.volumes_hit.iter().map(|l| format!("{l}:")).collect();
            text.push_str(&format!(" Caen dentro de {}: copia ya lo importante de esa unidad.", v.join(", ")));
        } else {
            text.push_str(" Copia ya lo importante a otro disco.");
        }
    } else if r.very_slow > 0 || r.slow > 4 {
        level = "warn";
        text = format!("{} zonas lentas y {} muy lentas: el disco las lee con reintentos. Suele ser el paso previo a que se estropeen.", r.slow, r.very_slow);
    } else {
        level = "ok";
        text = "Toda la superficie leída se lee bien y a velocidad normal.".to_string();
        if r.mode == "quick" {
            text.push_str(" Es una prueba rápida (una muestra de cada zona): para estar seguro haz la completa.");
        }
    }
    if let Some(p) = &r.previous {
        if grew {
            text.push_str(&format!(" Ha ido a peor: la vez anterior eran {}.", p.bad));
        } else if r.bad > 0 && r.bad == p.bad {
            text.push_str(" Igual que la vez anterior: de momento no crece.");
        }
    }
    (level.into(), text)
}

/// Rellena un resultado a partir de lo recorrido.
pub fn build_result(st: &ScanState, base: ScanResult) -> ScanResult {
    let mut r = base;
    r.cells = String::from_utf8_lossy(&st.cells).into_owned();
    r.done = st.done;
    r.total = st.cells.len();
    r.cell_bytes = st.cell_bytes;
    r.speeds = st.speeds.clone();
    r.curve = curve(&st.cells, &st.speeds, 100);
    let good: Vec<f32> = st.cells.iter().zip(&st.speeds).filter(|(c, s)| **c != TODO && **c != BAD && **s > 0.0).map(|(_, s)| *s).collect();
    r.avg_mbps = if good.is_empty() { 0.0 } else { good.iter().sum::<f32>() / good.len() as f32 };
    r.min_mbps = good.iter().copied().fold(f32::MAX, f32::min);
    if good.is_empty() {
        r.min_mbps = 0.0;
    }
    r.max_mbps = good.iter().copied().fold(0.0, f32::max);
    r.slow = count(&st.cells, SLOW);
    r.very_slow = count(&st.cells, VERY_SLOW);
    r.bad = count(&st.cells, BAD);
    r
}

// ---------- Estado en vivo ----------

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    pub cells: String,
    pub done: usize,
    pub total: usize,
    pub percent: u8,
    pub mbps: f32,
    pub eta_secs: u64,
    pub slow: u32,
    pub very_slow: u32,
    pub bad: u32,
}

static LIVE: Mutex<Option<HashMap<u32, Live>>> = Mutex::new(None);

pub fn set_live(number: u32, live: Live) {
    LIVE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(number, live);
}

pub fn get_live(number: u32) -> Option<Live> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&number).cloned())
}

pub fn clear_live(number: u32) {
    if let Some(m) = LIVE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(&number);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rawdisk::MemSource;

    fn disk(mb: usize, bad: Vec<(u64, u64)>) -> MemSource {
        MemSource { data: vec![7u8; mb * MB as usize], sector: 512, bad }
    }

    #[test]
    fn cells_cover_the_disk() {
        let st = ScanState::new(1000 * MB, false, true);
        assert_eq!(st.cell_bytes, MB);
        assert_eq!(st.cells.len(), 1000);
        let big = ScanState::new(2000 * 1024 * MB, false, true);
        assert!(big.cells.len() <= MAX_CELLS);
        assert!(big.cell_bytes.is_multiple_of(MB));
        assert!(big.cell_bytes * big.cells.len() as u64 >= 2000 * 1024 * MB);
    }

    #[test]
    fn healthy_disk_maps_green() {
        let mut d = disk(64, vec![]);
        let mut st = ScanState::new(d.len(), false, true);
        assert!(scan(&mut d, &mut st, |_| true));
        assert_eq!(st.done, st.cells.len());
        assert!(st.cells.iter().all(|c| *c == OK));
    }

    #[test]
    fn bad_range_maps_red_and_scan_continues() {
        let mut d = disk(64, vec![(10 * MB, 12 * MB)]);
        let mut st = ScanState::new(d.len(), false, true);
        assert!(scan(&mut d, &mut st, |_| true));
        let bad: Vec<usize> = st.cells.iter().enumerate().filter(|(_, c)| **c == BAD).map(|(i, _)| i).collect();
        assert_eq!(bad, vec![10, 11]);
        assert_eq!(st.cells[13], OK);
    }

    #[test]
    fn can_stop_and_resume() {
        let mut d = disk(64, vec![]);
        let mut st = ScanState::new(d.len(), false, true);
        let mut calls = 0;
        let finished = scan(&mut d, &mut st, |_| {
            calls += 1;
            calls < 20
        });
        assert!(!finished);
        let done = st.done;
        assert!(done > 0 && done < 64);
        assert!(scan(&mut d, &mut st, |_| true));
        assert_eq!(st.done, 64);
        assert!(st.cells.iter().all(|c| *c == OK));
    }

    #[test]
    fn quick_mode_samples_each_cell() {
        let mut d = disk(256, vec![(100 * MB + 1, 100 * MB + 2)]);
        // Zonas de 1 MB: el modo rápido lee 4 MB como máximo por zona, es decir, todo; con zonas grandes solo el principio.
        let mut st = ScanState::new(d.len(), true, true);
        st.cell_bytes = 16 * MB;
        st.cells = vec![TODO; 16];
        st.speeds = vec![0.0; 16];
        assert!(scan(&mut d, &mut st, |_| true));
        // El error está a 100 MB: en la zona 6 (96-112), no en los 4 primeros MB → el rápido no lo ve.
        assert!(st.cells.iter().all(|c| *c == OK));
    }

    #[test]
    fn classify_by_media() {
        assert_eq!(classify(5.0, true), OK);
        assert_eq!(classify(150.0, true), SLOW);
        assert_eq!(classify(700.0, true), VERY_SLOW);
        assert_eq!(classify(50.0, false), SLOW);
        assert_eq!(classify(20.0, false), OK);
    }

    #[test]
    fn curve_averages_and_skips_bad() {
        let cells = vec![OK, OK, BAD, OK];
        let speeds = vec![100.0, 80.0, 0.0, 60.0];
        let c = curve(&cells, &speeds, 4);
        assert_eq!(c, vec![100.0, 80.0, 60.0]);
        let two = curve(&cells, &speeds, 2);
        assert_eq!(two, vec![90.0, 60.0]);
    }

    #[test]
    fn bad_cells_map_to_volumes() {
        let cells = vec![OK, BAD, BAD, OK];
        let parts = vec![("C".to_string(), 0, MB), ("D".to_string(), MB, 2 * MB), ("".to_string(), 3 * MB, MB)];
        assert_eq!(volumes_hit(&cells, MB, &parts), vec!["D".to_string()]);
    }

    #[test]
    fn judge_compares_with_previous() {
        let mut r = ScanResult { bad: 5, volumes_hit: vec!["D".into()], previous: Some(Summary { bad: 2, ..Default::default() }), mode: "full".into(), ..Default::default() };
        let (lvl, text) = judge(&r);
        assert_eq!(lvl, "bad");
        assert!(text.contains("D:") && text.contains("a peor"));
        r.bad = 0;
        r.mode = "quick".into();
        let (lvl, text) = judge(&r);
        assert_eq!(lvl, "ok");
        assert!(text.contains("rápida"));
    }

    #[test]
    fn build_result_counts() {
        let mut d = disk(32, vec![(3 * MB, 4 * MB)]);
        let mut st = ScanState::new(d.len(), false, true);
        scan(&mut d, &mut st, |_| true);
        let r = build_result(&st, ScanResult::default());
        assert_eq!(r.bad, 1);
        assert_eq!(r.total, 32);
        assert!(r.cells.starts_with("..."));
        assert_eq!(r.cells.as_bytes()[3], BAD);
    }
}
