//! Tickets: portales web del técnico (intranet, GLPI, osTicket…) dentro de
//! AdminOps, incrustados en la ventana o en una ventana aparte.
//!
//! Seguridad: son páginas remotas, así que no tienen acceso a los comandos de
//! AdminOps (Tauri solo permite el IPC a la interfaz local, ver
//! capabilities/default.json). Además solo navegan dentro de los dominios del
//! portal: cualquier otro enlace se abre en el navegador del usuario.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Portal {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub url: String,
    /// Dominios adicionales por los que puede navegar (p. ej. el del inicio de sesión).
    #[serde(default)]
    pub extra_domains: Vec<String>,
}

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("portals.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Portal> {
    crate::paths::read_json(&path(app))
}

fn find(app: &tauri::AppHandle, id: &str) -> Result<Portal, String> {
    load(app).into_iter().find(|p| p.id == id).ok_or_else(|| "Ese portal ya no existe.".into())
}

fn embedded_label(id: &str) -> String {
    format!("portal-{id}")
}

fn window_label(id: &str) -> String {
    format!("portalwin-{id}")
}

/// `host` es `domain` o un subdominio suyo.
fn host_matches(host: &str, domain: &str) -> bool {
    let (h, d) = (host.to_ascii_lowercase(), domain.to_ascii_lowercase());
    h == d || h.ends_with(&format!(".{d}"))
}

/// ¿Puede el portal navegar a `url` sin salir de la app?
pub fn allowed(p: &Portal, url: &Url) -> bool {
    match url.scheme() {
        "about" | "data" | "blob" => true,
        "http" | "https" => {
            let Some(host) = url.host_str() else { return false };
            let own = Url::parse(&p.url).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default();
            host_matches(host, &own) || p.extra_domains.iter().any(|d| host_matches(host, d))
        }
        _ => false,
    }
}

/// Abre una dirección en el navegador predeterminado del usuario (vía el
/// Explorador, para que no herede los permisos de administrador).
fn open_external(url: &str) {
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:") {
        let _ = std::process::Command::new("explorer.exe").arg(url).spawn();
    }
}

fn validate(mut p: Portal) -> Result<Portal, String> {
    p.name = p.name.trim().to_string();
    p.url = p.url.trim().to_string();
    if p.name.is_empty() || p.name.chars().count() > 40 {
        return Err("El portal necesita un nombre (máximo 40 caracteres).".into());
    }
    if !p.url.contains("://") {
        p.url = format!("https://{}", p.url);
    }
    let u = Url::parse(&p.url).map_err(|_| "La dirección no es válida.".to_string())?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return Err("La dirección debe empezar por https:// (o http://).".into());
    }
    p.extra_domains = p
        .extra_domains
        .iter()
        .map(|d| d.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("*.").trim_end_matches('/').to_ascii_lowercase())
        .filter(|d| !d.is_empty())
        .collect();
    if let Some(bad) = p.extra_domains.iter().find(|d| !d.contains('.') || !d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')) {
        return Err(format!("«{bad}» no es un dominio válido (ejemplo: login.empresa.com)."));
    }
    Ok(p)
}

/// Activa "¿Guardar contraseña?" y el autocompletado de WebView2 (desactivados por defecto).
fn enable_autofill<R: tauri::Runtime>(webview: &tauri::Webview<R>) {
    #[cfg(windows)]
    let _ = webview.with_webview(|pw| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
        use windows_core::Interface;
        if let Ok(core) = pw.controller().CoreWebView2() {
            if let Ok(s) = core.Settings().and_then(|s| s.cast::<ICoreWebView2Settings4>()) {
                let _ = s.SetIsPasswordAutosaveEnabled(true);
                let _ = s.SetIsGeneralAutofillEnabled(true);
            }
        }
    });
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LoadEvent {
    id: String,
    url: String,
    loading: bool,
}

/// Los argumentos del navegador deben coincidir con los de la ventana principal:
/// WebView2 no admite dos configuraciones distintas en la misma carpeta de datos.
fn browser_args(app: &tauri::AppHandle) -> Option<String> {
    app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone())
}

