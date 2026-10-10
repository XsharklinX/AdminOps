//! Seguridad a fondo: lo que usan los atacantes y casi nadie mira. Exclusiones
//! de Defender sospechosas (carpetas enteras, Temp), SMBv1, Escritorio remoto
//! abierto en redes públicas, administradores locales que sobran, contraseñas
//! que nunca caducan, macros de Office permitidas, contraseñas guardadas en
//! claro (inicio de sesión automático, WDigest), UAC apagado y la cuenta de
//! invitado. Cada punto con su riesgo explicado y su arreglo.

use super::parse;
use crate::troubleshoot::{finding, fix, fix_confirm, Finding};
use crate::tweaks::model::{RegData, RegKind};
use crate::tweaks::registry;
use crate::tweaks::TweakState;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct SecRaw {
    pub exclusion_paths: Vec<String>,
    pub exclusion_ext: Vec<String>,
    pub exclusion_proc: Vec<String>,
    /// null: no se pudieron leer (falta administrador).
    pub exclusions_read: bool,
    pub smb1: Option<bool>,
    pub rdp_enabled: bool,
    pub rdp_public: bool,
    pub admins: Vec<String>,
    pub never_expire: Vec<String>,
    pub guest_enabled: bool,
    pub uac: Option<u32>,
    pub autologon_password: bool,
    pub wdigest: Option<u32>,
    /// app → valor de VBAWarnings (1: todas las macros permitidas).
    pub macros: Vec<(String, u32)>,
}

const SCRIPT: &str = r#"
$mp = $null; try { $mp = Get-MpPreference -ErrorAction Stop } catch { }
$smb = $null; try { $smb = [bool](Get-SmbServerConfiguration -ErrorAction Stop).EnableSMB1Protocol } catch { }
$ts = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' -ErrorAction SilentlyContinue
$rdpOn = ($ts -and $ts.fDenyTSConnections -eq 0)
$rdpPublic = $false
if ($rdpOn) { $rdpPublic = [bool](Get-NetFirewallRule -DisplayGroup '*Remote Desktop*', '*Escritorio remoto*' -Enabled True -Direction Inbound -ErrorAction SilentlyContinue | Where-Object { "$($_.Profile)" -match 'Public|Any' }) }
$admins = @(); try { $admins = @(Get-LocalGroupMember -SID 'S-1-5-32-544' -ErrorAction Stop | ForEach-Object { "$($_.Name)" }) } catch { }
$users = @(Get-LocalUser -ErrorAction SilentlyContinue)
$never = @($users | Where-Object { $_.Enabled -and $_.PasswordExpires -eq $null -and $_.PasswordRequired } | ForEach-Object { "$($_.Name)" })
$guest = [bool]($users | Where-Object { $_.SID -like '*-501' -and $_.Enabled })
$sys = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' -ErrorAction SilentlyContinue
$wl = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' -ErrorAction SilentlyContinue
$wd = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\SecurityProviders\WDigest' -ErrorAction SilentlyContinue
$macros = @(foreach ($app in 'Word', 'Excel', 'PowerPoint') {
  foreach ($base in @("$UserHive\Software\Policies\Microsoft\Office\16.0\$app\Security", "$UserHive\Software\Microsoft\Office\16.0\$app\Security")) {
    $v = (Get-ItemProperty $base -ErrorAction SilentlyContinue).VBAWarnings
    if ($null -ne $v) { ,@($app, [int]$v); break }
  }
})
[pscustomobject]@{
  exclusionPaths = @($mp.ExclusionPath | Where-Object { $_ }); exclusionExt = @($mp.ExclusionExtension | Where-Object { $_ }); exclusionProc = @($mp.ExclusionProcess | Where-Object { $_ }); exclusionsRead = [bool]$mp
  smb1 = $smb; rdpEnabled = $rdpOn; rdpPublic = $rdpPublic; admins = $admins; neverExpire = $never; guestEnabled = $guest
  uac = if ($sys -and $null -ne $sys.EnableLUA) { [int]$sys.EnableLUA } else { $null }
  autologonPassword = [bool]($wl -and $wl.DefaultPassword); wdigest = if ($wd -and $null -ne $wd.UseLogonCredential) { [int]$wd.UseLogonCredential } else { $null }
  macros = $macros
} | ConvertTo-Json -Depth 4 -Compress
"#;

