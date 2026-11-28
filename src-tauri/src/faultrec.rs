//! Grabar el fallo (Estado del equipo → Grabar el fallo).
//!
//! El técnico lo deja listo y se lo cede al usuario: AdminOps graba la pantalla hasta 2 minutos, solo en
//! este equipo, y a la vez apunta en una línea de tiempo los errores de Windows, los programas que dejaron
//! de responder y los picos de procesador y disco. Después se ve el vídeo con las marcas y cada marca salta
//! a su segundo. Se borra al cerrar el caso.
//!
//! - `capture`: captura (`Windows.Graphics.Capture`) de un monitor, una ventana o una zona, y vídeo MP4/H.264
//!   (con la voz en una segunda pista AAC si se pide) con el codificador de Windows.
//! - `watch`: el hilo que mide cada segundo (procesador, disco, programas colgados).
//! - `input`: clics y teclas (las letras que se escriben nunca se guardan: solo «•••»).
//! - `mic`: el micrófono y el volumen de cada décima de segundo.
//! - `win`: lo que se pregunta a Windows sobre monitores y ventanas (lista de ventanas, dónde están).
//! - `analysis`: de lo medido a marcas, y la «causa probable» (lógica pura, con pruebas).
//!
//! Los vídeos son del equipo (no viajan en el USB): `recordings` dentro de los datos de este equipo, como
//! `<id>.mp4` y `<id>.json` (inicio, duración, monitor, marcas e id del caso). Se escribe en `<id>.mp4.part`
//! y se renombra al terminar; al arrancar se borra lo que quedó a medias. Al cerrar un caso se borran sus
//! grabaciones; las que no cuelgan de ningún caso se borran al cerrar AdminOps.
//!
//! Los comandos son todos «seguros» para el modo auditoría: solo escriben en la carpeta de AdminOps.

mod analysis;
mod clip;
mod auto;
mod capture;
mod input;
mod mfenc;
mod mic;
mod ring;
mod vigil;
mod watch;
mod win;

use analysis::{Mark, Tick};
pub use input::InputEvent;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

/// Lo más que dura una grabación.
pub const MAX_SECS: u64 = 120;
/// Etiqueta de la ventana flotante con el tiempo y el botón «Parar».
const PILL: &str = "recpill";
const PILL_W: f64 = 430.0;
const PILL_H: f64 = 44.0;
/// Separación de la píldora al borde del monitor (píxeles lógicos).
const PILL_MARGIN: f64 = 16.0;
/// Etiqueta de la ventana transparente que cubre un monitor para dibujar la zona que se quiere grabar.
const AREA: &str = "recarea";
/// Etiqueta de la capa transparente donde se dibuja encima del monitor grabado.
const DRAW: &str = "recdraw";
/// La cámara en una esquina de lo que se graba.
const CAM: &str = "reccam";
const ID_PREFIX: &str = "fr-";

/// El momento del primer fotograma: de ahí cuentan los segundos del vídeo.
#[derive(Clone, Copy, Debug)]
pub struct Started {
    pub at: Instant,
    /// Segundos Unix.
    pub unix: u64,
}

/// Una grabación guardada (lo que dice su `.json`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Meta {
    pub id: String,
    /// Segundos Unix del primer fotograma.
    pub started: u64,
    /// Lo que dura el vídeo, en segundos.
    pub secs: u32,
    /// El monitor grabado («Monitor 2 · DELL U2419H»).
    pub monitor: String,
    pub width: u32,
    pub height: u32,
    /// El caso abierto al empezar; sin él, se borra al cerrar AdminOps.
    pub case_id: Option<String>,
    pub marks: Vec<Mark>,
    pub bytes: u64,
    /// Algo que el técnico debe saber (p. ej. que no se pudo leer el registro de eventos).
    pub note: Option<String>,
    /// Lo que se grabó: «Pantalla entera», «Ventana · Excel» (solo el programa, nunca el título) o «Zona de 800 × 600».
    pub target: String,
    /// Se apuntaron clics y teclas (aunque no hubiera ninguno).
    pub input_on: bool,
    /// Los clics y las teclas (las letras que se escriben nunca se guardan).
    pub input: Vec<InputEvent>,
    /// El volumen de cada décima de segundo, de 0 a 100; vacío sin voz.
    pub voice: Vec<u8>,
    /// El vídeo lleva de verdad la voz.
    pub mic: bool,
    /// El vídeo lleva de verdad el sonido del equipo.
    pub system_audio: bool,
    /// De dónde sale: «record» (grabada a mano), «replay» («¡Acaba de pasar!») o «auto» (grabación automática).
    #[serde(default = "origin_record")]
    pub origin: String,
    /// Qué disparó una grabación automática («Excel no responde»).
    pub trigger: Option<String>,
    /// Para «No volver a grabar por esto»: `app:<exe en minúsculas>`, `cpu` o `disk`.
    pub trigger_key: Option<String>,
    /// Las pistas del reproductor: procesador, disco, memoria y red de cada segundo.
    pub samples: Vec<analysis::Sample>,
}

fn origin_record() -> String {
    "record".to_string()
}

/// Qué grabar además del monitor (todo opcional: sin nada, el monitor entero y sin clics ni voz).
#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct StartOptions {
    /// Grabar solo esta ventana (el `id` que da `faultrec_windows`). Gana a `region`.
    pub window: Option<u64>,
    /// Grabar solo esta zona del monitor `monitor` (píxeles físicos desde su esquina superior izquierda).
    pub region: Option<Region>,
    /// Apuntar clics y teclas. La interfaz siempre lo manda.
    pub input: bool,
    /// Grabar el micrófono. Apagado de serie.
    pub mic: bool,
    /// Grabar también lo que suena en el equipo (el altavoz predeterminado), mezclado con la voz. Apagado de serie.
    pub system_audio: bool,
}

/// Una zona de un monitor, en píxeles físicos desde su esquina superior izquierda.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Una ventana que se puede grabar. El título solo sirve para el selector, en vivo: no se guarda.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecWindow {
    /// El `HWND`.
    pub id: u64,
    pub title: String,
    /// El programa («Microsoft Excel»).
    pub app: String,
    pub minimized: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub meta: Meta,
    /// El trozo que va en el informe del cliente (milisegundos), si se mandó.
    pub clip: Option<[u64; 2]>,
    /// Ruta del vídeo, solo para que la interfaz lo reproduzca (no se enseña).
    pub path: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub recording: bool,
    /// Parado y preparando las marcas.
    pub finishing: bool,
    pub secs: u32,
    pub max_secs: u32,
    pub id: Option<String>,
    /// El fallo de la última grabación, una sola vez.
    pub error: Option<String>,
    /// Se está grabando la voz ahora.
    pub mic: bool,
    /// Se está grabando el sonido del equipo ahora.
    pub system_audio: bool,
    /// En pausa: no se escribe nada y el tiempo no avanza.
    pub paused: bool,
    /// La capa de dibujo está abierta.
    pub drawing: bool,
    /// La cámara está en la esquina.
    pub camera: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecMonitor {
    /// Ruta del dispositivo de Windows (la misma de «Pantallas»).
    pub id: String,
    pub name: String,
    pub number: u32,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct DonePayload {
    id: Option<String>,
    error: Option<String>,
    /// Guardado con «¡Acaba de pasar!» (Ctrl+Alt+G): la interfaz no cambia de pantalla por esto.
    replay: bool,
}

/// Lo que cuenta `faultrec://region` al elegir (o cancelar) una zona.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct RegionPayload {
    monitor: Option<String>,
    /// `null` = cancelado.
    region: Option<Region>,
}

/// Las notas de una grabación en una sola frase, o ninguna.
fn join_notes(notes: Vec<String>) -> Option<String> {
    let notes: Vec<String> = notes.into_iter().filter(|n| !n.trim().is_empty()).collect();
    (!notes.is_empty()).then(|| notes.join(" "))
}

/// «Zona de 800 × 600».
fn region_target(w: u32, h: u32) -> String {
    format!("Zona de {w} × {h}")
}

// ---------- Nombres y carpeta (puro, con pruebas) ----------

/// El id de una grabación: `fr-` y los segundos Unix de cuando se pidió.
pub fn new_id(unix: u64) -> String {
    format!("{ID_PREFIX}{unix}")
}

/// Solo valen los id que se generan aquí: así ninguna ruta pedida desde la interfaz sale de la carpeta.
pub fn valid_id(id: &str) -> bool {
    id.strip_prefix(ID_PREFIX).is_some_and(|rest| !rest.is_empty() && rest.len() <= 20 && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn video_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.mp4"))
}

fn part_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.mp4.part"))
}

fn meta_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn dir(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("recordings")
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn read_meta(path: &Path) -> Option<Meta> {
    let text = std::fs::read_to_string(path).ok()?;
    let m: Meta = serde_json::from_str(&text).ok()?;
    valid_id(&m.id).then_some(m)
}

fn write_meta(dir: &Path, meta: &Meta) -> Result<(), String> {
    let json = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    let (path, tmp) = (meta_file(dir, &meta.id), crate::paths::tmp_for(&meta_file(dir, &meta.id)));
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Las grabaciones guardadas (con vídeo y marcas), las más recientes primero.
pub fn list_dir(dir: &Path) -> Vec<Meta> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Meta> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| read_meta(&p))
        .filter(|m| video_file(dir, &m.id).exists())
        .collect();
    out.sort_by_key(|m| std::cmp::Reverse(m.started));
    out
}

fn remove_recording(dir: &Path, id: &str) {
    let _ = std::fs::remove_file(thumb_file(dir, id));
    let _ = std::fs::remove_file(clip_file(dir, id));
    let _ = std::fs::remove_file(video_file(dir, id));
    let _ = std::fs::remove_file(meta_file(dir, id));
    let _ = std::fs::remove_file(part_file(dir, id));
}

/// Borra las grabaciones de un caso. Devuelve cuántas.
pub fn delete_case_in(dir: &Path, case_id: &str) -> usize {
    let mine: Vec<String> = list_dir(dir).into_iter().filter(|m| m.case_id.as_deref() == Some(case_id)).map(|m| m.id).collect();
    for id in &mine {
        remove_recording(dir, id);
    }
    mine.len()
}

/// Cuándo se limpia la carpeta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sweep {
    /// Al arrancar AdminOps: solo se queda lo del caso que sigue abierto.
    Start,
    /// Al cerrar AdminOps: se van las que no cuelgan de ningún caso.
    Exit,
}

fn keep(case_id: Option<&str>, open: Option<&str>, mode: Sweep) -> bool {
    match mode {
        Sweep::Start => case_id.is_some() && case_id == open,
        Sweep::Exit => case_id.is_some(),
    }
}

