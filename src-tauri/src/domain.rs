//! Dominio de Active Directory: estado, comprobaciones previas, unir, salir,
//! reparar la relación de confianza y cambiar el nombre del equipo.
//!
//! Las credenciales viajan en base64 dentro del script (`ps::text_var`) y
//! nunca se guardan ni aparecen en el registro de actividad.

use crate::ps::text_var;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

/// Ediciones que no pueden unirse a un dominio (Windows Home).
const HOME_EDITIONS: &[&str] = &["Core", "CoreN", "CoreSingleLanguage", "CoreCountrySpecific"];
/// Kerberos rechaza el inicio de sesión con más de 5 minutos de diferencia.
const MAX_SKEW_SECS: f64 = 300.0;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomainStatus {
    computer_name: String,
    part_of_domain: bool,
    domain: Option<String>,
    workgroup: Option<String>,
    edition: String,
    caption: String,
    #[serde(default)]
    can_join: bool,
    azure_ad_joined: bool,
    tenant: Option<String>,
    /// Controlador de dominio encontrado.
    dc: Option<String>,
    /// `None` si no se pudo comprobar (requiere administrador).
    secure_channel: Option<bool>,
    /// Segundos de diferencia con el controlador (positivo = el equipo va atrasado).
    time_offset: Option<f64>,
    /// El usuario con la sesión abierta es de dominio (no una cuenta local).
    #[serde(default)]
    user_is_domain: Option<bool>,
}

const STATUS_SCRIPT: &str = r#"
$cs = Get-CimInstance Win32_ComputerSystem
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$ds = (dsregcmd.exe /status 2>$null) -join "`n"
function Val($k) { if ($ds -match "(?m)^\s*$k\s*:\s*(\S.*)$") { $Matches[1].Trim() } else { $null } }
$dc = $null; $channel = $null; $offset = $null
if ($cs.PartOfDomain) {
  try { $dc = ([System.DirectoryServices.ActiveDirectory.Domain]::GetComputerDomain()).FindDomainController().Name } catch {}
  try { $channel = [bool](Test-ComputerSecureChannel -ErrorAction Stop) } catch {}
  if ($dc) {
    $line = w32tm.exe /stripchart /computer:$dc /samples:1 /dataonly 2>$null | Select-String '([+-]\d+[.,]\d+)s' | Select-Object -Last 1
    if ($line) { $offset = [double]($line.Matches[0].Groups[1].Value -replace ',', '.') }
  }
}
[pscustomobject]@{
  computerName = $env:COMPUTERNAME; partOfDomain = [bool]$cs.PartOfDomain
  domain = if ($cs.PartOfDomain) { $cs.Domain } else { $null }
  workgroup = if (-not $cs.PartOfDomain) { $cs.Workgroup } else { $null }
  edition = "$($cv.EditionID)"; caption = "$((Get-CimInstance Win32_OperatingSystem).Caption)"
  azureAdJoined = (Val 'AzureAdJoined') -eq 'YES'; tenant = Val 'TenantName'
  dc = $dc; secureChannel = $channel; timeOffset = $offset
} | ConvertTo-Json -Compress
"#;

/// SID de la máquina: el de cualquier cuenta local sin el RID final.
fn machine_sid() -> Option<String> {
    let out = crate::ps::powershell("(Get-LocalUser | Select-Object -First 1).SID.Value").ok()?;
    let sid = out.trim();
    sid.rsplit_once('-').map(|(prefix, _)| prefix.to_string())
}

pub fn status() -> Result<DomainStatus, String> {
    let out = crate::ps::powershell_opts(STATUS_SCRIPT, crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None })?;
    let mut s: DomainStatus = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    s.can_join = !HOME_EDITIONS.contains(&s.edition.as_str());
    let target = crate::target_user::get().map(|t| t.sid.clone());
    s.user_is_domain = match (target, machine_sid()) {
        (Some(t), Some(m)) => Some(!t.starts_with(&format!("{m}-"))),
        _ => None,
    };
    Ok(s)
}

#[tauri::command(async)]
pub fn domain_status() -> Result<DomainStatus, String> {
    status()
}

// ---------- Validaciones ----------

/// Nombre DNS del dominio (pgr.gob.do).
fn check_domain(d: &str) -> Result<String, String> {
    let d = d.trim().trim_end_matches('.').to_ascii_lowercase();
    let label_ok = |l: &str| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if d.len() > 253 || !d.contains('.') || !d.split('.').all(label_ok) {
        return Err("Escribe el nombre DNS completo del dominio (por ejemplo: pgr.gob.do).".into());
    }
    Ok(d)
}

