//! Instalar programas en lote con winget: catálogo verificado
//! (`tools/apps.toml`), listas predefinidas, listas propias del técnico y
//! búsqueda en winget para añadir cualquier otro programa.

use crate::software::{outcome, table, valid_id};
use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::State;

const CATALOG: &str = include_str!("../tools/apps.toml");
#[cfg(test)]
const CATEGORIES: &[&str] = &["browser", "compress", "media", "chat", "remote", "office", "utils", "tech", "security", "runtime", "games", "dev"];

/// El paquete ya estaba instalado y al día (winget intenta actualizar y no hay nada nuevo).
const UPDATE_NOT_APPLICABLE: i64 = 0x8A15002Bu32 as i32 as i64;
const ALREADY_INSTALLED: i64 = 0x8A150061u32 as i32 as i64;
const REBOOT_REQUIRED: i64 = 3010;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogApp {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "winget".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PresetList {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    apps: Vec<String>,
}

#[derive(Deserialize)]
struct Catalog {
    app: Vec<CatalogApp>,
    list: Vec<PresetList>,
}

static CAT: LazyLock<Catalog> = LazyLock::new(|| toml::from_str(CATALOG).expect("tools/apps.toml inválido"));

/// Lista propia del técnico. Guarda nombre y origen de cada programa porque
/// puede incluir programas que no están en el catálogo.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppList {
    #[serde(default)]
    id: String,
    name: String,
    apps: Vec<CatalogApp>,
}

static FILE_LOCK: Mutex<()> = Mutex::new(());

fn lists_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("app-lists.json")
}

fn source_for(id: &str) -> &'static str {
    // Los ids de Microsoft Store son 12 caracteres alfanuméricos en mayúsculas (9NKSQGP7F2NH).
    if id.len() == 12 && id.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        "msstore"
    } else {
        "winget"
    }
}

fn winget_missing(e: String) -> String {
    if e.contains("not recognized") || e.contains("no se reconoce") {
        "winget no está disponible en este equipo (se instala con 'Instalador de aplicación' desde Microsoft Store).".into()
    } else {
        e
    }
}

// ---------- Comandos ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCatalogView {
    apps: Vec<CatalogApp>,
    presets: Vec<PresetList>,
    lists: Vec<AppList>,
}

#[tauri::command]
pub fn app_catalog(app: tauri::AppHandle) -> AppCatalogView {
    AppCatalogView { apps: CAT.app.clone(), presets: CAT.list.clone(), lists: crate::paths::read_json(&lists_path(&app)) }
}

/// Ids de winget instalados en el equipo (vía `winget export`, independiente del idioma).
#[tauri::command(async)]
pub fn installed_apps() -> Result<Vec<String>, String> {
    let file = std::env::temp_dir().join(format!("adminops-winget-{}.json", std::process::id()));
    let script = format!(
        "$f = '{}'\nwinget export -o $f --accept-source-agreements --disable-interactivity | Out-Null\n\
         if (Test-Path $f) {{ Get-Content $f -Raw -Encoding UTF8; Remove-Item $f -Force }} else {{ '{{}}' }}",
        file.display()
    );
    let out = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(180)), task: None })
        .map_err(winget_missing)?;
    let v: serde_json::Value = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada de winget: {e}"))?;
    Ok(v["Sources"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["Packages"].as_array().cloned().unwrap_or_default())
        .filter_map(|p| p["PackageIdentifier"].as_str().map(String::from))
        .collect())
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    id: String,
    name: String,
    version: String,
    source: String,
}

pub fn parse_search(out: &str) -> Vec<SearchResult> {
    table(out)
        .into_iter()
        .filter(|r| r.len() >= 3 && valid_id(&r[1]))
        .map(|r| SearchResult {
            source: source_for(&r[1]).into(),
            id: r[1].clone(),
            name: r[0].clone(),
            version: r[2].clone(),
        })
        .collect()
}

#[tauri::command(async)]
pub fn search_apps(query: String) -> Result<Vec<SearchResult>, String> {
    let q = query.trim();
    if q.len() < 2 || q.len() > 60 || q.contains(['\n', '\r']) {
        return Err("Escribe entre 2 y 60 caracteres.".into());
    }
    let script = format!(
        "winget search --query '{}' --count 40 --accept-source-agreements --disable-interactivity | Out-String",
        q.replace('\'', "''")
    );
    let out = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })
        .map_err(winget_missing)?;
    Ok(parse_search(&out))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    id: String,
    name: String,
    ok: bool,
    message: String,
}

