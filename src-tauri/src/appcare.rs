//! Cuidado de AdminOps: inicio con Windows, limpieza de datos antiguos y aviso
//! de versiones nuevas.

use crate::tweaks::TweakState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

// ---------- Inicio con Windows ----------

/// Tarea programada al iniciar sesión, con los permisos más altos: AdminOps
/// necesita administrador y una entrada «Run» del registro no puede elevarse.
const TASK: &str = "AdminOps al iniciar sesión";

#[tauri::command(async)]
pub fn autostart_enabled() -> bool {
    let script = format!("{}[bool](Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue)", crate::ps::text_var("name", TASK));
    crate::pspool::query(&script, Some(Duration::from_secs(20)), "Inicio con Windows: estado").is_ok_and(|o| o.trim().eq_ignore_ascii_case("true"))
}

#[tauri::command(async)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = if enabled {
        format!(
            "$ErrorActionPreference = 'Stop'\n{}{}\
             $action = New-ScheduledTaskAction -Execute $exe -Argument '--minimized'\n\
             $trigger = New-ScheduledTaskTrigger -AtLogOn -User \"$env:USERDOMAIN\\$env:USERNAME\"\n\
             $trigger.Delay = 'PT20S'\n\
             $principal = New-ScheduledTaskPrincipal -UserId \"$env:USERDOMAIN\\$env:USERNAME\" -LogonType Interactive -RunLevel Highest\n\
             $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Seconds 0)\n\
             Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null\n'ok'",
            crate::ps::text_var("name", TASK),
            crate::ps::text_var("exe", &exe.display().to_string())
        )
    } else {
        format!("{}Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue\n'ok'", crate::ps::text_var("name", TASK))
    };
    crate::ps::powershell(&script).map(|_| ())
}

// ---------- Datos antiguos ----------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DataUsage {
    journal_entries: usize,
    snapshots: usize,
    snapshots_bytes: u64,
    reports: usize,
    reports_bytes: u64,
    speedtests: usize,
    logs_bytes: u64,
}

fn dir_files(dir: &Path) -> Vec<(PathBuf, u64, u64)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = e.metadata() else { continue };
        if m.is_dir() {
            out.extend(dir_files(&p));
        } else {
            let modified = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            out.push((p, m.len(), modified));
        }
    }
    out
}