/// Nombre de equipo NetBIOS: 1-15 caracteres, letras, números y guiones, no solo números.
pub fn check_computer_name(n: &str) -> Result<String, String> {
    let n = n.trim().to_ascii_uppercase();
    if n.is_empty() || n.len() > 15 {
        return Err("El nombre del equipo debe tener entre 1 y 15 caracteres.".into());
    }
    if !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || n.starts_with('-') || n.ends_with('-') {
        return Err("El nombre del equipo solo admite letras, números y guiones (no al principio ni al final).".into());
    }
    if n.chars().all(|c| c.is_ascii_digit()) {
        return Err("El nombre del equipo no puede ser solo números.".into());
    }
    Ok(n)
}

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() { Ok(()) } else { Err("Requiere ejecutar AdminOps como administrador.".into()) }
}

/// Prepara `$cred` a partir del usuario y la contraseña. Un usuario sin
/// dominio ("jperez") se completa con el dominio indicado.
fn credential_script(domain: &str, user: &str, password: &str) -> Result<String, String> {
    let user = user.trim();
    if user.is_empty() || password.is_empty() {
        return Err("Escribe el usuario y la contraseña de una cuenta del dominio con permiso.".into());
    }
    let full = if user.contains('\\') || user.contains('@') { user.to_string() } else { format!("{domain}\\{user}") };
    Ok(format!(
        "{}{}$cred = New-Object System.Management.Automation.PSCredential($u, (ConvertTo-SecureString $p -AsPlainText -Force))\n",
        text_var("u", &full),
        text_var("p", password)
    ))
}

/// Mensajes habituales de Windows, en claro.
fn friendly(e: String) -> String {
    let l = e.to_lowercase();
    if l.contains("password") && (l.contains("incorrect") || l.contains("bad")) || l.contains("contraseña incorrecta") || l.contains("nombre de usuario desconocido") || l.contains("unknown user name") {
        "Usuario o contraseña incorrectos.".into()
    } else if l.contains("could not be contacted") || l.contains("no se pudo establecer contacto") || l.contains("does not exist") || l.contains("no existe") {
        "No se pudo contactar con el dominio. Revisa que el DNS del equipo sea el del dominio y que haya conexión con la red de la empresa.".into()
    } else if l.contains("access is denied") || l.contains("acceso denegado") {
        "Acceso denegado: esa cuenta no tiene permiso para unir equipos (o la cuenta del equipo ya existe y no puede reutilizarla).".into()
    } else if l.contains("already") && l.contains("domain") || l.contains("ya está") {
        "El equipo ya pertenece a ese dominio.".into()
    } else {
        e
    }
}

// ---------- Comprobaciones previas ----------

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    label: String,
    /// ok | warn | fail
    status: &'static str,
    detail: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Probe {
    dcs: Vec<String>,
    reachable: Option<String>,
    offset: Option<f64>,
    dns: Vec<String>,
}

#[tauri::command(async)]
pub fn domain_check(domain: String) -> Result<Vec<Check>, String> {
    let d = check_domain(&domain)?;
    let s = status()?;
    let mut checks = vec![Check {
        label: "Edición de Windows".into(),
        status: if s.can_join { "ok" } else { "fail" },
        detail: if s.can_join {
            s.caption.clone()
        } else {
            format!("{}: las ediciones Home no pueden unirse a un dominio. Hay que actualizar a Pro.", s.caption)
        },
    }];
    let script = format!(
        "{}$dns = @(Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object {{ $_.ServerAddresses }} | ForEach-Object ServerAddresses | Select-Object -Unique)\n{}",
        text_var("d", &d),
        r#"
$dcs = @(Resolve-DnsName -Name "_ldap._tcp.dc._msdcs.$d" -Type SRV -DnsOnly -QuickTimeout -ErrorAction SilentlyContinue | Where-Object { $_.Type -eq 'SRV' } | Sort-Object Priority | ForEach-Object NameTarget)
$reachable = $null
foreach ($dc in $dcs | Select-Object -First 3) {
  $c = New-Object System.Net.Sockets.TcpClient
  try { if ($c.ConnectAsync($dc, 389).Wait(2500) -and $c.Connected) { $reachable = $dc; break } } catch {} finally { $c.Dispose() }
}
$offset = $null
if ($reachable) {
  $line = w32tm.exe /stripchart /computer:$reachable /samples:1 /dataonly 2>$null | Select-String '([+-]\d+[.,]\d+)s' | Select-Object -Last 1
  if ($line) { $offset = [double]($line.Matches[0].Groups[1].Value -replace ',', '.') }
}
[pscustomobject]@{ dcs = $dcs; reachable = $reachable; offset = $offset; dns = $dns } | ConvertTo-Json -Compress
"#
    );
    let out = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None })?;
    let p: Probe = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let dns = if p.dns.is_empty() { "ninguno".to_string() } else { p.dns.join(", ") };
    checks.push(if p.dcs.is_empty() {
        Check {
            label: "DNS del dominio".into(),
            status: "fail",
            detail: format!(
                "Los DNS de este equipo ({dns}) no conocen «{d}». Pon como DNS el servidor del dominio (Herramientas de red → DNS) o conéctate a la red/VPN de la empresa."
            ),
        }
    } else {
        Check { label: "DNS del dominio".into(), status: "ok", detail: format!("Controladores encontrados: {}", p.dcs.join(", ")) }
    });
    if !p.dcs.is_empty() {
        checks.push(match &p.reachable {
            Some(dc) => Check { label: "Conexión con el controlador".into(), status: "ok", detail: format!("{dc} responde (LDAP, puerto 389)") },
            None => Check {
                label: "Conexión con el controlador".into(),
                status: "fail",
                detail: "Ningún controlador responde en el puerto 389: revisa la red, la VPN o el firewall.".into(),
            },
        });
    }
    if let Some(off) = p.offset {
        let bad = off.abs() > MAX_SKEW_SECS;
        checks.push(Check {
            label: "Hora del equipo".into(),
            status: if bad { "fail" } else if off.abs() > 60.0 { "warn" } else { "ok" },
            detail: if bad {
                format!("Diferencia de {:.0} s con el dominio: más de 5 minutos impide iniciar sesión. Corrige la fecha y hora.", off.abs())
            } else {
                format!("Diferencia de {:.1} s con el dominio", off.abs())
            },
        });
    }
    if s.part_of_domain {
        checks.push(Check {
            label: "Situación actual".into(),
            status: "warn",
            detail: format!("El equipo ya está en el dominio {}.", s.domain.clone().unwrap_or_default()),
        });
    }
    Ok(checks)
}

