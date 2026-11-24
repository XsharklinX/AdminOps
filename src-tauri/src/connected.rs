//! Dispositivos conectados a este equipo (Hardware → Conectados).
//!
//! Lista lo que Windows tiene presente (SetupAPI + CfgMgr32, sin PowerShell: sale en
//! una fracción de segundo), dice si funciona, si le falta el driver o si da un error
//! (con el código de Windows dicho en una frase), y se entera de lo que se enchufa o
//! se quita: una ventana oculta recibe `WM_DEVICECHANGE`, las ráfagas se agrupan 1,5 s
//! y se compara la lista con la anterior (así se sabe *cuál* llegó, que el aviso de
//! Windows no dice). Si AdminOps no está a la vista y lo nuevo no funciona, avisa.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

// ---------- Lo que se enseña ----------

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ok,
    NoDriver,
    Error,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedDevice {
    /// Id de instancia de Windows (`USB\VID_…\…`).
    pub id: String,
    pub name: String,
    /// La clase, en español (Audio, Cámaras, USB…).
    pub class: String,
    pub manufacturer: String,
    pub status: Status,
    /// Código de error de Windows (0 = ninguno).
    pub code: u32,
    /// El estado dicho en una frase.
    pub problem: String,
    pub driver_version: Option<String>,
    /// AAAA-MM-DD.
    pub driver_date: Option<String>,
    /// Dispositivo del propio sistema (raíz, ACPI, volúmenes…): solo sale con «Ver todos».
    pub system: bool,
}

/// Lo que se lee de Windows de cada dispositivo, sin interpretar.
#[derive(Clone, Debug, Default)]
pub struct RawDevice {
    pub id: String,
    pub name: String,
    pub class_name: String,
    pub manufacturer: String,
    pub driver_version: Option<String>,
    pub driver_date: Option<String>,
    pub code: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedList {
    pub devices: Vec<ConnectedDevice>,
    /// Cuántos dispositivos del sistema se ocultaron (0 con «Ver todos»).
    pub hidden: usize,
}

// ---------- Clasificación (pura, con pruebas) ----------

/// El estado de un dispositivo. «Sin driver»: Windows lo dice con el código 28. «Con error»:
/// cualquier otro código. Que no tenga servicio ni driver propio no cuenta: Windows da por
/// buenos muchos dispositivos así (servicios Bluetooth, luces de un teclado…).
pub fn classify(code: u32) -> Status {
    match code {
        28 => Status::NoDriver,
        0 => Status::Ok,
        _ => Status::Error,
    }
}

/// El código de `ConfigManagerErrorCode` dicho en una frase.
pub fn code_phrase(code: u32) -> String {
    match code {
        0 => "Funciona bien",
        1 => "No está configurado bien",
        3 => "Falta memoria o el driver está dañado",
        10 => "No se puede iniciar",
        12 => "No hay recursos libres para él",
        14 => "Hay que reiniciar el equipo",
        18 => "Hay que reinstalar su driver",
        19 => "El registro de Windows tiene su configuración dañada",
        21 => "Windows lo está quitando",
        22 => "Está deshabilitado",
        24 => "No está presente o no funciona bien",
        28 => "No tiene driver instalado",
        29 => "Está desactivado desde la BIOS",
        31 => "Windows no puede cargar su driver",
        32 => "Su driver está desactivado",
        33 => "Windows no encuentra los recursos que necesita",
        34 => "Hay que configurarlo a mano",
        35 => "El firmware del equipo no le da recursos",
        36 => "Choca con otro por una interrupción",
        37 => "Su driver no pudo iniciarse",
        38 => "Un driver anterior sigue cargado",
        39 => "Su driver está dañado o falta",
        40 => "El registro de su servicio está dañado",
        41 => "Windows cargó el driver pero no encuentra el dispositivo",
        42 => "Está duplicado",
        43 => "Windows lo detuvo porque dio un fallo",
        44 => "Un programa o servicio lo detuvo",
        45 => "No está conectado",
        46 => "Windows no puede usarlo porque se está apagando",
        47 => "Está preparado para quitarlo",
        48 => "Su driver está bloqueado por compatibilidad",
        49 => "Falta espacio en el registro de Windows",
        50 => "Windows no puede aplicar todas sus propiedades",
        51 => "Espera a otro dispositivo",
        52 => "Windows no puede comprobar la firma de su driver",
        53 => "Está reservado para la depuración del sistema",
        54 => "Falló y se está reiniciando",
        n => return format!("Error {n} de Windows"),
    }
    .to_string()
}

/// La clase de Windows (`Net`, `MEDIA`…) en español.
pub fn class_es(class_name: &str) -> &'static str {
    match class_name.to_ascii_lowercase().as_str() {
        "media" | "audioendpoint" => "Audio",
        "camera" | "image" => "Cámaras",
        "keyboard" => "Teclados",
        "mouse" => "Ratones",
        "hidclass" => "Entrada (HID)",
        "usb" | "usbdevice" => "USB",
        "net" => "Red",
        "display" | "monitor" => "Pantallas",
        "diskdrive" | "hdc" | "scsiadapter" | "cdrom" | "wpd" | "storagevolume" | "floppydisk" | "tapedrive" => "Almacenamiento",
        "bluetooth" => "Bluetooth",
        "printer" | "printqueue" => "Impresoras",
        "smartcardreader" | "smartcard" | "sdhost" | "memorytechnologydriver" => "Lectores de tarjetas",
        "ports" => "Puertos",
        "battery" => "Batería",
        "biometric" => "Biometría",
        "system" => "Sistema",
        "processor" => "Procesador",
        _ => "Otros",
    }
}

