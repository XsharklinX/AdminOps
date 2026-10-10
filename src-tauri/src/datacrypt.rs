//! Cifrar los datos de AdminOps (1.2.9): clientes, contactos, notas, casos,
//! seguimientos, agenda y lo que se sabe de cada equipo van cifrados en disco con
//! AES-256-GCM. Un pendrive perdido o un equipo robado no enseña nada sin la clave.
//!
//! - Una **clave de datos** aleatoria cifra los archivos. Ella misma se guarda en
//!   `datacrypt.json` cifrada de dos maneras: con el PIN o la contraseña de
//!   AdminOps y con una **clave de rescate** que se enseña una sola vez.
//! - Al desbloquear AdminOps con el PIN, la clave de datos queda en memoria. Con
//!   la contraseña de Windows (el desbloqueo de emergencia) hace falta la clave de rescate.
//! - Sin la clave, los archivos cifrados se leen como vacíos y **no se escribe nada**
//!   encima: nunca se pierde un dato por estar bloqueado.
//! - Quedan fuera los ajustes, las preferencias y lo que se necesita antes de
//!   desbloquear (la pantalla de bloqueo con tu marca, la ventana, los portales).
//! - Un PIN de 4 a 8 números se puede probar entero con tiempo: para una protección
//!   de verdad hay que usar una contraseña larga.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Cabecera de un archivo cifrado: `AOD1` ‖ nonce(12) ‖ cifrado.
const MAGIC: &[u8; 4] = b"AOD1";
/// Cabecera de la clave de datos protegida: `AOW1` ‖ sal(16) ‖ nonce(12) ‖ cifrado.
const WRAP: &[u8; 4] = b"AOW1";
const ITERATIONS: u32 = 210_000;

/// Los archivos que se cifran (por nombre).
pub const PROTECTED: &[&str] = &[
    "clients.json",
    "contacts.json",
    "contacts-tags.json",
    "cases.json",
    "notas.json",
    "followups.json",
    "agenda.json",
    "soluciones.json",
    "puestos.json",
    "mapa-oficina.json",
    "remote-connections.json",
    "portal_logins.json",
    "recetas.json",
    "plantillas.json",
    "learned.json",
    "vaults.json",
    "app-lists.json",
];

pub const LOCKED_MSG: &str = "Los datos de AdminOps están cifrados: desbloquea AdminOps con tu PIN o contraseña (o con la clave de rescate) para guardarlos.";

pub fn is_protected(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| PROTECTED.iter().any(|p| p.eq_ignore_ascii_case(n)))
}

pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC.len() + 12 + 16 && bytes.starts_with(MAGIC)
}

// ---------- Cifrado (puro) ----------

fn cipher(key: &[u8; 32]) -> Aes256Gcm {
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key))
}

pub fn seal(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher(key).encrypt(&nonce, plain).map_err(|_| "No se pudo cifrar.".to_string())?;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&nonce);
    out.extend(ct);
    Ok(out)
}

/// `None` si no es un archivo cifrado o la clave no es la suya.
pub fn open(key: &[u8; 32], blob: &[u8]) -> Option<Vec<u8>> {
    if !is_sealed(blob) {
        return None;
    }
    let (nonce, ct) = blob[MAGIC.len()..].split_at(12);
    cipher(key).decrypt(Nonce::from_slice(nonce), ct).ok()
}

fn kek(secret: &str, salt: &[u8], iterations: u32) -> Aes256Gcm {
    let mut k = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(secret.as_bytes(), salt, iterations, &mut k);
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&k))
}

fn salt() -> [u8; 16] {
    use aes_gcm::aead::rand_core::RngCore;
    let mut s = [0u8; 16];
    OsRng.fill_bytes(&mut s);
    s
}

/// La clave de datos protegida con un secreto (el PIN o la clave de rescate).
pub fn wrap(data_key: &[u8; 32], secret: &str, iterations: u32) -> Result<Vec<u8>, String> {
    let salt = salt();
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = kek(secret, &salt, iterations).encrypt(&nonce, data_key.as_slice()).map_err(|_| "No se pudo proteger la clave.".to_string())?;
    let mut out = WRAP.to_vec();
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend(ct);
    Ok(out)
}

