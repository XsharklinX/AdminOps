//! Portales: webs del técnico dentro de AdminOps (tickets, inventario de la
//! empresa, correo de Outlook), incrustadas en la ventana o en una ventana aparte.
//!
//! Seguridad: son páginas remotas, así que no tienen acceso a los comandos de
//! AdminOps (Tauri solo permite el IPC a la interfaz local, ver
//! capabilities/default.json). Tickets e Inventario navegan por donde haga
//! falta, como un navegador: el portal guardado es solo la página de inicio. El
//! Correo, Teams y los routers se quedan en sus sitios: ahí los enlaces a otros
//! sitios se abren en el navegador del usuario, y lo que es parte de entrar
//! (redirecciones, envío del formulario) sigue dentro: ver `guard_navigation`.
//!
//! Rapidez: la vista de cada portal se crea una vez y se mantiene viva (oculta
//! cuando no se ve); la del último portal usado se puede precargar al abrir
//! AdminOps para que al entrar ya esté cargada. «Redactar» con el correo abierto
//! pulsa el botón de Outlook en la propia página en vez de recargarla entera.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

mod navigation;
mod mail;
mod login;
#[macro_use]
mod views;
pub use navigation::*;
pub use mail::*;
use login::*;
pub use views::*;

static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Portal {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub url: String,
    /// Dominios adicionales por los que puede navegar (p. ej. el del inicio de sesión).
    #[serde(default)]
    pub extra_domains: Vec<String>,
    /// "": Tickets · "inventory": inventario web · "mail": correo · "teams": Teams · "router": panel de un router (Mi red).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// Zoom de la página (1 = 100 %; 0 también es 100 %).
    #[serde(default, skip_serializing_if = "is_default_zoom")]
    pub zoom: f64,
    /// Ventanas emergentes: "" en la misma vista · "window" en una ventana aparte
    /// (hace falta en webs como Outlook, que abren así los mensajes y adjuntos).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub popups: String,
    /// Sesión privada: nada se guarda en el disco y la sesión se cierra al salir
    /// de AdminOps (para usar tu cuenta en el equipo de un cliente).
    #[serde(default, skip_serializing_if = "is_false")]
    pub private: bool,
    /// Rellenar el inicio de sesión con la cuenta guardada.
    #[serde(default, skip_serializing_if = "is_false")]
    pub autofill: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_default_zoom(z: &f64) -> bool {
    *z == 0.0 || *z == 1.0
}

impl Portal {
    fn zoom_factor(&self) -> f64 {
        if self.zoom > 0.0 {
            self.zoom.clamp(0.5, 2.0)
        } else {
            1.0
        }
    }
}

fn path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("portals.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Portal> {
    crate::paths::read_json(&path(app))
}

fn find(app: &tauri::AppHandle, id: &str) -> Result<Portal, String> {
    load(app).into_iter().find(|p| p.id == id).ok_or_else(|| "Ese portal ya no existe.".into())
}

// ---------- Validación ----------

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
    p.extra_domains = clean_domains(&p.extra_domains)?;
    if !["", "window"].contains(&p.popups.as_str()) {
        p.popups = String::new();
    }
    if p.zoom != 0.0 {
        p.zoom = (p.zoom * 100.0).round().clamp(50.0, 200.0) / 100.0;
    }
    Ok(p)
}

fn clean_domains(list: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for d in list {
        let d = d.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("*.").trim_end_matches('/').to_ascii_lowercase();
        if d.is_empty() || out.contains(&d) {
            continue;
        }
        if !d.contains('.') || !d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            return Err(format!("«{d}» no es un dominio válido (ejemplo: login.empresa.com)."));
        }
        out.push(d);
    }
    Ok(out)
}

// ---------- Comandos ----------

#[tauri::command]
pub fn list_portals(app: tauri::AppHandle) -> Vec<Portal> {
    load(&app).into_iter().filter(|p| p.kind != "router").collect()
}

