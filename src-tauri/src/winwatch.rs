//! Vigilancia de errores de Windows mientras AdminOps está abierta: cada minuto
//! mira el Visor de eventos (y el espacio libre) y, si aparece un error típico,
//! avisa explicando qué es y qué hacer. Sin jerga: el técnico se lo puede
//! contar tal cual al cliente.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};

const POLL: Duration = Duration::from_secs(60);
/// La primera vez se miran las últimas 24 h (pantallazos o apagados antes de abrir AdminOps).
const FIRST_LOOKBACK: u64 = 24 * 3600;
const MAX_ALERTS: usize = 200;

static FILE_LOCK: Mutex<()> = Mutex::new(());

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Alert {
    pub id: String,
    /// Última vez que ocurrió (segundos Unix).
    pub time: u64,
    /// Para agrupar repeticiones: tipo + sujeto (programa, servicio, disco…).
    pub key: String,
    /// bad | warn | info
    pub level: String,
    pub title: String,
    /// Dato concreto: programa, código de error, disco…
    pub detail: String,
    /// Qué significa, en lenguaje claro.
    pub explanation: String,
    /// Qué hacer.
    pub advice: String,
    /// Página de AdminOps donde se arregla o se ve más.
    pub page: Option<String>,
    pub count: u32,
    pub read: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RawEvent {
    provider: String,
    id: u32,
    time: String,
    #[serde(default)]
    props: Vec<String>,
    #[serde(default)]
    message: String,
    /// Modelo del disco en los eventos de disco (la ruta interna no le dice nada a nadie).
    #[serde(default)]
    disk: String,
}

/// Qué se vigila: (log, proveedor, ids).
const SOURCES: &[(&str, &str, &[u32])] = &[
    ("System", "Microsoft-Windows-WER-SystemErrorReporting", &[1001]),
    ("System", "Microsoft-Windows-Kernel-Power", &[41]),
    ("System", "disk", &[7, 51, 153]),
    ("System", "Ntfs", &[55, 98]),
    ("System", "Microsoft-Windows-Ntfs", &[55, 98]),
    ("System", "Microsoft-Windows-WHEA-Logger", &[1, 17, 18, 19, 47]),
    ("System", "Display", &[4101]),
    ("System", "Tcpip", &[4199]),
    ("System", "Service Control Manager", &[7031, 7034]),
    ("System", "Microsoft-Windows-Resource-Exhaustion-Detector", &[2004]),
    ("System", "Microsoft-Windows-WindowsUpdateClient", &[20]),
    ("System", "Microsoft-Windows-Kernel-Processor-Power", &[37]),
    ("Application", "Application Error", &[1000]),
    ("Application", "Application Hang", &[1002]),
    ("Microsoft-Windows-Windows Defender/Operational", "Microsoft-Windows-Windows Defender", &[1116, 1117]),
];

fn script(since_iso: &str) -> String {
    let filters: Vec<String> = SOURCES
        .iter()
        .map(|(log, prov, ids)| format!("@{{ LogName = '{log}'; ProviderName = '{prov}'; Id = {}; StartTime = $since }}", ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")))
        .collect();
    format!(
        "{}$since = [datetime]::Parse($sinceText, $null, 'RoundtripKind').ToLocalTime()\n\
         $out = foreach ($f in @({})) {{ try {{ Get-WinEvent -FilterHashtable $f -MaxEvents 20 -ErrorAction Stop }} catch {{ }} }}\n\
         $r = @($out | Sort-Object TimeCreated | ForEach-Object {{\n\
           $m = \"$($_.Message)\"; $first = ($m -split \"`n\" | Where-Object {{ $_.Trim() }} | Select-Object -First 1)\n\
           [pscustomobject]@{{ provider = \"$($_.ProviderName)\"; id = [int]$_.Id; time = $_.TimeCreated.ToUniversalTime().ToString('o')\n\
             props = @($_.Properties | Select-Object -First 8 | ForEach-Object {{ \"$($_.Value)\" }}); message = \"$first\".Trim()\n\
             disk = if ($_.ProviderName -eq 'disk' -and \"$($_.Properties[0].Value)\" -match 'Harddisk(\\d+)') {{ \"$((Get-Disk -Number ([int]$Matches[1]) -ErrorAction SilentlyContinue).FriendlyName)\" }} else {{ '' }} }}\n\
         }})\n\
         ConvertTo-Json -InputObject $r -Depth 3 -Compress",
        crate::ps::text_var("sinceText", since_iso),
        filters.join(", ")
    )
}

fn prop(e: &RawEvent, i: usize) -> String {
    e.props.get(i).map(|s| s.trim().chars().take(120).collect()).unwrap_or_default()
}

fn find_hex(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    let i = lower.find("0x")?;
    let hex: String = lower[i + 2..].chars().take_while(|c| c.is_ascii_hexdigit()).take(8).collect();
    (!hex.is_empty()).then(|| format!("0x{}", hex.to_uppercase()))
}

fn disk_name(e: &RawEvent) -> String {
    let n = e.disk.trim();
    if n.is_empty() {
        prop(e, 0)
    } else {
        format!("Disco {n}")
    }
}

/// Traduce un evento a un aviso (None si no merece aviso).
fn to_alert(e: &RawEvent) -> Option<Alert> {
    let a = |level: &str, key: String, title: String, detail: String, explanation: &str, advice: &str, page: Option<&str>| Alert {
        level: level.into(),
        key,
        title,
        detail,
        explanation: explanation.into(),
        advice: advice.into(),
        page: page.map(String::from),
        count: 1,
        ..Default::default()
    };
    let p = e.provider.as_str();
    Some(match (p, e.id) {
        ("Microsoft-Windows-WER-SystemErrorReporting", 1001) => {
            let code = find_hex(&prop(e, 0)).or_else(|| find_hex(&e.message)).unwrap_or_default();
            a(
                "bad",
                "bsod".into(),
                "El equipo tuvo un pantallazo azul".into(),
                if code.is_empty() { String::new() } else { format!("Código {code}") },
                "Windows se detuvo por un error grave y se reinició. Suele deberse a un driver defectuoso, la memoria RAM, el disco o sobrecalentamiento.",
                "Abre Diagnóstico → Estabilidad: AdminOps lee el volcado del pantallazo e indica el driver o la causa probable.",
                Some("diagnostics"),
            )
        }
        ("Microsoft-Windows-Kernel-Power", 41) => a(
            "warn",
            "power41".into(),
            "El equipo se apagó sin cerrar Windows".into(),
            String::new(),
            "Se reinició o se apagó de golpe: corte de luz, botón de encendido mantenido, fuente de alimentación que falla o sobrecalentamiento.",
            "Si se repite sin cortes de luz, revisa temperaturas (Hardware) y la fuente de alimentación. Un SAI/UPS evita los cortes.",
            Some("hardware"),
        ),
        ("disk", 7) => a(
            "bad",
            format!("disk7:{}", prop(e, 0)),
            "Un disco tiene sectores dañados".into(),
            disk_name(e),
            "El disco no pudo leer o escribir una zona de su superficie. Es síntoma de que se está estropeando y puede perder datos.",
            "Haz una copia de seguridad cuanto antes y revisa la salud del disco en Hardware (SMART). Plantéate cambiarlo.",
            Some("hardware"),
        ),
        ("disk", 51) | ("disk", 153) => a(
            "warn",
            format!("disk-io:{}", prop(e, 0)),
            "Un disco tarda en responder o da errores de lectura".into(),
            disk_name(e),
            "Windows tuvo que repetir operaciones con el disco. Causas habituales: cable SATA flojo, disco que empieza a fallar o USB externo con poca alimentación.",
            "Revisa la salud del disco en Hardware y, en un equipo de sobremesa, el cable SATA. Si es un USB, conéctalo directo al equipo.",
            Some("hardware"),
        ),
        ("Ntfs" | "Microsoft-Windows-Ntfs", 55 | 98) => a(
            "warn",
            "ntfs".into(),
            "El sistema de archivos tiene errores".into(),
            String::new(),
            "La estructura de archivos de una unidad está dañada (a menudo tras un apagado brusco). Puede haber archivos que no se abren.",
            "Ejecuta la comprobación de disco (Reparaciones → Comprobar disco) y reinicia cuando lo pida.",
            Some("repair"),
        ),
        ("Microsoft-Windows-WHEA-Logger", id) => a(
            if id == 18 || id == 1 { "bad" } else { "warn" },
            format!("whea:{id}"),
            if id == 18 || id == 1 { "Error grave de hardware".into() } else { "Error de hardware corregido".into() },
            String::new(),
            "El procesador, la memoria o una tarjeta PCIe informó de un error. Los corregidos no son graves si son aislados; si se repiten, anticipan cuelgues y pantallazos.",
            "Revisa temperaturas, quita overclock o XMP si lo hay y pasa la prueba de memoria (Hardware).",
            Some("hardware"),
        ),
        ("Display", 4101) => a(
            "warn",
            "display4101".into(),
            "El driver de la tarjeta gráfica dejó de responder".into(),
            prop(e, 0),
            "La pantalla se quedó negra o parpadeó un momento y Windows reinició el driver de vídeo. Suele ser un driver desactualizado o la gráfica calentándose.",
            "Actualiza el driver de la gráfica desde la web del fabricante y revisa su temperatura en Hardware.",
            Some("hardware"),
        ),
        ("Tcpip", 4199) => a(
            "warn",
            "ipconflict".into(),
            "Otro dispositivo usa la misma IP que este equipo".into(),
            e.message.split_whitespace().find(|w| w.chars().filter(|c| *c == '.').count() == 3).unwrap_or("").trim_end_matches(['.', ',']).to_string(),
            "Dos equipos con la misma dirección IP se quitan la conexión el uno al otro: cortes de red intermitentes.",
            "Deja que el router asigne las IP (DHCP) o reserva una IP fija por equipo en el router. En Dispositivos en la red se ve cuál es el otro.",
            Some("devices"),
        ),
        ("Service Control Manager", 7031 | 7034) => {
            let service = prop(e, 0);
            a(
                "info",
                format!("svc:{service}"),
                format!("El servicio «{service}» se detuvo inesperadamente"),
                service.clone(),
                "Un servicio de Windows o de un programa se cerró por un error. Si es de un programa concreto, ese programa puede fallar.",
                "Si se repite, reinstala el programa al que pertenece o revísalo en Optimizar → Servicios.",
                Some("services"),
            )
        }
        ("Microsoft-Windows-Resource-Exhaustion-Detector", 2004) => a(
            "bad",
            "lowmem".into(),
            "Windows se quedó sin memoria".into(),
            String::new(),
            "La memoria (RAM y archivo de paginación) se agotó: los programas se cuelgan o se cierran solos. Puede ser un programa con una fuga de memoria.",
            "Mira en Procesos qué consume más memoria. Si pasa a menudo, amplía la RAM o revisa el tamaño del archivo de paginación.",
            Some("processes"),
        ),
        ("Microsoft-Windows-WindowsUpdateClient", 20) => {
            let code = find_hex(&prop(e, 0)).unwrap_or_else(|| prop(e, 0));
            a(
                "warn",
                format!("wu:{code}"),
                "Windows Update no pudo instalar una actualización".into(),
                [code, prop(e, 1)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "),
                "Una actualización falló. A veces se reintenta sola; si se repite, deja el equipo sin parches de seguridad.",
                "En Programas → Windows Update verás el error explicado. Reparaciones → Reparar Windows Update suele arreglarlo.",
                Some("winupdate"),
            )
        }
        ("Microsoft-Windows-Kernel-Processor-Power", 37) => a(
            "warn",
            "throttle".into(),
            "El procesador está funcionando más lento por temperatura o energía".into(),
            String::new(),
            "El firmware limitó la velocidad del procesador, normalmente por calor (ventilador sucio, pasta térmica seca) o por un cargador de poca potencia en portátiles.",
            "Revisa temperaturas en Hardware, limpia el polvo y usa el cargador original.",
            Some("hardware"),
        ),
        ("Application Error", 1000) => {
            let app = prop(e, 0);
            if app.is_empty() {
                return None;
            }
            a(
                "info",
                format!("crash:{}", app.to_lowercase()),
                format!("{app} se cerró por un error"),
                [format!("Módulo: {}", prop(e, 3))].into_iter().filter(|s| s.len() > 8).collect::<Vec<_>>().join(""),
                "El programa falló y Windows lo cerró. Si el módulo es del propio programa, suele ser un fallo suyo; si es de Windows o de un driver, puede ser otra causa.",
                "Actualiza o reinstala el programa. Si se repite con varios programas, pasa un Diagnóstico.",
                Some("diagnostics"),
            )
        }
        ("Application Hang", 1002) => {
            let app = prop(e, 0);
            if app.is_empty() {
                return None;
            }
            a(
                "info",
                format!("hang:{}", app.to_lowercase()),
                format!("{app} dejó de responder"),
                String::new(),
                "El programa se quedó colgado y hubo que cerrarlo. Suele pasar por falta de memoria, disco lento o un fallo del programa.",
                "Si se repite, actualiza el programa y revisa en Procesos si algo consume demasiado.",
                Some("processes"),
            )
        }
        ("Microsoft-Windows-Windows Defender", 1116 | 1117) => {
            let threat = e.props.get(7).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| prop(e, 7));
            a(
                "bad",
                format!("defender:{}", threat.to_lowercase()),
                if e.id == 1116 { "Windows Defender detectó una amenaza".into() } else { "Windows Defender actuó contra una amenaza".into() },
                threat,
                "El antivirus encontró software malicioso o no deseado. Si ya actuó, suele estar en cuarentena.",
                "Abre Seguridad para revisar el estado y pasa un análisis completo. Revisa también los elementos sospechosos.",
                Some("security"),
            )
        }
        _ => return None,
    })
}

// ---------- Estado y avisos guardados ----------

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Store {
    /// Hasta cuándo se ha mirado (ISO, UTC).
    since: String,
    alerts: Vec<Alert>,
    /// Dispositivos con error en la última comprobación (id:código): solo se avisa de los nuevos.
    bad_devices: Vec<String>,
}

fn store_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("windows-alerts.json")
}

/// Añade avisos agrupando repeticiones (misma clave en las últimas 6 h). Devuelve los nuevos.
fn merge(store: &mut Store, fresh: Vec<Alert>) -> Vec<Alert> {
    let mut new = Vec::new();
    for mut a in fresh {
        if let Some(existing) = store.alerts.iter_mut().find(|x| x.key == a.key && a.time.saturating_sub(x.time) < 6 * 3600) {
            existing.count += 1;
            existing.time = existing.time.max(a.time);
            existing.read = false;
            continue;
        }
        a.id = format!("{:x}-{}", a.time, store.alerts.len());
        store.alerts.insert(0, a.clone());
        new.push(a);
    }
    store.alerts.truncate(MAX_ALERTS);
    new
}

fn check_disk_space(store: &Store) -> Option<Alert> {
    let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let d = disks.iter().find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with(&sys.to_uppercase()))?;
    let free = d.available_space();
    let total = d.total_space().max(1);
    let low = free < 5 * 1024 * 1024 * 1024 || free * 100 / total < 5;
    // Como mucho un aviso al día.
    let recent = store.alerts.iter().any(|a| a.key == "lowdisk" && now().saturating_sub(a.time) < 86_400);
    (low && !recent).then(|| Alert {
        level: "bad".into(),
        key: "lowdisk".into(),
        time: now(),
        title: format!("Queda muy poco espacio en {sys}"),
        detail: format!("{:.1} GB libres", free as f64 / 1024f64.powi(3)),
        explanation: "Sin espacio, Windows no puede actualizarse, los programas fallan al guardar y el equipo se vuelve muy lento.".into(),
        advice: "Usa Optimizar → Limpieza y Equipo → Espacio en disco para ver qué ocupa más.".into(),
        page: Some("space".into()),
        count: 1,
        ..Default::default()
    })
}

// ---------- Dispositivos que dejan de funcionar ----------

/// Cada cuánto se miran los dispositivos (la consulta es más pesada que la de eventos).
const DEVICE_EVERY: Duration = Duration::from_secs(5 * 60);
static LAST_DEVICES: Mutex<Option<std::time::Instant>> = Mutex::new(None);

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct BadDevice {
    id: String,
    name: String,
    #[serde(default)]
    class: String,
    code: u32,
}

/// Dispositivos presentes con error (sin los deshabilitados a propósito ni los desconectados).
const DEVICES_SCRIPT: &str = r#"
$r = @(Get-CimInstance Win32_PnPEntity -Filter "ConfigManagerErrorCode <> 0" -ErrorAction SilentlyContinue | Where-Object { @(22, 24, 45) -notcontains [int]$_.ConfigManagerErrorCode } | ForEach-Object { [pscustomobject]@{ id = "$($_.PNPDeviceID)"; name = "$($_.Name)"; class = "$($_.PNPClass)"; code = [int]$_.ConfigManagerErrorCode } })
ConvertTo-Json -InputObject $r -Depth 3 -Compress
"#;

fn device_alert(d: &BadDevice) -> Alert {
    let name = if d.name.trim().is_empty() { "Dispositivo desconocido".to_string() } else { d.name.trim().to_string() };
    let lower = name.to_lowercase();
    let wireless = lower.contains("wireless") || lower.contains("wi-fi") || lower.contains("wifi") || lower.contains("wlan") || lower.contains("802.11");
    let (what, advice, page) = match d.class.as_str() {
        "Net" if wireless => ("La tarjeta Wi-Fi", "Pulsa «Reiniciar la tarjeta» en Red → Velocidad y diagnóstico. Si vuelve a pasar, actualiza su driver desde la web del fabricante.", "network"),
        "Net" => ("La tarjeta de red", "Prueba «Solucionar problemas → No hay Internet». Si vuelve a pasar, actualiza su driver.", "troubleshoot"),
        "Bluetooth" => ("El Bluetooth", "Prueba «Solucionar problemas → Bluetooth», que reinicia el adaptador.", "troubleshoot"),
        "MEDIA" | "AudioEndpoint" => ("El sonido", "Prueba «Solucionar problemas → No suena», que reinicia el audio.", "troubleshoot"),
        "Display" => ("La tarjeta gráfica", "Instala el driver de la gráfica del fabricante y prueba «Solucionar problemas → Pantalla».", "troubleshoot"),
        "Camera" | "Image" => ("La cámara", "Reinstala su driver o comprueba que no esté bloqueada en la BIOS o con un botón del portátil.", "diagnostics"),
        "USB" | "HIDClass" | "Keyboard" | "Mouse" => ("Un dispositivo USB", "Desconéctalo y conéctalo en otro puerto. Si es un concentrador, conéctalo directo al equipo.", "diagnostics"),
        "Printer" | "PrintQueue" => ("Una impresora", "Prueba «Solucionar problemas → Impresora».", "troubleshoot"),
        _ => ("Un dispositivo", "Reinstala o actualiza su driver desde la web del fabricante. Diagnóstico → Drivers muestra todos los dispositivos con error.", "diagnostics"),
    };
    Alert {
        level: if d.code == 28 { "warn" } else { "bad" }.into(),
        key: format!("dev:{}:{}", d.id, d.code),
        time: now(),
        title: format!("{what} no funciona"),
        detail: format!("{name} · código {}", d.code),
        explanation: crate::network::wifictl::problem_text(d.code).into(),
        advice: advice.into(),
        page: Some(page.into()),
        count: 1,
        ..Default::default()
    }
}

/// Avisa de los dispositivos que han empezado a fallar desde la última comprobación.
fn check_devices(store: &mut Store) -> Vec<Alert> {
    {
        let mut last = LAST_DEVICES.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|t| t.elapsed() < DEVICE_EVERY) {
            return vec![];
        }
        *last = Some(std::time::Instant::now());
    }
    let Ok(out) = crate::pspool::query(DEVICES_SCRIPT, Some(Duration::from_secs(40)), "Vigilancia de dispositivos") else { return vec![] };
    let devices: Vec<BadDevice> = serde_json::from_str(out.trim()).unwrap_or_default();
    new_bad_devices(store, &devices).iter().map(device_alert).collect()
}

