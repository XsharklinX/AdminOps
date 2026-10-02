//! Portales: webs del técnico dentro de AdminOps (tickets, inventario de la
//! empresa, correo de Outlook), incrustadas en la ventana o en una ventana aparte.
//!
//! Seguridad: son páginas remotas, así que no tienen acceso a los comandos de
//! AdminOps (Tauri solo permite el IPC a la interfaz local, ver
//! capabilities/default.json). Los enlaces a otros sitios se abren en el
//! navegador del usuario; lo que es parte de entrar al portal (redirecciones,
//! envío del formulario de inicio de sesión) sigue dentro: ver `guard_navigation`.
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

/// Terminaciones que no son de nadie («com.do», «gob.do», «co.uk»): bajo ellas
/// hay empresas distintas, así que no sirven para decir «es de la misma casa».
fn public_suffix_like(labels: &[&str]) -> bool {
    labels.len() < 2
        || (labels.len() == 2 && labels[1].len() == 2 && ["com", "co", "org", "net", "gob", "gov", "edu", "ac", "mil", "nom", "web", "sld", "art", "gouv", "or", "ne", "go"].contains(&labels[0]))
}

/// ¿Es `host` de la misma casa que el portal (`own`)? El inicio de sesión de una
/// intranet casi nunca está en el mismo nombre que la web: va en un hermano
/// (`intranet.empresa.com` → `sso.empresa.com`), o la web se abre por su nombre
/// corto y contesta con el largo o con su IP.
fn same_org(host: &str, own: &str) -> bool {
    let (h, o) = (host.to_ascii_lowercase(), own.to_ascii_lowercase());
    if o.is_empty() {
        return false;
    }
    let internal = |x: &str| !x.contains('.') || crate::network::lan::is_private_host(x);
    if internal(&o) {
        // Portal de la red interna: el resto de la red interna, y su nombre largo.
        return internal(&h) || h.starts_with(&format!("{o}."));
    }
    if o.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }
    let labels: Vec<&str> = o.split('.').collect();
    // El nombre corto del mismo servidor (intranet.empresa.local → intranet).
    if h == labels[0] {
        return true;
    }
    labels.len() >= 3 && !public_suffix_like(&labels[1..]) && host_matches(&h, &labels[1..].join("."))
}

/// Sitios a los que cada portal ha ido como parte de entrar (ver
/// `guard_navigation`) o que el técnico ha permitido: valen mientras AdminOps
/// esté abierta, sin tener que recrear la vista.
static LEARNED: LazyLock<Mutex<HashMap<String, HashSet<String>>>> = LazyLock::new(Default::default);

fn learn(id: &str, host: &str) {
    LEARNED.lock().unwrap_or_else(|e| e.into_inner()).entry(id.to_string()).or_default().insert(host.to_ascii_lowercase());
}

fn learned(id: &str, host: &str) -> bool {
    LEARNED.lock().unwrap_or_else(|e| e.into_inner()).get(id).is_some_and(|set| set.iter().any(|d| host_matches(host, d)))
}

/// Páginas de inicio de sesión que usan media internet: «Entrar con Microsoft»,
/// «Entrar con Google», Okta, Auth0… El botón que lleva a ellas es un enlace
/// normal que pulsa el usuario, así que no se distingue de un enlace a otro
/// sitio: se admiten de fábrica en todos los portales.
const IDENTITY_PROVIDERS: &[&str] = &[
    "login.microsoftonline.com",
    "login.microsoft.com",
    "login.live.com",
    "login.windows.net",
    "b2clogin.com",
    "accounts.google.com",
    "okta.com",
    "auth0.com",
    "onelogin.com",
    "duosecurity.com",
];

/// ¿Puede el portal navegar a `url` sin salir de la app?
pub fn allowed(p: &Portal, url: &Url) -> bool {
    match url.scheme() {
        "about" | "data" | "blob" => true,
        "http" | "https" => {
            let Some(host) = url.host_str() else { return false };
            let own = Url::parse(&p.url).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default();
            host_matches(host, &own)
                || p.extra_domains.iter().any(|d| host_matches(host, d))
                || same_org(host, &own)
                || (url.scheme() == "https" && IDENTITY_PROVIDERS.iter().any(|d| host_matches(host, d)))
                || learned(&p.id, host)
        }
        _ => false,
    }
}

