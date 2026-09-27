//! Usuarios locales: listar, crear, cambiar contraseña, activar/desactivar,
//! administrador o estándar y eliminar (con o sin su perfil).
//!
//! Los usuarios se identifican por SID (los nombres de cuenta y de grupo
//! cambian con el idioma: "Administradores", "Administrators"…).
//! Las contraseñas nunca se registran: viajan en base64 dentro del script y el
//! registro de actividad solo guarda una descripción de la operación.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

const ADMINS: &str = "S-1-5-32-544";
const USERS: &str = "S-1-5-32-545";
/// Caracteres que Windows no admite en un nombre de usuario.
const FORBIDDEN: &[char] = &['"', '/', '\\', '[', ']', ':', ';', '|', '=', ',', '+', '*', '?', '<', '>', '@'];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUser {
    name: String,
    full_name: Option<String>,
    description: Option<String>,
    sid: String,
    enabled: bool,
    admin: bool,
    source: Option<String>,
    last_logon: Option<String>,
    password_last_set: Option<String>,
    password_expires: Option<String>,
    profile_loaded: bool,
    has_profile: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LocalUser {
    name: String,
    full_name: String,
    description: String,
    sid: String,
    enabled: bool,
    admin: bool,
    /// Cuenta de Microsoft vinculada: la contraseña es la de la cuenta online.
    microsoft: bool,
    /// ISO 8601
    last_logon: Option<String>,
    password_last_set: Option<String>,
    password_expires: Option<String>,
    has_profile: bool,
    /// Tiene la sesión iniciada ahora mismo.
    signed_in: bool,
    /// administrator | guest | default | wdag (cuentas integradas de Windows).
    builtin: Option<&'static str>,
    /// Es el usuario con la sesión abierta (el cliente).
    is_target: bool,
    /// Es la cuenta que ejecuta AdminOps.
    is_self: bool,
}

impl LocalUser {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// administrator | guest | default | wdag
    pub fn builtin_kind(&self) -> Option<&'static str> {
        self.builtin
    }

    /// Administrador local activo con el que se podría entrar sin dominio.
    pub fn is_active_local_admin(&self) -> bool {
        self.admin && self.enabled && !matches!(self.builtin, Some("default" | "wdag" | "guest"))
    }
}

fn builtin_kind(sid: &str) -> Option<&'static str> {
    match sid.rsplit('-').next()? {
        "500" => Some("administrator"),
        "501" => Some("guest"),
        "503" => Some("default"),
        "504" => Some("wdag"),
        _ => None,
    }
}

/// Solo SIDs de cuentas locales; evita que un valor manipulado llegue al script.
fn check_sid(sid: &str) -> Result<(), String> {
    let ok = sid.strip_prefix("S-1-5-21-").is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '-'));
    if ok { Ok(()) } else { Err("Identificador de usuario no válido.".into()) }
}

use crate::ps::text_var as ps_text;

fn run(script: &str, detail: &str, timeout_secs: u64) -> Result<String, String> {
    // Directo al pool con una descripción propia: el registro nunca ve el script.
    crate::pspool::query(script, Some(Duration::from_secs(timeout_secs)), detail)
}

