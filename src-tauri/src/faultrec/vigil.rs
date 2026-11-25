//! Vigilar: el último minuto siempre en memoria («¡Acaba de pasar!») y, si se quiere, la grabación automática.
//!
//! Mientras está encendido hay cuatro hilos, cada uno con su COM:
//! - la captura del monitor (5 fotogramas por segundo, 1280 de ancho como mucho), que codifica al momento con el
//!   H.264 de Media Foundation (`mfenc`) y guarda lo que sale en un búfer circular de 75 s (`ring`);
//! - el vigilante (`watch`), que apunta cada segundo procesador, disco y programas colgados, y mira los disparadores
//!   de la grabación automática (`auto`);
//! - los clics y las teclas, si se pidieron;
//! - el atajo global Ctrl+Alt+G (solo mientras Vigilar está encendido).
//!
//! Nada se escribe en disco hasta que se pide guardar una ventana de tiempo: el MP4 se escribe con lo ya comprimido
//! (sin volver a codificar), con las marcas de esos segundos, igual que una grabación normal. Sin micrófono nunca y
//! sin píldora (es largo); el borde amarillo de Windows se queda: es el aviso de que se mira la pantalla.

use super::analysis::{self, Tick};
use super::auto::{self, AutoSettings, Gate, Notifier, Offer, Trigger};
use super::input::{InputEvent, Recorder};
use super::mfenc::{self, StreamInfo};
use super::ring::{self, VideoRing};
use super::{now_unix, Meta, Started};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::Emitter;

/// Segundos de vídeo que se guardan (contados desde un fotograma clave).
pub const KEEP_SECS: u32 = 75;
/// Tope de memoria del vídeo.
pub const MAX_BYTES: usize = 40 * 1024 * 1024;
/// Fotogramas por segundo.
pub const FPS: u32 = 5;
/// Ancho máximo del vídeo.
pub const WIDTH: u32 = 1280;
/// Bits por segundo.
pub const BITRATE: u32 = 1_200_000;
/// Un fotograma clave cada 2 s.
pub const GOP: u32 = 2 * FPS;
/// Lo que se guarda de medidas y de clics: un poco más que el vídeo, que llega hasta un GOP más allá de los 75 s.
const KEEP_AUX_MS: u32 = 80_000;
/// Lo que guarda «¡Acaba de pasar!».
pub const JUST_HAPPENED_SECS: u64 = 60;
/// El atajo que guarda el último minuto.
pub const SHORTCUT: &str = "Ctrl+Alt+G";
#[cfg(windows)]
const HOTKEY_ID: i32 = 1;
/// Lo que se espera a que arranque la captura.
const START_WAIT: Duration = Duration::from_secs(10);

/// Qué más se apunta además del vídeo.
#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct WatchOptions {
    /// Apuntar clics y teclas (las letras que se escriben nunca se guardan).
    pub input: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchStatus {
    pub on: bool,
    /// Segundos Unix de cuando se encendió.
    pub since: Option<u64>,
    pub buffered_secs: u32,
    pub monitor: String,
    pub auto: AutoSettings,
    /// Está apagado solo porque se está grabando; al terminar vuelve a encenderse.
    pub paused_for_recording: bool,
    /// El atajo Ctrl+Alt+G está registrado (otro programa puede tenerlo).
    pub hotkey: bool,
    /// Las grabaciones automáticas que pasaron de los límites hoy.
    pub skipped_today: u32,
    /// Por qué se apagó solo la última vez (monitor desconectado…), una sola vez.
    pub error: Option<String>,
}

/// Lo que cuenta `faultrec://auto` al guardar una grabación automática.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct AutoPayload {
    id: String,
    trigger: String,
    /// Segundos Unix.
    at: u64,
}

/// Lo que cuenta `faultrec://open` al pulsar un aviso de Windows: la grabación a abrir.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct OpenPayload {
    id: String,
}

/// Con qué origen y por qué se guarda una ventana.
#[derive(Clone, Debug)]
pub struct Origin {
    /// `replay` o `auto`.
    pub kind: &'static str,
    pub trigger: Option<String>,
    pub trigger_key: Option<String>,
}

// ---------- Lo que comparten los hilos ----------

/// Los límites de la automática (2 por programa y hora, 10 al día) y el aviso agrupado (uno cada 10 min) son de
/// todo el día, no de cada vigilancia: pausar para grabar a mano o volver a encender Vigilar no los pone a cero.
static GATE: std::sync::LazyLock<Mutex<Gate>> = std::sync::LazyLock::new(|| Mutex::new(Gate::default()));
static NOTIFIER: std::sync::LazyLock<Mutex<Notifier>> = std::sync::LazyLock::new(|| Mutex::new(Notifier::default()));

struct Shared {
    video: Mutex<VideoRing>,
    info: Mutex<Option<StreamInfo>>,
    ticks: Mutex<VecDeque<Tick>>,
    input: Arc<Mutex<Recorder>>,
    input_on: bool,
    started: Started,
    monitor: String,
    size: Mutex<(u32, u32)>,
    stop: Arc<AtomicBool>,
    error: Mutex<Option<String>>,
}

struct Hotkey {
    thread_id: u32,
    handle: JoinHandle<()>,
}

