//! Conocimiento del técnico que viaja con AdminOps (en portable, en el USB):
//! soluciones («problema → lo que funcionó»), plantillas de texto, notas por
//! equipo y por red, plantillas de preparación de equipos y listas de puestos.
//!
//! Colecciones genéricas de documentos JSON con `id`: la forma de cada una la
//! define la interfaz; aquí se valida el tamaño y se guarda de forma atómica.

use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::Mutex;

static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Colecciones permitidas: (nombre, archivo, máximo de elementos).
const KINDS: &[(&str, &str, usize)] = &[
    ("solutions", "soluciones.json", 5000),
    ("templates", "plantillas.json", 1000),
    ("notes", "notas.json", 5000),
    ("recipes", "recetas.json", 200),
    ("stations", "puestos.json", 200),
];

/// Tamaño máximo de un elemento (JSON).
const MAX_ITEM: usize = 256 * 1024;

fn file(app: &tauri::AppHandle, kind: &str) -> Result<(std::path::PathBuf, usize), String> {
    let (_, name, max) = KINDS.iter().find(|(k, _, _)| *k == kind).ok_or("Colección desconocida.")?;
    Ok((crate::paths::shared_data_dir(app).join(name), *max))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("k{:x}{:x}", nanos, SEQ.fetch_add(1, Ordering::Relaxed))
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn read(app: &tauri::AppHandle, kind: &str) -> Vec<Value> {
    file(app, kind).map(|(p, _)| crate::paths::read_json(&p)).unwrap_or_default()
}

#[tauri::command]
pub fn library_list(app: tauri::AppHandle, kind: String) -> Result<Vec<Value>, String> {
    file(&app, &kind)?;
    let _g = lock();
    Ok(read(&app, &kind))
}

/// Upsert pure: devuelve el elemento guardado (con `id`, `created` y `updated`).
fn upsert(list: &mut Vec<Value>, mut item: Map<String, Value>, max: usize) -> Result<Value, String> {
    let id = item.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let t = now();
    item.insert("updated".into(), t.into());
    match list.iter().position(|x| !id.is_empty() && x.get("id").and_then(Value::as_str) == Some(id.as_str())) {
        Some(i) => {
            let created = list[i].get("created").cloned().unwrap_or(t.into());
            item.insert("created".into(), created);
            list[i] = Value::Object(item);
            Ok(list[i].clone())
        }
        None => {
            if list.len() >= max {
                return Err(format!("Se admiten hasta {max} elementos."));
            }
            item.insert("id".into(), new_id().into());
            item.insert("created".into(), t.into());
            list.push(Value::Object(item));
            Ok(list.last().cloned().unwrap_or(Value::Null))
        }
    }
}

#[tauri::command]
pub fn library_save(app: tauri::AppHandle, kind: String, item: Value) -> Result<Value, String> {
    let (path, max) = file(&app, &kind)?;
    let Value::Object(map) = item else { return Err("Elemento no válido.".into()) };
    if serde_json::to_string(&map).map_or(0, |s| s.len()) > MAX_ITEM {
        return Err("El elemento es demasiado grande.".into());
    }
    let _g = lock();
    let mut list: Vec<Value> = crate::paths::read_json(&path);
    let saved = upsert(&mut list, map, max)?;
    crate::paths::write_json(&path, &list)?;
    Ok(saved)
}

#[tauri::command]
pub fn library_delete(app: tauri::AppHandle, kind: String, id: String) -> Result<(), String> {
    let (path, _) = file(&app, &kind)?;
    let _g = lock();
    let mut list: Vec<Value> = crate::paths::read_json(&path);
    list.retain(|x| x.get("id").and_then(Value::as_str) != Some(id.as_str()));
    crate::paths::write_json(&path, &list)
}

/// Suma un uso (para ordenar por «más usadas»).
#[tauri::command]
pub fn library_touch(app: tauri::AppHandle, kind: String, id: String) -> Result<(), String> {
    let (path, _) = file(&app, &kind)?;
    let _g = lock();
    let mut list: Vec<Value> = crate::paths::read_json(&path);
    if let Some(Value::Object(m)) = list.iter_mut().find(|x| x.get("id").and_then(Value::as_str) == Some(id.as_str())) {
        let uses = m.get("uses").and_then(Value::as_u64).unwrap_or(0) + 1;
        m.insert("uses".into(), uses.into());
        m.insert("lastUsed".into(), now().into());
        crate::paths::write_json(&path, &list)?;
    }
    Ok(())
}

// ---------- Dónde estamos: para las notas por equipo y por red ----------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// Clave de este equipo (nombre + número de serie, para no confundir equipos con el mismo nombre).
    pub machine: String,
    pub machine_label: String,
    /// Clave de la red actual (MAC del router) y su nombre.
    pub network: String,
    pub network_label: String,
}

fn serial() -> String {
    crate::pspool::query("(Get-CimInstance Win32_BIOS -ErrorAction SilentlyContinue).SerialNumber", Some(std::time::Duration::from_secs(15)), "Número de serie")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[tauri::command(async)]
pub fn this_place() -> Place {
    let host = sysinfo::System::host_name().unwrap_or_default();
    let serial = serial();
    let generic = crate::sheet::generic_serial(&serial);
    let machine = if generic { format!("pc:{}", host.to_lowercase()) } else { format!("pc:{}:{}", host.to_lowercase(), serial.to_lowercase()) };
    let (network, network_label) = crate::network::lan::current().ok().flatten().map(|i| (i.key, i.network)).unwrap_or_default();
    Place { machine, machine_label: host, network, network_label }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_keeps_created_and_limits() {
        let mut l = Vec::new();
        let mut m = Map::new();
        m.insert("title".into(), "Impresora".into());
        let a = upsert(&mut l, m, 2).unwrap();
        let id = a["id"].as_str().unwrap().to_string();
        let created = a["created"].clone();
        let mut m2 = Map::new();
        m2.insert("id".into(), id.clone().into());
        m2.insert("title".into(), "Impresora HP".into());
        let b = upsert(&mut l, m2, 2).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(b["title"], "Impresora HP");
        assert_eq!(b["created"], created);
        upsert(&mut l, Map::new(), 2).unwrap();
        assert!(upsert(&mut l, Map::new(), 2).is_err());
    }
}