/// Limpia la carpeta de grabaciones: lo que quedó a medias (`.part`), los vídeos sin sus marcas (y al revés)
/// y lo que no toca guardar según `mode`. Devuelve cuántos archivos quitó.
pub fn sweep_dir(dir: &Path, open: Option<&str>, mode: Sweep) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    let mut removed = 0;
    let ext = |p: &Path| p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    let stem = |p: &Path| p.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_string();
    let mut with_meta: Vec<String> = Vec::new();
    for p in &files {
        match ext(p).as_str() {
            "part" | "tmp" => {
                if std::fs::remove_file(p).is_ok() {
                    removed += 1;
                }
            }
            "json" => {
                let id = stem(p);
                if !valid_id(&id) {
                    continue;
                }
                with_meta.push(id.clone());
                let ok = read_meta(p).is_some_and(|m| m.id == id && video_file(dir, &id).exists() && keep(m.case_id.as_deref(), open, mode));
                if !ok {
                    remove_recording(dir, &id);
                    removed += 1;
                }
            }
            _ => {}
        }
    }
    // La miniatura y el recorte del informe de una grabación que ya no está.
    for p in &files {
        if matches!(ext(p).as_str(), "jpg" | "clip") {
            let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
            let id = name.split('.').next().unwrap_or("");
            if valid_id(id) && !video_file(dir, id).exists() && std::fs::remove_file(p).is_ok() {
                removed += 1;
            }
        }
    }
    for p in &files {
        if ext(p) == "mp4" {
            let id = stem(p);
            if valid_id(&id) && !with_meta.contains(&id) && std::fs::remove_file(p).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}


// ---------- Pausas (puro, con pruebas) ----------

/// Una pausa: de cuándo a cuándo, en milisegundos de reloj desde el primer fotograma.
type Pause = (u32, u32);

/// Las pausas con la abierta (si la hay) puesta como una más que no termina.
fn with_open(pauses: &[Pause], open: Option<u32>) -> Vec<Pause> {
    let mut all = pauses.to_vec();
    if let Some(s) = open {
        all.push((s, u32::MAX));
    }
    all
}

/// Del reloj de pared al tiempo del vídeo: se quita lo que duraron las pausas hasta ese momento. Un instante que
/// cae dentro de una pausa se queda donde empezó.
fn video_ms(pauses: &[Pause], open: Option<u32>, wall_ms: u32) -> u32 {
    let paused: u32 = with_open(pauses, open).iter().filter(|(s, _)| wall_ms > *s).map(|(s, e)| wall_ms.min(*e) - s).fold(0, u32::saturating_add);
    wall_ms.saturating_sub(paused)
}

/// ¿Ese instante (reloj de pared) cae dentro de una pausa?
fn in_pause(pauses: &[Pause], wall_ms: u32) -> bool {
    pauses.iter().any(|(s, e)| (*s..*e).contains(&wall_ms))
}

/// Cuánto duraron en total las pausas (cerradas).
fn paused_total_ms(pauses: &[Pause]) -> u32 {
    pauses.iter().map(|(s, e)| e.saturating_sub(*s)).fold(0, u32::saturating_add)
}

/// Las medidas de procesador, disco y demás pasadas al tiempo del vídeo: lo medido en pausa se tira. Si dos caen en
/// el mismo segundo del vídeo (la pausa empezó a mitad de segundo), se queda la primera.
fn remap_ticks(ticks: &[Tick], pauses: &[Pause]) -> Vec<Tick> {
    let mut out: Vec<Tick> = ticks
        .iter()
        .filter(|t| !in_pause(pauses, t.t.saturating_mul(1000)))
        .map(|t| Tick { t: video_ms(pauses, None, t.t.saturating_mul(1000)) / 1000, ..t.clone() })
        .collect();
    out.dedup_by_key(|t| t.t);
    out
}

/// Los clics y las teclas pasados al tiempo del vídeo: lo hecho en pausa no está en el vídeo.
fn remap_input(input: Vec<InputEvent>, pauses: &[Pause]) -> Vec<InputEvent> {
    input.into_iter().filter(|e| !in_pause(pauses, e.t)).map(|e| InputEvent { t: video_ms(pauses, None, e.t), ..e }).collect()
}

// ---------- Estado de la grabación en marcha ----------

struct Active {
    id: String,
    stop: Arc<AtomicBool>,
    first: Arc<Mutex<Option<Started>>>,
    capture: Option<JoinHandle<Result<capture::Done, String>>>,
    watcher: Option<JoinHandle<Vec<Tick>>>,
    /// El hilo de clics y teclas, si se pidieron.
    input: Option<JoinHandle<Vec<InputEvent>>>,
    /// Se pidió apuntar clics y teclas (y el hilo arrancó).
    input_on: bool,
    monitor: String,
    /// Lo que se graba, para enseñarlo.
    target: String,
    /// Se graba una ventana (y no un monitor).
    window: bool,
    /// El micrófono arrancó.
    mic: bool,
    /// Por qué no arrancó el micrófono, si se pidió.
    mic_note: Option<String>,
    /// El sonido del equipo arrancó.
    sys: bool,
    /// Por qué no arrancó el sonido del equipo, si se pidió.
    sys_note: Option<String>,
    /// «Pausar»: lo mira el hilo de captura.
    pause: Arc<AtomicBool>,
    /// Las pausas ya terminadas (reloj de pared, en milisegundos desde el primer fotograma).
    pauses: Vec<Pause>,
    /// Desde cuándo está en pausa, si lo está.
    paused_at: Option<u32>,
    /// Las marcas puestas a mano (Ctrl+Alt+M o el botón), en segundos del vídeo.
    manual: Vec<u32>,
    /// El monitor donde se graba (o donde está la ventana): la capa de dibujo lo cubre.
    mon: Option<crate::displays::Monitor>,
    /// La zona que se graba dentro de `mon`, si es una zona: la cámara va en su esquina.
    area: Option<Region>,
    case_id: Option<String>,
    dir: PathBuf,
}

impl Active {
    /// Milisegundos de reloj desde el primer fotograma.
    fn wall_ms(&self) -> u32 {
        lock(&self.first).map_or(0, |s| s.at.elapsed().as_millis().min(u128::from(u32::MAX)) as u32)
    }

    /// Milisegundos del vídeo hasta ahora (sin lo que lleva en pausa).
    fn video_now_ms(&self) -> u32 {
        video_ms(&self.pauses, self.paused_at, self.wall_ms())
    }
}

static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);
static FINISH: Mutex<()> = Mutex::new(());
static LAST: Mutex<Option<Result<Meta, String>>> = Mutex::new(None);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);
static FINISHING: AtomicBool = AtomicBool::new(false);
static STARTING: AtomicBool = AtomicBool::new(false);
/// «Parar» pulsado mientras se preparaba la grabación (todavía sin `ACTIVE`).
static STOP_EARLY: AtomicBool = AtomicBool::new(false);
/// Hay una zona por elegir: el monitor que se pidió (o ninguno = el principal). Quien lo quita, avisa.
static AREA_PENDING: Mutex<Option<Option<String>>> = Mutex::new(None);
const STOPPED_EARLY: &str = "Se paró antes de empezar a grabar.";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Suelta `STARTING` pase lo que pase al preparar una grabación.
struct StartGuard;
impl Drop for StartGuard {
    fn drop(&mut self) {
        STARTING.store(false, Ordering::SeqCst);
    }
}

// ---------- La ventana flotante ----------

