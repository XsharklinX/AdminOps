//! Prueba de estrés con temperatura: unos minutos de carga controlada del
//! procesador (y, si se quiere, de la memoria) mientras se mide la temperatura,
//! la frecuencia y el ventilador. Dice si el equipo baja la velocidad por calor,
//! si el ventilador responde y si la pasta térmica parece agotada. Se para sola
//! si pasa de un límite seguro. Se guarda cada resultado para comparar con la
//! misma prueba de hace meses.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Por encima de esto se para la prueba (los procesadores se protegen a ~100 °C).
pub const STOP_AT: f64 = 97.0;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// Segundos desde el inicio.
    pub t: u32,
    pub temp: Option<f64>,
    /// MHz medios de los núcleos.
    pub mhz: Option<f64>,
    pub fan: Option<f64>,
    pub watts: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct StressResult {
    /// Segundos desde 1970.
    pub at: u64,
    pub seconds: u32,
    pub memory: bool,
    pub samples: Vec<Sample>,
    pub max_temp: Option<f64>,
    pub stopped_hot: bool,
    pub cancelled: bool,
    /// Frecuencia al final frente a la del primer minuto (1.0: sin bajar).
    pub clock_ratio: Option<f64>,
    pub verdict: Vec<String>,
    /// ok · warn · bad
    pub level: String,
}

/// Lo que se concluye de las muestras.
pub fn analyze(samples: &[Sample], stopped_hot: bool) -> (Vec<String>, &'static str, Option<f64>, Option<f64>) {
    let temps: Vec<f64> = samples.iter().filter_map(|s| s.temp).collect();
    let max_temp = temps.iter().cloned().fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.max(t))));
    let mhz: Vec<(u32, f64)> = samples.iter().filter_map(|s| s.mhz.map(|m| (s.t, m))).collect();
    let early: Vec<f64> = mhz.iter().filter(|(t, _)| *t <= 60).map(|(_, m)| *m).collect();
    let late: Vec<f64> = mhz.iter().rev().take(5).map(|(_, m)| *m).collect();
    let avg = |v: &[f64]| if v.is_empty() { None } else { Some(v.iter().sum::<f64>() / v.len() as f64) };
    let clock_ratio = match (early.iter().cloned().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x)))), avg(&late)) {
        (Some(e), Some(l)) if e > 0.0 => Some(l / e),
        _ => None,
    };
    let fans: Vec<f64> = samples.iter().filter_map(|s| s.fan).collect();
    let fan_up = match (fans.first(), fans.iter().cloned().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))))) {
        (Some(a), Some(b)) if *a > 0.0 => Some(b / a),
        _ => None,
    };
    let hot_fast = samples.iter().any(|s| s.t <= 60 && s.temp.is_some_and(|t| t >= 90.0));
    let mut out = Vec::new();
    let mut level = "ok";
    if stopped_hot {
        out.push(format!("Se paró sola al llegar a {STOP_AT:.0} °C: el equipo no aguanta la carga sin sobrecalentarse."));
        level = "bad";
    }
    if let Some(r) = clock_ratio {
        if r < 0.7 && max_temp.is_some_and(|t| t >= 85.0) {
            out.push(format!("Baja la velocidad al {:.0} % por calor (estrangulamiento térmico): rinde mucho menos de lo que puede.", r * 100.0));
            level = "bad";
        } else if r < 0.85 {
            out.push(format!("La frecuencia baja al {:.0} % con la carga: puede ser el plan de energía o el calor.", r * 100.0));
            if level == "ok" {
                level = "warn";
            }
        } else {
            out.push("Mantiene la velocidad durante toda la prueba.".into());
        }
    }
    match max_temp {
        Some(t) if t >= 95.0 => {
            out.push(format!("Temperatura máxima {t:.0} °C: demasiado alta."));
            level = "bad";
        }
        Some(t) if t >= 85.0 => {
            out.push(format!("Temperatura máxima {t:.0} °C: alta, pero dentro de lo que aguanta."));
            if level == "ok" {
                level = "warn";
            }
        }
        Some(t) => out.push(format!("Temperatura máxima {t:.0} °C: bien.")),
        None => out.push("No se pudo leer la temperatura del procesador (hace falta administrador y el driver PawnIO): solo se ha medido la frecuencia.".into()),
    }
    if hot_fast {
        out.push("Pasa de 90 °C en el primer minuto: el calor no llega al disipador. Suele ser pasta térmica seca o polvo: limpiar y cambiar la pasta.".into());
        level = "bad";
    }
    match fan_up {
        Some(f) if f < 1.15 && max_temp.is_some_and(|t| t >= 80.0) => {
            out.push("El ventilador apenas sube de vueltas con el calor: revisa que gire y que no esté sucio.".into());
            if level == "ok" {
                level = "warn";
            }
        }
        Some(_) => out.push("El ventilador responde a la temperatura.".into()),
        None => {}
    }
    (out, level, max_temp, clock_ratio)
}

