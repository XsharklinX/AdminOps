//! El estado de los discos para el informe del cliente y para el ticket.
//!
//! Una hoja clara: semáforo, qué se ha encontrado, qué significa para la
//! persona y qué se recomienda, sin siglas. Los números técnicos van aparte,
//! solo en la plantilla técnica. Y una frase lista para pegar en el ticket.
//!
//! Trabaja sobre `DiskView`, una foto de lo que ya calculó la pantalla de
//! Discos (veredicto, nota…). Se guarda al mirar los discos, y el informe usa la
//! última: no vuelve a leer los discos (tarda segundos y pide administrador).

use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DiskView {
    pub name: String,
    pub bus: String,
    pub media: String,
    pub size: u64,
    pub system: bool,
    /// ok | warn | bad
    pub level: String,
    pub title: String,
    pub text: String,
    pub advice: Vec<String>,
    pub score: u8,
    pub label: String,
    pub life_text: String,
    pub hours: i64,
    pub temperature: i64,
    pub wear: i64,
    pub reallocated: Option<u64>,
    pub pending: Option<u64>,
    pub uncorrectable: Option<u64>,
    pub crc: Option<u64>,
    /// (letra, tamaño, libre)
    pub volumes: Vec<(String, u64, u64)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Saved {
    pub at: u64,
    pub disks: Vec<DiskView>,
}

/// Lo que se copia desde la pantalla de Discos.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiskTexts {
    /// Para el cliente, en lenguaje claro.
    pub client: String,
    /// Corto, para pegar en el ticket.
    pub ticket: String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn gb(n: u64) -> String {
    let g = n as f64 / 1e9;
    if g >= 950.0 {
        format!("{:.1} TB", g / 1000.0)
    } else {
        format!("{g:.0} GB")
    }
}

/// El tamaño comercial al que cambiar un disco: el de siempre o el siguiente.
pub fn replacement_size(size: u64) -> &'static str {
    let g = size as f64 / 1e9;
    match g {
        x if x <= 140.0 => "256 GB",
        x if x <= 280.0 => "256 GB",
        x if x <= 560.0 => "500 GB",
        x if x <= 1100.0 => "1 TB",
        x if x <= 2200.0 => "2 TB",
        _ => "4 TB",
    }
}

fn kind(d: &DiskView) -> String {
    let t = if d.bus.eq_ignore_ascii_case("USB") {
        "disco externo".to_string()
    } else if d.media.eq_ignore_ascii_case("SSD") || d.bus.eq_ignore_ascii_case("NVMe") {
        "disco SSD".to_string()
    } else if d.media.eq_ignore_ascii_case("HDD") {
        "disco duro".to_string()
    } else {
        "disco".to_string()
    };
    if d.system {
        format!("{t} principal ({})", gb(d.size))
    } else {
        format!("{t} ({})", gb(d.size))
    }
}

fn bad_zones(d: &DiskView) -> u64 {
    d.pending.unwrap_or(0) + d.uncorrectable.unwrap_or(0)
}

/// Frase para el cliente: qué pasa, qué significa y qué hacer.
pub fn client_text(d: &DiskView) -> String {
    let k = kind(d);
    match d.level.as_str() {
        "bad" => {
            let zonas = bad_zones(d);
            let que = if zonas > 0 { format!("ha empezado a fallar: tiene {zonas} zonas que ya no se pueden leer") } else { "está dando errores".to_string() };
            format!(
                "Su {k} {que}. Todavía funciona, pero puede perder información en cualquier momento. Le recomendamos hacer una copia de sus archivos esta semana y cambiar el disco por uno nuevo (por ejemplo, un SSD de {}). Mientras tanto, conviene usarlo lo menos posible.",
                replacement_size(d.size)
            )
        }
        "warn" => format!(
            "Su {k} funciona, pero tiene señales de desgaste ({}). No hay riesgo inmediato, aunque conviene tener una copia al día de sus archivos y revisarlo de nuevo en unos meses.",
            d.title.to_lowercase()
        ),
        _ => format!("Su {k} está en buen estado (nota {} sobre 100). No hace falta hacer nada.", d.score),
    }
}

/// Una línea para el ticket.
pub fn ticket_text(d: &DiskView) -> String {
    let mut s = format!("Disco {} ({}{}, {}): {} ({}/100). {}", d.name.trim(), d.bus, if d.media.is_empty() || d.media == "Unspecified" { String::new() } else { format!(" {}", d.media) }, gb(d.size), d.label, d.score, d.title);
    if !s.ends_with('.') {
        s.push('.');
    }
    let mut facts = Vec::new();
    if d.hours > 0 {
        facts.push(format!("{} h", d.hours));
    }
    if d.temperature > 0 {
        facts.push(format!("{} °C", d.temperature));
    }
    if let Some(r) = d.reallocated.filter(|r| *r > 0) {
        facts.push(format!("{r} sectores apartados"));
    }
    if let Some(p) = d.pending.filter(|p| *p > 0) {
        facts.push(format!("{p} pendientes"));
    }
    if let Some(u) = d.uncorrectable.filter(|u| *u > 0) {
        facts.push(format!("{u} no corregibles"));
    }
    if let Some(c) = d.crc.filter(|c| *c > 0) {
        facts.push(format!("{c} errores CRC"));
    }
    if !facts.is_empty() {
        let _ = write!(s, " Datos: {}.", facts.join(", "));
    }
    if let Some(first) = d.advice.first() {
        let _ = write!(s, " Recomendado: {first}");
    }
    s
}

