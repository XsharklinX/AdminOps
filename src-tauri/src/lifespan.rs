//! ¿Reparar o cambiar este equipo? Junta la edad del procesador y del equipo, el
//! disco, la memoria, la compatibilidad con Windows 11 y la batería, dice qué
//! pieza limita al conjunto y compara lo que costaría alargarle la vida con lo
//! que cuesta uno nuevo. El resultado es una recomendación y un presupuesto
//! aproximado para el cliente, no una verdad: los precios los pone el técnico.

use serde::{Deserialize, Serialize};

/// Precios aproximados (€) con los que se calcula. El técnico los ajusta.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Costs {
    pub ssd: f64,
    pub ram: f64,
    pub battery: f64,
    pub new_pc: f64,
    /// Mano de obra por cada mejora.
    pub labor: f64,
}

impl Costs {
    /// Precios razonables: nunca negativos ni absurdos.
    pub fn sane(self) -> Self {
        let ok = |v: f64, max: f64| if v.is_finite() { v.clamp(0.0, max) } else { 0.0 };
        Costs { ssd: ok(self.ssd, 2000.0), ram: ok(self.ram, 2000.0), battery: ok(self.battery, 2000.0), new_pc: ok(self.new_pc, 20000.0).max(1.0), labor: ok(self.labor, 2000.0) }
    }
}

