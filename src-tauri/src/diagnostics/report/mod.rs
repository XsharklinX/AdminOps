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

mod format;
mod style;
mod compare;
mod sections;
mod mail;
#[allow(unused_imports)]
pub use format::*;
#[allow(unused_imports)]
pub use style::*;
#[allow(unused_imports)]
pub use compare::*;
#[allow(unused_imports)]
pub use sections::*;
#[allow(unused_imports)]
pub use mail::*;

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
        match self.input.template {
            Template::Technical => true,
            Template::Custom => self.settings.report_layout.technical,
            Template::Client => false,
        }
    }

    /// Las secciones del informe, en orden. Las dos plantillas de fábrica llevan
    /// todas; la propia, las que eligió el técnico.
    fn layout(&self) -> Vec<&'static str> {
        match self.input.template {
            Template::Custom => self.settings.report_layout.valid_sections(),
            _ => crate::workflow::REPORT_SECTIONS.to_vec(),
        }
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
         @bottom-left{{content:\"{} · Informe Nº {}\";font:8.5px \"Plex\",\"Segoe UI\",sans-serif;color:#8a95a5}}\
         @bottom-right{{content:\"Página \" counter(page) \" de \" counter(pages);font:8.5px \"Plex\",\"Segoe UI\",sans-serif;color:#8a95a5}}\
         @top-right{{content:\"{}\";font:8.5px \"Plex\",\"Segoe UI\",sans-serif;color:#8a95a5}}}}\
         @page:first{{@top-right{{content:none}}}}{}</style></head><body><main>",
        esc(&d.host),
        esc(c.number),
        css_str(&who),
        css_str(c.number),
        css_str(&format!("{} · {}", d.host, fmt_day(d.timestamp))),
        font_faces()
    );
    header(&mut h, c);

    // Presentación propia de este cliente (plantilla de su ficha).
    let intro = fill_fields(
        c.input.client.report.intro.trim(),
        &[
            ("cliente", c.input.client.name.as_str()),
            ("contacto", if c.input.client.contact.trim().is_empty() { c.input.client.name.as_str() } else { c.input.client.contact.as_str() }),
            ("numero", c.number),
            ("fecha", &fmt_day(c.cur.timestamp)),
            ("equipo", c.cur.host.as_str()),
            ("empresa", c.settings.company.as_str()),
            ("tecnico", c.technician),
        ],
    );
    if !intro.is_empty() {
        let _ = write!(h, "<div class=intro>{}</div>", esc(&intro));
    }

    let work: Vec<&Entry> = c.journal.iter().filter(|e| e.ok && e.op != Op::Revert).collect();
    // Cada sección, en el orden de la plantilla (las de fábrica: todas, en el de siempre).
    for section in c.layout() {
        match section {
            "summary" => {
                summary(&mut h, c, work.len());
                areas_section(&mut h, d);
            }
            "problem" if !c.input.problem.trim().is_empty() => {
                let _ = write!(h, "<h2>Motivo de la visita</h2><div class=text>{}</div>", esc(c.input.problem.trim()));
            }
            "work" => {
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
            }
            "findings" => {
                // Problemas resueltos y pendientes
                let solved = c.base.map(|b| resolved(d, b)).unwrap_or_default();
                let mut pending: Vec<&Finding> = d.findings.iter().filter(|f| c.technical() || important(f)).collect();
                pending.sort_by_key(|f| match f.severity {
                    Severity::Bad => 0,
                    Severity::Warn => 1,
                    Severity::Info => 2,
                });
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
                            pending_row(&mut h, f, c.technical());
                        }
                        h.push_str("</div>");
                    }
                    if both {
                        h.push_str("</div></div>");
                    }
                }
            }
            "comparison" => {
                if let Some(base) = c.base {
                    comparison(&mut h, c, base);
                }
            }
            "recommendations" => {
                // Recomendaciones
                if !c.input.recommendations.trim().is_empty() {
                    let _ = write!(h, "<h2>Recomendaciones</h2><div class=text>{}</div>", esc(c.input.recommendations.trim()));
                }
            }
            "billing" => {
                billing(&mut h, c);
                warranty_and_maintenance(&mut h, c);
            }
            "machine" => {
                if c.technical() {
                    technical_sections(&mut h, d);
                } else {
                    machine_summary(&mut h, d);
                }
            }
            "speed" => {
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
            }
            "notes" if !c.input.notes.trim().is_empty() => {
                let _ = write!(h, "<h2>Observaciones del técnico</h2><div class=text>{}</div>", esc(c.input.notes.trim()));
            }
            _ => {}
        }
    }

    signatures(&mut h, c);

    if !st.conditions.trim().is_empty() {
        let _ = write!(h, "<div class=conditions>{}</div>", esc(st.conditions.trim()));
    }
    let _ = write!(
        h,
        "<p class=about>Los datos de este informe se obtuvieron del propio equipo el {} con AdminOps {}. Las temperaturas, el espacio y el estado de los discos son los del momento del análisis. Informe Nº {}.</p>",
        fmt_ts(d.timestamp),
        env!("CARGO_PKG_VERSION"),
        esc(c.number)
    );
    let _ = write!(h, "<footer>Generado el {} con AdminOps</footer></main></body></html>", Local::now().format("%d/%m/%Y %H:%M"));
    h
}

