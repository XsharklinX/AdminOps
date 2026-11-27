//! Trabajos largos de noche: lo que tarda horas (la superficie entera de un
//! disco, DISM, SFC, la autoprueba larga de SMART) se apunta y AdminOps lo hace
//! a partir de la hora elegida, con el equipo sin suspenderse mientras trabaja.
//! Por la mañana, el Panel enseña qué salió.
//!
//! AdminOps tiene que quedarse abierta (minimizada o junto al reloj): no se
//! instala ninguna tarea en Windows.

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};

/// Lo que se puede dejar para la noche.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Job {
    /// Superficie de un disco (solo lectura).
    #[serde(rename_all = "camelCase")]
    Surface { disk: u32, model: String, hdd: bool, mode: String },
    /// Reparar la imagen de Windows y después comprobar sus archivos.
    Repair,
    /// Autoprueba larga del propio disco (la hace el disco por dentro).
    #[serde(rename_all = "camelCase")]
    SmartLong { disk: u32, model: String },
}

impl Job {
    pub fn title(&self) -> String {
        match self {
            Job::Surface { model, mode, .. } => format!("{} de la superficie · {model}", if mode == "full" { "Prueba completa" } else { "Prueba rápida" }),
            Job::Repair => "Reparar Windows (DISM y después SFC)".into(),
            Job::SmartLong { model, .. } => format!("Autoprueba larga del disco · {model}"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Queued {
    pub id: String,
    pub job: Job,
    pub title: String,
    pub added: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub id: String,
    pub title: String,
    pub started: u64,
    pub ended: u64,
    /// ok · warn · bad
    pub level: String,
    pub summary: String,
    /// Dónde verlo en AdminOps (página y sección).
    pub page: String,
    pub section: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct NightState {
    pub queue: Vec<Queued>,
    /// Hora de empezar («22:00»).
    pub at: String,
    /// Noche en la que se hizo lo último (AAAA-MM-DD del día en que empezó).
    pub last_night: Option<String>,
    pub results: Vec<Outcome>,
    /// Resultados que aún no se han enseñado en el Panel.
    pub unseen: u32,
    pub running: Option<String>,
}

impl Default for NightState {
    fn default() -> Self {
        NightState { queue: vec![], at: "22:00".into(), last_night: None, results: vec![], unseen: 0, running: None }
    }
}

/// La mañana corta la noche: lo que se apunte a partir de esta hora espera a la siguiente.
const MORNING: u32 = 7;
static FILE: Mutex<()> = Mutex::new(());
static WORKING: AtomicBool = AtomicBool::new(false);

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("trabajos-noche.json")
}

fn load(app: &tauri::AppHandle) -> NightState {
    crate::paths::read_json(&path(app))
}

fn save(app: &tauri::AppHandle, s: &NightState) -> Result<(), String> {
    crate::paths::write_json(&path(app), s)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// La noche a la que pertenece un momento: de madrugada, la del día anterior.
pub fn night_of(t: NaiveDateTime) -> NaiveDate {
    if t.hour() < MORNING {
        t.date().pred_opt().unwrap_or(t.date())
    } else {
        t.date()
    }
}

/// ¿Toca empezar? Desde la hora elegida hasta las 7 de la mañana, una vez por noche.
pub fn due(t: NaiveDateTime, at: &str, last_night: Option<&str>, queued: usize) -> bool {
    if queued == 0 {
        return false;
    }
    let start = NaiveTime::parse_from_str(at, "%H:%M").unwrap_or(NaiveTime::from_hms_opt(22, 0, 0).unwrap_or_default());
    let in_window = t.time() >= start || t.hour() < MORNING;
    let night = night_of(t).format("%Y-%m-%d").to_string();
    in_window && last_night != Some(night.as_str())
}

/// Que Windows no suspenda el equipo mientras se trabaja (y deje de impedirlo al acabar).
fn keep_awake(on: bool) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};
        SetThreadExecutionState(if on { ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED } else { ES_CONTINUOUS });
    }
    #[cfg(not(windows))]
    let _ = on;
}

fn run_one(app: &tauri::AppHandle, q: &Queued) -> Outcome {
    // Si AdminOps se cierra de golpe a medias, el aviso de abrir a salvo dirá cuál era (ver safestart.rs).
    let doing = format!("haciendo la tarea nocturna «{}»", q.title);
    crate::safestart::activity(&doing, true);
    let out = run_one_inner(app, q);
    crate::safestart::activity(&doing, false);
    out
}

fn run_one_inner(app: &tauri::AppHandle, q: &Queued) -> Outcome {
    let started = now();
    // En modo auditoría AdminOps solo mira: lo que cambia el equipo no se hace.
    if crate::audit::audit_mode() && q.job == Job::Repair {
        return Outcome { id: q.id.clone(), title: q.title.clone(), started, ended: now(), level: "warn".into(), summary: "No se hizo: el modo auditoría estaba activo.".into(), page: "machine".into(), section: Some("history".into()) };
    }
    let state = app.state::<crate::tweaks::TweakState>();
    let (level, summary, page, section) = match &q.job {
        Job::Surface { disk, model, hdd, mode } => match crate::diskscanio::disk_scan(app.clone(), *disk, model.clone(), *hdd, mode.clone(), true, state) {
            Ok(r) if r.bad > 0 => ("bad", format!("{} zonas que no se leen y {} lentas. La superficie falla: saca los datos y cambia el disco.", r.bad, r.slow + r.very_slow), "space", Some("health")),
            Ok(r) if r.slow + r.very_slow > 0 => ("warn", format!("Sin zonas ilegibles, pero {} lentas: vigílalo.", r.slow + r.very_slow), "space", Some("health")),
            Ok(r) => ("ok", format!("Superficie sana: {:.0} MB/s de media.", r.avg_mbps), "space", Some("health")),
            Err(e) => ("bad", format!("No se pudo terminar: {e}"), "space", Some("health")),
        },
        Job::Repair => {
            let msg = |id: &str| {
                crate::tweaks::run_action(app.clone(), id.into(), app.state())
                    .map(|op| serde_json::to_value(&op).ok().and_then(|v| v["message"].as_str().map(str::to_string)).unwrap_or_default())
            };
            match (msg("repair.dism"), msg("repair.sfc")) {
                (Ok(a), Ok(b)) => ("ok", format!("DISM: {a} · SFC: {b}"), "machine", Some("history")),
                (Err(e), _) | (_, Err(e)) => ("bad", format!("No se pudo terminar: {e}"), "machine", Some("history")),
            }
        }
        Job::SmartLong { disk, .. } => match crate::smartio::smart_selftest(*disk, "extended".into(), state) {
            Ok(_) => ("ok", "Autoprueba larga iniciada: el disco la hace por dentro en unas horas. El resultado sale en su ficha.".into(), "space", Some("health")),
            Err(e) => ("warn", format!("No se pudo empezar: {e}"), "space", Some("health")),
        },
    };
    Outcome { id: q.id.clone(), title: q.title.clone(), started, ended: now(), level: level.into(), summary, page: page.into(), section: section.map(str::to_string) }
}

/// Hace la cola entera, uno detrás de otro.
fn run_queue(app: &tauri::AppHandle) {
    // Reparar Windows, la superficie de un disco…: necesitan administrador. En modo usuario no se ejecutan ni se
    // gastan: esperan a una noche con AdminOps en modo técnico.
    if !crate::elevation::is_elevated() {
        log::info!("Trabajos de noche: AdminOps está en modo usuario; quedan para una noche en modo técnico.");
        return;
    }
    if WORKING.swap(true, Ordering::SeqCst) {
        return;
    }
    keep_awake(true);
    let night = night_of(Local::now().naive_local()).format("%Y-%m-%d").to_string();
    loop {
        let next = {
            let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
            let mut s = load(app);
            let Some(q) = s.queue.first().cloned() else { break };
            s.running = Some(q.title.clone());
            s.last_night = Some(night.clone());
            let _ = save(app, &s);
            q
        };
        log::info!("Trabajos de noche: empieza «{}»", next.title);
        let out = run_one(app, &next);
        log::info!("Trabajos de noche: «{}» → {}", next.title, out.summary);
        let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = load(app);
        s.queue.retain(|q| q.id != next.id);
        s.results.insert(0, out);
        s.results.truncate(30);
        s.unseen += 1;
        s.running = None;
        let _ = save(app, &s);
        let _ = app.emit("night-jobs", ());
    }
    keep_awake(false);
    WORKING.store(false, Ordering::SeqCst);
}

/// Mira cada medio minuto si toca empezar.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let s = {
            let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
            load(&app)
        };
        if due(Local::now().naive_local(), &s.at, s.last_night.as_deref(), s.queue.len()) {
            run_queue(&app);
        }
    });
}

