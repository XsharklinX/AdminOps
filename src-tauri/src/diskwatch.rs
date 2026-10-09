//! Vigilante de discos: cada cierto tiempo (aunque AdminOps esté minimizado)
//! mira los discos y avisa solo cuando una cifra cambia de verdad:
//! sectores pendientes que suben, un disco que anuncia que va a fallar,
//! temperatura sostenida, poco espacio libre o un disco que desaparece.
//!
//! Avisa de los cambios, no del estado: un disco que ya tenía 8 sectores
//! pendientes ayer no avisa cada media hora, avisa cuando pasan a 9.
//! Los avisos entran en la campana y en «Hoy», y quedan en el diario.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub enabled: bool,
    /// Cada cuántos minutos se mira.
    pub interval_min: u32,
    /// Avisar si queda menos de este % libre en una unidad.
    pub free_pct: u32,
    /// Temperatura sostenida (°C) a partir de la cual se avisa.
    pub temp_hdd: i64,
    pub temp_ssd: i64,
    /// Discos que no se vigilan («modelo|tamaño»).
    pub muted: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config { enabled: true, interval_min: 30, free_pct: 10, temp_hdd: 55, temp_ssd: 70, muted: Vec::new() }
    }
}

impl Config {
    pub fn sane(mut self) -> Self {
        self.interval_min = self.interval_min.clamp(5, 24 * 60);
        self.free_pct = self.free_pct.clamp(1, 50);
        self.temp_hdd = self.temp_hdd.clamp(35, 90);
        self.temp_ssd = self.temp_ssd.clamp(35, 100);
        self
    }
}

/// Lo que el vigilante necesita saber de un disco.
#[derive(Clone, Debug, Default)]
pub struct WatchDisk {
    /// «modelo|tamaño»
    pub key: String,
    pub model: String,
    pub bus: String,
    pub media: String,
    pub pending: u64,
    pub uncorrectable: u64,
    pub predict_failure: bool,
    pub temperature: i64,
    /// (letra, tamaño, libre)
    pub volumes: Vec<(String, u64, u64)>,
}

/// Lo que se recuerda entre una mirada y la siguiente.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Seen {
    pub model: String,
    pub bus: String,
    pub pending: u64,
    pub uncorrectable: u64,
    pub predict_failure: bool,
    /// Miradas seguidas por encima de la temperatura.
    pub hot_checks: u32,
    pub hot_alerted: bool,
    /// Letras de unidad que ya estaban por debajo del mínimo de espacio.
    pub low_space: Vec<String>,
}

pub type State = BTreeMap<String, Seen>;

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub key: String,
    /// bad | warn
    pub level: String,
    pub title: String,
    pub detail: String,
    pub explanation: String,
    pub advice: String,
}

fn finding(key: String, level: &str, title: String, detail: String, explanation: &str, advice: &str) -> Finding {
    Finding { key, level: level.into(), title, detail, explanation: explanation.into(), advice: advice.into() }
}

fn removable(bus: &str) -> bool {
    bus.eq_ignore_ascii_case("USB") || bus.eq_ignore_ascii_case("SD")
}

