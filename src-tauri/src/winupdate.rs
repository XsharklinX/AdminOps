//! Windows Update: historial con los errores explicados, pausar/reanudar y
//! ocultar una actualización problemática (API COM Microsoft.Update.Session).

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

const UX: &str = r"HKLM\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings";
const PAUSE_VALUES: &[&str] = &[
    "PauseUpdatesStartTime",
    "PauseUpdatesExpiryTime",
    "PauseFeatureUpdatesStartTime",
    "PauseFeatureUpdatesEndTime",
    "PauseQualityUpdatesStartTime",
    "PauseQualityUpdatesEndTime",
];

/// Códigos de error frecuentes de Windows Update, en claro.
pub fn explain(code: &str) -> Option<&'static str> {
    let c = code.to_ascii_uppercase();
    let table: &[(&str, &str)] = &[
        ("0X80070002", "Faltan archivos de la descarga: ejecuta «Reparar Windows Update» y vuelve a intentarlo."),
        ("0X80070003", "Faltan archivos de la descarga: ejecuta «Reparar Windows Update» y vuelve a intentarlo."),
        ("0X80070005", "Acceso denegado: suele ser un antivirus o permisos dañados. Repara Windows Update."),
        ("0X8007000D", "Datos de actualización dañados: ejecuta «Reparar Windows Update»."),
        ("0X80070020", "Otro programa (a menudo el antivirus) tenía un archivo bloqueado. Reinicia y reintenta."),
        ("0X80070070", "No hay espacio suficiente en el disco del sistema."),
        ("0X80070422", "El servicio Windows Update está desactivado."),
        ("0X80070643", "Falló la instalación (a menudo .NET o Defender). Reintenta tras reiniciar; si persiste, repara .NET."),
        ("0X800705B4", "Tiempo de espera agotado: revisa la conexión y reintenta."),
        ("0X80072EE2", "Sin conexión con los servidores de Microsoft (red, proxy o firewall)."),
        ("0X80072EFD", "Sin conexión con los servidores de Microsoft (red, proxy o firewall)."),
        ("0X80072F8F", "Error de certificado: suele ser la fecha y hora del equipo. Sincroniza la hora."),
        ("0X80073712", "Faltan componentes de Windows: ejecuta DISM y después SFC."),
        ("0X800F081F", "Faltan archivos de origen: ejecuta DISM (Reparar imagen de Windows)."),
        ("0X800F0831", "Falta una actualización anterior en el sistema: ejecuta DISM y reintenta."),
        ("0X800F0922", "Espacio insuficiente en la partición reservada o VPN activa. Desconecta la VPN y reintenta."),
        ("0X800F0988", "Almacén de componentes dañado: ejecuta «Limpiar almacén de componentes» y DISM."),
        ("0X8024200B", "Error al instalar: reinicia y vuelve a buscar actualizaciones."),
        ("0X80240034", "La descarga falló: ejecuta «Reparar Windows Update»."),
        ("0X8024402C", "No se pudo conectar: revisa el proxy o el DNS."),
        ("0X8024401C", "Tiempo de espera con el servidor de actualizaciones (WSUS o red)."),
        ("0X80244022", "El servidor de actualizaciones no responde; reintenta más tarde."),
        ("0X8024A105", "Error del agente de actualización: reinicia y reintenta."),
        ("0X80242016", "La actualización quedó a medias: reinicia el equipo para terminarla."),
        ("0XC1900101", "Fallo de un driver durante la actualización: actualiza drivers (gráfica, almacenamiento) y desconecta USB."),
        ("0XC1900208", "Un programa incompatible bloquea la actualización (a menudo el antivirus)."),
        ("0X8007001F", "Un dispositivo dejó de responder durante la instalación: revisa drivers."),
    ];
    table.iter().find(|(k, _)| c.starts_with(k)).map(|(_, v)| *v)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHistory {
    title: String,
    date: Option<String>,
    result: i32,
    hresult: i64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    title: String,
    date: Option<String>,
    /// ok | partial | failed | aborted | progress
    result: &'static str,
    code: Option<String>,
    explanation: Option<&'static str>,
}