fn restore_main(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

fn close_pill(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(PILL) {
        let _ = w.close();
    }
}

fn close_draw(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(DRAW) {
        let _ = w.close();
    }
    if let Some(w) = app.get_webview_window(CAM) {
        let _ = w.close();
    }
}

fn close_area(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window(AREA) {
        let _ = w.close();
    }
}

#[cfg(windows)]
fn exclude_from_capture(w: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
    if let Ok(h) = w.hwnd() {
        // SAFETY: el identificador es el de una ventana de esta misma app, viva en este momento.
        unsafe {
            SetWindowDisplayAffinity(h.0 as _, WDA_EXCLUDEFROMCAPTURE);
        }
    }
}

#[cfg(not(windows))]
fn exclude_from_capture(_: &tauri::WebviewWindow) {}

/// La píldora «● Grabando 0:41 · Parar»: ventana pequeña, sin marco y siempre encima, en la esquina
/// superior derecha del monitor que se graba y fuera de la grabación (donde Windows lo permite).
fn open_pill(app: &tauri::AppHandle, mon: Option<&crate::displays::Monitor>) -> Result<(), String> {
    close_pill(app);
    let mut b = tauri::WebviewWindowBuilder::new(app, PILL, tauri::WebviewUrl::App("index.html".into()))
        .title("AdminOps · grabando")
        .inner_size(PILL_W, PILL_H)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .theme(Some(tauri::Theme::Dark))
        .disable_drag_drop_handler()
        .initialization_script("window.__ADMINOPS_PILL__ = true;");
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    let w = b.build().map_err(|e| format!("No se pudo abrir el contador: {e}"))?;
    if let Some(m) = mon {
        // Primero dentro del monitor (así toma su escala) y luego, ya con su tamaño real, a la esquina.
        let _ = w.set_position(tauri::PhysicalPosition::new(m.x + m.width as i32 / 2, m.y + 40));
        let _ = w.set_size(tauri::LogicalSize::new(PILL_W, PILL_H));
        let scale = w.scale_factor().unwrap_or(1.0);
        let outer = w.outer_size().map_or((PILL_W * scale) as i32, |s| s.width as i32);
        let margin = (PILL_MARGIN * scale) as i32;
        let _ = w.set_position(tauri::PhysicalPosition::new(m.x + m.width as i32 - outer - margin, m.y + margin));
    }
    exclude_from_capture(&w);
    w.show().map_err(|e| e.to_string())
}

// ---------- Errores del registro de Windows ----------

const EVENTS_SCRIPT: &str = r#"
$from = [DateTimeOffset]::FromUnixTimeSeconds([int64]$fromText).LocalDateTime
$to = [DateTimeOffset]::FromUnixTimeSeconds([int64]$toText).LocalDateTime
$ev = foreach ($log in 'System', 'Application') {
  Get-WinEvent -FilterHashtable @{ LogName = $log; Level = 1, 2; StartTime = $from; EndTime = $to } -MaxEvents 300 -ErrorAction SilentlyContinue | ForEach-Object {
    [pscustomobject]@{ log = $log; provider = "$($_.ProviderName)"; id = [int]$_.Id; level = [int]$_.Level; t = [int64](($_.TimeCreated.ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds); message = '' }
  }
}
ConvertTo-Json -InputObject @($ev) -Depth 3 -Compress
"#;

fn events_script(from: u64, to: u64) -> String {
    format!("{}{}{EVENTS_SCRIPT}", crate::ps::text_var("fromText", &from.to_string()), crate::ps::text_var("toText", &to.to_string()))
}

fn read_events(from: u64, secs: u32) -> Result<Vec<crate::eventdigest::RawEvent>, String> {
    let script = events_script(from, from + u64::from(secs) + 2);
    let out = crate::pspool::query(&script, Some(Duration::from_secs(40)), "Grabar el fallo: errores de Windows")?;
    let t = out.trim();
    if t.is_empty() || t == "null" {
        return Ok(Vec::new());
    }
    serde_json::from_str(t).map_err(|e| format!("Respuesta inesperada: {e}"))
}

// ---------- Empezar y terminar ----------

/// Lo guarda todo al terminar de grabar. `quiet`: al cerrar AdminOps no se espera al registro de eventos.
fn save(app: &tauri::AppHandle, a: &Active, done: capture::Done, started: Started, ticks: &[Tick], input: Vec<InputEvent>, quiet: bool) -> Result<Meta, String> {
    let secs = ((done.duration_ms + 500) / 1000) as u32;
    // Con pausas, los errores de Windows se buscan en todo el rato de reloj y luego se pasan al tiempo del vídeo.
    let wall_secs = secs + paused_total_ms(&a.pauses).div_ceil(1000);
    let (mut events, events_note) = if quiet {
        (Vec::new(), Some("Se guardó al cerrar AdminOps: faltan las marcas de errores de Windows.".to_string()))
    } else {
        match read_events(started.unix, wall_secs) {
            Ok(v) => (v, None),
            Err(e) => {
                log::warn!("Grabar el fallo: no se pudo leer el registro de eventos: {e}");
                (Vec::new(), Some("No se pudo leer el registro de eventos de Windows: faltan las marcas de errores.".to_string()))
            }
        }
    };
    // Las notas, en este orden: lo que se perdió, la voz y el registro de eventos.
    let mut notes: Vec<String> = Vec::new();
    if done.lost {
        notes.push(if a.window { "La ventana se cerró durante la grabación: el vídeo termina ahí." } else { "El monitor se desconectó durante la grabación: el vídeo termina ahí." }.to_string());
    }
    if let Some(n) = &a.mic_note {
        notes.push(n.clone());
    } else if a.mic && !done.with_voice {
        notes.push("No se grabó la voz: Windows no pudo guardarla en el vídeo.".to_string());
    }
    if let Some(n) = &a.sys_note {
        notes.push(n.clone());
    } else if a.sys && !done.with_voice {
        notes.push("No se grabó el sonido del equipo: Windows no pudo guardarlo en el vídeo.".to_string());
    }
    notes.extend(events_note);
    let note = join_notes(notes);
    // Lo medido, lo pulsado y los errores, al tiempo del vídeo (sin las pausas).
    let remapped;
    let (ticks, mut input) = if a.pauses.is_empty() {
        (ticks, input)
    } else {
        for ev in &mut events {
            let wall_ms = (ev.t.saturating_sub(started.unix) * 1000).min(u64::from(u32::MAX)) as u32;
            ev.t = started.unix + u64::from(video_ms(&a.pauses, None, wall_ms) / 1000);
        }
        remapped = remap_ticks(ticks, &a.pauses);
        (remapped.as_slice(), remap_input(input, &a.pauses))
    };
    // Nada de lo apuntado después del último fotograma.
    input.retain(|e| u64::from(e.t) <= done.duration_ms);
    let mut marks = analysis::build_marks(ticks, &events, started.unix, secs);
    // Las marcas puestas a mano, tal cual, sin buscarles causa.
    if !a.manual.is_empty() {
        marks.extend(a.manual.iter().map(|&t| Mark { t: t.min(secs), kind: analysis::Kind::Manual, title: "Marca".into(), ..Default::default() }));
        marks.sort_by_key(|m| m.t);
    }
    let part = part_file(&a.dir, &a.id);
    // Si el caso se cerró mientras se grababa, la grabación ya no tiene dónde colgar: no se guarda.
    if a.case_id.is_some() && a.case_id != crate::cases::open_case_id(app) {
        let _ = std::fs::remove_file(&part);
        return Err("El caso se cerró mientras se grababa; la grabación no se guardó.".into());
    }
    let video = video_file(&a.dir, &a.id);
    std::fs::rename(&part, &video).map_err(|e| format!("No se pudo guardar el vídeo: {e}"))?;
    let bytes = std::fs::metadata(&video).map_or(0, |m| m.len());
    let meta = Meta {
        id: a.id.clone(),
        started: started.unix,
        secs,
        monitor: a.monitor.clone(),
        width: done.width,
        height: done.height,
        case_id: a.case_id.clone(),
        marks,
        samples: analysis::samples_of(ticks),
        bytes,
        note,
        target: a.target.clone(),
        input_on: a.input_on,
        input,
        mic: done.with_voice && a.mic,
        system_audio: done.with_voice && a.sys,
        voice: done.voice,
        origin: origin_record(),
        trigger: None,
        trigger_key: None,
    };
    if let Err(e) = write_meta(&a.dir, &meta) {
        remove_recording(&a.dir, &a.id);
        return Err(format!("No se pudieron guardar las marcas: {e}"));
    }
    Ok(meta)
}

/// Para la grabación, cierra el MP4 y guarda las marcas. Lo llaman «Parar», los dos minutos y el cierre de AdminOps;
/// si ya terminó (o está terminando) otro, devuelve su resultado.
/// Lo que dice el aviso de abrir a salvo si AdminOps se cierra de golpe grabando o vigilando.
const SAFE_RECORDING: &str = "grabando la pantalla (Grabar el fallo)";
pub(crate) const SAFE_WATCHING: &str = "vigilando la pantalla (Grabar el fallo)";

fn finish(app: &tauri::AppHandle, quiet: bool) -> Result<Meta, String> {
    crate::safestart::activity(SAFE_RECORDING, false);
    let _g = lock(&FINISH);
    let mut active = lock(&ACTIVE);
    let Some(mut a) = active.take() else {
        // Se pidió parar mientras se preparaba (antes del primer fotograma): lo recoge `faultrec_start`.
        if STARTING.load(Ordering::SeqCst) {
            STOP_EARLY.store(true, Ordering::SeqCst);
            return Err(STOPPED_EARLY.into());
        }
        drop(active);
        return lock(&LAST).clone().unwrap_or_else(|| Err("No se está grabando.".into()));
    };
    // Antes de soltar `ACTIVE`, para que nadie vea las dos cosas libres a la vez.
    FINISHING.store(true, Ordering::SeqCst);
    drop(active);
    a.stop.store(true, Ordering::SeqCst);
    let done = match a.capture.take().map(|h| h.join()) {
        Some(Ok(r)) => r,
        _ => Err("La grabación falló de forma inesperada.".to_string()),
    };
    let ticks = a.watcher.take().and_then(|h| h.join().ok()).unwrap_or_default();
    let input = a.input.take().and_then(|h| h.join().ok()).unwrap_or_default();
    // Lo primero que ve el usuario al parar: la píldora se va (también al cerrar AdminOps, o el proceso no
    // terminaría con ella abierta) y AdminOps vuelve al frente.
    close_pill(app);
    close_draw(app);
    stop_mark_hotkey(app);
    if !quiet {
        restore_main(app);
    }
    let started = *lock(&a.first);
    // Si se paró estando en pausa, esa pausa termina ahora.
    if let (Some(s), Some(st)) = (a.paused_at.take(), started) {
        a.pauses.push((s, st.at.elapsed().as_millis().min(u128::from(u32::MAX)) as u32));
    }
    let result = match (done, started) {
        (Ok(d), Some(s)) => save(app, &a, d, s, &ticks, input, quiet),
        (Err(e), _) => Err(e),
        (Ok(_), None) => Err("No llegó ningún fotograma.".into()),
    };
    if result.is_err() {
        let _ = std::fs::remove_file(part_file(&a.dir, &a.id));
    }
    let error = result.as_ref().err().cloned();
    if let Some(e) = &error {
        log::warn!("Grabar el fallo: {e}");
        *lock(&LAST_ERROR) = Some(e.clone());
    }
    *lock(&LAST) = Some(result.clone());
    FINISHING.store(false, Ordering::SeqCst);
    if !quiet {
        let _ = app.emit("faultrec://done", DonePayload { id: result.as_ref().ok().map(|m| m.id.clone()), error, replay: false });
        // Si Vigilar estaba encendido al empezar a grabar, vuelve a encenderse igual.
        vigil::resume_after_recording(app);
    }
    result
}

fn spawn_coordinator(app: tauri::AppHandle, stop: Arc<AtomicBool>) {
    let _ = std::thread::Builder::new().name("faultrec-coordinator".into()).spawn(move || {
        let mut shown: Option<u32> = None;
        loop {
            std::thread::sleep(Duration::from_millis(200));
            // El tiempo que cuenta es el del vídeo: lo que lleva en pausa no entra.
            let (ended, video) = match lock(&ACTIVE).as_ref() {
                None => return,
                Some(a) => (a.stop.load(Ordering::SeqCst) || a.capture.as_ref().is_none_or(JoinHandle::is_finished), a.video_now_ms()),
            };
            let elapsed = Duration::from_millis(u64::from(video));
            let secs = elapsed.as_secs().min(MAX_SECS) as u32;
            if shown != Some(secs) {
                shown = Some(secs);
                let _ = app.emit("faultrec://tick", secs);
            }
            if elapsed >= Duration::from_secs(MAX_SECS) {
                stop.store(true, Ordering::SeqCst);
            }
            if ended || elapsed >= Duration::from_secs(MAX_SECS) {
                break;
            }
        }
        let _ = finish(&app, false);
    });
}

/// Al abrir AdminOps: deja lista la carpeta (y su permiso para reproducir desde ella) y borra lo que quedó
/// a medias, lo que no tiene caso y lo de casos ya cerrados.
pub fn start(app: tauri::AppHandle) {
    let dir = dir(&app);
    let _ = std::fs::create_dir_all(&dir);
    // Solo esta carpeta (y no sus subcarpetas) se puede leer desde la interfaz.
    if let Err(e) = app.asset_protocol_scope().allow_directory(&dir, false) {
        log::warn!("Grabar el fallo: no se pudo permitir la carpeta de vídeos: {e}");
    }
    let _ = std::thread::Builder::new().name("faultrec-sweep".into()).spawn(move || {
        if lock(&ACTIVE).is_some() {
            return;
        }
        let open = crate::cases::open_case_id(&app);
        let n = sweep_dir(&dir, open.as_deref(), Sweep::Start);
        if n > 0 {
            log::info!("Grabar el fallo: se borraron {n} archivos de grabaciones que ya no hacían falta");
        }
    });
}

/// Al cerrar AdminOps por cualquier camino: si se estaba grabando, se cierra el MP4 bien; y las grabaciones
/// que no cuelgan de ningún caso se borran.
pub fn shutdown(app: &tauri::AppHandle) {
    // Vigilar se apaga y nada de lo que tenía en memoria se guarda.
    vigil::shutdown();
    let t0 = Instant::now();
    // Si otro hilo ya está terminando una grabación, se espera un poco (pero no se cuelga el cierre).
    let guard = loop {
        match FINISH.try_lock() {
            Ok(g) => break Some(g),
            Err(std::sync::TryLockError::Poisoned(e)) => break Some(e.into_inner()),
            // Si está guardando una grabación (leer el registro de eventos puede tardar hasta 40 s), se le deja
            // terminar: la ventana ya se cerró y cortarla a medias perdería el vídeo del caso.
            Err(std::sync::TryLockError::WouldBlock) if t0.elapsed() < Duration::from_secs(if FINISHING.load(Ordering::SeqCst) { 45 } else { 5 }) => std::thread::sleep(Duration::from_millis(50)),
            Err(std::sync::TryLockError::WouldBlock) => break None,
        }
    };
    drop(guard);
    if lock(&ACTIVE).is_some() {
        let _ = finish(app, true);
    }
    // Pase lo que pase, la píldora y el selector de zona no pueden quedarse abiertos: Tauri no sale mientras
    // quede una ventana. Sin zona pendiente, para que cerrar no avise a nadie.
    close_pill(app);
    close_draw(app);
    *lock(&AREA_PENDING) = None;
    close_area(app);
    let n = sweep_dir(&dir(app), None, Sweep::Exit);
    if n > 0 {
        log::info!("Grabar el fallo: al cerrar se borraron {n} archivos de grabaciones sin caso");
    }
}

// ---------- Comandos ----------

/// Los monitores que se pueden grabar.
#[tauri::command(async)]
pub fn faultrec_monitors() -> Vec<RecMonitor> {
    crate::displays::list()
        .map(|l| l.monitors)
        .unwrap_or_default()
        .into_iter()
        .map(|m| RecMonitor { id: m.id, name: m.name, number: m.number, primary: m.primary, width: m.width, height: m.height })
        .collect()
}

fn fail_start(app: &tauri::AppHandle, e: String) -> String {
    close_pill(app);
    restore_main(app);
    e
}

/// Los monitores de Windows (con su posición), el que se pide (o el principal) y, con ventana, el monitor donde está.
struct Chosen {
    /// El monitor que se pidió (o el principal). Con una ventana, `None`: no se mira.
    monitor: Option<crate::displays::Monitor>,
    /// Donde va la píldora y de donde sale el nombre: el monitor de la ventana o el elegido.
    shown: Option<crate::displays::Monitor>,
    /// Cuántos monitores hay (para llamarle «Monitor principal» si solo hay uno).
    count: usize,
}

fn choose(monitor: Option<&str>, window: Option<isize>) -> Result<Chosen, String> {
    let monitors = crate::displays::list().map(|l| l.monitors).unwrap_or_default();
    let count = monitors.len();
    let wanted = monitor.filter(|s| !s.is_empty());
    let primary = |ms: &[crate::displays::Monitor]| ms.iter().find(|m| m.primary).or(ms.first()).cloned();
    let chosen = if window.is_some() {
        None
    } else {
        match wanted {
            Some(id) => Some(monitors.iter().find(|m| m.id == id).ok_or("Ese monitor ya no está conectado.")?.clone()),
            None => primary(&monitors),
        }
    };
    let shown = match window {
        Some(h) => win::window_monitor_rect(h).and_then(|a| monitors.iter().find(|m| m.x == a.x && m.y == a.y).cloned()).or_else(|| primary(&monitors)),
        None => chosen.clone(),
    };
    Ok(Chosen { monitor: chosen, shown, count })
}

fn monitor_label(m: Option<&crate::displays::Monitor>, count: usize) -> String {
    match m {
        Some(m) if m.primary && count <= 1 => "Monitor principal".to_string(),
        Some(m) if m.primary => format!("Monitor principal · {}", m.name),
        Some(m) => format!("Monitor {} · {}", m.number, m.name),
        None => "Monitor principal".to_string(),
    }
}

/// Empieza a grabar el monitor que se diga (o el principal), una ventana o una zona de ese monitor (`opts`), con
/// clics y teclas y con la voz si se piden. AdminOps se minimiza y aparece la píldora.
#[tauri::command(async)]
pub fn faultrec_start(app: tauri::AppHandle, monitor: Option<String>, opts: Option<StartOptions>) -> Result<Status, String> {
    // Con Vigilar encendido se apaga (no pueden capturar a la vez) y al terminar de grabar vuelve a encenderse.
    let paused = vigil::pause_for_recording();
    let r = start_recording(app.clone(), monitor, opts);
    if paused && r.is_err() {
        vigil::resume_after_recording(&app);
    }
    if r.is_ok() {
        crate::safestart::activity(SAFE_RECORDING, true);
    }
    r
}

/// ¿Hay una grabación a mano en marcha, preparándose o terminando?
fn recording_busy() -> bool {
    STARTING.load(Ordering::SeqCst) || FINISHING.load(Ordering::SeqCst) || lock(&ACTIVE).is_some()
}

fn start_recording(app: tauri::AppHandle, monitor: Option<String>, opts: Option<StartOptions>) -> Result<Status, String> {
    if STARTING.swap(true, Ordering::SeqCst) {
        return Err("Ya se está preparando una grabación.".into());
    }
    let _guard = StartGuard;
    STOP_EARLY.store(false, Ordering::SeqCst);
    if lock(&ACTIVE).is_some() || FINISHING.load(Ordering::SeqCst) {
        return Err("Ya hay una grabación en marcha.".into());
    }
    let opts = opts.unwrap_or_default();
    // La ventana gana a la zona; la zona va con el monitor que se pidió.
    let window: Option<isize> = opts.window.map(|id| id as usize as isize);
    if let Some(h) = window {
        if !win::is_window(h) {
            return Err("Esa ventana ya no está abierta.".into());
        }
    }
    let chosen = choose(monitor.as_deref(), window)?;
    let gdi = chosen.monitor.as_ref().map(|m| m.gdi.clone()).unwrap_or_default();
    let label = monitor_label(chosen.shown.as_ref(), chosen.count);
    // La zona se recorta a lo que cabe en el monitor; si queda demasiado pequeña, no se empieza.
    let region = match (window, opts.region) {
        (None, Some(r)) => {
            let (mw, mh) = chosen.monitor.as_ref().map_or((u32::MAX / 2, u32::MAX / 2), |m| (m.width, m.height));
            let (x, y, w, h) = capture::crop_rect(mw, mh, r).ok_or("La zona es demasiado pequeña.")?;
            Some(Region { x: x as i32, y: y as i32, w, h })
        }
        _ => None,
    };
    let target = match (window, region) {
        (Some(h), _) => format!("Ventana · {}", win::app_name(h)),
        (None, Some(r)) => region_target(r.w, r.h),
        (None, None) => "Pantalla entera".to_string(),
    };
    let rec_dir = dir(&app);
    std::fs::create_dir_all(&rec_dir).map_err(|e| format!("No se pudo preparar la carpeta de grabaciones: {e}"))?;
    *lock(&LAST_ERROR) = None;
    *lock(&LAST) = None;

    // La voz: si el micrófono no arranca se graba igual, y la nota lo cuenta.
    let (mic, mic_note) = if opts.mic {
        match mic::start() {
            Ok(m) => (Some(m), None),
            Err(e) => {
                log::warn!("Grabar el fallo: el micrófono no arrancó: {e:?}");
                (None, Some(e.note()))
            }
        }
    } else {
        (None, None)
    };
    let mic_on = mic.is_some();
    // El sonido del equipo: igual que la voz, si no arranca se graba igual y la nota lo cuenta.
    let (sys, sys_note) = if opts.system_audio {
        match mic::start_system() {
            Ok(m) => (Some(m), None),
            Err(e) => {
                log::warn!("Grabar el fallo: el sonido del equipo no arrancó: {e:?}");
                (None, Some(e.note_system()))
            }
        }
    } else {
        (None, None)
    };
    let sys_on = sys.is_some();
    let pause = Arc::new(AtomicBool::new(false));

    // AdminOps se quita de en medio antes de grabar, para que no salga en el vídeo.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.minimize();
    }
    // Una ventana minimizada se trae al frente: minimizada, Windows no entrega imagen.
    let restored = window.is_some_and(win::restore_window);
    if let Err(e) = open_pill(&app, chosen.shown.as_ref()) {
        log::warn!("Grabar el fallo: {e}");
    }
    std::thread::sleep(Duration::from_millis(if restored { 800 } else { 400 }));

    // Reservado al momento: Vigilar puede estar guardando en el mismo segundo.
    let id = vigil::unique_id(&rec_dir);
    let stop = Arc::new(AtomicBool::new(false));
    let first: Arc<Mutex<Option<Started>>> = Arc::new(Mutex::new(None));
    let job = capture::Job { gdi: gdi.clone(), window, region, mic, sys, pause: pause.clone(), out: part_file(&rec_dir, &id), stop: stop.clone(), first: first.clone(), max: Duration::from_secs(MAX_SECS) };
    let capture_handle = std::thread::Builder::new().name("faultrec-capture".into()).spawn(move || capture::record(job)).map_err(|e| {
        let _ = std::fs::remove_file(part_file(&rec_dir, &id));
        fail_start(&app, e.to_string())
    })?;
    let mut capture_handle = Some(capture_handle);
    let (first2, stop2) = (first.clone(), stop.clone());
    let watcher = std::thread::Builder::new().name("faultrec-watch".into()).spawn(move || watch::run(first2, stop2)).map_err(|e| {
        stop.store(true, Ordering::SeqCst);
        // La captura ya arrancó: su .part se borra cuando termine (sweep_dir lo recoge si no).
        let _ = std::fs::remove_file(part_file(&rec_dir, &id));
        fail_start(&app, e.to_string())
    })?;
    // Clics y teclas: un hilo que sondea; si no arranca, se graba igual sin ellos.
    let input_handle = if opts.input {
        let (first3, stop3, watch3) = (first.clone(), stop.clone(), input::Watch { gdi, window, region });
        match std::thread::Builder::new().name("faultrec-input".into()).spawn(move || input::run(first3, stop3, watch3)) {
            Ok(h) => Some(h),
            Err(e) => {
                log::warn!("Grabar el fallo: no se pudo empezar a apuntar clics y teclas: {e}");
                None
            }
        }
    } else {
        None
    };

    // Hasta que llega el primer fotograma no se sabe si funciona: el fallo se cuenta aquí, no a medias.
    let waited = Instant::now();
    loop {
        if lock(&first).is_some() {
            break;
        }
        let problem = if STOP_EARLY.load(Ordering::SeqCst) {
            stop.store(true, Ordering::SeqCst);
            if let Some(h) = capture_handle.take() {
                let _ = h.join();
            }
            Some(STOPPED_EARLY.to_string())
        } else if capture_handle.as_ref().is_some_and(JoinHandle::is_finished) {
            Some(match capture_handle.take().map(JoinHandle::join) {
                Some(Ok(Err(e))) => e,
                Some(Ok(Ok(_))) => "La grabación terminó antes de empezar.".to_string(),
                _ => "La grabación falló de forma inesperada.".to_string(),
            })
        } else if waited.elapsed() > Duration::from_secs(10) {
            stop.store(true, Ordering::SeqCst);
            if let Some(h) = capture_handle.take() {
                let _ = h.join();
            }
            Some("Windows tardó demasiado en empezar a grabar.".to_string())
        } else {
            None
        };
        if let Some(e) = problem {
            stop.store(true, Ordering::SeqCst);
            let _ = watcher.join();
            if let Some(h) = input_handle {
                let _ = h.join();
            }
            let _ = std::fs::remove_file(part_file(&rec_dir, &id));
            return Err(fail_start(&app, e));
        }
        std::thread::sleep(Duration::from_millis(30));
    };

    let case_id = crate::cases::open_case_id(&app);
    {
        // Con `ACTIVE` bloqueado: o «Parar» ya se pulsó y se ve aquí, o llegará después y encontrará la grabación.
        let mut active = lock(&ACTIVE);
        if STOP_EARLY.load(Ordering::SeqCst) {
            drop(active);
            stop.store(true, Ordering::SeqCst);
            if let Some(h) = capture_handle.take() {
                let _ = h.join();
            }
            let _ = watcher.join();
            if let Some(h) = input_handle {
                let _ = h.join();
            }
            let _ = std::fs::remove_file(part_file(&rec_dir, &id));
            return Err(fail_start(&app, STOPPED_EARLY.into()));
        }
        let input_on = input_handle.is_some();
        *active = Some(Active {
            id: id.clone(),
            stop: stop.clone(),
            first,
            capture: capture_handle,
            watcher: Some(watcher),
            input: input_handle,
            input_on,
            monitor: label,
            target,
            window: window.is_some(),
            mic: mic_on,
            mic_note,
            sys: sys_on,
            sys_note,
            pause,
            pauses: Vec::new(),
            paused_at: None,
            manual: Vec::new(),
            mon: chosen.shown.clone(),
            area: region,
            case_id,
            dir: rec_dir,
        });
    }
    spawn_coordinator(app.clone(), stop);
    start_mark_hotkey(&app);
    let _ = app.emit("faultrec://tick", 0u32);
    Ok(Status { recording: true, finishing: false, secs: 0, max_secs: MAX_SECS as u32, id: Some(id), error: None, mic: mic_on, system_audio: sys_on, paused: false, drawing: false, camera: false })
}

/// Las ventanas que se pueden grabar (las de arriba primero). El título es solo para el selector: no se guarda.
#[tauri::command(async)]
pub fn faultrec_windows() -> Vec<RecWindow> {
    win::list_windows()
}

/// Abre una ventana transparente, sin marco y siempre encima, que cubre exactamente ese monitor (o el principal)
/// para dibujar encima la zona que se quiere grabar. Devuelve al momento; la zona llega por `faultrec://region`
/// (ver `faultrec_region_done`).
#[tauri::command(async)]
pub fn faultrec_pick_region(app: tauri::AppHandle, monitor: Option<String>) -> Result<(), String> {
    let chosen = choose(monitor.as_deref(), None)?;
    let m = chosen.monitor.ok_or("Windows no dio ningún monitor.")?;
    // Si ya había una abierta, se cierra sin avisar a nadie.
    *lock(&AREA_PENDING) = None;
    close_area(&app);
    let mut b = tauri::WebviewWindowBuilder::new(&app, AREA, tauri::WebviewUrl::App("index.html".into()))
        .title("AdminOps · elegir zona")
        .inner_size(400.0, 300.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .visible(false)
        .disable_drag_drop_handler()
        .initialization_script("window.__ADMINOPS_AREA__ = true;");
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    let w = b.build().map_err(|e| format!("No se pudo abrir el selector de zona: {e}"))?;
    *lock(&AREA_PENDING) = Some(monitor.clone().filter(|s| !s.is_empty()));
    // Si la ventana se cierra por otro camino (Alt+F4), se cuenta como cancelado.
    let app2 = app.clone();
    w.on_window_event(move |ev| {
        if let tauri::WindowEvent::Destroyed = ev {
            if let Some(monitor) = lock(&AREA_PENDING).take() {
                let _ = app2.emit("faultrec://region", RegionPayload { monitor, region: None });
            }
        }
    });
    // Primero dentro del monitor (así toma su escala) y luego, ya con su tamaño real, encima de él.
    for _ in 0..2 {
        let _ = w.set_position(tauri::PhysicalPosition::new(m.x, m.y));
        let _ = w.set_size(tauri::PhysicalSize::new(m.width, m.height));
    }
    w.show().map_err(|e| e.to_string())?;
    let _ = w.set_focus();
    Ok(())
}

/// Cierra el selector de zona y cuenta lo elegido por `faultrec://region` (`{ monitor, region }`; `region` null =
/// cancelado). Las medidas son píxeles físicos desde la esquina superior izquierda del monitor.
#[tauri::command(async)]
pub fn faultrec_region_done(app: tauri::AppHandle, region: Option<Region>) -> Result<(), String> {
    // Se quita antes de cerrar: así el cierre de la ventana no cuenta un segundo «cancelado».
    let pending = lock(&AREA_PENDING).take();
    close_area(&app);
    if let Some(monitor) = pending {
        let _ = app.emit("faultrec://region", RegionPayload { monitor, region });
    }
    Ok(())
}

/// Cómo va: grabando (y cuántos segundos), terminando de preparar las marcas, o el fallo de la última vez.
#[tauri::command]
pub fn faultrec_status() -> Status {
    let (recording, secs, id, mic, system_audio, paused) = match lock(&ACTIVE).as_ref() {
        Some(a) => (true, (a.video_now_ms() / 1000).min(MAX_SECS as u32), Some(a.id.clone()), a.mic, a.sys, a.paused_at.is_some()),
        None => (false, 0, None, false, false, false),
    };
    Status { recording, finishing: FINISHING.load(Ordering::SeqCst), secs, max_secs: MAX_SECS as u32, id, error: lock(&LAST_ERROR).take(), mic, system_audio, paused, drawing: DRAWING.load(Ordering::SeqCst), camera: CAMERA.load(Ordering::SeqCst) }
}


/// ¿Está abierta la capa de dibujo?
static DRAWING: AtomicBool = AtomicBool::new(false);
/// ¿Está la cámara en la esquina?
static CAMERA: AtomicBool = AtomicBool::new(false);

/// Dónde va la cámara: un cuadrado de un cuarto del lado corto de lo que se graba (entre 140 y 360 px físicos),
/// abajo a la derecha con un margen. `(x, y, lado)` en píxeles físicos de la pantalla.
pub fn camera_rect(mon: (i32, i32, u32, u32), area: Option<Region>) -> (i32, i32, u32) {
    let (mx, my, mw, mh) = mon;
    let (x, y, w, h) = match area {
        Some(r) => (mx + r.x, my + r.y, r.w, r.h),
        None => (mx, my, mw, mh),
    };
    let side = (w.min(h) / 4).clamp(140, 360).min(w.min(h));
    // El margen nunca la saca de la zona (en una zona pequeña la cámara la ocupa casi entera).
    let margin = (side / 8).min((w.min(h) - side) / 2) as i32;
    (x + w as i32 - side as i32 - margin, y + h as i32 - side as i32 - margin, side)
}

/// «Cámara»: la webcam en un círculo en la esquina de lo que se graba. Es una ventana más encima del monitor, así
/// que sale en el vídeo; se puede arrastrar a otra esquina y se cierra al parar. Avisa por `faultrec://camera`.
#[tauri::command(async)]
pub fn faultrec_camera(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    if !on {
        if let Some(w) = app.get_webview_window(CAM) {
            let _ = w.close();
        }
        return Ok(false);
    }
    if app.get_webview_window(CAM).is_some() {
        return Ok(true);
    }
    let (mon, area) = {
        let active = lock(&ACTIVE);
        let a = active.as_ref().ok_or("No se está grabando.")?;
        if a.window {
            return Err("La cámara sale al grabar un monitor o una zona: una ventana grabada no enseña lo que hay encima.".into());
        }
        (a.mon.clone().ok_or("No se sabe qué monitor se está grabando.")?, a.area)
    };
    let (x, y, side) = camera_rect((mon.x, mon.y, mon.width, mon.height), area);
    let mut b = tauri::WebviewWindowBuilder::new(&app, CAM, tauri::WebviewUrl::App("index.html".into()))
        .title("AdminOps · cámara")
        .inner_size(200.0, 200.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .disable_drag_drop_handler()
        .initialization_script("window.__ADMINOPS_CAM__ = true;");
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    let w = b.build().map_err(|e| format!("No se pudo abrir la cámara: {e}"))?;
    // La grabación pudo pararse sola mientras se creaba: `finish` suelta ACTIVE antes de cerrar estas ventanas, así
    // que si ya no está, nadie cerraría esta.
    if lock(&ACTIVE).is_none() {
        let _ = w.close();
        return Err("La grabación ya terminó.".into());
    }
    let app2 = app.clone();
    w.on_window_event(move |ev| {
        if let tauri::WindowEvent::Destroyed = ev {
            CAMERA.store(false, Ordering::SeqCst);
            let _ = app2.emit("faultrec://camera", false);
        }
    });
    // Primero dentro del monitor (así toma su escala) y luego, ya con su tamaño real, en la esquina.
    for _ in 0..2 {
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
        let _ = w.set_size(tauri::PhysicalSize::new(side, side));
    }
    w.show().map_err(|e| e.to_string())?;
    CAMERA.store(true, Ordering::SeqCst);
    let _ = app.emit("faultrec://camera", true);
    Ok(true)
}

/// Vuelve a poner la píldora y la barra del cohete por encima de todo. La capa de dibujo, al pulsarla, sube encima
/// de ellas (es otra ventana «siempre encima», la última activada): se llama al empezar cada trazo para que «Parar»,
/// «Quitar dibujo» y la barra sigan siempre a la vista y pulsables.
#[tauri::command]
pub fn faultrec_raise_controls(app: tauri::AppHandle) {
    for label in [PILL, "rocket"] {
        if let Some(p) = app.get_webview_window(label) {
            let _ = p.set_always_on_top(false);
            let _ = p.set_always_on_top(true);
        }
    }
}

/// «Dibujar»: abre (o cierra) una capa transparente, siempre encima, que cubre el monitor que se graba. Con el ratón
/// se pinta en rojo encima de lo que se ve y cada trazo se borra solo a los 3 segundos; Esc o el mismo botón la cierran.
/// Como la capa es una ventana más del monitor, sale en el vídeo. Avisa por `faultrec://draw` (`true` = abierta).
#[tauri::command(async)]
pub fn faultrec_draw(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    if !on {
        close_draw(&app);
        return Ok(false);
    }
    if app.get_webview_window(DRAW).is_some() {
        return Ok(true);
    }
    let mon = {
        let active = lock(&ACTIVE);
        let a = active.as_ref().ok_or("No se está grabando.")?;
        // Al grabar una ventana solo sale esa ventana: lo que se pinte encima del escritorio no entraría en el vídeo.
        if a.window {
            return Err("Dibujar vale al grabar un monitor o una zona: una ventana grabada no enseña lo que se pinta encima.".into());
        }
        a.mon.clone().ok_or("No se sabe qué monitor se está grabando.")?
    };
    let mut b = tauri::WebviewWindowBuilder::new(&app, DRAW, tauri::WebviewUrl::App("index.html".into()))
        .title("AdminOps · dibujar")
        .inner_size(400.0, 300.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .visible(false)
        .disable_drag_drop_handler()
        .initialization_script("window.__ADMINOPS_DRAW__ = true;");
    if let Some(args) = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    let w = b.build().map_err(|e| format!("No se pudo abrir la capa de dibujo: {e}"))?;
    // Igual que la cámara: si la grabación terminó mientras se creaba, nadie la cerraría.
    if lock(&ACTIVE).is_none() {
        let _ = w.close();
        return Err("La grabación ya terminó.".into());
    }
    // Si se cierra por cualquier camino (Esc, Alt+F4, parar la grabación), se avisa a quien tenga el botón.
    let app2 = app.clone();
    w.on_window_event(move |ev| {
        if let tauri::WindowEvent::Destroyed = ev {
            DRAWING.store(false, Ordering::SeqCst);
            let _ = app2.emit("faultrec://draw", false);
        }
    });
    // Primero dentro del monitor (así toma su escala) y luego, ya con su tamaño real, encima de él.
    for _ in 0..2 {
        let _ = w.set_position(tauri::PhysicalPosition::new(mon.x, mon.y));
        let _ = w.set_size(tauri::PhysicalSize::new(mon.width, mon.height));
    }
    w.show().map_err(|e| e.to_string())?;
    let _ = w.set_focus();
    DRAWING.store(true, Ordering::SeqCst);
    // La capa tapa todo el monitor: la píldora y la barra del cohete se vuelven a poner encima para poder quitarla.
    for label in [PILL, "rocket"] {
        if let Some(p) = app.get_webview_window(label) {
            let _ = p.set_always_on_top(false);
            let _ = p.set_always_on_top(true);
        }
    }
    let _ = app.emit("faultrec://draw", true);
    Ok(true)
}

/// Pone o quita la pausa de la grabación en marcha y lo avisa por `faultrec://paused` (`true` = en pausa).
fn set_paused(app: &tauri::AppHandle, on: bool) -> Result<Status, String> {
    {
        let mut active = lock(&ACTIVE);
        let a = active.as_mut().ok_or("No se está grabando.")?;
        // Antes del primer fotograma no hay reloj: no hay nada que pausar.
        if lock(&a.first).is_none() {
            return Err("La grabación todavía no ha empezado.".into());
        }
        let wall = a.wall_ms();
        if on && a.paused_at.is_none() {
            a.paused_at = Some(wall);
            a.pause.store(true, Ordering::SeqCst);
        } else if !on {
            if let Some(s) = a.paused_at.take() {
                a.pauses.push((s, wall));
            }
            a.pause.store(false, Ordering::SeqCst);
        }
    }
    let _ = app.emit("faultrec://paused", on);
    Ok(faultrec_status())
}

/// «Pausar»: el vídeo y el audio dejan de escribirse y el tiempo no avanza; al seguir, queda en una sola pieza.
#[tauri::command]
pub fn faultrec_pause(app: tauri::AppHandle) -> Result<Status, String> {
    set_paused(&app, true)
}

/// «Seguir»: sigue grabando donde se quedó.
#[tauri::command]
pub fn faultrec_resume(app: tauri::AppHandle) -> Result<Status, String> {
    set_paused(&app, false)
}

/// Pone una marca («Marca») en el segundo actual del vídeo. Devuelve ese segundo y lo avisa por `faultrec://marked`.
fn add_mark(app: &tauri::AppHandle) -> Result<u32, String> {
    /// Las que caben como mucho: más que eso ya no es una línea de tiempo que se pueda leer.
    const MAX_MANUAL: usize = 60;
    let t = {
        let mut active = lock(&ACTIVE);
        let a = active.as_mut().ok_or("No se está grabando.")?;
        if lock(&a.first).is_none() {
            return Err("La grabación todavía no ha empezado.".into());
        }
        let t = a.video_now_ms() / 1000;
        if a.manual.len() >= MAX_MANUAL {
            return Err("Ya hay demasiadas marcas en esta grabación.".into());
        }
        if !a.manual.contains(&t) {
            a.manual.push(t);
        }
        t
    };
    let _ = app.emit("faultrec://marked", t);
    Ok(t)
}

/// «Marcar»: apunta este momento en la línea de tiempo de la grabación (también con Ctrl+Alt+M).
#[tauri::command]
pub fn faultrec_mark(app: tauri::AppHandle) -> Result<u32, String> {
    add_mark(&app)
}

/// Hilo del atajo Ctrl+Alt+M (0 = aún no existe). Los atajos de Windows son de un hilo: se registran y se
/// sueltan desde el mismo, así que se le mandan mensajes.
static MARK_TID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[cfg(windows)]
const MARK_ON: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;
#[cfg(windows)]
const MARK_OFF: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 2;

/// Ctrl+Alt+M: marca el momento mientras se graba. Se coge al empezar a grabar y se suelta al parar: fuera de
/// una grabación es de quien lo use (en Word, por ejemplo, es un atajo propio).
fn start_mark_hotkey(app: &tauri::AppHandle) {
    mark_hotkey(app, true);
}

fn stop_mark_hotkey(app: &tauri::AppHandle) {
    mark_hotkey(app, false);
}

fn mark_hotkey(app: &tauri::AppHandle, on: bool) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
        if !on && MARK_TID.load(Ordering::SeqCst) == 0 {
            return;
        }
        let app = app.clone();
        ONCE.call_once(move || {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let _ = std::thread::Builder::new().name("faultrec-markkey".into()).spawn(move || {
                use windows_sys::Win32::System::Threading::GetCurrentThreadId;
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
                use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, PeekMessageW, MSG, PM_NOREMOVE, WM_HOTKEY, WM_USER};
                // SAFETY: punteros a datos de este hilo; el atajo es de este hilo y se suelta aquí mismo.
                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    // Crea la cola de mensajes del hilo antes de decir que está listo.
                    PeekMessageW(&mut msg, std::ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
                    MARK_TID.store(GetCurrentThreadId(), Ordering::SeqCst);
                    let _ = tx.send(());
                    let mut held = false;
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        match msg.message {
                            MARK_ON if !held => {
                                // Otro proceso de AdminOps puede tardar un poco en soltarlo.
                                for i in 0..3 {
                                    if RegisterHotKey(std::ptr::null_mut(), 6, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'M')) != 0 {
                                        held = true;
                                        break;
                                    }
                                    if i < 2 {
                                        std::thread::sleep(Duration::from_millis(700));
                                    }
                                }
                                if !held {
                                    log::warn!("Grabar el fallo: no se pudo registrar Ctrl+Alt+M para marcar: lo usa otro programa.");
                                }
                            }
                            MARK_OFF if held => {
                                UnregisterHotKey(std::ptr::null_mut(), 6);
                                held = false;
                            }
                            // Fuera de una grabación no hace nada.
                            WM_HOTKEY => drop(add_mark(&app)),
                            _ => {}
                        }
                    }
                }
            });
            let _ = rx.recv_timeout(Duration::from_secs(2));
        });
        let tid = MARK_TID.load(Ordering::SeqCst);
        if tid != 0 {
            // SAFETY: solo se pone un mensaje en la cola de ese hilo.
            unsafe {
                PostThreadMessageW(tid, if on { MARK_ON } else { MARK_OFF }, 0, 0);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (app, on, &ONCE);
}

/// Para y guarda. Vuelve a poner AdminOps al frente; devuelve la grabación con sus marcas.
#[tauri::command(async)]
pub fn faultrec_stop(app: tauri::AppHandle) -> Result<Meta, String> {
    finish(&app, false)
}

/// Las grabaciones que hay en este equipo (las del caso de ahora y, si no hay caso, las de esta sesión).
#[tauri::command(async)]
pub fn faultrec_list(app: tauri::AppHandle) -> Vec<Meta> {
    list_dir(&dir(&app))
}

/// Una grabación con la ruta de su vídeo.
#[tauri::command(async)]
pub fn faultrec_get(app: tauri::AppHandle, id: String) -> Result<Detail, String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let d = dir(&app);
    let meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
    let video = video_file(&d, &id);
    if !video.exists() {
        return Err("El vídeo de esa grabación ya no está.".into());
    }
    let clip = read_clip(&d, &id).map(|c| [c.from, c.to]);
    Ok(Detail { meta, clip, path: video.to_string_lossy().into_owned() })
}

// ---------- Miniatura, informe y exportar (1.4, G4) ----------

fn thumb_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.thumb.jpg"))
}

