//! Flujo de trabajo del técnico: ajustes (marca, checklist, precios), clientes y
//! sesiones de servicio (diagnóstico "antes" → trabajo → informe "después"),
//! con presupuesto o recibo, firma del cliente, garantías y próximo mantenimiento.

use crate::diagnostics;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn new_id() -> String {
    format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos())
}

const DAY: u64 = 86_400;

// ---------- Ajustes del técnico ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub technician: String,
    pub company: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    /// Logo como data URL (image/png, jpeg, webp o svg).
    pub logo: Option<String>,
    /// Condiciones o garantía que aparecen al pie del informe.
    pub conditions: String,
    pub checklist: Vec<String>,
    /// Ya pasó por el asistente de primer arranque.
    pub onboarded: bool,
    /// Dominio que se propone al unir equipos (p. ej. empresa.local).
    pub default_domain: String,
    /// Moneda de presupuestos y recibos (símbolo o código: RD$, €, USD…).
    pub currency: String,
    /// Nombre y porcentaje del impuesto (ITBIS 18, IVA 21…). 0: sin impuesto.
    pub tax_name: String,
    pub tax_rate: f64,
    /// Garantía de la mano de obra, en días (0: sin garantía).
    pub labor_warranty_days: u32,
    /// Cada cuántos meses se recomienda el próximo mantenimiento (0: no se recomienda).
    pub maintenance_months: u32,
    /// Días de validez de un presupuesto.
    pub quote_validity_days: u32,
    /// Servicios y piezas habituales con su precio, para añadirlos con un clic.
    pub catalog: Vec<CatalogItem>,
    /// Firma del técnico (data URL PNG) que aparece en los informes.
    pub tech_signature: Option<String>,
    /// Notificación de Windows al terminar una tarea larga con la app en segundo plano.
    pub notify_tasks: bool,
    /// Punto de restauración antes de cambiar el sistema: risky | always | never.
    pub restore_points: String,
    /// Al abrir, borrar historial, análisis e informes con más de estos meses (0: nunca).
    pub auto_cleanup_months: u32,
    /// Al abrir, comprobar si hay una versión nueva en GitHub.
    pub check_updates: bool,
    /// Vigilar el Visor de eventos y avisar de errores típicos de Windows.
    pub watch_windows: bool,
    /// De qué avisos llega además notificación de Windows: all | bad | none.
    pub notify_alerts: String,
    /// La X de la ventana minimiza en vez de cerrar (se sale con el botón «Salir»).
    pub close_minimizes: bool,
    /// Icono junto al reloj, con lo habitual a un clic derecho.
    pub tray_icon: bool,
    /// Tipos de visita con su checklist (mantenimiento, equipo nuevo…).
    pub visit_types: Vec<VisitType>,
    /// La plantilla de informe propia: qué secciones lleva y en qué orden.
    pub report_layout: ReportLayout,
}

/// Un tipo de visita y lo que hay que hacer en ella.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VisitType {
    pub name: String,
    pub items: Vec<ChecklistTemplate>,
}

/// Punto de la checklist. `auto`: tarea de AdminOps que lo marca sola al hacerse
/// (cleanup, updates, diagnostic…); vacío si se marca a mano.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChecklistTemplate {
    pub text: String,
    pub auto: String,
}

fn visit(name: &str, items: &[(&str, &str)]) -> VisitType {
    VisitType { name: name.into(), items: items.iter().map(|(t, a)| ChecklistTemplate { text: (*t).into(), auto: (*a).into() }).collect() }
}

pub fn default_visit_types() -> Vec<VisitType> {
    vec![
        visit(
            "Mantenimiento",
            &[
                ("Diagnóstico completo", "diagnostic"),
                ("Punto de restauración", "restorepoint"),
                ("Limpieza de temporales", "cleanup"),
                ("Programas actualizados", "updates"),
                ("Programas de inicio revisados", "startup"),
                ("Análisis antivirus", "antivirus"),
                ("Limpieza de polvo y ventiladores", ""),
                ("Prueba de funcionamiento con el cliente", ""),
            ],
        ),
        visit(
            "Equipo nuevo",
            &[
                ("Quitar programas preinstalados (bloatware)", "bloatware"),
                ("Instalar los programas del cliente", "install"),
                ("Crear el usuario", "users"),
                ("Unir al dominio", "domain"),
                ("Ajustes de privacidad", "privacy"),
                ("Windows y programas actualizados", "updates"),
                ("Copia de seguridad de drivers", "drivers"),
                ("Configurar correo e impresora", ""),
                ("Entregar contraseñas al cliente", ""),
            ],
        ),
        visit(
            "Equipo lento",
            &[
                ("Diagnóstico completo", "diagnostic"),
                ("Programas de inicio revisados", "startup"),
                ("Limpieza de temporales", "cleanup"),
                ("Ajustes de rendimiento", "performance"),
                ("Programas actualizados", "updates"),
                ("Análisis antivirus", "antivirus"),
                ("Revisar temperaturas y disco", ""),
            ],
        ),
        visit(
            "Sin Internet o red",
            &[
                ("Reparar la red", "network"),
                ("Revisar cable, Wi-Fi y router", ""),
                ("Comprobar navegación y correo", ""),
            ],
        ),
        visit(
            "Copia o traspaso de datos",
            &[
                ("Copia de los datos del usuario", "backup"),
                ("Comprobar que la copia se abre", ""),
                ("Restaurar en el equipo nuevo", ""),
            ],
        ),
    ]
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogItem {
    pub name: String,
    pub price: f64,
    /// Pieza (lleva garantía propia) o servicio.
    pub part: bool,
    pub warranty_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            technician: String::new(),
            company: String::new(),
            phone: String::new(),
            email: String::new(),
            website: String::new(),
            logo: None,
            conditions: String::new(),
            checklist: [
                "Copia de seguridad de los datos del cliente",
                "Antivirus activo y actualizado",
                "Windows Update sin actualizaciones pendientes",
                "Espacio libre suficiente en el disco del sistema",
                "Programas de inicio revisados",
                "Drivers sin errores",
                "Prueba de funcionamiento con el cliente",
            ]
            .map(String::from)
            .to_vec(),
            onboarded: false,
            default_domain: String::new(),
            currency: "RD$".into(),
            tax_name: "ITBIS".into(),
            tax_rate: 18.0,
            labor_warranty_days: 30,
            maintenance_months: 6,
            quote_validity_days: 15,
            catalog: [
                "Diagnóstico y revisión general",
                "Limpieza y optimización del sistema",
                "Eliminación de virus y programas no deseados",
                "Instalación y configuración de Windows",
                "Copia de seguridad y traspaso de datos",
                "Instalación de programas",
            ]
            .map(|name| CatalogItem { name: name.into(), ..Default::default() })
            .to_vec(),
            tech_signature: None,
            notify_tasks: true,
            restore_points: "risky".into(),
            auto_cleanup_months: 0,
            check_updates: true,
            watch_windows: true,
            notify_alerts: "all".into(),
            close_minimizes: false,
            tray_icon: false,
            visit_types: default_visit_types(),
            report_layout: ReportLayout::default(),
        }
    }
}

fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("settings.json")
}

pub fn settings(app: &tauri::AppHandle) -> Settings {
    crate::paths::read_json(&settings_path(app))
}

