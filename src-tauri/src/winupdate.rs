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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn embedded_scripts_parse() {
        for (name, script) in [("HISTORY_SCRIPT", HISTORY_SCRIPT), ("PENDING_SCRIPT", PENDING_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }
}
