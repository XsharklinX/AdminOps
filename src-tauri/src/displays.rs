//! Pantallas y monitores (Estado del equipo → Pantallas).
//!
//! Lee lo que Windows tiene activo con `QueryDisplayConfig` (sin PowerShell): para cada
//! monitor, su nombre y su cable (`DisplayConfigGetDeviceInfo`), la resolución y los Hz con
//! que va ahora, dónde está en el escritorio y a qué escala. La resolución propia del monitor
//! sale de lo que dice el driver y, si no, del EDID que el monitor guarda en el registro
//! (marca y modelo también). Con eso la interfaz dice si va «borroso» (no usa su resolución),
//! con una escala rara o con pocos Hz.
//!
//! Los arreglos (poner la resolución propia, extender, duplicar) cambian la pantalla y por eso
//! llevan red: antes de tocar nada se guarda la configuración completa y, si en 15 segundos
//! nadie la confirma, el propio programa la vuelve a poner. La cuenta atrás corre aquí, no en
//! la interfaz: si la ventana queda en un monitor apagado, la pantalla vuelve igual.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

/// Cuánto se espera la confirmación de un cambio de pantalla.
pub const CONFIRM_SECS: u64 = 15;

// ---------- Lo que se enseña ----------

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    /// Ruta del dispositivo de Windows: identifica al monitor aunque cambie de puerto.
    pub id: String,
    /// «LG 24MK430».
    pub name: String,
    pub maker: String,
    pub model: String,
    /// Número de pantalla de Windows (el de «Identificar»).
    pub number: u32,
    /// Nombre GDI (`\\.\DISPLAY1`); solo lo usa el programa.
    #[serde(skip)]
    pub gdi: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub hz: u32,
    /// Lo más que admite, si se sabe (rango del EDID).
    pub max_hz: Option<u32>,
    pub native_width: Option<u32>,
    pub native_height: Option<u32>,
    /// Los Hz de su resolución propia, si se sabe (solo para elegir bien al cambiarla).
    #[serde(skip)]
    pub native_hz: Option<u32>,
    /// HDMI, DisplayPort, VGA, Interna…
    pub connection: String,
    /// Escala de Windows en porcentaje (100, 125, 150…).
    pub scale: u32,
    pub primary: bool,
    /// Grados: 0, 90, 180 o 270.
    pub rotation: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PendingInfo {
    /// Cuándo se vuelve atrás solo (milisegundos desde 1970).
    pub until_ms: u64,
    pub title: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DisplayList {
    pub monitors: Vec<Monitor>,
    /// Dos o más monitores enseñan lo mismo (duplicados).
    pub duplicated: bool,
    /// Un cambio sin confirmar que se deshará solo.
    pub pending: Option<PendingInfo>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DisplayChange {
    pub message: String,
    pub until_ms: u64,
    pub seconds: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

// ---------- EDID (puro, con pruebas) ----------

/// Lo que se saca del EDID de un monitor.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Edid {
    /// Fabricante en tres letras (`GSM`, `DEL`…).
    pub maker: String,
    pub product: u16,
    /// Nombre del modelo (descriptor `0xFC`).
    pub name: Option<String>,
    /// Ancho, alto y Hz de la primera temporización detallada: la propia del monitor.
    pub native: Option<(u32, u32, u32)>,
    /// Frecuencia vertical máxima que declara (descriptor `0xFD`).
    pub max_hz: Option<u32>,
}

/// Analiza el bloque base (128 bytes) de un EDID. `None` si no lo es.
pub fn parse_edid(b: &[u8]) -> Option<Edid> {
    if b.len() < 128 || b[..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }
    let m = u16::from_be_bytes([b[8], b[9]]);
    let letter = |v: u16| {
        let n = (v & 0x1F) as u8;
        if (1..=26).contains(&n) {
            (b'A' + n - 1) as char
        } else {
            '?'
        }
    };
    let mut e = Edid { maker: [letter(m >> 10), letter(m >> 5), letter(m)].iter().collect(), product: u16::from_le_bytes([b[10], b[11]]), ..Default::default() };
    for (i, off) in [54usize, 72, 90, 108].into_iter().enumerate() {
        let d = &b[off..off + 18];
        if d[0] != 0 || d[1] != 0 {
            // Temporización detallada: solo vale la primera.
            if i == 0 {
                let clock = u64::from(u16::from_le_bytes([d[0], d[1]])) * 10_000;
                let h_active = u32::from(d[2]) | ((u32::from(d[4]) & 0xF0) << 4);
                let h_blank = u32::from(d[3]) | ((u32::from(d[4]) & 0x0F) << 8);
                let v_active = u32::from(d[5]) | ((u32::from(d[7]) & 0xF0) << 4);
                let v_blank = u32::from(d[6]) | ((u32::from(d[7]) & 0x0F) << 8);
                let total = u64::from(h_active + h_blank) * u64::from(v_active + v_blank);
                if h_active > 0 && v_active > 0 && total > 0 {
                    e.native = Some((h_active, v_active, ((clock as f64) / (total as f64)).round() as u32));
                }
            }
        } else if d[2] == 0 {
            match d[3] {
                0xFC => {
                    let text: String = d[5..18].iter().take_while(|&&c| c != b'\n' && c != 0).map(|&c| if (0x20..0x7F).contains(&c) { c as char } else { ' ' }).collect();
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        e.name = Some(text);
                    }
                }
                0xFD => {
                    // Byte 4, bit 1: el máximo vertical lleva 255 de más.
                    let max = u32::from(d[6]) + if d[4] & 0x02 != 0 { 255 } else { 0 };
                    if max > 0 {
                        e.max_hz = Some(max);
                    }
                }
                _ => {}
            }
        }
    }
    Some(e)
}

/// El fabricante en una palabra legible, o las tres letras si no se conoce.
pub fn maker_name(code: &str) -> String {
    match code {
        "GSM" | "LGD" => "LG",
        "SAM" | "SEC" => "Samsung",
        "DEL" | "DLL" => "Dell",
        "AUS" | "ACI" => "ASUS",
        "ACR" | "API" => "Acer",
        "BNQ" => "BenQ",
        "HWP" | "HPN" => "HP",
        "LEN" | "LNV" => "Lenovo",
        "PHL" => "Philips",
        "AOC" => "AOC",
        "MSI" => "MSI",
        "VSC" => "ViewSonic",
        "SNY" => "Sony",
        "IVM" => "Iiyama",
        "NEC" => "NEC",
        "EIZ" | "ENC" => "Eizo",
        "GBT" => "Gigabyte",
        "APP" => "Apple",
        "TSB" => "Toshiba",
        "SHP" => "Sharp",
        "PAN" | "MEI" => "Panasonic",
        "HKC" => "HKC",
        "XMI" => "Xiaomi",
        "HSD" => "Hannspree",
        "MAX" => "Maxdata",
        "CMN" | "BOE" | "AUO" | "IVO" | "SDC" | "LPL" => "",
        other => other,
    }
    .to_string()
}

/// «LG» + «24MK430» → «LG 24MK430» (sin repetir la marca si el modelo ya la lleva).
pub fn display_name(maker: &str, model: &str) -> String {
    let maker = maker.trim();
    let model = model.trim();
    if model.is_empty() {
        return maker.to_string();
    }
    if maker.is_empty() || model.to_lowercase().starts_with(&maker.to_lowercase()) {
        return model.to_string();
    }
    format!("{maker} {model}")
}

/// La clave del registro con el EDID de un monitor, a partir de la ruta que da Windows
/// (`\\?\DISPLAY#GSM5B09#5&2b8a1f9c&0&UID4352#{guid}`).
pub fn edid_key(device_path: &str) -> Option<String> {
    let p = device_path.trim_start_matches(r"\\?\");
    let parts: Vec<&str> = p.split('#').collect();
    if parts.len() < 3 || !parts[0].eq_ignore_ascii_case("DISPLAY") {
        return None;
    }
    let ok = |s: &str| !s.is_empty() && !s.contains(['\\', '/']);
    (ok(parts[1]) && ok(parts[2])).then(|| format!(r"HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\{}\{}\Device Parameters", parts[1], parts[2]))
}

/// El tipo de cable en español, según `outputTechnology` de Windows.
pub fn connection_name(tech: i32) -> &'static str {
    match tech {
        0 => "VGA",
        1 => "S-Video",
        2 => "Vídeo compuesto",
        3 => "Vídeo por componentes",
        4 => "DVI",
        5 => "HDMI",
        6 | 11 | 13 => "Interna",
        8 => "D-Jpn",
        9 => "SDI",
        10 => "DisplayPort",
        12 => "UDI",
        14 => "Adaptador de TV",
        15 => "Miracast",
        16 => "Por USB o red",
        17 => "Virtual",
        18 => "USB-C",
        i32::MIN => "Interna",
        _ => "Otra conexión",
    }
}

/// Los Hz de una fracción (60000/1000 → 60), redondeados.
pub fn hz_of(num: u32, den: u32) -> u32 {
    if den == 0 {
        0
    } else {
        ((f64::from(num) / f64::from(den)).round()) as u32
    }
}

/// «\\.\DISPLAY2» → 2.
pub fn display_number(gdi: &str) -> u32 {
    let digits: String = gdi.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().unwrap_or(0)
}

/// De entre los modos que ofrece el monitor para una resolución, los Hz más cercanos a los que se quieren
/// (a igual distancia, los más altos).
pub fn pick_hz(available: &[u32], want: u32) -> Option<u32> {
    available.iter().copied().filter(|&h| h > 0).min_by_key(|&h| (h.abs_diff(want), std::cmp::Reverse(h)))
}

// ---------- Cambio pendiente de confirmar ----------

struct Pending {
    gen: u64,
    snapshot: win::Snapshot,
    persist: Option<win::ModeChange>,
    until_ms: u64,
    title: String,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
/// Un solo cambio de pantalla a la vez: comprobar el pendiente, cambiar y guardar el pendiente van bajo este cerrojo.
/// Siempre se toma antes que `PENDING`.
static APPLY: Mutex<()> = Mutex::new(());
static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

fn pending_lock() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

fn apply_lock() -> std::sync::MutexGuard<'static, ()> {
    APPLY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Al cerrar AdminOps con un cambio sin confirmar se deshace antes de salir: un cambio temporal de
/// resolución, si no, se queda hasta que se cierre la sesión.
pub fn restore_pending_on_exit(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(p) = pending_lock().take() else { return };
    let result = win::restore(&p.snapshot);
    match &result {
        Ok(()) => log::info!("Pantallas: se cerró AdminOps con un cambio sin confirmar; vuelven a estar como antes"),
        Err(e) => log::warn!("Pantallas: se cerró AdminOps con un cambio sin confirmar y no se pudo deshacer: {e}"),
    }
    if let Some(state) = app.try_state::<TweakState>() {
        state.record(Op::Run, "Pantallas: vuelta a como estaban (se cerró AdminOps sin confirmar)", &result);
    }
}

// ---------- Piezas puras (con pruebas) ----------

/// Dónde está cada monitor (para comprobar que una disposición volvió a ser la de antes).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Placement {
    /// Adaptador (parte baja y alta) y número del monitor.
    pub target: (u32, i32, u32),
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// ¿Son la misma disposición (sin importar el orden)?
pub fn same_layout(a: &[Placement], b: &[Placement]) -> bool {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    a.sort();
    b.sort();
    a == b
}

/// Con las pantallas duplicadas, dos monitores salen con la fuente principal: solo el primero es «Principal».
pub fn keep_one_primary(monitors: &mut [Monitor]) {
    let mut seen = false;
    for m in monitors.iter_mut() {
        if m.primary {
            if seen {
                m.primary = false;
            }
            seen = true;
        }
    }
}

/// La resolución que se pone y sus Hz: de entre los modos de la resolución propia derecha o girada. En una
/// pantalla en vertical (90 o 270 grados) Windows cuenta el ancho y el alto girados, así que se prueba primero girada.
pub fn pick_mode(straight: &[u32], turned: &[u32], want_hz: u32, w: u32, h: u32, vertical: bool) -> Option<(u32, u32, u32)> {
    let s = pick_hz(straight, want_hz).map(|hz| (w, h, hz));
    let t = pick_hz(turned, want_hz).map(|hz| (h, w, hz));
    if vertical {
        t.or(s)
    } else {
        s.or(t)
    }
}

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_LWIN: u16 = 0x5B;
const VK_B: u16 = 0x42;
/// Las teclas que, si están pulsadas, se suman al atajo y lo estropean (Mayús, Ctrl, Alt y Win, izquierdas y derechas).
pub const MODIFIER_KEYS: [u16; 8] = [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C];

/// Los eventos del atajo Win + Ctrl + Mayús + B: primero se sueltan las teclas que estaban pulsadas (`held`).
/// Cada evento es (tecla, soltar).
pub fn driver_keys(held: &[u16]) -> Vec<(u16, bool)> {
    let mut v: Vec<(u16, bool)> = held.iter().map(|&k| (k, true)).collect();
    v.extend([(VK_LWIN, false), (VK_CONTROL, false), (VK_SHIFT, false), (VK_B, false), (VK_B, true), (VK_SHIFT, true), (VK_CONTROL, true), (VK_LWIN, true)]);
    v
}

/// Soltar todo lo que el atajo pudo dejar pulsado.
pub fn release_keys() -> [(u16, bool); 4] {
    [(VK_B, true), (VK_SHIFT, true), (VK_CONTROL, true), (VK_LWIN, true)]
}

/// Windows avisa de que cambió la pantalla (se conectó o se quitó un monitor, otra resolución…).
pub fn on_display_change() {
    use tauri::Emitter;
    if let Some(app) = APP.get() {
        let _ = app.emit("displays-changed", ());
    }
}

/// Guarda la app para poder avisar a la interfaz (la ventana oculta de `connected` llama a `on_display_change`).
pub fn start(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

// ---------- Windows ----------

#[cfg(windows)]
mod win {
    use super::{connection_name, display_number, driver_keys, hz_of, keep_one_primary, maker_name, display_name, parse_edid, pick_mode, edid_key, release_keys, same_layout, Edid, Monitor, Placement, MODIFIER_KEYS};
    use std::collections::HashSet;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Devices::Display::*;
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, POINT};
    use windows_sys::Win32::Graphics::Gdi::{
        ChangeDisplaySettingsExW, EnumDisplayDevicesW, EnumDisplaySettingsExW, MonitorFromPoint, CDS_TEST, CDS_UPDATEREGISTRY, DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_PRIMARY_DEVICE,
        DISP_CHANGE_SUCCESSFUL, DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP};

    /// La configuración completa de las pantallas, tal cual la da Windows.
    #[derive(Clone)]
    pub struct Snapshot {
        paths: Vec<DISPLAYCONFIG_PATH_INFO>,
        modes: Vec<DISPLAYCONFIG_MODE_INFO>,
    }

    /// Un cambio de resolución hecho de forma temporal, para dejarlo fijo si se confirma.
    #[derive(Clone)]
    pub struct ModeChange {
        device: Vec<u16>,
        mode: DEVMODEW,
    }

    /// Cuántos caminos y modos se aceptan: más que cualquier equipo real; protege la reserva de memoria.
    const MAX_ELEMENTS: u32 = 1024;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    /// Los caminos y modos activos.
    pub fn query() -> Result<Snapshot, String> {
        for _ in 0..3 {
            let (mut np, mut nm) = (0u32, 0u32);
            let rc = unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) };
            if rc != 0 {
                return Err(format!("Windows no dio la lista de pantallas (código {rc})."));
            }
            if np == 0 {
                return Err("Windows no tiene ninguna pantalla activa.".into());
            }
            if np > MAX_ELEMENTS || nm > MAX_ELEMENTS {
                return Err("Windows dio una lista de pantallas demasiado grande.".into());
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
            // Con solo los caminos activos, el último parámetro (topología) tiene que ser nulo.
            let rc = unsafe { QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), null_mut()) };
            if rc == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if rc != 0 {
                return Err(format!("Windows no dio la configuración de pantallas (código {rc})."));
            }
            paths.truncate(np as usize);
            modes.truncate(nm as usize);
            return Ok(Snapshot { paths, modes });
        }
        Err("La configuración de pantallas cambiaba mientras se leía: vuelve a intentarlo.".into())
    }

    fn target_name(p: &DISPLAYCONFIG_PATH_INFO) -> Option<DISPLAYCONFIG_TARGET_DEVICE_NAME> {
        let mut t = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        t.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        t.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
        t.header.adapterId = p.targetInfo.adapterId;
        t.header.id = p.targetInfo.id;
        (unsafe { DisplayConfigGetDeviceInfo(&mut t.header) } == 0).then_some(t)
    }

    fn source_name(p: &DISPLAYCONFIG_PATH_INFO) -> Option<String> {
        let mut s = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
        s.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
        s.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
        s.header.adapterId = p.sourceInfo.adapterId;
        s.header.id = p.sourceInfo.id;
        (unsafe { DisplayConfigGetDeviceInfo(&mut s.header) } == 0).then(|| from_wide(&s.viewGdiDeviceName))
    }

    /// La resolución propia que dice el driver (puede venir de extensiones del EDID).
    fn preferred_mode(p: &DISPLAYCONFIG_PATH_INFO) -> Option<(u32, u32)> {
        let mut t = DISPLAYCONFIG_TARGET_PREFERRED_MODE::default();
        t.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_PREFERRED_MODE;
        t.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_PREFERRED_MODE>() as u32;
        t.header.adapterId = p.targetInfo.adapterId;
        t.header.id = p.targetInfo.id;
        if unsafe { DisplayConfigGetDeviceInfo(&mut t.header) } != 0 {
            return None;
        }
        (t.width > 0 && t.height > 0 && t.width < 32_768 && t.height < 32_768).then_some((t.width, t.height))
    }

    fn read_edid(device_path: &str) -> Option<Edid> {
        let key = edid_key(device_path)?;
        let raw = crate::tweaks::registry::read_raw(&key, "EDID")?;
        parse_edid(&raw.bytes)
    }

    fn scale_at(x: i32, y: i32, w: u32, h: u32) -> u32 {
        let pt = POINT { x: x.saturating_add((w / 2) as i32), y: y.saturating_add((h / 2) as i32) };
        let hm = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
        if hm.is_null() {
            return 100;
        }
        let (mut dx, mut dy) = (0u32, 0u32);
        if unsafe { GetDpiForMonitor(hm, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) } != 0 || dx == 0 {
            return 100;
        }
        (dx * 100 + 48) / 96
    }

    /// El nombre GDI de la pantalla principal de Windows (`DISPLAY_DEVICE_PRIMARY_DEVICE`).
    fn primary_gdi() -> Option<String> {
        for i in 0..64u32 {
            // SAFETY: estructura a ceros con su tamaño puesto, como pide la función.
            let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
            dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            if unsafe { EnumDisplayDevicesW(null(), i, &mut dd, 0) } == 0 {
                break;
            }
            if dd.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0 {
                return Some(from_wide(&dd.DeviceName));
            }
        }
        None
    }

    /// Dónde está cada monitor de una configuración.
    fn placements(s: &Snapshot) -> Vec<Placement> {
        s.paths
            .iter()
            .filter_map(|p| {
                let idx = unsafe { p.sourceInfo.Anonymous.modeInfoIdx } as usize;
                let m = s.modes.get(idx).filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE)?;
                let sm = unsafe { m.Anonymous.sourceMode };
                Some(Placement { target: (p.targetInfo.adapterId.LowPart, p.targetInfo.adapterId.HighPart, p.targetInfo.id), x: sm.position.x, y: sm.position.y, width: sm.width, height: sm.height })
            })
            .collect()
    }

    /// Los monitores activos y si hay dos que enseñan lo mismo.
    pub fn monitors() -> Result<(Vec<Monitor>, bool), String> {
        let snap = query()?;
        let primary_source = primary_gdi();
        let mut out = Vec::new();
        let mut sources = HashSet::new();
        let mut duplicated = false;
        for p in &snap.paths {
            let key = (p.sourceInfo.adapterId.LowPart, p.sourceInfo.adapterId.HighPart, p.sourceInfo.id);
            if !sources.insert(key) {
                duplicated = true;
            }
            // Sin modo de origen válido no se sabe ni la resolución: ese camino no se enseña.
            let src_idx = unsafe { p.sourceInfo.Anonymous.modeInfoIdx } as usize;
            let Some(src) = snap.modes.get(src_idx).filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE) else { continue };
            let sm = unsafe { src.Anonymous.sourceMode };
            let tgt_idx = unsafe { p.targetInfo.Anonymous.modeInfoIdx } as usize;
            let signal = snap.modes.get(tgt_idx).filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_TARGET).map(|m| unsafe { m.Anonymous.targetMode.targetVideoSignalInfo });
            let mut hz = hz_of(p.targetInfo.refreshRate.Numerator, p.targetInfo.refreshRate.Denominator);
            if hz == 0 {
                if let Some(s) = &signal {
                    hz = hz_of(s.vSyncFreq.Numerator, s.vSyncFreq.Denominator);
                }
            }
            let name = target_name(p);
            let device_path = name.as_ref().map(|t| from_wide(&t.monitorDevicePath)).unwrap_or_default();
            let edid = read_edid(&device_path);
            let friendly = name.as_ref().map(|t| from_wide(&t.monitorFriendlyDeviceName)).unwrap_or_default();
            let generic = friendly.is_empty() || friendly.to_lowercase().contains("generic") || friendly.to_lowercase().contains("genérico");
            let maker = edid.as_ref().map(|e| maker_name(&e.maker)).unwrap_or_default();
            let model = edid.as_ref().and_then(|e| e.name.clone()).or_else(|| (!generic).then(|| friendly.clone())).unwrap_or_default();
            let tech = p.targetInfo.outputTechnology;
            let connection = connection_name(tech).to_string();
            let shown = display_name(&maker, &model);
            let shown = if shown.is_empty() {
                if connection == "Interna" { "Pantalla integrada".to_string() } else { "Monitor sin nombre".to_string() }
            } else {
                shown
            };
            // La propia: primero lo que dice el driver; si no, la primera temporización del EDID.
            let native = preferred_mode(p).or_else(|| edid.as_ref().and_then(|e| e.native.map(|(w, h, _)| (w, h))));
            let native_hz = edid.as_ref().and_then(|e| e.native.map(|(_, _, hz)| hz)).filter(|&h| h > 0);
            let max_hz = edid.as_ref().and_then(|e| e.max_hz.into_iter().chain(e.native.map(|(_, _, h)| h)).max()).filter(|&h| h > 0);
            let gdi = source_name(p).unwrap_or_default();
            // La fuente principal; si Windows no la dice, la que está en el origen.
            let primary = match &primary_source {
                Some(ps) => !gdi.is_empty() && gdi.eq_ignore_ascii_case(ps),
                None => sm.position.x == 0 && sm.position.y == 0,
            };
            out.push(Monitor {
                id: if device_path.is_empty() { format!("destino-{}-{}", p.targetInfo.adapterId.LowPart, p.targetInfo.id) } else { device_path },
                name: shown,
                maker,
                model,
                number: display_number(&gdi),
                gdi,
                width: sm.width,
                height: sm.height,
                x: sm.position.x,
                y: sm.position.y,
                hz,
                max_hz,
                native_width: native.map(|n| n.0),
                native_height: native.map(|n| n.1),
                native_hz,
                connection,
                scale: scale_at(sm.position.x, sm.position.y, sm.width, sm.height),
                primary,
                rotation: match p.targetInfo.rotation {
                    DISPLAYCONFIG_ROTATION_ROTATE90 => 90,
                    DISPLAYCONFIG_ROTATION_ROTATE180 => 180,
                    DISPLAYCONFIG_ROTATION_ROTATE270 => 270,
                    _ => 0,
                },
            });
        }
        out.sort_by_key(|m| (!m.primary, m.number, m.x));
        // Duplicados: varios monitores comparten la fuente principal y solo uno es «Principal».
        keep_one_primary(&mut out);
        Ok((out, duplicated))
    }

    pub fn snapshot() -> Result<Snapshot, String> {
        query()
    }

    /// Vuelve a poner una configuración guardada. Si Windows no la acepta, vuelve a lo guardado en el registro.
    pub fn restore(s: &Snapshot) -> Result<(), String> {
        let np = s.paths.len() as u32;
        let nm = s.modes.len() as u32;
        let base = SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES;
        let mut rc = unsafe { SetDisplayConfig(np, s.paths.as_ptr(), nm, s.modes.as_ptr(), base | SDC_SAVE_TO_DATABASE) };
        if rc != 0 {
            rc = unsafe { SetDisplayConfig(np, s.paths.as_ptr(), nm, s.modes.as_ptr(), base) };
        }
        if rc == 0 {
            return Ok(());
        }
        // Último recurso: la resolución guardada en el registro (deshace un cambio temporal).
        let back = unsafe { ChangeDisplaySettingsExW(null(), null(), null_mut(), 0, null()) };
        if back == DISP_CHANGE_SUCCESSFUL {
            // Eso solo devuelve la resolución: si se había extendido o duplicado, hay que comprobar que la disposición volvió.
            match query() {
                Ok(now) if same_layout(&placements(&now), &placements(s)) => Ok(()),
                _ => Err("Windows dejó la resolución guardada, pero los monitores no volvieron a la disposición de antes (extendidos, duplicados o en otro sitio): ponlos como estaban en Configuración → Pantalla.".into()),
            }
        } else {
            Err(format!("Windows no dejó volver a la configuración anterior (código {rc})."))
        }
    }

    pub fn set_topology(extend: bool) -> Result<(), String> {
        let flag = if extend { SDC_TOPOLOGY_EXTEND } else { SDC_TOPOLOGY_CLONE };
        let rc = unsafe { SetDisplayConfig(0, null(), 0, null(), SDC_APPLY | flag) };
        match rc {
            0 => Ok(()),
            87 => Err("Windows no puede hacer eso con las pantallas que hay ahora.".into()),
            n => Err(format!("Windows no cambió las pantallas (código {n}).")),
        }
    }

    fn mode_error(code: i32) -> String {
        match code {
            -2 => "La pantalla no admite ese modo.".into(),
            1 => "Hace falta reiniciar el equipo para cambiar la resolución.".into(),
            -1 => "Windows no pudo cambiar la resolución.".into(),
            n => format!("Windows no cambió la resolución (código {n})."),
        }
    }

    fn blank_mode() -> DEVMODEW {
        let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        dm
    }

    /// Pone una resolución en un monitor, de forma temporal (hasta confirmarla). Elige los Hz más
    /// parecidos a `want_hz` entre los que ofrece el monitor para esa resolución.
    pub fn set_mode(gdi: &str, w: u32, h: u32, want_hz: u32, rotation: u32) -> Result<ModeChange, String> {
        if gdi.is_empty() {
            return Err("No se sabe qué pantalla de Windows es esa.".into());
        }
        let device = wide(gdi);
        let mut current = blank_mode();
        if unsafe { EnumDisplaySettingsExW(device.as_ptr(), ENUM_CURRENT_SETTINGS, &mut current, 0) } == 0 {
            return Err("Windows no dio el modo actual de la pantalla.".into());
        }
        // Los Hz que ofrece para esa resolución (o para la girada, si la pantalla está en vertical).
        let mut straight = Vec::new();
        let mut turned = Vec::new();
        for i in 0..8192u32 {
            let mut m = blank_mode();
            if unsafe { EnumDisplaySettingsExW(device.as_ptr(), i, &mut m, 0) } == 0 {
                break;
            }
            if m.dmBitsPerPel < 32 {
                continue;
            }
            if m.dmPelsWidth == w && m.dmPelsHeight == h {
                straight.push(m.dmDisplayFrequency);
            } else if m.dmPelsWidth == h && m.dmPelsHeight == w {
                turned.push(m.dmDisplayFrequency);
            }
        }
        // En vertical (90 o 270 grados) Windows cuenta el ancho y el alto girados: se prueba primero girada.
        let Some((nw, nh, hz)) = pick_mode(&straight, &turned, want_hz, w, h, rotation == 90 || rotation == 270) else {
            return Err(format!("Esta pantalla no ofrece {w}×{h} con el cable y el driver que tiene."));
        };
        let mut next = current;
        next.dmPelsWidth = nw;
        next.dmPelsHeight = nh;
        next.dmDisplayFrequency = hz;
        next.dmFields |= DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
        let test = unsafe { ChangeDisplaySettingsExW(device.as_ptr(), &next, null_mut(), CDS_TEST, null()) };
        if test != DISP_CHANGE_SUCCESSFUL {
            return Err(mode_error(test));
        }
        let rc = unsafe { ChangeDisplaySettingsExW(device.as_ptr(), &next, null_mut(), 0, null()) };
        if rc != DISP_CHANGE_SUCCESSFUL {
            return Err(mode_error(rc));
        }
        Ok(ModeChange { device, mode: next })
    }

    /// Deja fijo (en el registro) un cambio de resolución que era temporal.
    pub fn persist(c: &ModeChange) -> Result<(), String> {
        let rc = unsafe { ChangeDisplaySettingsExW(c.device.as_ptr(), &c.mode, null_mut(), CDS_UPDATEREGISTRY, null()) };
        if rc == DISP_CHANGE_SUCCESSFUL {
            Ok(())
        } else {
            Err(mode_error(rc))
        }
    }

    /// Win + Ctrl + Mayús + B: Windows reinicia el driver gráfico (la pantalla parpadea un segundo).
    /// Antes se sueltan las teclas Alt, Ctrl, Mayús o Win que estén pulsadas (si no, se suman al atajo); si Windows
    /// no deja meter todas las teclas, se sueltan las del atajo y se dice que no se hizo.
    pub fn restart_driver() -> Result<(), String> {
        let to_input = |&(vk, up): &(u16, bool)| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } } };
        let send = |keys: &[INPUT]| unsafe { SendInput(keys.len() as u32, keys.as_ptr(), std::mem::size_of::<INPUT>() as i32) } as usize;
        // El bit alto de GetAsyncKeyState (valor negativo) dice que la tecla está pulsada ahora.
        let held: Vec<u16> = MODIFIER_KEYS.iter().copied().filter(|&vk| unsafe { GetAsyncKeyState(i32::from(vk)) } < 0).collect();
        let keys: Vec<INPUT> = driver_keys(&held).iter().map(to_input).collect();
        if send(&keys) == keys.len() {
            Ok(())
        } else {
            // Que no quede ninguna tecla del atajo pulsada.
            let ups: Vec<INPUT> = release_keys().iter().map(to_input).collect();
            send(&ups);
            Err("Windows no dejó pulsar el atajo entero: el driver gráfico no se ha reiniciado.".into())
        }
    }
}

