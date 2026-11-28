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

// ---------- Marca de «arrancó bien» y actualización fallida ----------

/// Carpeta de la marca de arranque: `%ProgramData%\AdminOps\arranque`. La crea el instalador; «Usuarios» puede crear
/// y cambiar archivos dentro (AdminOps escribe ahí sin administrador) y el instalador solo lee. El aviso de
/// actualización fallida va en la carpeta de las copias (`setups_dir`), que solo cambian los administradores.
pub fn boot_dir() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("AdminOps").join("arranque"))
}

/// La interfaz pintó su primera pantalla: se apunta `arranque-<versión>.ok` para que el instalador, que espera a
/// ver si la versión nueva abre, sepa que esta arrancó bien. Con el portable (.zip) no hay instalador: no se escribe.
/// Una instalada en un pendrive sí la escribe (también se actualiza con el instalador).
pub fn mark_boot_ok(version: &str) {
    if portable_zip() {
        return;
    }
    let Some(dir) = boot_dir() else { return };
    // Solo si la carpeta existe (la crea el instalador al abrir la versión nueva): si no, nadie está esperando.
    if !dir.is_dir() {
        return;
    }
    let name = format!("arranque-{version}.ok");
    if std::fs::write(dir.join(&name), now().to_string()).is_err() {
        log::warn!("No se pudo escribir la marca de arranque {name}.");
    }
}

/// Un campo («Clave: valor») de `actualizacion-fallida.txt`.
fn failed_field(file: &str, k: &str) -> Option<String> {
    file.trim_start_matches('\u{feff}').lines().find_map(|l| l.trim().strip_prefix(k).map(|v| v.trim().trim_end_matches('.').to_string())).filter(|v| !v.is_empty())
}

/// El texto del aviso a partir de `actualizacion-fallida.txt`.
pub fn failed_update_text(file: &str) -> Option<String> {
    let field = |k: &str| failed_field(file, k);
    let version = field("Version:")?;
    let when = field("Hora:").map(|h| format!(" ({h})")).unwrap_or_default();
    let why = field("Motivo:").map(|m| format!(" Motivo: {m}.")).unwrap_or_default();
    let previous = field("Anterior:").map(|v| format!("la {v}")).unwrap_or_else(|| "la versión anterior".into());
    Some(match field("Resultado:").as_deref() {
        Some("vuelta") => format!("La actualización a la {version} no llegó a abrir bien y AdminOps volvió sola a {previous}{when}.{why}"),
        Some("manual") => format!("La actualización a la {version} no llegó a abrir bien{when}. Se abrió el instalador de {previous} para volver; confírmalo en su ventana.{why}"),
        _ => format!("La actualización a la {version} no confirmó que abriera bien y no se pudo volver sola a la versión anterior{when}. Si algo falla, reinstala AdminOps desde la web.{why}"),
    })
}

/// ¿Sigue siendo noticia para la versión que corre? Si ya corre una más nueva que la de la actualización fallida, no.
/// Con la misma versión, «vuelta» es de una vez anterior (se volvió y luego se actualizó bien): tampoco.
pub fn failed_update_still_news(file: &str, running: &str) -> bool {
    let Some(version) = failed_field(file, "Version:") else { return false };
    match parse_version(&version).cmp(&parse_version(running)) {
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => failed_field(file, "Resultado:").as_deref() != Some("vuelta"),
        std::cmp::Ordering::Greater => true,
    }
}

/// Si el instalador dejó `actualizacion-fallida.txt` (en su carpeta de solo administradores), devuelve el aviso una
/// sola vez: no se puede borrar sin administrador, así que se apunta en los datos de este equipo que ya se enseñó.
fn take_failed_update(app: &tauri::AppHandle) -> Option<String> {
    let f = setups_dir()?.join("actualizacion-fallida.txt");
    let text = String::from_utf8_lossy(&std::fs::read(&f).ok()?).into_owned();
    let seen_file = crate::paths::machine_data_dir(app).join("aviso-actualizacion-visto.txt");
    let id = format!("{}|{}", failed_field(&text, "Version:").unwrap_or_default(), failed_field(&text, "Hora:").unwrap_or_default());
    if std::fs::read_to_string(&seen_file).ok().as_deref() == Some(id.as_str()) {
        return None;
    }
    let _ = std::fs::create_dir_all(crate::paths::machine_data_dir(app));
    let _ = std::fs::write(&seen_file, &id);
    if !failed_update_still_news(&text, &app.package_info().version.to_string()) {
        return None;
    }
    failed_update_text(&text)
}

