//! Por dónde puede navegar cada portal y qué se abre fuera.

use super::*;

pub(super) fn embedded_label(id: &str) -> String {
    format!("portal-{id}")
}

pub(super) fn window_label(id: &str) -> String {
    format!("portalwin-{id}")
}

/// `host` es `domain` o un subdominio suyo.
pub(super) fn host_matches(host: &str, domain: &str) -> bool {
    let (h, d) = (host.to_ascii_lowercase(), domain.to_ascii_lowercase());
    h == d || h.ends_with(&format!(".{d}"))
}

/// Terminaciones que no son de nadie («com.do», «gob.do», «co.uk»): bajo ellas
/// hay empresas distintas, así que no sirven para decir «es de la misma casa».
pub(super) fn public_suffix_like(labels: &[&str]) -> bool {
    labels.len() < 2
        || (labels.len() == 2 && labels[1].len() == 2 && ["com", "co", "org", "net", "gob", "gov", "edu", "ac", "mil", "nom", "web", "sld", "art", "gouv", "or", "ne", "go"].contains(&labels[0]))
}

/// ¿Es `host` de la misma casa que el portal (`own`)? El inicio de sesión de una
/// intranet casi nunca está en el mismo nombre que la web: va en un hermano
/// (`intranet.empresa.com` → `sso.empresa.com`), o la web se abre por su nombre
/// corto y contesta con el largo o con su IP.
pub(super) fn same_org(host: &str, own: &str) -> bool {
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
pub(super) static LEARNED: LazyLock<Mutex<HashMap<String, HashSet<String>>>> = LazyLock::new(Default::default);

pub(super) fn learn(id: &str, host: &str) {
    LEARNED.lock().unwrap_or_else(|e| e.into_inner()).entry(id.to_string()).or_default().insert(host.to_ascii_lowercase());
}

pub(super) fn learned(id: &str, host: &str) -> bool {
    LEARNED.lock().unwrap_or_else(|e| e.into_inner()).get(id).is_some_and(|set| set.iter().any(|d| host_matches(host, d)))
}

/// Páginas de inicio de sesión que usan media internet: «Entrar con Microsoft»,
/// «Entrar con Google», Okta, Auth0… El botón que lleva a ellas es un enlace
/// normal que pulsa el usuario, así que no se distingue de un enlace a otro
/// sitio: se admiten de fábrica en todos los portales.
pub(super) const IDENTITY_PROVIDERS: &[&str] = &[
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

/// Tickets e Inventario no limitan por dónde se navega. Se limitaba al sitio
/// del portal y eso rompía intranets de verdad: el inicio de sesión en otro
/// servidor, los adjuntos en otro, los enlaces entre sistemas de la empresa.
pub(super) fn free_navigation(p: &Portal) -> bool {
    p.kind.is_empty() || p.kind == "inventory"
}

/// ¿Puede el portal navegar a `url` sin salir de la app?
pub fn allowed(p: &Portal, url: &Url) -> bool {
    match url.scheme() {
        "about" | "data" | "blob" => true,
        "http" | "https" if free_navigation(p) => url.host_str().is_some(),
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
pub(super) fn off_site(redirected: bool, user_initiated: bool, form: bool) -> Option<&'static str> {
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
pub(super) struct BlockedEvent {
    pub(super) id: String,
    /// Sitio que se abrió fuera (solo el nombre: la dirección puede llevar datos).
    pub(super) host: String,
}

/// Avisa a la interfaz de que algo se abrió en el navegador de fuera, para que
/// el técnico pueda permitir ese sitio en el portal con un clic.
pub(super) fn report_blocked(app: &tauri::AppHandle, p: &Portal, url: &Url) {
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
pub(super) fn guard_navigation<R: tauri::Runtime>(webview: &tauri::Webview<R>, app: &tauri::AppHandle, p: &Portal) {
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
pub(super) fn open_external(url: &str) {
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:") {
        let _ = crate::shellopen::open(url);
    }
}
