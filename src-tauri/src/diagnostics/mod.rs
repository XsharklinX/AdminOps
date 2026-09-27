//! Diagnóstico del equipo: recolecta, evalúa hallazgos y guarda una "foto"
//! (snapshot) de cada análisis para comparar antes/después en el informe.

pub mod collect;
pub mod minidump;
mod pdf;
pub mod report;

use crate::tweaks::TweakState;
use collect::{Battery, DeviceProblem, PhysicalDisk, Stability, SystemHealth};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::Disks;
use tauri::State;

const MAX_SNAPSHOTS: usize = 40;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Bad,
}

/// Herramientas de Windows que la UI puede abrir (lista cerrada).
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    DeviceManager,
    EventViewer,
    Reliability,
    WindowsUpdate,
    WindowsSecurity,
    Activation,
    Storage,
}

/// A dónde lleva un hallazgo: una página/sección de AdminOps o una herramienta de Windows.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    Page { label: String, page: String, focus: Option<String> },
    Tool { label: String, tool: Tool },
}

fn page(label: &str, page: &str, focus: Option<&str>) -> Action {
    Action::Page { label: label.into(), page: page.into(), focus: focus.map(str::to_string) }
}

fn tool(label: &str, tool: Tool) -> Action {
    Action::Tool { label: label.into(), tool }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: Severity,
    pub area: String,
    pub title: String,
    pub detail: Option<String>,
    /// La primera es la acción principal (clic en el hallazgo).
    #[serde(default)]
    pub actions: Vec<Action>,
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
#[serde(rename_all = "camelCase", bound(deserialize = "T: Deserialize<'de>"))]
pub struct Section<T> {
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> Default for Section<T> {
    fn default() -> Self {
        Section { data: None, error: Some("No disponible en este análisis.".into()) }
    }
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
    /// Programas con actualización disponible (winget). Ausente en análisis antiguos.
    #[serde(default)]
    pub software_updates: Section<Vec<crate::software::SoftwareUpdate>>,
    /// Inventario de hardware (placa, BIOS, RAM, GPU…).
    #[serde(default)]
    pub hardware: Section<crate::hardware::Inventory>,
    /// Atributos SMART de discos SATA (requiere administrador).
    #[serde(default)]
    pub smart: Section<Vec<crate::hardware::smart::SmartDisk>>,
    #[serde(default)]
    pub memory_test: Section<Option<crate::hardware::MemoryTest>>,
    /// Temperaturas en el momento del análisis.
    #[serde(default)]
    pub temperatures: Section<Temperatures>,
    /// Auditoría de seguridad con nota 0-100. Ausente en análisis antiguos.
    #[serde(default)]
    pub security: Section<crate::security::Audit>,
    pub tweaks_applied: usize,
    pub findings: Vec<Finding>,
}

/// Temperaturas resumidas (lo que se guarda en el snapshot).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Temperatures {
    pub cpu: Option<f64>,
    /// (nombre, núcleo °C, punto caliente °C)
    pub gpus: Vec<(String, Option<f64>, Option<f64>)>,
    pub cpu_needs_driver: bool,
}

impl From<crate::hardware::sensors::Sensors> for Temperatures {
    fn from(s: crate::hardware::sensors::Sensors) -> Self {
        Temperatures {
            cpu: s.cpu_temp,
            gpus: s.gpus.into_iter().map(|g| (g.name, g.temperature, g.hotspot)).collect(),
            cpu_needs_driver: s.cpu_needs_driver,
        }
    }
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
    Finding { severity, area: area.into(), title, detail, actions: vec![] }
}

trait WithActions {
    fn with(self, actions: Vec<Action>) -> Self;
}

impl WithActions for Finding {
    fn with(mut self, actions: Vec<Action>) -> Self {
        self.actions = actions;
        self
    }
}

/// Convierte los datos en una lista de hallazgos priorizada (lo peor primero).
/// Cada hallazgo lleva a su detalle y a la herramienta que lo soluciona.
fn evaluate(d: &Diagnostics) -> Vec<Finding> {
    use Severity::*;
    let mut f = Vec::new();
    let sys_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let disks_detail = || page("Ver discos", "diagnostics", Some("disks"));
    let stability_detail = || page("Ver estabilidad", "diagnostics", Some("stability"));
    let security_detail = || page("Ver seguridad", "diagnostics", Some("security"));

    for v in &d.volumes {
        let pct = if v.total > 0 { v.free as f64 * 100.0 / v.total as f64 } else { 100.0 };
        let is_sys = v.mount.to_uppercase().starts_with(&sys_drive.to_uppercase());
        let sev = if pct < 10.0 { Some(Bad) } else if pct < 20.0 && is_sys { Some(Warn) } else { None };
        if let Some(sev) = sev {
            f.push(
                finding(
                    sev,
                    "Almacenamiento",
                    format!("{} con poco espacio libre ({pct:.0}%)", v.mount),
                    Some(format!(
                        "Quedan {} de {}.{}",
                        gb(v.free),
                        gb(v.total),
                        if is_sys { " Windows necesita espacio para actualizarse y paginar." } else { "" }
                    )),
                )
                .with(vec![page("Liberar espacio", "cleanup", None), tool("Almacenamiento de Windows", Tool::Storage)]),
            );
        }
    }

    if let Some(disks) = &d.disks.data {
        for k in disks {
            if !k.health.eq_ignore_ascii_case("Healthy") {
                let detail = format!("Estado operativo: {}. Hacer copia de seguridad cuanto antes.", k.operational);
                f.push(finding(Bad, "Discos", format!("{}: estado {}", k.name, k.health), Some(detail)).with(vec![disks_detail()]));
            }
            match k.wear {
                Some(w) if w >= 80 => f.push(
                    finding(Bad, "Discos", format!("{}: {w}% de vida útil consumida", k.name), Some("Planificar el reemplazo del SSD.".into()))
                        .with(vec![disks_detail()]),
                ),
                Some(w) if w >= 50 => {
                    f.push(finding(Warn, "Discos", format!("{}: {w}% de vida útil consumida", k.name), None).with(vec![disks_detail()]))
                }
                _ => {}
            }
            if let Some(t) = k.temperature.filter(|t| *t >= 60) {
                f.push(
                    finding(Warn, "Discos", format!("{}: temperatura alta ({t} °C)", k.name), Some("Revisar ventilación o disipador del SSD.".into()))
                        .with(vec![disks_detail()]),
                );
            }
            let errors = k.read_errors.unwrap_or(0) + k.write_errors.unwrap_or(0);
            if errors > 0 {
                f.push(finding(Warn, "Discos", format!("{}: {errors} errores de lectura/escritura", k.name), None).with(vec![disks_detail()]));
            }
        }
    }

    if let Some(a) = d.security.data.as_ref().filter(|a| a.score < 80) {
        let weak: Vec<&str> = a.checks.iter().filter(|c| c.status == "bad" || c.status == "warn").map(|c| c.label.as_str()).take(4).collect();
        f.push(
            finding(
                if a.score < 60 { Bad } else { Warn },
                "Seguridad",
                format!("Nota de seguridad {}/100", a.score),
                Some(format!("A mejorar: {}.", weak.join(", "))),
            )
            .with(vec![page("Ver seguridad", "security", None)]),
        );
    }

    if let Some(s) = &d.stability.data {
        if let Some(last) = s.bugchecks.first() {
            let name = last.name.clone().unwrap_or_else(|| last.code.clone());
            // Driver que más se repite como probable culpable en los minivolcados.
            let analyses: Vec<&minidump::DumpAnalysis> =
                s.minidumps.iter().flatten().filter_map(|m| m.analysis.as_ref()).filter(|a| a.culprit.is_some()).collect();
            let top = analyses
                .iter()
                .filter_map(|a| a.culprit.as_deref())
                .max_by_key(|c| analyses.iter().filter(|a| a.culprit.as_deref() == Some(*c)).count());
            let culprit = top.map(|c| {
                let times = analyses.iter().filter(|a| a.culprit.as_deref() == Some(c)).count();
                format!(" · driver probable: {c} (en {times} de {} volcados)", s.minidumps.as_ref().map_or(0, Vec::len))
            });
            let hint = top
                .and_then(|c| analyses.iter().find(|a| a.culprit.as_deref() == Some(c)))
                .and_then(|a| a.culprit_hint.clone())
                .or_else(|| last.hint.clone());
            f.push(
                finding(
                    Bad,
                    "Estabilidad",
                    format!("{} pantallazo(s) azul(es) en {} días (último: {name}){}", s.bugchecks.len(), s.days, culprit.unwrap_or_default()),
                    hint,
                )
                .with(vec![
                    stability_detail(),
                    page("Comprobar archivos (SFC)", "repair", Some("repair.sfc")),
                    tool("Monitor de confiabilidad", Tool::Reliability),
                ]),
            );
        }
        let unexpected = s.unexpected_shutdowns.len().saturating_sub(s.bugchecks.len());
        if unexpected >= 2 {
            f.push(
                finding(
                    Warn,
                    "Estabilidad",
                    format!("{unexpected} apagados inesperados en {} días", s.days),
                    Some("Cortes de corriente, botón de encendido mantenido o fuente/temperatura.".into()),
                )
                .with(vec![stability_detail(), tool("Visor de eventos", Tool::EventViewer)]),
            );
        }
        for c in s.crashes.iter().filter(|c| c.count >= 5) {
            f.push(
                finding(Warn, "Estabilidad", format!("{} se ha cerrado inesperadamente {} veces", c.app, c.count), None)
                    .with(vec![stability_detail(), tool("Monitor de confiabilidad", Tool::Reliability)]),
            );
        }
        if let Some(b) = s.boot_times.as_ref().and_then(|b| b.first()) {
            if b.ms >= 60_000 {
                f.push(
                    finding(
                        Warn,
                        "Rendimiento",
                        format!("El último arranque tardó {} s", b.ms / 1000),
                        Some("Revisar programas de inicio y el tipo de disco.".into()),
                    )
                    .with(vec![page("Revisar programas de inicio", "startup", None)]),
                );
            }
        }
    }

    if let Some(drivers) = &d.drivers.data {
        for dev in drivers {
            let sev = if dev.code == 22 { Info } else { Warn };
            f.push(
                finding(sev, "Drivers", format!("{}: {}", dev.name, dev.problem), Some(format!("Código {} · {}", dev.code, dev.device_id)))
                    .with(vec![page("Ver drivers", "diagnostics", Some("drivers")), tool("Administrador de dispositivos", Tool::DeviceManager)]),
            );
        }
    }

    if let Some(Some(b)) = &d.battery.data {
        let h = b.health();
        if h > 0.0 && h < 80.0 {
            let cycles = b.cycles.map(|c| format!(", {c} ciclos")).unwrap_or_default();
            f.push(
                finding(
                    if h < 60.0 { Bad } else { Warn },
                    "Batería",
                    format!("Batería al {h:.0}% de su capacidad original"),
                    Some(format!("{} mWh de {} mWh de fábrica{cycles}.", b.full, b.design)),
                )
                .with(vec![page("Ver batería", "diagnostics", Some("battery"))]),
            );
        }
    }

    if let Some(s) = &d.system.data {
        if s.pending_reboot {
            f.push(
                finding(Warn, "Sistema", "Hay un reinicio pendiente".into(), Some("Actualizaciones o cambios esperando a reiniciar.".into()))
                    .with(vec![security_detail(), tool("Windows Update", Tool::WindowsUpdate)]),
            );
        }
        if let Some(days) = days_since(&s.last_boot).filter(|d| *d >= 14) {
            f.push(
                finding(Info, "Sistema", format!("Sin reiniciar desde hace {days} días"), Some("Muchos problemas de lentitud se resuelven reiniciando.".into()))
                    .with(vec![security_detail()]),
            );
        }
        let update_actions =
            || vec![tool("Windows Update", Tool::WindowsUpdate), page("Reparar Windows Update", "repair", Some("repair.windows-update"))];
        match s.last_update.as_deref().and_then(days_since) {
            Some(days) if days >= 90 => f.push(
                finding(Bad, "Seguridad", format!("Última actualización hace {days} días"), Some("Revisar Windows Update.".into()))
                    .with(update_actions()),
            ),
            Some(days) if days >= 45 => {
                f.push(finding(Warn, "Seguridad", format!("Última actualización hace {days} días"), None).with(update_actions()))
            }
            _ => {}
        }
        let third_party_av = s.antivirus.iter().any(|a| !a.to_lowercase().contains("defender"));
        if s.defender_realtime == Some(false) && !third_party_av {
            f.push(
                finding(Bad, "Seguridad", "Protección en tiempo real desactivada".into(), Some("No hay otro antivirus activo.".into()))
                    .with(vec![tool("Seguridad de Windows", Tool::WindowsSecurity)]),
            );
        }
        if let Some(age) = s.signature_age_days.filter(|a| *a >= 7) {
            f.push(
                finding(Warn, "Seguridad", format!("Firmas del antivirus con {age} días de antigüedad"), None)
                    .with(vec![tool("Seguridad de Windows", Tool::WindowsSecurity)]),
            );
        }
        if s.activated == Some(false) {
            f.push(finding(Warn, "Sistema", "Windows no está activado".into(), None).with(vec![tool("Activación", Tool::Activation)]));
        }
    }

    if let Some(s) = &d.startup_enabled.data {
        if s.len() >= 12 {
            f.push(
                finding(Warn, "Rendimiento", format!("{} programas arrancan con Windows", s.len()), Some("Revisar la página Inicio.".into()))
                    .with(vec![page("Revisar programas de inicio", "startup", None)]),
            );
        }
    }
    // ---- Hardware ----
    let hw_detail = |focus: &str| page("Ver hardware", "hardware", Some(focus));
    if let Some(disks) = &d.smart.data {
        for k in disks {
            if k.predict_failure {
                f.push(finding(Bad, "Discos", format!("{}: el disco anuncia un fallo inminente (SMART)", k.model), Some("Copia de seguridad YA y reemplazo del disco.".into())).with(vec![hw_detail("smart")]));
            }
            let bad_sectors = k.reallocated.unwrap_or(0) + k.pending.unwrap_or(0) + k.uncorrectable.unwrap_or(0);
            if bad_sectors > 0 {
                f.push(
                    finding(
                        if bad_sectors >= 10 || k.pending.unwrap_or(0) > 0 { Bad } else { Warn },
                        "Discos",
                        format!("{}: {bad_sectors} sectores dañados o pendientes", k.model),
                        Some(format!(
                            "Reasignados {} · pendientes {} · no corregibles {}. El disco se está degradando: planificar el reemplazo.",
                            k.reallocated.unwrap_or(0), k.pending.unwrap_or(0), k.uncorrectable.unwrap_or(0)
                        )),
                    )
                    .with(vec![hw_detail("smart")]),
                );
            }
            if let Some(crc) = k.crc_errors.filter(|c| *c > 0) {
                f.push(finding(Warn, "Discos", format!("{}: {crc} errores de transmisión (CRC)", k.model), Some("Casi siempre es el cable SATA o el conector: cambiarlo.".into())).with(vec![hw_detail("smart")]));
            }
        }
    }
    if let Some(Some(m)) = &d.memory_test.data {
        if !m.passed {
            f.push(finding(Bad, "Memoria", "La prueba de memoria de Windows encontró errores".into(), Some("Probar los módulos de RAM uno a uno y reemplazar el defectuoso.".into())).with(vec![hw_detail("memory")]));
        }
    }
    if let Some(t) = &d.temperatures.data {
        if let Some(c) = t.cpu.filter(|c| *c >= 80.0) {
            f.push(
                finding(if c >= 90.0 { Bad } else { Warn }, "Temperatura", format!("CPU a {c:.0} °C en reposo"), Some("Limpiar el disipador y cambiar la pasta térmica.".into()))
                    .with(vec![hw_detail("sensors")]),
            );
        }
        for (name, temp, _) in &t.gpus {
            if let Some(g) = temp.filter(|g| *g >= 85.0) {
                f.push(finding(Warn, "Temperatura", format!("{name} a {g:.0} °C"), Some("Revisar ventiladores y polvo de la tarjeta gráfica.".into())).with(vec![hw_detail("sensors")]));
            }
        }
    }
    if let Some(hw) = &d.hardware.data {
        let sizes: std::collections::BTreeSet<u64> = hw.modules.iter().map(|m| m.capacity).collect();
        if hw.modules.len() == 1 && hw.ram_slots >= 2 {
            f.push(finding(Info, "Memoria", "La RAM funciona en un solo canal".into(), Some("Añadir un módulo igual al instalado duplicaría el ancho de banda de memoria.".into())).with(vec![hw_detail("memory")]));
        } else if sizes.len() > 1 {
            let list: Vec<String> = hw.modules.iter().map(|m| format!("{} GB", m.capacity / 1024u64.pow(3))).collect();
            f.push(
                finding(Info, "Memoria", "Módulos de RAM de distinto tamaño".into(), Some(format!("{} — parte de la memoria puede funcionar sin doble canal.", list.join(" + "))))
                    .with(vec![hw_detail("memory")]),
            );
        }
        if let Some(years) = hw.bios_date.as_deref().and_then(days_since).map(|d| d / 365).filter(|y| *y >= 3) {
            f.push(
                finding(Info, "Hardware", format!("BIOS de hace {years} años ({})", hw.bios_version), Some("Consultar en la web del fabricante si hay una versión más reciente (mejoras de estabilidad y seguridad).".into()))
                    .with(vec![hw_detail("board")]),
            );
        }
        for g in &hw.gpus {
            if let Some(days) = g.driver_date.as_deref().and_then(days_since).filter(|d| *d >= 365) {
                f.push(finding(Info, "Hardware", format!("Driver de {} de hace {} meses", g.name, days / 30), Some("Actualizar el driver gráfico desde la web del fabricante.".into())).with(vec![hw_detail("gpu")]));
            }
        }
    }

    if let Some(u) = d.software_updates.data.as_ref().filter(|u| !u.is_empty()) {
        let names: Vec<&str> = u.iter().take(6).map(|x| x.name.as_str()).collect();
        f.push(
            finding(
                if u.len() >= 10 { Warn } else { Info },
                "Software",
                format!("{} programas tienen actualizaciones pendientes", u.len()),
                Some(format!("{}{}", names.join(", "), if u.len() > 6 { "…" } else { "" })),
            )
            .with(vec![page("Actualizar software", "software", None)]),
        );
    }
    if let Some(days) = d.system.data.as_ref().and_then(|s| s.quick_scan_age_days).filter(|d| *d >= 14) {
        f.push(
            finding(Warn, "Seguridad", format!("Sin análisis antivirus desde hace {days} días"), None)
                .with(vec![page("Analizar ahora", "repair", Some("repair.defender-scan"))]),
        );
    }
    if let Some(b) = &d.bloat_installed.data {
        if !b.is_empty() {
            f.push(
                finding(Info, "Rendimiento", format!("{} apps promocionales o retiradas instaladas", b.len()), Some(b.join(", ")))
                    .with(vec![page("Revisar Bloatware", "bloatware", None)]),
            );
        }
    }

    f.sort_by(|a, b| b.severity.cmp(&a.severity));
    f
}

/// Abre una herramienta de Windows de la lista cerrada `Tool`.
#[tauri::command]
pub fn open_system_tool(tool: Tool) -> Result<(), String> {
    let (program, args): (&str, &[&str]) = match tool {
        Tool::DeviceManager => ("mmc.exe", &["devmgmt.msc"]),
        Tool::EventViewer => ("mmc.exe", &["eventvwr.msc"]),
        Tool::Reliability => ("perfmon.exe", &["/rel"]),
        // Los URI de Configuración se abren vía explorer para no heredar la elevación.
        Tool::WindowsUpdate => ("explorer.exe", &["ms-settings:windowsupdate"]),
        Tool::WindowsSecurity => ("explorer.exe", &["windowsdefender:"]),
        Tool::Activation => ("explorer.exe", &["ms-settings:activation"]),
        Tool::Storage => ("explorer.exe", &["ms-settings:storagesense"]),
    };
    log::info!("Abriendo herramienta {tool:?}");
    std::process::Command::new(program).args(args).spawn().map(|_| ()).map_err(|e| format!("No se pudo abrir: {e}"))
}

fn snapshots_dir(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("snapshots")
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskHealth {
    name: String,
    kind: String,
    size: u64,
    /// ok | warn | bad
    status: &'static str,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestFindings {
    timestamp: u64,
    findings: Vec<Finding>,
    security_score: Option<u32>,
    /// Fabricante y modelo del equipo.
    model: Option<String>,
    gpus: Vec<String>,
    windows: Option<String>,
    activated: Option<bool>,
    firmware: Option<String>,
    disks: Vec<DiskHealth>,
}

/// Resumen del último análisis guardado, sin volver a analizar (para el Panel).
#[tauri::command(async)]
pub fn latest_findings(app: tauri::AppHandle) -> Option<LatestFindings> {
    let d = latest_snapshot(&app)?;
    let inv = d.hardware.data.as_ref();
    let smart = d.smart.data.clone().unwrap_or_default();
    let disks = d
        .disks
        .data
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|k| {
            // Los atributos SMART (sectores dañados) mandan sobre el estado que da Windows.
            let s = smart.iter().find(|s| k.name.contains(s.model.trim()) || s.model.contains(k.name.trim()));
            let bad_sectors = s.map(|s| s.reallocated.unwrap_or(0) + s.pending.unwrap_or(0) + s.uncorrectable.unwrap_or(0)).unwrap_or(0);
            let (status, detail) = if s.is_some_and(|s| s.predict_failure) || s.is_some_and(|s| s.pending.unwrap_or(0) > 0 || s.uncorrectable.unwrap_or(0) > 0) {
                ("bad", format!("{bad_sectors} sectores dañados: haz copia de seguridad"))
            } else if k.health != "Healthy" && !k.health.is_empty() {
                ("bad", format!("Windows lo marca como «{}»", k.health))
            } else if bad_sectors > 0 || k.wear.unwrap_or(0) >= 80 {
                ("warn", if bad_sectors > 0 { format!("{bad_sectors} sectores reasignados") } else { format!("{} % de vida consumida", k.wear.unwrap_or(0)) })
            } else {
                ("ok", match (k.temperature.filter(|t| *t > 0), k.wear) {
                    (Some(t), Some(w)) => format!("{t} °C · {w} % de vida consumida"),
                    (Some(t), None) => format!("{t} °C"),
                    _ => "Sin problemas".to_string(),
                })
            };
            DiskHealth { kind: k.media_type.clone(), size: k.size, status, detail, name: k.name }
        })
        .collect();
    Some(LatestFindings {
        timestamp: d.timestamp,
        security_score: d.security.data.as_ref().map(|a| a.score),
        model: inv.map(|i| format!("{} {}", i.manufacturer, i.model).trim().to_string()).filter(|m| !m.is_empty()),
        gpus: inv.map(|i| i.gpus.iter().map(|g| g.name.clone()).collect()).unwrap_or_default(),
        windows: inv.map(|i| format!("{} {}", i.os.trim_start_matches("Microsoft "), i.os_version).trim().to_string()),
        activated: d.system.data.as_ref().and_then(|s| s.activated),
        firmware: inv.map(|i| i.firmware.clone()),
        disks,
        findings: d.findings,
    })
}

pub fn latest_snapshot(app: &tauri::AppHandle) -> Option<Diagnostics> {
    let ts = snapshot_files(&snapshots_dir(app)).into_iter().map(|(t, _)| t).max()?;
    load_snapshot(app, ts)
}

fn join<T>(r: std::thread::Result<Result<T, String>>) -> Result<T, String> {
    r.unwrap_or_else(|_| failed())
}

fn failed<T>() -> Result<T, String> {
    Err("El recolector falló".to_string())
}

fn collect(app: &tauri::AppHandle, state: &TweakState) -> Diagnostics {
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
    let (disks, stability, drivers, battery, system, startup, bloat, updates, hw, tweaks_applied, sec) = std::thread::scope(|s| {
        let disks = s.spawn(collect::disks);
        let stability = s.spawn(collect::stability);
        let drivers = s.spawn(collect::drivers);
        let battery = s.spawn(collect::battery);
        let system = s.spawn(collect::system);
        let startup = s.spawn(crate::tweaks::startup::enabled_names);
        let bloat = s.spawn(crate::tweaks::appx::recommended_installed);
        let updates = s.spawn(crate::software::list);
        let hw = s.spawn(|| {
            (
                crate::hardware::inventory(),
                crate::hardware::smart::read(),
                crate::hardware::memory_test(),
                crate::hardware::sensors::read(app).map(Temperatures::from),
            )
        });
        let tweaks = s.spawn(|| state.applied_count());
        let sec = s.spawn(|| (crate::security::extra(), crate::security::accounts()));
        (
            join(disks.join()),
            join(stability.join()),
            join(drivers.join()),
            join(battery.join()),
            join(system.join()),
            join(startup.join()),
            join(bloat.join()),
            join(updates.join()),
            hw.join().unwrap_or_else(|_| (failed(), failed(), failed(), failed())),
            tweaks.join().unwrap_or(0),
            sec.join().unwrap_or_else(|_| (failed(), None)),
        )
    });
    // La nota reutiliza lo ya recogido (antivirus, actualizaciones, programas).
    let security: Result<crate::security::Audit, String> = sec.0.map(|x| {
        let vulnerable = updates.as_ref().map(|u| u.iter().filter(|p| crate::security::is_risky(&p.id)).cloned().collect()).unwrap_or_default();
        crate::security::evaluate(system.as_ref().ok(), &x, sec.1.as_ref(), vulnerable)
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
        software_updates: updates.into(),
        hardware: hw.0.into(),
        smart: hw.1.into(),
        memory_test: hw.2.into(),
        temperatures: hw.3.into(),
        security: security.into(),
        tweaks_applied,
        findings: vec![],
    };
    d.findings = evaluate(&d);
    d
}

/// Analiza el equipo y guarda la foto (snapshot) para comparaciones.
pub fn run_and_save(app: &tauri::AppHandle, state: &TweakState) -> Diagnostics {
    let d = collect(app, state);
    save_snapshot(app, &d);
    d
}

#[tauri::command(async)]
pub fn run_diagnostics(app: tauri::AppHandle, state: State<'_, TweakState>) -> Result<Diagnostics, String> {
    Ok(run_and_save(&app, &state))
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
