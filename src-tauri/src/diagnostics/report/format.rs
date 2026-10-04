//! Cómo se escriben fechas, cifras y textos en el informe.

use super::*;

pub(super) fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Texto dentro de una cadena CSS (`content: "…"`).
pub(super) fn css_str(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().replace('\\', "\\\\").replace('"', "\\\"").replace('<', "\\3C ")
}

pub(super) fn local(ts: u64) -> DateTime<Local> {
    Local.timestamp_opt(ts as i64, 0).single().unwrap_or_else(Local::now)
}

pub(super) fn fmt_ts(ts: u64) -> String {
    local(ts).format("%d/%m/%Y %H:%M").to_string()
}

pub(super) fn fmt_day(ts: u64) -> String {
    local(ts).format("%d/%m/%Y").to_string()
}

pub(super) fn fmt_iso(iso: &str) -> String {
    parse_time(iso).map_or_else(|| iso.to_string(), |d| d.format("%d/%m/%Y %H:%M").to_string())
}

pub(super) fn iso_day(iso: &str) -> String {
    fmt_iso(iso).split(' ').next().unwrap_or("").to_string()
}

pub(super) fn gb(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1024f64.powi(3))
}

pub(super) fn yes_no(b: Option<bool>, yes: &str, no: &str) -> String {
    match b {
        Some(true) => yes.into(),
        Some(false) => no.into(),
        None => "No disponible".into(),
    }
}

/// 1234.5 → "RD$ 1,234.50"
pub(super) fn money(v: f64, currency: &str) -> String {
    let cents = (v.abs() * 100.0).round() as u64;
    let int = (cents / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if v < 0.0 && cents > 0 { "-" } else { "" };
    let cur = currency.trim();
    format!("{sign}{cur}{}{grouped}.{:02}", if cur.is_empty() { "" } else { " " }, cents % 100)
}

pub(super) fn qty(q: f64) -> String {
    if q.fract().abs() < 1e-9 {
        format!("{q:.0}")
    } else {
        format!("{q:.2}")
    }
}
