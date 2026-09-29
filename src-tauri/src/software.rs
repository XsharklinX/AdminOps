//! Actualizaciones de software con winget.

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

/// Última lista de winget: consultarla tarda 10-30 s, así que se reutiliza unos minutos.
static CACHE: Mutex<Option<(Instant, Vec<SoftwareUpdate>)>> = Mutex::new(None);
const CACHE_FOR: Duration = Duration::from_secs(10 * 60);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareUpdate {
    pub name: String,
    pub id: String,
    pub version: String,
    pub available: String,
    pub source: String,
}

/// Última línea "real" de una salida con barras de progreso (`\r`).
pub fn visible(line: &str) -> &str {
    line.rsplit('\r').next().unwrap_or(line)
}

/// Filas de las tablas que imprime winget (`upgrade`, `search`, `list`). Las
/// columnas se localizan por la posición de cada palabra del encabezado (la
/// línea anterior a "-----"), así que no depende del idioma de winget.
pub fn table(output: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = output.lines().map(visible).collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if i == 0 || t.len() < 10 || !t.chars().all(|c| c == '-') {
            continue;
        }
        let header: Vec<char> = lines[i - 1].chars().collect();
        let starts: Vec<usize> = (0..header.len())
            .filter(|&k| header[k] != ' ' && (k == 0 || header[k - 1] == ' '))
            .collect();
        if starts.len() < 3 {
            continue;
        }
        for row in &lines[i + 1..] {
            let chars: Vec<char> = row.chars().collect();
            if row.trim().is_empty() || chars.len() <= starts[1] {
                break;
            }
            let col = |k: usize| -> String {
                let from = starts[k].min(chars.len());
                let to = starts.get(k + 1).copied().unwrap_or(chars.len()).min(chars.len());
                chars[from..to].iter().collect::<String>().trim().to_string()
            };
            out.push((0..starts.len()).map(col).collect());
        }
    }
    out
}

/// Interpreta las tablas de `winget upgrade`.
pub fn parse_upgrades(output: &str) -> Vec<SoftwareUpdate> {
    table(output)
        .into_iter()
        .filter(|r| r.len() >= 4)
        .map(|r| SoftwareUpdate {
            name: r[0].clone(),
            id: r[1].clone(),
            version: r[2].clone(),
            available: r[3].clone(),
            source: r.get(4).cloned().unwrap_or_default(),
        })
        .filter(|u| !u.id.is_empty() && !u.available.is_empty())
        .collect()
}

/// Código de salida (el script termina con `"EXIT:$LASTEXITCODE"`) y última
/// línea útil de la salida de winget (sin barras de progreso).
pub fn outcome(out: &str) -> (Option<i64>, String) {
    let code = out.lines().last().and_then(|l| l.strip_prefix("EXIT:")).and_then(|c| c.trim().parse::<i64>().ok());
    let last = out
        .lines()
        .rev()
        .skip(1)
        .map(|l| visible(l).trim())
        .find(|l| l.len() > 3 && !l.chars().all(|c| "-\\|/ █▒".contains(c)))
        .unwrap_or("")
        .to_string();
    (code, last)
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
}

/// Programas con actualización disponible (y los guarda en la caché).
pub fn list() -> Result<Vec<SoftwareUpdate>, String> {
    // Proceso propio: winget tarda y no debe ocupar el PowerShell compartido de la app.
    let out = crate::ps::powershell_opts(
        "winget upgrade --include-unknown --accept-source-agreements --disable-interactivity | Out-String",
        crate::ps::Opts { timeout: Some(Duration::from_secs(120)), task: Some("software-list") },
    )
    .map_err(|e| {
        if e.contains("not recognized") || e.contains("no se reconoce") {
            "winget no está disponible en este equipo (se instala con 'Instalador de aplicación' desde Microsoft Store).".into()
        } else {
            e
        }
    })?;
    let v = parse_upgrades(&out);
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), v.clone()));
    Ok(v)
}

/// La lista reciente si la hay; si no, se consulta.
pub fn cached_or_list(max_age: Duration) -> Result<Vec<SoftwareUpdate>, String> {
    if let Some((t, v)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if t.elapsed() < max_age {
            return Ok(v.clone());
        }
    }
    list()
}

fn forget(id: &str) {
    if let Some((_, v)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        v.retain(|u| u.id != id);
    }
}

/// `refresh`: consultar winget aunque haya una lista reciente.
#[tauri::command(async)]
pub fn list_software_updates(refresh: Option<bool>) -> Result<Vec<SoftwareUpdate>, String> {
    if refresh.unwrap_or(false) {
        list()
    } else {
        cached_or_list(CACHE_FOR)
    }
}

/// La última lista conocida, sin consultar winget (para el Panel). None si no hay.
#[tauri::command]
pub fn cached_software_updates() -> Option<Vec<SoftwareUpdate>> {
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|(t, _)| t.elapsed() < Duration::from_secs(6 * 3600)).map(|(_, v)| v.clone())
}