struct Session {
    shared: Arc<Shared>,
    capture: Option<JoinHandle<()>>,
    watcher: Option<JoinHandle<()>>,
    input: Option<JoinHandle<()>>,
    hotkey: Option<Hotkey>,
    /// El monitor que se pidió y las opciones, para volver a encender igual tras una grabación.
    monitor_arg: Option<String>,
    opts: WatchOptions,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
/// Vigilar estaba encendido al pedir una grabación normal: al terminar vuelve a encenderse así.
static RESUME: Mutex<Option<(Option<String>, WatchOptions)>> = Mutex::new(None);
/// AdminOps se está cerrando: no se guarda nada más.
static SHUTTING: AtomicBool = AtomicBool::new(false);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn local_hhmm(unix: u64) -> String {
    chrono::DateTime::from_timestamp(unix as i64, 0).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default()
}

/// Un número que cambia a medianoche (hora local).
fn local_day() -> u32 {
    use chrono::Datelike;
    chrono::Local::now().date_naive().num_days_from_ce().max(0) as u32
}

// ---------- Avisos de Windows ----------

/// Un aviso de Windows que, al pulsarlo, trae AdminOps al frente y le dice qué grabación abrir.
fn notify(app: &tauri::AppHandle, text: &str, id: Option<String>) {
    use tauri_plugin_notification::NotificationExt;
    let handle = app.clone();
    let shown = crate::toast::show(app, "AdminOps", text, &[], move |_| {
        crate::tray::show_main(&handle);
        if let Some(id) = &id {
            let _ = handle.emit("faultrec://open", OpenPayload { id: id.clone() });
        }
    });
    if !shown {
        let _ = app.notification().builder().title("AdminOps").body(text.chars().take(240).collect::<String>()).show();
    }
}

// ---------- Guardar una ventana de tiempo ----------

type EventsRead = (Vec<crate::eventdigest::RawEvent>, Option<String>);

/// Escribe en `dir` la grabación de `[from_ms, to_ms]` (milisegundos de la sesión): MP4 con lo ya comprimido, marcas e
/// input de esos segundos. `events` da los errores del registro de Windows (desde Unix, segundos) y, si no se
/// pudieron leer, la nota que lo cuenta.
fn save_window(shared: &Shared, dir: &Path, case_id: Option<String>, from_ms: u64, to_ms: u64, origin: &Origin, events: impl FnOnce(u64, u32) -> EventsRead) -> Result<Meta, String> {
    const EMPTY: &str = "Todavía no hay nada guardado en memoria: espera unos segundos.";
    let (from, to) = ((from_ms as i64).saturating_mul(10_000), (to_ms as i64).saturating_mul(10_000));
    let samples = lock(&shared.video).window(from, to.max(from)).ok_or(EMPTY)?;
    let (Some(first), Some(last)) = (samples.first(), samples.last()) else { return Err(EMPTY.into()) };
    let start_ms = (first.time.max(0) / 10_000) as u64;
    let end_ms = (last.end().max(first.time) / 10_000) as u64;
    let secs = (((end_ms - start_ms) + 500) / 1000).max(1) as u32;
    let start_s = (start_ms / 1000) as u32;
    let started_unix = shared.started.unix + u64::from(start_s);
    let info = match lock(&shared.info).clone() {
        Some(i) => i,
        None => {
            // Sin las cabeceras del tipo de salida, se sacan del primer fotograma clave.
            let (width, height) = *lock(&shared.size);
            let header = mfenc::extract_sps_pps(&first.data).ok_or("El codificador no dio la cabecera del vídeo.")?;
            StreamInfo { width, height, fps: FPS, bitrate: BITRATE, header }
        }
    };
    let ticks: Vec<Tick> = ring::window_ticks(&lock(&shared.ticks), start_s, start_s.saturating_add(secs).saturating_add(1));
    let input: Vec<InputEvent> = ring::window_input(&lock(&shared.input).snapshot(), start_ms.min(u64::from(u32::MAX)) as u32, end_ms.min(u64::from(u32::MAX)) as u32);

    std::fs::create_dir_all(dir).map_err(|e| format!("No se pudo preparar la carpeta de grabaciones: {e}"))?;
    let id = unique_id(dir);
    let (part, video) = (super::part_file(dir, &id), super::video_file(dir, &id));
    let (found, events_note) = events(started_unix, secs);
    if let Err(e) = mfenc::save_mp4(&part, &samples, &info) {
        let _ = std::fs::remove_file(&part);
        return Err(format!("No se pudo guardar el vídeo: {e}"));
    }
    if SHUTTING.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&part);
        return Err("AdminOps se está cerrando: no se guardó nada.".into());
    }
    std::fs::rename(&part, &video).map_err(|e| format!("No se pudo guardar el vídeo: {e}"))?;
    let bytes = std::fs::metadata(&video).map_or(0, |m| m.len());
    let meta = Meta {
        id: id.clone(),
        started: started_unix,
        secs,
        monitor: shared.monitor.clone(),
        width: info.width,
        height: info.height,
        case_id,
        marks: analysis::build_marks(&ticks, &found, started_unix, secs),
        samples: analysis::samples_of(&ticks),
        bytes,
        note: events_note,
        target: "Pantalla entera".to_string(),
        input_on: shared.input_on,
        input,
        voice: Vec::new(),
        mic: false,
        origin: origin.kind.to_string(),
        trigger: origin.trigger.clone(),
        trigger_key: origin.trigger_key.clone(),
    };
    if let Err(e) = super::write_meta(dir, &meta) {
        super::remove_recording(dir, &id);
        return Err(format!("No se pudieron guardar las marcas: {e}"));
    }
    Ok(meta)
}

