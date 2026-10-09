//! La batería a lo largo del tiempo: Windows apunta cada semana cuánto carga la
//! batería (su capacidad) frente a la de fábrica. Con eso se ve si se gasta
//! rápido y, a este ritmo, cuándo quedará a la mitad. Sale del informe de
//! batería de Windows (`powercfg /batteryreport`): no se instala nada.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Una semana del historial de Windows.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryPoint {
    /// Día en que empieza el tramo (AAAA-MM-DD).
    pub day: String,
    /// mWh que carga a tope.
    pub full: u64,
    /// mWh de fábrica.
    pub design: u64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatteryHistory {
    pub name: String,
    pub design: u64,
    pub full: u64,
    pub cycles: Option<u32>,
    /// Un punto por tramo, del más antiguo al más reciente.
    pub points: Vec<BatteryPoint>,
    /// Puntos de capacidad (sobre 100) que pierde al mes, según el último año.
    pub loss_per_month: Option<f64>,
    /// Meses que faltan, a este ritmo, para quedar a la mitad de la de fábrica.
    pub months_to_half: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    name: String,
    design: u64,
    full: u64,
    cycles: Option<u32>,
    #[serde(default)]
    history: Vec<BatteryPoint>,
}

const SCRIPT: &str = r#"
if (-not (Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue)) { 'null'; return }
$f = Join-Path $env:TEMP ("adminops-battery-history-{0}.xml" -f $PID)
powercfg.exe /batteryreport /xml /output $f | Out-Null
if (-not (Test-Path $f)) { 'null'; return }
[xml]$x = Get-Content -LiteralPath $f -Raw
Remove-Item -LiteralPath $f -ErrorAction SilentlyContinue
$b = @($x.BatteryReport.Batteries.Battery)[0]
if (-not $b) { 'null'; return }
$h = @($x.BatteryReport.History.HistoryEntry) | Where-Object { $_ } | ForEach-Object {
  [pscustomobject]@{ day = "$($_.LocalStartDate)"; full = [uint64]("0$($_.FullChargeCapacity)" -replace '\D', ''); design = [uint64]("0$($_.DesignCapacity)" -replace '\D', '') }
}
ConvertTo-Json -Depth 3 -Compress -InputObject ([pscustomobject]@{
  name = "$($b.Id)".Trim(); design = [uint64]$b.DesignCapacity; full = [uint64]$b.FullChargeCapacity
  cycles = if ("$($b.CycleCount)" -match '^\d+$') { [uint32]$b.CycleCount } else { $null }
  history = @($h)
})
"#;

/// Días desde 1970 de una fecha «AAAA-MM-DD» (sin depender de la zona horaria).
fn days(day: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(day.get(..10)?, "%Y-%m-%d").ok()?;
    Some(d.signed_duration_since(chrono::NaiveDate::from_ymd_opt(1970, 1, 1)?).num_days())
}

/// Deja un punto por tramo válido, ordenados, con el día recortado. Los tramos
/// sin dato (capacidad 0, o más que la de fábrica por un error de lectura) fuera.
fn clean(mut points: Vec<BatteryPoint>, design_now: u64) -> Vec<BatteryPoint> {
    for p in &mut points {
        p.day = p.day.chars().take(10).collect();
        if p.design == 0 {
            p.design = design_now;
        }
    }
    points.retain(|p| p.full > 0 && p.design > 0 && p.full <= p.design * 11 / 10 && days(&p.day).is_some());
    points.sort_by(|a, b| a.day.cmp(&b.day));
    points.dedup_by(|a, b| a.day == b.day);
    points
}