/// Estado de la prueba en marcha (para la gráfica en vivo).
static LIVE: Mutex<Option<StressResult>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn history_file(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("stress-history.json")
}

/// Media de los relojes de los núcleos que da LibreHardwareMonitor, o lo que dice Windows.
fn sample(app: &tauri::AppHandle, t: u32, sys: &mut sysinfo::System) -> Sample {
    let s = crate::hardware::sensors::read(app).ok();
    let clocks: Vec<f64> = s.as_ref().map(|s| s.all.iter().filter(|x| x.hardware_type == "Cpu" && x.kind == "Clock" && x.name.contains("Core") && x.value > 100.0).map(|x| x.value).collect()).unwrap_or_default();
    let mhz = if clocks.is_empty() {
        sys.refresh_cpu_frequency();
        let f: Vec<u64> = sys.cpus().iter().map(|c| c.frequency()).filter(|f| *f > 0).collect();
        (!f.is_empty()).then(|| f.iter().sum::<u64>() as f64 / f.len() as f64)
    } else {
        Some(clocks.iter().sum::<f64>() / clocks.len() as f64)
    };
    Sample { t, temp: s.as_ref().and_then(|s| s.cpu_temp), mhz, fan: s.as_ref().and_then(|s| s.fans.first().map(|f| f.1)), watts: s.as_ref().and_then(|s| s.cpu_power) }
}