/// Un id libre: si ya hay una grabación de este segundo (dos atajos seguidos), el siguiente.
/// Un id libre, reservado al momento creando su `.part` vacío: dos guardados a la vez (Ctrl+Alt+G, el botón y una
/// automática) no pueden tomar el mismo aunque lleguen en el mismo segundo.
pub(super) fn unique_id(dir: &Path) -> String {
    let mut unix = now_unix();
    for _ in 0..10_000 {
        let id = super::new_id(unix);
        if !super::video_file(dir, &id).exists() && !super::meta_file(dir, &id).exists() && std::fs::OpenOptions::new().write(true).create_new(true).open(super::part_file(dir, &id)).is_ok() {
            return id;
        }
        unix += 1;
    }
    super::new_id(unix)
}

/// Lo mismo con los datos de la aplicación: carpeta, caso abierto y errores del registro de Windows.
fn save_window_in_app(app: &tauri::AppHandle, shared: &Shared, from_ms: u64, to_ms: u64, origin: &Origin) -> Result<Meta, String> {
    let dir = super::dir(app);
    let case_id = crate::cases::open_case_id(app);
    save_window(shared, &dir, case_id, from_ms, to_ms, origin, |from, secs| match super::read_events(from, secs) {
        Ok(v) => (v, None),
        Err(e) => {
            log::warn!("Vigilar: no se pudo leer el registro de eventos: {e}");
            (Vec::new(), Some("No se pudo leer el registro de eventos de Windows: faltan las marcas de errores.".to_string()))
        }
    })
}

fn current_shared() -> Option<Arc<Shared>> {
    lock(&SESSION).as_ref().map(|s| s.shared.clone())
}

/// «¡Acaba de pasar!»: guarda el último minuto (o lo que haya), avisa a la interfaz con `faultrec://done` y devuelve
/// la grabación.
pub fn just_happened(app: &tauri::AppHandle) -> Result<Meta, String> {
    let shared = current_shared().ok_or("Vigilar está apagado.")?;
    let end = lock(&shared.video).end().ok_or("Todavía no hay nada guardado en memoria: espera unos segundos.")?;
    let to_ms = (end.max(0) / 10_000) as u64;
    let from_ms = to_ms.saturating_sub(JUST_HAPPENED_SECS * 1000);
    let result = save_window_in_app(app, &shared, from_ms, to_ms, &Origin { kind: "replay", trigger: None, trigger_key: None });
    let error = result.as_ref().err().cloned();
    let _ = app.emit("faultrec://done", super::DonePayload { id: result.as_ref().ok().map(|m| m.id.clone()), error, replay: true });
    result
}

// ---------- Guardar lo que dispara la grabación automática ----------

fn save_auto(app: tauri::AppHandle, shared: Arc<Shared>, pending: auto::Pending) {
    let _ = std::thread::Builder::new().name("faultrec-auto-save".into()).spawn(move || {
        let first = pending.first;
        let from_ms = first.at_ms.saturating_sub(auto::BEFORE_SECS * 1000);
        let to_ms = first.at_ms + auto::AFTER_SECS * 1000;
        let origin = Origin { kind: "auto", trigger: Some(first.text.clone()), trigger_key: Some(first.key.clone()) };
        match save_window_in_app(&app, &shared, from_ms, to_ms, &origin) {
            Ok(meta) => {
                log::info!("Vigilar: grabación automática guardada ({}): {}", meta.id, first.text);
                let at = now_unix();
                let _ = app.emit("faultrec://auto", AutoPayload { id: meta.id.clone(), trigger: first.text.clone(), at });
                let notice = lock(&NOTIFIER).record(at, &first.short, &local_hhmm(at), &meta.id);
                if let Some(n) = notice {
                    notify(&app, &n.text, Some(n.id));
                }
            }
            Err(e) => log::warn!("Vigilar: no se pudo guardar la grabación automática: {e}"),
        }
    });
}

// ---------- Los hilos ----------

