//! Comprobador de puestos: dada una lista de equipos (nombres o IP), cuáles
//! responden y, si hay acceso de administración remota (WinRM o DCOM, lo normal
//! en un dominio), cuánto disco les queda, cuándo arrancaron, quién está
//! conectado y cuántos días llevan sin instalar actualizaciones.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use tauri::Emitter;
use std::time::{Duration, Instant};

/// Quién tenía la sesión abierta en cada puesto en la última comprobación a
/// fondo (equipo → usuario). Solo en memoria: sirve para que la ficha de una
/// persona diga en qué equipo está trabajando, sin preguntarle a nadie.
static SIGNED_IN: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

/// Usuario sin el dominio delante: `EMPRESA\maria.perez` → `maria.perez`.
fn bare_user(u: &str) -> String {
    u.rsplit('\\').next().unwrap_or(u).trim().to_lowercase()
}

/// Equipos donde `sam` tenía la sesión abierta en la última comprobación a fondo.
pub fn signed_in_on(sam: &str) -> Vec<String> {
    let want = bare_user(sam);
    let map = SIGNED_IN.lock().unwrap_or_else(|e| e.into_inner());
    let mut v: Vec<String> = map.iter().filter(|(_, u)| bare_user(u) == want).map(|(h, _)| h.clone()).collect();
    v.sort();
    v
}

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
    /// Salud de los discos según Windows: ok · warn · bad · «» (no se pudo leer).
    pub disk_health: String,
    /// Antivirus que dice el Centro de seguridad («Microsoft Defender»…).
    pub antivirus: String,
    /// Activo y al día (None: no se sabe, p. ej. en servidores).
    pub av_ok: Option<bool>,
    /// Pantallazos azules en los últimos 30 días.
    pub bsods: Option<u32>,
    /// Windows pide reiniciar (una actualización o instalación a medias). None: no se pudo leer.
    pub pending_reboot: Option<bool>,
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
  $health = ''
  try { $pd = @(Get-CimInstance -CimSession $s -Namespace root\Microsoft\Windows\Storage -ClassName MSFT_PhysicalDisk); $worst = ($pd | Measure-Object HealthStatus -Maximum).Maximum; $health = switch ([int]$worst) { 0 { 'ok' } 1 { 'warn' } default { 'bad' } } } catch { }
  $av = ''; $avOk = $null
  try { $p = Get-CimInstance -CimSession $s -Namespace root\SecurityCenter2 -ClassName AntiVirusProduct | Select-Object -First 1; if ($p) { $av = "$($p.displayName)"; $st = [int]$p.productState; $avOk = (($st -band 0x1000) -ne 0) -and (($st -band 0x10) -eq 0) } } catch { }
  $bsod = $null
  try { $since = [Management.ManagementDateTimeConverter]::ToDmtfDateTime((Get-Date).AddDays(-30)); $bsod = @(Get-CimInstance -CimSession $s -ClassName Win32_NTLogEvent -Filter "Logfile='System' AND EventCode=1001 AND SourceName='BugCheck' AND TimeWritten > '$since'").Count } catch { }
  $pend = $null
  try {
    $hk = [uint32]2147483650
    $r1 = Invoke-CimMethod -CimSession $s -Namespace root\cimv2 -ClassName StdRegProv -MethodName EnumKey -Arguments @{ hDefKey = $hk; sSubKeyName = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing' }
    $r2 = Invoke-CimMethod -CimSession $s -Namespace root\cimv2 -ClassName StdRegProv -MethodName EnumKey -Arguments @{ hDefKey = $hk; sSubKeyName = 'SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update' }
    if ($r1.ReturnValue -eq 0 -and $r2.ReturnValue -eq 0) { $pend = (@($r1.sNames) -contains 'RebootPending') -or (@($r2.sNames) -contains 'RebootRequired') }
  } catch { }
  [pscustomobject]@{
    diskHealth = $health; antivirus = $av; avOk = $avOk; bsods = $bsod; pendingReboot = $pend
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
    if r.pending_reboot == Some(true) {
        w.push("Reinicio pendiente".into());
    }
    if r.boot_days.is_some_and(|d| d > 14.0) {
        w.push(format!("{:.0} días sin reiniciar", r.boot_days.unwrap_or(0.0)));
    }
    if r.update_days.is_some_and(|d| d > 45.0) {
        w.push(format!("{:.0} días sin actualizaciones", r.update_days.unwrap_or(0.0)));
    }
    match r.disk_health.as_str() {
        "bad" => w.push("Un disco está fallando".into()),
        "warn" => w.push("Un disco da avisos de salud".into()),
        _ => {}
    }
    if r.av_ok == Some(false) {
        w.push(format!("Antivirus apagado o caducado{}", if r.antivirus.is_empty() { String::new() } else { format!(" ({})", r.antivirus) }));
    }
    if let Some(n) = r.bsods.filter(|n| *n > 0) {
        w.push(format!("{n} pantallazo(s) azul(es) en 30 días"));
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

/// Lista limpia de equipos a comprobar (sin repetidos ni vacíos), o por qué no vale.
fn prepare_hosts(hosts: &[String]) -> Result<Vec<String>, String> {
    let mut list: Vec<String> = hosts.iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
    list.dedup();
    if list.len() > 300 {
        return Err("Como mucho 300 equipos por comprobación.".into());
    }
    if let Some(bad) = list.iter().find(|h| !valid_host(h)) {
        return Err(format!("«{bad}» no es un nombre de equipo ni una IP válidos."));
    }
    Ok(list)
}

/// Comprueba todos los equipos con unos cuantos a la vez; cada uno se avisa en
/// `on_done` (posición en la lista y resultado) en cuanto termina. El resultado
/// final conserva el orden de la lista.
fn check_all(list: &[String], deep: bool, on_done: &(dyn Fn(&Station) + Sync)) -> Vec<Station> {
    let n = list.len();
    // A fondo, cada equipo abre un PowerShell: como mucho 8 a la vez.
    let workers = n.min(if deep { 8 } else { 24 });
    let next = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<Station>>> = Mutex::new(vec![None; n]);
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= n {
                        break;
                    }
                    let st = check(&list[i], deep);
                    on_done(&st);
                    slots.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(st);
                })
            })
            .collect();
        for h in hs {
            let _ = h.join();
        }
    });
    slots.into_inner().unwrap_or_else(|e| e.into_inner()).into_iter().flatten().collect()
}