const LIST_SCRIPT: &str = r#"
$admins = @{}
try {
  $group = (New-Object Security.Principal.SecurityIdentifier 'S-1-5-32-544').Translate([Security.Principal.NTAccount]).Value.Split('\')[-1]
  $g = [ADSI]"WinNT://$env:COMPUTERNAME/$group,group"
  foreach ($m in @($g.psbase.Invoke('Members'))) {
    $bytes = $m.GetType().InvokeMember('objectSid', 'GetProperty', $null, $m, $null)
    $admins[(New-Object Security.Principal.SecurityIdentifier($bytes, 0)).Value] = $true
  }
} catch {
  # ADSI falla si el grupo tiene miembros huérfanos: plan B.
  try { Get-LocalGroupMember -SID 'S-1-5-32-544' | ForEach-Object { $admins[$_.SID.Value] = $true } } catch {}
}
$profiles = @{}
Get-CimInstance Win32_UserProfile -ErrorAction SilentlyContinue | ForEach-Object { $profiles[$_.SID] = $_ }
$iso = { param($d) if ($d) { ([datetime]$d).ToString('o') } else { $null } }
$out = foreach ($u in Get-LocalUser) {
  $sid = $u.SID.Value
  $p = $profiles[$sid]
  [pscustomobject]@{
    name = $u.Name; fullName = $u.FullName; description = $u.Description; sid = $sid
    enabled = [bool]$u.Enabled; admin = [bool]$admins[$sid]
    source = [string]$u.PrincipalSource
    lastLogon = & $iso $u.LastLogon
    passwordLastSet = & $iso $u.PasswordLastSet
    passwordExpires = & $iso $u.PasswordExpires
    hasProfile = [bool]$p; profileLoaded = [bool]($p -and $p.Loaded)
  }
}
ConvertTo-Json -InputObject @($out) -Compress
"#;

pub fn list() -> Result<Vec<LocalUser>, String> {
    let out = run(LIST_SCRIPT, "usuarios locales: listar", 60)?;
    let raw: Vec<RawUser> = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada al leer los usuarios: {e}"))?;
    let target = crate::target_user::get().map(|t| t.sid.as_str());
    let own = crate::target_user::own_sid();
    let mut users: Vec<LocalUser> = raw
        .into_iter()
        .map(|r| LocalUser {
            builtin: builtin_kind(&r.sid),
            is_target: target == Some(r.sid.as_str()),
            is_self: own == Some(r.sid.as_str()),
            microsoft: r.source.as_deref() == Some("MicrosoftAccount"),
            name: r.name,
            full_name: r.full_name.unwrap_or_default(),
            description: r.description.unwrap_or_default(),
            sid: r.sid,
            enabled: r.enabled,
            admin: r.admin,
            last_logon: r.last_logon,
            password_last_set: r.password_last_set,
            password_expires: r.password_expires,
            has_profile: r.has_profile,
            signed_in: r.profile_loaded,
        })
        .collect();
    // Personas primero; las cuentas integradas al final.
    users.sort_by_key(|u| (u.builtin.is_some(), !u.enabled, u.name.to_lowercase()));
    Ok(users)
}

// ---------- Reglas de seguridad ----------

fn find<'a>(users: &'a [LocalUser], sid: &str) -> Result<&'a LocalUser, String> {
    users.iter().find(|u| u.sid == sid).ok_or_else(|| "Ese usuario ya no existe. Actualiza la lista.".into())
}

/// ¿Quedaría algún administrador activo si `sid` deja de serlo?
fn other_active_admins(users: &[LocalUser], sid: &str) -> usize {
    users.iter().filter(|u| u.sid != sid && u.admin && u.enabled).count()
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    Delete,
    Disable,
    Enable,
    Demote,
    Promote,
    Password,
}

/// Comprueba si la acción se puede hacer sin dejar el equipo inaccesible.
fn guard(users: &[LocalUser], sid: &str, action: Action) -> Result<(), String> {
    let u = find(users, sid)?;
    let who = &u.name;
    if matches!(u.builtin, Some("default" | "wdag")) {
        return Err(format!("«{who}» es una cuenta interna de Windows y no se debe modificar."));
    }
    let removes_admin = u.admin && u.enabled && matches!(action, Action::Delete | Action::Disable | Action::Demote);
    if removes_admin && other_active_admins(users, sid) == 0 {
        return Err(format!("«{who}» es el único administrador activo: el equipo se quedaría sin administrador."));
    }
    match action {
        Action::Delete | Action::Disable | Action::Demote if u.is_self => {
            Err(format!("«{who}» es la cuenta que está usando AdminOps ahora mismo."))
        }
        Action::Delete | Action::Disable if u.is_target || u.signed_in => {
            Err(format!("«{who}» tiene la sesión iniciada. Cierra su sesión antes."))
        }
        Action::Delete if u.builtin.is_some() => Err(format!("«{who}» es una cuenta integrada de Windows: se puede desactivar, no eliminar.")),
        Action::Promote | Action::Demote if u.builtin == Some("guest") => Err("La cuenta Invitado no puede ser administradora.".into()),
        Action::Password if u.microsoft => Err(format!(
            "«{who}» es una cuenta de Microsoft: su contraseña se cambia en account.microsoft.com."
        )),
        _ => Ok(()),
    }
}

