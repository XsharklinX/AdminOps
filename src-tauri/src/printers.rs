//! Impresoras: estado, trabajos en cola, vaciar la cola, página de prueba,
//! predeterminada y quitar impresoras (las "fantasma" de equipos o drivers viejos).

use crate::ps::text_var;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Printer {
    name: String,
    driver: Option<String>,
    port: Option<String>,
    default: bool,
    network: bool,
    shared: bool,
    offline: bool,
    /// Código de Win32_Printer.PrinterStatus (3 = inactiva, 4 = imprimiendo, 7 = sin conexión…).
    status: u32,
    /// Estado de error detectado (atasco, sin papel, sin tóner…), si lo hay.
    error: Option<String>,
    jobs: u32,
    /// Impresoras de software: PDF, XPS, OneNote, Fax.
    #[serde(rename = "virtual")]
    is_virtual: bool,
}

const LIST_SCRIPT: &str = r#"
$errors = @{ 1='Otro error'; 2='Sin papel'; 3='Poco papel'; 4='Sin papel'; 5='Poco tóner'; 6='Sin tóner'; 7='Puerta abierta'; 8='Atasco de papel'; 9='Sin conexión'; 10='Requiere atención'; 11='Bandeja de salida llena' }
$r = @(Get-CimInstance Win32_Printer -ErrorAction Stop | ForEach-Object {
  $jobs = 0
  try { $jobs = @(Get-PrintJob -PrinterName $_.Name -ErrorAction Stop).Count } catch {}
  $err = [int]$_.DetectedErrorState
  [pscustomobject]@{
    name = $_.Name; driver = $_.DriverName; port = $_.PortName
    default = [bool]$_.Default; network = [bool]$_.Network; shared = [bool]$_.Shared
    offline = [bool]$_.WorkOffline -or [int]$_.PrinterStatus -eq 7
    status = [int]$_.PrinterStatus
    error = if ($err -gt 2 -or $err -eq 1) { $errors[$err] } else { $null }
    jobs = $jobs
    virtual = [bool]($_.PortName -match '^(PORTPROMPT:|nul:|SHRFAX:|XPSPort:)' -or $_.DriverName -match 'Microsoft (Print To PDF|XPS Document Writer)|OneNote|Fax')
  }
})
ConvertTo-Json -InputObject $r -Compress
"#;

#[tauri::command(async)]
pub fn list_printers() -> Result<Vec<Printer>, String> {
    let out = crate::ps::powershell_opts(LIST_SCRIPT, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
    let mut v: Vec<Printer> = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    v.sort_by_key(|p| (p.is_virtual, !p.default, p.name.to_lowercase()));
    Ok(v)
}

/// Ejecuta una acción sobre la impresora `name` (el nombre viaja en base64).
fn on_printer(name: &str, body: &str, title: &str, tweaks: &TweakState) -> Result<(), String> {
    if name.is_empty() || name.len() > 300 {
        return Err("Nombre de impresora no válido.".into());
    }
    let script = format!(
        "{}$p = Get-CimInstance Win32_Printer | Where-Object {{ $_.Name -eq $name }}\n\
         if (-not $p) {{ throw 'La impresora ya no existe. Actualiza la lista.' }}\n{body}\n'ok'",
        text_var("name", name)
    );
    let result = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None }).map(|_| ());
    tweaks.record(Op::Run, &format!("{title}: {name}"), &result);
    result
}

#[tauri::command(async)]
pub fn clear_printer_queue(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    on_printer(&name, "Get-PrintJob -PrinterName $name | Remove-PrintJob", "Vaciar la cola de impresión", &tweaks)
}

#[tauri::command(async)]
pub fn print_test_page(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    on_printer(
        &name,
        "$r = Invoke-CimMethod -InputObject $p -MethodName PrintTestPage\n\
         if ($r.ReturnValue -ne 0) { throw \"Windows no pudo enviar la página de prueba (código $($r.ReturnValue)).\" }",
        "Página de prueba",
        &tweaks,
    )
}

#[tauri::command(async)]
pub fn set_default_printer(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    // Windows 10/11 cambia solo la predeterminada si "Permitir que Windows administre" está activo.
    on_printer(
        &name,
        "Set-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows NT\\CurrentVersion\\Windows' -Name LegacyDefaultPrinterMode -Value 1 -Type DWord\n\
         $r = Invoke-CimMethod -InputObject $p -MethodName SetDefaultPrinter\n\
         if ($r.ReturnValue -ne 0) { throw \"Windows no pudo cambiar la predeterminada (código $($r.ReturnValue)).\" }",
        "Impresora predeterminada",
        &tweaks,
    )
}

