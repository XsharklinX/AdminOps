//! Lo que hace que el informe dé gusto entregar (1.2.9): una portada con la
//! marca, el veredicto en un semáforo grande, lo más urgente por orden, y
//! gráficas sencillas del disco y del arranque. «Una página» usa lo mismo en pequeño.

use super::*;

/// El veredicto del informe: clase de color, título y las frases que lo explican.
pub(super) fn verdict(c: &Ctx, work: usize) -> (&'static str, &'static str, Vec<String>) {
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
    (cls, title, text)
}

/// Lo pendiente que importa, lo urgente primero.
pub(super) fn top_pending<'a>(c: &'a Ctx, max: usize) -> Vec<&'a Finding> {
    let mut v: Vec<&Finding> = c.cur.findings.iter().filter(important).collect();
    v.sort_by_key(|f| match f.severity {
        Severity::Bad => 0,
        Severity::Warn => 1,
        Severity::Info => 2,
    });
    v.retain(|f| f.severity != Severity::Info);
    v.truncate(max);
    v
}

fn prio_tag(s: Severity) -> &'static str {
    match s {
        Severity::Bad => "<span class=\"prio bad\">Urgente</span>",
        Severity::Warn => "<span class=\"prio warn\">Recomendado</span>",
        Severity::Info => "",
    }
}

/// La forma del semáforo además del color: ✓ bien, ! aviso, ■ problema.
fn symbol(cls: &str) -> &'static str {
    match cls {
        "v-ok" => "✓",
        "v-warn" => "!",
        _ => "■",
    }
}

/// La portada: marca, equipo, semáforo con tres frases y lo más urgente.
pub(super) fn cover(h: &mut String, c: &Ctx, work: usize) {
    let st = c.settings;
    let d = c.cur;
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
    let kind = if c.technical() { "Informe técnico" } else { "Informe de revisión" };
    let model = d.hardware.data.as_ref().map(|hw| format!("{} {}", hw.manufacturer.trim(), hw.model.trim()).trim().to_string()).unwrap_or_default();
    let (cls, title, text) = verdict(c, work);
    let _ = write!(
        h,
        "<section class=cover><div class=cv-brand>{logo}<div><div class=name>{name}</div><div class=contact>{}</div></div></div>\
         <div class=cv-main><div class=cv-kind>{kind}</div><h1 class=cv-host>{}</h1>{}\
         <div class=\"cv-light {cls}\"><span class=cv-sym>{}</span><div><div class=cv-t>{title}</div><ul>",
        contact.join(" · "),
        esc(&d.host),
        if model.is_empty() { String::new() } else { format!("<div class=cv-model>{}</div>", esc(&model)) },
        symbol(cls),
    );
    for s in text.iter().take(3) {
        let _ = write!(h, "<li>{}</li>", esc(s));
    }
    h.push_str("</ul></div></div>");
    let top = top_pending(c, 3);
    if !top.is_empty() {
        h.push_str("<div class=cv-top><h3>Lo más importante</h3><ol>");
        for f in top {
            let _ = write!(h, "<li>{}<b>{}</b></li>", prio_tag(f.severity), esc(&f.title));
        }
        h.push_str("</ol></div>");
    }
    let client = c.input.client.name.trim();
    let _ = write!(
        h,
        "</div><div class=cv-foot><div><span>Cliente</span><b>{}</b></div><div><span>Fecha</span><b>{}</b></div><div><span>Informe Nº</span><b>{}</b></div><div><span>Técnico</span><b>{}</b></div></div></section>",
        if client.is_empty() { "—".to_string() } else { esc(client) },
        fmt_day(d.timestamp),
        esc(c.number),
        if c.technician.is_empty() { "—".to_string() } else { esc(c.technician) },
    );
}

/// Lo pendiente con su prioridad, para «Una página».
pub(super) fn top_issues(h: &mut String, c: &Ctx, max: usize) {
    let top = top_pending(c, max);
    if top.is_empty() {
        return;
    }
    h.push_str("<h2>Lo que conviene hacer</h2>");
    for f in top {
        pending_row(h, f, false);
    }
}

fn bar(cls: &str, pct: f64) -> String {
    format!("<div class=bar><i class={cls} style=\"width:{:.0}%\"></i></div>", pct.clamp(0.0, 100.0))
}

/// Gráficas sencillas: lo lleno que está cada disco y cuánto tarda en arrancar Windows.
pub(super) fn charts(h: &mut String, c: &Ctx) {
    let d = c.cur;
    let mut rows = String::new();
    for v in d.volumes.iter().filter(|v| v.total > 0).take(4) {
        let free = v.free as f64 * 100.0 / v.total as f64;
        let cls = if free < 10.0 { "bad" } else if free < 20.0 { "warn" } else { "ok" };
        let _ = write!(
            rows,
            "<div class=crow><div class=cl>{}<small>{} libres de {}</small></div>{}<b class=cv>{:.0} %</b></div>",
            esc(v.mount.trim_end_matches('\\')),
            gb(v.free),
            gb(v.total),
            bar(cls, 100.0 - free),
            100.0 - free
        );
    }
    if let Some(ms) = d.stability.data.as_ref().and_then(|s| crate::diagnostics::typical_boot_ms(s.boot_times.as_deref().unwrap_or_default())) {
        let secs = ms as f64 / 1000.0;
        let cls = if secs < 30.0 { "ok" } else if secs < 60.0 { "warn" } else { "bad" };
        let _ = write!(rows, "<div class=crow><div class=cl>Arranque de Windows<small>lo normal en este equipo (escala hasta 2 min)</small></div>{}<b class=cv>{secs:.0} s</b></div>", bar(cls, secs / 120.0 * 100.0));
    }
    if rows.is_empty() {
        return;
    }
    let _ = write!(h, "<h2>Disco y arranque</h2><div class=charts>{rows}</div>");
}