const HISTORY_SCRIPT: &str = r#"
$s = New-Object -ComObject Microsoft.Update.Session
$h = $s.CreateUpdateSearcher()
$n = $h.GetTotalHistoryCount()
$out = @()
if ($n -gt 0) {
  $out = @($h.QueryHistory(0, [Math]::Min($n, 60)) | Where-Object { $_.Title } | ForEach-Object {
    [pscustomobject]@{ title = $_.Title; date = if ($_.Date) { $_.Date.ToUniversalTime().ToString('o') } else { $null }; result = [int]$_.ResultCode; hresult = [int64]$_.HResult }
  })
}
ConvertTo-Json -InputObject $out -Compress
"#;

pub fn history() -> Result<Vec<HistoryEntry>, String> {
    let out = crate::ps::powershell_opts(HISTORY_SCRIPT, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
    let raw: Vec<RawHistory> = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    Ok(raw
        .into_iter()
        .map(|r| {
            let code = (r.hresult != 0).then(|| format!("0x{:08X}", r.hresult as u32));
            HistoryEntry {
                explanation: code.as_deref().and_then(explain),
                result: match r.result {
                    1 => "progress",
                    2 => "ok",
                    3 => "partial",
                    4 => "failed",
                    _ => "aborted",
                },
                code,
                title: r.title,
                date: r.date,
            }
        })
        .collect())
}

#[tauri::command(async)]
pub fn update_history() -> Result<Vec<HistoryEntry>, String> {
    history()
}

// ---------- Pausa ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseState {
    /// ISO 8601 (UTC) hasta cuándo están pausadas, si lo están.
    paused_until: Option<String>,
}

#[tauri::command]
pub fn update_pause_state() -> PauseState {
    let until = crate::tweaks::registry::read_string(UX, "PauseUpdatesExpiryTime").filter(|s| !s.is_empty());
    let active = until.as_deref().and_then(|u| chrono::DateTime::parse_from_rfc3339(u).ok()).is_some_and(|d| d > chrono::Utc::now());
    PauseState { paused_until: until.filter(|_| active) }
}

/// Pausa las actualizaciones `days` días (1-35, como Configuración) o las reanuda con 0.
#[tauri::command(async)]
pub fn update_pause(days: u32, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if days > 35 {
        return Err("Windows permite pausar como máximo 35 días.".into());
    }
    let result = if days == 0 {
        PAUSE_VALUES.iter().try_for_each(|v| crate::tweaks::registry::delete(UX, v))
    } else {
        let now = chrono::Utc::now();
        let fmt = |d: chrono::DateTime<chrono::Utc>| d.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let (start, end) = (fmt(now), fmt(now + chrono::Duration::days(days as i64)));
        PAUSE_VALUES.iter().try_for_each(|v| {
            let value = if v.contains("Start") { &start } else { &end };
            crate::tweaks::registry::write(UX, v, crate::tweaks::model::RegKind::String, &crate::tweaks::model::RegData::Str(value.clone()))
        })
    };
    let title = if days == 0 { "Windows Update: reanudar actualizaciones".to_string() } else { format!("Windows Update: pausar {days} días") };
    tweaks.record(Op::Run, &title, &result);
    result
}

// ---------- Pendientes y ocultas ----------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PendingUpdate {
    id: String,
    title: String,
    kb: Option<String>,
    size: u64,
    hidden: bool,
    driver: bool,
}

