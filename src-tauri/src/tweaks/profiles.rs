//! Perfiles de un clic: aplican varios ajustes del catálogo con un solo punto
//! de restauración. Cada ajuste se registra por separado en el diario.

use super::engine::{self, Status};
use super::model::{Kind, Risk, Tweak};
use super::{apply_logged, ensure_restore_point, revert_logged, run_logged, TweakState};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::State;

#[derive(Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub items: Vec<String>,
}

#[derive(Deserialize)]
struct ProfileFile {
    profile: Vec<Profile>,
}

pub fn all() -> &'static [Profile] {
    static P: OnceLock<Vec<Profile>> = OnceLock::new();
    P.get_or_init(|| {
        toml::from_str::<ProfileFile>(include_str!("../../tweaks/profiles.toml"))
            .expect("profiles.toml inválido")
            .profile
    })
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
pub fn list_profiles(state: State<'_, TweakState>) -> Vec<ProfileView> {
    // Detectar cada ajuste una sola vez aunque aparezca en varios perfiles.
    let mut ids: Vec<&str> = all().iter().flat_map(|p| p.items.iter().map(String::as_str)).collect();
    ids.sort_unstable();
    ids.dedup();
    let tweaks: Vec<&Tweak> = ids.iter().filter_map(|id| state.find(id).ok()).collect();
    let statuses: Vec<(String, Status)> = std::thread::scope(|s| {
        let handles: Vec<_> = tweaks.iter().map(|t| (t.id.clone(), s.spawn(|| engine::detect(t)))).collect();
        handles.into_iter().map(|(id, h)| (id, h.join().unwrap_or(Status::Unknown))).collect()
    });
    let status_of = |id: &str| statuses.iter().find(|(i, _)| i == id).map_or(Status::Unknown, |(_, s)| *s);

    all()
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

fn find_profile(id: &str) -> Result<&'static Profile, String> {
    all().iter().find(|p| p.id == id).ok_or_else(|| format!("Perfil desconocido: {id}"))
}

#[tauri::command(async)]
pub fn apply_profile(id: String, skip_restore_point: bool, state: State<'_, TweakState>) -> Result<ProfileResult, String> {
    let p = find_profile(&id)?;
    if !crate::elevation::is_elevated() {
        return Err("Los perfiles requieren ejecutar AdminOps como administrador.".into());
    }
    let tweaks = tweaks_of(&state, p);
    let pending: Vec<&&Tweak> = tweaks
        .iter()
        .filter(|t| t.kind == Kind::Action || (t.supported_on(state.build) && engine::detect(t) != Status::Applied))
        .collect();
    let restore_point_created = !skip_restore_point
        && pending.iter().any(|t| t.kind == Kind::Toggle)
        && ensure_restore_point(&state, &format!("perfil {}", p.name), None)?;

    let mut reboot = false;
    let results = tweaks
        .iter()
        .map(|t| {
            let (outcome, message) = if !t.supported_on(state.build) {
                ("skipped", Some("No compatible con esta versión de Windows".into()))
            } else if !pending.iter().any(|x| x.id == t.id) {
                ("skipped", Some("Ya estaba aplicado".into()))
            } else if t.kind == Kind::Action {
                match run_logged(&state, t) {
                    Ok(m) => ("ran", Some(m)),
                    Err(e) => ("failed", Some(e)),
                }
            } else {
                match apply_logged(&state, t) {
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
pub fn revert_profile(id: String, state: State<'_, TweakState>) -> Result<ProfileResult, String> {
    let p = find_profile(&id)?;
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let mut reboot = false;
    let results = tweaks_of(&state, p)
        .into_iter()
        .filter(|t| t.kind == Kind::Toggle)
        .map(|t| {
            let (outcome, message) = match revert_logged(&state, t) {
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
    fn profiles_reference_existing_tweaks() {
        let catalog = super::super::catalog::load();
        for p in super::all() {
            assert!(!p.items.is_empty(), "{} vacío", p.id);
            for id in &p.items {
                assert!(catalog.iter().any(|t| &t.id == id), "{}: ajuste desconocido {id}", p.id);
            }
        }
    }
}
