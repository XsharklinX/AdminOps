//! Carpetas compartidas, a fondo: cambiar quién entra sin volver a crear la
//! compartición, explicar por qué alguien no puede entrar (lo que deja la
//! compartición ∩ lo que dejan los permisos del disco), las unidades de red de
//! este equipo, qué comparte otro equipo, cuánto ocupa cada carpeta y una copia
//! programada a otro disco.
//!
//! Todo con los mecanismos de Windows y las credenciales de quien usa el
//! equipo: no se guardan contraseñas ni se toca ningún otro equipo de la red.

use crate::ps::text_var;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

fn elevated() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

// ---------- Quién puede entrar ----------

const GRANT_SCRIPT: &str = r#"
if ($who -eq 'everyone') {
  $acct = ([Security.Principal.SecurityIdentifier]'S-1-1-0').Translate([Security.Principal.NTAccount]).Value
} else {
  $acct = $who
  try { [void]([Security.Principal.NTAccount]$acct).Translate([Security.Principal.SecurityIdentifier]) } catch {
    $acct = "$env:COMPUTERNAME\$who"
    try { [void]([Security.Principal.NTAccount]$acct).Translate([Security.Principal.SecurityIdentifier]) } catch { throw "No existe la cuenta «$who» en este equipo." }
  }
}
$s = Get-SmbShare -Name $name
Revoke-SmbShareAccess -Name $name -AccountName $acct -Force -ErrorAction SilentlyContinue | Out-Null
Grant-SmbShareAccess -Name $name -AccountName $acct -AccessRight $right -Force | Out-Null
# Los permisos del disco, para que lo concedido funcione de verdad: un solo
# permiso heredable en la carpeta, que Windows lleva a lo que hay dentro. Nunca
# en un disco entero ni en carpetas de Windows: ahí se dejan como están.
$p = "$($s.Path)".TrimEnd('\')
if ($p.Length -gt 3 -and (Test-Path -LiteralPath $p) -and $p -notlike "$env:SystemRoot*") {
  & icacls.exe $p /grant "$($acct):(OI)(CI)$ntfs" /C /Q | Out-Null
}
'ok'
"#;

fn right_label(right: &str) -> &'static str {
    match right {
        "Full" => "control total",
        "Change" => "leer y modificar",
        _ => "solo leer",
    }
}

/// Da (o cambia) el acceso de una cuenta a una carpeta ya compartida.
/// `account`: "everyone", un usuario local o una cuenta completa («EQUIPO\ana»).
#[tauri::command(async)]
pub fn share_grant(tweaks: State<'_, TweakState>, name: String, account: String, right: String) -> Result<(), String> {
    elevated()?;
    let (name, account) = (name.trim(), account.trim());
    if !crate::office::valid_share_name(name) || account.is_empty() || account.chars().count() > 120 {
        return Err("Elige la carpeta y a quién dar acceso.".into());
    }
    let ntfs = match right.as_str() {
        "Read" => "RX",
        "Change" => "M",
        "Full" => "F",
        _ => return Err("Permiso no válido.".into()),
    };
    let script = format!("$ErrorActionPreference = 'Stop'\n{}{}{}{}{GRANT_SCRIPT}", text_var("name", name), text_var("who", account), text_var("right", &right), text_var("ntfs", ntfs));
    let r = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(900)), task: None }).map(|_| ());
    let who = if account == "everyone" { "Todos" } else { account.rsplit('\\').next().unwrap_or(account) };
    tweaks.record(Op::Run, &format!("Carpeta compartida «{name}»: {who} → {}", right_label(&right)), &r);
    r
}

/// Quita a una cuenta de la lista de la compartición (o le quita un «Denegar»).
/// Los permisos del disco no se tocan.
#[tauri::command(async)]
pub fn share_revoke(tweaks: State<'_, TweakState>, name: String, account: String, deny: bool) -> Result<(), String> {
    elevated()?;
    let (name, account) = (name.trim(), account.trim());
    if !crate::office::valid_share_name(name) || account.is_empty() {
        return Err("Elige la carpeta y la cuenta.".into());
    }
    let verb = if deny { "Unblock-SmbShareAccess" } else { "Revoke-SmbShareAccess" };
    let script = format!("$ErrorActionPreference = 'Stop'\n{}{}{verb} -Name $name -AccountName $who -Force | Out-Null\n'ok'", text_var("name", name), text_var("who", account));
    let r = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, &format!("Carpeta compartida «{name}»: quitar a {}", account.rsplit('\\').next().unwrap_or(account)), &r);
    r
}

// ---------- ¿Por qué no puede entrar? ----------

/// Lo que se averigua en el equipo; el veredicto se razona en Rust (`explain`).
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct Facts {
    /// La cuenta existe (se pudo resolver).
    found: bool,
    /// Es una cuenta local de este equipo.
    local: bool,
    enabled: bool,
    /// Nunca se le puso contraseña.
    no_password: bool,
    microsoft: bool,
    /// 0 nada · 1 leer · 2 modificar · 3 control total
    share: u8,
    share_deny: bool,
    path_exists: bool,
    /// Se pudieron leer los permisos del disco.
    acl_read: bool,
    ntfs_read: bool,
    ntfs_write: bool,
    ntfs_deny_read: bool,
    ntfs_deny_write: bool,
    category: String,
    file_sharing: bool,
    /// Windows no deja entrar por red a cuentas sin contraseña (lo normal).
    blank_limit: bool,
    host: String,
}