/// Se apunta quién estaba en cada equipo (para la ficha de la persona).
fn remember_signed_in(out: &[Station], deep: bool) {
    let mut map = SIGNED_IN.lock().unwrap_or_else(|e| e.into_inner());
    for st in out {
        match st.remote.as_ref().map(|r| r.user.trim()).filter(|u| !u.is_empty()) {
            Some(u) => {
                map.insert(st.host.to_uppercase(), u.to_string());
            }
            None if deep => {
                map.remove(&st.host.to_uppercase());
            }
            None => {}
        }
    }
}

#[tauri::command(async)]
pub fn check_stations(hosts: Vec<String>, deep: bool) -> Result<Vec<Station>, String> {
    let list = prepare_hosts(&hosts)?;
    let t = Instant::now();
    let out = check_all(&list, deep, &|_| {});
    log::info!("Puestos comprobados: {} en {:.1} s", out.len(), t.elapsed().as_secs_f64());
    remember_signed_in(&out, deep);
    Ok(out)
}

/// Lo que se manda a la interfaz cada vez que termina un equipo.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StationProgress {
    /// La comprobación a la que pertenece (la interfaz ignora las de antes).
    run: String,
    done: usize,
    total: usize,
    station: Station,
}

/// Como `check_stations`, pero cada equipo sale hacia la interfaz (evento
/// «stations-progress») en cuanto termina, sin esperar al resto.
#[tauri::command(async)]
pub fn check_stations_stream(app: tauri::AppHandle, hosts: Vec<String>, deep: bool, run: String) -> Result<Vec<Station>, String> {
    let list = prepare_hosts(&hosts)?;
    let total = list.len();
    let done = AtomicUsize::new(0);
    let t = Instant::now();
    let out = check_all(&list, deep, &|st| {
        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = app.emit("stations-progress", StationProgress { run: run.clone(), done: n, total, station: st.clone() });
    });
    log::info!("Puestos comprobados: {} en {:.1} s", out.len(), t.elapsed().as_secs_f64());
    remember_signed_in(&out, deep);
    Ok(out)
}

