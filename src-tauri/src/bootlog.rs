//! Arranques y cuelgues de los últimos 60 días, del registro de eventos de
//! Windows: cuándo arrancó y se apagó el equipo, los apagones (se reinició sin
//! apagarse bien), los pantallazos azules con su código, y cuánto tardó cada
//! arranque.
//!
//! Lo de los arranques, apagones y pantallazos se lee sin administrador. La
//! duración de cada arranque está en un registro que Windows solo deja leer
//! como administrador: sin él, se dice así en la pantalla.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const SCRIPT: &str = r#"
$since = (Get-Date).AddDays(-60)
$unix = { param($d) [int64]([DateTimeOffset]$d).ToUnixTimeSeconds() }
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; Id = 12, 13, 41, 1001; StartTime = $since } -ErrorAction SilentlyContinue | ForEach-Object {
  $kind = $null; $code = $null
  if ($_.Id -eq 12 -and $_.ProviderName -eq 'Microsoft-Windows-Kernel-General') { $kind = 'boot' }
  elseif ($_.Id -eq 13 -and $_.ProviderName -eq 'Microsoft-Windows-Kernel-General') { $kind = 'shutdown' }
  elseif ($_.Id -eq 41 -and $_.ProviderName -eq 'Microsoft-Windows-Kernel-Power') {
    $kind = 'unexpected'
    $bc = [int64]$_.Properties[0].Value
    if ($bc -ne 0) { $code = '0x{0:X8}' -f $bc }
  }
  elseif ($_.Id -eq 1001 -and $_.ProviderName -like '*WER-SystemErrorReporting*') {
    $kind = 'bsod'
    $m = [regex]::Match([string]$_.Properties[0].Value, '0x[0-9a-fA-F]+')
    if ($m.Success) { $code = $m.Value }
  }
  if ($kind) { [pscustomobject]@{ t = (& $unix $_.TimeCreated); kind = $kind; code = $code } }
})
$boots = $null
try {
  $boots = @(Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-Diagnostics-Performance/Operational'; Id = 100; StartTime = $since } -ErrorAction Stop | ForEach-Object {
    $d = @{}
    foreach ($n in ([xml]$_.ToXml()).Event.EventData.Data) { $d[$n.Name] = $n.'#text' }
    [pscustomobject]@{ t = (& $unix $_.TimeCreated); ms = [int64]$d['BootTime']; mainMs = [int64]$d['MainPathBootTime'] }
  })
} catch {
  # Sin arranques apuntados no es lo mismo que sin permiso para leerlos.
  if ("$($_.FullyQualifiedErrorId)" -like 'NoMatchingEventsFound*') { $boots = @() } else { $boots = $null }
}
$dumps = @()
try {
  $dumps = @(Get-ChildItem (Join-Path $env:SystemRoot 'Minidump') -Filter *.dmp -ErrorAction Stop | ForEach-Object { [pscustomobject]@{ name = $_.Name; t = (& $unix $_.LastWriteTime); size = $_.Length } })
} catch {}
ConvertTo-Json -InputObject ([pscustomobject]@{ events = $ev; boots = $boots; dumps = $dumps }) -Depth 4 -Compress
"#;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootEvent {
    pub t: u64,
    /// "boot", "shutdown", "unexpected" (apagón o cuelgue) o "bsod".
    pub kind: String,
    /// Código del pantallazo (0x0000009F…), si lo hubo.
    pub code: Option<String>,
    /// Nombre del código (DRIVER_POWER_STATE_FAILURE…), si se conoce.
    #[serde(default)]
    pub name: Option<String>,
    /// Qué suele haber detrás y qué mirar.
    #[serde(default)]
    pub hint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootTime {
    pub t: u64,
    /// Hasta el escritorio usable, en ms.
    pub ms: u64,
    /// Hasta el escritorio (sin lo que arranca después), en ms.
    pub main_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dump {
    pub name: String,
    pub t: u64,
    pub size: u64,
    /// Lo que dice el volcado: el código y el driver probable (los 10 más recientes).
    #[serde(default)]
    pub analysis: Option<crate::diagnostics::minidump::DumpAnalysis>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootLog {
    pub events: Vec<BootEvent>,
    /// None: hace falta administrador para leer cuánto tardó cada arranque.
    pub boots: Option<Vec<BootTime>>,
    pub dumps: Vec<Dump>,
}

#[tauri::command(async)]
pub fn boot_history() -> Result<BootLog, String> {
    let out = crate::ps::powershell_opts(SCRIPT, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
    let mut log: BootLog = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada al leer los arranques: {e}"))?;
    // Sin administrador Windows no da error al leer ese registro: dice que no
    // hay nada. Vacío y sin permisos es «hace falta administrador», no «ninguno».
    if !crate::elevation::is_elevated() && log.boots.as_ref().is_some_and(|b| b.is_empty()) {
        log.boots = None;
    }
    log.events.sort_by_key(|e| std::cmp::Reverse(e.t));
    if let Some(b) = &mut log.boots {
        b.sort_by_key(|b| b.t);
    }
    log.dumps.sort_by_key(|d| std::cmp::Reverse(d.t));
    // Lo mismo que el diagnóstico: una sola tabla de códigos y el análisis de los volcados.
    for e in &mut log.events {
        if let Some(code) = &e.code {
            let (name, hint) = crate::diagnostics::collect::bugcheck_info(code);
            e.name = name.map(str::to_string);
            e.hint = hint.map(str::to_string);
        }
    }
    let dir = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("Minidump");
    for d in log.dumps.iter_mut().take(10) {
        d.analysis = crate::diagnostics::minidump::analyze_file(&dir.join(&d.name));
    }
    Ok(log)
}

#[cfg(test)]
mod tests {
    #[test]
    fn script_parses() {
        let errors = crate::ps::parse_errors(super::SCRIPT);
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn reads_the_shape() {
        let json = r#"{"events":[{"t":5,"kind":"bsod","code":"0x0000009F"},{"t":9,"kind":"boot","code":null}],"boots":null,"dumps":[]}"#;
        let log: super::BootLog = serde_json::from_str(json).unwrap();
        assert_eq!(log.events.len(), 2);
        assert!(log.boots.is_none());
    }

    /// Equipo real: `cargo test real_boot_history -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_boot_history() {
        let t = std::time::Instant::now();
        let log = super::boot_history().unwrap();
        println!("{:?}: {} eventos, arranques {:?}, {} volcados", t.elapsed(), log.events.len(), log.boots.as_ref().map(|b| b.len()), log.dumps.len());
    }
}