const PENDING_SCRIPT: &str = r#"
$s = New-Object -ComObject Microsoft.Update.Session
$searcher = $s.CreateUpdateSearcher()
$r = $searcher.Search('IsInstalled=0')
$out = @($r.Updates | ForEach-Object {
  [pscustomobject]@{
    id = $_.Identity.UpdateID; title = $_.Title
    kb = if ($_.KBArticleIDs.Count) { 'KB' + $_.KBArticleIDs.Item(0) } else { $null }
    size = [uint64]$_.MaxDownloadSize; hidden = [bool]$_.IsHidden; driver = [int]$_.Type -eq 2
  }
})
ConvertTo-Json -InputObject $out -Compress
"#;

/// Busca en los servidores de Microsoft (puede tardar uno o dos minutos).
#[tauri::command(async)]
pub fn pending_updates(app: tauri::AppHandle) -> Result<Vec<PendingUpdate>, String> {
    let task = Task::new(&app, "wu-search").named("Buscar actualizaciones");
    task.step("Buscando actualizaciones en los servidores de Microsoft…");
    let out = crate::ps::powershell_opts(PENDING_SCRIPT, task.opts(Some(Duration::from_secs(300))))?;
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))
}

/// Oculta (o vuelve a mostrar) una actualización para que Windows no la instale.
#[tauri::command(async)]
pub fn set_update_hidden(id: String, title: String, hidden: bool, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let valid = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if !valid {
        return Err("Identificador de actualización no válido.".into());
    }
    let script = format!(
        "$s = New-Object -ComObject Microsoft.Update.Session\n\
         $r = $s.CreateUpdateSearcher().Search(\"UpdateID='{id}'\")\n\
         if ($r.Updates.Count -eq 0) {{ throw 'Esa actualización ya no está disponible.' }}\n\
         $r.Updates.Item(0).IsHidden = ${hidden}\n'ok'"
    );
    let result = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(180)), task: None }).map(|_| ());
    let what = if hidden { "ocultar" } else { "volver a mostrar" };
    tweaks.record(Op::Run, &format!("Windows Update: {what} «{}»", title.chars().take(120).collect::<String>()), &result);
    result
}

// ---------- Instalar con progreso por actualización ----------

/// Una línea de progreso de una actualización (sale del script como JSON y se reenvía a la
/// pantalla como evento `wu-progress`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub id: String,
    /// downloading | installing | done | reboot | failed
    pub phase: String,
    pub percent: Option<u32>,
    /// Bytes bajados hasta ahora y tamaño total de la descarga.
    pub bytes: Option<u64>,
    pub total: Option<u64>,
    /// Si falló: el código (0x…) y su explicación.
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Lee una línea del script. Lo que no es una línea de progreso válida se ignora.
pub fn parse_progress_line(line: &str) -> Option<UpdateProgress> {
    let mut p: UpdateProgress = serde_json::from_str(line.trim()).ok()?;
    if !matches!(p.phase.as_str(), "downloading" | "installing" | "done" | "reboot" | "failed") || !valid_guid(&p.id) {
        return None;
    }
    p.percent = p.percent.map(|n| n.min(100));
    if let Some(c) = p.code.as_deref() {
        p.explanation = explain(c).map(str::to_string);
    }
    Some(p)
}