/// Qué hacer con una navegación que sale de los sitios del portal. `Some(motivo)`:
/// es parte de usar la web y se deja pasar; `None`: es un enlace a otro sitio, y
/// va al navegador del usuario.
///
/// Un inicio de sesión sale del dominio de tres maneras, y ninguna es «pulsar un
/// enlace»: el servidor redirige (302 al proveedor de identidad), la página
/// navega sola por script, o se envía un formulario (la petición lleva
/// `Content-Type`). Cortar cualquiera de las tres rompe la entrada: la página de
/// destino se abría en el navegador de fuera, sin la sesión ni los datos del
/// formulario, y solo enseñaba un error.
fn off_site(redirected: bool, user_initiated: bool, form: bool) -> Option<&'static str> {
    if redirected {
        Some("redirección del servidor")
    } else if form {
        Some("envío de un formulario")
    } else if !user_initiated {
        Some("navegación de la propia página")
    } else {
        None
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BlockedEvent {
    id: String,
    /// Sitio que se abrió fuera (solo el nombre: la dirección puede llevar datos).
    host: String,
}

/// Avisa a la interfaz de que algo se abrió en el navegador de fuera, para que
/// el técnico pueda permitir ese sitio en el portal con un clic.
fn report_blocked(app: &tauri::AppHandle, p: &Portal, url: &Url) {
    let host = url.host_str().unwrap_or_default().to_string();
    if host.is_empty() {
        return;
    }
    log::info!("Portal «{}»: «{host}» no es del portal y se abrió en el navegador del usuario", p.name);
    let _ = app.emit("portal-blocked", BlockedEvent { id: p.id.clone(), host });
}

/// Decide cada navegación de la página con lo que WebView2 sabe de ella (si es
/// una redirección, si la inició el usuario, si envía un formulario). Lo que es
/// de los sitios del portal pasa; lo que es parte de entrar pasa y ese sitio
/// queda admitido; un enlace a otro sitio se cancela y se abre fuera.
fn guard_navigation<R: tauri::Runtime>(webview: &tauri::Webview<R>, app: &tauri::AppHandle, p: &Portal) {
    #[cfg(windows)]
    {
        let (app, p) = (app.clone(), p.clone());
        let _ = webview.with_webview(move |pw| unsafe {
            use webview2_com::NavigationStartingEventHandler;
            let Ok(core) = pw.controller().CoreWebView2() else { return };
            let handler = NavigationStartingEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = windows_core::PWSTR::null();
                if args.Uri(&mut uri).is_err() {
                    return Ok(());
                }
                let Ok(url) = Url::parse(&webview2_com::take_pwstr(uri)) else { return Ok(()) };
                if !matches!(url.scheme(), "http" | "https") || allowed(&p, &url) {
                    return Ok(());
                }
                let (mut redirected, mut user, mut form) = (windows_core::BOOL(0), windows_core::BOOL(0), windows_core::BOOL(0));
                let _ = args.IsRedirected(&mut redirected);
                let _ = args.IsUserInitiated(&mut user);
                if let Ok(headers) = args.RequestHeaders() {
                    let _ = headers.Contains(windows_core::w!("Content-Type"), &mut form);
                }
                match off_site(redirected.as_bool(), user.as_bool(), form.as_bool()) {
                    Some(why) => {
                        let host = url.host_str().unwrap_or_default();
                        learn(&p.id, host);
                        log::info!("Portal «{}»: se sigue a «{host}» ({why})", p.name);
                    }
                    None => {
                        let _ = args.SetCancel(true);
                        open_external(url.as_str());
                        report_blocked(&app, &p, &url);
                    }
                }
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core.add_NavigationStarting(&handler, &mut token);
        });
    }
    #[cfg(not(windows))]
    {
        let _ = (webview, app, p);
    }
}

/// Abre una dirección en el navegador predeterminado del usuario (vía el
/// Explorador, para que no herede los permisos de administrador).
fn open_external(url: &str) {
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:") {
        let _ = crate::shellopen::open(url);
    }
}

// ---------- Correo (Outlook) y Teams ----------

/// Dominios por los que navegan las webs de Microsoft dentro de AdminOps: el
/// correo, el inicio de sesión de Microsoft y los visores de adjuntos.
pub const MAIL_DOMAINS: &[&str] = &[
    "outlook.office.com",
    "outlook.office365.com",
    "outlook.live.com",
    "outlook.com",
    "office.com",
    "office.net",
    "officeapps.live.com",
    "live.com",
    "login.microsoftonline.com",
    "login.microsoft.com",
    "microsoftonline.com",
    "msauth.net",
    "msftauth.net",
    "microsoft.com",
    "sharepoint.com",
    // Outlook nuevo y portal de Microsoft 365.
    "cloud.microsoft",
    "static.microsoft",
    "microsoft365.com",
    // Inicio de sesión de empresa (único y automático).
    "windows.net",
    "microsoftazuread-sso.com",
];

/// Lo propio de Teams en la web, además de los dominios de Microsoft: la web en
/// sí, los servidores de chat y llamadas, y los archivos que se comparten.
pub const TEAMS_DOMAINS: &[&str] = &[
    "teams.microsoft.com",
    "teams.live.com",
    "teams.cloud.microsoft",
    "teams.microsoft.us",
    "skype.com",
    "skypeforbusiness.com",
    "lync.com",
    "trouter.io",
    "trafficmanager.net",
    "asm.skype.com",
    "svc.ms",
];

/// Portales de Microsoft: el Correo y Teams comparten inicio de sesión, ventanas
/// emergentes propias y la misma lista de dominios permitidos.
fn is_microsoft_kind(kind: &str) -> bool {
    kind == "mail" || kind == "teams"
}