#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> Settings {
    settings(&app)
}

/// Firma dibujada en pantalla: PNG en data URL y de tamaño razonable.
fn check_signature(sig: Option<&str>) -> Result<(), String> {
    match sig {
        Some(s) if !s.starts_with("data:image/png;base64,") => Err("La firma no es una imagen válida.".into()),
        Some(s) if s.len() > 400_000 => Err("La firma es demasiado grande.".into()),
        _ => Ok(()),
    }
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    if let Some(logo) = &settings.logo {
        let ok_type = ["data:image/png;", "data:image/jpeg;", "data:image/webp;", "data:image/svg+xml;"].iter().any(|p| logo.starts_with(p));
        if !ok_type {
            return Err("El logo debe ser una imagen PNG, JPG, WebP o SVG.".into());
        }
        if logo.len() > 700_000 {
            return Err("El logo es demasiado grande (máximo ~500 KB).".into());
        }
    }
    check_signature(settings.tech_signature.as_deref())?;
    if !settings.tax_rate.is_finite() || !(0.0..=100.0).contains(&settings.tax_rate) {
        return Err("El impuesto debe estar entre 0 y 100 %.".into());
    }
    if settings.catalog.iter().any(|c| !c.price.is_finite() || c.price < 0.0) {
        return Err("Los precios del catálogo no pueden ser negativos.".into());
    }
    if !["risky", "always", "never"].contains(&settings.restore_points.as_str()) {
        return Err("Opción de puntos de restauración no válida.".into());
    }
    crate::tweaks::set_restore_point_policy(&settings.restore_points);
    crate::window_state::set_close_minimizes(settings.close_minimizes);
    crate::tray::set_visible(&app, settings.tray_icon);
    crate::paths::write_json(&settings_path(&app), &settings)
}

// ---------- Presupuesto / recibo ----------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Template {
    /// Resumen claro para el cliente.
    #[default]
    Client,
    /// Todo el detalle del análisis.
    Technical,
    /// La del técnico: las secciones que elija, en su orden (Ajustes → Informes).
    Custom,
}

/// Secciones del informe que se pueden quitar o cambiar de sitio, en su orden
/// de fábrica. La cabecera va siempre arriba y las firmas y condiciones, abajo.
pub const REPORT_SECTIONS: [&str; 12] = ["summary", "problem", "work", "findings", "disks", "backups", "comparison", "recommendations", "billing", "machine", "speed", "notes"];

/// La plantilla propia del técnico.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportLayout {
    pub name: String,
    /// Con todo el detalle técnico (como la plantilla técnica) o en lenguaje claro.
    pub technical: bool,
    /// Las secciones que lleva, en orden.
    pub sections: Vec<String>,
}

impl Default for ReportLayout {
    fn default() -> Self {
        ReportLayout { name: "Mi plantilla".into(), technical: false, sections: REPORT_SECTIONS.iter().map(|s| s.to_string()).collect() }
    }
}