#[tauri::command(async)]
pub fn remove_printer(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    on_printer(
        &name,
        "Get-PrintJob -PrinterName $name -ErrorAction SilentlyContinue | Remove-PrintJob -ErrorAction SilentlyContinue\nRemove-Printer -Name $name",
        "Quitar impresora",
        &tweaks,
    )
}

// ---------- Revisión de una impresora ----------

/// Por qué no imprime, en el orden en que hay que mirarlo.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrinterCheck {
    /// "ok" | "warn" | "bad"
    pub level: String,
    pub title: String,
    pub text: String,
    /// La cola de impresión de Windows está en marcha.
    pub spooler: bool,
    /// Dirección del puerto TCP/IP, si es de red.
    pub host: String,
    /// La impresora responde en la red (None: no es de red o no se pudo probar).
    pub reachable: Option<bool>,
    pub jobs: u32,
    /// El trabajo más viejo de la cola, en minutos.
    pub oldest_job_min: u32,
    /// Lo que la propia impresora cuenta por SNMP (consumibles, páginas, avisos).
    pub device: Option<crate::snmp::PrinterStatus>,
}

const CHECK_SCRIPT: &str = r#"
$p = Get-CimInstance Win32_Printer | Where-Object { $_.Name -eq $name }
if (-not $p) { throw 'La impresora ya no existe. Actualiza la lista.' }
$spool = (Get-Service -Name Spooler -ErrorAction SilentlyContinue).Status -eq 'Running'
$addr = ''
$port = Get-PrinterPort -Name $p.PortName -ErrorAction SilentlyContinue
if ($port -and $port.PrinterHostAddress) { $addr = "$($port.PrinterHostAddress)" }
elseif ("$($p.PortName)" -match '^(\d{1,3}(\.\d{1,3}){3})$') { $addr = $matches[1] }
elseif ("$($p.PortName)" -match '^IP_(.+)$') { $addr = $matches[1] }
$jobs = @()
try { $jobs = @(Get-PrintJob -PrinterName $p.Name -ErrorAction Stop) } catch {}
$oldest = 0
if ($jobs.Count) {
  $t = ($jobs | ForEach-Object { $_.SubmittedTime } | Sort-Object | Select-Object -First 1)
  if ($t) { $oldest = [int]((New-TimeSpan -Start $t -End (Get-Date)).TotalMinutes) }
}
[pscustomobject]@{
  spooler = [bool]$spool; host = $addr; jobs = [int]$jobs.Count; oldestJobMin = [int]$oldest
  offline = [bool]($p.WorkOffline -or [int]$p.PrinterStatus -eq 7); error = [int]$p.DetectedErrorState
} | ConvertTo-Json -Compress
"#;

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct CheckRaw {
    spooler: bool,
    host: String,
    jobs: u32,
    oldest_job_min: u32,
    offline: bool,
    error: u32,
}

/// Puertos por los que habla una impresora de red: RAW, IPP y LPD.
const PRINT_PORTS: &[u16] = &[9100, 631, 515];

/// ¿Responde algo en esa dirección por alguno de los puertos de impresión?
fn printer_reachable(host: &str) -> Option<bool> {
    let ip: std::net::IpAddr = host.parse().ok()?;
    Some(open_print_ports(ip).next().is_some())
}

/// Puertos de impresión abiertos en esa dirección.
fn open_print_ports(ip: std::net::IpAddr) -> impl Iterator<Item = u16> {
    PRINT_PORTS
        .iter()
        .copied()
        .filter(move |p| std::net::TcpStream::connect_timeout(&std::net::SocketAddr::new(ip, *p), Duration::from_millis(500)).is_ok())
}

