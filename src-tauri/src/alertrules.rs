//! Reglas de alerta propias: «si pasa esto, avísame así». El técnico elige qué
//! mirar (espacio libre, memoria, un servicio parado, una copia atrasada, el tóner
//! de una impresora…), a partir de qué valor y durante cuánto tiempo, y AdminOps lo
//! vigila en segundo plano y lo avisa en la campana y en «Hoy».
//!
//! Aquí solo está la lógica: dadas unas reglas y unas lecturas, decide cuáles saltan.
//! Leer las cifras (discos, servicios, impresoras…) está en `alertrulesio.rs`.
//!
//! Una regla salta **una vez** cuando se cumple de forma sostenida y no vuelve a saltar
//! hasta que deja de cumplirse: se avisa de los cambios, no del estado.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// disk_free · cpu · ram · service_running · backup_age · toner · disk_score
    pub metric: String,
    /// La unidad (C), el servicio, la impresora o la copia. Vacío: la de por defecto.
    pub target: String,
    /// lt · gt
    pub op: String,
    pub value: f64,
    /// Minutos que tiene que cumplirse seguido.
    pub for_minutes: u32,
    /// warn · bad
    pub level: String,
}

impl Default for Rule {
    fn default() -> Self {
        Rule { id: String::new(), name: String::new(), enabled: true, metric: "disk_free".into(), target: String::new(), op: "lt".into(), value: 10.0, for_minutes: 0, level: "warn".into() }
    }
}

pub const METRICS: &[&str] = &["disk_free", "cpu", "ram", "service_running", "backup_age", "toner", "disk_score"];

impl Rule {
    pub fn clean(mut self) -> Self {
        let cut = |s: &str, n: usize| s.trim().chars().filter(|c| !c.is_control()).take(n).collect::<String>();
        self.name = cut(&self.name, 80);
        self.target = cut(&self.target, 120);
        if !METRICS.contains(&self.metric.as_str()) {
            self.metric = "disk_free".into();
        }
        if self.op != "gt" {
            self.op = "lt".into();
        }
        if self.level != "bad" {
            self.level = "warn".into();
        }
        if self.metric == "service_running" {
            // Salta cuando NO está en marcha.
            self.op = "lt".into();
            self.value = 1.0;
        }
        self.value = if self.value.is_finite() { self.value.clamp(-1000.0, 1_000_000.0) } else { 0.0 };
        self.for_minutes = self.for_minutes.min(24 * 60);
        if self.name.is_empty() {
            self.name = describe_metric(&self.metric, &self.target, &self.op, self.value);
        }
        self
    }

    /// La clave de la lectura que necesita.
    pub fn reading_key(&self) -> String {
        format!("{}|{}", self.metric, self.target.trim().to_lowercase())
    }
}

/// «Espacio libre en C: menor que 10 %»
pub fn describe_metric(metric: &str, target: &str, op: &str, value: f64) -> String {
    let cmp = if op == "gt" { "mayor que" } else { "menor que" };
    let t = if target.trim().is_empty() { String::new() } else { format!(" ({})", target.trim()) };
    match metric {
        "disk_free" => format!("Espacio libre{t} {cmp} {value:.0} %"),
        "cpu" => format!("Procesador {cmp} {value:.0} %"),
        "ram" => format!("Memoria en uso {cmp} {value:.0} %"),
        "service_running" => format!("Servicio parado{t}"),
        "backup_age" => format!("Copia de seguridad{t} con más de {value:.0} días"),
        "toner" => format!("Tóner{t} {cmp} {value:.0} %"),
        "disk_score" => format!("Salud de un disco {cmp} {value:.0} puntos"),
        _ => "Regla".into(),
    }
}

/// Lo que cada regla recuerda entre una pasada y la siguiente.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RuleState {
    /// Desde cuándo se cumple sin parar.
    pub since: Option<u64>,
    pub fired: bool,
}

pub type State = HashMap<String, RuleState>;
pub type Readings = HashMap<String, f64>;

#[derive(Clone, Debug, PartialEq)]
pub struct Fired {
    pub rule_id: String,
    pub level: String,
    pub title: String,
    pub detail: String,
}