// ---------- Inicio con Windows ----------

/// Tarea programada al iniciar sesión, con los permisos más altos: AdminOps
/// necesita administrador y una entrada «Run» del registro no puede elevarse.
const TASK: &str = "AdminOps al iniciar sesión";

/// Una línea de la comprobación tras actualizar.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SelfCheck {
    pub text: String,
    /// ok | fixed | warn | failed (una actualización que no abrió y se deshizo)
    pub state: &'static str,
}

/// La primera vez que se abre una versión nueva, AdminOps mira sus propias piezas y arregla lo que pueda: que su
/// carpeta de datos se pueda escribir, que el arranque del cohete apunte a este .exe, que no quede la tarea antigua y
/// que la barra de los usuarios se pueda leer. Devuelve solo lo que había que contar (nada si todo estaba bien).
#[tauri::command(async)]
pub fn selfcheck_after_update(app: tauri::AppHandle) -> Vec<SelfCheck> {
    let version = app.package_info().version.to_string();
    let mut out = Vec::new();
    if let Some(text) = (!crate::audit::active()).then(|| take_failed_update(&app)).flatten() {
        out.push(SelfCheck { text, state: "failed" });
    }
    let mark = crate::paths::shared_data_dir(&app).join("version-comprobada.txt");
    if std::fs::read_to_string(&mark).ok().as_deref() == Some(version.as_str()) {
        return out;
    }
    // Con el modo auditoría no se cambia nada: solo se mira.
    let audit = crate::audit::active();
    let data = crate::paths::shared_data_dir(&app);
    let probe = data.join(".adminops-comprobacion");
    if std::fs::create_dir_all(&data).is_err() || std::fs::write(&probe, b"ok").is_err() {
        out.push(SelfCheck { text: "No se puede escribir en la carpeta de datos de AdminOps: no se guardarán notas ni casos. Revisa los permisos o el espacio del disco.".into(), state: "warn" });
    }
    let _ = std::fs::remove_file(&probe);
    if !crate::paths::is_portable() && !audit {
        if crate::rocketplus::fix_startup_path() == Some(true) {
            out.push(SelfCheck { text: "El cohete al iniciar sesión apuntaba a la versión anterior: corregido.".into(), state: "fixed" });
        }
        match crate::rocketplus::fix_legacy_task() {
            Some(true) => out.push(SelfCheck { text: "Quitada la tarea antigua del cohete (ahora arranca sin administrador).".into(), state: "fixed" }),
            Some(false) => out.push(SelfCheck { text: "Queda la tarea antigua del cohete: pasa a modo técnico una vez para quitarla.".into(), state: "warn" }),
            None => {}
        }
    }
    if let Some(pd) = std::env::var_os("ProgramData") {
        let f = std::path::PathBuf::from(pd).join("AdminOps").join("barra-usuarios.json");
        if let Ok(b) = std::fs::read(&f) {
            if serde_json::from_slice::<Vec<String>>(&b).is_err() && !audit {
                let fixed = std::fs::remove_file(&f).is_ok();
                out.push(SelfCheck { text: if fixed { "La barra de los usuarios estaba dañada: vuelve a la de fábrica.".into() } else { "La barra de los usuarios está dañada: elígela otra vez en Ajustes (modo técnico).".into() }, state: if fixed { "fixed" } else { "warn" } });
            }
        }
    }
    // Si queda algo por hacer (la tarea antigua sin modo técnico…), se vuelve a mirar en el próximo arranque.
    if !audit && !out.iter().any(|c| c.state == "warn") {
        let _ = std::fs::write(&mark, &version);
    }
    out
}

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
             $principal = New-ScheduledTaskPrincipal -UserId \"$env:USERDOMAIN\\$env:USERNAME\" -LogonType Interactive -RunLevel Limited\n\
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

/// Versión del formato de los datos de AdminOps. **Súbelo** cuando una versión
/// cambie sus datos de forma que la anterior no los sepa leer: entonces «Volver a
/// la versión anterior» avisa antes de hacerlo.
pub const DATA_FORMAT: u32 = 1;

/// El Setup de la versión anterior, guardado antes de actualizar (1.5, idea 11).
#[derive(Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PreviousSetup {
    pub version: String,
    pub file: String,
    pub sha256: String,
    pub size: u64,
    /// Cuándo se guardó (segundos Unix).
    pub saved_at: u64,
    /// El `DATA_FORMAT` de esa versión.
    pub data_format: u32,
}

