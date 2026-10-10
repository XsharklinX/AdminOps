//! Detalles de la interfaz que necesitan al sistema:
//! - ¿Hay una llamada o una reunión? (micrófono o cámara en uso), para que los
//!   sonidos de AdminOps se callen solos.
//! - Archivos soltados sobre la ventana: la interfaz los pasa con
//!   `chrome.webview.postMessageWithAdditionalObjects` y aquí se lee su ruta
//!   real, que el navegador no da. Así se puede abrir una imagen de disco de
//!   200 GB sin copiarla.

use serde::Serialize;
#[cfg(windows)]
use tauri::Emitter;

/// Mensaje con el que la interfaz manda los archivos soltados.
#[cfg(windows)]
const DROP_MESSAGE: &str = "adminops-drop";

/// Lo que se ha soltado y qué cree AdminOps que es.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Dropped {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// image · vault · profile · pdf · report · unknown
    pub kind: String,
}

/// Qué es un archivo por su nombre.
pub fn kind_of(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "img" | "dd" | "raw" | "bin" | "iso" | "vhd" | "vhdx" => "image",
        "zip" | "aovault" => "vault",
        "json" if lower.contains("perfil") || lower.contains("profile") || lower.contains("receta") => "profile",
        "json" => "config",
        "pdf" => "pdf",
        "dmp" => "dump",
        _ => "unknown",
    }
}

fn describe(path: &str) -> Dropped {
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Dropped { path: path.to_string(), kind: kind_of(&name).into(), name, size }
}

/// Escucha los archivos que la interfaz suelta sobre la ventana principal.
pub fn watch_drops(app: &tauri::AppHandle) {
    #[cfg(windows)]
    {
        use tauri::Manager;
        let Some(window) = app.get_webview_window("main") else { return };
        let handle = app.clone();
        let _ = window.with_webview(move |pw| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2File, ICoreWebView2WebMessageReceivedEventArgs2};
            use webview2_com::WebMessageReceivedEventHandler;
            use windows_core::Interface;
            let Ok(core) = pw.controller().CoreWebView2() else { return };
            let handler = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut text = windows_core::PWSTR::null();
                if args.TryGetWebMessageAsString(&mut text).is_err() {
                    return Ok(());
                }
                if webview2_com::take_pwstr(text) != DROP_MESSAGE {
                    return Ok(());
                }
                let Ok(args2) = args.cast::<ICoreWebView2WebMessageReceivedEventArgs2>() else { return Ok(()) };
                let Ok(objects) = args2.AdditionalObjects() else { return Ok(()) };
                let mut count = 0u32;
                let _ = objects.Count(&mut count);
                let mut files = Vec::new();
                for i in 0..count.min(20) {
                    let Ok(obj) = objects.GetValueAtIndex(i) else { continue };
                    let Ok(file) = obj.cast::<ICoreWebView2File>() else { continue };
                    let mut path = windows_core::PWSTR::null();
                    if file.Path(&mut path).is_ok() {
                        let p = webview2_com::take_pwstr(path);
                        if !p.is_empty() {
                            files.push(describe(&p));
                        }
                    }
                }
                if !files.is_empty() {
                    log::info!("Archivos soltados sobre la ventana: {}", files.len());
                    let _ = handle.emit("files-dropped", files);
                }
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core.add_WebMessageReceived(&handler, &mut token);
        });
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// ¿Está el micrófono o la cámara en uso ahora mismo? Windows apunta en el
/// registro cuándo empezó y terminó cada programa de usarlos: «terminó» a cero
/// quiere decir que sigue usándolos.
#[tauri::command(async)]
pub fn media_in_use() -> bool {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let base = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for device in ["microphone", "webcam"] {
            let Ok(root) = hkcu.open_subkey(format!("{base}\\{device}")) else { continue };
            if any_in_use(&root, 0) {
                return true;
            }
        }
        false
    }
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
fn any_in_use(key: &winreg::RegKey, depth: u8) -> bool {
    let start: u64 = key.get_value("LastUsedTimeStart").unwrap_or(0);
    let stop: Option<u64> = key.get_value("LastUsedTimeStop").ok();
    if start > 0 && stop == Some(0) {
        return true;
    }
    // Programas de la tienda directamente; los de escritorio, bajo «NonPackaged».
    depth < 2 && key.enum_keys().flatten().any(|name| key.open_subkey(&name).map(|k| any_in_use(&k, depth + 1)).unwrap_or(false))
}