impl ReportLayout {
    /// Las secciones que de verdad se pintan: conocidas y sin repetir.
    pub fn valid_sections(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for s in &self.sections {
            if let Some(known) = REPORT_SECTIONS.iter().find(|k| *k == s) {
                if !out.contains(known) {
                    out.push(known);
                }
            }
        }
        out
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DocKind {
    #[default]
    None,
    Quote,
    Receipt,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Line {
    pub description: String,
    pub part: bool,
    pub qty: f64,
    pub price: f64,
    /// Garantía de la pieza en días (0: sin garantía propia).
    pub warranty_days: u32,
}

impl Line {
    pub fn amount(&self) -> f64 {
        self.qty * self.price
    }

    fn filled(&self) -> bool {
        !self.description.trim().is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Billing {
    pub kind: DocKind,
    pub lines: Vec<Line>,
    /// Descuento en importe (no en porcentaje), antes de impuestos.
    pub discount: f64,
    /// Forma de pago (recibo): efectivo, transferencia…
    pub payment: String,
}

/// Importes de un presupuesto o recibo.
pub struct Totals {
    pub subtotal: f64,
    pub discount: f64,
    pub tax: f64,
    pub total: f64,
}

impl Billing {
    /// Solo cuenta si hay un documento y alguna línea con descripción.
    pub fn active(&self) -> bool {
        self.kind != DocKind::None && self.lines.iter().any(Line::filled)
    }

    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter().filter(|l| l.filled())
    }

    pub fn totals(&self, tax_rate: f64) -> Totals {
        let round = |v: f64| (v * 100.0).round() / 100.0;
        let subtotal = round(self.lines().map(Line::amount).sum());
        let discount = round(self.discount.clamp(0.0, subtotal));
        let tax = round((subtotal - discount) * tax_rate / 100.0);
        Totals { subtotal, discount, tax, total: round(subtotal - discount + tax) }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.lines.len() > 100 {
            return Err("Demasiadas líneas en el presupuesto.".into());
        }
        let bad = |v: f64| !v.is_finite() || !(0.0..=1e10).contains(&v);
        if self.lines.iter().any(|l| bad(l.qty) || bad(l.price)) || bad(self.discount) {
            return Err("Revisa cantidades y precios: no pueden ser negativos.".into());
        }
        Ok(())
    }
}

/// Número de documento correlativo por año: 2026-0001, 2026-0002…
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Numbering {
    year: i32,
    seq: u32,
}

pub fn next_number(app: &tauri::AppHandle) -> String {
    use chrono::Datelike;
    let path = crate::paths::shared_data_dir(app).join("numbering.json");
    let mut n: Numbering = crate::paths::read_json(&path);
    let year = chrono::Local::now().year();
    if n.year != year {
        n = Numbering { year, seq: 0 };
    }
    n.seq += 1;
    let _ = crate::paths::write_json(&path, &n);
    format!("{year}-{:04}", n.seq)
}

// ---------- Clientes ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Machine {
    pub host: String,
    pub os: String,
    pub first_seen: u64,
    pub last_seen: u64,
    /// Resumen del hardware en la última visita (CPU, RAM, GPU, placa).
    pub hardware: String,
    /// Ficha completa del equipo (inventario de la oficina).
    pub inventory: Option<MachineInventory>,
}

/// Lo que hay que saber de un equipo para el inventario y para decidir si renovarlo.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MachineInventory {
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub cpu: String,
    pub cores: u32,
    pub ram_gb: f64,
    /// "SSD NVMe 477 GB + HDD 932 GB"
    pub disks: String,
    pub gpu: String,
    pub os: String,
    pub bios_year: Option<i32>,
    pub installed: String,
    pub tpm: Option<bool>,
    pub secure_boot: Option<bool>,
    pub security: Option<u32>,
    pub battery: Option<f64>,
    pub ip: String,
    pub mac: String,
    /// ok | upgrade | replace
    pub verdict: String,
    pub reasons: Vec<String>,
    pub updated: u64,
}

impl MachineInventory {
    pub fn from(d: &diagnostics::Diagnostics, lan: Option<&crate::network::lan::LanInfo>) -> Self {
        use chrono::Datelike;
        let hw = d.hardware.data.as_ref();
        let sys = d.system.data.as_ref();
        let disks = d.disks.data.as_deref().unwrap_or(&[]);
        let gb = |b: u64| b as f64 / 1024f64.powi(3);
        let disk_text = disks
            .iter()
            .map(|k| {
                let kind = if k.media_type.eq_ignore_ascii_case("SSD") {
                    if k.bus_type.eq_ignore_ascii_case("NVMe") { "SSD NVMe" } else { "SSD" }
                } else if k.media_type.eq_ignore_ascii_case("HDD") {
                    "HDD"
                } else {
                    "Disco"
                };
                let size = gb(k.size);
                if size >= 1000.0 { format!("{kind} {:.1} TB", size / 1024.0) } else { format!("{kind} {size:.0} GB") }
            })
            .collect::<Vec<_>>()
            .join(" + ");
        let bios_year = hw.and_then(|h| h.bios_date.as_deref()).and_then(diagnostics::parse_time).map(|t| t.year());
        let ram_gb = gb(d.ram_total);
        let battery = d.battery.data.as_ref().and_then(|b| b.as_ref().map(|b| b.health()));
        let mut replace = Vec::new();
        let mut upgrade = Vec::new();
        if let Some(y) = bios_year {
            if chrono::Local::now().year() - y >= 7 {
                replace.push(format!("Equipo de hacia {y}: más de 7 años"));
            }
        }
        if ram_gb < 3.5 {
            replace.push(format!("Solo {ram_gb:.0} GB de memoria"));
        } else if ram_gb < 7.5 {
            upgrade.push(format!("Ampliar la memoria ({ram_gb:.0} GB → 8-16 GB)"));
        }
        let win10 = d.os.contains("Windows 10");
        if win10 && sys.and_then(|s| s.tpm_ready) == Some(false) {
            replace.push("Windows 10 sin soporte y no puede pasar a Windows 11 (sin TPM 2.0)".into());
        } else if win10 {
            upgrade.push("Actualizar a Windows 11 (Windows 10 sin soporte desde octubre de 2025)".into());
        }
        if !disks.is_empty() && disks.iter().all(|k| k.media_type.eq_ignore_ascii_case("HDD")) {
            upgrade.push("Cambiar el disco duro por un SSD (el cambio que más se nota)".into());
        }
        for k in disks.iter().filter(|k| !k.health.eq_ignore_ascii_case("Healthy")) {
            upgrade.push(format!("Reemplazar el disco {} ({})", k.name, k.health));
        }
        for k in d.smart.data.iter().flatten() {
            if k.predict_failure || k.pending.unwrap_or(0) > 0 || k.uncorrectable.unwrap_or(0) > 0 {
                let text = format!("Reemplazar el disco {} (sectores dañados)", k.model.trim());
                if !upgrade.contains(&text) {
                    upgrade.push(text);
                }
            }
        }
        if let Some(b) = battery.filter(|b| *b < 60.0) {
            upgrade.push(format!("Cambiar la batería ({b:.0}% de su capacidad)"));
        }
        let verdict = if !replace.is_empty() {
            "replace"
        } else if !upgrade.is_empty() {
            "upgrade"
        } else {
            "ok"
        };
        MachineInventory {
            manufacturer: hw.map(|h| h.manufacturer.trim().to_string()).unwrap_or_default(),
            model: hw.map(|h| h.model.trim().to_string()).unwrap_or_default(),
            serial: hw.map(|h| h.serial.trim().to_string()).unwrap_or_default(),
            cpu: d.cpu.clone(),
            cores: hw.map_or(0, |h| h.cores),
            ram_gb: (ram_gb * 10.0).round() / 10.0,
            disks: disk_text,
            gpu: hw.map(|h| h.gpus.iter().map(|g| g.name.clone()).collect::<Vec<_>>().join(" + ")).unwrap_or_default(),
            os: d.os.clone(),
            bios_year,
            installed: sys.map(|s| s.install_date.chars().take(10).collect()).unwrap_or_default(),
            tpm: sys.and_then(|s| s.tpm_ready),
            secure_boot: sys.and_then(|s| s.secure_boot),
            security: d.security.data.as_ref().map(|a| a.score),
            battery,
            ip: lan.map(|l| l.ip.clone()).unwrap_or_default(),
            mac: lan.map(|l| l.mac.clone()).unwrap_or_default(),
            verdict: verdict.into(),
            reasons: replace.into_iter().chain(upgrade).collect(),
            updated: now(),
        }
    }
}

/// Mapa de la red de la oficina del cliente (dispositivos encontrados).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkMap {
    pub name: String,
    pub gateway: String,
    pub saved: u64,
    pub devices: Vec<MapDevice>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MapDevice {
    pub ip: String,
    pub mac: String,
    pub name: String,
    pub vendor: String,
    pub alias: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionRecord {
    pub id: String,
    pub host: String,
    pub started: u64,
    pub ended: u64,
    pub report: Option<String>,
    /// Problemas críticos y advertencias antes → después.
    pub bad_before: usize,
    pub bad_after: usize,
    pub warn_before: usize,
    pub warn_after: usize,
    pub work_items: usize,
    pub notes: String,
    /// Si el hardware cambió respecto a la visita anterior: "antes → ahora".
    pub hardware_change: Option<String>,
    /// Número del informe (y del presupuesto o recibo).
    pub number: String,
    pub doc_kind: DocKind,
    pub total: f64,
    pub currency: String,
    pub warranties: Vec<Warranty>,
    /// Fecha recomendada para el próximo mantenimiento.
    pub next_maintenance: Option<u64>,
    /// El cliente firmó en pantalla.
    pub signed: bool,
    /// Cifras del equipo al terminar, para comparar visitas.
    pub metrics: Option<VisitMetrics>,
    pub visit_type: String,
    /// Contacto de la agenda que pidió el trabajo.
    pub contact_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Warranty {
    pub item: String,
    pub until: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VisitMetrics {
    pub bad: usize,
    pub warn: usize,
    pub security: Option<u32>,
    /// Unidad del sistema: libre y total en bytes.
    pub sys_free: Option<u64>,
    pub sys_total: Option<u64>,
    pub startup: Option<usize>,
    pub updates: Option<usize>,
    pub boot_ms: Option<u64>,
    pub ram_total: u64,
    pub battery_health: Option<f64>,
}

impl VisitMetrics {
    pub fn from(d: &diagnostics::Diagnostics) -> Self {
        let count = |sev| d.findings.iter().filter(|f| f.severity == sev).count();
        let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
        let vol = d.volumes.iter().find(|v| v.mount.to_uppercase().starts_with(&sys));
        VisitMetrics {
            bad: count(diagnostics::Severity::Bad),
            warn: count(diagnostics::Severity::Warn),
            security: d.security.data.as_ref().map(|a| a.score),
            sys_free: vol.map(|v| v.free),
            sys_total: vol.map(|v| v.total),
            startup: d.startup_enabled.data.as_ref().map(Vec::len),
            updates: d.software_updates.data.as_ref().map(Vec::len),
            boot_ms: d.stability.data.as_ref().and_then(|s| s.boot_times.as_ref()?.first().map(|b| b.ms)),
            ram_total: d.ram_total,
            battery_health: d.battery.data.as_ref().and_then(|b| b.as_ref().map(|b| b.health())),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub contact: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub notes: String,
    pub created: u64,
    pub machines: Vec<Machine>,
    pub sessions: Vec<SessionRecord>,
    pub network: Option<NetworkMap>,
    /// Cómo quiere este cliente sus informes.
    pub report: ClientReport,
}

/// Plantilla de informe de un cliente: formato, presentación y el correo con el
/// que se le envía. Los textos admiten {cliente}, {contacto}, {numero}, {fecha},
/// {equipo}, {empresa} y {tecnico}.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientReport {
    /// Formato por defecto (`None`: el de siempre, para el cliente).
    pub template: Option<Template>,
    /// Texto que va al principio del informe.
    pub intro: String,
    /// Destinatarios además del correo del cliente (separados por comas).
    pub to: String,
    pub subject: String,
    pub body: String,
}

impl ClientReport {
    fn cleaned(mut self) -> Self {
        let cut = |s: &str, n: usize| s.trim().chars().take(n).collect::<String>();
        self.intro = cut(&self.intro, 1500);
        self.to = cut(&self.to, 400);
        self.subject = cut(&self.subject, 200);
        self.body = cut(&self.body, 4000);
        self
    }
}

fn clients_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("clients.json")
}

pub fn find_client(app: &tauri::AppHandle, id: &str) -> Option<Client> {
    load_clients(app).into_iter().find(|c| c.id == id)
}

fn load_clients(app: &tauri::AppHandle) -> Vec<Client> {
    crate::paths::read_json(&clients_path(app))
}

fn save_clients(app: &tauri::AppHandle, clients: &[Client]) -> Result<(), String> {
    crate::paths::write_json(&clients_path(app), &clients)
}

#[tauri::command]
pub fn list_clients(app: tauri::AppHandle) -> Vec<Client> {
    let mut c = load_clients(&app);
    c.sort_by_key(|c| c.name.to_lowercase());
    c
}

/// Crea (id vacío) o actualiza los datos de contacto de un cliente. Equipos y
/// sesiones no se tocan desde aquí: los gestiona la sesión de servicio.
#[tauri::command]
pub fn save_client(app: tauri::AppHandle, client: Client) -> Result<Client, String> {
    if client.name.trim().is_empty() {
        return Err("El cliente necesita un nombre.".into());
    }
    let mut all = load_clients(&app);
    let saved = match all.iter_mut().find(|c| !client.id.is_empty() && c.id == client.id) {
        Some(existing) => {
            existing.name = client.name.trim().into();
            existing.contact = client.contact;
            existing.phone = client.phone;
            existing.email = client.email;
            existing.address = client.address;
            existing.notes = client.notes;
            existing.report = client.report.cleaned();
            existing.clone()
        }
        None => {
            let c = Client { id: new_id(), name: client.name.trim().into(), created: now(), machines: vec![], sessions: vec![], network: None, report: client.report.clone().cleaned(), ..client };
            all.push(c.clone());
            c
        }
    };
    save_clients(&app, &all)?;
    Ok(saved)
}

#[tauri::command]
pub fn delete_client(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut all = load_clients(&app);
    all.retain(|c| c.id != id);
    save_clients(&app, &all)
}

/// Pospone (o quita, con `None`) el próximo mantenimiento de la última visita.
#[tauri::command]
pub fn set_next_maintenance(app: tauri::AppHandle, client_id: String, date: Option<u64>) -> Result<(), String> {
    let mut all = load_clients(&app);
    let c = all.iter_mut().find(|c| c.id == client_id).ok_or("Cliente no encontrado.")?;
    let last = c.sessions.first_mut().ok_or("El cliente no tiene visitas.")?;
    last.next_maintenance = date;
    save_clients(&app, &all)
}

/// Garantías que salen de un trabajo: la mano de obra y cada pieza con garantía propia.
pub fn warranties(billing: &Billing, labor_days: u32, from: u64) -> Vec<Warranty> {
    let mut w = Vec::new();
    if labor_days > 0 && (billing.kind == DocKind::Receipt || !billing.active()) {
        w.push(Warranty { item: "Mano de obra".into(), until: from + labor_days as u64 * DAY });
    }
    if billing.kind == DocKind::Receipt {
        for l in billing.lines().filter(|l| l.part && l.warranty_days > 0) {
            w.push(Warranty { item: l.description.trim().into(), until: from + l.warranty_days as u64 * DAY });
        }
    }
    w
}

/// Próxima fecha de mantenimiento: `months` meses (de 30 días) después de `from`.
pub fn maintenance_date(months: u32, from: u64) -> Option<u64> {
    (months > 0).then(|| from + months as u64 * 30 * DAY)
}

/// Archiva una visita en la ficha del cliente y registra (o actualiza) el equipo.
pub fn archive_visit(app: &tauri::AppHandle, client_id: &str, after: &diagnostics::Diagnostics, mut record: SessionRecord) -> Result<(), String> {
    let mut all = load_clients(app);
    let Some(c) = all.iter_mut().find(|c| c.id == client_id) else { return Ok(()) };
    record.hardware_change = upsert_machine(c, after, record.started);
    c.sessions.insert(0, record);
    save_clients(app, &all)
}

/// Registra o actualiza el equipo en la ficha del cliente. Devuelve el cambio de hardware, si lo hubo.
fn upsert_machine(c: &mut Client, d: &diagnostics::Diagnostics, since: u64) -> Option<String> {
    let hardware = d.hardware.data.as_ref().map(|h| h.summary()).unwrap_or_default();
    let lan = crate::network::lan::current().ok().flatten();
    let inventory = Some(MachineInventory::from(d, lan.as_ref()));
    match c.machines.iter_mut().find(|m| m.host.eq_ignore_ascii_case(&d.host)) {
        Some(m) => {
            let change = (!m.hardware.is_empty() && !hardware.is_empty() && m.hardware != hardware).then(|| format!("{} → {}", m.hardware, hardware));
            m.last_seen = now();
            m.os = d.os.clone();
            if !hardware.is_empty() {
                m.hardware = hardware;
            }
            m.inventory = inventory;
            change
        }
        None => {
            c.machines.push(Machine { host: d.host.clone(), os: d.os.clone(), first_seen: since.min(now()), last_seen: now(), hardware, inventory });
            None
        }
    }
}

/// Un dato del equipo que cambió entre dos momentos.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub label: String,
    pub before: String,
    pub after: String,
    /// ¿Ha ido a mejor? `None` si no es ni mejor ni peor (p. ej. otra versión de Windows).
    pub better: Option<bool>,
}

/// Qué cambió en un equipo del cliente desde la visita anterior.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MachineChanges {
    pub host: String,
    /// Fecha de la visita con la que se compara.
    pub since: u64,
    /// Fecha del estado actual (ahora si `live`, o la última visita).
    pub until: u64,
    /// `true`: se compara con cómo está este equipo ahora mismo.
    pub live: bool,
    pub changes: Vec<Change>,
}

/// Compara las cifras de dos momentos. Solo lo que cambia de verdad: medio GB
/// de disco o un par de segundos de arranque son ruido.
pub fn metric_changes(a: &VisitMetrics, b: &VisitMetrics) -> Vec<Change> {
    let mut out = Vec::new();
    let mut push = |label: &str, before: String, after: String, better: Option<bool>| out.push(Change { label: label.into(), before, after, better });
    let gb = |x: u64| format!("{:.0} GB", x as f64 / 1024f64.powi(3));
    if (a.bad, a.warn) != (b.bad, b.warn) {
        let worse = b.bad > a.bad || (b.bad == a.bad && b.warn > a.warn);
        push("Problemas", format!("{} graves, {} avisos", a.bad, a.warn), format!("{} graves, {} avisos", b.bad, b.warn), Some(!worse));
    }
    if let (Some(x), Some(y)) = (a.security, b.security) {
        if x.abs_diff(y) >= 5 {
            push("Nota de seguridad", x.to_string(), y.to_string(), Some(y > x));
        }
    }
    if let (Some(x), Some(y)) = (a.sys_free, b.sys_free) {
        if x.abs_diff(y) >= 5 * 1024u64.pow(3) {
            push("Espacio libre en el disco del sistema", gb(x), gb(y), Some(y > x));
        }
    }
    if let (Some(x), Some(y)) = (a.startup, b.startup) {
        if x.abs_diff(y) >= 2 {
            push("Programas que arrancan con Windows", x.to_string(), y.to_string(), Some(y < x));
        }
    }
    if let (Some(x), Some(y)) = (a.updates, b.updates) {
        if x.abs_diff(y) >= 3 {
            push("Programas con versión nueva", x.to_string(), y.to_string(), Some(y < x));
        }
    }
    if let (Some(x), Some(y)) = (a.boot_ms, b.boot_ms) {
        // Diferencias de menos de 5 s o del 20 % son normales entre arranques.
        if x.abs_diff(y) >= 5000 && x.abs_diff(y) * 5 >= x.max(1) {
            push("Arranque", format!("{:.0} s", x as f64 / 1000.0), format!("{:.0} s", y as f64 / 1000.0), Some(y < x));
        }
    }
    if a.ram_total > 0 && b.ram_total > 0 && a.ram_total.abs_diff(b.ram_total) >= 512 * 1024 * 1024 {
        push("Memoria RAM", gb(a.ram_total), gb(b.ram_total), Some(b.ram_total > a.ram_total));
    }
    if let (Some(x), Some(y)) = (a.battery_health, b.battery_health) {
        if (x - y).abs() >= 5.0 {
            push("Salud de la batería", format!("{x:.0} %"), format!("{y:.0} %"), Some(y > x));
        }
    }
    out
}

/// Visitas de un equipo con cifras guardadas, la más reciente primero.
fn visits_of<'a>(c: &'a Client, host: &str) -> Vec<&'a SessionRecord> {
    let mut v: Vec<&SessionRecord> = c.sessions.iter().filter(|s| s.host.eq_ignore_ascii_case(host) && s.metrics.is_some()).collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.ended));
    v
}

