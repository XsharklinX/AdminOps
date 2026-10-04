//! Antes y después: qué se resolvió y cómo cambiaron las cifras.

use super::*;

pub(super) fn sev_class(s: Severity) -> &'static str {
    match s {
        Severity::Bad => "bad",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

pub(super) fn op_label(op: Op) -> &'static str {
    match op {
        Op::Apply => "Aplicado",
        Op::Revert => "Deshecho",
        Op::Run => "Ejecutado",
        Op::RestorePoint => "Punto de restauración",
    }
}

/// Números de serie de relleno que ponen algunos fabricantes de placas.
pub(super) fn real_serial(s: &str) -> Option<&str> {
    let t = s.trim();
    let l = t.to_lowercase();
    let fake = ["default string", "to be filled by o.e.m.", "system serial number", "not specified", "none", "n/a", "0", "123456789", "chassis serial number"];
    (!t.is_empty() && !fake.contains(&l.as_str()) && !t.chars().all(|c| c == '0' || c == ' ')).then_some(t)
}

pub(super) fn important(f: &&Finding) -> bool {
    f.severity != Severity::Info
}

/// Problemas del "antes" que ya no aparecen en el "después".
pub(super) fn resolved<'a>(cur: &Diagnostics, base: &'a Diagnostics) -> Vec<&'a Finding> {
    base.findings.iter().filter(important).filter(|b| !cur.findings.iter().any(|c| crate::diagnostics::same_problem(b, c))).collect()
}

/// Fila de la tabla antes/después.
pub(super) struct Row {
    pub(super) label: String,
    pub(super) before: String,
    pub(super) after: String,
    /// (diferencia numérica, ¿es mejor que suba?)
    pub(super) delta: (f64, bool),
    pub(super) text: String,
}

/// Métrica contable de un análisis: (etiqueta, cómo obtenerla, ¿mejor si sube?).
pub(super) type Metric = (&'static str, fn(&Diagnostics) -> Option<usize>, bool);

pub(super) fn comparison_rows(cur: &Diagnostics, base: &Diagnostics) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for v in &cur.volumes {
        if let Some(b) = base.volumes.iter().find(|b| b.mount == v.mount) {
            let delta = v.free as f64 - b.free as f64;
            // Menos de 100 MB es el ir y venir normal de temporales: no es un cambio.
            let delta = if delta.abs() < 100.0 * 1024.0 * 1024.0 { 0.0 } else { delta };
            rows.push(Row {
                label: format!("Espacio libre en {}", v.mount),
                before: gb(b.free),
                after: gb(v.free),
                delta: (delta, true),
                text: format!("{:+.1} GB", delta / 1024f64.powi(3)),
            });
        }
    }
    let metrics: [Metric; 7] = [
        ("Nota de seguridad (0-100)", |d| d.security.data.as_ref().map(|a| a.score as usize), true),
        ("Problemas críticos", |d| Some(d.findings.iter().filter(|f| f.severity == Severity::Bad).count()), false),
        ("Advertencias", |d| Some(d.findings.iter().filter(|f| f.severity == Severity::Warn).count()), false),
        ("Programas que arrancan con Windows", |d| d.startup_enabled.data.as_ref().map(Vec::len), false),
        ("Programas con actualizaciones pendientes", |d| d.software_updates.data.as_ref().map(Vec::len), false),
        ("Apps promocionales instaladas", |d| d.bloat_installed.data.as_ref().map(Vec::len), false),
        ("Ajustes de optimización aplicados", |d| Some(d.tweaks_applied), true),
    ];
    for (label, get, up_is_better) in metrics {
        if let (Some(b), Some(c)) = (get(base), get(cur)) {
            rows.push(Row {
                label: label.into(),
                before: b.to_string(),
                after: c.to_string(),
                delta: (c as f64 - b as f64, up_is_better),
                text: format!("{:+}", c as i64 - b as i64),
            });
        }
    }
    let boot = |d: &Diagnostics| d.stability.data.as_ref()?.boot_times.as_ref()?.first().map(|b| b.ms);
    if let (Some(b), Some(c)) = (boot(base), boot(cur)) {
        if b != c {
            let delta = c as f64 - b as f64;
            rows.push(Row {
                label: "Tiempo del último arranque".into(),
                before: format!("{:.1} s", b as f64 / 1000.0),
                after: format!("{:.1} s", c as f64 / 1000.0),
                delta: (delta, false),
                text: format!("{:+.1} s", delta / 1000.0),
            });
        }
    }
    rows
}

pub(super) fn comparison(h: &mut String, c: &Ctx, base: &Diagnostics) {
    let rows = comparison_rows(c.cur, base);
    let class = |r: &Row| {
        let (d, up) = r.delta;
        if d.abs() < f64::EPSILON {
            "muted"
        } else if (d > 0.0) == up {
            "better"
        } else {
            "worse"
        }
    };
    // Para el cliente, solo lo que cambió.
    let rows: Vec<&Row> = rows.iter().filter(|r| c.technical() || class(r) != "muted").collect();
    if rows.is_empty() {
        return;
    }
    let _ = write!(
        h,
        "<h2>Antes y después</h2><p class=muted>Comparación del análisis al llegar ({}) con el análisis final.</p><table><thead><tr><th>Indicador</th><th class=num>Antes</th><th class=num>Después</th><th class=num>Cambio</th></tr></thead>",
        fmt_ts(base.timestamp)
    );
    for r in rows {
        let cls = class(r);
        let text = if cls == "muted" { "Sin cambios".to_string() } else { r.text.clone() };
        let _ = write!(
            h,
            "<tr><td>{}</td><td class=num>{}</td><td class=num>{}</td><td class=\"num {cls}\">{}</td></tr>",
            esc(&r.label),
            esc(&r.before),
            esc(&r.after),
            esc(&text)
        );
    }
    h.push_str("</table>");
}
