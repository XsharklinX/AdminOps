//! Microsoft 365 a través de Microsoft Graph, con la cuenta del técnico.
//!
//! Lo que el dominio local no sabe y Microsoft 365 sí:
//! - **por qué falla un inicio de sesión** (los registros de Entra ID, con el
//!   motivo y el paso del MFA en que se quedó),
//! - **los métodos de MFA** de una persona, para quitar el del móvil que perdió y
//!   que tenga que registrarlos de nuevo, y **cerrar todas sus sesiones**,
//! - **el estado de Microsoft 365** («Exchange Online tiene una incidencia»),
//! - **el calendario de Outlook** (las visitas de la Agenda) y **Teams** (avisar
//!   a alguien de que su equipo está listo).
//!
//! **Inicio de sesión por código de dispositivo.** AdminOps enseña un código, el
//! técnico lo escribe en microsoft.com/devicelogin desde su navegador de
//! siempre y hace allí el MFA como siempre. No pasa por ninguna vista web
//! integrada. El token de renovación se guarda cifrado (como las contraseñas de
//! los portales) y el de acceso solo en memoria.
//!
//! **Con permisos delegados**: la aplicación la registra IT en Entra ID y solo
//! puede hacer lo que IT le haya concedido *y* lo que la cuenta del técnico pueda
//! hacer por su rol. Se pide `.default`: lo que IT haya aprobado, sin más; si falta
//! un permiso concreto, esa función lo dice y las demás siguen funcionando.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;

const GRAPH: &str = "https://graph.microsoft.com/v1.0";
const LOGIN: &str = "https://login.microsoftonline.com";
/// Lo que IT haya aprobado para la aplicación, más poder renovar la sesión.
const SCOPE: &str = "https://graph.microsoft.com/.default offline_access openid profile";

static FILE_LOCK: Mutex<()> = Mutex::new(());
/// Token de acceso en memoria (dura ~1 h) y cuándo caduca.
static ACCESS: LazyLock<Mutex<Option<(String, Instant)>>> = LazyLock::new(Default::default);
/// Inicio de sesión por código en curso.
static DEVICE: LazyLock<Mutex<Option<DeviceFlow>>> = LazyLock::new(Default::default);

struct DeviceFlow {
    device_code: String,
    expires: Instant,
}

// ---------- Configuración ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct Stored {
    tenant: String,
    client_id: String,
    /// Token de renovación, cifrado (secrets::seal). Vacío = sin sesión.
    refresh: String,
    /// Cuenta con la que se inició sesión (para enseñarla).
    account: String,
}

fn path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("graph.json")
}

fn load(app: &tauri::AppHandle) -> Stored {
    crate::paths::read_json(&path(app))
}

fn store(app: &tauri::AppHandle, s: &Stored) -> Result<(), String> {
    crate::paths::write_json(&path(app), s)
}

/// Id de inquilino: un GUID o un dominio (empresa.onmicrosoft.com, empresa.gob.do).
pub fn valid_tenant(t: &str) -> bool {
    let t = t.trim();
    !t.is_empty() && t.len() <= 100 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && (is_guid(t) || t.contains('.'))
}

pub fn is_guid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5 && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Cuerpo `application/x-www-form-urlencoded` (lo que piden los puntos de inicio de sesión).
pub fn form(pairs: &[(&str, &str)]) -> String {
    fn enc(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
                b' ' => "+".to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    }
    pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect::<Vec<_>>().join("&")
}

/// Cuenta del token de identidad (sin comprobar la firma: solo es para enseñarla).
pub fn account_from_id_token(id_token: &str) -> String {
    use base64::Engine;
    let Some(payload) = id_token.split('.').nth(1) else { return String::new() };
    let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')) else { return String::new() };
    let v: Value = serde_json::from_slice(&bytes).unwrap_or_default();
    v["preferred_username"].as_str().or_else(|| v["upn"].as_str()).unwrap_or("").to_string()
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(Duration::from_secs(25)).user_agent("AdminOps").build().map_err(|e| e.to_string())
}

// ---------- Errores ----------