fn holds(op: &str, reading: f64, value: f64) -> bool {
    if op == "gt" {
        reading > value
    } else {
        reading < value
    }
}

fn say(r: &Rule, reading: f64) -> String {
    match r.metric.as_str() {
        "service_running" => "El servicio no está en marcha".into(),
        "backup_age" => format!("La última copia tiene {reading:.0} días"),
        "disk_free" | "cpu" | "ram" | "toner" => format!("Ahora está en {reading:.0} %"),
        _ => format!("Valor actual: {reading:.0}"),
    }
}

/// Decide qué reglas saltan ahora. Una regla sin lectura no cambia de estado.
pub fn evaluate(rules: &[Rule], readings: &Readings, state: &State, now: u64) -> (Vec<Fired>, State) {
    let mut next = State::new();
    let mut out = Vec::new();
    for r in rules.iter().filter(|r| r.enabled) {
        let mut st = state.get(&r.id).cloned().unwrap_or_default();
        if let Some(v) = readings.get(&r.reading_key()).copied() {
            if holds(&r.op, v, r.value) {
                let since = *st.since.get_or_insert(now);
                if !st.fired && now.saturating_sub(since) >= u64::from(r.for_minutes) * 60 {
                    st.fired = true;
                    out.push(Fired { rule_id: r.id.clone(), level: r.level.clone(), title: r.name.clone(), detail: say(r, v) });
                }
            } else {
                st = RuleState::default();
            }
        }
        next.insert(r.id.clone(), st);
    }
    (out, next)
}

/// `check`: (qué se mide, sobre qué, comparación y valor).
fn rule(id: &str, name: &str, check: (&str, &str, &str, f64), for_minutes: u32, level: &str) -> Rule {
    let (metric, target, op, value) = check;
    Rule { id: id.into(), name: name.into(), enabled: true, metric: metric.into(), target: target.into(), op: op.into(), value, for_minutes, level: level.into() }
}