pub fn validate_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Escribe un nombre de usuario.".into());
    }
    if n.chars().count() > 20 {
        return Err("El nombre de usuario admite como máximo 20 caracteres.".into());
    }
    if let Some(c) = n.chars().find(|c| FORBIDDEN.contains(c) || c.is_control()) {
        return Err(format!("El nombre no puede contener «{c}»."));
    }
    if n.chars().all(|c| c == '.' || c == ' ') {
        return Err("El nombre no puede ser solo puntos o espacios.".into());
    }
    if n.ends_with('.') {
        return Err("El nombre no puede terminar en punto.".into());
    }
    if sysinfo::System::host_name().is_some_and(|h| h.eq_ignore_ascii_case(n)) {
        return Err("El nombre de usuario no puede ser igual al nombre del equipo.".into());
    }
    Ok(n.to_string())
}

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() { Ok(()) } else { Err("Requiere ejecutar AdminOps como administrador.".into()) }
}

// ---------- Comandos ----------

#[tauri::command(async)]
pub fn list_users() -> Result<Vec<LocalUser>, String> {
    list()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    name: String,
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    password: String,
    admin: bool,
    #[serde(default)]
    password_never_expires: bool,
    #[serde(default)]
    must_change: bool,
}

#[tauri::command(async)]
pub fn create_user(user: NewUser, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    let name = validate_name(&user.name)?;
    if list()?.iter().any(|u| u.name.eq_ignore_ascii_case(&name)) {
        return Err(format!("Ya existe un usuario llamado «{name}»."));
    }
    let full = user.full_name.trim().chars().take(64).collect::<String>();
    let script = format!(
        "{}{}{}$admin = ${}\n$never = ${}\n$mustChange = ${}\n{}",
        ps_text("n", &name),
        ps_text("full", &full),
        crate::ps::secret_var("pw", &user.password),
        user.admin,
        user.password_never_expires,
        user.must_change,
        r#"
$params = @{ Name = $n; FullName = $full; AccountNeverExpires = $true }
if ($pw) {
  $params.Password = $pw
  if ($never -and -not $mustChange) { $params.PasswordNeverExpires = $true }
} else {
  $params.NoPassword = $true
}
$u = New-LocalUser @params
try {
  Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $u
  if ($admin) { Add-LocalGroupMember -SID 'S-1-5-32-544' -Member $u }
  if ($mustChange -and $pw) {
    $a = [ADSI]"WinNT://$env:COMPUTERNAME/$n,user"
    $a.PasswordExpired = 1
    $a.SetInfo()
  }
} catch {
  # Sin grupos no podría iniciar sesión: deshacer la creación.
  Remove-LocalUser -SID $u.SID -ErrorAction SilentlyContinue
  throw
}
'ok'
"#
    );
    let kind = if user.admin { "administrador" } else { "estándar" };
    let result = run(&script, &format!("usuarios locales: crear «{name}»"), 60).map(|_| ());
    tweaks.record(Op::Run, &format!("Usuarios: crear «{name}» ({kind})"), &result);
    result.map_err(friendly)
}

#[tauri::command(async)]
pub fn set_user_password(sid: String, password: String, must_change: bool, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    check_sid(&sid)?;
    let users = list()?;
    guard(&users, &sid, Action::Password)?;
    let name = find(&users, &sid)?.name.clone();
    let script = format!(
        "$sid = '{sid}'\n{}$mustChange = ${must_change}\n{}",
        crate::ps::secret_var("pw", &password),
        r#"
$u = Get-LocalUser -SID $sid
if ($pw) {
  Set-LocalUser -SID $sid -Password $pw
} else {
  $a = [ADSI]"WinNT://$env:COMPUTERNAME/$($u.Name),user"
  $a.SetPassword('')
}
if ($mustChange -and $pw) {
  $a = [ADSI]"WinNT://$env:COMPUTERNAME/$($u.Name),user"
  $a.PasswordExpired = 1
  $a.SetInfo()
}
'ok'
"#
    );
    let what = if password.is_empty() { "quitar la contraseña" } else { "cambiar la contraseña" };
    let result = run(&script, &format!("usuarios locales: {what} de «{name}»"), 60).map(|_| ());
    tweaks.record(Op::Run, &format!("Usuarios: {what} de «{name}»"), &result);
    result.map_err(friendly)
}

#[tauri::command(async)]
pub fn set_user_enabled(sid: String, enabled: bool, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    check_sid(&sid)?;
    let users = list()?;
    guard(&users, &sid, if enabled { Action::Enable } else { Action::Disable })?;
    let name = find(&users, &sid)?.name.clone();
    let cmd = if enabled { "Enable-LocalUser" } else { "Disable-LocalUser" };
    let result = run(&format!("{cmd} -SID '{sid}'\n'ok'"), &format!("usuarios locales: {cmd} «{name}»"), 60).map(|_| ());
    let what = if enabled { "activar" } else { "desactivar" };
    tweaks.record(Op::Run, &format!("Usuarios: {what} «{name}»"), &result);
    result.map_err(friendly)
}

