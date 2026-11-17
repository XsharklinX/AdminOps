//! ¿Es el perfil o el equipo? Un usuario LOCAL temporal (nunca del dominio) para
//! entrar con él y probar: si con él funciona, el problema está en el perfil de
//! la persona; si no, en el equipo.
//!
//! - Contraseña de un solo uso, fácil de dictar (la misma que Personas).
//! - Se apunta en el diario; la contraseña no.
//! - Se borra, con su carpeta de perfil, al pulsar «Borrar el de prueba» o, si se
//!   olvida, a las 24 h con una tarea programada que se borra sola. Además la
//!   cuenta caduca sola pasadas 25 h.
//! - La cuenta de prueba se reconoce por su descripción, no por un nombre que
//!   venga de fuera: así nunca se borra otra cuenta.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use tauri::State;

/// La descripción (máximo 48 caracteres en Windows) lleva la hora de caducidad en segundos.
const MARK: &str = "AdminOps prueba hasta ";
const TASK_PATH: &str = "\\AdminOps\\";
const TASK_NAME: &str = "Borrar usuario de prueba";
const LIFETIME_SECS: u64 = 24 * 3600;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestUser {
    pub name: String,
    /// Segundos Unix en los que se borra solo.
    pub expires: u64,
    pub has_profile: bool,
    pub signed_in: bool,
    /// SID de la cuenta: solo para comprobar que es la que creó AdminOps; no viaja a la interfaz.
    #[serde(skip)]
    pub sid: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreatedTestUser {
    pub name: String,
    /// Solo se enseña ahora: no se guarda en ningún sitio.
    pub password: String,
    pub expires: u64,
}

/// `prueba-1116` (mes y día) y, si ya existiera, `prueba-1116-2`.
pub fn pick_name(month: u32, day: u32, taken: &[String]) -> String {
    let base = format!("prueba-{month:02}{day:02}");
    if !taken.iter().any(|t| t.eq_ignore_ascii_case(&base)) {
        return base;
    }
    (2..10).map(|n| format!("{base}-{n}")).find(|c| !taken.iter().any(|t| t.eq_ignore_ascii_case(c))).unwrap_or(format!("{base}-x"))
}

/// Solo los nombres que fabrica este módulo: se pegan en un script, así que no pueden ser otra cosa.
pub fn valid_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("prueba-") else { return false };
    let (date, suffix) = rest.split_once('-').map_or((rest, ""), |(d, s)| (d, s));
    date.len() == 4 && date.chars().all(|c| c.is_ascii_digit()) && (suffix.is_empty() || (suffix.len() == 1 && suffix.chars().all(|c| c.is_ascii_alphanumeric())))
}

pub fn description(expires: u64) -> String {
    format!("{MARK}{expires}")
}

