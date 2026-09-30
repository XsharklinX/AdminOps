//! La oficina completa: encender equipos por la red (Wake-on-LAN), acceso
//! remoto (Escritorio remoto, Asistencia remota), carpetas compartidas y
//! conflictos de IP en la red.

use crate::ps::text_var;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;
use tauri::State;

fn elevated() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

// ---------- Wake-on-LAN ----------

fn parse_mac(mac: &str) -> Option<[u8; 6]> {
    let hex: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 12 || mac.chars().any(|c| !c.is_ascii_hexdigit() && !":-. ".contains(c)) {
        return None;
    }
    let mut out = [0u8; 6];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Paquete mágico: 6 bytes 0xFF y la MAC repetida 16 veces.
fn magic_packet(mac: [u8; 6]) -> Vec<u8> {
    let mut p = vec![0xFFu8; 6];
    for _ in 0..16 {
        p.extend_from_slice(&mac);
    }
    p
}

/// Difusión de la subred del equipo (192.168.1.255 para 192.168.1.x/24).
fn subnet_broadcast(ip: Ipv4Addr, prefix: u32) -> Ipv4Addr {
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix.min(32)) };
    Ipv4Addr::from(u32::from(ip) | !mask)
}

#[tauri::command(async)]
pub fn wake_on_lan(mac: String) -> Result<(), String> {
    let mac = parse_mac(&mac).ok_or("La MAC no es válida (formato 00:11:22:33:44:55).")?;
    let packet = magic_packet(mac);
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;
    let mut targets = vec![Ipv4Addr::BROADCAST];
    if let Some(info) = crate::network::lan::current().ok().flatten() {
        if let Ok(ip) = info.ip.parse::<Ipv4Addr>() {
            targets.push(subnet_broadcast(ip, info.prefix));
        }
    }
    let mut sent = false;
    // Varias veces y a los dos puertos habituales: el paquete no tiene confirmación.
    for _ in 0..3 {
        for t in &targets {
            for port in [9u16, 7] {
                sent |= socket.send_to(&packet, (*t, port)).is_ok();
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    if sent {
        log::info!("Wake-on-LAN enviado a {}", mac.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":"));
        Ok(())
    } else {
        Err("No se pudo enviar el paquete de encendido.".into())
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WolAdapter {
    name: String,
    description: String,
    /// Enabled | Disabled | Unsupported
    magic_packet: String,
    wired: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    /// Este equipo acepta Escritorio remoto.
    rdp_enabled: bool,
    /// La edición permite ser servidor de Escritorio remoto (Pro o superior).
    rdp_supported: bool,
    /// Inicio rápido de Windows activo (impide el encendido por red desde apagado).
    fast_startup: bool,
    adapters: Vec<WolAdapter>,
    /// Nombre del equipo y su IP en la red, para conectarse a él.
    host: String,
    ip: String,
    mac: String,
}

const WOL_SCRIPT: &str = r#"
$r = @(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | ForEach-Object {
  $pm = Get-NetAdapterPowerManagement -Name $_.Name -ErrorAction SilentlyContinue
  [pscustomobject]@{ name = "$($_.Name)"; description = "$($_.InterfaceDescription)"
    magicPacket = if ($pm) { "$($pm.WakeOnMagicPacket)" } else { 'Unsupported' }
    wired = [bool]("$($_.PhysicalMediaType)" -match '802\.3') }
})
ConvertTo-Json -InputObject $r -Compress
"#;

#[tauri::command(async)]
pub fn remote_status() -> Result<RemoteStatus, String> {
    use crate::tweaks::registry::read_u32;
    let adapters: Vec<WolAdapter> = serde_json::from_str(crate::ps::powershell(WOL_SCRIPT)?.trim()).unwrap_or_default();
    let lan = crate::network::lan::current().ok().flatten();
    Ok(RemoteStatus {
        rdp_enabled: read_u32(r"HKLM\SYSTEM\CurrentControlSet\Control\Terminal Server", "fDenyTSConnections") == Some(0),
        rdp_supported: crate::vault::vault_support().bitlocker,
        fast_startup: read_u32(r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Power", "HiberbootEnabled").unwrap_or(1) == 1,
        adapters,
        host: sysinfo::System::host_name().unwrap_or_default(),
        ip: lan.as_ref().map(|l| l.ip.clone()).unwrap_or_default(),
        mac: lan.map(|l| l.mac).unwrap_or_default(),
    })
}

/// Deja este equipo listo para encenderse por la red: paquete mágico en los
/// adaptadores de cable e inicio rápido desactivado. La BIOS debe permitirlo.
#[tauri::command(async)]
pub fn enable_wake_on_lan(tweaks: State<'_, TweakState>) -> Result<(), String> {
    elevated()?;
    let script = r#"
Get-NetAdapter -Physical | Where-Object { "$($_.PhysicalMediaType)" -match '802\.3' } | ForEach-Object {
  Set-NetAdapterPowerManagement -Name $_.Name -WakeOnMagicPacket Enabled -ErrorAction SilentlyContinue
}
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Power' -Name HiberbootEnabled -Value 0 -Type DWord
'ok'
"#;
    let r = crate::ps::powershell(script).map(|_| ());
    tweaks.record(Op::Run, "Preparar el encendido por red (Wake-on-LAN)", &r);
    r
}

/// Activa o desactiva el Escritorio remoto de este equipo (con autenticación de red y su regla de firewall).
#[tauri::command(async)]
pub fn set_remote_desktop(tweaks: State<'_, TweakState>, enabled: bool) -> Result<(), String> {
    elevated()?;
    if enabled && !crate::vault::vault_support().bitlocker {
        return Err("Windows Home no puede recibir conexiones de Escritorio remoto (sí conectarse a otros). Usa AnyDesk o RustDesk.".into());
    }
    // "@FirewallAPI.dll,-28752" es el grupo "Escritorio remoto" en cualquier idioma.
    let script = format!(
        "Set-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Terminal Server' -Name fDenyTSConnections -Value {} -Type DWord\n\
         Set-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Terminal Server\\WinStations\\RDP-Tcp' -Name UserAuthentication -Value 1 -Type DWord\n\
         {}-NetFirewallRule -Group '@FirewallAPI.dll,-28752' -ErrorAction SilentlyContinue\n'ok'",
        if enabled { 0 } else { 1 },
        if enabled { "Enable" } else { "Disable" }
    );
    let r = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, if enabled { "Escritorio remoto activado" } else { "Escritorio remoto desactivado" }, &r);
    r
}

fn valid_host(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || ".-_:".contains(c))
}

#[tauri::command]
pub fn open_remote_desktop(host: String) -> Result<(), String> {
    let host = host.trim();
    if !valid_host(host) {
        return Err("Escribe un nombre de equipo o una IP.".into());
    }
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    std::process::Command::new(format!("{system}\\System32\\mstsc.exe")).arg(format!("/v:{host}")).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Abre la Asistencia remota de Windows (msra.exe): invitar a alguien de
/// confianza o ayudar a quien te invitó. Viene con Windows y no depende de la
/// Store. (La Asistencia rápida ya no se puede usar y se quitó de AdminOps.)
#[tauri::command]
pub fn open_remote_assistance() -> Result<(), String> {
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let exe = std::path::PathBuf::from(system).join("System32").join("msra.exe");
    if !exe.is_file() {
        return Err("Este equipo no tiene la Asistencia remota de Windows. Usa AnyDesk, RustDesk o TeamViewer, aquí abajo.".into());
    }
    crate::shellopen::open(&exe.to_string_lossy())
}

// ---------- Carpetas compartidas ----------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccess {
    account: String,
    /// Full | Change | Read
    right: String,
    allow: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    name: String,
    path: String,
    description: String,
    access: Vec<ShareAccess>,
    /// Archivos abiertos ahora mismo desde otros equipos.
    open_files: u32,
    /// La carpeta que se comparte ya no existe: la compartición está rota y
    /// quien entre verá un error raro de Windows en vez de una explicación.
    missing_path: bool,
    /// Se comparte con «Todos», pero los permisos del disco (NTFS) no dejan
    /// entrar a todos. Es la trampa clásica: el recurso parece abierto y la
    /// gente recibe «acceso denegado» sin que nadie entienda por qué, porque el
    /// acceso real es la intersección de los dos permisos.
    ntfs_blocks: bool,
}

/// Un archivo que alguien tiene abierto ahora mismo desde otro equipo.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpenFile {
    /// Solo el nombre del archivo: la ruta llevaría el nombre del usuario.
    name: String,
    /// Quién lo tiene abierto.
    user: String,
    /// Está bloqueado para escritura por ese usuario.
    locked: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SharingStatus {
    shares: Vec<Share>,
    /// Public | Private | DomainAuthenticated
    category: String,
    file_sharing: bool,
    discovery: bool,
    /// Equipos conectados ahora a las carpetas de este (usuario@equipo).
    sessions: Vec<String>,
    /// Qué archivos están abiertos ahora mismo, y por quién.
    open: Vec<OpenFile>,
}

const SHARES_SCRIPT: &str = r#"
$open = @(Get-SmbOpenFile -ErrorAction SilentlyContinue)
$shares = @(Get-SmbShare -ErrorAction SilentlyContinue | Where-Object { -not $_.Special -and $_.Name -notmatch '\$$' -and $_.ShareType -eq 'FileSystemDirectory' } | ForEach-Object {
  $s = $_
  $acc = @(Get-SmbShareAccess -Name $s.Name -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ account = "$($_.AccountName)"; right = "$($_.AccessRight)"; allow = "$($_.AccessControlType)" -eq 'Allow' } })
  $existe = Test-Path -LiteralPath "$($s.Path)"
  # ¿Se comparte con Todos pero el disco no deja entrar a Todos?
  $paraTodos = @($acc | Where-Object { $_.allow -and "$($_.account)" -match 'Everyone|Todos' }).Count -gt 0
  $ntfsBloquea = $false
  if ($paraTodos -and $existe) {
    try {
      $acl = (Get-Acl -LiteralPath "$($s.Path)").Access
      $abierto = @($acl | Where-Object { "$($_.AccessControlType)" -eq 'Allow' -and "$($_.IdentityReference)" -match 'Everyone|Todos|Users|Usuarios|Authenticated' }).Count -gt 0
      $ntfsBloquea = -not $abierto
    } catch {}
  }
  [pscustomobject]@{
    name = "$($s.Name)"; path = "$($s.Path)"; description = "$($s.Description)"
    access = $acc
    openFiles = @($open | Where-Object { "$($_.ShareRelativePath)" -and "$($_.Path)".StartsWith("$($s.Path)", 'OrdinalIgnoreCase') }).Count
    missingPath = -not $existe
    ntfsBlocks = [bool]$ntfsBloquea
  }
})
$abiertos = @($open | Where-Object { "$($_.ShareRelativePath)" } | ForEach-Object {
  [pscustomobject]@{
    name = (Split-Path "$($_.ShareRelativePath)" -Leaf)
    user = "$($_.ClientUserName)"
    locked = [bool]("$($_.Locks)" -ne '0' -and "$($_.Locks)")
  }
} | Select-Object -First 50)
$profile = Get-NetConnectionProfile -ErrorAction SilentlyContinue | Select-Object -First 1
$fs = @(Get-NetFirewallRule -Group '@FirewallAPI.dll,-28502' -Direction Inbound -ErrorAction SilentlyContinue | Where-Object { "$($_.Enabled)" -eq 'True' }).Count -gt 0
$nd = @(Get-NetFirewallRule -Group '@FirewallAPI.dll,-32752' -Direction Inbound -ErrorAction SilentlyContinue | Where-Object { "$($_.Enabled)" -eq 'True' }).Count -gt 0
$sessions = @(Get-SmbSession -ErrorAction SilentlyContinue | ForEach-Object { "$($_.ClientUserName) desde $($_.ClientComputerName)" } | Sort-Object -Unique)
[pscustomobject]@{ shares = $shares; category = if ($profile) { "$($profile.NetworkCategory)" } else { '' }; fileSharing = $fs; discovery = $nd; sessions = $sessions; open = $abiertos } | ConvertTo-Json -Depth 4 -Compress
"#;
#[tauri::command(async)]
pub fn list_shares() -> Result<SharingStatus, String> {
    let out = crate::pspool::query(SHARES_SCRIPT, Some(Duration::from_secs(40)), "Carpetas compartidas")?;
    serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))
}

