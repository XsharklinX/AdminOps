//! Diagnóstico del equipo: recolecta, evalúa hallazgos y guarda una "foto"
//! (snapshot) de cada análisis para comparar antes/después en el informe.

pub mod collect;
mod pdf;
pub mod report;

use crate::tweaks::TweakState;
use collect::{Battery, DeviceProblem, PhysicalDisk, Stability, SystemHealth};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::Disks;
use tauri::{Manager, State};

const MAX_SNAPSHOTS: usize = 40;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Bad,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: Severity,
    pub area: String,
    pub title: String,
    pub detail: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub mount: String,
    pub total: u64,
    pub free: u64,
}

/// Resultado de un recolector: el dato o por qué no se pudo obtener.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Section<T> {
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> From<Result<T, String>> for Section<T> {
    fn from(r: Result<T, String>) -> Self {
        match r {
            Ok(d) => Section { data: Some(d), error: None },
            Err(e) => Section { data: None, error: Some(e) },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    /// Segundos epoch. También identifica el snapshot.
    pub timestamp: u64,
    pub host: String,
    pub os: String,
    pub cpu: String,
    pub ram_total: u64,
    pub admin: bool,
    pub volumes: Vec<Volume>,
    pub disks: Section<Vec<PhysicalDisk>>,
    pub stability: Section<Stability>,
    pub drivers: Section<Vec<DeviceProblem>>,
    pub battery: Section<Option<Battery>>,
    pub system: Section<SystemHealth>,
    pub startup_enabled: Section<Vec<String>>,
    pub bloat_installed: Section<Vec<String>>,
    pub tweaks_applied: usize,
    pub findings: Vec<Finding>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Fechas de PowerShell: `ToString('o')` incluye zona salvo en fechas "sin tipo"
/// (p. ej. Get-HotFix), que se interpretan como hora local.
pub fn parse_time(iso: &str) -> Option<chrono::DateTime<chrono::Local>> {
    use chrono::TimeZone;
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) {
        return Some(t.with_timezone(&chrono::Local));
    }
    let naive = chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
    chrono::Local.from_local_datetime(&naive).earliest()
}

fn days_since(iso: &str) -> Option<i64> {
    Some((chrono::Local::now() - parse_time(iso)?).num_days())
}

fn gb(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1024f64.powi(3))
}

fn finding(severity: Severity, area: &str, title: String, detail: Option<String>) -> Finding {
    Finding { severity, area: area.into(), title, detail }
}

/// Convierte los datos en una lista de hallazgos priorizada (lo peor primero).
fn evaluate(d: &Diagnostics) -> Vec<Finding> {
    use Severity::*;
    let mut f = Vec::new();
    let sys_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());

    for v in &d.volumes {
        let pct = if v.total > 0 { v.free as f64 * 100.0 / v.total as f64 } else { 100.0 };
        let is_sys = v.mount.to_uppercase().starts_with(&sys_drive.to_uppercase());
        let sev = if pct < 10.0 { Some(Bad) } else if pct < 20.0 && is_sys { Some(Warn) } else { None };
        if let Some(sev) = sev {
            f.push(finding(
                sev,
                "Almacenamiento",
                format!("{} con poco espacio libre ({pct:.0}%)", v.mount),
                Some(format!("Quedan {} de {}.{}", gb(v.free), gb(v.total), if is_sys { " Windows necesita espacio para actualizarse y paginar." } else { "" })),
            ));
        }
    }

    if let Some(disks) = &d.disks.data {
        for k in disks {
            if !k.health.eq_ignore_ascii_case("Healthy") {
                f.push(finding(Bad, "Discos", format!("{}: estado {}", k.name, k.health), Some(format!("Estado operativo: {}. Hacer copia de seguridad cuanto antes.", k.operational))));
            }
            match k.wear {
                Some(w) if w >= 80 => f.push(finding(Bad, "Discos", format!("{}: {w}% de vida útil consumida", k.name), Some("Planificar el reemplazo del SSD.".into()))),
                Some(w) if w >= 50 => f.push(finding(Warn, "Discos", format!("{}: {w}% de vida útil consumida", k.name), None)),
                _ => {}
            }
            if let Some(t) = k.temperature.filter(|t| *t >= 60) {
                f.push(finding(Warn, "Discos", format!("{}: temperatura alta ({t} °C)", k.name), Some("Revisar ventilación o disipador del SSD.".into())));
            }
            let errors = k.read_errors.unwrap_or(0) + k.write_errors.unwrap_or(0);
            if errors > 0 {
                f.push(finding(Warn, "Discos", format!("{}: {errors} errores de lectura/escritura", k.name), None));
            }
        }
    }

    if let Some(s) = &d.stability.data {
        if !s.bugchecks.is_empty() {
            let last = s.bugchecks.first().unwrap();
            let name = last.name.clone().unwrap_or_else(|| last.code.clone());
            f.push(finding(
                Bad,
                "Estabilidad",
                format!("{} pantallazo(s) azul(es) en {} días (último: {name})", s.bugchecks.len(), s.days),
                last.hint.clone(),
            ));
        }
        let unexpected = s.unexpected_shutdowns.len().saturating_sub(s.bugchecks.len());
        if unexpected >= 2 {
            f.push(finding(
                Warn,
                "Estabilidad",
                format!("{unexpected} apagados inesperados en {} días", s.days),
                Some("Cortes de corriente, botón de encendido mantenido o fuente/temperatura.".into()),
            ));
        }
        for c in s.crashes.iter().filter(|c| c.count >= 5) {
            f.push(finding(Warn, "Estabilidad", format!("{} se ha cerrado inesperadamente {} veces", c.app, c.count), None));
        }
        if let Some(b) = s.boot_times.as_ref().and_then(|b| b.first()) {
            if b.ms >= 60_000 {
                f.push(finding(Warn, "Rendimiento", format!("El último arranque tardó {} s", b.ms / 1000), Some("Revisar programas de inicio y el tipo de disco.".into())));
            }
        }
    }

    if let Some(drivers) = &d.drivers.data {
        for dev in drivers {
            let sev = if dev.code == 22 { Info } else { Warn };
            f.push(finding(sev, "Drivers", format!("{}: {}", dev.name, dev.problem), Some(format!("Código {} · {}", dev.code, dev.device_id))));
        }
    }

    if let Some(Some(b)) = &d.battery.data {
        let h = b.health();
        if h > 0.0 && h < 80.0 {
            f.push(finding(
                if h < 60.0 { Bad } else { Warn },
                "Batería",
                format!("Batería al {h:.0}% de su capacidad original"),
                Some(format!("{} mWh de {} mWh de fábrica{}.", b.full, b.design, b.cycles.map(|c| format!(", {c} ciclos")).unwrap_or_default())),
            ));
        }
    }

    if let Some(s) = &d.system.data {
        if s.pending_reboot {
            f.push(finding(Warn, "Sistema", "Hay un reinicio pendiente".into(), Some("Actualizaciones o cambios esperando a reiniciar.".into())));
        }
        if let Some(days) = days_since(&s.last_boot).filter(|d| *d >= 14) {
            f.push(finding(Info, "Sistema", format!("Sin reiniciar desde hace {days} días"), Some("Muchos problemas de lentitud se resuelven reiniciando.".into())));
        }
        match s.last_update.as_deref().and_then(days_since) {
            Some(days) if days >= 90 => f.push(finding(Bad, "Seguridad", format!("Última actualización hace {days} días"), Some("Revisar Windows Update.".into()))),
            Some(days) if days >= 45 => f.push(finding(Warn, "Seguridad", format!("Última actualización hace {days} días"), None)),
            _ => {}
        }
        let third_party_av = s.antivirus.iter().any(|a| !a.to_lowercase().contains("defender"));
        if s.defender_realtime == Some(false) && !third_party_av {
            f.push(finding(Bad, "Seguridad", "Protección en tiempo real desactivada".into(), Some("No hay otro antivirus activo.".into())));
        }
        if let Some(age) = s.signature_age_days.filter(|a| *a >= 7) {
            f.push(finding(Warn, "Seguridad", format!("Firmas del antivirus con {age} días de antigüedad"), None));
        }
        if s.activated == Some(false) {
            f.push(finding(Warn, "Sistema", "Windows no está activado".into(), None));
        }
    }

    if let Some(s) = &d.startup_enabled.data {
        if s.len() >= 12 {
            f.push(finding(Warn, "Rendimiento", format!("{} programas arrancan con Windows", s.len()), Some("Revisar la página Inicio.".into())));
        }
    }
    if let Some(b) = &d.bloat_installed.data {
        if !b.is_empty() {
            f.push(finding(Info, "Rendimiento", format!("{} apps promocionales o retiradas instaladas", b.len()), Some(b.join(", "))));
        }
    }

    f.sort_by(|a, b| b.severity.cmp(&a.severity));
    f
}