/// Páginas de inicio de sesión de Microsoft (donde se rellena la cuenta guardada).
fn is_microsoft_login(host: &str) -> bool {
    ["login.microsoftonline.com", "login.live.com", "login.microsoft.com"].iter().any(|d| host.eq_ignore_ascii_case(d))
}

/// ¿Es el Outlook personal (outlook.com / hotmail) y no el del trabajo (Microsoft 365)?
fn is_personal_outlook(p: &Portal) -> bool {
    Url::parse(&p.url).ok().and_then(|u| u.host_str().map(|h| host_matches(h, "live.com") || host_matches(h, "outlook.com"))).unwrap_or(false)
}

/// Enlace de Outlook para escribir un mensaje nuevo.
fn compose_url(p: &Portal, to: &str, subject: &str, body: &str) -> Url {
    let base = if is_personal_outlook(p) { "https://outlook.live.com/mail/0/deeplink/compose" } else { "https://outlook.office.com/mail/deeplink/compose" };
    let mut u = Url::parse(base).expect("dirección fija válida");
    {
        let mut q = u.query_pairs_mut();
        if !to.is_empty() {
            q.append_pair("to", to);
        }
        if !subject.is_empty() {
            q.append_pair("subject", subject);
        }
        if !body.is_empty() {
            q.append_pair("body", body);
        }
    }
    u
}

/// «Redactar» con Outlook ya abierto: pulsa su propio botón de correo nuevo, que
/// abre el borrador al instante (el enlace de redactar recarga toda la web y
/// tarda varios segundos). Si Outlook aún está cargando espera a que aparezca el
/// botón; si no aparece (o no está en el correo, p. ej. en el inicio de sesión)
/// usa el enlace. La dirección va como texto JSON, nunca como código.
fn compose_click_script(fallback: &Url) -> String {
    let url = serde_json::to_string(fallback.as_str()).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
  const fallback = {url};
  if (window.__adminopsCompose) return;
  if (!location.pathname.startsWith("/mail")) {{ location.assign(fallback); return; }}
  window.__adminopsCompose = true;
  const names = ["new mail", "new message", "new email", "correo nuevo", "nuevo correo", "mensaje nuevo", "nuevo mensaje", "correo electrónico nuevo"];
  const norm = (s) => (s || "").replace(/\s+/g, " ").trim().toLowerCase();
  const find = () => Array.from(document.querySelectorAll('button, [role="button"]')).find((el) =>
    el.offsetParent !== null && !el.disabled && el.getAttribute("aria-disabled") !== "true" &&
    [el.getAttribute("aria-label"), el.getAttribute("title"), el.textContent].some((t) => names.includes(norm(t))));
  let tries = 0;
  const tick = () => {{
    const b = find();
    if (b) {{ window.__adminopsCompose = false; b.click(); return; }}
    if (++tries > 40) {{ window.__adminopsCompose = false; location.assign(fallback); return; }}
    setTimeout(tick, 150);
  }};
  tick();
}})();"#
    )
}

fn sign_out_url(p: &Portal) -> &'static str {
    match (p.kind.as_str(), is_personal_outlook(p)) {
        // Teams no tiene una página propia de salida: se cierra la sesión de Microsoft.
        ("teams", _) => "https://login.microsoftonline.com/common/oauth2/v2.0/logout",
        (_, true) => "https://login.live.com/logout.srf",
        (_, false) => "https://outlook.office.com/owa/logoff.owa",
    }
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

// ---------- Inicio de sesión guardado ----------

/// Cuenta de un portal. La contraseña va cifrada como las de los routers: en
/// portable, con la clave del USB (viaja con él); instalado, con la de Windows.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct Login {
    user: String,
    #[serde(default)]
    secret: String,
}

fn logins_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("portal_logins.json")
}

fn load_logins(app: &tauri::AppHandle) -> HashMap<String, Login> {
    crate::paths::read_json(&logins_path(app))
}

fn login_of(app: &tauri::AppHandle, id: &str) -> Option<(String, String)> {
    let l = load_logins(app).remove(id)?;
    let pass = if l.secret.is_empty() { String::new() } else { crate::secrets::open(&l.secret).ok()? };
    Some((l.user, pass))
}