/// ¿Es una exclusión de Defender de las que usan los programas maliciosos?
pub fn risky_exclusion(path: &str) -> Option<&'static str> {
    let p = path.trim().trim_end_matches('\\').to_ascii_lowercase();
    if p.len() <= 2 || (p.len() == 3 && p.ends_with(":\\")) || p.ends_with(':') {
        return Some("excluye una unidad entera");
    }
    if p.contains("\\temp") || p.contains("\\appdata\\local\\temp") {
        return Some("excluye una carpeta temporal");
    }
    if p.ends_with("\\users") || p.ends_with("\\usuarios") || p.contains("\\users\\public") {
        return Some("excluye las carpetas de los usuarios");
    }
    if p.ends_with("\\appdata") || p.ends_with("\\appdata\\roaming") || p.ends_with("\\appdata\\local") || p.ends_with("\\downloads") || p.ends_with("\\descargas") || p.ends_with("\\programdata") {
        return Some("excluye una carpeta donde se guardan descargas y programas de usuario");
    }
    if p.ends_with("\\windows") || p.ends_with("\\windows\\system32") {
        return Some("excluye la carpeta de Windows");
    }
    None
}

/// Extensiones excluidas que permiten saltarse el antivirus con cualquier programa.
pub fn risky_extension(ext: &str) -> bool {
    matches!(ext.trim_start_matches('.').to_ascii_lowercase().as_str(), "exe" | "dll" | "ps1" | "bat" | "cmd" | "vbs" | "js" | "scr" | "msi" | "hta")
}

