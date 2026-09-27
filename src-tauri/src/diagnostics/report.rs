//! Informe del servicio: un solo HTML autocontenido que se imprime a PDF con Edge.
//!
//! Dos plantillas: la del cliente (estado del equipo en lenguaje claro, trabajo
//! hecho, pendientes, presupuesto o recibo, garantías y firmas) y la técnica
//! (lo mismo más todo el detalle del análisis).

use super::{latest_snapshot, load_snapshot, parse_time, Diagnostics, Finding, Severity};
use crate::network::speedtest::SpeedResult;
use crate::paths::reports_dir;
use crate::tweaks::journal::{Entry, Op};
use crate::tweaks::TweakState;
use crate::workflow::{Billing, ChecklistItem, Client, DocKind, Settings, Template, Warranty};
use base64::Engine;
use chrono::{DateTime, Local, TimeZone};
use serde::Deserialize;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use tauri::State;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Texto dentro de una cadena CSS (`content: "…"`).
fn css_str(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().replace('\\', "\\\\").replace('"', "\\\"").replace('<', "\\3C ")
}

fn local(ts: u64) -> DateTime<Local> {
    Local.timestamp_opt(ts as i64, 0).single().unwrap_or_else(Local::now)
}

fn fmt_ts(ts: u64) -> String {
    local(ts).format("%d/%m/%Y %H:%M").to_string()
}

fn fmt_day(ts: u64) -> String {
    local(ts).format("%d/%m/%Y").to_string()
}

fn fmt_iso(iso: &str) -> String {
    parse_time(iso).map_or_else(|| iso.to_string(), |d| d.format("%d/%m/%Y %H:%M").to_string())
}

fn iso_day(iso: &str) -> String {
    fmt_iso(iso).split(' ').next().unwrap_or("").to_string()
}

fn gb(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1024f64.powi(3))
}

fn yes_no(b: Option<bool>, yes: &str, no: &str) -> String {
    match b {
        Some(true) => yes.into(),
        Some(false) => no.into(),
        None => "No disponible".into(),
    }
}