/// El trozo que se mandó al informe (JSON, con extensión propia para que no se tome por una grabación).
fn clip_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.clip"))
}

/// Lo que se mete en el informe del cliente: tres fotogramas del trozo y las marcas que caen dentro.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportClip {
    pub id: String,
    /// Milisegundos del vídeo.
    pub from: u64,
    pub to: u64,
    /// Qué pasó («Excel no responde») o «Grabación del fallo».
    pub title: String,
    /// Segundos Unix del principio de la grabación.
    pub started: u64,
    /// Milisegundo de cada fotograma.
    pub times: Vec<u64>,
    /// Los fotogramas, JPEG en base64.
    pub frames: Vec<String>,
    /// «0:14 · Disco al 95 % (causa probable)».
    pub lines: Vec<String>,
}

fn read_clip(dir: &Path, id: &str) -> Option<ReportClip> {
    std::fs::read_to_string(clip_file(dir, id)).ok().and_then(|t| serde_json::from_str(&t).ok())
}

/// «1:12».
fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// El trozo `[from, to]` dentro de lo que dura la grabación (y con al menos un segundo).
fn clamp_range(meta: &Meta, from_ms: u64, to_ms: u64) -> (u64, u64) {
    let len = u64::from(meta.secs.max(1)) * 1000;
    let (a, b) = (from_ms.min(to_ms).min(len), from_ms.max(to_ms).min(len));
    if b.saturating_sub(a) < 1000 {
        (a.min(len.saturating_sub(1000)), (a + 1000).min(len))
    } else {
        (a, b)
    }
}