/// El vigilante: mide cada segundo, guarda las medidas y mira los disparadores.
fn watch_main(app: Option<tauri::AppHandle>, shared: Arc<Shared>) {
    let _com = super::watch::ComGuard::new();
    let mut sampler = super::watch::Sampler::new();
    let mut hang = auto::HangTracker::default();
    let mut busy = auto::BusyTracker::default();
    let mut events = auto::EventPoller::default();
    let mut n: u32 = 0;
    sampler.run_loop(shared.started.at, &shared.stop, |s| {
        n += 1;
        let t = s.tick.t;
        {
            let mut q = lock(&shared.ticks);
            q.push_back(s.tick.clone());
            ring::trim_ticks(&mut q, t.saturating_sub(KEEP_AUX_MS / 1000));
        }
        let Some(app) = &app else { return };
        let settings = auto::get(app);
        let at_ms = shared.started.at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        // Apagada (o sin ese disparador), lo que se sabía se olvida: al encenderla no dispara por lo que ya pasaba.
        if !(settings.enabled && settings.hang) {
            hang = auto::HangTracker::default();
        }
        if !(settings.enabled && settings.busy) {
            busy = auto::BusyTracker::default();
        }
        if !(settings.enabled && settings.app_error) {
            events = auto::EventPoller::default();
        }
        let mut triggers: Vec<Trigger> = Vec::new();
        if settings.enabled {
            if settings.hang {
                // Los juegos no: se quedan sin responder al cargar y se cierran con Alt+F4 sin que falle nada.
                let hung: Vec<_> = s.hung.iter().filter(|h| !h.game).cloned().collect();
                for (key, name) in hang.observe(t, &hung) {
                    triggers.push(auto::hang_trigger(&key, &name, at_ms));
                }
            }
            if settings.busy {
                for (key, secs) in busy.observe(s.tick.cpu, s.tick.disk) {
                    triggers.push(auto::busy_trigger(key, secs, at_ms));
                }
            }
            if settings.app_error && n.is_multiple_of(auto::EVENT_POLL_SECS) {
                for ev in events.poll() {
                    if crate::games::is_game(&ev.program, Some(ev.path.as_str()).filter(|p| !p.is_empty())) {
                        log::debug!("Vigilar: {} es un juego; no se graba por su cierre", ev.program);
                        continue;
                    }
                    triggers.push(auto::event_trigger(&ev, at_ms));
                }
            }
        }
        let due = {
            let mut gate = lock(&GATE);
            let (now, day) = (now_unix(), local_day());
            for trig in triggers {
                let (key, text) = (trig.key.clone(), trig.text.clone());
                match gate.offer(trig, &settings.ignored, now, day) {
                    Offer::Started => log::info!("Vigilar: {text}: se guardará en {} s", auto::AFTER_SECS),
                    Offer::Joined => log::info!("Vigilar: {text}: va en la misma grabación"),
                    Offer::Skipped => log::info!("Vigilar: {text}: no se graba (límite de grabaciones automáticas)"),
                    Offer::Ignored => log::debug!("Vigilar: {key} está en «No volver a grabar por esto»"),
                }
            }
            gate.take_due(at_ms)
        };
        if let Some(p) = due {
            save_auto(app.clone(), shared.clone(), p);
        }
        // El resumen de las que se guardaron en los últimos 10 minutos.
        if let Some(notice) = lock(&NOTIFIER).due(now_unix()) {
            notify(app, &notice.text, Some(notice.id));
        }
    });
}

#[cfg(windows)]
fn capture_main(shared: Arc<Shared>, gdi: String, ready: Sender<Result<(), String>>) {
    use windows::Win32::Media::MediaFoundation::{MFShutdown, MFStartup, MFSTARTUP_FULL, MF_VERSION};
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    super::capture::keep_mta_alive();
    // SAFETY: COM y Media Foundation se inician y se cierran en este hilo; todo lo que crea `capture_run` se suelta antes.
    let result = unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED);
        let mf = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        let result = match &mf {
            Ok(()) => capture_run(&shared, &gdi, &ready),
            Err(x) => Err(format!("No se pudo iniciar el codificador de vídeo de Windows: {x}")),
        };
        if mf.is_ok() {
            let _ = MFShutdown();
        }
        if com.is_ok() {
            CoUninitialize();
        }
        result
    };
    if let Err(e) = result {
        log::warn!("Vigilar: la captura se paró: {e}");
        // Si todavía se esperaba a que arrancara, se cuenta ahí; si no, queda para el estado.
        if ready.send(Err(e.clone())).is_err() {
            *lock(&shared.error) = Some(e);
        }
        // Sin captura no hay nada que vigilar: se para todo y se suelta la memoria.
        shared.stop.store(true, Ordering::SeqCst);
        lock(&shared.video).clear();
    }
}

#[cfg(not(windows))]
fn capture_main(_shared: Arc<Shared>, _gdi: String, ready: Sender<Result<(), String>>) {
    let _ = ready.send(Err("Vigilar solo funciona en Windows.".into()));
}