/// Qué cambió en cada equipo del cliente. En este equipo se compara la última
/// visita con cómo está ahora (según el último diagnóstico); en los demás, las
/// dos últimas visitas entre sí.
pub fn client_changes(c: &Client, live: Option<&diagnostics::Diagnostics>) -> Vec<MachineChanges> {
    let mut out = Vec::new();
    for m in &c.machines {
        let visits = visits_of(c, &m.host);
        let Some(last) = visits.first() else { continue };
        let last_metrics = last.metrics.clone().unwrap_or_default();
        let here = live.filter(|d| d.host.eq_ignore_ascii_case(&m.host) && d.timestamp > last.ended);
        let entry = if let Some(d) = here {
            let mut changes = metric_changes(&last_metrics, &VisitMetrics::from(d));
            if !m.os.is_empty() && !d.os.is_empty() && m.os != d.os {
                changes.insert(0, Change { label: "Windows".into(), before: m.os.clone(), after: d.os.clone(), better: None });
            }
            let hw = d.hardware.data.as_ref().map(|h| h.summary()).unwrap_or_default();
            if !m.hardware.is_empty() && !hw.is_empty() && m.hardware != hw {
                changes.insert(0, Change { label: "Hardware".into(), before: m.hardware.clone(), after: hw, better: None });
            }
            MachineChanges { host: m.host.clone(), since: last.ended, until: d.timestamp, live: true, changes }
        } else if let Some(prev) = visits.get(1) {
            let mut changes = metric_changes(&prev.metrics.clone().unwrap_or_default(), &last_metrics);
            if let Some(hc) = &last.hardware_change {
                let (before, after) = hc.split_once(" → ").unwrap_or(("", hc));
                changes.insert(0, Change { label: "Hardware".into(), before: before.into(), after: after.into(), better: None });
            }
            MachineChanges { host: m.host.clone(), since: prev.ended, until: last.ended, live: false, changes }
        } else {
            continue;
        };
        out.push(entry);
    }
    out
}

