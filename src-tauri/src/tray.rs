//! Icono junto al reloj (Ajustes → General, apagado de fábrica): lo habitual a
//! un clic derecho sin tener la ventana delante. Con «Al cerrar la ventana,
//! minimizar» activado, la X la esconde aquí en vez de dejarla en la barra de tareas.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

const ID: &str = "adminops";
/// ¿Está el icono a la vista? Lo consulta el cierre de la ventana.
static VISIBLE: AtomicBool = AtomicBool::new(false);

pub fn visible() -> bool {
    VISIBLE.load(Ordering::SeqCst)
}

/// Trae la ventana principal al frente, esté minimizada u oculta.
pub fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Lo que se pide desde el menú y hace la interfaz (un análisis, abrir la campana).
fn ask_ui(app: &tauri::AppHandle, action: &str) {
    show_main(app);
    let _ = app.emit("tray-action", action);
}

fn build(app: &tauri::AppHandle) -> tauri::Result<()> {
    let item = |id: &str, text: &str| MenuItem::with_id(app, id, text, true, None::<&str>);
    let menu = Menu::with_items(
        app,
        &[
            &item("open", "Abrir AdminOps")?,
            &PredefinedMenuItem::separator(app)?,
            &item("quick-scan", "Análisis rápido")?,
            &item("note", "Nota de llamada")?,
            &item("monitor", "Mini monitor")?,
            &item("alerts", "Avisos")?,
            &PredefinedMenuItem::separator(app)?,
            &item("quit", "Salir de AdminOps")?,
        ],
    )?;
    let mut tray = TrayIconBuilder::with_id(ID)
        .tooltip("AdminOps")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "note" => {
                if let Err(e) = crate::quicknote::open(app) {
                    log::warn!("{e}");
                }
            }
            "monitor" => {
                if let Err(e) = crate::minimon::open(app) {
                    log::warn!("{e}");
                }
            }
            "quit" => crate::window_state::quit(app),
            other => ask_ui(app, other),
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app).map(|_| ())
}

/// Enseña o quita el icono. Se crea la primera vez que se pide.
pub fn set_visible(app: &tauri::AppHandle, on: bool) {
    let shown = match app.tray_by_id(ID) {
        Some(tray) => tray.set_visible(on).is_ok() && on,
        None if on => match build(app) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("No se pudo poner el icono junto al reloj: {e}");
                false
            }
        },
        None => false,
    };
    VISIBLE.store(shown, Ordering::SeqCst);
    // Sin icono no hay de dónde volver a sacar una ventana escondida.
    if !shown {
        if let Some(w) = app.get_webview_window("main") {
            if !w.is_visible().unwrap_or(true) {
                let _ = w.show();
            }
        }
    }
}