#[tauri::command(async)]
pub fn set_user_admin(sid: String, admin: bool, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    check_sid(&sid)?;
    let users = list()?;
    guard(&users, &sid, if admin { Action::Promote } else { Action::Demote })?;
    let name = find(&users, &sid)?.name.clone();
    // Al quitar administrador se asegura que siga en Usuarios para poder iniciar sesión.
    let script = if admin {
        format!("Add-LocalGroupMember -SID '{ADMINS}' -Member (Get-LocalUser -SID '{sid}')\n'ok'")
    } else {
        format!(
            "$u = Get-LocalUser -SID '{sid}'\n\
             try {{ Add-LocalGroupMember -SID '{USERS}' -Member $u -ErrorAction Stop }} catch [Microsoft.PowerShell.Commands.MemberExistsException] {{}}\n\
             Remove-LocalGroupMember -SID '{ADMINS}' -Member $u\n'ok'"
        )
    };
    let result = run(&script, &format!("usuarios locales: administrador={admin} «{name}»"), 60).map(|_| ());
    let what = if admin { "hacer administrador a" } else { "quitar administrador a" };
    tweaks.record(Op::Run, &format!("Usuarios: {what} «{name}»"), &result);
    result.map_err(friendly)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    exists: bool,
    size: u64,
    files: u64,
}

/// Tamaño de la carpeta de perfil (para avisar antes de borrarla).
#[tauri::command(async)]
pub fn user_profile_size(sid: String) -> Result<ProfileInfo, String> {
    check_sid(&sid)?;
    let path = crate::tweaks::registry::read_string(
        &format!(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}"),
        "ProfileImagePath",
    );
    let Some(path) = path else { return Ok(ProfileInfo { exists: false, size: 0, files: 0 }) };
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let dir = std::path::PathBuf::from(path.replace("%SystemDrive%", &drive));
    if !dir.is_dir() {
        return Ok(ProfileInfo { exists: false, size: 0, files: 0 });
    }
    let (size, files) = crate::space::folder_size(&dir);
    Ok(ProfileInfo { exists: true, size, files })
}

#[tauri::command(async)]
pub fn delete_user(sid: String, delete_profile: bool, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    check_sid(&sid)?;
    let users = list()?;
    guard(&users, &sid, Action::Delete)?;
    let name = find(&users, &sid)?.name.clone();
    let script = format!(
        "$sid = '{sid}'\n$deleteProfile = ${delete_profile}\n{}",
        r#"
$p = Get-CimInstance Win32_UserProfile -Filter "SID='$sid'" -ErrorAction SilentlyContinue
if ($deleteProfile -and $p) {
  if ($p.Loaded) { throw 'El perfil está en uso: cierra la sesión de ese usuario o reinicia el equipo.' }
  $p | Remove-CimInstance
}
Remove-LocalUser -SID $sid
'ok'
"#
    );
    // Borrar un perfil grande puede tardar varios minutos.
    let result = run(&script, &format!("usuarios locales: eliminar «{name}»"), 900).map(|_| ());
    let extra = if delete_profile { " y su perfil" } else { "" };
    tweaks.record(Op::Run, &format!("Usuarios: eliminar «{name}»{extra}"), &result);
    result.map_err(friendly)
}

