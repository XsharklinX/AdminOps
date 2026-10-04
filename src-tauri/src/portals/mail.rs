//! Correo (Outlook) y Teams: dominios, redactar y cerrar sesión.

use super::*;

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
pub(super) fn is_microsoft_kind(kind: &str) -> bool {
    kind == "mail" || kind == "teams"
}

/// Páginas de inicio de sesión de Microsoft (donde se rellena la cuenta guardada).
pub(super) fn is_microsoft_login(host: &str) -> bool {
    ["login.microsoftonline.com", "login.live.com", "login.microsoft.com"].iter().any(|d| host.eq_ignore_ascii_case(d))
}

/// ¿Es el Outlook personal (outlook.com / hotmail) y no el del trabajo (Microsoft 365)?
pub(super) fn is_personal_outlook(p: &Portal) -> bool {
    Url::parse(&p.url).ok().and_then(|u| u.host_str().map(|h| host_matches(h, "live.com") || host_matches(h, "outlook.com"))).unwrap_or(false)
}

/// Enlace de Outlook para escribir un mensaje nuevo.
pub(super) fn compose_url(p: &Portal, to: &str, subject: &str, body: &str) -> Url {
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
pub(super) fn compose_click_script(fallback: &Url) -> String {
    let url = serde_json::to_string(fallback.as_str()).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
  pub(super) const fallback = {url};
  if (window.__adminopsCompose) return;
  if (!location.pathname.startsWith("/mail")) {{ location.assign(fallback); return; }}
  window.__adminopsCompose = true;
  pub(super) const names = ["new mail", "new message", "new email", "correo nuevo", "nuevo correo", "mensaje nuevo", "nuevo mensaje", "correo electrónico nuevo"];
  pub(super) const norm = (s) => (s || "").replace(/\s+/g, " ").trim().toLowerCase();
  pub(super) const find = () => Array.from(document.querySelectorAll('button, [role="button"]')).find((el) =>
    el.offsetParent !== null && !el.disabled && el.getAttribute("aria-disabled") !== "true" &&
    [el.getAttribute("aria-label"), el.getAttribute("title"), el.textContent].some((t) => names.includes(norm(t))));
  let tries = 0;
  pub(super) const tick = () => {{
    pub(super) const b = find();
    if (b) {{ window.__adminopsCompose = false; b.click(); return; }}
    if (++tries > 40) {{ window.__adminopsCompose = false; location.assign(fallback); return; }}
    setTimeout(tick, 150);
  }};
  tick();
}})();"#
    )
}

pub(super) fn sign_out_url(p: &Portal) -> &'static str {
    match (p.kind.as_str(), is_personal_outlook(p)) {
        // Teams no tiene una página propia de salida: se cierra la sesión de Microsoft.
        ("teams", _) => "https://login.microsoftonline.com/common/oauth2/v2.0/logout",
        (_, true) => "https://login.live.com/logout.srf",
        (_, false) => "https://outlook.office.com/owa/logoff.owa",
    }
}
