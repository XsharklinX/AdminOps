//! Cuidado de AdminOps: inicio con Windows, limpieza de datos antiguos y aviso
//! de versiones nuevas.

use crate::tweaks::TweakState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

// ---------- Inicio con Windows ----------

/// Tarea programada al iniciar sesión, con los permisos más altos: AdminOps
/// necesita administrador y una entrada «Run» del registro no puede elevarse.
const TASK: &str = "AdminOps al iniciar sesión";

#[tauri::command(async)]
pub fn autostart_enabled() -> bool {
    let script = format!("{}[bool](Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue)", crate::ps::text_var("name", TASK));
    crate::pspool::query(&script, Some(Duration::from_secs(20)), "Inicio con Windows: estado").is_ok_and(|o| o.trim().eq_ignore_ascii_case("true"))
}

#[tauri::command(async)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = if enabled {
        format!(
            "$ErrorActionPreference = 'Stop'\n{}{}\
             $action = New-ScheduledTaskAction -Execute $exe -Argument '--minimized'\n\
             $trigger = New-ScheduledTaskTrigger -AtLogOn -User \"$env:USERDOMAIN\\$env:USERNAME\"\n\
             $trigger.Delay = 'PT20S'\n\
             $principal = New-ScheduledTaskPrincipal -UserId \"$env:USERDOMAIN\\$env:USERNAME\" -LogonType Interactive -RunLevel Highest\n\
             $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Seconds 0)\n\
             Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null\n'ok'",
            crate::ps::text_var("name", TASK),
            crate::ps::text_var("exe", &exe.display().to_string())
        )
    } else {
        format!("{}Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue\n'ok'", crate::ps::text_var("name", TASK))
    };
    crate::ps::powershell(&script).map(|_| ())
}

// ---------- Datos antiguos ----------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DataUsage {
    journal_entries: usize,
    snapshots: usize,
    snapshots_bytes: u64,
    reports: usize,
    reports_bytes: u64,
    speedtests: usize,
    logs_bytes: u64,
}

fn dir_files(dir: &Path) -> Vec<(PathBuf, u64, u64)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = e.metadata() else { continue };
        if m.is_dir() {
            out.extend(dir_files(&p));
        } else {
            let modified = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            out.push((p, m.len(), modified));
        }
    }
    out
}

#[tauri::command(async)]
pub fn data_usage(app: tauri::AppHandle, state: State<'_, TweakState>) -> DataUsage {
    let snaps = crate::diagnostics::snapshot_files(&crate::diagnostics::snapshots_dir(&app));
    let reports = dir_files(&crate::paths::reports_dir(&app));
    DataUsage {
        journal_entries: state.journal_len(),
        snapshots: snaps.len(),
        snapshots_bytes: snaps.iter().filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum(),
        reports: reports.len(),
        reports_bytes: reports.iter().map(|f| f.1).sum(),
        speedtests: crate::network::speedtest::history(&app).len(),
        logs_bytes: dir_files(&crate::paths::logs_dir(&app)).iter().map(|f| f.1).sum(),
    }
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cleaned {
    journal: usize,
    snapshots: usize,
    reports: usize,
    freed: u64,
}

/// Envía a la papelera (los informes pueden hacer falta: así se pueden recuperar).
fn to_recycle_bin(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let list: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    let script = format!(
        "{}Add-Type -AssemblyName Microsoft.VisualBasic\nforeach ($p in @(ConvertFrom-Json $json)) {{ [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($p, 'OnlyErrorDialogs', 'SendToRecycleBin') }}\n'ok'",
        crate::ps::text_var("json", &serde_json::to_string(&list).unwrap_or_default())
    );
    crate::pspool::query(&script, Some(Duration::from_secs(120)), "Limpieza: informes antiguos a la papelera").map(|_| ())
}

pub fn cleanup(app: &tauri::AppHandle, state: &TweakState, months: u32, journal: bool, snapshots: bool, reports: bool) -> Result<Cleaned, String> {
    if months == 0 {
        return Err("Elige una antigüedad.".into());
    }
    let cutoff = now().saturating_sub(months as u64 * 30 * 86_400);
    let mut out = Cleaned::default();
    if journal {
        out.journal = state.prune_journal(cutoff);
    }
    if snapshots {
        let mut snaps = crate::diagnostics::snapshot_files(&crate::diagnostics::snapshots_dir(app));
        snaps.sort_by_key(|s| std::cmp::Reverse(s.0));
        // El más reciente se queda siempre (es el «antes» de la próxima comparación), y el de una sesión en curso.
        let keep = crate::workflow::active_baseline(app);
        for (_, p) in snaps.into_iter().skip(1).filter(|(ts, _)| *ts < cutoff && Some(*ts) != keep) {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if std::fs::remove_file(&p).is_ok() {
                out.snapshots += 1;
                out.freed += size;
            }
        }
    }
    if reports {
        let old: Vec<(PathBuf, u64, u64)> = dir_files(&crate::paths::reports_dir(app)).into_iter().filter(|f| f.2 < cutoff).collect();
        to_recycle_bin(&old.iter().map(|f| f.0.clone()).collect::<Vec<_>>())?;
        out.reports = old.len();
        out.freed += old.iter().map(|f| f.1).sum::<u64>();
    }
    log::info!("Limpieza de datos ({months} meses): {} entradas, {} análisis, {} informes", out.journal, out.snapshots, out.reports);
    Ok(out)
}

#[tauri::command(async)]
pub fn data_cleanup(app: tauri::AppHandle, state: State<'_, TweakState>, months: u32, journal: bool, snapshots: bool, reports: bool) -> Result<Cleaned, String> {
    cleanup(&app, &state, months, journal, snapshots, reports)
}

/// Limpieza automática al abrir (Ajustes → General), como mucho una vez al día.
pub fn auto_cleanup(app: &tauri::AppHandle, state: &TweakState) {
    let months = crate::workflow::settings(app).auto_cleanup_months;
    if months == 0 {
        return;
    }
    let stamp = crate::paths::shared_data_dir(app).join("last-cleanup.txt");
    let last: u64 = std::fs::read_to_string(&stamp).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    if now().saturating_sub(last) < 86_400 {
        return;
    }
    let _ = std::fs::write(&stamp, now().to_string());
    if let Err(e) = cleanup(app, state, months, true, true, true) {
        log::warn!("Limpieza automática: {e}");
    }
}

// ---------- Versiones nuevas ----------

const RELEASES: &str = "https://api.github.com/repos/XsharklinX/AdminOps/releases/latest";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: String,
    newer: bool,
    url: String,
    notes: String,
    /// Hay un archivo publicado que AdminOps puede descargar e instalar aquí.
    installable: bool,
    /// Tamaño de ese archivo, en bytes (0 si no lo hay).
    size: u64,
    /// Hay alguna versión publicada en GitHub. Sin ninguna, no hay nada que comparar.
    published: bool,
}

