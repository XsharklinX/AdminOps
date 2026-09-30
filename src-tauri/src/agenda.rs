//! Agenda: visitas a clientes, pero también tareas, llamadas y reuniones sin
//! cliente (quien trabaja en una sola empresa no tiene «clientes»). Lo que toca
//! hoy, lo de mañana y un aviso de Windows antes de cada cosa. Viaja con los
//! datos compartidos (en el USB con el portable), como los clientes.

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
    /// "" o "visit" visita · "task" tarea · "call" llamada · "meeting" reunión.
    pub kind: String,
    /// Qué es («Cambiar el disco del servidor»). Obligatorio si no hay cliente.
    pub title: String,
    /// Cliente, si es una visita a uno. Puede ir vacío.
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
    /// Tipo de visita (id de los configurados en Ajustes): al empezar la sesión
    /// trae ya su checklist, en vez de elegirlo otra vez a mano.
    pub visit_type: String,
    /// "" no se repite · weekly · biweekly · monthly · quarterly · semiannual · yearly
    pub repeat_every: String,
    /// Dónde es (sede, planta, sala). Lo que hace falta para llegar.
    pub place: String,
    /// Id del evento en el calendario de Outlook, si se puso ahí (graph.rs).
    pub outlook_event: String,
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

impl Visit {
    /// Cómo se nombra en avisos, listas y conflictos: el título o, si no hay, el cliente.
    pub fn label(&self) -> &str {
        if self.title.is_empty() {
            &self.client_name
        } else {
            &self.title
        }
    }
}

const KINDS: &[&str] = &["visit", "task", "call", "meeting"];