fn new_bad_devices(store: &mut Store, devices: &[BadDevice]) -> Vec<BadDevice> {
    let keys: Vec<String> = devices.iter().map(|d| format!("{}:{}", d.id, d.code)).collect();
    let fresh = devices.iter().zip(&keys).filter(|(_, k)| !store.bad_devices.contains(k)).map(|(d, _)| d.clone()).collect();
    store.bad_devices = keys;
    fresh
}

fn notify(app: &tauri::AppHandle, a: &Alert) {
    use tauri_plugin_notification::NotificationExt;
    let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    if focused {
        return;
    }
    let body = if a.detail.is_empty() { a.explanation.clone() } else { format!("{} · {}", a.detail, a.explanation) };
    let _ = app.notification().builder().title(&a.title).body(body.chars().take(240).collect::<String>()).show();
    // Windows no avisa de si se pulsa el aviso, pero al pulsarlo pone AdminOps
    // delante: la interfaz abre entonces este aviso (ver App.tsx).
    use tauri::Emitter;
    let _ = app.emit("alert-notified", &a.key);
}

/// Una pasada: eventos nuevos desde la última vez y espacio libre.
fn poll(app: &tauri::AppHandle) -> Result<Vec<Alert>, String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store: Store = crate::paths::read_json(&store_path(app));
    let started = chrono::Utc::now();
    let since = if store.since.is_empty() { (started - chrono::Duration::seconds(FIRST_LOOKBACK as i64)).to_rfc3339() } else { store.since.clone() };
    let out = crate::pspool::query(&script(&since), Some(Duration::from_secs(45)), "Vigilancia de errores de Windows")?;
    let events: Vec<RawEvent> = serde_json::from_str(out.trim()).unwrap_or_default();
    let mut fresh: Vec<Alert> = events
        .iter()
        .filter_map(|e| {
            let mut a = to_alert(e)?;
            a.time = chrono::DateTime::parse_from_rfc3339(&e.time).map(|t| t.timestamp() as u64).unwrap_or_else(|_| now());
            Some(a)
        })
        .collect();
    fresh.extend(check_disk_space(&store));
    fresh.extend(check_devices(&mut store));
    let new = merge(&mut store, fresh);
    store.since = started.to_rfc3339();
    crate::paths::write_json(&store_path(app), &store)?;
    Ok(new)
}

