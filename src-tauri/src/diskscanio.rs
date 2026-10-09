//! Mapa de superficie: los comandos de la interfaz (leer el disco, guardar el
//! resultado, seguir otro día). El algoritmo está en `diskscan.rs`.

use crate::diskscan::*;
use crate::rawdisk::{BlockSource, Mode, RawDisk};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Lo que se ve por el camino (la interfaz lo consulta cada segundo).
#[tauri::command]
pub fn disk_scan_live(number: u32) -> Option<Live> {
    get_live(number)
}

fn store_path(app: &tauri::AppHandle, number: u32) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join(format!("superficie-{number}.json"))
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    /// El análisis en curso o el último terminado.
    current: Option<ScanResult>,
    /// El último que se completó (para comparar).
    last_full: Option<Summary>,
}

/// El último análisis de este disco (el terminado o el que quedó a medias).
#[tauri::command]
pub fn disk_scan_last(app: tauri::AppHandle, number: u32, model: String, size: u64) -> Option<ScanResult> {
    let saved: Saved = crate::paths::read_json(&store_path(&app, number));
    saved.current.filter(|r| r.model == model && r.size == size)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Letras, desplazamientos y tamaños de las particiones del disco.
fn partitions_of(number: u32) -> Vec<(String, u64, u64)> {
    let script = format!("Get-Partition -DiskNumber {number} -ErrorAction SilentlyContinue | ForEach-Object {{ \"$($_.DriveLetter)|$($_.Offset)|$($_.Size)\" }}");
    crate::ps::powershell(&script)
        .map(|out| {
            out.lines()
                .filter_map(|l| {
                    let mut p = l.trim().split('|');
                    let (letter, off, size) = (p.next()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?);
                    Some((letter.to_string(), off, size))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Recorre el disco en solo lectura. `mode`: "quick" o "full". Con `resume`, sigue donde se quedó.
#[tauri::command(async)]
pub fn disk_scan(app: tauri::AppHandle, number: u32, model: String, hdd: bool, mode: String, resume: bool, tweaks: tauri::State<'_, crate::tweaks::TweakState>) -> Result<ScanResult, String> {
    use crate::tweaks::journal::Op;
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let quick = mode != "full";
    let mut disk = RawDisk::open(number, Mode::Read)?;
    let size = disk.len();
    if size == 0 {
        return Err("No se pudo saber el tamaño del disco.".into());
    }
    let path = store_path(&app, number);
    let mut saved: Saved = crate::paths::read_json(&path);

    let mut st = ScanState::new(size, quick, hdd);
    let mut base = ScanResult { number, model: model.clone(), size, mode: if quick { "quick".into() } else { "full".into() }, started: now(), ..Default::default() };
    if resume {
        if let Some(prev) = saved.current.as_ref().filter(|r| r.finished == 0 && r.model == model && r.size == size && r.mode == base.mode && r.cell_bytes == st.cell_bytes) {
            st.cells = prev.cells.bytes().collect();
            st.speeds = if prev.speeds.len() == st.cells.len() { prev.speeds.clone() } else { vec![0.0; st.cells.len()] };
            st.done = prev.done.min(st.cells.len());
            base.started = prev.started;
        }
    }
    base.previous = saved.last_full.clone();

    let task = crate::task::Task::new(&app, format!("disk-scan:{number}")).named(if quick { "Prueba rápida de la superficie" } else { "Mapa de superficie" });
    task.step(if st.done > 0 { "Siguiendo donde se quedó…" } else { "Leyendo el disco (solo lectura)…" });
    let began = Instant::now();
    let done_at_start = st.done;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut last_save = Instant::now();
    let total_cells = st.cells.len();

    let finished = scan(&mut disk, &mut st, |s| {
        if last_emit.elapsed() >= Duration::from_millis(700) {
            last_emit = Instant::now();
            let pct = (s.done * 100 / total_cells.max(1)) as u8;
            let progressed = s.done.saturating_sub(done_at_start).max(1);
            let per_cell = began.elapsed().as_secs_f64() / progressed as f64;
            let eta = (per_cell * (total_cells - s.done) as f64) as u64;
            let recent = s.speeds[..s.done].iter().rev().take(8).copied().filter(|v| *v > 0.0).collect::<Vec<_>>();
            let mbps = if recent.is_empty() { 0.0 } else { recent.iter().sum::<f32>() / recent.len() as f32 };
            set_live(number, Live { cells: String::from_utf8_lossy(&s.cells).into_owned(), done: s.done, total: total_cells, percent: pct, mbps, eta_secs: eta, slow: count(&s.cells, SLOW), very_slow: count(&s.cells, VERY_SLOW), bad: count(&s.cells, BAD) });
            task.step(format!("Superficie {pct} % · {mbps:.0} MB/s · quedan {} min", eta / 60));
        }
        if last_save.elapsed() >= Duration::from_secs(60) {
            last_save = Instant::now();
            saved.current = Some(build_result(s, base.clone()));
            let _ = crate::paths::write_json(&path, &saved);
        }
        !task.cancelled()
    });
    clear_live(number);

    let mut r = build_result(&st, base);
    if finished {
        r.finished = now();
        r.volumes_hit = volumes_hit(&st.cells, st.cell_bytes, &partitions_of(number));
        let (level, text) = judge(&r);
        r.level = level;
        r.text = text;
        if !quick {
            saved.last_full = Some(Summary { finished: r.finished, slow: r.slow, very_slow: r.very_slow, bad: r.bad });
        }
    } else {
        r.level = "ok".into();
        r.text = format!("Parado al {} %: se puede seguir otro día desde donde se quedó.", st.done * 100 / total_cells.max(1));
    }
    saved.current = Some(r.clone());
    let _ = crate::paths::write_json(&path, &saved);
    let label = if quick { "Prueba rápida de superficie" } else { "Mapa de superficie" };
    let journal: Result<(), String> = Ok(());
    tweaks.record(Op::Run, &format!("{label} del disco {number}: {}", if finished { format!("{} lentas, {} con error", r.slow + r.very_slow, r.bad) } else { "parada".into() }), &journal);
    Ok(r)
}