/// La hora de caducidad que lleva una descripción, si es la de una cuenta de prueba.
pub fn parse_description(d: &str) -> Option<u64> {
    d.strip_prefix(MARK)?.trim().parse().ok()
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

/// La orden de una línea que ejecuta la tarea programada: borra la cuenta de prueba
/// (solo si lo es) con su perfil y después se borra a sí misma. Sin comillas dobles.
fn cleanup_command(name: &str) -> String {
    format!(
        "$n='{name}'; $u=Get-LocalUser -Name $n -ErrorAction SilentlyContinue; if ($u -and $u.Description -like '{MARK}*') {{ $p=Get-CimInstance Win32_UserProfile | Where-Object {{ $_.SID -eq $u.SID.Value }}; if ($p) {{ if ($p.Loaded) {{ exit 1 }}; Remove-CimInstance -InputObject $p }}; Remove-LocalUser -SID $u.SID }}; Unregister-ScheduledTask -TaskPath '{TASK_PATH}' -TaskName '{TASK_NAME}' -Confirm:$false"
    )
}

/// La cuenta de prueba que haya ahora en este equipo.
fn current() -> Result<Option<TestUser>, String> {
    let script = format!(
        "$m = '{MARK}'\n{}",
        r#"
$u = Get-LocalUser | Where-Object { $_.Description -like ($m + '*') } | Select-Object -First 1
if (-not $u) { 'null'; return }
$p = Get-CimInstance Win32_UserProfile | Where-Object { $_.SID -eq $u.SID.Value } | Select-Object -First 1
[pscustomobject]@{ name = $u.Name; description = $u.Description; sid = $u.SID.Value; hasProfile = [bool]$p; signedIn = [bool]($p -and $p.Loaded) } | ConvertTo-Json -Compress
"#
    );
    let out = crate::ps::powershell(&script)?;
    let t = out.trim();
    if t.is_empty() || t == "null" {
        return Ok(None);
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        name: String,
        description: String,
        #[serde(default)]
        sid: String,
        has_profile: bool,
        signed_in: bool,
    }
    let r: Raw = serde_json::from_str(t).map_err(|e| format!("Respuesta inesperada de Windows: {e}"))?;
    let expires = parse_description(&r.description).unwrap_or(0);
    Ok(Some(TestUser { name: r.name, expires, has_profile: r.has_profile, signed_in: r.signed_in, sid: r.sid }))
}

/// El usuario de prueba de ahora, si hay uno. Solo mira.
#[tauri::command(async)]
pub fn test_user_status() -> Result<Option<TestUser>, String> {
    current()
}

/// Crea el usuario de prueba y la tarea que lo borra en 24 h.
#[tauri::command(async)]
pub fn create_test_user(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Result<CreatedTestUser, String> {
    need_admin()?;
    if let Some(t) = current()? {
        return Err(format!("Ya hay un usuario de prueba («{}»): bórralo antes de crear otro.", t.name));
    }
    let taken: Vec<String> = crate::users::list()?.into_iter().map(|u| u.name().to_string()).collect();
    let local = chrono::Local::now();
    use chrono::Datelike;
    let name = pick_name(local.month(), local.day(), &taken);
    if !valid_name(&name) {
        return Err("No se pudo elegir un nombre para el usuario de prueba.".into());
    }
    let password = crate::people::temp_password();
    let expires = now() + LIFETIME_SECS;
    let script = format!(
        "{}{}{}$expires = [int64]{expires}\n$cmd = {}\n{}",
        crate::ps::text_var("n", &name),
        crate::ps::secret_var("pw", &password),
        crate::ps::text_var("desc", &description(expires)),
        crate::ps::ps_literal(&cleanup_command(&name)),
        r#"
$when = [DateTimeOffset]::FromUnixTimeSeconds($expires).LocalDateTime
$u = New-LocalUser -Name $n -Password $pw -FullName 'Usuario de prueba' -Description $desc -AccountExpires $when.AddHours(1) -PasswordNeverExpires
try {
  Add-LocalGroupMember -SID 'S-1-5-32-545' -Member $u
  $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command "' + $cmd + '"')
  # Cada hora hasta que pueda borrarla (si la sesión sigue abierta, la orden sale sin quitar la tarea).
  $trigger = New-ScheduledTaskTrigger -Once -At $when -RepetitionInterval (New-TimeSpan -Hours 1) -RepetitionDuration (New-TimeSpan -Days 30)
  $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
  $settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Hours 1)
  Register-ScheduledTask -TaskPath '\AdminOps\' -TaskName 'Borrar usuario de prueba' -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description 'Borra el usuario de prueba de AdminOps' -Force | Out-Null
} catch {
  # Sin la tarea que lo borra no se deja la cuenta: se deshace la creación.
  Remove-LocalUser -SID $u.SID -ErrorAction SilentlyContinue
  throw
}
$u.SID.Value
"#
    );
    // Directo al pool con una descripción propia: el registro nunca ve el script (lleva la contraseña).
    let result = crate::pspool::query(&script, Some(std::time::Duration::from_secs(90)), "usuario de prueba: crear").map_err(|e| friendly(&e));
    if let Ok(sid) = &result {
        save_sid(&app, sid.trim());
    }
    let result = result.map(|_| ());
    tweaks.record(Op::Run, &format!("Usuario de prueba «{name}» creado (se borra solo en 24 h)"), &result);
    result.map(|_| CreatedTestUser { name, password, expires })
}

/// Borra el usuario de prueba con su carpeta de perfil y quita la tarea.
#[tauri::command(async)]
pub fn delete_test_user(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    let Some(user) = current()? else {
        // Sin cuenta, solo puede quedar la tarea: se quita.
        let _ = crate::ps::powershell(&format!("Unregister-ScheduledTask -TaskPath '{TASK_PATH}' -TaskName '{TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue\n'ok'"));
        return Ok(());
    };
    if !valid_name(&user.name) || !same_sid(saved_sid(&app).as_deref(), &user.sid) {
        return Err("Esa cuenta no la creó AdminOps desde este equipo: bórrala desde Usuarios.".into());
    }
    if user.signed_in {
        return Err(format!("«{}» tiene la sesión abierta: ciérrala (o reinicia) y vuelve a pulsar.", user.name));
    }
    let script = format!("{}\n'ok'", cleanup_command(&user.name).replace("exit 1", "throw 'El perfil está en uso: cierra la sesión de ese usuario.'"));
    let result = crate::ps::powershell(&script).map(|_| ()).map_err(|e| friendly(&e));
    tweaks.record(Op::Run, &format!("Usuario de prueba «{}» borrado con su perfil", user.name), &result);
    result
}

fn sid_file(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("usuario-de-prueba.json")
}

fn save_sid(app: &tauri::AppHandle, sid: &str) {
    if sid.starts_with("S-1-") {
        let _ = crate::paths::write_json(&sid_file(app), &serde_json::json!({ "sid": sid }));
    }
}

fn saved_sid(app: &tauri::AppHandle) -> Option<String> {
    let v: serde_json::Value = crate::paths::read_json(&sid_file(app));
    v.get("sid").and_then(|s| s.as_str()).map(str::to_string)
}

/// ¿Es la cuenta que creó AdminOps? Sin SID guardado o distinto, no se toca.
pub fn same_sid(saved: Option<&str>, current: &str) -> bool {
    matches!(saved, Some(s) if !s.is_empty() && s.eq_ignore_ascii_case(current.trim()))
}

fn friendly(e: &str) -> String {
    let l = e.to_lowercase();
    if l.contains("password") && (l.contains("policy") || l.contains("complex")) || l.contains("directiva de contrase") {
        "La directiva de contraseñas del equipo rechazó la contraseña de un solo uso.".into()
    } else if l.contains("access") && l.contains("denied") || l.contains("acceso denegado") {
        "Acceso denegado: ejecuta AdminOps como administrador.".into()
    } else {
        crate::escalate::scrub(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_borra_la_cuenta_con_el_sid_guardado() {
        assert!(same_sid(Some("S-1-5-21-1-2-3-1005"), "S-1-5-21-1-2-3-1005"));
        assert!(!same_sid(Some("S-1-5-21-1-2-3-1005"), "S-1-5-21-1-2-3-1006"));
        assert!(!same_sid(None, "S-1-5-21-1-2-3-1005"));
        assert!(!same_sid(Some(""), ""));
    }

    #[test]
    fn elige_un_nombre_libre() {
        assert_eq!(pick_name(11, 6, &[]), "prueba-1106");
        assert_eq!(pick_name(11, 6, &["Prueba-1106".into()]), "prueba-1106-2");
        assert_eq!(pick_name(11, 6, &["prueba-1106".into(), "prueba-1106-2".into()]), "prueba-1106-3");
    }

    #[test]
    fn solo_nombres_propios() {
        assert!(valid_name("prueba-1116") && valid_name("prueba-1116-2"));
        assert!(!valid_name("administrador") && !valid_name("prueba-11'; calc") && !valid_name("prueba-111") && !valid_name("prueba-1116-22"));
    }

    #[test]
    fn la_descripcion_cabe_y_se_lee() {
        let d = description(1_763_289_000);
        assert!(d.chars().count() <= 48, "Windows admite 48 caracteres: {}", d.chars().count());
        assert_eq!(parse_description(&d), Some(1_763_289_000));
        assert_eq!(parse_description("Cuenta de Ana"), None);
    }

    #[test]
    fn la_orden_de_la_tarea_no_lleva_comillas_dobles_y_borra_solo_pruebas() {
        let c = cleanup_command("prueba-1116");
        assert!(!c.contains('"'));
        assert!(c.contains("AdminOps prueba hasta *") && c.contains("Unregister-ScheduledTask"));
    }

    #[test]
    fn los_scripts_son_validos() {
        // El parser de PowerShell de verdad (solo lee: no ejecuta nada del script).
        let c = cleanup_command("prueba-1116");
        assert_eq!(crate::ps::parse_errors(&c), "");
    }
}