const EXPLAIN_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$s = Get-SmbShare -Name $share
if (-not $s) { throw 'Esa carpeta ya no está compartida.' }
$sidOf = { param($n) try { ([Security.Principal.NTAccount]"$n").Translate([Security.Principal.SecurityIdentifier]).Value } catch { '' } }
$uSid = & $sidOf $user
if (-not $uSid) { $uSid = & $sidOf "$env:COMPUTERNAME\$user" }
$local = $null
if ($uSid) { $local = Get-LocalUser | Where-Object { "$($_.SID)" -eq $uSid } | Select-Object -First 1 }

# A qué grupos pertenece: los de siempre al entrar por red, más los grupos locales.
$sids = New-Object 'System.Collections.Generic.HashSet[string]'
foreach ($x in 'S-1-1-0', 'S-1-5-11', 'S-1-5-2') { [void]$sids.Add($x) }
if ($uSid) {
  [void]$sids.Add($uSid)
  foreach ($g in @(Get-LocalGroup)) {
    try {
      $grp = [ADSI]"WinNT://$env:COMPUTERNAME/$($g.Name),group"
      foreach ($m in @($grp.Invoke('Members'))) {
        $b = $m.GetType().InvokeMember('objectSid', 'GetProperty', $null, $m, $null)
        if ((New-Object Security.Principal.SecurityIdentifier($b, 0)).Value -eq $uSid) { [void]$sids.Add("$($g.SID)"); break }
      }
    } catch {}
  }
}

$rank = @{ Read = 1; Change = 2; Full = 3 }
$shareAllow = 0; $shareDeny = $false
foreach ($a in @(Get-SmbShareAccess -Name $share)) {
  $sid = & $sidOf $a.AccountName
  if (-not $sid -or -not $sids.Contains($sid)) { continue }
  if ("$($a.AccessControlType)" -eq 'Allow') {
    $r = $rank["$($a.AccessRight)"]; if (-not $r) { $r = 1 }
    if ($r -gt $shareAllow) { $shareAllow = $r }
  } else { $shareDeny = $true }
}

$exists = Test-Path -LiteralPath "$($s.Path)"
$aclOk = $false; $nr = $false; $nw = $false; $dr = $false; $dw = $false
if ($exists) {
  try {
    $acl = Get-Acl -LiteralPath "$($s.Path)" -ErrorAction Stop
    $aclOk = $true
    foreach ($e in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
      if (-not $sids.Contains($e.IdentityReference.Value)) { continue }
      $r = [int64][int]$e.FileSystemRights
      # Leer datos (0x1) o genérico leer/todo; escribir datos (0x2) o genérico escribir/todo.
      $read = (($r -band 0x1) -ne 0) -or (($r -band 0x90000000) -ne 0)
      $write = (($r -band 0x2) -ne 0) -or (($r -band 0x50000000) -ne 0)
      if ("$($e.AccessControlType)" -eq 'Allow') { if ($read) { $nr = $true }; if ($write) { $nw = $true } }
      else { if ($read) { $dr = $true }; if ($write) { $dw = $true } }
    }
  } catch {}
}
$profile = Get-NetConnectionProfile | Select-Object -First 1
$fs = @(Get-NetFirewallRule -Group '@FirewallAPI.dll,-28502' -Direction Inbound | Where-Object { "$($_.Enabled)" -eq 'True' }).Count -gt 0
$blank = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa' -Name LimitBlankPasswordUse).LimitBlankPasswordUse
[pscustomobject]@{
  found = [bool]$uSid; local = [bool]$local
  enabled = if ($local) { [bool]$local.Enabled } else { $true }
  noPassword = [bool]($local -and -not $local.PasswordLastSet)
  microsoft = [bool]($local -and "$($local.PrincipalSource)" -eq 'MicrosoftAccount')
  share = $shareAllow; shareDeny = $shareDeny
  pathExists = [bool]$exists; aclRead = $aclOk
  ntfsRead = $nr; ntfsWrite = $nw; ntfsDenyRead = $dr; ntfsDenyWrite = $dw
  category = if ($profile) { "$($profile.NetworkCategory)" } else { '' }
  fileSharing = $fs
  blankLimit = "$blank" -ne '0'
  host = "$env:COMPUTERNAME"
} | ConvertTo-Json -Compress
"#;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// bad | warn | ok
    level: &'static str,
    text: String,
    /// Qué botón lo arregla: "" | "sharing" (activar compartir) | "permissions" (quién puede entrar).
    fix: &'static str,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Explain {
    /// none | read | write
    verdict: &'static str,
    headline: String,
    /// Lo que deja la compartición y lo que dejan los permisos del disco.
    share_right: &'static str,
    disk_right: &'static str,
    findings: Vec<Finding>,
}

