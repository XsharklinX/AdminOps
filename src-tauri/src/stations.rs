//! Comprobador de puestos: dada una lista de equipos (nombres o IP), cuáles
//! responden y, si hay acceso de administración remota (WinRM o DCOM, lo normal
//! en un dominio), cuánto disco les queda, cuándo arrancaron, quién está
//! conectado y cuántos días llevan sin instalar actualizaciones.

use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Remote {
    pub os: String,
    pub user: String,
    pub boot_days: Option<f64>,
    pub free_gb: Option<f64>,
    pub total_gb: Option<f64>,
    /// Días desde la última actualización instalada.
    pub update_days: Option<f64>,
    /// Por dónde se leyó: WinRM | DCOM.
    pub via: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    pub host: String,
    pub ip: String,
    pub online: bool,
    pub ms: Option<u32>,
    /// Puertos abiertos de los que importan: 3389 (Escritorio remoto), 445 (carpetas), 5985 (WinRM).
    pub ports: Vec<u16>,
    pub remote: Option<Remote>,
    /// Por qué no se pudo leer a fondo.
    pub remote_error: String,
    /// Avisos: poco disco, muchos días sin reiniciar o sin actualizar.
    pub warnings: Vec<String>,
}

fn valid_host(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
}

fn resolve(host: &str) -> Option<std::net::IpAddr> {
    if let Ok(ip) = host.parse() {
        return Some(ip);
    }
    (host, 0).to_socket_addrs().ok()?.map(|a| a.ip()).find(|ip| ip.is_ipv4())
}

fn port_open(ip: std::net::IpAddr, port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::new(ip, port), Duration::from_millis(1500)).is_ok()
}

const REMOTE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$s = $null; $via = ''
try { $s = New-CimSession -ComputerName $target -OperationTimeoutSec 10; $via = 'WinRM' } catch {
  $s = New-CimSession -ComputerName $target -OperationTimeoutSec 10 -SessionOption (New-CimSessionOption -Protocol Dcom); $via = 'DCOM'
}
try {
  $os = Get-CimInstance -CimSession $s -ClassName Win32_OperatingSystem
  $cs = Get-CimInstance -CimSession $s -ClassName Win32_ComputerSystem
  $d = Get-CimInstance -CimSession $s -ClassName Win32_LogicalDisk -Filter "DeviceID='$($os.SystemDrive)'"
  $last = $null
  try { $last = Get-CimInstance -CimSession $s -ClassName Win32_QuickFixEngineering | Where-Object { $_.InstalledOn } | Sort-Object InstalledOn -Descending | Select-Object -First 1 } catch { }
  [pscustomobject]@{
    os = "$($os.Caption)"; user = "$($cs.UserName)"; via = $via
    bootDays = ((Get-Date) - $os.LastBootUpTime).TotalDays
    freeGb = if ($d) { $d.FreeSpace / 1GB } else { $null }; totalGb = if ($d) { $d.Size / 1GB } else { $null }
    updateDays = if ($last) { ((Get-Date) - [datetime]$last.InstalledOn).TotalDays } else { $null }
  } | ConvertTo-Json -Compress
} finally { if ($s) { Remove-CimSession $s } }
"#;

fn remote(host: &str) -> Result<Remote, String> {
    let script = format!("{}{REMOTE_SCRIPT}", crate::ps::text_var("target", host));
    // Proceso propio (no el grupo compartido): una comprobación de muchos equipos
    // tardaría minutos y dejaría esperando al resto de AdminOps.
    let out = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(45)), task: Some("stations") })?;
    serde_json::from_str(out.trim()).map_err(|_| "Respuesta inesperada.".to_string())
}

fn explain_remote(e: &str) -> String {
    let l = e.to_lowercase();
    if l.contains("access is denied") || l.contains("acceso denegado") {
        "Sin permiso: hace falta un usuario administrador de ese equipo (en un dominio, un administrador del dominio).".into()
    } else if l.contains("rpc") || l.contains("winrm") || l.contains("cannot connect") || l.contains("no se puede") || l.contains("timeout") || l.contains("tiempo") {
        "No acepta administración remota (WinRM o DCOM bloqueados por el firewall).".into()
    } else {
        e.chars().take(160).collect()
    }
}

pub fn warnings(r: &Remote) -> Vec<String> {
    let mut w = Vec::new();
    if let (Some(free), Some(total)) = (r.free_gb, r.total_gb) {
        if free < 10.0 || (total > 0.0 && free / total < 0.1) {
            w.push(format!("Poco disco: {free:.1} GB libres"));
        }
    }
    if r.boot_days.is_some_and(|d| d > 14.0) {
        w.push(format!("{:.0} días sin reiniciar", r.boot_days.unwrap_or(0.0)));
    }
    if r.update_days.is_some_and(|d| d > 45.0) {
        w.push(format!("{:.0} días sin actualizaciones", r.update_days.unwrap_or(0.0)));
    }
    w
}