/// La captura: lee el monitor, codifica cada 200 ms y guarda en el búfer.
#[cfg(windows)]
unsafe fn capture_run(shared: &Shared, gdi: &str, ready: &Sender<Result<(), String>>) -> Result<(), String> {
    unsafe {
        let mut grab = super::capture::Grabber::open_monitor(gdi, WIDTH)?;
        let (w, h) = (grab.out_w, grab.out_h);
        let mut enc = mfenc::H264Mft::new(w, h, FPS, BITRATE, GOP)?;
        *lock(&shared.size) = (w, h);
        // Hasta que no llega el primer fotograma no se sabe si funciona: el fallo se cuenta al encender.
        let waiting = Instant::now();
        while !grab.have_image {
            if shared.stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            if grab.poll()? {
                break;
            }
            if waiting.elapsed() > super::capture::FIRST_FRAME_WAIT {
                return Err("Windows no entregó imagen de ese monitor (¿está apagado o protegido?).".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = ready.send(Ok(()));

        let t0 = shared.started.at;
        let tick = Duration::from_millis(1000 / u64::from(FPS));
        let tick_hns = (tick.as_nanos() / 100) as i64;
        let mut next = Instant::now();
        let mut last_hns: i64 = -1;
        let mut last_new = Instant::now();
        let mut last_check = Instant::now();
        let mut out = Vec::new();
        while !shared.stop.load(Ordering::SeqCst) {
            // Si Windows no entrega nada nuevo (pantalla quieta), se repite el último fotograma.
            if grab.poll()? {
                last_new = Instant::now();
            }
            let mut hns = (t0.elapsed().as_nanos() / 100) as i64;
            if hns <= last_hns {
                hns = last_hns + 1;
            }
            last_hns = hns;
            out.clear();
            enc.encode(&grab.image, hns, tick_hns, &mut out)?;
            if lock(&shared.info).is_none() {
                *lock(&shared.info) = enc.stream_info();
            }
            if !out.is_empty() {
                let mut ring = lock(&shared.video);
                for s in out.drain(..) {
                    ring.push(s);
                }
            }
            if last_new.elapsed() >= super::capture::QUIET_CHECK && last_check.elapsed() >= Duration::from_secs(1) {
                last_check = Instant::now();
                if !grab.monitor_alive() {
                    return Err("El monitor se desconectó: Vigilar se apagó.".into());
                }
            }
            next += tick;
            let now = Instant::now();
            if now > next + Duration::from_secs(1) {
                // Se quedó muy atrás (equipo parado): no se acumula retraso.
                next = now;
            }
            while Instant::now() < next && !shared.stop.load(Ordering::SeqCst) {
                std::thread::sleep((next - Instant::now()).min(Duration::from_millis(20)));
            }
        }
        Ok(())
    }
}

// ---------- El atajo Ctrl+Alt+G ----------

/// Registra el atajo en un hilo propio con su bucle de mensajes. `None` si otro programa ya lo usa.
#[cfg(windows)]
fn start_hotkey(app: tauri::AppHandle) -> Option<Hotkey> {
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, PeekMessageW, MSG, PM_NOREMOVE, WM_HOTKEY};
    let (tx, rx) = std::sync::mpsc::channel::<(bool, u32)>();
    let handle = std::thread::Builder::new()
        .name("faultrec-hotkey".into())
        .spawn(move || {
            // SAFETY: punteros a datos de este hilo; el atajo se quita antes de salir.
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                // Crea la cola de mensajes del hilo, para que `WM_QUIT` llegue aunque se mande enseguida.
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                let ok = RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'G')) != 0;
                let _ = tx.send((ok, GetCurrentThreadId()));
                if !ok {
                    return;
                }
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    if msg.message == WM_HOTKEY {
                        on_hotkey(&app);
                    }
                }
                UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
            }
        })
        .ok()?;
    match rx.recv_timeout(Duration::from_secs(3)) {
        Ok((true, thread_id)) => {
            log::info!("Vigilar: {SHORTCUT} listo");
            Some(Hotkey { thread_id, handle })
        }
        _ => {
            log::warn!("No se pudo registrar {SHORTCUT} para guardar el último minuto: lo usa otro programa.");
            let _ = handle.join();
            None
        }
    }
}

#[cfg(not(windows))]
fn start_hotkey(_app: tauri::AppHandle) -> Option<Hotkey> {
    None
}

/// Quita el atajo y espera a que el hilo termine.
fn stop_hotkey(h: Hotkey) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        // SAFETY: solo manda un mensaje al hilo del atajo, que sigue vivo hasta recibirlo.
        unsafe {
            PostThreadMessageW(h.thread_id, WM_QUIT, 0, 0);
        }
    }
    let _ = h.handle.join();
}

/// Se pulsó el atajo: guarda el último minuto sin parar el bucle de mensajes.
#[cfg(windows)]
fn on_hotkey(app: &tauri::AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("faultrec-hotkey-save".into()).spawn(move || match just_happened(&app) {
        Ok(m) => notify(&app, "Guardado el último minuto · ver grabación", Some(m.id)),
        Err(e) => {
            log::warn!("Vigilar: no se pudo guardar el último minuto: {e}");
            notify(&app, &format!("No se pudo guardar el último minuto: {e}"), None);
        }
    });
}

// ---------- Encender y apagar ----------

/// Enciende los hilos de Vigilar sobre el monitor `gdi`. Con `app` (siempre salvo en las pruebas) se registra el
/// atajo y se vigilan los disparadores de la grabación automática.
fn launch(app: Option<tauri::AppHandle>, gdi: String, label: String, monitor_arg: Option<String>, opts: WatchOptions) -> Result<Session, String> {
    // Una automática que esperaba en la sesión anterior (pausada o apagada) no pasa a esta: sus tiempos no valen aquí.
    lock(&GATE).drop_pending();
    let started = Started { at: Instant::now(), unix: now_unix() };
    let stop = Arc::new(AtomicBool::new(false));
    let input = Arc::new(Mutex::new(Recorder::new()));
    let shared = Arc::new(Shared {
        video: Mutex::new(VideoRing::new(KEEP_SECS, MAX_BYTES)),
        info: Mutex::new(None),
        ticks: Mutex::new(VecDeque::new()),
        input: input.clone(),
        input_on: opts.input,
        started,
        monitor: label,
        size: Mutex::new((0, 0)),
        stop: stop.clone(),
        error: Mutex::new(None),
    });
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let (s1, gdi1) = (shared.clone(), gdi.clone());
    let capture_handle = std::thread::Builder::new().name("faultrec-vigil-capture".into()).spawn(move || capture_main(s1, gdi1, tx)).map_err(|e| e.to_string())?;
    let mut session = Session { shared: shared.clone(), capture: Some(capture_handle), watcher: None, input: None, hotkey: None, monitor_arg, opts: opts.clone() };
    match rx.recv_timeout(START_WAIT) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            teardown(session);
            return Err(e);
        }
        Err(_) => {
            teardown(session);
            return Err("Windows tardó demasiado en empezar a vigilar.".into());
        }
    }
    let (s2, app2) = (shared.clone(), app.clone());
    match std::thread::Builder::new().name("faultrec-vigil-watch".into()).spawn(move || watch_main(app2, s2)) {
        Ok(h) => session.watcher = Some(h),
        Err(e) => {
            teardown(session);
            return Err(e.to_string());
        }
    }
    if opts.input {
        let (st, rec) = (stop.clone(), input);
        let watch = super::input::Watch { gdi, window: None, region: None };
        match std::thread::Builder::new().name("faultrec-vigil-input".into()).spawn(move || super::input::run_ring(started, st, watch, rec, KEEP_AUX_MS)) {
            Ok(h) => session.input = Some(h),
            Err(e) => log::warn!("Vigilar: no se pudo empezar a apuntar clics y teclas: {e}"),
        }
    }
    if let Some(app) = app {
        session.hotkey = start_hotkey(app);
    }
    Ok(session)
}