/// Script que rellena el inicio de sesión. En las páginas de Microsoft sigue sus
/// dos pasos (cuenta → Siguiente → contraseña → Iniciar sesión) una sola vez; en
/// las demás solo rellena, sin enviar. Los datos van como texto JSON, nunca como código.
fn autofill_script(user: &str, pass: &str, microsoft: bool) -> String {
    let js = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
  if (window.__adminopsFill) return;
  window.__adminopsFill = true;
  const user = {user}, pass = {pass}, microsoft = {microsoft};
  const set = (el, v) => {{
    const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), "value");
    if (d && d.set) d.set.call(el, v); else el.value = v;
    el.dispatchEvent(new Event("input", {{ bubbles: true }}));
    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
  }};
  const visible = (el) => !!el && el.offsetParent !== null && !el.disabled && !el.readOnly;
  const next = () => setTimeout(() => {{ const b = document.getElementById("idSIButton9"); if (b) b.click(); }}, 400);
  let userDone = !user, passDone = !pass, tries = 0;
  const tick = () => {{
    tries++;
    if (microsoft) {{
      const u = document.querySelector('input[name="loginfmt"]');
      if (!userDone && visible(u)) {{ if (!u.value) {{ set(u, user); next(); }} userDone = true; }}
      const p = document.querySelector('input[name="passwd"]');
      if (!passDone && visible(p)) {{ if (!p.value) {{ set(p, pass); next(); }} passDone = true; }}
    }} else {{
      const p = Array.from(document.querySelectorAll('input[type="password"]')).find(visible);
      if (p) {{
        const scope = p.form || document;
        const inputs = Array.from(scope.querySelectorAll("input")).filter(visible);
        const u = inputs.slice(0, inputs.indexOf(p)).reverse().find((x) => ["text", "email", ""].includes((x.getAttribute("type") || "").toLowerCase()));
        if (u && user && !u.value) set(u, user);
        if (pass && !p.value) set(p, pass);
        userDone = passDone = true;
      }}
    }}
    if ((userDone && passDone) || tries > 60) clearInterval(timer);
  }};
  const timer = setInterval(tick, 500);
  tick();
}})();"#,
        user = js(user),
        pass = js(pass),
        microsoft = microsoft
    )
}

/// ¿Hay que rellenar el inicio de sesión en esta página? Solo en las de
/// Microsoft (correo) o en el propio dominio del portal, y nunca por http en Internet.
fn autofill_target(p: &Portal, url: &Url) -> Option<bool> {
    if !p.autofill {
        return None;
    }
    let host = url.host_str()?;
    if is_microsoft_login(host) && url.scheme() == "https" {
        return Some(true);
    }
    let own = Url::parse(&p.url).ok()?;
    let secure = url.scheme() == "https" || own.scheme() == "http";
    (secure && host_matches(host, own.host_str()?)).then_some(false)
}

// ---------- Estado de las vistas ----------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LoadEvent {
    id: String,
    url: String,
    loading: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StateEvent {
    id: String,
    can_back: bool,
    can_forward: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ErrorEvent {
    id: String,
    /// Vacío: la última carga fue bien.
    message: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TitleEvent {
    id: String,
    title: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadInfo {
    id: String,
    /// Número propio de la descarga (para abrirla sin pasar su ruta a la interfaz).
    download: u64,
    /// Solo el nombre del archivo: la ruta lleva el nombre del usuario.
    name: String,
    /// "running" | "done" | "failed"
    state: String,
}

/// Una descarga de esta sesión: dónde quedó el archivo y si terminó.
struct Download {
    /// Número propio, que no cambia aunque se poden las viejas.
    number: u64,
    url: Url,
    path: PathBuf,
    done: bool,
}

/// Descargas de esta sesión. Se podan las más viejas: antes la lista crecía sin
/// límite mientras AdminOps estuviera abierto, porque el número de cada descarga
/// era su posición en ella y quitar una habría descolocado a todas las demás.
static DOWNLOADS: LazyLock<Mutex<Vec<Download>>> = LazyLock::new(Default::default);
/// Cuántas descargas se recuerdan (la interfaz enseña 20 por portal).
const MAX_DOWNLOADS: usize = 200;
/// Número de la siguiente descarga.
static NEXT_DOWNLOAD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
/// Cuándo se creó cada vista, para medir cuánto tarda en cargar la primera vez.
static CREATED: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);
/// Dirección a la que ir al crear la vista (p. ej. «Redactar» con el correo aún cerrado).
static PENDING: LazyLock<Mutex<HashMap<String, Url>>> = LazyLock::new(Default::default);
/// Cuándo se vio por última vez cada portal incrustado.
static LAST_SEEN: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);
/// Último rectángulo aplicado a cada vista. Colocar una vista hay que pedírselo
/// al hilo de la ventana y eso espera: al redimensionar llegaban decenas de
/// peticiones iguales y la interfaz se quedaba tiesa. Si no ha cambiado, no se toca.
/// Rectángulo de una vista: x, y, ancho y alto en píxeles lógicos.
type Rect = (f64, f64, f64, f64);
static BOUNDS: LazyLock<Mutex<HashMap<String, Rect>>> = LazyLock::new(Default::default);
/// Una vista sin usarse este tiempo se cierra (vuelve a abrirse al entrar).
///
/// Cerrarla libera un proceso de navegador entero, pero **volver a entrar cuesta
/// una carga completa**: Teams y Outlook tardan lo suyo en arrancar, y media hora
/// es poquísimo para una jornada de trabajo. En el equipo del técnico se aguantan
/// vivas casi toda la sesión; en uno justo de recursos se sigue cerrando pronto,
/// que es donde la memoria importa de verdad.
fn idle_close() -> Duration {
    if crate::pspool::modest_machine() {
        Duration::from_secs(30 * 60)
    } else {
        Duration::from_secs(4 * 60 * 60)
    }
}



fn file_name(p: &std::path::Path, url: &Url) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .or_else(|| url.path_segments().and_then(|mut s| s.next_back().map(String::from)))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "archivo".into())
}