/// Instala los programas uno tras otro (winget no admite instalaciones en paralelo).
#[tauri::command(async)]
pub fn install_apps(app: tauri::AppHandle, apps: Vec<CatalogApp>, state: State<'_, TweakState>) -> Result<Vec<InstallResult>, String> {
    let task = Task::new(&app, "install-apps");
    let total = apps.len();
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for (i, a) in apps.into_iter().enumerate() {
        if !seen.insert(a.id.clone()) {
            continue;
        }
        if !valid_id(&a.id) {
            results.push(InstallResult { id: a.id, name: a.name, ok: false, message: "Identificador no válido.".into() });
            continue;
        }
        if task.cancelled() {
            results.push(InstallResult { id: a.id, name: a.name, ok: false, message: crate::ps::CANCELLED_MSG.into() });
            continue;
        }
        task.step(format!("{}/{total} · Instalando {}…", i + 1, a.name));
        let source = source_for(&a.id);
        let script = format!(
            "$o = winget install --id '{}' --exact --silent --source {source} --accept-package-agreements \
             --accept-source-agreements --disable-interactivity | Out-String\n$o\n\"EXIT:$LASTEXITCODE\"",
            a.id
        );
        let r = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(30 * 60))))
            .map_err(winget_missing)
            .and_then(|out| {
                let (code, last) = outcome(&out);
                match code {
                    Some(0) => Ok("Instalado".to_string()),
                    Some(REBOOT_REQUIRED) => Ok("Instalado (requiere reiniciar)".to_string()),
                    Some(UPDATE_NOT_APPLICABLE | ALREADY_INSTALLED) => Ok("Ya estaba instalado".to_string()),
                    _ => Err(if last.is_empty() { format!("winget terminó con código {code:?}") } else { last }),
                }
            });
        state.record(Op::Run, &format!("Instalar {}", a.name), &r);
        results.push(InstallResult { ok: r.is_ok(), message: r.unwrap_or_else(|e| e), id: a.id, name: a.name });
    }
    Ok(results)
}

#[tauri::command]
pub fn save_app_list(app: tauri::AppHandle, list: AppList) -> Result<AppList, String> {
    let mut l = list;
    l.name = l.name.trim().to_string();
    if l.name.is_empty() || l.name.chars().count() > 40 {
        return Err("La lista necesita un nombre (máximo 40 caracteres).".into());
    }
    l.apps.retain(|a| valid_id(&a.id));
    if l.apps.is_empty() {
        return Err("Selecciona al menos un programa.".into());
    }
    for a in &mut l.apps {
        a.source = source_for(&a.id).into();
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = lists_path(&app);
    let mut lists: Vec<AppList> = crate::paths::read_json(&path);
    match lists.iter_mut().find(|x| !l.id.is_empty() && x.id == l.id) {
        Some(x) => *x = l.clone(),
        None => {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            l.id = format!("list-{stamp:x}");
            lists.push(l.clone());
        }
    }
    crate::paths::write_json(&path, &lists)?;
    Ok(l)
}

#[tauri::command]
pub fn delete_app_list(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = lists_path(&app);
    let mut lists: Vec<AppList> = crate::paths::read_json(&path);
    lists.retain(|l| l.id != id);
    crate::paths::write_json(&path, &lists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        let mut ids = HashSet::new();
        for a in &CAT.app {
            assert!(ids.insert(a.id.as_str()), "repetido: {}", a.id);
            assert!(valid_id(&a.id), "{}", a.id);
            assert!(CATEGORIES.contains(&a.category.as_str()), "{}: categoría {}", a.id, a.category);
            assert_eq!(a.source, source_for(&a.id), "{}: origen", a.id);
        }
        for l in &CAT.list {
            for id in &l.apps {
                assert!(ids.contains(id.as_str()), "lista {}: {id} no está en el catálogo", l.id);
            }
        }
    }

    #[test]
    fn exit_codes() {
        assert_eq!(UPDATE_NOT_APPLICABLE, -1978335189);
        assert_eq!(ALREADY_INSTALLED, -1978335135);
    }

    #[test]
    fn parses_search_table() {
        let out = "\r  \\ \rName            Id                     Version  Match         Source\r\n\
                   ------------------------------------------------------------------------\r\n\
                   AnyDesk         AnyDesk.AnyDesk        9.5.0    Tag: remote   winget\r\n\
                   WhatsApp        9NKSQGP7F2NH           Unknown                msstore\r\n";
        let r = parse_search(out);
        assert_eq!(r.len(), 2, "{r:#?}");
        assert_eq!(r[0], SearchResult { id: "AnyDesk.AnyDesk".into(), name: "AnyDesk".into(), version: "9.5.0".into(), source: "winget".into() });
        assert_eq!(r[1].source, "msstore");
    }

    /// Usa la red: `cargo test catalog_ids_exist -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn catalog_ids_exist() {
        let bad: Vec<&str> = CAT
            .app
            .iter()
            .filter(|a| {
                let script = format!(
                    "winget show --id '{}' --exact --source {} --accept-source-agreements --disable-interactivity | Out-Null; $LASTEXITCODE",
                    a.id, a.source
                );
                crate::ps::powershell(&script).map(|c| c.trim() != "0").unwrap_or(true)
            })
            .map(|a| a.id.as_str())
            .collect();
        assert!(bad.is_empty(), "no existen: {bad:?}");
    }

    #[test]
    #[ignore]
    fn installed_real() {
        let v = installed_apps().unwrap();
        println!("{} instalados: {:?}", v.len(), &v[..v.len().min(10)]);
    }
}
