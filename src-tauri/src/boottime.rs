//! Cuánto tarda en arrancar AdminOps, por partes, para atacar lo lento con
//! datos. La interfaz añade lo suyo (carga del código, primera página) y lo
//! muestra en Ajustes → Rendimiento de AdminOps.

use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static STEPS: Mutex<Vec<Step>> = Mutex::new(Vec::new());

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub name: String,
    /// Desde que arrancó el proceso.
    pub at_ms: u64,
    pub ms: u64,
}

/// Al principio de todo: el resto se mide desde aquí.
pub fn mark_start() {
    START.get_or_init(Instant::now);
}

pub fn since_start_ms() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Mide un paso del arranque y lo anota.
pub fn step<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let at = since_start_ms();
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_millis() as u64;
    STEPS.lock().unwrap_or_else(|e| e.into_inner()).push(Step { name: name.into(), at_ms: at, ms });
    r
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupTiming {
    /// Pasos medidos del arranque del backend.
    pub steps: Vec<Step>,
    /// Milisegundos desde que arrancó el proceso hasta esta llamada: con
    /// `performance.now()` de la interfaz se sabe cuándo empezó a cargarse.
    pub now_ms: u64,
}

#[tauri::command]
pub fn startup_timing() -> StartupTiming {
    StartupTiming { steps: STEPS.lock().unwrap_or_else(|e| e.into_inner()).clone(), now_ms: since_start_ms() }
}

/// La interfaz deja el resumen en el registro técnico (para comparar versiones).
#[tauri::command]
pub fn log_timing(summary: String) {
    log::info!("Tiempos: {}", summary.chars().take(2000).collect::<String>());
}

#[cfg(test)]
mod tests {
    #[test]
    fn records_steps_in_order() {
        super::mark_start();
        let v = super::step("prueba", || 7);
        assert_eq!(v, 7);
        let t = super::startup_timing();
        assert!(t.steps.iter().any(|s| s.name == "prueba"));
        assert!(t.now_ms >= t.steps.last().unwrap().at_ms);
    }
}
