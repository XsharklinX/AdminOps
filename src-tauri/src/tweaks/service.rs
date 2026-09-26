//! Servicios de Windows: el tipo de inicio se lee del registro (instantáneo) y
//! se cambia con `sc.exe`, que notifica correctamente al Service Control Manager.

use super::model::Startup;
use super::registry;
use crate::ps;

fn key(name: &str) -> String {
    format!(r"HKLM\SYSTEM\CurrentControlSet\Services\{name}")
}

pub fn exists(name: &str) -> bool {
    registry::read_u32(&key(name), "Start").is_some()
}

pub fn startup(name: &str) -> Option<Startup> {
    let k = key(name);
    let start = registry::read_u32(&k, "Start")?;
    let delayed = registry::read_u32(&k, "DelayedAutostart") == Some(1);
    Some(match start {
        2 if delayed => Startup::Delayed,
        0..=2 => Startup::Automatic,
        3 => Startup::Manual,
        _ => Startup::Disabled,
    })
}

pub fn is_running(name: &str) -> bool {
    ps::exec("sc.exe", &["query", name]).is_ok_and(|out| out.contains("RUNNING"))
}

pub fn set_startup(name: &str, s: Startup) -> Result<(), String> {
    let mode = match s {
        Startup::Automatic => "auto",
        Startup::Delayed => "delayed-auto",
        Startup::Manual => "demand",
        Startup::Disabled => "disabled",
    };
    ps::exec("sc.exe", &["config", name, "start=", mode])
        .map(|_| ())
        .map_err(|e| format!("No se pudo configurar {name}: {e}"))
}

/// Detiene el servicio. No falla si ya estaba detenido.
pub fn stop(name: &str) {
    let _ = ps::exec("sc.exe", &["stop", name]);
}

pub fn start(name: &str) {
    let _ = ps::exec("sc.exe", &["start", name]);
}