pub fn findings(r: &SecRaw) -> Vec<Finding> {
    let mut out = Vec::new();
    if r.exclusions_read {
        for p in &r.exclusion_paths {
            if let Some(why) = risky_exclusion(p) {
                out.push(finding("bad", format!("Exclusión de Defender: {p}"), format!("Defender no mira lo que hay ahí: {why}. Es la señal típica de una infección (el programa malicioso se excluye a sí mismo) o de un «truco» que alguien aplicó para que algo funcionara.")).fixes(vec![fix_confirm(format!("sec.excl.path:{p}"), "Quitar la exclusión", true, "Defender volverá a analizar esa carpeta. Si un programa legítimo dependía de ella, puede que avise o lo bloquee.")]));
            }
        }
        for e in r.exclusion_ext.iter().filter(|e| risky_extension(e)) {
            out.push(finding("bad", format!("Exclusión de Defender: archivos .{}", e.trim_start_matches('.')), "Defender ignora todos los archivos con esa extensión en todo el equipo.").fixes(vec![fix_confirm(format!("sec.excl.ext:{e}"), "Quitar la exclusión", true, "Defender volverá a analizar esos archivos.")]));
        }
        if !r.exclusion_proc.is_empty() {
            out.push(finding("info", format!("{} proceso(s) excluidos de Defender", r.exclusion_proc.len()), format!("{}. Si no los reconoces, revísalos en Seguridad de Windows → Exclusiones.", r.exclusion_proc.iter().take(5).cloned().collect::<Vec<_>>().join(", "))));
        }
    } else {
        out.push(finding("info", "Exclusiones de Defender sin revisar", "Hace falta ejecutar AdminOps como administrador para leerlas."));
    }
    if r.smb1 == Some(true) {
        out.push(finding("warn", "SMBv1 está activado", "Es la versión antigua de compartir carpetas, la que usó WannaCry. Solo la necesitan aparatos muy viejos (algún NAS o escáner).").fixes(vec![fix("sec.smb1", "Desactivar SMBv1", true)]));
    }
    if r.rdp_enabled && r.rdp_public {
        out.push(finding("bad", "Escritorio remoto abierto en redes públicas", "El cortafuegos deja entrar al Escritorio remoto también en redes públicas (cafeterías, hoteles). Es una de las entradas favoritas de los atacantes.").fixes(vec![fix("sec.rdp.public", "Cerrarlo en redes públicas", true)]));
    } else if r.rdp_enabled {
        out.push(finding("info", "Escritorio remoto activado", "Solo en redes privadas o de dominio. Úsalo dentro de la red o por VPN, nunca abierto a Internet en el router."));
    }
    let users: Vec<&String> = r.admins.iter().filter(|a| !a.to_ascii_lowercase().contains("domain admins") && !a.to_ascii_lowercase().contains("admins. del dominio")).collect();
    if users.len() > 2 {
        out.push(finding("warn", format!("{} cuentas con permisos de administrador", users.len()), format!("{}. Cada administrador de más es una puerta más: el día a día se hace mejor con una cuenta estándar.", users.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))).page("users"));
    }
    if !r.never_expire.is_empty() {
        out.push(finding("info", format!("{} cuenta(s) cuya contraseña nunca caduca", r.never_expire.len()), format!("{}. No es grave si la contraseña es buena y única; lo es si es la misma en todos los equipos.", r.never_expire.join(", "))).page("users"));
    }
    if r.guest_enabled {
        out.push(finding("warn", "La cuenta de invitado está activada", "Permite entrar sin contraseña a quien tenga el equipo delante y, a veces, desde la red.").fixes(vec![fix("sec.guest", "Desactivar la cuenta de invitado", true)]));
    }
    if r.uac == Some(0) {
        out.push(finding("bad", "El control de cuentas (UAC) está apagado", "Cualquier programa se ejecuta con todos los permisos sin preguntar.").fixes(vec![fix_confirm("sec.uac", "Activar el control de cuentas", true, "Hace falta reiniciar para que surta efecto.")]));
    }
    if r.autologon_password {
        out.push(finding("bad", "Contraseña de inicio automático guardada en claro", "Windows entra solo y la contraseña está escrita tal cual en el registro: cualquiera con acceso al equipo puede leerla.").fixes(vec![fix_confirm("sec.autologon", "Borrar la contraseña guardada", true, "El equipo dejará de iniciar sesión solo: habrá que escribir la contraseña al encenderlo.")]));
    }
    if r.wdigest == Some(1) {
        out.push(finding("bad", "WDigest guarda las contraseñas en memoria", "Con esta opción, las herramientas de los atacantes leen las contraseñas de quien haya iniciado sesión.").fixes(vec![fix("sec.wdigest", "Desactivar WDigest", true)]));
    }
    for (app, v) in &r.macros {
        if *v == 1 {
            out.push(finding("bad", format!("{app} ejecuta todas las macros sin preguntar"), "Un documento con macros maliciosas se ejecuta solo al abrirlo: es la vía de entrada más común del ransomware.").fixes(vec![fix(format!("sec.macros:{app}"), "Pedir permiso antes de ejecutar macros", false)]));
        }
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(0, finding("ok", "Nada raro en lo que no se ve", "Exclusiones de Defender, SMBv1, Escritorio remoto, cuentas, UAC, contraseñas guardadas y macros: en orden."));
    }
    out
}

pub fn check() -> Result<Vec<Finding>, String> {
    let script = format!("{}{SCRIPT}", crate::target_user::script_prelude());
    let out = crate::pspool::query(&script, Some(Duration::from_secs(60)), "Seguridad a fondo")?;
    Ok(findings(&parse::<SecRaw>(&out)?))
}

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

