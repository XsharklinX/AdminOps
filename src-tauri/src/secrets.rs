//! Contraseñas que guarda AdminOps (acceso a routers).
//!
//! - **Instalado**: DPAPI de Windows, atadas a este usuario en este equipo.
//! - **Portable**: AES-256-GCM con una clave aleatoria guardada en el USB
//!   (`AdminOps-data\.clave`), para que funcionen en cualquier equipo al que se
//!   lleve el USB. Nunca se guardan en claro, pero quien tenga el USB entero
//!   podría leerlas: por eso la app ofrece bloqueo con PIN o contraseña.
//!
//! Formato: `p1:` + base64(nonce ‖ cifrado) en portable; base64(DPAPI) sin prefijo.
//!
//! **Con el bloqueo de AdminOps activado (PIN o contraseña)**, la clave del USB
//! se guarda a su vez cifrada con ese PIN (`AOK1` + sal + nonce + clave cifrada,
//! PBKDF2-HMAC-SHA256 de 210 000 vueltas): quien se lleve el pendrive no puede
//! leer las contraseñas sin el PIN. Al desbloquear AdminOps se descifra y queda
//! solo en memoria. Si se olvida el PIN, esas contraseñas no se recuperan (el
//! resto de los datos sí).

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use std::path::Path;

const PREFIX: &str = "p1:";
const KEY_FILE: &str = ".clave";
/// Cabecera de la clave del USB cuando va protegida con el PIN.
const WRAPPED: &[u8; 4] = b"AOK1";
const WRAP_ITERATIONS: u32 = 210_000;

/// La clave del USB ya descifrada con el PIN (solo en memoria).
static UNLOCKED: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);

const LOCKED_MSG: &str = "Las contraseñas guardadas están protegidas con el PIN de AdminOps: desbloquéalo con tu PIN para usarlas.";

fn kek(secret: &str, salt: &[u8]) -> Aes256Gcm {
    let mut k = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(secret.as_bytes(), salt, WRAP_ITERATIONS, &mut k);
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&k))
}

/// La clave cifrada con el PIN: AOK1 ‖ sal(16) ‖ nonce(12) ‖ cifrado.
fn wrap(key: &[u8], secret: &str) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::rand_core::RngCore;
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = kek(secret, &salt).encrypt(&nonce, key).map_err(|_| "No se pudo proteger la clave.".to_string())?;
    let mut out = WRAPPED.to_vec();
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend(ct);
    Ok(out)
}

fn unwrap_key(blob: &[u8], secret: &str) -> Option<Vec<u8>> {
    if blob.len() < 4 + 16 + 12 + 16 || !blob.starts_with(WRAPPED) {
        return None;
    }
    let (salt, rest) = blob[4..].split_at(16);
    let (nonce, ct) = rest.split_at(12);
    kek(secret, salt).decrypt(Nonce::from_slice(nonce), ct).ok().filter(|k| k.len() == 32)
}

/// Escribe la clave del USB (oculta). Un archivo oculto no se puede
/// sobrescribir directamente en Windows: se le quita el atributo antes.
fn write_key_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    unhide(path);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("No se pudo guardar la clave en el USB: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("No se pudo guardar la clave en el USB: {e}"))?;
    hide(path);
    Ok(())
}

/// Los 32 bytes de la clave: del archivo, o de memoria si va protegida con el PIN.
fn key_bytes(root: &Path) -> Result<Vec<u8>, String> {
    let path = root.join(KEY_FILE);
    match std::fs::read(&path) {
        Ok(b) if b.len() == 32 => Ok(b),
        Ok(b) if b.starts_with(WRAPPED) => UNLOCKED.lock().unwrap_or_else(|e| e.into_inner()).clone().ok_or_else(|| LOCKED_MSG.to_string()),
        Ok(_) => Err("La clave de las contraseñas guardadas del USB está dañada.".into()),
        Err(_) => {
            let key = Aes256Gcm::generate_key(OsRng);
            std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
            write_key_file(&path, key.as_slice())?;
            Ok(key.to_vec())
        }
    }
}

/// ¿La clave del USB va protegida con el PIN?
pub fn key_is_protected(root: &Path) -> bool {
    std::fs::read(root.join(KEY_FILE)).is_ok_and(|b| b.starts_with(WRAPPED))
}

/// Al desbloquear AdminOps con el PIN: descifra la clave (si va protegida) y,
/// si aún no lo estaba, la protege con ese PIN. Devuelve si quedó disponible.
pub fn key_unlock(root: &Path, secret: &str) -> bool {
    let path = root.join(KEY_FILE);
    match std::fs::read(&path) {
        Ok(b) if b.starts_with(WRAPPED) => match unwrap_key(&b, secret) {
            Some(k) => {
                *UNLOCKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
                true
            }
            None => false,
        },
        Ok(b) if b.len() == 32 => key_protect(root, secret).is_ok(),
        _ => false,
    }
}

/// Protege (o vuelve a proteger, con un PIN nuevo) la clave del USB.
pub fn key_protect(root: &Path, secret: &str) -> Result<(), String> {
    let key = key_bytes(root)?;
    write_key_file(&root.join(KEY_FILE), &wrap(&key, secret)?)?;
    *UNLOCKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(key);
    Ok(())
}

/// Al quitar el bloqueo: la clave vuelve a guardarse sin PIN.
pub fn key_unprotect(root: &Path) -> Result<(), String> {
    let key = key_bytes(root)?;
    write_key_file(&root.join(KEY_FILE), &key)
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Clave del USB (se crea la primera vez).
fn portable_key(root: &Path) -> Result<Aes256Gcm, String> {
    let bytes = key_bytes(root)?;
    Ok(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&bytes)))
}