fn valid_share_name(n: &str) -> bool {
    !n.is_empty() && n.chars().count() <= 80 && !n.ends_with('$') && !n.chars().any(|c| "\\/[]:|<>+=;,?*\"".contains(c) || c.is_control())
}

/// Comparte una carpeta. `who`: "everyone" o el nombre de un usuario local; `write`: permitir cambios.
#[tauri::command(async)]
pub fn create_share(tweaks: State<'_, TweakState>, path: String, name: String, who: String, write: bool) -> Result<(), String> {
    elevated()?;
    let dir = std::path::Path::new(path.trim());
    if !dir.is_dir() {
        return Err("La carpeta no existe.".into());
    }
    if crate::wipe::forbidden(dir) {
        return Err("No se comparten carpetas del sistema ni unidades enteras: elige una carpeta concreta.".into());
    }
    let name = name.trim();
    if !valid_share_name(name) {
        return Err("El nombre del recurso no es válido (sin \\ / : * ? \" < > | y sin $ al final).".into());
    }
    let account = if who == "everyone" {
        // "Todos" / "Everyone" según el idioma de Windows.
        "$acct = ([Security.Principal.SecurityIdentifier]'S-1-1-0').Translate([Security.Principal.NTAccount]).Value\n".to_string()
    } else {
        format!("{}$acct = \"$env:COMPUTERNAME\\$user\"\n", text_var("user", who.trim()))
    };
    let (share_right, ntfs) = if write { ("-ChangeAccess", "M") } else { ("-ReadAccess", "RX") };
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n{}{}{account}\
         New-SmbShare -Name $name -Path $path {share_right} $acct -FolderEnumerationMode AccessBased | Out-Null\n\
         & icacls.exe $path /grant \"$($acct):(OI)(CI){ntfs}\" /T /C /Q | Out-Null\n'ok'",
        text_var("path", &dir.display().to_string()),
        text_var("name", name)
    );
    let r = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, &format!("Carpeta compartida «{name}» ({})", if write { "lectura y escritura" } else { "solo lectura" }), &r);
    r
}