#[cfg(not(windows))]
mod win {
    use super::Monitor;

    #[derive(Clone)]
    pub struct Snapshot;
    #[derive(Clone)]
    pub struct ModeChange;

    const ONLY_WINDOWS: &str = "Solo funciona en Windows.";

    pub fn monitors() -> Result<(Vec<Monitor>, bool), String> {
        Err(ONLY_WINDOWS.into())
    }
    pub fn snapshot() -> Result<Snapshot, String> {
        Err(ONLY_WINDOWS.into())
    }
    pub fn restore(_: &Snapshot) -> Result<(), String> {
        Err(ONLY_WINDOWS.into())
    }
    pub fn set_topology(_: bool) -> Result<(), String> {
        Err(ONLY_WINDOWS.into())
    }
    pub fn set_mode(_: &str, _: u32, _: u32, _: u32, _: u32) -> Result<ModeChange, String> {
        Err(ONLY_WINDOWS.into())
    }
    pub fn persist(_: &ModeChange) -> Result<(), String> {
        Err(ONLY_WINDOWS.into())
    }
    pub fn restart_driver() -> Result<(), String> {
        Err(ONLY_WINDOWS.into())
    }
}

// ---------- Comandos ----------

/// Los monitores activos, con lo que hace falta para decir si van bien.
pub fn list() -> Result<DisplayList, String> {
    let (monitors, duplicated) = win::monitors()?;
    let pending = pending_lock().as_ref().map(|p| PendingInfo { until_ms: p.until_ms, title: p.title.clone() });
    Ok(DisplayList { monitors, duplicated, pending })
}

