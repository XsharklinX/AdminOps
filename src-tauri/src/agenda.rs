//! Agenda de mantenimientos: visitas planificadas a los clientes, lo que toca
//! hoy y un aviso de Windows antes de cada visita. Viaja con los datos
//! compartidos (en el USB con el portable), como los clientes.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;

static FILE_LOCK: Mutex<()> = Mutex::new(());
/// Día (AAAAMMDD) del último resumen «hoy tienes…», para darlo una vez al día.
static SUMMARY_DAY: Mutex<Option<u32>> = Mutex::new(None);

/// Cuánto antes de la visita se avisa.
const REMIND_BEFORE: u64 = 30 * 60;
const DAY: u64 = 86_400;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Visit {
    pub id: String,
    pub client_id: String,
    /// Copia del nombre, por si el cliente se borra después.
    pub client_name: String,
    /// Inicio (segundos desde 1970, UTC).
    pub start: u64,
    pub minutes: u32,
    /// Equipos que hay que revisar.
    pub machines: u32,
    pub notes: String,
    /// planned | done | cancelled
    pub status: String,
    /// Ya se avisó de esta visita.
    pub reminded: bool,
}

/// Cliente con el mantenimiento vencido o cerca, sin visita planificada.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Due {
    pub client_id: String,
    pub name: String,
    pub date: u64,
    pub machines: u32,
    pub phone: String,
    pub email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgendaView {
    pub visits: Vec<Visit>,
    pub due: Vec<Due>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("agenda.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Visit> {
    crate::paths::read_json(&path(app))
}

fn save(app: &tauri::AppHandle, v: &[Visit]) -> Result<(), String> {
    crate::paths::write_json(&path(app), &v)
}

fn validate(mut v: Visit) -> Result<Visit, String> {
    if v.client_id.trim().is_empty() {
        return Err("Elige el cliente.".into());
    }
    if v.start == 0 {
        return Err("Elige el día y la hora.".into());
    }
    v.minutes = if v.minutes == 0 { 60 } else { v.minutes.clamp(15, 12 * 60) };
    v.machines = v.machines.min(500);
    v.notes = v.notes.trim().chars().take(500).collect();
    if !["planned", "done", "cancelled"].contains(&v.status.as_str()) {
        v.status = "planned".into();
    }
    Ok(v)
}

/// Clientes que necesitan visita en las próximas dos semanas (o ya la necesitan)
/// y no la tienen planificada.
pub fn due_clients(clients: &[crate::workflow::Client], visits: &[Visit], at: u64) -> Vec<Due> {
    let mut out: Vec<Due> = clients
        .iter()
        .filter_map(|c| {
            let date = c.sessions.first()?.next_maintenance?;
            if date > at + 14 * DAY {
                return None;
            }
            let planned = visits.iter().any(|v| v.client_id == c.id && v.status == "planned" && v.start + DAY >= at);
            (!planned).then(|| Due {
                client_id: c.id.clone(),
                name: c.name.clone(),
                date,
                machines: c.machines.len() as u32,
                phone: c.phone.clone(),
                email: c.email.clone(),
            })
        })
        .collect();
    out.sort_by_key(|d| d.date);
    out
}

/// Visitas de las que toca avisar ahora (empiezan en los próximos 30 minutos).
fn to_remind(visits: &[Visit], at: u64) -> Vec<usize> {
    visits
        .iter()
        .enumerate()
        .filter(|(_, v)| v.status == "planned" && !v.reminded && v.start > at && v.start <= at + REMIND_BEFORE)
        .map(|(i, _)| i)
        .collect()
}

fn local_time(ts: u64) -> chrono::DateTime<chrono::Local> {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(ts as i64, 0).single().unwrap_or_else(chrono::Local::now)
}

fn machines_text(n: u32) -> String {
    match n {
        0 => String::new(),
        1 => " · 1 equipo".into(),
        n => format!(" · {n} equipos"),
    }
}

fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body.chars().take(240).collect::<String>()).show();
}

/// Revisa la agenda: resumen del día (una vez) y aviso antes de cada visita.
fn tick(app: &tauri::AppHandle) {
    use chrono::Datelike;
    let at = now();
    let today = local_time(at);
    let day_key = today.year() as u32 * 10_000 + today.month() * 100 + today.day();
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut visits = load(app);

    let mut summary = SUMMARY_DAY.lock().unwrap_or_else(|e| e.into_inner());
    if *summary != Some(day_key) {
        *summary = Some(day_key);
        let todays: Vec<&Visit> = visits.iter().filter(|v| v.status == "planned" && v.start >= at && local_time(v.start).date_naive() == today.date_naive()).collect();
        if !todays.is_empty() {
            let list = todays.iter().map(|v| format!("{} {}", local_time(v.start).format("%H:%M"), v.client_name)).collect::<Vec<_>>().join(" · ");
            let n = todays.len();
            notify(app, &format!("Hoy tienes {n} {}", if n == 1 { "visita" } else { "visitas" }), &list);
        }
    }
    drop(summary);

    let due = to_remind(&visits, at);
    if due.is_empty() {
        return;
    }
    for i in due {
        let v = &mut visits[i];
        v.reminded = true;
        let body = format!("{} a las {}{}{}", v.client_name, local_time(v.start).format("%H:%M"), machines_text(v.machines), if v.notes.is_empty() { String::new() } else { format!(" · {}", v.notes) });
        notify(app, "Visita en menos de 30 minutos", &body);
    }
    if let Err(e) = save(app, &visits) {
        log::warn!("Agenda: no se pudo guardar el aviso: {e}");
    }
}

