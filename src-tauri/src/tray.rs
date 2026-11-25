//! Icono junto al reloj (Ajustes → General, apagado de fábrica): lo habitual a
//! un clic derecho sin tener la ventana delante. Con «Al cerrar la ventana,
//! minimizar» activado, la X la esconde aquí en vez de dejarla en la barra de tareas.
//!
//! Desde la bandeja se pueden lanzar arreglos rápidos (cola de impresión, IP,
//! Explorador, temporales…). Los que salen se eligen en Ajustes → General.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

const ID: &str = "adminops";
/// ¿Está el icono a la vista? Lo consulta el cierre de la ventana.
static VISIBLE: AtomicBool = AtomicBool::new(false);
/// Ya se avisó en esta sesión de dónde está AdminOps al esconderla.
static TOLD: AtomicBool = AtomicBool::new(false);
/// Hay un arreglo de la bandeja en marcha.
static RUNNING_FIX: AtomicBool = AtomicBool::new(false);

/// Arreglos rápidos disponibles en la bandeja. El id es el del catálogo,
/// salvo «repair.network-renew», que aquí significa «repair.ip-renew»: solo
/// renueva la IP (nunca el «Restablecer red» completo, que quita la IP fija).
const FIXES: &[(&str, &str)] = &[
    ("repair.print-queue", "Vaciar cola de impresión"),
    ("repair.network-renew", "Renovar la IP"),
    ("repair.explorer", "Reiniciar Explorador"),
    ("cleanup.user-temp", "Limpiar temporales"),
    ("cleanup.dns-cache", "Vaciar caché DNS"),
];

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

/// Qué arreglos rápidos están activados en Ajustes → General.
fn enabled_fixes(app: &tauri::AppHandle) -> Vec<String> {
    let settings = crate::workflow::settings(app);
    match settings.tray_fixes {
        // Sin configurar: los cuatro de fábrica.
        None => FIXES.iter().take(4).map(|(id, _)| id.to_string()).collect(),
        // Elegidos en Ajustes (vacío = ninguno).
        Some(list) => list.into_iter().filter(|id| FIXES.iter().any(|(f, _)| f == id)).collect(),
    }
}

/// Crea el menú completo con los arreglos elegidos.
fn build_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>, fixes: &[String]) -> tauri::Result<Menu<R>> {
    let item = |id: &str, text: &str| MenuItem::with_id(app, id, text, true, None::<&str>);
    let open = item("open", "Abrir AdminOps")?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let quick = item("quick-scan", "Análisis rápido")?;
    let note = item("note", "Nota de llamada")?;
    let monitor = item("monitor", "Mini monitor")?;
    let alerts = item("alerts", "Avisos")?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = item("quit", "Salir de AdminOps")?;
    let mut fix_items: Vec<MenuItem<R>> = Vec::new();
    for id in fixes {
        if let Some((_, label)) = FIXES.iter().find(|(f, _)| *f == id) {
            fix_items.push(item(&format!("fix:{id}"), label)?);
        }
    }
    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![&open, &sep1, &quick, &note, &monitor, &alerts, &sep2];
    for f in &fix_items {
        refs.push(f);
    }
    if !fix_items.is_empty() {
        refs.push(&sep3);
    }
    refs.push(&quit);
    Menu::with_items(app, &refs)
}