/// ¿Puede AdminOps usar PowerShell con normalidad en este equipo? (antivirus o
/// directivas de empresa). Un aviso claro al arrancar en vez de fallos sueltos.
fn environment_check(app: &tauri::AppHandle) {
    let problem = match crate::pspool::query("$ExecutionContext.SessionState.LanguageMode", Some(Duration::from_secs(30)), "Comprobar PowerShell") {
        Ok(mode) if mode.trim() == "FullLanguage" => None,
        Ok(mode) => Some((
            "env:clm",
            "PowerShell está restringido en este equipo".to_string(),
            format!("Modo {}", mode.trim()),
            "Una directiva de la empresa (AppLocker o WDAC) limita PowerShell. Buena parte de AdminOps lo usa: diagnóstico, ajustes, red, usuarios… y no funcionará aquí.",
            "Pide a TI que permita AdminOps o úsalo en equipos sin esa restricción. La información en vivo del Panel y los procesos sí funcionan.",
        )),
        Err(e) => Some((
            "env:ps",
            "AdminOps no puede usar PowerShell en este equipo".to_string(),
            e.chars().take(120).collect(),
            "PowerShell no arranca o se cierra al momento. Suele ser el antivirus de empresa bloqueándolo, o una directiva que lo impide.",
            "Pide a TI que añada AdminOps (la carpeta donde está instalado) a las exclusiones del antivirus.",
        )),
    };
    let Some((key, title, detail, explanation, advice)) = problem else { return };
    log::warn!("{title}: {detail}");
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store: Store = crate::paths::read_json(&store_path(app));
    let alert = Alert { level: "bad".into(), key: key.into(), time: now(), title, detail, explanation: explanation.into(), advice: advice.into(), page: None, count: 1, ..Default::default() };
    let new = merge(&mut store, vec![alert]);
    let _ = crate::paths::write_json(&store_path(app), &store);
    for a in &new {
        let _ = app.emit("windows-alert", a);
    }
}

