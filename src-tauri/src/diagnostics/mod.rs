//! Diagnóstico del equipo: recolecta, evalúa hallazgos y guarda una "foto"
//! (snapshot) de cada análisis para comparar antes/después en el informe.

pub mod collect;
pub mod minidump;
pub(crate) mod pdf;
pub mod report;

use crate::tweaks::TweakState;
use collect::{Battery, DeviceProblem, PhysicalDisk, Stability, SystemHealth};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::Disks;
use tauri::{Emitter, State};

const MAX_SNAPSHOTS: usize = 40;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Bad,
}

/// Herramientas de Windows que la UI puede abrir (lista cerrada).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
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

/// Qué se puede hacer con un hallazgo: arreglarlo ahí mismo, ir a su detalle o
/// abrir la herramienta de Windows correspondiente.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    /// Ejecuta un ajuste o reparación del catálogo sin salir de la página.
    /// `safe`: se puede aplicar en lote («Arreglar todo lo seguro») porque no
    /// cambia el comportamiento de Windows ni borra nada del usuario.
    Fix { label: String, id: String, safe: bool },
    Page { label: String, page: String, focus: Option<String> },
    Tool { label: String, tool: Tool },
}

fn fix(label: &str, id: &str, safe: bool) -> Action {
    Action::Fix { label: label.into(), id: id.into(), safe }
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
    /// Identifica el problema sin sus cifras («C: con poco espacio» es el mismo al
    /// 8 % que al 7 %): es lo que se guarda al marcarlo como «Ya lo sé».
    #[serde(default)]
    pub key: String,
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
            Err(e) => Section { data: None, error: Some(explain_error(&e)) },
        }
    }
}

/// Lo que dice Windows cuando una lectura falla, en palabras del técnico: qué
/// pasó y qué hacer. Lo que no se reconoce se deja tal cual.
pub(crate) fn explain_error(e: &str) -> String {
    let l = e.to_lowercase();
    if l.contains("rpc") || l.contains("0x800706ba") || l.contains("0x800706be") {
        "Windows no respondió: su servicio de consultas estaba ocupado o reiniciándose. Vuelve a analizar.".into()
    } else if l.contains("tardó más de") || l.contains("timed out") || l.contains("tiempo de espera") {
        "Windows tardó demasiado en responder y se dejó de esperar. Suele pasar con el equipo muy cargado: vuelve a analizar.".into()
    } else if l.contains("denied") || l.contains("denegado") {
        "Hace falta abrir AdminOps como administrador.".into()
    } else {
        e.to_string()
    }
}

/// Algo que este análisis no pudo comprobar, y por qué. Un diagnóstico que no
/// avisa de la temperatura puede ser que esté bien o que no se pudiera leer:
/// esto dice cuál de las dos.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Unchecked {
    pub what: String,
    pub why: String,
    #[serde(default)]
    pub action: Option<Action>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
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
    /// Lo que no se pudo comprobar en este análisis, y por qué.
    #[serde(default)]
    pub unchecked: Vec<Unchecked>,
    /// Análisis rápido: solo lo que se lee en segundos. No se guarda ni se compara.
    #[serde(default)]
    pub quick: bool,
    /// Cuántos de los programas que arrancan con Windows no son de Microsoft.
    #[serde(default)]
    pub startup_third_party: Option<usize>,
    /// Versión de Windows (para saber si sigue con soporte).
    #[serde(default)]
    pub windows: Option<WindowsVersion>,
    /// Cómo de justa va la memoria según el historial de rendimiento.
    #[serde(default)]
    pub memory: Option<MemoryPressure>,
    /// Cuánto se ha llenado el disco del sistema desde un análisis anterior.
    #[serde(default)]
    pub disk_trend: Option<DiskTrend>,
    /// Qué cambió respecto al análisis anterior de este equipo (si lo hay).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<Changes>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowsVersion {
    /// 19045, 22631, 26100…
    pub build: u32,
    /// «22H2», «24H2».
    pub display: String,
    /// «Professional», «Enterprise», «EnterpriseS» (LTSC)…
    pub edition: String,
    pub product: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPressure {
    /// Uso medio, %.
    pub avg: f32,
    /// Parte del tiempo por encima del 90 %, en %.
    pub high_share: f32,
    /// Minutos medidos (con AdminOps abierta).
    pub minutes: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiskTrend {
    pub mount: String,
    /// Bytes libres de menos respecto al análisis de hace `days` días.
    pub lost: u64,
    pub days: u32,
    /// A ese ritmo, días que quedan hasta llenarse.
    pub days_left: u32,
}

/// Diferencia entre dos análisis del mismo equipo.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    /// Fecha del análisis con el que se compara.
    pub since: u64,
    /// Títulos de los problemas que antes no estaban.
    pub new: Vec<String>,
    /// Problemas que había y ya no aparecen.
    pub resolved: Vec<Finding>,
}

/// Clave de un problema sin sus cifras: «C: con poco espacio libre (8%)» y
/// «(7%)» son el mismo problema, no uno nuevo y otro resuelto.
fn problem_key(f: &Finding) -> String {
    let t: String = f.title.chars().filter(|c| !c.is_ascii_digit()).collect();
    format!("{}|{}", f.area.to_lowercase(), t.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase())
}

pub fn same_problem(a: &Finding, b: &Finding) -> bool {
    problem_key(a) == problem_key(b)
}