/// Portal del panel de un router (uno por red): lo crea o actualiza su dirección.
#[tauri::command]
pub fn router_portal(app: tauri::AppHandle, key: String, name: String, url: String) -> Result<Portal, String> {
    let id: String = format!("r{}", key.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    let p = validate(Portal { id: id.clone(), name: if name.trim().is_empty() { "Router".into() } else { name.chars().take(40).collect() }, url, kind: "router".into(), ..Default::default() })?;
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    match list.iter_mut().find(|x| x.id == id) {
        Some(x) if x.url == p.url => return Ok(x.clone()),
        Some(x) => *x = p.clone(),
        None => list.push(p.clone()),
    }
    // Cambió la dirección: la vista abierta se recrea con la nueva.
    forget(&id);
    if let Some(v) = app.get_webview(&embedded_label(&id)) {
        let _ = v.close();
    }
    crate::paths::write_json(&path(&app), &list)?;
    Ok(p)
}

/// Cierra las vistas de un portal (se recrean con la configuración nueva al volver a mostrarlo).
fn close_views(app: &tauri::AppHandle, id: &str) {
    forget(id);
    if let Some(v) = app.get_webview(&embedded_label(id)) {
        let _ = v.close();
    }
    if let Some(w) = app.get_webview_window(&window_label(id)) {
        let _ = w.close();
    }
}

#[tauri::command]
pub fn save_portal(app: tauri::AppHandle, portal: Portal) -> Result<Portal, String> {
    let mut p = validate(portal)?;
    // Desde aquí solo Tickets, inventario, correo o Teams (los routers tienen sus propias reglas).
    if !["inventory", "mail", "teams"].contains(&p.kind.as_str()) {
        p.kind = String::new();
    }
    if is_microsoft_kind(&p.kind) {
        // Outlook y Teams necesitan sus dominios y abrir mensajes, adjuntos y
        // reuniones en ventana propia.
        let mut domains: Vec<String> = MAIL_DOMAINS.iter().map(|d| d.to_string()).collect();
        if p.kind == "teams" {
            domains.extend(TEAMS_DOMAINS.iter().map(|d| d.to_string()));
        }
        // Empresas con su propia página de inicio de sesión (p. ej. sts.empresa.com).
        let company = crate::workflow::settings(&app).default_domain;
        if !company.trim().is_empty() {
            domains.push(company);
        }
        domains.extend(p.extra_domains.iter().cloned());
        p.extra_domains = clean_domains(&domains).unwrap_or_else(|_| p.extra_domains.clone());
        p.popups = "window".into();
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    match list.iter_mut().find(|x| !p.id.is_empty() && x.id == p.id) {
        Some(x) => {
            // El zoom se cambia desde la barra, no desde el editor.
            if p.zoom == 0.0 {
                p.zoom = x.zoom;
            }
            *x = p.clone();
            // La vista abierta sigue con las reglas viejas: se recrea al volver a mostrarla.
            // Y los sitios aprendidos se olvidan: mandan los dominios que se acaban de guardar.
            close_views(&app, &p.id);
            LEARNED.lock().unwrap_or_else(|e| e.into_inner()).remove(&p.id);
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

/// Permite que el portal navegue por un sitio que se abrió fuera. Vale al
/// momento (sin recargar la vista) y queda guardado en el portal.
#[tauri::command(async)]
pub fn portal_allow_domain(app: tauri::AppHandle, id: String, host: String) -> Result<(), String> {
    let host = host.trim().trim_start_matches("*.").to_ascii_lowercase();
    if host.is_empty() || host.len() > 253 || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return Err("Ese sitio no es válido.".into());
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let p = list.iter_mut().find(|p| p.id == id).ok_or("Ese portal ya no existe.")?;
    if !p.extra_domains.contains(&host) {
        p.extra_domains.push(host.clone());
    }
    let name = p.name.clone();
    crate::paths::write_json(&path(&app), &list)?;
    learn(&id, &host);
    log::info!("Portal «{name}»: «{host}» permitido por el técnico");
    Ok(())
}

#[tauri::command(async)]
pub fn delete_portal(app: tauri::AppHandle, id: String) -> Result<(), String> {
    close_views(&app, &id);
    LEARNED.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    list.retain(|p| p.id != id);
    let mut logins = load_logins(&app);
    if logins.remove(&id).is_some() {
        crate::paths::write_json(&logins_path(&app), &logins)?;
    }
    crate::paths::write_json(&path(&app), &list)
}

/// Muestra el portal incrustado en el rectángulo indicado (px lógicos de la ventana).
/// Asíncrono: crear la vista espera al hilo principal.
#[tauri::command(async)]
pub fn portal_show(app: tauri::AppHandle, id: String, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let label = embedded_label(&id);
    hide_embedded(&app, Some(&label));
    let (pos, size) = (LogicalPosition::new(x, y), LogicalSize::new(width.max(50.0), height.max(50.0)));
    LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), Instant::now());
    if let Some(v) = app.get_webview(&label) {
        if remember_bounds(&id, x, y, size.width, size.height) {
            let _ = v.set_position(pos);
            let _ = v.set_size(size);
        }
        return v.show().map_err(|e| e.to_string());
    }
    remember_bounds(&id, x, y, size.width, size.height);
    let p = find(&app, &id)?;
    create_embedded(&app, &p, pos, size, true)?;
    Ok(())
}

/// Carga un portal en segundo plano para que al entrar ya esté listo. Los de
/// sesión privada no: iniciarían sesión (y pedirían la verificación) sin que
/// el técnico lo haya pedido.
#[tauri::command(async)]
pub fn portal_preload(app: tauri::AppHandle, id: String, width: f64, height: f64) -> Result<(), String> {
    if app.get_webview(&embedded_label(&id)).is_some() {
        return Ok(());
    }
    let Ok(p) = find(&app, &id) else { return Ok(()) };
    if p.private {
        return Ok(());
    }
    create_embedded(&app, &p, LogicalPosition::new(0.0, 0.0), LogicalSize::new(width.max(400.0), height.max(300.0)), false)?;
    Ok(())
}

/// Apunta el rectángulo de una vista. Devuelve `true` si cambió (y hay que
/// aplicarlo); `false` si es el mismo de antes y no hace falta molestar a la ventana.
fn remember_bounds(id: &str, x: f64, y: f64, width: f64, height: f64) -> bool {
    let r = (x, y, width, height);
    let mut map = BOUNDS.lock().unwrap_or_else(|e| e.into_inner());
    if map.get(id) == Some(&r) {
        return false;
    }
    map.insert(id.to_string(), r);
    true
}

#[tauri::command(async)]
pub fn portal_bounds(app: tauri::AppHandle, id: String, x: f64, y: f64, width: f64, height: f64) {
    let (w, h) = (width.max(50.0), height.max(50.0));
    if !remember_bounds(&id, x, y, w, h) {
        return;
    }
    if let Some(v) = app.get_webview(&embedded_label(&id)) {
        let _ = v.set_position(LogicalPosition::new(x, y));
        let _ = v.set_size(LogicalSize::new(w, h));
    }
}

/// Oculta un portal incrustado (su página se tapa o deja de estar visible). La
/// vista sigue viva: al volver está donde se dejó, con la sesión intacta.
#[tauri::command(async)]
pub fn portal_hide(app: tauri::AppHandle, id: String) {
    if let Some(v) = app.get_webview(&embedded_label(&id)) {
        let _ = v.hide();
        LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()).insert(id, Instant::now());
    }
}

/// Script que escribe `text` en el campo que el técnico tiene seleccionado en
/// la web (el de «Resolución» del ticket, por ejemplo). Devuelve si lo hizo.
/// El texto va como JSON, nunca como código. Baja por los iframes de la misma
/// web, que es donde muchos sistemas de tickets meten su formulario.
fn insert_text_script(text: &str) -> String {
    let t = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
  const t = {t};
  let e = document.activeElement;
  try {{ while (e && e.tagName === "IFRAME" && e.contentDocument) e = e.contentDocument.activeElement; }} catch (_) {{}}
  if (!e) return false;
  if (e.isContentEditable) {{ e.focus(); e.ownerDocument.execCommand("insertText", false, t); return true; }}
  const campo = e.tagName === "TEXTAREA" || (e.tagName === "INPUT" && /^(text|search|)$/i.test(e.type || ""));
  if (!campo || e.readOnly || e.disabled) return false;
  const proto = e.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  const set = Object.getOwnPropertyDescriptor(proto, "value").set;
  const a = e.selectionStart ?? e.value.length, b = e.selectionEnd ?? e.value.length;
  set.call(e, e.value.slice(0, a) + t + e.value.slice(b));
  e.dispatchEvent(new Event("input", {{ bubbles: true }}));
  e.dispatchEvent(new Event("change", {{ bubbles: true }}));
  return true;
}})()"#
    )
}

