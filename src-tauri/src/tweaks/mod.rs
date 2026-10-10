//! Motor de ajustes: catálogo, diario de cambios, puntos de restauración y los
//! comandos que la UI puede invocar.

pub mod appx;
mod catalog;
pub mod profiles;
mod engine;
pub(crate) mod journal;
pub mod model;
pub mod preview;
pub(crate) mod registry;
mod restore;
pub mod roundtrip;
mod service;
pub mod startup;

use engine::Status;
use journal::{entry, Entry, Op};
use model::{Kind, Risk, Tweak};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use crate::task::Task;
use tauri::State;

/// No crear más de un punto de restauración automático cada 30 min: aplicar
/// cinco ajustes seguidos no debe generar cinco puntos.
const RESTORE_POINT_COOLDOWN: Duration = Duration::from_secs(30 * 60);

/// Cuándo crear un punto de restauración antes de cambiar el sistema (Ajustes → General):
/// 0 = solo antes de cambios con riesgo (por defecto), 1 = antes de cualquier cambio, 2 = nunca.
static RP_POLICY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_restore_point_policy(policy: &str) {
    let v = match policy {
        "always" => 1,
        "never" => 2,
        _ => 0,
    };
    RP_POLICY.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// Prefijo de error que la UI reconoce para ofrecer "continuar sin punto de restauración".
const RP_FAILED: &str = "RESTORE_POINT_FAILED::";

pub struct TweakState {
    catalog: Vec<Tweak>,
    journal: Mutex<journal::Journal>,
    last_restore_point: Mutex<Option<Instant>>,
    build: u32,
    /// Para avisar a la interfaz de cada cambio que se puede deshacer (botón en el aviso).
    app: Option<tauri::AppHandle>,
}

impl TweakState {
    pub fn new(app: &tauri::AppHandle) -> Self {
        Self { app: Some(app.clone()), ..Self::with_data_dir(crate::paths::machine_data_dir(app)) }
    }

    /// Estado con el diario en `dir` (modo línea de comandos y pruebas).
    pub fn with_data_dir(dir: std::path::PathBuf) -> Self {
        let build = registry::read_string(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber")
            .and_then(|b| b.parse().ok())
            .unwrap_or(0);
        Self {
            catalog: catalog::load(),
            journal: Mutex::new(journal::Journal::load(dir.join("journal.json"))),
            last_restore_point: Mutex::new(None),
            build,
            app: None,
        }
    }

    fn find(&self, id: &str) -> Result<&Tweak, String> {
        self.catalog.iter().find(|t| t.id == id).ok_or_else(|| format!("Ajuste desconocido: {id}"))
    }

    fn check_can_modify(&self, t: &Tweak) -> Result<(), String> {
        if !t.supported_on(self.build) {
            return Err(format!("No compatible con esta versión de Windows (build {}).", self.build));
        }
        if t.needs_admin() && !crate::elevation::is_elevated() {
            return Err("Requiere ejecutar AdminOps como administrador.".into());
        }
        Ok(())
    }

    /// Tareas que puede ejecutar el mantenimiento programado: solo limpiezas.
    pub fn maintenance_tasks(&self) -> Vec<(String, String)> {
        self.catalog
            .iter()
            .filter(|t| t.category == "cleanup" && t.kind == Kind::Action && t.supported_on(self.build))
            .map(|t| (t.id.clone(), t.name.clone()))
            .collect()
    }

    /// Ejecuta una limpieza sin interfaz (mantenimiento programado) y la anota en el diario.
    pub fn run_maintenance_task(&self, id: &str) -> Result<String, String> {
        let t = self.find(id)?;
        if t.category != "cleanup" || t.kind != Kind::Action {
            return Err(format!("{id} no es una tarea de limpieza."));
        }
        self.check_can_modify(t)?;
        let mut e = entry(Op::Run, Some(&t.id), &format!("{} (mantenimiento programado)", t.name));
        let result = engine::run_action(t, None);
        match &result {
            Ok(msg) => e.message = Some(msg.clone()),
            Err(err) => {
                e.ok = false;
                e.message = Some(err.clone());
            }
        }
        self.log(e);
        result
    }

    fn log(&self, e: Entry) -> u64 {
        let (id, undoable) = {
            let mut j = self.journal.lock().unwrap_or_else(|e| e.into_inner());
            let id = j.push(e);
            (id, j.get(id).map(|x| (x.undoable, x.title.clone())))
        };
        if let (Some(app), Some((true, title))) = (&self.app, undoable) {
            use tauri::Emitter;
            let _ = app.emit("undoable-change", serde_json::json!({ "id": id, "title": title }));
        }
        id
    }

    /// Ejecuta una tarea del catálogo (reparación, limpieza) desde otra parte de AdminOps.
    pub fn run_catalog(&self, id: &str) -> Result<String, String> {
        let t = self.find(id)?;
        self.check_can_modify(t)?;
        run_logged(self, t, None)
    }

    /// Aplica o ejecuta un elemento del catálogo, según sea un ajuste o una tarea.
    /// Lo usa el botón «Arreglar» de los hallazgos del diagnóstico.
    pub fn fix_catalog(&self, id: &str) -> Result<String, String> {
        let t = self.find(id)?;
        self.check_can_modify(t)?;
        match t.kind {
            Kind::Action => run_logged(self, t, None),
            _ => apply_logged(self, t, None).map(|()| if t.reboot { "Aplicado. Reinicia para que surta efecto.".into() } else { "Aplicado.".to_string() }),
        }
    }

    /// Registra en el diario una operación ajena al catálogo (finalizar un
    /// proceso, actualizar software…). No se puede deshacer.
    pub fn record<T>(&self, op: Op, title: &str, result: &Result<T, String>) {
        let mut e = entry(op, None, title);
        if let Err(err) = result {
            e.ok = false;
            e.message = Some(err.clone());
        }
        self.log(e);
    }

    /// Cuántos ajustes (toggle) del catálogo están aplicados ahora mismo.
    pub fn applied_count(&self) -> usize {
        let toggles: Vec<&Tweak> = self.catalog.iter().filter(|t| t.kind == Kind::Toggle).collect();
        std::thread::scope(|s| {
            let handles: Vec<_> = toggles.iter().map(|t| s.spawn(|| engine::detect(t))).collect();
            handles.into_iter().filter_map(|h| h.join().ok()).filter(|st| *st == Status::Applied).count()
        })
    }

    /// Entradas del diario desde `since` (segundos epoch), de más antigua a más reciente.
    /// Historial antiguo fuera (ver `Journal::prune_before`).
    pub fn prune_journal(&self, before: u64) -> usize {
        self.journal.lock().unwrap_or_else(|e| e.into_inner()).prune_before(before)
    }

    pub fn journal_len(&self) -> usize {
        self.journal.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn journal_since(&self, since: u64) -> Vec<Entry> {
        let mut v: Vec<Entry> = self.journal.lock().unwrap_or_else(|e| e.into_inner()).newest_first().into_iter().filter(|e| e.timestamp >= since).collect();
        v.reverse();
        v
    }
}

/// Crea un punto de restauración salvo que ya haya uno reciente de esta sesión.
/// Devuelve si lo creó. Si falla, el error lleva el prefijo `RP_FAILED`.
fn ensure_restore_point(state: &TweakState, task: &Task, what: &str, tweak_id: Option<&str>, risky: bool) -> Result<bool, String> {
    match RP_POLICY.load(std::sync::atomic::Ordering::Relaxed) {
        2 => return Ok(false),
        0 if !risky => return Ok(false),
        _ => {}
    }
    let recent = state.last_restore_point.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|i| i.elapsed() < RESTORE_POINT_COOLDOWN);
    if recent {
        return Ok(false);
    }
    task.step("Creando punto de restauración (puede tardar 1–2 minutos)…");
    let desc = format!("AdminOps: antes de '{what}'");
    let mut e = entry(Op::RestorePoint, tweak_id, &desc);
    let result = restore::create(&desc, task);
    if let Err(err) = &result {
        e.ok = false;
        e.message = Some(err.clone());
    }
    state.log(e);
    match result {
        Ok(()) => {
            *state.last_restore_point.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
            Ok(true)
        }
        Err(err) => Err(format!("{RP_FAILED}{err}")),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakView {
    id: String,
    name: String,
    description: String,
    category: String,
    risk: Risk,
    kind: Kind,
    note: Option<String>,
    reboot: bool,
    needs_admin: bool,
    supported: bool,
    status: Status,
    /// Hay una aplicación hecha por AdminOps que se puede deshacer exactamente.
    has_backup: bool,
    changes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    status: Status,
    message: String,
    restore_point_created: bool,
}

fn view(state: &TweakState, t: &Tweak, status: Status) -> TweakView {
    TweakView {
        id: t.id.clone(),
        name: t.name.clone(),
        description: t.description.clone(),
        category: t.category.clone(),
        risk: t.risk,
        kind: t.kind,
        note: t.note.clone(),
        reboot: t.reboot,
        needs_admin: t.needs_admin(),
        supported: t.supported_on(state.build),
        status,
        has_backup: state.journal.lock().unwrap_or_else(|e| e.into_inner()).pending_apply(&t.id).is_some(),
        changes: engine::describe(t),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakIndex {
    id: String,
    name: String,
    description: String,
    category: String,
}

/// Nombre y categoría de todo el catálogo, sin detectar el estado (para la búsqueda global).
#[tauri::command]
pub fn tweak_index(state: State<'_, TweakState>) -> Vec<TweakIndex> {
    state
        .catalog
        .iter()
        .map(|t| TweakIndex { id: t.id.clone(), name: t.name.clone(), description: t.description.clone(), category: t.category.clone() })
        .collect()
}

#[tauri::command(async)]
pub fn list_tweaks(category: Option<String>, state: State<'_, TweakState>) -> Result<Vec<TweakView>, String> {
    let selected: Vec<&Tweak> = state
        .catalog
        .iter()
        .filter(|t| category.as_ref().is_none_or(|c| &t.category == c))
        .collect();
    // La detección por script lanza PowerShell (~300 ms): en paralelo.
    let statuses: Vec<Status> = std::thread::scope(|s| {
        let handles: Vec<_> = selected.iter().map(|t| s.spawn(|| engine::detect(t))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or(Status::Unknown)).collect()
    });
    Ok(selected.iter().zip(statuses).map(|(t, st)| view(&state, t, st)).collect())
}

#[tauri::command(async)]
pub fn apply_tweak(
    app: tauri::AppHandle,
    id: String,
    skip_restore_point: bool,
    state: State<'_, TweakState>,
) -> Result<OpResult, String> {
    let t = state.find(&id)?;
    let task = Task::new(&app, format!("tweak:{id}")).named(t.name.clone());
    if t.kind != Kind::Toggle {
        return Err("Esto es una tarea puntual, no un ajuste.".into());
    }
    state.check_can_modify(t)?;

    let restore_point_created =
        !skip_restore_point && ensure_restore_point(&state, &task, &t.name, Some(&t.id), t.risk >= Risk::Medium)?;
    task.step(format!("Aplicando {}…", t.name));

    apply_logged(&state, t, Some(&task))?;
    let message = if t.reboot { "Aplicado. Reinicia para que surta efecto." } else { "Aplicado." };
    Ok(OpResult { status: engine::detect(t), message: message.into(), restore_point_created })
}

/// Aplica un ajuste y lo registra en el diario (sin punto de restauración).
fn apply_logged(state: &TweakState, t: &Tweak, task: Option<&Task>) -> Result<(), String> {
    let mut e = entry(Op::Apply, Some(&t.id), &t.name);
    match engine::apply(t, task) {
        Ok(backups) => {
            e.backups = backups;
            state.log(e);
            Ok(())
        }
        Err(err) => {
            e.ok = false;
            e.message = Some(err.clone());
            state.log(e);
            Err(err)
        }
    }
}

/// Deshace la última aplicación hecha por AdminOps usando su copia del diario.
/// `Ok(false)` si no había nada que deshacer.
fn revert_logged(state: &TweakState, t: &Tweak, task: Option<&Task>) -> Result<bool, String> {
    let pending = state.journal.lock().unwrap_or_else(|e| e.into_inner()).pending_apply(&t.id).map(|e| (e.id, e.backups.clone()));
    let Some((entry_id, backups)) = pending else { return Ok(false) };
    let result = engine::restore(Some(t), &backups, task);
    finish_revert(state, Some(t), &t.name, Some(entry_id), result).map(|_| true)
}

/// Ejecuta una tarea puntual y la registra en el diario.
fn run_logged(state: &TweakState, t: &Tweak, task: Option<&Task>) -> Result<String, String> {
    let mut e = entry(Op::Run, Some(&t.id), &t.name);
    let result = engine::run_action(t, task);
    match &result {
        Ok(msg) => e.message = Some(msg.clone()),
        Err(err) => {
            e.ok = false;
            e.message = Some(err.clone());
        }
    }
    state.log(e);
    result
}

/// Deshace un ajuste: con la copia exacta del diario si AdminOps lo aplicó,
/// o con los valores de fábrica del catálogo si ya venía aplicado.
#[tauri::command(async)]
pub fn revert_tweak(app: tauri::AppHandle, id: String, state: State<'_, TweakState>) -> Result<OpResult, String> {
    let t = state.find(&id)?;
    state.check_can_modify(t)?;
    let task = Task::new(&app, format!("tweak:{id}")).named(t.name.clone());
    task.step(format!("Deshaciendo {}…", t.name));
    let pending = state.journal.lock().unwrap_or_else(|e| e.into_inner()).pending_apply(&t.id).map(|e| (e.id, e.backups.clone()));
    let result = match &pending {
        Some((_, backups)) => engine::restore(Some(t), backups, Some(&task)),
        None => engine::revert_to_defaults(t, Some(&task)),
    };
    finish_revert(&state, Some(t), &t.name, pending.map(|p| p.0), result)
}

/// Deshace una entrada concreta del historial (ajuste, cambio de Inicio o app quitada).
#[tauri::command(async)]
pub fn revert_entry(app: tauri::AppHandle, entry_id: u64, state: State<'_, TweakState>) -> Result<OpResult, String> {
    let e = state.journal.lock().unwrap_or_else(|e| e.into_inner()).get(entry_id).cloned().ok_or("Entrada no encontrada")?;
    if !e.undoable || e.reverted {
        return Err("Esta entrada no se puede deshacer.".into());
    }
    let tweak = match e.tweak_id.as_deref() {
        Some(id) => Some(state.find(id)?),
        None => None,
    };
    match tweak {
        Some(t) => state.check_can_modify(t)?,
        None if !crate::elevation::is_elevated() && needs_admin(&e.backups) => {
            return Err("Requiere ejecutar AdminOps como administrador.".into())
        }
        None => {}
    }
    let task = Task::new(&app, format!("entry:{entry_id}"));
    task.step(format!("Deshaciendo {}…", e.title));
    let result = engine::restore(tweak, &e.backups, Some(&task));
    finish_revert(&state, tweak, &e.title, Some(entry_id), result)
}

fn needs_admin(backups: &[journal::Backup]) -> bool {
    backups.iter().any(|b| match b {
        journal::Backup::Registry { path, .. } => !path.to_ascii_uppercase().starts_with("HKCU"),
        _ => true,
    })
}

fn finish_revert(
    state: &TweakState,
    t: Option<&Tweak>,
    title: &str,
    applied_entry: Option<u64>,
    result: Result<(), String>,
) -> Result<OpResult, String> {
    let mut e = entry(Op::Revert, t.map(|t| t.id.as_str()), title);
    match result {
        Ok(()) => {
            if let Some(id) = applied_entry {
                state.journal.lock().unwrap_or_else(|e| e.into_inner()).mark_reverted(id);
            } else {
                e.message = Some("Sin copia previa: restaurados valores de fábrica.".into());
            }
            state.log(e);
            let reboot = t.is_some_and(|t| t.reboot);
            let message = if reboot { "Deshecho. Reinicia para que surta efecto." } else { "Deshecho." };
            let status = t.map_or(Status::Action, engine::detect);
            Ok(OpResult { status, message: message.into(), restore_point_created: false })
        }
        Err(err) => {
            e.ok = false;
            e.message = Some(err.clone());
            state.log(e);
            Err(err)
        }
    }
}

#[tauri::command(async)]
pub fn run_action(app: tauri::AppHandle, id: String, state: State<'_, TweakState>) -> Result<OpResult, String> {
    let t = state.find(&id)?;
    if t.kind != Kind::Action {
        return Err("Esto es un ajuste, no una tarea.".into());
    }
    state.check_can_modify(t)?;
    let task = Task::new(&app, format!("tweak:{id}")).named(t.name.clone());
    task.step(format!("Ejecutando {}…", t.name));
    run_logged(&state, t, Some(&task)).map(|message| OpResult { status: Status::Action, message, restore_point_created: false })
}

#[tauri::command]
pub fn get_journal(state: State<'_, TweakState>) -> Vec<Entry> {
    state.journal.lock().unwrap_or_else(|e| e.into_inner()).newest_first()
}

#[tauri::command(async)]
pub fn create_restore_point(app: tauri::AppHandle, state: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let desc = "AdminOps: punto manual";
    let mut e = entry(Op::RestorePoint, None, desc);
    let task = Task::new(&app, "restore-point");
    task.step("Creando punto de restauración (puede tardar 1–2 minutos)…");
    let result = restore::create(desc, &task);
    match &result {
        Ok(()) => *state.last_restore_point.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now()),
        Err(err) => {
            e.ok = false;
            e.message = Some(err.clone());
        }
    }
    state.log(e);
    result
}

#[tauri::command(async)]
pub fn list_restore_points() -> Result<Vec<restore::RestorePoint>, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    restore::list()
}

#[tauri::command]
pub fn open_system_restore() -> Result<(), String> {
    restore::open_wizard()
}

/// Ejecuta el arreglo de un hallazgo del diagnóstico (botón «Arreglar»).
#[tauri::command(async)]
pub fn fix_finding(id: String, state: State<'_, TweakState>) -> Result<String, String> {
    state.fix_catalog(&id)
}
