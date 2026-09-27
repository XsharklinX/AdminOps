//! Bloqueo de AdminOps con PIN o contraseña (al abrir y tras un tiempo sin uso).
//!
//! Se guarda solo un hash PBKDF2-HMAC-SHA256 con sal aleatoria. Es un bloqueo
//! de la interfaz: impide que otra persona use la app en este equipo, no cifra
//! los datos (para eso está la Caja fuerte). Si se olvida, se puede
//! desbloquear con la contraseña de Windows de la cuenta que ejecuta AdminOps.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ITERATIONS: u32 = 210_000;
const MAX_FAILS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(30);

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct Stored {
    /// pin | password
    kind: String,
    salt: String,
    hash: String,
    iterations: u32,
    /// Minutos sin usar la app antes de bloquearla (0: solo al abrir).
    idle_minutes: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockStatus {
    pub enabled: bool,
    pub kind: String,
    pub idle_minutes: u32,
    /// Segundos de espera tras varios intentos fallidos.
    pub wait_secs: u64,
}

/// (fallos seguidos, bloqueado hasta)
static FAILS: Mutex<(u32, Option<Instant>)> = Mutex::new((0, None));

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("applock.json")
}

fn load(app: &tauri::AppHandle) -> Option<Stored> {
    let s: Stored = crate::paths::read_json(&path(app));
    (!s.hash.is_empty()).then_some(s)
}

fn random(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
        BCryptGenRandom(std::ptr::null_mut(), buf.as_mut_ptr(), n as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG);
    }
    buf
}