/// Explicación de por qué no cargó la página.
fn explain_web_error(status: i32) -> Option<String> {
    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        let s = COREWEBVIEW2_WEB_ERROR_STATUS(status);
        let m = match s {
            COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED => return None,
            COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED => "No se encuentra el servidor: el nombre no existe o este equipo no está en la red de la empresa (¿falta la VPN?).",
            COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE | COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT => {
                "El servidor no responde. Puede estar apagado, bloqueado por el firewall, o ser una intranet que solo funciona por http:// y esté guardada como https:// (compruébalo en «Editar portal»)."
            }
            COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT => "El servidor tarda demasiado en responder.",
            COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED => "Este equipo no tiene conexión a Internet ni a la red.",
            COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET | COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_ABORTED => "La conexión se cortó mientras cargaba.",
            COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED
            | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID
            | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_REVOKED
            | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_COMMON_NAME_IS_INCORRECT => {
                "El certificado de seguridad de la web no es válido (caducado o de otro sitio). Revisa la fecha del equipo."
            }
            COREWEBVIEW2_WEB_ERROR_STATUS_VALID_AUTHENTICATION_CREDENTIALS_REQUIRED => "La web pide un usuario y contraseña que no se aceptaron.",
            COREWEBVIEW2_WEB_ERROR_STATUS_VALID_PROXY_AUTHENTICATION_REQUIRED => "El proxy de la red pide usuario y contraseña.",
            // Un código sin traducir sirve de poco al técnico, pero es justo lo
            // que hace falta para saber qué pasó: va en el mensaje y al registro.
            _ => return Some(format!("No se pudo abrir la página (código {status} de WebView2). Está en el registro técnico: Ajustes → Datos de AdminOps.")),
        };
        Some(m.to_string())
    }
    #[cfg(not(windows))]
    {
        (status != 0).then(|| format!("No se pudo abrir la página (código {status})."))
    }
}