/// Datos de un archivo elegido por otro camino (la interfaz los pide al soltar
/// sin ruta, por ejemplo desde una versión antigua de WebView2).
#[tauri::command(async)]
pub fn describe_file(path: String) -> Dropped {
    describe(&path)
}

/// Enseña un archivo o una carpeta en el Explorador (solo lo selecciona; no abre ni ejecuta nada).
#[tauri::command(async)]
pub fn reveal_path(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("Esa ruta no existe en este equipo.".into());
    }
    let arg = if p.is_dir() { path.clone() } else { format!("/select,{path}") };
    std::process::Command::new("explorer.exe").arg(arg).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Abre un PDF soltado sobre la ventana con el visor del equipo. Solo PDF.
#[tauri::command(async)]
pub fn open_dropped_pdf(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if kind_of(&path) != "pdf" || !p.is_file() {
        return Err("Solo se abren PDF desde aquí.".into());
    }
    crate::shellopen::open(&path)
}

/// Codifica un texto para un enlace mailto: (todo lo que no sea letra o cifra).
pub fn mail_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.replace('\n', "\r\n").bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Prepara un correo en el programa de correo del equipo («Enviar a… → Correo»).
#[tauri::command(async)]
pub fn compose_mail(subject: String, body: String) -> Result<(), String> {
    // Los programas de correo cortan los enlaces muy largos: se recorta con aviso.
    let mut body = body;
    if body.chars().count() > 1800 {
        body = body.chars().take(1800).collect::<String>() + "\n[…recortado: el resto está en el portapapeles]";
    }
    crate::shellopen::open(&format!("mailto:?subject={}&body={}", mail_encode(&subject), mail_encode(&body)))
}

/// Guarda un texto en un archivo que elige el técnico (biblioteca compartida…). None si cancela.
#[tauri::command(async)]
pub fn save_text_file(app: tauri::AppHandle, name: String, content: String, ext: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let ext: String = ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(10).collect();
    let safe: String = name.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let Some(path) = app.dialog().file().set_file_name(&safe).add_filter("AdminOps", &[ext.as_str()]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::write(&path, content).map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(Some(path.display().to_string()))
}

/// Abre un archivo de texto que elige el técnico. None si cancela.
#[tauri::command(async)]
pub fn open_text_file(app: tauri::AppHandle, ext: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let ext: String = ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(10).collect();
    let Some(path) = app.dialog().file().add_filter("AdminOps", &[ext.as_str(), "json"]).blocking_pick_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > 20 * 1024 * 1024 {
        return Err("El archivo es demasiado grande para ser una biblioteca de AdminOps.".into());
    }
    std::fs::read_to_string(&path).map(Some).map_err(|e| format!("No se pudo leer: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_los_archivos_soltados() {
        assert_eq!(kind_of("Disco cliente.IMG"), "image");
        assert_eq!(kind_of("copia.vhdx"), "image");
        assert_eq!(kind_of("cifrado.zip"), "vault");
        assert_eq!(kind_of("perfil-oficina.json"), "profile");
        assert_eq!(kind_of("adminops-config.json"), "config");
        assert_eq!(kind_of("Informe.pdf"), "pdf");
        assert_eq!(kind_of("MEMORY.DMP"), "dump");
        assert_eq!(kind_of("foto.jpg"), "unknown");
        assert_eq!(kind_of("sin_extension"), "unknown");
    }

    #[test]
    fn codifica_el_correo() {
        assert_eq!(mail_encode("Hola señor\n1"), "Hola%20se%C3%B1or%0D%0A1");
    }
}