#[tauri::command(async)]
pub fn remove_share(tweaks: State<'_, TweakState>, name: String) -> Result<(), String> {
    elevated()?;
    let script = format!("$ErrorActionPreference = 'Stop'\n{}Remove-SmbShare -Name $name -Force\n'ok'", text_var("name", &name));
    let r = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, &format!("Dejar de compartir «{name}»"), &r);
    r
}

/// Red privada + compartir archivos + detección de redes (lo necesario para que otros vean las carpetas).
#[tauri::command(async)]
pub fn enable_file_sharing(tweaks: State<'_, TweakState>) -> Result<(), String> {
    elevated()?;
    let script = "Get-NetConnectionProfile | Where-Object { \"$($_.NetworkCategory)\" -eq 'Public' } | Set-NetConnectionProfile -NetworkCategory Private\n\
                  Enable-NetFirewallRule -Group '@FirewallAPI.dll,-28502' -ErrorAction SilentlyContinue\n\
                  Enable-NetFirewallRule -Group '@FirewallAPI.dll,-32752' -ErrorAction SilentlyContinue\n'ok'";
    let r = crate::ps::powershell(script).map(|_| ());
    tweaks.record(Op::Run, "Activar compartir archivos en la red privada", &r);
    r
}

// ---------- Conflictos de IP ----------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IpConflict {
    time: String,
    ip: String,
    /// MAC del otro equipo que usa la misma IP.
    mac: String,
}

