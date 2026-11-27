//! La barra de herramientas del cohete: lo que necesita además de `rocket` (la ventana y su borde) y
//! `rocketplus`: los arreglos rápidos que estén activos, abrir Teams o el correo y Ctrl+Alt+T para sacarla
//! o esconderla desde cualquier programa.

use tauri::Emitter;

pub const BAR_EVENT: &str = "rocket://bar";
/// Las herramientas de la barra, en su orden de fábrica.
/// Las de después de «app» son opcionales: apagadas de fábrica, se encienden en «Personalizar».
pub const TOOLS: [&str; 20] = ["rec", "clip", "speed", "note", "mon", "fix", "card", "help", "teams", "mail", "app", "contacts", "printer", "screens", "wifi", "keyboard", "folders", "lock", "restart", "cmd"];

pub fn valid_tools(list: &[String]) -> bool {
    list.iter().all(|t| TOOLS.contains(&t.as_str())) && {
        let mut seen = std::collections::HashSet::new();
        list.iter().all(|t| seen.insert(t))
    }
}

/// Los arreglos rápidos activos (los mismos del menú del reloj): (id, nombre).
#[tauri::command]
pub fn bar_fixes(app: tauri::AppHandle) -> Vec<(String, String)> {
    let all: std::collections::HashMap<String, String> = crate::tray::tray_fixes_available().into_iter().collect();
    crate::tray::enabled_fixes(&app).into_iter().filter_map(|id| all.get(&id).map(|l| (id.clone(), l.clone()))).collect()
}

/// Teams o el correo, como el usuario.
#[tauri::command]
pub fn bar_open_comm(kind: String) -> Result<(), String> {
    match kind.as_str() {
        "teams" => crate::shellopen::open("msteams:"),
        "mail" => crate::shellopen::open("mailto:"),
        // Outlook clásico o el nuevo, el que haya. Se mira antes qué está registrado: sin programa, Windows dice
        // «abrir» y saca su aviso en inglés en vez de fallar (y ya no se probaría el siguiente).
        "outlook" => match ["outlook", "ms-outlook", "mailto"].into_iter().find(|p| crate::shellopen::protocol_registered(p)) {
            Some(p) => crate::shellopen::open(&format!("{p}:")),
            None => Err("No hay Outlook ni ningún programa de correo instalado en este equipo.".into()),
        },
        _ => Err("No se sabe qué abrir.".into()),
    }
}