/// Problemas nuevos y resueltos entre `base` (antes) y `cur` (ahora). Los
/// informativos no cuentan: no son problemas.
pub fn changes(cur: &Diagnostics, base: &Diagnostics) -> Changes {
    let matters = |f: &&Finding| f.severity != Severity::Info;
    Changes {
        since: base.timestamp,
        new: cur
            .findings
            .iter()
            .filter(matters)
            .filter(|c| !base.findings.iter().any(|b| same_problem(b, c)))
            .map(|c| c.title.clone())
            .collect(),
        resolved: base
            .findings
            .iter()
            .filter(matters)
            .filter(|b| !cur.findings.iter().any(|c| same_problem(b, c)))
            .cloned()
            .collect(),
    }
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
    Finding { severity, area: area.into(), title, detail, actions: vec![], key: String::new() }
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
    let boots_detail = || page("Ver arranques y cuelgues", "machine", Some("boots"));
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
                .with(vec![
                    fix("Limpiar temporales", "cleanup.user-temp", true),
                    page("Liberar espacio", "cleanup", None),
                    tool("Almacenamiento de Windows", Tool::Storage),
                ]),
            );
        }
    }

    if let Some(disks) = &d.disks.data {
        for k in disks {
            // La causa más frecuente de un equipo lento, y la mejora que más se nota.
            if k.is_system && k.media_type.eq_ignore_ascii_case("HDD") {
                f.push(
                    finding(
                        Warn,
                        "Rendimiento",
                        "Windows está instalado en un disco mecánico".into(),
                        Some(format!("{}: con un disco de platos el equipo tarda en arrancar y en abrir cualquier cosa. Cambiarlo por un SSD es la mejora que más se nota.", k.name)),
                    )
                    .with(vec![disks_detail()]),
                );
            }
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
                    boots_detail(),
                    fix("Comprobar archivos (SFC)", "repair.sfc", false),
                    page("Ver reparaciones", "troubleshoot", Some("repairs")),
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
                .with(vec![boots_detail(), tool("Visor de eventos", Tool::EventViewer)]),
            );
        }
        // Errores de disco que Windows apunta en su registro, digan lo que digan los
        // propios datos del disco: bloques dañados (7) y sistema de archivos corrupto (55)
        // son graves; reintentos y errores de la controladora (11, 51, 153), un aviso.
        let events = |ids: &[u32]| s.disk_errors.iter().filter(|e| ids.contains(&e.id)).map(|e| e.count).sum::<u32>();
        let (grave, leve) = (events(&[7, 55]), events(&[11, 51, 153]));
        let health = || page("Salud y reparación", "space", Some("health"));
        if grave > 0 {
            f.push(
                finding(
                    Bad,
                    "Discos",
                    format!("Windows ha apuntado {grave} errores graves de disco en {} días", s.days),
                    Some("Bloques dañados o sistema de archivos corrupto. Copia de seguridad cuanto antes y revisar el disco, aunque sus propios datos digan que está bien.".into()),
                )
                .with(vec![health(), tool("Visor de eventos", Tool::EventViewer)]),
            );
        } else if leve >= 5 {
            f.push(
                finding(
                    Warn,
                    "Discos",
                    format!("{leve} errores de lectura o escritura en disco en {} días", s.days),
                    Some("Operaciones que Windows tuvo que reintentar o que la controladora dio por fallidas. Suele ser el cable, el puerto o un disco USB que se desconecta; si se repite, el disco.".into()),
                )
                .with(vec![health(), tool("Visor de eventos", Tool::EventViewer)]),
            );
        }
        for c in s.crashes.iter().filter(|c| c.count >= 5) {
            f.push(
                finding(Warn, "Estabilidad", format!("{} se ha cerrado inesperadamente {} veces", c.app, c.count), None)
                    .with(vec![stability_detail(), tool("Monitor de confiabilidad", Tool::Reliability)]),
            );
        }
        // El valor típico de los últimos arranques, no el último: uno suelto
        // lento (una actualización, un disco externo) no es que el equipo arranque mal.
        if let Some(ms) = typical_boot_ms(s.boot_times.as_deref().unwrap_or_default()).filter(|ms| *ms >= SLOW_BOOT_MS) {
            let n = s.boot_times.as_ref().map_or(0, Vec::len);
            f.push(
                finding(
                    Warn,
                    "Rendimiento",
                    format!("El equipo tarda unos {} s en arrancar", ms / 1000),
                    Some(format!("Es lo habitual en sus últimos {n} arranques. Revisar programas de inicio y el tipo de disco.")),
                )
                .with(vec![boots_detail(), page("Revisar programas de inicio", "startup", None)]),
            );
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
            || vec![tool("Windows Update", Tool::WindowsUpdate), fix("Reparar Windows Update", "repair.windows-update", false)];
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
        // Firmas y análisis son datos de Defender: con otro antivirus (Sophos, ESET…)
        // Defender queda en reposo y esas fechas se quedan viejas sin que sea un problema.
        if let Some(age) = s.signature_age_days.filter(|a| *a >= 7 && !third_party_av) {
            f.push(
                finding(Warn, "Seguridad", format!("Firmas del antivirus con {age} días de antigüedad"), None)
                    .with(vec![tool("Seguridad de Windows", Tool::WindowsSecurity)]),
            );
        }
        if s.activated == Some(false) {
            f.push(finding(Warn, "Sistema", "Windows no está activado".into(), None).with(vec![tool("Activación", Tool::Activation)]));
        }
    }

    // Solo cuentan los de terceros: lo de Microsoft viene con Windows u Office y,
    // contándolo todo, el aviso saltaba en casi todos los equipos (7 de 9 reales).
    if let Some(n) = d.startup_third_party.filter(|n| *n >= STARTUP_THIRD_PARTY) {
        f.push(
            finding(
                Warn,
                "Rendimiento",
                format!("{n} programas de terceros arrancan con Windows"),
                Some("Cada uno alarga el arranque y se queda en memoria. Desactivar los que no hagan falta desde el principio.".into()),
            )
            .with(vec![page("Revisar programas de inicio", "startup", None)]),
        );
    }

    // ---- Windows con soporte ----
    if let Some((end, name)) = d.windows.as_ref().and_then(support_end) {
        let today = chrono::Local::now().date_naive();
        let when = end.format("%d/%m/%Y");
        let update = || vec![tool("Windows Update", Tool::WindowsUpdate)];
        if end < today {
            f.push(
                finding(
                    Warn,
                    "Seguridad",
                    format!("{name} ya no tiene soporte de Microsoft"),
                    Some(format!("Dejó de recibir parches de seguridad el {when} (salvo que el equipo tenga contratadas las actualizaciones extendidas). Pasar a una versión con soporte.")),
                )
                .with(update()),
            );
        } else if (end - today).num_days() <= 90 {
            f.push(finding(Info, "Seguridad", format!("El soporte de {name} termina el {when}"), Some("Después dejará de recibir parches de seguridad: conviene actualizar antes.".into())).with(update()));
        }
    }

    // ---- Memoria, con lo medido (no con una regla fija de GB) ----
    if let Some(m) = d.memory.as_ref().filter(|m| m.avg >= 85.0 || m.high_share >= 25.0) {
        f.push(
            finding(
                Warn,
                "Memoria",
                "La memoria se queda corta".into(),
                Some(format!(
                    "De media al {:.0} %, y por encima del 90 % el {:.0} % del tiempo (medido {} h con AdminOps abierta). El equipo tiene {}: quitar lo que arranca solo o ampliarla.",
                    m.avg,
                    m.high_share,
                    (m.minutes / 60).max(1),
                    gb(d.ram_total)
                )),
            )
            .with(vec![page("Ver rendimiento", "machine", Some("performance")), page("Ver procesos", "processes", None)]),
        );
    }

    // ---- Disco que se llena ----
    if let Some(t) = d.disk_trend.as_ref().filter(|t| t.days_left <= FULL_SOON_DAYS) {
        f.push(
            finding(
                Warn,
                "Almacenamiento",
                format!("{} se está llenando: {} menos en {} días", t.mount, gb(t.lost), t.days),
                Some(format!("A este ritmo se queda sin espacio en unos {} días. Mirar qué está creciendo.", t.days_left)),
            )
            .with(vec![page("Ver qué ocupa", "space", Some("space"))]),
        );
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
            // El adaptador básico de Microsoft no es un driver antiguo: es que la
            // tarjeta gráfica no tiene el suyo.
            if is_basic_display(&g.name) {
                f.push(
                    finding(
                        Warn,
                        "Drivers",
                        "La tarjeta gráfica no tiene su driver instalado".into(),
                        Some("Windows usa su adaptador básico: sin aceleración, con la resolución limitada y el vídeo a tirones. Instalar el driver del fabricante (Intel, AMD o NVIDIA).".into()),
                    )
                    .with(vec![hw_detail("gpu"), tool("Administrador de dispositivos", Tool::DeviceManager)]),
                );
                continue;
            }
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
    let defender_is_the_av = d.system.data.as_ref().is_none_or(|s| s.antivirus.iter().all(|a| a.to_lowercase().contains("defender")));
    if let Some(days) = d.system.data.as_ref().and_then(|s| s.quick_scan_age_days).filter(|d| *d >= 14 && defender_is_the_av) {
        f.push(
            finding(Warn, "Seguridad", format!("Sin análisis antivirus desde hace {days} días"), None)
                .with(vec![fix("Analizar ahora", "repair.defender-scan", false)]),
        );
    }
    // Las apps que trae Windows (Solitaire, Xbox…) no son un hallazgo: están en
    // todos los equipos (9 de 9 reales). Tienen su sitio en Programas → Bloatware.

    f.sort_by_key(|x| std::cmp::Reverse(x.severity));
    for x in &mut f {
        x.key = problem_key(x);
    }
    f
}

/// A partir de cuántos programas de terceros al inicio merece un aviso.
const STARTUP_THIRD_PARTY: usize = 10;
/// Un disco que se llena en menos de esto, al ritmo que lleva, merece un aviso.
const FULL_SOON_DAYS: u32 = 45;

