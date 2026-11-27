//! Lo que se apunta mientras se graba y cómo se convierte en marcas de la línea de tiempo.
//!
//! Todo esto es lógica pura (sin Windows ni PowerShell) para poder probarla: tramos de procesador y
//! de disco por encima de un umbral durante unos segundos, programas que dejaron de responder y
//! errores del registro de eventos de Windows. Al final se señala la «causa probable».

use crate::eventdigest::RawEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Procesador total a partir del cual se apunta un pico.
pub const CPU_PCT: f32 = 90.0;
/// Disco ocupado a partir del cual se apunta un pico.
pub const DISK_PCT: f32 = 95.0;
/// Segundos seguidos por encima del umbral para que cuente.
pub const MIN_RUN: usize = 3;
/// Cuánto antes de un «no responde» o un error de programa puede haber empezado la causa probable.
pub const CAUSE_WINDOW: u32 = 10;
/// Errores del registro que se apuntan como mucho (el resto es ruido en una grabación de dos minutos).
const MAX_EVENT_MARKS: usize = 30;

/// De qué tipo es una marca (decide su color).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Disk,
    Cpu,
    Hang,
    Error,
    /// Puesta a mano con el botón «Marcar» o Ctrl+Alt+M.
    Manual,
}

/// Una marca de la línea de tiempo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Mark {
    /// Segundo del vídeo en que empieza.
    pub t: u32,
    pub kind: Kind,
    pub title: String,
    pub detail: String,
    /// La marca de disco o procesador que más probablemente provocó un «no responde» o un error de programa.
    #[serde(skip_serializing_if = "is_false")]
    pub probable: bool,
    /// Un programa que falló o dejó de responder (lo que dispara la búsqueda de la causa probable).
    #[serde(skip_serializing_if = "is_false")]
    pub app: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Lo medido en un segundo de grabación.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tick {
    /// Segundo (desde el primer fotograma) en que empezó el intervalo medido.
    pub t: u32,
    pub cpu: f32,
    pub disk: Option<f32>,
    /// El programa que más procesador usaba (solo se mira cuando el procesador va alto).
    pub cpu_top: Option<String>,
    /// El programa que más leía y escribía (solo se mira cuando el disco va alto).
    pub io_top: Option<String>,
    /// Programas con una ventana «No responde» en ese momento.
    pub hung: Vec<String>,
    /// Memoria en uso, en porcentaje.
    pub mem: Option<f32>,
    /// Bytes por segundo de red, entre bajada y subida.
    pub net: Option<u64>,
}

/// Un segundo de las pistas del reproductor (procesador, disco, memoria y red), lo que se guarda en el `.json`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sample {
    pub t: u32,
    pub cpu: f32,
    pub disk: Option<f32>,
    pub mem: Option<f32>,
    pub net: Option<u64>,
}

/// Las pistas de una grabación, un valor por segundo, redondeadas (el `.json` no crece de más).
pub fn samples_of(ticks: &[Tick]) -> Vec<Sample> {
    let r1 = |v: f32| (v * 10.0).round() / 10.0;
    ticks.iter().map(|t| Sample { t: t.t, cpu: r1(t.cpu), disk: t.disk.map(r1), mem: t.mem.map(r1), net: t.net }).collect()
}