/// ¿Es un dispositivo del propio sistema, sin interés para quien arregla un equipo?
pub fn is_system(id: &str, class_name: &str) -> bool {
    let id = id.to_ascii_uppercase();
    if ["ROOT\\", "HTREE\\", "SWD\\", "SW\\", "STORAGE\\VOLUME", "ACPI\\", "ACPI_HAL\\"].iter().any(|p| id.starts_with(p)) {
        return true;
    }
    matches!(
        class_name.to_ascii_lowercase().as_str(),
        "system" | "volume" | "volumesnapshot" | "processor" | "computer" | "softwaredevice" | "softwarecomponent" | "firmware" | "legacydriver" | "nonpnpdriver" | "extension"
    )
}

/// ¿Sale en la lista? Los del sistema solo con «Ver todos», salvo que den problemas.
pub fn visible(d: &ConnectedDevice, show_all: bool) -> bool {
    show_all || !d.system || d.status != Status::Ok
}

pub fn build_device(raw: RawDevice) -> ConnectedDevice {
    let status = classify(raw.code);
    let problem = code_phrase(raw.code);
    let name = if raw.name.trim().is_empty() { "Dispositivo desconocido".to_string() } else { raw.name.trim().to_string() };
    let system = is_system(&raw.id, &raw.class_name);
    ConnectedDevice {
        class: class_es(&raw.class_name).to_string(),
        manufacturer: raw.manufacturer.trim().to_string(),
        system,
        id: raw.id,
        name,
        status,
        code: raw.code,
        problem,
        driver_version: raw.driver_version,
        driver_date: raw.driver_date,
    }
}

// ---------- Ráfagas de avisos de Windows ----------

/// Un USB trae varias interfaces y Windows avisa de cada una: se espera a que pase
/// un rato sin avisos (o a que se cumpla el máximo, si no para nunca).
pub const QUIET_MS: u64 = 1500;
pub const MAX_BURST_MS: u64 = 6000;

#[derive(Debug, Default)]
pub struct Burst {
    first: Option<u64>,
    last: u64,
}

impl Burst {
    pub fn push(&mut self, now_ms: u64) {
        self.first.get_or_insert(now_ms);
        self.last = now_ms;
    }

    /// Cuánto falta para dar la ráfaga por cerrada (0 = ya).
    pub fn remaining(&self, now_ms: u64) -> u64 {
        let Some(first) = self.first else { return 0 };
        let quiet = (self.last + QUIET_MS).saturating_sub(now_ms);
        let cap = (first + MAX_BURST_MS).saturating_sub(now_ms);
        quiet.min(cap)
    }
}

/// Qué llegó y qué se quitó entre dos listas (por id de instancia).
pub fn diff(prev: &HashMap<String, ConnectedDevice>, now: &[ConnectedDevice]) -> (Vec<ConnectedDevice>, Vec<ConnectedDevice>) {
    let arrived: Vec<ConnectedDevice> = now.iter().filter(|d| !prev.contains_key(&d.id)).cloned().collect();
    let ids: std::collections::HashSet<&str> = now.iter().map(|d| d.id.as_str()).collect();
    let mut removed: Vec<ConnectedDevice> = prev.values().filter(|d| !ids.contains(d.id.as_str())).cloned().collect();
    removed.sort_by(|a, b| a.id.cmp(&b.id));
    (arrived, removed)
}

/// Como `diff`, pero se compara con la lista completa (los dispositivos del sistema incluidos) y solo
/// después se deja lo que se enseña: así uno del sistema que pasa a error, o que se arregla, no sale
/// como «recién conectado» ni «recién quitado».
pub fn diff_visible(prev: &HashMap<String, ConnectedDevice>, now: &[ConnectedDevice]) -> (Vec<ConnectedDevice>, Vec<ConnectedDevice>) {
    let (mut arrived, mut removed) = diff(prev, now);
    arrived.retain(|d| visible(d, false));
    removed.retain(|d| visible(d, false));
    (arrived, removed)
}

// ---------- Avisos repetidos ----------

/// Cuánto tiempo no se repite el aviso del mismo dispositivo.
const NOTIFY_COOLDOWN: Duration = Duration::from_secs(5 * 60);
/// Cuántos comprobadores de «¿sigue mal?» pueden esperar a la vez.
const MAX_CHECKERS: usize = 4;

static NOTIFIED: Mutex<Option<HashMap<String, std::time::Instant>>> = Mutex::new(None);
static CHECKERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// De estos ids, los que no se avisaron en los últimos 5 minutos; los deja apuntados.
pub fn fresh_ids(map: &mut HashMap<String, std::time::Instant>, ids: &[String], now: std::time::Instant) -> Vec<String> {
    map.retain(|_, at| now.saturating_duration_since(*at) < NOTIFY_COOLDOWN);
    ids.iter()
        .filter(|id| {
            if map.contains_key(*id) {
                false
            } else {
                map.insert((*id).clone(), now);
                true
            }
        })
        .cloned()
        .collect()
}