/// Avisos de otras partes de AdminOps (dispositivos vigilados…): a la campana
/// y, si la ventana no está delante, como notificación de Windows.
pub fn push_alerts(app: &tauri::AppHandle, alerts: Vec<Alert>) {
    let new = {
        let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut store: Store = crate::paths::read_json(&store_path(app));
        let new = merge(&mut store, alerts);
        let _ = crate::paths::write_json(&store_path(app), &store);
        new
    };
    for a in &new {
        log::info!("Aviso: {} {}", a.title, a.detail);
        let _ = app.emit("windows-alert", a);
        notify(app, a);
    }
}

/// Hilo de vigilancia (se inicia al arrancar si está activada en Ajustes).
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Deja arrancar la app antes de la primera pasada.
        std::thread::sleep(Duration::from_secs(8));
        environment_check(&app);
        std::thread::sleep(Duration::from_secs(12));
        loop {
            if crate::workflow::settings(&app).watch_windows {
                match poll(&app) {
                    Ok(new) => {
                        for a in &new {
                            log::info!("Aviso de Windows: {} {}", a.title, a.detail);
                            let _ = app.emit("windows-alert", a);
                        }
                        // Una notificación de Windows por pasada como mucho (la más grave).
                        if let Some(a) = new.iter().min_by_key(|a| match a.level.as_str() {
                            "bad" => 0,
                            "warn" => 1,
                            _ => 2,
                        }) {
                            if a.level != "info" || new.len() == 1 {
                                notify(&app, a);
                            }
                        }
                    }
                    Err(e) => log::debug!("Vigilancia de Windows: {e}"),
                }
            }
            std::thread::sleep(POLL);
        }
    });
}