/// Cómo queda un equipo frente a los demás del mismo cliente.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MachineRank {
    pub host: String,
    /// Valor de este equipo y mediana del resto, ya con su unidad puesta.
    pub value: String,
    pub typical: String,
    /// Cuántas veces peor que la mediana (1 = igual). `None` si no aplica.
    pub factor: Option<f64>,
    /// Este equipo está notablemente peor que el resto.
    pub worse: bool,
}

/// Una medida comparable entre los equipos de un cliente.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// Qué se compara: «Arranque», «RAM»…
    pub label: String,
    /// Frase lista para leer («PC-CONTA arranca 3 veces más lento que el resto»).
    pub summary: String,
    pub machines: Vec<MachineRank>,
}

/// Qué se compara: etiqueta, de dónde sale el valor, cómo se escribe, si más
/// es peor, y a partir de cuántas veces la mediana se avisa.
type Metric = (&'static str, fn(&VisitMetrics) -> Option<f64>, fn(f64) -> String, bool, f64);

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

/// Últimas cifras conocidas de cada equipo del cliente (de su visita más reciente).
fn latest_metrics(c: &Client) -> Vec<(String, VisitMetrics)> {
    c.machines
        .iter()
        .filter_map(|m| {
            let s = c.sessions.iter().filter(|s| s.host.eq_ignore_ascii_case(&m.host)).max_by_key(|s| s.ended)?;
            Some((m.host.clone(), s.metrics.clone()?))
        })
        .collect()
}

/// Compara los equipos de un cliente entre sí. Solo cifras técnicas del propio
/// equipo (arranque, memoria, disco, seguridad): nada del usuario ni de lo que
/// hay dentro. Sirve para decir con datos qué equipo toca renovar.
pub fn compare_machines(c: &Client) -> Vec<Comparison> {
    let data = latest_metrics(c);
    // Con menos de tres equipos la mediana no dice nada útil.
    if data.len() < 3 {
        return vec![];
    }
    let mut out = Vec::new();
    let metrics: [Metric; 4] = [
        ("Arranque", |m| m.boot_ms.map(|b| b as f64 / 1000.0), |v| format!("{v:.0} s"), true, 2.0),
        ("Memoria RAM", |m| (m.ram_total > 0).then_some(m.ram_total as f64), |v| format!("{:.0} GB", v / 1024f64.powi(3)), false, 1.5),
        ("Espacio libre", |m| m.sys_free.map(|f| f as f64), |v| format!("{:.0} GB", v / 1024f64.powi(3)), false, 2.0),
        ("Nota de seguridad", |m| m.security.map(|s| s as f64), |v| format!("{v:.0}"), false, 1.3),
    ];

    for (label, pick, fmt, high_is_worse, threshold) in metrics {
        let values: Vec<(String, f64)> = data.iter().filter_map(|(h, m)| pick(m).map(|v| (h.clone(), v))).collect();
        if values.len() < 3 {
            continue;
        }
        let Some(mid) = median(values.iter().map(|(_, v)| *v).collect()) else { continue };
        if mid <= 0.0 {
            continue;
        }
        let mut machines: Vec<MachineRank> = values
            .iter()
            .map(|(host, v)| {
                // «Veces peor»: arriba de la mediana si más es peor, abajo si al revés.
                let factor = if high_is_worse { v / mid } else { mid / v.max(0.001) };
                MachineRank {
                    host: host.clone(),
                    value: fmt(*v),
                    typical: fmt(mid),
                    factor: Some((factor * 10.0).round() / 10.0),
                    worse: factor >= threshold,
                }
            })
            .collect();
        machines.sort_by(|a, b| b.factor.partial_cmp(&a.factor).unwrap_or(std::cmp::Ordering::Equal));

        let worst = machines.first().filter(|m| m.worse);
        let summary = match worst {
            Some(m) if label == "Arranque" => format!("{} arranca {:.1} veces más lento que el resto de la oficina ({} frente a {}).", m.host, m.factor.unwrap_or(1.0), m.value, m.typical),
            Some(m) if label == "Memoria RAM" => format!("{} tiene menos memoria que el resto ({} frente a {}).", m.host, m.value, m.typical),
            Some(m) if label == "Espacio libre" => format!("{} va mucho más justo de disco que el resto ({} frente a {}).", m.host, m.value, m.typical),
            Some(m) => format!("{} está por debajo del resto en {} ({} frente a {}).", m.host, label.to_lowercase(), m.value, m.typical),
            None => format!("Todos los equipos andan parecidos en {} (unos {}).", label.to_lowercase(), fmt(mid)),
        };
        out.push(Comparison { label: label.to_string(), summary, machines });
    }
    out
}

/// Comparación entre los equipos de un cliente (solo cifras técnicas).
#[tauri::command(async)]
pub fn compare_client_machines(app: tauri::AppHandle, client_id: String) -> Result<Vec<Comparison>, String> {
    let c = find_client(&app, &client_id).ok_or("Cliente no encontrado.")?;
    Ok(compare_machines(&c))
}

#[tauri::command(async)]
pub fn visit_changes(app: tauri::AppHandle, client_id: String) -> Result<Vec<MachineChanges>, String> {
    let c = find_client(&app, &client_id).ok_or("Cliente no encontrado.")?;
    let live = diagnostics::latest_snapshot(&app);
    Ok(client_changes(&c, live.as_ref()))
}

/// Añade (o actualiza) este equipo en el inventario de un cliente, sin sesión de servicio.
/// Usa el último diagnóstico si es reciente; si no, hace uno.
#[tauri::command(async)]
pub fn inventory_add_this(app: tauri::AppHandle, state: State<'_, TweakState>, client_id: String) -> Result<Client, String> {
    let recent = diagnostics::latest_snapshot(&app).filter(|d| now().saturating_sub(d.timestamp) < 12 * 3600);
    let d = match recent {
        Some(d) => d,
        None => {
            let task = crate::task::Task::new(&app, "inventory");
            task.step("Analizando este equipo…");
            diagnostics::run_and_save(&app, &state, false)
        }
    };
    let mut all = load_clients(&app);
    let c = all.iter_mut().find(|c| c.id == client_id).ok_or("Cliente no encontrado.")?;
    upsert_machine(c, &d, d.timestamp);
    let saved = c.clone();
    save_clients(&app, &all)?;
    Ok(saved)
}

/// Quita un equipo del inventario del cliente (las visitas no se tocan).
#[tauri::command]
pub fn inventory_remove(app: tauri::AppHandle, client_id: String, host: String) -> Result<(), String> {
    let mut all = load_clients(&app);
    let c = all.iter_mut().find(|c| c.id == client_id).ok_or("Cliente no encontrado.")?;
    c.machines.retain(|m| !m.host.eq_ignore_ascii_case(&host));
    save_clients(&app, &all)
}

/// Guarda en la ficha del cliente los dispositivos encontrados en su red.
#[tauri::command]
pub fn save_network_map(app: tauri::AppHandle, client_id: String, map: NetworkMap) -> Result<(), String> {
    let mut all = load_clients(&app);
    let c = all.iter_mut().find(|c| c.id == client_id).ok_or("Cliente no encontrado.")?;
    c.network = Some(NetworkMap { saved: now(), devices: map.devices.into_iter().take(1024).collect(), ..map });
    save_clients(&app, &all)
}

// ---------- Sesión de servicio ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
    /// Tarea que la marca sola (ver `ChecklistTemplate`).
    pub auto: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ActiveSession {
    pub id: String,
    pub client_id: String,
    pub client_name: String,
    pub host: String,
    pub started: u64,
    /// Snapshot del diagnóstico "antes".
    pub baseline: u64,
    pub checklist: Vec<ChecklistItem>,
    pub notes: String,
    /// Motivo de la visita (lo que cuenta el cliente).
    pub problem: String,
    /// Recomendaciones para el cliente.
    pub recommendations: String,
    pub template: Template,
    pub billing: Billing,
    /// Firma del cliente (data URL PNG) y nombre de quien firma.
    pub signature: Option<String>,
    pub signer: String,
    pub labor_warranty_days: u32,
    pub maintenance_months: u32,
    /// Tipo de visita elegido al empezar.
    pub visit_type: String,
    /// Contacto de la agenda que pidió el trabajo.
    pub contact_id: String,
}

/// Análisis «antes» de la sesión en curso (no se borra al limpiar datos antiguos).
pub fn active_baseline(app: &tauri::AppHandle) -> Option<u64> {
    load_session(app).map(|s| s.baseline)
}

fn session_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("session.json")
}