fn derive(secret: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(secret.as_bytes(), salt, iterations, &mut out);
    out
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Comparación en tiempo constante.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn matches(s: &Stored, secret: &str) -> bool {
    let (Ok(salt), Ok(hash)) = (b64().decode(&s.salt), b64().decode(&s.hash)) else { return false };
    same(&derive(secret, &salt, s.iterations.max(1)), &hash)
}

fn validate(kind: &str, secret: &str) -> Result<(), String> {
    match kind {
        "pin" if (4..=8).contains(&secret.len()) && secret.chars().all(|c| c.is_ascii_digit()) => Ok(()),
        "pin" => Err("El PIN debe tener de 4 a 8 números.".into()),
        "password" if secret.chars().count() >= 6 && secret.chars().count() <= 128 => Ok(()),
        "password" => Err("La contraseña debe tener al menos 6 caracteres.".into()),
        _ => Err("Tipo de bloqueo no válido.".into()),
    }
}

fn wait_secs() -> u64 {
    let f = FAILS.lock().unwrap_or_else(|e| e.into_inner());
    f.1.map_or(0, |until| until.saturating_duration_since(Instant::now()).as_secs())
}

/// Comprueba un intento, con espera tras `MAX_FAILS` fallos seguidos.
fn attempt(ok: impl FnOnce() -> bool) -> Result<bool, String> {
    let wait = wait_secs();
    if wait > 0 {
        return Err(format!("Demasiados intentos. Espera {wait} s."));
    }
    let good = ok();
    let mut f = FAILS.lock().unwrap_or_else(|e| e.into_inner());
    if good {
        *f = (0, None);
    } else {
        f.0 += 1;
        if f.0 >= MAX_FAILS {
            *f = (0, Some(Instant::now() + LOCKOUT));
        }
    }
    Ok(good)
}

#[tauri::command]
pub fn lock_status(app: tauri::AppHandle) -> LockStatus {
    match load(&app) {
        Some(s) => LockStatus { enabled: true, kind: s.kind, idle_minutes: s.idle_minutes, wait_secs: wait_secs() },
        None => LockStatus { enabled: false, kind: String::new(), idle_minutes: 0, wait_secs: 0 },
    }
}

#[tauri::command(async)]
pub fn lock_verify(app: tauri::AppHandle, secret: String) -> Result<bool, String> {
    let Some(s) = load(&app) else { return Ok(true) };
    attempt(|| matches(&s, &secret))
}

/// Pone o cambia el bloqueo. Si ya había uno, pide el PIN o la contraseña actual.
#[tauri::command(async)]
pub fn lock_set(app: tauri::AppHandle, kind: String, secret: String, idle_minutes: u32, current: String) -> Result<(), String> {
    if let Some(s) = load(&app) {
        if !attempt(|| matches(&s, &current))? {
            return Err("El PIN o la contraseña actual no es correcto.".into());
        }
    }
    validate(&kind, &secret)?;
    let salt = random(16);
    let stored = Stored {
        hash: b64().encode(derive(&secret, &salt, ITERATIONS)),
        salt: b64().encode(&salt),
        iterations: ITERATIONS,
        kind,
        idle_minutes: idle_minutes.min(240),
    };
    crate::paths::write_json(&path(&app), &stored)?;
    log::info!("Bloqueo de la app configurado");
    Ok(())
}

/// Cambia solo el tiempo sin uso (no pide el PIN: no debilita el bloqueo… salvo con 0, que lo pide).
#[tauri::command]
pub fn lock_set_idle(app: tauri::AppHandle, idle_minutes: u32, current: String) -> Result<(), String> {
    let mut s = load(&app).ok_or("No hay bloqueo configurado.")?;
    if idle_minutes == 0 && !attempt(|| matches(&s, &current))? {
        return Err("Para dejar de bloquear por inactividad, escribe el PIN o la contraseña actual.".into());
    }
    s.idle_minutes = idle_minutes.min(240);
    crate::paths::write_json(&path(&app), &s)
}

#[tauri::command(async)]
pub fn lock_disable(app: tauri::AppHandle, current: String) -> Result<(), String> {
    let Some(s) = load(&app) else { return Ok(()) };
    if !attempt(|| matches(&s, &current))? {
        return Err("El PIN o la contraseña no es correcto.".into());
    }
    std::fs::remove_file(path(&app)).map_err(|e| e.to_string())?;
    log::info!("Bloqueo de la app quitado");
    Ok(())
}

/// Intentos con la contraseña de Windows: más estrictos, porque cuentan para
/// el bloqueo de la cuenta de Windows (10 fallos la bloquean en Windows 11).
static WIN_FAILS: Mutex<(u32, Option<Instant>)> = Mutex::new((0, None));

/// Desbloqueo de emergencia con la contraseña de Windows de esta cuenta.
#[tauri::command(async)]
pub fn lock_verify_windows(password: String) -> Result<bool, String> {
    let mut f = WIN_FAILS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(until) = f.1.filter(|u| *u > Instant::now()) {
        return Err(format!("Demasiados intentos con la contraseña de Windows. Espera {} min.", until.saturating_duration_since(Instant::now()).as_secs() / 60 + 1));
    }
    if password.is_empty() {
        return Ok(false);
    }
    let ok = windows_password_ok(&password);
    if ok {
        *f = (0, None);
    } else {
        f.0 += 1;
        if f.0 >= 3 {
            *f = (0, Some(Instant::now() + Duration::from_secs(15 * 60)));
        }
    }
    Ok(ok)
}

#[cfg(windows)]
fn windows_password_ok(password: &str) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{LogonUserW, LOGON32_LOGON_INTERACTIVE, LOGON32_PROVIDER_DEFAULT};
    let user = std::env::var("USERNAME").unwrap_or_default();
    let domain = std::env::var("USERDOMAIN").unwrap_or_else(|_| ".".into());
    let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (u, d, p) = (w(&user), w(&domain), w(password));
    let mut token = std::ptr::null_mut();
    let ok = unsafe { LogonUserW(u.as_ptr(), d.as_ptr(), p.as_ptr(), LOGON32_LOGON_INTERACTIVE, LOGON32_PROVIDER_DEFAULT, &mut token) } != 0;
    if ok {
        unsafe { CloseHandle(token) };
    }
    ok
}

#[cfg(not(windows))]
fn windows_password_ok(_: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_validation() {
        let salt = random(16);
        assert!(salt.iter().any(|b| *b != 0), "la sal debe ser aleatoria");
        let s = Stored { kind: "pin".into(), salt: b64().encode(&salt), hash: b64().encode(derive("4821", &salt, 1000)), iterations: 1000, idle_minutes: 5 };
        assert!(matches(&s, "4821"));
        assert!(!matches(&s, "4822"));
        assert!(!matches(&s, ""));
        assert!(validate("pin", "1234").is_ok());
        assert!(validate("pin", "12a4").is_err());
        assert!(validate("pin", "123").is_err());
        assert!(validate("password", "corta").is_err());
        assert!(validate("password", "bastante larga").is_ok());
        // Sin probar la contraseña de Windows: un intento fallido cuenta para bloquear la cuenta.
    }
}