/// Pone un archivo en el portapapeles como si se hubiera copiado en el Explorador (CF_HDROP).
#[cfg(windows)]
pub fn copy_file_to_clipboard(path: &std::path::Path) -> Result<(), String> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT};
    use windows_sys::Win32::UI::Shell::DROPFILES;
    const CF_HDROP: u32 = 15;
    // DROPFILES y, detrás, la lista de rutas en UTF-16 terminada en dos ceros.
    let wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().chain([0, 0]).collect();
    let head = std::mem::size_of::<DROPFILES>();
    let total = head + wide.len() * 2;
    // SAFETY: memoria global propia, del tamaño exacto; si SetClipboardData la acepta, pasa a ser del sistema.
    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total);
        if mem.is_null() {
            return Err("Sin memoria para copiar.".into());
        }
        let p = GlobalLock(mem) as *mut u8;
        if p.is_null() {
            GlobalFree(mem);
            return Err("Sin memoria para copiar.".into());
        }
        let df = p as *mut DROPFILES;
        (*df).pFiles = head as u32;
        (*df).fWide = 1;
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, p.add(head), wide.len() * 2);
        GlobalUnlock(mem);
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            GlobalFree(mem);
            return Err("El portapapeles está ocupado por otro programa: vuelve a probar.".into());
        }
        EmptyClipboard();
        let ok = !SetClipboardData(CF_HDROP, mem as HANDLE).is_null();
        CloseClipboard();
        if !ok {
            GlobalFree(mem);
            return Err("Windows no aceptó el archivo en el portapapeles.".into());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn copy_file_to_clipboard(_: &std::path::Path) -> Result<(), String> {
    Err("Solo en Windows.".into())
}

const QA_PKG: &str = "MicrosoftCorporationII.QuickAssist_8wekyb3d8bbwe";

/// ¿Está la Asistencia rápida de la Tienda? (Windows 11 la trae, pero se puede quitar.)
fn quick_assist_installed() -> bool {
    crate::ps::powershell("if (Get-AppxPackage -AllUsers -Name 'MicrosoftCorporationII.QuickAssist' -ErrorAction SilentlyContinue) { 'si' }")
        .map(|s| s.trim() == "si")
        .unwrap_or(false)
}

/// Asistencia rápida de Windows, lista para dar el código. `ms-quick-assist:` solo existe con la aplicación de la
/// Tienda instalada; si no está, se prueba la antigua (quickassist.exe) y, si tampoco, se instala con winget.
/// Devuelve lo que hizo, para decirlo en la barra.
#[tauri::command(async)]
pub fn bar_quick_assist() -> Result<String, String> {
    let open_store = || crate::shellopen::open(&format!("shell:AppsFolder\\{QA_PKG}!App"));
    if crate::shellopen::protocol_registered("ms-quick-assist") {
        crate::shellopen::open("ms-quick-assist:")?;
        return Ok("Asistencia rápida abierta".into());
    }
    if quick_assist_installed() {
        open_store()?;
        return Ok("Asistencia rápida abierta".into());
    }
    if let Some(sys) = std::env::var_os("SystemRoot") {
        let old = std::path::Path::new(&sys).join(r"System32\quickassist.exe");
        if old.exists() {
            crate::shellopen::run_as_user(&old.to_string_lossy(), "")?;
            return Ok("Asistencia rápida abierta".into());
        }
    }
    if !crate::elevation::is_elevated() {
        return Err("Asistencia rápida no está en este equipo. Para instalarla, abre AdminOps como administrador y vuelve a pulsar.".into());
    }
    // Abrirla no cambia nada (SAFE); instalarla sí: con el modo auditoría puesto, no.
    if crate::audit::active() {
        return Err("Asistencia rápida no está en este equipo, y con el modo auditoría puesto AdminOps no instala nada.".into());
    }
    // Instalarla desde la Tienda (id de Asistencia rápida en Microsoft Store) y abrirla.
    crate::ps::exec_opts(
        "winget",
        &["install", "--id", "9P7BP5VNWKX5", "--source", "msstore", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity"],
        crate::ps::Opts { timeout: Some(std::time::Duration::from_secs(300)), task: None },
    )
    .map_err(|e| format!("Asistencia rápida no está en este equipo y no se pudo instalar: {}", crate::ps::clean_error(&e)))?;
    open_store()?;
    Ok("Asistencia rápida estaba quitada: instalada y abierta".into())
}

/// Captura de pantalla: la herramienta de recortes de Windows; si no está su enlace, Win+Mayús+S.
#[tauri::command(async)]
pub fn bar_screen_clip(app: tauri::AppHandle) -> Result<(), String> {
    if crate::shellopen::protocol_registered("ms-screenclip") {
        return crate::quicknote::open_screen_clip(app);
    }
    #[cfg(windows)]
    // SAFETY: entrada de teclado sintética con estructuras propias, como si se pulsara Win+Mayús+S.
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_LWIN, VK_SHIFT};
        let key = |vk: u16, up: bool| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } } };
        let s = u16::from(b'S');
        let seq = [key(VK_LWIN, false), key(VK_SHIFT, false), key(s, false), key(s, true), key(VK_SHIFT, true), key(VK_LWIN, true)];
        if SendInput(seq.len() as u32, seq.as_ptr(), std::mem::size_of::<INPUT>() as i32) == 0 {
            return Err("No se pudo abrir el recorte: pulsa Win+Mayús+S.".into());
        }
    }
    Ok(())
}

/// «Wi-Fi Oficina-5G · 68 %» o «Cable»: lo que dice `netsh wlan show interfaces` (SSID y señal; vale en
/// cualquier idioma porque se buscan «SSID» y el porcentaje).
pub fn network_line(netsh: &str) -> String {
    let mut ssid = None;
    let mut signal = None;
    let mut connected = false;
    for l in netsh.lines() {
        let Some((k, v)) = l.split_once(':') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if k == "SSID" {
            ssid = Some(v.to_string());
        } else if v.ends_with('%') && signal.is_none() {
            signal = Some(v.to_string());
        } else if v.eq_ignore_ascii_case("connected") || v.eq_ignore_ascii_case("conectado") {
            connected = true;
        }
    }
    match (connected, ssid) {
        (true, Some(s)) => format!("Wi-Fi «{s}»{}", signal.map(|x| format!(" · señal {x}")).unwrap_or_default()),
        _ => "Cable (o sin Wi-Fi)".into(),
    }
}

