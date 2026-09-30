//! Seguimientos: «volver a mirar esto el jueves».
//!
//! Lo que no se puede cerrar hoy (esperar a que el usuario pruebe, volver a
//! mirar un disco que se llena, llamar para confirmar) se apunta con una fecha y
//! aparece en «Hoy» cuando toca, con un aviso de Windows. Deja de depender de
//! acordarse o de una libreta.
//!
//! Son trabajo del técnico: se guardan con sus datos (en portable, en el USB).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

static FILE_LOCK: Mutex<()> = Mutex::new(());
/// Hechos que se conservan (los más recientes); los pendientes se guardan todos.
const KEEP_DONE: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Followup {
    pub id: String,
    pub text: String,
    /// Cuándo toca (segundos Unix).
    pub due: u64,
    pub done: bool,
    /// De quién o de qué equipo, si se sabe.
    pub person: String,
    pub machine: String,
    pub created: u64,
    /// Ya se avisó por Windows de que toca.
    pub notified: bool,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("followups.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Followup> {
    crate::paths::read_json(&path(app))
}

fn save(app: &tauri::AppHandle, list: &[Followup]) -> Result<(), String> {
    crate::paths::write_json(&path(app), &list)
}

fn clean(v: &str, max: usize) -> String {
    let limpio: String = v.chars().filter(|c| !c.is_control()).collect();
    limpio.trim().chars().take(max).collect()
}

fn validate(mut f: Followup) -> Result<Followup, String> {
    f.text = clean(&f.text, 300);
    if f.text.is_empty() {
        return Err("Escribe qué hay que hacer.".into());
    }
    if f.due == 0 {
        return Err("Elige cuándo.".into());
    }
    f.person = clean(&f.person, 120);
    f.machine = clean(&f.machine, 64).to_uppercase();
    Ok(f)
}

/// Pendientes primero, por fecha; luego los hechos, los más recientes.
fn ordered(mut v: Vec<Followup>) -> Vec<Followup> {
    v.sort_by(|a, b| a.done.cmp(&b.done).then(if a.done { b.due.cmp(&a.due) } else { a.due.cmp(&b.due) }));
    v
}

/// Quita los hechos más viejos cuando hay demasiados. Los pendientes nunca.
fn prune(list: &mut Vec<Followup>) {
    let hechos = list.iter().filter(|f| f.done).count();
    if hechos <= KEEP_DONE {
        return;
    }
    let mut fechas: Vec<u64> = list.iter().filter(|f| f.done).map(|f| f.due).collect();
    fechas.sort_unstable();
    let corte = fechas[hechos - KEEP_DONE];
    list.retain(|f| !f.done || f.due >= corte);
}

/// Los que ya tocan y aún no se han avisado. Se marcan como avisados.
pub fn take_due(list: &mut [Followup], at: u64) -> Vec<Followup> {
    let mut out = Vec::new();
    for f in list.iter_mut().filter(|f| !f.done && !f.notified && f.due <= at) {
        f.notified = true;
        out.push(f.clone());
    }
    out
}

/// Revisa si ha llegado la hora de algún seguimiento y avisa por Windows. Lo
/// llama el mismo bucle que avisa de las visitas de la Agenda.
pub fn check_due(app: &tauri::AppHandle) {
    use tauri_plugin_notification::NotificationExt;
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(app);
    let due = take_due(&mut list, now());
    if due.is_empty() {
        return;
    }
    let _ = save(app, &list);
    let titulo = if due.len() == 1 { "Seguimiento pendiente".to_string() } else { format!("{} seguimientos pendientes", due.len()) };
    let cuerpo = due.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join(" · ");
    let _ = app.notification().builder().title(titulo).body(cuerpo.chars().take(240).collect::<String>()).show();
}

// ---------- Comandos ----------

#[tauri::command]
pub fn list_followups(app: tauri::AppHandle) -> Vec<Followup> {
    ordered(load(&app))
}

#[tauri::command]
pub fn add_followup(app: tauri::AppHandle, followup: Followup) -> Result<Followup, String> {
    let mut f = validate(followup)?;
    f.id = format!("f{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
    f.created = now();
    f.done = false;
    f.notified = false;
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    list.push(f.clone());
    save(&app, &list)?;
    Ok(f)
}

#[tauri::command]
pub fn set_followup_done(app: tauri::AppHandle, id: String, done: bool) -> Result<(), String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let f = list.iter_mut().find(|f| f.id == id).ok_or("Ese seguimiento ya no existe.")?;
    f.done = done;
    prune(&mut list);
    save(&app, &list)
}

/// Lo aplaza unos días (y vuelve a avisar cuando toque).
#[tauri::command]
pub fn snooze_followup(app: tauri::AppHandle, id: String, days: u32) -> Result<(), String> {
    if !(1..=60).contains(&days) {
        return Err("Se puede aplazar entre 1 y 60 días.".into());
    }
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let f = list.iter_mut().find(|f| f.id == id).ok_or("Ese seguimiento ya no existe.")?;
    // Desde hoy si ya había vencido; desde su fecha si aún no.
    f.due = f.due.max(now()) + u64::from(days) * 86_400;
    f.notified = false;
    save(&app, &list)
}

#[tauri::command]
pub fn delete_followup(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    list.retain(|f| f.id != id);
    save(&app, &list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(id: &str, due: u64, done: bool) -> Followup {
        Followup { id: id.into(), text: "Volver a mirar".into(), due, done, ..Default::default() }
    }

    #[test]
    fn validation() {
        assert!(validate(Followup { text: "  ".into(), due: 10, ..Default::default() }).is_err());
        assert!(validate(Followup { text: "Llamar".into(), due: 0, ..Default::default() }).is_err());
        let v = validate(Followup { text: " Llamar a María \u{7}".into(), due: 10, machine: "pc-conta-03".into(), ..Default::default() }).unwrap();
        assert_eq!(v.text, "Llamar a María");
        assert_eq!(v.machine, "PC-CONTA-03");
    }

    /// Los pendientes van primero y por fecha; los hechos después, los recientes antes.
    #[test]
    fn pending_first_by_date() {
        let v = ordered(vec![f("hecho-viejo", 5, true), f("tarde", 30, false), f("pronto", 10, false), f("hecho-nuevo", 50, true)]);
        let ids: Vec<&str> = v.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["pronto", "tarde", "hecho-nuevo", "hecho-viejo"]);
    }

    /// Solo se avisa una vez, y solo de lo que ya toca y no está hecho.
    #[test]
    fn each_followup_is_notified_once() {
        let mut list = vec![f("ya", 100, false), f("luego", 500, false), f("hecho", 50, true)];
        let d = take_due(&mut list, 200);
        assert_eq!(d.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["ya"]);
        assert!(take_due(&mut list, 200).is_empty(), "no se repite");
        assert_eq!(take_due(&mut list, 600).len(), 1, "el de luego, cuando llega");
    }

    /// Nunca se borra un pendiente al podar, aunque haya muchos hechos.
    #[test]
    fn pruning_keeps_every_pending() {
        let mut list: Vec<Followup> = (0..KEEP_DONE as u64 + 50).map(|i| f(&format!("h{i}"), i, true)).collect();
        list.push(f("pendiente-viejo", 0, false));
        prune(&mut list);
        assert_eq!(list.iter().filter(|x| x.done).count(), KEEP_DONE);
        assert!(list.iter().any(|x| x.id == "pendiente-viejo"));
    }
}