/// Lo que ve Ajustes → Actualizaciones.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviousInfo {
    pub version: String,
    pub size: u64,
    pub saved_at: u64,
    /// Los datos de ahora los lee también la versión anterior.
    pub compatible: bool,
}

fn previous_dir(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("version-anterior")
}

/// La versión y la huella de la copia guardada se apuntan también en HKLM, que solo puede
/// escribir un administrador: la carpeta de datos la puede tocar cualquier programa del
/// usuario, y lo que se ejecuta al volver se ejecuta como administrador.
const PREVIOUS_KEY: &str = r"SOFTWARE\AdminOps\VersionAnterior";

#[cfg(windows)]
fn trusted_previous() -> Option<(String, String)> {
    let k = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(PREVIOUS_KEY).ok()?;
    Some((k.get_value("Version").ok()?, k.get_value("Sha256").ok()?))
}

#[cfg(not(windows))]
fn trusted_previous() -> Option<(String, String)> {
    None
}

#[cfg(windows)]
fn set_trusted_previous(version: &str, sha256: &str) -> Result<(), String> {
    let (k, _) = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).create_subkey(PREVIOUS_KEY).map_err(|e| format!("sin permiso para apuntarla ({e})"))?;
    k.set_value("Version", &version.to_string()).map_err(|e| e.to_string())?;
    k.set_value("Sha256", &sha256.to_string()).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn set_trusted_previous(_version: &str, _sha256: &str) -> Result<(), String> {
    Err("solo en Windows".into())
}

/// Copias del instalador que guarda el propio instalador (la versión instalada y la anterior), en una carpeta que
/// solo cambian los administradores; su huella y su formato de datos, en HKLM. Desde la 1.5.3.
const SETUPS_KEY: &str = r"SOFTWARE\AdminOps\Instaladores";

fn setups_dir() -> Option<PathBuf> {
    std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join("AdminOps").join("instalador"))
}

/// La copia protegida más nueva que sea anterior a `current`: (info, ruta, huella apuntada en HKLM).
#[cfg(windows)]
fn protected_previous(current: &str) -> Option<(PreviousInfo, PathBuf, String)> {
    let root = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(SETUPS_KEY).ok()?;
    let dir = setups_dir()?;
    let mut best: Option<(PreviousInfo, PathBuf, String)> = None;
    for v in root.enum_keys().flatten() {
        let file = format!("AdminOps-{v}-Setup.exe");
        if !setup_name_ok(&file, &v) || parse_version(&v) >= parse_version(current) {
            continue;
        }
        if best.as_ref().is_some_and(|(b, _, _)| parse_version(&b.version) >= parse_version(&v)) {
            continue;
        }
        let Ok(k) = root.open_subkey(&v) else { continue };
        let Ok(sha) = k.get_value::<String, _>("Sha256") else { continue };
        let format: u32 = k.get_value("DataFormat").unwrap_or(0);
        let path = dir.join(&file);
        let Ok(m) = std::fs::metadata(&path) else { continue };
        let saved_at = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        best = Some((PreviousInfo { version: v.clone(), size: m.len(), saved_at, compatible: format == DATA_FORMAT }, path, sha));
    }
    best
}

#[cfg(not(windows))]
fn protected_previous(_current: &str) -> Option<(PreviousInfo, PathBuf, String)> {
    None
}

/// ¿Ya hay copia protegida del instalador de esta versión? Entonces no hace falta descargarla antes de actualizar.
#[cfg(windows)]
fn protected_copy_of(version: &str) -> bool {
    let has_key = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(format!(r"{SETUPS_KEY}\{version}")).is_ok();
    has_key && setups_dir().is_some_and(|d| d.join(format!("AdminOps-{version}-Setup.exe")).is_file())
}

#[cfg(not(windows))]
fn protected_copy_of(_version: &str) -> bool {
    false
}

/// El único nombre válido para la copia: el que genera la build, con una versión de números y puntos.
pub fn setup_name_ok(file: &str, version: &str) -> bool {
    !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.') && file == format!("AdminOps-{version}-Setup.exe")
}

/// La versión guardada sirve si es más antigua que la de ahora y su archivo sigue ahí con su tamaño.
pub fn usable_previous(meta: &PreviousSetup, current: &str, file_len: Option<u64>) -> Option<PreviousInfo> {
    if meta.version.is_empty() || meta.size == 0 || parse_version(&meta.version) >= parse_version(current) || file_len != Some(meta.size) {
        return None;
    }
    Some(PreviousInfo { version: meta.version.clone(), size: meta.size, saved_at: meta.saved_at, compatible: meta.data_format == DATA_FORMAT })
}

