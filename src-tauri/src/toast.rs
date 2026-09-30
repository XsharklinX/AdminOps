//! Avisos de Windows con botones («Hecho», «Mañana»).
//!
//! El plugin de notificaciones de Tauri no tiene botones en escritorio. Aquí se
//! usa la misma biblioteca que usa él por debajo, con el mismo identificador de
//! aplicación, y se escucha qué botón se pulsa. Funciona mientras AdminOps está
//! abierto (lo normal: vive en la bandeja). Si Windows no deja mostrarlo, se
//! cae al aviso de siempre, sin botones.

use tauri_winrt_notification::Toast;

/// El mismo criterio que el plugin: el identificador de la app, salvo al
/// ejecutarlo desde `target\debug` o `target\release` (sin instalar), donde
/// Windows solo acepta el de PowerShell.
fn app_id(app: &tauri::AppHandle) -> String {
    let dir = tauri::utils::platform::current_exe().ok().and_then(|e| e.parent().map(|p| p.display().to_string())).unwrap_or_default();
    let sep = std::path::MAIN_SEPARATOR;
    if dir.ends_with(&format!("{sep}target{sep}debug")) || dir.ends_with(&format!("{sep}target{sep}release")) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    }
}

/// Muestra un aviso con botones. `on_action` recibe el argumento del botón
/// pulsado, o None si se pulsó el aviso en sí. Devuelve false si Windows no lo
/// mostró (y entonces conviene el aviso normal).
pub fn show<F>(app: &tauri::AppHandle, title: &str, body: &str, buttons: &[(&str, String)], on_action: F) -> bool
where
    F: Fn(Option<String>) + Send + 'static,
{
    let mut toast = Toast::new(&app_id(app)).title(title).text1(&body.chars().take(240).collect::<String>());
    // La biblioteca no escapa los botones: solo textos fijos y argumentos sin
    // caracteres de XML.
    let limpio = |s: &str| !s.contains(['\'', '"', '<', '>', '&']);
    for (label, arg) in buttons.iter().filter(|(l, a)| limpio(l) && limpio(a)) {
        toast = toast.add_button(label, arg);
    }
    let toast = toast.on_activated(move |arg| {
        on_action(arg);
        Ok(())
    });
    match toast.show() {
        Ok(()) => true,
        Err(e) => {
            log::debug!("Aviso con botones no disponible: {e}");
            false
        }
    }
}

/// Lee «accion:id» de un botón.
pub fn parse_action(arg: &str) -> Option<(&str, &str)> {
    let (a, id) = arg.split_once(':')?;
    (!a.is_empty() && !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some((a, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_arguments() {
        assert_eq!(parse_action("done:f18c3a"), Some(("done", "f18c3a")));
        assert_eq!(parse_action("snooze:f1"), Some(("snooze", "f1")));
        assert_eq!(parse_action("done:"), None);
        assert_eq!(parse_action("sinDosPuntos"), None);
        assert_eq!(parse_action("done:../../x"), None, "solo ids de verdad");
    }
}