fn validate(mut v: Visit) -> Result<Visit, String> {
    v.client_id = v.client_id.trim().to_string();
    v.title = v.title.trim().chars().take(120).collect();
    if v.client_id.is_empty() && v.title.is_empty() {
        return Err("Escribe qué es, o elige un cliente.".into());
    }
    if !KINDS.contains(&v.kind.as_str()) {
        v.kind = "visit".into();
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
    if repeat_seconds(&v.repeat_every).is_none() {
        v.repeat_every = String::new();
    }
    v.visit_type = v.visit_type.trim().chars().take(60).collect();
    v.place = v.place.trim().chars().take(120).collect();
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

/// «Visita», «Tarea», «Llamada», «Reunión».
pub fn kind_name(kind: &str) -> &'static str {
    match kind {
        "task" => "Tarea",
        "call" => "Llamada",
        "meeting" => "Reunión",
        _ => "Visita",
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
            let list = todays.iter().map(|v| format!("{} {}", local_time(v.start).format("%H:%M"), v.label())).collect::<Vec<_>>().join(" · ");
            let n = todays.len();
            notify(app, &format!("Hoy tienes {n} {} en la agenda", if n == 1 { "cosa" } else { "cosas" }), &list);
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
        let body = format!("{} a las {}{}{}", v.label(), local_time(v.start).format("%H:%M"), machines_text(v.machines), if v.notes.is_empty() { String::new() } else { format!(" · {}", v.notes) });
        notify(app, &format!("{} en menos de 30 minutos", kind_name(&v.kind)), &body);
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
            crate::followups::check_due(&app);
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

/// Visita guardada y, si la hay, la advertencia de que se pisa con otra.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedVisit {
    pub visit: Visit,
    /// Cliente de la visita con la que se solapa ("" si no hay conflicto). No
    /// impide guardar: a veces se solapan a propósito; solo avisa.
    pub conflict: String,
}

#[tauri::command]
pub fn save_visit(app: tauri::AppHandle, visit: Visit) -> Result<SavedVisit, String> {
    let mut v = validate(visit)?;
    if v.client_id.is_empty() {
        v.client_name.clear();
    } else {
        let client = crate::workflow::find_client(&app, &v.client_id).ok_or("Ese cliente ya no existe.")?;
        v.client_name = client.name.clone();
        if v.machines == 0 && v.kind == "visit" {
            v.machines = client.machines.len() as u32;
        }
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
            // El editor no conoce el evento de Outlook: se conserva el que tenía.
            if v.outlook_event.is_empty() {
                v.outlook_event = x.outlook_event.clone();
            }
            *x = v.clone();
        }
        None => {
            v.id = format!("v{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
            v.reminded = false;
            list.push(v.clone());
        }
    }
    let conflict = overlaps(&list, &v).unwrap_or_default();
    save(&app, &list)?;
    Ok(SavedVisit { visit: v, conflict })
}

/// Cambia el estado de una visita. Al darla por hecha, si se repite, se crea ya
/// la siguiente: así el ciclo de mantenimiento no depende de que alguien se
/// acuerde de volver a apuntarlo.
#[tauri::command]
pub fn set_visit_status(app: tauri::AppHandle, id: String, status: String) -> Result<Option<Visit>, String> {
    if !["planned", "done", "cancelled"].contains(&status.as_str()) {
        return Err("Estado no válido.".into());
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let v = list.iter_mut().find(|v| v.id == id).ok_or("Esa visita ya no existe.")?;
    v.status = status.clone();
    let mut creada = None;
    if status == "done" {
        if let Some(mut siguiente) = next_in_series(v) {
            siguiente.id = format!("v{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
            list.push(siguiente.clone());
            creada = Some(siguiente);
        }
    }
    save(&app, &list)?;
    Ok(creada)
}

#[tauri::command]
pub fn delete_visit(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    list.retain(|v| v.id != id);
    save(&app, &list)
}

/// Cada cuánto se repite una visita. La mayor parte del trabajo de
/// mantenimiento es periódico, y volver a crearla a mano cada vez es la razón
/// por la que las agendas se abandonan.
pub fn repeat_seconds(every: &str) -> Option<u64> {
    match every {
        "weekly" => Some(7 * DAY),
        "biweekly" => Some(14 * DAY),
        "monthly" => Some(30 * DAY),
        "quarterly" => Some(91 * DAY),
        "semiannual" => Some(182 * DAY),
        "yearly" => Some(365 * DAY),
        _ => None,
    }
}

/// Visitas que se pisan con otra ya planificada.
///
/// Dos visitas a la vez no es un detalle estético: significa que una de las dos
/// no se va a atender, y el técnico se entera el día de la visita.
pub fn overlaps(visits: &[Visit], candidate: &Visit) -> Option<String> {
    let fin = |v: &Visit| v.start + u64::from(v.minutes) * 60;
    visits
        .iter()
        .find(|v| v.id != candidate.id && v.status == "planned" && v.start < fin(candidate) && candidate.start < fin(v))
        .map(|v| v.label().to_string())
}

/// La siguiente visita de una serie que se repite, si la hay.
///
/// Al marcar una como hecha se crea la próxima, para que el ciclo no dependa de
/// que alguien se acuerde. Se cuenta desde la fecha planificada, no desde hoy,
/// para que atender con retraso no desplace toda la serie.
pub fn next_in_series(done: &Visit) -> Option<Visit> {
    let paso = repeat_seconds(&done.repeat_every)?;
    let mut siguiente = done.clone();
    siguiente.id = String::new();
    siguiente.start = done.start + paso;
    siguiente.status = "planned".into();
    siguiente.reminded = false;
    Some(siguiente)
}

/// Una visita por su id (para ponerla en Outlook).
pub fn visit_by_id(app: &tauri::AppHandle, id: &str) -> Option<Visit> {
    load(app).into_iter().find(|v| v.id == id)
}

/// Apunta en la visita el evento de Outlook que le corresponde.
pub fn set_outlook_event(app: &tauri::AppHandle, id: &str, event: &str) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(app);
    let v = list.iter_mut().find(|v| v.id == id).ok_or("Esa visita ya no existe.")?;
    v.outlook_event = event.chars().take(300).collect();
    save(app, &list)
}

/// Aplaza una visita el número de días indicado.
#[tauri::command]
pub fn postpone_visit(app: tauri::AppHandle, id: String, days: i64) -> Result<(), String> {
    if !(-30..=365).contains(&days) || days == 0 {
        return Err("Solo se puede aplazar entre 1 y 365 días (o adelantar hasta 30).".into());
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let v = list.iter_mut().find(|v| v.id == id).ok_or("Esa visita ya no existe.")?;
    let nuevo = v.start as i64 + days * DAY as i64;
    if nuevo <= 0 {
        return Err("La fecha resultante no es válida.".into());
    }
    v.start = nuevo as u64;
    // Cambió la fecha: hay que volver a avisar.
    v.reminded = false;
    save(&app, &list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visita(id: &str, start: u64, minutes: u32) -> Visit {
        Visit { id: id.into(), client_id: "c1".into(), client_name: "Cliente".into(), start, minutes, status: "planned".into(), ..Default::default() }
    }

    /// Dos visitas a la vez no es un detalle estético: una de las dos no se va a
    /// atender, y hay que saberlo al planificar, no el día de la visita.
    #[test]
    fn warns_when_two_visits_overlap() {
        let list = vec![visita("a", 10_000, 60)];
        // Empieza dentro de la anterior.
        assert_eq!(overlaps(&list, &visita("b", 11_800, 60)).as_deref(), Some("Cliente"));
        // Justo después: no se pisan.
        assert!(overlaps(&list, &visita("b", 13_600, 60)).is_none());
        // Justo antes de que empiece la otra.
        assert!(overlaps(&list, &visita("b", 6_400, 60)).is_none());
        // La misma visita no se pisa consigo misma al editarla.
        assert!(overlaps(&list, &visita("a", 10_000, 120)).is_none());
        // Una visita cancelada ya no ocupa hueco.
        let cancelada = vec![Visit { status: "cancelled".into(), ..visita("a", 10_000, 60) }];
        assert!(overlaps(&cancelada, &visita("b", 11_000, 60)).is_none());
    }

    /// Al dar por hecha una visita periódica se crea ya la siguiente, contada
    /// desde la fecha planificada: atender con retraso no debe desplazar la serie.
    #[test]
    fn a_repeating_visit_schedules_the_next_one() {
        let hecha = Visit { repeat_every: "monthly".into(), visit_type: "mantenimiento".into(), place: "Planta 2".into(), reminded: true, ..visita("a", 1_000_000, 90) };
        let siguiente = next_in_series(&hecha).expect("mensual se repite");
        assert_eq!(siguiente.start, 1_000_000 + 30 * DAY);
        assert_eq!(siguiente.status, "planned");
        assert!(!siguiente.reminded, "la nueva aún no se ha avisado");
        assert!(siguiente.id.is_empty(), "el id lo pone quien la guarda");
        // Se conserva lo que define la visita, no solo la fecha.
        assert_eq!(siguiente.visit_type, "mantenimiento");
        assert_eq!(siguiente.place, "Planta 2");
        assert_eq!(siguiente.minutes, 90);
        // Una visita suelta no genera nada.
        assert!(next_in_series(&visita("a", 1_000_000, 60)).is_none());
    }

    #[test]
    fn repeat_periods_are_known() {
        assert_eq!(repeat_seconds("weekly"), Some(7 * DAY));
        assert_eq!(repeat_seconds("quarterly"), Some(91 * DAY));
        assert_eq!(repeat_seconds(""), None);
        assert_eq!(repeat_seconds("cada rato"), None);
    }

    /// Los campos nuevos se limpian igual que los de siempre.
    #[test]
    fn validation_cleans_the_new_fields() {
        let v = validate(Visit { client_id: "c1".into(), start: 10, repeat_every: "cada rato".into(), place: "  Sede central  ".into(), visit_type: " rutina ".into(), ..Default::default() }).unwrap();
        assert_eq!(v.repeat_every, "", "un periodo inventado no se guarda");
        assert_eq!(v.place, "Sede central");
        assert_eq!(v.visit_type, "rutina");
    }
    use crate::workflow::{Client, SessionRecord};

    fn visit(client: &str, start: u64, status: &str) -> Visit {
        Visit { id: format!("v{start}"), client_id: client.into(), client_name: client.into(), start, minutes: 60, status: status.into(), ..Default::default() }
    }

    /// Quien trabaja en una sola empresa no tiene clientes: una tarea con título basta.
    #[test]
    fn entries_without_a_client_are_fine() {
        let v = validate(Visit { title: "  Cambiar el disco del servidor ".into(), kind: "task".into(), start: 10, ..Default::default() }).unwrap();
        assert_eq!((v.title.as_str(), v.kind.as_str(), v.label()), ("Cambiar el disco del servidor", "task", "Cambiar el disco del servidor"));
        // Sin título ni cliente no se sabe qué es.
        assert!(validate(Visit { title: "   ".into(), start: 10, ..Default::default() }).is_err());
        // Un tipo inventado es una visita; las viejas (sin tipo) también.
        assert_eq!(validate(Visit { client_id: "c1".into(), kind: "fiesta".into(), start: 10, ..Default::default() }).unwrap().kind, "visit");
        // Con cliente y sin título, se nombra por el cliente.
        assert_eq!(Visit { client_name: "Clínica Norte".into(), ..Default::default() }.label(), "Clínica Norte");
        assert_eq!(kind_name("call"), "Llamada");
        assert_eq!(kind_name(""), "Visita");
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