pub fn run(tweaks: &TweakState, kind: &str, arg: &str) -> Option<Result<String, String>> {
    let r = match kind {
        "sec.excl.path" | "sec.excl.ext" => need_admin().and_then(|()| {
            let flag = if kind == "sec.excl.path" { "ExclusionPath" } else { "ExclusionExtension" };
            crate::ps::powershell(&format!("$ErrorActionPreference = 'Stop'\n{}Remove-MpPreference -{flag} $v\n'ok'", crate::ps::text_var("v", arg))).map(|_| format!("Exclusión «{arg}» quitada: Defender vuelve a analizarlo."))
        }),
        "sec.smb1" => tweaks.fix_catalog("security.smb1-off"),
        "sec.rdp.public" => need_admin().and_then(|()| crate::ps::powershell("Get-NetFirewallRule -DisplayGroup '*Remote Desktop*', '*Escritorio remoto*' -Direction Inbound -ErrorAction SilentlyContinue | Set-NetFirewallRule -Profile Domain, Private\n'ok'").map(|_| "El Escritorio remoto ya no acepta conexiones en redes públicas.".into())),
        "sec.guest" => need_admin().and_then(|()| crate::ps::powershell("Get-LocalUser | Where-Object { $_.SID -like '*-501' } | Disable-LocalUser\n'ok'").map(|_| "Cuenta de invitado desactivada.".into())),
        "sec.uac" => need_admin().and_then(|()| registry::write(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System", "EnableLUA", RegKind::Dword, &RegData::Int(1)).map(|()| "Control de cuentas activado. Reinicia para que surta efecto.".into())),
        "sec.autologon" => need_admin().and_then(|()| registry::delete(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon", "DefaultPassword").and_then(|()| registry::write(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon", "AutoAdminLogon", RegKind::String, &RegData::Str("0".into()))).map(|()| "Contraseña guardada borrada; el inicio automático queda desactivado.".into())),
        "sec.wdigest" => need_admin().and_then(|()| registry::write(r"HKLM\SYSTEM\CurrentControlSet\Control\SecurityProviders\WDigest", "UseLogonCredential", RegKind::Dword, &RegData::Int(0)).map(|()| "WDigest desactivado (se nota tras cerrar sesión o reiniciar).".into())),
        "sec.macros" if ["Word", "Excel", "PowerPoint"].contains(&arg) => registry::write(&format!(r"HKCU\Software\Microsoft\Office\16.0\{arg}\Security"), "VBAWarnings", RegKind::Dword, &RegData::Int(2)).map(|()| format!("{arg} preguntará antes de ejecutar macros.")),
        _ => return None,
    };
    Some(r)
}

pub fn title(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "sec.excl.path" | "sec.excl.ext" => "Defender: quitar una exclusión sospechosa",
        "sec.rdp.public" => "Cerrar el Escritorio remoto en redes públicas",
        "sec.guest" => "Desactivar la cuenta de invitado",
        "sec.uac" => "Activar el control de cuentas (UAC)",
        "sec.autologon" => "Borrar la contraseña de inicio automático",
        "sec.wdigest" => "Desactivar WDigest",
        "sec.macros" => "Office: pedir permiso antes de ejecutar macros",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusiones_peligrosas() {
        assert!(risky_exclusion(r"C:\").is_some());
        assert!(risky_exclusion(r"C:\Users\Public").is_some());
        assert!(risky_exclusion(r"C:\Users\ana\AppData\Local\Temp").is_some());
        assert!(risky_exclusion(r"C:\Windows\System32").is_some());
        assert!(risky_exclusion(r"D:\Datos\SQL").is_none());
        assert!(risky_extension(".exe"));
        assert!(!risky_extension("mdf"));
    }

    #[test]
    fn hallazgos() {
        let r = SecRaw { exclusions_read: true, exclusion_paths: vec![r"C:\Users\Public".into(), r"D:\SQL".into()], smb1: Some(true), rdp_enabled: true, rdp_public: true, admins: vec!["PC\\Administrador".into(), "PC\\ana".into(), "PC\\soporte".into()], wdigest: Some(1), macros: vec![("Word".into(), 1)], ..Default::default() };
        let f = findings(&r);
        assert_eq!(f.iter().filter(|x| x.title.starts_with("Exclusión")).count(), 1);
        assert!(f.iter().any(|x| x.title.contains("SMBv1")));
        assert!(f.iter().any(|x| x.title.contains("redes públicas")));
        assert!(f.iter().any(|x| x.title.contains("3 cuentas")));
        assert!(f.iter().any(|x| x.fixes.iter().any(|y| y.id == "sec.macros:Word")));
        let ok = findings(&SecRaw { exclusions_read: true, ..Default::default() });
        assert_eq!(ok[0].level, "ok");
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