// ---------- Actualizaciones que no se quieren (viajan con AdminOps) ----------

fn ignored_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("actualizaciones-ignoradas.json")
}

#[tauri::command]
pub fn ignored_updates(app: tauri::AppHandle) -> Vec<String> {
    crate::paths::read_json(&ignored_path(&app))
}

#[tauri::command]
pub fn set_update_ignored(app: tauri::AppHandle, id: String, ignored: bool) -> Result<Vec<String>, String> {
    if !valid_id(&id) {
        return Err("Identificador no válido.".into());
    }
    let mut v: Vec<String> = crate::paths::read_json(&ignored_path(&app));
    v.retain(|x| x != &id);
    if ignored {
        v.push(id);
        v.sort();
    }
    crate::paths::write_json(&ignored_path(&app), &v)?;
    Ok(v)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeResult {
    id: String,
    name: String,
    ok: bool,
    message: String,
}

#[tauri::command(async)]
pub fn upgrade_software(app: tauri::AppHandle, ids: Vec<String>, state: State<'_, TweakState>) -> Result<Vec<UpgradeResult>, String> {
    let task = Task::new(&app, "software");
    task.step("Comprobando actualizaciones disponibles…");
    // La lista que se acaba de ver en pantalla: no hace falta volver a preguntar a winget.
    let available = cached_or_list(Duration::from_secs(15 * 60))?;
    let total = ids.len();
    let mut results = Vec::new();
    for (i, id) in ids.into_iter().enumerate() {
        let Some(u) = available.iter().find(|u| u.id == id).filter(|_| valid_id(&id)) else {
            results.push(UpgradeResult { name: id.clone(), id, ok: false, message: "Ya no tiene actualización pendiente.".into() });
            continue;
        };
        if task.cancelled() {
            results.push(UpgradeResult { id, name: u.name.clone(), ok: false, message: crate::ps::CANCELLED_MSG.into() });
            continue;
        }
        task.step(format!("{}/{total} · Actualizando {} a {}…", i + 1, u.name, u.available));
        // Con su origen (winget o msstore) winget no consulta los demás: más rápido.
        let source = if matches!(u.source.as_str(), "winget" | "msstore") { format!(" --source {}", u.source) } else { String::new() };
        let script = format!(
            "$o = winget upgrade --id '{id}' --exact --silent --include-unknown{source} --accept-package-agreements \
             --accept-source-agreements --disable-interactivity | Out-String\n$o\n\"EXIT:$LASTEXITCODE\""
        );
        let r = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(30 * 60)))).and_then(|out| {
            let (code, last) = outcome(&out);
            match code {
                Some(0) => Ok(format!("Actualizado a {}", u.available)),
                _ => Err(if last.is_empty() { format!("winget terminó con código {code:?}") } else { last }),
            }
        });
        let title = format!("Actualizar {} ({} → {})", u.name, u.version, u.available);
        state.record(Op::Run, &title, &r);
        if r.is_ok() {
            forget(&id);
            crate::apps::invalidate_installed();
        }
        results.push(UpgradeResult {
            id,
            name: u.name.clone(),
            ok: r.is_ok(),
            message: r.unwrap_or_else(|e| e),
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\r   - \r   \\ \rName                                              Id                                   Version              Available           Source\r
--------------------------------------------------------------------------------------------------------------------------------------\r
7-Zip 25.01 (x64)                                 7zip.7zip                            25.01                26.03               winget\r
LOOT versión 0.29.1                               LOOT.LOOT                            0.29.1               0.29.2              winget\r
Albion Online                                     Albion.Online                        Unknown              25.08.27            winget\r
31 upgrades available.\r
";

    #[test]
    fn parses_winget_table() {
        let u = parse_upgrades(SAMPLE);
        assert_eq!(u.len(), 3, "{u:#?}");
        assert_eq!(u[0], SoftwareUpdate {
            name: "7-Zip 25.01 (x64)".into(),
            id: "7zip.7zip".into(),
            version: "25.01".into(),
            available: "26.03".into(),
            source: "winget".into(),
        });
        assert_eq!(u[1].name, "LOOT versión 0.29.1");
        assert_eq!(u[1].id, "LOOT.LOOT");
        assert_eq!(u[2].version, "Unknown");
    }

    #[test]
    fn rejects_suspicious_ids() {
        assert!(valid_id("Microsoft.VisualStudioCode"));
        assert!(!valid_id("x'; Remove-Item C:\\ -Recurse; '"));
    }

    /// Lista real (solo lectura): `cargo test list_real_updates -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn list_real_updates() {
        let u = list().unwrap();
        println!("{} actualizaciones", u.len());
        for x in u.iter().take(5) {
            println!("{x:?}");
        }
    }
}
