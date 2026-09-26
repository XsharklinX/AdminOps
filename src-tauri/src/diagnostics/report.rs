//! Informe HTML para el cliente: un solo archivo autocontenido, pensado para
//! abrirse en el navegador e imprimirse o guardarse como PDF.

use super::{latest_snapshot, load_snapshot, parse_time, Diagnostics, Severity};
use crate::tweaks::journal::{Entry, Op};
use crate::tweaks::TweakState;
use chrono::{DateTime, Local, TimeZone};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use tauri::State;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn local(ts: u64) -> DateTime<Local> {
    Local.timestamp_opt(ts as i64, 0).single().unwrap_or_else(Local::now)
}

fn fmt_ts(ts: u64) -> String {
    local(ts).format("%d/%m/%Y %H:%M").to_string()
}

fn fmt_iso(iso: &str) -> String {
    parse_time(iso).map_or_else(|| iso.to_string(), |d| d.format("%d/%m/%Y %H:%M").to_string())
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

use crate::paths::reports_dir;

const CSS: &str = r#"
*{box-sizing:border-box}body{margin:0;background:#f4f6f9;color:#1b2330;font:14px/1.5 "Segoe UI",system-ui,sans-serif}
main{max-width:900px;margin:0 auto;padding:40px 32px;background:#fff;min-height:100vh}
header{display:flex;justify-content:space-between;align-items:flex-end;border-bottom:3px solid #0bb5d6;padding-bottom:16px;margin-bottom:24px}
h1{margin:0;font-size:26px}h1 small{display:block;font-size:13px;font-weight:400;color:#5b6778;letter-spacing:.08em;text-transform:uppercase}
h2{font-size:15px;text-transform:uppercase;letter-spacing:.1em;color:#0a7f97;margin:32px 0 10px;border-bottom:1px solid #e3e8ef;padding-bottom:6px}
.meta{text-align:right;color:#5b6778;font-size:13px}.meta b{color:#1b2330}
table{width:100%;border-collapse:collapse;font-size:13px}th,td{text-align:left;padding:6px 8px;border-bottom:1px solid #eef1f5;vertical-align:top}
th{color:#5b6778;font-weight:600;font-size:11px;text-transform:uppercase;letter-spacing:.06em}
td.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.chips{display:flex;gap:10px;margin-bottom:12px}.chip{border-radius:8px;padding:8px 14px;font-weight:600}
.chip span{display:block;font-size:22px}.bad{background:#fdecef;color:#c0223f}.warn{background:#fff5e0;color:#9a6200}.ok{background:#e6f8ef;color:#12784a}.info{background:#e8f4fb;color:#1c6a93}
.f{display:flex;gap:10px;padding:8px 10px;border-radius:6px;margin-bottom:6px}.f b{display:block}.f small{color:#5b6778}
.dot{width:8px;height:8px;border-radius:50%;margin-top:7px;flex:none}.dot.bad{background:#e0284a}.dot.warn{background:#e39a00}.dot.info{background:#2a8fc4}
.better{color:#12784a;font-weight:600}.worse{color:#c0223f;font-weight:600}.muted{color:#8a95a5}
.notes{white-space:pre-wrap;background:#f7f9fb;border-left:3px solid #0bb5d6;padding:12px 14px;border-radius:4px}
footer{margin-top:40px;color:#8a95a5;font-size:12px;text-align:center}
header .brand{display:flex;align-items:center;gap:14px}header .brand svg{width:52px;height:52px;flex:none}
@page{size:A4;margin:14mm 12mm}
@media print{*{-webkit-print-color-adjust:exact;print-color-adjust:exact}body{background:#fff}main{padding:0;max-width:none;min-height:0}h2{break-after:avoid}tr,.f{break-inside:avoid}}
"#;

const LOGO: &str = include_str!("../../../src/assets/logo.svg");

struct Ctx<'a> {
    cur: &'a Diagnostics,
    base: Option<&'a Diagnostics>,
    journal: &'a [Entry],
    technician: &'a str,
    client: &'a str,
    notes: &'a str,
}

fn sev_class(s: Severity) -> &'static str {
    match s {
        Severity::Bad => "bad",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

fn comparison(h: &mut String, cur: &Diagnostics, base: &Diagnostics) {
    // (métrica, antes, después, delta legible, mejor si sube)
    let mut rows: Vec<(String, String, String, Option<(f64, bool)>, String)> = Vec::new();
    for v in &cur.volumes {
        if let Some(b) = base.volumes.iter().find(|b| b.mount == v.mount) {
            let delta = v.free as f64 - b.free as f64;
            // Menos de 100 MB es el ir y venir normal de temporales: no es un cambio.
            let delta = if delta.abs() < 100.0 * 1024.0 * 1024.0 { 0.0 } else { delta };
            rows.push((format!("Espacio libre en {}", v.mount), gb(b.free), gb(v.free), Some((delta, true)), format!("{:+.1} GB", delta / 1024f64.powi(3))));
        }
    }
    let count = |d: &Diagnostics, f: fn(&Diagnostics) -> Option<usize>| f(d);
    let metrics: [(&str, fn(&Diagnostics) -> Option<usize>, bool); 5] = [
        ("Programas que arrancan con Windows", |d| d.startup_enabled.data.as_ref().map(Vec::len), false),
        ("Apps promocionales instaladas", |d| d.bloat_installed.data.as_ref().map(Vec::len), false),
        ("Ajustes de optimización aplicados", |d| Some(d.tweaks_applied), true),
        ("Problemas críticos", |d| Some(d.findings.iter().filter(|f| f.severity == Severity::Bad).count()), false),
        ("Advertencias", |d| Some(d.findings.iter().filter(|f| f.severity == Severity::Warn).count()), false),
    ];
    for (label, get, up_is_better) in metrics {
        if let (Some(b), Some(c)) = (count(base, get), count(cur, get)) {
            let delta = c as f64 - b as f64;
            rows.push((label.into(), b.to_string(), c.to_string(), Some((delta, up_is_better)), format!("{:+}", c as i64 - b as i64)));
        }
    }
    let boot = |d: &Diagnostics| d.stability.data.as_ref()?.boot_times.as_ref()?.first().map(|b| b.ms);
    if let (Some(b), Some(c)) = (boot(base), boot(cur)) {
        if b != c {
            let delta = c as f64 - b as f64;
            rows.push(("Tiempo del último arranque".into(), format!("{:.1} s", b as f64 / 1000.0), format!("{:.1} s", c as f64 / 1000.0), Some((delta, false)), format!("{:+.1} s", delta / 1000.0)));
        }
    }

    let _ = write!(h, "<h2>Antes y después</h2><p class=muted>Comparado con el análisis del {}.</p><table><tr><th>Métrica</th><th>Antes</th><th>Después</th><th>Cambio</th></tr>", fmt_ts(base.timestamp));
    for (label, before, after, delta, text) in rows {
        let cls = match delta {
            Some((d, _)) if d.abs() < f64::EPSILON => "muted",
            Some((d, up)) if (d > 0.0) == up => "better",
            Some(_) => "worse",
            None => "muted",
        };
        let text = if cls == "muted" { "Sin cambios".to_string() } else { text };
        let _ = write!(h, "<tr><td>{}</td><td class=num>{}</td><td class=num>{}</td><td class=\"num {cls}\">{}</td></tr>", esc(&label), esc(&before), esc(&after), esc(&text));
    }
    h.push_str("</table>");
}

fn op_label(op: Op) -> &'static str {
    match op {
        Op::Apply => "Aplicado",
        Op::Revert => "Deshecho",
        Op::Run => "Ejecutado",
        Op::RestorePoint => "Punto de restauración",
    }
}

fn build(c: &Ctx) -> String {
    let d = c.cur;
    let mut h = String::with_capacity(32 * 1024);
    let _ = write!(
        h,
        "<!doctype html><html lang=es><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Informe {} — {}</title><style>{CSS}</style></head><body><main>",
        esc(&d.host),
        local(d.timestamp).format("%d/%m/%Y")
    );
    let _ = write!(
        h,
        "<header><div class=brand>{LOGO}<h1><small>Informe técnico · AdminOps</small>{}</h1></div><div class=meta>{}{}<div>Fecha: <b>{}</b></div></div></header>",
        esc(&d.host),
        if c.client.is_empty() { String::new() } else { format!("<div>Cliente: <b>{}</b></div>", esc(c.client)) },
        if c.technician.is_empty() { String::new() } else { format!("<div>Técnico: <b>{}</b></div>", esc(c.technician)) },
        fmt_ts(d.timestamp)
    );

    // Resumen
    let n = |s| d.findings.iter().filter(|f| f.severity == s).count();
    h.push_str("<h2>Resumen</h2><div class=chips>");
    let _ = write!(h, "<div class=\"chip bad\"><span>{}</span>Críticos</div>", n(Severity::Bad));
    let _ = write!(h, "<div class=\"chip warn\"><span>{}</span>Advertencias</div>", n(Severity::Warn));
    let _ = write!(h, "<div class=\"chip info\"><span>{}</span>Informativos</div></div>", n(Severity::Info));
    if d.findings.is_empty() {
        h.push_str("<div class=\"f ok\"><b>No se encontraron problemas.</b></div>");
    }
    for f in &d.findings {
        let _ = write!(
            h,
            "<div class=\"f {0}\"><div class=\"dot {0}\"></div><div><b>{1}</b>{2}</div></div>",
            sev_class(f.severity),
            esc(&f.title),
            f.detail.as_deref().map(|x| format!("<small>{} · {}</small>", esc(&f.area), esc(x))).unwrap_or_else(|| format!("<small>{}</small>", esc(&f.area)))
        );
    }

    if let Some(base) = c.base {
        comparison(&mut h, d, base);
    }

    // Trabajo realizado
    let work: Vec<&Entry> = c.journal.iter().filter(|e| e.ok).collect();
    if !work.is_empty() {
        h.push_str("<h2>Trabajo realizado</h2><table><tr><th>Fecha</th><th>Acción</th><th>Detalle</th></tr>");
        for e in work {
            let _ = write!(
                h,
                "<tr><td class=num>{}</td><td>{}</td><td>{}{}</td></tr>",
                fmt_ts(e.timestamp),
                op_label(e.op),
                esc(&e.title),
                e.message.as_deref().map(|m| format!(" <span class=muted>— {}</span>", esc(m))).unwrap_or_default()
            );
        }
        h.push_str("</table>");
    }

    // Equipo
    h.push_str("<h2>Equipo</h2><table>");
    let _ = write!(h, "<tr><th>Sistema</th><td>{}</td></tr><tr><th>Procesador</th><td>{}</td></tr><tr><th>Memoria</th><td>{}</td></tr>", esc(&d.os), esc(&d.cpu), gb(d.ram_total));
    if let Some(s) = &d.system.data {
        let _ = write!(h, "<tr><th>Instalado</th><td>{}</td></tr><tr><th>Último arranque</th><td>{}</td></tr>", fmt_iso(&s.install_date), fmt_iso(&s.last_boot));
    }
    h.push_str("</table><table style=margin-top:12px><tr><th>Unidad</th><th>Libre</th><th>Total</th><th>Uso</th></tr>");
    for v in &d.volumes {
        let used = if v.total > 0 { 100.0 - v.free as f64 * 100.0 / v.total as f64 } else { 0.0 };
        let _ = write!(h, "<tr><td>{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{used:.0}%</td></tr>", esc(&v.mount), gb(v.free), gb(v.total));
    }
    h.push_str("</table>");

    // Discos
    if let Some(disks) = &d.disks.data {
        h.push_str("<h2>Salud de discos</h2><table><tr><th>Disco</th><th>Tipo</th><th>Estado</th><th>Temp.</th><th>Desgaste</th><th>Horas</th></tr>");
        for k in disks {
            let _ = write!(
                h,
                "<tr><td>{}<div class=muted>{}</div></td><td>{} · {}</td><td>{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{}</td></tr>",
                esc(&k.name),
                gb(k.size),
                esc(&k.media_type),
                esc(&k.bus_type),
                if k.health.eq_ignore_ascii_case("Healthy") { "Saludable".to_string() } else { esc(&k.health) },
                k.temperature.map(|t| format!("{t} °C")).unwrap_or("—".into()),
                k.wear.map(|w| format!("{w}%")).unwrap_or("—".into()),
                k.power_on_hours.map(|p| p.to_string()).unwrap_or("—".into())
            );
        }
        h.push_str("</table>");
    }

    // Estabilidad
    if let Some(s) = &d.stability.data {
        let _ = write!(h, "<h2>Estabilidad (últimos {} días)</h2>", s.days);
        let _ = write!(h, "<p>{} pantallazos azules · {} apagados inesperados</p>", s.bugchecks.len(), s.unexpected_shutdowns.len());
        if !s.bugchecks.is_empty() {
            h.push_str("<table><tr><th>Fecha</th><th>Código</th><th>Posible causa</th></tr>");
            for b in &s.bugchecks {
                let _ = write!(h, "<tr><td class=num>{}</td><td>{} {}</td><td>{}</td></tr>", fmt_iso(&b.time), esc(&b.code), esc(b.name.as_deref().unwrap_or("")), esc(b.hint.as_deref().unwrap_or("")));
            }
            h.push_str("</table>");
        }
        if !s.crashes.is_empty() {
            h.push_str("<table style=margin-top:12px><tr><th>Aplicación que falla</th><th>Veces</th><th>Último fallo</th></tr>");
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
            h.push_str("<table><tr><th>Dispositivo</th><th>Problema</th><th>Código</th></tr>");
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
            "<h2>Batería</h2><table><tr><th>Capacidad</th><td>{:.0}% de la original ({} de {} mWh)</td></tr><tr><th>Ciclos</th><td>{}</td></tr><tr><th>Modelo</th><td>{} {} · {}</td></tr></table>",
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
    if let Some(s) = &d.system.data {
        h.push_str("<h2>Seguridad y mantenimiento</h2><table>");
        let av = if s.antivirus.is_empty() { "No detectado".to_string() } else { s.antivirus.join(", ") };
        let _ = write!(h, "<tr><th>Antivirus</th><td>{}</td></tr>", esc(&av));
        let _ = write!(h, "<tr><th>Protección en tiempo real (Defender)</th><td>{}</td></tr>", yes_no(s.defender_realtime, "Activa", "Desactivada"));
        let _ = write!(
            h,
            "<tr><th>Última actualización</th><td>{}</td></tr>",
            s.last_update.as_deref().map(|u| format!("{}{}", fmt_iso(u).split(' ').next().unwrap_or(""), s.last_update_id.as_deref().map(|i| format!(" ({})", esc(i))).unwrap_or_default())).unwrap_or("No disponible".into())
        );
        let _ = write!(h, "<tr><th>Reinicio pendiente</th><td>{}</td></tr>", if s.pending_reboot { "Sí" } else { "No" });
        let _ = write!(h, "<tr><th>Windows activado</th><td>{}</td></tr>", yes_no(s.activated, "Sí", "No"));
        let _ = write!(h, "<tr><th>Arranque seguro</th><td>{}</td></tr>", yes_no(s.secure_boot, "Activado", "Desactivado"));
        let _ = write!(h, "<tr><th>TPM</th><td>{}</td></tr></table>", yes_no(s.tpm_ready, "Listo", "No disponible o no listo"));
    }

    if !c.notes.trim().is_empty() {
        let _ = write!(h, "<h2>Observaciones del técnico</h2><div class=notes>{}</div>", esc(c.notes.trim()));
    }

    let _ = write!(h, "<footer>Generado con AdminOps el {}</footer></main></body></html>", Local::now().format("%d/%m/%Y %H:%M"));
    h
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

#[tauri::command(async)]
pub fn generate_report(
    app: tauri::AppHandle,
    state: State<'_, TweakState>,
    baseline: Option<u64>,
    technician: String,
    client: String,
    notes: String,
) -> Result<String, String> {
    let cur = latest_snapshot(&app).ok_or("Ejecuta un diagnóstico antes de generar el informe.")?;
    let base = baseline.filter(|b| *b != cur.timestamp).and_then(|b| load_snapshot(&app, b));
    // Sin base, el trabajo realizado es el de hoy.
    let since = base.as_ref().map_or_else(
        || Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|d| d.and_local_timezone(Local).single()).map_or(0, |d| d.timestamp() as u64),
        |b| b.timestamp,
    );
    let journal = state.journal_since(since);
    let html = build(&Ctx { cur: &cur, base: base.as_ref(), journal: &journal, technician: &technician, client: &client, notes: &notes });

    let dir = reports_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    let safe_host: String = cur.host.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let base_name = format!("AdminOps_{}_{}", safe_host, Local::now().format("%Y-%m-%d_%H%M"));
    let pdf = dir.join(format!("{base_name}.pdf"));
    let path = match super::pdf::html_to_pdf(&html, &pdf) {
        Ok(()) => pdf,
        Err(e) => {
            // Sin Edge (muy raro) el informe no se pierde: se guarda como HTML.
            let html_path = dir.join(format!("{base_name}.html"));
            std::fs::write(&html_path, &html).map_err(|w| format!("{e} · No se pudo guardar el HTML: {w}"))?;
            html_path
        }
    };
    shell_open(&[path.as_os_str()])?;
    Ok(path.display().to_string())
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