/// Abre la copia sin dejar que nadie la cambie ni la borre mientras está abierta (solo leer).
fn open_locked(path: &Path) -> Result<std::fs::File, String> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(1); // FILE_SHARE_READ
    }
    o.open(path).map_err(|e| format!("No se pudo leer la copia guardada: {e}"))
}

fn sha256_of(mut f: &std::fs::File) -> Result<String, String> {
    use sha2::Digest;
    use std::io::Read;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("No se pudo leer la copia guardada: {e}"))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// Antes de actualizar: descarga el Setup de la versión instalada (la de ahora), lo
/// comprueba con su huella publicada y lo guarda para poder volver. Si no se puede
/// (sin conexión, sin huella), se actualiza igual y se apunta en el registro.
async fn save_current_setup(app: &tauri::AppHandle, client: &reqwest::Client, current: &str, task: &crate::task::Task) -> Result<(), String> {
    use futures_util::StreamExt;
    use sha2::Digest;
    use std::io::Write;
    let dir = previous_dir(app);
    let meta_path = dir.join("version.json");
    let have: PreviousSetup = crate::paths::read_json(&meta_path);
    let name = format!("AdminOps-{current}-Setup.exe");
    if have.version == current && setup_name_ok(&have.file, &have.version) && std::fs::metadata(dir.join(&have.file)).is_ok_and(|m| m.len() == have.size) && trusted_previous().is_some_and(|(v, h)| v == current && h == have.sha256) {
        return Ok(());
    }
    // Tope: guardar la copia no puede retener la actualización.
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    let url = format!("https://api.github.com/repos/XsharklinX/AdminOps/releases/tags/v{current}");
    let r: Release = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|_| format!("no se encontró la versión {current} publicada"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let asset = pick_asset(&r.assets, current, false).ok_or_else(|| format!("la versión {current} no publica su Setup"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let part = dir.join(format!("{name}.descargando"));
    let resp = client.get(&asset.browser_download_url).send().await.and_then(|r| r.error_for_status()).map_err(|_| "no se pudo descargar".to_string())?;
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hasher = sha2::Sha256::new();
    let mut got = 0u64;
    let mut stream = resp.bytes_stream();
    loop {
        if task.cancelled() || std::time::Instant::now() > deadline {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err(if task.cancelled() { "cancelado".into() } else { "tardó demasiado".into() });
        }
        let next = match tokio::time::timeout(Duration::from_secs(20), stream.next()).await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(_) => {
                drop(file);
                let _ = std::fs::remove_file(&part);
                return Err("la descarga dejó de responder".into());
            }
        };
        let Ok(chunk) = next else {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err("la descarga se cortó".into());
        };
        hasher.update(&chunk);
        if let Err(e) = file.write_all(&chunk) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err(e.to_string());
        }
        got += chunk.len() as u64;
    }
    drop(file);
    let sha256 = hex(&hasher.finalize());
    let sums = published_sums(client, &r.assets).await.and_then(|t| crate::checksums::hash_for(&t, &asset.name));
    // Solo se guarda lo comprobado con una huella publicada: es lo que se instalaría al volver.
    let ok = got == asset.size && matches!(crate::checksums::verdict(&sha256, asset.digest.as_deref(), sums.as_deref()), crate::checksums::Verdict::Verified { .. });
    if !ok {
        let _ = std::fs::remove_file(&part);
        return Err("no coincide con su huella publicada (o no la publica)".into());
    }
    // Solo se guarda una: la que hubiera se va.
    if !have.file.is_empty() && have.file != name && !have.file.contains(['/', '\\']) {
        let _ = std::fs::remove_file(dir.join(&have.file));
    }
    let out = dir.join(&name);
    let _ = std::fs::remove_file(&out);
    std::fs::rename(&part, &out).map_err(|e| e.to_string())?;
    // Sin la huella en HKLM la copia no sirve para volver: se borra.
    if let Err(e) = set_trusted_previous(current, &sha256) {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    let meta = PreviousSetup { version: current.into(), file: name, sha256, size: got, saved_at: now(), data_format: DATA_FORMAT };
    crate::paths::write_json(&meta_path, &meta)?;
    log::info!("Actualización: guardado el Setup de la {current} para poder volver ({got} bytes)");
    Ok(())
}

fn previous_meta(app: &tauri::AppHandle) -> Option<(PreviousSetup, PathBuf)> {
    let dir = previous_dir(app);
    let meta: PreviousSetup = crate::paths::read_json(&dir.join("version.json"));
    // Solo el nombre exacto que genera la build («AdminOps-1.5.0-Setup.exe»): nada de rutas ni «C:otro.exe».
    if !setup_name_ok(&meta.file, &meta.version) {
        return None;
    }
    let path = dir.join(&meta.file);
    Some((meta, path))
}

/// Ajustes → Actualizaciones: la versión anterior guardada, si hay una que sirva.
#[tauri::command(async)]
pub fn previous_version(app: tauri::AppHandle) -> Option<PreviousInfo> {
    if portable_zip() {
        return None;
    }
    let current = app.package_info().version.to_string();
    // Primero la copia que guarda el instalador (1.5.3 en adelante); si no, la que guardaba AdminOps antes de actualizar.
    if let Some((info, _, _)) = protected_previous(&current) {
        return Some(info);
    }
    let (meta, path) = previous_meta(&app)?;
    let len = std::fs::metadata(&path).ok().map(|m| m.len());
    usable_previous(&meta, &current, len)
}

/// «Volver a la X»: comprueba otra vez la huella de la copia guardada y abre su
/// instalador en modo «volver» (deja instalar una versión más antigua encima).
/// `accept_data`: el técnico ya vio el aviso de que los datos cambiaron.
#[tauri::command(async)]
pub fn rollback_previous(app: tauri::AppHandle, accept_data: bool) -> Result<String, String> {
    let info = previous_version(app.clone()).ok_or("No hay ninguna versión anterior guardada.")?;
    if !info.compatible && !accept_data {
        return Err(format!("La {} no sabe leer algunos datos de esta versión. Confírmalo antes de volver.", info.version));
    }
    if let Some((p, path, trusted_sha)) = protected_previous(&app.package_info().version.to_string()) {
        // Abierta y bloqueada desde que se comprueba hasta que el instalador arranca.
        let locked = open_locked(&path)?;
        if sha256_of(&locked)? != trusted_sha {
            return Err(format!("La copia guardada de la {} está dañada (su huella no coincide). No se usa.", p.version));
        }
        log::info!("Volver a la versión anterior: se abre el Setup guardado de la {}", p.version);
        crate::shellopen::run_installer(&path.display().to_string(), "--volver")?;
        close_when_installer_runs_holding(app, format!("AdminOps-{}-Setup.exe", p.version), Some(locked));
        return Ok(p.version);
    }
    let (meta, path) = previous_meta(&app).ok_or("No hay ninguna versión anterior guardada.")?;
    // La huella buena es la de HKLM (solo la escribe un administrador), no la del archivo de la carpeta de datos.
    let (trusted_version, trusted_sha) = trusted_previous().ok_or("No hay ninguna versión anterior guardada.")?;
    if trusted_version != meta.version {
        return Err("La copia guardada no es la que AdminOps apuntó. No se usa.".into());
    }
    // Abierta y bloqueada desde que se comprueba hasta que el instalador arranca: nadie puede cambiarla entre medias.
    let locked = open_locked(&path)?;
    if sha256_of(&locked)? != trusted_sha {
        return Err(format!("La copia guardada de la {} está dañada (su huella no coincide). No se usa.", meta.version));
    }
    log::info!("Volver a la versión anterior: se abre el Setup de la {}", meta.version);
    crate::shellopen::run_installer(&path.display().to_string(), "--volver")?;
    close_when_installer_runs_holding(app, meta.file.clone(), Some(locked));
    Ok(meta.version)
}

/// Descarga la versión nueva, la comprueba y la instala sin más pasos (lo pide el técnico
/// desde la ventana de «Versión nueva»): el instalador se abre con `--actualizar`, actualiza
/// conservando los datos y vuelve a abrir AdminOps; el portable se cambia solo al cerrarse.
/// Devuelve "installer", "portable" o "portable-manual" (no se pudo solo: se enseña el .zip).
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<InstallOutcome, String> {
    use futures_util::StreamExt;
    use sha2::Digest;
    use std::io::Write;
    let current = app.package_info().version.to_string();
    // Una sola actualización a la vez (Ajustes y la ventana de «Versión nueva» usan el mismo archivo).
    let _busy = UpdateBusy::take().ok_or("Ya se está descargando la actualización.")?;
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

    // Si la red se queda muda a mitad (Wi-Fi caída sin cortar la conexión), la descarga falla en 30 s en vez de esperar para siempre.
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .user_agent(format!("AdminOps/{current}"))
        .build()
        .map_err(|e| e.to_string())?;
    // Antes de nada, la versión de ahora queda guardada para poder volver a ella (no en el portable:
    // ahí la carpeta anterior ya es la copia).
    // Si el instalador ya guardó la suya (1.5.3 en adelante), no hace falta. Y sin administrador no se puede apuntar en
    // HKLM: la descarga no serviría de nada.
    if !zip && !protected_copy_of(&current) && crate::elevation::is_elevated() {
        task.step(format!("Guardando la versión {current} por si hay que volver…"));
        if let Err(e) = save_current_setup(&app, &client, &current, &task).await {
            log::warn!("Actualización: no se guardó la versión {current} para poder volver: {e}");
        }
    }
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
        // El portable se actualiza solo: se descomprime aparte y, en cuanto AdminOps se cierra,
        // se copia encima de su carpeta (los datos y el marcador no se tocan) y se vuelve a abrir.
        task.step("Preparando la versión nueva…");
        match stage_portable(&out, &std::env::temp_dir().join("AdminOps-actualizacion").join(format!("AdminOps-{latest}-nueva"))).and_then(|staged| swap_portable_on_exit(&app, &staged, &out)) {
            Ok(()) => Ok(outcome("portable")),
            Err(e) => {
                log::warn!("Actualización del portable: no se pudo hacer sola ({e}); se enseña el .zip");
                let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", out.display())).spawn();
                Ok(outcome("portable-manual"))
            }
        }
    } else {
        // Con `--actualizar` el instalador actualiza sin preguntar nada y vuelve a abrir AdminOps.
        // El instalador pide administrador (instala en Archivos de programa): en modo usuario sale el aviso de Windows.
        // `--anterior`: dónde está la copia de la versión de ahora, por si la nueva no abre y el instalador tiene que volver.
        let args = format!("--actualizar --anterior \"{}\"", previous_dir(&app).display());
        crate::shellopen::run_installer(&out.display().to_string(), &args)?;
        close_when_installer_runs(app.clone(), asset.name.clone());
        Ok(outcome("installer"))
    }
}

/// Descomprime el .zip del portable en `to`: solo `AdminOps.exe` y la carpeta `lhm`, sin el
/// marcador (el de la carpeta de ahora se queda como está) y sin nada fuera de la carpeta.
fn stage_portable(zip_path: &Path, to: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to).map_err(|e| format!("no se pudo preparar la carpeta: {e}"))?;
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("el .zip no se puede abrir: {e}"))?;
    for i in 0..z.len() {
        let mut entry = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name() else { continue };
        // Dentro del .zip todo va en «AdminOps-<versión>-portable/…»: se quita esa primera carpeta.
        let rel: PathBuf = name.components().skip(1).collect();
        let first = rel.components().next().map(|c| c.as_os_str().to_string_lossy().to_lowercase());
        let wanted = match first.as_deref() {
            Some("adminops.exe") => rel.components().count() == 1,
            Some("lhm") => true,
            _ => false,
        };
        if !wanted || entry.is_dir() {
            continue;
        }
        let dest = to.join(&rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("no se pudo descomprimir: {e}"))?;
    }
    if !to.join("AdminOps.exe").metadata().is_ok_and(|m| m.len() > 1_000_000) {
        return Err("el .zip no trae AdminOps.exe".into());
    }
    Ok(to.to_path_buf())
}