// ---------- Lo reciente ----------

/// Cuánto se recuerda lo que llegó o se quitó (la interfaz resalta 2 min y atenúa 30 s).
const KEEP_MS: u64 = 5 * 60_000;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecentChange {
    /// `arrived` o `removed`.
    pub kind: String,
    /// Milisegundos desde 1970.
    pub at: u64,
    pub device: ConnectedDevice,
}

static RECENT: Mutex<Vec<RecentChange>> = Mutex::new(Vec::new());

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Apunta lo que llegó y se quitó; lo que vuelve a llegar borra su «quitado» y al revés.
pub fn remember(list: &mut Vec<RecentChange>, at: u64, arrived: &[ConnectedDevice], removed: &[ConnectedDevice]) {
    list.retain(|c| at.saturating_sub(c.at) <= KEEP_MS);
    for d in arrived {
        list.retain(|c| c.device.id != d.id);
        list.push(RecentChange { kind: "arrived".into(), at, device: d.clone() });
    }
    for d in removed {
        list.retain(|c| c.device.id != d.id);
        list.push(RecentChange { kind: "removed".into(), at, device: d.clone() });
    }
}

/// El texto del aviso de un dispositivo recién conectado que no funciona.
pub fn alert_text(bad: &[ConnectedDevice]) -> Option<String> {
    let first = bad.iter().find(|d| d.status != Status::Ok)?;
    let why = if first.status == Status::NoDriver { "sin driver" } else { "con un error" };
    let more = bad.len() - 1;
    Some(if more == 0 { format!("Recién conectado: {}, {why}", first.name) } else { format!("Recién conectado: {}, {why} (y {more} más con problemas)", first.name) })
}

// ---------- Leer Windows ----------

#[cfg(windows)]
mod win {
    use super::{build_device, ConnectedDevice, RawDevice};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows_sys::Win32::Devices::Properties::{DEVPKEY_Device_DriverDate, DEVPKEY_Device_DriverVersion, DEVPKEY_Device_Manufacturer, DEVPKEY_NAME};
    use windows_sys::Win32::Foundation::{DEVPROPKEY, INVALID_HANDLE_VALUE};

    const DN_HAS_PROBLEM_FLAG: u32 = 0x400;
    const TYPE_STRING: u32 = 0x12;
    const TYPE_FILETIME: u32 = 0x10;

    fn utf16(bytes: &[u8]) -> String {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
        String::from_utf16_lossy(&units)
    }