fn parse_version(v: &str) -> Vec<u32> {
    v.trim_start_matches(['v', 'V']).split(['.', '-']).map(|p| p.parse().unwrap_or(0)).collect()
}

#[derive(serde::Deserialize, Clone, Debug, Default)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    /// «sha256:…», cuando GitHub lo da.
    #[serde(default)]
    digest: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct Release {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// Lo que se hizo con la huella del archivo descargado.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    /// "installer" o "portable".
    how: String,
    /// "both" (dos huellas publicadas coinciden), "one" (una) o "none" (no había con qué comparar).
    verified: String,
    /// La huella SHA-256 de lo descargado, para comprobarla a mano.
    sha256: String,
}

/// El `SHA256SUMS.txt` de la versión, si la versión lo trae (solo del repositorio de AdminOps).
async fn published_sums(client: &reqwest::Client, assets: &[Asset]) -> Option<String> {
    let a = assets.iter().find(|a| a.name == "SHA256SUMS.txt" && a.browser_download_url.starts_with(DOWNLOADS) && a.size < 64 * 1024)?;
    client.get(&a.browser_download_url).send().await.ok()?.error_for_status().ok()?.text().await.ok()
}

/// De dónde se descarga: solo los archivos publicados en el repositorio de AdminOps.
const DOWNLOADS: &str = "https://github.com/XsharklinX/AdminOps/releases/download/";

/// El archivo de la versión que toca en este equipo: el instalador, o el .zip
/// si AdminOps es la versión portable. El nombre tiene que ser exactamente el
/// que genera la build y venir del repositorio de AdminOps.
fn pick_asset(assets: &[Asset], version: &str, portable_zip: bool) -> Option<Asset> {
    let name = if portable_zip { format!("AdminOps-{version}-portable.zip") } else { format!("AdminOps-{version}-Setup.exe") };
    assets.iter().find(|a| a.name == name && a.browser_download_url.starts_with(DOWNLOADS) && a.size > 1_000_000).cloned()
}

/// ¿Es la versión portable (carpeta con el marcador), que se actualiza con el .zip?
fn portable_zip() -> bool {
    crate::paths::portable_reason() == "marker"
}