/// Rellena {cliente}, {fecha}… en los textos de la plantilla del cliente. Los
/// campos desconocidos se dejan tal cual.
fn fill_fields(text: &str, values: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (k, v) in values {
        out = out.replace(&format!("{{{k}}}"), v.trim());
    }
    out
}

impl Ctx<'_> {
    fn checklist_done(&self) -> usize {
        self.input.checklist.iter().filter(|c| c.done).count()
    }
}

/// Abre un archivo con la app predeterminada. Se lanza a través de explorer.exe
/// para que el navegador NO herede los privilegios de administrador de AdminOps.
pub(crate) fn shell_open(args: &[&std::ffi::OsStr]) -> Result<(), String> {
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

/// El HTML del informe (el que luego pasa a PDF) con el número dado.
fn report_html(app: &tauri::AppHandle, state: &TweakState, input: &ReportInput, number: &str) -> Result<(String, Diagnostics), String> {
    let cur = latest_snapshot(app).ok_or("Ejecuta un diagnóstico antes de generar el informe.")?;
    let base = input.baseline.filter(|b| *b != cur.timestamp).and_then(|b| load_snapshot(app, b));
    let today = || Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|d| d.and_local_timezone(Local).single()).map_or(0, |d| d.timestamp() as u64);
    let since = input.since.or(base.as_ref().map(|b| b.timestamp)).unwrap_or_else(today);
    let journal = state.journal_since(since);
    let settings = crate::workflow::settings(app);
    let technician = input.technician.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| settings.technician.clone());
    // Último test de velocidad del periodo del informe.
    let speed = crate::network::speedtest::history(app).into_iter().find(|r| r.timestamp >= since);
    let html = build(&Ctx {
        cur: &cur,
        base: base.as_ref(),
        journal: &journal,
        technician: technician.trim(),
        input,
        settings: &settings,
        speed: speed.as_ref(),
        number,
        since,
    });
    Ok((html, cur))
}