/// Lanza la prueba (`seconds`: de 60 a 600). Se ve en vivo con `stress_live`.
#[tauri::command(async)]
pub fn stress_run(app: tauri::AppHandle, seconds: u32, memory: bool) -> Result<StressResult, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una prueba de estrés en marcha.".into());
    }
    let seconds = seconds.clamp(60, 600);
    let task = crate::task::Task::new(&app, "stress").named("Prueba de estrés");
    let stop = Arc::new(AtomicBool::new(false));
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let mut workers = Vec::new();
    for i in 0..threads {
        let stop = stop.clone();
        workers.push(std::thread::spawn(move || {
            // Cuentas que no se pueden saltar: mantienen el núcleo al 100 %.
            let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ i as u64;
            while !stop.load(Ordering::Relaxed) {
                for _ in 0..200_000 {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                }
                std::hint::black_box(x);
            }
        }));
    }
    // Memoria: se recorre un bloque grande (hasta 1/4 de la libre, máximo 4 GB).
    let mem_worker = memory.then(|| {
        let stop = stop.clone();
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        let size = ((sys.available_memory() / 4).min(4 << 30) / 8) as usize;
        std::thread::spawn(move || {
            let mut buf = vec![0u64; size.max(1)];
            let mut v = 1u64;
            while !stop.load(Ordering::Relaxed) {
                for chunk in buf.chunks_mut(4096) {
                    for x in chunk.iter_mut() {
                        *x = x.wrapping_add(v);
                    }
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
                v = v.wrapping_mul(6364136223846793005).wrapping_add(1);
            }
            std::hint::black_box(&buf);
        })
    });
    let started = Instant::now();
    let mut result = StressResult { at: now(), memory, ..Default::default() };
    let mut sys = sysinfo::System::new();
    loop {
        let t = started.elapsed().as_secs() as u32;
        let s = sample(&app, t, &mut sys);
        let hot = s.temp.is_some_and(|x| x >= STOP_AT);
        result.samples.push(s);
        task.step(format!("Carga al 100 % · {t} de {seconds} s ({} %)", t * 100 / seconds));
        *LIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(result.clone());
        if hot {
            result.stopped_hot = true;
            break;
        }
        if task.cancelled() {
            result.cancelled = true;
            break;
        }
        if t >= seconds {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    stop.store(true, Ordering::SeqCst);
    for w in workers {
        let _ = w.join();
    }
    if let Some(m) = mem_worker {
        let _ = m.join();
    }
    result.seconds = started.elapsed().as_secs() as u32;
    let (verdict, level, max_temp, ratio) = analyze(&result.samples, result.stopped_hot);
    result.verdict = verdict;
    result.level = level.into();
    result.max_temp = max_temp;
    result.clock_ratio = ratio;
    if !result.cancelled {
        let mut hist: Vec<StressResult> = crate::paths::read_json(&history_file(&app));
        let mut light = result.clone();
        light.samples = light.samples.into_iter().step_by(5).collect();
        hist.push(light);
        let keep = hist.len().saturating_sub(10);
        hist.drain(..keep);
        let _ = crate::paths::write_json(&history_file(&app), &hist);
    }
    *LIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(result.clone());
    RUNNING.store(false, Ordering::SeqCst);
    Ok(result)
}

#[tauri::command]
pub fn stress_live() -> Option<StressResult> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Pruebas anteriores de este equipo (para comparar).
#[tauri::command]
pub fn stress_history(app: tauri::AppHandle) -> Vec<StressResult> {
    crate::paths::read_json(&history_file(&app))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: u32, temp: f64, mhz: f64, fan: f64) -> Sample {
        Sample { t, temp: Some(temp), mhz: Some(mhz), fan: Some(fan), watts: None }
    }

    #[test]
    fn estrangulamiento_por_calor() {
        let v: Vec<Sample> = (0..60).map(|i| s(i * 5, 60.0 + i as f64 * 0.6, if i < 12 { 4200.0 } else { 1900.0 }, 1200.0 + i as f64 * 20.0)).collect();
        let (verdict, level, max, ratio) = analyze(&v, false);
        assert_eq!(level, "bad");
        assert!(verdict.iter().any(|x| x.contains("estrangulamiento")), "{verdict:?}");
        assert!(max.unwrap() > 90.0);
        assert!(ratio.unwrap() < 0.5);
    }

    #[test]
    fn equipo_sano() {
        let v: Vec<Sample> = (0..60).map(|i| s(i * 5, 55.0 + (i as f64).min(20.0), 4000.0, 1000.0 + i as f64 * 15.0)).collect();
        let (verdict, level, _, _) = analyze(&v, false);
        assert_eq!(level, "ok", "{verdict:?}");
        assert!(verdict.iter().any(|x| x.contains("Mantiene la velocidad")));
    }

    #[test]
    fn calor_en_el_primer_minuto_y_ventilador_quieto() {
        let v: Vec<Sample> = (0..20).map(|i| s(i * 5, 80.0 + i as f64, 3500.0, 1100.0)).collect();
        let (verdict, level, _, _) = analyze(&v, false);
        assert_eq!(level, "bad");
        assert!(verdict.iter().any(|x| x.contains("pasta")));
        assert!(verdict.iter().any(|x| x.contains("ventilador apenas")));
    }

    #[test]
    fn sin_temperatura_lo_dice() {
        let v = vec![Sample { t: 0, mhz: Some(3000.0), ..Default::default() }, Sample { t: 60, mhz: Some(3000.0), ..Default::default() }];
        let (verdict, _, max, _) = analyze(&v, false);
        assert!(max.is_none());
        assert!(verdict.iter().any(|x| x.contains("No se pudo leer la temperatura")));
    }
}