    unsafe fn property(set: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<(u32, Vec<u8>)> {
        let mut ty = 0u32;
        let mut need = 0u32;
        unsafe {
            SetupDiGetDevicePropertyW(set, data, key, &mut ty, null_mut(), 0, &mut need, 0);
            if need == 0 || need > 64 * 1024 {
                return None;
            }
            let mut buf = vec![0u8; need as usize];
            if SetupDiGetDevicePropertyW(set, data, key, &mut ty, buf.as_mut_ptr(), need, &mut need, 0) == 0 {
                return None;
            }
            Some((ty, buf))
        }
    }

    unsafe fn text_property(set: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<String> {
        let (ty, buf) = unsafe { property(set, data, key)? };
        (ty == TYPE_STRING).then(|| utf16(&buf)).filter(|s| !s.trim().is_empty())
    }

    unsafe fn registry_text(set: HDEVINFO, data: &SP_DEVINFO_DATA, prop: u32) -> Option<String> {
        let mut buf = vec![0u8; 1024];
        let mut need = 0u32;
        let mut ty = 0u32;
        let ok = unsafe { SetupDiGetDeviceRegistryPropertyW(set, data, prop, &mut ty, buf.as_mut_ptr(), buf.len() as u32, &mut need) };
        (ok != 0).then(|| utf16(&buf)).filter(|s| !s.trim().is_empty())
    }

    /// FILETIME (100 ns desde 1601) → AAAA-MM-DD.
    fn filetime_date(buf: &[u8]) -> Option<String> {
        let ft = u64::from_le_bytes(buf.get(..8)?.try_into().ok()?);
        let secs = (ft / 10_000_000) as i64 - 11_644_473_600;
        let day = chrono::DateTime::from_timestamp(secs, 0)?.format("%Y-%m-%d").to_string();
        // Windows usa 1601 o 1970 para «sin fecha».
        (!day.starts_with("1601") && !day.starts_with("1970")).then_some(day)
    }

    pub fn enumerate_raw() -> Result<Vec<RawDevice>, String> {
        unsafe {
            let set = SetupDiGetClassDevsW(null(), null(), null_mut(), DIGCF_ALLCLASSES | DIGCF_PRESENT);
            if set == INVALID_HANDLE_VALUE as isize || set == 0 {
                return Err("Windows no dejó leer la lista de dispositivos.".into());
            }
            let mut out = Vec::new();
            let mut i = 0u32;
            loop {
                let mut data: SP_DEVINFO_DATA = std::mem::zeroed();
                data.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
                if SetupDiEnumDeviceInfo(set, i, &mut data) == 0 {
                    break;
                }
                i += 1;
                let mut idbuf = [0u16; 512];
                if SetupDiGetDeviceInstanceIdW(set, &data, idbuf.as_mut_ptr(), idbuf.len() as u32, null_mut()) == 0 {
                    continue;
                }
                let id = String::from_utf16_lossy(&idbuf[..idbuf.iter().position(|&u| u == 0).unwrap_or(0)]);
                let name = text_property(set, &data, &DEVPKEY_NAME).or_else(|| registry_text(set, &data, SPDRP_FRIENDLYNAME)).or_else(|| registry_text(set, &data, SPDRP_DEVICEDESC)).unwrap_or_default();
                let manufacturer = text_property(set, &data, &DEVPKEY_Device_Manufacturer).or_else(|| registry_text(set, &data, SPDRP_MFG)).unwrap_or_default();
                let class_name = registry_text(set, &data, SPDRP_CLASS).unwrap_or_default();
                let driver_version = text_property(set, &data, &DEVPKEY_Device_DriverVersion);
                let driver_date = property(set, &data, &DEVPKEY_Device_DriverDate).filter(|(t, _)| *t == TYPE_FILETIME).and_then(|(_, b)| filetime_date(&b));
                let (mut status, mut problem) = (0u32, 0u32);
                let code = if CM_Get_DevNode_Status(&mut status, &mut problem, data.DevInst, 0) == CR_SUCCESS && status & DN_HAS_PROBLEM_FLAG != 0 { problem } else { 0 };
                out.push(RawDevice { id, name, class_name, manufacturer, driver_version, driver_date, code });
            }
            SetupDiDestroyDeviceInfoList(set);
            Ok(out)
        }
    }

    pub fn enumerate() -> Result<Vec<ConnectedDevice>, String> {
        Ok(enumerate_raw()?.into_iter().map(build_device).collect())
    }
}

#[cfg(not(windows))]
mod win {
    use super::ConnectedDevice;
    pub fn enumerate() -> Result<Vec<ConnectedDevice>, String> {
        Err("Solo funciona en Windows.".into())
    }
}

/// Todos los dispositivos presentes, ordenados: primero los que dan problemas.
pub fn list(show_all: bool) -> Result<ConnectedList, String> {
    let all = win::enumerate()?;
    let hidden = all.iter().filter(|d| !visible(d, show_all)).count();
    let mut devices: Vec<ConnectedDevice> = all.into_iter().filter(|d| visible(d, show_all)).collect();
    devices.sort_by(|a, b| {
        let rank = |d: &ConnectedDevice| match d.status {
            Status::Error => 0,
            Status::NoDriver => 1,
            Status::Ok => 2,
        };
        rank(a).cmp(&rank(b)).then_with(|| a.class.cmp(&b.class)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(ConnectedList { devices, hidden })
}

#[tauri::command(async)]
pub fn connected_list(show_all: bool) -> Result<ConnectedList, String> {
    list(show_all)
}

/// Lo que llegó o se quitó hace poco (para resaltarlo aunque la pestaña no estuviera abierta).
#[tauri::command]
pub fn connected_recent() -> Vec<RecentChange> {
    let now = now_ms();
    RECENT.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|c| now.saturating_sub(c.at) <= KEEP_MS).cloned().collect()
}

// ---------- Acciones ----------

/// El id de instancia va como argumento (no por una shell), pero se rechazan los raros.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 400 && id.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !id.contains(['"', '`', '$', ';', '|', '<', '>'])
}

/// Buscar un driver en Windows Update para el dispositivo `$id`.
pub const DRIVER_SEARCH_BODY: &str = r#"# Que Windows vuelva a mirar el hardware
pnputil.exe /scan-devices | Out-Null
# Los identificadores de hardware del dispositivo
$ids = @()
foreach ($k in 'DEVPKEY_Device_HardwareIds', 'DEVPKEY_Device_CompatibleIds') {
    $d = (Get-PnpDeviceProperty -InstanceId $id -KeyName $k -ErrorAction SilentlyContinue).Data
    if ($d) { $ids += $d }
}
# Los drivers que Windows Update ofrece para este equipo
$session = New-Object -ComObject Microsoft.Update.Session
$found = $session.CreateUpdateSearcher().Search("IsInstalled=0 and Type='Driver'")
$mine = @($found.Updates | Where-Object { $ids -contains $_.DriverHardwareID })
if ($mine.Count -eq 0) { 'Windows Update no tiene un driver para este dispositivo.'; return }
# Lo descarga y lo instala
$col = New-Object -ComObject Microsoft.Update.UpdateColl
foreach ($u in $mine) { if (-not $u.EulaAccepted) { $u.AcceptEula() }; [void]$col.Add($u) }
$dl = $session.CreateUpdateDownloader(); $dl.Updates = $col; [void]$dl.Download()
$inst = $session.CreateUpdateInstaller(); $inst.Updates = $col; $r = $inst.Install()
$names = ($mine | ForEach-Object { $_.Title }) -join ', '
if ($r.ResultCode -eq 2) { if ($r.RebootRequired) { "Driver instalado ($names). Hay que reiniciar el equipo." } else { "Driver instalado: $names." } } else { throw "Windows Update no pudo instalar el driver ($names)." }
"#;

/// El script de una acción sobre un dispositivo (el que se ejecuta y el que enseña «Ver el comando»).
/// `action`: `scan`, `restart` o `enable`.
pub fn action_script(action: &str, id: &str) -> Result<(&'static str, String), String> {
    if !valid_id(id) {
        return Err("Falta el dispositivo.".into());
    }
    let var = crate::ps::text_var("id", id);
    let var = var.trim_end();
    match action {
        "scan" => Ok(("Buscar un driver para el dispositivo", format!("# Buscar un driver en Windows Update para este dispositivo\n\n# El dispositivo (id de instancia)\n{var}\n{DRIVER_SEARCH_BODY}"))),
        "restart" => Ok((
            "Volver a detectar el dispositivo",
            format!("# Volver a detectar un dispositivo\n\n# El dispositivo (id de instancia)\n{var}\n# Windows lo para y lo vuelve a iniciar\npnputil.exe /restart-device $id\n"),
        )),
        "enable" => Ok(("Habilitar el dispositivo", format!("# Habilitar un dispositivo\n\n# El dispositivo (id de instancia)\n{var}\n# Windows lo vuelve a poner en marcha\npnputil.exe /enable-device $id\n"))),
        _ => Err(format!("Acción desconocida: {action}")),
    }
}

/// Lo que cambia algo en un dispositivo. Pide administrador.
#[tauri::command(async)]
pub fn connected_action(action: String, id: String, state: State<'_, TweakState>) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let (title, script) = action_script(&action, &id)?;
    let result = match action.as_str() {
        "scan" => crate::ps::powershell_opts(&script, crate::ps::Opts::task("connected-driver", Some(Duration::from_secs(15 * 60)))).map(|s| s.trim().to_string()),
        "restart" => crate::ps::exec_opts("pnputil.exe", &["/restart-device", &id], crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None }).map(|_| "Dispositivo vuelto a detectar.".to_string()),
        _ => crate::ps::exec_opts("pnputil.exe", &["/enable-device", &id], crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None }).map(|_| "Dispositivo habilitado.".to_string()),
    };
    state.record(Op::Run, title, &result);
    result
}

// ---------- Escuchar lo que se enchufa y se quita ----------

#[cfg(windows)]
mod watcher {
    use std::sync::mpsc::Sender;
    use std::sync::{Mutex, OnceLock};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static TX: OnceLock<Mutex<Sender<()>>> = OnceLock::new();

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // DBT_DEVNODES_CHANGED: algo se conectó, se quitó o cambió de estado.
        if msg == WM_DEVICECHANGE && wparam as u32 == DBT_DEVNODES_CHANGED {
            if let Some(tx) = TX.get() {
                let _ = tx.lock().unwrap_or_else(|e| e.into_inner()).send(());
            }
        }
        // WM_DISPLAYCHANGE: otra resolución, un monitor conectado o quitado (Máquina → Pantallas).
        if msg == WM_DISPLAYCHANGE {
            crate::displays::on_display_change();
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// Crea la ventana oculta que recibe los avisos de dispositivos. Bloquea con su bucle de mensajes.
    pub fn run(tx: Sender<()>) -> Result<(), String> {
        let _ = TX.set(Mutex::new(tx));
        let class: Vec<u16> = "AdminOpsDeviceWatch\0".encode_utf16().collect();
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(proc);
            wc.hInstance = hinst;
            wc.lpszClassName = class.as_ptr();
            if RegisterClassW(&wc) == 0 {
                return Err("No se pudo registrar la ventana de avisos de dispositivos.".into());
            }
            // Una ventana normal que nunca se enseña: las de «solo mensajes» no reciben este aviso.
            let hwnd = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null());
            if hwnd.is_null() {
                return Err("No se pudo crear la ventana de avisos de dispositivos.".into());
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DeviceEvent {
    ids: Vec<String>,
}

/// Arranca la escucha (un hilo que espera y otro con la ventana oculta).
pub fn start(app: tauri::AppHandle) {
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            if let Err(e) = watcher::run(tx) {
                log::warn!("Sin aviso de dispositivos nuevos: {e}");
            }
        });
        std::thread::spawn(move || {
            // Espera a que la app termine de arrancar antes de la primera lectura.
            std::thread::sleep(Duration::from_secs(6));
            let mut known: HashMap<String, ConnectedDevice> = match list(true) {
                Ok(l) => l.devices.into_iter().map(|d| (d.id.clone(), d)).collect(),
                Err(e) => {
                    log::warn!("Sin lista de dispositivos para vigilar: {e}");
                    return;
                }
            };
            // Los avisos que llegaron mientras tanto no cuentan: ya están en la lista de arriba.
            while rx.try_recv().is_ok() {}
            loop {
                if rx.recv().is_err() {
                    return;
                }
                let mut burst = Burst::default();
                burst.push(now_ms());
                loop {
                    let wait = burst.remaining(now_ms());
                    if wait == 0 {
                        break;
                    }
                    match rx.recv_timeout(Duration::from_millis(wait)) {
                        Ok(()) => burst.push(now_ms()),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                // La lista completa: si no, un dispositivo del sistema que cambia de estado parecería llegar o irse.
                let Ok(now) = list(true) else { continue };
                let (arrived, removed) = diff_visible(&known, &now.devices);
                known = now.devices.into_iter().map(|d| (d.id.clone(), d)).collect();
                if arrived.is_empty() && removed.is_empty() {
                    continue;
                }
                {
                    let mut recent = RECENT.lock().unwrap_or_else(|e| e.into_inner());
                    remember(&mut recent, now_ms(), &arrived, &removed);
                }
                use tauri::Emitter;
                if !arrived.is_empty() {
                    log::info!("Dispositivos conectados: {}", arrived.iter().map(|d| d.name.as_str()).collect::<Vec<_>>().join(", "));
                    let _ = app.emit("device-arrived", DeviceEvent { ids: arrived.iter().map(|d| d.id.clone()).collect() });
                }
                if !removed.is_empty() {
                    let _ = app.emit("device-removed", DeviceEvent { ids: removed.iter().map(|d| d.id.clone()).collect() });
                }
                let bad: Vec<String> = arrived.iter().filter(|d| d.status != Status::Ok).map(|d| d.id.clone()).collect();
                // El mismo dispositivo no se avisa dos veces en 5 minutos (un USB flojo avisaría sin parar).
                let bad = {
                    let mut g = NOTIFIED.lock().unwrap_or_else(|e| e.into_inner());
                    fresh_ids(g.get_or_insert_with(HashMap::new), &bad, std::time::Instant::now())
                };
                if !bad.is_empty() && CHECKERS.load(std::sync::atomic::Ordering::SeqCst) < MAX_CHECKERS {
                    CHECKERS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let app = app.clone();
                    // Windows suele instalar el driver solo en unos segundos: se avisa si sigue mal.
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(8));
                        if let Ok(l) = list(false) {
                            let still: Vec<ConnectedDevice> = l.devices.into_iter().filter(|d| bad.contains(&d.id) && d.status != Status::Ok).collect();
                            notify(&app, &still);
                        }
                        CHECKERS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    });
                }
            }
        });
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// ¿Quiere el técnico este aviso? Ajustes → General: de todos, solo de los graves o de ninguno.
pub fn wanted(setting: &str, bad: &[ConnectedDevice]) -> bool {
    match setting {
        "none" => false,
        // «Solo los graves»: un error sí; un dispositivo sin driver, no.
        "bad" => bad.iter().any(|d| d.status == Status::Error),
        _ => true,
    }
}

fn notify(app: &tauri::AppHandle, bad: &[ConnectedDevice]) {
    use tauri::{Emitter, Manager};
    use tauri_plugin_notification::NotificationExt;
    let Some(text) = alert_text(bad) else { return };
    let focused = app.get_webview_window("main").map(|w| w.is_focused().unwrap_or(false) && !w.is_minimized().unwrap_or(false)).unwrap_or(false);
    if focused || !wanted(&crate::workflow::settings(app).notify_alerts, bad) {
        return;
    }
    let handle = app.clone();
    let shown = crate::toast::show(app, "AdminOps", &text, &[], move |_| {
        crate::tray::show_main(&handle);
        let _ = handle.emit("tray-action", "connected");
    });
    if !shown {
        let _ = app.notification().builder().title("AdminOps").body(text.chars().take(240).collect::<String>()).show();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: &str, class: &str, code: u32, _service: bool, driver: bool) -> RawDevice {
        RawDevice {
            id: id.into(),
            name: "Prueba".into(),
            class_name: class.into(),
            driver_version: driver.then(|| "1.0.0.0".to_string()),
            code,
            ..Default::default()
        }
    }

    fn dev(id: &str, status: Status) -> ConnectedDevice {
        let mut d = build_device(raw(id, "USB", 0, true, true));
        d.status = status;
        d
    }

    #[test]
    fn el_estado_sale_del_codigo() {
        assert_eq!(classify(0), Status::Ok);
        assert_eq!(classify(28), Status::NoDriver);
        assert_eq!(classify(10), Status::Error);
        assert_eq!(classify(22), Status::Error);
        assert_eq!(classify(43), Status::Error);
    }

    #[test]
    fn los_codigos_se_dicen_en_una_frase() {
        assert_eq!(code_phrase(10), "No se puede iniciar");
        assert_eq!(code_phrase(22), "Está deshabilitado");
        assert_eq!(code_phrase(28), "No tiene driver instalado");
        assert_eq!(code_phrase(43), "Windows lo detuvo porque dio un fallo");
        assert_eq!(code_phrase(45), "No está conectado");
        assert_eq!(code_phrase(52), "Windows no puede comprobar la firma de su driver");
        assert_eq!(code_phrase(99), "Error 99 de Windows");
        assert_eq!(code_phrase(0), "Funciona bien");
        // Todas las conocidas tienen una frase propia, no la genérica.
        for c in [1, 3, 10, 12, 14, 18, 19, 21, 22, 24, 28, 29, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54] {
            assert!(!code_phrase(c).starts_with("Error "), "{c}");
        }
    }

    #[test]
    fn las_clases_salen_en_espanol() {
        assert_eq!(class_es("MEDIA"), "Audio");
        assert_eq!(class_es("AudioEndpoint"), "Audio");
        assert_eq!(class_es("Camera"), "Cámaras");
        assert_eq!(class_es("Keyboard"), "Teclados");
        assert_eq!(class_es("Mouse"), "Ratones");
        assert_eq!(class_es("USB"), "USB");
        assert_eq!(class_es("Net"), "Red");
        assert_eq!(class_es("Monitor"), "Pantallas");
        assert_eq!(class_es("DiskDrive"), "Almacenamiento");
        assert_eq!(class_es("Bluetooth"), "Bluetooth");
        assert_eq!(class_es("PrintQueue"), "Impresoras");
        assert_eq!(class_es("SmartCardReader"), "Lectores de tarjetas");
        assert_eq!(class_es(""), "Otros");
        assert_eq!(class_es("ClaseRara"), "Otros");
    }

    #[test]
    fn los_dispositivos_del_sistema_se_filtran_salvo_que_den_problemas() {
        assert!(is_system("ROOT\\SYSTEM\\0000", ""));
        assert!(is_system("ACPI\\PNP0A08\\0", "System"));
        assert!(is_system("SWD\\MMDEVAPI\\{0.0.0}", "MEDIA"));
        assert!(is_system("STORAGE\\VOLUME\\{abc}", "Volume"));
        assert!(!is_system("USB\\VID_046D&PID_C52B\\5&1", "USB"));
        assert!(!is_system("PCI\\VEN_10EC&DEV_8168\\4&1", "Net"));
        let ok = build_device(raw("ROOT\\X\\0", "System", 0, true, true));
        assert!(ok.system);
        assert!(!visible(&ok, false));
        assert!(visible(&ok, true));
        // Un dispositivo del sistema con error sí se enseña.
        let bad = build_device(raw("ROOT\\X\\0", "System", 10, true, true));
        assert!(visible(&bad, false));
        let normal = build_device(raw("USB\\A\\1", "USB", 0, true, true));
        assert!(visible(&normal, false));
    }

    #[test]
    fn un_dispositivo_construido_dice_su_estado_y_su_frase() {
        let d = build_device(raw("USB\\A\\1", "Net", 43, true, true));
        assert_eq!((d.status, d.class.as_str(), d.problem.as_str()), (Status::Error, "Red", "Windows lo detuvo porque dio un fallo"));
        let s = build_device(raw("USB\\A\\2", "", 28, false, false));
        assert_eq!((s.status, s.problem.as_str()), (Status::NoDriver, "No tiene driver instalado"));
        let mut r = raw("USB\\A\\3", "USB", 0, true, true);
        r.name = "   ".into();
        assert_eq!(build_device(r).name, "Dispositivo desconocido");
    }

    #[test]
    fn las_rafagas_se_agrupan_en_un_segundo_y_medio() {
        let mut b = Burst::default();
        assert_eq!(b.remaining(0), 0);
        b.push(1000);
        assert_eq!(b.remaining(1000), 1500);
        // Llegan más avisos de la misma conexión: la espera se alarga.
        b.push(2000);
        b.push(3200);
        assert_eq!(b.remaining(3200), 1500);
        assert_eq!(b.remaining(4000), 700);
        assert_eq!(b.remaining(4700), 0);
        // Si no paran nunca, se cierra al llegar al máximo.
        let mut c = Burst::default();
        for t in (0..=6000).step_by(1000) {
            c.push(t);
        }
        assert_eq!(c.remaining(6000), 0);
    }

    #[test]
    fn se_ve_que_llega_y_que_se_quita() {
        let a = dev("USB\\A", Status::Ok);
        let b = dev("USB\\B", Status::NoDriver);
        let prev: HashMap<String, ConnectedDevice> = [a.clone(), b.clone()].into_iter().map(|d| (d.id.clone(), d)).collect();
        let c = dev("USB\\C", Status::Ok);
        let (arrived, removed) = diff(&prev, &[a.clone(), c.clone()]);
        assert_eq!(arrived.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["USB\\C"]);
        assert_eq!(removed.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["USB\\B"]);
        let (arrived, removed) = diff(&prev, &[a, b]);
        assert!(arrived.is_empty() && removed.is_empty());
    }

    #[test]
    fn un_dispositivo_del_sistema_que_cambia_de_estado_no_llega_ni_se_va() {
        let mut sys_ok = dev("ROOT\\SYS\\0", Status::Ok);
        sys_ok.system = true;
        let mut sys_bad = sys_ok.clone();
        sys_bad.status = Status::Error;
        // Del sistema y bien: oculto. Pasa a error: con la lista completa sigue siendo el mismo, no llega.
        let prev: HashMap<String, ConnectedDevice> = [(sys_ok.id.clone(), sys_ok.clone())].into_iter().collect();
        let (arrived, removed) = diff_visible(&prev, std::slice::from_ref(&sys_bad));
        assert!(arrived.is_empty() && removed.is_empty());
        // Y al arreglarse tampoco sale como quitado.
        let prev: HashMap<String, ConnectedDevice> = [(sys_bad.id.clone(), sys_bad.clone())].into_iter().collect();
        let (arrived, removed) = diff_visible(&prev, &[sys_ok]);
        assert!(arrived.is_empty() && removed.is_empty());
        // Uno normal que llega sí se cuenta; uno del sistema que llega bien, no.
        let usb = dev("USB\\A", Status::Ok);
        let mut other = dev("ROOT\\B", Status::Ok);
        other.system = true;
        let (arrived, _) = diff_visible(&HashMap::new(), &[usb.clone(), other]);
        assert_eq!(arrived.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), [usb.id.as_str()]);
    }

