//! Configuración de empresa: exportarla e importarla en un archivo.
//!
//! Para que otro técnico empiece sin que se lo expliquen: el responsable
//! exporta una vez los datos de la empresa (nombre, logo, condiciones, precios,
//! tipos de visita y checklist), los portales (Tickets, Correo, Teams,
//! inventario), el dominio y la aplicación de Microsoft 365, y el nuevo técnico
//! importa el archivo en la bienvenida o en Ajustes.
//!
//! **Nunca lleva contraseñas ni sesiones**: ni las cuentas guardadas de los
//! portales, ni el token de Microsoft 365, ni la firma o el nombre del técnico.
//! Cada uno entra con su cuenta.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

const KIND: &str = "adminops-empresa";

/// Ajustes que son de la empresa (los demás son de cada técnico: su nombre,
/// su firma, sus avisos…).
const COMPANY_KEYS: &[&str] = &[
    "company",
    "phone",
    "email",
    "website",
    "logo",
    "conditions",
    "checklist",
    "defaultDomain",
    "currency",
    "taxName",
    "taxRate",
    "laborWarrantyDays",
    "maintenanceMonths",
    "quoteValidityDays",
    "catalog",
    "visitTypes",
];

/// Lo que se copia de cada portal (sin su id: cada equipo crea el suyo).
const PORTAL_KEYS: &[&str] = &["name", "url", "extraDomains", "kind", "zoom", "popups", "private", "autofill"];

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct CompanyFile {
    pub kind: String,
    pub version: String,
    pub created: u64,
    pub settings: serde_json::Map<String, Value>,
    pub portals: Vec<serde_json::Map<String, Value>>,
    /// Inquilino e id de la aplicación de Microsoft 365 (no el inicio de sesión).
    pub graph: Option<GraphApp>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphApp {
    pub tenant: String,
    pub client_id: String,
}

fn pick(v: &Value, keys: &[&str]) -> serde_json::Map<String, Value> {
    keys.iter().filter_map(|k| v.get(*k).map(|x| ((*k).to_string(), x.clone()))).collect()
}

/// Lo exportable, a partir de los ajustes y portales actuales.
pub fn build(settings: &Value, portals: &[Value], graph: Option<GraphApp>) -> CompanyFile {
    CompanyFile {
        kind: KIND.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        created: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        settings: pick(settings, COMPANY_KEYS),
        portals: portals.iter().filter(|p| p["kind"].as_str() != Some("router")).map(|p| pick(p, PORTAL_KEYS)).collect(),
        graph: graph.filter(|g| !g.tenant.is_empty() && !g.client_id.is_empty()),
    }
}

/// Ajustes actuales con los de la empresa encima (solo las claves de empresa).
pub fn merge_settings(current: &Value, file: &CompanyFile) -> Value {
    let mut out = current.clone();
    if let Some(o) = out.as_object_mut() {
        for (k, v) in &file.settings {
            if COMPANY_KEYS.contains(&k.as_str()) {
                o.insert(k.clone(), v.clone());
            }
        }
    }
    out
}

/// Portales del archivo que aún no están (se comparan por dirección).
pub fn new_portals<'a>(file: &'a CompanyFile, existing: &[Value]) -> Vec<&'a serde_json::Map<String, Value>> {
    let norm = |u: &str| u.trim().trim_end_matches('/').to_lowercase();
    file.portals
        .iter()
        .filter(|p| {
            let url = p.get("url").and_then(Value::as_str).unwrap_or("");
            !url.is_empty() && !existing.iter().any(|e| norm(e["url"].as_str().unwrap_or("")) == norm(url))
        })
        .collect()
}

fn current_graph(app: &tauri::AppHandle) -> Option<GraphApp> {
    let v = serde_json::to_value(crate::graph::graph_status(app.clone())).ok()?;
    Some(GraphApp { tenant: v["tenant"].as_str()?.to_string(), client_id: v["clientId"].as_str()?.to_string() })
}

/// Guarda la configuración de empresa en un archivo. None si se cancela.
#[tauri::command(async)]
pub fn company_export(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let settings = serde_json::to_value(crate::workflow::settings(&app)).map_err(|e| e.to_string())?;
    let portals: Vec<Value> = crate::portals::list_portals(app.clone()).iter().filter_map(|p| serde_json::to_value(p).ok()).collect();
    let file = build(&settings, &portals, current_graph(&app));
    let name = format!("AdminOps-empresa-{}.json", chrono::Local::now().format("%Y-%m-%d"));
    let Some(path) = app.dialog().file().set_file_name(&name).add_filter("Configuración de AdminOps", &["json"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("No se pudo guardar: {e}"))?;
    log::info!("Configuración de empresa exportada ({} portales)", file.portals.len());
    Ok(Some(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    /// Ruta del archivo (para aplicarlo); la interfaz no la enseña.
    path: String,
    company: String,
    domain: String,
    /// Portales nuevos y ya existentes.
    portals_new: Vec<String>,
    portals_existing: usize,
    graph: bool,
    visit_types: usize,
    catalog: usize,
}

fn read_file(path: &PathBuf) -> Result<CompanyFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("No se pudo leer: {e}"))?;
    let f: CompanyFile = serde_json::from_str(&text).map_err(|_| "Ese archivo no es una configuración de AdminOps.".to_string())?;
    if f.kind != KIND {
        return Err("Ese archivo no es una configuración de empresa de AdminOps.".into());
    }
    Ok(f)
}

/// Elige un archivo y enseña qué trae, sin aplicar nada. None si se cancela.
#[tauri::command(async)]
pub fn company_import_preview(app: tauri::AppHandle) -> Result<Option<ImportPreview>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(path) = app.dialog().file().add_filter("Configuración de AdminOps", &["json"]).blocking_pick_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let f = read_file(&path)?;
    let existing: Vec<Value> = crate::portals::list_portals(app.clone()).iter().filter_map(|p| serde_json::to_value(p).ok()).collect();
    let fresh = new_portals(&f, &existing);
    let s = |k: &str| f.settings.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let n = |k: &str| f.settings.get(k).and_then(Value::as_array).map_or(0, Vec::len);
    Ok(Some(ImportPreview {
        path: path.display().to_string(),
        company: s("company"),
        domain: s("defaultDomain"),
        portals_new: fresh.iter().filter_map(|p| p.get("name").and_then(Value::as_str).map(String::from)).collect(),
        portals_existing: f.portals.len() - fresh.len(),
        graph: f.graph.is_some(),
        visit_types: n("visitTypes"),
        catalog: n("catalog"),
    }))
}

