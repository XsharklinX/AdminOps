//! Windows Update: por qué falla ESTA actualización. Se leen los fallos con su
//! código y su KB, el registro del servicio de componentes (CBS) y si hay una
//! instalación atascada; el código se traduce con el diccionario y se proponen
//! los arreglos en orden, de menos a más invasivo: espacio, DISM, SFC, reiniciar
//! los componentes y volver a intentarlo.

use super::parse;
use crate::troubleshoot::{finding, fix, fix_confirm, Finding};
use crate::tweaks::TweakState;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Failure {
    pub at: i64,
    pub code: i64,
    pub title: String,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct WuDeepRaw {
    pub failures: Vec<Failure>,
    /// Últimas líneas del registro CBS con errores.
    pub cbs: Vec<String>,
    pub pending_xml: bool,
    pub free_gb: f64,
}

const SCRIPT: &str = r#"
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-WindowsUpdateClient'; Id = 20; StartTime = (Get-Date).AddDays(-30) } -MaxEvents 40 -ErrorAction SilentlyContinue)
$fails = @($ev | ForEach-Object { [pscustomobject]@{ at = [int64](($_.TimeCreated.ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds); code = [int64]$_.Properties[0].Value; title = "$($_.Properties[1].Value)" } })
$cbs = @()
$log = "$env:windir\Logs\CBS\CBS.log"
if (Test-Path $log) {
  try {
    $fs = [System.IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
    $len = $fs.Length; $take = [Math]::Min($len, 3MB); [void]$fs.Seek($len - $take, 'Begin')
    $buf = New-Object byte[] $take; [void]$fs.Read($buf, 0, $take); $fs.Close()
    $text = [System.Text.Encoding]::UTF8.GetString($buf)
    $cbs = @($text -split "`n" | Where-Object { $_ -match 'Cannot repair member file|CBS_E_|STORE_CORRUPTION|0x800f081f|0x80073712|Failed to|corrupt' } | Select-Object -Last 15 | ForEach-Object { $_.Trim() })
  } catch { }
}
$v = Get-Volume -DriveLetter $env:SystemDrive.TrimEnd(':') -ErrorAction SilentlyContinue
[pscustomobject]@{ failures = $fails; cbs = $cbs; pendingXml = (Test-Path "$env:windir\WinSxS\pending.xml"); freeGb = if ($v) { $v.SizeRemaining / 1GB } else { 0 } } | ConvertTo-Json -Depth 4 -Compress
"#;

/// «2024-10 Actualización acumulativa … (KB5044284)» → «KB5044284».
pub fn kb_of(title: &str) -> Option<String> {
    let i = title.to_ascii_uppercase().find("KB")?;
    let digits: String = title[i + 2..].chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.len() >= 6).then(|| format!("KB{digits}"))
}

/// El código de error tal como lo guarda el evento (con signo) en 32 bits.
pub fn code32(raw: i64) -> u32 {
    (raw & 0xFFFF_FFFF) as u32
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub kb: String,
    pub title: String,
    pub code: u32,
    pub times: usize,
}

/// Los fallos agrupados por actualización (la misma KB que falla una y otra vez).
pub fn group(fails: &[Failure]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for f in fails {
        let kb = kb_of(&f.title).unwrap_or_else(|| f.title.chars().take(60).collect());
        match out.iter_mut().find(|g| g.kb == kb) {
            Some(g) => g.times += 1,
            None => out.push(Group { kb, title: f.title.clone(), code: code32(f.code), times: 1 }),
        }
    }
    out.sort_by_key(|g| std::cmp::Reverse(g.times));
    out
}

/// ¿Dice el registro CBS que el almacén de componentes está dañado?
pub fn store_corrupt(cbs: &[String]) -> bool {
    cbs.iter().any(|l| {
        let l = l.to_ascii_lowercase();
        l.contains("store_corruption") || l.contains("cannot repair member file") || l.contains("0x80073712") || l.contains("0x800f081f") || l.contains("corrupt")
    })
}

pub fn findings(raw: &WuDeepRaw) -> Vec<Finding> {
    let mut out = Vec::new();
    let dism = fix_confirm("wu.dism", "1. Reparar el almacén de componentes (DISM)", true, "Unos 10-20 minutos con Internet. Puedes seguir usando el equipo.");
    let sfc = fix("wu.sfc", "2. Comprobar los archivos de Windows (SFC)", true);
    let retry = fix("wu.retry", "Volver a buscar actualizaciones", false);
    for g in group(&raw.failures).iter().take(3) {
        let (name, why, todo) = match crate::errcodes::explain(g.code) {
            Some(e) => (e.name, e.why.to_string(), e.todo.to_string()),
            None => ("Error de Windows Update".to_string(), "Windows no da más detalles de este código.".to_string(), "Prueba los arreglos en orden: almacén de componentes, archivos de sistema y reinicio de Windows Update.".to_string()),
        };
        let veces = if g.times > 1 { format!(" ({} intentos)", g.times) } else { String::new() };
        let mut fixes = Vec::new();
        if raw.free_gb > 0.0 && raw.free_gb < 15.0 {
            fixes.push(fix("cleanup.temp", "0. Liberar espacio", false));
        }
        fixes.extend([dism.clone(), sfc.clone(), retry.clone()]);
        out.push(finding("bad", format!("{} falla con 0x{:08X}{veces}", g.kb, g.code), format!("{name}. {why} Qué hacer: {todo}")).fixes(fixes).page("winupdate"));
    }
    if store_corrupt(&raw.cbs) {
        let sample = raw.cbs.last().cloned().unwrap_or_default();
        out.push(finding("bad", "El almacén de componentes de Windows tiene daños", format!("El registro del servicio de componentes (CBS) lo dice. Es lo que hace fallar las actualizaciones y SFC. Última línea: «{}».", sample.chars().take(160).collect::<String>())).fixes(vec![dism.clone(), sfc.clone()]));
    }
    if raw.pending_xml {
        out.push(finding("warn", "Hay una instalación de componentes a medias", "Windows tiene operaciones pendientes del último reinicio (pending.xml). Reinicia el equipo antes de volver a intentar nada.").fixes(vec![retry]));
    }
    out
}

/// Lo que añade a la comprobación de Windows Update de «Solucionar problemas».
pub fn check() -> Vec<Finding> {
    crate::pspool::query(SCRIPT, Some(Duration::from_secs(60)), "Solucionar: Windows Update a fondo").ok().and_then(|o| parse::<WuDeepRaw>(&o).ok()).map(|r| findings(&r)).unwrap_or_default()
}

pub fn run(tweaks: &TweakState, kind: &str, _arg: &str) -> Option<Result<String, String>> {
    Some(match kind {
        "wu.dism" => tweaks.run_catalog("repair.dism"),
        "wu.sfc" => tweaks.run_catalog("repair.sfc"),
        "wu.retry" => crate::ps::exec("UsoClient.exe", &["StartInteractiveScan"]).map(|_| "Windows Update está buscando actualizaciones: míralo en Configuración → Windows Update.".into()),
        _ => return None,
    })
}

pub fn title(kind: &str) -> Option<&'static str> {
    (kind == "wu.retry").then_some("Volver a buscar actualizaciones de Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saca_la_kb() {
        assert_eq!(kb_of("2024-10 Actualización acumulativa para Windows 11 (KB5044284)").as_deref(), Some("KB5044284"));
        assert_eq!(kb_of("Definición de seguridad"), None);
    }

    #[test]
    fn agrupa_y_explica() {
        let fails = vec![
            Failure { at: 1, code: -2146498529, title: "Actualización (KB5044284)".into() },
            Failure { at: 2, code: -2146498529, title: "Actualización (KB5044284)".into() },
            Failure { at: 3, code: 0x8024_402C, title: "Otra (KB5000001)".into() },
        ];
        let g = group(&fails);
        assert_eq!(g[0].kb, "KB5044284");
        assert_eq!(g[0].times, 2);
        assert_eq!(g[0].code, 0x800F_081F);
        let f = findings(&WuDeepRaw { failures: fails, free_gb: 8.0, ..Default::default() });
        assert!(f[0].title.contains("0x800F081F") && f[0].title.contains("2 intentos"), "{}", f[0].title);
        assert!(f[0].detail.contains("Faltan archivos de origen"));
        assert_eq!(f[0].fixes[0].id, "cleanup.temp");
    }

    #[test]
    fn cbs_dañado() {
        assert!(store_corrupt(&["2024-10-01 Error CSI    Cannot repair member file [l:24]".into()]));
        assert!(!store_corrupt(&["Info CBS Session started".into()]));
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