/// Tramos seguidos de al menos `min_len` valores a partir de `threshold`: (índice de inicio, longitud).
/// Un valor que falta (`None`) corta el tramo.
pub fn runs(values: &[Option<f32>], threshold: f32, min_len: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, v) in values.iter().chain(std::iter::once(&None)).enumerate() {
        let high = v.is_some_and(|x| x >= threshold);
        match (high, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if i - s >= min_len {
                    out.push((s, i - s));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// El nombre que más se repite (el primero que apareció si hay empate).
pub fn most_common<'a>(names: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut order: Vec<&str> = Vec::new();
    let mut count: HashMap<&str, usize> = HashMap::new();
    for n in names {
        let c = count.entry(n).or_insert(0);
        if *c == 0 {
            order.push(n);
        }
        *c += 1;
    }
    let mut best: Option<(&str, usize)> = None;
    for n in order {
        let c = count[n];
        if best.is_none_or(|(_, b)| c > b) {
            best = Some((n, c));
        }
    }
    best.map(|(n, _)| n.to_string())
}

/// «14 s», «1 min 5 s», «2 min».
pub fn fmt_secs(secs: u32) -> String {
    if secs < 60 {
        format!("{secs} s")
    } else if secs.is_multiple_of(60) {
        format!("{} min", secs / 60)
    } else {
        format!("{} min {} s", secs / 60, secs % 60)
    }
}

/// Marcas de procesador y de disco: una por tramo, en su inicio, con el programa que más usaba.
pub fn usage_marks(ticks: &[Tick]) -> Vec<Mark> {
    let mut marks = Vec::new();
    let cpu: Vec<Option<f32>> = ticks.iter().map(|t| Some(t.cpu)).collect();
    for (s, len) in runs(&cpu, CPU_PCT, MIN_RUN) {
        let run = &ticks[s..s + len];
        let avg = run.iter().map(|t| t.cpu).sum::<f32>() / len as f32;
        let who = most_common(run.iter().filter_map(|t| t.cpu_top.as_deref()));
        marks.push(Mark {
            t: ticks[s].t,
            kind: Kind::Cpu,
            title: format!("CPU al {} %", avg.round() as u32),
            detail: who.map(|n| format!("{n} usando mucho el procesador")).unwrap_or_default(),
            ..Default::default()
        });
    }
    let disk: Vec<Option<f32>> = ticks.iter().map(|t| t.disk).collect();
    for (s, len) in runs(&disk, DISK_PCT, MIN_RUN) {
        let run = &ticks[s..s + len];
        let avg = run.iter().filter_map(|t| t.disk).sum::<f32>() / len as f32;
        let who = most_common(run.iter().filter_map(|t| t.io_top.as_deref()));
        marks.push(Mark {
            t: ticks[s].t,
            kind: Kind::Disk,
            title: format!("Disco al {} %", avg.round().min(100.0) as u32),
            detail: who.map(|n| format!("{n} leyendo y escribiendo en el disco")).unwrap_or_default(),
            ..Default::default()
        });
    }
    marks
}

/// Marcas de «no responde»: una al empezar y, si el programa vuelve (o su ventana desaparece), cuánto duró.
pub fn hang_marks(ticks: &[Tick]) -> Vec<Mark> {
    let mut marks: Vec<Mark> = Vec::new();
    // Programa → (posición de su marca, segundo en que empezó).
    let mut open: HashMap<String, (usize, u32)> = HashMap::new();
    for tick in ticks {
        for name in &tick.hung {
            if !open.contains_key(name) {
                open.insert(name.clone(), (marks.len(), tick.t));
                marks.push(Mark {
                    t: tick.t,
                    kind: Kind::Hang,
                    title: format!("{name} no responde"),
                    app: true,
                    ..Default::default()
                });
            }
        }
        let back: Vec<String> = open.keys().filter(|n| !tick.hung.contains(n)).cloned().collect();
        for name in back {
            if let Some((idx, since)) = open.remove(&name) {
                marks[idx].detail = format!("{name} no respondió {}", fmt_secs(tick.t.saturating_sub(since).max(1)));
            }
        }
    }
    for (name, (idx, _)) in open {
        marks[idx].detail = format!("{name} seguía sin responder al terminar la grabación");
    }
    marks
}

/// ¿El evento es un programa que falló o se colgó?
pub fn is_app_fault(provider: &str, id: u32) -> bool {
    let p = provider.to_ascii_lowercase();
    (p.contains("application error") && id == 1000) || (p.contains("application hang") && id == 1002) || (p.contains(".net runtime") && id == 1026)
}

/// Marcas de los errores del registro de eventos que cayeron dentro de la grabación.
/// `start_unix` es el segundo (Unix) del primer fotograma y `secs` lo que duró.
/// El ruido conocido de Windows (DCOM 10016…) no se apunta: taparía lo importante.
pub fn event_marks(events: &[RawEvent], start_unix: u64, secs: u32) -> Vec<Mark> {
    let mut out: Vec<Mark> = Vec::new();
    let mut seen: Vec<(String, u32, u32)> = Vec::new();
    let mut sorted: Vec<&RawEvent> = events.iter().filter(|e| e.t + 1 >= start_unix).collect();
    sorted.sort_by_key(|e| e.t);
    for e in sorted {
        let t = (e.t.saturating_sub(start_unix) as u32).min(secs);
        let known = crate::eventdigest::known_title(&e.provider, e.id);
        if known.is_some_and(|(_, weight)| weight == "noise") {
            continue;
        }
        if seen.iter().any(|(p, id, at)| *p == e.provider && *id == e.id && *at == t) {
            continue;
        }
        seen.push((e.provider.clone(), e.id, t));
        let (title, detail) = match known {
            Some((friendly, _)) => (format!("Error {}", e.id), friendly.to_string()),
            None => (format!("Error {} de {}", e.id, e.provider), String::new()),
        };
        out.push(Mark { t, kind: Kind::Error, title, detail, app: is_app_fault(&e.provider, e.id), ..Default::default() });
        if out.len() >= MAX_EVENT_MARKS {
            break;
        }
    }
    out
}

/// Ordena las marcas y señala la causa probable: para cada «no responde» o error de programa, la marca
/// de disco o procesador que empezó en los `CAUSE_WINDOW` segundos anteriores (la más cercana).
/// Si no hay ninguna, ninguna lleva la etiqueta.
pub fn finish_marks(mut marks: Vec<Mark>) -> Vec<Mark> {
    marks.sort_by_key(|m| (m.t, m.kind as u8));
    let triggers: Vec<u32> = marks.iter().filter(|m| m.app).map(|m| m.t).collect();
    for at in triggers {
        let cause = marks
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(m.kind, Kind::Disk | Kind::Cpu) && m.t <= at && at - m.t <= CAUSE_WINDOW)
            .max_by_key(|(i, m)| (m.t, std::cmp::Reverse(*i)))
            .map(|(i, _)| i);
        if let Some(i) = cause {
            marks[i].probable = true;
        }
    }
    marks
}

/// Todas las marcas de una grabación.
pub fn build_marks(ticks: &[Tick], events: &[RawEvent], start_unix: u64, secs: u32) -> Vec<Mark> {
    let mut all = usage_marks(ticks);
    all.extend(hang_marks(ticks));
    all.extend(event_marks(events, start_unix, secs));
    finish_marks(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(t: u32, cpu: f32, disk: Option<f32>) -> Tick {
        Tick { t, cpu, disk, ..Default::default() }
    }

    fn event(provider: &str, id: u32, t: u64) -> RawEvent {
        RawEvent { log: "Application".into(), provider: provider.into(), id, level: 2, t, message: "x".into() }
    }

    #[test]
    fn tramos_con_umbral_y_duracion() {
        let v: Vec<Option<f32>> = [10.0, 95.0, 96.0, 20.0, 99.0, 99.0, 99.0, 99.0, 5.0].iter().map(|x| Some(*x)).collect();
        // El de 2 segundos no cuenta; el de 4 sí.
        assert_eq!(runs(&v, 95.0, 3), vec![(4, 4)]);
        assert_eq!(runs(&v, 95.0, 2), vec![(1, 2), (4, 4)]);
        // Un tramo que llega hasta el final también cuenta.
        let w: Vec<Option<f32>> = [1.0, 99.0, 99.0, 99.0].iter().map(|x| Some(*x)).collect();
        assert_eq!(runs(&w, 95.0, 3), vec![(1, 3)]);
        // Justo en el umbral cuenta; un hueco corta el tramo.
        let g = vec![Some(95.0), Some(95.0), None, Some(95.0), Some(95.0), Some(95.0)];
        assert_eq!(runs(&g, 95.0, 3), vec![(3, 3)]);
        assert!(runs(&[], 95.0, 3).is_empty());
    }

    #[test]
    fn el_mas_repetido_y_los_empates() {
        assert_eq!(most_common(["a", "b", "b", "c"].into_iter()), Some("b".into()));
        assert_eq!(most_common(["x", "y"].into_iter()), Some("x".into()));
        assert_eq!(most_common(std::iter::empty()), None);
    }

    #[test]
    fn duraciones_legibles() {
        assert_eq!(fmt_secs(14), "14 s");
        assert_eq!(fmt_secs(65), "1 min 5 s");
        assert_eq!(fmt_secs(120), "2 min");
    }

    #[test]
    fn marca_de_disco_y_de_cpu_con_su_programa() {
        let mut ticks: Vec<Tick> = (0..20).map(|t| tick(t, 20.0, Some(10.0))).collect();
        for tk in &mut ticks[5..10] {
            tk.disk = Some(100.0);
            tk.io_top = Some("OneDrive".into());
        }
        for tk in &mut ticks[12..15] {
            tk.cpu = 93.0;
            tk.cpu_top = Some("Chrome".into());
        }
        // Dos segundos solo: no cuenta.
        ticks[17].cpu = 99.0;
        ticks[18].cpu = 99.0;
        let marks = usage_marks(&ticks);
        assert_eq!(marks.len(), 2);
        let cpu = marks.iter().find(|m| m.kind == Kind::Cpu).unwrap();
        assert_eq!((cpu.t, cpu.title.as_str()), (12, "CPU al 93 %"));
        assert_eq!(cpu.detail, "Chrome usando mucho el procesador");
        let disk = marks.iter().find(|m| m.kind == Kind::Disk).unwrap();
        assert_eq!((disk.t, disk.title.as_str()), (5, "Disco al 100 %"));
        assert_eq!(disk.detail, "OneDrive leyendo y escribiendo en el disco");
    }

    #[test]
    fn sin_programa_conocido_no_se_inventa_ninguno() {
        let ticks: Vec<Tick> = (0..4).map(|t| tick(t, 99.0, None)).collect();
        let marks = usage_marks(&ticks);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].detail, "");
    }

    #[test]
    fn colgado_con_su_duracion() {
        let mut ticks: Vec<Tick> = (0..30).map(|t| tick(t, 10.0, None)).collect();
        for tk in &mut ticks[4..18] {
            tk.hung = vec!["Excel".into()];
        }
        // Otro que sigue colgado hasta el final.
        for tk in &mut ticks[25..30] {
            tk.hung = vec!["Word".into()];
        }
        let marks = hang_marks(&ticks);
        assert_eq!(marks.len(), 2);
        assert_eq!((marks[0].t, marks[0].title.as_str()), (4, "Excel no responde"));
        assert_eq!(marks[0].detail, "Excel no respondió 14 s");
        assert!(marks[0].app);
        assert_eq!(marks[1].title, "Word no responde");
        assert_eq!(marks[1].detail, "Word seguía sin responder al terminar la grabación");
    }

    #[test]
    fn eventos_dentro_de_la_grabacion() {
        let start = 1_000;
        let events = vec![
            event("Application Hang", 1002, 1_043),
            event("Application Hang", 1002, 1_043),
            event("Microsoft-Windows-DistributedCOM", 10016, 1_010),
            event("Raro", 77, 1_020),
            event("Application Error", 1000, 900),
            event("Application Error", 1000, 5_000),
        ];
        let marks = event_marks(&events, start, 72);
        assert_eq!(marks.len(), 3, "{marks:?}");
        let hang = marks.iter().find(|m| m.title == "Error 1002").unwrap();
        assert_eq!(hang.t, 43);
        assert_eq!(hang.detail, "Un programa dejó de responder");
        assert!(hang.app);
        let raro = marks.iter().find(|m| m.t == 20).unwrap();
        assert_eq!(raro.title, "Error 77 de Raro");
        assert!(!raro.app);
        // El que cae después del final se pega al final; el de antes de empezar no entra.
        assert!(marks.iter().any(|m| m.t == 72 && m.title == "Error 1000"));
    }

    #[test]
    fn causa_probable_la_mas_cercana_en_diez_segundos() {
        let marks = vec![
            Mark { t: 3, kind: Kind::Cpu, title: "CPU al 95 %".into(), ..Default::default() },
            Mark { t: 14, kind: Kind::Disk, title: "Disco al 100 %".into(), ..Default::default() },
            Mark { t: 20, kind: Kind::Hang, title: "Excel no responde".into(), app: true, ..Default::default() },
        ];
        let out = finish_marks(marks);
        assert_eq!(out.iter().filter(|m| m.probable).count(), 1);
        assert!(out.iter().find(|m| m.kind == Kind::Disk).unwrap().probable);
        assert!(!out.iter().find(|m| m.kind == Kind::Cpu).unwrap().probable);
    }

    #[test]
    fn sin_candidata_en_la_ventana_ninguna_lleva_la_etiqueta() {
        let marks = vec![
            Mark { t: 2, kind: Kind::Disk, title: "Disco al 99 %".into(), ..Default::default() },
            Mark { t: 30, kind: Kind::Hang, title: "Excel no responde".into(), app: true, ..Default::default() },
        ];
        assert!(finish_marks(marks).iter().all(|m| !m.probable));
        // Sin «no responde» ni error de programa tampoco hay causa que buscar.
        let solo = vec![Mark { t: 2, kind: Kind::Disk, ..Default::default() }, Mark { t: 3, kind: Kind::Error, ..Default::default() }];
        assert!(finish_marks(solo).iter().all(|m| !m.probable));
        // Una candidata que empieza después del fallo no vale.
        let despues = vec![Mark { t: 12, kind: Kind::Disk, ..Default::default() }, Mark { t: 10, kind: Kind::Hang, app: true, ..Default::default() }];
        assert!(finish_marks(despues).iter().all(|m| !m.probable));
    }

    #[test]
    fn el_json_de_las_marcas_es_estable() {
        let m = Mark { t: 24, kind: Kind::Hang, title: "Excel no responde".into(), app: true, ..Default::default() };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"kind\":\"hang\""), "{json}");
        assert!(!json.contains("probable"), "{json}");
        let back: Mark = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn los_umbrales_son_los_del_encargo() {
        assert_eq!((CPU_PCT, DISK_PCT, MIN_RUN, CAUSE_WINDOW), (90.0, 95.0, 3, 10));
    }
}
