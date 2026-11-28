//! Preferencias de la interfaz que tienen que seguir ahí al pasar de modo técnico a modo usuario (y al revés): cada modo
//! usa su propia carpeta de WebView2 y, por tanto, su propio localStorage. Se guardan en un JSON junto a las notas.
//! Solo valen las claves de la lista: no es un almacén libre. Son datos de AdminOps (no cambian el equipo).

use std::collections::BTreeMap;

/// Claves permitidas: plantillas de notas y la entrada del diario de «Volver a como estaba» del teclado.
const KEYS: &[&str] = &["noteTemplates", "kbdUndo"];

static FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

type Prefs = BTreeMap<String, serde_json::Value>;

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("preferencias-interfaz.json")
}

fn check(key: &str) -> Result<(), String> {
    if KEYS.contains(&key) {
        Ok(())
    } else {
        Err("Preferencia no válida.".into())
    }
}

/// Lo guardado para `key`, o nada si no hay.
#[tauri::command]
pub fn ui_pref_get(app: tauri::AppHandle, key: String) -> Result<Option<serde_json::Value>, String> {
    check(&key)?;
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let all: Prefs = crate::paths::read_json(&path(&app));
    Ok(all.get(&key).filter(|v| !v.is_null()).cloned())
}

/// Guarda `value` para `key`; con `null` la quita.
#[tauri::command]
pub fn ui_pref_set(app: tauri::AppHandle, key: String, value: Option<serde_json::Value>) -> Result<(), String> {
    check(&key)?;
    if value.as_ref().is_some_and(|v| v.to_string().len() > 1_000_000) {
        return Err("Es demasiado grande.".into());
    }
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _x = crate::paths::CrossProcessLock::acquire("AdminOpsPrefs");
    let p = path(&app);
    // Con el archivo cifrado y bloqueado, o dañado, no se pisa: leer lo da por vacío, pero escribir lo rechaza si está bloqueado.
    let mut all: Prefs = crate::paths::read_json(&p);
    match value.filter(|v| !v.is_null()) {
        Some(v) => {
            all.insert(key, v);
        }
        None => {
            all.remove(&key);
        }
    }
    crate::paths::write_json(&p, &all)
}

#[cfg(test)]
mod tests {
    #[test]
    fn solo_claves_de_la_lista() {
        assert!(super::check("noteTemplates").is_ok());
        assert!(super::check("kbdUndo").is_ok());
        assert!(super::check("otra").is_err());
    }
}