/// Historial (atrás/adelante) y errores de carga, directamente de WebView2.
fn watch_navigation<R: tauri::Runtime>(webview: &tauri::Webview<R>, app: &tauri::AppHandle, id: &str) {
    #[cfg(windows)]
    {
        let (app_h, id_h, app_n, id_n) = (app.clone(), id.to_string(), app.clone(), id.to_string());
        let _ = webview.with_webview(move |pw| unsafe {
            use webview2_com::{HistoryChangedEventHandler, NavigationCompletedEventHandler};
            let Ok(core) = pw.controller().CoreWebView2() else { return };
            let history = HistoryChangedEventHandler::create(Box::new(move |sender, _| {
                if let Some(core) = sender {
                    let (mut back, mut fwd) = (windows_core::BOOL(0), windows_core::BOOL(0));
                    let _ = core.CanGoBack(&mut back);
                    let _ = core.CanGoForward(&mut fwd);
                    let _ = app_h.emit("portal-state", StateEvent { id: id_h.clone(), can_back: back.as_bool(), can_forward: fwd.as_bool() });
                }
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core.add_HistoryChanged(&history, &mut token);
            let completed = NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
                if let Some(args) = args {
                    let mut ok = windows_core::BOOL(0);
                    let _ = args.IsSuccess(&mut ok);
                    let message = if ok.as_bool() {
                        String::new()
                    } else {
                        let mut status = Default::default();
                        let _ = args.WebErrorStatus(&mut status);
                        match explain_web_error(status.0) {
                            Some(m) => m,
                            None => return Ok(()),
                        }
                    };
                    // Primera carga completa: cuánto tardó desde que se creó la vista.
                    if let Some(t) = CREATED.lock().unwrap_or_else(|e| e.into_inner()).remove(&id_n) {
                        log::info!("Portal {id_n}: primera carga en {} ms{}", t.elapsed().as_millis(), if message.is_empty() { "" } else { " (con error)" });
                    }
                    // Cada fallo, al registro con su código y su dirección: sin esto,
                    // «no se pudo abrir la página» no se puede diagnosticar a distancia.
                    if !message.is_empty() {
                        let url = sender
                            .as_ref()
                            .and_then(|c| {
                                let mut uri = windows_core::PWSTR::null();
                                c.Source(&mut uri).ok().map(|()| webview2_com::take_pwstr(uri))
                            })
                            .unwrap_or_default();
                        let mut status = Default::default();
                        let _ = args.WebErrorStatus(&mut status);
                        log::warn!("Portal {id_n}: no cargó «{url}» (código {} de WebView2)", status.0);
                    }
                    let _ = app_n.emit("portal-error", ErrorEvent { id: id_n.clone(), message });
                }
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core.add_NavigationCompleted(&completed, &mut token);
        });
    }
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

// Los portales ocultos NO se congelan (TrySuspend). Se hizo en la 1.1.5 para
// ahorrar CPU y rompió el inicio de sesión de Microsoft: un portal precargado
// quedaba congelado en la página de inicio de sesión, su contexto caducaba y
// al pedir el código Microsoft rechazaba la verificación («Sorry, we're having
// trouble verifying your account»). Una vista oculta se queda viva, como antes;
// el propio WebView2 ya frena los temporizadores de lo que no se ve.

/// Olvida todo lo que se recordaba de un portal: su rectángulo, cuándo se vio,
/// cuándo se creó y a dónde tenía que ir. Se llama al cerrar o borrar su vista,
/// para que nada quede colgando de un portal que ya no existe.
fn forget(id: &str) {
    BOUNDS.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    CREATED.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
}

/// Los routers usan un certificado propio que el navegador no reconoce: en el
/// panel de un router se acepta, pero solo para direcciones de la red local.
fn accept_router_certificates<R: tauri::Runtime>(webview: &tauri::Webview<R>, p: &Portal) {
    if p.kind != "router" {
        return;
    }
    #[cfg(windows)]
    let _ = webview.with_webview(|pw| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_14, COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW};
        use webview2_com::ServerCertificateErrorDetectedEventHandler;
        use windows_core::Interface;
        let Ok(core) = pw.controller().CoreWebView2() else { return };
        let Ok(core14) = core.cast::<ICoreWebView2_14>() else { return };
        let handler = ServerCertificateErrorDetectedEventHandler::create(Box::new(|_, args| {
            if let Some(args) = args {
                let mut uri = windows_core::PWSTR::null();
                if args.RequestUri(&mut uri).is_ok() {
                    let uri = webview2_com::take_pwstr(uri);
                    let host = Url::parse(&uri).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default();
                    if crate::network::lan::is_private_host(&host) {
                        let _ = args.SetAction(COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW);
                    }
                }
            }
            Ok(())
        }));
        let mut token = 0i64;
        let _ = core14.add_ServerCertificateErrorDetected(&handler, &mut token);
    });
}

/// Dominios de la empresa a los que el navegador interno puede enviar la cuenta
/// de Windows sin preguntar (autenticación integrada: intranets con dominio,
/// como *.pgr.gob.do). Solo los de los portales del técnico y su dominio habitual.
pub fn integrated_auth_domains(portals: &[Portal], default_domain: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |d: String| {
        if !d.is_empty() && !out.contains(&d) {
            out.push(d);
        }
    };
    // El correo y Teams son de Microsoft: no se les envía la cuenta de Windows.
    for p in portals.iter().filter(|p| p.kind != "router" && !is_microsoft_kind(&p.kind)) {
        let Some(host) = Url::parse(&p.url).ok().and_then(|u| u.host_str().map(str::to_ascii_lowercase)) else { continue };
        if host.parse::<std::net::IpAddr>().is_ok() || !host.contains('.') {
            continue;
        }
        let labels: Vec<&str> = host.split('.').collect();
        add(host.clone());
        // inventario.pgr.gob.do → *.pgr.gob.do (el resto de webs de la empresa).
        if labels.len() >= 3 {
            add(format!("*.{}", labels[1..].join(".")));
        }
    }
    let d = default_domain.trim().trim_start_matches("*.").to_ascii_lowercase();
    if d.contains('.') && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        add(format!("*.{d}"));
    }
    out
}

/// El argumento para WebView2 (vacío si no hay dominios). Se lee al arrancar,
/// antes de crear la ventana: un portal nuevo se aplica al volver a abrir AdminOps.
pub fn integrated_auth_arg() -> Option<String> {
    let dir = crate::paths::shared_data_dir_early()?;
    let portals: Vec<Portal> = crate::paths::read_json(&dir.join("portals.json"));
    let settings: serde_json::Value = crate::paths::read_json(&dir.join("settings.json"));
    let domains = integrated_auth_domains(&portals, settings["defaultDomain"].as_str().unwrap_or(""));
    (!domains.is_empty()).then(|| format!("--auth-server-allowlist={}", domains.join(",")))
}

/// Variable que lee el propio WebView2 al arrancar. `run()` la deja puesta con
/// los argumentos definitivos de la ventana principal.
pub const ARGS_ENV: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";

/// Los argumentos de la ventana principal más, si hay intranets de empresa, la
/// lista de dominios con los que entrar usando la cuenta de Windows.
pub fn merged_browser_args(base: &str, auth: Option<&str>) -> String {
    match auth {
        Some(a) if !a.is_empty() => format!("{base} {a}").trim().to_string(),
        _ => base.trim().to_string(),
    }
}

/// Los argumentos del navegador de un portal.
///
/// **Tienen que ser exactamente las mismas opciones con las que se creó la
/// ventana principal**, que son las del archivo de configuración. WebView2 no
/// admite dos configuraciones distintas en la misma carpeta de datos: si no
/// coinciden, se niega a crear la vista del portal y la página se queda para
/// siempre en «Abriendo…», sin ningún error a la vista.
///
/// La lista de dominios de la intranet (`--auth-server-allowlist`) **no** va
/// aquí. Va en la variable de entorno `ARGS_ENV`, que WebView2 aplica por igual a
/// todas las vistas por su cuenta. Añadirla también en las opciones del portal
/// (se hizo en la 1.1.6 creyendo que faltaba) dejaba al portal distinto de la
/// ventana principal y rompía el Correo, Teams y Tickets a la vez en cuanto
/// había una intranet configurada.
fn browser_args(app: &tauri::AppHandle) -> Option<String> {
    portal_browser_args(app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()))
}