    #[test]
    fn el_mismo_dispositivo_no_se_avisa_dos_veces_en_cinco_minutos() {
        let t0 = std::time::Instant::now();
        let mut map = HashMap::new();
        let ids = vec!["USB\\A".to_string(), "USB\\B".to_string()];
        assert_eq!(fresh_ids(&mut map, &ids, t0), ids);
        assert!(fresh_ids(&mut map, &ids, t0 + Duration::from_secs(60)).is_empty());
        assert_eq!(fresh_ids(&mut map, &ids[..1], t0 + NOTIFY_COOLDOWN + Duration::from_secs(1)), &ids[..1]);
    }

    #[test]
    fn el_script_de_buscar_driver_falla_si_no_se_instala() {
        assert!(DRIVER_SEARCH_BODY.contains("throw \"Windows Update no pudo instalar"));
    }

    #[test]
    fn lo_reciente_se_cambia_y_caduca() {
        let a = dev("USB\\A", Status::Ok);
        let mut list = Vec::new();
        remember(&mut list, 1_000, std::slice::from_ref(&a), &[]);
        remember(&mut list, 2_000, &[], std::slice::from_ref(&a));
        // Quitado después de llegar: solo queda lo último.
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "removed");
        remember(&mut list, 3_000, &[a], &[]);
        assert_eq!((list.len(), list[0].kind.as_str()), (1, "arrived"));
        // Pasado el tiempo, se olvida.
        remember(&mut list, 3_000 + KEEP_MS + 1, &[], &[]);
        assert!(list.is_empty());
    }