/// Compara lo que hay ahora con lo de la vez anterior. Devuelve los avisos y el estado nuevo.
pub fn evaluate(prev: &State, now: &[WatchDisk], cfg: &Config) -> (Vec<Finding>, State) {
    let mut out = Vec::new();
    let mut next = State::new();

    for d in now {
        if cfg.muted.contains(&d.key) {
            continue;
        }
        let old = prev.get(&d.key);
        let mut s = Seen { model: d.model.clone(), bus: d.bus.clone(), pending: d.pending, uncorrectable: d.uncorrectable, predict_failure: d.predict_failure, ..Default::default() };

        // Sectores que no se pueden leer: lo importante es que SUBAN.
        let (op, ou) = old.map_or((0, 0), |o| (o.pending, o.uncorrectable));
        if d.pending > op || d.uncorrectable > ou {
            let mut bits = Vec::new();
            if d.pending > op {
                bits.push(format!("sectores pendientes {op} → {}", d.pending));
            }
            if d.uncorrectable > ou {
                bits.push(format!("no corregibles {ou} → {}", d.uncorrectable));
            }
            out.push(finding(
                format!("diskwatch-sectors:{}:{}:{}", d.key, d.pending, d.uncorrectable),
                if d.uncorrectable > ou || d.pending >= 20 { "bad" } else { "warn" },
                format!("{} tiene más sectores ilegibles", d.model),
                bits.join(" · "),
                "El disco ha encontrado zonas que no sabe leer. Cuando esa cifra sube, el disco se está estropeando y hay datos en riesgo.",
                "Haz hoy una copia de lo importante (Discos → Rescatar archivos) y planifica cambiar el disco.",
            ));
        }

        // El propio disco anuncia que va a fallar.
        if d.predict_failure && !old.is_some_and(|o| o.predict_failure) {
            out.push(finding(
                format!("diskwatch-predict:{}", d.key),
                "bad",
                format!("{} anuncia que va a fallar", d.model),
                "SMART: fallo inminente".into(),
                "Es el aviso oficial del propio disco (SMART). Cuando lo da, suele quedar poco tiempo.",
                "Copia ya lo importante y cambia el disco.",
            ));
        }

        // Temperatura sostenida: dos miradas seguidas por encima.
        let limit = if d.media.eq_ignore_ascii_case("SSD") || d.bus.eq_ignore_ascii_case("NVMe") { cfg.temp_ssd } else { cfg.temp_hdd };
        if d.temperature >= limit {
            s.hot_checks = old.map_or(0, |o| o.hot_checks) + 1;
            s.hot_alerted = old.is_some_and(|o| o.hot_alerted);
            if s.hot_checks >= 2 && !s.hot_alerted {
                s.hot_alerted = true;
                out.push(finding(
                    format!("diskwatch-hot:{}", d.key),
                    "warn",
                    format!("{} lleva un rato caliente", d.model),
                    format!("{} °C (límite {limit} °C)", d.temperature),
                    "Con calor constante un disco dura menos y, en un SSD, baja la velocidad para protegerse.",
                    "Revisa la ventilación del equipo o de la caja externa.",
                ));
            }
        }

        // Poco espacio libre: una vez al bajar del mínimo, otra vez si se recupera y vuelve a bajar.
        for (letter, size, free) in &d.volumes {
            if *size == 0 {
                continue;
            }
            let pct = (*free as f64 / *size as f64 * 100.0) as u32;
            if pct < cfg.free_pct {
                s.low_space.push(letter.clone());
                if !old.is_some_and(|o| o.low_space.contains(letter)) {
                    out.push(finding(
                        format!("diskwatch-free:{letter}"),
                        if pct < 5 { "bad" } else { "warn" },
                        format!("Queda poco espacio en {letter}:"),
                        format!("{pct} % libre ({:.1} GB)", *free as f64 / 1024f64.powi(3)),
                        "Con el disco casi lleno Windows no puede actualizarse, los programas fallan al guardar y todo va más lento.",
                        "Mira qué ocupa más en Discos → Espacio y libera lo que sobre.",
                    ));
                }
            }
        }
        next.insert(d.key.clone(), s);
    }

    // Un disco fijo que estaba y ya no está. Los USB se quitan a menudo: ahí no se avisa.
    for (key, o) in prev {
        if !next.contains_key(key) && !cfg.muted.contains(key) && !removable(&o.bus) && !now.iter().any(|d| &d.key == key) {
            out.push(finding(
                format!("diskwatch-gone:{key}"),
                "bad",
                format!("{} ya no aparece", o.model),
                "El disco se ha desconectado".into(),
                "Un disco que desaparece de repente suele ser el cable, la alimentación o que el disco se está muriendo.",
                "Apaga el equipo, revisa los cables del disco y vuelve a encender. Si no vuelve, copia lo que puedas del resto.",
            ));
        }
    }
    (out, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(pending: u64) -> WatchDisk {
        WatchDisk { key: "WDC|1000".into(), model: "WDC 1TB".into(), bus: "SATA".into(), media: "HDD".into(), pending, temperature: 38, volumes: vec![("D".into(), 1000, 600)], ..Default::default() }
    }

    #[test]
    fn quiet_when_nothing_changes() {
        let cfg = Config::default();
        let (f1, s1) = evaluate(&State::new(), &[disk(0)], &cfg);
        assert!(f1.is_empty());
        let (f2, _) = evaluate(&s1, &[disk(0)], &cfg);
        assert!(f2.is_empty());
    }

    #[test]
    fn warns_only_when_pending_rises() {
        let cfg = Config::default();
        let (_, s0) = evaluate(&State::new(), &[disk(0)], &cfg);
        let (f, s1) = evaluate(&s0, &[disk(8)], &cfg);
        assert_eq!(f.len(), 1);
        assert!(f[0].detail.contains("0 → 8"));
        // La misma cifra otra vez: nada.
        let (again, s2) = evaluate(&s1, &[disk(8)], &cfg);
        assert!(again.is_empty());
        let (up, _) = evaluate(&s2, &[disk(9)], &cfg);
        assert_eq!(up.len(), 1);
    }

    #[test]
    fn sustained_heat_needs_two_checks_and_alerts_once() {
        let cfg = Config::default();
        let mut hot = disk(0);
        hot.temperature = 60;
        let (f1, s1) = evaluate(&State::new(), &[hot.clone()], &cfg);
        assert!(f1.is_empty());
        let (f2, s2) = evaluate(&s1, &[hot.clone()], &cfg);
        assert_eq!(f2.len(), 1);
        let (f3, s3) = evaluate(&s2, &[hot.clone()], &cfg);
        assert!(f3.is_empty());
        // Se enfría y vuelve a calentarse: avisa de nuevo.
        let (_, s4) = evaluate(&s3, &[disk(0)], &cfg);
        let (_, s5) = evaluate(&s4, &[hot.clone()], &cfg);
        let (f6, _) = evaluate(&s5, &[hot], &cfg);
        assert_eq!(f6.len(), 1);
    }

    #[test]
    fn low_space_alerts_on_crossing() {
        let cfg = Config::default();
        let mut full = disk(0);
        full.volumes = vec![("D".into(), 1000, 50)];
        let (_, s0) = evaluate(&State::new(), &[disk(0)], &cfg);
        let (f, s1) = evaluate(&s0, &[full.clone()], &cfg);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].key, "diskwatch-free:D");
        let (f2, _) = evaluate(&s1, &[full], &cfg);
        assert!(f2.is_empty());
    }

    #[test]
    fn disappearing_fixed_disk_alerts_but_usb_does_not() {
        let cfg = Config::default();
        let (_, s0) = evaluate(&State::new(), &[disk(0)], &cfg);
        let (f, _) = evaluate(&s0, &[], &cfg);
        assert_eq!(f.len(), 1);
        assert!(f[0].title.contains("ya no aparece"));
        let mut usb = disk(0);
        usb.bus = "USB".into();
        let (_, su) = evaluate(&State::new(), &[usb], &cfg);
        let (fu, _) = evaluate(&su, &[], &cfg);
        assert!(fu.is_empty());
    }

    #[test]
    fn muted_disks_are_ignored() {
        let cfg = Config { muted: vec!["WDC|1000".into()], ..Default::default() };
        let (f, s) = evaluate(&State::new(), &[disk(50)], &cfg);
        assert!(f.is_empty() && s.is_empty());
    }

    #[test]
    fn predict_failure_alerts_once() {
        let cfg = Config::default();
        let mut d = disk(0);
        d.predict_failure = true;
        let (f, s) = evaluate(&State::new(), &[d.clone()], &cfg);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, "bad");
        let (f2, _) = evaluate(&s, &[d], &cfg);
        assert!(f2.is_empty());
    }

    #[test]
    fn config_is_clamped() {
        let c = Config { interval_min: 1, free_pct: 90, ..Default::default() }.sane();
        assert_eq!((c.interval_min, c.free_pct), (5, 50));
    }
}