macro_rules! configure {
    ($builder:expr, $app:expr, $portal:expr, $label:expr, $get:ident) => {{
        let (p_nav, p_new, app_new, app_load, label_new) = ($portal.clone(), $portal.clone(), $app.clone(), $app.clone(), $label.clone());
        let id = $portal.id.clone();
        let mut b = $builder
            .on_navigation(move |url| {
                let ok = allowed(&p_nav, url);
                if !ok {
                    open_external(url.as_str());
                }
                ok
            })
            .on_new_window(move |url, _| {
                // Ventanas emergentes: dentro del portal se abren en la misma vista.
                if allowed(&p_new, &url) {
                    let (app, label) = (app_new.clone(), label_new.clone());
                    std::thread::spawn(move || {
                        if let Some(w) = app.$get(&label) {
                            let _ = w.navigate(url);
                        }
                    });
                } else {
                    open_external(url.as_str());
                }
                NewWindowResponse::Deny
            })
            .on_page_load(move |_, payload| {
                let loading = matches!(payload.event(), PageLoadEvent::Started);
                let _ = app_load.emit("portal-load", LoadEvent { id: id.clone(), url: payload.url().to_string(), loading });
            });
        if let Some(args) = browser_args($app) {
            b = b.additional_browser_args(&args);
        }
        b
    }};
}

fn main_window(app: &tauri::AppHandle) -> Result<tauri::Window, String> {
    app.get_window("main").ok_or_else(|| "No se encontró la ventana principal.".into())
}

fn hide_embedded(app: &tauri::AppHandle, except: Option<&str>) {
    if let Ok(w) = main_window(app) {
        for v in w.webviews() {
            if v.label().starts_with("portal-") && Some(v.label()) != except {
                let _ = v.hide();
            }
        }
    }
}

// ---------- Comandos ----------

#[tauri::command]
pub fn list_portals(app: tauri::AppHandle) -> Vec<Portal> {
    load(&app)
}

#[tauri::command]
pub fn save_portal(app: tauri::AppHandle, portal: Portal) -> Result<Portal, String> {
    let mut p = validate(portal)?;
    let _guard = FILE_LOCK.lock().unwrap();
    let mut list = load(&app);
    match list.iter_mut().find(|x| !p.id.is_empty() && x.id == p.id) {
        Some(x) => {
            *x = p.clone();
            // La vista abierta sigue con las reglas viejas: se recrea al volver a mostrarla.
            if let Some(v) = app.get_webview(&embedded_label(&p.id)) {
                let _ = v.close();
            }
        }
        None => {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            p.id = format!("p{stamp:x}");
            list.push(p.clone());
        }
    }
    crate::paths::write_json(&path(&app), &list)?;
    Ok(p)
}

#[tauri::command(async)]
pub fn delete_portal(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if let Some(v) = app.get_webview(&embedded_label(&id)) {
        let _ = v.close();
    }
    if let Some(w) = app.get_webview_window(&window_label(&id)) {
        let _ = w.close();
    }
    let _guard = FILE_LOCK.lock().unwrap();
    let mut list = load(&app);
    list.retain(|p| p.id != id);
    crate::paths::write_json(&path(&app), &list)
}

/// Muestra el portal incrustado en el rectángulo indicado (px lógicos de la ventana).
/// Asíncrono: crear la vista espera al hilo principal.
#[tauri::command(async)]
pub fn portal_show(app: tauri::AppHandle, id: String, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let label = embedded_label(&id);
    hide_embedded(&app, Some(&label));
    let (pos, size) = (LogicalPosition::new(x, y), LogicalSize::new(width.max(50.0), height.max(50.0)));
    if let Some(v) = app.get_webview(&label) {
        let _ = v.set_position(pos);
        let _ = v.set_size(size);
        return v.show().map_err(|e| e.to_string());
    }
    let p = find(&app, &id)?;
    let url: Url = p.url.parse().map_err(|_| "La dirección del portal no es válida.".to_string())?;
    let builder = configure!(tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(url)), &app, p, label, get_webview);
    let v = main_window(&app)?.add_child(builder, pos, size).map_err(|e| format!("No se pudo abrir el portal: {e}"))?;
    enable_autofill(&v);
    log::info!("Tickets: abierto el portal «{}»", p.name);
    Ok(())
}

