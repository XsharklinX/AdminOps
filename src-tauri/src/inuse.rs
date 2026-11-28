//! «La cámara la está usando otra aplicación»: cuál. Windows apunta en el registro del usuario, por cada programa, cuándo
//! empezó y cuándo dejó de usar la cámara o el micrófono (CapabilityAccessManager\ConsentStore). Si empezó y aún no ha
//! terminado (`LastUsedTimeStop` = 0), la tiene ahora.

#[derive(serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InUse {
    /// Nombre del programa («zoom.exe», «Microsoft Teams»).
    pub apps: Vec<String>,
}

/// El nombre legible de una entrada del almacén: «C:#Program Files#Zoom#zoom.exe» → «zoom.exe»; las de la Tienda,
/// «MSTeams_8wekyb3d8bbwe» → «MSTeams».
pub fn app_name(key: &str) -> String {
    let last = key.rsplit('#').next().unwrap_or(key);
    // Solo los nombres de la Tienda llevan «_» + editor; un exe («my_app.exe») se queda como está.
    if key.contains('#') || last.to_ascii_lowercase().ends_with(".exe") {
        return last.to_string();
    }
    last.split('_').next().unwrap_or(last).to_string()
}

/// La propia AdminOps (y su WebView2) no cuenta: no es «otra aplicación».
fn is_own(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "adminops.exe" || n == "msedgewebview2.exe"
}

/// ¿Quién usa ahora la cámara («webcam») o el micrófono («microphone»)?
#[tauri::command(async)]
pub fn device_in_use(kind: String) -> Result<InUse, String> {
    if kind != "webcam" && kind != "microphone" {
        return Err("Dispositivo no válido.".into());
    }
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_USERS};
        let base = format!(r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{kind}");
        // El usuario de la sesión, también en modo técnico con la cuenta de otro.
        let root = match crate::target_user::hkcu_redirect() {
            Some(sid) => winreg::RegKey::predef(HKEY_USERS).open_subkey(format!(r"{sid}\{base}")),
            None => winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(&base),
        };
        let Ok(root) = root else { return Ok(InUse::default()) };
        let mut apps = Vec::new();
        let mut check = |k: &winreg::RegKey, name: &str| {
            let start: u64 = k.get_value("LastUsedTimeStart").unwrap_or(0);
            let stop: u64 = k.get_value("LastUsedTimeStop").unwrap_or(1);
            if start > 0 && stop == 0 {
                let n = app_name(name);
                if !is_own(&n) {
                    apps.push(n);
                }
            }
        };
        for name in root.enum_keys().flatten() {
            if name == "NonPackaged" {
                if let Ok(np) = root.open_subkey("NonPackaged") {
                    for n in np.enum_keys().flatten() {
                        if let Ok(k) = np.open_subkey(&n) {
                            check(&k, &n);
                        }
                    }
                }
            } else if let Ok(k) = root.open_subkey(&name) {
                check(&k, &name);
            }
        }
        apps.sort();
        apps.dedup();
        Ok(InUse { apps })
    }
    #[cfg(not(windows))]
    Ok(InUse::default())
}

#[cfg(test)]
mod tests {
    #[test]
    fn nombre_del_programa() {
        assert_eq!(super::app_name("C:#Program Files#Zoom#bin#Zoom.exe"), "Zoom.exe");
        assert_eq!(super::app_name("MSTeams_8wekyb3d8bbwe"), "MSTeams");
        assert_eq!(super::app_name("my_app.exe"), "my_app.exe");
        assert_eq!(super::app_name("C:#Tools#my_app.exe"), "my_app.exe");
    }

    #[test]
    fn la_propia_adminops_no_cuenta() {
        assert!(super::is_own("AdminOps.exe"));
        assert!(super::is_own("msedgewebview2.exe"));
        assert!(!super::is_own("zoom.exe"));
    }
}
