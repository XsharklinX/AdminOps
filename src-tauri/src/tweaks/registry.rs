//! Lectura/escritura del registro con copia exacta (tipo + bytes) para poder
//! restaurar el valor original tal cual estaba.

use super::model::{RegData, RegKind, RegistryAction};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use winreg::enums::*;
use winreg::types::ToRegValue;
use winreg::{RegKey, RegValue};

/// Copia binaria de un valor del registro.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RawValue {
    pub vtype: u32,
    pub bytes: Vec<u8>,
}

/// Resuelve la colmena. HKCU se redirige a `HKEY_USERS\<SID>` cuando el usuario
/// con la sesión abierta no es el que ejecuta AdminOps (ver `target_user`).
fn split(path: &str) -> Result<(RegKey, String), String> {
    let (hive, sub) = path.split_once('\\').ok_or_else(|| format!("Ruta inválida: {path}"))?;
    let root = match hive.to_ascii_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        "HKCU" | "HKEY_CURRENT_USER" => match crate::target_user::hkcu_redirect() {
            Some(sid) => return Ok((RegKey::predef(HKEY_USERS), format!("{sid}\\{sub}"))),
            None => HKEY_CURRENT_USER,
        },
        "HKCR" | "HKEY_CLASSES_ROOT" => HKEY_CLASSES_ROOT,
        "HKU" | "HKEY_USERS" => HKEY_USERS,
        _ => return Err(format!("Colmena no soportada: {hive}")),
    };
    Ok((RegKey::predef(root), sub.to_string()))
}

fn open_read(path: &str) -> Option<RegKey> {
    let (root, sub) = split(path).ok()?;
    root.open_subkey_with_flags(&sub, KEY_READ | KEY_WOW64_64KEY).ok()
}

fn open_write(path: &str) -> Result<RegKey, String> {
    let (root, sub) = split(path)?;
    root.create_subkey_with_flags(&sub, KEY_READ | KEY_WRITE | KEY_WOW64_64KEY)
        .map(|(k, _)| k)
        .map_err(|e| access_err(path, e))
}

fn access_err(path: &str, e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        format!("Acceso denegado a {path} (¿falta ejecutar como administrador?)")
    } else {
        format!("{path}: {e}")
    }
}

fn to_regtype(v: u32) -> RegType {
    match v {
        1 => REG_SZ,
        2 => REG_EXPAND_SZ,
        3 => REG_BINARY,
        4 => REG_DWORD,
        7 => REG_MULTI_SZ,
        11 => REG_QWORD,
        _ => REG_NONE,
    }
}

pub fn read_raw(path: &str, name: &str) -> Option<RawValue> {
    let v = open_read(path)?.get_raw_value(name).ok()?;
    Some(RawValue { vtype: v.vtype as u32, bytes: v.bytes.into_owned() })
}

/// Todos los valores de una clave como (nombre, copia binaria). Vacío si no existe.
pub fn values(path: &str) -> Vec<(String, RawValue)> {
    let Some(key) = open_read(path) else { return vec![] };
    key.enum_values()
        .filter_map(Result::ok)
        .map(|(name, v)| (name, RawValue { vtype: v.vtype as u32, bytes: v.bytes.into_owned() }))
        .collect()
}

/// Decodifica un REG_SZ / REG_EXPAND_SZ (UTF-16LE terminado en nulo).
pub fn raw_to_string(raw: &RawValue) -> Option<String> {
    if raw.vtype != 1 && raw.vtype != 2 {
        return None;
    }
    let words: Vec<u16> = raw.bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    let end = words.iter().position(|&w| w == 0).unwrap_or(words.len());
    Some(String::from_utf16_lossy(&words[..end]))
}

pub fn read_u32(path: &str, name: &str) -> Option<u32> {
    open_read(path)?.get_value::<u32, _>(name).ok()
}

pub fn read_string(path: &str, name: &str) -> Option<String> {
    open_read(path)?.get_value::<String, _>(name).ok()
}

/// ¿El valor actual coincide con `data`?
pub fn matches(path: &str, name: &str, kind: RegKind, data: &RegData) -> bool {
    let Some(key) = open_read(path) else { return false };
    match (kind, data) {
        (RegKind::Dword, RegData::Int(n)) => key.get_value::<u32, _>(name).is_ok_and(|v| v == *n as u32),
        (RegKind::Qword, RegData::Int(n)) => key.get_value::<u64, _>(name).is_ok_and(|v| v == *n as u64),
        (RegKind::String | RegKind::Expand, RegData::Str(s)) => key.get_value::<String, _>(name).is_ok_and(|v| v == *s),
        _ => false,
    }
}

pub fn is_applied(a: &RegistryAction) -> bool {
    matches(&a.path, &a.name, a.kind, &a.value)
}

pub fn write(path: &str, name: &str, kind: RegKind, data: &RegData) -> Result<(), String> {
    let key = open_write(path)?;
    let r = match (kind, data) {
        (RegKind::Dword, RegData::Int(n)) => key.set_value(name, &(*n as u32)),
        (RegKind::Qword, RegData::Int(n)) => key.set_value(name, &(*n as u64)),
        (RegKind::String, RegData::Str(s)) => key.set_value(name, s),
        (RegKind::Expand, RegData::Str(s)) => {
            let mut v = s.to_reg_value();
            v.vtype = REG_EXPAND_SZ;
            key.set_raw_value(name, &v)
        }
        _ => return Err(format!("{path}\\{name}: el tipo no coincide con el valor")),
    };
    r.map_err(|e| access_err(path, e))
}

pub fn write_raw(path: &str, name: &str, raw: &RawValue) -> Result<(), String> {
    let key = open_write(path)?;
    let v = RegValue { bytes: Cow::Borrowed(&raw.bytes), vtype: to_regtype(raw.vtype) };
    key.set_raw_value(name, &v).map_err(|e| access_err(path, e))
}

pub fn delete(path: &str, name: &str) -> Result<(), String> {
    let (root, sub) = split(path)?;
    let Ok(key) = root.open_subkey_with_flags(&sub, KEY_SET_VALUE | KEY_WOW64_64KEY) else {
        return Ok(()); // la clave no existe: nada que borrar
    };
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(access_err(path, e)),
    }
}
