//! La resolución de un caso en tres tonos: breve (para el campo del ticket), para
//! la persona atendida (amable, sin jerga) y técnica (la bitácora de siempre).
//!
//! No se inventa nada: todas las líneas salen de lo que de verdad se hizo (el
//! diario) y de lo que encontró el último análisis. Solo cambia cómo se cuenta.

use serde::Serialize;

/// Lo que se sabe del caso.
#[derive(Default, Clone, Debug)]
pub struct Facts {
    pub ticket: String,
    pub person: String,
    pub machine: String,
    pub notes: String,
    pub minutes: u64,
    /// Lo hecho, con cuántas veces seguidas.
    pub done: Vec<(String, usize)>,
    /// Lo que falló, con su motivo si se sabe.
    pub failed: Vec<String>,
    /// Lo que encontró el último análisis: (título, detalle).
    pub findings: Vec<(String, Option<String>)>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tones {
    pub brief: String,
    pub friendly: String,
}

fn once(t: &str, n: usize) -> String {
    if n > 1 {
        format!("{t} ({n} veces)")
    } else {
        t.to_string()
    }
}

pub fn brief(f: &Facts) -> String {
    let who: Vec<&str> = [f.ticket.as_str(), f.machine.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
    let head = if who.is_empty() { String::new() } else { format!("{}: ", who.join(" · ")) };
    let body = if f.done.is_empty() {
        "revisado, sin cambios en el equipo".to_string()
    } else {
        let shown: Vec<String> = f.done.iter().take(3).map(|(t, n)| once(t, *n)).collect();
        let more = f.done.len().saturating_sub(3);
        format!("{}{}", shown.join("; "), if more > 0 { format!(" (y {more} más)") } else { String::new() })
    };
    let failed = if f.failed.is_empty() { String::new() } else { format!(" No se pudo: {}.", f.failed.iter().take(2).cloned().collect::<Vec<_>>().join("; ")) };
    format!("{head}{body}.{failed} {} min.", f.minutes.max(1))
}

pub fn friendly(f: &Facts) -> String {
    let first = f.person.split_whitespace().next().unwrap_or("");
    let mut out = vec![if first.is_empty() { "Hola:".to_string() } else { format!("Hola {first}:") }, String::new()];
    let equipo = if f.machine.is_empty() { "su equipo".to_string() } else { format!("su equipo ({})", f.machine) };
    if f.done.is_empty() {
        out.push(format!("Hemos revisado {equipo} y de momento no ha hecho falta cambiar nada."));
    } else {
        out.push(format!("Hemos revisado {equipo} y esto es lo que hicimos:"));
        out.extend(f.done.iter().map(|(t, n)| format!("• {}", once(t, *n))));
    }
    if !f.findings.is_empty() {
        out.push(String::new());
        out.push("Lo que encontramos:".into());
        out.extend(f.findings.iter().take(4).map(|(t, d)| match d.as_deref().filter(|d| !d.trim().is_empty()) {
            Some(d) => format!("• {t} ({d})"),
            None => format!("• {t}"),
        }));
    }
    if !f.failed.is_empty() {
        out.push(String::new());
        out.push("Esto no hemos podido hacerlo todavía:".into());
        out.extend(f.failed.iter().map(|t| format!("• {t}")));
    }
    if !f.notes.trim().is_empty() {
        out.push(String::new());
        out.push(f.notes.trim().to_string());
    }
    out.push(String::new());
    out.push("Si vuelve a pasar, avísenos y lo miramos enseguida.".into());
    out.push("Un saludo.".into());
    out.join("\n")
}

pub fn tones(f: &Facts) -> Tones {
    Tones { brief: brief(f), friendly: friendly(f) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            ticket: "#4521".into(),
            person: "María Pérez".into(),
            machine: "PC-CONTA-03".into(),
            minutes: 25,
            done: vec![("Limpiar archivos temporales".into(), 1), ("Reiniciar el cliente de correo".into(), 2), ("Actualizar controladores".into(), 1), ("Desactivar servicio X".into(), 1)],
            failed: vec!["Reparar la impresora (sin permisos)".into()],
            findings: vec![("Disco C: casi lleno".into(), Some("3 % libre".into()))],
            ..Default::default()
        }
    }

    #[test]
    fn brief_is_one_line_and_counts_the_rest() {
        let b = brief(&facts());
        assert!(!b.contains('\n'));
        assert!(b.starts_with("#4521 · PC-CONTA-03: Limpiar archivos temporales; Reiniciar el cliente de correo (2 veces); Actualizar controladores (y 1 más)."), "{b}");
        assert!(b.contains("No se pudo: Reparar la impresora") && b.ends_with("25 min."), "{b}");
    }

    #[test]
    fn brief_without_changes() {
        let b = brief(&Facts { minutes: 0, ..Default::default() });
        assert_eq!(b, "revisado, sin cambios en el equipo. 1 min.");
    }

    #[test]
    fn friendly_greets_by_first_name_and_lists_only_real_actions() {
        let t = friendly(&facts());
        assert!(t.starts_with("Hola María:") && t.contains("su equipo (PC-CONTA-03)"), "{t}");
        assert!(t.contains("• Reiniciar el cliente de correo (2 veces)") && t.contains("Lo que encontramos:") && t.contains("• Disco C: casi lleno (3 % libre)"), "{t}");
        assert!(t.contains("Esto no hemos podido hacerlo todavía:") && t.ends_with("Un saludo."), "{t}");
        assert_eq!(t.matches("• ").count(), 4 + 1 + 1, "{t}");
    }

    #[test]
    fn friendly_with_nothing_done() {
        let t = friendly(&Facts::default());
        assert!(t.starts_with("Hola:") && t.contains("no ha hecho falta cambiar nada"), "{t}");
    }
}