/// Cuándo deja Microsoft de dar parches a esa versión de Windows, y su nombre.
/// None si no se sabe o no aplica (LTSC, Windows Server, versiones más nuevas que
/// esta tabla). Las ediciones Enterprise y Education tienen un año más en Windows 11.
fn support_end(w: &WindowsVersion) -> Option<(chrono::NaiveDate, String)> {
    let e = w.edition.to_lowercase();
    // LTSC («EnterpriseS», «IoTEnterpriseS») y servidores van por su propio calendario.
    if (e.ends_with('s') && e.contains("enterprise")) || w.product.to_lowercase().contains("server") || w.build == 0 {
        return None;
    }
    let long = e.contains("enterprise") || e.contains("education");
    let date = |y, m, d| chrono::NaiveDate::from_ymd_opt(y, m, d);
    let (end, name) = match w.build {
        0..=10239 => (date(2023, 1, 10), "Esta versión de Windows".to_string()),
        10240..=21999 => (date(2025, 10, 14), "Windows 10".to_string()),
        22000..=22620 => (if long { date(2024, 10, 8) } else { date(2023, 10, 10) }, "Windows 11 21H2".to_string()),
        22621..=22630 => (if long { date(2025, 10, 14) } else { date(2024, 10, 8) }, "Windows 11 22H2".to_string()),
        22631..=26099 => (if long { date(2026, 11, 10) } else { date(2025, 11, 11) }, "Windows 11 23H2".to_string()),
        26100..=26199 => (if long { date(2027, 10, 12) } else { date(2026, 10, 13) }, "Windows 11 24H2".to_string()),
        26200..=26299 => (if long { date(2028, 10, 10) } else { date(2027, 10, 12) }, "Windows 11 25H2".to_string()),
        _ => return None,
    };
    Some((end?, name))
}

fn read_windows() -> Option<WindowsVersion> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let k = winreg::RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion").ok()?;
    let get = |n: &str| k.get_value::<String, _>(n).unwrap_or_default();
    Some(WindowsVersion { build: get("CurrentBuildNumber").parse().ok()?, display: get("DisplayVersion"), edition: get("EditionID"), product: get("ProductName") })
}

/// Cuánto se ha llenado un disco entre dos lecturas y cuánto le queda a ese ritmo.
/// None si no ha perdido espacio de forma apreciable (menos de 5 GB).
fn trend_of(mount: &str, before: u64, free: u64, days: f64) -> Option<DiskTrend> {
    let lost = before.checked_sub(free)?;
    if lost < 5 * 1024u64.pow(3) || days < 1.0 {
        return None;
    }
    let per_day = lost as f64 / days;
    Some(DiskTrend { mount: mount.to_string(), lost, days: days.round() as u32, days_left: (free as f64 / per_day).round() as u32 })
}

/// El disco del sistema frente al análisis más antiguo de las últimas tres semanas
/// (con al menos tres días: de un día para otro no hay tendencia que valga).
fn disk_trend(app: &tauri::AppHandle, volumes: &[Volume]) -> Option<DiskTrend> {
    let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
    let cur = volumes.iter().find(|v| v.mount.to_uppercase().starts_with(&sys))?;
    let t = now();
    let ts = snapshot_files(&snapshots_dir(app)).into_iter().map(|(ts, _)| ts).filter(|ts| (3 * 86400..=21 * 86400).contains(&t.saturating_sub(*ts))).min()?;
    let old = load_snapshot(app, ts)?;
    let before = old.volumes.iter().find(|v| v.mount.eq_ignore_ascii_case(&cur.mount))?;
    trend_of(&cur.mount, before.free, cur.free, (t - ts) as f64 / 86400.0)
}

// ---------- «Ya lo sé» ----------

/// Un hallazgo que el técnico da por sabido en este equipo («disco pendiente de
/// cambio»). Sigue saliendo en el análisis y en el informe, pero aparte y sin contar.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Accepted {
    pub key: String,
    pub title: String,
    pub reason: String,
    pub at: u64,
}

fn accepted_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("accepted-findings.json")
}