/// Los errores de Microsoft, explicados. `code` es el de Graph o el de inicio de sesión.
pub fn explain(status: u16, code: &str, message: &str) -> String {
    let c = code.to_ascii_lowercase();
    if c.contains("nonpremium") || message.contains("premium") {
        return "Los registros de inicio de sesión necesitan Entra ID P1 o P2 en la organización.".into();
    }
    match status {
        401 => "La sesión de Microsoft 365 ha caducado. Vuelve a conectar en Ajustes → Portales y correo → Microsoft 365.".into(),
        403 => "Microsoft 365 no deja hacer esto: o IT no ha dado ese permiso a AdminOps, o tu cuenta no tiene el rol necesario (por ejemplo, administrador de autenticación o lector de informes).".into(),
        404 => "Microsoft 365 no encuentra a esa persona o ese elemento. Revisa que el usuario sea el de Microsoft 365 (normalmente su correo).".into(),
        429 => "Microsoft 365 pide esperar un momento antes de volver a consultar.".into(),
        _ if message.is_empty() => format!("Microsoft 365 respondió con un error ({status})."),
        _ => format!("Microsoft 365: {message}"),
    }
}

// ---------- Tokens ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    id_token: String,
    error: String,
    error_description: String,
}

async fn token_request(tenant: &str, body: String) -> Result<(u16, TokenResponse), String> {
    let r = client()?
        .post(format!("{LOGIN}/{tenant}/oauth2/v2.0/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|_| "No se llega a Microsoft 365. Revisa la conexión a Internet.".to_string())?;
    let status = r.status().as_u16();
    let t: TokenResponse = r.json().await.unwrap_or_default();
    Ok((status, t))
}

/// Guarda los tokens nuevos (el de renovación cifrado y el de acceso en memoria).
fn keep_tokens(app: &tauri::AppHandle, t: &TokenResponse) -> Result<(), String> {
    *ACCESS.lock().unwrap_or_else(|e| e.into_inner()) = Some((t.access_token.clone(), Instant::now() + Duration::from_secs(t.expires_in.saturating_sub(120).max(60))));
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(app);
    if !t.refresh_token.is_empty() {
        s.refresh = crate::secrets::seal(&t.refresh_token)?;
    }
    if !t.id_token.is_empty() {
        let a = account_from_id_token(&t.id_token);
        if !a.is_empty() {
            s.account = a;
        }
    }
    store(app, &s)
}

/// Un token de acceso válido: el de memoria o uno renovado.
async fn access_token(app: &tauri::AppHandle) -> Result<String, String> {
    if let Some((t, until)) = ACCESS.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        if Instant::now() < until {
            return Ok(t);
        }
    }
    let s = load(app);
    if s.refresh.is_empty() {
        return Err("Microsoft 365 no está conectado. Conéctalo en Ajustes → Portales y correo → Microsoft 365.".into());
    }
    let refresh = crate::secrets::open(&s.refresh).map_err(|_| "No se pudo leer la sesión guardada de Microsoft 365. Vuelve a conectar.".to_string())?;
    let (status, t) = token_request(&s.tenant, form(&[("grant_type", "refresh_token"), ("client_id", &s.client_id), ("refresh_token", &refresh), ("scope", SCOPE)])).await?;
    if status != 200 || t.access_token.is_empty() {
        // La sesión ya no vale (caducó o la revocaron): hay que volver a conectar.
        let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = load(app);
        s.refresh.clear();
        let _ = store(app, &s);
        return Err("La sesión de Microsoft 365 ha caducado. Vuelve a conectar en Ajustes → Portales y correo → Microsoft 365.".into());
    }
    keep_tokens(app, &t)?;
    Ok(t.access_token)
}

/// Llamada a Graph. Con un 401 renueva una vez y reintenta.
async fn call(app: &tauri::AppHandle, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value, String> {
    for intento in 0..2 {
        let token = access_token(app).await?;
        let mut req = client()?.request(method.clone(), format!("{GRAPH}{path}")).bearer_auth(&token);
        if let Some(b) = &body {
            req = req.json(b);
        }
        let r = req.send().await.map_err(|_| "No se llega a Microsoft 365. Revisa la conexión a Internet.".to_string())?;
        let status = r.status().as_u16();
        if status == 401 && intento == 0 {
            *ACCESS.lock().unwrap_or_else(|e| e.into_inner()) = None;
            continue;
        }
        let text = r.text().await.unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            return Ok(v);
        }
        let code = v["error"]["code"].as_str().unwrap_or("");
        let msg = v["error"]["message"].as_str().unwrap_or("");
        log::warn!("Graph {path}: {status} {code}");
        return Err(explain(status, code, msg));
    }
    Err(explain(401, "", ""))
}

/// Usuario para una ruta de Graph: su UPN (normalmente el correo), sin nada raro.
fn user_path(upn: &str) -> Result<String, String> {
    let u = upn.trim();
    let ok = u.len() <= 120 && u.contains('@') && u.chars().all(|c| c.is_ascii_alphanumeric() || "@.-_'".contains(c));
    if !ok {
        return Err("Esa persona no tiene una cuenta de Microsoft 365 reconocible (hace falta su usuario, normalmente el correo).".into());
    }
    Ok(format!("/users/{}", u.replace('\'', "''")))
}

// ---------- Conexión ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatus {
    configured: bool,
    connected: bool,
    account: String,
    tenant: String,
    client_id: String,
}

