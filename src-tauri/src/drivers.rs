//! Copia de seguridad de drivers de terceros con pnputil.

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use std::time::Duration;
use tauri::State;

#[tauri::command(async)]
pub fn backup_drivers(app: tauri::AppHandle, state: State<'_, TweakState>) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let host = sysinfo::System::host_name().unwrap_or_else(|| "equipo".into());
    let dest = crate::paths::drivers_backup_dir(&app).join(format!("{host}_{}", chrono::Local::now().format("%Y-%m-%d_%H%M")));
    std::fs::create_dir_all(&dest).map_err(|e| format!("No se pudo crear {}: {e}", dest.display()))?;

    let task = Task::new(&app, "drivers-backup");
    task.step("Exportando drivers de terceros (puede tardar un par de minutos)…");
    let result = crate::ps::exec_opts(
        "pnputil.exe",
        &["/export-driver", "*", &dest.display().to_string()],
        task.opts(Some(Duration::from_secs(15 * 60))),
    )
    .map(|_| {
        let count = std::fs::read_dir(&dest).map(|d| d.flatten().filter(|e| e.path().is_dir()).count()).unwrap_or(0);
        format!("{count} drivers copiados en {}", dest.display())
    });
    state.record(Op::Run, "Copia de seguridad de drivers", &result);
    if result.is_ok() {
        let _ = std::process::Command::new("explorer.exe").arg(&dest).spawn();
    } else {
        let _ = std::fs::remove_dir_all(&dest);
    }
    result
}