fn explain(f: &Facts, user: &str, share: &str) -> Explain {
    let mut out: Vec<Finding> = Vec::new();
    let mut add = |level: &'static str, fix: &'static str, text: String| out.push(Finding { level, text, fix });
    let mut blocked = false;

    if f.category == "Public" {
        blocked = true;
        add("bad", "sharing", "Windows trata esta red como pública: desde otros equipos no se ve ninguna carpeta de este.".into());
    } else if !f.file_sharing {
        blocked = true;
        add("bad", "sharing", "«Compartir archivos e impresoras» está cerrado en el firewall de este equipo.".into());
    }
    if !f.path_exists {
        blocked = true;
        add("bad", "", "La carpeta que se comparte ya no existe en el disco: se movió, se renombró o se borró.".into());
    }
    if !f.found {
        blocked = true;
        add(
            "bad",
            "",
            format!("En este equipo no hay ninguna cuenta «{user}». Para entrar desde otro equipo hace falta una cuenta de aquí (o del dominio) con su contraseña: créala en Usuarios y dale acceso."),
        );
    } else {
        if !f.enabled {
            blocked = true;
            add("bad", "", "La cuenta está desactivada: no puede iniciar sesión ni entrar por red.".into());
        }
        if f.no_password && f.blank_limit {
            blocked = true;
            add("bad", "", "Parece que la cuenta no tiene contraseña, y Windows no deja entrar por red a cuentas sin contraseña. Ponle una en Usuarios.".into());
        }
        if f.microsoft {
            add("warn", "", "Es una cuenta de Microsoft: desde otro equipo se entra con el correo y la contraseña de Microsoft, no con el PIN.".into());
        }
    }

    let share_level = if f.share_deny { 0 } else { f.share.min(3) };
    if f.found {
        if f.share_deny {
            add("bad", "permissions", "En la compartición hay un «Denegar» que le afecta, y un denegar gana siempre a lo permitido.".into());
        } else if f.share == 0 {
            add("bad", "permissions", "No está en la lista de quién puede entrar a esta carpeta compartida (ni él ni ningún grupo suyo).".into());
        }
    }

    let disk_read = f.ntfs_read && !f.ntfs_deny_read;
    let disk_write = disk_read && f.ntfs_write && !f.ntfs_deny_write;
    if f.path_exists && f.found {
        if !f.acl_read {
            add("warn", "", "No se pudieron leer los permisos del disco de esa carpeta: no se puede asegurar lo que le dejan hacer.".into());
        } else if f.ntfs_deny_read || (f.ntfs_deny_write && share_level >= 2) {
            add("bad", "", "En los permisos del disco (pestaña Seguridad de la carpeta) hay un «Denegar» que le afecta. Se quita desde ahí.".into());
        } else if !disk_read {
            add("bad", "permissions", "Los permisos del disco (pestaña Seguridad) no le dejan entrar. El acceso real es lo que permitan la compartición y el disco a la vez.".into());
        } else if share_level >= 2 && !disk_write {
            add("warn", "permissions", "La compartición le deja modificar, pero los permisos del disco solo le dejan leer: no podrá guardar cambios.".into());
        } else if share_level == 1 && disk_write {
            add("ok", "", "El disco le dejaría modificar, pero la compartición es de solo lectura: manda lo más restrictivo.".into());
        }
    }

    let can_read = !blocked && f.found && share_level >= 1 && (disk_read || !f.acl_read);
    let can_write = can_read && share_level >= 2 && disk_write;
    let verdict = if can_write {
        "write"
    } else if can_read {
        "read"
    } else {
        "none"
    };
    if verdict != "none" {
        add(
            "ok",
            "",
            format!(
                "En este equipo todo le deja entrar. Si aun así no puede, el fallo está en el otro lado: que escriba bien \\\\{}\\{share} y que use el usuario y la contraseña de esta cuenta (si su equipo recuerda otros, hay que borrarlos en el Administrador de credenciales).",
                f.host
            ),
        );
    }
    let headline = match verdict {
        "write" => format!("«{user}» puede entrar a «{share}», leer y modificar."),
        "read" => format!("«{user}» puede entrar a «{share}», pero solo leer."),
        _ => format!("«{user}» no puede entrar a «{share}»."),
    };
    Explain {
        verdict,
        headline,
        share_right: match share_level {
            0 => "none",
            1 => "read",
            2 => "change",
            _ => "full",
        },
        disk_right: if !f.acl_read {
            "unknown"
        } else if disk_write {
            "write"
        } else if disk_read {
            "read"
        } else {
            "none"
        },
        findings: out,
    }
}

/// Qué puede hacer de verdad una cuenta en una carpeta compartida, y qué se lo impide.
#[tauri::command(async)]
pub fn share_explain(name: String, user: String) -> Result<Explain, String> {
    let (name, user) = (name.trim(), user.trim());
    if !crate::office::valid_share_name(name) || user.is_empty() || user.chars().count() > 120 {
        return Err("Elige una carpeta y escribe un usuario.".into());
    }
    let script = format!("{}{}{EXPLAIN_SCRIPT}", text_var("share", name), text_var("user", user));
    let out = crate::pspool::query(&script, Some(Duration::from_secs(40)), "Acceso a una carpeta compartida")?;
    let facts: Facts = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    Ok(explain(&facts, user, name))
}

// ---------- Unidades de red de este equipo ----------

/// `\\equipo\recurso[\subcarpeta]` → (equipo, resto). Solo rutas de red normales.
fn parse_unc(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix(r"\\")?;
    let (host, tail) = rest.split_once('\\')?;
    let tail = tail.trim_end_matches('\\');
    let ok_segment = |s: &str| !s.is_empty() && !s.chars().any(|c| "\"<>|/:*?".contains(c) || c.is_control());
    if !crate::office::valid_host(host) || path.chars().count() > 260 || !tail.split('\\').all(ok_segment) {
        return None;
    }
    Some((host, tail))
}

fn drive_letter(letter: &str) -> Result<char, String> {
    let mut it = letter.trim().trim_end_matches(':').chars();
    match (it.next(), it.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Ok(c.to_ascii_uppercase()),
        _ => Err("Letra de unidad no válida.".into()),
    }
}