/// Cuánto baja al mes (recta por mínimos cuadrados sobre el último año) y
/// cuándo llegaría al 50 %. Sin datos suficientes, o si no baja, nada.
fn trend(points: &[BatteryPoint]) -> (Option<f64>, Option<f64>) {
    let Some(last) = points.last().and_then(|p| days(&p.day)) else { return (None, None) };
    let recent: Vec<(f64, f64)> = points
        .iter()
        .filter_map(|p| Some((days(&p.day)?, p.full as f64 * 100.0 / p.design as f64)))
        .filter(|(d, _)| last - d <= 365)
        .map(|(d, h)| (d as f64, h))
        .collect();
    // Menos de dos meses de datos no dicen nada de un desgaste que va por años.
    if recent.len() < 4 || recent.last().map(|r| r.0).unwrap_or(0.0) - recent[0].0 < 60.0 {
        return (None, None);
    }
    let n = recent.len() as f64;
    let (sx, sy) = recent.iter().fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y));
    let (mx, my) = (sx / n, sy / n);
    let (num, den) = recent.iter().fold((0.0, 0.0), |(a, b), (x, y)| (a + (x - mx) * (y - my), b + (x - mx) * (x - mx)));
    if den == 0.0 {
        return (None, None);
    }
    let per_day = num / den;
    let per_month = -per_day * 30.44;
    if per_month <= 0.05 {
        return (Some(per_month.max(0.0)), None);
    }
    let now = recent.last().map(|r| r.1).unwrap_or(0.0);
    let to_half = if now > 50.0 { Some((now - 50.0) / per_month) } else { Some(0.0) };
    (Some(per_month), to_half)
}

#[tauri::command(async)]
pub fn battery_history() -> Result<Option<BatteryHistory>, String> {
    let out = crate::pspool::query(SCRIPT, Some(Duration::from_secs(60)), "Historial de la batería")?;
    let out = out.trim();
    if out.is_empty() || out == "null" {
        return Ok(None);
    }
    let raw: Raw = serde_json::from_str(out).map_err(|e| format!("Respuesta inesperada del informe de batería: {e}"))?;
    let points = clean(raw.history, raw.design);
    let (loss_per_month, months_to_half) = trend(&points);
    Ok(Some(BatteryHistory { name: raw.name, design: raw.design, full: raw.full, cycles: raw.cycles, points, loss_per_month, months_to_half }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(day: &str, full: u64) -> BatteryPoint {
        BatteryPoint { day: day.into(), full, design: 50_000 }
    }

    #[test]
    fn script_parses() {
        let errors = crate::ps::parse_errors(SCRIPT);
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn cleans_the_windows_history() {
        let raw = vec![
            p("2026-03-02T00:00:00", 45_000),
            p("2026-01-05T00:00:00", 47_000),
            // Sin dato, o un disparate: fuera.
            p("2026-02-02T00:00:00", 0),
            p("2026-02-09T00:00:00", 90_000),
            BatteryPoint { day: "2026-02-16".into(), full: 46_000, design: 0 },
        ];
        let c = clean(raw, 50_000);
        assert_eq!(c.iter().map(|x| x.day.as_str()).collect::<Vec<_>>(), ["2026-01-05", "2026-02-16", "2026-03-02"]);
        // Sin la de fábrica en el tramo, se usa la de hoy.
        assert_eq!(c[1].design, 50_000);
    }

    #[test]
    fn projects_when_it_will_be_at_half() {
        // Del 100 % al 88 % en un año: un punto al mes; desde el 88 %, 38 puntos hasta el 50 %.
        let pts: Vec<BatteryPoint> = (0..13)
            .map(|m| {
                let d = chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + chrono::Duration::days((m as f64 * 30.44) as i64);
                p(&d.format("%Y-%m-%d").to_string(), 50_000 - m * 500)
            })
            .collect();
        let (loss, half) = trend(&pts);
        assert!((loss.unwrap() - 1.0).abs() < 0.05, "{loss:?}");
        assert!((half.unwrap() - 38.0).abs() < 1.5, "{half:?}");
    }

    #[test]
    fn says_nothing_without_enough_history() {
        assert_eq!(trend(&[p("2026-01-05", 45_000), p("2026-01-12", 44_900)]), (None, None));
        // Si no baja, no hay fecha para el 50 %.
        let flat: Vec<BatteryPoint> = (0..6).map(|i| p(&format!("2026-0{}-01", i + 1), 45_000)).collect();
        assert_eq!(trend(&flat).1, None);
    }
}