/// Escribe un texto en el campo seleccionado del portal (la resolución del caso
/// en el formulario del ticket). `false` si el portal no está abierto o no hay
/// un campo de texto seleccionado: entonces la interfaz lo deja copiado y lo dice.
#[tauri::command(async)]
pub fn portal_insert_text(app: tauri::AppHandle, id: String, text: String) -> Result<bool, String> {
    let Some(v) = app.get_webview(&embedded_label(&id)) else { return Ok(false) };
    let text: String = text.chars().take(8000).collect();
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        let js = insert_text_script(&text);
        v.with_webview(move |pw| unsafe {
            use webview2_com::ExecuteScriptCompletedHandler;
            let Ok(core) = pw.controller().CoreWebView2() else {
                let _ = tx.send(false);
                return;
            };
            let tx2 = tx.clone();
            let handler = ExecuteScriptCompletedHandler::create(Box::new(move |res, json: String| {
                let _ = tx2.send(res.is_ok() && json.trim() == "true");
                Ok(())
            }));
            if core.ExecuteScript(&windows_core::HSTRING::from(js.as_str()), &handler).is_err() {
                let _ = tx.send(false);
            }
        })
        .map_err(|e| e.to_string())?;
        // WebView2 contesta en milisegundos; si no, se da por no escrito.
        Ok(rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false))
    }
    #[cfg(not(windows))]
    {
        let _ = (v, text);
        Ok(false)
    }
}

/// Destruye la vista de un portal (se vuelve a crear al mostrarlo).
///
/// Para las vistas que no llegaron a arrancar. Una vista de WebView2 es una
/// ventana del sistema que va por encima de la interfaz; si WebView2 no llega a
/// iniciarla, no pinta nada ni responde a «ocultar», y se queda invisible encima
/// de la página tragándose los clics y el scroll de lo que haya debajo. Pasó con
/// la página de Ajustes en la 1.1.6. Destruirla es lo único que la quita seguro.
#[tauri::command(async)]
pub fn portal_reset(app: tauri::AppHandle, id: String) {
    close_views(&app, &id);
    log::warn!("Portal {id}: vista destruida porque no llegó a arrancar");
}

/// Oculta los portales incrustados (al salir de Tickets o al abrir un diálogo encima).
#[tauri::command(async)]
pub fn portal_hide_all(app: tauri::AppHandle) {
    hide_embedded(&app, None);
}

/// Cierra las vistas de portales que llevan mucho rato sin verse. Cada una es un
/// proceso de WebView2: con Tickets, inventario y correo abiertos son tres para
/// toda la sesión. Al volver a entrar se crean de nuevo (y la sesión web sigue).
#[tauri::command(async)]
pub fn portal_close_idle(app: tauri::AppHandle) -> usize {
    let Ok(w) = main_window(&app) else { return 0 };
    let limite = idle_close();
    let idle_ids: Vec<String> = {
        let seen = LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner());
        w.webviews()
            .iter()
            .filter_map(|v| v.label().strip_prefix("portal-").map(String::from))
            // Sin marca de uso o vista hace mucho: fuera. La visible se marca al mostrarla.
            .filter(|id| seen.get(id).is_none_or(|t| t.elapsed() > limite))
            .collect()
    };
    let mut closed = 0;
    for id in idle_ids {
        let Some(v) = app.get_webview(&embedded_label(&id)) else { continue };
        forget(&id);
        if v.close().is_ok() {
            log::info!("Portal {id}: vista cerrada por inactividad");
            closed += 1;
        }
    }
    closed
}