/// Genera el informe PDF (o HTML si falta Edge), lo abre y devuelve su ruta y número.
pub fn create_report(app: &tauri::AppHandle, state: &TweakState, input: ReportInput) -> Result<Created, String> {
    input.billing.validate()?;
    let number = crate::workflow::next_number(app);
    let (html, cur) = report_html(app, state, &input, &number)?;

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

/// El diario de cambios de este equipo en un PDF, como constancia de lo hecho.
/// `ids`: solo esas entradas (lo que se ve con el filtro puesto); vacío: todas.
/// Devuelve el nombre del archivo guardado, o None si se cancela.
#[tauri::command(async)]
pub fn export_journal_pdf(app: tauri::AppHandle, state: State<'_, TweakState>, ids: Vec<u64>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut entries = state.journal_since(0);
    if !ids.is_empty() {
        entries.retain(|e| ids.contains(&e.id));
    }
    if entries.is_empty() {
        return Err("No hay cambios que exportar.".into());
    }
    entries.reverse(); // lo más reciente, arriba
    let settings = crate::workflow::settings(&app);
    let host = sysinfo::System::host_name().unwrap_or_default();
    let html = journal_html(&entries, &host, &settings);
    let safe_host: String = host.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let name = format!("Diario_{safe_host}_{}.pdf", Local::now().format("%Y-%m-%d"));
    let Some(out) = app.dialog().file().set_file_name(&name).add_filter("PDF", &["pdf"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    super::pdf::html_to_pdf(&html, &out)?;
    let _ = shell_open(&[out.as_os_str()]);
    log::info!("Diario de cambios exportado a PDF ({} entradas)", entries.len());
    Ok(out.file_name().map(|n| n.to_string_lossy().into_owned()))
}

/// El diario como documento: quién, qué equipo, y cada cambio con su resultado.
fn journal_html(entries: &[Entry], host: &str, settings: &Settings) -> String {
    let who = if settings.company.trim().is_empty() { "AdminOps".to_string() } else { settings.company.trim().to_string() };
    let failed = entries.iter().filter(|e| !e.ok).count();
    let undone = entries.iter().filter(|e| e.reverted).count();
    let mut h = String::with_capacity(8 * 1024 + entries.len() * 220);
    let _ = write!(
        h,
        "<!doctype html><html lang=es><head><meta charset=utf-8><title>Diario de cambios — {}</title><style>{CSS}\
         @page{{size:A4;margin:14mm 13mm 16mm;\
         @bottom-left{{content:\"{} · Diario de cambios de {}\";font:8.5px \"Plex\",\"Segoe UI\",sans-serif;color:#8a95a5}}\
         @bottom-right{{content:\"Página \" counter(page) \" de \" counter(pages);font:8.5px \"Plex\",\"Segoe UI\",sans-serif;color:#8a95a5}}}}{}</style></head><body><main>",
        esc(host),
        css_str(&who),
        css_str(host),
        font_faces()
    );
    let _ = write!(h, "<h1>Diario de cambios</h1><p class=muted>Equipo {} · {}", esc(host), esc(&who));
    if !settings.technician.trim().is_empty() {
        let _ = write!(h, " · {}", esc(settings.technician.trim()));
    }
    let (first, last) = (entries.last().map_or(0, |e| e.timestamp), entries.first().map_or(0, |e| e.timestamp));
    let _ = write!(
        h,
        "</p><p>{} {} entre el {} y el {}{}{}.</p>",
        entries.len(),
        if entries.len() == 1 { "cambio" } else { "cambios" },
        fmt_day(first),
        fmt_day(last),
        if failed > 0 { format!(" · {failed} con error") } else { String::new() },
        if undone > 0 { format!(" · {undone} deshechos después") } else { String::new() }
    );
    h.push_str("<table><thead><tr><th>Fecha</th><th>Acción</th><th>Cambio</th><th>Resultado</th></tr></thead>");
    for e in entries {
        let result = if !e.ok {
            "Error"
        } else if e.reverted {
            "Deshecho"
        } else {
            "Correcto"
        };
        let _ = write!(
            h,
            "<tr><td class=nowrap>{}</td><td class=nowrap>{}</td><td>{}{}</td><td class=nowrap>{result}</td></tr>",
            fmt_ts(e.timestamp),
            op_label(e.op),
            esc(&e.title),
            e.message.as_deref().filter(|m| !m.trim().is_empty()).map(|m| format!(" <span class=muted>— {}</span>", esc(m))).unwrap_or_default()
        );
    }
    let _ = write!(
        h,
        "</table><p class=about>Cambios hechos con AdminOps {} en este equipo. Generado el {}.</p></main></body></html>",
        env!("CARGO_PKG_VERSION"),
        Local::now().format("%d/%m/%Y %H:%M")
    );
    h
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

/// Lo que se elige en la pantalla del informe, convertido en lo que necesita el informe.
fn report_input(app: &tauri::AppHandle, options: &ReportOptions) -> ReportInput {
    use crate::workflow::{find_client, maintenance_date, warranties};
    let settings = crate::workflow::settings(app);
    let found = options.client_id.as_deref().and_then(|id| find_client(app, id));
    let archive = options.archive && found.is_some();
    let now = Local::now().timestamp() as u64;
    ReportInput {
        baseline: options.baseline,
        client: found.unwrap_or(Client { name: options.client.trim().into(), ..Default::default() }),
        technician: Some(options.technician.clone()),
        notes: options.notes.clone(),
        checklist: vec![],
        since: None,
        template: options.template,
        billing: options.billing.clone(),
        problem: options.problem.clone(),
        recommendations: options.recommendations.clone(),
        signature: None,
        signer: String::new(),
        warranties: if archive { warranties(&options.billing, settings.labor_warranty_days, now) } else { vec![] },
        next_maintenance: if archive { maintenance_date(settings.maintenance_months, now) } else { None },
    }
}

/// La vista previa del informe mientras se prepara: el mismo HTML que irá al
/// PDF, sin crear el archivo, sin gastar número y sin guardar nada en el cliente.
/// Usa el último análisis (el PDF analiza de nuevo justo antes de crearse).
#[tauri::command(async)]
pub fn preview_report(app: tauri::AppHandle, state: State<'_, TweakState>, options: ReportOptions) -> Result<String, String> {
    let input = report_input(&app, &options);
    report_html(&app, &state, &input, "BORRADOR").map(|(html, _)| html)
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

pub(crate) fn url_encode(s: &str) -> String {
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
mod fill_tests {
    #[test]
    fn fills_client_template_fields() {
        let t = super::fill_fields("Visita a {cliente} ({equipo}) el {fecha}. {otro}", &[("cliente", "Sol"), ("equipo", "PC1"), ("fecha", "3/10/2026")]);
        assert_eq!(t, "Visita a Sol (PC1) el 3/10/2026. {otro}");
    }
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

    fn sample_input(template: Template) -> ReportInput {
        ReportInput {
            baseline: None,
            client: Client { name: "Farmacia Central".into(), ..Default::default() },
            technician: None,
            notes: "Se limpió el ventilador.".into(),
            checklist: vec![],
            since: Some(0),
            template,
            billing: Billing::default(),
            problem: "Va muy lento.".into(),
            recommendations: "Cambiar el disco.".into(),
            signature: None,
            signer: String::new(),
            warranties: vec![],
            next_maintenance: None,
        }
    }

    /// La plantilla propia lleva solo las secciones elegidas y en su orden; las
    /// de fábrica siguen llevando todas, en el de siempre.
    #[test]
    fn custom_template_chooses_sections_and_order() {
        let cur = Diagnostics { host: "PC-01".into(), ..Default::default() };
        let render = |template, settings: &Settings| {
            let input = sample_input(template);
            build(&Ctx { cur: &cur, base: None, journal: &[], technician: "Ana", input: &input, settings, speed: None, number: "2026-0001", since: 0 })
        };
        let at = |html: &str, what: &str| html.find(what);

        let factory = render(Template::Client, &Settings::default());
        let (problem, advice, notes) = (at(&factory, "<h2>Motivo de la visita</h2>"), at(&factory, "<h2>Recomendaciones</h2>"), at(&factory, "<h2>Observaciones del técnico</h2>"));
        assert!(problem.is_some() && problem < advice && advice < notes);

        let mine = Settings {
            report_layout: crate::workflow::ReportLayout { name: "Corta".into(), technical: false, sections: vec!["recommendations".into(), "problem".into(), "inventada".into(), "problem".into()] },
            ..Default::default()
        };
        let custom = render(Template::Custom, &mine);
        assert!(at(&custom, "<h2>Recomendaciones</h2>") < at(&custom, "<h2>Motivo de la visita</h2>"));
        assert_eq!(custom.matches("<h2>Motivo de la visita</h2>").count(), 1, "una sección repetida sale una vez");
        assert!(!custom.contains("Observaciones del técnico"));
        // Sin ninguna sección sigue siendo un informe válido: cabecera y cierre.
        let empty = Settings { report_layout: crate::workflow::ReportLayout { sections: vec![], ..Default::default() }, ..Default::default() };
        let bare = render(Template::Custom, &empty);
        assert!(bare.contains("Farmacia Central") && bare.ends_with("</html>") && !bare.contains("<h2>Recomendaciones</h2>"));
        // Con el detalle técnico activado, la plantilla propia lo lleva.
        let tech = Settings { report_layout: crate::workflow::ReportLayout { technical: true, ..Default::default() }, ..Default::default() };
        assert!(render(Template::Custom, &tech).contains("<h2>Equipo en detalle</h2>"));
        assert!(!render(Template::Custom, &Settings::default()).contains("<h2>Equipo en detalle</h2>"));
    }

    #[test]
    fn journal_document_lists_every_change() {
        let entries: Vec<Entry> = serde_json::from_str(
            r#"[{"id":2,"timestamp":1790000100,"op":"run","tweakId":null,"title":"Limpiar <temporales>","ok":true,"message":"1,2 GB liberados"},
                {"id":1,"timestamp":1790000000,"op":"apply","tweakId":"x","title":"Desactivar un servicio","ok":false,"message":"Acceso denegado"}]"#,
        )
        .unwrap();
        let html = journal_html(&entries, "PC-01", &Settings { company: "Soporte DB".into(), ..Default::default() });
        assert!(html.contains("Diario de cambios") && html.contains("PC-01") && html.contains("Soporte DB"));
        assert!(html.contains("2 cambios") && html.contains("1 con error"));
        assert!(html.contains("Limpiar &lt;temporales&gt;") && html.contains("1,2 GB liberados"));
        assert!(html.contains(">Error<") && html.contains(">Correcto<"));
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

    /// El cuadro por áreas: lo que falta sale como «sin datos», no como «bien».
    #[test]
    fn areas_tell_the_truth() {
        let mut d = Diagnostics { ram_total: 4 * 1024u64.pow(3), ..Default::default() };
        d.volumes = vec![super::super::Volume { mount: std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()) + "\\", total: 100 * 1024u64.pow(3), free: 5 * 1024u64.pow(3) }];
        d.drivers = super::super::Section { data: Some(vec![]), error: None };
        let a = areas(&d);
        let get = |n: &str| a.iter().find(|x| x.name == n).unwrap().clone();
        assert_eq!(get("Espacio").level, "bad", "5 % libre");
        assert_eq!(get("Memoria").level, "warn", "4 GB");
        assert_eq!(get("Dispositivos").level, "ok");
        assert_eq!(get("Discos").level, "na", "sin datos no es «sano»");
        assert_eq!(get("Seguridad").level, "na");
        assert!(a.iter().all(|x| x.name != "Batería"), "sin batería no se enseña");
        let r = ring(85);
        assert!(r.contains("#12784a") && r.contains(">85<"));
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
            checklist: vec![ChecklistItem { text: "Copia de seguridad".into(), done: true, ..Default::default() }, ChecklistItem { text: "Antivirus actualizado".into(), done: false, ..Default::default() }],
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