fn valid_guid(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

const INSTALL_SCRIPT: &str = r#"
function Emit($id, $phase, $pct, $bytes, $total, $hr) {
  $o = [ordered]@{ id = $id; phase = $phase }
  if ($null -ne $pct) { $o.percent = [int]$pct }
  if ($null -ne $bytes) { $o.bytes = [uint64]$bytes }
  if ($null -ne $total) { $o.total = [uint64]$total }
  if ($null -ne $hr -and $hr -ne 0) { $o.code = '0x{0:X8}' -f ([int64]$hr -band 0xFFFFFFFFL) }
  [Console]::Out.WriteLine((ConvertTo-Json -InputObject $o -Compress))
  [Console]::Out.Flush()
}
# Windows Update no acepta «sin aviso» al empezar una descarga o instalación: hace falta un objeto que reciba
# los avisos (aquí no hace nada; el avance se lee del propio trabajo cada medio segundo)
if (-not ('AdminOpsWu.Quiet' -as [type])) {
  Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
namespace AdminOpsWu {
  [ComImport, Guid("8C3F1CDD-6173-4591-AEBD-A56A53CA77C1"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IDownloadProgressChangedCallback { void Invoke([In, MarshalAs(UnmanagedType.Interface)] object job, [In, MarshalAs(UnmanagedType.Interface)] object args); }
  [ComImport, Guid("77254866-9F5B-4C8E-B9E2-C77A8530D64B"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IDownloadCompletedCallback { void Invoke([In, MarshalAs(UnmanagedType.Interface)] object job, [In, MarshalAs(UnmanagedType.Interface)] object args); }
  [ComImport, Guid("E01402D5-F8DA-43BA-A012-38894BD048F6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IInstallationProgressChangedCallback { void Invoke([In, MarshalAs(UnmanagedType.Interface)] object job, [In, MarshalAs(UnmanagedType.Interface)] object args); }
  [ComImport, Guid("45F4F6F3-D602-4F98-9A8A-3EFA152AD2D3"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IInstallationCompletedCallback { void Invoke([In, MarshalAs(UnmanagedType.Interface)] object job, [In, MarshalAs(UnmanagedType.Interface)] object args); }
  [ComVisible(true)]
  public class Quiet : IDownloadProgressChangedCallback, IDownloadCompletedCallback, IInstallationProgressChangedCallback, IInstallationCompletedCallback {
    public void Invoke(object job, object args) { }
  }
}
'@
}
$cb = New-Object AdminOpsWu.Quiet
$s = New-Object -ComObject Microsoft.Update.Session
$found = $s.CreateUpdateSearcher().Search('IsInstalled=0 and IsHidden=0')
$coll = New-Object -ComObject Microsoft.Update.UpdateColl
foreach ($u in $found.Updates) {
  if ($ids -contains $u.Identity.UpdateID) {
    if (-not $u.EULAAccepted) { $u.AcceptEula() }
    [void]$coll.Add($u)
  }
}
if ($coll.Count -eq 0) { throw 'Esas actualizaciones ya no están disponibles. Vuelve a buscar.' }

# Descarga: una lectura del progreso cada medio segundo, con el avance de cada actualización
$dl = $s.CreateUpdateDownloader()
$dl.Updates = $coll
$job = $dl.BeginDownload($cb, $cb, $null)
$last = @{}
do {
  Start-Sleep -Milliseconds 500
  $p = $job.GetProgress()
  for ($i = 0; $i -le [Math]::Min($p.CurrentUpdateIndex, $coll.Count - 1); $i++) {
    $u = $coll.Item($i); $id = $u.Identity.UpdateID
    if ($i -lt $p.CurrentUpdateIndex) { $pct = 100; $b = [uint64]$u.MaxDownloadSize } else { $pct = [int]$p.CurrentUpdatePercentComplete; $b = [uint64]$p.CurrentUpdateBytesDownloaded }
    if ($last[$id] -ne $pct) { $last[$id] = $pct; Emit $id 'downloading' $pct $b $u.MaxDownloadSize 0 }
  }
} while (-not $job.IsCompleted)
$dres = $dl.EndDownload($job)

# Instalación de las que se bajaron bien
$icoll = New-Object -ComObject Microsoft.Update.UpdateColl
for ($i = 0; $i -lt $coll.Count; $i++) {
  $u = $coll.Item($i); $id = $u.Identity.UpdateID
  if ([int]$dres.GetUpdateResult($i).ResultCode -eq 2 -or $u.IsDownloaded) { Emit $id 'downloading' 100 $u.MaxDownloadSize $u.MaxDownloadSize 0; [void]$icoll.Add($u) }
  else { Emit $id 'failed' $null $null $null ([int64]$dres.GetUpdateResult($i).HResult) }
}
if ($icoll.Count -gt 0) {
  $inst = $s.CreateUpdateInstaller()
  $inst.Updates = $icoll
  $inst.AllowSourcePrompts = $false
  $ij = $inst.BeginInstall($cb, $cb, $null)
  $last = @{}
  do {
    Start-Sleep -Milliseconds 500
    $p = $ij.GetProgress()
    for ($i = 0; $i -le [Math]::Min($p.CurrentUpdateIndex, $icoll.Count - 1); $i++) {
      $id = $icoll.Item($i).Identity.UpdateID
      $pct = if ($i -lt $p.CurrentUpdateIndex) { 100 } else { [int]$p.CurrentUpdatePercentComplete }
      if ($last[$id] -ne $pct) { $last[$id] = $pct; Emit $id 'installing' $pct $null $null 0 }
    }
  } while (-not $ij.IsCompleted)
  $ires = $inst.EndInstall($ij)
  for ($i = 0; $i -lt $icoll.Count; $i++) {
    $id = $icoll.Item($i).Identity.UpdateID
    $r = $ires.GetUpdateResult($i)
    if ([int]$r.ResultCode -eq 2 -or [int]$r.ResultCode -eq 3) {
      if ($r.RebootRequired) { Emit $id 'reboot' 100 $null $null 0 } else { Emit $id 'done' 100 $null $null 0 }
    } else { Emit $id 'failed' $null $null $null ([int64]$r.HResult) }
  }
}
"#;

/// ¿Hay una instalación en marcha? Lo sabe el programa, no la pantalla: si se sale de la pestaña y se
/// vuelve, la interfaz lo pregunta aquí en vez de dar por parada una tarea que sigue.
static INSTALLING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Marca la instalación como en marcha mientras viva; al soltarse (también si hay un fallo) se quita.
struct InstallingGuard;

impl InstallingGuard {
    fn take() -> Option<Self> {
        INSTALLING.compare_exchange(false, true, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).ok().map(|_| InstallingGuard)
    }
}

impl Drop for InstallingGuard {
    fn drop(&mut self) {
        INSTALLING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[tauri::command]
pub fn update_install_running() -> bool {
    INSTALLING.load(std::sync::atomic::Ordering::SeqCst)
}

/// Descarga e instala las actualizaciones elegidas y va diciendo, por cada una, cómo va
/// (evento `wu-progress`). Devuelve el último estado de cada una.
#[tauri::command(async)]
pub fn update_install(app: tauri::AppHandle, ids: Vec<String>, tweaks: State<'_, TweakState>) -> Result<Vec<UpdateProgress>, String> {
    use tauri::Emitter;
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if ids.is_empty() || ids.len() > 100 || !ids.iter().all(|i| valid_guid(i)) {
        return Err("Identificador de actualización no válido.".into());
    }
    let Some(_running) = InstallingGuard::take() else {
        return Err("Ya hay una instalación de actualizaciones en marcha: espera a que termine.".into());
    };
    let task = Task::new(&app, "wu-install").named("Instalar actualizaciones de Windows");
    task.step(format!("Instalando {} actualizaciones…", ids.len()));
    let list = ids.iter().map(|i| format!("'{i}'")).collect::<Vec<_>>().join(",");
    let script = format!("$ids = @({list})\n{INSTALL_SCRIPT}");
    let mut last: Vec<UpdateProgress> = Vec::new();
    let r = crate::ps::powershell_stream(&script, task.opts(Some(Duration::from_secs(2 * 60 * 60))), &mut |line| {
        if let Some(p) = parse_progress_line(line) {
            let _ = app.emit("wu-progress", &p);
            match last.iter_mut().find(|x| x.id == p.id) {
                Some(x) => *x = p,
                None => last.push(p),
            }
        }
    });
    let result = match r {
        Ok(s) if s.code == Some(0) => Ok(()),
        Ok(s) => Err(crate::ps::clean_error(&s.stderr)),
        Err(e) => Err(e),
    };
    let bad = last.iter().filter(|p| p.phase == "failed").count();
    let journal = match &result {
        Ok(()) if bad > 0 => Err(format!("{bad} actualizaciones fallaron")),
        other => other.clone(),
    };
    tweaks.record(Op::Run, &format!("Windows Update: instalar {} actualizaciones", ids.len()), &journal);
    result.map(|()| last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_from_the_script() {
        let id = "0a1b2c3d-0000-4000-8000-0123456789ab";
        let p = parse_progress_line(&format!(r#"{{"id":"{id}","phase":"downloading","percent":81,"bytes":54525952,"total":67108864}}"#)).unwrap();
        assert_eq!((p.phase.as_str(), p.percent, p.bytes, p.total), ("downloading", Some(81), Some(54525952), Some(67108864)));
        let f = parse_progress_line(&format!(r#"{{"id":"{id}","phase":"failed","code":"0x80070070"}}"#)).unwrap();
        assert!(f.explanation.unwrap().contains("espacio"));
        let r = parse_progress_line(&format!(r#"{{"id":"{id}","phase":"reboot","percent":250}}"#)).unwrap();
        assert_eq!(r.percent, Some(100));
        // Lo que no es progreso, o no es de una actualización, se ignora.
        assert_eq!(parse_progress_line("Descargando…"), None);
        assert_eq!(parse_progress_line(r#"{"id":"no-es-un-guid","phase":"done"}"#), None);
        assert_eq!(parse_progress_line(&format!(r#"{{"id":"{id}","phase":"otra"}}"#)), None);
    }

    #[test]
    fn only_one_install_at_a_time() {
        let first = InstallingGuard::take().expect("libre");
        assert!(update_install_running());
        assert!(InstallingGuard::take().is_none());
        drop(first);
        assert!(!update_install_running());
        assert!(InstallingGuard::take().is_some());
    }

    #[test]
    fn explains_common_codes() {
        assert!(explain("0x80070070").unwrap().contains("espacio"));
        assert!(explain("0x800f081f").unwrap().contains("DISM"));
        assert!(explain("0x12345678").is_none());
    }

    /// Solo lectura: historial real.
    /// Equipo real (depende de su estado): `cargo test real_history -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_history() {
        let h = history().unwrap();
        println!("{} entradas; fallidas: {}", h.len(), h.iter().filter(|e| e.result == "failed").count());
    }

    #[test]
    fn install_script_formats_negative_hresult() {
        assert!(!INSTALL_SCRIPT.contains("[uint32]("), "[uint32](…) lanza con un HResult negativo");
        // Se ejecuta de verdad solo la función Emit con un HResult negativo.
        let start = INSTALL_SCRIPT.find("function Emit").unwrap();
        let end = start + INSTALL_SCRIPT[start..].find("
}
").unwrap() + 3;
        let script = format!("{}
Emit '0a1b2c3d-0000-4000-8000-0123456789ab' 'failed' $null $null $null ([int64]-2145124329)", &INSTALL_SCRIPT[start..end]);
        let mut lines = Vec::new();
        let r = crate::ps::powershell_stream(&script, crate::ps::Opts::default(), &mut |l| lines.push(l.to_string())).unwrap();
        assert_eq!(r.code, Some(0), "stderr: {}", r.stderr);
        let p = lines.iter().find_map(|l| parse_progress_line(l)).unwrap_or_else(|| panic!("sin línea de progreso: {lines:?} {}", r.stderr));
        assert_eq!((p.phase.as_str(), p.code.as_deref()), ("failed", Some("0x80240017")));
    }

    #[test]
    fn embedded_scripts_parse() {
        let install = format!("$ids = @('0a1b2c3d-0000-4000-8000-0123456789ab')\n{INSTALL_SCRIPT}");
        let errors = crate::ps::parse_errors(&install);
        assert!(errors.is_empty(), "INSTALL_SCRIPT: {errors}");
        for (name, script) in [("HISTORY_SCRIPT", HISTORY_SCRIPT), ("PENDING_SCRIPT", PENDING_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }
}
