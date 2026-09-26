//! Motor de ajustes: catálogo, diario de cambios, puntos de restauración y los
//! comandos que la UI puede invocar.

pub mod appx;
mod catalog;
mod engine;
pub(crate) mod journal;
pub mod model;
pub(crate) mod registry;
mod restore;
mod service;
pub mod startup;

use engine::Status;
use journal::{entry, Entry, Op};
use model::{Kind, Risk, Tweak};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Manager, State};

/// No crear más de un punto de restauración automático cada 30 min: aplicar
/// cinco ajustes seguidos no debe generar cinco puntos.
const RESTORE_POINT_COOLDOWN: Duration = Duration::from_secs(30 * 60);

/// Prefijo de error que la UI reconoce para ofrecer "continuar sin punto de restauración".
const RP_FAILED: &str = "RESTORE_POINT_FAILED::";

pub struct TweakState {
    catalog: Vec<Tweak>,
    journal: Mutex<journal::Journal>,
    last_restore_point: Mutex<Option<Instant>>,
    build: u32,
}

impl TweakState {
    pub fn new(app: &tauri::AppHandle) -> Self {
        let dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("AdminOps"));
        let build = registry::read_string(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber")
            .and_then(|b| b.parse().ok())
            .unwrap_or(0);
        Self {
            catalog: catalog::load(),
            journal: Mutex::new(journal::Journal::load(dir.join("journal.json"))),
            last_restore_point: Mutex::new(None),
            build,
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

    fn log(&self, e: Entry) -> u64 {
        self.journal.lock().unwrap().push(e)
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
    pub fn journal_since(&self, since: u64) -> Vec<Entry> {
        let mut v: Vec<Entry> = self.journal.lock().unwrap().newest_first().into_iter().filter(|e| e.timestamp >= since).collect();
        v.reverse();
        v
    }
}

/// Crea un punto de restauración salvo que ya haya uno reciente de esta sesión.
/// Devuelve si lo creó. Si falla, el error lleva el prefijo `RP_FAILED`.
fn ensure_restore_point(state: &TweakState, what: &str, tweak_id: Option<&str>) -> Result<bool, String> {
    let recent = state.last_restore_point.lock().unwrap().is_some_and(|i| i.elapsed() < RESTORE_POINT_COOLDOWN);
    if recent {
        return Ok(false);
    }
    let desc = format!("AdminOps: antes de '{what}'");
    let mut e = entry(Op::RestorePoint, tweak_id, &desc);
    let result = restore::create(&desc);
    if let Err(err) = &result {
        e.ok = false;
        e.message = Some(err.clone());
    }
    state.log(e);
    match result {
        Ok(()) => {
            *state.last_restore_point.lock().unwrap() = Some(Instant::now());
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
        has_backup: state.journal.lock().unwrap().pending_apply(&t.id).is_some(),
        changes: engine::describe(t),
    }
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
pub fn apply_tweak(id: String, skip_restore_point: bool, state: State<'_, TweakState>) -> Result<OpResult, String> {
    let t = state.find(&id)?;
    if t.kind != Kind::Toggle {
        return Err("Esto es una tarea puntual, no un ajuste.".into());
    }
    state.check_can_modify(t)?;

    let restore_point_created =
        t.risk >= Risk::Medium && !skip_restore_point && ensure_restore_point(&state, &t.name, Some(&t.id))?;

    let mut e = entry(Op::Apply, Some(&t.id), &t.name);
    match engine::apply(t) {
        Ok(backups) => {
            e.backups = backups;
            state.log(e);
            let status = engine::detect(t);
            let message = if t.reboot { "Aplicado. Reinicia para que surta efecto." } else { "Aplicado." };
            Ok(OpResult { status, message: message.into(), restore_point_created })
        }
        Err(err) => {
            e.ok = false;
            e.message = Some(err.clone());
            state.log(e);
            Err(err)
        }
    }
}

/// Deshace un ajuste: con la copia exacta del diario si AdminOps lo aplicó,
/// o con los valores de fábrica del catálogo si ya venía aplicado.
#[tauri::command(async)]
pub fn revert_tweak(id: String, state: State<'_, TweakState>) -> Result<OpResult, String> {
    let t = state.find(&id)?;
    state.check_can_modify(t)?;
    let pending = state.journal.lock().unwrap().pending_apply(&t.id).map(|e| (e.id, e.backups.clone()));
    let result = match &pending {
        Some((_, backups)) => engine::restore(Some(t), backups),
        None => engine::revert_to_defaults(t),
    };
    finish_revert(&state, Some(t), &t.name, pending.map(|p| p.0), result)
}

/// Deshace una entrada concreta del historial (ajuste, cambio de Inicio o app quitada).
#[tauri::command(async)]
pub fn revert_entry(entry_id: u64, state: State<'_, TweakState>) -> Result<OpResult, String> {
    let e = state.journal.lock().unwrap().get(entry_id).cloned().ok_or("Entrada no encontrada")?;
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
    let result = engine::restore(tweak, &e.backups);
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
                state.journal.lock().unwrap().mark_reverted(id);
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
pub fn run_action(id: String, state: State<'_, TweakState>) -> Result<OpResult, String> {
    let t = state.find(&id)?;
    if t.kind != Kind::Action {
        return Err("Esto es un ajuste, no una tarea.".into());
    }
    state.check_can_modify(t)?;
    let mut e = entry(Op::Run, Some(&t.id), &t.name);
    let result = engine::run_action(t);
    match &result {
        Ok(msg) => e.message = Some(msg.clone()),
        Err(err) => {
            e.ok = false;
            e.message = Some(err.clone());
        }
    }
    state.log(e);
    result.map(|message| OpResult { status: Status::Action, message, restore_point_created: false })
}

#[tauri::command]
pub fn get_journal(state: State<'_, TweakState>) -> Vec<Entry> {
    state.journal.lock().unwrap().newest_first()
}

#[tauri::command(async)]
pub fn create_restore_point(state: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let desc = "AdminOps: punto manual";
    let mut e = entry(Op::RestorePoint, None, desc);
    let result = restore::create(desc);
    match &result {
        Ok(()) => *state.last_restore_point.lock().unwrap() = Some(Instant::now()),
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
