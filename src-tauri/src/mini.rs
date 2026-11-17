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
        .inner_size(MINI_W, MINI_H)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .theme(Some(tauri::Theme::Dark))
        // Esquina inferior derecha, por encima de la barra de tareas.
        .position(mini_x(app), mini_y(app))
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

/// Pantalla principal en píxeles LÓGICOS (origen, ancho, alto). `.position()` del
/// constructor trabaja en lógicos y `Monitor::size()` da físicos: con el escalado
/// de Windows al 125 % o más, sin dividir, la mini quedaba fuera de la pantalla.
fn screen_logical(app: &tauri::AppHandle) -> (f64, f64, f64, f64) {
    match app.primary_monitor().ok().flatten() {
        Some(m) => {
            let k = m.scale_factor().max(0.5);
            let (p, s) = (m.position(), m.size());
            (p.x as f64 / k, p.y as f64 / k, s.width as f64 / k, s.height as f64 / k)
        }
        None => (0.0, 0.0, 1280.0, 720.0),
    }
}

/// Ancho y alto de la mini, y el margen con el borde y con la barra de tareas.
const MINI_W: f64 = 200.0;
const MINI_H: f64 = 260.0;
const MARGIN: f64 = 16.0;
const TASKBAR: f64 = 56.0;

fn mini_x(app: &tauri::AppHandle) -> f64 {
    let (x, _, w, _) = screen_logical(app);
    (x + w - MINI_W - MARGIN).max(x)
}

fn mini_y(app: &tauri::AppHandle) -> f64 {
    let (_, y, _, h) = screen_logical(app);
    (y + h - MINI_H - MARGIN - TASKBAR).max(y)
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