async fn latest_release(app: &tauri::AppHandle, current: &str) -> Result<Release, String> {
    crate::outbound::guard(app, "updates")?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(8)).user_agent(format!("AdminOps/{current}")).build().map_err(|e| e.to_string())?;
    let resp = client.get(RELEASES).send().await.map_err(|_| "No se pudo consultar si hay versiones nuevas: no hay conexión con GitHub.".to_string())?;
    // 404: el repositorio no tiene ninguna versión publicada (o no es público).
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Todavía no hay ninguna versión publicada para descargar.".into());
    }
    resp.error_for_status().map_err(|_| "No se pudo consultar si hay versiones nuevas.".to_string())?.json().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let r = match latest_release(&app, &current).await {
        Ok(r) => r,
        // Sin ninguna versión publicada no hay error que contar: se dice tal cual.
        Err(e) if e.starts_with("Todavía no hay") => {
            return Ok(UpdateInfo { latest: current.clone(), current, newer: false, url: String::new(), notes: String::new(), installable: false, size: 0, published: false });
        }
        Err(e) => return Err(e),
    };
    let latest = r.tag_name.trim_start_matches(['v', 'V']).to_string();
    let asset = pick_asset(&r.assets, &latest, portable_zip());
    // Solo se abren enlaces de GitHub.
    let url = if r.html_url.starts_with("https://github.com/") { r.html_url } else { "https://github.com/XsharklinX/AdminOps/releases/latest".into() };
    Ok(UpdateInfo {
        newer: parse_version(&r.tag_name) > parse_version(&current),
        installable: asset.is_some(),
        size: asset.map_or(0, |a| a.size),
        published: true,
        latest,
        current,
        url,
        notes: r.body.chars().take(2000).collect(),
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Descarga la versión nueva y la deja lista: abre el instalador (que cierra
/// AdminOps, actualiza y conserva los datos) o, en la versión portable, enseña
/// el .zip descargado. Devuelve "installer" o "portable".
///
/// No instala nada por su cuenta ni en silencio: el instalador se abre a la
/// vista y es el técnico quien pulsa «Actualizar».
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<InstallOutcome, String> {
    use futures_util::StreamExt;
    use sha2::Digest;
    use std::io::Write;
    let current = app.package_info().version.to_string();
    let r = latest_release(&app, &current).await?;
    if parse_version(&r.tag_name) <= parse_version(&current) {
        return Err(format!("Ya tienes la última versión ({current})."));
    }
    let latest = r.tag_name.trim_start_matches(['v', 'V']).to_string();
    let zip = portable_zip();
    let asset = pick_asset(&r.assets, &latest, zip).ok_or("Esa versión no tiene un archivo que AdminOps pueda instalar aquí. Descárgala desde su página.")?;

    let task = crate::task::Task::new(&app, "update").named(format!("Actualizar AdminOps a la {latest}"));
    let dir = if zip { tauri::Manager::path(&app).download_dir().unwrap_or_else(|_| std::env::temp_dir()) } else { std::env::temp_dir().join("AdminOps-actualizacion") };
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo preparar la descarga: {e}"))?;
    let out = dir.join(&asset.name);
    let part = dir.join(format!("{}.descargando", asset.name));

    let client = reqwest::Client::builder().connect_timeout(Duration::from_secs(15)).user_agent(format!("AdminOps/{current}")).build().map_err(|e| e.to_string())?;
    let resp = client.get(&asset.browser_download_url).send().await.and_then(|r| r.error_for_status()).map_err(|_| "No se pudo descargar la versión nueva.".to_string())?;
    let mut file = std::fs::File::create(&part).map_err(|e| format!("No se pudo guardar la descarga: {e}"))?;
    let mut hasher = sha2::Sha256::new();
    let mut stream = resp.bytes_stream();
    let (mut got, mut last) = (0u64, u64::MAX);
    let fail = |part: &std::path::Path, msg: String| {
        let _ = std::fs::remove_file(part);
        msg
    };
    while let Some(chunk) = stream.next().await {
        if task.cancelled() {
            drop(file);
            return Err(fail(&part, "Descarga cancelada.".into()));
        }
        let chunk = match chunk {
            Ok(c) => c,
            Err(_) => {
                drop(file);
                return Err(fail(&part, "La descarga se cortó. Vuelve a intentarlo.".into()));
            }
        };
        hasher.update(&chunk);
        if let Err(e) = file.write_all(&chunk) {
            drop(file);
            return Err(fail(&part, format!("No se pudo guardar la descarga: {e}")));
        }
        got += chunk.len() as u64;
        let pct = got * 100 / asset.size.max(1);
        if pct != last {
            last = pct;
            task.step(format!("Descargando la versión {latest}… {pct} %"));
        }
    }
    drop(file);
    // Lo descargado tiene que ser exactamente lo publicado.
    if got != asset.size {
        return Err(fail(&part, "La descarga quedó incompleta. Vuelve a intentarlo.".into()));
    }
    // La huella de lo descargado contra las publicadas: la que calcula GitHub y la del SHA256SUMS.txt.
    let sha256 = hex(&hasher.finalize());
    let sums = published_sums(&client, &r.assets).await.and_then(|t| crate::checksums::hash_for(&t, &asset.name));
    let verified = match crate::checksums::verdict(&sha256, asset.digest.as_deref(), sums.as_deref()) {
        crate::checksums::Verdict::Mismatch => {
            return Err(fail(&part, "El archivo descargado no coincide con la huella publicada. No se instala.".into()));
        }
        crate::checksums::Verdict::Verified { sources: 2 } => "both",
        crate::checksums::Verdict::Verified { .. } => "one",
        crate::checksums::Verdict::Unavailable => {
            log::warn!("Actualización: la versión no publica huella del archivo; solo se comprobó el tamaño.");
            "none"
        }
    };
    let _ = std::fs::remove_file(&out);
    std::fs::rename(&part, &out).map_err(|e| format!("No se pudo guardar la descarga: {e}"))?;
    log::info!("Actualización {latest} descargada ({} bytes)", got);

    let outcome = |how: &str| InstallOutcome { how: how.into(), verified: verified.into(), sha256: sha256.clone() };
    if zip {
        let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", out.display())).spawn();
        Ok(outcome("portable"))
    } else {
        crate::shellopen::open(&out.display().to_string())?;
        close_when_installer_runs(app.clone(), asset.name.clone());
        Ok(outcome("installer"))
    }
}

/// En cuanto el instalador está en marcha (el técnico aceptó el aviso de Windows),
/// AdminOps se cierra sola: así el instalador no tiene que cerrarla a la fuerza ni
/// encuentra sus archivos ocupados. Si en dos minutos no arranca (se canceló el
/// aviso), AdminOps sigue abierta.
fn close_when_installer_runs(app: tauri::AppHandle, installer: String) {
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut sys = sysinfo::System::new();
        while start.elapsed() < Duration::from_secs(120) {
            std::thread::sleep(Duration::from_millis(700));
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            if sys.processes().values().any(|p| p.name().eq_ignore_ascii_case(&installer)) {
                log::info!("Actualización: el instalador está en marcha; AdminOps se cierra");
                // Un momento para que el instalador termine de abrir su ventana.
                std::thread::sleep(Duration::from_millis(1500));
                app.exit(0);
                return;
            }
        }
        log::info!("Actualización: el instalador no llegó a abrirse; AdminOps sigue abierta");
    });
}

#[tauri::command]
pub fn open_release_page(url: String) -> Result<(), String> {
    if !url.starts_with("https://github.com/XsharklinX/AdminOps") {
        return Err("Dirección no permitida.".into());
    }
    crate::shellopen::open(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(parse_version("v1.0.1") > parse_version("1.0.0"));
        assert!(parse_version("1.1.10") > parse_version("1.1.9"));
        assert!(parse_version("1.0.0") > parse_version("0.23.0"));
        assert!(parse_version("0.23.0") == parse_version("v0.23.0"));
    }

    /// Solo se instala el archivo con el nombre exacto de la build, publicado en
    /// el repositorio de AdminOps.
    #[test]
    fn only_our_own_installer_is_picked() {
        let asset = |name: &str, url: &str, size| Asset { name: name.into(), browser_download_url: url.into(), size, digest: None };
        let ours = "https://github.com/XsharklinX/AdminOps/releases/download/v2.0.0/";
        let assets = vec![
            asset("AdminOps-2.0.0-instalador-clasico.exe", &format!("{ours}AdminOps-2.0.0-instalador-clasico.exe"), 8_000_000),
            asset("AdminOps-2.0.0-Setup.exe", &format!("{ours}AdminOps-2.0.0-Setup.exe"), 11_000_000),
            asset("AdminOps-2.0.0-portable.zip", &format!("{ours}AdminOps-2.0.0-portable.zip"), 10_000_000),
        ];
        assert_eq!(pick_asset(&assets, "2.0.0", false).unwrap().name, "AdminOps-2.0.0-Setup.exe");
        assert_eq!(pick_asset(&assets, "2.0.0", true).unwrap().name, "AdminOps-2.0.0-portable.zip");
        assert!(pick_asset(&assets, "2.0.1", false).is_none(), "otra versión no vale");
        // Mismo nombre, pero desde otro sitio o sospechosamente pequeño: no.
        let fake = vec![asset("AdminOps-2.0.0-Setup.exe", "https://example.com/AdminOps-2.0.0-Setup.exe", 11_000_000)];
        assert!(pick_asset(&fake, "2.0.0", false).is_none());
        let tiny = vec![asset("AdminOps-2.0.0-Setup.exe", &format!("{ours}AdminOps-2.0.0-Setup.exe"), 900)];
        assert!(pick_asset(&tiny, "2.0.0", false).is_none());
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
    }
}
