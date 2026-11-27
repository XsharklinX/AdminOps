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

/// «Volver a modo usuario»: abre AdminOps otra vez como el usuario de la sesión y sin elevar (se le pide al
/// escritorio de Windows, que corre como él) y cierra esta instancia elevada.
#[tauri::command]
pub fn relaunch_as_user(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    crate::shellopen::run_as_user_visible(&exe.to_string_lossy(), "").map_err(|e| format!("No se pudo volver a modo usuario: {e}"))?;
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
