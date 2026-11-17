//! AdminOps en miniatura: una ventanita siempre encima con el caso de ahora,
//! sus arreglos rápidos y el reloj. Ctrl+Alt+A la abre o la cierra.
//!
//! Al abrir la mini se minimiza la ventana principal; al volver a abrir la
//! entera, la mini se cierra.

use tauri::Manager;

const LABEL: &str = "mini";
/// Ctrl+Alt+A.
pub const SHORTCUT: &str = "Ctrl+Alt+A";

/// Abre (o trae al frente) la ventana mini y minimiza la principal.
pub fn open(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        return w.set_focus().map_err(|e| e.to_string());
    }
    // Minimizar la principal antes de crear la mini para no tapar nada.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.minimize();
    }
    let mut b = tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("index.html".into()))
        .title("AdminOps · mini")
        .inner_size(200.0, 260.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .theme(Some(tauri::Theme::Dark))
        // Esquina inferior derecha, con margen para no pegarse al borde.
        .position(get_mini_x(app) - 216.0, get_mini_y(app) - 276.0)
        .disable_drag_drop_handler()
        .initialization_script("window.__ADMINOPS_MINI__ = true;");
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    b.build().map(|_| ()).map_err(|e| format!("No se pudo abrir la mini: {e}"))
}

/// Cierra la mini y restaura la ventana principal.
pub fn close(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

/// Alterna la mini: si está abierta la cierra (y vuelve la principal); si no,
/// la abre.
pub fn toggle(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window(LABEL).is_some() {
        close(app);
        Ok(())
    } else {
        open(app)
    }
}

fn get_mini_x(app: &tauri::AppHandle) -> f64 {
    app.primary_monitor().ok().flatten().map(|m| m.size().width as f64).unwrap_or(1920.0)
}

fn get_mini_y(app: &tauri::AppHandle) -> f64 {
    app.primary_monitor().ok().flatten().map(|m| m.size().height as f64).unwrap_or(1080.0)
}

/// Abrir la mini desde la propia aplicación (menú "⋯" o barra del caso).
#[tauri::command(async)]
pub fn open_adminops_mini(app: tauri::AppHandle) -> Result<(), String> {
    open(&app)
}

/// Cerrar la mini desde la propia mini (botón "Abrir entera").
#[tauri::command]
pub fn close_adminops_mini(app: tauri::AppHandle) {
    close(&app);
}

/// Registra el atajo global Ctrl+Alt+A.
pub fn start(app: tauri::AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};
        if RegisterHotKey(std::ptr::null_mut(), 2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'A')) == 0 {
            log::warn!("No se pudo registrar {SHORTCUT} para la mini: lo usa otro programa.");
            return;
        }
        log::info!("Mini: {SHORTCUT} listo");
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                if let Err(e) = toggle(&app) {
                    log::warn!("{e}");
                }
            }
        }
    });
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(test)]
mod tests {
    #[test]
    fn label_is_short() {
        assert_eq!(super::LABEL, "mini");
    }
}