#[tauri::command(async)]
pub fn portal_bounds(app: tauri::AppHandle, id: String, x: f64, y: f64, width: f64, height: f64) {
    if let Some(v) = app.get_webview(&embedded_label(&id)) {
        let _ = v.set_position(LogicalPosition::new(x, y));
        let _ = v.set_size(LogicalSize::new(width.max(50.0), height.max(50.0)));
    }
}

/// Oculta los portales incrustados (al salir de Tickets o al abrir un diálogo encima).
#[tauri::command(async)]
pub fn portal_hide_all(app: tauri::AppHandle) {
    hide_embedded(&app, None);
}

#[tauri::command(async)]
pub fn portal_nav(app: tauri::AppHandle, id: String, action: String) -> Result<(), String> {
    let Some(v) = app.get_webview(&embedded_label(&id)) else { return Ok(()) };
    match action.as_str() {
        "back" => v.eval("history.back()"),
        "forward" => v.eval("history.forward()"),
        "reload" => v.reload(),
        "home" => {
            let p = find(&app, &id)?;
            v.navigate(p.url.parse().map_err(|_| "Dirección no válida.".to_string())?)
        }
        _ => return Err("Acción desconocida.".into()),
    }
    .map_err(|e| e.to_string())
}

/// Abre el portal en una ventana propia de AdminOps.
#[tauri::command(async)]
pub fn portal_open_window(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let label = window_label(&id);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.unminimize();
        return w.set_focus().map_err(|e| e.to_string());
    }
    let p = find(&app, &id)?;
    let url: Url = p.url.parse().map_err(|_| "La dirección del portal no es válida.".to_string())?;
    let builder = configure!(tauri::WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url)), &app, p, label, get_webview_window)
        .title(format!("{} · AdminOps", p.name))
        .inner_size(1200.0, 820.0)
        .min_inner_size(600.0, 400.0)
        .theme(Some(tauri::Theme::Dark));
    let w = builder.build().map_err(|e| format!("No se pudo abrir la ventana: {e}"))?;
    enable_autofill(w.as_ref());
    Ok(())
}

#[tauri::command]
pub fn portal_open_external(app: tauri::AppHandle, id: String) -> Result<(), String> {
    open_external(&find(&app, &id)?.url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn portal() -> Portal {
        Portal { id: "p1".into(), name: "Intranet".into(), url: "https://intranet.pgr.gob.do/tickets".into(), extra_domains: vec!["login.microsoftonline.com".into()] }
    }

    #[test]
    fn navigation_stays_inside_the_portal_domains() {
        let p = portal();
        let ok = |u: &str| allowed(&p, &u.parse().unwrap());
        assert!(ok("https://intranet.pgr.gob.do/soporte/123"));
        assert!(ok("https://archivos.intranet.pgr.gob.do/x.pdf"));
        assert!(ok("https://login.microsoftonline.com/oauth"));
        assert!(ok("about:blank"));
        assert!(!ok("https://pgr.gob.do/"));
        assert!(!ok("https://intranet.pgr.gob.do.evil.com/"));
        assert!(!ok("https://evilintranet.pgr.gob.do/"));
        assert!(!ok("file:///C:/Windows/win.ini"));
        assert!(!ok("javascript:alert(1)"));
    }

    #[test]
    fn validates_portals() {
        let v = validate(Portal { id: String::new(), name: " Intranet ".into(), url: "intranet.pgr.gob.do".into(), extra_domains: vec!["https://*.sso.pgr.gob.do/".into()] }).unwrap();
        assert_eq!(v.url, "https://intranet.pgr.gob.do");
        assert_eq!(v.name, "Intranet");
        assert_eq!(v.extra_domains, vec!["sso.pgr.gob.do"]);
        assert!(validate(Portal { url: "ftp://x.com".into(), ..portal() }).is_err());
        assert!(validate(Portal { url: "https://".into(), ..portal() }).is_err());
        assert!(validate(Portal { extra_domains: vec!["no válido".into()], ..portal() }).is_err());
        assert!(validate(Portal { name: "".into(), ..portal() }).is_err());
    }
}