pub fn accepted(app: &tauri::AppHandle) -> Vec<Accepted> {
    std::fs::read_to_string(accepted_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_accepted(app: &tauri::AppHandle, list: &[Accepted]) -> Result<(), String> {
    let path = accepted_path(app);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&path, serde_json::to_string_pretty(list).map_err(|e| e.to_string())?).map_err(|e| format!("No se pudo guardar: {e}"))
}

#[tauri::command]
pub fn diag_accepted(app: tauri::AppHandle) -> Vec<Accepted> {
    accepted(&app)
}

#[tauri::command]
pub fn diag_accept(app: tauri::AppHandle, key: String, title: String, reason: String) -> Result<Vec<Accepted>, String> {
    if key.trim().is_empty() {
        return Err("Ese hallazgo no se puede marcar.".into());
    }
    let mut list = accepted(&app);
    list.retain(|a| a.key != key);
    list.push(Accepted { key, title: title.chars().take(200).collect(), reason: reason.trim().chars().take(200).collect(), at: now() });
    save_accepted(&app, &list)?;
    Ok(list)
}

#[tauri::command]
pub fn diag_unaccept(app: tauri::AppHandle, key: String) -> Result<Vec<Accepted>, String> {
    let mut list = accepted(&app);
    list.retain(|a| a.key != key);
    save_accepted(&app, &list)?;
    Ok(list)
}

/// Un arranque que pasa de esto es lento.
const SLOW_BOOT_MS: u64 = 60_000;

/// Lo que tarda en arrancar normalmente: la mediana de los arranques apuntados.
fn typical_boot_ms(times: &[collect::BootTime]) -> Option<u64> {
    let mut ms: Vec<u64> = times.iter().map(|b| b.ms).collect();
    ms.sort_unstable();
    ms.get(ms.len().checked_sub(1)? / 2).copied()
}

/// «Adaptador de pantalla básico de Microsoft»: lo que usa Windows cuando la
/// tarjeta gráfica no tiene su driver.
fn is_basic_display(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("basic display") || n.contains("pantalla básico") || n.contains("pantalla basico")
}

/// Lo que este análisis no pudo comprobar, con el motivo en claro.
fn unchecked(d: &Diagnostics) -> Vec<Unchecked> {
    if d.quick {
        return vec![Unchecked {
            what: "Análisis rápido".into(),
            why: "Solo se ha mirado lo que se lee en segundos. Quedan fuera las actualizaciones de programas, los sectores dañados (SMART), las piezas y temperaturas, la nota de seguridad y lo que arranca con Windows: para eso, el análisis completo.".into(),
            action: None,
        }];
    }
    let mut u = Vec::new();
    let mut add = |what: &str, why: String, action: Option<Action>| u.push(Unchecked { what: what.into(), why, action });
    let needs_admin = || "Hace falta abrir AdminOps como administrador.".to_string();

    // Secciones enteras que fallaron.
    let sections: [(&str, Option<&String>); 7] = [
        ("Salud de los discos", d.disks.error.as_ref()),
        ("Estabilidad (pantallazos, apagones, programas que fallan)", d.stability.error.as_ref()),
        ("Drivers", d.drivers.error.as_ref()),
        ("Batería", d.battery.error.as_ref()),
        ("Sistema y seguridad", d.system.error.as_ref()),
        ("Piezas del equipo", d.hardware.error.as_ref()),
        ("Nota de seguridad", d.security.error.as_ref()),
    ];
    for (what, e) in sections {
        if let Some(e) = e {
            add(what, e.clone(), None);
        }
    }
    if let Some(p) = d.system.data.as_ref().and_then(|s| s.partial.as_ref()) {
        add("Actualizaciones de Windows, antivirus y activación", p.clone(), None);
    }

    // Temperaturas: el aviso de sobrecalentamiento solo puede saltar si se leen.
    let sensors = page("Ver temperaturas", "hardware", Some("sensors"));
    match (&d.temperatures.data, &d.temperatures.error) {
        (Some(t), _) if t.cpu.is_none() => {
            let why = if !d.admin {
                needs_admin()
            } else if t.cpu_needs_driver {
                "Falta el driver PawnIO (libre y firmado) que lee los sensores de la placa. Se instala desde Hardware.".into()
            } else {
                "Este equipo no da la temperatura del procesador.".into()
            };
            add("Temperatura del procesador", why, Some(sensors));
        }
        (None, Some(e)) => add("Temperaturas", e.clone(), Some(sensors)),
        _ => {}
    }

    // SMART: muchos discos (NVMe, RAID, USB) no lo dan por la vía de Windows.
    let has_disks = d.disks.data.as_ref().is_some_and(|k| !k.is_empty());
    match (&d.smart.data, &d.smart.error) {
        (Some(v), _) if v.is_empty() && has_disks => add(
            "Sectores dañados (SMART)",
            if d.admin {
                "Estos discos no dan esos datos por la vía de Windows (habitual en NVMe, RAID y USB). Su estado general y su desgaste sí se leen: están en Salud de discos.".into()
            } else {
                needs_admin()
            },
            Some(page("Ver discos", "diagnostics", Some("disks"))),
        ),
        (None, Some(e)) => add("Sectores dañados (SMART)", e.clone(), None),
        _ => {}
    }

    if d.software_updates.data.is_none() {
        add(
            "Programas con actualización pendiente",
            "Todavía se están buscando (la primera vez puede tardar más de un minuto). Se añaden solas a este análisis en cuanto terminen.".into(),
            Some(page("Ver actualizaciones", "software", None)),
        );
    }
    if let Some(s) = &d.stability.data {
        if s.boot_times.is_none() {
            add("Cuánto tarda en arrancar", needs_admin(), None);
        }
        if s.minidumps.is_none() {
            add("Volcados de los pantallazos azules", needs_admin(), None);
        }
    }
    u
}

/// Lo que hace falta para rehacer la nota de seguridad cuando llegan las
/// actualizaciones de programas (los «programas de riesgo» salen de ahí).
type SecRaw = (Option<crate::security::Extra>, Option<crate::security::Accounts>);

/// Completa un análisis guardado sin las actualizaciones de programas: las añade
/// y rehace la nota de seguridad, los hallazgos y lo que no se pudo comprobar.
fn complete_updates(d: &mut Diagnostics, updates: Vec<crate::software::SoftwareUpdate>, sec: &SecRaw) {
    if let Some(x) = &sec.0 {
        let vulnerable = updates.iter().filter(|p| crate::security::is_risky(&p.id)).cloned().collect();
        d.security = Section { data: Some(crate::security::evaluate(d.system.data.as_ref(), x, sec.1.as_ref(), vulnerable)), error: None };
    }
    d.software_updates = Section { data: Some(updates), error: None };
    d.findings = evaluate(d);
    d.unchecked = unchecked(d);
}

/// Las actualizaciones de programas (winget) pueden tardar más de un minuto y el
/// análisis no las espera. Antes el análisis se guardaba sin ellas y así se
/// quedaba (y así salía en el informe). Ahora, cuando llegan, se añaden al
/// análisis guardado y se avisa a la interfaz.
fn complete_later(app: tauri::AppHandle, ts: u64, sec: SecRaw) {
    std::thread::spawn(move || {
        let Some(updates) = crate::software::wait_for_list(Duration::from_secs(300)) else { return };
        let Some(mut d) = load_snapshot(&app, ts) else { return };
        if d.software_updates.data.is_some() {
            return;
        }
        complete_updates(&mut d, updates, &sec);
        let dir = snapshots_dir(&app);
        if let Some(prev) = snapshot_files(&dir).into_iter().map(|(t, _)| t).filter(|t| *t < ts).max().and_then(|t| load_snapshot(&app, t)) {
            d.changes = Some(changes(&d, &prev));
        }
        save_snapshot(&app, &d);
        log::info!("Diagnóstico: actualizaciones de programas añadidas al análisis guardado");
        let _ = app.emit("diagnostics-updated", &d);
    });
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

pub fn snapshots_dir(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("snapshots")
}

/// Qué análisis guardados sobran: de los días anteriores a anteayer basta el último
/// de cada día (en un equipo real había 38 análisis de pocos días). Lo de las
/// últimas 48 horas no se toca, ni el punto de partida de una sesión en curso:
/// las sesiones y los informes comparan con un análisis concreto.
fn to_prune(timestamps: &[u64], now: u64, keep: Option<u64>) -> Vec<u64> {
    let mut last_of_day: std::collections::BTreeMap<u64, u64> = Default::default();
    let old: Vec<u64> = timestamps.iter().copied().filter(|t| now.saturating_sub(*t) > 48 * 3600).collect();
    for t in &old {
        let e = last_of_day.entry(t / 86400).or_insert(*t);
        *e = (*e).max(*t);
    }
    old.into_iter().filter(|t| last_of_day.get(&(t / 86400)) != Some(t) && Some(*t) != keep).collect()
}

fn save_snapshot(app: &tauri::AppHandle, d: &Diagnostics) {
    let dir = snapshots_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string(d) {
        let _ = std::fs::write(dir.join(format!("{}.json", d.timestamp)), json);
    }
    *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = Some((dir.clone(), std::time::Instant::now(), d.clone()));
    let all: Vec<u64> = snapshot_files(&dir).into_iter().map(|(t, _)| t).collect();
    for t in to_prune(&all, now(), crate::workflow::active_baseline(app)) {
        let _ = std::fs::remove_file(dir.join(format!("{t}.json")));
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

pub fn snapshot_files(dir: &PathBuf) -> Vec<(u64, PathBuf)> {
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
    /// Hallazgos marcados como «Ya lo sé» en este equipo (no van en `findings`).
    accepted: usize,
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
    let mut d = latest_snapshot(&app)?;
    // Lo que el técnico ya sabe de este equipo no cuenta en el Panel ni en la barra de arriba.
    let known: Vec<String> = accepted(&app).into_iter().map(|a| a.key).collect();
    let total = d.findings.len();
    d.findings.retain(|f| !known.contains(&problem_key(f)));
    let accepted_count = total - d.findings.len();
    for f in &mut d.findings {
        f.key = problem_key(f);
    }
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
        accepted: accepted_count,
        findings: d.findings,
    })
}

/// El último análisis, ya leído. El Panel y la barra de arriba lo piden cada
/// minuto, y leer y descifrar el archivo cada vez costaba 0,6 s de media (6 s en
/// el peor caso) desde un pendrive. Se renueva al guardar un análisis.
static LATEST: Mutex<Option<(PathBuf, std::time::Instant, Diagnostics)>> = Mutex::new(None);
/// Pasado este tiempo se vuelve a mirar la carpeta, por si los análisis cambiaron
/// por otro camino (restaurar una copia, otra ventana de AdminOps).
const LATEST_TRUST: std::time::Duration = std::time::Duration::from_secs(5 * 60);

pub fn latest_snapshot(app: &tauri::AppHandle) -> Option<Diagnostics> {
    let dir = snapshots_dir(app);
    if let Some((d, read, snap)) = LATEST.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        // Sigue valiendo mientras su archivo exista (la limpieza de datos puede borrarlo).
        if *d == dir && read.elapsed() < LATEST_TRUST && dir.join(format!("{}.json", snap.timestamp)).is_file() {
            return Some(snap.clone());
        }
    }
    let Some(ts) = snapshot_files(&dir).into_iter().map(|(t, _)| t).max() else {
        *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return None;
    };
    let snap = load_snapshot(app, ts)?;
    *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = Some((dir, std::time::Instant::now(), snap.clone()));
    Some(snap)
}

fn failed<T>() -> Result<T, String> {
    Err("El recolector falló".to_string())
}

type HwBundle = (Result<crate::hardware::Inventory, String>, Result<Vec<crate::hardware::smart::SmartDisk>, String>, Result<Option<crate::hardware::MemoryTest>, String>);

/// Inventario, SMART y prueba de memoria casi no cambian entre un diagnóstico y
/// el siguiente: se reutilizan unos minutos salvo que se pida "a fondo". Las
/// temperaturas nunca se cachean (son en vivo).
static HW_CACHE: Mutex<Option<(Instant, HwBundle)>> = Mutex::new(None);
const HW_CACHE_FOR: Duration = Duration::from_secs(5 * 60);

fn hardware_bundle(force: bool) -> HwBundle {
    if !force {
        if let Some((t, hw)) = HW_CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if t.elapsed() < HW_CACHE_FOR {
                return hw.clone();
            }
        }
    }
    let hw = (crate::hardware::inventory(), crate::hardware::smart::read(), crate::hardware::memory_test());
    *HW_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), hw.clone()));
    hw
}

