//! Etiquetas con QR para los equipos: se pegan en la caja y, al escanearlas con el
//! móvil, enseñan la ficha mínima (equipo, dueño, última visita y a quién llamar).
//!
//! El QR lleva el texto de la ficha, no un enlace: se lee sin internet, sin
//! servidor y sin AdminOps. Se imprimen sueltas o en hoja de 12 o 24.

use serde::Deserialize;
use std::fmt::Write;

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LabelInfo {
    pub host: String,
    /// Dueño o cliente.
    pub owner: String,
    /// Dónde está (sala, planta…).
    pub place: String,
    pub last_visit: String,
    pub phone: String,
    pub extension: String,
}

fn clean(s: &str, n: usize) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(n).collect()
}

/// El texto que lleva el QR (varias líneas, legible en cualquier lector).
pub fn payload(i: &LabelInfo) -> String {
    let mut lines = vec![clean(&i.host, 60)];
    let owner = [clean(&i.owner, 80), clean(&i.place, 60)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
    if !owner.is_empty() {
        lines.push(owner);
    }
    if !i.last_visit.trim().is_empty() {
        lines.push(format!("Última visita: {}", clean(&i.last_visit, 30)));
    }
    let contact = [(!i.phone.trim().is_empty()).then(|| format!("Soporte: {}", clean(&i.phone, 30))), (!i.extension.trim().is_empty()).then(|| format!("ext. {}", clean(&i.extension, 10)))]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    if !contact.is_empty() {
        lines.push(contact);
    }
    lines.push(format!("ID AdminOps: {}", clean(&i.host, 60)));
    lines.join("\n")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn qr_svg(text: &str, size: u32) -> Result<String, String> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(code.render::<svg::Color<'_>>().min_dimensions(size, size).quiet_zone(true).dark_color(svg::Color("#111111")).light_color(svg::Color("#ffffff")).build())
}

/// Una hoja A4 de etiquetas, `cols` × `rows`.
pub fn sheet_html(labels: &[LabelInfo], cols: usize, rows: usize, company: &str) -> Result<String, String> {
    let (cols, rows) = (cols.clamp(1, 4), rows.clamp(1, 12));
    // Tamaño de cada etiqueta sobre una hoja A4 con 8 mm de margen.
    let w = (210.0 - 16.0) / cols as f64;
    let h = (297.0 - 16.0) / rows as f64;
    let mut out = format!(
        "<!doctype html><html lang=es><head><meta charset=utf-8><title>Etiquetas</title><style>\
         @page{{size:A4;margin:8mm}}*{{box-sizing:border-box}}body{{margin:0;font:11px/1.35 \"Segoe UI\",system-ui,sans-serif;color:#111}}\
         .sheet{{display:grid;grid-template-columns:repeat({cols},{w:.2}mm);grid-auto-rows:{h:.2}mm}}\
         .l{{display:flex;gap:3mm;align-items:center;padding:3mm;border:.2mm dashed #bbb;overflow:hidden;break-inside:avoid}}\
         .l svg{{width:{q:.1}mm;height:{q:.1}mm;flex:none}}.t{{min-width:0}}.t b{{display:block;font-size:14px;overflow-wrap:anywhere}}\
         .t div{{overflow-wrap:anywhere}}.t .m{{color:#555;font-size:9.5px}}</style></head><body><div class=sheet>",
        q = (h - 6.0).min(w * 0.45).max(14.0)
    );
    for i in labels.iter().take(cols * rows) {
        if i.host.trim().is_empty() {
            continue;
        }
        let _ = write!(out, "<div class=l>{}<div class=t><b>{}</b>", qr_svg(&payload(i), 160)?, esc(clean(&i.host, 40).as_str()));
        let owner = [clean(&i.owner, 60), clean(&i.place, 40)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        if !owner.is_empty() {
            let _ = write!(out, "<div>{}</div>", esc(&owner));
        }
        if !i.phone.trim().is_empty() || !i.extension.trim().is_empty() {
            let _ = write!(out, "<div>Soporte {}{}</div>", esc(&clean(&i.phone, 30)), if i.extension.trim().is_empty() { String::new() } else { format!(" · ext. {}", esc(&clean(&i.extension, 10))) });
        }
        if !i.last_visit.trim().is_empty() {
            let _ = write!(out, "<div class=m>Última visita {}</div>", esc(&clean(&i.last_visit, 30)));
        }
        if !company.trim().is_empty() {
            let _ = write!(out, "<div class=m>{}</div>", esc(&clean(company, 40)));
        }
        out.push_str("</div></div>");
    }
    out.push_str("</div></body></html>");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> LabelInfo {
        LabelInfo { host: "PC-CONTA-03".into(), owner: "María Pérez".into(), place: "Contabilidad".into(), last_visit: "02/10/2026".into(), phone: "600 000 000".into(), extension: "214".into() }
    }

    #[test]
    fn payload_reads_like_a_card() {
        let p = payload(&info());
        assert_eq!(p.lines().next(), Some("PC-CONTA-03"));
        assert!(p.contains("María Pérez · Contabilidad") && p.contains("Última visita: 02/10/2026") && p.contains("Soporte: 600 000 000 · ext. 214"));
        assert!(p.ends_with("ID AdminOps: PC-CONTA-03"));
    }

    #[test]
    fn payload_skips_empty_parts() {
        let p = payload(&LabelInfo { host: "PC-1".into(), ..Default::default() });
        assert_eq!(p, "PC-1\nID AdminOps: PC-1");
    }

    #[test]
    fn sheet_has_one_label_per_machine_and_escapes() {
        let mut b = info();
        b.owner = "<b>x</b> & co".into();
        let h = sheet_html(&[info(), b], 3, 8, "Mi empresa").unwrap();
        assert_eq!(h.matches("class=l>").count(), 2);
        assert!(h.contains("&lt;b&gt;x&lt;/b&gt; &amp; co") && h.contains("<svg"));
        assert!(h.contains("repeat(3,"));
    }

    #[test]
    fn sheet_limits_labels_to_the_grid() {
        let many: Vec<LabelInfo> = (0..30).map(|i| LabelInfo { host: format!("PC-{i}"), ..Default::default() }).collect();
        assert_eq!(sheet_html(&many, 2, 6, "").unwrap().matches("class=l>").count(), 12);
    }
}