/// Un PowerShell oculto espera a que AdminOps se cierre, copia la versión nueva encima del
/// portable (reintentando mientras haya archivos ocupados) y lo vuelve a abrir. AdminOps se
/// cierra sola poco después. Si no puede copiar, abre igual AdminOps y enseña el .zip.
fn swap_portable_on_exit(app: &tauri::AppHandle, staged: &Path, zip_path: &Path) -> Result<(), String> {
    use base64::Engine;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dst = exe.parent().ok_or("sin carpeta del programa")?;
    if !crate::paths::exe_dir_writable() {
        return Err("la carpeta del portable no se puede escribir".into());
    }
    let q = |p: &Path| crate::psquote::ps_escape(&p.display().to_string());
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$src = '{src}'; $dst = '{dst}'; $exe = '{exe}'; $zip = '{zip}'
try {{ Wait-Process -Id {pid} -Timeout 90 -ErrorAction SilentlyContinue }} catch {{}}
if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ Start-Process explorer.exe -ArgumentList ('/select,' + [char]34 + $zip + [char]34); exit }}
$ok = $false
for ($i = 0; $i -lt 60 -and -not $ok; $i++) {{
  try {{
    if (Test-Path -LiteralPath (Join-Path $src 'lhm')) {{ Copy-Item -LiteralPath (Join-Path $src 'lhm') -Destination $dst -Recurse -Force }}
    Copy-Item -LiteralPath (Join-Path $src 'AdminOps.exe') -Destination $exe -Force
    $ok = $true
  }} catch {{ Start-Sleep -Milliseconds 500 }}
}}
Start-Process -FilePath $exe -WorkingDirectory $dst
if ($ok) {{ Remove-Item -LiteralPath $src -Recurse -Force -ErrorAction SilentlyContinue }} else {{ Start-Process explorer.exe -ArgumentList ('/select,' + [char]34 + $zip + [char]34) }}"#,
        src = q(staged),
        dst = q(dst),
        exe = q(&exe),
        zip = q(zip_path),
        pid = std::process::id(),
    );
    let wide: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(wide);
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-EncodedCommand", &encoded]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().map_err(|e| format!("no se pudo preparar el cambio: {e}"))?;
    log::info!("Actualización del portable lista: AdminOps se cierra para cambiar de versión");
    let app = app.clone();
    std::thread::spawn(move || {
        // Un momento para que la ventana diga que se reinicia.
        std::thread::sleep(Duration::from_millis(2500));
        app.exit(0);
    });
    Ok(())
}