const QUICK_SKIPPED: &str = "No se mira en el análisis rápido.";

fn skipped<T>() -> Result<T, String> {
    Err(QUICK_SKIPPED.into())
}

/// `quick`: solo lo que se lee en segundos (discos, estabilidad, drivers, batería y
/// sistema). Sin winget, SMART, piezas, temperaturas, seguridad ni programas de inicio.
fn collect(app: &tauri::AppHandle, state: &TweakState, force: bool, quick: bool) -> (Diagnostics, SecRaw) {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let mut volumes: Vec<Volume> = Disks::new_with_refreshed_list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .map(|d| Volume { mount: d.mount_point().to_string_lossy().into_owned(), total: d.total_space(), free: d.available_space() })
        .collect();
    volumes.sort_by(|a, b| a.mount.cmp(&b.mount));

    let base = Diagnostics {
        timestamp: now(),
        host: sysinfo::System::host_name().unwrap_or_default(),
        os: sysinfo::System::long_os_version().unwrap_or_default(),
        cpu: sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        ram_total: sys.total_memory(),
        admin: crate::elevation::is_elevated(),
        quick,
        windows: read_windows(),
        memory: crate::perfhistory::memory_pressure().map(|(avg, high_share, minutes)| MemoryPressure { avg, high_share, minutes }),
        disk_trend: disk_trend(app, &volumes),
        volumes,
        ..Default::default()
    };
    // Antes de esperar a los recolectores: la interfaz ya puede mostrar el equipo,
    // el veredicto se rellena con cada sección según va llegando.
    emit_progress(app, "meta", &base);

    // Todos los recolectores en paralelo: el total es el del más lento, no la
    // suma. Cada uno avisa a la interfaz en cuanto termina (evento
    // `diagnostics-progress`): las secciones se ven aparecer una a una en vez
    // de esperar a que acaben todas para mostrar algo.
    let updates_max_age = if force { Duration::ZERO } else { Duration::from_secs(10 * 60) };
    let (disks, stability, drivers, battery, system, startup, bloat, updates, hw, tweaks_applied, sec) = std::thread::scope(|s| {
        macro_rules! collect_emit {
            ($key:literal, $f:expr) => {
                s.spawn(|| {
                    let section: Section<_> = $f.into();
                    emit_progress(app, $key, &section);
                    section
                })
            };
        }
        let disks = collect_emit!("disks", collect::disks());
        let stability = collect_emit!("stability", collect::stability());
        let drivers = collect_emit!("drivers", collect::drivers());
        let battery = collect_emit!("battery", collect::battery());
        let system = collect_emit!("system", collect::system());
        let startup = s.spawn(|| {
            let r = if quick { skipped() } else { crate::tweaks::startup::enabled_overview() };
            let third_party = r.as_ref().ok().map(|x| x.1);
            let section: Section<Vec<String>> = r.map(|x| x.0).into();
            emit_progress(app, "startupEnabled", &section);
            (section, third_party)
        });
        let bloat = collect_emit!("bloatInstalled", if quick { skipped() } else { crate::tweaks::appx::recommended_installed() });
        // Sin esperar a winget: lo que haya, y si hace falta se actualiza detrás.
        let updates = collect_emit!("softwareUpdates", if quick { skipped() } else { crate::software::cached_or_refresh(updates_max_age) });
        let hw = s.spawn(|| {
            let hw = if quick { (skipped(), skipped(), skipped()) } else { hardware_bundle(force) };
            let inventory: Section<_> = hw.0.into();
            let smart: Section<_> = hw.1.into();
            let memory_test: Section<_> = hw.2.into();
            emit_progress(app, "hardware", &inventory);
            emit_progress(app, "smart", &smart);
            emit_progress(app, "memoryTest", &memory_test);
            let temperatures: Section<_> = if quick { skipped::<Temperatures>().into() } else { crate::hardware::sensors::read(app).map(Temperatures::from).into() };
            emit_progress(app, "temperatures", &temperatures);
            (inventory, smart, memory_test, temperatures)
        });
        let tweaks = s.spawn(|| if quick { 0 } else { state.applied_count() });
        let sec = s.spawn(|| if quick { (skipped(), None) } else { (crate::security::extra(), crate::security::accounts()) });
        (
            disks.join().unwrap_or_default(),
            stability.join().unwrap_or_default(),
            drivers.join().unwrap_or_default(),
            battery.join().unwrap_or_default(),
            system.join().unwrap_or_default(),
            startup.join().unwrap_or_default(),
            bloat.join().unwrap_or_default(),
            updates.join().unwrap_or_default(),
            hw.join().unwrap_or_default(),
            tweaks.join().unwrap_or(0),
            sec.join().unwrap_or_else(|_| (failed(), None)),
        )
    });
    let sec_raw: SecRaw = (sec.0.clone().ok(), sec.1.clone());
    // La nota reutiliza lo ya recogido (antivirus, actualizaciones, programas).
    let security: Result<crate::security::Audit, String> = sec.0.map(|x| {
        let vulnerable = updates.data.iter().flatten().filter(|p| crate::security::is_risky(&p.id)).cloned().collect();
        crate::security::evaluate(system.data.as_ref(), &x, sec.1.as_ref(), vulnerable)
    });
    let security: Section<_> = security.into();
    emit_progress(app, "security", &security);

    let mut d = Diagnostics {
        disks,
        stability,
        drivers,
        battery,
        system,
        startup_enabled: startup.0,
        startup_third_party: startup.1,
        bloat_installed: bloat,
        software_updates: updates,
        hardware: hw.0,
        smart: hw.1,
        memory_test: hw.2,
        temperatures: hw.3,
        security,
        tweaks_applied,
        findings: vec![],
        ..base
    };
    d.findings = evaluate(&d);
    d.unchecked = unchecked(&d);
    (d, sec_raw)
}

/// Avisa a la interfaz de que una sección del diagnóstico ya está lista, para
/// que se pinte al momento en vez de esperar a que terminen todas.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress<'a, T: Serialize> {
    key: &'a str,
    section: &'a T,
}

// Manual: el derive añadiría `T: Clone` aunque solo se guarda una referencia,
// que siempre es Clone.
impl<T: Serialize> Clone for Progress<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Serialize> Copy for Progress<'_, T> {}

fn emit_progress<T: Serialize>(app: &tauri::AppHandle, key: &'static str, section: &T) {
    let _ = app.emit("diagnostics-progress", Progress { key, section });
}