fn build(app: &tauri::AppHandle) -> tauri::Result<()> {
    let fixes = enabled_fixes(app);
    let menu = build_menu(app, &fixes)?;
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
            "quit" => {
                if crate::paths::is_portable() {
                    // El diálogo de salida vive en la ventana principal: que se vea.
                    show_main(app);
                    let _ = app.emit("tray-action", "portable-exit");
                } else {
                    crate::window_state::quit(app);
                }
            }
            other => {
                if let Some(id) = other.strip_prefix("fix:") {
                    // PowerShell puede tardar (renovar la IP, 10-30 s): fuera del hilo de la
                    // ventana, o toda la aplicación se queda en «No responde» mientras tanto.
                    let (app, id) = (app.clone(), id.to_string());
                    std::thread::spawn(move || {
                        if let Err(e) = run_fix_from_tray(&app, &id) {
                            log::warn!("{e}");
                        }
                    });
                } else {
                    ask_ui(app, other);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    match app.default_window_icon() {
        Some(icon) => tray = tray.icon(icon.clone()),
        // Sin imagen, Windows pone un hueco transparente: está, pero no se ve.
        None => log::warn!("Icono junto al reloj: no hay imagen del programa; saldrá en blanco"),
    }
    tray.build(app).map(|_| ())
}

/// Vuelve a construir el menú cuando cambian los ajustes (Ajustes → General).
pub fn refresh(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id(ID) {
        let fixes = enabled_fixes(app);
        if let Ok(menu) = build_menu(app, &fixes) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Suelta el «hay un arreglo en marcha» al salir, pase lo que pase (también si revienta).
struct FixRunning;
impl Drop for FixRunning {
    fn drop(&mut self) {
        RUNNING_FIX.store(false, Ordering::SeqCst);
    }
}

/// Ejecuta un arreglo rápido desde la bandeja (o la mini), uno a la vez.
fn run_fix_from_tray(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    if RUNNING_FIX.swap(true, Ordering::SeqCst) {
        let _ = crate::toast::show(app, "AdminOps", "Ya hay un arreglo en marcha: espera a que termine.", &[], move |_| {});
        return Err("Ya hay un arreglo en marcha: espera a que termine.".into());
    }
    let _running = FixRunning;
    run_fix_now(app, id)
}

/// El id del catálogo de ajustes para un arreglo de la bandeja: casi todos son el mismo,
/// salvo «repair.network-renew», que es «repair.ip-renew».
pub(crate) fn catalog_id(id: &str) -> &str {
    match id {
        "repair.network-renew" => "repair.ip-renew",
        _ => id,
    }
}

/// El arreglo en sí, con sus avisos.
fn run_fix_now(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let catalog_id = catalog_id(id);
    let label = FIXES.iter().find(|(f, _)| *f == id).map(|(_, l)| *l).unwrap_or(id);

    // El modo auditoría bloquea cualquier cambio; el aviso lo explica sin intentar nada.
    if crate::audit::active() {
        let body = format!("{label}: Modo auditoría activo · no se cambia nada");
        let _ = crate::toast::show(app, "AdminOps", &body, &[], move |_| {});
        return Ok(());
    }

    let state = app.state::<crate::tweaks::TweakState>();

    // Si el arreglo necesita administrador y AdminOps no lo es, se avisa y se ofrece
    // abrir la ventana principal para que el técnico reinicie como administrador.
    if state.tweak_needs_admin(catalog_id) && !crate::elevation::is_elevated() {
        let body = format!("{label} necesita ejecutar AdminOps como administrador.");
        let showed = crate::toast::show(
            app,
            "AdminOps",
            &body,
            &[("Abrir AdminOps", "open".into())],
            {
                let app = app.clone();
                move |arg| {
                    if arg.as_deref() == Some("open") {
                        show_main(&app);
                    }
                }
            },
        );
        if !showed {
            // Si el aviso interactivo no funciona, al menos se abre la ventana.
            show_main(app);
        }
        return Ok(());
    }

    let result = state.run_catalog(catalog_id);
    let (title, body) = match &result {
        Ok(msg) => ("AdminOps", format!("{label}: {msg}")),
        Err(err) => ("AdminOps", format!("{label}: falló · {err}")),
    };
    let ok = result.is_ok();
    let _ = crate::toast::show(app, title, &body, &[], move |_| {});
    if ok { Ok(()) } else { result.map(|_| ()) }
}

/// Comando que la interfaz puede llamar para ejecutar un arreglo de la bandeja
/// (también se usan desde la mini ventana).
#[tauri::command(async)]
pub fn tray_run_fix(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if !FIXES.iter().any(|(f, _)| f == &id) {
        return Err("Arreglo no disponible en la bandeja.".into());
    }
    run_fix_from_tray(&app, &id)
}

/// Lista de arreglos que puede mostrar la bandeja, con su etiqueta.
#[tauri::command]
pub fn tray_fixes_available() -> Vec<(String, String)> {
    FIXES.iter().map(|(id, label)| (id.to_string(), label.to_string())).collect()
}

/// Ids de los arreglos de la bandeja, para validar los ajustes guardados.
pub fn fixes_available_ids() -> std::collections::HashSet<String> {
    FIXES.iter().map(|(id, _)| id.to_string()).collect()
}

/// La primera vez que la X esconde la ventana, se dice dónde ha ido: Windows 11
/// mete los iconos nuevos en los ocultos (la flecha «^») y, sin aviso, parecía
/// que AdminOps se había cerrado.
pub fn tell_where(app: &tauri::AppHandle) {
    if TOLD.swap(true, Ordering::SeqCst) {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("AdminOps sigue abierta")
        .body("Está junto al reloj. Si no ves el icono, pulsa la flecha «^» de al lado y arrástralo a la barra para tenerlo siempre a la vista.")
        .show();
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
    log::info!("Icono junto al reloj: {}", if shown { "puesto" } else { "quitado" });
    // Sin icono no hay de dónde volver a sacar una ventana escondida.
    if !shown {
        if let Some(w) = app.get_webview_window("main") {
            if !w.is_visible().unwrap_or(true) {
                let _ = w.show();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_have_labels() {
        assert_eq!(FIXES.len(), tray_fixes_available().len());
        assert!(FIXES.iter().all(|(_, label)| !label.is_empty()));
    }

    #[test]
    fn network_renew_maps_to_reset() {
        let ids: std::collections::HashSet<String> = fixes_available_ids();
        assert!(ids.contains("repair.network-renew"));
    }
}