/// Qué pasó en el trozo, para el título del informe: el programa que se colgó o falló, si lo hay.
fn clip_title(meta: &Meta, from_ms: u64, to_ms: u64) -> String {
    let inside = |m: &Mark| (from_ms..=to_ms).contains(&(u64::from(m.t) * 1000));
    meta.marks
        .iter()
        .find(|m| inside(m) && m.app)
        .map(|m| m.title.clone())
        .or_else(|| meta.trigger.clone())
        .or_else(|| meta.marks.iter().find(|m| inside(m)).map(|m| m.title.clone()))
        .unwrap_or_else(|| "Grabación del fallo".to_string())
}

/// Las marcas del trozo, una por línea.
fn clip_lines(meta: &Meta, from_ms: u64, to_ms: u64) -> Vec<String> {
    meta.marks
        .iter()
        .filter(|m| (from_ms..=to_ms).contains(&(u64::from(m.t) * 1000)))
        .map(|m| format!("{} · {}{}", clock((u64::from(m.t) * 1000).saturating_sub(from_ms)), m.title, if m.probable { " (causa probable)" } else { "" }))
        .collect()
}

/// La miniatura de la lista: un fotograma del momento de la primera marca (o de la mitad), en JPEG. Se hace una vez y
/// se guarda junto al vídeo; se borra con él.
#[tauri::command(async)]
pub fn faultrec_thumb(app: tauri::AppHandle, id: String) -> Result<String, String> {
    use base64::Engine;
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let d = dir(&app);
    let cached = thumb_file(&d, &id);
    let bytes = match std::fs::read(&cached) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            let meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
            let at = meta.marks.iter().map(|m| u64::from(m.t) * 1000).min().unwrap_or(u64::from(meta.secs) * 500);
            let img = clip::frames_jpeg(&video_file(&d, &id), &[at], clip::THUMB_W)?.into_iter().next().ok_or("Sin fotograma.")?;
            let _ = std::fs::write(&cached, &img);
            img
        }
    };
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// «Al informe»: tres fotogramas del trozo y sus marcas quedan listos para el PDF del cliente (se reemplaza lo que
/// hubiera de esta grabación). El vídeo no sale del equipo.
#[tauri::command(async)]
pub fn faultrec_to_report(app: tauri::AppHandle, id: String, from_ms: u64, to_ms: u64) -> Result<ReportClip, String> {
    use base64::Engine;
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let d = dir(&app);
    let meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
    let (from, to) = clamp_range(&meta, from_ms, to_ms);
    let marks: Vec<u64> = meta.marks.iter().map(|m| u64::from(m.t) * 1000).collect();
    let times = clip::report_times(from, to, &marks);
    let frames = clip::frames_jpeg(&video_file(&d, &id), &times, clip::REPORT_W)?;
    let c = ReportClip {
        id: id.clone(),
        from,
        to,
        title: clip_title(&meta, from, to),
        started: meta.started,
        times,
        frames: frames.iter().map(|f| base64::engine::general_purpose::STANDARD.encode(f)).collect(),
        lines: clip_lines(&meta, from, to),
    };
    let text = serde_json::to_string(&c).map_err(|e| e.to_string())?;
    std::fs::write(clip_file(&d, &id), text).map_err(|e| format!("No se pudo guardar para el informe: {e}"))?;
    Ok(c)
}