/// Dónde se corta la cadena: cola de Windows → impresora encendida y en red →
/// estado del aparato → atascos. Igual que en la red, se señala el primer
/// eslabón roto y no el último síntoma.
fn printer_verdict(r: &CheckRaw, reachable: Option<bool>, device: Option<&crate::snmp::PrinterStatus>) -> (String, String, String) {
    const ERRORES: &[(u32, &str)] = &[
        (2, "se ha quedado sin papel"),
        (4, "se ha quedado sin papel"),
        (6, "se ha quedado sin tóner o tinta"),
        (7, "tiene una puerta o tapa abierta"),
        (8, "tiene un atasco de papel"),
        (11, "tiene la bandeja de salida llena"),
    ];
    if !r.spooler {
        return (
            "bad".into(),
            "La cola de impresión de Windows está parada".into(),
            "Sin ese servicio no imprime ninguna impresora del equipo, no solo esta. Se arranca desde Solucionar problemas → Reparaciones.".into(),
        );
    }
    if reachable == Some(false) {
        return (
            "bad".into(),
            format!("La impresora no responde en {}", r.host),
            "El equipo llega a la red pero esa dirección no contesta por ningún puerto de impresión. Suele ser que está apagada, en reposo profundo, o que le cambiaron la dirección: si se la da el router, conviene fijársela.".into(),
        );
    }
    // Lo que dice el propio aparato por SNMP es más preciso que lo que sabe Windows.
    let alertas: Vec<&String> = device.map(|d| d.alerts.iter().filter(|a| blocking_alert(a)).collect()).unwrap_or_default();
    if let Some(primera) = alertas.first() {
        let resto = if alertas.len() > 1 { format!(" (y además: {})", alertas[1..].iter().map(|a| a.to_lowercase()).collect::<Vec<_>>().join(", ")) } else { String::new() };
        return (
            "warn".into(),
            format!("La impresora avisa: {}{resto}", primera.to_lowercase()),
            "Lo dice el propio aparato. Hay que resolverlo allí; hasta entonces los trabajos se quedan esperando en la cola.".into(),
        );
    }
    if let Some((_, que)) = ERRORES.iter().find(|(c, _)| *c == r.error) {
        return ("warn".into(), format!("La impresora {que}"), "Hay que resolverlo en el propio aparato; hasta entonces los trabajos se quedan esperando en la cola.".into());
    }
    if r.offline {
        return (
            "warn".into(),
            "Windows la tiene marcada como «sin conexión»".into(),
            "El aparato responde, así que casi siempre basta con quitarle esa marca (clic derecho en la impresora → «Usar impresora en línea»). Si se vuelve a marcar sola, revisa el cable o la Wi-Fi de la impresora.".into(),
        );
    }
    if r.jobs > 0 && r.oldest_job_min >= 10 {
        return (
            "warn".into(),
            format!("Hay {} trabajo(s) atascados en la cola", r.jobs),
            format!("El más antiguo lleva {} minutos esperando. Vacía la cola y vuelve a imprimir; si se repite a menudo, suele ser el driver.", r.oldest_job_min),
        );
    }
    // Consumible casi acabado: aún imprime, pero es la próxima llamada.
    if let Some(s) = device.and_then(lowest_supply).filter(|s| s.percent.is_some_and(|p| p <= LOW_SUPPLY)) {
        return (
            "warn".into(),
            format!("{} al {} %", capitalize(&s.name), s.percent.unwrap_or(0)),
            "La impresora funciona, pero conviene tener el recambio a mano o pedirlo ya.".into(),
        );
    }
    let donde = if r.host.is_empty() { "conectada a este equipo".to_string() } else { format!("respondiendo en {}", r.host) };
    let nivel = device.and_then(lowest_supply).map(|s| format!(" El consumible más bajo: {} al {} %.", s.name.to_lowercase(), s.percent.unwrap_or(0))).unwrap_or_default();
    (
        "ok".into(),
        "La impresora está lista".into(),
        format!("Cola de Windows en marcha e impresora {donde}.{nivel} Si aun así no sale nada, imprime la página de prueba para descartar el driver."),
    )
}

/// A partir de aquí un consumible se da por casi acabado.
const LOW_SUPPLY: u8 = 10;

/// Avisos del aparato que impiden imprimir (no los de «queda poco»).
fn blocking_alert(a: &str) -> bool {
    !(a.starts_with("Queda poco") || a.contains("casi llena") || a.contains("mantenimiento"))
}