/// Qué hacer en varios puestos a la vez.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
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
    /// Reinicia la cola de impresión (desatasca trabajos).
    Spooler,
    /// Actualiza las firmas de Microsoft Defender.
    Defender,
    /// Vacía la cola de impresión (borra los trabajos atascados).
    ClearQueue,
    /// Borra los temporales de más de dos días (Windows y cada perfil).
    FreeSpace,
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
        StationAction::Spooler => r#"
$code = Invoke-Command -ComputerName $target -ScriptBlock { try { Restart-Service Spooler -Force -ErrorAction Stop; 0 } catch { 1 } }
[pscustomobject]@{ code = [int]$code } | ConvertTo-Json -Compress
"#,
        StationAction::Defender => r#"
$code = Invoke-Command -ComputerName $target -ScriptBlock { try { Update-MpSignature -ErrorAction Stop; 0 } catch { 1 } }
[pscustomobject]@{ code = [int]$code } | ConvertTo-Json -Compress
"#,
        StationAction::ClearQueue => r#"
$code = Invoke-Command -ComputerName $target -ScriptBlock {
  try {
    Stop-Service Spooler -Force -ErrorAction Stop
    Get-ChildItem (Join-Path $env:windir 'System32\spool\PRINTERS') -File -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
    Start-Service Spooler -ErrorAction Stop
    0
  } catch { try { Start-Service Spooler -ErrorAction SilentlyContinue } catch { }; 1 }
}
[pscustomobject]@{ code = [int]$code } | ConvertTo-Json -Compress
"#,
        StationAction::FreeSpace => r#"
$r = Invoke-Command -ComputerName $target -ScriptBlock {
  try {
    $dev = $env:SystemDrive
    $before = (Get-CimInstance Win32_LogicalDisk -Filter ("DeviceID='" + $dev + "'")).FreeSpace
    $cut = (Get-Date).AddDays(-2)
    $dirs = @(Join-Path $env:windir 'Temp') + @(Get-ChildItem (Join-Path $dev 'Users') -Directory -ErrorAction SilentlyContinue | ForEach-Object { Join-Path $_.FullName 'AppData\Local\Temp' })
    foreach ($d in $dirs) {
      if (Test-Path -LiteralPath $d) {
        Get-ChildItem -LiteralPath $d -Force -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -lt $cut } | Remove-Item -Force -ErrorAction SilentlyContinue
      }
    }
    $after = (Get-CimInstance Win32_LogicalDisk -Filter ("DeviceID='" + $dev + "'")).FreeSpace
    [pscustomobject]@{ code = 0; freedMb = [int][math]::Max(0, ($after - $before) / 1MB) }
  } catch { [pscustomobject]@{ code = 1; freedMb = 0 } }
}
[pscustomobject]@{ code = [int]$r.code; freedMb = [int]$r.freedMb } | ConvertTo-Json -Compress
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
        (StationAction::ClearQueue, 1) => "No se pudo vaciar la cola (el servicio de impresión no se detuvo).".into(),
        (StationAction::FreeSpace, 1) => "No se pudo limpiar los temporales de ese equipo.".into(),
        (StationAction::Spooler | StationAction::Defender, 1) => "Windows no pudo hacerlo en ese equipo (servicio deshabilitado o sin Defender).".into(),
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
                    StationAction::ClearQueue => "Cola de impresión vaciada.".into(),
                    StationAction::FreeSpace => format!("Temporales antiguos borrados: {} MB liberados.", v["freedMb"].as_i64().unwrap_or(0)),
                    StationAction::Restart => format!("Se reiniciará en {} minutos (el usuario ve el aviso).", RESTART_DELAY_S / 60),
                    StationAction::CancelRestart => "Reinicio cancelado.".into(),
                    StationAction::Gpupdate => "Directivas actualizadas.".into(),
                    StationAction::Message => "Mensaje enviado.".into(),
                    StationAction::Spooler => "Cola de impresión reiniciada.".into(),
                    StationAction::Defender => "Firmas de Defender actualizadas.".into(),
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
        StationAction::Spooler => "Reiniciar la cola de impresión en",
        StationAction::Defender => "Actualizar Defender en",
        StationAction::ClearQueue => "Vaciar la cola de impresión en",
        StationAction::FreeSpace => "Liberar espacio (temporales) en",
    };
    let failed = out.iter().filter(|r| !r.ok).count();
    let summary: Result<(), String> = if failed == 0 { Ok(()) } else { Err(format!("{failed} de {} fallaron", out.len())) };
    state.record(crate::tweaks::journal::Op::Run, &format!("{what} {} puesto(s)", out.len()), &summary);
    Ok(out)
}