pub fn unwrap(blob: &[u8], secret: &str, iterations: u32) -> Option<[u8; 32]> {
    if blob.len() < 4 + 16 + 12 + 16 || !blob.starts_with(WRAP) {
        return None;
    }
    let (salt, rest) = blob[4..].split_at(16);
    let (nonce, ct) = rest.split_at(12);
    let key = kek(secret, salt, iterations).decrypt(Nonce::from_slice(nonce), ct).ok()?;
    key.try_into().ok()
}

/// Una clave de rescate legible: 5 grupos de 5 letras y cifras sin confusiones (≈ 125 bits).
pub fn recovery_key() -> String {
    use aes_gcm::aead::rand_core::RngCore;
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 25];
    OsRng.fill_bytes(&mut bytes);
    bytes.chunks(5).map(|g| g.iter().map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char).collect::<String>()).collect::<Vec<_>>().join("-")
}

/// Lo que se escribió al copiar la clave de rescate (mayúsculas, espacios, guiones de más) igual que la original.
pub fn clean_recovery(input: &str) -> String {
    let c: String = input.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    c.as_bytes().chunks(5).map(|g| String::from_utf8_lossy(g).into_owned()).collect::<Vec<_>>().join("-")
}

// ---------- Estado ----------

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
struct Config {
    enabled: bool,
    iterations: u32,
    /// base64 de la clave de datos protegida con el PIN o la contraseña.
    by_secret: String,
    /// base64 de la clave de datos protegida con la clave de rescate.
    by_recovery: String,
}

static KEY: Mutex<Option<[u8; 32]>> = Mutex::new(None);
/// ¿Hay cifrado activado? `None` hasta que se lee la configuración.
static ENABLED: Mutex<Option<bool>> = Mutex::new(None);
/// Mientras se descifra todo para desactivar el cifrado: los archivos se escriben en claro.
static PLAIN: AtomicBool = AtomicBool::new(false);

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

fn config_path() -> Option<PathBuf> {
    crate::paths::shared_data_dir_early().map(|d| d.join("datacrypt.json"))
}