#[tauri::command(async)]
pub fn displays_list() -> Result<DisplayList, String> {
    list()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Reverted {
    ok: bool,
    message: String,
}

/// Cuenta atrás en Rust: si nadie confirma ni deshace, vuelve la configuración guardada.
fn arm(app: tauri::AppHandle, gen: u64, deadline: Instant) {
    std::thread::spawn(move || {
        use tauri::{Emitter, Manager};
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let guard = pending_lock();
            if !matches!(guard.as_ref(), Some(p) if p.gen == gen) {
                return; // ya se confirmó, se deshizo o lo sustituyó otro cambio
            }
            if Instant::now() < deadline {
                continue;
            }
            drop(guard);
            // Sin un cambio a medias: se espera al que esté en marcha y se vuelve a mirar si sigue siendo este.
            let _apply = apply_lock();
            let p = {
                let mut g = pending_lock();
                if matches!(g.as_ref(), Some(p) if p.gen == gen) { g.take() } else { None }
            };
            let Some(p) = p else { return };
            let result = win::restore(&p.snapshot);
            let message = match &result {
                Ok(()) => "No se confirmó: las pantallas vuelven a estar como antes.".to_string(),
                Err(e) => e.clone(),
            };
            app.state::<TweakState>().record(Op::Run, "Pantallas: vuelta a como estaban (sin confirmar)", &result);
            let _ = app.emit("display-reverted", Reverted { ok: result.is_ok(), message });
            return;
        }
    });
}