#[tauri::command(async)]
pub fn data_usage(app: tauri::AppHandle, state: State<'_, TweakState>) -> DataUsage {
    let snaps = crate::diagnostics::snapshot_files(&crate::diagnostics::snapshots_dir(&app));
    let reports = dir_files(&crate::paths::reports_dir(&app));
    DataUsage {
        journal_entries: state.journal_len(),
        snapshots: snaps.len(),
        snapshots_bytes: snaps.iter().filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum(),
        reports: reports.len(),
        reports_bytes: reports.iter().map(|f| f.1).sum(),
        speedtests: crate::network::speedtest::history(&app).len(),
        logs_bytes: dir_files(&crate::paths::logs_dir(&app)).iter().map(|f| f.1).sum(),
    }
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cleaned {
    journal: usize,
    snapshots: usize,
    reports: usize,
    freed: u64,
}

/// Envía a la papelera (los informes pueden hacer falta: así se pueden recuperar).
fn to_recycle_bin(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let list: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    let script = format!(
        "{}Add-Type -AssemblyName Microsoft.VisualBasic\nforeach ($p in @(ConvertFrom-Json $json)) {{ [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($p, 'OnlyErrorDialogs', 'SendToRecycleBin') }}\n'ok'",
        crate::ps::text_var("json", &serde_json::to_string(&list).unwrap_or_default())
    );
    crate::pspool::query(&script, Some(Duration::from_secs(120)), "Limpieza: informes antiguos a la papelera").map(|_| ())
}

pub fn cleanup(app: &tauri::AppHandle, state: &TweakState, months: u32, journal: bool, snapshots: bool, reports: bool) -> Result<Cleaned, String> {
    if months == 0 {
        return Err("Elige una antigüedad.".into());
    }
    let cutoff = now().saturating_sub(months as u64 * 30 * 86_400);
    let mut out = Cleaned::default();
    if journal {
        out.journal = state.prune_journal(cutoff);
    }
    if snapshots {
        let mut snaps = crate::diagnostics::snapshot_files(&crate::diagnostics::snapshots_dir(app));
        snaps.sort_by_key(|s| std::cmp::Reverse(s.0));
        // El más reciente se queda siempre (es el «antes» de la próxima comparación), y el de una sesión en curso.
        let keep = crate::workflow::active_baseline(app);
        for (_, p) in snaps.into_iter().skip(1).filter(|(ts, _)| *ts < cutoff && Some(*ts) != keep) {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if std::fs::remove_file(&p).is_ok() {
                out.snapshots += 1;
                out.freed += size;
            }
        }
    }
    if reports {
        let old: Vec<(PathBuf, u64, u64)> = dir_files(&crate::paths::reports_dir(app)).into_iter().filter(|f| f.2 < cutoff).collect();
        to_recycle_bin(&old.iter().map(|f| f.0.clone()).collect::<Vec<_>>())?;
        out.reports = old.len();
        out.freed += old.iter().map(|f| f.1).sum::<u64>();
    }
    log::info!("Limpieza de datos ({months} meses): {} entradas, {} análisis, {} informes", out.journal, out.snapshots, out.reports);
    Ok(out)
}

#[tauri::command(async)]
pub fn data_cleanup(app: tauri::AppHandle, state: State<'_, TweakState>, months: u32, journal: bool, snapshots: bool, reports: bool) -> Result<Cleaned, String> {
    cleanup(&app, &state, months, journal, snapshots, reports)
}

/// Limpieza automática al abrir (Ajustes → General), como mucho una vez al día.
pub fn auto_cleanup(app: &tauri::AppHandle, state: &TweakState) {
    let months = crate::workflow::settings(app).auto_cleanup_months;
    if months == 0 {
        return;
    }
    let stamp = crate::paths::shared_data_dir(app).join("last-cleanup.txt");
    let last: u64 = std::fs::read_to_string(&stamp).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    if now().saturating_sub(last) < 86_400 {
        return;
    }
    let _ = std::fs::write(&stamp, now().to_string());
    if let Err(e) = cleanup(app, state, months, true, true, true) {
        log::warn!("Limpieza automática: {e}");
    }
}

// ---------- Versiones nuevas ----------

const RELEASES: &str = "https://api.github.com/repos/XsharklinX/AdminOps/releases/latest";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: String,
    newer: bool,
    url: String,
    notes: String,
}

fn parse_version(v: &str) -> Vec<u32> {
    v.trim_start_matches(['v', 'V']).split(['.', '-']).map(|p| p.parse().unwrap_or(0)).collect()
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
        html_url: String,
        #[serde(default)]
        body: String,
    }
    let current = app.package_info().version.to_string();
    let client = reqwest::Client::builder().timeout(Duration::from_secs(8)).user_agent(format!("AdminOps/{current}")).build().map_err(|e| e.to_string())?;
    let r: Release = client
        .get(RELEASES)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|_| "No se pudo consultar si hay versiones nuevas.".to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    // Solo se abren enlaces de GitHub.
    let url = if r.html_url.starts_with("https://github.com/") { r.html_url } else { "https://github.com/XsharklinX/AdminOps/releases/latest".into() };
    Ok(UpdateInfo {
        newer: parse_version(&r.tag_name) > parse_version(&current),
        latest: r.tag_name.trim_start_matches(['v', 'V']).to_string(),
        current,
        url,
        notes: r.body.chars().take(2000).collect(),
    })
}

#[tauri::command]
pub fn open_release_page(url: String) -> Result<(), String> {
    if !url.starts_with("https://github.com/XsharklinX/AdminOps") {
        return Err("Dirección no permitida.".into());
    }
    std::process::Command::new("explorer.exe").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(parse_version("v1.0.1") > parse_version("1.0.0"));
        assert!(parse_version("1.0.0") > parse_version("0.23.0"));
        assert!(parse_version("0.23.0") == parse_version("v0.23.0"));
    }
}