/// 1234.5 → "RD$ 1,234.50"
fn money(v: f64, currency: &str) -> String {
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

fn qty(q: f64) -> String {
    if q.fract().abs() < 1e-9 {
        format!("{q:.0}")
    } else {
        format!("{q:.2}")
    }
}

const CSS: &str = r#"
*{box-sizing:border-box}
html{background:#e8ecf1}
body{margin:0;color:#1b2330;font:12.5px/1.5 "IBM Plex Sans","Segoe UI",system-ui,sans-serif;counter-reset:sec}
main{max-width:840px;margin:24px auto;background:#fff;padding:44px 48px 36px;box-shadow:0 1px 4px rgba(20,30,50,.12)}
.top{display:flex;justify-content:space-between;gap:24px;align-items:flex-start}
.brand{display:flex;gap:14px;align-items:center;min-width:0}
.brand svg,.brand img{width:50px;height:50px;object-fit:contain;flex:none}
.brand .name{font-size:16px;font-weight:600;line-height:1.25}
.brand .contact{color:#5b6778;font-size:11px}
.doc{text-align:right;flex:none}
.doc .kind{font-size:21px;font-weight:600;letter-spacing:-.01em;line-height:1.2}
.doc .num{color:#5b6778;font-size:11.5px;margin-top:2px}.doc .num b{color:#1b2330;font-weight:600}
.rule{height:3px;background:#2f63d8;border-radius:2px;margin:18px 0 18px}
.info{display:grid;grid-template-columns:repeat(3,1fr);gap:10px}
.box{background:#f4f6f9;border-radius:8px;padding:10px 12px;min-width:0}
.box h4,.label{margin:0 0 4px;font-size:10px;font-weight:600;color:#5b6778;text-transform:uppercase;letter-spacing:.07em}
.box .main{font-weight:600;font-size:13px;overflow-wrap:anywhere}.box div{overflow-wrap:anywhere}.box .sub{color:#5b6778;font-size:11.5px}
.verdict{display:flex;gap:20px;align-items:center;border:1px solid #e3e8ef;border-left:5px solid var(--c);border-radius:10px;padding:14px 18px;margin-top:16px;break-inside:avoid}
.verdict .state{flex:1;min-width:0}.verdict .state .t{font-size:17px;font-weight:600;color:var(--c)}.verdict .state p{margin:3px 0 0;color:#3b4656}
.kpis{display:flex;gap:6px}.kpi{text-align:center;min-width:74px;padding:4px 6px}.kpi b{display:block;font-size:21px;font-weight:600;line-height:1.2;font-variant-numeric:tabular-nums}.kpi span{font-size:10.5px;color:#5b6778}
.v-ok{--c:#12784a}.v-warn{--c:#b06f00}.v-bad{--c:#c0223f}
h2{font-size:14px;font-weight:600;margin:26px 0 9px;padding-bottom:5px;border-bottom:1px solid #e3e8ef;break-after:avoid}
h2::before{counter-increment:sec;content:counter(sec) ". ";color:#2f63d8}
h3{font-size:12.5px;font-weight:600;margin:16px 0 6px;color:#3b4656;break-after:avoid}
p{margin:6px 0}
table{width:100%;border-collapse:collapse;font-size:12px}
th,td{text-align:left;padding:6px 8px;border-bottom:1px solid #edf0f4;vertical-align:top}
thead th,tr.head th{color:#5b6778;font-weight:600;font-size:11px;background:#f4f6f9;border-bottom:0}
table.kv th{width:30%;color:#5b6778;font-weight:500}
.nowrap{white-space:nowrap}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.muted{color:#8a95a5}.better{color:#12784a;font-weight:600}.worse{color:#c0223f;font-weight:600}.warn{color:#b06f00;font-weight:600}
.items{list-style:none;padding:0;margin:0;columns:2;column-gap:24px}
.items li{padding:2px 0 2px 20px;position:relative;break-inside:avoid}
.items li::before{position:absolute;left:0;top:2px;font-weight:700}
.items .y::before{content:"✓";color:#12784a}.items .n::before{content:"–";color:#8a95a5}.items .x::before{content:"!";color:#c0223f}
.items.one{columns:1}
.f{display:flex;gap:10px;padding:6px 10px;border-radius:6px;margin-bottom:5px;background:#f7f9fb;break-inside:avoid}
.f .dot{width:8px;height:8px;border-radius:50%;margin-top:6px;flex:none}
.dot.bad{background:#e0284a}.dot.warn{background:#e39a00}.dot.info{background:#2a8fc4}.dot.ok{background:#1f9d62}
.fgrid{display:grid;grid-template-columns:1fr 1fr;gap:0 8px}
.f b{font-weight:600}.f small{display:block;color:#5b6778}
.cols{display:grid;grid-template-columns:1fr 1fr;gap:20px}
.text{white-space:pre-wrap;background:#f7f9fb;border-left:3px solid #2f63d8;padding:10px 14px;border-radius:4px}
.bill thead th{padding:7px 8px}.bill td{padding:7px 8px}
.totals{width:300px;margin:8px 0 0 auto}.totals td{border:0;padding:3px 8px}
.totals tr.grand td{font-size:15px;font-weight:700;border-top:2px solid #1b2330;padding-top:7px}
.note{font-size:11px;color:#5b6778;margin-top:6px}
.warranty td.num{width:120px}
.speed{display:flex;gap:10px}.speed div{flex:1;background:#f4f6f9;border-radius:8px;padding:8px 12px;font-size:11px;color:#5b6778}.speed b{display:block;font-size:17px;color:#1b2330;font-weight:600}
.sign{display:grid;grid-template-columns:1fr 1fr;gap:36px;margin-top:34px;break-inside:avoid}
.sign .pad{height:74px;display:flex;align-items:flex-end;justify-content:center;border-bottom:1px solid #1b2330;padding-bottom:3px}
.sign img{max-height:70px;max-width:100%}
.sign .who{text-align:center;margin-top:5px;font-size:11.5px}.sign .who span{display:block;color:#5b6778;font-size:10.5px}
.conditions{font-size:10px;color:#5b6778;white-space:pre-wrap;border-top:1px solid #e3e8ef;margin-top:26px;padding-top:8px}
footer{margin-top:22px;color:#8a95a5;font-size:10px;text-align:center}
@media print{
*{-webkit-print-color-adjust:exact;print-color-adjust:exact}
html{background:#fff}main{margin:0;padding:0;max-width:none;box-shadow:none}
tr,.f,.box,.kpi{break-inside:avoid}footer{display:none}
}
"#;

const LOGO: &str = include_str!("../../../src/assets/logo.svg");

/// Datos del informe además del diagnóstico.
pub struct ReportInput {
    /// Snapshot "antes" para comparar.
    pub baseline: Option<u64>,
    pub client: Client,
    /// `None`: el técnico de los ajustes.
    pub technician: Option<String>,
    pub notes: String,
    pub checklist: Vec<ChecklistItem>,
    /// Desde cuándo contar el trabajo realizado (por defecto, la base o hoy).
    pub since: Option<u64>,
    pub template: Template,
    pub billing: Billing,
    pub problem: String,
    pub recommendations: String,
    pub signature: Option<String>,
    pub signer: String,
    pub warranties: Vec<Warranty>,
    pub next_maintenance: Option<u64>,
}

pub struct Created {
    pub path: String,
    pub number: String,
}

struct Ctx<'a> {
    cur: &'a Diagnostics,
    base: Option<&'a Diagnostics>,
    journal: &'a [Entry],
    technician: &'a str,
    input: &'a ReportInput,
    settings: &'a Settings,
    speed: Option<&'a SpeedResult>,
    number: &'a str,
    since: u64,
}

impl Ctx<'_> {
    fn technical(&self) -> bool {
        self.input.template == Template::Technical
    }
}

fn sev_class(s: Severity) -> &'static str {
    match s {
        Severity::Bad => "bad",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

fn op_label(op: Op) -> &'static str {
    match op {
        Op::Apply => "Aplicado",
        Op::Revert => "Deshecho",
        Op::Run => "Ejecutado",
        Op::RestorePoint => "Punto de restauración",
    }
}

/// Números de serie de relleno que ponen algunos fabricantes de placas.
fn real_serial(s: &str) -> Option<&str> {
    let t = s.trim();
    let l = t.to_lowercase();
    let fake = ["default string", "to be filled by o.e.m.", "system serial number", "not specified", "none", "n/a", "0", "123456789", "chassis serial number"];
    (!t.is_empty() && !fake.contains(&l.as_str()) && !t.chars().all(|c| c == '0' || c == ' ')).then_some(t)
}

fn important(f: &&Finding) -> bool {
    f.severity != Severity::Info
}

/// Problemas del "antes" que ya no aparecen en el "después".
fn resolved<'a>(cur: &Diagnostics, base: &'a Diagnostics) -> Vec<&'a Finding> {
    base.findings.iter().filter(important).filter(|b| !cur.findings.iter().any(|c| c.title == b.title)).collect()
}

/// Fila de la tabla antes/después.
struct Row {
    label: String,
    before: String,
    after: String,
    /// (diferencia numérica, ¿es mejor que suba?)
    delta: (f64, bool),
    text: String,
}

/// Métrica contable de un análisis: (etiqueta, cómo obtenerla, ¿mejor si sube?).
type Metric = (&'static str, fn(&Diagnostics) -> Option<usize>, bool);

fn comparison_rows(cur: &Diagnostics, base: &Diagnostics) -> Vec<Row> {
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

fn comparison(h: &mut String, c: &Ctx, base: &Diagnostics) {
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

fn header(h: &mut String, c: &Ctx) {
    let st = c.settings;
    let d = c.cur;
    // El logo del técnico (data URL validada al guardar) sustituye al de AdminOps.
    let logo = match &st.logo {
        Some(url) => format!("<img src=\"{}\" alt=\"\">", esc(url)),
        None => LOGO.to_string(),
    };
    let name = if st.company.trim().is_empty() {
        if st.technician.trim().is_empty() { "Servicio técnico".to_string() } else { esc(st.technician.trim()) }
    } else {
        esc(st.company.trim())
    };
    let contact: Vec<String> = [&st.phone, &st.email, &st.website].into_iter().filter(|x| !x.trim().is_empty()).map(|x| esc(x.trim())).collect();
    let kind = match (c.input.billing.active(), c.input.billing.kind) {
        (true, DocKind::Quote) => "Informe y presupuesto",
        (true, DocKind::Receipt) => "Informe y recibo",
        _ if c.technical() => "Informe técnico",
        _ => "Informe de servicio",
    };
    let _ = write!(
        h,
        "<div class=top><div class=brand>{logo}<div><div class=name>{name}</div><div class=contact>{}</div></div></div>\
         <div class=doc><div class=kind>{kind}</div><div class=num>Nº <b>{}</b> · {}</div></div></div><div class=rule></div>",
        contact.join(" · "),
        esc(c.number),
        fmt_day(d.timestamp)
    );

    // Cliente · Equipo · Servicio
    let cl = &c.input.client;
    h.push_str("<div class=info><div class=box><h4>Cliente</h4>");
    if cl.name.trim().is_empty() {
        h.push_str("<div class=muted>—</div>");
    } else {
        let _ = write!(h, "<div class=main>{}</div>", esc(cl.name.trim()));
    }
    for x in [&cl.contact, &cl.phone, &cl.email, &cl.address] {
        if !x.trim().is_empty() && x.trim() != cl.name.trim() {
            let _ = write!(h, "<div class=sub>{}</div>", esc(x.trim()));
        }
    }
    h.push_str("</div><div class=box><h4>Equipo</h4>");
    let _ = write!(h, "<div class=main>{}</div>", esc(&d.host));
    if let Some(hw) = &d.hardware.data {
        let model = format!("{} {}", hw.manufacturer.trim(), hw.model.trim());
        if !model.trim().is_empty() {
            let _ = write!(h, "<div class=sub>{}</div>", esc(model.trim()));
        }
        if let Some(serial) = real_serial(&hw.serial) {
            let _ = write!(h, "<div class=sub>Nº de serie: {}</div>", esc(serial));
        }
    }
    let _ = write!(h, "<div class=sub>{}</div></div><div class=box><h4>Servicio</h4>", esc(&d.os));
    if !c.technician.is_empty() {
        let _ = write!(h, "<div class=main>{}</div>", esc(c.technician));
    }
    let started = c.base.map(|b| b.timestamp).unwrap_or(c.since);
    if started > 0 && started < d.timestamp && d.timestamp - started < 7 * 86_400 {
        let mins = (d.timestamp - started) / 60;
        let _ = write!(
            h,
            "<div class=sub>{} · {}–{}</div><div class=sub>Duración: {}</div>",
            fmt_day(started),
            local(started).format("%H:%M"),
            local(d.timestamp).format("%H:%M"),
            if mins < 60 { format!("{mins} min") } else { format!("{} h {} min", mins / 60, mins % 60) }
        );
    } else {
        let _ = write!(h, "<div class=sub>{}</div>", fmt_ts(d.timestamp));
    }
    h.push_str("</div></div>");
}

fn summary(h: &mut String, c: &Ctx, work: usize) {
    let d = c.cur;
    let n = |s| d.findings.iter().filter(|f| f.severity == s).count();
    let (bad, warn) = (n(Severity::Bad), n(Severity::Warn));
    let (cls, title) = match (bad, warn) {
        (0, 0) => ("v-ok", "Equipo en buen estado"),
        (0, _) => ("v-warn", "Equipo en buen estado, con puntos a mejorar"),
        _ => ("v-bad", "El equipo requiere atención"),
    };
    let solved = c.base.map(|b| resolved(d, b).len());
    let mut text = Vec::new();
    if work > 0 {
        text.push(format!("Se realizaron {work} {} de mantenimiento y reparación.", if work == 1 { "acción" } else { "acciones" }));
    }
    if let (Some(s), Some(b)) = (solved, c.base) {
        let before = b.findings.iter().filter(important).count();
        if before > 0 && s > 0 {
            text.push(format!("Se {} {s} de los {before} problemas detectados al llegar.", if s == 1 { "resolvió" } else { "resolvieron" }));
        }
    }
    let minor = |w: usize| if w == 1 { "1 punto a mejorar".to_string() } else { format!("{w} puntos a mejorar") };
    match (bad, warn) {
        (0, 0) => text.push("No quedan problemas pendientes.".into()),
        (0, w) => text.push(format!("{} {}, sin gravedad.", if w == 1 { "Queda" } else { "Quedan" }, minor(w))),
        (1, 0) => text.push("Queda 1 problema importante que conviene resolver.".into()),
        (b, 0) => text.push(format!("Quedan {b} problemas importantes que conviene resolver.")),
        (b, w) => text.push(format!(
            "{} {b} {} y {} que conviene resolver.",
            if b == 1 { "Queda" } else { "Quedan" },
            if b == 1 { "problema importante" } else { "problemas importantes" },
            minor(w)
        )),
    }
    let _ = write!(h, "<div class=\"verdict {cls}\"><div class=state><div class=t>{title}</div><p>{}</p></div><div class=kpis>", esc(&text.join(" ")));
    if let Some(a) = &d.security.data {
        let _ = write!(h, "<div class=kpi><b>{}</b><span>Seguridad /100</span></div>", a.score);
    }
    if let Some(s) = solved {
        let _ = write!(h, "<div class=kpi><b>{s}</b><span>Resueltos</span></div>");
    }
    let _ = write!(h, "<div class=kpi><b>{}</b><span>Pendientes</span></div><div class=kpi><b>{work}</b><span>Acciones</span></div></div></div>", bad + warn);
}

fn finding_row(h: &mut String, f: &Finding, detail: bool, dot: &str) {
    let small = match (&f.detail, detail) {
        (Some(x), true) => format!("<small>{} · {}</small>", esc(&f.area), esc(x)),
        _ => format!("<small>{}</small>", esc(&f.area)),
    };
    let _ = write!(h, "<div class=f><div class=\"dot {dot}\"></div><div><b>{}</b>{small}</div></div>", esc(&f.title));
}

fn billing(h: &mut String, c: &Ctx) {
    let b = &c.input.billing;
    if !b.active() {
        return;
    }
    let st = c.settings;
    let cur = &st.currency;
    let quote = b.kind == DocKind::Quote;
    let with_warranty = !quote && b.lines().any(|l| l.part && l.warranty_days > 0);
    let _ = write!(
        h,
        "<h2>{}</h2><table class=bill><thead><tr><th>Descripción</th>{}<th class=num>Cant.</th><th class=num>Precio</th><th class=num>Importe</th></tr></thead>",
        if quote { "Presupuesto" } else { "Recibo" },
        if with_warranty { "<th>Garantía</th>" } else { "" }
    );
    for l in b.lines() {
        let warranty = if !with_warranty {
            String::new()
        } else if l.part && l.warranty_days > 0 {
            format!("<td>{}</td>", days_label(l.warranty_days))
        } else {
            "<td class=muted>—</td>".into()
        };
        let _ = write!(
            h,
            "<tr><td>{}{}</td>{warranty}<td class=num>{}</td><td class=num>{}</td><td class=num>{}</td></tr>",
            esc(l.description.trim()),
            if l.part { " <span class=muted>(pieza)</span>" } else { "" },
            qty(l.qty),
            money(l.price, cur),
            money(l.amount(), cur)
        );
    }
    h.push_str("</table>");
    let t = b.totals(st.tax_rate);
    let _ = write!(h, "<table class=totals><tr><td>Subtotal</td><td class=num>{}</td></tr>", money(t.subtotal, cur));
    if t.discount > 0.0 {
        let _ = write!(h, "<tr><td>Descuento</td><td class=num>-{}</td></tr>", money(t.discount, cur));
    }
    if st.tax_rate > 0.0 {
        let name = if st.tax_name.trim().is_empty() { "Impuesto" } else { st.tax_name.trim() };
        let _ = write!(h, "<tr><td>{} ({}%)</td><td class=num>{}</td></tr>", esc(name), qty(st.tax_rate), money(t.tax, cur));
    }
    let _ = write!(h, "<tr class=grand><td>Total</td><td class=num>{}</td></tr></table>", money(t.total, cur));
    if quote {
        if st.quote_validity_days > 0 {
            let until = c.cur.timestamp + st.quote_validity_days as u64 * 86_400;
            let _ = write!(h, "<p class=note>Presupuesto válido hasta el {} ({}). Los precios pueden variar si cambia el alcance del trabajo.</p>", fmt_day(until), days_label(st.quote_validity_days));
        }
    } else if !b.payment.trim().is_empty() {
        let _ = write!(h, "<p class=note>Pagado · {}.</p>", esc(b.payment.trim()));
    }
}

fn days_label(days: u32) -> String {
    match days {
        1 => "1 día".into(),
        d if d.is_multiple_of(365) => {
            let y = d / 365;
            if y == 1 { "1 año".into() } else { format!("{y} años") }
        }
        d if d.is_multiple_of(30) => {
            let m = d / 30;
            if m == 1 { "1 mes".into() } else { format!("{m} meses") }
        }
        d => format!("{d} días"),
    }
}

fn warranty_and_maintenance(h: &mut String, c: &Ctx) {
    let w = &c.input.warranties;
    let next = c.input.next_maintenance;
    if w.is_empty() && next.is_none() {
        return;
    }
    h.push_str(if w.is_empty() { "<h2>Próximo mantenimiento</h2>" } else if next.is_none() { "<h2>Garantía</h2>" } else { "<h2>Garantía y próximo mantenimiento</h2>" });
    if !w.is_empty() {
        h.push_str("<table class=warranty><thead><tr><th>Cubre</th><th class=num>Válida hasta</th></tr></thead>");
        for x in w {
            let _ = write!(h, "<tr><td>{}</td><td class=num>{}</td></tr>", esc(&x.item), fmt_day(x.until));
        }
        h.push_str("</table><p class=note>La garantía cubre el trabajo realizado y las piezas indicadas. No cubre daños por golpes, líquidos, subidas de tensión ni manipulación por terceros.</p>");
    }
    if let Some(n) = next {
        let _ = write!(
            h,
            "<p>Recomendamos el próximo mantenimiento preventivo hacia el <b>{}</b>: limpieza, actualizaciones y revisión de la salud del equipo alargan su vida útil y evitan averías.</p>",
            local(n).format("%d/%m/%Y")
        );
    }
}

fn signatures(h: &mut String, c: &Ctx) {
    let st = c.settings;
    let quote = c.input.billing.active() && c.input.billing.kind == DocKind::Quote;
    let img = |s: &Option<String>| s.as_deref().map(|u| format!("<img src=\"{}\" alt=\"\">", esc(u))).unwrap_or_default();
    let client_name = if c.input.signer.trim().is_empty() { c.input.client.name.trim() } else { c.input.signer.trim() };
    let _ = write!(
        h,
        "<div class=sign><div><div class=pad>{}</div><div class=who>{}<span>Técnico{}</span></div></div>\
         <div><div class=pad>{}</div><div class=who>{}<span>{}</span></div></div></div>",
        img(&st.tech_signature),
        if c.technician.is_empty() { "&nbsp;".to_string() } else { esc(c.technician) },
        if st.company.trim().is_empty() { String::new() } else { format!(" · {}", esc(st.company.trim())) },
        img(&c.input.signature),
        if client_name.is_empty() { "&nbsp;".to_string() } else { esc(client_name) },
        if quote {
            "Acepto el presupuesto".to_string()
        } else {
            format!("Conforme con el servicio{}", if c.input.signature.is_some() { format!(" · firmado el {}", fmt_day(c.cur.timestamp)) } else { String::new() })
        }
    );
}

/// Ficha resumida del equipo para el cliente.
fn machine_summary(h: &mut String, d: &Diagnostics) {
    h.push_str("<h2>Estado del equipo</h2><table class=kv>");
    let row = |h: &mut String, k: &str, v: &str| {
        if !v.trim().is_empty() {
            let _ = write!(h, "<tr><th>{k}</th><td>{v}</td></tr>");
        }
    };
    row(h, "Sistema", &esc(&d.os));
    row(h, "Procesador", &esc(&d.cpu));
    row(h, "Memoria", &gb(d.ram_total));
    let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    if let Some(v) = d.volumes.iter().find(|v| v.mount.to_uppercase().starts_with(&sys)) {
        let pct = if v.total > 0 { v.free as f64 * 100.0 / v.total as f64 } else { 0.0 };
        let cls = if pct < 10.0 { "worse" } else if pct < 20.0 { "warn" } else { "better" };
        row(h, "Disco del sistema", &format!("<span class={cls}>{} libres</span> de {} ({pct:.0}%)", gb(v.free), gb(v.total)));
    }
    if let Some(disks) = &d.disks.data {
        let mut bad: Vec<&str> = disks.iter().filter(|k| !k.health.eq_ignore_ascii_case("Healthy")).map(|k| k.name.as_str()).collect();
        // Windows puede dar un disco por "sano" aunque SMART ya muestre sectores dañados.
        for k in d.smart.data.iter().flatten() {
            let failing = k.predict_failure || k.pending.unwrap_or(0) > 0 || k.uncorrectable.unwrap_or(0) > 0 || k.reallocated.unwrap_or(0) > 0;
            if failing && !bad.iter().any(|b| b.contains(k.model.trim()) || k.model.contains(b.trim())) {
                bad.push(k.model.as_str());
            }
        }
        let text = if bad.is_empty() {
            format!("<span class=better>{}</span>", if disks.len() == 1 { "Saludable" } else { "Todos saludables" })
        } else {
            format!("<span class=worse>Revisar: {}</span>", esc(&bad.join(", ")))
        };
        row(h, "Salud de los discos", &text);
    }
    if let Some(Some(b)) = &d.battery.data {
        let hlt = b.health();
        let cls = if hlt < 60.0 { "worse" } else if hlt < 80.0 { "warn" } else { "better" };
        row(h, "Batería", &format!("<span class={cls}>{hlt:.0}% de su capacidad original</span>"));
    }
    if let Some(s) = &d.system.data {
        let av = if s.antivirus.is_empty() { "<span class=worse>No detectado</span>".to_string() } else { esc(&s.antivirus.join(", ")) };
        row(h, "Antivirus", &av);
        row(h, "Windows activado", &yes_no(s.activated, "Sí", "<span class=worse>No</span>"));
    }
    if let Some(u) = &d.software_updates.data {
        row(h, "Programas por actualizar", &if u.is_empty() { "Ninguno".to_string() } else { u.len().to_string() });
    }
    h.push_str("</table>");
}

fn technical_sections(h: &mut String, d: &Diagnostics) {
    // Equipo
    h.push_str("<h2>Equipo en detalle</h2><table class=kv>");
    let _ = write!(h, "<tr><th>Sistema</th><td>{}</td></tr><tr><th>Procesador</th><td>{}</td></tr><tr><th>Memoria</th><td>{}</td></tr>", esc(&d.os), esc(&d.cpu), gb(d.ram_total));
    if let Some(s) = &d.system.data {
        let _ = write!(h, "<tr><th>Instalado</th><td>{}</td></tr><tr><th>Último arranque</th><td>{}</td></tr>", fmt_iso(&s.install_date), fmt_iso(&s.last_boot));
    }
    if let Some(hw) = &d.hardware.data {
        let row = |h: &mut String, k: &str, v: String| {
            if !v.trim().is_empty() {
                let _ = write!(h, "<tr><th>{k}</th><td>{}</td></tr>", esc(&v));
            }
        };
        row(h, "Equipo", format!("{} {}", hw.manufacturer, hw.model));
        row(h, "Nº de serie", real_serial(&hw.serial).unwrap_or("").to_string());
        row(h, "Placa base", format!("{} {}", hw.board_manufacturer, hw.board_product));
        row(h, "BIOS", format!("{} {}{} · {}", hw.bios_vendor, hw.bios_version, hw.bios_date.as_deref().map(|d| format!(" ({})", iso_day(d))).unwrap_or_default(), hw.firmware));
        row(h, "Procesador", format!("{} · {} núcleos / {} hilos · {} MHz", hw.cpu, hw.cores, hw.threads, hw.max_mhz));
        row(h, "Memoria", format!("{} en {} de {} ranuras", gb(hw.ram_total), hw.modules.len(), hw.ram_slots));
        for g in &hw.gpus {
            row(
                h,
                "Gráfica",
                format!(
                    "{}{} · driver {}{}",
                    g.name,
                    g.vram.map(|v| format!(" ({})", gb(v))).unwrap_or_default(),
                    g.driver_version,
                    g.driver_date.as_deref().map(|d| format!(" del {}", iso_day(d))).unwrap_or_default()
                ),
            );
        }
        for m in &hw.monitors {
            row(h, "Monitor", format!("{} {}", m.manufacturer, m.name));
        }
        row(h, "Windows", format!("{} {} (compilación {}) · {}", hw.os, hw.os_version, hw.os_build, hw.architecture));
        h.push_str("</table>");
        if !hw.modules.is_empty() {
            h.push_str("<h3>Módulos de memoria</h3><table><thead><tr><th>Ranura</th><th>Módulo</th><th class=num>Capacidad</th><th class=num>Velocidad</th></tr></thead>");
            for m in &hw.modules {
                let _ = write!(
                    h,
                    "<tr><td>{}</td><td>{} {} {}</td><td class=num>{}</td><td class=num>{}</td></tr>",
                    esc(&m.slot),
                    esc(&m.manufacturer),
                    esc(&m.part_number),
                    esc(&m.kind),
                    gb(m.capacity),
                    m.configured_speed.or(m.speed).map(|s| format!("{s} MT/s")).unwrap_or("—".into())
                );
            }
            h.push_str("</table>");
        }
    } else {
        h.push_str("</table>");
    }
    if let Some(t) = &d.temperatures.data {
        let mut parts: Vec<String> = Vec::new();
        if let Some(c) = t.cpu {
            parts.push(format!("CPU {c:.0} °C"));
        }
        for (name, temp, hot) in &t.gpus {
            if let Some(g) = temp {
                parts.push(format!("{name} {g:.0} °C{}", hot.map(|x| format!(" (punto caliente {x:.0} °C)")).unwrap_or_default()));
            }
        }
        if !parts.is_empty() {
            let _ = write!(h, "<p><b>Temperaturas en el análisis:</b> {}</p>", esc(&parts.join(" · ")));
        }
    }
    if let Some(Some(m)) = &d.memory_test.data {
        let _ = write!(h, "<p><b>Prueba de memoria:</b> {} ({})</p>", if m.passed { "sin errores" } else { "<span class=worse>errores detectados</span>" }, iso_day(&m.time));
    }

    // Almacenamiento
    h.push_str("<h2>Almacenamiento</h2><table><thead><tr><th>Unidad</th><th class=num>Libre</th><th class=num>Total</th><th class=num>Uso</th></tr></thead>");
    for v in &d.volumes {
        let used = if v.total > 0 { 100.0 - v.free as f64 * 100.0 / v.total as f64 } else { 0.0 };
        let _ = write!(h, "<tr><td>{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{used:.0}%</td></tr>", esc(&v.mount), gb(v.free), gb(v.total));
    }
    h.push_str("</table>");
    if let Some(disks) = &d.disks.data {
        h.push_str("<h3>Salud de los discos</h3><table><thead><tr><th>Disco</th><th>Tipo</th><th>Estado</th><th class=num>Temp.</th><th class=num>Desgaste</th><th class=num>Horas</th></tr></thead>");
        for k in disks {
            let health = if k.health.eq_ignore_ascii_case("Healthy") { "<span class=better>Saludable</span>".to_string() } else { format!("<span class=worse>{}</span>", esc(&k.health)) };
            let _ = write!(
                h,
                "<tr><td>{}<div class=muted>{}</div></td><td>{} · {}</td><td>{health}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{}</td></tr>",
                esc(&k.name),
                gb(k.size),
                esc(&k.media_type),
                esc(&k.bus_type),
                k.temperature.map(|t| format!("{t} °C")).unwrap_or("—".into()),
                k.wear.map(|w| format!("{w}%")).unwrap_or("—".into()),
                k.power_on_hours.map(|p| p.to_string()).unwrap_or("—".into())
            );
        }
        h.push_str("</table>");
    }
    if let Some(smart) = d.smart.data.as_ref().filter(|s| !s.is_empty()) {
        h.push_str("<h3>Atributos SMART</h3><table><thead><tr><th>Disco</th><th class=num>Reasignados</th><th class=num>Pendientes</th><th class=num>No corregibles</th><th class=num>Errores CRC</th><th class=num>Horas</th></tr></thead>");
        let n = |v: Option<u64>| v.map(|x| x.to_string()).unwrap_or("—".into());
        for k in smart {
            let _ = write!(
                h,
                "<tr><td>{}{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{}</td></tr>",
                esc(&k.model),
                if k.predict_failure { " <span class=worse>· fallo previsto</span>" } else { "" },
                n(k.reallocated),
                n(k.pending),
                n(k.uncorrectable),
                n(k.crc_errors),
                n(k.power_on_hours)
            );
        }
        h.push_str("</table>");
    }

    // Estabilidad
    if let Some(s) = &d.stability.data {
        let _ = write!(h, "<h2>Estabilidad (últimos {} días)</h2><p>{} pantallazos azules · {} apagados inesperados</p>", s.days, s.bugchecks.len(), s.unexpected_shutdowns.len());
        if !s.bugchecks.is_empty() {
            h.push_str("<table><thead><tr><th>Fecha</th><th>Código</th><th>Posible causa</th></tr></thead>");
            for b in &s.bugchecks {
                let _ = write!(h, "<tr><td class=num>{}</td><td>{} {}</td><td>{}</td></tr>", fmt_iso(&b.time), esc(&b.code), esc(b.name.as_deref().unwrap_or("")), esc(b.hint.as_deref().unwrap_or("")));
            }
            h.push_str("</table>");
        }
        if !s.crashes.is_empty() {
            h.push_str("<h3>Aplicaciones que fallan</h3><table><thead><tr><th>Aplicación</th><th class=num>Veces</th><th class=num>Último fallo</th></tr></thead>");
            for cr in &s.crashes {
                let _ = write!(h, "<tr><td>{}</td><td class=num>{}</td><td class=num>{}</td></tr>", esc(&cr.app), cr.count, fmt_iso(&cr.last));
            }
            h.push_str("</table>");
        }
    }

    // Drivers
    if let Some(drivers) = &d.drivers.data {
        h.push_str("<h2>Dispositivos y drivers</h2>");
        if drivers.is_empty() {
            h.push_str("<p>Todos los dispositivos funcionan correctamente.</p>");
        } else {
            h.push_str("<table><thead><tr><th>Dispositivo</th><th>Problema</th><th class=num>Código</th></tr></thead>");
            for dev in drivers {
                let _ = write!(h, "<tr><td>{}</td><td>{}</td><td class=num>{}</td></tr>", esc(&dev.name), esc(&dev.problem), dev.code);
            }
            h.push_str("</table>");
        }
    }

    // Batería
    if let Some(Some(b)) = &d.battery.data {
        let _ = write!(
            h,
            "<h2>Batería</h2><table class=kv><tr><th>Capacidad</th><td>{:.0}% de la original ({} de {} mWh)</td></tr><tr><th>Ciclos</th><td>{}</td></tr><tr><th>Modelo</th><td>{} {} · {}</td></tr></table>",
            b.health(),
            b.full,
            b.design,
            b.cycles.map(|c| c.to_string()).unwrap_or("No disponible".into()),
            esc(&b.manufacturer),
            esc(&b.name),
            esc(&b.chemistry)
        );
    }

    // Seguridad
    if let Some(a) = &d.security.data {
        let verdict = if a.score >= 80 { "bien protegido" } else if a.score >= 60 { "mejorable" } else { "en riesgo" };
        let _ = write!(h, "<h2>Seguridad: {}/100 ({verdict})</h2><table>", a.score);
        for c in a.checks.iter().filter(|c| c.status != "unknown") {
            let mark = match c.status.as_str() {
                "ok" => "<span class=better>Correcto</span>",
                "warn" => "<span class=warn>Mejorable</span>",
                _ => "<span class=worse>Riesgo</span>",
            };
            let _ = write!(h, "<tr><th>{}</th><td>{mark}</td><td>{}</td></tr>", esc(&c.label), esc(&c.detail));
        }
        h.push_str("</table>");
    }
    if let Some(s) = &d.system.data {
        h.push_str("<h3>Protección y mantenimiento de Windows</h3><table class=kv>");
        let av = if s.antivirus.is_empty() { "No detectado".to_string() } else { s.antivirus.join(", ") };
        let _ = write!(h, "<tr><th>Antivirus</th><td>{}</td></tr>", esc(&av));
        let _ = write!(h, "<tr><th>Protección en tiempo real (Defender)</th><td>{}</td></tr>", yes_no(s.defender_realtime, "Activa", "Desactivada"));
        if let Some(days) = s.quick_scan_age_days {
            let _ = write!(h, "<tr><th>Último análisis antivirus</th><td>{}</td></tr>", if days == 0 { "Hoy".to_string() } else { format!("Hace {days} días") });
        }
        let _ = write!(
            h,
            "<tr><th>Última actualización</th><td>{}</td></tr>",
            s.last_update.as_deref().map(|u| format!("{}{}", iso_day(u), s.last_update_id.as_deref().map(|i| format!(" ({})", esc(i))).unwrap_or_default())).unwrap_or("No disponible".into())
        );
        let _ = write!(h, "<tr><th>Reinicio pendiente</th><td>{}</td></tr>", if s.pending_reboot { "Sí" } else { "No" });
        let _ = write!(h, "<tr><th>Windows activado</th><td>{}</td></tr>", yes_no(s.activated, "Sí", "No"));
        let _ = write!(h, "<tr><th>Arranque seguro</th><td>{}</td></tr>", yes_no(s.secure_boot, "Activado", "Desactivado"));
        let _ = write!(h, "<tr><th>TPM</th><td>{}</td></tr></table>", yes_no(s.tpm_ready, "Listo", "No disponible o no listo"));
    }

    // Software con actualizaciones
    if let Some(updates) = d.software_updates.data.as_ref().filter(|u| !u.is_empty()) {
        let _ = write!(h, "<h2>Programas con actualizaciones pendientes ({})</h2><table><thead><tr><th>Programa</th><th class=num>Instalada</th><th class=num>Disponible</th></tr></thead>", updates.len());
        for u in updates.iter().take(40) {
            let _ = write!(h, "<tr><td>{}</td><td class=num>{}</td><td class=num>{}</td></tr>", esc(&u.name), esc(&u.version), esc(&u.available));
        }
        h.push_str("</table>");
    }
}

fn build(c: &Ctx) -> String {
    let d = c.cur;
    let st = c.settings;
    let mut h = String::with_capacity(48 * 1024);
    let who = if st.company.trim().is_empty() { "AdminOps".to_string() } else { st.company.trim().to_string() };
    let _ = write!(
        h,
        "<!doctype html><html lang=es><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Informe {} — {}</title><style>{CSS}\
         @page{{size:A4;margin:14mm 13mm 16mm;\
         @bottom-left{{content:\"{} · Informe Nº {}\";font:8.5px \"Segoe UI\",sans-serif;color:#8a95a5}}\
         @bottom-right{{content:\"Página \" counter(page) \" de \" counter(pages);font:8.5px \"Segoe UI\",sans-serif;color:#8a95a5}}}}</style></head><body><main>",
        esc(&d.host),
        esc(c.number),
        css_str(&who),
        css_str(c.number)
    );
    header(&mut h, c);

    let work: Vec<&Entry> = c.journal.iter().filter(|e| e.ok && e.op != Op::Revert).collect();
    summary(&mut h, c, work.len());

    if !c.input.problem.trim().is_empty() {
        let _ = write!(h, "<h2>Motivo de la visita</h2><div class=text>{}</div>", esc(c.input.problem.trim()));
    }

    // Trabajo realizado
    if !work.is_empty() || c.checklist_done() > 0 {
        h.push_str("<h2>Trabajo realizado</h2>");
        if c.technical() {
            if !work.is_empty() {
                h.push_str("<table><thead><tr><th>Fecha</th><th>Acción</th><th>Detalle</th></tr></thead>");
                for e in &work {
                    let _ = write!(
                        h,
                        "<tr><td class=nowrap>{}</td><td class=nowrap>{}</td><td>{}{}</td></tr>",
                        fmt_ts(e.timestamp),
                        op_label(e.op),
                        esc(&e.title),
                        e.message.as_deref().map(|m| format!(" <span class=muted>— {}</span>", esc(m))).unwrap_or_default()
                    );
                }
                h.push_str("</table>");
            }
        } else {
            let mut seen: Vec<&str> = Vec::new();
            for e in &work {
                let t = if e.op == Op::RestorePoint { "Punto de restauración creado antes de los cambios" } else { e.title.as_str() };
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
            if !seen.is_empty() {
                h.push_str("<ul class=items>");
                for t in seen.iter().take(40) {
                    let _ = write!(h, "<li class=y>{}</li>", esc(t));
                }
                if seen.len() > 40 {
                    let _ = write!(h, "<li class=n>y {} acciones más</li>", seen.len() - 40);
                }
                h.push_str("</ul>");
            }
        }
    }

    // Checklist de servicio
    if !c.input.checklist.is_empty() {
        let done = c.checklist_done();
        let _ = write!(h, "<h3>Revisión de servicio · {done} de {}</h3><ul class=items>", c.input.checklist.len());
        for item in &c.input.checklist {
            let _ = write!(h, "<li class={}>{}</li>", if item.done { "y" } else { "n" }, esc(&item.text));
        }
        h.push_str("</ul>");
    }

    // Problemas resueltos y pendientes
    let solved = c.base.map(|b| resolved(d, b)).unwrap_or_default();
    let pending: Vec<&Finding> = d.findings.iter().filter(|f| c.technical() || important(f)).collect();
    if !solved.is_empty() || !pending.is_empty() {
        h.push_str("<h2>Problemas detectados</h2>");
        let both = !solved.is_empty() && !pending.is_empty() && !c.technical();
        if both {
            h.push_str("<div class=cols><div>");
        }
        if !solved.is_empty() {
            h.push_str("<h3>Resueltos en esta visita</h3>");
            for f in &solved {
                finding_row(&mut h, f, c.technical(), "ok");
            }
        }
        if both {
            h.push_str("</div><div>");
        }
        if !pending.is_empty() {
            h.push_str(if c.base.is_some() { "<h3>Pendientes</h3>" } else { "<h3>Estado actual</h3>" });
            h.push_str(if both { "<div>" } else { "<div class=fgrid>" });
            for f in &pending {
                finding_row(&mut h, f, c.technical(), sev_class(f.severity));
            }
            h.push_str("</div>");
        }
        if both {
            h.push_str("</div></div>");
        }
    }

    if let Some(base) = c.base {
        comparison(&mut h, c, base);
    }

    // Recomendaciones
    if !c.input.recommendations.trim().is_empty() {
        let _ = write!(h, "<h2>Recomendaciones</h2><div class=text>{}</div>", esc(c.input.recommendations.trim()));
    }

    billing(&mut h, c);
    warranty_and_maintenance(&mut h, c);

    if c.technical() {
        technical_sections(&mut h, d);
    } else {
        machine_summary(&mut h, d);
    }

    // Velocidad de Internet
    if let Some(sp) = c.speed {
        let _ = write!(
            h,
            "<h2>Conexión a Internet</h2><div class=speed><div>Bajada<b>{:.1} Mbps</b></div><div>Subida<b>{:.1} Mbps</b></div>\
             <div>Latencia<b>{:.0} ms</b></div><div>Variación<b>{:.1} ms</b></div></div><p class=muted>{} · {}{}</p>",
            sp.download_mbps,
            sp.upload_mbps,
            sp.latency_ms,
            sp.jitter_ms,
            esc(&sp.server),
            fmt_ts(sp.timestamp),
            sp.download_latency_ms.map(|l| format!(" · latencia con carga {l:.0} ms")).unwrap_or_default()
        );
    }

    if !c.input.notes.trim().is_empty() {
        let _ = write!(h, "<h2>Observaciones del técnico</h2><div class=text>{}</div>", esc(c.input.notes.trim()));
    }

    signatures(&mut h, c);

    if !st.conditions.trim().is_empty() {
        let _ = write!(h, "<div class=conditions>{}</div>", esc(st.conditions.trim()));
    }
    let _ = write!(h, "<footer>Generado el {} con AdminOps</footer></main></body></html>", Local::now().format("%d/%m/%Y %H:%M"));
    h
}

impl Ctx<'_> {
    fn checklist_done(&self) -> usize {
        self.input.checklist.iter().filter(|c| c.done).count()
    }
}

/// Abre un archivo con la app predeterminada. Se lanza a través de explorer.exe
/// para que el navegador NO herede los privilegios de administrador de AdminOps.
fn shell_open(args: &[&std::ffi::OsStr]) -> Result<(), String> {
    std::process::Command::new("explorer.exe").args(args).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn inside_reports(app: &tauri::AppHandle, path: &str) -> Result<PathBuf, String> {
    let dir = reports_dir(app).canonicalize().map_err(|_| "No hay informes".to_string())?;
    let p = Path::new(path).canonicalize().map_err(|_| "El informe ya no existe".to_string())?;
    if !p.starts_with(&dir) {
        return Err("Ruta fuera de la carpeta de informes".into());
    }
    let s = p.display().to_string();
    Ok(PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s)))
}

/// Genera el informe PDF (o HTML si falta Edge), lo abre y devuelve su ruta y número.
pub fn create_report(app: &tauri::AppHandle, state: &TweakState, input: ReportInput) -> Result<Created, String> {
    input.billing.validate()?;
    let cur = latest_snapshot(app).ok_or("Ejecuta un diagnóstico antes de generar el informe.")?;
    let base = input.baseline.filter(|b| *b != cur.timestamp).and_then(|b| load_snapshot(app, b));
    let today = || Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|d| d.and_local_timezone(Local).single()).map_or(0, |d| d.timestamp() as u64);
    let since = input.since.or(base.as_ref().map(|b| b.timestamp)).unwrap_or_else(today);
    let journal = state.journal_since(since);
    let settings = crate::workflow::settings(app);
    let technician = input.technician.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| settings.technician.clone());
    // Último test de velocidad del periodo del informe.
    let speed = crate::network::speedtest::history(app).into_iter().find(|r| r.timestamp >= since);
    let number = crate::workflow::next_number(app);
    let html = build(&Ctx {
        cur: &cur,
        base: base.as_ref(),
        journal: &journal,
        technician: technician.trim(),
        input: &input,
        settings: &settings,
        speed: speed.as_ref(),
        number: &number,
        since,
    });

    let dir = reports_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    let safe_host: String = cur.host.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let base_name = format!("Informe_{number}_{safe_host}");
    let pdf = dir.join(format!("{base_name}.pdf"));
    let path = match super::pdf::html_to_pdf(&html, &pdf) {
        Ok(()) => pdf,
        Err(e) => {
            log::warn!("No se pudo crear el PDF, se guarda en HTML: {e}");
            // Sin Edge (muy raro) el informe no se pierde: se guarda como HTML.
            let html_path = dir.join(format!("{base_name}.html"));
            std::fs::write(&html_path, &html).map_err(|w| format!("{e} · No se pudo guardar el HTML: {w}"))?;
            html_path
        }
    };
    shell_open(&[path.as_os_str()])?;
    Ok(Created { path: path.display().to_string(), number })
}

/// Opciones del informe hecho a mano (sin sesión de servicio).
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportOptions {
    pub baseline: Option<u64>,
    pub technician: String,
    /// Cliente de la ficha (sus datos van al informe) o solo un nombre.
    pub client_id: Option<String>,
    pub client: String,
    pub notes: String,
    pub template: Template,
    pub billing: Billing,
    pub problem: String,
    pub recommendations: String,
    /// Guardar la visita en la ficha del cliente.
    pub archive: bool,
}

#[tauri::command(async)]
pub fn generate_report(app: tauri::AppHandle, state: State<'_, TweakState>, options: ReportOptions) -> Result<String, String> {
    use crate::workflow::{archive_visit, find_client, maintenance_date, warranties, SessionRecord, VisitMetrics};
    let settings = crate::workflow::settings(&app);
    let found = options.client_id.as_deref().and_then(|id| find_client(&app, id));
    let client = found.clone().unwrap_or(Client { name: options.client.trim().into(), ..Default::default() });
    let now = Local::now().timestamp() as u64;
    // Garantías y mantenimiento solo cuando la visita queda en la ficha del cliente.
    let archive = options.archive && found.is_some();
    let warranties = if archive { warranties(&options.billing, settings.labor_warranty_days, now) } else { vec![] };
    let next_maintenance = if archive { maintenance_date(settings.maintenance_months, now) } else { None };
    let created = create_report(
        &app,
        &state,
        ReportInput {
            baseline: options.baseline,
            client,
            technician: Some(options.technician),
            notes: options.notes.clone(),
            checklist: vec![],
            since: None,
            template: options.template,
            billing: options.billing.clone(),
            problem: options.problem,
            recommendations: options.recommendations,
            signature: None,
            signer: String::new(),
            warranties: warranties.clone(),
            next_maintenance,
        },
    )?;
    if let (true, Some(c)) = (archive, found) {
        if let Some(after) = latest_snapshot(&app) {
            let before = options.baseline.and_then(|b| load_snapshot(&app, b));
            let count = |d: &Diagnostics, sev| d.findings.iter().filter(|f| f.severity == sev).count();
            let started = before.as_ref().map_or(after.timestamp, |b| b.timestamp);
            let active = options.billing.active();
            let record = SessionRecord {
                id: format!("{:x}", now),
                host: after.host.clone(),
                started,
                ended: now,
                report: Some(created.path.clone()),
                bad_before: before.as_ref().map_or(count(&after, Severity::Bad), |b| count(b, Severity::Bad)),
                bad_after: count(&after, Severity::Bad),
                warn_before: before.as_ref().map_or(count(&after, Severity::Warn), |b| count(b, Severity::Warn)),
                warn_after: count(&after, Severity::Warn),
                work_items: state.journal_since(started).iter().filter(|e| e.ok).count(),
                notes: options.notes,
                number: created.number.clone(),
                doc_kind: if active { options.billing.kind } else { DocKind::None },
                total: if active { options.billing.totals(settings.tax_rate).total } else { 0.0 },
                currency: settings.currency.clone(),
                warranties,
                next_maintenance,
                metrics: Some(VisitMetrics::from(&after)),
                ..Default::default()
            };
            archive_visit(&app, &c.id, &after, record)?;
        }
    }
    Ok(created.path)
}

#[tauri::command]
pub fn open_report(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let p = inside_reports(&app, &path)?;
    shell_open(&[p.as_os_str()])
}

#[tauri::command]
pub fn reveal_report(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let p = inside_reports(&app, &path)?;
    let arg = format!("/select,{}", p.display());
    shell_open(&[std::ffi::OsStr::new(&arg)])
}

// ---------- Envío por correo ----------

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Base64 en líneas de 76 caracteres (RFC 2045).
fn b64_lines(bytes: &[u8]) -> String {
    let s = b64(bytes);
    s.as_bytes().chunks(76).map(|c| std::str::from_utf8(c).unwrap_or("")).collect::<Vec<_>>().join("\r\n")
}

fn header_text(s: &str) -> String {
    if s.is_ascii() {
        s.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", b64(s.as_bytes()))
    }
}

fn check_mail_fields(to: &str, subject: &str) -> Result<(), String> {
    if [to, subject].iter().any(|s| s.contains(['\r', '\n'])) || to.len() > 320 || subject.len() > 300 {
        return Err("Revisa el destinatario y el asunto.".into());
    }
    if !to.trim().is_empty() && !to.split([',', ';']).all(|a| a.trim().contains('@')) {
        return Err("El correo del destinatario no es válido.".into());
    }
    Ok(())
}

/// Correo en formato .eml, marcado como borrador para que el programa de correo lo abra listo para enviar.
fn build_eml(to: &str, subject: &str, body: &str, file_name: &str, attachment: &[u8]) -> String {
    let boundary = format!("adminops-{:x}", attachment.len() ^ 0x5eed);
    let body = body.replace("\r\n", "\n").replace('\n', "\r\n");
    let mime = if file_name.to_lowercase().ends_with(".pdf") { "application/pdf" } else { "text/html" };
    format!(
        "X-Unsent: 1\r\nTo: {to}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n\
         --{boundary}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n\
         --{boundary}\r\nContent-Type: {mime}; name=\"{file_name}\"\r\nContent-Disposition: attachment; filename=\"{file_name}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n\
         --{boundary}--\r\n",
        header_text(subject),
        b64_lines(body.as_bytes()),
        b64_lines(attachment)
    )
}

/// Prepara el correo con el informe adjunto y lo abre en el programa de correo predeterminado.
#[tauri::command]
pub fn email_report(app: tauri::AppHandle, path: String, to: String, subject: String, body: String) -> Result<(), String> {
    check_mail_fields(&to, &subject)?;
    let p = inside_reports(&app, &path)?;
    let data = std::fs::read(&p).map_err(|e| format!("No se pudo leer el informe: {e}"))?;
    if data.len() > 20 * 1024 * 1024 {
        return Err("El informe es demasiado grande para enviarlo por correo.".into());
    }
    let file_name: String = p.file_name().map(|n| n.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect()).unwrap_or_else(|| "Informe.pdf".into());
    let eml = build_eml(to.trim(), subject.trim(), &body, &file_name, &data);
    let dir = reports_dir(&app).join("Correo");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join(format!("{}.eml", file_name.rsplit_once('.').map_or(file_name.as_str(), |x| x.0)));
    std::fs::write(&out, eml).map_err(|e| format!("No se pudo preparar el correo: {e}"))?;
    shell_open(&[out.as_os_str()])
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Alternativa sin adjunto (webmail, apps que no abren .eml): abre un correo nuevo
/// con el texto y muestra el PDF en su carpeta para arrastrarlo.
#[tauri::command]
pub fn email_report_manual(app: tauri::AppHandle, path: String, to: String, subject: String, body: String) -> Result<(), String> {
    check_mail_fields(&to, &subject)?;
    let p = inside_reports(&app, &path)?;
    let body: String = body.chars().take(1500).collect();
    let url = format!("mailto:{}?subject={}&body={}", url_encode(to.trim()).replace("%40", "@"), url_encode(subject.trim()), url_encode(&body.replace('\n', "\r\n")));
    shell_open(&[std::ffi::OsStr::new(&url)])?;
    let arg = format!("/select,{}", p.display());
    shell_open(&[std::ffi::OsStr::new(&arg)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_and_quantities() {
        assert_eq!(money(1234.5, "RD$"), "RD$ 1,234.50");
        assert_eq!(money(0.0, "€"), "€ 0.00");
        assert_eq!(money(1_000_000.0, ""), "1,000,000.00");
        assert_eq!(money(-5.0, "$"), "-$ 5.00");
        assert_eq!(qty(2.0), "2");
        assert_eq!(qty(1.5), "1.50");
        assert_eq!(days_label(365), "1 año");
        assert_eq!(days_label(90), "3 meses");
        assert_eq!(days_label(45), "45 días");
    }

    #[test]
    fn css_strings_cannot_break_out() {
        assert_eq!(css_str("A \"B\" \\ C"), "A \\\"B\\\" \\\\ C");
        assert!(!css_str("x</style>").contains('<'));
    }

    #[test]
    fn eml_is_well_formed() {
        let eml = build_eml("ana@example.com", "Informe ñ", "Hola\nAdjunto", "Informe.pdf", b"%PDF-1.4");
        assert!(eml.starts_with("X-Unsent: 1\r\n"));
        assert!(eml.contains("Subject: =?UTF-8?B?"));
        assert!(eml.contains("filename=\"Informe.pdf\""));
        assert!(eml.contains(&b64(b"%PDF-1.4")));
        assert!(check_mail_fields("a@b.c", "x\r\nBcc: z@z").is_err());
        assert!(check_mail_fields("no-es-correo", "x").is_err());
        assert!(check_mail_fields("", "x").is_ok());
        assert_eq!(url_encode("a b&c"), "a%20b%26c");
    }

    /// Informes de muestra con análisis reales para revisar el diseño:
    /// `ADMINOPS_SNAPSHOTS=<carpeta snapshots> ADMINOPS_SAMPLE_OUT=<carpeta> cargo test sample_reports -- --ignored`
    #[test]
    #[ignore]
    fn sample_reports() {
        use crate::workflow::{CatalogItem, Line};
        let dir = PathBuf::from(std::env::var("ADMINOPS_SNAPSHOTS").unwrap());
        let out = PathBuf::from(std::env::var("ADMINOPS_SAMPLE_OUT").unwrap());
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
        files.sort();
        let load = |p: &PathBuf| serde_json::from_str::<Diagnostics>(&std::fs::read_to_string(p).unwrap()).unwrap();
        let cur = load(files.last().unwrap());
        let base = load(&files[files.len().saturating_sub(4)]);
        let journal: Vec<Entry> = serde_json::from_str(r#"[
            {"id":1,"timestamp":1790000000,"op":"restorePoint","tweakId":null,"title":"Punto de restauración","ok":true},
            {"id":2,"timestamp":1790000100,"op":"run","tweakId":"cleanup.user-temp","title":"Limpiar archivos temporales","ok":true,"message":"1,2 GB liberados"},
            {"id":3,"timestamp":1790000200,"op":"apply","tweakId":"x","title":"Desactivar programas de inicio innecesarios","ok":true},
            {"id":4,"timestamp":1790000300,"op":"run","tweakId":"y","title":"Reparar archivos del sistema (SFC)","ok":true}]"#)
        .unwrap();
        let settings = Settings {
            technician: "David Bonilla".into(),
            company: "Soporte Técnico DB".into(),
            phone: "809-555-0100".into(),
            email: "soporte@example.com".into(),
            conditions: "El cliente declara haber recibido el equipo funcionando. Los datos son responsabilidad del cliente.".into(),
            catalog: vec![CatalogItem::default()],
            ..Default::default()
        };
        let line = |d: &str, part, q, p, w| Line { description: d.into(), part, qty: q, price: p, warranty_days: w };
        let input = |template, kind| ReportInput {
            baseline: None,
            client: Client { name: "Farmacia Central".into(), contact: "Ana Pérez".into(), phone: "809-555-0199".into(), email: "ana@example.com".into(), address: "Av. Principal 12".into(), ..Default::default() },
            technician: None,
            notes: "El equipo se calentaba y se apagaba solo. Se limpió el ventilador.".into(),
            checklist: vec![ChecklistItem { text: "Copia de seguridad".into(), done: true }, ChecklistItem { text: "Antivirus actualizado".into(), done: false }],
            since: Some(0),
            template,
            billing: Billing {
                kind,
                lines: vec![line("Diagnóstico y revisión general", false, 1.0, 800.0, 0), line("SSD Kingston 480 GB", true, 1.0, 3200.0, 365), line("Instalación de programas", false, 2.0, 350.0, 0)],
                discount: 200.0,
                payment: "Transferencia".into(),
            },
            problem: "El equipo va muy lento y a veces se apaga solo.".into(),
            recommendations: "Sustituir el disco D: (muestra fallos SMART) y hacer copia de seguridad cuanto antes.".into(),
            signature: None,
            signer: String::new(),
            warranties: crate::workflow::warranties(&Billing { kind, lines: vec![line("SSD Kingston 480 GB", true, 1.0, 3200.0, 365)], ..Default::default() }, 30, cur.timestamp),
            next_maintenance: crate::workflow::maintenance_date(6, cur.timestamp),
        };
        for (name, template, kind) in [("cliente", Template::Client, DocKind::Receipt), ("tecnico", Template::Technical, DocKind::Quote)] {
            let inp = input(template, kind);
            let html = build(&Ctx { cur: &cur, base: Some(&base), journal: &journal, technician: "David Bonilla", input: &inp, settings: &settings, speed: None, number: "2026-0042", since: 0 });
            std::fs::write(out.join(format!("{name}.html")), &html).unwrap();
            super::super::pdf::html_to_pdf(&html, &out.join(format!("{name}.pdf"))).unwrap();
        }
    }
}
