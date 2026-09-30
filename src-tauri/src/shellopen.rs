//! Abrir enlaces y protocolos (msteams:, mailto:, tel:, https:, ms-settings:)
//! como el usuario normal aunque AdminOps se ejecute como administrador.
//!
//! Un programa elevado que abre Teams o el navegador los abriría también como
//! administrador (Teams ni siquiera arranca así). Pasarle el enlace a
//! `explorer.exe` tampoco sirve: con enlaces con parámetros («?users=…») no lo
//! entiende y abre Documentos. Lo correcto, según Microsoft, es pedírselo al
//! propio escritorio de Windows, que corre como el usuario: es exactamente lo
//! que pasa al hacer doble clic en un enlace.

/// Abre `uri` con el programa asociado, sin permisos de administrador.
pub fn open(uri: &str) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return shell_execute(uri);
    }
    let owned = uri.to_string();
    // COM en un hilo propio (apartamento de un solo hilo, como pide el Shell).
    let r = std::thread::spawn(move || via_desktop(&owned)).join().unwrap_or_else(|_| Err("fallo interno".into()));
    match r {
        Ok(()) => Ok(()),
        Err(e) => {
            log::warn!("Abrir sin elevar falló ({e}); se usa el Explorador.");
            std::process::Command::new("explorer.exe").arg(uri).spawn().map(|_| ()).map_err(|e| e.to_string())
        }
    }
}

/// ¿Hay un programa registrado para este protocolo (msteams:, ms-settings:…)?
///
/// Se comprueba antes de abrirlo: si no lo hay, Windows saca su propio aviso en
/// inglés («This file does not have an app associated with it») en vez de dejar
/// que AdminOps explique qué pasa.
///
/// No basta con que exista la clave del protocolo. En Windows 11 queda la clave
/// de la Asistencia rápida, con su valor `URL Protocol`, **sin ninguna subclave**
/// cuando la app de la Store no está: para Windows el protocolo «existe» pero no
/// hay nada que lo abra. Por eso se exige además el `shell\open\command`, que es
/// lo que de verdad lanza el programa.
pub fn protocol_registered(scheme: &str) -> bool {
    #[cfg(windows)]
    {
        let hkcr = winreg::RegKey::predef(winreg::enums::HKEY_CLASSES_ROOT);
        let Ok(key) = hkcr.open_subkey(scheme) else { return false };
        if key.get_raw_value("URL Protocol").is_err() {
            return false;
        }
        // Las apps de la Store usan `DelegateExecute` y dejan el valor por
        // defecto vacío, así que lo que se mira es que la clave exista.
        key.open_subkey(r"shell\open\command").is_ok()
    }
    #[cfg(not(windows))]
    {
        let _ = scheme;
        false
    }
}

#[cfg(windows)]
fn shell_execute(uri: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (verb, file) = (wide("open"), wide(uri));
    let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
    // Mayor que 32: éxito (documentado así por Windows).
    if r as isize > 32 {
        Ok(())
    } else {
        Err(format!("Windows no tiene ningún programa para abrir esto (código {}).", r as isize))
    }
}

/// El Shell del escritorio de Windows (corre como el usuario, sin elevar).
#[cfg(windows)]
fn desktop_shell() -> Result<windows::Win32::UI::Shell::IShellDispatch2, String> {
    use windows::core::Interface;
    use windows::Win32::System::Com::{CoCreateInstance, IDispatch, IServiceProvider, CLSCTX_LOCAL_SERVER};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Shell::{
        IShellBrowser, IShellFolderViewDual, IShellView, IShellWindows, SID_STopLevelBrowser, ShellWindows, SVGIO_BACKGROUND, SWC_DESKTOP, SWFO_NEEDDISPATCH,
    };
    let e = |what: &'static str| move |err: windows::core::Error| format!("{what}: {err}");
    unsafe {
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER).map_err(e("ShellWindows"))?;
        let desktop_loc = VARIANT::from(0i32); // CSIDL_DESKTOP
        let empty = VARIANT::default();
        let mut hwnd = 0i32;
        let disp = windows.FindWindowSW(&desktop_loc, &empty, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH).map_err(e("escritorio"))?;
        let provider: IServiceProvider = disp.cast().map_err(e("servicios"))?;
        let browser: IShellBrowser = provider.QueryService(&SID_STopLevelBrowser).map_err(e("navegador del escritorio"))?;
        let view: IShellView = browser.QueryActiveShellView().map_err(e("vista"))?;
        // Primero como IDispatch y luego la interfaz concreta (así lo expone el Shell).
        let view_disp: IDispatch = view.GetItemObject(SVGIO_BACKGROUND).map_err(e("vista de carpeta"))?;
        let folder_view: IShellFolderViewDual = view_disp.cast().map_err(e("vista de carpeta (dual)"))?;
        folder_view.Application().map_err(e("Shell"))?.cast().map_err(e("Shell2"))
    }
}

/// Con COM iniciado en este hilo (apartamento de un solo hilo, como pide el Shell).
#[cfg(windows)]
fn with_com<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let r = f();
        if init.is_ok() {
            CoUninitialize();
        }
        r
    }
}

#[cfg(windows)]
fn via_desktop(uri: &str) -> Result<(), String> {
    use windows::core::BSTR;
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    with_com(|| {
        let shell = desktop_shell()?;
        unsafe {
            shell
                .ShellExecute(&BSTR::from(uri), &VARIANT::default(), &VARIANT::default(), &VARIANT::from(BSTR::from("open")), &VARIANT::from(SW_SHOWNORMAL.0))
                .map_err(|e| format!("abrir: {e}"))
        }
    })
}

#[cfg(not(windows))]
fn shell_execute(_: &str) -> Result<(), String> {
    Err("Solo disponible en Windows.".into())
}

#[cfg(not(windows))]
fn via_desktop(_: &str) -> Result<(), String> {
    Err("Solo disponible en Windows.".into())
}

#[cfg(test)]
mod tests {
    /// Se llega al escritorio de Windows (sin abrir nada): `cargo test desktop_shell -- --ignored`
    #[test]
    #[ignore]
    fn desktop_shell_reachable() {
        super::with_com(super::desktop_shell).unwrap();
    }

    /// Un protocolo sin programa que lo abra no cuenta como registrado.
    /// `https` siempre lo está; `adminops-inventado` nunca.
    #[test]
    fn protocol_needs_a_real_handler() {
        assert!(super::protocol_registered("https"), "https debería tener programa asociado");
        assert!(!super::protocol_registered("adminops-protocolo-inventado"));
    }
}