/// Analiza el equipo y guarda la foto (snapshot) para comparaciones.
/// `force`: ignora la caché de hardware y de actualizaciones (winget) y las
/// vuelve a consultar; úsalo solo cuando el técnico pide expresamente un
/// análisis a fondo, no en cada sesión o comparación interna.
pub fn run_and_save(app: &tauri::AppHandle, state: &TweakState, force: bool) -> Diagnostics {
    let (mut d, sec) = collect(app, state, force, false);
    if let Some(prev) = latest_snapshot(app).filter(|p| p.timestamp < d.timestamp) {
        d.changes = Some(changes(&d, &prev));
    }
    save_snapshot(app, &d);
    if d.software_updates.data.is_none() {
        complete_later(app.clone(), d.timestamp, sec);
    }
    d
}

#[tauri::command(async)]
pub fn run_diagnostics(app: tauri::AppHandle, state: State<'_, TweakState>, force: Option<bool>, quick: Option<bool>) -> Result<Diagnostics, String> {
    // Visible en el indicador de tareas mientras dura.
    let _task = crate::task::Task::new(&app, "diagnostics").named("Diagnóstico del equipo");
    if quick.unwrap_or(false) {
        // Una primera mirada: ni se guarda ni se compara con los anteriores (le
        // faltan secciones, y parecería que sus problemas se han «resuelto»).
        return Ok(collect(&app, &state, false, true).0);
    }
    Ok(run_and_save(&app, &state, force.unwrap_or(false)))
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
    v.sort_by_key(|s| std::cmp::Reverse(s.timestamp));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(sev: Severity, area: &str, title: &str) -> Finding {
        Finding { severity: sev, area: area.into(), title: title.into(), detail: None, actions: vec![], key: String::new() }
    }

    fn diag(ts: u64, findings: Vec<Finding>) -> Diagnostics {
        Diagnostics { timestamp: ts, findings, ..Default::default() }
    }

    #[test]
    fn changes_ignore_numbers_and_info() {
        let before = diag(1, vec![
            f(Severity::Warn, "Almacenamiento", "C:\\ con poco espacio libre (8%)"),
            f(Severity::Bad, "Seguridad", "Firewall desactivado"),
            f(Severity::Info, "Sistema", "Hay 3 ajustes aplicados"),
        ]);
        let after = diag(2, vec![
            f(Severity::Warn, "Almacenamiento", "C:\\ con poco espacio libre (7%)"),
            f(Severity::Bad, "Estabilidad", "2 pantallazos azules"),
            f(Severity::Info, "Sistema", "Hay 5 ajustes aplicados"),
        ]);
        let c = changes(&after, &before);
        assert_eq!(c.since, 1);
        assert_eq!(c.new, vec!["2 pantallazos azules".to_string()]);
        assert_eq!(c.resolved.len(), 1);
        assert_eq!(c.resolved[0].title, "Firewall desactivado");
    }

    fn boots(secs: &[u64]) -> Vec<collect::BootTime> {
        secs.iter().map(|s| collect::BootTime { time: String::new(), ms: s * 1000 }).collect()
    }

    fn stability(boot_secs: Option<&[u64]>) -> Stability {
        Stability { days: 30, bugchecks: vec![], unexpected_shutdowns: vec![], crashes: vec![], minidumps: Some(vec![]), boot_times: boot_secs.map(boots), disk_errors: vec![] }
    }

    fn ok<T>(data: T) -> Section<T> {
        Section { data: Some(data), error: None }
    }

    /// Un arranque suelto lento no es que el equipo arranque mal: cuenta lo habitual.
    /// (Los datos son de equipos reales: 81, 107, 59 y 45 s; y 40, 48, 106 y 41 s.)
    #[test]
    fn slow_boot_uses_the_typical_boot_not_the_last() {
        assert_eq!(typical_boot_ms(&boots(&[40, 48, 106, 41])), Some(41_000));
        assert_eq!(typical_boot_ms(&boots(&[81, 107, 59, 45])), Some(59_000));
        assert_eq!(typical_boot_ms(&boots(&[143, 157, 177, 183])), Some(157_000));
        assert_eq!(typical_boot_ms(&[]), None);

        let slow = |secs: &[u64]| {
            let d = Diagnostics { stability: ok(stability(Some(secs))), ..Default::default() };
            evaluate(&d).into_iter().any(|f| f.area == "Rendimiento" && f.title.contains("arrancar"))
        };
        // El último (el primero de la lista) tardó 106 s, pero lo normal son 41-48: no avisa.
        assert!(!slow(&[106, 40, 48, 41]));
        assert!(slow(&[143, 157, 177, 183]));
    }

    #[test]
    fn missing_graphics_driver_is_a_warning_not_an_old_driver() {
        let gpu = |name: &str| crate::hardware::Gpu { name: name.into(), driver_date: Some("2006-06-21T00:00:00".into()), ..Default::default() };
        let findings = |name: &str| {
            let d = Diagnostics { hardware: ok(crate::hardware::Inventory { gpus: vec![gpu(name)], ..Default::default() }), ..Default::default() };
            evaluate(&d)
        };
        let basic = findings("Adaptador de pantalla básico de Microsoft");
        let f = basic.iter().find(|f| f.title.contains("tarjeta gráfica")).expect("debe avisar");
        assert_eq!(f.severity, Severity::Warn);
        assert!(!basic.iter().any(|f| f.title.starts_with("Driver de")), "no es un driver antiguo");
        assert!(findings("Microsoft Basic Display Adapter").iter().any(|f| f.title.contains("tarjeta gráfica")));
        // Una gráfica con su driver, aunque sea viejo, sigue siendo solo un dato.
        let intel = findings("Intel(R) UHD Graphics 630");
        assert!(intel.iter().all(|f| !f.title.contains("no tiene su driver")));
    }

    #[test]
    fn errors_are_explained() {
        assert!(explain_error("El servidor RPC no está disponible").starts_with("Windows no respondió"));
        assert!(explain_error("PowerShell tardó más de 120 s y se detuvo.").starts_with("Windows tardó demasiado"));
        assert!(explain_error("Access is denied").contains("administrador"));
        assert_eq!(explain_error("Otra cosa"), "Otra cosa");
        // Una sección que falla guarda ya la explicación.
        let s: Section<u8> = Err("El servidor RPC no está disponible".to_string()).into();
        assert!(s.error.unwrap().starts_with("Windows no respondió"));
    }

    #[test]
    fn says_what_could_not_be_checked_and_why() {
        let mut d = Diagnostics {
            admin: true,
            disks: ok(vec![]),
            stability: ok(stability(Some(&[30]))),
            drivers: ok(vec![]),
            battery: ok(None),
            hardware: ok(Default::default()),
            temperatures: ok(Temperatures { cpu: Some(50.0), ..Default::default() }),
            smart: ok(vec![]),
            software_updates: ok(vec![]),
            ..Default::default()
        };
        d.system = Section { data: None, error: Some(explain_error("PowerShell tardó más de 120 s y se detuvo.")) };
        d.security = Section { data: None, error: None };
        let u = unchecked(&d);
        assert_eq!(u.len(), 1, "{u:#?}");
        assert_eq!(u[0].what, "Sistema y seguridad");

        // Sin temperatura por falta del driver, con discos que no dan SMART y sin administrador para los arranques.
        d.system = Section { data: None, error: None };
        d.temperatures = ok(Temperatures { cpu: None, gpus: vec![], cpu_needs_driver: true });
        d.disks = ok(vec![PhysicalDisk { name: "NVMe".into(), media_type: "SSD".into(), bus_type: "NVMe".into(), health: "Healthy".into(), operational: "OK".into(), size: 1, temperature: None, wear: None, power_on_hours: None, read_errors: None, write_errors: None, is_system: true }]);
        d.stability = ok(stability(None));
        d.software_updates = Section { data: None, error: Some("x".into()) };
        let whats: Vec<String> = unchecked(&d).into_iter().map(|x| x.what).collect();
        assert_eq!(whats, ["Temperatura del procesador", "Sectores dañados (SMART)", "Programas con actualización pendiente", "Cuánto tarda en arrancar"]);
        assert!(unchecked(&d)[0].why.contains("PawnIO"));
    }

    /// Las actualizaciones que llegan después se añaden al análisis y cuentan.
    #[test]
    fn late_software_updates_complete_the_analysis() {
        let mut d = Diagnostics { software_updates: Section { data: None, error: Some("buscando".into()) }, ..Default::default() };
        d.unchecked = unchecked(&d);
        assert!(d.unchecked.iter().any(|u| u.what.contains("actualización")));
        let up = |i: usize| crate::software::SoftwareUpdate { name: format!("Programa {i}"), id: format!("Vendor.App{i}"), version: "1".into(), available: "2".into(), source: "winget".into() };
        complete_updates(&mut d, (0..12).map(up).collect(), &(None, None));
        assert_eq!(d.software_updates.data.as_ref().map(Vec::len), Some(12));
        assert!(d.findings.iter().any(|f| f.area == "Software" && f.title.starts_with("12 programas")));
        assert!(!d.unchecked.iter().any(|u| u.what.contains("actualización")));
    }

    fn disk(name: &str, media: &str, system: bool) -> PhysicalDisk {
        PhysicalDisk { name: name.into(), media_type: media.into(), bus_type: "SATA".into(), health: "Healthy".into(), operational: "OK".into(), size: 1, temperature: None, wear: None, power_on_hours: None, read_errors: None, write_errors: None, is_system: system }
    }

    fn titles(d: &Diagnostics) -> Vec<String> {
        evaluate(d).into_iter().map(|f| f.title).collect()
    }

    /// Un PC de oficina normal, como los 9 reales: apps de Windows instaladas, una
    /// docena de programas al inicio (casi todos de Microsoft), RAM en un módulo,
    /// BIOS y driver gráfico de hace años. Antes daba 7 hallazgos; ahora, ningún
    /// aviso: solo sugerencias. Si alguien sube el ruido, esta prueba lo dice.
    #[test]
    fn a_healthy_office_pc_raises_no_warnings() {
        let hw = crate::hardware::Inventory {
            ram_slots: 2,
            modules: vec![Default::default()],
            bios_date: Some("2019-03-01T00:00:00".into()),
            bios_version: "1.2".into(),
            gpus: vec![crate::hardware::Gpu { name: "Intel(R) UHD Graphics 630".into(), driver_date: Some("2022-01-01T00:00:00".into()), ..Default::default() }],
            ..Default::default()
        };
        let d = Diagnostics {
            admin: true,
            volumes: vec![Volume { mount: "C:\\".into(), total: 500, free: 300 }],
            disks: ok(vec![disk("SSD de oficina", "SSD", true)]),
            stability: ok(stability(Some(&[35, 40, 38]))),
            drivers: ok(vec![]),
            startup_enabled: ok((0..14).map(|i| format!("Programa {i}")).collect()),
            startup_third_party: Some(4),
            bloat_installed: ok(vec!["Solitaire Collection".into(), "Xbox".into(), "Copilot".into()]),
            hardware: ok(hw),
            windows: Some(WindowsVersion { build: 26200, display: "25H2".into(), edition: "Professional".into(), product: "Windows 11 Pro".into() }),
            ..Default::default()
        };
        let f = evaluate(&d);
        let serious: Vec<&Finding> = f.iter().filter(|x| x.severity != Severity::Info).collect();
        assert!(serious.is_empty(), "un equipo sano no debe dar avisos: {:#?}", serious.iter().map(|x| &x.title).collect::<Vec<_>>());
        // Lo que no es problema sale como sugerencia, y las apps de Windows ni eso.
        assert!(f.iter().any(|x| x.title.contains("un solo canal")));
        assert!(!f.iter().any(|x| x.title.contains("promocionales")));
        assert!(f.iter().all(|x| !x.key.is_empty()), "todos llevan su clave");
    }

    #[test]
    fn startup_warning_counts_third_party_only() {
        let d = |n| Diagnostics { startup_enabled: ok((0..20).map(|i| format!("P{i}")).collect()), startup_third_party: n, ..Default::default() };
        assert!(titles(&d(Some(4))).is_empty());
        assert_eq!(titles(&d(Some(11))), ["11 programas de terceros arrancan con Windows"]);
        // Un análisis antiguo, sin ese dato, no avisa a ciegas.
        assert!(titles(&d(None)).is_empty());
    }

    #[test]
    fn windows_on_a_mechanical_disk_and_disk_errors_in_the_log() {
        let hdd = Diagnostics { disks: ok(vec![disk("WDC WD10EZEX", "HDD", true), disk("Datos", "HDD", false)]), ..Default::default() };
        assert_eq!(titles(&hdd), ["Windows está instalado en un disco mecánico"]);
        assert!(titles(&Diagnostics { disks: ok(vec![disk("NVMe", "SSD", true), disk("Datos", "HDD", false)]), ..Default::default() }).is_empty());

        let with = |events: Vec<(u32, u32)>| {
            let mut s = stability(Some(&[30]));
            s.disk_errors = events.into_iter().map(|(id, count)| collect::DiskEvent { id, count, last: String::new() }).collect();
            evaluate(&Diagnostics { stability: ok(s), ..Default::default() })
        };
        let grave = with(vec![(7, 2), (153, 40)]);
        assert_eq!((grave.len(), grave[0].severity), (1, Severity::Bad));
        assert!(grave[0].title.contains("2 errores graves"));
        let leve = with(vec![(153, 6)]);
        assert_eq!((leve.len(), leve[0].severity), (1, Severity::Warn));
        // Un reintento suelto no es un aviso.
        assert!(with(vec![(153, 2)]).is_empty());
    }

    #[test]
    fn windows_support_dates() {
        let w = |build, edition: &str| WindowsVersion { build, display: String::new(), edition: edition.into(), product: "Windows".into() };
        let end = |build, edition: &str| support_end(&w(build, edition)).map(|(d, n)| (d.to_string(), n));
        assert_eq!(end(19045, "Professional"), Some(("2025-10-14".into(), "Windows 10".into())));
        assert_eq!(end(22631, "Professional"), Some(("2025-11-11".into(), "Windows 11 23H2".into())));
        assert_eq!(end(22631, "Enterprise"), Some(("2026-11-10".into(), "Windows 11 23H2".into())));
        assert_eq!(end(26200, "Core").map(|x| x.0), Some("2027-10-12".into()));
        // LTSC, servidores y versiones que esta tabla aún no conoce: no se opina.
        assert_eq!(end(19044, "EnterpriseS"), None);
        assert_eq!(end(30000, "Professional"), None);
        assert_eq!(support_end(&WindowsVersion { build: 20348, display: String::new(), edition: "ServerStandard".into(), product: "Windows Server 2022".into() }), None);
        // Windows 10 sin soporte es un aviso.
        let d = Diagnostics { windows: Some(w(19045, "Professional")), ..Default::default() };
        assert_eq!(titles(&d), ["Windows 10 ya no tiene soporte de Microsoft"]);
    }

    #[test]
    fn memory_and_filling_disk() {
        let mem = |avg, high_share| Diagnostics { ram_total: 8 << 30, memory: Some(MemoryPressure { avg, high_share, minutes: 600 }), ..Default::default() };
        assert_eq!(titles(&mem(88.0, 10.0)), ["La memoria se queda corta"]);
        assert_eq!(titles(&mem(70.0, 30.0)), ["La memoria se queda corta"]);
        assert!(titles(&mem(60.0, 5.0)).is_empty());

        const GB: u64 = 1 << 30;
        // 20 GB menos en 10 días con 30 GB libres: 15 días para llenarse.
        let t = trend_of("C:\\", 50 * GB, 30 * GB, 10.0).unwrap();
        assert_eq!((t.days, t.days_left), (10, 15));
        assert!(titles(&Diagnostics { disk_trend: Some(t), ..Default::default() })[0].contains("se está llenando"));
        // Poco perdido, o espacio que crece: no hay tendencia que contar.
        assert_eq!(trend_of("C:\\", 50 * GB, 48 * GB, 10.0), None);
        assert_eq!(trend_of("C:\\", 30 * GB, 50 * GB, 10.0), None);
        // Se llena, pero a un ritmo que da para meses: no avisa.
        let slow = trend_of("C:\\", 400 * GB, 390 * GB, 10.0).unwrap();
        assert!(titles(&Diagnostics { disk_trend: Some(slow), ..Default::default() }).is_empty());
    }

    #[test]
    fn quick_analysis_says_what_it_skipped() {
        let d = Diagnostics { quick: true, ..Default::default() };
        let u = unchecked(&d);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].what, "Análisis rápido");
    }

    /// De los días viejos queda el último de cada día; lo reciente y el punto de
    /// partida de una sesión no se tocan.
    #[test]
    fn old_snapshots_keep_one_per_day() {
        const DAY: u64 = 86400;
        let now = 100 * DAY + 3600;
        let old_day = |n: u64, secs: u64| (100 - n) * DAY + secs;
        let all = vec![
            old_day(5, 100), old_day(5, 200), old_day(5, 300), // hace 5 días: tres
            old_day(4, 100), // hace 4: uno
            old_day(1, 100), old_day(1, 200), // ayer: dentro de las 48 h
            now - 60,
        ];
        assert_eq!(to_prune(&all, now, None), vec![old_day(5, 100), old_day(5, 200)]);
        // El punto de partida de la sesión en curso se conserva aunque sea viejo.
        assert_eq!(to_prune(&all, now, Some(old_day(5, 100))), vec![old_day(5, 200)]);
        assert!(to_prune(&[], now, None).is_empty());
    }

    #[test]
    fn parses_powershell_dates_with_and_without_zone() {
        assert!(super::parse_time("2026-09-26T08:56:22.5000000-04:00").is_some());
        assert!(super::parse_time("2026-09-16T00:00:00.0000000").is_some());
        assert!(super::parse_time("basura").is_none());
    }
}