impl Default for Costs {
    fn default() -> Self {
        Costs { ssd: 55.0, ram: 35.0, battery: 60.0, new_pc: 620.0, labor: 20.0 }
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Input {
    pub cpu: String,
    pub year: Option<i32>,
    pub this_year: i32,
    pub ram_gb: f64,
    pub windows10: bool,
    pub windows11: bool,
    pub tpm_ready: Option<bool>,
    pub secure_boot: Option<bool>,
    /// Hay discos y todos son mecánicos.
    pub hdd_only: bool,
    /// Algún disco está fallando (nota del módulo de discos).
    pub disk_failing: bool,
    /// Capacidad de la batería en % del diseño (None: sin batería).
    pub battery: Option<f64>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub name: String,
    pub score: u8,
    pub text: String,
    /// Es lo que más limita al equipo.
    pub limiter: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    pub label: String,
    pub cost: f64,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// keep · upgrade · replace
    pub decision: String,
    pub headline: String,
    pub detail: String,
    pub parts: Vec<Part>,
    pub upgrades: Vec<Upgrade>,
    pub upgrade_total: f64,
    pub new_cost: f64,
    /// Años más que se le pueden pedir, a ojo.
    pub years_left: u8,
    /// Texto para el cliente.
    pub client_text: String,
}

/// Generación del procesador: ("Intel", 8) para un i5-8250U, ("AMD", 3) para un Ryzen 5 3600.
pub fn cpu_generation(name: &str) -> Option<(&'static str, u32)> {
    let n = name.to_lowercase();
    let digits_after = |s: &str| -> Option<String> {
        let d: String = s.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
        (!d.is_empty()).then_some(d)
    };
    if let Some(p) = n.find("ryzen") {
        // «Ryzen 5 3600», «Ryzen 7 5800X», «Ryzen 5 7640U»: tras la serie (3, 5, 7, 9) van 4 cifras.
        let rest = &n[p + 5..];
        let model = rest.split_whitespace().filter_map(|w| {
            let d: String = w.chars().take_while(|c| c.is_ascii_digit()).collect();
            (d.len() == 4).then_some(d)
        });
        if let Some(m) = model.into_iter().next() {
            return m[..1].parse().ok().map(|g| ("AMD", g));
        }
        return None;
    }
    if let Some(p) = n.find("-") {
        // «i5-8250U», «i7-10510U», «i5-12400», «i3-N305» (sin cifras iniciales: se ignora).
        if n[..p].ends_with(['3', '5', '7', '9']) && n[..p].contains('i') {
            let d = digits_after(&n[p + 1..])?;
            let g = match d.len() {
                4 => d[..1].parse().ok()?,
                5 => d[..2].parse().ok()?,
                3 => 2, // i5-750: primera generación; las de 3 cifras son anteriores a la 2.ª
                _ => return None,
            };
            return Some(("Intel", if d.len() == 3 { 1 } else { g }));
        }
    }
    if n.contains("core(tm)2") || n.contains("pentium") || n.contains("celeron") {
        return Some(("Intel", 0));
    }
    None
}

fn cpu_score(name: &str) -> (u8, String) {
    match cpu_generation(name) {
        Some(("Intel", g)) => {
            let s = match g {
                12.. => 95,
                10..=11 => 80,
                8..=9 => 65,
                6..=7 => 40,
                _ => 15,
            };
            (s, format!("Intel de {g}.ª generación"))
        }
        Some((_, g)) => {
            let s = match g {
                5.. => 90,
                3..=4 => 75,
                2 => 60,
                _ => 40,
            };
            (s, format!("AMD Ryzen serie {g}000"))
        }
        None => (60, "No se pudo valorar el procesador".into()),
    }
}

/// ¿Puede ir Windows 11? Procesador, TPM, arranque seguro y memoria.
pub fn win11_blockers(i: &Input) -> Vec<String> {
    let mut out = Vec::new();
    match cpu_generation(&i.cpu) {
        Some(("Intel", g)) if g < 8 => out.push("el procesador es demasiado antiguo".to_string()),
        Some(("AMD", g)) if g < 2 => out.push("el procesador es demasiado antiguo".to_string()),
        _ => {}
    }
    if i.tpm_ready == Some(false) {
        out.push("no tiene TPM 2.0".into());
    }
    if i.secure_boot == Some(false) {
        // Se puede activar en la BIOS: no bloquea, pero se avisa.
    }
    if i.ram_gb < 3.5 {
        out.push("tiene menos de 4 GB de memoria".into());
    }
    out
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

pub fn evaluate(i: &Input, c: &Costs) -> Report {
    let (cpu_s, cpu_t) = cpu_score(&i.cpu);
    let age = i.year.map(|y| (i.this_year - y).max(0));
    let age_s: u8 = match age {
        Some(a) if a >= 8 => 20,
        Some(6..=7) => 45,
        Some(4..=5) => 70,
        Some(_) => 95,
        None => 60,
    };
    let blockers = win11_blockers(i);
    let win_s: u8 = if i.windows11 {
        95
    } else if i.windows10 && !blockers.is_empty() {
        20
    } else {
        60
    };
    let ram_s: u8 = match i.ram_gb {
        r if r < 4.0 => 15,
        r if r < 8.0 => 40,
        r if r < 16.0 => 75,
        _ => 95,
    };
    let disk_s: u8 = if i.disk_failing { 5 } else if i.hdd_only { 25 } else { 90 };

    let mut parts = vec![
        Part { name: "Procesador".into(), score: cpu_s, text: cpu_t, limiter: false },
        Part { name: "Edad del equipo".into(), score: age_s, text: age.map_or("No se sabe la fecha de la BIOS".into(), |a| format!("Unos {a} años")), limiter: false },
        Part { name: "Memoria".into(), score: ram_s, text: format!("{:.0} GB", i.ram_gb), limiter: false },
        Part {
            name: "Disco".into(),
            score: disk_s,
            text: if i.disk_failing { "Está fallando".into() } else if i.hdd_only { "Disco mecánico (lento)".into() } else { "SSD".into() },
            limiter: false,
        },
        Part {
            name: "Windows".into(),
            score: win_s,
            text: if i.windows11 {
                "Windows 11".into()
            } else if blockers.is_empty() {
                "Windows 10 (puede pasar a Windows 11)".into()
            } else {
                format!("Windows 10 y no puede pasar a Windows 11: {}", blockers.join(", "))
            },
            limiter: false,
        },
    ];
    if let Some(b) = i.battery {
        parts.push(Part { name: "Batería".into(), score: b.clamp(0.0, 100.0) as u8, text: format!("{b:.0} % de su capacidad"), limiter: false });
    }

    // Lo que se arregla con dinero.
    let mut upgrades = Vec::new();
    if i.hdd_only || i.disk_failing {
        upgrades.push(Upgrade { label: "Cambiar el disco por un SSD".into(), cost: c.ssd + c.labor });
    }
    if i.ram_gb < 7.5 {
        upgrades.push(Upgrade { label: "Ampliar la memoria a 8 GB o más".into(), cost: c.ram + c.labor });
    }
    if i.battery.is_some_and(|b| b < 60.0) {
        upgrades.push(Upgrade { label: "Cambiar la batería".into(), cost: c.battery + c.labor });
    }
    let upgrade_total: f64 = upgrades.iter().map(|u| u.cost).sum();

    // Lo que no se arregla: procesador y edad, y Windows 11 si lo bloquea la máquina.
    let hard_limit = cpu_s <= 40 || age_s <= 20 || (i.windows10 && !blockers.is_empty() && (cpu_s <= 40 || i.tpm_ready == Some(false)));
    let decision = if hard_limit || (upgrade_total > c.new_pc * 0.5 && !upgrades.is_empty()) {
        "replace"
    } else if !upgrades.is_empty() || (i.windows10 && blockers.is_empty()) {
        "upgrade"
    } else {
        "keep"
    };

    if let Some(min) = parts.iter().map(|p| p.score).min() {
        for p in parts.iter_mut().filter(|p| p.score == min && p.score < 70) {
            p.limiter = true;
        }
    }
    let limiter = parts.iter().find(|p| p.limiter).map(|p| p.name.to_lowercase());

    let (headline, detail, years_left) = match decision {
        "replace" => (
            "Conviene sustituirlo".to_string(),
            match &limiter {
                Some(l) => format!("Lo que más lo limita es {l}, y no se arregla cambiando una pieza. Alargarle la vida costaría {upgrade_total:.0} € y seguiría siendo un equipo antiguo; uno nuevo ronda los {:.0} €.", c.new_pc),
                None => format!("Uno nuevo ronda los {:.0} €.", c.new_pc),
            },
            0u8,
        ),
        "upgrade" => (
            "Merece la pena mejorarlo".to_string(),
            format!(
                "{}. Con {upgrade_total:.0} € rinde como uno nuevo para ofimática; un equipo nuevo ronda los {:.0} €.",
                capitalize(&upgrades.iter().map(|u| u.label.to_lowercase()).collect::<Vec<_>>().join(", ")),
                c.new_pc
            ),
            3,
        ),
        _ => ("Puede seguir como está".to_string(), "No hay ninguna pieza que lo frene. Con un mantenimiento normal le quedan años de uso.".to_string(), if age_s >= 70 { 4 } else { 2 }),
    };
    let client_text = match decision {
        "replace" => format!("Su equipo tiene ya unos años y las piezas que más lo frenan no se pueden cambiar. Le recomendamos sustituirlo por uno nuevo (alrededor de {:.0} €): arreglarlo costaría {} y quedaría igualmente anticuado.", c.new_pc, if upgrade_total > 0.0 { format!("unos {upgrade_total:.0} €") } else { "dinero sin cambiar el problema de fondo".into() }),
        "upgrade" => format!("Su equipo todavía sirve. Con unos {upgrade_total:.0} € ({}) funcionará como uno nuevo para su trabajo diario, frente a los {:.0} € de comprar uno.", upgrades.iter().map(|u| u.label.to_lowercase()).collect::<Vec<_>>().join(", "), c.new_pc),
        _ => "Su equipo está bien para el uso que le da. No hace falta cambiar nada ahora.".to_string(),
    };
    Report { decision: decision.into(), headline, detail, parts, upgrades, upgrade_total, new_cost: c.new_pc, years_left, client_text }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modern() -> Input {
        Input { cpu: "12th Gen Intel(R) Core(TM) i5-12400".into(), year: Some(2023), this_year: 2026, ram_gb: 16.0, windows11: true, tpm_ready: Some(true), ..Default::default() }
    }

    #[test]
    fn generation_from_names() {
        assert_eq!(cpu_generation("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz"), Some(("Intel", 8)));
        assert_eq!(cpu_generation("12th Gen Intel(R) Core(TM) i5-12400"), Some(("Intel", 12)));
        assert_eq!(cpu_generation("Intel(R) Core(TM) i7-10510U"), Some(("Intel", 10)));
        assert_eq!(cpu_generation("Intel(R) Core(TM) i5-4590 CPU @ 3.30GHz"), Some(("Intel", 4)));
        assert_eq!(cpu_generation("AMD Ryzen 5 3600 6-Core Processor"), Some(("AMD", 3)));
        assert_eq!(cpu_generation("AMD Ryzen 7 5800X 8-Core Processor"), Some(("AMD", 5)));
        assert_eq!(cpu_generation("Intel(R) Pentium(R) CPU G4560"), Some(("Intel", 0)));
        assert_eq!(cpu_generation("Procesador raro"), None);
    }

    #[test]
    fn modern_machine_is_kept() {
        let r = evaluate(&modern(), &Costs::default());
        assert_eq!(r.decision, "keep");
        assert!(r.upgrades.is_empty() && r.client_text.contains("No hace falta cambiar nada"));
    }

    #[test]
    fn old_pc_with_hdd_and_little_ram_is_worth_upgrading() {
        let i = Input { cpu: "Intel(R) Core(TM) i5-8250U".into(), year: Some(2019), this_year: 2026, ram_gb: 4.0, windows10: true, hdd_only: true, tpm_ready: Some(true), ..Default::default() };
        let r = evaluate(&i, &Costs::default());
        assert_eq!(r.decision, "upgrade", "{r:?}");
        assert_eq!(r.upgrades.len(), 2);
        assert!((r.upgrade_total - (55.0 + 35.0 + 40.0)).abs() < 0.01);
        assert!(r.parts.iter().any(|p| p.limiter && (p.name == "Disco" || p.name == "Memoria")));
        assert!(r.client_text.contains("130 €") && r.client_text.contains("620 €"), "{}", r.client_text);
        assert!(r.detail.starts_with("Cambiar el disco por un ssd") || r.detail.starts_with("Cambiar el disco"), "{}", r.detail);
    }

    #[test]
    fn very_old_machine_is_replaced() {
        let i = Input { cpu: "Intel(R) Core(TM) i5-4590".into(), year: Some(2014), this_year: 2026, ram_gb: 8.0, windows10: true, hdd_only: true, tpm_ready: Some(false), ..Default::default() };
        let r = evaluate(&i, &Costs::default());
        assert_eq!(r.decision, "replace");
        assert_eq!(r.years_left, 0);
        assert!(r.parts.iter().find(|p| p.name == "Windows").unwrap().text.contains("no puede pasar a Windows 11"));
    }

    #[test]
    fn upgrades_that_cost_more_than_half_a_new_pc_mean_replace() {
        let c = Costs { ssd: 200.0, ram: 150.0, labor: 20.0, new_pc: 500.0, ..Default::default() };
        let i = Input { cpu: "Intel(R) Core(TM) i7-10510U".into(), year: Some(2021), this_year: 2026, ram_gb: 4.0, windows11: true, hdd_only: true, tpm_ready: Some(true), ..Default::default() };
        assert_eq!(evaluate(&i, &c).decision, "replace");
    }

    #[test]
    fn failing_disk_on_a_good_machine_is_an_upgrade() {
        let mut i = modern();
        i.disk_failing = true;
        let r = evaluate(&i, &Costs::default());
        assert_eq!(r.decision, "upgrade");
        assert_eq!(r.upgrades[0].label, "Cambiar el disco por un SSD");
    }

    #[test]
    fn worn_battery_adds_an_upgrade_on_laptops() {
        let mut i = modern();
        i.battery = Some(45.0);
        let r = evaluate(&i, &Costs::default());
        assert!(r.upgrades.iter().any(|u| u.label.contains("batería")));
        assert!(r.parts.iter().any(|p| p.name == "Batería"));
    }
}