/// Quitar la grabación del informe.
#[tauri::command]
pub fn faultrec_report_remove(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    match std::fs::remove_file(clip_file(&dir(&app), &id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("No se pudo quitar del informe: {e}")),
        _ => Ok(()),
    }
}

/// Lo que dejó «Exportar MP4».
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    pub path: String,
    pub ms: u64,
}

/// El nombre que se propone al exportar: «grabacion-2026-11-23-1042.mp4».
fn export_name(started: u64, from_ms: u64) -> String {
    let at = chrono::DateTime::from_timestamp((started + from_ms / 1000) as i64, 0).map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d-%H%M").to_string()).unwrap_or_default();
    if at.is_empty() {
        "grabacion.mp4".into()
    } else {
        format!("grabacion-{at}.mp4")
    }
}

/// «Exportar MP4»: pregunta dónde guardarlo y copia el trozo sin volver a codificar. `None` si se canceló. Es lo único
/// que saca el vídeo del equipo, y solo porque lo pide el técnico.
#[tauri::command(async)]
pub fn faultrec_export(app: tauri::AppHandle, id: String, from_ms: u64, to_ms: u64) -> Result<Option<Exported>, String> {
    use tauri_plugin_dialog::DialogExt;
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let d = dir(&app);
    let meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
    let (from, to) = clamp_range(&meta, from_ms, to_ms);
    let Some(mut out) = app.dialog().file().set_file_name(export_name(meta.started, from)).add_filter("Vídeo MP4", &["mp4"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    if !out.extension().is_some_and(|x| x.eq_ignore_ascii_case("mp4")) {
        out.set_extension("mp4");
    }
    let ms = clip::export(&video_file(&d, &id), &out, from, to)?;
    Ok(Some(Exported { path: out.to_string_lossy().into_owned(), ms }))
}

/// La carpeta temporal de AdminOps (lo que se copia al portapapeles va ahí con un nombre legible).
fn temp_out_dir() -> Result<PathBuf, String> {
    let out_dir = std::env::temp_dir().join("AdminOps");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("No se pudo preparar la carpeta temporal: {e}"))?;
    Ok(out_dir)
}

/// «Para enviar»: una copia pequeña (720p como mucho, 15 fotogramas por segundo) para mandarla por Teams o el correo.
/// Se deja en la carpeta temporal y se pone en el portapapeles como archivo. Tarda un poco: se vuelve a codificar.
#[tauri::command(async)]
pub fn faultrec_export_small(app: tauri::AppHandle, id: String) -> Result<Exported, String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let d = dir(&app);
    let meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
    let video = video_file(&d, &id);
    if !video.exists() {
        return Err("El vídeo de esa grabación ya no está.".into());
    }
    let out = temp_out_dir()?.join(small_name(&export_name(meta.started, 0)));
    let ms = clip::shrink(&video, &out)?;
    crate::rocketbar::copy_file_to_clipboard(&out)?;
    Ok(Exported { path: out.to_string_lossy().into_owned(), ms })
}

/// «GIF de 8 s»: un GIF del trozo (8 segundos como mucho, 480 de ancho, 10 fotogramas por segundo) y al portapapeles.
#[tauri::command(async)]
pub fn faultrec_export_gif(app: tauri::AppHandle, id: String, from_ms: u64, to_ms: u64) -> Result<Exported, String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let d = dir(&app);
    let meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
    let video = video_file(&d, &id);
    if !video.exists() {
        return Err("El vídeo de esa grabación ya no está.".into());
    }
    let (from, to) = clamp_range(&meta, from_ms, to_ms);
    let out = temp_out_dir()?.join(gif_name(&export_name(meta.started, from)));
    let ms = clip::gif(&video, &out, from, to)?;
    crate::rocketbar::copy_file_to_clipboard(&out)?;
    Ok(Exported { path: out.to_string_lossy().into_owned(), ms })
}

/// «grabacion-2026-11-23-1042.mp4» → «grabacion-2026-11-23-1042-para-enviar.mp4».
fn small_name(name: &str) -> String {
    format!("{}-para-enviar.mp4", name.trim_end_matches(".mp4"))
}

/// «grabacion-2026-11-23-1042.mp4» → «grabacion-2026-11-23-1042.gif».
fn gif_name(name: &str) -> String {
    format!("{}.gif", name.trim_end_matches(".mp4"))
}

// ---------- Al parar, desde la barra del cohete ----------

/// «Al caso»: la grabación pasa al caso abierto (así no se borra al cerrar AdminOps y sale en el caso).
/// Devuelve si ya estaba en él.
#[tauri::command(async)]
pub fn faultrec_attach_case(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let open = crate::cases::open_case_id(&app).ok_or("No hay ningún caso abierto: abre uno en AdminOps y vuelve a probar.")?;
    let d = dir(&app);
    let mut meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
    if meta.case_id.as_deref() == Some(open.as_str()) {
        return Ok(true);
    }
    meta.case_id = Some(open);
    write_meta(&d, &meta)?;
    Ok(false)
}

/// «Copiar»: el vídeo, como archivo, al portapapeles (para pegarlo en Teams, el correo o una carpeta). Se copia
/// antes a la carpeta temporal con un nombre legible, que es el que verá quien lo reciba.
#[tauri::command(async)]
pub fn faultrec_copy_file(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let d = dir(&app);
    let meta = read_meta(&meta_file(&d, &id)).ok_or("Esa grabación ya no está.")?;
    let out_dir = std::env::temp_dir().join("AdminOps");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let out = out_dir.join(export_name(meta.started, 0));
    std::fs::copy(video_file(&d, &id), &out).map_err(|e| format!("No se pudo preparar el vídeo: {e}"))?;
    crate::rocketbar::copy_file_to_clipboard(&out)
}

/// «Abrir»: el vídeo con el reproductor de Windows, como el usuario.
#[tauri::command(async)]
pub fn faultrec_open_file(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    let f = video_file(&dir(&app), &id);
    if !f.exists() {
        return Err("Esa grabación ya no está.".into());
    }
    crate::shellopen::open(&f.to_string_lossy())
}

/// Lo que va en el informe: los trozos mandados de las grabaciones del caso abierto (o, sin caso, de las que no
/// tienen caso), de la más antigua a la más nueva.
pub fn report_clips(app: &tauri::AppHandle) -> Vec<ReportClip> {
    let d = dir(app);
    let open = crate::cases::open_case_id(app);
    let mut out: Vec<ReportClip> = list_dir(&d).into_iter().filter(|m| m.case_id == open).filter_map(|m| read_clip(&d, &m.id)).collect();
    out.sort_by_key(|c| c.started);
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// La sección «Grabación del fallo» del informe: por cada trozo, sus tres fotogramas con el segundo y sus marcas.
pub fn report_html(clips: &[ReportClip]) -> String {
    use std::fmt::Write;
    if clips.is_empty() {
        return String::new();
    }
    let mut h = String::from("<h2>Grabación del fallo</h2>");
    for c in clips {
        let when = chrono::DateTime::from_timestamp((c.started + c.from / 1000) as i64, 0)
            .map(|t| t.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string())
            .unwrap_or_default();
        let _ = write!(h, "<h3>{} · {}</h3><div style=\"display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin:6px 0 4px\">", esc(&c.title), esc(&when));
        for (i, f) in c.frames.iter().enumerate() {
            let t = c.times.get(i).copied().unwrap_or(c.from).saturating_sub(c.from);
            let _ = write!(
                h,
                "<figure style=\"margin:0\"><img src=\"data:image/jpeg;base64,{f}\" alt=\"\" style=\"width:100%;border-radius:6px;border:1px solid #ddd\"><figcaption style=\"font-size:11px;color:#666\">{}</figcaption></figure>",
                clock(t)
            );
        }
        h.push_str("</div>");
        if !c.lines.is_empty() {
            h.push_str("<ul>");
            for l in &c.lines {
                let _ = write!(h, "<li>{}</li>", esc(l));
            }
            h.push_str("</ul>");
        }
    }
    h.push_str("<p class=muted>Fotogramas de la grabación hecha en el equipo; el vídeo no sale de él.</p>");
    h
}

/// Borra una grabación (vídeo y marcas).
#[tauri::command(async)]
pub fn faultrec_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("Grabación desconocida.".into());
    }
    if lock(&ACTIVE).as_ref().is_some_and(|a| a.id == id) {
        return Err("Esa grabación todavía está en marcha.".into());
    }
    let d = dir(&app);
    remove_recording(&d, &id);
    // Si otro programa tiene el vídeo abierto (el antivirus, un reproductor), no se da por borrado.
    if video_file(&d, &id).exists() {
        return Err("No se pudo borrar el vídeo: otro programa lo tiene abierto. Se borrará al cerrar AdminOps.".into());
    }
    Ok(())
}

// ---------- Vigilar y la grabación automática ----------

/// Enciende Vigilar: el último minuto en memoria, sin escribir nada en disco. Con `opts.input` apunta también clics y
/// teclas. Estado: ver `faultrec_watch_status`.
#[tauri::command(async)]
pub fn faultrec_watch_start(app: tauri::AppHandle, monitor: Option<String>, opts: Option<vigil::WatchOptions>) -> Result<vigil::WatchStatus, String> {
    let r = vigil::start(&app, monitor, opts);
    if r.is_ok() {
        crate::safestart::activity(SAFE_WATCHING, true);
    }
    r
}