/// Reglas listas para empezar: (nombre de la plantilla, reglas).
pub fn templates() -> Vec<(&'static str, Vec<Rule>)> {
    vec![
        (
            "Oficina estándar",
            vec![
                rule("t-space", "Poco espacio en el disco de Windows", ("disk_free", "", "lt", 10.0), 0, "bad"),
                rule("t-spooler", "La cola de impresión está parada", ("service_running", "Spooler", "lt", 1.0), 5, "warn"),
                rule("t-backup", "La copia de seguridad está atrasada", ("backup_age", "", "gt", 7.0), 0, "warn"),
                rule("t-ram", "Memoria casi llena de forma sostenida", ("ram", "", "gt", 92.0), 20, "warn"),
            ],
        ),
        (
            "Servidor pequeño",
            vec![
                rule("t-space", "Poco espacio en el disco de Windows", ("disk_free", "", "lt", 15.0), 0, "bad"),
                rule("t-time", "El servicio de hora está parado", ("service_running", "W32Time", "lt", 1.0), 5, "warn"),
                rule("t-backup", "La copia de seguridad no se ha hecho en 2 días", ("backup_age", "", "gt", 2.0), 0, "bad"),
                rule("t-cpu", "Procesador al máximo de forma sostenida", ("cpu", "", "gt", 90.0), 20, "warn"),
                rule("t-ram", "Memoria casi llena de forma sostenida", ("ram", "", "gt", 95.0), 15, "warn"),
                rule("t-disk", "Un disco pierde salud", ("disk_score", "", "lt", 60.0), 0, "bad"),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space_rule(minutes: u32) -> Rule {
        Rule { id: "a".into(), name: "Poco espacio".into(), for_minutes: minutes, ..Default::default() }.clean()
    }

    fn reading(rule: &Rule, v: f64) -> Readings {
        Readings::from([(rule.reading_key(), v)])
    }

    #[test]
    fn fires_once_and_rearms() {
        let r = space_rule(0);
        let (f1, s1) = evaluate(std::slice::from_ref(&r), &reading(&r, 5.0), &State::new(), 1000);
        assert_eq!(f1.len(), 1);
        assert_eq!(f1[0].detail, "Ahora está en 5 %");
        // Sigue igual: no vuelve a saltar.
        let (f2, s2) = evaluate(std::slice::from_ref(&r), &reading(&r, 4.0), &s1, 1060);
        assert!(f2.is_empty());
        // Se arregla y vuelve a pasar: salta otra vez.
        let (f3, s3) = evaluate(std::slice::from_ref(&r), &reading(&r, 30.0), &s2, 1120);
        assert!(f3.is_empty());
        let (f4, _) = evaluate(std::slice::from_ref(&r), &reading(&r, 3.0), &s3, 1180);
        assert_eq!(f4.len(), 1);
    }

    #[test]
    fn needs_to_hold_for_the_whole_time() {
        let r = Rule { metric: "ram".into(), op: "gt".into(), value: 90.0, for_minutes: 10, id: "m".into(), ..Default::default() }.clean();
        let (f, s) = evaluate(std::slice::from_ref(&r), &reading(&r, 95.0), &State::new(), 0);
        assert!(f.is_empty());
        let (f, s) = evaluate(std::slice::from_ref(&r), &reading(&r, 96.0), &s, 5 * 60);
        assert!(f.is_empty());
        // Se corta a mitad: se empieza a contar de nuevo.
        let (f, s) = evaluate(std::slice::from_ref(&r), &reading(&r, 50.0), &s, 8 * 60);
        assert!(f.is_empty());
        let (f, s) = evaluate(std::slice::from_ref(&r), &reading(&r, 97.0), &s, 9 * 60);
        assert!(f.is_empty());
        let (f, _) = evaluate(std::slice::from_ref(&r), &reading(&r, 97.0), &s, 19 * 60);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn missing_readings_change_nothing() {
        let r = space_rule(0);
        let (_, s1) = evaluate(std::slice::from_ref(&r), &reading(&r, 5.0), &State::new(), 0);
        let (f, s2) = evaluate(std::slice::from_ref(&r), &Readings::new(), &s1, 60);
        assert!(f.is_empty());
        assert!(s2["a"].fired, "sin lectura no se re-arma ni se olvida");
    }

    #[test]
    fn disabled_rules_do_not_run() {
        let mut r = space_rule(0);
        r.enabled = false;
        let (f, s) = evaluate(std::slice::from_ref(&r), &reading(&r, 1.0), &State::new(), 0);
        assert!(f.is_empty() && s.is_empty());
    }

    #[test]
    fn service_rule_fires_when_not_running() {
        let r = Rule { id: "s".into(), metric: "service_running".into(), target: "Spooler".into(), op: "gt".into(), value: 99.0, ..Default::default() }.clean();
        assert_eq!((r.op.as_str(), r.value), ("lt", 1.0));
        assert_eq!(r.name, "Servicio parado (Spooler)");
        let (f, _) = evaluate(std::slice::from_ref(&r), &reading(&r, 0.0), &State::new(), 0);
        assert_eq!(f[0].detail, "El servicio no está en marcha");
        let (f, _) = evaluate(std::slice::from_ref(&r), &reading(&r, 1.0), &State::new(), 0);
        assert!(f.is_empty());
    }

    #[test]
    fn clean_fixes_junk_and_names_the_rule() {
        let r = Rule { metric: "raro".into(), op: "???".into(), level: "x".into(), value: f64::NAN, for_minutes: 99999, name: "  ".into(), ..Default::default() }.clean();
        assert_eq!((r.metric.as_str(), r.op.as_str(), r.level.as_str(), r.for_minutes), ("disk_free", "lt", "warn", 1440));
        assert_eq!(r.value, 0.0);
        assert!(r.name.starts_with("Espacio libre"));
    }

    #[test]
    fn templates_are_valid_and_unique() {
        for (name, rules) in templates() {
            let ids: std::collections::HashSet<&str> = rules.iter().map(|r| r.id.as_str()).collect();
            assert_eq!(ids.len(), rules.len(), "{name}");
            for r in rules {
                assert_eq!(r.clone().clean(), r, "{name}/{}", r.id);
            }
        }
    }
}