#[tauri::command(async)]
pub fn portal_nav(app: tauri::AppHandle, id: String, action: String) -> Result<(), String> {
    let Some(v) = app.get_webview(&embedded_label(&id)) else { return Ok(()) };

    match action.as_str() {
        "back" => v.eval("history.back()"),
        "forward" => v.eval("history.forward()"),
        "reload" => v.reload(),
        "stop" => v.eval("window.stop()"),
        "print" => v.eval("window.print()"),
        "home" => {
            let p = find(&app, &id)?;
            v.navigate(p.url.parse().map_err(|_| "Dirección no válida.".to_string())?)
        }
        _ => return Err("Acción desconocida.".into()),
    }
    .map_err(|e| e.to_string())
}

/// Ir a una dirección escrita en la barra. Fuera del portal se abre en el
/// navegador del usuario (devuelve `false`).
#[tauri::command(async)]
pub fn portal_go(app: tauri::AppHandle, id: String, url: String) -> Result<bool, String> {
    let p = find(&app, &id)?;
    let text = url.trim();
    let text = if text.contains("://") { text.to_string() } else { format!("https://{text}") };
    let u = Url::parse(&text).map_err(|_| "Esa dirección no es válida.".to_string())?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err("Solo direcciones web (https://…).".into());
    }
    if !allowed(&p, &u) {
        open_external(u.as_str());
        return Ok(false);
    }
    if let Some(v) = app.get_webview(&embedded_label(&id)) {
        v.navigate(u).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

/// Cambia el zoom de la página y lo recuerda para ese portal.
#[tauri::command(async)]
pub fn portal_zoom(app: tauri::AppHandle, id: String, zoom: f64) -> Result<f64, String> {
    let z = ((zoom * 100.0).round() / 100.0).clamp(0.5, 2.0);
    if let Some(v) = app.get_webview(&embedded_label(&id)) {
        v.set_zoom(z).map_err(|e| e.to_string())?;
    }
    if let Some(w) = app.get_webview_window(&window_label(&id)) {
        let _ = w.set_zoom(z);
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    if let Some(p) = list.iter_mut().find(|p| p.id == id) {
        p.zoom = z;
        crate::paths::write_json(&path(&app), &list)?;
    }
    Ok(z)
}

/// Busca texto en la página (resalta la siguiente coincidencia).
#[tauri::command(async)]
pub fn portal_find(app: tauri::AppHandle, id: String, text: String, backwards: bool) -> Result<(), String> {
    let Some(v) = app.get_webview(&embedded_label(&id)) else { return Ok(()) };
    let q = serde_json::to_string(&text.chars().take(200).collect::<String>()).map_err(|e| e.to_string())?;
    v.eval(format!("window.find({q}, false, {backwards}, true)")).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginInfo {
    user: String,
    has_password: bool,
}

/// Cuenta guardada de un portal (la contraseña nunca vuelve a la interfaz).
#[tauri::command]
pub fn portal_login_get(app: tauri::AppHandle, id: String) -> Option<LoginInfo> {
    load_logins(&app).remove(&id).map(|l| LoginInfo { user: l.user, has_password: !l.secret.is_empty() })
}

/// Guarda la cuenta de un portal. `password`: `None` deja la que había; vacía la borra.
#[tauri::command]
pub fn portal_login_set(app: tauri::AppHandle, id: String, user: String, password: Option<String>) -> Result<(), String> {
    find(&app, &id)?;
    let user = user.trim().chars().take(200).collect::<String>();
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut logins = load_logins(&app);
    let old = logins.remove(&id).unwrap_or_default();
    let secret = match password {
        None => old.secret,
        Some(p) if p.is_empty() => String::new(),
        Some(p) => crate::secrets::seal(&p)?,
    };
    if !user.is_empty() || !secret.is_empty() {
        logins.insert(id, Login { user, secret });
    }
    crate::paths::write_json(&logins_path(&app), &logins)
}

/// Escribir un correo nuevo en el Correo de AdminOps. `false` si no hay correo
/// configurado (la interfaz usa entonces el programa de correo de Windows).
///
/// Sin nada que rellenar, se pulsa el botón de correo nuevo del propio Outlook:
/// abre el borrador al instante y con su aspecto de siempre. Con destinatario o
/// asunto hay que usar su enlace de «redactar», que abre el editor suelto (el de
/// la cinta completa); dentro del buzón se ve como una página a medio vestir, así
/// que ese va en una ventana propia, que es para lo que está hecho.
#[tauri::command(async)]
pub fn portal_compose(app: tauri::AppHandle, to: String, subject: Option<String>, body: Option<String>) -> Result<bool, String> {
    let Some(p) = load(&app).into_iter().find(|p| p.kind == "mail") else { return Ok(false) };
    let to = to.trim();
    if !to.is_empty() && (to.len() > 254 || !to.contains('@') || to.chars().any(|c| c.is_whitespace() || "<>\"".contains(c))) {
        return Err("El correo no es válido.".into());
    }
    let body: String = body.unwrap_or_default().chars().take(4000).collect();
    let subject = subject.as_deref().unwrap_or("").trim().to_string();
    let url = compose_url(&p, to, &subject, &body);
    // Sin datos que rellenar, con el correo abierto: su propio botón (instantáneo).
    let blank = to.is_empty() && subject.is_empty() && body.is_empty();
    if blank {
        if let Some(v) = app.get_webview(&embedded_label(&p.id)) {
            return v.eval(compose_click_script(&url)).map(|()| true).map_err(|e| e.to_string());
        }
    }
    let label = format!("portalcompose-{}", p.id);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.unminimize();
        w.navigate(url).map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
        return Ok(true);
    }
    let builder = configure!(tauri::WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url)), &app, p, label, get_webview_window)
        .title("Mensaje nuevo · AdminOps")
        .inner_size(1040.0, 720.0)
        .min_inner_size(520.0, 400.0)
        .theme(Some(tauri::Theme::Dark));
    let w = builder.build().map_err(|e| format!("No se pudo abrir el mensaje: {e}"))?;
    enable_autofill(w.as_ref());
    if p.zoom_factor() != 1.0 {
        let _ = w.set_zoom(p.zoom_factor());
    }
    Ok(true)
}