/// Mensajes de Windows habituales, en claro.
fn friendly(e: String) -> String {
    let lower = e.to_lowercase();
    if lower.contains("password policy") || lower.contains("directiva de contrase") || lower.contains("complejidad") {
        "La contraseña no cumple la directiva del equipo (longitud mínima o complejidad).".into()
    } else if lower.contains("already exists") || lower.contains("ya existe") {
        "Ya existe un usuario o grupo con ese nombre.".into()
    } else if lower.contains("access") && lower.contains("denied") || lower.contains("acceso denegado") {
        "Acceso denegado: ejecuta AdminOps como administrador.".into()
    } else {
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(name: &str, sid: &str, admin: bool, enabled: bool) -> LocalUser {
        LocalUser {
            name: name.into(),
            full_name: String::new(),
            description: String::new(),
            sid: sid.into(),
            enabled,
            admin,
            microsoft: false,
            last_logon: None,
            password_last_set: None,
            password_expires: None,
            has_profile: true,
            signed_in: false,
            builtin: builtin_kind(sid),
            is_target: false,
            is_self: false,
        }
    }

    const D: &str = "S-1-5-21-1-2-3";

    #[test]
    fn never_leaves_the_pc_without_admin() {
        let users = vec![
            user("Administrador", &format!("{D}-500"), true, false),
            user("cliente", &format!("{D}-1001"), true, true),
            user("hijo", &format!("{D}-1002"), false, true),
        ];
        let cliente = format!("{D}-1001");
        assert!(guard(&users, &cliente, Action::Demote).is_err());
        assert!(guard(&users, &cliente, Action::Disable).is_err());
        assert!(guard(&users, &cliente, Action::Delete).is_err());
        assert!(guard(&users, &format!("{D}-1002"), Action::Delete).is_ok());
        assert!(guard(&users, &format!("{D}-1002"), Action::Promote).is_ok());
        // Con otro administrador activo sí se puede.
        let mut more = users.clone();
        more.push(user("tecnico", &format!("{D}-1003"), true, true));
        assert!(guard(&more, &cliente, Action::Demote).is_ok());
    }

    #[test]
    fn protects_session_self_and_builtin() {
        let mut users = vec![
            user("tecnico", &format!("{D}-1003"), true, true),
            user("cliente", &format!("{D}-1001"), true, true),
            user("DefaultAccount", &format!("{D}-503"), false, false),
            user("Invitado", &format!("{D}-501"), false, false),
            user("Administrador", &format!("{D}-500"), true, false),
        ];
        users[0].is_self = true;
        users[1].is_target = true;
        assert!(guard(&users, &format!("{D}-1003"), Action::Delete).is_err());
        assert!(guard(&users, &format!("{D}-1003"), Action::Demote).is_err());
        assert!(guard(&users, &format!("{D}-1003"), Action::Password).is_ok());
        assert!(guard(&users, &format!("{D}-1001"), Action::Delete).is_err());
        assert!(guard(&users, &format!("{D}-1001"), Action::Demote).is_ok());
        assert!(guard(&users, &format!("{D}-503"), Action::Enable).is_err());
        assert!(guard(&users, &format!("{D}-501"), Action::Promote).is_err());
        assert!(guard(&users, &format!("{D}-501"), Action::Enable).is_ok());
        assert!(guard(&users, &format!("{D}-500"), Action::Enable).is_ok());
        assert!(guard(&users, &format!("{D}-500"), Action::Delete).is_err());
        users[1].microsoft = true;
        assert!(guard(&users, &format!("{D}-1001"), Action::Password).is_err());
    }

    #[test]
    fn validates_names_and_sids() {
        assert_eq!(validate_name("  Ana López ").unwrap(), "Ana López");
        assert!(validate_name("").is_err());
        assert!(validate_name("abcdefghijklmnopqrstu").is_err()); // 21
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("user@x").is_err());
        assert!(validate_name("...").is_err());
        assert!(validate_name("pepe.").is_err());
        assert!(check_sid("S-1-5-21-123-456-1001").is_ok());
        assert!(check_sid("S-1-5-32-544").is_err());
        assert!(check_sid("S-1-5-21-1'; Remove-Item").is_err());
    }

    #[test]
    fn passwords_travel_encoded() {
        let line = crate::ps::secret_var("pw", "O'Neil \"x\" $(calc) ñ");
        assert!(!line.contains("O'Neil") && !line.contains("calc"));
        let out = crate::pspool::query(
            &format!("{line}[Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($pw))"),
            None,
            "t",
        )
        .unwrap();
        assert_eq!(out, "O'Neil \"x\" $(calc) ñ");
        assert_eq!(crate::ps::secret_var("pw", ""), "$pw = $null\n");
        // Los textos normales van literales, sin interpretar nada.
        let t = ps_text("t", "O'Neil ‘x’ $(calc)");
        assert_eq!(crate::pspool::query(&format!("{t}$t"), None, "t").unwrap(), "O'Neil ‘x’ $(calc)");
    }

    /// Solo lectura: lista los usuarios reales de este equipo.
    #[test]
    fn lists_real_users() {
        let users = list().unwrap();
        assert!(!users.is_empty());
        assert!(users.iter().any(|u| u.builtin == Some("administrator")));
        assert!(users.iter().any(|u| u.admin), "{users:#?}");
    }

    #[test]
    fn embedded_scripts_parse() {
        let errors = crate::ps::parse_errors(LIST_SCRIPT);
        assert!(errors.is_empty(), "LIST_SCRIPT: {errors}");
    }
}
