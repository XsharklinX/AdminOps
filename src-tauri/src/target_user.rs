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

/// SID de la cuenta que ejecuta AdminOps (puede no ser el usuario destino).
pub fn own_sid() -> Option<&'static str> {
    static OWN: OnceLock<Option<String>> = OnceLock::new();
    OWN.get_or_init(|| {
        let mut sys = System::new();
        let pid = sysinfo::get_current_pid().ok()?;
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_user(UpdateKind::Always),
        );
        sys.process(pid)?.user_id().map(|u| u.to_string())
    })
    .as_deref()
}

/// Carpetas del usuario destino (las del perfil del cliente aunque AdminOps
/// se haya elevado con otra cuenta).
pub struct UserDirs {
    pub profile: std::path::PathBuf,
    pub app_data: std::path::PathBuf,
    pub local_app_data: std::path::PathBuf,
    pub temp: std::path::PathBuf,
}

pub fn user_dirs() -> Option<UserDirs> {
    use std::path::PathBuf;
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    match hkcu_redirect() {
        Some(sid) => {
            let raw = crate::tweaks::registry::read_string(
                &format!(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}"),
                "ProfileImagePath",
            )?;
            // Suele ser %SystemDrive%\Users\<nombre>.
            let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
            let profile = PathBuf::from(raw.replace("%SystemDrive%", &drive));
            let local = profile.join(r"AppData\Local");
            Some(UserDirs { app_data: profile.join(r"AppData\Roaming"), temp: local.join("Temp"), local_app_data: local, profile })
        }
        None => Some(UserDirs {
            profile: env("USERPROFILE")?,
            app_data: env("APPDATA")?,
            local_app_data: env("LOCALAPPDATA")?,
            temp: std::env::temp_dir(),
        }),
    }
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Variables que reciben todos los scripts del catálogo para actuar sobre el
/// usuario destino y no sobre la cuenta que elevó AdminOps:
/// `$UserSid`, `$UserHive` (ruta de registro), `$UserProfile`, `$UserAppData`,
/// `$UserLocalAppData` y `$UserTemp`.
pub fn script_prelude() -> String {
    let sid = get().map(|t| t.sid.as_str()).unwrap_or("");
    match hkcu_redirect() {
        Some(sid) => {
            let profile = crate::tweaks::registry::read_string(
                &format!(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}"),
                "ProfileImagePath",
            )
            .unwrap_or_default();
            format!(
                "$UserSid = {sid}\n$UserHive = {hive}\n\
                 $UserProfile = [Environment]::ExpandEnvironmentVariables({profile})\n\
                 $UserAppData = Join-Path $UserProfile 'AppData\\Roaming'\n\
                 $UserLocalAppData = Join-Path $UserProfile 'AppData\\Local'\n\
                 $UserTemp = Join-Path $UserLocalAppData 'Temp'\n",
                sid = ps_quote(sid),
                hive = ps_quote(&format!(r"Registry::HKEY_USERS\{sid}")),
                profile = ps_quote(&profile),
            )
        }
        None => format!(
            "$UserSid = {}\n$UserHive = 'HKCU:'\n$UserProfile = $env:USERPROFILE\n\
             $UserAppData = $env:APPDATA\n$UserLocalAppData = $env:LOCALAPPDATA\n$UserTemp = $env:TEMP\n",
            ps_quote(sid)
        ),
    }
}