#[tauri::command(async)]
pub fn night_jobs(app: tauri::AppHandle) -> NightState {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    load(&app)
}

#[tauri::command(async)]
pub fn night_job_add(app: tauri::AppHandle, job: Job) -> Result<NightState, String> {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(&app);
    if s.queue.iter().any(|q| q.job == job) {
        return Ok(s);
    }
    let t = now();
    s.queue.push(Queued { id: format!("{t:x}-{}", s.queue.len()), title: job.title(), job, added: t });
    // Lo apuntado de día es para esta noche aunque anoche ya se trabajara.
    if Local::now().hour() >= MORNING {
        let today = Local::now().date_naive();
        if s.last_night.as_deref() == Some(today.format("%Y-%m-%d").to_string().as_str()) && s.running.is_none() {
            s.last_night = today.pred_opt().map(|d| d.format("%Y-%m-%d").to_string());
        }
    }
    save(&app, &s)?;
    Ok(s)
}

#[tauri::command(async)]
pub fn night_job_remove(app: tauri::AppHandle, id: String) -> Result<NightState, String> {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(&app);
    s.queue.retain(|q| q.id != id);
    save(&app, &s)?;
    Ok(s)
}

#[tauri::command(async)]
pub fn night_set_time(app: tauri::AppHandle, at: String) -> Result<NightState, String> {
    NaiveTime::parse_from_str(&at, "%H:%M").map_err(|_| "Hora no válida (HH:MM).".to_string())?;
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(&app);
    s.at = at;
    save(&app, &s)?;
    Ok(s)
}