fn check(host: &str, deep: bool) -> Station {
    let mut s = Station { host: host.to_string(), ..Default::default() };
    let Some(ip) = resolve(host) else {
        s.remote_error = "El nombre no se resuelve (no existe o no está en esta red).".into();
        return s;
    };
    s.ip = ip.to_string();
    if let std::net::IpAddr::V4(v4) = ip {
        let p = crate::network::diag::ping("puesto", &v4.to_string(), 2);
        s.ms = p.min_ms;
        s.online = p.received > 0;
    }
    s.ports = [3389u16, 445, 5985].into_iter().filter(|p| port_open(ip, *p)).collect();
    // Con el firewall bloqueando el ping, que tenga puertos abiertos también es estar encendido.
    s.online |= !s.ports.is_empty();
    if deep && s.online {
        match remote(host) {
            Ok(r) => {
                s.warnings = warnings(&r);
                s.remote = Some(r);
            }
            Err(e) => s.remote_error = explain_remote(&e),
        }
    }
    s
}

#[tauri::command(async)]
pub fn check_stations(hosts: Vec<String>, deep: bool) -> Result<Vec<Station>, String> {
    let mut list: Vec<String> = hosts.iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
    list.dedup();
    if list.len() > 300 {
        return Err("Como mucho 300 equipos por comprobación.".into());
    }
    if let Some(bad) = list.iter().find(|h| !valid_host(h)) {
        return Err(format!("«{bad}» no es un nombre de equipo ni una IP válidos."));
    }
    let t = Instant::now();
    // En grupos para no abrir cientos de conexiones a la vez.
    let mut out = Vec::with_capacity(list.len());
    // A fondo, cada equipo abre un PowerShell: como mucho 8 a la vez.
    for chunk in list.chunks(if deep { 8 } else { 24 }) {
        let part: Vec<Station> = std::thread::scope(|s| {
            let h: Vec<_> = chunk.iter().map(|host| s.spawn(move || check(host, deep))).collect();
            h.into_iter().filter_map(|x| x.join().ok()).collect()
        });
        out.extend(part);
    }
    log::info!("Puestos comprobados: {} en {:.1} s", out.len(), t.elapsed().as_secs_f64());
    Ok(out)
}

/// Qué hacer en varios puestos a la vez.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StationAction {
    /// Reinicio con aviso al usuario y margen para cancelarlo.
    Restart,
    /// Anula un reinicio programado.
    CancelRestart,
    /// Vuelve a aplicar las directivas de grupo del equipo.
    Gpupdate,
    /// Mensaje en pantalla a quien esté conectado.
    Message,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub host: String,
    pub ok: bool,
    pub detail: String,
}

/// Minutos de margen antes de reiniciar: el usuario ve el aviso y guarda su trabajo.
const RESTART_DELAY_S: u32 = 120;

/// Cada acción usa la herramienta estándar de Windows para ello (shutdown, msg,
/// administración remota); el resultado vuelve como código de salida.
fn action_script(action: StationAction) -> &'static str {
    match action {
        StationAction::Restart => r#"
& shutdown.exe /r /m "\\$target" /t $delay /d p:0:0 /c $text 2>&1 | Out-Null
[pscustomobject]@{ code = $LASTEXITCODE } | ConvertTo-Json -Compress
"#,
        StationAction::CancelRestart => r#"
& shutdown.exe /a /m "\\$target" 2>&1 | Out-Null
[pscustomobject]@{ code = $LASTEXITCODE } | ConvertTo-Json -Compress
"#,
        StationAction::Message => r#"
& msg.exe * "/server:$target" /time:600 $text 2>&1 | Out-Null
[pscustomobject]@{ code = $LASTEXITCODE } | ConvertTo-Json -Compress
"#,
        StationAction::Gpupdate => r#"
$code = Invoke-Command -ComputerName $target -ScriptBlock { gpupdate.exe /target:computer /force /wait:0 | Out-Null; $LASTEXITCODE }
[pscustomobject]@{ code = [int]$code } | ConvertTo-Json -Compress
"#,
    }
}

fn explain_code(action: StationAction, code: i64) -> String {
    match (action, code) {
        (StationAction::Restart, 1190) => "Ya tenía un reinicio programado.".into(),
        (StationAction::CancelRestart, 1116) => "No había ningún reinicio programado.".into(),
        (_, 5) => "Sin permiso: hace falta un usuario administrador de ese equipo.".into(),
        (_, 53) | (_, 1722) | (_, 1726) => "No se pudo contactar (apagado, o el firewall bloquea la administración remota).".into(),
        (StationAction::Message, _) => "No se pudo mostrar el mensaje (el equipo no admite mensajes remotos o nadie tiene sesión).".into(),
        _ => format!("Windows devolvió el código {code}."),
    }
}

