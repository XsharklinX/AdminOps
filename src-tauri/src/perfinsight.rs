//! Fugas de memoria y disco al 100 % por programa, con el historial de
//! rendimiento: el programa cuya memoria crece sin parar durante horas, quién
//! lee o escribe en disco cuando se queda saturado y a qué hora del día va más
//! cargado el equipo. Responde «se pone lento a media tarde» con un nombre.

use crate::perfhistory::Sample;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Leak {
    pub name: String,
    pub from_mb: u32,
    pub to_mb: u32,
    pub hours: f32,
    /// Cuándo se midió el último valor.
    pub at: u64,
    pub series: Vec<(u64, u32)>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiskHog {
    pub name: String,
    /// En cuántos minutos con el disco saturado aparecía entre los que más usaban el disco.
    pub minutes: usize,
    pub avg_kbs: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeakHour {
    pub hour: u32,
    pub cpu: f32,
    pub busy: f32,
    pub names: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Insights {
    pub samples: usize,
    pub hours: f32,
    pub leaks: Vec<Leak>,
    pub saturated_minutes: usize,
    pub disk_hogs: Vec<DiskHog>,
    pub peak: Option<PeakHour>,
}

/// Un hueco de más de esto corta la serie (AdminOps estuvo cerrada).
const GAP: u64 = 15 * 60;

/// Programas cuya memoria crece sin parar en el último tramo continuo.
pub fn leaks(samples: &[Sample]) -> Vec<Leak> {
    let mut series: BTreeMap<String, Vec<(u64, u32)>> = BTreeMap::new();
    for s in samples {
        for (name, mb) in &s.mem {
            series.entry(name.clone()).or_default().push((s.t, *mb));
        }
    }
    let mut out = Vec::new();
    for (name, mut v) in series {
        v.sort_by_key(|x| x.0);
        // El último tramo sin huecos.
        let mut start = 0;
        for i in 1..v.len() {
            if v[i].0 - v[i - 1].0 > GAP {
                start = i;
            }
        }
        let run = &v[start..];
        if run.len() < 60 {
            continue;
        }
        let hours = (run[run.len() - 1].0 - run[0].0) as f32 / 3600.0;
        // Se comparan medias del principio y del final para no fiarse de un pico.
        let q = (run.len() / 6).max(3);
        let head = run[..q].iter().map(|x| x.1 as f64).sum::<f64>() / q as f64;
        let tail = run[run.len() - q..].iter().map(|x| x.1 as f64).sum::<f64>() / q as f64;
        let steps = run.windows(2).count().max(1);
        let rising = run.windows(2).filter(|w| w[1].1 as f64 >= w[0].1 as f64 * 0.97).count();
        if hours >= 2.0 && tail >= head * 1.8 && tail - head >= 800.0 && rising * 100 / steps >= 85 {
            let step = (run.len() / 60).max(1);
            out.push(Leak { name, from_mb: head as u32, to_mb: tail as u32, hours, at: run[run.len() - 1].0, series: run.iter().step_by(step).copied().collect() });
        }
    }
    out.sort_by_key(|l| std::cmp::Reverse(l.to_mb - l.from_mb));
    out
}

/// Quién usaba el disco en los minutos en que estaba saturado.
pub fn disk_hogs(samples: &[Sample]) -> (usize, Vec<DiskHog>) {
    let hot: Vec<&Sample> = samples.iter().filter(|s| s.busy.is_some_and(|b| b >= 85.0)).collect();
    let mut by: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for s in &hot {
        for (name, kbs) in &s.io {
            let e = by.entry(name.clone()).or_default();
            e.0 += 1;
            e.1 += *kbs as u64;
        }
    }
    let mut v: Vec<DiskHog> = by.into_iter().map(|(name, (m, sum))| DiskHog { name, minutes: m, avg_kbs: (sum / m.max(1) as u64) as u32 }).collect();
    v.sort_by_key(|h| std::cmp::Reverse(h.minutes));
    v.truncate(5);
    (hot.len(), v)
}

/// La hora del día en que el equipo va más cargado, si destaca de verdad.
pub fn peak_hour(samples: &[Sample], offset_secs: i64) -> Option<PeakHour> {
    if samples.len() < 240 {
        return None;
    }
    let mut by: BTreeMap<u32, (f32, f32, usize, BTreeMap<String, usize>)> = BTreeMap::new();
    for s in samples {
        let h = (((s.t as i64 + offset_secs).rem_euclid(86_400)) / 3600) as u32;
        let e = by.entry(h).or_default();
        e.0 += s.cpu;
        e.1 += s.busy.unwrap_or(0.0);
        e.2 += 1;
        for n in s.top.iter().chain(s.io.first().map(|x| &x.0)) {
            *e.3.entry(n.clone()).or_default() += 1;
        }
    }
    let load = |e: &(f32, f32, usize, BTreeMap<String, usize>)| (e.0 + e.1) / e.2.max(1) as f32;
    let overall = by.values().map(load).sum::<f32>() / by.len().max(1) as f32;
    let (hour, e) = by.iter().filter(|(_, e)| e.2 >= 20).max_by(|a, b| load(a.1).total_cmp(&load(b.1)))?;
    if load(e) < overall * 1.5 || load(e) < 50.0 {
        return None;
    }
    let mut names: Vec<(&String, &usize)> = e.3.iter().collect();
    names.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    Some(PeakHour { hour: *hour, cpu: e.0 / e.2 as f32, busy: e.1 / e.2 as f32, names: names.into_iter().take(3).map(|(n, _)| n.clone()).collect() })
}

pub fn insights(samples: &[Sample], offset_secs: i64) -> Insights {
    let (saturated_minutes, disk_hogs) = disk_hogs(samples);
    let hours = match (samples.first(), samples.last()) {
        (Some(a), Some(b)) => (b.t.saturating_sub(a.t)) as f32 / 3600.0,
        _ => 0.0,
    };
    Insights { samples: samples.len(), hours, leaks: leaks(samples), saturated_minutes, disk_hogs, peak: peak_hour(samples, offset_secs) }
}

#[tauri::command]
pub fn perf_insights() -> Insights {
    let offset = chrono::Local::now().offset().local_minus_utc() as i64;
    insights(&crate::perfhistory::samples(), offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: u64, mem: Vec<(&str, u32)>) -> Sample {
        Sample { t, cpu: 10.0, ram: 50.0, disk: 40.0, mem: mem.into_iter().map(|(n, m)| (n.to_string(), m)).collect(), ..Default::default() }
    }

    #[test]
    fn detecta_una_fuga() {
        // Teams sube de 400 MB a 3,1 GB en 6 h; Chrome se queda igual.
        let v: Vec<Sample> = (0..360).map(|i| s(1000 + i * 60, vec![("Teams.exe", 400 + (i as u32) * 8), ("chrome.exe", 900 + (i as u32 % 5) * 20)])).collect();
        let l = leaks(&v);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "Teams.exe");
        assert!(l[0].hours > 5.5);
        assert!(l[0].to_mb > 2500);
    }

    #[test]
    fn un_hueco_corta_la_serie() {
        let mut v: Vec<Sample> = (0..200).map(|i| s(i * 60, vec![("x.exe", 100 + i as u32 * 10)])).collect();
        // AdminOps cerrada dos horas: lo de antes no cuenta y lo de después es corto.
        v.extend((0..30).map(|i| s(200 * 60 + 7200 + i * 60, vec![("x.exe", 3000)])));
        assert!(leaks(&v).is_empty());
    }

    #[test]
    fn quien_satura_el_disco() {
        let mut v: Vec<Sample> = (0..10).map(|i| Sample { t: i * 60, busy: Some(95.0), io: vec![("SearchIndexer.exe".into(), 30_000), ("chrome.exe".into(), 500)], ..Default::default() }).collect();
        v.push(Sample { t: 999, busy: Some(10.0), io: vec![("otro.exe".into(), 50_000)], ..Default::default() });
        let (n, hogs) = disk_hogs(&v);
        assert_eq!(n, 10);
        assert_eq!(hogs[0].name, "SearchIndexer.exe");
        assert_eq!(hogs[0].avg_kbs, 30_000);
        assert!(!hogs.iter().any(|h| h.name == "otro.exe"));
    }

    #[test]
    fn hora_punta() {
        let v: Vec<Sample> = (0..(24 * 60 * 2)).map(|i| {
            let t = i * 60;
            let hour = (t / 3600) % 24;
            Sample { t, cpu: if hour == 17 { 85.0 } else { 15.0 }, busy: Some(if hour == 17 { 60.0 } else { 5.0 }), top: (hour == 17).then(|| "backup.exe".to_string()), ..Default::default() }
        }).collect();
        let p = peak_hour(&v, 0).unwrap();
        assert_eq!(p.hour, 17);
        assert_eq!(p.names[0], "backup.exe");
        let flat: Vec<Sample> = (0..500).map(|i| Sample { t: i * 60, cpu: 20.0, ..Default::default() }).collect();
        assert!(peak_hour(&flat, 0).is_none());
    }
}