/// Cambia la pantalla y deja 15 s para confirmarlo. `action`: `native`, `extend` o `clone`.
#[tauri::command(async)]
pub fn display_apply(app: tauri::AppHandle, action: String, id: Option<String>, state: State<'_, TweakState>) -> Result<DisplayChange, String> {
    // Comprobar el pendiente, cambiar la pantalla y guardar el pendiente, sin que otra llamada se cuele en medio.
    let _apply = apply_lock();
    if pending_lock().is_some() {
        return Err("Hay un cambio sin confirmar: dile «Mantener» o «Deshacer» primero.".into());
    }
    let snapshot = win::snapshot()?;
    let mut persist = None;
    let title = match action.as_str() {
        "native" => {
            let id = id.ok_or("Falta el monitor.")?;
            let (monitors, _) = win::monitors()?;
            let m = monitors.iter().find(|m| m.id == id).ok_or("Ese monitor ya no está conectado.")?;
            let (w, h) = m.native_width.zip(m.native_height).ok_or("No se sabe cuál es la resolución propia de este monitor.")?;
            let want = m.native_hz.or((m.hz > 0).then_some(m.hz)).unwrap_or(60);
            let title = format!("Pantallas: poner {w}×{h} en {}", m.name);
            match win::set_mode(&m.gdi, w, h, want, m.rotation) {
                Ok(c) => persist = Some(c),
                Err(e) => {
                    state.record(Op::Run, &title, &Err::<(), _>(e.clone()));
                    return Err(e);
                }
            }
            title
        }
        "extend" | "clone" => {
            let extend = action == "extend";
            let title = if extend { "Pantallas: extender el escritorio" } else { "Pantallas: duplicar la imagen" };
            let r = win::set_topology(extend);
            state.record(Op::Run, title, &r);
            r?;
            title.to_string()
        }
        _ => return Err(format!("Acción desconocida: {action}")),
    };
    if persist.is_some() {
        state.record(Op::Run, &title, &Ok::<(), String>(()));
    }
    let gen = GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let until_ms = now_ms() + CONFIRM_SECS * 1000;
    *pending_lock() = Some(Pending { gen, snapshot, persist, until_ms, title: title.clone() });
    arm(app, gen, Instant::now() + Duration::from_secs(CONFIRM_SECS));
    Ok(DisplayChange { message: format!("{title}. ¿Se ve bien? Si no confirmas, vuelve como estaba en {CONFIRM_SECS} s."), until_ms, seconds: CONFIRM_SECS })
}