// ---------- Acciones ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    domain: String,
    user: String,
    password: String,
    #[serde(default)]
    ou: String,
    #[serde(default)]
    new_name: String,
}

#[tauri::command(async)]
pub fn domain_join(req: JoinRequest, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    let d = check_domain(&req.domain)?;
    let s = status()?;
    if !s.can_join {
        return Err("Windows Home no puede unirse a un dominio.".into());
    }
    if s.part_of_domain {
        return Err(format!("El equipo ya está en el dominio {}. Sácalo primero si quieres cambiarlo.", s.domain.unwrap_or_default()));
    }
    let new_name = if req.new_name.trim().is_empty() { String::new() } else { check_computer_name(&req.new_name)? };
    let ou = req.ou.trim();
    if ou.contains(['\n', '\r']) || (!ou.is_empty() && !ou.to_ascii_uppercase().contains("DC=")) {
        return Err("La unidad organizativa debe tener el formato OU=Equipos,DC=empresa,DC=com.".into());
    }
    let script = format!(
        "{}{}{}{}{}",
        credential_script(&d, &req.user, &req.password)?,
        text_var("d", &d),
        text_var("ou", ou),
        text_var("newName", &new_name),
        r#"
$params = @{ DomainName = $d; Credential = $cred; Force = $true; ErrorAction = 'Stop' }
if ($ou) { $params.OUPath = $ou }
if ($newName -and $newName -ne $env:COMPUTERNAME) { $params.NewName = $newName }
Add-Computer @params
'ok'
"#
    );
    let result = crate::pspool::query(&script, Some(Duration::from_secs(180)), &format!("dominio: unir a {d}")).map(|_| ()).map_err(friendly);
    let rename = if new_name.is_empty() { String::new() } else { format!(" como {new_name}") };
    tweaks.record(Op::Run, &format!("Dominio: unir el equipo a {d}{rename} (requiere reiniciar)"), &result);
    result
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaveRequest {
    user: String,
    password: String,
    workgroup: String,
}

/// Cuentas locales de administrador activas: sin alguna, al salir del dominio
/// nadie podría entrar como administrador.
fn local_admins() -> Vec<String> {
    crate::users::list()
        .unwrap_or_default()
        .into_iter()
        .filter(|u| u.is_active_local_admin())
        .map(|u| u.name().to_string())
        .collect()
}

#[tauri::command(async)]
pub fn domain_leave(req: LeaveRequest, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    let s = status()?;
    let Some(domain) = s.domain.filter(|_| s.part_of_domain) else {
        return Err("El equipo no está en ningún dominio.".into());
    };
    if local_admins().is_empty() {
        return Err(
            "No hay ningún administrador local activo: al salir del dominio nadie podría administrar el equipo. Crea uno en Usuarios locales antes.".into(),
        );
    }
    let wg = req.workgroup.trim().to_ascii_uppercase();
    let wg = if wg.is_empty() { "WORKGROUP".to_string() } else { wg };
    if wg.len() > 15 || !wg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("El grupo de trabajo solo admite letras, números y guiones (máximo 15).".into());
    }
    let script = format!(
        "{}{}Remove-Computer -UnjoinDomainCredential $cred -WorkgroupName $wg -Force -ErrorAction Stop\n'ok'",
        credential_script(&domain, &req.user, &req.password)?,
        text_var("wg", &wg)
    );
    let result = crate::pspool::query(&script, Some(Duration::from_secs(180)), &format!("dominio: salir de {domain}")).map(|_| ()).map_err(friendly);
    tweaks.record(Op::Run, &format!("Dominio: sacar el equipo de {domain} al grupo {wg} (requiere reiniciar)"), &result);
    result
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    user: String,
    password: String,
}

