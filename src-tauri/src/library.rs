//! Conocimiento del técnico que viaja con AdminOps (en portable, en el USB):
//! soluciones («problema → lo que funcionó»), plantillas de texto, notas por
//! equipo y por red, plantillas de preparación de equipos y listas de puestos.
//!
//! Colecciones genéricas de documentos JSON con `id`: la forma de cada una la
//! define la interfaz; aquí se valida el tamaño y se guarda de forma atómica.

use serde::{Deserialize, Serialize};
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

/// El número de serie no cambia mientras AdminOps está abierta: se lee una vez.
fn serial() -> String {
    static SERIAL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SERIAL
        .get_or_init(|| {
            crate::pspool::query("(Get-CimInstance Win32_BIOS -ErrorAction SilentlyContinue).SerialNumber", Some(std::time::Duration::from_secs(15)), "Número de serie")
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        })
        .clone()
}

/// Lo que no cambia (el número de serie) y las redes ya vistas desde este equipo,
/// guardado en disco: identificar «este equipo y esta red» deja de costar un
/// PowerShell de varios segundos en cada arranque (7 s medidos desde un pendrive).
#[derive(Serialize, Deserialize, Default)]
struct PlaceCache {
    #[serde(default)]
    serial: Option<String>,
    /// Huella de la red (puerta de enlace y su MAC) → (clave, nombre).
    #[serde(default)]
    networks: std::collections::BTreeMap<String, (String, String)>,
}

fn place_cache_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("lugar.json")
}

fn load_place_cache(app: &tauri::AppHandle) -> PlaceCache {
    std::fs::read_to_string(place_cache_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_place_cache(app: &tauri::AppHandle, c: &PlaceCache) {
    if let Ok(json) = serde_json::to_string(c) {
        let _ = std::fs::write(place_cache_path(app), json);
    }
}

#[tauri::command(async)]
pub fn this_place(app: tauri::AppHandle) -> Place {
    let mut cache = load_place_cache(&app);
    let (place, dirty) = place_with(&mut cache);
    if dirty {
        save_place_cache(&app, &cache);
    }
    place
}

/// Sin nada guardado: lo pregunta todo a Windows.
#[cfg(test)]
pub(crate) fn place_uncached() -> Place {
    place_with(&mut PlaceCache::default()).0
}

fn place_with(cache: &mut PlaceCache) -> (Place, bool) {
    let host = sysinfo::System::host_name().unwrap_or_default();
    let mut dirty = false;
    let serial = match &cache.serial {
        Some(s) => s.clone(),
        None => {
            let s = serial();
            // Un fallo pasajero no se guarda: se volverá a preguntar.
            if !s.is_empty() {
                cache.serial = Some(s.clone());
                dirty = true;
            }
            s
        }
    };
    let generic = crate::sheet::generic_serial(&serial);
    let machine = if generic { format!("pc:{}", host.to_lowercase()) } else { format!("pc:{}:{}", host.to_lowercase(), serial.to_lowercase()) };
    // Una red ya vista se reconoce por su puerta de enlace, sin PowerShell. Solo
    // la primera vez en cada red se pregunta a Windows su clave y su nombre.
    let fp = crate::network::lan::fingerprint();
    let (network, network_label) = match fp.as_ref().and_then(|f| cache.networks.get(f)) {
        Some(known) => known.clone(),
        None => {
            let found = crate::network::lan::current_recent().ok().flatten().map(|i| (i.key, i.network)).unwrap_or_default();
            if let (Some(f), false) = (fp, found.0.is_empty()) {
                cache.networks.insert(f, found.clone());
                dirty = true;
            }
            found
        }
    };
    (Place { machine, machine_label: host, network, network_label }, dirty)
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