// ---------- Revisar el equipo de una persona ----------

/// Lo que se lee de más en un equipo cuando se le revisa desde la ficha de su usuario.
const REVIEW_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$s = $null
try { $s = New-CimSession -ComputerName $target -OperationTimeoutSec 10 } catch {
  $s = New-CimSession -ComputerName $target -OperationTimeoutSec 10 -SessionOption (New-CimSessionOption -Protocol Dcom)
}
try {
  $os = Get-CimInstance -CimSession $s -ClassName Win32_OperatingSystem
  $errs = $null
  try { $since = [Management.ManagementDateTimeConverter]::ToDmtfDateTime((Get-Date).AddDays(-2)); $errs = @(Get-CimInstance -CimSession $s -ClassName Win32_NTLogEvent -Filter "Logfile='System' AND EventType=1 AND TimeWritten > '$since'").Count } catch { }
  $jobs = $null
  try { $jobs = @(Get-CimInstance -CimSession $s -ClassName Win32_PrintJob).Count } catch { }
  [pscustomobject]@{
    totalMb = [int]($os.TotalVisibleMemorySize / 1024); freeMb = [int]($os.FreePhysicalMemory / 1024)
    errors = $errs; printJobs = $jobs
  } | ConvertTo-Json -Compress
} finally { if ($s) { Remove-CimSession $s } }
"#;

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct ReviewExtra {
    total_mb: Option<u32>,
    free_mb: Option<u32>,
    errors: Option<u32>,
    print_jobs: Option<u32>,
}

/// Una cosa que mirar en el equipo, con su arreglo a un clic si lo hay.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssue {
    /// bad · warn · ok
    pub level: &'static str,
    pub text: String,
    pub fix: Option<StationAction>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StationReview {
    pub station: Station,
    /// Memoria en uso, %.
    pub mem_used_pct: Option<u32>,
    /// Errores del registro del sistema en los últimos 2 días.
    pub errors: Option<u32>,
    pub print_jobs: Option<u32>,
    pub issues: Vec<ReviewIssue>,
}

fn gb(v: f64) -> String {
    format!("{v:.1}").replace('.', ",")
}