fn load_session(app: &tauri::AppHandle) -> Option<ActiveSession> {
    std::fs::read_to_string(session_path(app)).ok().and_then(|s| serde_json::from_str(&s).ok())
}

#[tauri::command]
pub fn get_session(app: tauri::AppHandle) -> Option<ActiveSession> {
    load_session(&app)
}

#[tauri::command(async)]
pub fn start_session(
    app: tauri::AppHandle,
    state: State<'_, TweakState>,
    client_id: String,
    visit_type: Option<String>,
    contact_id: Option<String>,
) -> Result<ActiveSession, String> {
    if load_session(&app).is_some() {
        return Err("Ya hay una sesión en curso en este equipo.".into());
    }
    let client = find_client(&app, &client_id).ok_or("Cliente no encontrado.")?;
    let task = crate::task::Task::new(&app, "session");
    task.step("Diagnóstico inicial (antes de trabajar)…");
    let before = diagnostics::run_and_save(&app, &state, false);
    let st = settings(&app);
    let visit = visit_type.as_deref().and_then(|v| st.visit_types.iter().find(|t| t.name == v));
    let checklist = match visit {
        Some(v) => v.items.iter().map(|i| ChecklistItem { text: i.text.clone(), done: false, auto: i.auto.clone() }).collect(),
        None => st.checklist.iter().map(|text| ChecklistItem { text: text.clone(), done: false, auto: String::new() }).collect(),
    };
    let session = ActiveSession {
        id: new_id(),
        client_id: client.id,
        client_name: client.name,
        host: before.host.clone(),
        started: now(),
        baseline: before.timestamp,
        checklist,
        visit_type: visit.map(|v| v.name.clone()).unwrap_or_default(),
        contact_id: contact_id.unwrap_or_default(),
        template: client.report.template.unwrap_or_default(),
        signer: client.contact,
        labor_warranty_days: st.labor_warranty_days,
        maintenance_months: st.maintenance_months,
        ..Default::default()
    };
    crate::paths::write_json(&session_path(&app), &session)?;
    log::info!("Sesión de servicio iniciada para {} en {}", session.client_name, session.host);
    Ok(session)
}

