//! Las secciones del informe: cabecera, resumen, áreas, cobro, firmas y detalle técnico.

use super::*;

pub(super) fn header(h: &mut String, c: &Ctx) {
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

pub(super) fn summary(h: &mut String, c: &Ctx, work: usize) {
    let d = c.cur;
    let n = |s| d.findings.iter().filter(|f| f.severity == s).count();
    let (bad, warn) = (n(Severity::Bad), n(Severity::Warn));
    let (cls, title, text) = verdict(c, work);
    let solved = c.base.map(|b| resolved(d, b).len());
    let _ = write!(h, "<div class=\"verdict {cls}\"><div class=state><div class=t>{title}</div><p>{}</p></div><div class=kpis>", esc(&text.join(" ")));
    if let Some(a) = &d.security.data {
        h.push_str(&ring(a.score));
    }
    if let Some(s) = solved {
        let _ = write!(h, "<div class=kpi><b>{s}</b><span>Resueltos</span></div>");
    }
    let _ = write!(h, "<div class=kpi><b>{}</b><span>Pendientes</span></div><div class=kpi><b>{work}</b><span>Acciones</span></div></div></div>", bad + warn);
}

/// Nota de seguridad en un anillo (verde, ámbar o rojo según la nota).
pub(super) fn ring(score: u32) -> String {
    let s = score.min(100) as f64;
    let color = if s >= 80.0 { "#12784a" } else if s >= 60.0 { "#b06f00" } else { "#c0223f" };
    let len = 2.0 * std::f64::consts::PI * 22.0;
    format!(
        "<div class=ring><svg viewBox=\"0 0 52 52\"><circle cx=26 cy=26 r=22 fill=none stroke=\"#e8ecf1\" stroke-width=5 /><circle cx=26 cy=26 r=22 fill=none stroke=\"{color}\" stroke-width=5 stroke-linecap=round stroke-dasharray=\"{:.1} {len:.1}\" transform=\"rotate(-90 26 26)\" /><text x=26 y=30.5 text-anchor=middle font-size=13 font-weight=600 fill=\"#1b2330\">{score}</text></svg><span>Seguridad /100</span></div>",
        len * s / 100.0
    )
}

/// Estado de una parte del equipo para el cuadro «Estado por áreas».
#[derive(Debug, Clone, PartialEq)]
pub struct AreaState {
    pub name: &'static str,
    /// ok | warn | bad | na
    pub level: &'static str,
    pub text: String,
}

/// El disco del informe en el formato de la página Discos, para dar el mismo veredicto.
pub(super) fn as_disk(k: &crate::diagnostics::collect::PhysicalDisk, smart: Option<&crate::hardware::smart::SmartDisk>) -> crate::disks::Disk {
    crate::disks::Disk {
        model: k.name.clone(),
        bus: k.bus_type.clone(),
        media: k.media_type.clone(),
        size: k.size,
        health: k.health.clone(),
        temperature: k.temperature.map_or(-1, i64::from),
        hours: k.power_on_hours.map_or(-1, |h| h as i64),
        read_errors: k.read_errors.map_or(-1, |e| e as i64),
        write_errors: k.write_errors.map_or(-1, |e| e as i64),
        wear: k.wear.map_or(-1, i64::from),
        predict_failure: smart.is_some_and(|s| s.predict_failure),
        reallocated: smart.and_then(|s| s.reallocated),
        pending: smart.and_then(|s| s.pending),
        uncorrectable: smart.and_then(|s| s.uncorrectable),
        crc_errors: smart.and_then(|s| s.crc_errors),
        ..Default::default()
    }
}

pub(super) fn disk_verdicts(d: &Diagnostics) -> Vec<(String, crate::disks::Verdict)> {
    d.disks
        .data
        .iter()
        .flatten()
        .map(|k| {
            let smart = d.smart.data.iter().flatten().find(|s| crate::disks::same_model(&s.model, &k.name));
            (k.name.clone(), crate::disks::verdict(&as_disk(k, smart)))
        })
        .collect()
}

/// Cómo está cada parte del equipo, en una línea. Lo que no se pudo medir sale como «—».
pub fn areas(d: &Diagnostics) -> Vec<AreaState> {
    let mut out = Vec::new();
    let a = |name, level, text: String| AreaState { name, level, text };

    // Discos: el peor veredicto de todos.
    let verdicts = disk_verdicts(d);
    out.push(match verdicts.iter().max_by_key(|(_, v)| match v.level.as_str() { "bad" => 2, "warn" => 1, _ => 0 }) {
        None => a("Discos", "na", "No se pudo leer".into()),
        Some((_, v)) if v.level == "ok" => a("Discos", "ok", if verdicts.len() == 1 { "Sano".into() } else { format!("Los {} sanos", verdicts.len()) }),
        Some((name, v)) => a("Discos", if v.level == "bad" { "bad" } else { "warn" }, format!("{}: {}", name.trim(), v.title.to_lowercase())),
    });

    // Espacio del disco del sistema.
    let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    out.push(match d.volumes.iter().find(|v| v.mount.to_uppercase().starts_with(&sys)).filter(|v| v.total > 0) {
        None => a("Espacio", "na", "—".into()),
        Some(v) => {
            let pct = v.free as f64 * 100.0 / v.total as f64;
            let level = if pct < 10.0 { "bad" } else if pct < 20.0 { "warn" } else { "ok" };
            a("Espacio", level, format!("{} libres ({pct:.0} %)", gb(v.free)))
        }
    });

    // Seguridad.
    out.push(match &d.security.data {
        None => a("Seguridad", "na", "—".into()),
        Some(s) => a("Seguridad", if s.score >= 80 { "ok" } else if s.score >= 60 { "warn" } else { "bad" }, format!("{}/100 · {}", s.score, if s.score >= 80 { "bien protegido" } else if s.score >= 60 { "mejorable" } else { "en riesgo" })),
    });

    // Actualizaciones de programas y de Windows.
    let pending_programs = d.software_updates.data.as_ref().map(Vec::len);
    let reboot = d.system.data.as_ref().is_some_and(|s| s.pending_reboot);
    out.push(match (pending_programs, reboot) {
        (None, false) => a("Actualizaciones", "na", "—".into()),
        (n, reboot) => {
            let n = n.unwrap_or(0);
            let mut t = if n == 0 { "Programas al día".to_string() } else { format!("{n} {} por actualizar", if n == 1 { "programa" } else { "programas" }) };
            if reboot {
                t.push_str(" · reinicio pendiente");
            }
            a("Actualizaciones", if n > 0 || reboot { "warn" } else { "ok" }, t)
        }
    });

    // Estabilidad: pantallazos azules y apagados inesperados.
    out.push(match &d.stability.data {
        None => a("Estabilidad", "na", "—".into()),
        Some(s) => {
            let n = s.bugchecks.len() + s.unexpected_shutdowns.len();
            let level = match n {
                0 => "ok",
                1 | 2 => "warn",
                _ => "bad",
            };
            a("Estabilidad", level, if n == 0 { format!("Sin cuelgues en {} días", s.days) } else { format!("{n} cuelgues o apagados en {} días", s.days) })
        }
    });

    // Dispositivos.
    out.push(match &d.drivers.data {
        None => a("Dispositivos", "na", "—".into()),
        Some(v) if v.is_empty() => a("Dispositivos", "ok", "Todos funcionan".into()),
        Some(v) => a("Dispositivos", "warn", format!("{} con problemas", v.len())),
    });

    // Memoria: prueba y cantidad.
    let ram_gb = d.ram_total as f64 / 1024f64.powi(3);
    out.push(match &d.memory_test.data {
        Some(Some(m)) if !m.passed => a("Memoria", "bad", "La prueba de memoria dio errores".into()),
        _ if d.ram_total > 0 && ram_gb < 7.5 => a("Memoria", "warn", format!("{ram_gb:.0} GB: justa para Windows 11")),
        _ if d.ram_total > 0 => a("Memoria", "ok", format!("{ram_gb:.0} GB")),
        _ => a("Memoria", "na", "—".into()),
    });

    // Temperatura del procesador.
    out.push(match d.temperatures.data.as_ref().and_then(|t| t.cpu) {
        None => a("Temperatura", "na", "—".into()),
        Some(c) => a("Temperatura", if c >= 90.0 { "bad" } else if c >= 80.0 { "warn" } else { "ok" }, format!("Procesador a {c:.0} °C")),
    });

    // Batería (solo portátiles).
    if let Some(Some(b)) = &d.battery.data {
        let h = b.health();
        out.push(a("Batería", if h < 60.0 { "bad" } else if h < 80.0 { "warn" } else { "ok" }, format!("{h:.0} % de su capacidad original")));
    }
    out
}

pub(super) fn areas_section(h: &mut String, d: &Diagnostics) {
    let list = areas(d);
    if list.iter().all(|x| x.level == "na") {
        return;
    }
    h.push_str("<h2>Estado por áreas</h2><div class=areas>");
    for x in &list {
        let label = match x.level {
            "ok" => "Bien",
            "warn" => "Mejorable",
            "bad" => "Atención",
            _ => "Sin datos",
        };
        let _ = write!(h, "<div class=\"area a-{}\"><div class=n>{}<span class=s>{label}</span></div><p>{}</p></div>", x.level, x.name, esc(&x.text));
    }
    h.push_str("</div>");
}

/// Un pendiente con su prioridad: lo urgente y lo recomendable se distinguen de un vistazo.
pub(super) fn pending_row(h: &mut String, f: &Finding, detail: bool) {
    let tag = match f.severity {
        Severity::Bad => "<span class=\"prio bad\">Urgente</span>",
        Severity::Warn => "<span class=\"prio warn\">Recomendado</span>",
        Severity::Info => "",
    };
    let small = match (&f.detail, detail) {
        (Some(x), true) => format!("<small>{} · {}</small>", esc(&f.area), esc(x)),
        _ => format!("<small>{}</small>", esc(&f.area)),
    };
    let _ = write!(h, "<div class=f><div class=\"dot {}\"></div><div>{tag}<b>{}</b>{small}</div></div>", sev_class(f.severity), esc(&f.title));
}

pub(super) fn finding_row(h: &mut String, f: &Finding, detail: bool, dot: &str) {
    let small = match (&f.detail, detail) {
        (Some(x), true) => format!("<small>{} · {}</small>", esc(&f.area), esc(x)),
        _ => format!("<small>{}</small>", esc(&f.area)),
    };
    let _ = write!(h, "<div class=f><div class=\"dot {dot}\"></div><div><b>{}</b>{small}</div></div>", esc(&f.title));
}

pub(super) fn billing(h: &mut String, c: &Ctx) {
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

pub(super) fn days_label(days: u32) -> String {
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

pub(super) fn warranty_and_maintenance(h: &mut String, c: &Ctx) {
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

pub(super) fn signatures(h: &mut String, c: &Ctx) {
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
pub(super) fn machine_summary(h: &mut String, d: &Diagnostics) {
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

pub(super) fn technical_sections(h: &mut String, d: &Diagnostics) {
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
        h.push_str("<h3>Salud de los discos</h3><table><thead><tr><th>Disco</th><th>Tipo</th><th>Veredicto</th><th class=num>Temp.</th><th class=num>Desgaste</th><th class=num>Horas</th></tr></thead>");
        let verdicts = disk_verdicts(d);
        for k in disks {
            let health = match verdicts.iter().find(|(n, _)| *n == k.name).map(|(_, v)| v) {
                Some(v) if v.level == "ok" => "<span class=better>Sano</span>".to_string(),
                Some(v) => format!("<span class={}>{}</span>", if v.level == "bad" { "worse" } else { "warn" }, esc(&v.title)),
                None => esc(&k.health),
            };
            let _ = write!(
                h,
                "<tr><td>{}<div class=muted>{}</div></td><td>{}</td><td>{health}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{}</td></tr>",
                esc(&k.name),
                gb(k.size),
                // «Unspecified» no le dice nada a nadie; el desgaste solo tiene sentido en un SSD.
                esc(&[k.media_type.as_str(), k.bus_type.as_str()].into_iter().filter(|x| !x.is_empty() && !x.eq_ignore_ascii_case("Unspecified")).collect::<Vec<_>>().join(" · ")),
                k.temperature.filter(|t| *t > 0).map(|t| format!("{t} °C")).unwrap_or("—".into()),
                k.wear.filter(|_| k.media_type.eq_ignore_ascii_case("SSD")).map(|w| format!("{w}%")).unwrap_or("—".into()),
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
                "info" => "<span>Para saber</span>",
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