fn load() -> Config {
    config_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(c: &Config) -> Result<(), String> {
    let p = config_path().ok_or("No se encuentra la carpeta de datos.")?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(c).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

pub fn enabled() -> bool {
    let mut g = ENABLED.lock().unwrap_or_else(|e| e.into_inner());
    *g.get_or_insert_with(|| load().enabled)
}

fn set_enabled(on: bool) {
    *ENABLED.lock().unwrap_or_else(|e| e.into_inner()) = Some(on);
}

fn key() -> Option<[u8; 32]> {
    *KEY.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn unlocked() -> bool {
    key().is_some()
}

// ---------- Lo que usa paths::read_json / write_json ----------

/// Lo que hay en un archivo, descifrado si hace falta. `None`: está cifrado y no se puede abrir (sin clave).
pub fn read(bytes: Vec<u8>) -> Option<Vec<u8>> {
    if !is_sealed(&bytes) {
        return Some(bytes);
    }
    open(&key()?, &bytes)
}

/// Lo que se escribe en un archivo: cifrado si toca. Con el archivo cifrado y la
/// clave sin abrir, es un error: nunca se pisa lo cifrado con lo vacío.
pub fn for_write(path: &Path, plain: Vec<u8>) -> Result<Vec<u8>, String> {
    if !is_protected(path) {
        return Ok(plain);
    }
    let on_disk_sealed = std::fs::File::open(path).ok().is_some_and(|mut f| {
        use std::io::Read;
        let mut head = [0u8; 4];
        f.read_exact(&mut head).is_ok() && &head == MAGIC
    });
    let want = !PLAIN.load(Ordering::SeqCst) && (on_disk_sealed || enabled());
    if !want {
        return Ok(plain);
    }
    let k = key().ok_or_else(|| LOCKED_MSG.to_string())?;
    seal(&k, &plain)
}

// ---------- Activar, desactivar, desbloquear ----------

fn protected_files(dir: &Path) -> Vec<PathBuf> {
    PROTECTED.iter().map(|n| dir.join(n)).filter(|p| p.is_file()).collect()
}

/// Cifra (o descifra) cada archivo protegido que ya existe.
fn convert(dir: &Path, to_sealed: bool) -> Result<usize, String> {
    let mut n = 0;
    for p in protected_files(dir) {
        let bytes = std::fs::read(&p).map_err(|e| format!("No se pudo leer {}: {e}", p.display()))?;
        let sealed = is_sealed(&bytes);
        if sealed == to_sealed {
            continue;
        }
        let plain = read(bytes).ok_or_else(|| format!("{} está cifrado con otra clave.", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))?;
        let out = if to_sealed { seal(&key().ok_or(LOCKED_MSG)?, &plain)? } else { plain };
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, out).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub unlocked: bool,
    /// Cuántos archivos de datos hay y cuántos están cifrados ahora.
    pub files: usize,
    pub sealed: usize,
}

fn data_dir() -> Result<PathBuf, String> {
    crate::paths::shared_data_dir_early().ok_or_else(|| "No se encuentra la carpeta de datos.".to_string())
}

pub fn status() -> Status {
    let files = data_dir().map(|d| protected_files(&d)).unwrap_or_default();
    let sealed = files.iter().filter(|p| std::fs::File::open(p).ok().is_some_and(|mut f| {
        use std::io::Read;
        let mut head = [0u8; 4];
        f.read_exact(&mut head).is_ok() && &head == MAGIC
    })).count();
    Status { enabled: enabled(), unlocked: unlocked(), files: files.len(), sealed }
}

/// Al desbloquear AdminOps con el PIN o la contraseña: si hay cifrado, abre la clave de datos.
/// Con ella, además, termina de cifrar lo que hubiera quedado a medias.
pub fn unlock_with(secret: &str) -> bool {
    if !enabled() {
        return true;
    }
    let c = load();
    let Some(k) = b64().decode(&c.by_secret).ok().and_then(|b| unwrap(&b, secret, c.iterations.max(1))) else { return false };
    *KEY.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
    if let Ok(dir) = data_dir() {
        if let Err(e) = convert(&dir, true) {
            log::warn!("Cifrado de datos: {e}");
        }
    }
    true
}

/// Al cambiar el PIN o la contraseña de AdminOps: la clave de datos pasa a ir protegida con la nueva.
pub fn rewrap(current: &str, new: &str) -> Result<(), String> {
    if !enabled() {
        return Ok(());
    }
    let mut c = load();
    let iterations = c.iterations.max(1);
    let k = match key() {
        Some(k) => k,
        None => b64().decode(&c.by_secret).ok().and_then(|b| unwrap(&b, current, iterations)).ok_or("No se pudo abrir la clave de los datos con el PIN actual.")?,
    };
    c.by_secret = b64().encode(wrap(&k, new, iterations)?);
    save(&c)?;
    *KEY.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
    Ok(())
}

/// Activa el cifrado: devuelve la clave de rescate (se enseña una sola vez).
pub fn enable(secret: &str, iterations: u32) -> Result<String, String> {
    if enabled() {
        return Err("Los datos ya están cifrados.".into());
    }
    let dir = data_dir()?;
    use aes_gcm::aead::rand_core::RngCore;
    let mut k = [0u8; 32];
    OsRng.fill_bytes(&mut k);
    let recovery = recovery_key();
    let cfg = Config { enabled: true, iterations, by_secret: b64().encode(wrap(&k, secret, iterations)?), by_recovery: b64().encode(wrap(&k, &clean_recovery(&recovery), iterations)?) };
    // Primero se guarda la clave (protegida): si algo se corta a mitad, nada queda cifrado sin poder abrirse.
    save(&cfg)?;
    *KEY.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
    set_enabled(true);
    convert(&dir, true)?;
    Ok(recovery)
}

/// Desactiva el cifrado: descifra todo (con el secreto) y borra las claves.
pub fn disable(secret: &str) -> Result<usize, String> {
    if !enabled() {
        return Ok(0);
    }
    let c = load();
    let k = b64().decode(&c.by_secret).ok().and_then(|b| unwrap(&b, secret, c.iterations.max(1))).ok_or("El PIN o la contraseña no es correcto.")?;
    *KEY.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
    let dir = data_dir()?;
    // Primero se vuelve todo a claro; solo entonces se quita la configuración. Si se corta, sigue activado y consistente.
    PLAIN.store(true, Ordering::SeqCst);
    let n = convert(&dir, false);
    PLAIN.store(false, Ordering::SeqCst);
    let n = n?;
    save(&Config::default())?;
    set_enabled(false);
    *KEY.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(n)
}

/// Desbloqueo con la clave de rescate (cuando se entró con la contraseña de Windows).
pub fn unlock_recovery(input: &str) -> bool {
    if !enabled() {
        return true;
    }
    let c = load();
    let Some(k) = b64().decode(&c.by_recovery).ok().and_then(|b| unwrap(&b, &clean_recovery(input), c.iterations.max(1))) else { return false };
    *KEY.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
    true
}

// ---------- Órdenes ----------

#[tauri::command]
pub fn data_crypt_status(app: tauri::AppHandle) -> serde_json::Value {
    let s = status();
    serde_json::json!({ "available": crate::applock::lock_status(app).enabled, "enabled": s.enabled, "unlocked": s.unlocked, "files": s.files, "sealed": s.sealed })
}

#[tauri::command(async)]
pub fn data_crypt_enable(app: tauri::AppHandle, secret: String) -> Result<String, String> {
    if !crate::applock::verify(&app, &secret)? {
        return Err("El PIN o la contraseña no es correcto.".into());
    }
    let recovery = enable(&secret, ITERATIONS)?;
    log::info!("Datos de AdminOps cifrados");
    Ok(recovery)
}

#[tauri::command(async)]
pub fn data_crypt_disable(app: tauri::AppHandle, secret: String) -> Result<usize, String> {
    if !crate::applock::verify(&app, &secret)? {
        return Err("El PIN o la contraseña no es correcto.".into());
    }
    let n = disable(&secret)?;
    log::info!("Cifrado de los datos de AdminOps quitado");
    Ok(n)
}

/// Los intentos de la clave de rescate también tienen espera.
static FAILS: Mutex<(u32, Option<std::time::Instant>)> = Mutex::new((0, None));

#[tauri::command(async)]
pub fn data_crypt_unlock_recovery(key: String) -> Result<bool, String> {
    let mut f = FAILS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(until) = f.1.filter(|u| *u > std::time::Instant::now()) {
        return Err(format!("Demasiados intentos. Espera {} s.", until.saturating_duration_since(std::time::Instant::now()).as_secs() + 1));
    }
    let ok = unlock_recovery(&key);
    if ok {
        *f = (0, None);
    } else {
        f.0 += 1;
        if f.0 >= 5 {
            *f = (0, Some(std::time::Instant::now() + std::time::Duration::from_secs(60)));
        }
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las pruebas que tocan la clave en memoria (una sola para todo el programa) no corren a la vez.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// Pocas vueltas para que las pruebas vayan deprisa; el valor real es ITERATIONS.
    const FAST: u32 = 1000;

    #[test]
    fn seal_and_open_round_trip() {
        let k = [7u8; 32];
        let blob = seal(&k, br#"{"clientes":["Farmacia Central"]}"#).unwrap();
        assert!(is_sealed(&blob) && !String::from_utf8_lossy(&blob).contains("Farmacia"));
        assert_eq!(open(&k, &blob).unwrap(), br#"{"clientes":["Farmacia Central"]}"#);
        // Dos cifrados del mismo texto son distintos (nonce aleatorio).
        assert_ne!(seal(&k, b"x").unwrap(), seal(&k, b"x").unwrap());
    }

    #[test]
    fn wrong_key_or_tampering_is_refused() {
        let blob = seal(&[1u8; 32], b"secreto").unwrap();
        assert!(open(&[2u8; 32], &blob).is_none());
        let mut bad = blob.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(open(&[1u8; 32], &bad).is_none(), "un cambio de un bit se nota");
        assert!(open(&[1u8; 32], &blob[..blob.len() - 1]).is_none());
        assert!(open(&[1u8; 32], b"{\"plano\":true}").is_none(), "lo que no es cifrado no se abre");
        assert!(!is_sealed(b"AOD1"));
    }

    #[test]
    fn the_data_key_is_protected_by_the_secret_and_by_the_recovery_key() {
        let k = [9u8; 32];
        let w = wrap(&k, "4821", FAST).unwrap();
        assert_eq!(unwrap(&w, "4821", FAST), Some(k));
        assert_eq!(unwrap(&w, "4822", FAST), None);
        assert_eq!(unwrap(&w, "4821", FAST + 1), None, "las vueltas también cuentan");
        assert!(!String::from_utf8_lossy(&w).contains("4821"));
        let r = recovery_key();
        let wr = wrap(&k, &clean_recovery(&r), FAST).unwrap();
        // La clave de rescate vale escrita en minúsculas, sin guiones o con espacios.
        assert_eq!(unwrap(&wr, &clean_recovery(&r.to_lowercase().replace('-', "")), FAST), Some(k));
        assert_eq!(unwrap(&wr, &clean_recovery(&format!(" {} ", r.replace('-', " "))), FAST), Some(k));
        assert_eq!(unwrap(&wr, "otra", FAST), None);
    }

    #[test]
    fn recovery_keys_are_readable_and_different() {
        let a = recovery_key();
        assert_eq!(a.len(), 29);
        assert_eq!(a.matches('-').count(), 4);
        assert!(a.chars().all(|c| c == '-' || (c.is_ascii_uppercase() || c.is_ascii_digit()) && !"01OI".contains(c)), "{a}");
        assert_ne!(a, recovery_key());
        assert_eq!(clean_recovery("abcde fghij-klmno_pqrst uvwxy"), "ABCDE-FGHIJ-KLMNO-PQRST-UVWXY");
    }

    #[test]
    fn only_business_data_is_protected() {
        for n in ["clients.json", "Contacts.JSON", "cases.json", "agenda.json", "followups.json", "notas.json"] {
            assert!(is_protected(&Path::new("datos").join(n)), "{n}");
        }
        // Lo que hace falta antes de desbloquear, y lo que no es del técnico, queda en claro.
        for n in ["settings.json", "applock.json", "preferencias.json", "window.json", "portals.json", "datacrypt.json", "journal.json", "lugar.json", "session.json", "errors.json", "outbound.json"] {
            assert!(!is_protected(Path::new(n)), "{n}");
        }
    }

    /// Cifrar todo, volver a abrir con la clave y descifrar todo, sin perder nada y sin pisar lo cifrado estando bloqueado.
    #[test]
    fn enabling_and_disabling_never_loses_data() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("adminops-datacrypt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("clients.json"), br#"[{"name":"Farmacia Central"}]"#).unwrap();
        std::fs::write(dir.join("settings.json"), br#"{"company":"Soporte"}"#).unwrap();
        let k = [5u8; 32];
        *KEY.lock().unwrap() = Some(k);
        assert_eq!(convert(&dir, true).unwrap(), 1);
        let on_disk = std::fs::read(dir.join("clients.json")).unwrap();
        assert!(is_sealed(&on_disk) && !String::from_utf8_lossy(&on_disk).contains("Farmacia"));
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), br#"{"company":"Soporte"}"#, "los ajustes siguen en claro");
        // Repetir no hace nada.
        assert_eq!(convert(&dir, true).unwrap(), 0);
        // Con la clave se lee; sin ella, no se puede abrir (y no se pisa).
        assert_eq!(read(on_disk.clone()).unwrap(), br#"[{"name":"Farmacia Central"}]"#);
        *KEY.lock().unwrap() = None;
        assert!(read(on_disk.clone()).is_none());
        assert!(convert(&dir, false).is_err(), "sin clave no se puede descifrar");
        assert_eq!(std::fs::read(dir.join("clients.json")).unwrap(), on_disk, "y el archivo sigue intacto");
        *KEY.lock().unwrap() = Some(k);
        assert_eq!(convert(&dir, false).unwrap(), 1);
        assert_eq!(std::fs::read(dir.join("clients.json")).unwrap(), br#"[{"name":"Farmacia Central"}]"#);
        *KEY.lock().unwrap() = None;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sealed_file_is_never_overwritten_while_locked() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("adminops-datacrypt-w-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("contacts.json");
        std::fs::write(&file, seal(&[3u8; 32], b"[1]").unwrap()).unwrap();
        *KEY.lock().unwrap() = None;
        assert_eq!(for_write(&file, b"[]".to_vec()).unwrap_err(), LOCKED_MSG, "bloqueado: error, no un archivo vacío");
        *KEY.lock().unwrap() = Some([3u8; 32]);
        let out = for_write(&file, b"[2]".to_vec()).unwrap();
        assert_eq!(open(&[3u8; 32], &out).unwrap(), b"[2]");
        // Un archivo que no es de los protegidos pasa tal cual.
        assert_eq!(for_write(&dir.join("settings.json"), b"{}".to_vec()).unwrap(), b"{}");
        *KEY.lock().unwrap() = None;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