#[tauri::command(async)]
pub fn bar_network() -> String {
    network_line(&crate::ps::exec("netsh", &["wlan", "show", "interfaces"]).unwrap_or_default())
}

/// El Escritorio del usuario con la sesión abierta (no el de la cuenta que elevó AdminOps), también si
/// OneDrive lo tiene redirigido.
fn user_desktop() -> Option<std::path::PathBuf> {
    let u = crate::target_user::user_dirs()?;
    let root = match crate::target_user::hkcu_redirect() {
        Some(sid) => format!(r"HKU\{sid}"),
        None => "HKCU".into(),
    };
    let raw = crate::tweaks::registry::read_string(&format!(r"{root}\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders"), "Desktop");
    let p = raw
        .map(|r| r.replace("%USERPROFILE%", &u.profile.to_string_lossy()))
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| u.profile.join("Desktop"));
    p.is_dir().then_some(p)
}

/// «Grabación 27-11-2026 10.42.mp4»: la fecha y la hora en que se grabó (sin «:», que Windows no deja).
pub fn desktop_name(started: u64) -> String {
    let dt = chrono::DateTime::from_timestamp(started as i64, 0).map(|d| d.with_timezone(&chrono::Local));
    match dt {
        Some(d) => format!("Grabación {}.mp4", d.format("%d-%m-%Y %H.%M")),
        None => "Grabación.mp4".into(),
    }
}

/// «Guardar en el Escritorio»: el vídeo al Escritorio del usuario, con la fecha en el nombre, para que quien
/// se pierde en el equipo sepa dónde está lo que tiene que mandar. Devuelve el nombre del archivo.
#[tauri::command(async)]
pub fn bar_save_desktop(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let d = crate::faultrec::faultrec_get(app, id)?;
    let desk = user_desktop().ok_or("No se encuentra el Escritorio de este usuario.")?;
    let mut name = desktop_name(d.meta.started);
    // Si ya hay uno con ese nombre (dos grabaciones en el mismo minuto), «(2)», «(3)»…
    let mut n = 2;
    while desk.join(&name).exists() {
        name = desktop_name(d.meta.started).replace(".mp4", &format!(" ({n}).mp4"));
        n += 1;
    }
    std::fs::copy(&d.path, desk.join(&name)).map_err(|e| format!("No se pudo guardar en el Escritorio: {e}"))?;
    Ok(name)
}

/// Ctrl+Alt+T: saca o esconde la barra (también con una presentación a pantalla completa).
pub fn start_hotkey(app: tauri::AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
        // Si lo tiene el proceso «solo el cohete», lo suelta al ver abierta esta AdminOps: se reintenta un rato.
        let mut ok = false;
        for _ in 0..15 {
            if RegisterHotKey(std::ptr::null_mut(), 5, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'T')) != 0 {
                ok = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        if !ok {
            log::warn!("No se pudo registrar Ctrl+Alt+T para la barra: lo usa otro programa.");
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY && crate::rocket::force_show(&app) {
                let _ = app.emit(BAR_EVENT, ());
            }
        }
    });
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(test)]
mod tests {
    #[test]
    fn nombre_en_el_escritorio() {
        let n = super::desktop_name(1_764_239_000);
        assert!(n.starts_with("Grabación ") && n.ends_with(".mp4") && !n.contains(':'), "{n}");
    }

    #[test]
    fn red_para_el_ticket() {
        let wifi = "    Nombre                 : Wi-Fi\n    Estado                 : conectado\n    SSID                   : Oficina-5G\n    Señal                  : 68%\n";
        assert_eq!(super::network_line(wifi), "Wi-Fi «Oficina-5G» · señal 68%");
        assert_eq!(super::network_line(""), "Cable (o sin Wi-Fi)");
    }

    #[test]
    fn herramientas_validas() {
        assert!(super::valid_tools(&["rec".into(), "app".into()]));
        assert!(!super::valid_tools(&["rec".into(), "rec".into()]));
        assert!(!super::valid_tools(&["borrar".into()]));
        assert!(super::valid_tools(&[]));
    }
}