/// Guarda lo editable de la sesión (checklist, textos, presupuesto, firma…);
/// cliente, equipo y fechas no cambian.
#[tauri::command]
pub fn update_session(app: tauri::AppHandle, session: ActiveSession) -> Result<(), String> {
    let mut s = load_session(&app).ok_or("No hay ninguna sesión en curso.")?;
    session.billing.validate()?;
    check_signature(session.signature.as_deref())?;
    s.checklist = session.checklist;
    s.notes = session.notes;
    s.problem = session.problem;
    s.recommendations = session.recommendations;
    s.template = session.template;
    s.billing = session.billing;
    s.signature = session.signature;
    s.signer = session.signer;
    s.labor_warranty_days = session.labor_warranty_days.min(3650);
    s.maintenance_months = session.maintenance_months.min(60);
    s.contact_id = session.contact_id;
    crate::paths::write_json(&session_path(&app), &s)
}

#[tauri::command]
pub fn cancel_session(app: tauri::AppHandle) -> Result<(), String> {
    let _ = std::fs::remove_file(session_path(&app));
    Ok(())
}

/// Diagnóstico "después", informe con comparación y archivo en la ficha del cliente.
#[tauri::command(async)]
pub fn finish_session(app: tauri::AppHandle, state: State<'_, TweakState>) -> Result<String, String> {
    let s = load_session(&app).ok_or("No hay ninguna sesión en curso.")?;
    let task = crate::task::Task::new(&app, "session");
    task.step("Diagnóstico final (después del trabajo)…");
    let after = diagnostics::run_and_save(&app, &state, false);
    let before = diagnostics::load_snapshot(&app, s.baseline);
    let client = find_client(&app, &s.client_id);
    let st = settings(&app);
    let ended = now();
    let warranties = warranties(&s.billing, s.labor_warranty_days, ended);
    let next_maintenance = maintenance_date(s.maintenance_months, ended);
    task.step("Generando informe PDF…");
    let created = diagnostics::report::create_report(
        &app,
        &state,
        diagnostics::report::ReportInput {
            baseline: Some(s.baseline),
            client: client.clone().unwrap_or(Client { name: s.client_name.clone(), ..Default::default() }),
            technician: None,
            notes: s.notes.clone(),
            checklist: s.checklist.clone(),
            since: Some(s.started),
            template: s.template,
            billing: s.billing.clone(),
            problem: s.problem.clone(),
            recommendations: s.recommendations.clone(),
            signature: s.signature.clone(),
            signer: s.signer.clone(),
            warranties: warranties.clone(),
            next_maintenance,
        },
    )?;

    let count = |d: &diagnostics::Diagnostics, sev| d.findings.iter().filter(|f| f.severity == sev).count();
    let record = SessionRecord {
        id: s.id.clone(),
        host: s.host.clone(),
        started: s.started,
        ended,
        report: Some(created.path.clone()),
        bad_before: before.as_ref().map_or(0, |b| count(b, diagnostics::Severity::Bad)),
        bad_after: count(&after, diagnostics::Severity::Bad),
        warn_before: before.as_ref().map_or(0, |b| count(b, diagnostics::Severity::Warn)),
        warn_after: count(&after, diagnostics::Severity::Warn),
        work_items: state.journal_since(s.started).iter().filter(|e| e.ok).count(),
        notes: s.notes.clone(),
        hardware_change: None,
        number: created.number,
        doc_kind: if s.billing.active() { s.billing.kind } else { DocKind::None },
        total: if s.billing.active() { s.billing.totals(st.tax_rate).total } else { 0.0 },
        currency: st.currency.clone(),
        warranties,
        next_maintenance,
        signed: s.signature.is_some(),
        metrics: Some(VisitMetrics::from(&after)),
        visit_type: s.visit_type.clone(),
        contact_id: s.contact_id.clone(),
    };
    archive_visit(&app, &s.client_id, &after, record)?;
    let _ = std::fs::remove_file(session_path(&app));
    log::info!("Sesión de servicio finalizada: {}", created.path);
    Ok(created.path)
}

// ---------- Copia de la configuración ----------

/// Ajustes, portales de Tickets y preferencias de la interfaz en un .json (sin contraseñas).
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ConfigBackup {
    app: String,
    version: String,
    settings: Option<Settings>,
    portals: Vec<crate::portals::Portal>,
    prefs: serde_json::Value,
}

#[tauri::command(async)]
pub fn export_config(app: tauri::AppHandle, prefs: serde_json::Value) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let backup = ConfigBackup {
        app: "AdminOps".into(),
        version: app.package_info().version.to_string(),
        settings: Some(settings(&app)),
        portals: crate::portals::export_portals(&app),
        prefs,
    };
    let Some(file) = app
        .dialog()
        .file()
        .set_file_name(format!("Configuración AdminOps {}.json", chrono::Local::now().format("%Y-%m-%d")))
        .add_filter("Configuración de AdminOps", &["json"])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    std::fs::write(&file, json).map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(Some(file.display().to_string()))
}