/// ¿Es el Teams personal (teams.live.com) y no el del trabajo?
fn is_personal_teams(p: &Portal) -> bool {
    Url::parse(&p.url).ok().and_then(|u| u.host_str().map(|h| host_matches(h, "teams.live.com"))).unwrap_or(false)
}

/// Abre un chat o una llamada de Teams con esa persona dentro de AdminOps.
/// `false` si no hay Teams configurado (la interfaz usa entonces la aplicación
/// de Teams del equipo, o Teams en el navegador).
#[tauri::command(async)]
pub fn portal_teams(app: tauri::AppHandle, email: String, call: bool) -> Result<bool, String> {
    let Some(p) = load(&app).into_iter().find(|p| p.kind == "teams") else { return Ok(false) };
    let e = email.trim();
    if e.len() > 254 || !e.contains('@') || e.chars().any(|c| c.is_whitespace() || "<>\"".contains(c)) {
        return Err("El correo no es válido.".into());
    }
    let base = if is_personal_teams(&p) { "https://teams.live.com/l" } else { "https://teams.microsoft.com/l" };
    let kind = if call { "call" } else { "chat" };
    let mut url: Url = format!("{base}/{kind}/0/0").parse().map_err(|_| "Dirección no válida.".to_string())?;
    url.query_pairs_mut().append_pair("users", e);
    match app.get_webview(&embedded_label(&p.id)) {
        Some(v) => {
            v.navigate(url).map_err(|e| e.to_string())?;
        }
        // Aún sin abrir: la vista se creará directamente en ese chat.
        None => {
            PENDING.lock().unwrap_or_else(|e| e.into_inner()).insert(p.id.clone(), url);
        }
    }
    Ok(true)
}

