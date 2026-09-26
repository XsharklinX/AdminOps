//! Detección y solicitud de privilegios de administrador (UAC).
//!
//! En release el ejecutable lleva un manifiesto `requireAdministrator` (ver
//! build.rs), así que Windows pide UAC al abrirlo. En desarrollo corre como
//! usuario normal y la UI ofrece relanzar elevado.

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
        app.exit(0);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("Solo disponible en Windows.".into())
    }
}