/// Aplica lo elegido del archivo: datos de empresa, portales y Microsoft 365.
#[tauri::command(async)]
pub fn company_import_apply(app: tauri::AppHandle, path: String, settings: bool, portals: bool, graph: bool) -> Result<String, String> {
    let f = read_file(&PathBuf::from(&path))?;
    let mut done = Vec::new();
    if settings && !f.settings.is_empty() {
        let cur = serde_json::to_value(crate::workflow::settings(&app)).map_err(|e| e.to_string())?;
        let merged: crate::workflow::Settings = serde_json::from_value(merge_settings(&cur, &f)).map_err(|e| format!("Datos de empresa no válidos: {e}"))?;
        crate::workflow::save_settings(app.clone(), merged)?;
        done.push("datos de la empresa".to_string());
    }
    if portals {
        let existing: Vec<Value> = crate::portals::list_portals(app.clone()).iter().filter_map(|p| serde_json::to_value(p).ok()).collect();
        let mut n = 0;
        for p in new_portals(&f, &existing) {
            let mut v = Value::Object(p.clone());
            v["id"] = Value::String(String::new());
            if let Ok(portal) = serde_json::from_value::<crate::portals::Portal>(v) {
                if crate::portals::save_portal(app.clone(), portal).is_ok() {
                    n += 1;
                }
            }
        }
        if n > 0 {
            done.push(format!("{n} {}", if n == 1 { "portal" } else { "portales" }));
        }
    }
    if graph {
        if let Some(g) = &f.graph {
            crate::graph::graph_configure(app.clone(), g.tenant.clone(), g.client_id.clone())?;
            done.push("la aplicación de Microsoft 365 (falta que conectes tu cuenta)".into());
        }
    }
    log::info!("Configuración de empresa importada: {}", done.join(", "));
    Ok(if done.is_empty() { "No había nada nuevo que importar.".into() } else { format!("Importado: {}.", done.join(", ")) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Lo del técnico (nombre, firma, avisos) nunca viaja; los routers tampoco.
    #[test]
    fn export_keeps_only_company_things() {
        let settings = json!({ "technician": "David", "techSignature": "data:...", "company": "Soporte DB", "taxRate": 18.0, "visitTypes": [{ "name": "Rutina" }], "watchWindows": true });
        let portals = vec![
            json!({ "id": "p1", "name": "Tickets", "url": "https://tickets.example.com/", "kind": "", "autofill": true }),
            json!({ "id": "r1", "name": "Router", "url": "http://192.168.1.1", "kind": "router" }),
        ];
        let f = build(&settings, &portals, Some(GraphApp { tenant: "empresa.onmicrosoft.com".into(), client_id: "x".into() }));
        assert_eq!(f.settings.keys().cloned().collect::<Vec<_>>(), ["company", "taxRate", "visitTypes"]);
        assert_eq!(f.portals.len(), 1);
        assert!(!f.portals[0].contains_key("id"));
        let text = serde_json::to_string(&f).unwrap();
        assert!(!text.contains("David") && !text.contains("techSignature"));
        assert!(build(&settings, &[], Some(GraphApp::default())).graph.is_none(), "sin Microsoft 365 configurado no se exporta");
    }

    #[test]
    fn import_merges_and_skips_existing_portals() {
        let f = build(&json!({ "company": "Soporte DB", "currency": "RD$" }), &[json!({ "name": "Tickets", "url": "https://Tickets.example.com" }), json!({ "name": "Correo", "url": "https://outlook.office.com/mail/" })], None);
        let cur = json!({ "technician": "Ana", "company": "", "currency": "€", "watchWindows": false });
        let m = merge_settings(&cur, &f);
        assert_eq!((m["technician"].as_str(), m["company"].as_str(), m["currency"].as_str()), (Some("Ana"), Some("Soporte DB"), Some("RD$")));
        let existing = vec![json!({ "url": "https://tickets.example.com/" })];
        let fresh = new_portals(&f, &existing);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0]["name"], "Correo");
    }
}
