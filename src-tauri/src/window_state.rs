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

/// Ajustes → General: la X minimiza en vez de cerrar. En memoria, porque se
/// consulta al cerrar la ventana y ahí no conviene leer un archivo.
static CLOSE_MINIMIZES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Se ha pedido salir de verdad (botón «Salir»): la X ya no se intercepta.
static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_close_minimizes(on: bool) {
    CLOSE_MINIMIZES.store(on, std::sync::atomic::Ordering::SeqCst);
}

/// ¿Hay que minimizar en vez de cerrar?
pub fn minimizes_on_close() -> bool {
    use std::sync::atomic::Ordering::SeqCst;
    CLOSE_MINIMIZES.load(SeqCst) && !QUITTING.load(SeqCst)
}

/// Cierra AdminOps de verdad, aunque la X esté puesta para minimizar.
#[tauri::command]
pub fn quit_app(window: tauri::Window) {
    save(&window);
    quit(window.app_handle());
}

/// Salida de verdad, desde el botón «Salir» o desde el icono junto al reloj.
pub fn quit(app: &tauri::AppHandle) {
    QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
    log::info!("Salida pedida («Salir»)");
    app.exit(0);
}

/// ¿Se ha enseñado ya la ventana? (se enseña una sola vez, desde donde llegue antes).
static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Aplica el tamaño y la posición guardados, sin enseñar la ventana todavía.
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
}

/// ¿Ha dado señales de vida la interfaz? (la ventana puede estar ya visible sin
/// que WebView2 haya cargado nada: es justo lo que se quiere detectar).
static READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// ¿Ha empezado siquiera a ejecutarse el código de la interfaz? Se avisa antes
/// de pintar nada: en un equipo lento puede tardar bastante en llegar de aquí a
/// la primera pantalla, y eso **no** es un fallo.
static BOOTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Espera a que la interfaz esté pintada (o a que pase `max`). Para lo que se
/// prepara en segundo plano al abrir: mientras WebView2 arranca, cada PowerShell
/// que se lanza le quita disco y procesador, y en un arranque lento (desde un
/// pendrive, o la primera vez tras instalar) eso lo alargaba todavía más.
pub fn wait_until_ready(max: std::time::Duration) {
    let start = std::time::Instant::now();
    while !READY.load(std::sync::atomic::Ordering::SeqCst) && start.elapsed() < max {
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// Enseña la ventana (una sola vez). Se llama cuando la interfaz ya está
/// pintada, o pasado el margen del vigilante si tarda más de la cuenta.
pub fn reveal(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering::SeqCst;
    if SHOWN.swap(true, SeqCst) {
        return;
    }
    let Some(w) = app.get_webview_window("main") else { return };
    let _ = w.show();
    // Arrancada con Windows va minimizada: no se le roba el foco al usuario.
    if !std::env::args().any(|a| a == "--minimized") {
        let _ = w.set_focus();
    }
    log::info!("Ventana visible a los {} ms", crate::boottime::since_start_ms());
}

/// La interfaz ya pintó su primera página: se enseña la ventana con contenido,
/// nunca vacía.
#[tauri::command]
pub fn ui_ready(app: tauri::AppHandle) {
    READY.store(true, std::sync::atomic::Ordering::SeqCst);
    BOOTING.store(true, std::sync::atomic::Ordering::SeqCst);
    reveal(&app);
}

/// El código de la interfaz ha empezado a ejecutarse. Todavía no hay nada
/// pintado, pero WebView2 está vivo y cargando: a partir de aquí, por mucho que
/// tarde, no hay que recargar nada.
#[tauri::command]
pub fn ui_booting() {
    BOOTING.store(true, std::sync::atomic::Ordering::SeqCst);
    log::info!("La interfaz empezó a cargar a los {} ms", crate::boottime::since_start_ms());
}

/// Tope de espera antes de enseñar la ventana aunque la interfaz no haya
/// avisado. Es una red de seguridad, no el camino normal: existe para que un
/// equipo donde algo va muy mal no se quede sin ninguna ventana.
const REVEAL_ANYWAY: std::time::Duration = std::time::Duration::from_secs(20);
/// Cada cuánto se comprueba si la interfaz ya pintó.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_millis(250);
/// Sin **ninguna** señal de la interfaz pasado este tiempo, se recarga la vista
/// una vez. Generoso a propósito: en un equipo viejo el código de la interfaz
/// puede tardar diez segundos largos en arrancar, y recargar entonces sería
/// empezar de cero y dejarlo peor de lo que estaba.
const RELOAD_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// Vigilante del arranque de la interfaz.
///
/// **La ventana se enseña cuando hay algo pintado, no antes.** Enseñarla a los
/// dos segundos y medio pasara lo que pasara era lo que producía el rectángulo
/// negro del que se quejaban los técnicos: si WebView2 aún no ha pintado nada,
/// lo único que se ve es el color de fondo de una ventana vacía, y eso parece
/// una aplicación colgada. Mejor tardar un segundo más y aparecer entera.
///
/// Dos redes de seguridad, las dos generosas a propósito:
/// - a los 20 s se enseña igualmente, para que nunca se quede sin ventana;
/// - a los 25 s se recarga la vista **solo si WebView2 no ha cargado nada**. Si
///   el código de la interfaz llegó a arrancar (`ui_booting`), por lento que vaya
///   el equipo se le deja terminar: recargar a un equipo viejo que solo iba
///   despacio es tirar el trabajo hecho y empezar otra vez.
pub fn watch_first_paint(app: tauri::AppHandle) {
    use std::sync::atomic::Ordering::SeqCst;
    std::thread::spawn(move || {
        let inicio = std::time::Instant::now();
        while inicio.elapsed() < REVEAL_ANYWAY {
            if READY.load(SeqCst) {
                return; // la enseñó `ui_ready`, ya dibujada
            }
            std::thread::sleep(CHECK_EVERY);
        }
        log::warn!("La interfaz no avisó en {} s: se enseña la ventana igualmente.", REVEAL_ANYWAY.as_secs());
        reveal(&app);
        if BOOTING.load(SeqCst) {
            return;
        }
        std::thread::sleep(RELOAD_AFTER - REVEAL_ANYWAY);
        if BOOTING.load(SeqCst) {
            return;
        }
        log::warn!("WebView2 no cargó nada en {} s: se recarga la vista una vez.", RELOAD_AFTER.as_secs());
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.reload();
        }
    });
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