/// Cierra la sesión del correo. En sesión privada basta con cerrar la vista (no
/// queda nada guardado); si no, se cierra en Microsoft.
#[tauri::command(async)]
pub fn portal_sign_out(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let p = find(&app, &id)?;
    if p.private {
        close_views(&app, &id);
        return Ok(());
    }
    if is_microsoft_kind(&p.kind) {
        if let Some(v) = app.get_webview(&embedded_label(&id)) {
            v.navigate(sign_out_url(&p).parse().map_err(|_| "Dirección no válida.".to_string())?).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    close_views(&app, &id);
    Ok(())
}

fn download_path(download: u64) -> Result<PathBuf, String> {
    let list = DOWNLOADS.lock().unwrap_or_else(|e| e.into_inner());
    let d = list.iter().find(|d| d.number == download).ok_or("Esa descarga ya no está.")?;
    if !d.done || !d.path.exists() {
        return Err("El archivo ya no está en su carpeta.".into());
    }
    Ok(d.path.clone())
}

#[tauri::command]
pub fn portal_download_open(download: u64) -> Result<(), String> {
    crate::shellopen::open(&download_path(download)?.to_string_lossy())
}

#[tauri::command]
pub fn portal_download_reveal(download: u64) -> Result<(), String> {
    let p = download_path(download)?;
    crate::ps::hidden("explorer.exe").arg(format!("/select,{}", p.display())).spawn().map(|_| ()).map_err(|e| e.to_string())
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
    guard_navigation(w.as_ref(), &app, &p);
    enable_autofill(w.as_ref());
    accept_router_certificates(w.as_ref(), &p);
    if p.zoom_factor() != 1.0 {
        let _ = w.set_zoom(p.zoom_factor());
    }
    Ok(())
}

#[tauri::command]
pub fn portal_open_external(app: tauri::AppHandle, id: String) -> Result<(), String> {
    open_external(&find(&app, &id)?.url);
    Ok(())
}

/// Portales de Tickets (no los de routers) para la copia de la configuración.
/// Las cuentas guardadas no se incluyen.
pub fn export_portals(app: &tauri::AppHandle) -> Vec<Portal> {
    load(app).into_iter().filter(|p| p.kind != "router").collect()
}

/// Añade los portales importados que no estén ya (por dirección).
pub fn import_portals(app: &tauri::AppHandle, portals: Vec<Portal>) -> Result<usize, String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(app);
    let mut added = 0;
    for p in portals {
        let kind = if ["inventory", "mail"].contains(&p.kind.as_str()) { p.kind.clone() } else { String::new() };
        let Ok(mut p) = validate(Portal { kind, ..p }) else { continue };
        if list.iter().any(|x| x.url.eq_ignore_ascii_case(&p.url)) {
            continue;
        }
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        p.id = format!("p{stamp:x}");
        list.push(p);
        added += 1;
    }
    crate::paths::write_json(&path(app), &list)?;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrated_auth_only_for_company_domains() {
        let p = |url: &str, kind: &str| Portal { name: "x".into(), url: url.into(), kind: kind.into(), ..Default::default() };
        let portals = vec![
            p("https://inventario.empresa.local/equipos", "inventory"),
            p("https://tickets.empresa.local", ""),
            p("http://192.168.1.1", "router"),
            p("https://intranet", ""),
            p("https://outlook.office.com/mail/", "mail"),
        ];
        assert_eq!(integrated_auth_domains(&portals, "empresa.local"), vec!["inventario.empresa.local", "*.empresa.local", "tickets.empresa.local"]);
        assert!(integrated_auth_domains(&[], "").is_empty());
        assert_eq!(integrated_auth_domains(&[], "empresa.local"), vec!["*.empresa.local"]);
    }

    fn portal() -> Portal {
        Portal { id: "p1".into(), name: "Intranet".into(), url: "https://intranet.empresa.local/tickets".into(), extra_domains: vec!["login.microsoftonline.com".into()], ..Default::default() }
    }

    /// Teams navega por sus dominios y por los de Microsoft, y por nada más.
    #[test]
    fn teams_navigates_its_own_domains() {
        let mut domains: Vec<String> = MAIL_DOMAINS.iter().chain(TEAMS_DOMAINS.iter()).map(|d| d.to_string()).collect();
        domains = clean_domains(&domains).unwrap();
        let p = Portal { id: "t1".into(), name: "Teams".into(), url: "https://teams.microsoft.com/v2/".into(), extra_domains: domains, kind: "teams".into(), ..Default::default() };
        let ok = |u: &str| allowed(&p, &u.parse().unwrap());
        assert!(ok("https://teams.microsoft.com/v2/"));
        assert!(ok("https://login.microsoftonline.com/common/oauth2/v2.0/authorize"));
        assert!(ok("https://statics.teams.cdn.office.net/x.js"));
        assert!(ok("https://empresa.sharepoint.com/archivo.docx"));
        assert!(!ok("https://teams.microsoft.com.evil.com/"));
        assert!(!ok("https://google.com/"));
        assert!(!ok("javascript:alert(1)"));
    }

    /// La variable de entorno lleva los argumentos de siempre más, si hay
    /// intranets, la lista de dominios. (Los portales NO la copian en sus
    /// opciones: ver `browser_args`.)
    #[test]
    fn merged_args_add_the_allowlist_only_when_there_is_one() {
        let base = "--disable-features=msWebOOUI";
        let auth = "--auth-server-allowlist=*.empresa.local";
        assert_eq!(merged_browser_args(base, Some(auth)), format!("{base} {auth}"));
        // Sin intranets configuradas, los argumentos son solo los de siempre.
        assert_eq!(merged_browser_args(base, None), base);
        assert_eq!(merged_browser_args(base, Some("")), base);
    }

    /// **Regresión de la 1.1.6**: los portales copiaban en sus opciones la lista
    /// de dominios de la intranet, quedaban distintos de la ventana principal y
    /// WebView2 no los creaba: Correo, Teams y Tickets se quedaban en «Abriendo…».
    /// Las opciones del portal tienen que ser las de la ventana principal, tal
    /// cual, aunque la variable de entorno lleve más cosas.
    #[test]
    fn portal_options_never_include_the_environment_args() {
        let base = Some("--disable-features=msWebOOUI".to_string());
        std::env::set_var(ARGS_ENV, "--disable-features=msWebOOUI --auth-server-allowlist=*.empresa.local");
        assert_eq!(portal_browser_args(base.clone()), base);
        std::env::remove_var(ARGS_ENV);
    }

    /// El texto que se pega en el ticket va como dato JSON, nunca como código.
    #[test]
    fn insert_text_passes_the_text_as_data() {
        let js = insert_text_script("Hola\"); alert(1); (\"");
        assert!(js.contains(r#"const t = "Hola\"); alert(1); (\"";"#), "{js}");
        assert!(js.starts_with("(() => {") && js.trim_end().ends_with("})()"), "{js}");
    }

    /// Cada portal de Microsoft cierra sesión donde le corresponde.
    #[test]
    fn sign_out_goes_to_the_right_page() {
        let p = |url: &str, kind: &str| Portal { name: "x".into(), url: url.into(), kind: kind.into(), ..Default::default() };
        assert!(sign_out_url(&p("https://teams.microsoft.com/v2/", "teams")).contains("logout"));
        assert_eq!(sign_out_url(&p("https://outlook.live.com/mail/", "mail")), "https://login.live.com/logout.srf");
        assert_eq!(sign_out_url(&p("https://outlook.office.com/mail/", "mail")), "https://outlook.office.com/owa/logoff.owa");
        assert!(is_microsoft_kind("mail") && is_microsoft_kind("teams") && !is_microsoft_kind("inventory"));
    }

    /// Colocar la vista solo se le pide a la ventana cuando el rectángulo cambia.
    #[test]
    fn repeated_bounds_are_ignored() {
        let id = "prueba-bounds";
        BOUNDS.lock().unwrap().remove(id);
        assert!(remember_bounds(id, 0.0, 0.0, 800.0, 600.0));
        assert!(!remember_bounds(id, 0.0, 0.0, 800.0, 600.0));
        assert!(remember_bounds(id, 0.0, 0.0, 801.0, 600.0));
        BOUNDS.lock().unwrap().remove(id);
    }

    #[test]
    fn tickets_and_inventory_browse_freely() {
        for kind in ["", "inventory"] {
            let p = Portal { kind: kind.into(), ..portal() };
            let ok = |u: &str| allowed(&p, &u.parse().unwrap());
            assert!(ok("https://intranet.empresa.local/soporte/123"));
            assert!(ok("https://otro-sistema.empresa.com/login?next=/tickets"));
            assert!(ok("http://192.168.1.50:8080/glpi"));
            assert!(ok("https://google.com/"));
            // Sigue sin poder abrir archivos del equipo ni ejecutar nada.
            assert!(!ok("file:///C:/Windows/win.ini"));
            assert!(!ok("javascript:alert(1)"));
            assert!(!ok("ms-settings:network"));
        }
    }

    #[test]
    fn navigation_stays_inside_the_portal_domains() {
        // El Correo, Teams y los routers sí se quedan en sus sitios.
        let p = Portal { kind: "mail".into(), ..portal() };
        let ok = |u: &str| allowed(&p, &u.parse().unwrap());
        assert!(ok("https://intranet.empresa.local/soporte/123"));
        assert!(ok("https://archivos.intranet.empresa.local/x.pdf"));
        assert!(ok("https://login.microsoftonline.com/oauth"));
        assert!(ok("about:blank"));
        // El resto de la casa: el inicio de sesión suele estar en un hermano.
        assert!(ok("https://sso.empresa.local/login"));
        assert!(ok("https://empresa.local/"));
        assert!(!ok("https://otra.gob.do/"));
        assert!(!ok("https://intranet.empresa.local.evil.com/"));
        assert!(!ok("https://google.com/"));
        assert!(!ok("file:///C:/Windows/win.ini"));
        assert!(!ok("javascript:alert(1)"));
    }

    /// «Entrar con Microsoft / Google» funciona sin configurar nada, y solo por https.
    #[test]
    fn sign_in_buttons_work_out_of_the_box() {
        let p = Portal { id: "sso".into(), name: "Correo".into(), url: "https://correo.empresa.com".into(), kind: "mail".into(), ..Default::default() };
        let ok = |u: &str| allowed(&p, &u.parse().unwrap());
        assert!(ok("https://login.microsoftonline.com/common/oauth2/v2.0/authorize"));
        assert!(ok("https://accounts.google.com/o/oauth2/auth"));
        assert!(ok("https://empresa.okta.com/login"));
        assert!(!ok("http://login.microsoftonline.com/"));
        assert!(!ok("https://www.google.com/search?q=x"));
        assert!(!ok("https://login.microsoftonline.com.evil.net/"));
    }

    #[test]
    fn same_house_without_opening_the_door_to_everyone() {
        // Hermanos bajo el dominio de la empresa.
        assert!(same_org("sso.empresa.com", "intranet.empresa.com"));
        assert!(same_org("empresa.com", "www.empresa.com"));
        assert!(!same_org("empresa.com.evil.net", "intranet.empresa.com"));
        // «com.do» o «co.uk» no son de nadie: no valen como casa común.
        assert!(!same_org("otra.com.do", "empresa.com.do"));
        assert!(!same_org("otra.co.uk", "empresa.co.uk"));
        assert!(!same_org("google.com", "empresa.com"));
        // Intranet por nombre corto o IP: su nombre largo y el resto de la red interna.
        assert!(same_org("intranet.empresa.local", "intranet"));
        assert!(same_org("192.168.1.20", "intranet"));
        assert!(same_org("sso", "10.0.0.5"));
        assert!(same_org("intranet", "intranet.empresa.local"));
        assert!(!same_org("google.com", "intranet"));
        assert!(!same_org("8.8.8.8", "10.0.0.5"));
        assert!(!same_org("9.9.9.9", "8.8.8.8"));
    }

    #[test]
    fn signing_in_is_not_a_link_to_another_site() {
        // Redirección del servidor, formulario enviado o navegación por script: pasan.
        assert!(off_site(true, false, false).is_some());
        assert!(off_site(true, true, false).is_some());
        assert!(off_site(false, true, true).is_some());
        assert!(off_site(false, false, false).is_some());
        // Un enlace que pulsa el usuario hacia otro sitio: al navegador de fuera.
        assert!(off_site(false, true, false).is_none());

        // Un sitio al que se llegó entrando queda admitido para ese portal, y solo para él.
        let p = Portal { id: "aprende".into(), name: "X".into(), url: "https://correo.empresa.com".into(), kind: "mail".into(), ..Default::default() };
        let idp: Url = "https://idp.proveedor.net/sso".parse().unwrap();
        assert!(!allowed(&p, &idp));
        learn(&p.id, "idp.proveedor.net");
        assert!(allowed(&p, &idp));
        assert!(!allowed(&Portal { id: "otro".into(), ..p.clone() }, &idp));
        LEARNED.lock().unwrap().remove("aprende");
    }

    #[test]
    fn validates_portals() {
        let v = validate(Portal { name: " Intranet ".into(), url: "intranet.empresa.local".into(), extra_domains: vec!["https://*.sso.empresa.local/".into()], zoom: 1.234, popups: "otra".into(), ..Default::default() }).unwrap();
        assert_eq!(v.url, "https://intranet.empresa.local");
        assert_eq!(v.name, "Intranet");
        assert_eq!(v.extra_domains, vec!["sso.empresa.local"]);
        assert_eq!(v.zoom, 1.23);
        assert_eq!(v.popups, "");
        assert!(validate(Portal { url: "ftp://x.com".into(), ..portal() }).is_err());
        assert!(validate(Portal { url: "https://".into(), ..portal() }).is_err());
        assert!(validate(Portal { extra_domains: vec!["no válido".into()], ..portal() }).is_err());
        assert!(validate(Portal { name: "".into(), ..portal() }).is_err());
    }

    #[test]
    fn old_portals_file_still_loads() {
        let old = r#"[{"id":"p1","name":"GLPI","url":"https://glpi.empresa.com","extraDomains":[]}]"#;
        let list: Vec<Portal> = serde_json::from_str(old).unwrap();
        assert_eq!(list[0].zoom_factor(), 1.0);
        assert!(!list[0].private && !list[0].autofill);
        // Lo que no se usa no se escribe (el archivo sigue igual de limpio).
        assert!(!serde_json::to_string(&list[0]).unwrap().contains("zoom"));
    }

    #[test]
    fn outlook_links() {
        let work = Portal { url: "https://outlook.office.com/mail/".into(), kind: "mail".into(), ..portal() };
        let home = Portal { url: "https://outlook.live.com/mail/".into(), kind: "mail".into(), ..portal() };
        assert!(!is_personal_outlook(&work));
        assert!(is_personal_outlook(&home));
        let c = compose_url(&work, "ana@empresa.com", "Equipo listo", "");
        assert_eq!(c.host_str(), Some("outlook.office.com"));
        assert!(c.as_str().contains("to=ana%40empresa.com") && c.as_str().contains("subject=Equipo+listo"));
        assert!(compose_url(&home, "", "", "").as_str().starts_with("https://outlook.live.com/mail/0/deeplink/compose"));
        assert!(compose_url(&work, "", "", "Hola
Ana").as_str().contains("body=Hola%0AAna"));
    }

    #[test]
    fn compose_click_falls_back_to_the_link() {
        let work = Portal { url: "https://outlook.office.com/mail/".into(), kind: "mail".into(), ..portal() };
        let s = compose_click_script(&compose_url(&work, "", "", ""));
        assert!(s.contains(r#"const fallback = "https://outlook.office.com/mail/deeplink/compose"#));
        assert!(s.contains("correo nuevo") && s.contains("location.assign(fallback)"));
    }

    #[test]
    fn autofill_only_on_trusted_pages() {
        let mail = Portal { url: "https://outlook.office.com/mail/".into(), kind: "mail".into(), autofill: true, ..portal() };
        let at = |p: &Portal, u: &str| autofill_target(p, &u.parse().unwrap());
        assert_eq!(at(&mail, "https://login.microsoftonline.com/common/oauth2"), Some(true));
        assert_eq!(at(&mail, "https://login.live.com/login.srf"), Some(true));
        assert_eq!(at(&mail, "http://login.microsoftonline.com/x"), None);
        assert_eq!(at(&mail, "https://login.microsoftonline.com.evil.com/"), None);
        assert_eq!(at(&Portal { autofill: false, ..mail.clone() }, "https://login.microsoftonline.com/"), None);
        let glpi = Portal { url: "https://glpi.empresa.com".into(), autofill: true, ..portal() };
        assert_eq!(at(&glpi, "https://glpi.empresa.com/index.php"), Some(false));
        assert_eq!(at(&glpi, "http://glpi.empresa.com/"), None);
        assert_eq!(at(&glpi, "https://otra.com/"), None);
        let intranet = Portal { url: "http://intranet".into(), autofill: true, ..portal() };
        assert_eq!(at(&intranet, "http://intranet/login"), Some(false));
    }

    #[test]
    fn autofill_script_escapes_data_and_parses() {
        let s = autofill_script("ana\"</script>@x.com", "p'a\"s\\s", true);
        assert!(s.contains(r#""ana\"</script>@x.com""#));
        assert!(s.contains(r#""p'a\"s\\s""#));
        assert!(!s.contains("p'a\"s\\s\n"));
    }

    #[test]
    fn web_errors_are_explained() {
        assert!(explain_web_error(0).is_some());
        #[cfg(windows)]
        {
            use webview2_com::Microsoft::Web::WebView2::Win32::*;
            assert!(explain_web_error(COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED.0).is_none());
            assert!(explain_web_error(COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED.0).unwrap().contains("VPN"));
        }
    }
}

#[cfg(test)]
mod script_dump {
    /// Vuelca el script a un archivo para comprobar su sintaxis con Node:
    /// `cargo test --lib dump_autofill_script -- --ignored`
    #[test]
    #[ignore]
    fn dump_autofill_script() {
        let dir = std::env::var("ADMINOPS_DUMP_DIR").unwrap_or_else(|_| ".".into());
        std::fs::write(format!("{dir}/autofill_ms.js"), super::autofill_script("a@b.com", "x\"y", true)).unwrap();
        std::fs::write(format!("{dir}/autofill_generic.js"), super::autofill_script("usuario", "", false)).unwrap();
        let url = "https://outlook.office.com/mail/deeplink/compose".parse().unwrap();
        std::fs::write(format!("{dir}/compose_click.js"), super::compose_click_script(&url)).unwrap();
    }
}