/// El tóner o tinta con menos nivel conocido.
fn lowest_supply(d: &crate::snmp::PrinterStatus) -> Option<&crate::snmp::Supply> {
    d.supplies.iter().filter(|s| s.consumable && s.percent.is_some()).min_by_key(|s| s.percent)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Revisa una impresora y dice qué le pasa.
#[tauri::command(async)]
pub fn check_printer(name: String) -> Result<PrinterCheck, String> {
    if name.is_empty() || name.len() > 300 {
        return Err("Nombre de impresora no válido.".into());
    }
    let script = format!("{}{CHECK_SCRIPT}", text_var("name", &name));
    let out = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
    let r: CheckRaw = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let reachable = if r.host.is_empty() { None } else { printer_reachable(&r.host) };
    // Solo se pregunta por SNMP si la impresora está en la red y contesta.
    let device = match (reachable, r.host.parse::<std::net::IpAddr>()) {
        (Some(true), Ok(ip)) => Some(crate::snmp::printer_status(ip)).filter(|d| d.reachable),
        _ => None,
    };
    let (level, title, text) = printer_verdict(&r, reachable, device.as_ref());
    Ok(PrinterCheck { level, title, text, spooler: r.spooler, host: r.host, reachable, jobs: r.jobs, oldest_job_min: r.oldest_job_min, device })
}

// ---------- Impresoras de la red ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoundPrinter {
    pub ip: String,
    /// Puertos de impresión abiertos (9100 RAW, 631 IPP, 515 LPD).
    pub ports: Vec<u16>,
    /// Ya está instalada en este equipo.
    pub installed: bool,
    /// Nombre con el que se anuncia (mDNS o SNMP); vacío si no lo dice.
    pub name: String,
    pub model: String,
    /// Cómo se encontró: "mdns", "wsd" o "ports".
    pub via: String,
}

/// Direcciones que ya responden en esta red, según Windows. Se busca solo entre
/// estas y no por todo el rango: es mucho más rápido y no se molesta a equipos
/// que no han dicho nada.
const NEIGHBOURS: &str = r#"
@(Get-NetNeighbor -AddressFamily IPv4 -ErrorAction SilentlyContinue |
  Where-Object { $_.State -in 'Reachable','Stale','Permanent' -and "$($_.IPAddress)" -notmatch '^(169\.254|22[4-9]\.|23[0-9]\.|255\.)' } |
  ForEach-Object { "$($_.IPAddress)" } | Sort-Object -Unique) -join ','
"#;