    #[test]
    fn el_aviso_dice_que_pasa_y_respeta_los_ajustes() {
        let nd = dev("USB\\A", Status::NoDriver);
        let er = dev("USB\\B", Status::Error);
        assert_eq!(alert_text(std::slice::from_ref(&nd)).unwrap(), "Recién conectado: Prueba, sin driver");
        assert_eq!(alert_text(std::slice::from_ref(&er)).unwrap(), "Recién conectado: Prueba, con un error");
        assert_eq!(alert_text(&[nd.clone(), er.clone()]).unwrap(), "Recién conectado: Prueba, sin driver (y 1 más con problemas)");
        assert!(alert_text(&[dev("USB\\C", Status::Ok)]).is_none());
        assert!(alert_text(&[]).is_none());
        assert!(wanted("all", std::slice::from_ref(&nd)));
        assert!(!wanted("none", std::slice::from_ref(&er)));
        assert!(!wanted("bad", &[nd]));
        assert!(wanted("bad", &[er]));
    }

    #[test]
    fn los_scripts_de_las_acciones_son_powershell_valido() {
        for action in ["scan", "restart", "enable"] {
            let (title, script) = action_script(action, "USB\\VID_046D&PID_C52B\\5&2A'1").unwrap();
            assert!(!title.is_empty());
            let errors = crate::ps::parse_errors(&script);
            assert!(errors.trim().is_empty(), "{action}: {errors}");
        }
        assert!(action_script("scan", "").is_err());
        assert!(action_script("scan", "a\"; calc").is_err());
        assert!(action_script("borrar", "USB\\A").is_err());
    }

    /// Equipo real: `cargo test --lib dispositivos_reales -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dispositivos_reales() {
        let all = win::enumerate().expect("lista");
        let count = |f: &dyn Fn(&ConnectedDevice) -> bool| all.iter().filter(|d| f(d)).count();
        let shown = list(false).unwrap();
        println!("Presentes en total: {}", all.len());
        println!("  Bien: {}", count(&|d| d.status == Status::Ok));
        println!("  Sin driver: {}", count(&|d| d.status == Status::NoDriver));
        println!("  Con error: {}", count(&|d| d.status == Status::Error));
        println!("  Del sistema: {}", count(&|d| d.system));
        println!("Visibles sin «Ver todos»: {} (ocultos {})", shown.devices.len(), shown.hidden);
        for d in shown.devices.iter().filter(|d| d.status != Status::Ok) {
            println!("  [{:?}] {} · {} · código {} · {}", d.status, d.class, d.name, d.code, d.problem);
        }
        let mut by_class: std::collections::BTreeMap<&str, usize> = Default::default();
        for d in &shown.devices {
            *by_class.entry(d.class.as_str()).or_default() += 1;
        }
        println!("Por clase: {by_class:?}");
        assert!(!all.is_empty());
    }
}