/// Las unidades de red que Windows recuerda para el usuario con la sesión
/// abierta (`HKCU\Network`): letra y ruta.
#[cfg(windows)]
fn network_key() -> Option<winreg::RegKey> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_USERS};
    use winreg::RegKey;
    match crate::target_user::hkcu_redirect() {
        Some(sid) => RegKey::predef(HKEY_USERS).open_subkey(format!(r"{sid}\Network")).ok(),
        None => RegKey::predef(HKEY_CURRENT_USER).open_subkey("Network").ok(),
    }
}

#[cfg(windows)]
fn remembered() -> Vec<(char, String)> {
    let Some(key) = network_key() else { return Vec::new() };
    let mut out: Vec<(char, String)> = key
        .enum_keys()
        .flatten()
        .filter_map(|name| {
            let letter = drive_letter(&name).ok()?;
            let path: String = key.open_subkey(&name).ok()?.get_value("RemotePath").ok()?;
            Some((letter, path))
        })
        .collect();
    out.sort();
    out
}

#[cfg(not(windows))]
fn remembered() -> Vec<(char, String)> {
    Vec::new()
}

/// Windows deja de recordar la unidad (por si `net use` no pudo quitarla al no
/// estar conectada).
#[cfg(windows)]
fn forget(letter: char) {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_USERS, KEY_ALL_ACCESS};
    use winreg::RegKey;
    let key = match crate::target_user::hkcu_redirect() {
        Some(sid) => RegKey::predef(HKEY_USERS).open_subkey_with_flags(format!(r"{sid}\Network"), KEY_ALL_ACCESS),
        None => RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Network", KEY_ALL_ACCESS),
    };
    if let Ok(key) = key {
        let _ = key.delete_subkey_all(letter.to_string());
    }
}

#[cfg(not(windows))]
fn forget(_: char) {}

/// ¿Contesta ese equipo por el puerto de las carpetas compartidas (445)?
fn answers(host: &str) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    (host, 445u16).to_socket_addrs().map(|addrs| addrs.take(2).any(|a| TcpStream::connect_timeout(&a, Duration::from_millis(1500)).is_ok())).unwrap_or(false)
}

#[cfg(windows)]
fn used_letters() -> u32 {
    unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() }
}

#[cfg(not(windows))]
fn used_letters() -> u32 {
    0
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetDrive {
    letter: String,
    path: String,
    host: String,
    /// El equipo que la sirve contesta ahora mismo.
    reachable: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetDrives {
    drives: Vec<NetDrive>,
    /// Letras libres para conectar una unidad nueva.
    free: Vec<String>,
}

/// Unidades de red que Windows recuerda, y si el equipo que las sirve contesta.
#[tauri::command(async)]
pub fn network_drives() -> NetDrives {
    let known = remembered();
    let drives: Vec<NetDrive> = std::thread::scope(|scope| {
        let jobs: Vec<_> = known
            .iter()
            .map(|(letter, path)| {
                scope.spawn(move || {
                    let host = parse_unc(path).map(|(h, _)| h.to_string()).unwrap_or_default();
                    let reachable = !host.is_empty() && answers(&host);
                    NetDrive { letter: letter.to_string(), path: path.clone(), host, reachable }
                })
            })
            .collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    });
    let used = used_letters();
    let free = ('D'..='Z').rev().filter(|c| used & (1 << (*c as u32 - 'A' as u32)) == 0 && !known.iter().any(|(l, _)| l == c)).map(|c| c.to_string()).collect();
    NetDrives { drives, free }
}

/// Lo que dice `net use` cuando falla, en claro.
fn net_error(raw: &str) -> String {
    let code = raw.split(|c: char| !c.is_ascii_digit()).find(|t| t.len() >= 2 && t.len() <= 4).and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
    net_code(code).unwrap_or_else(|| "Windows no pudo conectar la unidad.".into())
}

fn net_code(code: u32) -> Option<String> {
    Some(
        match code {
            5 => "Ese equipo no deja entrar con tu usuario: pide otro usuario y contraseña. Abre la ruta en el Explorador para escribirlos.",
            53 | 51 | 1231 | 1232 => "No se encuentra ese equipo en la red: revisa el nombre, que esté encendido y en la misma red.",
            67 => "Ese equipo no comparte ninguna carpeta con ese nombre.",
            85 => "Esa letra ya está en uso.",
            86 | 1326 | 1327 | 2242 => "El usuario o la contraseña no valen para ese equipo. Abre la ruta en el Explorador para escribirlos.",
            1219 => "Ya hay una conexión a ese equipo con otro usuario, y Windows solo admite uno a la vez por equipo. Cierra la otra unidad de ese equipo o reinicia la sesión.",
            1272 => "Ese equipo solo deja entrar como invitado y este Windows lo bloquea por seguridad: hace falta una cuenta con contraseña en ese equipo.",
            _ => return None,
        }
        .to_string(),
    )
}

/// `net use …` en la sesión del usuario: las unidades de red son de cada sesión,
/// y las que crea un programa elevado no las ve el Explorador. Por eso, elevado,
/// se le pide al escritorio de Windows que lo ejecute (como al escribirlo en
/// Inicio → Ejecutar). `Ok(true)`: Windows ya contestó; `Ok(false)`: se lanzó y
/// hay que comprobar el resultado.
fn net_use(args: &[&str]) -> Result<bool, String> {
    if !crate::elevation::is_elevated() && crate::target_user::hkcu_redirect().is_none() {
        return crate::ps::exec("net.exe", args).map(|_| true).map_err(|e| net_error(&e));
    }
    let line = args.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.to_string() }).collect::<Vec<_>>().join(" ");
    crate::shellopen::run_as_user("net.exe", &line).map(|_| false)
}

