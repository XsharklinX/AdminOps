//! Cuentas: qué cuentas hay conectadas en el equipo y en la sesión (cuenta de
//! Microsoft, cuenta profesional o educativa, Entra ID / Azure AD, dominio,
//! Office, credenciales guardadas) y cómo desconectarlas sin dar veinte vueltas
//! por Configuración.
//!
//! El caso típico: al instalar Windows se entra con una cuenta profesional y el
//! equipo queda unido a Entra ID, cuando debía usarse con una cuenta local.
//! Solución: crear un administrador local, sacar el equipo de Entra ID
//! (`dsregcmd /leave`) y entrar con la cuenta local.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceJoin {
    pub azure_ad_joined: bool,
    pub domain_joined: bool,
    pub workplace_joined: bool,
    pub enterprise_joined: bool,
    pub tenant_name: String,
    pub domain_name: String,
    pub device_id: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkAccount {
    /// Huella del certificado: identifica la cuenta para desconectarla.
    pub id: String,
    pub email: String,
    pub tenant: String,
    /// device: el equipo está unido (Entra ID); user: cuenta añadida por el usuario.
    pub scope: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct OfficeAccount {
    pub id: String,
    pub email: String,
    pub name: String,
    /// Profesional o educativa | Personal (Microsoft)
    pub kind: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub target: String,
    pub user: String,
    /// Windows | Genérica | Certificado…
    pub kind: String,
    #[serde(skip)]
    raw_type: u32,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountsStatus {
    /// Usuario con la sesión abierta (EQUIPO\usuario, AzureAD\usuario, DOMINIO\usuario).
    pub session_user: String,
    /// local | microsoft | azuread | domain
    pub session_kind: String,
    pub device: DeviceJoin,
    pub work_accounts: Vec<WorkAccount>,
    /// Cuentas de Microsoft personales usadas en este usuario (Store, Outlook, OneDrive…).
    pub microsoft_accounts: Vec<String>,
    pub office_accounts: Vec<OfficeAccount>,
    pub credentials: Vec<Credential>,
    /// Hay un administrador local activo (se podrá entrar si se saca el equipo de Entra ID o del dominio).
    pub has_local_admin: bool,
    /// Las cuentas del usuario se leen de otro usuario (AdminOps elevado con otra cuenta): no se pueden quitar desde aquí.
    pub other_user: bool,
}

const JOIN_SCRIPT: &str = r#"
$cs = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue
$ds = (dsregcmd.exe /status 2>$null) -join "`n"
function F($k) { if ($ds -match "(?m)^\s*$k\s*:\s*(.+)$") { $Matches[1].Trim() } else { '' } }
[pscustomobject]@{
  sessionUser = "$($cs.UserName)"
  azureAdJoined = (F 'AzureAdJoined') -eq 'YES'; domainJoined = [bool]$cs.PartOfDomain
  workplaceJoined = (F 'WorkplaceJoined') -eq 'YES'; enterpriseJoined = (F 'EnterpriseJoined') -eq 'YES'
  tenantName = F 'TenantName'; domainName = if ($cs.PartOfDomain) { "$($cs.Domain)" } else { F 'DomainName' }; deviceId = F 'DeviceId'
} | ConvertTo-Json -Compress
"#;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct JoinRaw {
    session_user: String,
    #[serde(flatten)]
    device: DeviceJoin,
}

/// Raíz del registro del usuario de la sesión (HKCU, o HKU\SID si AdminOps se elevó con otra cuenta).
fn user_hive() -> (RegKey, String) {
    match crate::target_user::hkcu_redirect() {
        Some(sid) => (RegKey::predef(HKEY_USERS), format!(r"{sid}\")),
        None => (RegKey::predef(HKEY_CURRENT_USER), String::new()),
    }
}

fn reg_str(k: &RegKey, name: &str) -> String {
    k.get_value::<String, _>(name).unwrap_or_default().trim().to_string()
}

fn work_accounts() -> Vec<WorkAccount> {
    let mut out = Vec::new();
    // Equipo unido a Entra ID (Azure AD).
    if let Ok(join) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SYSTEM\CurrentControlSet\Control\CloudDomainJoin\JoinInfo") {
        for id in join.enum_keys().flatten() {
            if let Ok(k) = join.open_subkey(&id) {
                out.push(WorkAccount { email: reg_str(&k, "UserEmail"), tenant: reg_str(&k, "TenantId"), id: id.clone(), scope: "device".into() });
            }
        }
    }
    // Cuentas profesionales o educativas añadidas por el usuario («Acceso al trabajo o la escuela»).
    let (hive, prefix) = user_hive();
    if let Ok(join) = hive.open_subkey(format!(r"{prefix}SOFTWARE\Microsoft\Windows NT\CurrentVersion\WorkplaceJoin\JoinInfo")) {
        for id in join.enum_keys().flatten() {
            if let Ok(k) = join.open_subkey(&id) {
                out.push(WorkAccount { email: reg_str(&k, "UserEmail"), tenant: reg_str(&k, "TenantId"), id: id.clone(), scope: "user".into() });
            }
        }
    }
    out
}

fn microsoft_accounts() -> Vec<String> {
    let (hive, prefix) = user_hive();
    hive.open_subkey(format!(r"{prefix}SOFTWARE\Microsoft\IdentityCRL\UserExtendedProperties"))
        .map(|k| k.enum_keys().flatten().filter(|e| e.contains('@')).collect())
        .unwrap_or_default()
}

fn office_accounts() -> Vec<OfficeAccount> {
    let (hive, prefix) = user_hive();
    let Ok(ids) = hive.open_subkey(format!(r"{prefix}SOFTWARE\Microsoft\Office\16.0\Common\Identity\Identities")) else { return vec![] };
    ids.enum_keys()
        .flatten()
        .filter_map(|id| {
            let k = ids.open_subkey(&id).ok()?;
            let email = reg_str(&k, "EmailAddress");
            if email.is_empty() {
                return None;
            }
            let provider = reg_str(&k, "ProviderId");
            Some(OfficeAccount {
                id,
                email,
                name: reg_str(&k, "FriendlyName"),
                kind: if provider.eq_ignore_ascii_case("LiveId") { "Personal (Microsoft)".into() } else { "Profesional o educativa".into() },
            })
        })
        .collect()
}

#[cfg(windows)]
fn credentials() -> Vec<Credential> {
    use windows_sys::Win32::Security::Credentials::{CredEnumerateW, CredFree, CREDENTIALW};
    let mut count = 0u32;
    let mut list: *mut *mut CREDENTIALW = std::ptr::null_mut();
    if unsafe { CredEnumerateW(std::ptr::null(), 0, &mut count, &mut list) } == 0 {
        return vec![];
    }
    let read = |p: *const u16| -> String {
        if p.is_null() {
            return String::new();
        }
        let len = (0..).take_while(|&i| unsafe { *p.add(i) } != 0).count();
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
    };
    let mut out = Vec::new();
    for i in 0..count as usize {
        let c = unsafe { &**list.add(i) };
        let kind = match c.Type {
            1 => "Genérica",
            2 => "Windows",
            3 => "Certificado",
            4 => "Windows (visible)",
            _ => "Otra",
        };
        out.push(Credential { target: read(c.TargetName), user: read(c.UserName), kind: kind.into(), raw_type: c.Type });
    }
    unsafe { CredFree(list.cast()) };
    out.sort_by_key(|c| c.target.to_lowercase());
    out
}

#[cfg(not(windows))]
fn credentials() -> Vec<Credential> {
    vec![]
}

fn session_kind(user: &str, device: &DeviceJoin, microsoft_linked: bool) -> &'static str {
    let upper = user.to_uppercase();
    if upper.starts_with("AZUREAD\\") {
        "azuread"
    } else if microsoft_linked {
        "microsoft"
    } else if device.domain_joined && !device.domain_name.is_empty() && upper.starts_with(&format!("{}\\", device.domain_name.split('.').next().unwrap_or("").to_uppercase())) {
        "domain"
    } else {
        "local"
    }
}

#[tauri::command(async)]
pub fn accounts_status() -> Result<AccountsStatus, String> {
    let out = crate::pspool::query(JOIN_SCRIPT, Some(Duration::from_secs(40)), "Cuentas: estado del equipo")?;
    let raw: JoinRaw = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let users = crate::users::list().unwrap_or_default();
    let short = raw.session_user.rsplit('\\').next().unwrap_or("").to_lowercase();
    let linked = users.iter().any(|u| u.name().to_lowercase() == short && u.is_microsoft());
    Ok(AccountsStatus {
        session_kind: session_kind(&raw.session_user, &raw.device, linked).into(),
        session_user: raw.session_user,
        device: raw.device,
        work_accounts: work_accounts(),
        microsoft_accounts: microsoft_accounts(),
        office_accounts: office_accounts(),
        credentials: credentials(),
        has_local_admin: users.iter().any(|u| u.is_active_local_admin() && !u.is_microsoft()),
        other_user: crate::target_user::hkcu_redirect().is_some(),
    })
}

// ---------- Desconectar ----------

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

fn valid_thumbprint(id: &str) -> bool {
    id.len() == 40 && id.chars().all(|c| c.is_ascii_hexdigit())
}

/// Saca el equipo de Entra ID (Azure AD). Solo si hay un administrador local
/// con el que entrar después: si no, el equipo quedaría sin forma de iniciar sesión.
#[tauri::command(async)]
pub fn leave_azure_ad(tweaks: State<'_, TweakState>) -> Result<String, String> {
    need_admin()?;
    let users = crate::users::list()?;
    if !users.iter().any(|u| u.is_active_local_admin() && !u.is_microsoft()) {
        return Err("Antes crea un administrador local (con contraseña): sin él no se podría entrar al equipo después de sacarlo de Entra ID.".into());
    }
    let r = crate::ps::exec_opts("dsregcmd.exe", &["/leave"], crate::ps::Opts { timeout: Some(Duration::from_secs(120)), task: None })
        .map(|_| "Equipo sacado de Entra ID (Azure AD). Cierra la sesión y entra con la cuenta local.".to_string());
    tweaks.record(Op::Run, "Cuentas: sacar el equipo de Entra ID (dsregcmd /leave)", &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r
}

/// Desconecta una cuenta profesional o educativa añadida por el usuario
/// («Acceso al trabajo o la escuela»): su certificado y su registro.
#[tauri::command(async)]
pub fn remove_work_account(tweaks: State<'_, TweakState>, id: String) -> Result<String, String> {
    if !valid_thumbprint(&id) {
        return Err("Cuenta no válida.".into());
    }
    if crate::target_user::hkcu_redirect().is_some() {
        return Err("AdminOps se abrió con otra cuenta de Windows: para quitar la cuenta profesional del usuario, abre AdminOps con su propia sesión (clic derecho → Ejecutar como administrador).".into());
    }
    let account = work_accounts().into_iter().find(|a| a.id == id && a.scope == "user").ok_or("Esa cuenta ya no está conectada.")?;
    let script = format!(
        "{}$ErrorActionPreference = 'Continue'\n\
         Remove-Item -Path \"Cert:\\CurrentUser\\My\\$id\" -Force -ErrorAction SilentlyContinue\n\
         Remove-Item -Path \"HKCU:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\WorkplaceJoin\\JoinInfo\\$id\" -Recurse -Force -ErrorAction Stop\n\
         'ok'",
        crate::ps::text_var("id", &id)
    );
    let r = crate::ps::powershell(&script).map(|_| format!("Cuenta {} desconectada. Cierra la sesión para que Windows deje de usarla.", account.email));
    tweaks.record(Op::Run, &format!("Cuentas: desconectar la cuenta profesional {}", account.email), &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r
}

/// Cierra la sesión de una cuenta en Office (Outlook, Word, Teams clásico…).
#[tauri::command(async)]
pub fn office_sign_out(tweaks: State<'_, TweakState>, id: String) -> Result<String, String> {
    if id.is_empty() || id.len() > 80 || !id.chars().all(|c| c.is_ascii_alphanumeric() || "_-{}".contains(c)) {
        return Err("Cuenta no válida.".into());
    }
    let account = office_accounts().into_iter().find(|a| a.id == id).ok_or("Esa cuenta ya no está en Office.")?;
    let (hive, prefix) = user_hive();
    let r = hive
        .open_subkey_with_flags(format!(r"{prefix}SOFTWARE\Microsoft\Office\16.0\Common\Identity\Identities"), KEY_READ | KEY_WRITE)
        .and_then(|k| k.delete_subkey_all(&id))
        .map(|_| format!("{} ya no está en Office. Cierra y vuelve a abrir Outlook, Word…", account.email))
        .map_err(|e| format!("No se pudo quitar: {e}"));
    tweaks.record(Op::Run, &format!("Cuentas: cerrar la sesión de Office de {}", account.email), &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r
}

/// Borra una credencial guardada de Windows (contraseñas de carpetas de red, Outlook, Escritorio remoto…).
#[tauri::command(async)]
pub fn delete_credential(tweaks: State<'_, TweakState>, target: String) -> Result<(), String> {
    let c = credentials().into_iter().find(|c| c.target == target).ok_or("Esa credencial ya no existe.")?;
    let r = delete_cred(&c);
    tweaks.record(Op::Run, &format!("Cuentas: borrar la credencial guardada «{}»", c.target), &r);
    r
}

#[cfg(windows)]
fn delete_cred(c: &Credential) -> Result<(), String> {
    use windows_sys::Win32::Security::Credentials::CredDeleteW;
    let wide: Vec<u16> = c.target.encode_utf16().chain(Some(0)).collect();
    if unsafe { CredDeleteW(wide.as_ptr(), c.raw_type, 0) } == 0 {
        Err("Windows no dejó borrarla (puede que la use el sistema).".into())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn delete_cred(_: &Credential) -> Result<(), String> {
    Err("Solo disponible en Windows.".into())
}

/// Cierra la sesión de Windows (para entrar con otra cuenta).
#[tauri::command]
pub fn sign_out_windows(tweaks: State<'_, TweakState>) -> Result<(), String> {
    let r = crate::ps::exec("shutdown.exe", &["/l"]).map(|_| ());
    tweaks.record(Op::Run, "Cuentas: cerrar la sesión de Windows", &r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(JOIN_SCRIPT);
        assert!(e.is_empty(), "{e}");
    }

    #[test]
    fn session_kinds() {
        let d = DeviceJoin { domain_joined: true, domain_name: "pgr.gob.do".into(), ..Default::default() };
        assert_eq!(session_kind(r"AzureAD\JuanPerez", &DeviceJoin::default(), false), "azuread");
        assert_eq!(session_kind(r"PC-01\juan", &DeviceJoin::default(), true), "microsoft");
        assert_eq!(session_kind(r"PGR\juan", &d, false), "domain");
        assert_eq!(session_kind(r"PC-01\admin", &d, false), "local");
        assert!(valid_thumbprint("0123456789ABCDEF0123456789abcdef01234567"));
        assert!(!valid_thumbprint("x; rm"));
    }

    /// Equipo real (solo lectura): `cargo test accounts_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn accounts_real() {
        let s = accounts_status().unwrap();
        println!("sesión: {} ({}) · dispositivo {:?}", s.session_kind, s.session_user.rsplit('\\').next().unwrap_or(""), s.device.azure_ad_joined);
        println!("trabajo: {} · Microsoft: {} · Office: {} · credenciales: {} · admin local: {}", s.work_accounts.len(), s.microsoft_accounts.len(), s.office_accounts.len(), s.credentials.len(), s.has_local_admin);
    }
}