#[cfg(test)]
mod calibrate {
    use super::*;

    /// Vuelve a pasar las reglas de hoy por análisis ya guardados y dice qué
    /// cambia: para medir un cambio de reglas con equipos reales en vez de
    /// adivinarlo. Solo lee. `ADMINOPS_SNAPSHOTS` es una lista de carpetas de
    /// snapshots separadas por «;»:
    /// `ADMINOPS_SNAPSHOTS="G:\...\snapshots;C:\...\snapshots" cargo test recalibrate -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn recalibrate() {
        let dirs = std::env::var("ADMINOPS_SNAPSHOTS").expect("falta ADMINOPS_SNAPSHOTS");
        let key = |f: &Finding| {
            let t: String = f.title.chars().map(|c| if c.is_ascii_digit() { 'N' } else { c }).collect();
            let t = if f.area == "Drivers" { t.rsplit(": ").next().unwrap_or("").to_string() } else { t };
            format!("{}: {t}", f.area)
        };
        let (mut before, mut after, mut n) = (0usize, 0usize, 0usize);
        let mut gone: std::collections::BTreeMap<String, usize> = Default::default();
        let mut new: std::collections::BTreeMap<String, usize> = Default::default();
        let mut unchecked_seen: std::collections::BTreeMap<String, usize> = Default::default();
        for dir in dirs.split(';').filter(|d| !d.trim().is_empty()) {
            // El más reciente de cada equipo.
            let Some((_, path)) = snapshot_files(&PathBuf::from(dir.trim())).into_iter().max_by_key(|(t, _)| *t) else { continue };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(mut d) = serde_json::from_str::<Diagnostics>(&text) else {
                println!("(un análisis antiguo no se pudo leer)");
                continue;
            };
            n += 1;
            let old: Vec<String> = d.findings.iter().map(key).collect();
            // Lo que hoy se filtra al recoger los datos.
            if let Some(v) = d.drivers.data.as_mut() {
                v.retain(|x| !(x.code == 24 && x.device_id.to_ascii_uppercase().starts_with("ACPI\\")));
            }
            let fresh: Vec<String> = evaluate(&d).iter().map(key).collect();
            before += old.len();
            after += fresh.len();
            for k in old.iter().filter(|k| !fresh.contains(k)) {
                *gone.entry(k.clone()).or_default() += 1;
            }
            for k in fresh.iter().filter(|k| !old.contains(k)) {
                *new.entry(k.clone()).or_default() += 1;
            }
            for u in unchecked(&d) {
                *unchecked_seen.entry(u.what).or_default() += 1;
            }
        }
        println!("{n} equipos · hallazgos antes {before} · ahora {after}");
        println!("Dejan de salir:");
        for (k, c) in &gone {
            println!("  {c}  {k}");
        }
        println!("Salen nuevos:");
        for (k, c) in &new {
            println!("  {c}  {k}");
        }
        println!("«No se pudo comprobar» (en cuántos equipos):");
        for (k, c) in &unchecked_seen {
            println!("  {c}  {k}");
        }
    }
}