/// Espera a que Windows recuerde (o deje de recordar) una unidad.
fn wait_remembered(letter: char, present: bool, limit: Duration) -> bool {
    let until = std::time::Instant::now() + limit;
    loop {
        if remembered().iter().any(|(l, _)| *l == letter) == present {
            return true;
        }
        if std::time::Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

const NEEDS_LOGIN: &str = "Windows no conectó la unidad. Lo normal es que ese equipo pida usuario y contraseña: pulsa «Abrir» para escribirlos en el Explorador (marca «Recordar mis credenciales») y vuelve a intentarlo.";

/// Conecta una carpeta de otro equipo como unidad (X:), y que Windows la recuerde.
#[tauri::command(async)]
pub fn map_network_drive(tweaks: State<'_, TweakState>, letter: String, path: String) -> Result<(), String> {
    let letter = drive_letter(&letter)?;
    let path = path.trim().trim_end_matches('\\');
    let Some((host, _)) = parse_unc(path) else {
        return Err(r"Escribe la ruta así: \\EQUIPO\Carpeta".into());
    };
    if used_letters() & (1 << (letter as u32 - 'A' as u32)) != 0 || remembered().iter().any(|(l, _)| *l == letter) {
        return Err(format!("La letra {letter}: ya está en uso."));
    }
    let r = (|| {
        if !answers(host) {
            return Err(format!("«{host}» no responde: revisa el nombre, que esté encendido y en la misma red."));
        }
        let device = format!("{letter}:");
        if !net_use(&["use", &device, path, "/persistent:yes"])? && !wait_remembered(letter, true, Duration::from_secs(12)) {
            return Err(NEEDS_LOGIN.to_string());
        }
        Ok(())
    })();
    tweaks.record(Op::Run, &format!("Conectar la unidad de red {letter}: ({path})"), &r);
    r
}

/// Quita una unidad de red de este equipo. La carpeta del otro equipo no se toca.
#[tauri::command(async)]
pub fn unmap_network_drive(tweaks: State<'_, TweakState>, letter: String) -> Result<(), String> {
    let letter = drive_letter(&letter)?;
    if !remembered().iter().any(|(l, _)| *l == letter) {
        return Err("Esa letra no es una unidad de red.".into());
    }
    let device = format!("{letter}:");
    let direct = net_use(&["use", &device, "/delete", "/y"]);
    if !matches!(direct, Ok(true)) {
        wait_remembered(letter, false, Duration::from_secs(5));
    }
    // Si no estaba conectada, `net use` no la encuentra: que Windows la olvide igual.
    forget(letter);
    let r = if remembered().iter().any(|(l, _)| *l == letter) { Err("No se pudo quitar la unidad.".to_string()) } else { Ok(()) };
    tweaks.record(Op::Run, &format!("Quitar la unidad de red {letter}:"), &r);
    r
}

/// Vuelve a conectar una unidad que sale con la X roja. Devuelve qué pasó.
#[tauri::command(async)]
pub fn reconnect_network_drive(letter: String) -> Result<String, String> {
    let letter = drive_letter(&letter)?;
    let Some((_, path)) = remembered().into_iter().find(|(l, _)| *l == letter) else {
        return Err("Esa letra no es una unidad de red.".into());
    };
    let host = parse_unc(&path).map(|(h, _)| h.to_string()).unwrap_or_default();
    if host.is_empty() || !answers(&host) {
        return Err(format!("«{host}» no responde: está apagado, fuera de la red o ha cambiado de nombre. La unidad volverá sola cuando ese equipo esté disponible."));
    }
    let device = format!("{letter}:");
    match net_use(&["use", &device, &path, "/persistent:yes"]) {
        Ok(true) => Ok(format!("{letter}: reconectada.")),
        // «Ya está en uso»: Windows la tiene; se reconecta sola al abrirla.
        Err(e) if e == net_code(85).unwrap_or_default() => Ok(format!("{letter}: ya estaba conectada.")),
        Err(e) => Err(e),
        Ok(false) => Ok(format!("Se pidió a Windows que reconecte {letter}:. Si sigue con la X roja, pulsa «Abrir»: Windows pedirá el usuario y la contraseña.")),
    }
}

/// Abre una ruta de red o una unidad en el Explorador, como el usuario: si el
/// otro equipo pide usuario y contraseña, es Windows quien los pide y los guarda.
#[tauri::command]
pub fn open_network_path(path: String) -> Result<(), String> {
    let path = path.trim();
    let is_drive = path.len() <= 3 && drive_letter(path.trim_end_matches('\\')).is_ok();
    let is_host = path.strip_prefix(r"\\").is_some_and(crate::office::valid_host);
    if !is_drive && !is_host && parse_unc(path).is_none() {
        return Err("Ruta de red no válida.".into());
    }
    let target = if is_drive { format!("{}:\\", &path[..1]) } else { path.to_string() };
    crate::shellopen::open(&target)
}

// ---------- Qué comparte otro equipo ----------

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteShare {
    name: String,
    /// folder | printer
    kind: &'static str,
    remark: String,
}

#[cfg(windows)]
fn enum_shares(host: &str) -> Result<Vec<RemoteShare>, u32> {
    use windows_sys::Win32::NetworkManagement::NetManagement::NetApiBufferFree;
    use windows_sys::Win32::Storage::FileSystem::{NetShareEnum, SHARE_INFO_1};
    unsafe fn text(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
    let server: Vec<u16> = format!(r"\\{host}").encode_utf16().chain(Some(0)).collect();
    let mut buf: *mut u8 = std::ptr::null_mut();
    let (mut read, mut total, mut resume) = (0u32, 0u32, 0u32);
    // Nivel 1: nombre, tipo y comentario; lo mismo que enseña el Explorador en \\EQUIPO.
    let rc = unsafe { NetShareEnum(server.as_ptr(), 1, &mut buf, u32::MAX, &mut read, &mut total, &mut resume) };
    if rc != 0 {
        return Err(rc);
    }
    let mut out = Vec::new();
    if !buf.is_null() {
        let items = unsafe { std::slice::from_raw_parts(buf as *const SHARE_INFO_1, read as usize) };
        for i in items {
            let name = unsafe { text(i.shi1_netname) };
            // Solo carpetas (0) e impresoras (1) normales: ni IPC$ ni los recursos administrativos.
            let kind = match i.shi1_type {
                0 => "folder",
                1 => "printer",
                _ => continue,
            };
            if name.ends_with('$') {
                continue;
            }
            out.push(RemoteShare { name, kind, remark: unsafe { text(i.shi1_remark) } });
        }
        unsafe { NetApiBufferFree(buf as _) };
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

#[cfg(not(windows))]
fn enum_shares(_: &str) -> Result<Vec<RemoteShare>, u32> {
    Err(50)
}

/// Lo que comparte otro equipo de la red, tal como lo enseña el Explorador al
/// abrir `\\EQUIPO`, con las credenciales de quien usa este.
#[tauri::command(async)]
pub fn remote_shares(host: String) -> Result<Vec<RemoteShare>, String> {
    let host = host.trim().trim_start_matches('\\').trim_end_matches('\\');
    if !crate::office::valid_host(host) {
        return Err("Escribe el nombre del equipo o su IP.".into());
    }
    if !answers(host) {
        return Err(format!("«{host}» no responde: revisa el nombre, que esté encendido, en la misma red y con «compartir archivos» activado."));
    }
    enum_shares(host).map_err(|code| net_code(code).unwrap_or_else(|| format!("Ese equipo no dejó ver lo que comparte (código {code}).")))
}

// ---------- Cuánto ocupa cada carpeta ----------

#[derive(Deserialize)]
struct SharePath {
    name: String,
    path: String,
}

const PATHS_SCRIPT: &str = r#"
$r = @(Get-SmbShare -ErrorAction SilentlyContinue | Where-Object { -not $_.Special -and $_.Name -notmatch '\$$' -and $_.ShareType -eq 'FileSystemDirectory' } | ForEach-Object { [pscustomobject]@{ name = "$($_.Name)"; path = "$($_.Path)" } })
ConvertTo-Json -InputObject $r -Compress
"#;

fn share_paths() -> Result<Vec<SharePath>, String> {
    let out = crate::pspool::query(PATHS_SCRIPT, Some(Duration::from_secs(30)), "Carpetas compartidas")?;
    Ok(serde_json::from_str(out.trim()).unwrap_or_default())
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ShareSize {
    name: String,
    bytes: u64,
    files: u64,
    /// Espacio libre y total del disco donde está la carpeta.
    disk_free: u64,
    disk_total: u64,
}

/// (libre, total) del disco que contiene `path`.
#[cfg(windows)]
fn disk_space(path: &std::path::Path) -> (u64, u64) {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let (mut free, mut total) = (0u64, 0u64);
    let ok = unsafe { windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) };
    if ok == 0 {
        (0, 0)
    } else {
        (free, total)
    }
}

#[cfg(not(windows))]
fn disk_space(_: &std::path::Path) -> (u64, u64) {
    (0, 0)
}

/// Tamaño de cada carpeta compartida y espacio libre de su disco. Los discos
/// enteros compartidos no se recorren: se da lo ocupado del disco.
#[tauri::command(async)]
pub fn share_sizes() -> Result<Vec<ShareSize>, String> {
    Ok(share_paths()?
        .into_iter()
        .filter(|s| std::path::Path::new(&s.path).is_dir())
        .map(|s| {
            let dir = std::path::Path::new(&s.path);
            let (disk_free, disk_total) = disk_space(dir);
            let (bytes, files) = if s.path.trim_end_matches('\\').len() <= 2 { (disk_total.saturating_sub(disk_free), 0) } else { crate::space::folder_size(dir) };
            ShareSize { name: s.name, bytes, files, disk_free, disk_total }
        })
        .collect())
}

// ---------- Copia programada a otro disco ----------

const BACKUP_PATH: &str = r"\AdminOps\Copias\";

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ShareBackup {
    /// Nombre de la carpeta compartida.
    share: String,
    /// Carpeta donde se copia.
    dest: String,
    /// HH:MM de cada día.
    time: String,
    last_run: Option<String>,
    /// Código de robocopy: menos de 8 es que copió bien.
    last_result: Option<i64>,
    next_run: Option<String>,
    running: bool,
    /// La última copia terminó bien (`None`: aún no se ha hecho ninguna).
    #[serde(skip_deserializing)]
    ok: Option<bool>,
}

const BACKUPS_SCRIPT: &str = r#"
$r = @(Get-ScheduledTask -TaskPath '\AdminOps\Copias\' -ErrorAction SilentlyContinue | ForEach-Object {
  $i = $_ | Get-ScheduledTaskInfo -ErrorAction SilentlyContinue
  $ran = $i -and $i.LastRunTime -and $i.LastRunTime.Year -gt 2000
  $start = "$(@($_.Triggers)[0].StartBoundary)"
  [pscustomobject]@{
    share = "$($_.TaskName)" -replace '^Copia de ', ''
    dest = "$($_.Description)"
    time = if ($start) { ([datetime]$start).ToString('HH:mm') } else { '' }
    lastRun = if ($ran) { $i.LastRunTime.ToString('o') } else { $null }
    lastResult = if ($ran) { [int64]$i.LastTaskResult } else { $null }
    nextRun = if ($i -and $i.NextRunTime -and $i.NextRunTime.Year -gt 2000) { $i.NextRunTime.ToString('o') } else { $null }
    running = "$($_.State)" -eq 'Running'
  }
})
ConvertTo-Json -InputObject $r -Compress
"#;

/// Robocopy devuelve un mapa de bits: por debajo de 8 no hubo fallos (0 nada que
/// copiar, 1 copió, 2-7 avisos de archivos de más o distintos). 267009 es «la
/// tarea está en marcha».
fn copy_ok(code: i64) -> bool {
    (0..8).contains(&code) || code == 267_009
}

/// Copias programadas de carpetas compartidas, con cómo fue la última.
#[tauri::command(async)]
pub fn share_backups() -> Result<Vec<ShareBackup>, String> {
    let out = crate::pspool::query(BACKUPS_SCRIPT, Some(Duration::from_secs(40)), "Copias de carpetas compartidas")?;
    let mut list: Vec<ShareBackup> = serde_json::from_str(out.trim()).unwrap_or_default();
    for b in &mut list {
        b.ok = b.last_result.map(copy_ok);
    }
    Ok(list)
}

const BACKUP_SET_SCRIPT: &str = r#"
$s = Get-SmbShare -Name $name
$src = "$($s.Path)".TrimEnd('\')
if ($src.Length -le 2) { throw 'No se programa la copia de un disco entero: comparte y copia una carpeta concreta.' }
if (-not (Test-Path -LiteralPath $src)) { throw 'La carpeta que se comparte ya no existe.' }
$dst = (Join-Path $dest $name).TrimEnd('\')
if ("$dst\".StartsWith("$src\", 'OrdinalIgnoreCase') -or "$src\".StartsWith("$dst\", 'OrdinalIgnoreCase')) { throw 'El destino no puede estar dentro de la carpeta que se copia (ni al revés).' }
New-Item -ItemType Directory -Force -Path $dst | Out-Null
# /E: copia lo nuevo y lo cambiado, con subcarpetas. Nunca borra nada en el destino.
$a = '"' + $src + '" "' + $dst + '" /E /R:1 /W:1 /XJ /NP /NFL /NDL /LOG:"' + $dst + '.log"'
$action = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\robocopy.exe" -Argument $a
$trigger = New-ScheduledTaskTrigger -Daily -At $time
$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 8)
Register-ScheduledTask -TaskPath '\AdminOps\Copias\' -TaskName "Copia de $name" -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description $dst -Force | Out-Null
'ok'
"#;

fn valid_time(t: &str) -> bool {
    matches!(t.split_once(':'), Some((h, m)) if h.len() == 2 && m.len() == 2 && h.parse::<u8>().is_ok_and(|h| h < 24) && m.parse::<u8>().is_ok_and(|m| m < 60))
}

/// Programa una copia diaria de una carpeta compartida en otra carpeta (mejor,
/// de otro disco). Se copia a `<destino>\<nombre de la carpeta compartida>`.
#[tauri::command(async)]
pub fn set_share_backup(tweaks: State<'_, TweakState>, name: String, dest: String, time: String) -> Result<(), String> {
    elevated()?;
    let name = name.trim();
    if !crate::office::valid_share_name(name) {
        return Err("Elige la carpeta compartida.".into());
    }
    let dir = std::path::Path::new(dest.trim());
    if !dir.is_absolute() || !dir.is_dir() || dest.contains('"') {
        return Err("Elige una carpeta de destino que exista (por ejemplo, en un disco externo).".into());
    }
    if !valid_time(&time) {
        return Err("La hora debe ser HH:MM.".into());
    }
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n{}{}{}{BACKUP_SET_SCRIPT}",
        text_var("name", name),
        text_var("dest", &dir.display().to_string()),
        text_var("time", &time)
    );
    let r = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, &format!("Copia diaria de la carpeta compartida «{name}» a las {time}"), &r);
    r
}

fn backup_task(verb: &str, name: &str) -> Result<(), String> {
    elevated()?;
    if !crate::office::valid_share_name(name.trim()) {
        return Err("Elige la carpeta compartida.".into());
    }
    let script =
        format!("$ErrorActionPreference = 'Stop'\n{}{verb} -TaskPath '{BACKUP_PATH}' -TaskName \"Copia de $name\"\n'ok'", text_var("name", name.trim()));
    crate::ps::powershell(&script).map(|_| ())
}

/// Deja de copiar. Lo ya copiado se queda donde está.
#[tauri::command(async)]
pub fn remove_share_backup(tweaks: State<'_, TweakState>, name: String) -> Result<(), String> {
    let r = backup_task("Unregister-ScheduledTask -Confirm:$false", &name);
    tweaks.record(Op::Run, &format!("Quitar la copia diaria de «{}»", name.trim()), &r);
    r
}

/// Lanza la copia ahora, sin esperar a su hora.
#[tauri::command(async)]
pub fn run_share_backup(tweaks: State<'_, TweakState>, name: String) -> Result<(), String> {
    let r = backup_task("Start-ScheduledTask", &name);
    tweaks.record(Op::Run, &format!("Copiar ahora la carpeta compartida «{}»", name.trim()), &r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unc_and_letters() {
        assert_eq!(parse_unc(r"\\SERVIDOR\Facturas"), Some(("SERVIDOR", "Facturas")));
        assert_eq!(parse_unc(r"\\192.168.1.20\Datos\2026\"), Some(("192.168.1.20", r"Datos\2026")));
        assert!(parse_unc(r"\\SERVIDOR").is_none());
        assert!(parse_unc(r"\\SERVIDOR\").is_none());
        assert!(parse_unc(r"C:\Datos").is_none());
        assert!(parse_unc("\\\\SRV\\a\" /delete").is_none());
        assert!(parse_unc(r"\\a b\x").is_none());
        assert_eq!(drive_letter("z:"), Ok('Z'));
        assert!(drive_letter("ZZ").is_err() && drive_letter("1").is_err() && drive_letter("").is_err());
    }

    #[test]
    fn net_errors_are_explained() {
        assert!(net_error("Error de sistema 53.\r\n\r\nNo se ha encontrado la ruta de acceso de la red.").contains("No se encuentra"));
        assert!(net_error("System error 1219 has occurred.").contains("otro usuario"));
        assert!(net_error("algo raro").contains("no pudo conectar"));
        assert!(valid_time("03:30") && valid_time("23:59") && !valid_time("24:00") && !valid_time("3:5") && !valid_time("ab:cd"));
        assert!(copy_ok(0) && copy_ok(1) && copy_ok(7) && !copy_ok(8) && !copy_ok(16) && !copy_ok(-1));
    }

    fn facts() -> Facts {
        Facts {
            found: true,
            local: true,
            enabled: true,
            share: 2,
            path_exists: true,
            acl_read: true,
            ntfs_read: true,
            ntfs_write: true,
            category: "Private".into(),
            file_sharing: true,
            blank_limit: true,
            host: "PC".into(),
            ..Default::default()
        }
    }

    #[test]
    fn effective_access_is_the_intersection() {
        // Todo en orden: lee y modifica.
        let e = explain(&facts(), "ana", "Datos");
        assert_eq!((e.verdict, e.share_right, e.disk_right), ("write", "change", "write"));
        assert!(e.findings.iter().all(|f| f.level == "ok"));

        // La compartición deja modificar, el disco solo leer: lee.
        let e = explain(&Facts { ntfs_write: false, ..facts() }, "ana", "Datos");
        assert_eq!(e.verdict, "read");
        assert!(e.findings.iter().any(|f| f.level == "warn" && f.fix == "permissions"));

        // Compartido con ella, pero el disco no la deja: no entra.
        let e = explain(&Facts { ntfs_read: false, ntfs_write: false, ..facts() }, "ana", "Datos");
        assert_eq!((e.verdict, e.disk_right), ("none", "none"));

        // No está en la lista de la compartición.
        let e = explain(&Facts { share: 0, ..facts() }, "ana", "Datos");
        assert_eq!((e.verdict, e.share_right), ("none", "none"));

        // Un denegar gana a un permitir.
        assert_eq!(explain(&Facts { share: 3, share_deny: true, ..facts() }, "ana", "Datos").verdict, "none");
        assert_eq!(explain(&Facts { ntfs_deny_read: true, ..facts() }, "ana", "Datos").verdict, "none");
        assert_eq!(explain(&Facts { ntfs_deny_write: true, ..facts() }, "ana", "Datos").verdict, "read");
    }

    #[test]
    fn blockers_outside_permissions() {
        let none = |f: Facts, fix: &str| {
            let e = explain(&f, "ana", "Datos");
            assert_eq!(e.verdict, "none");
            assert!(e.findings.iter().any(|x| x.level == "bad" && x.fix == fix), "{:?}", e.findings);
        };
        none(Facts { category: "Public".into(), ..facts() }, "sharing");
        none(Facts { file_sharing: false, ..facts() }, "sharing");
        none(Facts { enabled: false, ..facts() }, "");
        none(Facts { no_password: true, ..facts() }, "");
        none(Facts { found: false, ..facts() }, "");
        none(Facts { path_exists: false, ..facts() }, "");
        // Sin contraseña pero con la directiva desactivada: sí entra.
        assert_eq!(explain(&Facts { no_password: true, blank_limit: false, ..facts() }, "ana", "Datos").verdict, "write");
    }

    #[test]
    fn embedded_scripts_parse() {
        for (name, script) in [("GRANT", GRANT_SCRIPT), ("EXPLAIN", EXPLAIN_SCRIPT), ("PATHS", PATHS_SCRIPT), ("BACKUPS", BACKUPS_SCRIPT), ("BACKUP_SET", BACKUP_SET_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }

    /// Equipo real: `cargo test shares_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn shares_real() {
        let d = network_drives();
        println!("unidades {:?} · libres {:?}", d.drives, d.free);
        println!("tamaños {:?}", share_sizes());
        println!("copias {:?}", share_backups());
        println!("localhost comparte {:?}", remote_shares("localhost".into()));
    }
}