/// "La relación de confianza entre esta estación de trabajo y el dominio principal ha fallado."
#[tauri::command(async)]
pub fn domain_repair(req: Credentials, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    let s = status()?;
    let Some(domain) = s.domain.filter(|_| s.part_of_domain) else {
        return Err("El equipo no está en ningún dominio.".into());
    };
    let script = format!(
        "{}if (-not (Test-ComputerSecureChannel -Repair -Credential $cred -ErrorAction Stop)) {{ throw 'Windows no pudo reparar la relación de confianza.' }}\n'ok'",
        credential_script(&domain, &req.user, &req.password)?
    );
    let result = crate::pspool::query(&script, Some(Duration::from_secs(120)), "dominio: reparar la relación de confianza").map(|_| ()).map_err(friendly);
    tweaks.record(Op::Run, &format!("Dominio: reparar la relación de confianza con {domain}"), &result);
    result
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    new_name: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    password: String,
}

#[tauri::command(async)]
pub fn rename_computer(req: RenameRequest, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    let name = check_computer_name(&req.new_name)?;
    let s = status()?;
    if name.eq_ignore_ascii_case(&s.computer_name) {
        return Err("El equipo ya se llama así.".into());
    }
    // En un dominio, el cambio de nombre también renombra la cuenta del equipo: pide credenciales.
    let (cred, param) = match s.domain.filter(|_| s.part_of_domain) {
        Some(d) => (credential_script(&d, &req.user, &req.password)?, " -DomainCredential $cred"),
        None => (String::new(), ""),
    };
    let script = format!("{cred}{}Rename-Computer -NewName $n{param} -Force -ErrorAction Stop\n'ok'", text_var("n", &name));
    let result = crate::pspool::query(&script, Some(Duration::from_secs(120)), &format!("equipo: renombrar a {name}")).map(|_| ()).map_err(friendly);
    tweaks.record(Op::Run, &format!("Cambiar el nombre del equipo: {} → {name} (requiere reiniciar)", s.computer_name), &result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names() {
        assert_eq!(check_domain(" PGR.gob.do. ").unwrap(), "pgr.gob.do");
        assert!(check_domain("PGR").is_err());
        assert!(check_domain("pgr..do").is_err());
        assert!(check_domain("pgr.gob.do; calc").is_err());
        assert_eq!(check_computer_name("pc-soporte-01").unwrap(), "PC-SOPORTE-01");
        assert!(check_computer_name("1234").is_err());
        assert!(check_computer_name("nombre-demasiado-largo").is_err());
        assert!(check_computer_name("-pc").is_err());
        assert!(check_computer_name("pc_01").is_err());
    }

    #[test]
    fn credentials_never_appear_in_clear() {
        let s = credential_script("pgr.gob.do", "jperez", "S3cr3t'$(calc)").unwrap();
        assert!(!s.contains("S3cr3t") && !s.contains("jperez"));
        assert!(credential_script("pgr.gob.do", "", "x").is_err());
        let out = crate::pspool::query(&format!("{s}$cred.UserName"), None, "t").unwrap();
        assert_eq!(out, r"pgr.gob.do\jperez");
        let s = credential_script("pgr.gob.do", "jperez@pgr.gob.do", "x").unwrap();
        assert_eq!(crate::pspool::query(&format!("{s}$cred.UserName"), None, "t").unwrap(), "jperez@pgr.gob.do");
    }

    /// Solo lectura: estado real de este equipo.
    #[test]
    fn real_status() {
        let s = status().unwrap();
        println!("{s:#?}");
        assert!(!s.computer_name.is_empty());
    }

    #[test]
    fn embedded_scripts_parse() {
        let errors = crate::ps::parse_errors(STATUS_SCRIPT);
        assert!(errors.is_empty(), "STATUS_SCRIPT: {errors}");
    }
}