#[tauri::command]
pub fn graph_status(app: tauri::AppHandle) -> GraphStatus {
    let s = load(&app);
    GraphStatus { configured: !s.tenant.is_empty() && !s.client_id.is_empty(), connected: !s.refresh.is_empty(), account: s.account, tenant: s.tenant, client_id: s.client_id }
}

/// Guarda el inquilino y el id de la aplicación que dio IT. Si cambian, se cierra la sesión.
#[tauri::command]
pub fn graph_configure(app: tauri::AppHandle, tenant: String, client_id: String) -> Result<(), String> {
    let (tenant, client_id) = (tenant.trim().to_string(), client_id.trim().to_ascii_lowercase());
    if !valid_tenant(&tenant) {
        return Err("El inquilino es un identificador (GUID) o el dominio de Microsoft 365 de la organización, por ejemplo empresa.onmicrosoft.com.".into());
    }
    if !is_guid(&client_id) {
        return Err("El id de la aplicación es un GUID que da IT al registrarla (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).".into());
    }
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(&app);
    if s.tenant != tenant || s.client_id != client_id {
        s.refresh.clear();
        s.account.clear();
        *ACCESS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    s.tenant = tenant;
    s.client_id = client_id;
    store(&app, &s)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

/// Empieza a conectar: devuelve el código que el técnico escribe en microsoft.com/devicelogin.
#[tauri::command]
pub async fn graph_login_start(app: tauri::AppHandle) -> Result<DeviceCode, String> {
    let s = load(&app);
    if s.tenant.is_empty() || s.client_id.is_empty() {
        return Err("Primero escribe el inquilino y el id de la aplicación que te dio IT.".into());
    }
    let r = client()?
        .post(format!("{LOGIN}/{}/oauth2/v2.0/devicecode", s.tenant))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form(&[("client_id", &s.client_id), ("scope", SCOPE)]))
        .send()
        .await
        .map_err(|_| "No se llega a Microsoft 365. Revisa la conexión a Internet.".to_string())?;
    let v: Value = r.json().await.unwrap_or_default();
    let (Some(device_code), Some(user_code)) = (v["device_code"].as_str(), v["user_code"].as_str()) else {
        let desc = v["error_description"].as_str().unwrap_or("").lines().next().unwrap_or("").to_string();
        return Err(if desc.contains("AADSTS7000218") || desc.contains("client_assertion") {
            "La aplicación de Entra ID no admite este inicio de sesión: IT tiene que activar «Permitir flujos de clientes públicos».".into()
        } else if desc.contains("AADSTS700016") {
            "Ese id de aplicación no existe en ese inquilino. Revísalo con IT.".into()
        } else if desc.contains("AADSTS90002") {
            "Ese inquilino no existe. Revisa el dominio o el GUID.".into()
        } else {
            format!("Microsoft 365 no dejó empezar el inicio de sesión. {desc}")
        });
    };
    let expires_in = v["expires_in"].as_u64().unwrap_or(900);
    *DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(DeviceFlow { device_code: device_code.to_string(), expires: Instant::now() + Duration::from_secs(expires_in) });
    Ok(DeviceCode {
        user_code: user_code.to_string(),
        verification_uri: v["verification_uri"].as_str().unwrap_or("https://microsoft.com/devicelogin").to_string(),
        expires_in,
        interval: v["interval"].as_u64().unwrap_or(5),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPoll {
    /// pending | done | expired | declined | error
    state: String,
    message: String,
    account: String,
}

/// ¿Terminó ya el técnico de iniciar sesión en el navegador?
#[tauri::command]
pub async fn graph_login_poll(app: tauri::AppHandle) -> Result<LoginPoll, String> {
    let code = {
        let d = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        match d.as_ref() {
            None => return Ok(LoginPoll { state: "error".into(), message: "No hay ningún inicio de sesión en curso.".into(), account: String::new() }),
            Some(f) if Instant::now() > f.expires => return Ok(LoginPoll { state: "expired".into(), message: "El código caducó. Vuelve a empezar.".into(), account: String::new() }),
            Some(f) => f.device_code.clone(),
        }
    };
    let s = load(&app);
    let (status, t) = token_request(&s.tenant, form(&[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", &s.client_id), ("device_code", &code)])).await?;
    if status == 200 && !t.access_token.is_empty() {
        keep_tokens(&app, &t)?;
        *DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let account = load(&app).account;
        log::info!("Microsoft 365 conectado");
        return Ok(LoginPoll { state: "done".into(), message: String::new(), account });
    }
    let (state, message) = match t.error.as_str() {
        "authorization_pending" | "slow_down" => ("pending", String::new()),
        "expired_token" => ("expired", "El código caducó. Vuelve a empezar.".to_string()),
        "authorization_declined" => ("declined", "Se canceló el inicio de sesión en el navegador.".to_string()),
        _ => ("error", t.error_description.lines().next().unwrap_or("No se pudo iniciar sesión.").to_string()),
    };
    if state != "pending" {
        *DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    Ok(LoginPoll { state: state.into(), message, account: String::new() })
}

/// Abre la página donde se escribe el código (siempre la de Microsoft, nunca otra).
#[tauri::command]
pub fn graph_open_devicelogin() -> Result<(), String> {
    crate::shellopen::open("https://microsoft.com/devicelogin")
}

/// Cierra la sesión de Microsoft 365 en AdminOps (borra los tokens guardados).
#[tauri::command]
pub fn graph_logout(app: tauri::AppHandle) -> Result<(), String> {
    *ACCESS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(&app);
    s.refresh.clear();
    s.account.clear();
    store(&app, &s)
}

// ---------- Inicios de sesión ----------

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    /// ISO 8601.
    pub when: String,
    pub ok: bool,
    pub code: i64,
    /// Qué pasó, en español.
    pub reason: String,
    pub app: String,
    pub client: String,
    pub ip: String,
    pub place: String,
    /// Detalle del MFA, si lo hubo (método y cómo acabó).
    pub mfa: String,
    /// Acceso condicional: success | failure | notApplied.
    pub conditional_access: String,
}

/// Los códigos de error de inicio de sesión más habituales, explicados. Los que
/// no están aquí se enseñan con el texto de Microsoft, sin inventar.
pub fn signin_reason(code: i64, fallback: &str) -> String {
    let m = match code {
        0 => "Correcto",
        50126 => "Usuario o contraseña incorrectos",
        50053 => "Cuenta bloqueada por demasiados intentos, o desde una dirección sospechosa",
        50057 => "La cuenta está desactivada",
        50055 => "La contraseña ha caducado",
        50074 | 50076 => "Hacía falta el MFA y no se completó (es normal si la persona se detuvo ahí)",
        500121 => "Falló la verificación MFA: no llegó a aprobarse o se rechazó",
        50158 => "Faltaba superar una comprobación de seguridad externa",
        53003 => "Bloqueado por una directiva de acceso condicional",
        50140 => "Se preguntó si mantener la sesión iniciada (no es un fallo)",
        50097 => "Hacía falta que el dispositivo estuviera registrado",
        _ => "",
    };
    if m.is_empty() {
        let f = fallback.trim();
        if f.is_empty() {
            format!("Error {code} de Microsoft")
        } else {
            format!("{f} (código {code})")
        }
    } else {
        m.to_string()
    }
}

pub fn parse_signins(v: &Value) -> Vec<SignIn> {
    v["value"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    let code = s["status"]["errorCode"].as_i64().unwrap_or(0);
                    let loc = &s["location"];
                    let place = [loc["city"].as_str(), loc["countryOrRegion"].as_str()].into_iter().flatten().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
                    let mfa = [s["mfaDetail"]["authMethod"].as_str(), s["mfaDetail"]["authDetail"].as_str()].into_iter().flatten().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
                    SignIn {
                        when: s["createdDateTime"].as_str().unwrap_or("").to_string(),
                        ok: code == 0,
                        code,
                        reason: signin_reason(code, s["status"]["failureReason"].as_str().unwrap_or("")),
                        app: s["appDisplayName"].as_str().unwrap_or("").to_string(),
                        client: s["clientAppUsed"].as_str().unwrap_or("").to_string(),
                        ip: s["ipAddress"].as_str().unwrap_or("").to_string(),
                        place,
                        mfa,
                        conditional_access: s["conditionalAccessStatus"].as_str().unwrap_or("").to_string(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Los últimos inicios de sesión de una persona (los 15 más recientes).
#[tauri::command]
pub async fn graph_signins(app: tauri::AppHandle, upn: String) -> Result<Vec<SignIn>, String> {
    user_path(&upn)?;
    let filtro = format!("userPrincipalName eq '{}'", upn.trim().replace('\'', "''"));
    let q = form(&[("$filter", &filtro), ("$top", "15"), ("$orderby", "createdDateTime desc")]).replace('+', "%20");
    Ok(parse_signins(&call(&app, reqwest::Method::GET, &format!("/auditLogs/signIns?{q}"), None).await?))
}

// ---------- Métodos de MFA y sesiones ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethod {
    /// phone | authenticator | fido2 | email | oath | hello | tap | password
    pub kind: String,
    pub id: String,
    /// Qué es, en español: «Teléfono móvil +1 809…», «Microsoft Authenticator (iPhone de María)».
    pub label: String,
    /// Se puede quitar desde aquí (la contraseña no).
    pub removable: bool,
}

/// Tipo de método → (clave propia, ruta para borrarlo, nombre).
fn method_kind(odata: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match odata {
        "#microsoft.graph.phoneAuthenticationMethod" => ("phone", "phoneMethods", "Teléfono"),
        "#microsoft.graph.microsoftAuthenticatorAuthenticationMethod" => ("authenticator", "microsoftAuthenticatorMethods", "Microsoft Authenticator"),
        "#microsoft.graph.fido2AuthenticationMethod" => ("fido2", "fido2Methods", "Llave de seguridad"),
        "#microsoft.graph.emailAuthenticationMethod" => ("email", "emailMethods", "Correo de recuperación"),
        "#microsoft.graph.softwareOathAuthenticationMethod" => ("oath", "softwareOathMethods", "App de códigos"),
        "#microsoft.graph.windowsHelloForBusinessAuthenticationMethod" => ("hello", "windowsHelloForBusinessMethods", "Windows Hello"),
        "#microsoft.graph.temporaryAccessPassAuthenticationMethod" => ("tap", "temporaryAccessPassMethods", "Pase de acceso temporal"),
        "#microsoft.graph.passwordAuthenticationMethod" => ("password", "", "Contraseña"),
        _ => return None,
    })
}

pub fn parse_methods(v: &Value) -> Vec<AuthMethod> {
    v["value"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    let (kind, route, name) = method_kind(m["@odata.type"].as_str().unwrap_or(""))?;
                    let extra = m["phoneNumber"].as_str().or_else(|| m["displayName"].as_str()).or_else(|| m["emailAddress"].as_str()).or_else(|| m["model"].as_str()).unwrap_or("");
                    Some(AuthMethod {
                        kind: kind.into(),
                        id: m["id"].as_str().unwrap_or("").into(),
                        label: if extra.is_empty() { name.to_string() } else { format!("{name} · {extra}") },
                        removable: !route.is_empty(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
pub async fn graph_mfa_methods(app: tauri::AppHandle, upn: String) -> Result<Vec<AuthMethod>, String> {
    let u = user_path(&upn)?;
    Ok(parse_methods(&call(&app, reqwest::Method::GET, &format!("{u}/authentication/methods"), None).await?))
}

/// Quita un método de MFA (el móvil que perdió, la app de un teléfono viejo).
/// Al quitar todos, la persona tendrá que registrarlos de nuevo al entrar.
#[tauri::command]
pub async fn graph_mfa_remove(app: tauri::AppHandle, upn: String, kind: String, id: String) -> Result<(), String> {
    let u = user_path(&upn)?;
    let route = [
        ("phone", "phoneMethods"),
        ("authenticator", "microsoftAuthenticatorMethods"),
        ("fido2", "fido2Methods"),
        ("email", "emailMethods"),
        ("oath", "softwareOathMethods"),
        ("hello", "windowsHelloForBusinessMethods"),
        ("tap", "temporaryAccessPassMethods"),
    ]
    .iter()
    .find(|(k, _)| *k == kind)
    .map(|(_, r)| *r)
    .ok_or("Ese método no se puede quitar desde aquí.")?;
    if id.is_empty() || id.len() > 100 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Método no válido.".into());
    }
    let r = call(&app, reqwest::Method::DELETE, &format!("{u}/authentication/{route}/{id}"), None).await.map(|_| ());
    let tweaks = app.state::<TweakState>();
    tweaks.record(Op::Run, &format!("Microsoft 365: quitado un método de MFA ({kind}) de {}", upn.trim()), &r);
    r
}

/// Cierra todas las sesiones de la persona en Microsoft 365 (tendrá que volver a entrar en todo).
#[tauri::command]
pub async fn graph_revoke_sessions(app: tauri::AppHandle, upn: String) -> Result<(), String> {
    let u = user_path(&upn)?;
    let r = call(&app, reqwest::Method::POST, &format!("{u}/revokeSignInSessions"), None).await.map(|_| ());
    let tweaks = app.state::<TweakState>();
    tweaks.record(Op::Run, &format!("Microsoft 365: cerradas todas las sesiones de {}", upn.trim()), &r);
    r
}

// ---------- Estado de Microsoft 365 ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceIssue {
    pub id: String,
    pub service: String,
    pub title: String,
    /// incident (algo no funciona) | advisory (funciona con limitaciones)
    pub classification: String,
    pub status: String,
    pub impact: String,
    pub start: String,
}

pub fn parse_issues(v: &Value) -> Vec<ServiceIssue> {
    let mut out: Vec<ServiceIssue> = v["value"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|i| !i["isResolved"].as_bool().unwrap_or(false))
                .map(|i| ServiceIssue {
                    id: i["id"].as_str().unwrap_or("").into(),
                    service: i["service"].as_str().unwrap_or("").into(),
                    title: i["title"].as_str().unwrap_or("").into(),
                    classification: i["classification"].as_str().unwrap_or("").into(),
                    status: i["status"].as_str().unwrap_or("").into(),
                    impact: i["impactDescription"].as_str().unwrap_or("").into(),
                    start: i["startDateTime"].as_str().unwrap_or("").into(),
                })
                .collect()
        })
        .unwrap_or_default();
    // Lo que no funciona, antes de lo que funciona con limitaciones.
    out.sort_by_key(|i| (i.classification != "incident", std::cmp::Reverse(i.start.clone())));
    out
}

/// Incidencias abiertas de Microsoft 365 en la organización.
#[tauri::command]
pub async fn graph_service_health(app: tauri::AppHandle) -> Result<Vec<ServiceIssue>, String> {
    let q = form(&[("$filter", "isResolved eq false"), ("$top", "40")]).replace('+', "%20");
    Ok(parse_issues(&call(&app, reqwest::Method::GET, &format!("/admin/serviceAnnouncement/issues?{q}"), None).await?))
}

// ---------- Calendario y Teams ----------

/// Pone una visita de la Agenda en el calendario de Outlook del técnico (o la
/// actualiza si ya estaba). Devuelve el id del evento, que se guarda en la visita.
#[tauri::command]
pub async fn graph_calendar_sync(app: tauri::AppHandle, visit_id: String) -> Result<String, String> {
    let v = crate::agenda::visit_by_id(&app, &visit_id).ok_or("Esa visita ya no existe.")?;
    let iso = |t: u64| chrono::DateTime::from_timestamp(t as i64, 0).map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default();
    let evento = json!({
        "subject": if v.client_name.is_empty() || v.title.is_empty() { format!("{} · {}", crate::agenda::kind_name(&v.kind), v.label()) } else { format!("{} · {}", v.title, v.client_name) },
        "start": { "dateTime": iso(v.start), "timeZone": "UTC" },
        "end": { "dateTime": iso(v.start + u64::from(v.minutes) * 60), "timeZone": "UTC" },
        "location": { "displayName": v.place },
        "body": { "contentType": "text", "content": format!("{}{}\n\nCreada desde AdminOps.", if v.machines > 0 { format!("{} equipos. ", v.machines) } else { String::new() }, v.notes) },
        "reminderMinutesBeforeStart": 30,
    });
    let id = if v.outlook_event.is_empty() {
        let r = call(&app, reqwest::Method::POST, "/me/events", Some(evento)).await?;
        r["id"].as_str().unwrap_or("").to_string()
    } else {
        match call(&app, reqwest::Method::PATCH, &format!("/me/events/{}", v.outlook_event), Some(evento.clone())).await {
            Ok(_) => v.outlook_event.clone(),
            // Lo borraron en Outlook: se crea otra vez.
            Err(_) => call(&app, reqwest::Method::POST, "/me/events", Some(evento)).await?["id"].as_str().unwrap_or("").to_string(),
        }
    };
    if id.is_empty() {
        return Err("Outlook no devolvió el evento creado.".into());
    }
    crate::agenda::set_outlook_event(&app, &visit_id, &id)?;
    Ok(id)
}

/// Escribe a una persona por Teams (chat uno a uno) sin abrir Teams.
#[tauri::command]
pub async fn graph_teams_send(app: tauri::AppHandle, upn: String, text: String) -> Result<(), String> {
    let u = user_path(&upn)?;
    let text: String = text.trim().chars().take(2000).collect();
    if text.is_empty() {
        return Err("Escribe el mensaje.".into());
    }
    let me = call(&app, reqwest::Method::GET, "/me?$select=id", None).await?;
    let my_id = me["id"].as_str().ok_or("No se pudo saber quién eres en Microsoft 365.")?;
    let member = |bind: String| json!({ "@odata.type": "#microsoft.graph.aadUserConversationMember", "roles": ["owner"], "user@odata.bind": bind });
    let chat = call(
        &app,
        reqwest::Method::POST,
        "/chats",
        Some(json!({ "chatType": "oneOnOne", "members": [member(format!("{GRAPH}/users('{my_id}')")), member(format!("{GRAPH}{u}"))] })),
    )
    .await?;
    let chat_id = chat["id"].as_str().ok_or("Teams no devolvió el chat.")?;
    let r = call(&app, reqwest::Method::POST, &format!("/chats/{chat_id}/messages"), Some(json!({ "body": { "contentType": "text", "content": text } })))
        .await
        .map(|_| ());
    let tweaks = app.state::<TweakState>();
    tweaks.record(Op::Run, &format!("Teams: mensaje enviado a {}", upn.trim()), &r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenants_and_app_ids_are_validated() {
        assert!(valid_tenant("empresa.onmicrosoft.com"));
        assert!(valid_tenant("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0"));
        assert!(!valid_tenant("empresa"));
        assert!(!valid_tenant("empresa.com/../x"));
        assert!(is_guid("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0"));
        assert!(!is_guid("0f1e2d3c-4b5a-6978-8796"));
        assert!(!is_guid("zzzzzzzz-4b5a-6978-8796-a5b4c3d2e1f0"));
    }

    #[test]
    fn form_bodies_are_encoded() {
        assert_eq!(form(&[("scope", "https://graph.microsoft.com/.default offline_access")]), "scope=https%3A%2F%2Fgraph.microsoft.com%2F.default+offline_access");
        assert_eq!(form(&[("a", "b&c=d")]), "a=b%26c%3Dd");
    }

    /// La cuenta sale del token de identidad (solo para enseñarla).
    #[test]
    fn account_comes_from_the_id_token() {
        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"preferred_username":"tecnico@empresa.gob.do"}"#);
        assert_eq!(account_from_id_token(&format!("x.{payload}.y")), "tecnico@empresa.gob.do");
        assert_eq!(account_from_id_token("basura"), "");
    }

    /// Los fallos de inicio de sesión se explican; los que no conocemos no se inventan.
    #[test]
    fn signin_failures_are_explained_without_guessing() {
        assert_eq!(signin_reason(0, ""), "Correcto");
        assert!(signin_reason(50126, "").contains("contraseña"));
        assert!(signin_reason(500121, "").contains("MFA"));
        assert!(signin_reason(53003, "").contains("acceso condicional"));
        // Uno que no está en la lista: el texto de Microsoft, con su código.
        assert_eq!(signin_reason(399287, "Something went wrong"), "Something went wrong (código 399287)");
        assert_eq!(signin_reason(399287, ""), "Error 399287 de Microsoft");
    }

    #[test]
    fn signins_are_parsed() {
        let v = json!({ "value": [{
            "createdDateTime": "2026-09-29T22:28:35Z", "appDisplayName": "Microsoft Teams", "clientAppUsed": "Browser",
            "ipAddress": "190.0.0.1", "conditionalAccessStatus": "notApplied",
            "status": { "errorCode": 500121, "failureReason": "Authentication failed during strong authentication request." },
            "location": { "city": "Santo Domingo", "countryOrRegion": "DO" },
            "mfaDetail": { "authMethod": "Text message", "authDetail": "" }
        }]});
        let s = parse_signins(&v);
        assert_eq!(s.len(), 1);
        assert!(!s[0].ok);
        assert_eq!(s[0].place, "Santo Domingo, DO");
        assert_eq!(s[0].mfa, "Text message");
        assert!(s[0].reason.contains("MFA"));
    }

    #[test]
    fn mfa_methods_are_parsed_and_the_password_is_not_removable() {
        let v = json!({ "value": [
            { "@odata.type": "#microsoft.graph.passwordAuthenticationMethod", "id": "28c10230-6103-485e-b985-444c60001490" },
            { "@odata.type": "#microsoft.graph.phoneAuthenticationMethod", "id": "3179e48a-750b-4051-897c-87b9720928f7", "phoneNumber": "+1 8095550109", "phoneType": "mobile" },
            { "@odata.type": "#microsoft.graph.microsoftAuthenticatorAuthenticationMethod", "id": "abc-1", "displayName": "iPhone de María" },
            { "@odata.type": "#microsoft.graph.algoNuevo", "id": "z" }
        ]});
        let m = parse_methods(&v);
        assert_eq!(m.len(), 3, "lo desconocido no se enseña");
        assert!(!m[0].removable);
        assert_eq!(m[1].label, "Teléfono · +1 8095550109");
        assert!(m[2].removable && m[2].label.contains("iPhone de María"));
    }

    #[test]
    fn open_incidents_come_first() {
        let v = json!({ "value": [
            { "id": "a", "service": "Microsoft Teams", "title": "Retrasos", "classification": "advisory", "isResolved": false, "startDateTime": "2026-09-29T10:00:00Z" },
            { "id": "b", "service": "Exchange Online", "title": "No se envía correo", "classification": "incident", "isResolved": false, "startDateTime": "2026-09-29T09:00:00Z" },
            { "id": "c", "service": "SharePoint", "title": "Resuelto", "classification": "incident", "isResolved": true }
        ]});
        let i = parse_issues(&v);
        assert_eq!(i.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    }

    #[test]
    fn errors_are_explained() {
        assert!(explain(403, "Authorization_RequestDenied", "").contains("permiso"));
        assert!(explain(403, "Authentication_RequestFromNonPremiumTenantOrB2CTenant", "").contains("P1"));
        assert!(explain(401, "", "").contains("caducado"));
    }

    #[test]
    fn user_paths_are_safe() {
        assert_eq!(user_path("maria.perez@empresa.gob.do").unwrap(), "/users/maria.perez@empresa.gob.do");
        assert!(user_path("maria perez").is_err());
        assert!(user_path("x@y/../../me").is_err());
    }
}
