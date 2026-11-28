//! Detección y solicitud de privilegios de administrador (UAC).
//!
//! AdminOps se abre sin administrador («Modo usuario»: cualquier usuario del
//! equipo puede usarla). «Modo técnico» la relanza elevada con UAC
//! (`relaunch_as_admin`) y «Volver a modo usuario» la relanza sin elevar como el
//! usuario de la sesión (`relaunch_as_user`).

#[cfg(windows)]
pub fn is_elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

#[tauri::command]
pub fn is_admin() -> bool {
    is_elevated()
}

/// Lo pasa «Volver a modo usuario» a la instancia nueva: se queda en modo usuario.
const USER_MODE_ARG: &str = "--modo-usuario";

/// «Abrir en modo técnico en este equipo»: un archivo en la carpeta de este equipo (en portable, la del equipo
/// dentro del USB; instalada, la de esta cuenta). Nunca en los datos compartidos, que en portable viajan en el USB
/// y harían pedir el administrador en todos los PC de los usuarios.
fn tech_on_open_file(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("abrir-en-modo-tecnico.txt")
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TechOnOpen {
    /// El ajuste está puesto en este equipo.
    pub on: bool,
    /// Hay que pedir el administrador ahora: puesto, sin elevar y no se viene de «Volver a modo usuario».
    pub relaunch: bool,
}

#[tauri::command]
pub fn tech_on_open(app: tauri::AppHandle) -> TechOnOpen {
    let on = tech_on_open_file(&app).is_file();
    let from_user_mode = std::env::args().any(|a| a == USER_MODE_ARG);
    TechOnOpen { on, relaunch: on && !is_elevated() && !from_user_mode && !crate::audit::active() }
}

#[tauri::command]
pub fn set_tech_on_open(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    let f = tech_on_open_file(&app);
    if on {
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("No se pudo guardar: {e}"))?;
        }
        std::fs::write(&f, b"1").map_err(|e| format!("No se pudo guardar: {e}"))
    } else {
        match std::fs::remove_file(&f) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("No se pudo guardar: {e}")),
            _ => Ok(()),
        }
    }
}

/// «Volver a modo usuario»: abre AdminOps otra vez como el usuario de la sesión y sin elevar (se le pide al
/// escritorio de Windows, que corre como él) y cierra esta instancia elevada.
#[tauri::command]
pub fn relaunch_as_user(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // `--modo-usuario`: la instancia nueva no vuelve a pedir el administrador aunque esté puesto «Abrir en modo técnico».
    crate::shellopen::run_as_user_visible(&exe.to_string_lossy(), USER_MODE_ARG).map_err(|e| format!("No se pudo volver a modo usuario: {e}"))?;
    crate::rocketplus::skip_rocket_behind();
    app.exit(0);
    Ok(())
}

/// Relanza el ejecutable actual con el verbo `runas` y, si el usuario acepta
/// el UAC, cierra esta instancia.
#[tauri::command]
pub fn relaunch_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
        let verb = wide("runas".as_ref());
        let file = wide(exe.as_os_str());

        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute devuelve un valor > 32 si tuvo éxito.
        if (result as isize) <= 32 {
            return Err("El usuario canceló el UAC o no se pudo elevar.".into());
        }
        crate::rocketplus::skip_rocket_behind();
        app.exit(0);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("Solo disponible en Windows.".into())
    }
}