/// Empezar ya, sin esperar a la noche.
#[tauri::command(async)]
pub fn night_run_now(app: tauri::AppHandle) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Hace falta modo técnico (administrador) para esto.".into());
    }
    std::thread::spawn(move || run_queue(&app));
    Ok(())
}

/// El Panel ya enseñó los resultados.
#[tauri::command(async)]
pub fn night_seen(app: tauri::AppHandle) -> Result<(), String> {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load(&app);
    s.unseen = 0;
    save(&app, &s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    fn at(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn the_night_starts_at_the_chosen_time_and_ends_in_the_morning() {
        assert!(!due(at(16, 21, 59), "22:00", None, 1));
        assert!(due(at(16, 22, 0), "22:00", None, 1));
        // De madrugada sigue siendo la noche del día 16.
        assert!(due(at(17, 3, 0), "22:00", None, 1));
        assert_eq!(night_of(at(17, 3, 0)).day(), 16);
        // Por la mañana ya no.
        assert!(!due(at(17, 7, 30), "22:00", None, 1));
        // Una vez por noche, y sin nada apuntado nunca.
        assert!(!due(at(17, 2, 0), "22:00", Some("2026-10-16"), 1));
        assert!(due(at(17, 22, 30), "22:00", Some("2026-10-16"), 1));
        assert!(!due(at(16, 23, 0), "22:00", None, 0));
        // Una hora rara no rompe nada: se usa las 22:00.
        assert!(due(at(16, 22, 5), "a las diez", None, 1));
    }

    #[test]
    fn each_job_says_what_it_is() {
        let s = Job::Surface { disk: 1, model: "ST3500".into(), hdd: true, mode: "full".into() };
        assert_eq!(s.title(), "Prueba completa de la superficie · ST3500");
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"], "surface");
        let back: Job = serde_json::from_value(serde_json::json!({ "kind": "repair" })).unwrap();
        assert_eq!(back, Job::Repair);
    }
}
