//! Recuerda el tamaño, la posición y si la ventana estaba maximizada.
//!
//! Se guarda con los datos de ESTE equipo (en portable, dentro del USB y por
//! equipo), porque las pantallas cambian de un PC a otro.

use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition, PhysicalSize};

#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq)]
struct Saved {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("window.json")
}

/// ¿Cae la esquina superior izquierda (con margen) dentro de alguna pantalla conectada?
fn visible_on(monitors: &[(i32, i32, u32, u32)], s: &Saved) -> bool {
    let (px, py) = (s.x + 60, s.y + 20);
    monitors.iter().any(|&(mx, my, mw, mh)| px >= mx && py >= my && px < mx + mw as i32 && py < my + mh as i32)
}

/// Aplica el estado guardado y muestra la ventana (se crea oculta para no parpadear).
pub fn restore(app: &tauri::AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let saved: Option<Saved> = std::fs::read_to_string(path(app)).ok().and_then(|t| serde_json::from_str(&t).ok());
    if let Some(s) = saved.filter(|s| s.width >= 800 && s.height >= 500) {
        let monitors: Vec<(i32, i32, u32, u32)> = w
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| (m.position().x, m.position().y, m.size().width, m.size().height))
            .collect();
        let _ = w.set_size(PhysicalSize::new(s.width, s.height));
        // Si esa pantalla ya no está (portátil sin el monitor externo), queda centrada.
        if visible_on(&monitors, &s) {
            let _ = w.set_position(PhysicalPosition::new(s.x, s.y));
        } else {
            let _ = w.center();
        }
        if s.maximized {
            let _ = w.maximize();
        }
    }
    let _ = w.show();
    let _ = w.set_focus();
}

/// Guarda el estado al cerrar. Maximizada o minimizada se conserva el último tamaño normal.
pub fn save(window: &tauri::Window) {
    let app = window.app_handle();
    let mut s: Saved = std::fs::read_to_string(path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    s.maximized = window.is_maximized().unwrap_or(false);
    if !s.maximized {
        if let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) {
            (s.x, s.y, s.width, s.height) = (pos.x, pos.y, size.width, size.height);
        }
    }
    let _ = crate::paths::write_json(&path(app), &s);
}

/// Tamaño de toda la interfaz (Ajustes → Apariencia). Las vistas web de Tickets y del router no se escalan.
#[tauri::command]
pub fn set_ui_zoom(app: tauri::AppHandle, scale: f64) -> Result<(), String> {
    use tauri::Manager;
    if !(0.5..=2.0).contains(&scale) {
        return Err("Tamaño no válido.".into());
    }
    let w = app.get_webview_window("main").ok_or("No se encontró la ventana principal.")?;
    w.set_zoom(scale).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_restores_onto_connected_screens() {
        let screens = [(0, 0, 1920, 1080)];
        let on = Saved { x: 100, y: 100, width: 1280, height: 820, maximized: false };
        let off = Saved { x: 2500, y: 100, ..on };
        assert!(visible_on(&screens, &on));
        assert!(!visible_on(&screens, &off));
        // Segundo monitor a la izquierda (coordenadas negativas).
        assert!(visible_on(&[(0, 0, 1920, 1080), (-1920, 0, 1920, 1080)], &Saved { x: -1800, ..on }));
    }
}
