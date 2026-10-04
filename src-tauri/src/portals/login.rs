//! Inicio de sesión guardado y relleno automático.

use super::*;

// ---------- Inicio de sesión guardado ----------

/// Cuenta de un portal. La contraseña va cifrada como las de los routers: en
/// portable, con la clave del USB (viaja con él); instalado, con la de Windows.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct Login {
    pub(super) user: String,
    #[serde(default)]
    pub(super) secret: String,
}

pub(super) fn logins_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("portal_logins.json")
}

pub(super) fn load_logins(app: &tauri::AppHandle) -> HashMap<String, Login> {
    crate::paths::read_json(&logins_path(app))
}

pub(super) fn login_of(app: &tauri::AppHandle, id: &str) -> Option<(String, String)> {
    let l = load_logins(app).remove(id)?;
    let pass = if l.secret.is_empty() { String::new() } else { crate::secrets::open(&l.secret).ok()? };
    Some((l.user, pass))
}

/// Script que rellena el inicio de sesión. En las páginas de Microsoft sigue sus
/// dos pasos (cuenta → Siguiente → contraseña → Iniciar sesión) una sola vez; en
/// las demás solo rellena, sin enviar. Los datos van como texto JSON, nunca como código.
pub(super) fn autofill_script(user: &str, pass: &str, microsoft: bool) -> String {
    let js = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(() => {{
  if (window.__adminopsFill) return;
  window.__adminopsFill = true;
  pub(super) const user = {user}, pass = {pass}, microsoft = {microsoft};
  pub(super) const set = (el, v) => {{
    pub(super) const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), "value");
    if (d && d.set) d.set.call(el, v); else el.value = v;
    el.dispatchEvent(new Event("input", {{ bubbles: true }}));
    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
  }};
  pub(super) const visible = (el) => !!el && el.offsetParent !== null && !el.disabled && !el.readOnly;
  pub(super) const next = () => setTimeout(() => {{ const b = document.getElementById("idSIButton9"); if (b) b.click(); }}, 400);
  let userDone = !user, passDone = !pass, tries = 0;
  pub(super) const tick = () => {{
    tries++;
    if (microsoft) {{
      pub(super) const u = document.querySelector('input[name="loginfmt"]');
      if (!userDone && visible(u)) {{ if (!u.value) {{ set(u, user); next(); }} userDone = true; }}
      pub(super) const p = document.querySelector('input[name="passwd"]');
      if (!passDone && visible(p)) {{ if (!p.value) {{ set(p, pass); next(); }} passDone = true; }}
    }} else {{
      pub(super) const p = Array.from(document.querySelectorAll('input[type="password"]')).find(visible);
      if (p) {{
        pub(super) const scope = p.form || document;
        pub(super) const inputs = Array.from(scope.querySelectorAll("input")).filter(visible);
        pub(super) const u = inputs.slice(0, inputs.indexOf(p)).reverse().find((x) => ["text", "email", ""].includes((x.getAttribute("type") || "").toLowerCase()));
        if (u && user && !u.value) set(u, user);
        if (pass && !p.value) set(p, pass);
        userDone = passDone = true;
      }}
    }}
    if ((userDone && passDone) || tries > 60) clearInterval(timer);
  }};
  pub(super) const timer = setInterval(tick, 500);
  tick();
}})();"#,
        user = js(user),
        pass = js(pass),
        microsoft = microsoft
    )
}

/// ¿Hay que rellenar el inicio de sesión en esta página? Solo en las de
/// Microsoft (correo) o en el propio dominio del portal, y nunca por http en Internet.
pub(super) fn autofill_target(p: &Portal, url: &Url) -> Option<bool> {
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
