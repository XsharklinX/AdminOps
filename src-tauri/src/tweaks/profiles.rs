//! Perfiles de un clic: aplican varios ajustes del catálogo con un solo punto
//! de restauración. Cada ajuste se registra por separado en el diario.

use super::engine::{self, Status};
use super::model::{Kind, Risk, Tweak};
use super::{apply_logged, ensure_restore_point, revert_logged, run_logged, TweakState};
use crate::task::Task;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::State;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub items: Vec<String>,
    /// Creado por el técnico (editable y exportable).
    #[serde(default)]
    pub custom: bool,
}

#[derive(Deserialize)]
struct ProfileFile {
    profile: Vec<Profile>,
}

fn builtin() -> &'static [Profile] {
    static P: OnceLock<Vec<Profile>> = OnceLock::new();
    P.get_or_init(|| {
        toml::from_str::<ProfileFile>(include_str!("../../tweaks/profiles.toml"))
            .expect("profiles.toml inválido")
            .profile
    })
}

fn custom_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("profiles.json")
}

fn load_custom(app: &tauri::AppHandle) -> Vec<Profile> {
    let mut v: Vec<Profile> = crate::paths::read_json(&custom_path(app));
    v.iter_mut().for_each(|p| p.custom = true);
    v
}

/// Perfiles de fábrica + los del técnico.
pub fn all(app: &tauri::AppHandle) -> Vec<Profile> {
    builtin().iter().cloned().chain(load_custom(app)).collect()
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    s.split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-")
}

/// Valida un perfil propio contra el catálogo y le asigna id si no tiene.
fn validate(state: &TweakState, mut p: Profile, existing: &[Profile]) -> Result<Profile, String> {
    p.name = p.name.trim().to_string();
    if p.name.is_empty() {
        return Err("El perfil necesita un nombre.".into());
    }
    p.items.dedup();
    if p.items.is_empty() {
        return Err("Añade al menos un ajuste al perfil.".into());
    }
    if let Some(bad) = p.items.iter().find(|id| state.find(id).is_err()) {
        return Err(format!("El ajuste «{bad}» no existe en esta versión de AdminOps."));
    }
    if p.icon.trim().is_empty() {
        p.icon = "layers".into();
    }
    p.custom = true;
    let taken = |id: &str| builtin().iter().any(|b| b.id == id) || existing.iter().any(|e| e.id == id);
    if p.id.is_empty() || !p.id.starts_with("custom-") {
        let base = format!("custom-{}", slug(&p.name));
        let mut id = base.clone();
        let mut n = 2;
        while taken(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        p.id = id;
    }
    Ok(p)
}

#[tauri::command]
pub fn save_custom_profile(app: tauri::AppHandle, profile: Profile, state: State<'_, TweakState>) -> Result<Profile, String> {
    let mut list = load_custom(&app);
    let is_update = !profile.id.is_empty() && list.iter().any(|p| p.id == profile.id);
    let others: Vec<Profile> = list.iter().filter(|p| p.id != profile.id).cloned().collect();
    let p = validate(&state, profile, if is_update { &others } else { &list })?;
    match list.iter_mut().find(|x| x.id == p.id) {
        Some(x) => *x = p.clone(),
        None => list.push(p.clone()),
    }
    crate::paths::write_json(&custom_path(&app), &list)?;
    Ok(p)
}

#[tauri::command]
pub fn delete_custom_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut list = load_custom(&app);
    list.retain(|p| p.id != id);
    crate::paths::write_json(&custom_path(&app), &list)
}

/// JSON de los perfiles propios (todos o los indicados) para compartirlos.
#[tauri::command]
pub fn export_custom_profiles(app: tauri::AppHandle, ids: Option<Vec<String>>) -> Result<String, String> {
    let list: Vec<Profile> =
        load_custom(&app).into_iter().filter(|p| ids.as_ref().is_none_or(|ids| ids.contains(&p.id))).collect();
    serde_json::to_string_pretty(&list).map_err(|e| e.to_string())
}

/// Importa perfiles (un objeto o una lista). Devuelve cuántos se añadieron.
#[tauri::command]
pub fn import_custom_profiles(app: tauri::AppHandle, json: String, state: State<'_, TweakState>) -> Result<usize, String> {
    let incoming: Vec<Profile> = serde_json::from_str::<Vec<Profile>>(&json)
        .or_else(|_| serde_json::from_str::<Profile>(&json).map(|p| vec![p]))
        .map_err(|e| format!("El texto no es un perfil de AdminOps válido: {e}"))?;
    let mut list = load_custom(&app);
    let mut added = 0;
    for mut p in incoming {
        p.id.clear(); // siempre id nuevo: no pisar perfiles existentes
        let p = validate(&state, p, &list)?;
        list.push(p);
        added += 1;
    }
    crate::paths::write_json(&custom_path(&app), &list)?;
    Ok(added)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileItem {
    id: String,
    name: String,
    kind: Kind,
    risk: Risk,
    status: Status,
    supported: bool,
    has_backup: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    custom: bool,
    id: String,
    name: String,
    icon: String,
    description: String,
    items: Vec<ProfileItem>,
    needs_admin: bool,
}

fn tweaks_of<'a>(state: &'a TweakState, p: &Profile) -> Vec<&'a Tweak> {
    p.items.iter().filter_map(|id| state.find(id).ok()).collect()
}

