//! Contraseñas que guarda AdminOps (acceso a routers).
//!
//! - **Instalado**: DPAPI de Windows, atadas a este usuario en este equipo.
//! - **Portable**: AES-256-GCM con una clave aleatoria guardada en el USB
//!   (`AdminOps-data\.clave`), para que funcionen en cualquier equipo al que se
//!   lleve el USB. Nunca se guardan en claro, pero quien tenga el USB entero
//!   podría leerlas: por eso la app ofrece bloqueo con PIN o contraseña.
//!
//! Formato: `p1:` + base64(nonce ‖ cifrado) en portable; base64(DPAPI) sin prefijo.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use std::path::Path;

const PREFIX: &str = "p1:";
const KEY_FILE: &str = ".clave";

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Clave del USB (se crea la primera vez).
fn portable_key(root: &Path) -> Result<Aes256Gcm, String> {
    let path = root.join(KEY_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => return Err("La clave de las contraseñas guardadas del USB está dañada.".into()),
        Err(_) => {
            let key = Aes256Gcm::generate_key(OsRng);
            std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
            std::fs::write(&path, key.as_slice()).map_err(|e| format!("No se pudo crear la clave en el USB: {e}"))?;
            hide(&path);
            key.to_vec()
        }
    };
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