#[cfg(test)]
mod pool_bench {
    /// Cuánto tarda cada parte del diagnóstico por separado (pool ya caliente):
    /// `cargo test --release collector_times -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn collector_times() {
        let _ = crate::pspool::query("1", None, "calentar");
        let time = |name: &str, f: &dyn Fn() -> bool| {
            let t = std::time::Instant::now();
            let ok = f();
            println!("{:>7} ms  {name}{}", t.elapsed().as_millis(), if ok { "" } else { "  (falló)" });
        };
        time("seguridad: extra (nuevo, en paralelo)", &|| crate::security::extra().is_ok());
        time("seguridad: cuentas", &|| crate::security::accounts().is_some());
        time("actualizaciones (winget, sin caché)", &|| crate::software::list().is_ok());
        time("actualizaciones (con caché caliente)", &|| crate::software::cached_or_list(std::time::Duration::from_secs(600)).is_ok());
        time("hardware: inventario", &|| crate::hardware::inventory().is_ok());
        time("hardware: SMART", &|| crate::hardware::smart::read().is_ok());
    }

    /// Tiempo de los recolectores con PowerShell en paralelo, como en el diagnóstico:
    /// `ADMINOPS_PS_HOSTS=3 cargo test --release diag_collectors_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn diag_collectors_cost() {
        // Calentar el pool para medir solo el trabajo.
        let _ = crate::pspool::query("1", None, "calentar");
        let t = std::time::Instant::now();
        std::thread::scope(|s| {
            let h = [
                s.spawn(|| super::collect::disks().is_ok()),
                s.spawn(|| super::collect::stability().is_ok()),
                s.spawn(|| super::collect::drivers().is_ok()),
                s.spawn(|| super::collect::battery().is_ok()),
                s.spawn(|| super::collect::system().is_ok()),
                s.spawn(|| crate::tweaks::startup::enabled_names().is_ok()),
                s.spawn(|| crate::tweaks::appx::recommended_installed().is_ok()),
                s.spawn(|| crate::security::extra().is_ok()),
                s.spawn(|| crate::security::accounts().is_some()),
            ];
            for x in h {
                let _ = x.join();
            }
        });
        println!("hosts {:?}: {:.1} s", std::env::var("ADMINOPS_PS_HOSTS").ok(), t.elapsed().as_secs_f64());
    }
}