fn run_action(host: &str, action: StationAction, text: &str) -> ActionResult {
    let script = format!(
        "{}{}{}{}",
        crate::ps::text_var("target", host),
        crate::ps::text_var("text", text),
        format_args!("$delay = {RESTART_DELAY_S}\n"),
        action_script(action)
    );
    let r = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: Some("stations") })
        .and_then(|out| serde_json::from_str::<serde_json::Value>(out.trim()).map_err(|_| "Respuesta inesperada.".to_string()));
    let (ok, detail) = match r {
        Ok(v) => match v["code"].as_i64().unwrap_or(-1) {
            0 => (
                true,
                match action {
                    StationAction::Restart => format!("Se reiniciará en {} minutos (el usuario ve el aviso).", RESTART_DELAY_S / 60),
                    StationAction::CancelRestart => "Reinicio cancelado.".into(),
                    StationAction::Gpupdate => "Directivas actualizadas.".into(),
                    StationAction::Message => "Mensaje enviado.".into(),
                },
            ),
            code => (false, explain_code(action, code)),
        },
        Err(e) => (false, explain_remote(&e)),
    };
    ActionResult { host: host.to_string(), ok, detail }
}

/// Reiniciar, cancelar un reinicio, actualizar directivas o enviar un mensaje a
/// varios puestos a la vez. Cada acción queda en el Historial.
#[tauri::command(async)]
pub fn station_action(
    hosts: Vec<String>,
    action: StationAction,
    text: Option<String>,
    state: tauri::State<'_, crate::tweaks::TweakState>,
) -> Result<Vec<ActionResult>, String> {
    let mut list: Vec<String> = hosts.iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
    list.dedup();
    if list.is_empty() {
        return Err("Elige al menos un equipo.".into());
    }
    if list.len() > 100 {
        return Err("Como mucho 100 equipos a la vez.".into());
    }
    if let Some(bad) = list.iter().find(|h| !valid_host(h)) {
        return Err(format!("«{bad}» no es un nombre de equipo ni una IP válidos."));
    }
    let text = text.unwrap_or_default().trim().chars().take(250).collect::<String>();
    if action == StationAction::Message && text.is_empty() {
        return Err("Escribe el mensaje.".into());
    }
    let text = if action == StationAction::Restart && text.is_empty() {
        "Este equipo se reiniciará en unos minutos por mantenimiento. Guarda tu trabajo.".to_string()
    } else {
        text
    };
    let mut out = Vec::with_capacity(list.len());
    for chunk in list.chunks(8) {
        let part: Vec<ActionResult> = std::thread::scope(|s| {
            let h: Vec<_> = chunk.iter().map(|host| s.spawn(|| run_action(host, action, &text))).collect();
            h.into_iter().filter_map(|x| x.join().ok()).collect()
        });
        out.extend(part);
    }
    let what = match action {
        StationAction::Restart => "Reiniciar",
        StationAction::CancelRestart => "Cancelar reinicio de",
        StationAction::Gpupdate => "Actualizar directivas en",
        StationAction::Message => "Mensaje a",
    };
    let failed = out.iter().filter(|r| !r.ok).count();
    let summary: Result<(), String> = if failed == 0 { Ok(()) } else { Err(format!("{failed} de {} fallaron", out.len())) };
    state.record(crate::tweaks::journal::Op::Run, &format!("{what} {} puesto(s)", out.len()), &summary);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_scripts_parse() {
        for a in [StationAction::Restart, StationAction::CancelRestart, StationAction::Gpupdate, StationAction::Message] {
            let e = crate::ps::parse_errors(&format!("$target = 'pc1'\n$text = 'hola'\n$delay = 120\n{}", action_script(a)));
            assert!(e.is_empty(), "{a:?}: {e}");
        }
    }

    #[test]
    fn explains_common_codes() {
        assert!(explain_code(StationAction::Restart, 5).contains("permiso"));
        assert!(explain_code(StationAction::Restart, 1190).contains("Ya tenía"));
        assert!(explain_code(StationAction::Gpupdate, 53).contains("contactar"));
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(&format!("$target = 'pc1'\n{REMOTE_SCRIPT}"));
        assert!(e.is_empty(), "{e}");
    }

    #[test]
    fn validates_hosts() {
        assert!(valid_host("PC-CONTA-01"));
        assert!(valid_host("192.168.1.20"));
        assert!(valid_host("pc1.empresa.local"));
        assert!(!valid_host("pc1; calc"));
        assert!(!valid_host("$(x)"));
        assert!(check_stations(vec!["a b".into()], false).is_err());
    }

    #[test]
    fn warns_about_disk_reboot_and_updates() {
        let r = Remote { free_gb: Some(4.0), total_gb: Some(240.0), boot_days: Some(30.0), update_days: Some(90.0), ..Default::default() };
        assert_eq!(warnings(&r).len(), 3);
        assert!(warnings(&Remote { free_gb: Some(100.0), total_gb: Some(240.0), boot_days: Some(1.0), update_days: Some(5.0), ..Default::default() }).is_empty());
    }

    /// Equipo real: `cargo test stations_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn stations_real() {
        let host = sysinfo::System::host_name().unwrap();
        for s in check_stations(vec![host, "127.0.0.1".into(), "no-existe-xyz".into()], true).unwrap() {
            println!("{s:#?}");
        }
    }

    #[test]
    fn this_pc_is_online() {
        let s = check("127.0.0.1", false);
        assert!(s.online);
    }
}