/// Busca impresoras en la red que este equipo aún no tiene instaladas.
///
/// Hasta ahora no había forma de saber qué impresoras hay en la oficina sin
/// preguntar o ir mirando aparato por aparato.
#[tauri::command(async)]
pub fn find_network_printers() -> Result<Vec<FoundPrinter>, String> {
    // Mientras Windows dice qué vecinos conoce, se pregunta en la red quién es impresora.
    let anuncios = std::thread::spawn(|| crate::discovery::discover(Duration::from_millis(2500)));
    let out = crate::pspool::query(NEIGHBOURS, Some(Duration::from_secs(30)), "Buscar impresoras en la red")?;
    let anuncios = anuncios.join().unwrap_or_default();
    let mut ips: Vec<std::net::IpAddr> = out.trim().split(',').filter_map(|s| s.trim().parse().ok()).collect();
    ips.extend(anuncios.keys().filter(|ip| ip.is_ipv4()).copied());
    ips.sort();
    ips.dedup();
    if ips.is_empty() {
        return Ok(Vec::new());
    }
    // Los puertos ya instalados, para marcar las que no son novedad.
    let instaladas: Vec<String> = list_printers().unwrap_or_default().into_iter().filter_map(|p| p.port).collect();
    let anuncios = &anuncios;
    let mut found: Vec<FoundPrinter> = std::thread::scope(|s| {
        let hilos: Vec<_> = ips
            .iter()
            .map(|&addr| {
                s.spawn(move || {
                    let ports: Vec<u16> = open_print_ports(addr).collect();
                    let anuncio = anuncios.get(&addr);
                    // Lo que se anunció como impresora cuenta aunque tenga los puertos cerrados.
                    if ports.is_empty() && anuncio.is_none() {
                        return None;
                    }
                    let mut p = FoundPrinter {
                        ip: addr.to_string(),
                        ports,
                        installed: false,
                        name: anuncio.map(|a| a.name.clone()).unwrap_or_default(),
                        model: anuncio.map(|a| a.model.clone()).unwrap_or_default(),
                        via: anuncio.map_or("ports", |a| a.via).to_string(),
                    };
                    if p.name.is_empty() || p.model.is_empty() {
                        if let Some((name, model)) = crate::snmp::printer_identity(addr) {
                            if p.name.is_empty() {
                                p.name = name;
                            }
                            if p.model.is_empty() {
                                p.model = model;
                            }
                        }
                    }
                    Some(p)
                })
            })
            .collect();
        hilos.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    });
    for f in &mut found {
        f.installed = instaladas.iter().any(|p| p.contains(&f.ip));
    }
    found.sort_by(|a, b| a.installed.cmp(&b.installed).then(a.ip.cmp(&b.ip)));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura: lista las impresoras reales.
    #[test]
    fn lists_real_printers() {
        let v = super::list_printers().unwrap();
        println!("{v:#?}");
        assert!(v.iter().any(|p| p.is_virtual), "Microsoft Print to PDF suele existir siempre");
    }

    #[test]
    fn embedded_scripts_parse() {
        for (nombre, script) in [("LIST_SCRIPT", super::LIST_SCRIPT), ("CHECK_SCRIPT", super::CHECK_SCRIPT), ("NEIGHBOURS", super::NEIGHBOURS)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{nombre}: {errors}");
        }
    }

    /// La revisión señala el primer eslabón roto, no el último síntoma.
    #[test]
    fn printer_verdict_points_at_the_broken_link() {
        let base = CheckRaw { spooler: true, host: "192.168.1.30".into(), ..Default::default() };
        assert_eq!(printer_verdict(&base, Some(true), None).0, "ok");

        // Sin cola de impresión no se mira nada más: no imprime ninguna.
        let (level, title, _) = printer_verdict(&CheckRaw { spooler: false, ..base.clone() }, Some(true), None);
        assert_eq!(level, "bad");
        assert!(title.contains("cola"), "{title}");

        // No responde: se dice con la dirección, que es lo accionable.
        let (level, title, _) = printer_verdict(&base, Some(false), None);
        assert_eq!(level, "bad");
        assert!(title.contains("192.168.1.30"), "{title}");

        // Responde, pero Windows la marcó sin conexión.
        let (_, title, _) = printer_verdict(&CheckRaw { offline: true, ..base.clone() }, Some(true), None);
        assert!(title.contains("sin conexión"), "{title}");

        // Un error del aparato manda sobre la cola.
        let (_, title, _) = printer_verdict(&CheckRaw { error: 8, jobs: 5, oldest_job_min: 40, ..base.clone() }, Some(true), None);
        assert!(title.contains("atasco"), "{title}");

        // Una cola con trabajos recientes es imprimir normal, no un atasco.
        assert_eq!(printer_verdict(&CheckRaw { jobs: 2, oldest_job_min: 2, ..base.clone() }, Some(true), None).0, "ok");
        let (_, title, _) = printer_verdict(&CheckRaw { jobs: 2, oldest_job_min: 30, ..base.clone() }, Some(true), None);
        assert!(title.contains("atascados"), "{title}");
    }

    /// Lo que cuenta la impresora por SNMP: un atasco manda; el tóner bajo
    /// solo se dice si no hay nada peor; «queda poco papel» no bloquea.
    #[test]
    fn printer_verdict_uses_what_the_device_says() {
        use crate::snmp::{PrinterStatus, Supply};
        let base = CheckRaw { spooler: true, host: "192.168.1.30".into(), ..Default::default() };
        let toner = |p| Supply { name: "Tóner negro".into(), percent: Some(p), consumable: true };
        let fusor = Supply { name: "Fusor".into(), percent: Some(2), consumable: false };
        let d = PrinterStatus { reachable: true, supplies: vec![toner(8), fusor.clone()], ..Default::default() };

        let (level, title, _) = printer_verdict(&base, Some(true), Some(&d));
        assert_eq!((level.as_str(), title.as_str()), ("warn", "Tóner negro al 8 %"));

        let atasco = PrinterStatus { alerts: vec!["Atasco de papel".into(), "Puerta o tapa abierta".into()], ..d.clone() };
        let (_, title, _) = printer_verdict(&base, Some(true), Some(&atasco));
        assert!(title.contains("atasco de papel") && title.contains("puerta"), "{title}");

        let poco_papel = PrinterStatus { alerts: vec!["Queda poco papel".into()], supplies: vec![toner(60), fusor], ..d.clone() };
        let (level, _, text) = printer_verdict(&base, Some(true), Some(&poco_papel));
        assert_eq!(level, "ok");
        assert!(text.contains("tóner negro al 60 %"), "{text}");

        // Sin cola de Windows, eso sigue siendo lo primero.
        assert!(printer_verdict(&CheckRaw { spooler: false, ..base }, Some(true), Some(&atasco)).1.contains("cola"));
    }
}