/// Para los hilos, quita el atajo y suelta toda la memoria.
fn teardown(mut s: Session) {
    s.shared.stop.store(true, Ordering::SeqCst);
    for h in [s.capture.take(), s.watcher.take(), s.input.take()].into_iter().flatten() {
        let _ = h.join();
    }
    if let Some(h) = s.hotkey.take() {
        stop_hotkey(h);
    }
    lock(&s.shared.video).clear();
    lock(&s.shared.ticks).clear();
    *lock(&s.shared.input) = Recorder::new();
    *lock(&s.shared.info) = None;
}

/// Enciende Vigilar sobre el monitor que se diga (o el principal).
pub fn start(app: &tauri::AppHandle, monitor: Option<String>, opts: Option<WatchOptions>) -> Result<WatchStatus, String> {
    let opts = opts.unwrap_or_default();
    if super::recording_busy() {
        return Err("Hay una grabación en marcha: Vigilar se enciende solo al terminar.".into());
    }
    let mut guard = lock(&SESSION);
    if let Some(s) = guard.as_ref() {
        if s.monitor_arg == monitor && s.opts == opts && !s.shared.stop.load(Ordering::SeqCst) {
            drop(guard);
            return Ok(status(app));
        }
    }
    // Otro monitor u otras opciones: se vuelve a empezar.
    if let Some(old) = guard.take() {
        teardown(old);
    }
    *lock(&RESUME) = None;
    let chosen = super::choose(monitor.as_deref(), None)?;
    let gdi = chosen.monitor.as_ref().map(|m| m.gdi.clone()).unwrap_or_default();
    let label = super::monitor_label(chosen.shown.as_ref(), chosen.count);
    SHUTTING.store(false, Ordering::SeqCst);
    let session = launch(Some(app.clone()), gdi, label, monitor, opts)?;
    *guard = Some(session);
    drop(guard);
    log::info!("Vigilar encendido");
    Ok(status(app))
}

/// Apaga Vigilar y suelta la memoria.
pub fn stop(app: &tauri::AppHandle) -> WatchStatus {
    *lock(&RESUME) = None;
    let old = lock(&SESSION).take();
    if let Some(s) = old {
        teardown(s);
        log::info!("Vigilar apagado");
    }
    status(app)
}

/// Cómo va Vigilar. Si la captura se paró sola (monitor desconectado), recoge el fallo y lo apaga del todo.
pub fn status(app: &tauri::AppHandle) -> WatchStatus {
    let mut error = None;
    let dead = lock(&SESSION).as_ref().is_some_and(|s| s.shared.stop.load(Ordering::SeqCst));
    if dead {
        let old = lock(&SESSION).take();
        if let Some(s) = old {
            error = lock(&s.shared.error).take().or_else(|| Some("Vigilar se apagó: la captura de pantalla se paró.".to_string()));
            teardown(s);
        }
    }
    let paused = lock(&RESUME).is_some();
    let auto_settings = auto::get(app);
    let guard = lock(&SESSION);
    match guard.as_ref() {
        Some(s) => WatchStatus {
            on: true,
            since: Some(s.shared.started.unix),
            buffered_secs: lock(&s.shared.video).buffered_secs(),
            monitor: s.shared.monitor.clone(),
            auto: auto_settings,
            paused_for_recording: false,
            hotkey: s.hotkey.is_some(),
            skipped_today: lock(&GATE).skipped_today(local_day()),
            error,
        },
        None => WatchStatus { auto: auto_settings, paused_for_recording: paused, error, ..Default::default() },
    }
}

/// Se va a grabar de verdad: si Vigilar estaba encendido, se apaga y se apunta cómo volver a encenderlo.
/// `true` si lo apagó.
pub fn pause_for_recording() -> bool {
    let old = lock(&SESSION).take();
    let Some(s) = old else { return false };
    *lock(&RESUME) = Some((s.monitor_arg.clone(), s.opts.clone()));
    teardown(s);
    log::info!("Vigilar se apaga mientras se graba");
    true
}