/// Qué hay que mirar, de lo más grave a lo menos. Función pura: no lee nada.
pub fn review_issues(st: &Station, mem_used_pct: Option<u32>, errors: Option<u32>, print_jobs: Option<u32>) -> Vec<ReviewIssue> {
    let issue = |level: &'static str, text: String, fix: Option<StationAction>| ReviewIssue { level, text, fix };
    if !st.online {
        return vec![issue("bad", "El equipo no responde: está apagado o no está en esta red.".into(), None)];
    }
    let Some(r) = &st.remote else {
        let why = if st.remote_error.is_empty() { "no se pudo leer".to_string() } else { st.remote_error.clone() };
        return vec![issue("warn", format!("Responde, pero no se pudo revisar a fondo: {why}"), None)];
    };
    let mut v = Vec::new();
    if let (Some(free), Some(total)) = (r.free_gb, r.total_gb) {
        if total > 0.0 {
            let used = ((1.0 - free / total) * 100.0).round();
            if free < 5.0 || free / total < 0.05 {
                v.push(issue("bad", format!("Disco del sistema al {used:.0} % · {} GB libres", gb(free)), Some(StationAction::FreeSpace)));
            } else if free < 10.0 || free / total < 0.10 {
                v.push(issue("warn", format!("Disco del sistema al {used:.0} % · {} GB libres", gb(free)), Some(StationAction::FreeSpace)));
            }
        }
    }
    match r.disk_health.as_str() {
        "bad" => v.push(issue("bad", "Un disco está fallando: conviene una copia de seguridad ya.".into(), None)),
        "warn" => v.push(issue("warn", "Un disco da avisos de salud.".into(), None)),
        _ => {}
    }
    if let Some(n) = print_jobs.filter(|n| *n > 0) {
        v.push(issue("warn", format!("{n} {} en la cola de impresión", if n == 1 { "trabajo atascado" } else { "trabajos atascados" }), Some(StationAction::ClearQueue)));
    }
    let days = r.boot_days.map(|d| d.floor() as u32);
    if r.pending_reboot == Some(true) {
        v.push(issue("warn", format!("Tiene un reinicio pendiente{}", days.map(|d| format!(" · encendido desde hace {d} días")).unwrap_or_default()), Some(StationAction::Restart)));
    } else if let Some(d) = days.filter(|d| *d >= 14) {
        v.push(issue("warn", format!("Encendido desde hace {d} días"), Some(StationAction::Restart)));
    }
    if let Some(m) = mem_used_pct.filter(|m| *m >= 90) {
        v.push(issue("warn", format!("Memoria al {m} %: va justo de RAM"), None));
    }
    if let Some(n) = errors.filter(|n| *n >= 25) {
        v.push(issue("warn", format!("{n} errores en el registro del sistema en los últimos 2 días"), None));
    }
    if let Some(n) = r.bsods.filter(|n| *n > 0) {
        v.push(issue("bad", format!("{n} pantallazo(s) azul(es) en 30 días"), None));
    }
    if r.av_ok == Some(false) {
        v.push(issue("warn", "Antivirus apagado o caducado".into(), None));
    }
    if let Some(d) = r.update_days.filter(|d| *d > 45.0) {
        v.push(issue("warn", format!("{d:.0} días sin instalar actualizaciones"), None));
    }
    // Lo grave primero; dentro de cada grado, el orden en que se ha ido mirando.
    v.sort_by_key(|i| if i.level == "bad" { 0 } else { 1 });
    if v.is_empty() {
        v.push(issue("ok", "Todo en orden: espacio, memoria, reinicios y cola de impresión.".into(), None));
    }
    v
}

