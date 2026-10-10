//! Más problemas con respuesta (1.2.8): Outlook y Office, OneDrive y Teams, el
//! perfil temporal, los navegadores, licencias, USB que se desconectan, y lo
//! que se añade a Windows Update e impresoras. Cada módulo mira el equipo,
//! explica lo que encuentra y ofrece el arreglo que toca, con el mismo formato
//! de «Solucionar problemas» (`troubleshoot.rs`).

pub mod browsers;
pub mod licenses;
pub mod maildns;
pub mod onedrive;
pub mod outlook;
pub mod printdeep;
pub mod tempprofile;
pub mod usbdrop;
pub mod wudeep;

use crate::troubleshoot::Finding;
use crate::tweaks::TweakState;

/// Comprobación de un síntoma nuevo, o None si no es de aquí.
pub fn check(symptom: &str) -> Option<Result<Vec<Finding>, String>> {
    Some(match symptom {
        "outlook" => outlook::check(),
        "onedrive" => onedrive::check(),
        "profile" => tempprofile::check(),
        "browser" => browsers::check(),
        "license" => licenses::check(),
        "usb" => usbdrop::check(),
        _ => return None,
    })
}

/// Arreglo de uno de estos módulos, o None si la clave no es de aquí.
pub fn run(tweaks: &TweakState, kind: &str, arg: &str) -> Option<Result<String, String>> {
    let module = kind.split('.').next().unwrap_or("");
    match module {
        "ol" => outlook::run(kind, arg),
        "od" | "teams" => onedrive::run(kind, arg),
        "prof" => tempprofile::run(kind, arg),
        "br" => browsers::run(kind, arg),
        "usb" => usbdrop::run(tweaks, kind, arg),
        "wu" => wudeep::run(tweaks, kind, arg),
        "pr" => printdeep::run(kind, arg),
        "reveal" => Some(crate::uxio::reveal_path(arg.to_string()).map(|()| String::new())),
        _ => None,
    }
}

/// Nombre del arreglo para el diario («Desactivar un complemento de Outlook»…).
pub fn title(kind: &str) -> &'static str {
    outlook::title(kind)
        .or_else(|| onedrive::title(kind))
        .or_else(|| tempprofile::title(kind))
        .or_else(|| browsers::title(kind))
        .or_else(|| usbdrop::title(kind))
        .or_else(|| wudeep::title(kind))
        .or_else(|| printdeep::title(kind))
        .unwrap_or("")
}

/// Lee un JSON de un script de PowerShell, con un error comprensible.
pub(crate) fn parse<T: serde::de::DeserializeOwned>(out: &str) -> Result<T, String> {
    serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))
}

/// «12,3 GB»
pub(crate) fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1024f64.powi(3)).replace('.', ",")
}

/// Carpeta de las copias del registro hechas antes de tocarlo (junto a los datos de AdminOps).
pub(crate) fn backup_dir() -> std::path::PathBuf {
    crate::paths::shared_data_dir_early().unwrap_or_else(std::env::temp_dir).join("copias-registro")
}

/// Copia de una clave del registro antes de tocarla (`reg export`). Devuelve el archivo.
pub(crate) fn reg_backup(reg_path: &str, label: &str) -> Result<std::path::PathBuf, String> {
    let dir = backup_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("{label}-{}.reg", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    crate::ps::exec("reg.exe", &["export", &crate::tweaks::registry::reg_exe_path(reg_path), &file.display().to_string(), "/y"])?;
    Ok(file)
}
