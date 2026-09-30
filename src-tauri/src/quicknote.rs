//! Nota de llamada: Ctrl+Alt+N, aunque AdminOps esté minimizado.
//!
//! Suena el teléfono mientras estás con otra cosa. Con el atajo sale una
//! ventana pequeña, siempre encima, para apuntar quién llama, desde qué equipo y
//! qué le pasa; con un clic se convierte en un caso o en un seguimiento. Es lo
//! que hoy acaba en una libreta o en la memoria.
//!
//! El atajo se registra con `RegisterHotKey` de Windows, sin plugins: si otro
//! programa ya lo usa, se dice en el registro y la nota se sigue pudiendo abrir
//! desde AdminOps (Ctrl+K → «Nota de llamada»).

use tauri::Manager;

const LABEL: &str = "note";
/// Ctrl+Alt+N.
pub const SHORTCUT: &str = "Ctrl+Alt+N";

/// Abre (o trae al frente) la ventana de la nota.
pub fn open(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        return w.set_focus().map_err(|e| e.to_string());
    }
    let mut b = tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("index.html".into()))
        .title("Nota de llamada · AdminOps")
        .inner_size(400.0, 470.0)
        .resizable(false)
        .always_on_top(true)
        .center()
        .theme(Some(tauri::Theme::Dark))
        // La interfaz sabe así que es la nota y no la aplicación entera.
        .initialization_script("window.__ADMINOPS_NOTE__ = true;");
    // Las mismas opciones de navegador que la ventana principal: WebView2 no
    // admite dos configuraciones distintas en la misma carpeta de datos y, si no
    // coinciden, la ventana no llega a cargar (ver portals::browser_args).
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    b.build().map(|_| ()).map_err(|e| format!("No se pudo abrir la nota: {e}"))
}

/// Registra el atajo global en un hilo propio con su bucle de mensajes (así lo
/// exige `RegisterHotKey` cuando no hay ventana asociada).
pub fn start(app: tauri::AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
        if RegisterHotKey(std::ptr::null_mut(), 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'N')) == 0 {
            log::warn!("No se pudo registrar {SHORTCUT} para la nota de llamada: lo usa otro programa.");
            return;
        }
        log::info!("Nota de llamada: {SHORTCUT} listo");
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                if let Err(e) = open(&app) {
                    log::warn!("{e}");
                }
            }
        }
    });
    #[cfg(not(windows))]
    let _ = app;
}

/// Abrir la nota desde la propia aplicación (Ctrl+K, la barra del caso).
#[tauri::command(async)]
pub fn open_quick_note(app: tauri::AppHandle) -> Result<(), String> {
    open(&app)
}

/// Cerrar la nota (lo pide la propia nota al terminar).
#[tauri::command]
pub fn close_quick_note(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
}

/// Abre el recorte de pantalla de Windows (Win+Mayús+S). Lo que se recorta
/// queda en el portapapeles y, en cuanto llega, se le tapan con el OCR de
/// Windows las rutas y los nombres de usuario y de equipo (ver `ocr`). La
/// interfaz recibe «screen-clip» con lo que se tapó.
#[tauri::command]
pub fn open_screen_clip(app: tauri::AppHandle) -> Result<(), String> {
    if !crate::shellopen::protocol_registered("ms-screenclip") {
        return Err("Este equipo no tiene la herramienta de recortes de Windows. Prueba con Win+Mayús+S, o con Impr Pant.".into());
    }
    #[cfg(windows)]
    let before = crate::ocr::clipboard_sequence();
    crate::shellopen::open("ms-screenclip:")?;
    #[cfg(windows)]
    std::thread::spawn(move || {
        use tauri::Emitter;
        // Dos minutos para recortar; el primer cambio del portapapeles es el recorte.
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while std::time::Instant::now() < limit {
            std::thread::sleep(std::time::Duration::from_millis(300));
            if crate::ocr::clipboard_sequence() != before {
                // La herramienta pone varios formatos seguidos: se le deja terminar.
                std::thread::sleep(std::time::Duration::from_millis(500));
                let r = crate::ocr::redact_clipboard();
                if r.error != crate::ocr::NO_IMAGE {
                    log::info!("Recorte: {} de {} palabras tapadas{}", r.covered, r.words, if r.error.is_empty() { String::new() } else { format!(" ({})", r.error) });
                    let _ = app.emit("screen-clip", &r);
                }
                return;
            }
        }
    });
    Ok(())
}

/// Tapa los datos personales de la imagen que haya ahora en el portapapeles
/// (un recorte hecho con Win+Mayús+S fuera de AdminOps, una captura…).
#[tauri::command(async)]
pub fn redact_clipboard_image() -> Result<crate::ocr::Redacted, String> {
    #[cfg(windows)]
    {
        let r = crate::ocr::redact_clipboard();
        if r.error.is_empty() {
            Ok(r)
        } else {
            Err(r.error)
        }
    }
    #[cfg(not(windows))]
    Err("Solo en Windows.".into())
}