/// Las opciones del portal a partir de las de la ventana principal: las mismas,
/// sin mirar la variable de entorno. Separado para poder probarlo.
fn portal_browser_args(main_window: Option<String>) -> Option<String> {
    main_window
}

macro_rules! configure {
    ($builder:expr, $app:expr, $portal:expr, $label:expr, $get:ident) => {{
        let (p_nav, p_new, p_load, app_new, app_load, app_title, app_dl, label_new) =
            ($portal.clone(), $portal.clone(), $portal.clone(), $app.clone(), $app.clone(), $app.clone(), $app.clone(), $label.clone());
        let app_blocked = $app.clone();
        let (id, id_title, id_dl) = ($portal.id.clone(), $portal.id.clone(), $portal.id.clone());
        let popups_in_window = $portal.popups == "window";
        let mut b = $builder
            .incognito($portal.private)
            .zoom_hotkeys_enabled(true)
            .on_navigation(move |url| {
                // Las páginas web las decide `guard_navigation`, que sabe si es una
                // redirección o un formulario; aquí solo se sabe la dirección.
                if matches!(url.scheme(), "http" | "https") {
                    return true;
                }
                let ok = allowed(&p_nav, url);
                if !ok {
                    open_external(url.as_str());
                }
                ok
            })
            .on_new_window(move |url, _| {
                if !allowed(&p_new, &url) {
                    open_external(url.as_str());
                    report_blocked(&app_blocked, &p_new, &url);
                    return NewWindowResponse::Deny;
                }
                // Webs como Outlook abren mensajes y adjuntos en su propia ventana.
                if popups_in_window {
                    return NewWindowResponse::Allow;
                }
                // Si no, dentro del portal se abren en la misma vista.
                let (app, label) = (app_new.clone(), label_new.clone());
                std::thread::spawn(move || {
                    if let Some(w) = app.$get(&label) {
                        let _ = w.navigate(url);
                    }
                });
                NewWindowResponse::Deny
            })
            .on_document_title_changed(move |_, title| {
                let _ = app_title.emit("portal-title", TitleEvent { id: id_title.clone(), title });
            })
            .on_download(move |_, event| {
                let mut list = DOWNLOADS.lock().unwrap_or_else(|e| e.into_inner());
                let info = match event {
                    DownloadEvent::Requested { url, destination } => {
                        let number = NEXT_DOWNLOAD.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        list.push(Download { number, url: url.clone(), path: destination.clone(), done: false });
                        // Las más viejas se olvidan; su número no se reutiliza.
                        if list.len() > MAX_DOWNLOADS {
                            let sobran = list.len() - MAX_DOWNLOADS;
                            list.drain(..sobran);
                        }
                        DownloadInfo { id: id_dl.clone(), download: number, name: file_name(destination, &url), state: "running".into() }
                    }
                    DownloadEvent::Finished { url, path, success } => {
                        let Some(i) = list.iter().rposition(|d| d.url == url && !d.done) else { return true };
                        if let Some(p) = path {
                            list[i].path = p;
                        }
                        list[i].done = true;
                        let name = file_name(&list[i].path, &url);
                        DownloadInfo { id: id_dl.clone(), download: list[i].number, name, state: if success { "done" } else { "failed" }.into() }
                    }
                    _ => return true,
                };
                drop(list);
                let _ = app_dl.emit("portal-download", info);
                true
            })
            .on_page_load(move |view, payload| {
                let loading = matches!(payload.event(), PageLoadEvent::Started);
                let _ = app_load.emit("portal-load", LoadEvent { id: id.clone(), url: payload.url().to_string(), loading });
                if loading {
                    return;
                }
                if let Some(microsoft) = autofill_target(&p_load, payload.url()) {
                    if let Some((user, pass)) = login_of(&app_load, &p_load.id) {
                        let _ = view.eval(autofill_script(&user, &pass, microsoft));
                    }
                }
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
            let Some(id) = v.label().strip_prefix("portal-") else { continue };
            if Some(v.label()) == except {
                continue;
            }
            let _ = v.hide();
            LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), Instant::now());
        }
    }
}

