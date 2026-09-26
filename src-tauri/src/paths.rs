//! Dónde guarda AdminOps sus datos.
//!
//! - **Instalado**: `%APPDATA%\com.adminops.app\` y los informes en `Documentos\AdminOps\Informes`.
//! - **Portable** (hay un archivo `AdminOps.portable` junto al .exe): todo en
//!   `<carpeta del exe>\AdminOps-data\`, sin dejar rastro en el equipo del cliente.
//!   El diario y los análisis se separan por equipo, porque las copias de
//!   seguridad del diario solo sirven para el PC donde se hicieron.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::Manager;

const MARKER: &str = "AdminOps.portable";

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}

/// Carpeta portable, si el marcador existe junto al ejecutable.
fn portable_root() -> Option<&'static PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        let dir = exe_dir()?;
        dir.join(MARKER).is_file().then(|| dir.join("AdminOps-data"))
    })
    .as_ref()
}

pub fn is_portable() -> bool {
    portable_root().is_some()
}

fn host() -> String {
    sysinfo::System::host_name()
        .unwrap_or_else(|| "equipo".into())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

/// Datos propios de ESTE equipo (diario de cambios, análisis).
pub fn machine_data_dir(app: &tauri::AppHandle) -> PathBuf {
    match portable_root() {
        Some(root) => root.join("equipos").join(host()),
        None => app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("AdminOps")),
    }
}

/// Informes PDF. En portable se juntan los de todos los equipos.
pub fn reports_dir(app: &tauri::AppHandle) -> PathBuf {
    match portable_root() {
        Some(root) => root.join("Informes"),
        None => app
            .path()
            .document_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("AdminOps")
            .join("Informes"),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    portable: bool,
    data_dir: String,
    reports_dir: String,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        portable: is_portable(),
        data_dir: machine_data_dir(&app).display().to_string(),
        reports_dir: reports_dir(&app).display().to_string(),
    }
}
