//! Flujo de trabajo del técnico: ajustes (marca y checklist), clientes y
//! sesiones de servicio (diagnóstico "antes" → trabajo → informe "después").

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
    crate::paths::write_json(&settings_path(&app), &settings)
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
    let client = load_clients(&app).into_iter().find(|c| c.id == client_id).ok_or("Cliente no encontrado.")?;
    let task = crate::task::Task::new(&app, "session");
    task.step("Diagnóstico inicial (antes de trabajar)…");
    let before = diagnostics::run_and_save(&app, &state);
    let session = ActiveSession {
        id: new_id(),
        client_id: client.id,
        client_name: client.name,
        host: before.host.clone(),
        started: now(),
        baseline: before.timestamp,
        checklist: settings(&app).checklist.into_iter().map(|text| ChecklistItem { text, done: false }).collect(),
        notes: String::new(),
    };
    crate::paths::write_json(&session_path(&app), &session)?;
    log::info!("Sesión de servicio iniciada para {} en {}", session.client_name, session.host);
    Ok(session)
}

#[tauri::command]
pub fn update_session(app: tauri::AppHandle, checklist: Vec<ChecklistItem>, notes: String) -> Result<(), String> {
    let mut s = load_session(&app).ok_or("No hay ninguna sesión en curso.")?;
    s.checklist = checklist;
    s.notes = notes;
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
    task.step("Generando informe PDF…");
    let path = diagnostics::report::create_report(
        &app,
        &state,
        diagnostics::report::ReportInput {
            baseline: Some(s.baseline),
            client: s.client_name.clone(),
            technician: None,
            notes: s.notes.clone(),
            checklist: s.checklist.clone(),
            since: Some(s.started),
        },
    )?;

    let count = |d: &diagnostics::Diagnostics, sev| d.findings.iter().filter(|f| f.severity == sev).count();
    let hardware = after.hardware.data.as_ref().map(|h| h.summary()).unwrap_or_default();
    let mut record = SessionRecord {
        id: s.id.clone(),
        host: s.host.clone(),
        started: s.started,
        ended: now(),
        report: Some(path.clone()),
        bad_before: before.as_ref().map_or(0, |b| count(b, diagnostics::Severity::Bad)),
        bad_after: count(&after, diagnostics::Severity::Bad),
        warn_before: before.as_ref().map_or(0, |b| count(b, diagnostics::Severity::Warn)),
        warn_after: count(&after, diagnostics::Severity::Warn),
        work_items: state.journal_since(s.started).iter().filter(|e| e.ok).count(),
        notes: s.notes.clone(),
        hardware_change: None,
    };
    let mut all = load_clients(&app);
    if let Some(c) = all.iter_mut().find(|c| c.id == s.client_id) {
        match c.machines.iter_mut().find(|m| m.host.eq_ignore_ascii_case(&s.host)) {
            Some(m) => {
                if !m.hardware.is_empty() && !hardware.is_empty() && m.hardware != hardware {
                    record.hardware_change = Some(format!("{} → {}", m.hardware, hardware));
                }
                m.last_seen = now();
                m.os = after.os.clone();
                if !hardware.is_empty() {
                    m.hardware = hardware.clone();
                }
            }
            None => c.machines.push(Machine { host: s.host.clone(), os: after.os.clone(), first_seen: s.started, last_seen: now(), hardware: hardware.clone() }),
        }
        c.sessions.insert(0, record);
    }
    save_clients(&app, &all)?;
    let _ = std::fs::remove_file(session_path(&app));
    log::info!("Sesión de servicio finalizada: {path}");
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_settings_include_a_checklist() {
        let s = super::Settings::default();
        assert!(s.checklist.len() >= 5);
        // Un archivo antiguo sin checklist mantiene la de por defecto.
        let old: super::Settings = serde_json::from_str(r#"{"technician":"Ana"}"#).unwrap();
        assert_eq!(old.technician, "Ana");
        assert!(!old.checklist.is_empty());
    }
}