#[tauri::command(async)]
pub fn list_profiles(app: tauri::AppHandle, state: State<'_, TweakState>) -> Vec<ProfileView> {
    let profiles = all(&app);
    // Detectar cada ajuste una sola vez aunque aparezca en varios perfiles.
    let mut ids: Vec<&str> = profiles.iter().flat_map(|p| p.items.iter().map(String::as_str)).collect();
    ids.sort_unstable();
    ids.dedup();
    let tweaks: Vec<&Tweak> = ids.iter().filter_map(|id| state.find(id).ok()).collect();
    let statuses: Vec<(String, Status)> = std::thread::scope(|s| {
        let handles: Vec<_> = tweaks.iter().map(|t| (t.id.clone(), s.spawn(|| engine::detect(t)))).collect();
        handles.into_iter().map(|(id, h)| (id, h.join().unwrap_or(Status::Unknown))).collect()
    });
    let status_of = |id: &str| statuses.iter().find(|(i, _)| i == id).map_or(Status::Unknown, |(_, s)| *s);

    profiles
        .iter()
        .map(|p| {
            let items: Vec<ProfileItem> = tweaks_of(&state, p)
                .into_iter()
                .map(|t| ProfileItem {
                    id: t.id.clone(),
                    name: t.name.clone(),
                    kind: t.kind,
                    risk: t.risk,
                    status: status_of(&t.id),
                    supported: t.supported_on(state.build),
                    has_backup: state.journal.lock().unwrap().pending_apply(&t.id).is_some(),
                })
                .collect();
            ProfileView {
                custom: p.custom,
                id: p.id.clone(),
                name: p.name.clone(),
                icon: p.icon.clone(),
                description: p.description.clone(),
                needs_admin: tweaks_of(&state, p).iter().any(|t| t.needs_admin()),
                items,
            }
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    id: String,
    name: String,
    /// applied | reverted | ran | skipped | failed
    outcome: &'static str,
    message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResult {
    results: Vec<ItemResult>,
    restore_point_created: bool,
    reboot: bool,
}

fn find_profile(app: &tauri::AppHandle, id: &str) -> Result<Profile, String> {
    all(app).into_iter().find(|p| p.id == id).ok_or_else(|| format!("Perfil desconocido: {id}"))
}

#[tauri::command(async)]
pub fn apply_profile(
    app: tauri::AppHandle,
    id: String,
    skip_restore_point: bool,
    state: State<'_, TweakState>,
) -> Result<ProfileResult, String> {
    let p = find_profile(&app, &id)?;
    let task = Task::new(&app, format!("profile:{id}"));
    if !crate::elevation::is_elevated() {
        return Err("Los perfiles requieren ejecutar AdminOps como administrador.".into());
    }
    let tweaks = tweaks_of(&state, &p);
    let pending: Vec<&&Tweak> = tweaks
        .iter()
        .filter(|t| t.kind == Kind::Action || (t.supported_on(state.build) && engine::detect(t) != Status::Applied))
        .collect();
    let restore_point_created = !skip_restore_point
        && pending.iter().any(|t| t.kind == Kind::Toggle)
        && ensure_restore_point(&state, &task, &format!("perfil {}", p.name), None)?;

    let mut reboot = false;
    let total = pending.len();
    let mut n = 0;
    let results = tweaks
        .iter()
        .map(|t| {
            let (outcome, message) = if task.cancelled() {
                ("skipped", Some(crate::ps::CANCELLED_MSG.into()))
            } else if !t.supported_on(state.build) {
                ("skipped", Some("No compatible con esta versión de Windows".into()))
            } else if !pending.iter().any(|x| x.id == t.id) {
                ("skipped", Some("Ya estaba aplicado".into()))
            } else if t.kind == Kind::Action {
                n += 1;
                task.step(format!("{n}/{total} · {}…", t.name));
                match run_logged(&state, t, Some(&task)) {
                    Ok(m) => ("ran", Some(m)),
                    Err(e) => ("failed", Some(e)),
                }
            } else {
                n += 1;
                task.step(format!("{n}/{total} · {}…", t.name));
                match apply_logged(&state, t, Some(&task)) {
                    Ok(()) => {
                        reboot |= t.reboot;
                        ("applied", None)
                    }
                    Err(e) => ("failed", Some(e)),
                }
            };
            ItemResult { id: t.id.clone(), name: t.name.clone(), outcome, message }
        })
        .collect();
    Ok(ProfileResult { results, restore_point_created, reboot })
}

/// Deshace los ajustes del perfil que AdminOps aplicó (los que ya venían
/// aplicados de antes no se tocan).
#[tauri::command(async)]
pub fn revert_profile(app: tauri::AppHandle, id: String, state: State<'_, TweakState>) -> Result<ProfileResult, String> {
    let p = find_profile(&app, &id)?;
    let task = Task::new(&app, format!("profile:{id}"));
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let mut reboot = false;
    let results = tweaks_of(&state, &p)
        .into_iter()
        .filter(|t| t.kind == Kind::Toggle)
        .map(|t| {
            task.step(format!("Deshaciendo {}…", t.name));
            let (outcome, message) = match revert_logged(&state, t, Some(&task)) {
                Ok(true) => {
                    reboot |= t.reboot;
                    ("reverted", None)
                }
                Ok(false) => ("skipped", Some("No lo aplicó AdminOps".into())),
                Err(e) => ("failed", Some(e)),
            };
            ItemResult { id: t.id.clone(), name: t.name.clone(), outcome, message }
        })
        .collect();
    Ok(ProfileResult { results, restore_point_created: false, reboot })
}

#[cfg(test)]
mod tests {
    #[test]
    fn slugs() {
        assert_eq!(super::slug("Técnico Rápido!! ñu"), "tecnico-rapido-nu");
    }

    #[test]
    fn profiles_reference_existing_tweaks() {
        let catalog = super::super::catalog::load();
        for p in super::builtin() {
            assert!(!p.items.is_empty(), "{} vacío", p.id);
            for id in &p.items {
                assert!(catalog.iter().any(|t| &t.id == id), "{}: ajuste desconocido {id}", p.id);
            }
        }
    }
}