/// En cuanto el instalador está en marcha (el técnico aceptó el aviso de Windows),
/// AdminOps se cierra sola: así el instalador no tiene que cerrarla a la fuerza ni
/// encuentra sus archivos ocupados. Si en dos minutos no arranca (se canceló el
/// aviso), AdminOps sigue abierta.
fn close_when_installer_runs(app: tauri::AppHandle, installer: String) {
    close_when_installer_runs_holding(app, installer, None);
}

/// Igual, guardando abierto (y bloqueado) el archivo del instalador hasta que arranca o pasan dos minutos.
fn close_when_installer_runs_holding(app: tauri::AppHandle, installer: String, hold: Option<std::fs::File>) {
    std::thread::spawn(move || {
        let _hold = hold;
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

/// ¿Se está instalando una actualización? (Al cerrarse por ella no se deja el cohete abierto.)
pub fn updating() -> bool {
    UPDATING.load(std::sync::atomic::Ordering::Acquire)
}

/// Candado de `install_update`: se suelta al terminar, salga bien o mal.
struct UpdateBusy;

static UPDATING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl UpdateBusy {
    fn take() -> Option<Self> {
        use std::sync::atomic::Ordering;
        UPDATING.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).ok().map(|_| UpdateBusy)
    }
}

impl Drop for UpdateBusy {
    fn drop(&mut self) {
        UPDATING.store(false, std::sync::atomic::Ordering::Release);
    }
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

    /// Del .zip del portable solo salen el programa y sus sensores: ni el marcador ni nada fuera de la carpeta.
    #[test]
    fn el_portable_nuevo_se_desempaqueta_sin_el_marcador() {
        use std::io::Write;
        let base = std::env::temp_dir().join(format!("adminops-stage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let zip_path = base.join("AdminOps-9.9.9-portable.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("AdminOps-9.9.9-portable/AdminOps.exe", o).unwrap();
            z.write_all(&vec![7u8; 1_200_000]).unwrap();
            z.start_file("AdminOps-9.9.9-portable/AdminOps.portable", o).unwrap();
            z.write_all(b"marcador").unwrap();
            z.start_file("AdminOps-9.9.9-portable/lhm/LibreHardwareMonitorLib.dll", o).unwrap();
            z.write_all(b"dll").unwrap();
            z.start_file("AdminOps-9.9.9-portable/otra.txt", o).unwrap();
            z.write_all(b"no").unwrap();
            z.finish().unwrap();
        }
        let to = base.join("nueva");
        stage_portable(&zip_path, &to).unwrap();
        assert_eq!(std::fs::metadata(to.join("AdminOps.exe")).unwrap().len(), 1_200_000);
        assert!(to.join("lhm").join("LibreHardwareMonitorLib.dll").is_file());
        assert!(!to.join("AdminOps.portable").exists());
        assert!(!to.join("otra.txt").exists());
        // Sin programa dentro no sirve.
        let empty = base.join("vacio.zip");
        zip::ZipWriter::new(std::fs::File::create(&empty).unwrap()).finish().unwrap();
        assert!(stage_portable(&empty, &base.join("nada")).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

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
    fn la_version_anterior_sirve_solo_si_es_mas_antigua_y_esta_entera() {
        let meta = PreviousSetup { version: "1.5.0".into(), file: "AdminOps-1.5.0-Setup.exe".into(), sha256: "ab".into(), size: 13_000_000, saved_at: 1, data_format: DATA_FORMAT };
        let ok = usable_previous(&meta, "1.6.0", Some(13_000_000)).unwrap();
        assert_eq!(ok.version, "1.5.0");
        assert!(ok.compatible);
        // La misma o una más nueva: no hay a dónde volver.
        assert!(usable_previous(&meta, "1.5.0", Some(13_000_000)).is_none());
        assert!(usable_previous(&meta, "1.4.9", Some(13_000_000)).is_none());
        // Falta el archivo o está a medias.
        assert!(usable_previous(&meta, "1.6.0", None).is_none());
        assert!(usable_previous(&meta, "1.6.0", Some(12)).is_none());
        // Otro formato de datos: sirve, pero avisando.
        let newer_format = PreviousSetup { data_format: DATA_FORMAT + 1, ..meta };
        assert!(!usable_previous(&newer_format, "1.6.0", Some(13_000_000)).unwrap().compatible);
        assert!(usable_previous(&PreviousSetup::default(), "1.6.0", Some(0)).is_none());
    }

    #[test]
    fn el_aviso_de_actualizacion_fallida_dice_lo_que_paso() {
        let vuelta = super::failed_update_text("Version: 1.5.4\r\nHora: 27/11/2026 14:05\r\nResultado: vuelta\r\nMotivo: La versión nueva no confirmó que abría en 180 s; se volvió a la 1.5.3\r\n").unwrap();
        assert!(vuelta.contains("volvió sola a la versión anterior (27/11/2026 14:05)."));
        assert!(!vuelta.contains(".."));
        let con = super::failed_update_text("Version: 1.5.4\nResultado: vuelta\nAnterior: 1.5.3\n").unwrap();
        assert!(con.contains("volvió sola a la 1.5.3."));
        let manual = super::failed_update_text("Version: 1.5.3\nResultado: manual\nAnterior: 1.5.2\n").unwrap();
        assert!(manual.contains("Se abrió el instalador de la 1.5.2 para volver; confírmalo en su ventana."));
        let sin = super::failed_update_text("Version: 1.5.4\nResultado: sin-copia\nMotivo: no hay copia.\n").unwrap();
        assert!(sin.contains("no se pudo volver sola"));
        assert!(!sin.contains(".."));
        assert!(super::failed_update_text("basura").is_none());
    }

    #[test]
    fn el_aviso_viejo_no_se_ensena() {
        let vuelta = "Version: 1.5.4\nResultado: vuelta\n";
        // Volvió a la 1.5.3: es noticia.
        assert!(super::failed_update_still_news(vuelta, "1.5.3"));
        // Ya corre la 1.5.4 (se actualizó bien después) o una más nueva: no.
        assert!(!super::failed_update_still_news(vuelta, "1.5.4"));
        assert!(!super::failed_update_still_news(vuelta, "1.5.5"));
        // Sin copia o esperando a que se confirme el instalador de la anterior, sigue la 1.5.4: sí.
        assert!(super::failed_update_still_news("Version: 1.5.4\nResultado: sin-copia\n", "1.5.4"));
        assert!(super::failed_update_still_news("Version: 1.5.4\nResultado: manual\n", "1.5.4"));
        assert!(!super::failed_update_still_news("basura", "1.5.4"));
    }

    #[test]
    fn la_copia_solo_vale_con_su_nombre_exacto() {
        assert!(setup_name_ok("AdminOps-1.5.0-Setup.exe", "1.5.0"));
        assert!(!setup_name_ok("C:otro.exe", "1.5.0"));
        assert!(!setup_name_ok("..\\AdminOps-1.5.0-Setup.exe", "1.5.0"));
        assert!(!setup_name_ok("AdminOps-1.5.0-Setup.exe", "1.4.0"));
        assert!(!setup_name_ok("AdminOps-1.5.0\\x-Setup.exe", "1.5.0\\x"));
        assert!(!setup_name_ok("AdminOps--Setup.exe", ""));
    }

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