/// Crea la vista incrustada de un portal (oculta si `visible` es falso).
fn create_embedded(app: &tauri::AppHandle, p: &Portal, pos: LogicalPosition<f64>, size: LogicalSize<f64>, visible: bool) -> Result<tauri::Webview, String> {
    let label = embedded_label(&p.id);
    let start = PENDING.lock().unwrap_or_else(|e| e.into_inner()).remove(&p.id);
    let url: Url = match start {
        Some(u) => u,
        None => p.url.parse().map_err(|_| "La dirección del portal no es válida.".to_string())?,
    };
    CREATED.lock().unwrap_or_else(|e| e.into_inner()).insert(p.id.clone(), Instant::now());
    let t = Instant::now();
    let builder = configure!(tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(url)), app, p, label, get_webview);
    // Oculta: se crea fuera de la vista y se esconde, para no verse un instante arriba a la izquierda.
    let (pos, size) = if visible { (pos, size) } else { (LogicalPosition::new(-30000.0, -30000.0), size) };
    let v = main_window(app)?.add_child(builder, pos, size).map_err(|e| format!("No se pudo abrir el portal: {e}"))?;
    if !visible {
        let _ = v.hide();
    }
    guard_navigation(&v, app, p);
    enable_autofill(&v);
    accept_router_certificates(&v, p);
    watch_navigation(&v, app, &p.id);
    if p.zoom_factor() != 1.0 {
        let _ = v.set_zoom(p.zoom_factor());
    }
    log::info!("Portal «{}»: vista creada en {} ms{}", p.name, t.elapsed().as_millis(), if visible { "" } else { " (precarga)" });
    Ok(v)
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
        // Empresas con su propia página de inicio de sesión (p. ej. sts.pgr.gob.do).
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
            p("https://inventario.pgr.gob.do/equipos", "inventory"),
            p("https://tickets.pgr.gob.do", ""),
            p("http://192.168.1.1", "router"),
            p("https://intranet", ""),
            p("https://outlook.office.com/mail/", "mail"),
        ];
        assert_eq!(integrated_auth_domains(&portals, "pgr.gob.do"), vec!["inventario.pgr.gob.do", "*.pgr.gob.do", "tickets.pgr.gob.do"]);
        assert!(integrated_auth_domains(&[], "").is_empty());
        assert_eq!(integrated_auth_domains(&[], "empresa.local"), vec!["*.empresa.local"]);
    }

    fn portal() -> Portal {
        Portal { id: "p1".into(), name: "Intranet".into(), url: "https://intranet.pgr.gob.do/tickets".into(), extra_domains: vec!["login.microsoftonline.com".into()], ..Default::default() }
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
        let auth = "--auth-server-allowlist=*.pgr.gob.do";
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
        std::env::set_var(ARGS_ENV, "--disable-features=msWebOOUI --auth-server-allowlist=*.pgr.gob.do");
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
    fn navigation_stays_inside_the_portal_domains() {
        let p = portal();
        let ok = |u: &str| allowed(&p, &u.parse().unwrap());
        assert!(ok("https://intranet.pgr.gob.do/soporte/123"));
        assert!(ok("https://archivos.intranet.pgr.gob.do/x.pdf"));
        assert!(ok("https://login.microsoftonline.com/oauth"));
        assert!(ok("about:blank"));
        // El resto de la casa: el inicio de sesión suele estar en un hermano.
        assert!(ok("https://sso.pgr.gob.do/login"));
        assert!(ok("https://pgr.gob.do/"));
        assert!(!ok("https://otra.gob.do/"));
        assert!(!ok("https://intranet.pgr.gob.do.evil.com/"));
        assert!(!ok("https://google.com/"));
        assert!(!ok("file:///C:/Windows/win.ini"));
        assert!(!ok("javascript:alert(1)"));
    }

    /// «Entrar con Microsoft / Google» funciona sin configurar nada, y solo por https.
    #[test]
    fn sign_in_buttons_work_out_of_the_box() {
        let p = Portal { id: "sso".into(), name: "Tickets".into(), url: "https://tickets.empresa.com".into(), ..Default::default() };
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
        let p = Portal { id: "aprende".into(), name: "X".into(), url: "https://tickets.empresa.com".into(), ..Default::default() };
        let idp: Url = "https://idp.proveedor.net/sso".parse().unwrap();
        assert!(!allowed(&p, &idp));
        learn(&p.id, "idp.proveedor.net");
        assert!(allowed(&p, &idp));
        assert!(!allowed(&Portal { id: "otro".into(), ..p.clone() }, &idp));
        LEARNED.lock().unwrap().remove("aprende");
    }

    #[test]
    fn validates_portals() {
        let v = validate(Portal { name: " Intranet ".into(), url: "intranet.pgr.gob.do".into(), extra_domains: vec!["https://*.sso.pgr.gob.do/".into()], zoom: 1.234, popups: "otra".into(), ..Default::default() }).unwrap();
        assert_eq!(v.url, "https://intranet.pgr.gob.do");
        assert_eq!(v.name, "Intranet");
        assert_eq!(v.extra_domains, vec!["sso.pgr.gob.do"]);
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