const CONFLICTS_SCRIPT: &str = r#"
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Tcpip'; Id = 4199; StartTime = (Get-Date).AddDays(-30) } -MaxEvents 50 -ErrorAction SilentlyContinue)
$r = @($ev | ForEach-Object {
  $m = $_.Message
  $ip = [regex]::Match($m, '\b(\d{1,3}\.){3}\d{1,3}\b').Value
  $mac = [regex]::Match($m, '\b([0-9A-Fa-f]{2}[-:]){5}[0-9A-Fa-f]{2}\b').Value
  if ($ip) { [pscustomobject]@{ time = $_.TimeCreated.ToString('o'); ip = $ip; mac = $mac } }
})
ConvertTo-Json -InputObject $r -Compress
"#;

/// Conflictos de IP que Windows detectó en los últimos 30 días (otro equipo con la misma IP).
#[tauri::command(async)]
pub fn ip_conflicts() -> Result<Vec<IpConflict>, String> {
    let out = crate::pspool::query(CONFLICTS_SCRIPT, Some(Duration::from_secs(30)), "Conflictos de IP")?;
    Ok(serde_json::from_str(out.trim()).unwrap_or_default())
}

// ---------- Exportar ----------

/// Guarda un CSV (inventario, dispositivos…) donde elija el usuario. Con BOM para que Excel lea las tildes.
#[tauri::command(async)]
pub fn export_csv(app: tauri::AppHandle, name: String, content: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_".contains(c) { c } else { '_' }).collect();
    let Some(file) = app.dialog().file().set_file_name(format!("{safe}.csv")).add_filter("CSV (Excel)", &["csv"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::write(&file, format!("\u{feff}{content}")).map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(Some(file.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macs_and_packets() {
        assert_eq!(parse_mac("aa:bb:cc:00:11:22"), Some([0xaa, 0xbb, 0xcc, 0, 0x11, 0x22]));
        assert_eq!(parse_mac("AA-BB-CC-00-11-22"), Some([0xaa, 0xbb, 0xcc, 0, 0x11, 0x22]));
        assert!(parse_mac("aa:bb:cc").is_none());
        assert!(parse_mac("zz:bb:cc:00:11:22").is_none());
        let p = magic_packet([1, 2, 3, 4, 5, 6]);
        assert_eq!(p.len(), 102);
        assert_eq!(&p[..6], &[0xFF; 6]);
        assert_eq!(&p[96..], &[1, 2, 3, 4, 5, 6]);
        assert_eq!(subnet_broadcast("192.168.1.37".parse().unwrap(), 24), Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(subnet_broadcast("10.0.5.9".parse().unwrap(), 16), Ipv4Addr::new(10, 0, 255, 255));
    }

    #[test]
    fn names_and_hosts() {
        assert!(valid_share_name("Facturas 2026"));
        assert!(!valid_share_name("Oculta$"));
        assert!(!valid_share_name("a/b"));
        assert!(valid_host("192.168.1.20") && valid_host("PC-RECEPCION") && !valid_host("a b") && !valid_host("x;calc"));
    }

    /// Equipo real: `cargo test office_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn office_real() {
        let r = remote_status().unwrap();
        println!("rdp {} (admite {}) inicio rápido {} host {} ip {} mac {}", r.rdp_enabled, r.rdp_supported, r.fast_startup, r.host, r.ip, r.mac);
        for a in r.adapters {
            println!("  {} · {} · wol {} · cable {}", a.name, a.description, a.magic_packet, a.wired);
        }
        let s = list_shares().unwrap();
        println!("red {} compartir {} detección {} carpetas {} sesiones {:?}", s.category, s.file_sharing, s.discovery, s.shares.len(), s.sessions);
        println!("conflictos: {:?}", ip_conflicts().unwrap());
    }

    #[test]
    fn embedded_scripts_parse() {
        for (name, script) in [("WOL", WOL_SCRIPT), ("SHARES", SHARES_SCRIPT), ("CONFLICTS", CONFLICTS_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }
}