/// Lee a distancia el equipo de una persona: espacio, memoria, días encendido,
/// errores recientes y cola de impresión. Solo mira.
#[tauri::command(async)]
pub fn review_station(host: String) -> Result<StationReview, String> {
    let host = host.trim().to_string();
    if !valid_host(&host) {
        return Err(format!("«{host}» no es un nombre de equipo ni una IP válidos."));
    }
    let station = check(&host, true);
    let mut extra = ReviewExtra::default();
    if station.remote.is_some() {
        let script = format!("{}{REVIEW_SCRIPT}", crate::ps::text_var("target", &host));
        if let Ok(out) = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: Some("stations") }) {
            extra = serde_json::from_str(out.trim()).unwrap_or_default();
        }
    }
    let mem_used_pct = match (extra.total_mb, extra.free_mb) {
        (Some(t), Some(f)) if t > 0 => Some((u64::from(t.saturating_sub(f)) * 100 / u64::from(t)) as u32),
        _ => None,
    };
    let issues = review_issues(&station, mem_used_pct, extra.errors, extra.print_jobs);
    Ok(StationReview { station, mem_used_pct, errors: extra.errors, print_jobs: extra.print_jobs, issues })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_script_parses() {
        let e = crate::ps::parse_errors(&format!("$target = 'pc1'\n{REVIEW_SCRIPT}"));
        assert!(e.is_empty(), "{e}");
    }

    fn st(remote: Option<Remote>) -> Station {
        Station { host: "PC-1".into(), online: true, remote, ..Default::default() }
    }

    #[test]
    fn review_orders_by_gravity_and_offers_fixes() {
        let r = Remote { free_gb: Some(3.1), total_gb: Some(120.0), boot_days: Some(23.4), ..Default::default() };
        let v = review_issues(&st(Some(r)), Some(60), Some(3), Some(4));
        assert_eq!(v[0].level, "bad");
        assert_eq!(v[0].fix, Some(StationAction::FreeSpace));
        assert!(v[0].text.contains("3,1 GB"));
        assert!(v.iter().any(|i| i.fix == Some(StationAction::ClearQueue) && i.text.starts_with("4 trabajos")));
        assert!(v.iter().any(|i| i.fix == Some(StationAction::Restart) && i.text.contains("23 días")));
    }

    #[test]
    fn review_of_a_healthy_pc_says_so() {
        let r = Remote { free_gb: Some(100.0), total_gb: Some(240.0), boot_days: Some(2.0), update_days: Some(5.0), ..Default::default() };
        let v = review_issues(&st(Some(r)), Some(40), Some(0), Some(0));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].level, "ok");
    }

    #[test]
    fn review_says_when_it_cannot_read() {
        let off = Station { host: "PC-2".into(), ..Default::default() };
        assert_eq!(review_issues(&off, None, None, None)[0].level, "bad");
        let mut s = st(None);
        s.remote_error = "Sin permiso".into();
        let v = review_issues(&s, None, None, None);
        assert!(v[0].text.contains("Sin permiso"));
        let pending = Remote { pending_reboot: Some(true), boot_days: Some(9.0), ..Default::default() };
        assert!(review_issues(&st(Some(pending)), None, None, None)[0].text.starts_with("Tiene un reinicio pendiente"));
    }

    #[test]
    fn checks_every_host_and_keeps_the_order() {
        let list: Vec<String> = ["127.0.0.1", "no-existe-xyz-1", "127.0.0.1"].iter().map(|s| s.to_string()).collect();
        let seen = AtomicUsize::new(0);
        let out = check_all(&list, false, &|_| {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(out.len(), 3);
        assert_eq!(seen.load(Ordering::SeqCst), 3);
        assert_eq!(out[1].host, "no-existe-xyz-1");
    }

    #[test]
    fn action_scripts_parse() {
        for a in [StationAction::Restart, StationAction::CancelRestart, StationAction::Gpupdate, StationAction::Message, StationAction::ClearQueue, StationAction::FreeSpace] {
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
        let w = warnings(&Remote { disk_health: "bad".into(), av_ok: Some(false), antivirus: "Avast".into(), bsods: Some(3), ..Default::default() });
        assert!(w.iter().any(|x| x.contains("disco está fallando")));
        assert!(w.iter().any(|x| x.contains("Avast")));
        assert!(w.iter().any(|x| x.contains("3 pantallazo")));
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
