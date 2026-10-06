//! Mini monitor: una ventanita siempre encima con procesador, memoria,
//! temperatura y red, para vigilar el equipo mientras se prueba otra cosa (un
//! juego, un programa pesado) sin tener AdminOps delante.

use tauri::Manager;

const LABEL: &str = "monitor";

/// Abre (o trae al frente) el mini monitor.
pub fn open(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        return w.set_focus().map_err(|e| e.to_string());
    }
    let mut b = tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("index.html".into()))
        .title("AdminOps · monitor")
        .inner_size(250.0, 172.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .theme(Some(tauri::Theme::Dark))
        // La interfaz sabe así que es el monitor y no la aplicación entera.
        .initialization_script("window.__ADMINOPS_MONITOR__ = true;");
    // Las mismas opciones de navegador que la ventana principal (ver quicknote::open).
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    b.build().map(|_| ()).map_err(|e| format!("No se pudo abrir el mini monitor: {e}"))
}

/// Al cerrarse la ventana principal, el monitor se va con ella: si no, AdminOps
/// seguiría abierta sin nada a la vista más que esto.
pub fn close(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
}

#[tauri::command(async)]
pub fn open_mini_monitor(app: tauri::AppHandle) -> Result<(), String> {
    open(&app)
}
