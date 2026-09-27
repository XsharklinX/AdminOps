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
    /// Dominio que se propone al unir equipos (p. ej. pgr.gob.do).
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
            existing.clone()
        }
        None => {
            let c = Client { id: new_id(), name: client.name.trim().into(), created: now(), machines: vec![], sessions: vec![], ..client };
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
    let hardware = after.hardware.data.as_ref().map(|h| h.summary()).unwrap_or_default();
    let mut all = load_clients(app);
    let Some(c) = all.iter_mut().find(|c| c.id == client_id) else { return Ok(()) };
    match c.machines.iter_mut().find(|m| m.host.eq_ignore_ascii_case(&record.host)) {
        Some(m) => {
            if !m.hardware.is_empty() && !hardware.is_empty() && m.hardware != hardware {
                record.hardware_change = Some(format!("{} → {}", m.hardware, hardware));
            }
            m.last_seen = now();
            m.os = after.os.clone();
            if !hardware.is_empty() {
                m.hardware = hardware;
            }
        }
        None => c.machines.push(Machine { host: record.host.clone(), os: after.os.clone(), first_seen: record.started, last_seen: now(), hardware }),
    }
    c.sessions.insert(0, record);
    save_clients(app, &all)
}

// ---------- Sesión de servicio ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
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
pub fn start_session(app: tauri::AppHandle, state: State<'_, TweakState>, client_id: String) -> Result<ActiveSession, String> {
    if load_session(&app).is_some() {
        return Err("Ya hay una sesión en curso en este equipo.".into());
    }
    let client = find_client(&app, &client_id).ok_or("Cliente no encontrado.")?;
    let task = crate::task::Task::new(&app, "session");
    task.step("Diagnóstico inicial (antes de trabajar)…");
    let before = diagnostics::run_and_save(&app, &state);
    let st = settings(&app);
    let session = ActiveSession {
        id: new_id(),
        client_id: client.id,
        client_name: client.name,
        host: before.host.clone(),
        started: now(),
        baseline: before.timestamp,
        checklist: st.checklist.into_iter().map(|text| ChecklistItem { text, done: false }).collect(),
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
    let after = diagnostics::run_and_save(&app, &state);
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
    };
    archive_visit(&app, &s.client_id, &after, record)?;
    let _ = std::fs::remove_file(session_path(&app));
    log::info!("Sesión de servicio finalizada: {}", created.path);
    Ok(created.path)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn old_session_files_still_load() {
        let s: ActiveSession = serde_json::from_str(r#"{"id":"a","clientName":"X","checklist":[{"text":"t","done":true}]}"#).unwrap();
        assert_eq!(s.template, Template::Client);
        assert_eq!(s.billing.kind, DocKind::None);
        assert!(check_signature(Some("data:image/jpeg;base64,xx")).is_err());
        assert!(check_signature(Some("data:image/png;base64,xx")).is_ok());
    }
}