/// «Mantener»: el cambio se queda.
#[tauri::command(async)]
pub fn display_confirm() -> Result<String, String> {
    let _apply = apply_lock();
    let mut pending = pending_lock();
    let p = pending.as_ref().ok_or("Ya no hay ningún cambio por confirmar.")?;
    if let Some(c) = &p.persist {
        // Si no se puede dejar fijo, el cambio sigue pendiente (y la cuenta atrás sigue): se deshará solo si nadie lo arregla.
        win::persist(c).map_err(|e| format!("{e} El cambio sigue sin confirmar: se deshará solo a su hora, o pulsa «Deshacer»."))?;
    }
    *pending = None;
    Ok("Cambio mantenido.".into())
}

/// «Deshacer»: vuelve la configuración de antes del cambio.
#[tauri::command(async)]
pub fn display_undo(state: State<'_, TweakState>) -> Result<String, String> {
    let _apply = apply_lock();
    let p = pending_lock().take().ok_or("Ya no hay ningún cambio que deshacer.")?;
    let r = win::restore(&p.snapshot);
    state.record(Op::Run, "Pantallas: cambio deshecho", &r);
    r.map(|_| "Pantallas como estaban.".to_string())
}

/// Reinicia el driver gráfico con el atajo de Windows (la pantalla parpadea un segundo).
#[tauri::command(async)]
pub fn display_restart_driver(state: State<'_, TweakState>) -> Result<String, String> {
    let r = win::restart_driver();
    state.record(Op::Run, "Pantallas: reiniciar el driver gráfico", &r);
    r.map(|_| "Driver gráfico reiniciado: la pantalla parpadea un segundo.".to_string())
}