fn snapshots_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("AdminOps")).join("snapshots")
}

fn save_snapshot(app: &tauri::AppHandle, d: &Diagnostics) {
    let dir = snapshots_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string(d) {
        let _ = std::fs::write(dir.join(format!("{}.json", d.timestamp)), json);
    }
    // Conservar solo los más recientes.
    let mut files = snapshot_files(&dir);
    if files.len() > MAX_SNAPSHOTS {
        files.sort();
        for (_, p) in files.iter().take(files.len() - MAX_SNAPSHOTS) {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn snapshot_files(dir: &PathBuf) -> Vec<(u64, PathBuf)> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let ts = p.file_stem()?.to_str()?.parse().ok()?;
            Some((ts, p))
        })
        .collect()
}

pub fn load_snapshot(app: &tauri::AppHandle, ts: u64) -> Option<Diagnostics> {
    let p = snapshots_dir(app).join(format!("{ts}.json"));
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

pub fn latest_snapshot(app: &tauri::AppHandle) -> Option<Diagnostics> {
    let ts = snapshot_files(&snapshots_dir(app)).into_iter().map(|(t, _)| t).max()?;
    load_snapshot(app, ts)
}

fn join<T>(r: std::thread::Result<Result<T, String>>) -> Result<T, String> {
    r.unwrap_or_else(|_| Err("El recolector falló".to_string()))
}

fn collect(state: &TweakState) -> Diagnostics {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let mut volumes: Vec<Volume> = Disks::new_with_refreshed_list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .map(|d| Volume { mount: d.mount_point().to_string_lossy().into_owned(), total: d.total_space(), free: d.available_space() })
        .collect();
    volumes.sort_by(|a, b| a.mount.cmp(&b.mount));

    // Todos los recolectores en paralelo: el total es el del más lento, no la suma.
    let (disks, stability, drivers, battery, system, startup, bloat, tweaks_applied) = std::thread::scope(|s| {
        let disks = s.spawn(collect::disks);
        let stability = s.spawn(collect::stability);
        let drivers = s.spawn(collect::drivers);
        let battery = s.spawn(collect::battery);
        let system = s.spawn(collect::system);
        let startup = s.spawn(crate::tweaks::startup::enabled_names);
        let bloat = s.spawn(crate::tweaks::appx::recommended_installed);
        let tweaks = s.spawn(|| state.applied_count());
        (
            join(disks.join()),
            join(stability.join()),
            join(drivers.join()),
            join(battery.join()),
            join(system.join()),
            join(startup.join()),
            join(bloat.join()),
            tweaks.join().unwrap_or(0),
        )
    });

    let mut d = Diagnostics {
        timestamp: now(),
        host: sysinfo::System::host_name().unwrap_or_default(),
        os: sysinfo::System::long_os_version().unwrap_or_default(),
        cpu: sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        ram_total: sys.total_memory(),
        admin: crate::elevation::is_elevated(),
        volumes,
        disks: disks.into(),
        stability: stability.into(),
        drivers: drivers.into(),
        battery: battery.into(),
        system: system.into(),
        startup_enabled: startup.into(),
        bloat_installed: bloat.into(),
        tweaks_applied,
        findings: vec![],
    };
    d.findings = evaluate(&d);
    d
}

#[tauri::command(async)]
pub fn run_diagnostics(app: tauri::AppHandle, state: State<'_, TweakState>) -> Result<Diagnostics, String> {
    let d = collect(&state);
    save_snapshot(&app, &d);
    Ok(d)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    timestamp: u64,
    bad: usize,
    warn: usize,
}

#[tauri::command(async)]
pub fn list_snapshots(app: tauri::AppHandle) -> Vec<SnapshotInfo> {
    let mut v: Vec<SnapshotInfo> = snapshot_files(&snapshots_dir(&app))
        .into_iter()
        .filter_map(|(ts, _)| {
            let d = load_snapshot(&app, ts)?;
            let count = |s| d.findings.iter().filter(|f| f.severity == s).count();
            Some(SnapshotInfo { timestamp: ts, bad: count(Severity::Bad), warn: count(Severity::Warn) })
        })
        .collect();
    v.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_powershell_dates_with_and_without_zone() {
        assert!(super::parse_time("2026-09-26T08:56:22.5000000-04:00").is_some());
        assert!(super::parse_time("2026-09-16T00:00:00.0000000").is_some());
        assert!(super::parse_time("basura").is_none());
    }
}