pub fn texts(d: &DiskView) -> DiskTexts {
    DiskTexts { client: client_text(d), ticket: ticket_text(d) }
}

fn when(at: u64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(at as i64, 0).single().map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_default()
}

/// La sección «Estado de los discos» del informe.
pub fn html(saved: &Saved, technical: bool) -> String {
    if saved.disks.is_empty() {
        return String::new();
    }
    let mut h = String::from("<h2>Estado de los discos</h2>");
    for d in &saved.disks {
        let cls = match d.level.as_str() {
            "bad" => "v-bad",
            "warn" => "v-warn",
            _ => "v-ok",
        };
        let lvl = match d.level.as_str() {
            "bad" => "Fallando",
            "warn" => "Con avisos",
            _ => "Sano",
        };
        let _ = write!(
            h,
            "<div class=\"verdict {cls}\" style=\"margin-top:8px;break-inside:avoid\"><div class=state><div class=t>{}</div><p>{}</p>",
            esc(&format!("{lvl} · {}", kind(d))),
            esc(&client_text(d))
        );
        if technical {
            let _ = write!(h, "<p class=muted>{} · nota {}/100 ({}){}</p>", esc(d.name.trim()), d.score, esc(&d.label), if d.life_text.is_empty() { String::new() } else { format!(" · {}", esc(&d.life_text)) });
            let mut facts = Vec::new();
            if d.hours > 0 {
                facts.push(format!("{} h encendido", d.hours));
            }
            if d.temperature > 0 {
                facts.push(format!("{} °C", d.temperature));
            }
            if d.wear >= 0 {
                facts.push(format!("{} % de desgaste", d.wear));
            }
            for (n, v) in [("sectores apartados", d.reallocated), ("pendientes", d.pending), ("no corregibles", d.uncorrectable), ("errores de conexión", d.crc)] {
                if let Some(v) = v {
                    facts.push(format!("{v} {n}"));
                }
            }
            if !facts.is_empty() {
                let _ = write!(h, "<p class=muted>{}</p>", esc(&facts.join(" · ")));
            }
            for (l, size, free) in &d.volumes {
                if *size > 0 {
                    let _ = write!(h, "<p class=muted>Unidad {l}: {} libres de {}</p>", gb(*free), gb(*size));
                }
            }
        }
        if d.level != "ok" && !d.advice.is_empty() {
            h.push_str("<ul class=\"items one\">");
            for a in d.advice.iter().take(4) {
                let _ = write!(h, "<li class=x>{}</li>", esc(a));
            }
            h.push_str("</ul>");
        }
        h.push_str("</div></div>");
    }
    let _ = write!(h, "<p class=muted>Estado de los discos medido el {}.</p>", when(saved.at));
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(level: &str) -> DiskView {
        DiskView {
            name: "Samsung SSD 870 EVO 500GB".into(),
            bus: "SATA".into(),
            media: "SSD".into(),
            size: 500_107_862_016,
            system: true,
            level: level.into(),
            title: "Ya ha tenido sectores dañados (8 apartados)".into(),
            advice: vec!["Ten una copia al día de lo importante.".into()],
            score: 71,
            label: "Bueno".into(),
            hours: 12_480,
            temperature: 38,
            pending: Some(8),
            ..Default::default()
        }
    }

    #[test]
    fn client_text_depends_on_level() {
        let bad = client_text(&disk("bad"));
        assert!(bad.contains("8 zonas") && bad.contains("500 GB") && bad.contains("esta semana"), "{bad}");
        let warn = client_text(&disk("warn"));
        assert!(warn.contains("desgaste") && !warn.contains("esta semana"), "{warn}");
        let ok = client_text(&disk("ok"));
        assert!(ok.contains("buen estado") && ok.contains("71"), "{ok}");
        assert!(!ok.contains("SMART") && !ok.contains("CRC"), "sin siglas para el cliente");
    }

    #[test]
    fn ticket_text_is_one_line_with_facts() {
        let t = ticket_text(&disk("warn"));
        assert!(!t.contains('\n'));
        assert!(t.contains("Samsung SSD 870 EVO 500GB (SATA SSD, 500 GB)") && t.contains("Bueno (71/100)"), "{t}");
        assert!(t.contains("12480 h") && t.contains("8 pendientes"), "{t}");
        assert!(t.ends_with("Ten una copia al día de lo importante."), "{t}");
    }

    #[test]
    fn replacement_sizes() {
        assert_eq!(replacement_size(120_000_000_000), "256 GB");
        assert_eq!(replacement_size(500_000_000_000), "500 GB");
        assert_eq!(replacement_size(1_000_000_000_000), "1 TB");
        assert_eq!(replacement_size(2_000_000_000_000), "2 TB");
        assert_eq!(replacement_size(8_000_000_000_000), "4 TB");
    }

    #[test]
    fn html_has_client_and_technical_versions() {
        let saved = Saved { at: 1_760_000_000, disks: vec![disk("bad")] };
        let client = html(&saved, false);
        assert!(client.contains("Estado de los discos") && client.contains("verdict v-bad") && client.contains("Fallando"));
        assert!(!client.contains("pendientes") && !client.contains("nota 71"));
        let tech = html(&saved, true);
        assert!(tech.contains("nota 71/100") && tech.contains("8 pendientes") && tech.contains("12480 h encendido"));
        assert!(html(&Saved::default(), true).is_empty());
    }

    #[test]
    fn html_escapes_model_names() {
        let mut d = disk("ok");
        d.name = "<script>x</script>".into();
        let h = html(&Saved { at: 1, disks: vec![d] }, true);
        assert!(!h.contains("<script>"));
    }
}