// ---------- «Ver el comando» ----------

/// La resolución que se pide («1920x1080») como dos números, o error.
pub fn parse_size(context: &str) -> Result<(u32, u32), String> {
    let (w, h) = context.split_once(['x', '×']).ok_or("Falta la resolución.")?;
    let (w, h): (u32, u32) = (w.trim().parse().map_err(|_| "La resolución no es válida.")?, h.trim().parse().map_err(|_| "La resolución no es válida.")?);
    if !(320..=16_384).contains(&w) || !(200..=16_384).contains(&h) {
        return Err("La resolución no es válida.".into());
    }
    Ok((w, h))
}

/// El comando equivalente de una acción de pantallas (`native`, `extend`, `clone`, `restart-driver`).
/// Los tres últimos usan lo mismo que AdminOps; poner una resolución se hace en PowerShell con la misma
/// función de Windows (`ChangeDisplaySettingsEx`), que no tiene un cmdlet propio.
pub fn action_script(action: &str, context: &str) -> Result<(&'static str, String), String> {
    match action {
        "extend" => Ok((
            "Extender el escritorio a todas las pantallas",
            "# Extender el escritorio a todas las pantallas (lo mismo que Win + P → Extender)\n\n# DisplaySwitch.exe es el programa de Windows que cambia el modo de proyección\nDisplaySwitch.exe /extend\n".into(),
        )),
        "clone" => Ok((
            "Duplicar la imagen en las pantallas",
            "# Duplicar la imagen en todas las pantallas (lo mismo que Win + P → Duplicar)\n\n# DisplaySwitch.exe es el programa de Windows que cambia el modo de proyección\nDisplaySwitch.exe /clone\n".into(),
        )),
        "restart-driver" => Ok((
            "Reiniciar el driver gráfico",
            "# Reiniciar el driver gráfico: Windows lo hace con Win + Ctrl + Mayús + B\n\
             # (la pantalla parpadea un segundo)\n\
             Add-Type -Namespace AdminOps -Name Teclas -MemberDefinition '[System.Runtime.InteropServices.DllImport(\"user32.dll\")] public static extern void keybd_event(byte vk, byte scan, int flags, int extra);'\n\
             $abajo = 0; $arriba = 2\n\
             # Win, Ctrl y Mayús pulsadas, luego la B\n\
             foreach ($k in 0x5B, 0x11, 0x10, 0x42) { [AdminOps.Teclas]::keybd_event($k, 0, $abajo, 0) }\n\
             # Y se sueltan al revés\n\
             foreach ($k in 0x42, 0x10, 0x11, 0x5B) { [AdminOps.Teclas]::keybd_event($k, 0, $arriba, 0) }\n"
                .into(),
        )),
        "native" => {
            let (w, h) = parse_size(context)?;
            let script = format!(
                "# Poner la resolución propia del monitor principal ({w}×{h})\n\
                 # Windows no trae un cmdlet para esto: se usa la misma función de Windows que usa AdminOps\n\
                 $ancho = {w}; $alto = {h}\n\
                 Add-Type -Namespace AdminOps -Name Pantalla -MemberDefinition @'\n\
                 [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]\n\
                 public struct DEVMODE {{\n\
                 \x20 [System.Runtime.InteropServices.MarshalAs(System.Runtime.InteropServices.UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;\n\
                 \x20 public short dmSpecVersion; public short dmDriverVersion; public short dmSize; public short dmDriverExtra; public int dmFields;\n\
                 \x20 public int dmPositionX; public int dmPositionY; public int dmDisplayOrientation; public int dmDisplayFixedOutput;\n\
                 \x20 public short dmColor; public short dmDuplex; public short dmYResolution; public short dmTTOption; public short dmCollate;\n\
                 \x20 [System.Runtime.InteropServices.MarshalAs(System.Runtime.InteropServices.UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;\n\
                 \x20 public short dmLogPixels; public int dmBitsPerPel; public int dmPelsWidth; public int dmPelsHeight; public int dmDisplayFlags; public int dmDisplayFrequency;\n\
                 \x20 public int dmICMMethod; public int dmICMIntent; public int dmMediaType; public int dmDitherType; public int dmReserved1; public int dmReserved2; public int dmPanningWidth; public int dmPanningHeight;\n\
                 }}\n\
                 [System.Runtime.InteropServices.DllImport(\"user32.dll\", CharSet = System.Runtime.InteropServices.CharSet.Unicode)] public static extern bool EnumDisplaySettings(string dispositivo, int modo, ref DEVMODE dm);\n\
                 [System.Runtime.InteropServices.DllImport(\"user32.dll\", CharSet = System.Runtime.InteropServices.CharSet.Unicode)] public static extern int ChangeDisplaySettings(ref DEVMODE dm, int flags);\n\
                 '@\n\
                 $dm = New-Object AdminOps.Pantalla+DEVMODE\n\
                 $dm.dmSize = [System.Runtime.InteropServices.Marshal]::SizeOf($dm)\n\
                 # El modo actual de la pantalla principal, con el ancho y el alto pedidos\n\
                 [void][AdminOps.Pantalla]::EnumDisplaySettings($null, -1, [ref]$dm)\n\
                 $dm.dmPelsWidth = $ancho; $dm.dmPelsHeight = $alto\n\
                 $dm.dmFields = 0x80000 -bor 0x100000\n\
                 # 0 = cambio temporal; 1 lo deja fijo\n\
                 $r = [AdminOps.Pantalla]::ChangeDisplaySettings([ref]$dm, 0)\n\
                 if ($r -ne 0) {{ throw \"Windows no cambió la resolución (código $r).\" }}\n"
            );
            Ok(("Poner la resolución propia del monitor", script))
        }
        _ => Err(format!("Acción desconocida: display.{action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un EDID de un LG 24MK430 (bloque base) armado a mano: 1920×1080 a 60 Hz, de 56 a 75 Hz.
    fn sample() -> Vec<u8> {
        let mut b = vec![0u8; 128];
        b[..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        // GSM: G = 7, S = 19, M = 13.
        let m: u16 = (7 << 10) | (19 << 5) | 13;
        b[8..10].copy_from_slice(&m.to_be_bytes());
        b[10..12].copy_from_slice(&0x5B09u16.to_le_bytes());
        // Temporización 1: reloj de 148,5 MHz, 1920 + 280 por 1080 + 45.
        b[54..72].copy_from_slice(&[0x02, 0x3A, 0x80, 0x18, 0x71, 0x38, 0x2D, 0x40, 0x58, 0x2C, 0x45, 0x00, 0, 0, 0, 0, 0, 0x1E]);
        // Nombre.
        let mut name = [0x20u8; 18];
        name[..5].copy_from_slice(&[0, 0, 0, 0xFC, 0]);
        name[5..12].copy_from_slice(b"24MK430");
        name[12] = b'\n';
        b[72..90].copy_from_slice(&name);
        // Rango: de 56 a 75 Hz.
        b[90..108].copy_from_slice(&[0, 0, 0, 0xFD, 0, 56, 75, 30, 83, 0x0A, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]);
        b
    }

    #[test]
    fn el_edid_dice_marca_modelo_y_resolucion_propia() {
        let e = parse_edid(&sample()).expect("EDID válido");
        assert_eq!(e.maker, "GSM");
        assert_eq!(e.product, 0x5B09);
        assert_eq!(e.name.as_deref(), Some("24MK430"));
        assert_eq!(e.native, Some((1920, 1080, 60)));
        assert_eq!(e.max_hz, Some(75));
        assert_eq!(display_name(&maker_name(&e.maker), e.name.as_deref().unwrap()), "LG 24MK430");
    }

    #[test]
    fn un_edid_roto_o_corto_no_se_acepta() {
        assert!(parse_edid(&[]).is_none());
        assert!(parse_edid(&[0u8; 128]).is_none());
        let mut corto = sample();
        corto.truncate(100);
        assert!(parse_edid(&corto).is_none());
    }

    #[test]
    fn sin_temporizacion_en_el_primer_descriptor_no_hay_resolucion_propia() {
        let mut b = sample();
        // El primer descriptor pasa a ser un nombre: no es una temporización.
        b[54..72].copy_from_slice(&[0, 0, 0, 0xFC, 0, b'X', b'\n', 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]);
        let e = parse_edid(&b).unwrap();
        assert_eq!(e.native, None);
    }

    #[test]
    fn el_maximo_de_hz_con_el_desplazamiento_de_255() {
        let mut b = sample();
        b[90..108].copy_from_slice(&[0, 0, 0, 0xFD, 0x02, 56, 0x2D, 30, 83, 0x0A, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]);
        assert_eq!(parse_edid(&b).unwrap().max_hz, Some(45 + 255));
    }

    #[test]
    fn las_marcas_y_los_nombres_se_dicen_bien() {
        assert_eq!(maker_name("DEL"), "Dell");
        assert_eq!(maker_name("SAM"), "Samsung");
        assert_eq!(maker_name("ZZZ"), "ZZZ");
        // Los paneles de portátil no tienen una marca que decir.
        assert_eq!(maker_name("BOE"), "");
        assert_eq!(display_name("Dell", "Dell P2419H"), "Dell P2419H");
        assert_eq!(display_name("", "Panel"), "Panel");
        assert_eq!(display_name("LG", ""), "LG");
    }

    #[test]
    fn la_clave_del_registro_sale_de_la_ruta_de_windows() {
        let k = edid_key(r"\\?\DISPLAY#GSM5B09#5&2b8a1f9c&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}").unwrap();
        assert_eq!(k, r"HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\GSM5B09\5&2b8a1f9c&0&UID4352\Device Parameters");
        assert!(edid_key(r"\\?\USB#VID_1#2#{x}").is_none());
        assert!(edid_key("DISPLAY#a").is_none());
        assert!(edid_key(r"\\?\DISPLAY#..\X#a#{g}").is_none());
    }

    #[test]
    fn el_cable_sale_en_espanol() {
        assert_eq!(connection_name(5), "HDMI");
        assert_eq!(connection_name(10), "DisplayPort");
        assert_eq!(connection_name(0), "VGA");
        assert_eq!(connection_name(i32::MIN), "Interna");
        assert_eq!(connection_name(15), "Miracast");
        assert_eq!(connection_name(99), "Otra conexión");
    }

    #[test]
    fn los_hz_y_el_numero_de_pantalla() {
        assert_eq!(hz_of(60000, 1000), 60);
        assert_eq!(hz_of(59940, 1000), 60);
        assert_eq!(hz_of(1, 0), 0);
        assert_eq!(display_number(r"\\.\DISPLAY2"), 2);
        assert_eq!(display_number(r"\\.\DISPLAY12"), 12);
        assert_eq!(display_number(""), 0);
    }

    #[test]
    fn se_eligen_los_hz_mas_cercanos() {
        assert_eq!(pick_hz(&[30, 60, 75], 60), Some(60));
        assert_eq!(pick_hz(&[30, 75], 60), Some(75));
        assert_eq!(pick_hz(&[50, 70], 60), Some(70));
        assert_eq!(pick_hz(&[0], 60), None);
        assert_eq!(pick_hz(&[], 60), None);
    }

    fn monitor(primary: bool) -> Monitor {
        Monitor {
            id: "m".into(),
            name: "M".into(),
            maker: String::new(),
            model: String::new(),
            number: 1,
            gdi: String::new(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            hz: 60,
            max_hz: None,
            native_width: None,
            native_height: None,
            native_hz: None,
            connection: "HDMI".into(),
            scale: 100,
            primary,
            rotation: 0,
        }
    }

    #[test]
    fn al_duplicar_solo_uno_es_el_principal() {
        let mut v = vec![monitor(true), monitor(true), monitor(false)];
        keep_one_primary(&mut v);
        assert_eq!(v.iter().map(|m| m.primary).collect::<Vec<_>>(), vec![true, false, false]);
        let mut sin = vec![monitor(false)];
        keep_one_primary(&mut sin);
        assert!(!sin[0].primary);
        let mut uno = vec![monitor(false), monitor(true)];
        keep_one_primary(&mut uno);
        assert_eq!(uno.iter().map(|m| m.primary).collect::<Vec<_>>(), vec![false, true]);
    }

    #[test]
    fn una_pantalla_en_vertical_prueba_primero_la_resolucion_girada() {
        // Horizontal: la derecha primero.
        assert_eq!(pick_mode(&[60], &[60], 60, 1920, 1080, false), Some((1920, 1080, 60)));
        // Vertical: Windows cuenta 1080×1920.
        assert_eq!(pick_mode(&[60], &[60], 60, 1920, 1080, true), Some((1080, 1920, 60)));
        // Si solo ofrece una de las dos, esa.
        assert_eq!(pick_mode(&[], &[75], 60, 1920, 1080, false), Some((1080, 1920, 75)));
        assert_eq!(pick_mode(&[50], &[], 60, 1920, 1080, true), Some((1920, 1080, 50)));
        assert_eq!(pick_mode(&[], &[], 60, 1920, 1080, true), None);
    }

    #[test]
    fn la_disposicion_es_la_misma_aunque_cambie_el_orden() {
        let a = Placement { target: (1, 0, 100), x: 0, y: 0, width: 1920, height: 1080 };
        let b = Placement { target: (1, 0, 200), x: 1920, y: 0, width: 1280, height: 1024 };
        assert!(same_layout(&[a.clone(), b.clone()], &[b.clone(), a.clone()]));
        // Duplicados: el segundo monitor en el mismo sitio que el primero no es la misma disposición.
        let b_clon = Placement { x: 0, ..b.clone() };
        assert!(!same_layout(&[a.clone(), b.clone()], &[a.clone(), b_clon]));
        assert!(!same_layout(std::slice::from_ref(&a), &[a.clone(), b]));
        assert!(same_layout(&[], &[]));
    }

    #[test]
    fn el_atajo_del_driver_suelta_primero_lo_que_estaba_pulsado() {
        let limpio = driver_keys(&[]);
        assert_eq!(limpio.len(), 8);
        assert_eq!(limpio[0], (0x5B, false));
        assert_eq!(limpio[3], (0x42, false));
        assert_eq!(limpio[7], (0x5B, true));
        // Con Alt izquierda pulsada, lo primero es soltarla.
        let con_alt = driver_keys(&[0xA4]);
        assert_eq!(con_alt.len(), 9);
        assert_eq!(con_alt[0], (0xA4, true));
        assert_eq!(&con_alt[1..], &limpio[..]);
        // Soltar lo del atajo: todo hacia arriba.
        assert!(release_keys().iter().all(|&(_, up)| up));
        assert!(MODIFIER_KEYS.contains(&0xA4) && MODIFIER_KEYS.contains(&0x5C));
    }

    #[test]
    fn la_resolucion_del_comando_se_valida() {
        assert_eq!(parse_size("1920x1080").unwrap(), (1920, 1080));
        assert_eq!(parse_size("1280×1024").unwrap(), (1280, 1024));
        assert!(parse_size("1920").is_err());
        assert!(parse_size("axb").is_err());
        assert!(parse_size("1x1").is_err());
        assert!(parse_size("1920x1080; calc").is_err());
    }

    #[test]
    fn los_comandos_de_pantalla_son_powershell_valido() {
        for (action, ctx) in [("extend", ""), ("clone", ""), ("restart-driver", ""), ("native", "1920x1080")] {
            let (title, script) = action_script(action, ctx).unwrap();
            assert!(!title.is_empty());
            let errors = crate::ps::parse_errors(&script);
            assert!(errors.trim().is_empty(), "{action}: {errors}");
        }
        assert!(action_script("native", "").is_err());
        assert!(action_script("borrar", "").is_err());
    }

    /// Equipo real (solo lee): `cargo test --lib monitores_reales -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn monitores_reales() {
        let l = list().expect("lista");
        println!("Monitores activos: {} (duplicados: {})", l.monitors.len(), l.duplicated);
        for m in &l.monitors {
            println!(
                "  #{} {} · {}×{} a {} Hz (admite hasta {:?}) · propia {:?}×{:?} · {} · escala {}% · en ({}, {}) · principal {} · giro {}°",
                m.number, m.name, m.width, m.height, m.hz, m.max_hz, m.native_width, m.native_height, m.connection, m.scale, m.x, m.y, m.primary, m.rotation
            );
        }
        assert!(!l.monitors.is_empty());
    }
}