#[tauri::command]
pub fn list_windows_alerts(app: tauri::AppHandle) -> Vec<Alert> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let store: Store = crate::paths::read_json(&store_path(&app));
    store.alerts
}

#[tauri::command]
pub fn mark_windows_alerts_read(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store: Store = crate::paths::read_json(&store_path(&app));
    store.alerts.iter_mut().for_each(|a| a.read = true);
    crate::paths::write_json(&store_path(&app), &store)
}

#[tauri::command]
pub fn clear_windows_alerts(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store: Store = crate::paths::read_json(&store_path(&app));
    store.alerts.clear();
    crate::paths::write_json(&store_path(&app), &store)
}

/// Comprobar ahora (botón del centro de avisos).
#[tauri::command(async)]
pub fn check_windows_now(app: tauri::AppHandle) -> Result<Vec<Alert>, String> {
    poll(&app)
}

/// Para el centro de avisos: cuántos avisos quedan por leer.
#[tauri::command]
pub fn unread_windows_alerts(app: tauri::AppHandle) -> usize {
    let store: Store = crate::paths::read_json(&store_path(&app));
    store.alerts.iter().filter(|a| !a.read).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(provider: &str, id: u32, props: &[&str], message: &str) -> RawEvent {
        RawEvent { provider: provider.into(), id, time: "2026-09-27T10:00:00Z".into(), props: props.iter().map(|s| s.to_string()).collect(), message: message.into(), disk: String::new() }
    }

    #[test]
    fn translates_common_errors() {
        let bsod = to_alert(&ev("Microsoft-Windows-WER-SystemErrorReporting", 1001, &["0x0000009f (0x3, 0xffff)"], "")).unwrap();
        assert_eq!(bsod.level, "bad");
        assert_eq!(bsod.detail, "Código 0x0000009F");
        let crash = to_alert(&ev("Application Error", 1000, &["chrome.exe", "1.0", "0", "ntdll.dll"], "")).unwrap();
        assert_eq!(crash.title, "chrome.exe se cerró por un error");
        assert_eq!(crash.detail, "Módulo: ntdll.dll");
        assert_eq!(crash.page.as_deref(), Some("diagnostics"));
        let wu = to_alert(&ev("Microsoft-Windows-WindowsUpdateClient", 20, &["0x80070643", "Actualización de seguridad"], "")).unwrap();
        assert!(wu.detail.starts_with("0x80070643"));
        assert!(to_alert(&ev("Application Error", 1000, &[], "")).is_none());
        assert!(to_alert(&ev("Otro", 1, &[], "")).is_none());
    }

    #[test]
    fn groups_repeated_alerts() {
        let mut s = Store::default();
        let crash = |t| Alert { key: "crash:x.exe".into(), time: t, count: 1, ..Default::default() };
        assert_eq!(merge(&mut s, vec![crash(1000)]).len(), 1);
        assert_eq!(merge(&mut s, vec![crash(2000)]).len(), 0);
        assert_eq!(s.alerts[0].count, 2);
        // Pasadas 6 h, es un aviso nuevo.
        assert_eq!(merge(&mut s, vec![crash(2000 + 7 * 3600)]).len(), 1);
    }

    #[test]
    fn script_parses() {
        let errors = crate::ps::parse_errors(&script("2026-09-27T10:00:00Z"));
        assert!(errors.is_empty(), "{errors}");
        let errors = crate::ps::parse_errors(DEVICES_SCRIPT);
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn warns_only_about_newly_broken_devices() {
        let dev = |id: &str, code| BadDevice { id: id.into(), name: "Realtek 8851BE Wireless LAN WiFi 6".into(), class: "Net".into(), code };
        let mut s = Store::default();
        assert_eq!(new_bad_devices(&mut s, &[dev("PCI\\A", 10)]).len(), 1);
        // Sigue igual: no se repite.
        assert_eq!(new_bad_devices(&mut s, &[dev("PCI\\A", 10)]).len(), 0);
        // Se arregla y vuelve a fallar: aviso nuevo.
        assert_eq!(new_bad_devices(&mut s, &[]).len(), 0);
        assert_eq!(new_bad_devices(&mut s, &[dev("PCI\\A", 10)]).len(), 1);
        let a = device_alert(&dev("PCI\\A", 10));
        assert_eq!(a.title, "La tarjeta Wi-Fi no funciona");
        assert_eq!(a.page.as_deref(), Some("network"));
    }

    /// Equipo real: `cargo test devices_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn devices_real() {
        let out = crate::pspool::query(DEVICES_SCRIPT, None, "prueba").unwrap();
        let devices: Vec<BadDevice> = serde_json::from_str(out.trim()).unwrap();
        for d in &devices {
            let a = device_alert(d);
            println!("[{}] {} · {} → {:?}", a.level, a.title, a.detail, a.page);
        }
    }

    /// Equipo real: `cargo test winwatch_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn winwatch_real() {
        let since = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let out = crate::pspool::query(&script(&since), None, "prueba").unwrap();
        let events: Vec<RawEvent> = serde_json::from_str(out.trim()).unwrap();
        let mut s = Store::default();
        let alerts: Vec<Alert> = events.iter().filter_map(to_alert).collect();
        println!("{} eventos → {} avisos → {} agrupados", events.len(), alerts.len(), merge(&mut s, alerts).len());
        for a in s.alerts.iter().take(12) {
            println!("[{}] {} · {} (x{})", a.level, a.title, a.detail, a.count);
        }
    }
}
