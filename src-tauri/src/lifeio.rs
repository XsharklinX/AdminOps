//! ¿Reparar o cambiar este equipo?: el comando de la interfaz. La valoración está
//! en `lifespan.rs`; aquí se arma con lo que ya saben el último análisis y el
//! estado de los discos.

use crate::diagnostics::{self, latest_snapshot};
use crate::lifespan::{evaluate, Costs, Input, Report};
use chrono::Datelike;

#[tauri::command]
pub fn life_report(app: tauri::AppHandle, costs: Option<Costs>) -> Result<Report, String> {
    let d = latest_snapshot(&app).ok_or("Ejecuta un diagnóstico antes: la valoración usa sus datos.")?;
    let hw = d.hardware.data.as_ref();
    let sys = d.system.data.as_ref();
    let disks: Vec<_> = d.disks.data.as_deref().unwrap_or(&[]).iter().filter(|k| !k.bus_type.eq_ignore_ascii_case("USB")).collect();
    let year = hw.and_then(|h| h.bios_date.as_deref()).and_then(diagnostics::parse_time).map(|t| t.year());
    // Un disco que el módulo de discos da por «fallando» (la última vez que se miraron).
    let disk_failing = crate::disks::last_saved(&app).disks.iter().any(|k| k.level == "bad" && !k.bus.eq_ignore_ascii_case("USB"));
    let input = Input {
        cpu: d.cpu.clone(),
        year,
        this_year: chrono::Local::now().year(),
        ram_gb: d.ram_total as f64 / 1024f64.powi(3),
        windows10: d.os.contains("Windows 10"),
        windows11: d.os.contains("Windows 11"),
        tpm_ready: sys.and_then(|s| s.tpm_ready),
        secure_boot: sys.and_then(|s| s.secure_boot),
        hdd_only: !disks.is_empty() && disks.iter().all(|k| k.media_type.eq_ignore_ascii_case("HDD")),
        disk_failing,
        battery: d.battery.data.as_ref().and_then(|b| b.as_ref().map(|b| b.health())),
    };
    Ok(evaluate(&input, &costs.unwrap_or_default().sane()))
}