#[cfg(windows)]
fn hide(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN};
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe { SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN) };
}

#[cfg(not(windows))]
fn hide(_: &Path) {}

#[cfg(windows)]
fn unhide(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_NORMAL};
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: cadena terminada en cero; solo cambia atributos (falla sin más si no existe).
    unsafe { SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_NORMAL) };
}

#[cfg(not(windows))]
fn unhide(_: &Path) {}

fn seal_with(root: &Path, secret: &str) -> Result<String, String> {
    let cipher = portable_key(root)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher.encrypt(&nonce, secret.as_bytes()).map_err(|_| "No se pudo cifrar la contraseña.".to_string())?;
    let mut out = nonce.to_vec();
    out.extend(ct);
    Ok(format!("{PREFIX}{}", b64().encode(out)))
}

fn open_with(root: &Path, stored: &str) -> Result<String, String> {
    let raw = b64().decode(stored.trim_start_matches(PREFIX)).map_err(|_| "Contraseña guardada dañada.".to_string())?;
    if raw.len() < 13 {
        return Err("Contraseña guardada dañada.".into());
    }
    let (nonce, ct) = raw.split_at(12);
    let plain = portable_key(root)?
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| "La contraseña se guardó con otro USB de AdminOps.".to_string())?;
    String::from_utf8(plain).map_err(|_| "Contraseña guardada dañada.".into())
}

/// Comienzo en base64 de todo blob de DPAPI (versión 1 + el GUID del proveedor).
const DPAPI_B64: &str = "AQAAANCMnd8BFdERjHoAwE/Cl+s";

/// ¿Es una contraseña cifrada con DPAPI (solo legible en el equipo donde se guardó)?
pub fn is_dpapi(stored: &str) -> bool {
    stored.starts_with(DPAPI_B64)
}

/// Cifra con la clave de un pendrive concreto (al pasar los datos de un equipo al pendrive).
pub fn seal_in(root: &Path, secret: &str) -> Result<String, String> {
    seal_with(root, secret)
}

#[cfg(test)]
pub fn open_in(root: &Path, stored: &str) -> Result<String, String> {
    open_with(root, stored)
}

/// Cifra una contraseña para guardarla.
pub fn seal(secret: &str) -> Result<String, String> {
    if secret.is_empty() {
        return Ok(String::new());
    }
    match crate::paths::portable_data_root() {
        Some(root) => seal_with(root, secret),
        None => Ok(b64().encode(crate::network::lan::dpapi(secret.as_bytes(), true)?)),
    }
}

/// Descifra una contraseña guardada (en cualquiera de los dos formatos).
pub fn open(stored: &str) -> Result<String, String> {
    if stored.is_empty() {
        return Ok(String::new());
    }
    if stored.starts_with(PREFIX) {
        let root = crate::paths::portable_data_root().ok_or("Esta contraseña se guardó con AdminOps portable: ábrela desde el USB.")?;
        return open_with(root, stored);
    }
    let raw = b64().decode(stored).map_err(|_| "Contraseña guardada dañada.".to_string())?;
    String::from_utf8(crate::network::lan::dpapi(&raw, false)?).map_err(|_| "Contraseña guardada dañada.".into())
}

/// En portable, una contraseña antigua (DPAPI) que se puede leer en este equipo
/// se pasa al formato del USB para que funcione en todos. None si no hace falta.
pub fn upgrade(stored: &str) -> Option<String> {
    let root = crate::paths::portable_data_root()?;
    if stored.is_empty() || stored.starts_with(PREFIX) {
        return None;
    }
    let plain = open(stored).ok()?;
    seal_with(root, &plain).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Con el PIN: sin desbloquear no se abre nada; con el PIN bueno, sí; el
    /// malo no sirve; y quitar la protección deja la misma clave.
    #[test]
    fn pin_protects_the_usb_key() {
        let dir = std::env::temp_dir().join(format!("adminops-pin-{}", std::process::id()));
        let s = seal_with(&dir, "clave del router").unwrap();
        let raw = std::fs::read(dir.join(KEY_FILE)).unwrap();
        key_protect(&dir, "4821").unwrap();
        assert!(key_is_protected(&dir));
        let blob = std::fs::read(dir.join(KEY_FILE)).unwrap();
        assert!(!blob.windows(32).any(|w| w == raw.as_slice()), "la clave no queda en claro");
        *UNLOCKED.lock().unwrap() = None;
        assert_eq!(open_with(&dir, &s).unwrap_err(), LOCKED_MSG);
        assert!(!key_unlock(&dir, "0000"));
        assert!(key_unlock(&dir, "4821"));
        assert_eq!(open_with(&dir, &s).unwrap(), "clave del router");
        // Cambiar de PIN: sigue abriendo lo de antes.
        key_protect(&dir, "9999").unwrap();
        *UNLOCKED.lock().unwrap() = None;
        assert!(key_unlock(&dir, "9999"));
        key_unprotect(&dir).unwrap();
        assert_eq!(std::fs::read(dir.join(KEY_FILE)).unwrap(), raw);
        *UNLOCKED.lock().unwrap() = None;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn portable_roundtrip_and_wrong_usb() {
        let dir = std::env::temp_dir().join(format!("adminops-secrets-{}", std::process::id()));
        let other = dir.join("otro");
        let s = seal_with(&dir, "clave del router ñ").unwrap();
        assert!(s.starts_with(PREFIX));
        assert!(!s.contains("router"));
        assert_eq!(open_with(&dir, &s).unwrap(), "clave del router ñ");
        // Cada vez con un nonce distinto.
        assert_ne!(seal_with(&dir, "x").unwrap(), seal_with(&dir, "x").unwrap());
        // Otro USB (otra clave) no puede leerla.
        assert!(open_with(&other, &s).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
