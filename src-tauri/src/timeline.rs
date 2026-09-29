//! Línea de tiempo del equipo: en un solo sitio y por orden, lo que hizo AdminOps,
//! los avisos de Windows, los análisis, y lo que pasó en Windows (arranques,
//! apagados bruscos, actualizaciones, drivers y programas instalados).
//! Responde a «¿qué cambió desde que funcionaba?».

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Segundos Unix.
    pub time: u64,
    /// change | alert | scan | windows
    pub kind: &'static str,
    /// ok | info | warn | bad
    pub level: String,
    pub title: String,
    pub detail: String,
    pub page: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RawEvent {
    provider: String,
    id: u32,
    time: String,
    #[serde(default)]
    props: Vec<String>,
}

const SCRIPT: &str = r#"
$since = (Get-Date).AddDays(-$days)
$f = @(
  @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-Kernel-General'; Id = 12, 13; StartTime = $since },
  @{ LogName = 'System'; ProviderName = 'EventLog'; Id = 6008; StartTime = $since },
  @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-WindowsUpdateClient'; Id = 19, 20; StartTime = $since },
  @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-UserPnp'; Id = 20001; StartTime = $since },
  @{ LogName = 'Application'; ProviderName = 'MsiInstaller'; Id = 1033, 1034; StartTime = $since }
)
$r = @(foreach ($x in $f) { try { Get-WinEvent -FilterHashtable $x -MaxEvents 150 -ErrorAction Stop | ForEach-Object {
  [pscustomobject]@{ provider = "$($_.ProviderName)"; id = [int]$_.Id; time = $_.TimeCreated.ToUniversalTime().ToString('o'); props = @($_.Properties | Select-Object -First 6 | ForEach-Object { "$($_.Value)" }) }
} } catch { } })
ConvertTo-Json -InputObject $r -Depth 3 -Compress
"#;

fn prop(e: &RawEvent, i: usize) -> String {
    e.props.get(i).map(|s| s.trim().chars().take(140).collect()).unwrap_or_default()
}

fn windows_event(e: &RawEvent) -> Option<Event> {
    let time = chrono::DateTime::parse_from_rfc3339(&e.time).ok()?.timestamp() as u64;
    let ev = |level: &str, title: String, detail: String, page: Option<&str>| Event { time, kind: "windows", level: level.into(), title, detail, page: page.map(String::from) };
    Some(match (e.provider.as_str(), e.id) {
        ("Microsoft-Windows-Kernel-General", 12) => ev("info", "Windows arrancó".into(), String::new(), None),
        ("Microsoft-Windows-Kernel-General", 13) => ev("info", "Windows se apagó".into(), String::new(), None),
        ("EventLog", 6008) => ev("warn", "Apagado inesperado".into(), "El equipo se apagó sin cerrar Windows (corte de luz, cuelgue o botón).".into(), Some("diagnostics")),
        ("Microsoft-Windows-WindowsUpdateClient", 19) => ev("ok", "Actualización instalada".into(), prop(e, 0), Some("winupdate")),
        ("Microsoft-Windows-WindowsUpdateClient", 20) => ev("bad", "Actualización fallida".into(), [prop(e, 1), prop(e, 0)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "), Some("winupdate")),
        ("Microsoft-Windows-UserPnp", 20001) => {
            let name = prop(e, 2);
            if name.is_empty() {
                return None;
            }
            ev("info", "Driver instalado".into(), [name, prop(e, 3)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "), None)
        }
        ("MsiInstaller", 1033) => ev("info", "Programa instalado".into(), [prop(e, 0), prop(e, 1)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" "), Some("uninstall")),
        ("MsiInstaller", 1034) => ev("info", "Programa desinstalado".into(), prop(e, 0), None),
        _ => return None,
    })
}

fn journal_event(e: &crate::tweaks::journal::Entry) -> Event {
    let verb = match e.op {
        Op::Apply => "Aplicado",
        Op::Revert => "Deshecho",
        Op::Run => "Ejecutado",
        Op::RestorePoint => "Punto de restauración",
    };
    Event {
        time: e.timestamp,
        kind: "change",
        level: if e.ok { "ok" } else { "bad" }.into(),
        title: e.title.clone(),
        detail: [verb.to_string(), e.message.clone().unwrap_or_default()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "),
        page: Some("history".into()),
    }
}

#[tauri::command(async)]
pub fn machine_timeline(app: tauri::AppHandle, tweaks: State<'_, TweakState>, days: u32) -> Result<Vec<Event>, String> {
    let days = days.clamp(1, 365);
    let since = chrono::Utc::now().timestamp() as u64 - u64::from(days) * 86_400;
    let mut out: Vec<Event> = tweaks.journal_since(since).iter().map(journal_event).collect();

    for a in crate::winwatch::list_windows_alerts(app.clone()).into_iter().filter(|a| a.time >= since) {
        let times = if a.count > 1 { format!(" (x{})", a.count) } else { String::new() };
        out.push(Event { time: a.time, kind: "alert", level: a.level, title: format!("{}{times}", a.title), detail: a.detail, page: a.page });
    }

    let dir = crate::diagnostics::snapshots_dir(&app);
    for (ts, _) in crate::diagnostics::snapshot_files(&dir).into_iter().filter(|(ts, _)| *ts >= since) {
        if let Some(d) = crate::diagnostics::load_snapshot(&app, ts) {
            let count = |s| d.findings.iter().filter(|f| f.severity == s).count();
            let (bad, warn) = (count(crate::diagnostics::Severity::Bad), count(crate::diagnostics::Severity::Warn));
            out.push(Event {
                time: ts,
                kind: "scan",
                level: if bad > 0 { "bad" } else if warn > 0 { "warn" } else { "ok" }.into(),
                title: "Diagnóstico".into(),
                detail: match (bad, warn) {
                    (0, 0) => "Sin problemas".into(),
                    _ => format!("{bad} problema(s), {warn} aviso(s)"),
                },
                page: Some("diagnostics".into()),
            });
        }
    }

    // Lo de Windows es opcional: si falla (PowerShell bloqueado), el resto se muestra igual.
    let script = format!("$days = {days}\n{SCRIPT}");
    match crate::pspool::query(&script, Some(Duration::from_secs(60)), "Línea de tiempo") {
        Ok(raw) => {
            let events: Vec<RawEvent> = serde_json::from_str(raw.trim()).unwrap_or_default();
            out.extend(events.iter().filter_map(windows_event));
        }
        Err(e) => log::debug!("Línea de tiempo: {e}"),
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.time));
    out.truncate(1500);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(&format!("$days = 30\n{SCRIPT}"));
        assert!(e.is_empty(), "{e}");
    }

    #[test]
    fn translates_windows_events() {
        let ev = |p: &str, id, props: &[&str]| RawEvent { provider: p.into(), id, time: "2026-09-27T10:00:00Z".into(), props: props.iter().map(|s| s.to_string()).collect() };
        assert_eq!(windows_event(&ev("EventLog", 6008, &[])).unwrap().level, "warn");
        let msi = windows_event(&ev("MsiInstaller", 1033, &["7-Zip", "23.01"])).unwrap();
        assert_eq!(msi.detail, "7-Zip 23.01");
        assert!(windows_event(&ev("Microsoft-Windows-UserPnp", 20001, &[])).is_none());
        assert!(windows_event(&ev("Otro", 1, &[])).is_none());
    }
}