/// Devuelve las preferencias de la interfaz para aplicarlas (los ajustes y portales ya quedan guardados).
#[tauri::command(async)]
pub fn import_config(app: tauri::AppHandle) -> Result<Option<serde_json::Value>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(file) = app.dialog().file().add_filter("Configuración de AdminOps", &["json"]).blocking_pick_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(&file).map_err(|e| format!("No se pudo leer: {e}"))?;
    if text.len() > 5_000_000 {
        return Err("El archivo es demasiado grande.".into());
    }
    let backup: ConfigBackup = serde_json::from_str(&text).map_err(|_| "No es una configuración de AdminOps.".to_string())?;
    if backup.app != "AdminOps" {
        return Err("No es una configuración de AdminOps.".into());
    }
    if let Some(mut s) = backup.settings {
        // El asistente del primer arranque ya se hizo en este equipo.
        s.onboarded = true;
        save_settings(app.clone(), s)?;
    }
    crate::portals::import_portals(&app, backup.portals)?;
    Ok(Some(backup.prefs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_machines_of_the_same_client() {
        let pc = |host: &str, boot: u64, ram_gb: u64| {
            (
                Machine { host: host.into(), ..Default::default() },
                SessionRecord {
                    host: host.into(),
                    ended: 100,
                    metrics: Some(VisitMetrics { boot_ms: Some(boot), ram_total: ram_gb * 1024 * 1024 * 1024, ..Default::default() }),
                    ..Default::default()
                },
            )
        };
        let rows = [pc("PC-CONTA", 120_000, 4), pc("PC-RECEP", 40_000, 8), pc("PC-DIR", 35_000, 8)];
        let c = Client {
            machines: rows.iter().map(|(m, _)| m.clone()).collect(),
            sessions: rows.iter().map(|(_, s)| s.clone()).collect(),
            ..Default::default()
        };
        let cmp = compare_machines(&c);
        let boot = cmp.iter().find(|x| x.label == "Arranque").expect("falta el arranque");
        assert_eq!(boot.machines[0].host, "PC-CONTA");
        assert!(boot.machines[0].worse);
        assert!(boot.summary.contains("PC-CONTA") && boot.summary.contains("más lento"), "{}", boot.summary);
        // El que va como los demás no se marca.
        assert!(!boot.machines.last().unwrap().worse);
    }

    #[test]
    fn no_compara_con_menos_de_tres_equipos() {
        let c = Client {
            machines: vec![Machine { host: "A".into(), ..Default::default() }],
            sessions: vec![SessionRecord { host: "A".into(), metrics: Some(VisitMetrics::default()), ..Default::default() }],
            ..Default::default()
        };
        assert!(compare_machines(&c).is_empty());
    }

    #[test]
    fn client_report_is_optional_and_trimmed() {
        // Fichas antiguas, sin plantilla de informe.
        let c: Client = serde_json::from_str(r#"{"id":"c1","name":"Ana"}"#).unwrap();
        assert_eq!(c.report, ClientReport::default());
        let r = ClientReport { intro: format!("  {}  ", "x".repeat(2000)), subject: " Hola ".into(), template: Some(Template::Technical), ..Default::default() }.cleaned();
        assert_eq!((r.intro.len(), r.subject.as_str(), r.template), (1500, "Hola", Some(Template::Technical)));
    }

    #[test]
    fn metric_changes_skip_noise_and_rate_direction() {
        let g = 1024u64.pow(3);
        let a = VisitMetrics { bad: 2, warn: 3, security: Some(60), sys_free: Some(40 * g), startup: Some(12), updates: Some(8), boot_ms: Some(40_000), ram_total: 8 * g, battery_health: Some(90.0), ..Default::default() };
        let same = VisitMetrics { sys_free: Some(40 * g + g / 2), boot_ms: Some(41_000), ..a.clone() };
        assert!(metric_changes(&a, &same).is_empty());
        let b = VisitMetrics { bad: 0, warn: 1, security: Some(80), sys_free: Some(20 * g), startup: Some(6), ram_total: 16 * g, ..a.clone() };
        let c = metric_changes(&a, &b);
        let find = |l: &str| c.iter().find(|x| x.label == l).cloned();
        assert_eq!(find("Problemas").and_then(|x| x.better), Some(true));
        assert_eq!(find("Nota de seguridad").and_then(|x| x.better), Some(true));
        assert_eq!(find("Espacio libre en el disco del sistema").and_then(|x| x.better), Some(false));
        assert_eq!(find("Programas que arrancan con Windows").and_then(|x| x.better), Some(true));
        assert_eq!(find("Memoria RAM").map(|x| x.after), Some("16 GB".to_string()));
        assert!(find("Arranque").is_none());
    }

    #[test]
    fn client_changes_between_last_two_visits() {
        let m = |bad| Some(VisitMetrics { bad, ..Default::default() });
        let c = Client {
            machines: vec![Machine { host: "PC1".into(), ..Default::default() }, Machine { host: "PC2".into(), ..Default::default() }],
            sessions: vec![
                SessionRecord { host: "PC1".into(), ended: 100, metrics: m(1), ..Default::default() },
                SessionRecord { host: "pc1".into(), ended: 200, metrics: m(3), hardware_change: Some("8 GB → 16 GB".into()), ..Default::default() },
                SessionRecord { host: "PC2".into(), ended: 150, metrics: m(0), ..Default::default() },
            ],
            ..Default::default()
        };
        let r = client_changes(&c, None);
        // PC2 solo tiene una visita: no hay con qué comparar.
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].since, r[0].until, r[0].live), (100, 200, false));
        assert_eq!(r[0].changes[0].label, "Hardware");
        assert_eq!(r[0].changes[1].better, Some(false));
    }

    #[test]
    fn default_settings_include_a_checklist() {
        let s = Settings::default();
        assert!(s.checklist.len() >= 5);
        // Un archivo antiguo sin checklist mantiene la de por defecto.
        let old: Settings = serde_json::from_str(r#"{"technician":"Ana"}"#).unwrap();
        assert_eq!(old.technician, "Ana");
        assert!(!old.checklist.is_empty());
        assert_eq!(old.tax_rate, 18.0);
    }

    #[test]
    fn billing_totals_and_warranties() {
        let line = |d: &str, part, qty, price, w| Line { description: d.into(), part, qty, price, warranty_days: w };
        let b = Billing {
            kind: DocKind::Receipt,
            lines: vec![line("Mano de obra", false, 1.0, 1500.0, 0), line("SSD 500 GB", true, 1.0, 3200.0, 365), line("  ", false, 3.0, 99.0, 0)],
            discount: 200.0,
            payment: String::new(),
        };
        let t = b.totals(18.0);
        assert_eq!(t.subtotal, 4700.0);
        assert_eq!(t.discount, 200.0);
        assert_eq!(t.tax, 810.0);
        assert_eq!(t.total, 5310.0);
        assert!(b.validate().is_ok());
        let w = warranties(&b, 30, 0);
        assert_eq!(w.len(), 2);
        assert_eq!(w[1].until, 365 * DAY);
        // Un presupuesto no da garantías (el trabajo aún no se hizo).
        let quote = Billing { kind: DocKind::Quote, ..b.clone() };
        assert!(warranties(&quote, 30, 0).is_empty());
        let neg = Billing { discount: -1.0, ..b };
        assert!(neg.validate().is_err());
        assert_eq!(maintenance_date(0, 5), None);
    }

    /// Con un análisis real: `ADMINOPS_SNAPSHOTS=<carpeta> cargo test inventory_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn inventory_real() {
        let dir = std::path::PathBuf::from(std::env::var("ADMINOPS_SNAPSHOTS").unwrap());
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        files.sort();
        let d: diagnostics::Diagnostics = serde_json::from_str(&std::fs::read_to_string(files.last().unwrap()).unwrap()).unwrap();
        let inv = MachineInventory::from(&d, None);
        println!("{} {} · {} · {} GB · {} · {} · año {:?} · {}", inv.manufacturer, inv.model, inv.cpu, inv.ram_gb, inv.disks, inv.os, inv.bios_year, inv.verdict);
        for r in inv.reasons {
            println!("  → {r}");
        }
    }

    #[test]
    fn old_session_files_still_load() {
        let s: ActiveSession = serde_json::from_str(r#"{"id":"a","clientName":"X","checklist":[{"text":"t","done":true}]}"#).unwrap();
        assert_eq!(s.template, Template::Client);
        assert_eq!(s.billing.kind, DocKind::None);
        assert!(check_signature(Some("data:image/jpeg;base64,xx")).is_err());
        assert!(check_signature(Some("data:image/png;base64,xx")).is_ok());
    }
}
