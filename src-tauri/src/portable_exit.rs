//! Salir sin dejar rastro: en modo portable, lista y borra lo que ESTA
//! ejecución de AdminOps ha usado en el equipo. Nunca toca datos de una
//! instalación normal ni la carpeta `%LOCALAPPDATA%\AdminOps\WebView`.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub kind: String,
    pub label: String,
    pub detail: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FailedItem {
    pub label: String,
    pub error: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub ok: bool,
    pub removed: Vec<String>,
    pub failed: Vec<FailedItem>,
}

/// Rutas usadas por esta ejecución portable y que se pueden quitar al salir.
static WEBVIEW_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static LHM_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static TASK_CREATED: AtomicBool = AtomicBool::new(false);

/// Guarda la carpeta de datos de WebView2 que ha usado esta ejecución.
pub fn set_webview_dir(path: PathBuf) {
    if crate::paths::is_portable() {
        if let Ok(mut d) = WEBVIEW_DIR.lock() {
            *d = Some(path);
        }
    }
}

/// Guarda la copia local de las DLL de sensores cargadas en esta ejecución.
pub fn set_lhm_dir(path: PathBuf) {
    if crate::paths::is_portable() {
        if let Ok(mut d) = LHM_DIR.lock() {
            *d = Some(path);
        }
    }
}

/// Marca que esta sesión ha creado la tarea programada de mantenimiento.
pub fn mark_maintenance_task_created() {
    TASK_CREATED.store(true, Ordering::SeqCst);
}

/// ¿Está la tarea programada creada por esta sesión? (No se pregunta por otras.)
fn task_exists() -> bool {
    if !TASK_CREATED.load(Ordering::SeqCst) {
        return false;
    }
    let script = r"Get-ScheduledTask -TaskPath '\AdminOps\' -TaskName 'Mantenimiento' -ErrorAction SilentlyContinue | Select-Object -First 1 | ConvertTo-Json -Compress";
    match crate::ps::powershell(script) {
        Ok(out) => !out.trim().is_empty() && out.trim() != "null",
        Err(_) => false,
    }
}

fn remove_task() -> Result<(), String> {
    let script = r"Unregister-ScheduledTask -TaskPath '\AdminOps\' -TaskName 'Mantenimiento' -Confirm:$false -ErrorAction Stop; 'ok'";
    crate::ps::powershell(script).map(|_| ()).map_err(|e| format!("No se pudo quitar la tarea programada: {e}"))
}

/// Carpetas temporales de esta sesión (nombre propio + PID del proceso).
fn own_temp_dirs() -> Vec<PathBuf> {
    let pid = std::process::id();
    let temp = std::env::temp_dir();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&temp) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("adminops-") && name.ends_with(&format!("-{pid}")) {
                out.push(e.path());
            }
        }
    }
    out
}

/// La carpeta de WebView2 que se puede quitar: solo si está en el pendrive
/// o en la carpeta temporal propia; nunca `%LOCALAPPDATA%\AdminOps\WebView`.
fn tracked_webview_dir() -> Option<PathBuf> {
    let path = WEBVIEW_DIR.lock().ok().and_then(|d| d.clone())?;
    // Carpetas del pendrive portable: sí.
    if let Some(root) = crate::paths::portable_data_root() {
        if path.starts_with(root) {
            return Some(path);
        }
    }
    // Carpeta temporal propia: sí.
    let temp_webview = std::env::temp_dir().join("AdminOps").join("WebView");
    if path == temp_webview {
        return Some(path);
    }
    None
}

fn tracked_lhm_dir() -> Option<PathBuf> {
    LHM_DIR.lock().ok().and_then(|d| d.clone())
}

pub fn leftovers(_app: &tauri::AppHandle) -> Vec<Leftover> {
    if !crate::paths::is_portable() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(path) = tracked_webview_dir().filter(|p| p.exists()) {
        out.push(Leftover {
            kind: "folder".into(),
            label: "Caché del navegador interno".into(),
            detail: Some(crate::escalate::scrub(&path.display().to_string())),
        });
    }
    if let Some(path) = tracked_lhm_dir().filter(|p| p.exists()) {
        out.push(Leftover {
            kind: "folder".into(),
            label: "Lector de temperaturas".into(),
            detail: Some(crate::escalate::scrub(&path.display().to_string())),
        });
    }
    let temps = own_temp_dirs();
    if !temps.is_empty() {
        out.push(Leftover {
            kind: "temp".into(),
            label: format!("Carpeta{} temporal{} de esta sesión", if temps.len() == 1 { "" } else { "s" }, if temps.len() == 1 { "" } else { "es" }),
            detail: Some(temps.iter().map(|p| crate::escalate::scrub(&p.display().to_string())).collect::<Vec<_>>().join("\n")),
        });
    }
    if task_exists() {
        out.push(Leftover {
            kind: "task".into(),
            label: "Tarea programada de mantenimiento".into(),
            detail: None,
        });
    }
    out
}

fn remove_dir(path: &PathBuf) -> Result<(), String> {
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| format!("No se pudo borrar {}: {e}", crate::escalate::scrub(&path.display().to_string())))
    } else {
        std::fs::remove_file(path).map_err(|e| format!("No se pudo borrar {}: {e}", crate::escalate::scrub(&path.display().to_string())))
    }
}

pub fn cleanup(_app: &tauri::AppHandle, _keep_night_jobs: bool) -> Result<CleanupResult, String> {
    if !crate::paths::is_portable() {
        return Err("Solo disponible en modo portable.".into());
    }
    let mut result = CleanupResult { ok: true, removed: Vec::new(), failed: Vec::new() };

    if let Some(path) = tracked_webview_dir().filter(|p| p.exists()) {
        match remove_dir(&path) {
            Ok(()) => result.removed.push("Caché del navegador interno".into()),
            Err(e) => {
                result.ok = false;
                result.failed.push(FailedItem { label: "Caché del navegador interno".into(), error: e });
            }
        }
    }

    if let Some(path) = tracked_lhm_dir().filter(|p| p.exists()) {
        match remove_dir(&path) {
            Ok(()) => result.removed.push("Lector de temperaturas".into()),
            Err(e) => {
                result.ok = false;
                result.failed.push(FailedItem { label: "Lector de temperaturas".into(), error: e });
            }
        }
    }

    for path in own_temp_dirs().into_iter().filter(|p| p.exists()) {
        match remove_dir(&path) {
            Ok(()) => result.removed.push(format!("Temporal: {}", crate::escalate::scrub(&path.display().to_string()))),
            Err(e) => {
                result.ok = false;
                result.failed.push(FailedItem { label: format!("Temporal: {}", crate::escalate::scrub(&path.display().to_string())), error: e });
            }
        }
    }

    if !_keep_night_jobs && task_exists() {
        match remove_task() {
            Ok(()) => result.removed.push("Tarea programada de mantenimiento".into()),
            Err(e) => {
                result.ok = false;
                result.failed.push(FailedItem { label: "Tarea programada de mantenimiento".into(), error: e });
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn portable_leftovers(app: tauri::AppHandle) -> Vec<Leftover> {
    leftovers(&app)
}

#[tauri::command]
pub fn portable_cleanup(app: tauri::AppHandle, keep_night_jobs: bool) -> Result<CleanupResult, String> {
    cleanup(&app, keep_night_jobs)
}