/// Vigila la agenda mientras AdminOps está abierta (cada minuto).
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Deja que termine de arrancar la app antes del primer resumen.
        std::thread::sleep(Duration::from_secs(20));
        loop {
            tick(&app);
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}

// ---------- Comandos ----------

#[tauri::command]
pub fn list_agenda(app: tauri::AppHandle) -> AgendaView {
    let mut visits = load(&app);
    visits.sort_by_key(|v| v.start);
    let clients = crate::workflow::list_clients(app.clone());
    let due = due_clients(&clients, &visits, now());
    AgendaView { visits, due }
}

#[tauri::command]
pub fn save_visit(app: tauri::AppHandle, visit: Visit) -> Result<Visit, String> {
    let mut v = validate(visit)?;
    let client = crate::workflow::find_client(&app, &v.client_id).ok_or("Ese cliente ya no existe.")?;
    v.client_name = client.name.clone();
    if v.machines == 0 {
        v.machines = client.machines.len() as u32;
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    match list.iter_mut().find(|x| !v.id.is_empty() && x.id == v.id) {
        Some(x) => {
            // Si cambia la hora, se vuelve a avisar.
            if x.start != v.start {
                v.reminded = false;
            } else {
                v.reminded = x.reminded;
            }
            *x = v.clone();
        }
        None => {
            v.id = format!("v{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
            v.reminded = false;
            list.push(v.clone());
        }
    }
    save(&app, &list)?;
    Ok(v)
}

#[tauri::command]
pub fn set_visit_status(app: tauri::AppHandle, id: String, status: String) -> Result<(), String> {
    if !["planned", "done", "cancelled"].contains(&status.as_str()) {
        return Err("Estado no válido.".into());
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let v = list.iter_mut().find(|v| v.id == id).ok_or("Esa visita ya no existe.")?;
    v.status = status;
    save(&app, &list)
}

#[tauri::command]
pub fn delete_visit(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    list.retain(|v| v.id != id);
    save(&app, &list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{Client, SessionRecord};

    fn visit(client: &str, start: u64, status: &str) -> Visit {
        Visit { id: format!("v{start}"), client_id: client.into(), client_name: client.into(), start, minutes: 60, status: status.into(), ..Default::default() }
    }

    #[test]
    fn validates_visits() {
        assert!(validate(Visit { start: 10, ..Default::default() }).is_err());
        assert!(validate(Visit { client_id: "c1".into(), ..Default::default() }).is_err());
        let v = validate(Visit { client_id: "c1".into(), start: 10, minutes: 5, status: "raro".into(), notes: "x".repeat(900), ..Default::default() }).unwrap();
        assert_eq!((v.minutes, v.status.as_str(), v.notes.len()), (15, "planned", 500));
        assert_eq!(validate(Visit { client_id: "c1".into(), start: 10, ..Default::default() }).unwrap().minutes, 60);
    }

    #[test]
    fn reminds_once_shortly_before() {
        let at = 1_000_000;
        let mut v = vec![
            visit("a", at + 10 * 60, "planned"),
            visit("b", at + 2 * 3600, "planned"),
            visit("c", at + 5 * 60, "cancelled"),
            visit("d", at - 60, "planned"),
        ];
        assert_eq!(to_remind(&v, at), vec![0]);
        v[0].reminded = true;
        assert!(to_remind(&v, at).is_empty());
    }

    #[test]
    fn due_clients_skip_the_ones_already_planned() {
        let at = 100 * DAY;
        let client = |id: &str, next: Option<u64>| Client {
            id: id.into(),
            name: id.to_uppercase(),
            sessions: vec![SessionRecord { next_maintenance: next, ..Default::default() }],
            ..Default::default()
        };
        let clients = vec![client("a", Some(at - DAY)), client("b", Some(at + 5 * DAY)), client("c", Some(at + 60 * DAY)), client("d", None), client("e", Some(at))];
        let visits = vec![visit("e", at + 2 * DAY, "planned"), visit("a", at - 30 * DAY, "done")];
        let due = due_clients(&clients, &visits, at);
        assert_eq!(due.iter().map(|d| d.client_id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }
}