/// Apaga Vigilar y suelta toda la memoria.
#[tauri::command(async)]
pub fn faultrec_watch_stop(app: tauri::AppHandle) -> vigil::WatchStatus {
    crate::safestart::activity(SAFE_WATCHING, false);
    vigil::stop(&app)
}

#[tauri::command(async)]
pub fn faultrec_watch_status(app: tauri::AppHandle) -> vigil::WatchStatus {
    let st = vigil::status(&app);
    // Si Vigilar se apagó solo (captura caída), deja de constar para el aviso de abrir a salvo.
    if !st.on && !st.paused_for_recording {
        crate::safestart::activity(SAFE_WATCHING, false);
    }
    st
}

/// «¡Acaba de pasar!»: guarda el último minuto como una grabación (`origin` «replay») y avisa con `faultrec://done`.
#[tauri::command(async)]
pub fn faultrec_just_happened(app: tauri::AppHandle) -> Result<Meta, String> {
    vigil::just_happened(&app)
}

#[tauri::command]
pub fn faultrec_auto_get(app: tauri::AppHandle) -> auto::AutoSettings {
    auto::get(&app)
}

#[tauri::command(async)]
pub fn faultrec_auto_set(app: tauri::AppHandle, settings: auto::AutoSettings) -> Result<auto::AutoSettings, String> {
    auto::set(&app, settings)
}

/// «No volver a grabar por esto»: añade `key` (`app:<exe>`, `cpu` o `disk`) a la lista, sin repetir.
#[tauri::command(async)]
pub fn faultrec_auto_ignore(app: tauri::AppHandle, key: String, label: String) -> Result<auto::AutoSettings, String> {
    let key = key.trim().to_lowercase();
    if !auto::valid_key(&key) {
        return Err("Eso no se puede ignorar.".into());
    }
    let mut s = auto::get(&app);
    if !s.ignored.iter().any(|i| i.key == key) {
        s.ignored.push(auto::Ignored { key, label });
    }
    auto::set(&app, s)
}

/// Quita `key` de «No volver a grabar por esto».
#[tauri::command(async)]
pub fn faultrec_auto_unignore(app: tauri::AppHandle, key: String) -> Result<auto::AutoSettings, String> {
    let key = key.trim().to_lowercase();
    let mut s = auto::get(&app);
    s.ignored.retain(|i| i.key != key);
    auto::set(&app, s)
}

