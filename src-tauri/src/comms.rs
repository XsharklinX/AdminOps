//! Abrir Teams y el Correo como prefiera el técnico: dentro de AdminOps (el
//! portal de siempre, eso lo hace la interfaz), en el navegador o en su
//! aplicación de Windows. Se abren como el usuario normal (`shellopen`), igual
//! que con un doble clic, aunque AdminOps vaya como administrador.

use serde::Serialize;
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

const TEAMS_WEB: &str = "https://teams.microsoft.com";
const MAIL_WEB: &str = "https://outlook.office.com/mail/";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommApps {
    /// Teams instalado (responde a `msteams:`).
    pub teams: bool,
    /// Qué aplicación de correo hay: "outlook" (clásico), "new-outlook" o None.
    pub mail: Option<String>,
}

fn has_protocol(name: &str) -> bool {
    RegKey::predef(HKEY_CLASSES_ROOT).open_subkey(name).map(|k| k.get_raw_value("URL Protocol").is_ok()).unwrap_or(false)
}

fn has_app_path(exe: &str) -> bool {
    let sub = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
    [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].iter().any(|h| RegKey::predef(*h).open_subkey(&sub).is_ok())
}

pub fn detect() -> CommApps {
    let mail = if has_app_path("OUTLOOK.EXE") {
        Some("outlook".to_string())
    } else if has_app_path("olk.exe") || has_protocol("ms-outlook") {
        Some("new-outlook".to_string())
    } else {
        None
    };
    CommApps { teams: has_protocol("msteams"), mail }
}

/// La web: la del portal que tenga configurado el técnico, o la de Microsoft.
fn web_url(app: &tauri::AppHandle, kind: &str) -> String {
    crate::portals::list_portals(app.clone())
        .into_iter()
        .find(|p| p.kind == kind)
        .map(|p| p.url)
        .filter(|u| u.starts_with("https://") || u.starts_with("http://"))
        .unwrap_or_else(|| (if kind == "teams" { TEAMS_WEB } else { MAIL_WEB }).to_string())
}

#[tauri::command(async)]
pub fn comm_apps() -> CommApps {
    detect()
}

/// `kind`: "teams" o "mail". `how`: "browser" o "app".
#[tauri::command(async)]
pub fn open_comm(app: tauri::AppHandle, kind: String, how: String) -> Result<(), String> {
    if kind != "teams" && kind != "mail" {
        return Err("Solo se puede abrir Teams o el Correo.".into());
    }
    match how.as_str() {
        "browser" => crate::shellopen::open(&web_url(&app, &kind)),
        "app" => {
            let found = detect();
            if kind == "teams" {
                if !found.teams {
                    return Err("Teams no está instalado en este equipo. Elige abrirlo en AdminOps o en el navegador (Ajustes → Portales y correo).".into());
                }
                crate::shellopen::open("msteams:")
            } else {
                match found.mail.as_deref() {
                    Some("outlook") => crate::shellopen::open("outlook.exe"),
                    Some(_) if has_app_path("olk.exe") => crate::shellopen::open("olk.exe"),
                    Some(_) => crate::shellopen::open("ms-outlook:"),
                    None => Err("No hay ninguna aplicación de Outlook en este equipo. Elige abrir el Correo en AdminOps o en el navegador (Ajustes → Portales y correo).".into()),
                }
            }
        }
        _ => Err("Forma de abrir no válida.".into()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn detect_does_not_fail() {
        // Solo lee el registro: en cualquier equipo devuelve algo, sin errores.
        let a = super::detect();
        assert!(a.mail.as_deref().is_none_or(|m| m == "outlook" || m == "new-outlook"));
    }
}
