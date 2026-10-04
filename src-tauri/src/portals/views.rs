//! Las vistas: crearlas, sus eventos, descargas y argumentos del navegador.

use super::*;

// ---------- Estado de las vistas ----------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoadEvent {
    pub(super) id: String,
    pub(super) url: String,
    pub(super) loading: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct StateEvent {
    pub(super) id: String,
    pub(super) can_back: bool,
    pub(super) can_forward: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct ErrorEvent {
    pub(super) id: String,
    /// Vacío: la última carga fue bien.
    pub(super) message: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct TitleEvent {
    pub(super) id: String,
    pub(super) title: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct DownloadInfo {
    pub(super) id: String,
    /// Número propio de la descarga (para abrirla sin pasar su ruta a la interfaz).
    pub(super) download: u64,
    /// Solo el nombre del archivo: la ruta lleva el nombre del usuario.
    pub(super) name: String,
    /// "running" | "done" | "failed"
    pub(super) state: String,
}

/// Una descarga de esta sesión: dónde quedó el archivo y si terminó.
pub(super) struct Download {
    /// Número propio, que no cambia aunque se poden las viejas.
    pub(super) number: u64,
    pub(super) url: Url,
    pub(super) path: PathBuf,
    pub(super) done: bool,
}

/// Descargas de esta sesión. Se podan las más viejas: antes la lista crecía sin
/// límite mientras AdminOps estuviera abierto, porque el número de cada descarga
/// era su posición en ella y quitar una habría descolocado a todas las demás.
pub(super) static DOWNLOADS: LazyLock<Mutex<Vec<Download>>> = LazyLock::new(Default::default);
/// Cuántas descargas se recuerdan (la interfaz enseña 20 por portal).
pub(super) const MAX_DOWNLOADS: usize = 200;
/// Número de la siguiente descarga.
pub(super) static NEXT_DOWNLOAD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
/// Cuándo se creó cada vista, para medir cuánto tarda en cargar la primera vez.
pub(super) static CREATED: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);
/// Dirección a la que ir al crear la vista (p. ej. «Redactar» con el correo aún cerrado).
pub(super) static PENDING: LazyLock<Mutex<HashMap<String, Url>>> = LazyLock::new(Default::default);
/// Cuándo se vio por última vez cada portal incrustado.
pub(super) static LAST_SEEN: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);
/// Último rectángulo aplicado a cada vista. Colocar una vista hay que pedírselo
/// al hilo de la ventana y eso espera: al redimensionar llegaban decenas de
/// peticiones iguales y la interfaz se quedaba tiesa. Si no ha cambiado, no se toca.
/// Rectángulo de una vista: x, y, ancho y alto en píxeles lógicos.
pub(super) type Rect = (f64, f64, f64, f64);
pub(super) static BOUNDS: LazyLock<Mutex<HashMap<String, Rect>>> = LazyLock::new(Default::default);
/// Una vista sin usarse este tiempo se cierra (vuelve a abrirse al entrar).
///
/// Cerrarla libera un proceso de navegador entero, pero **volver a entrar cuesta
/// una carga completa**: Teams y Outlook tardan lo suyo en arrancar, y media hora
/// es poquísimo para una jornada de trabajo. En el equipo del técnico se aguantan
/// vivas casi toda la sesión; en uno justo de recursos se sigue cerrando pronto,
/// que es donde la memoria importa de verdad.
pub(super) fn idle_close() -> Duration {
    if crate::pspool::modest_machine() {
        Duration::from_secs(30 * 60)
    } else {
        Duration::from_secs(4 * 60 * 60)
    }
}

pub(super) fn file_name(p: &std::path::Path, url: &Url) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .or_else(|| url.path_segments().and_then(|mut s| s.next_back().map(String::from)))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "archivo".into())
}

/// Explicación de por qué no cargó la página.
pub(super) fn explain_web_error(status: i32) -> Option<String> {
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
pub(super) fn watch_navigation<R: tauri::Runtime>(webview: &tauri::Webview<R>, app: &tauri::AppHandle, id: &str) {
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
pub(super) fn enable_autofill<R: tauri::Runtime>(webview: &tauri::Webview<R>) {
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
pub(super) fn forget(id: &str) {
    BOUNDS.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    LAST_SEEN.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    CREATED.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
}

/// Los routers usan un certificado propio que el navegador no reconoce: en el
/// panel de un router se acepta, pero solo para direcciones de la red local.
pub(super) fn accept_router_certificates<R: tauri::Runtime>(webview: &tauri::Webview<R>, p: &Portal) {
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
/// como *.empresa.local). Solo los de los portales del técnico y su dominio habitual.
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
        // inventario.empresa.com → *.empresa.com (el resto de webs de la empresa).
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
pub(super) fn browser_args(app: &tauri::AppHandle) -> Option<String> {
    portal_browser_args(app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()))
}

/// Las opciones del portal a partir de las de la ventana principal: las mismas,
/// sin mirar la variable de entorno. Separado para poder probarlo.
pub(super) fn portal_browser_args(main_window: Option<String>) -> Option<String> {
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

pub(super) fn main_window(app: &tauri::AppHandle) -> Result<tauri::Window, String> {
    app.get_window("main").ok_or_else(|| "No se encontró la ventana principal.".into())
}

pub(super) fn hide_embedded(app: &tauri::AppHandle, except: Option<&str>) {
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
pub(super) fn create_embedded(app: &tauri::AppHandle, p: &Portal, pos: LogicalPosition<f64>, size: LogicalSize<f64>, visible: bool) -> Result<tauri::Webview, String> {
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
