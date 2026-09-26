//! Usuario al que se aplican los ajustes de HKCU.
//!
//! Si el técnico eleva AdminOps con **otra** cuenta de administrador, el HKCU
//! del proceso es el de esa cuenta, no el del cliente. Detectamos al dueño de
//! `explorer.exe` (el usuario con la sesión abierta) y, si difiere, los accesos
//! a HKCU se redirigen a `HKEY_USERS\<SID>` de ese usuario.

use serde::Serialize;
use std::sync::OnceLock;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TargetUser {
    pub name: String,
    pub sid: String,
    /// El usuario destino no es el que ejecuta AdminOps: HKCU se redirige.
    pub redirected: bool,
}

static TARGET: OnceLock<Option<TargetUser>> = OnceLock::new();

pub fn get() -> Option<&'static TargetUser> {
    TARGET.get_or_init(detect).as_ref()
}

/// SID al que redirigir HKCU, o `None` si es el propio usuario del proceso.
pub fn hkcu_redirect() -> Option<&'static str> {
    get().filter(|t| t.redirected).map(|t| t.sid.as_str())
}

fn detect() -> Option<TargetUser> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_user(UpdateKind::Always),
    );
    let me = sysinfo::get_current_pid().ok().and_then(|pid| sys.process(pid)).and_then(|p| p.user_id()).cloned();

    let explorers: Vec<_> = sys
        .processes()
        .values()
        .filter(|p| p.name().eq_ignore_ascii_case("explorer.exe"))
        .filter_map(|p| p.user_id().cloned())
        .collect();

    // Si alguno de los explorer es nuestro, no hay nada que redirigir.
    let target = match &me {
        Some(me) if explorers.contains(me) => me.clone(),
        _ => explorers.into_iter().next().or(me.clone())?,
    };

    let users = Users::new_with_refreshed_list();
    let name = users.get_user_by_id(&target).map(|u| u.name().to_string()).unwrap_or_default();
    Some(TargetUser {
        sid: target.to_string(),
        name,
        redirected: me.as_ref() != Some(&target),
    })
}

#[tauri::command]
pub fn get_target_user() -> Option<TargetUser> {
    get().cloned()
}