/// Terminó la grabación: si Vigilar estaba encendido antes, vuelve a encenderse igual.
pub fn resume_after_recording(app: &tauri::AppHandle) {
    let Some((monitor, opts)) = lock(&RESUME).take() else { return };
    if SHUTTING.load(Ordering::SeqCst) {
        return;
    }
    match start(app, monitor, Some(opts)) {
        Ok(_) => log::info!("Vigilar vuelve a encenderse tras la grabación"),
        Err(e) => {
            log::warn!("Vigilar no pudo volver a encenderse tras la grabación: {e}");
            crate::safestart::activity(crate::faultrec::SAFE_WATCHING, false);
        }
    }
}

/// Al cerrar AdminOps: se apaga y no se guarda nada de lo que había en memoria.
pub fn shutdown() {
    SHUTTING.store(true, Ordering::SeqCst);
    *lock(&RESUME) = None;
    let old = lock(&SESSION).take();
    if let Some(s) = old {
        teardown(s);
    }
}

#[cfg(test)]
mod tests {
    use super::super::ring::HNS;
    use super::super::valid_id;
    use super::*;

    #[test]
    fn el_estado_y_las_opciones_van_en_camel_case() {
        let s = WatchStatus { on: true, since: Some(1_700_000_000), buffered_secs: 42, monitor: "Monitor principal".into(), hotkey: true, skipped_today: 3, ..Default::default() };
        let v = serde_json::to_value(&s).unwrap();
        for key in ["on", "since", "bufferedSecs", "monitor", "auto", "pausedForRecording", "hotkey", "skippedToday", "error"] {
            assert!(v.get(key).is_some(), "falta {key} en {v}");
        }
        assert_eq!(v["bufferedSecs"], 42);
        assert_eq!(v["auto"]["enabled"], false);
        assert_eq!(v["auto"]["appError"], true);
        let none: WatchOptions = serde_json::from_str("{}").unwrap();
        assert!(!none.input);
        assert!(serde_json::from_str::<WatchOptions>(r#"{"input":true}"#).unwrap().input);
        let p = serde_json::to_value(AutoPayload { id: "fr-1".into(), trigger: "Excel no responde".into(), at: 5 }).unwrap();
        assert_eq!(p, serde_json::json!({"id": "fr-1", "trigger": "Excel no responde", "at": 5}));
        assert_eq!(serde_json::to_value(OpenPayload { id: "fr-1".into() }).unwrap(), serde_json::json!({"id": "fr-1"}));
    }

    #[test]
    fn la_hora_local_tiene_dos_cifras() {
        let h = local_hhmm(1_700_000_000);
        assert_eq!(h.len(), 5, "{h}");
        assert_eq!(&h[2..3], ":");
        assert!(local_day() > 700_000);
    }

    fn fake_shared() -> Shared {
        let mut ring = VideoRing::new(KEEP_SECS, MAX_BYTES);
        for i in 0..100i64 {
            ring.push(ring::VSample { data: vec![0, 0, 0, 1, 0x65, 1, 2, 3], time: i * HNS / 5, duration: HNS / 5, key: i % 10 == 0 });
        }
        Shared {
            video: Mutex::new(ring),
            info: Mutex::new(Some(StreamInfo { width: 1280, height: 720, fps: FPS, bitrate: BITRATE, header: vec![0, 0, 0, 1, 0x67, 0x42, 0, 0x1F, 0, 0, 0, 1, 0x68, 0xCE] })),
            ticks: Mutex::new((0..20).map(|t| Tick { t, cpu: 1.0, ..Default::default() }).collect()),
            input: Arc::new(Mutex::new(Recorder::new())),
            input_on: false,
            started: Started { at: Instant::now(), unix: 1_700_000_000 },
            monitor: "Monitor principal".into(),
            size: Mutex::new((1280, 720)),
                    stop: Arc::new(AtomicBool::new(false)),
            error: Mutex::new(None),
        }
    }

    #[test]
    fn sin_nada_en_memoria_no_se_guarda_y_los_id_no_se_pisan() {
        let shared = fake_shared();
        lock(&shared.video).clear();
        let dir = std::env::temp_dir().join(format!("adminops-vigil-vacio-{}", std::process::id()));
        let origin = Origin { kind: "replay", trigger: None, trigger_key: None };
        let r = save_window(&shared, &dir, None, 0, 5000, &origin, |_, _| (Vec::new(), None));
        assert!(r.unwrap_err().contains("Todavía no hay nada"));
        // Dos ids seguidos no coinciden aunque sea el mismo segundo.
        std::fs::create_dir_all(&dir).unwrap();
        let a = unique_id(&dir);
        std::fs::write(super::super::video_file(&dir, &a), b"x").unwrap();
        let b = unique_id(&dir);
        assert_ne!(a, b);
        assert!(valid_id(&a) && valid_id(&b));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Vigila de verdad 8 s, guarda los últimos 5 y comprueba el MP4 con Media Foundation.
    /// `cargo test --lib faultrec -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore = "captura la pantalla de este equipo durante 8 segundos"]
    fn vigila_de_verdad_y_guarda_los_ultimos_cinco_segundos() {
        let session = launch(None, String::new(), "Monitor principal".into(), None, WatchOptions { input: true }).expect("encender Vigilar");
        std::thread::sleep(Duration::from_secs(8));
        let shared = session.shared.clone();
        let (buffered, bytes, frames) = {
            let r = lock(&shared.video);
            (r.buffered_secs(), r.bytes(), r.len())
        };
        println!("VIGILAR: {buffered} s en memoria · {frames} muestras · {bytes} bytes · medidas {}", lock(&shared.ticks).len());
        let end = lock(&shared.video).end().expect("hay vídeo");
        let to_ms = (end / 10_000) as u64;
        let dir = std::env::temp_dir().join(format!("adminops-vigil-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let origin = Origin { kind: "replay", trigger: None, trigger_key: None };
        let meta = save_window(&shared, &dir, Some("caso-prueba".into()), to_ms.saturating_sub(5000), to_ms, &origin, |_, _| (Vec::new(), None)).expect("guardar");
        let video = super::super::video_file(&dir, &meta.id);
        let info = super::super::capture::video_info(&video).expect("leer el MP4");
        let on_disk = std::fs::metadata(&video).unwrap().len();
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(super::super::meta_file(&dir, &meta.id)).unwrap()).unwrap();
        println!("GUARDADO: {} · {}x{} · duración {} ms · {} bytes · secs {} · marcas {} · origin {}", meta.id, info.width, info.height, info.duration_ms, on_disk, meta.secs, meta.marks.len(), json["origin"]);
        let keys = lock(&shared.video).window(0, i64::MAX).unwrap().iter().filter(|s| s.key).count();
        println!("CLAVES: {keys} fotogramas clave en {buffered} s");
        teardown(session);
        // Los tiempos del MP4 empiezan en 0 y el vídeo empieza en un fotograma clave: dura lo pedido más hasta un GOP (2 s).
        assert!((4_000..=7_500).contains(&info.duration_ms), "duración {}", info.duration_ms);
        assert_eq!((info.width, info.height), (meta.width, meta.height));
        assert!(info.width <= WIDTH && info.width.is_multiple_of(2) && info.height.is_multiple_of(2));
        assert!(on_disk > 1000 && !info.has_audio);
        assert_eq!((json["origin"].as_str(), json["caseId"].as_str(), json["mic"].as_bool()), (Some("replay"), Some("caso-prueba"), Some(false)));
        assert!(keys >= 3, "se esperaban claves cada 2 s, hay {keys}");
        assert!(video.exists() && !super::super::part_file(&dir, &meta.id).exists());
        // Al apagar se suelta la memoria.
        assert!(lock(&shared.video).is_empty() && lock(&shared.ticks).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Mide el procesador del proceso con Vigilar encendido (30 s con la pantalla como esté y 30 s con el cursor
    /// moviéndose) y la memoria del búfer lleno (a los 80 s). Tarda 80 s.
    #[cfg(windows)]
    #[test]
    #[ignore = "mide el consumo de Vigilar durante 80 segundos"]
    fn mide_el_consumo_de_vigilar() {
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        use windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos;
        fn cpu_ms() -> u64 {
            let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
            let (mut c, mut e, mut k, mut u) = (z, z, z, z);
            // SAFETY: punteros a variables locales vivas durante la llamada.
            unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
            let t = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32 | u64::from(f.dwLowDateTime)) / 10_000;
            t(k) + t(u)
        }
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as f64;
        let session = launch(None, String::new(), "Monitor principal".into(), None, WatchOptions { input: true }).expect("encender Vigilar");
        let shared = session.shared.clone();
        let report = |label: &str, cpu0: u64, t0: Instant| {
            let (dc, dt) = ((cpu_ms() - cpu0) as f64, t0.elapsed().as_millis() as f64);
            println!("CPU {label}: {:.1} % de un núcleo · {:.2} % del equipo ({cores} núcleos) en {:.0} s", dc / dt * 100.0, dc / dt * 100.0 / cores, dt / 1000.0);
        };
        // Los primeros segundos (arranque y primeros fotogramas) no cuentan.
        std::thread::sleep(Duration::from_secs(5));
        let (c0, t0) = (cpu_ms(), Instant::now());
        std::thread::sleep(Duration::from_secs(30));
        report("pantalla como esté", c0, t0);
        let (c0, t0) = (cpu_ms(), Instant::now());
        let mut i = 0u32;
        while t0.elapsed() < Duration::from_secs(30) {
            // El cursor da vueltas: Windows entrega fotogramas nuevos todo el rato.
            let a = f64::from(i) / 8.0;
            // SAFETY: sin punteros.
            unsafe { SetCursorPos(500 + (a.cos() * 300.0) as i32, 400 + (a.sin() * 200.0) as i32) };
            i += 1;
            std::thread::sleep(Duration::from_millis(16));
        }
        report("cursor moviéndose", c0, t0);
        std::thread::sleep(Duration::from_secs(15));
        let (buffered, bytes, frames) = {
            let r = lock(&shared.video);
            (r.buffered_secs(), r.bytes(), r.len())
        };
        println!("BUFER LLENO: {buffered} s · {frames} muestras · {:.1} MB de vídeo (tope {} MB) · medidas {} · clics/teclas {}", bytes as f64 / 1_048_576.0, MAX_BYTES / 1_048_576, lock(&shared.ticks).len(), lock(&shared.input).snapshot().len());
        let working_set = {
            let mut sys = sysinfo::System::new();
            let pid = sysinfo::Pid::from_u32(std::process::id());
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            sys.process(pid).map_or(0, |p| p.memory())
        };
        println!("MEMORIA del proceso de la prueba: {:.0} MB", working_set as f64 / 1_048_576.0);
        teardown(session);
        assert!(buffered >= 75, "{buffered} s");
        assert!(bytes <= MAX_BYTES + 2 * 1_048_576);
    }
}