/// Borra las grabaciones de un caso (al cerrarlo o descartarlo).
pub fn delete_for_case(app: &tauri::AppHandle, case_id: &str) {
    let n = delete_case_in(&dir(app), case_id);
    if n > 0 {
        log::info!("Grabar el fallo: se borraron {n} grabaciones del caso cerrado");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camara_en_la_esquina() {
        // Monitor 1920×1080 en (0,0): lado 270, abajo a la derecha con margen 33.
        assert_eq!(super::camera_rect((0, 0, 1920, 1080), None), (1920 - 270 - 33, 1080 - 270 - 33, 270));
        // Zona pequeña: nunca más grande que la zona, y dentro de ella.
        let (x, y, s) = super::camera_rect((100, 0, 1920, 1080), Some(super::Region { x: 10, y: 20, w: 120, h: 100 }));
        assert!(s <= 100 && x >= 110 && y >= 20, "{x} {y} {s}");
    }

    #[test]
    fn g4_trozo_titulo_lineas_y_nombre() {
        let m = Meta {
            id: "fr-1".into(),
            secs: 40,
            marks: vec![
                Mark { t: 14, kind: Kind::Disk, title: "Disco al 95 %".into(), probable: true, ..Default::default() },
                Mark { t: 22, kind: Kind::Hang, title: "Excel no responde".into(), app: true, ..Default::default() },
                Mark { t: 35, kind: Kind::Error, title: "Error 1000".into(), app: true, ..Default::default() },
            ],
            ..Default::default()
        };
        // Dentro de lo que dura, al derecho y con al menos un segundo.
        assert_eq!(clamp_range(&m, 27_000, 13_000), (13_000, 27_000));
        assert_eq!(clamp_range(&m, 39_800, 90_000), (39_000, 40_000));
        assert_eq!(clamp_range(&m, 5_000, 5_000), (5_000, 6_000));
        // El título es el programa que falló dentro del trozo.
        assert_eq!(clip_title(&m, 13_000, 27_000), "Excel no responde");
        assert_eq!(clip_title(&m, 13_000, 15_000), "Disco al 95 %");
        assert_eq!(clip_title(&m, 0, 5_000), "Grabación del fallo");
        // Las marcas, con el segundo contado desde el principio del trozo.
        assert_eq!(clip_lines(&m, 13_000, 27_000), vec!["0:01 · Disco al 95 % (causa probable)".to_string(), "0:09 · Excel no responde".to_string()]);
        assert!(export_name(0, 0).starts_with("grabacion"));
        // La sección del informe escapa el texto y lleva los tres fotogramas.
        let c = ReportClip { id: "fr-1".into(), from: 13_000, to: 27_000, title: "<b>Excel</b>".into(), started: 1_700_000_000, times: vec![13_000, 20_000, 26_800], frames: vec!["AAA".into(), "BBB".into(), "CCC".into()], lines: vec!["0:09 · Excel no responde".into()] };
        let h = report_html(&[c]);
        assert!(h.contains("Grabación del fallo") && h.contains("&lt;b&gt;Excel") && h.matches("data:image/jpeg;base64,").count() == 3 && h.contains("0:07"));
        assert!(report_html(&[]).is_empty());
    }
    use analysis::Kind;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("adminops-faultrec-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(dir: &Path, id: &str, case: Option<&str>, started: u64) {
        std::fs::write(video_file(dir, id), b"mp4").unwrap();
        let m = Meta { id: id.into(), started, secs: 72, monitor: "Monitor principal".into(), width: 1920, height: 1080, case_id: case.map(String::from), marks: vec![Mark { t: 24, kind: Kind::Hang, title: "Excel no responde".into(), app: true, ..Default::default() }], ..Default::default() };
        write_meta(dir, &m).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn los_nombres_de_archivo_y_los_id() {
        assert_eq!(new_id(1_700_000_000), "fr-1700000000");
        assert!(valid_id("fr-1700000000"));
        for bad in ["", "fr-", "fr-12a", "fr-../x", "x-123", "fr-1/2", "fr-1\\2", "../fr-1", "fr-123456789012345678901"] {
            assert!(!valid_id(bad), "{bad}");
        }
        let d = Path::new("recordings");
        assert_eq!(video_file(d, "fr-1"), Path::new("recordings").join("fr-1.mp4"));
        assert_eq!(part_file(d, "fr-1"), Path::new("recordings").join("fr-1.mp4.part"));
        assert_eq!(meta_file(d, "fr-1"), Path::new("recordings").join("fr-1.json"));
    }

    #[test]
    fn el_json_se_guarda_y_se_lee() {
        let d = temp("json");
        put(&d, "fr-100", Some("caso-1"), 100);
        let back = read_meta(&meta_file(&d, "fr-100")).unwrap();
        assert_eq!((back.secs, back.width, back.case_id.as_deref()), (72, 1920, Some("caso-1")));
        assert_eq!(back.marks[0].title, "Excel no responde");
        // Un json roto o con un id que no es el nuestro no vale.
        std::fs::write(meta_file(&d, "fr-101"), "{no es json").unwrap();
        assert!(read_meta(&meta_file(&d, "fr-101")).is_none());
        std::fs::write(meta_file(&d, "fr-102"), r#"{"id":"../../x"}"#).unwrap();
        assert!(read_meta(&meta_file(&d, "fr-102")).is_none());
        // Sin campos nuevos sigue leyéndose (datos de otra versión).
        std::fs::write(meta_file(&d, "fr-103"), r#"{"id":"fr-103","started":5}"#).unwrap();
        assert_eq!(read_meta(&meta_file(&d, "fr-103")).unwrap().started, 5);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_lista_va_de_mas_reciente_a_mas_antigua_y_pide_el_video() {
        let d = temp("list");
        put(&d, "fr-100", None, 100);
        put(&d, "fr-300", None, 300);
        put(&d, "fr-200", None, 200);
        std::fs::remove_file(video_file(&d, "fr-200")).unwrap();
        let ids: Vec<String> = list_dir(&d).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["fr-300", "fr-100"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cerrar_un_caso_borra_solo_sus_grabaciones() {
        let d = temp("case");
        put(&d, "fr-1", Some("a"), 1);
        put(&d, "fr-2", Some("b"), 2);
        put(&d, "fr-3", Some("a"), 3);
        put(&d, "fr-4", None, 4);
        assert_eq!(delete_case_in(&d, "a"), 2);
        assert_eq!(names(&d), vec!["fr-2.json", "fr-2.mp4", "fr-4.json", "fr-4.mp4"]);
        assert_eq!(delete_case_in(&d, "a"), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn al_arrancar_solo_queda_lo_del_caso_abierto() {
        let d = temp("start");
        put(&d, "fr-1", Some("abierto"), 1);
        put(&d, "fr-2", Some("cerrado"), 2);
        put(&d, "fr-3", None, 3);
        std::fs::write(part_file(&d, "fr-4"), b"a medias").unwrap();
        // Un vídeo sin sus marcas y unas marcas sin su vídeo tampoco valen.
        std::fs::write(video_file(&d, "fr-5"), b"mp4").unwrap();
        write_meta(&d, &Meta { id: "fr-6".into(), case_id: Some("abierto".into()), ..Default::default() }).unwrap();
        // Lo que no es nuestro no se toca.
        std::fs::write(d.join("notas.txt"), b"mio").unwrap();
        let n = sweep_dir(&d, Some("abierto"), Sweep::Start);
        assert_eq!(names(&d), vec!["fr-1.json", "fr-1.mp4", "notas.txt"]);
        assert!(n >= 5, "{n}");
        // Sin caso abierto no se queda nada.
        sweep_dir(&d, None, Sweep::Start);
        assert_eq!(names(&d), vec!["notas.txt"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn al_cerrar_se_van_las_que_no_tienen_caso() {
        let d = temp("exit");
        put(&d, "fr-1", Some("abierto"), 1);
        put(&d, "fr-2", None, 2);
        sweep_dir(&d, None, Sweep::Exit);
        assert_eq!(names(&d), vec!["fr-1.json", "fr-1.mp4"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn limpiar_una_carpeta_que_no_existe_no_falla() {
        assert_eq!(sweep_dir(Path::new("no-existe-adminops-faultrec"), None, Sweep::Start), 0);
        assert!(list_dir(Path::new("no-existe-adminops-faultrec")).is_empty());
    }

    #[test]
    fn el_script_de_eventos_es_correcto_y_no_mete_numeros_sin_comillas() {
        let script = events_script(1_700_000_000, 1_700_000_075);
        assert!(script.starts_with("$fromText = '1700000000'\n$toText = '1700000075'\n"), "{script}");
        let e = crate::ps::parse_errors(&script);
        assert!(e.is_empty(), "{e}");
    }

    #[test]
    fn las_notas_se_juntan_en_orden_y_sin_vacias() {
        assert_eq!(join_notes(vec![]), None);
        assert_eq!(join_notes(vec!["".into(), "  ".into()]), None);
        assert_eq!(join_notes(vec!["Una.".into()]).as_deref(), Some("Una."));
        assert_eq!(join_notes(vec!["Uno.".into(), "".into(), "Dos.".into()]).as_deref(), Some("Uno. Dos."));
    }

    #[test]
    fn lo_que_se_graba_se_cuenta_sin_el_titulo_de_la_ventana() {
        assert_eq!(region_target(800, 600), "Zona de 800 × 600");
        assert_eq!(region_target(1920, 64), "Zona de 1920 × 64");
    }


    #[test]
    fn el_tiempo_del_video_no_cuenta_las_pausas() {
        let pauses: Vec<Pause> = vec![(10_000, 14_000), (20_000, 21_000)];
        assert_eq!(video_ms(&pauses, None, 5_000), 5_000);
        // Dentro de una pausa se queda donde empezó.
        assert_eq!(video_ms(&pauses, None, 12_000), 10_000);
        assert_eq!(video_ms(&pauses, None, 14_000), 10_000);
        assert_eq!(video_ms(&pauses, None, 18_000), 14_000);
        assert_eq!(video_ms(&pauses, None, 30_000), 25_000);
        // Una pausa abierta cuenta hasta ahora.
        assert_eq!(video_ms(&pauses, Some(40_000), 45_000), 45_000 - 4_000 - 1_000 - 5_000);
        assert_eq!(video_ms(&[], None, 7), 7);
        assert!(in_pause(&pauses, 10_000) && in_pause(&pauses, 13_999) && !in_pause(&pauses, 14_000) && !in_pause(&pauses, 9_999));
        assert_eq!(paused_total_ms(&pauses), 5_000);
    }

    #[test]
    fn lo_medido_y_lo_pulsado_se_pasa_al_tiempo_del_video() {
        let pauses: Vec<Pause> = vec![(2_000, 5_000)];
        let tick = |t: u32| Tick { t, cpu: t as f32, ..Default::default() };
        let out = remap_ticks(&[tick(0), tick(1), tick(2), tick(3), tick(4), tick(5), tick(6)], &pauses);
        assert_eq!(out.iter().map(|t| t.t).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_eq!(out.iter().map(|t| t.cpu as u32).collect::<Vec<_>>(), vec![0, 1, 5, 6]);
        let ev = |t: u32| InputEvent { t, kind: "keys".into(), ..Default::default() };
        let input = remap_input(vec![ev(1_000), ev(3_000), ev(5_500)], &pauses);
        assert_eq!(input.iter().map(|e| e.t).collect::<Vec<_>>(), vec![1_000, 2_500]);
        // Sin pausas no se toca nada.
        assert_eq!(remap_input(vec![ev(9)], &[]).len(), 1);
    }

    #[test]
    fn los_nombres_de_las_copias_para_enviar() {
        assert_eq!(small_name("grabacion-2026-11-23-1042.mp4"), "grabacion-2026-11-23-1042-para-enviar.mp4");
        assert_eq!(gif_name("grabacion-2026-11-23-1042.mp4"), "grabacion-2026-11-23-1042.gif");
        assert_eq!(gif_name("grabacion.mp4"), "grabacion.gif");
    }

    #[test]
    fn las_opciones_nuevas_del_audio_del_equipo_se_leen() {
        let none: StartOptions = serde_json::from_str("{}").unwrap();
        assert!(!none.system_audio);
        let o: StartOptions = serde_json::from_str(r#"{"systemAudio":true,"mic":true}"#).unwrap();
        assert!(o.system_audio && o.mic);
    }

    #[test]
    fn las_opciones_de_empezar_se_leen_en_camel_case_y_todo_es_opcional() {
        let none: StartOptions = serde_json::from_str("{}").unwrap();
        assert_eq!((none.window, none.region, none.input, none.mic), (None, None, false, false));
        let o: StartOptions = serde_json::from_str(r#"{"window":263950,"region":{"x":10,"y":20,"w":800,"h":600},"input":true,"mic":true}"#).unwrap();
        assert_eq!(o.window, Some(263_950));
        assert_eq!(o.region, Some(Region { x: 10, y: 20, w: 800, h: 600 }));
        assert!(o.input && o.mic);
        // Un HWND de 64 bits cabe.
        let big: StartOptions = serde_json::from_str(r#"{"window":18446744073709551615}"#).unwrap();
        assert_eq!(big.window, Some(u64::MAX));
        // Una zona negativa (el monitor a la izquierda del principal, ya relativa a él) no se admite como tal,
        // pero x e y negativos sí se leen: `crop_rect` los recorta.
        let neg: StartOptions = serde_json::from_str(r#"{"region":{"x":-5,"y":-5,"w":100,"h":100}}"#).unwrap();
        assert_eq!(neg.region.map(|r| (r.x, r.y)), Some((-5, -5)));
    }

    #[test]
    fn la_lista_de_ventanas_y_el_aviso_de_zona_van_en_camel_case() {
        let w = RecWindow { id: 131_330, title: "Informe.xlsx - Excel".into(), app: "Microsoft Excel".into(), minimized: true };
        assert_eq!(serde_json::to_string(&w).unwrap(), r#"{"id":131330,"title":"Informe.xlsx - Excel","app":"Microsoft Excel","minimized":true}"#);
        let p = RegionPayload { monitor: Some("\\\\?\\DISPLAY#X".into()), region: Some(Region { x: 1, y: 2, w: 3, h: 4 }) };
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["region"], serde_json::json!({"x":1,"y":2,"w":3,"h":4}));
        assert!(v["monitor"].is_string());
        let cancelled = serde_json::to_value(RegionPayload { monitor: None, region: None }).unwrap();
        assert_eq!(cancelled, serde_json::json!({"monitor":null,"region":null}));
    }

    #[test]
    fn una_grabacion_con_todo_se_guarda_y_se_lee_y_la_vieja_sigue_valiendo() {
        let d = temp("meta2");
        let m = Meta {
            id: "fr-200".into(),
            started: 200,
            secs: 3,
            target: "Ventana · Excel".into(),
            input_on: true,
            input: vec![InputEvent { t: 800, kind: "click".into(), x: 0.25, y: 0.5, label: "Doble clic".into() }, InputEvent { t: 1500, kind: "keys".into(), label: "Ctrl + P".into(), ..Default::default() }],
            voice: vec![0, 12, 80],
            mic: true,
            ..Default::default()
        };
        write_meta(&d, &m).unwrap();
        assert_eq!(read_meta(&meta_file(&d, "fr-200")).unwrap(), m);
        let json = std::fs::read_to_string(meta_file(&d, "fr-200")).unwrap();
        for key in ["\"target\"", "\"inputOn\"", "\"input\"", "\"voice\"", "\"mic\""] {
            assert!(json.contains(key), "{key} en {json}");
        }
        // Una grabación de antes de estos campos se lee con los valores por defecto.
        std::fs::write(meta_file(&d, "fr-201"), r#"{"id":"fr-201","started":5,"monitor":"Monitor principal"}"#).unwrap();
        let old = read_meta(&meta_file(&d, "fr-201")).unwrap();
        assert_eq!((old.target.as_str(), old.input_on, old.input.len(), old.voice.len(), old.mic), ("", false, 0, 0, false));
        // Las de antes de Vigilar cuentan como grabadas a mano; las nuevas llevan su origen y su disparador en camelCase.
        assert_eq!((old.origin.as_str(), old.trigger.as_deref(), old.trigger_key.as_deref()), ("record", None, None));
        let auto = Meta { id: "fr-202".into(), origin: "auto".into(), trigger: Some("Excel no responde".into()), trigger_key: Some("app:excel.exe".into()), ..Default::default() };
        write_meta(&d, &auto).unwrap();
        let json = std::fs::read_to_string(meta_file(&d, "fr-202")).unwrap();
        assert!(json.contains("\"origin\": \"auto\"") && json.contains("\"triggerKey\": \"app:excel.exe\"") && json.contains("\"trigger\": \"Excel no responde\""), "{json}");
        assert_eq!(read_meta(&meta_file(&d, "fr-202")).unwrap(), auto);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Graba 3 segundos con lo que se diga y devuelve lo que dejó y lo que se lee del MP4.
    #[cfg(windows)]
    fn record_three_seconds(name: &str, window: Option<isize>, region: Option<Region>, mic: Option<mic::Mic>) -> (capture::Done, u64, u32, u32, bool, Vec<InputEvent>) {
        let d = temp(name);
        let video = video_file(&d, "fr-1");
        let first: Arc<Mutex<Option<Started>>> = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let job = capture::Job { gdi: String::new(), window, region, mic, sys: None, pause: Arc::new(AtomicBool::new(false)), out: video.clone(), stop: stop.clone(), first: first.clone(), max: Duration::from_secs(3) };
        // Clics y teclas a la vez, como al grabar de verdad (aquí nadie pulsa nada).
        let (f2, s2) = (first.clone(), stop.clone());
        let watch = input::Watch { gdi: String::new(), window, region };
        let inp = std::thread::spawn(move || input::run(f2, s2, watch));
        let done = capture::record(job).expect("grabar");
        stop.store(true, Ordering::SeqCst);
        let events = inp.join().expect("el hilo de clics y teclas");
        let info = capture::video_info(&video).expect("leer el vídeo grabado");
        let _ = std::fs::remove_dir_all(&d);
        (done, info.duration_ms, info.width, info.height, info.has_audio, events)
    }

    /// Una ventana de verdad: la de arriba que no esté minimizada.
    /// `cargo test faultrec -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore = "graba una ventana de este equipo durante 3 segundos"]
    fn graba_de_verdad_una_ventana() {
        let list = win::list_windows();
        println!("VENTANAS: {} · {:?}", list.len(), list.iter().map(|w| (w.app.as_str(), w.minimized)).collect::<Vec<_>>());
        let Some(w) = list.iter().find(|w| !w.minimized) else {
            println!("No hay ninguna ventana abierta que grabar: prueba saltada.");
            return;
        };
        assert!(!w.app.is_empty() && !w.title.is_empty());
        let (done, ms, vw, vh, audio, events) = record_three_seconds("ventana", Some(w.id as usize as isize), None, None);
        println!("VENTANA {}: {}x{} · {} ms (medida {} ms) · audio {audio} · {} eventos", w.app, vw, vh, ms, done.duration_ms, events.len());
        assert_eq!((vw, vh), (done.width, done.height));
        assert!(vw % 2 == 0 && vh % 2 == 0 && vw >= 2 && vh >= 2);
        assert!((2_500..=4_000).contains(&ms), "duración {ms}");
        assert!(!audio && done.voice.is_empty() && !done.with_voice);
    }

    /// Una zona de verdad: 640×360 desde (40, 40) del monitor principal.
    #[cfg(windows)]
    #[test]
    #[ignore = "graba una zona de la pantalla de este equipo durante 3 segundos"]
    fn graba_de_verdad_una_zona() {
        let zone = Region { x: 40, y: 40, w: 640, h: 360 };
        let (done, ms, vw, vh, audio, events) = record_three_seconds("zona", None, Some(zone), None);
        println!("ZONA: {vw}x{vh} · {ms} ms (medida {} ms) · audio {audio} · {} eventos", done.duration_ms, events.len());
        assert_eq!((vw, vh), (640, 360));
        assert_eq!((done.width, done.height), (640, 360));
        assert!((2_500..=4_000).contains(&ms), "duración {ms}");
    }

    /// Con el micrófono por defecto: si no hay, o Windows no lo deja, se salta.
    #[cfg(windows)]
    #[test]
    #[ignore = "graba el micrófono de este equipo durante 3 segundos"]
    fn graba_de_verdad_la_voz() {
        let mic = match mic::start() {
            Ok(m) => m,
            Err(e) => {
                println!("MICRÓFONO: no se puede usar ({}): prueba saltada.", e.note());
                return;
            }
        };
        let (done, ms, _, _, audio, _) = record_three_seconds("voz", None, None, Some(mic));
        println!("VOZ: {ms} ms · pista de audio {audio} · {} décimas · volumen máximo {:?}", done.voice.len(), done.voice.iter().max());
        assert!(audio, "el MP4 no lleva la pista de voz");
        assert!(done.with_voice);
        assert!((2_500..=4_000).contains(&ms), "duración {ms}");
        // Una décima de volumen por cada décima de segundo, salvo lo que le falte al principio o al final.
        assert!((25..=35).contains(&done.voice.len()), "{} décimas", done.voice.len());
        assert!(done.voice.iter().all(|&v| v <= 100));
    }

    /// La grabación de verdad: 3 segundos del monitor principal en este equipo.
    /// `cargo test faultrec -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore = "graba la pantalla de este equipo durante 3 segundos"]
    fn graba_de_verdad_tres_segundos() {
        let d = temp("real");
        let part = part_file(&d, "fr-1");
        let first: Arc<Mutex<Option<Started>>> = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let job = capture::Job { gdi: String::new(), window: None, region: None, mic: None, sys: None, pause: Arc::new(AtomicBool::new(false)), out: part.clone(), stop: stop.clone(), first: first.clone(), max: Duration::from_secs(3) };
        // El hilo que apunta procesador, disco y programas colgados, como al grabar de verdad.
        let (f2, s2) = (first.clone(), stop.clone());
        let watcher = std::thread::spawn(move || watch::run(f2, s2));
        let t0 = Instant::now();
        let done = capture::record(job).expect("grabar");
        let wall = t0.elapsed();
        stop.store(true, Ordering::SeqCst);
        let ticks = watcher.join().expect("el hilo de medidas");
        println!("MEDIDAS: {} segundos apuntados · {:?}", ticks.len(), ticks.iter().map(|t| (t.t, t.cpu.round(), t.disk.map(f32::round))).collect::<Vec<_>>());
        assert!((2..=3).contains(&ticks.len()), "{} medidas", ticks.len());
        assert!(ticks.iter().enumerate().all(|(i, t)| t.t as usize == i && (0.0..=100.0).contains(&t.cpu)));
        let size = std::fs::metadata(&part).expect("el archivo existe").len();
        let video = video_file(&d, "fr-1");
        std::fs::rename(&part, &video).unwrap();
        let info = capture::video_info(&video).expect("leer el vídeo grabado");
        println!("GRABACION: {size} bytes · {}x{} · duración del MP4 {} ms · medida al grabar {} ms · reloj {} ms", done.width, done.height, info.duration_ms, done.duration_ms, wall.as_millis());
        assert!(size > 0);
        assert!(first.lock().unwrap().is_some());
        assert_eq!((info.width, info.height), (done.width, done.height));
        assert!((2_500..=4_000).contains(&info.duration_ms), "duración {}", info.duration_ms);
        let _ = std::fs::remove_dir_all(&d);
    }
}
