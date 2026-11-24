//! La grabación automática de Vigilar: qué la dispara, cuándo no se graba y cómo se avisa.
//!
//! Todo lo que decide es lógica pura con pruebas (los instantes los pone quien llama): los tres disparadores
//! (un programa que deja de responder, un error de aplicación del registro de Windows, el procesador o el disco al
//! tope), las listas de «No volver a grabar por esto», los límites (2 por programa y hora, 10 por día) y el aviso
//! agrupado de Windows (uno cada 10 minutos como mucho). Lo único que toca Windows es leer el registro de eventos,
//! y está al final, aparte.
//!
//! Los ajustes se guardan en `grabacion-auto.json`, en los datos de este equipo. **Apagada de serie.**

use super::watch::HungApp;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

/// Cuánto antes del disparo empieza lo que se guarda.
pub const BEFORE_SECS: u64 = 30;
/// Cuánto se espera después del disparo antes de guardar.
pub const AFTER_SECS: u64 = 15;
/// Segundos seguidos sin responder para que cuente como «se colgó».
pub const HANG_CONFIRM_SECS: u32 = 3;
/// Procesador o disco a partir del cual empieza a contar «al tope».
pub const BUSY_ON_PCT: f32 = 95.0;
/// Por debajo de esto se considera que ya bajó.
pub const BUSY_OFF_PCT: f32 = 80.0;
/// «Al tope» es más de estos segundos seguidos.
pub const BUSY_SECS: u32 = 20;
/// Segundos por debajo de `BUSY_OFF_PCT` antes de poder volver a disparar.
pub const BUSY_REARM_SECS: u32 = 30;
/// Como mucho tantas grabaciones automáticas por programa (o por «procesador», «disco») y hora.
pub const MAX_PER_KEY_HOUR: usize = 2;
/// Como mucho tantas al día.
pub const MAX_PER_DAY: u32 = 10;
/// Un solo aviso de Windows cada tanto.
pub const NOTICE_SECS: u64 = 600;
/// Cada cuánto se mira el registro de eventos de Windows (segundos).
pub const EVENT_POLL_SECS: u32 = 10;
/// Lo más que se aguanta en «No volver a grabar por esto».
const MAX_IGNORED: usize = 200;

// ---------- Ajustes ----------

/// Algo que no se vuelve a grabar: `key` es `app:<exe en minúsculas>`, `cpu` o `disk`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Ignored {
    pub key: String,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoSettings {
    /// Apagada de serie.
    pub enabled: bool,
    /// Un programa deja de responder.
    pub hang: bool,
    /// Error o cuelgue de aplicación en el registro de eventos.
    pub app_error: bool,
    /// Procesador o disco al tope.
    pub busy: bool,
    pub ignored: Vec<Ignored>,
}

impl Default for AutoSettings {
    fn default() -> Self {
        AutoSettings { enabled: false, hang: true, app_error: true, busy: true, ignored: Vec::new() }
    }
}

/// ¿Es una clave de las que se generan aquí?
pub fn valid_key(key: &str) -> bool {
    key == "cpu" || key == "disk" || key.strip_prefix("app:").is_some_and(|exe| !exe.is_empty() && exe.chars().count() <= 80 && !exe.contains(['\\', '/', '<', '>', '"']))
}

/// Deja los ajustes en forma: claves válidas y en minúsculas, sin repetir, etiquetas cortas y un máximo de entradas.
pub fn normalize(mut s: AutoSettings) -> AutoSettings {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<Ignored> = Vec::new();
    for i in s.ignored.drain(..) {
        let key = i.key.trim().to_lowercase();
        if !valid_key(&key) || !seen.insert(key.clone()) || out.len() >= MAX_IGNORED {
            continue;
        }
        let label: String = i.label.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect();
        out.push(Ignored { label: if label.is_empty() { key.clone() } else { label }, key });
    }
    s.ignored = out;
    s
}

static CACHE: Mutex<Option<AutoSettings>> = Mutex::new(None);

fn path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("grabacion-auto.json")
}

/// Los ajustes de ahora (se leen del disco la primera vez).
pub fn get(app: &tauri::AppHandle) -> AutoSettings {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(|| normalize(crate::paths::read_json::<AutoSettings>(&path(app)))).clone()
}

/// Guarda los ajustes (ya normalizados) y devuelve lo que quedó.
pub fn set(app: &tauri::AppHandle, s: AutoSettings) -> Result<AutoSettings, String> {
    let s = normalize(s);
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    crate::paths::write_json(&path(app), &s)?;
    *g = Some(s.clone());
    Ok(s)
}

// ---------- Disparos ----------

/// Algo que pasó y que puede dar una grabación.
#[derive(Clone, Debug, PartialEq)]
pub struct Trigger {
    /// `app:<exe>`, `cpu` o `disk`.
    pub key: String,
    /// Lo que se enseña en la grabación: «Excel no responde».
    pub text: String,
    /// Lo que se dice en el aviso de Windows: «Excel se colgó».
    pub short: String,
    /// Milisegundos desde que empezó Vigilar.
    pub at_ms: u64,
}

/// «excel.exe» → «Excel». Del registro de eventos solo llega el nombre del archivo.
pub fn program_label(exe: &str) -> String {
    let stem = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    let stem = stem.strip_suffix(".exe").or_else(|| stem.strip_suffix(".EXE")).unwrap_or(stem).to_lowercase();
    let mut c = stem.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Programa".to_string(),
    }
}

pub fn hang_trigger(key: &str, name: &str, at_ms: u64) -> Trigger {
    // La clave del vigilante es el .exe a secas; la de «No volver a grabar por esto» y la de los errores del registro
    // llevan «app:». Sin unirlas, ignorar un cuelgue daba error y el mismo programa contaba dos veces en el límite.
    let key = if key.starts_with("app:") { key.to_string() } else { format!("app:{}", key.to_lowercase()) };
    Trigger { key, text: format!("{name} no responde"), short: format!("{name} se colgó"), at_ms }
}

pub fn busy_trigger(key: &str, secs: u32, at_ms: u64) -> Trigger {
    let (what, short) = if key == "disk" { ("Disco", "El disco estuvo al 100 %") } else { ("Procesador", "El procesador estuvo al 100 %") };
    Trigger { key: key.to_string(), text: format!("{what} al 100 % · {secs} s"), short: short.to_string(), at_ms }
}

/// De un evento 1000 (error de aplicación) o 1002 (se colgó) del registro.
pub fn event_trigger(ev: &AppEvent, at_ms: u64) -> Trigger {
    let name = program_label(&ev.program);
    let key = format!("app:{}", ev.program.to_lowercase());
    if ev.event_id == 1002 {
        hang_trigger(&key, &name, at_ms)
    } else {
        Trigger { key, text: format!("Error de aplicación · {name}"), short: format!("{name} falló"), at_ms }
    }
}

/// Un programa que pasa a no responder y sigue así `HANG_CONFIRM_SECS`. Los que ya estaban colgados al empezar no cuentan.
#[derive(Default)]
pub struct HangTracker {
    since: HashMap<String, u32>,
    fired: HashSet<String>,
    primed: bool,
}

impl HangTracker {
    /// Lo que se ve en el segundo `t`. Devuelve `(clave, nombre)` de lo que acaba de confirmarse.
    pub fn observe(&mut self, t: u32, hung: &[HungApp]) -> Vec<(String, String)> {
        let now: HashSet<&str> = hung.iter().map(|h| h.key.as_str()).collect();
        self.since.retain(|k, _| now.contains(k.as_str()));
        self.fired.retain(|k| now.contains(k.as_str()));
        let mut out = Vec::new();
        for h in hung {
            if !self.primed {
                self.fired.insert(h.key.clone());
                self.since.insert(h.key.clone(), t);
                continue;
            }
            let since = *self.since.entry(h.key.clone()).or_insert(t);
            if t.saturating_sub(since) >= HANG_CONFIRM_SECS && self.fired.insert(h.key.clone()) {
                out.push((h.key.clone(), h.name.clone()));
            }
        }
        self.primed = true;
        out
    }
}

/// Una medida (procesador o disco) que sube y se queda arriba.
#[derive(Default)]
struct Level {
    run: u32,
    latched: bool,
    low: u32,
}

impl Level {
    /// Un segundo más. Devuelve los segundos seguidos al tope la vez que se supera `BUSY_SECS`.
    fn step(&mut self, v: Option<f32>) -> Option<u32> {
        if self.latched {
            // Ya disparó: hasta que no baje de 80 % durante 30 s no vuelve a hacerlo.
            if let Some(x) = v {
                if x < BUSY_OFF_PCT {
                    self.low += 1;
                    if self.low >= BUSY_REARM_SECS {
                        *self = Level::default();
                    }
                } else {
                    self.low = 0;
                }
            }
            return None;
        }
        match v {
            Some(x) if x >= BUSY_ON_PCT => {
                self.run += 1;
                if self.run > BUSY_SECS {
                    self.latched = true;
                    self.low = 0;
                    return Some(self.run);
                }
            }
            _ => self.run = 0,
        }
        None
    }
}

/// Procesador y disco al tope.
#[derive(Default)]
pub struct BusyTracker {
    cpu: Level,
    disk: Level,
}

impl BusyTracker {
    /// Lo medido en un segundo. Devuelve `("cpu" | "disk", segundos seguidos)` de lo que acaba de dispararse.
    pub fn observe(&mut self, cpu: f32, disk: Option<f32>) -> Vec<(&'static str, u32)> {
        let mut out = Vec::new();
        if let Some(n) = self.cpu.step(Some(cpu)) {
            out.push(("cpu", n));
        }
        if let Some(n) = self.disk.step(disk) {
            out.push(("disk", n));
        }
        out
    }
}

// ---------- Decidir si se graba ----------

/// Lo que pasa con un disparo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// Está en «No volver a grabar por esto».
    Ignored,
    /// Pasa de los límites: se cuenta pero no se graba.
    Skipped,
    /// Empieza una grabación (se guardará dentro de `AFTER_SECS`).
    Started,
    /// Ya hay una grabación esperando: este va en la misma.
    Joined,
}

/// Una grabación automática que espera sus segundos de después.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub first: Trigger,
    pub joined: Vec<Trigger>,
}

#[derive(Default)]
pub struct Gate {
    /// Grabaciones de la última hora: (clave, instante Unix).
    history: Vec<(String, u64)>,
    day: u32,
    today: u32,
    skipped: u32,
    pending: Option<Pending>,
}

impl Gate {
    /// Tira la grabación que esperaba sus 15 s: sus tiempos son de la sesión de Vigilar que acaba de terminar y en la
    /// siguiente no significan nada. Los límites del día (`history`, `today`, `skipped`) se quedan.
    pub fn drop_pending(&mut self) {
        // No llegó a guardarse: tampoco cuenta para los límites (ni la de hoy ni las de esa hora por programa).
        if let Some(p) = self.pending.take() {
            self.today = self.today.saturating_sub(1);
            for key in std::iter::once(&p.first.key).chain(p.joined.iter().map(|j| &j.key)) {
                if let Some(i) = self.history.iter().rposition(|(k, _)| k == key) {
                    self.history.remove(i);
                }
            }
        }
    }

    /// `now`: segundos Unix; `day`: un número que cambia a medianoche (hora local).
    pub fn offer(&mut self, t: Trigger, ignored: &[Ignored], now: u64, day: u32) -> Offer {
        self.new_day(day);
        if ignored.iter().any(|i| i.key == t.key) {
            return Offer::Ignored;
        }
        self.history.retain(|(_, at)| now.saturating_sub(*at) < 3600);
        if let Some(p) = &mut self.pending {
            self.history.push((t.key.clone(), now));
            p.joined.push(t);
            return Offer::Joined;
        }
        if self.history.iter().filter(|(k, _)| *k == t.key).count() >= MAX_PER_KEY_HOUR || self.today >= MAX_PER_DAY {
            self.skipped += 1;
            return Offer::Skipped;
        }
        self.history.push((t.key.clone(), now));
        self.today += 1;
        self.pending = Some(Pending { first: t, joined: Vec::new() });
        Offer::Started
    }

    /// La grabación que ya esperó sus `AFTER_SECS` (si `now_ms` es el instante de ahora en milisegundos de Vigilar).
    pub fn take_due(&mut self, now_ms: u64) -> Option<Pending> {
        if self.pending.as_ref().is_some_and(|p| now_ms >= p.first.at_ms + AFTER_SECS * 1000) {
            self.pending.take()
        } else {
            None
        }
    }

    #[cfg(test)]
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Las que pasaron de los límites hoy.
    pub fn skipped_today(&mut self, day: u32) -> u32 {
        self.new_day(day);
        self.skipped
    }

    fn new_day(&mut self, day: u32) {
        if self.day != day {
            self.day = day;
            self.today = 0;
            self.skipped = 0;
        }
    }
}

// ---------- El aviso de Windows ----------

/// El texto del aviso: la grabación suelta («Excel se colgó a las 10:42 · ver grabación») o el resumen.
pub fn notice_text(count: u32, short: &str, hhmm: &str) -> String {
    if count <= 1 {
        format!("{short} a las {hhmm} · ver grabación")
    } else {
        format!("{count} grabaciones automáticas desde las {hhmm}")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub text: String,
    /// La grabación a la que lleva «ver grabación» (la última).
    pub id: String,
}

struct Window {
    start: u64,
    count: u32,
    short: String,
    hhmm: String,
    last_id: String,
    sent_first: bool,
}

/// Un solo aviso cada `NOTICE_SECS` como mucho: la primera grabación avisa al momento y las demás de esos 10 minutos
/// se resumen en un único aviso al vencer el plazo.
#[derive(Default)]
pub struct Notifier {
    window: Option<Window>,
    last_sent: Option<u64>,
}

impl Notifier {
    /// Se acaba de guardar una grabación automática (`now`: segundos Unix; `hhmm`: la hora local).
    pub fn record(&mut self, now: u64, short: &str, hhmm: &str, id: &str) -> Option<Notice> {
        let due = self.due(now);
        if let Some(w) = &mut self.window {
            w.count += 1;
            w.last_id = id.to_string();
            return due;
        }
        let quiet = self.last_sent.is_some_and(|ls| now < ls + NOTICE_SECS);
        let mut w = Window { start: now, count: 1, short: short.to_string(), hhmm: hhmm.to_string(), last_id: id.to_string(), sent_first: false };
        let out = if quiet {
            None
        } else {
            w.sent_first = true;
            self.last_sent = Some(now);
            Some(Notice { text: notice_text(1, short, hhmm), id: id.to_string() })
        };
        self.window = Some(w);
        due.or(out)
    }

    /// Vence el plazo de 10 minutos: si hubo más de una (o la primera no pudo avisar), este es su único aviso.
    pub fn due(&mut self, now: u64) -> Option<Notice> {
        if self.window.as_ref().is_none_or(|w| now < w.start + NOTICE_SECS) {
            return None;
        }
        let w = self.window.take()?;
        if w.count > 1 || !w.sent_first {
            self.last_sent = Some(now);
            Some(Notice { text: notice_text(w.count, &w.short, &w.hhmm), id: w.last_id })
        } else {
            None
        }
    }
}

// ---------- Eventos del registro de Windows ----------

/// Un evento 1000 (Application Error) o 1002 (Application Hang).
#[derive(Clone, Debug, PartialEq)]
pub struct AppEvent {
    pub record_id: u64,
    pub event_id: u32,
    /// El primer dato del evento: el archivo del programa («EXCEL.EXE»).
    pub program: String,
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(&s[a..b])
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Lee lo que interesa de un evento en XML: el número de registro, el id y el primer `Data`.
pub fn parse_event_xml(xml: &str) -> Option<AppEvent> {
    let record_id: u64 = between(xml, "<EventRecordID>", "</EventRecordID>")?.trim().parse().ok()?;
    // `<EventID Qualifiers='0'>1000</EventID>` o `<EventID>1000</EventID>`.
    let id_at = xml.find("<EventID")?;
    let rest = &xml[id_at..];
    let event_id: u32 = between(rest, ">", "</EventID>")?.trim().parse().ok()?;
    let data = &xml[xml.find("<EventData")?..];
    let d = data.find("<Data")?;
    let after = &data[d..];
    let program = unescape(between(after, ">", "</Data>")?.trim());
    let program: String = program.rsplit(['\\', '/']).next().unwrap_or(&program).trim().to_string();
    (!program.is_empty()).then_some(AppEvent { record_id, event_id, program })
}

/// Pregunta al registro Application cada vez que se le pide.
#[derive(Default)]
pub struct EventPoller {
    /// El último número de registro visto (cualquier evento del canal al empezar, después los de aquí).
    last: Option<u64>,
}

impl EventPoller {
    /// Los eventos 1000 y 1002 nuevos desde la última vez, de más viejo a más nuevo. La primera vez solo anota dónde
    /// va el registro: lo que ya había antes de empezar a vigilar no dispara nada.
    pub fn poll(&mut self) -> Vec<AppEvent> {
        match self.last {
            None => {
                self.last = windows_log::newest_record_id();
                Vec::new()
            }
            Some(last) => {
                let mut v = windows_log::new_app_events(last);
                v.sort_by_key(|e| e.record_id);
                if let Some(m) = v.last() {
                    self.last = Some(m.record_id);
                }
                v
            }
        }
    }
}

#[cfg(windows)]
mod windows_log {
    use super::{parse_event_xml, AppEvent};
    use windows_sys::Win32::System::EventLog::{EvtClose, EvtNext, EvtQuery, EvtRender, EvtQueryChannelPath, EvtQueryReverseDirection, EvtRenderEventXml};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// El XML de un evento, o `None` si no se pudo leer.
    fn render(event: isize) -> Option<String> {
        let mut used = 0u32;
        let mut props = 0u32;
        // SAFETY: la primera llamada, sin búfer, solo dice cuánto hace falta; la segunda escribe en `buf`, que tiene ese tamaño.
        unsafe {
            EvtRender(0, event, EvtRenderEventXml, 0, std::ptr::null_mut(), &mut used, &mut props);
            if used == 0 || used > 1 << 20 {
                return None;
            }
            let mut buf = vec![0u16; used as usize / 2 + 1];
            if EvtRender(0, event, EvtRenderEventXml, (buf.len() * 2) as u32, buf.as_mut_ptr().cast(), &mut used, &mut props) == 0 {
                return None;
            }
            let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..n]))
        }
    }

    /// Lee hasta `max` eventos del canal Application con ese filtro, de más nuevo a más viejo.
    fn query(filter: &str, max: usize) -> Vec<String> {
        let (path, q) = (wide("Application"), wide(filter));
        let mut out = Vec::new();
        // SAFETY: los punteros son de búferes vivos durante la llamada y cada manejador se cierra una vez.
        unsafe {
            let h = EvtQuery(0, path.as_ptr(), q.as_ptr(), EvtQueryChannelPath | EvtQueryReverseDirection);
            if h == 0 {
                return out;
            }
            let mut events = [0isize; 16];
            'all: loop {
                let mut n = 0u32;
                if EvtNext(h, events.len() as u32, events.as_mut_ptr(), 1000, 0, &mut n) == 0 || n == 0 {
                    break;
                }
                for &ev in &events[..n as usize] {
                    if out.len() < max {
                        if let Some(xml) = render(ev) {
                            out.push(xml);
                        }
                    }
                    EvtClose(ev);
                }
                if out.len() >= max {
                    break 'all;
                }
            }
            EvtClose(h);
        }
        out
    }

    /// El número del último evento del canal, sea cual sea.
    pub fn newest_record_id() -> Option<u64> {
        let xml = query("*", 1).into_iter().next()?;
        super::between(&xml, "<EventRecordID>", "</EventRecordID>")?.trim().parse().ok()
    }

    pub fn new_app_events(after: u64) -> Vec<AppEvent> {
        let filter = format!("*[System[(EventID=1000 or EventID=1002) and (Provider[@Name='Application Error'] or Provider[@Name='Application Hang']) and EventRecordID>{after}]]");
        query(&filter, 50).iter().filter_map(|x| parse_event_xml(x)).collect()
    }
}

#[cfg(not(windows))]
mod windows_log {
    use super::AppEvent;
    pub fn newest_record_id() -> Option<u64> {
        None
    }
    pub fn new_app_events(_after: u64) -> Vec<AppEvent> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lo_pendiente_no_pasa_a_la_siguiente_vigilancia() {
        let mut g = Gate::default();
        let t = hang_trigger("excel.exe", "Excel", 3_600_000);
        assert!(matches!(g.offer(t.clone(), &[], 1_000, 1), Offer::Started));
        g.drop_pending();
        // En la sesión nueva, el siguiente disparo empieza su propia grabación (no se une a la vieja).
        let t2 = hang_trigger("outlook.exe", "Outlook", 2_000);
        assert!(matches!(g.offer(t2, &[], 1_010, 1), Offer::Started));
    }

    #[test]
    fn un_cuelgue_ignorado_no_se_graba_y_cuenta_como_su_error() {
        let t = hang_trigger("EXCEL.EXE", "Excel", 1_000);
        assert_eq!(t.key, "app:excel.exe");
        // Ya con el prefijo, no se duplica.
        assert_eq!(hang_trigger("app:excel.exe", "Excel", 1_000).key, "app:excel.exe");
        // Misma clave que la que «No volver a grabar por esto» guarda desde un error de aplicación.
        let ev = AppEvent { record_id: 1, event_id: 1002, program: "EXCEL.EXE".into() };
        assert_eq!(event_trigger(&ev, 2_000).key, t.key);
    }

    fn hung(key: &str, name: &str) -> HungApp {
        HungApp { key: key.into(), name: name.into() }
    }

    fn trig(key: &str, at_ms: u64) -> Trigger {
        Trigger { key: key.into(), text: format!("{key} falla"), short: format!("{key} falló"), at_ms }
    }

    #[test]
    fn de_serie_esta_apagada_y_con_todo_lo_demas_encendido() {
        let s = AutoSettings::default();
        assert!(!s.enabled && s.hang && s.app_error && s.busy && s.ignored.is_empty());
        // Un archivo vacío o de otra versión se lee con lo mismo.
        let from_empty: AutoSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(from_empty, s);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v, serde_json::json!({"enabled": false, "hang": true, "appError": true, "busy": true, "ignored": []}));
        let one: AutoSettings = serde_json::from_str(r#"{"enabled":true,"ignored":[{"key":"app:excel.exe","label":"Excel"}]}"#).unwrap();
        assert!(one.enabled && one.hang && one.app_error && one.busy);
        assert_eq!(one.ignored, vec![Ignored { key: "app:excel.exe".into(), label: "Excel".into() }]);
    }

    #[test]
    fn normalizar_deja_claves_validas_sin_repetir() {
        let s = normalize(AutoSettings {
            ignored: vec![
                Ignored { key: " APP:Excel.EXE ".into(), label: "  Microsoft   Excel ".into() },
                Ignored { key: "app:excel.exe".into(), label: "otra".into() },
                Ignored { key: "cpu".into(), label: String::new() },
                Ignored { key: "../x".into(), label: "mala".into() },
                Ignored { key: "app:".into(), label: "vacía".into() },
                Ignored { key: "app:a\\b".into(), label: "ruta".into() },
            ],
            ..Default::default()
        });
        assert_eq!(s.ignored, vec![Ignored { key: "app:excel.exe".into(), label: "Microsoft Excel".into() }, Ignored { key: "cpu".into(), label: "cpu".into() }]);
        let many = normalize(AutoSettings { ignored: (0..500).map(|i| Ignored { key: format!("app:p{i}.exe"), label: "x".into() }).collect(), ..Default::default() });
        assert_eq!(many.ignored.len(), MAX_IGNORED);
    }

    #[test]
    fn nombres_de_programa_del_registro() {
        assert_eq!(program_label("EXCEL.EXE"), "Excel");
        assert_eq!(program_label("C:\\Program Files\\Outlook\\OUTLOOK.EXE"), "Outlook");
        assert_eq!(program_label(""), "Programa");
        let ev = AppEvent { record_id: 1, event_id: 1000, program: "EXCEL.EXE".into() };
        let t = event_trigger(&ev, 5000);
        assert_eq!((t.key.as_str(), t.text.as_str(), t.short.as_str()), ("app:excel.exe", "Error de aplicación · Excel", "Excel falló"));
        let hang = event_trigger(&AppEvent { event_id: 1002, ..ev }, 5000);
        assert_eq!((hang.text.as_str(), hang.short.as_str()), ("Excel no responde", "Excel se colgó"));
        assert_eq!(busy_trigger("cpu", 38, 0).text, "Procesador al 100 % · 38 s");
        assert_eq!(busy_trigger("disk", 21, 0).text, "Disco al 100 % · 21 s");
    }

    #[test]
    fn un_programa_colgado_dispara_a_los_tres_segundos_y_una_sola_vez() {
        let mut h = HangTracker::default();
        let excel = [hung("excel.exe", "Excel")];
        // Ya colgado al empezar: no cuenta.
        assert!(h.observe(0, &excel).is_empty());
        for t in 1..20 {
            assert!(h.observe(t, &excel).is_empty(), "t={t}");
        }
        // Se recupera y vuelve a colgarse: ahora sí, a los 3 s.
        assert!(h.observe(20, &[]).is_empty());
        assert!(h.observe(21, &excel).is_empty());
        assert!(h.observe(22, &excel).is_empty());
        assert!(h.observe(23, &excel).is_empty());
        assert_eq!(h.observe(24, &excel), vec![("excel.exe".to_string(), "Excel".to_string())]);
        assert!(h.observe(25, &excel).is_empty());
        assert!(h.observe(40, &excel).is_empty());
    }

    #[test]
    fn un_cuelgue_de_menos_de_tres_segundos_no_dispara() {
        let mut h = HangTracker::default();
        let word = [hung("winword.exe", "Word")];
        assert!(h.observe(0, &[]).is_empty());
        assert!(h.observe(1, &word).is_empty());
        assert!(h.observe(2, &word).is_empty());
        assert!(h.observe(3, &[]).is_empty());
        // Otra vez desde cero.
        assert!(h.observe(4, &word).is_empty());
        assert!(h.observe(5, &word).is_empty());
        assert!(h.observe(6, &word).is_empty());
        assert_eq!(h.observe(7, &word).len(), 1);
    }

    #[test]
    fn dos_programas_colgados_disparan_cada_uno_por_su_cuenta() {
        let mut h = HangTracker::default();
        assert!(h.observe(0, &[]).is_empty());
        assert!(h.observe(1, &[hung("a.exe", "A")]).is_empty());
        assert!(h.observe(2, &[hung("a.exe", "A"), hung("b.exe", "B")]).is_empty());
        assert!(h.observe(3, &[hung("a.exe", "A"), hung("b.exe", "B")]).is_empty());
        assert_eq!(h.observe(4, &[hung("a.exe", "A"), hung("b.exe", "B")]), vec![("a.exe".to_string(), "A".to_string())]);
        // B se colgó un segundo después: dispara un segundo después.
        assert_eq!(h.observe(5, &[hung("a.exe", "A"), hung("b.exe", "B")]), vec![("b.exe".to_string(), "B".to_string())]);
        assert!(h.observe(6, &[hung("a.exe", "A"), hung("b.exe", "B")]).is_empty());
    }

    #[test]
    fn el_tope_dispara_pasados_20_segundos_y_no_repite_hasta_bajar() {
        let mut b = BusyTracker::default();
        // 20 s seguidos al 96 %: todavía no (hace falta MÁS de 20).
        for _ in 0..20 {
            assert!(b.observe(96.0, None).is_empty());
        }
        assert_eq!(b.observe(96.0, None), vec![("cpu", 21)]);
        // Sigue arriba: no repite.
        for _ in 0..60 {
            assert!(b.observe(99.0, None).is_empty());
        }
        // Baja a 85 % (no baja de 80): sigue sin poder.
        for _ in 0..40 {
            assert!(b.observe(85.0, None).is_empty());
        }
        for _ in 0..25 {
            assert!(b.observe(99.0, None).is_empty());
        }
        // Baja de 80 % durante 29 s y sube otra vez: sigue sin poder; con 30 s sí.
        for _ in 0..29 {
            b.observe(50.0, None);
        }
        for _ in 0..30 {
            assert!(b.observe(99.0, None).is_empty());
        }
        for _ in 0..30 {
            b.observe(50.0, None);
        }
        for _ in 0..20 {
            assert!(b.observe(99.0, None).is_empty());
        }
        assert_eq!(b.observe(99.0, None), vec![("cpu", 21)]);
    }

    #[test]
    fn una_bajada_corta_reinicia_la_cuenta_y_el_disco_va_aparte() {
        let mut b = BusyTracker::default();
        for _ in 0..15 {
            b.observe(97.0, Some(10.0));
        }
        // Un respiro de un segundo reinicia: hacen falta otros 21.
        b.observe(40.0, Some(10.0));
        for _ in 0..20 {
            assert!(b.observe(97.0, Some(10.0)).is_empty());
        }
        assert_eq!(b.observe(97.0, Some(10.0)), vec![("cpu", 21)]);
        // El disco: lo que no se mide (None) corta la cuenta.
        let mut d = BusyTracker::default();
        for _ in 0..15 {
            d.observe(10.0, Some(99.0));
        }
        d.observe(10.0, None);
        for _ in 0..20 {
            assert!(d.observe(10.0, Some(99.0)).is_empty());
        }
        assert_eq!(d.observe(10.0, Some(99.0)), vec![("disk", 21)]);
    }

    #[test]
    fn lo_ignorado_no_se_graba_ni_se_cuenta() {
        let mut g = Gate::default();
        let ign = vec![Ignored { key: "app:excel.exe".into(), label: "Excel".into() }];
        assert_eq!(g.offer(trig("app:excel.exe", 0), &ign, 1000, 1), Offer::Ignored);
        assert!(!g.has_pending());
        assert_eq!(g.skipped_today(1), 0);
        assert_eq!(g.offer(trig("app:word.exe", 0), &ign, 1000, 1), Offer::Started);
    }

    #[test]
    fn dos_por_programa_y_hora_y_diez_por_dia() {
        let mut g = Gate::default();
        let mut t = 1000u64;
        // Dos de Excel se graban; la tercera de la misma hora no.
        for i in 0..2 {
            assert_eq!(g.offer(trig("app:excel.exe", t * 1000), &[], t, 1), Offer::Started);
            assert!(g.take_due(t * 1000 + 15_000).is_some(), "{i}");
            t += 100;
        }
        assert_eq!(g.offer(trig("app:excel.exe", t * 1000), &[], t, 1), Offer::Skipped);
        assert_eq!(g.skipped_today(1), 1);
        // Otro programa sí; y Excel otra vez pasada la hora de la primera.
        assert_eq!(g.offer(trig("app:word.exe", t * 1000), &[], t, 1), Offer::Started);
        assert!(g.take_due(t * 1000 + 15_000).is_some());
        assert_eq!(g.offer(trig("app:excel.exe", (1000 + 3600) * 1000), &[], 1000 + 3600, 1), Offer::Started);
        assert!(g.take_due((1000 + 3600) * 1000 + 15_000).is_some());
        // Ya van 4 hoy: hasta 10 con claves distintas.
        let mut n = 4;
        for i in 0..20 {
            let now = 10_000 + i * 4000;
            match g.offer(trig(&format!("app:p{i}.exe"), now * 1000), &[], now, 1) {
                Offer::Started => {
                    n += 1;
                    assert!(g.take_due(now * 1000 + 15_000).is_some());
                }
                Offer::Skipped => {}
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(n, MAX_PER_DAY);
        assert_eq!(g.skipped_today(1), 1 + (20 - 6));
        // Al día siguiente se vuelve a empezar, también la cuenta de las que se saltaron.
        assert_eq!(g.skipped_today(2), 0);
        assert_eq!(g.offer(trig("app:p0.exe", 0), &[], 200_000, 2), Offer::Started);
    }

    #[test]
    fn un_disparo_que_llega_esperando_va_en_la_misma_grabacion() {
        let mut g = Gate::default();
        assert_eq!(g.offer(trig("app:excel.exe", 100_000), &[], 500, 1), Offer::Started);
        assert!(g.take_due(110_000).is_none());
        assert_eq!(g.offer(trig("cpu", 105_000), &[], 505, 1), Offer::Joined);
        assert_eq!(g.offer(trig("app:excel.exe", 108_000), &[], 508, 1), Offer::Joined);
        assert!(g.take_due(114_999).is_none());
        let p = g.take_due(115_000).unwrap();
        assert_eq!(p.first.key, "app:excel.exe");
        assert_eq!(p.joined.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), vec!["cpu", "app:excel.exe"]);
        assert!(!g.has_pending());
        assert!(g.take_due(200_000).is_none());
    }

    #[test]
    fn los_textos_del_aviso() {
        assert_eq!(notice_text(1, "Excel se colgó", "10:42"), "Excel se colgó a las 10:42 · ver grabación");
        assert_eq!(notice_text(3, "Excel se colgó", "10:42"), "3 grabaciones automáticas desde las 10:42");
        assert_eq!(notice_text(0, "x", "09:05"), "x a las 09:05 · ver grabación");
    }

    #[test]
    fn la_primera_avisa_y_las_demas_se_juntan_en_un_resumen() {
        let mut n = Notifier::default();
        let t0 = 10_000u64;
        let first = n.record(t0, "Excel se colgó", "10:42", "fr-1").unwrap();
        assert_eq!(first, Notice { text: "Excel se colgó a las 10:42 · ver grabación".into(), id: "fr-1".into() });
        // Dos más dentro de los 10 minutos: nada ahora.
        assert!(n.record(t0 + 120, "Word se colgó", "10:44", "fr-2").is_none());
        assert!(n.record(t0 + 400, "Outlook falló", "10:48", "fr-3").is_none());
        assert!(n.due(t0 + 599).is_none());
        // Al vencer, el resumen con las tres (la primera incluida) y llevando a la última.
        let sum = n.due(t0 + 600).unwrap();
        assert_eq!(sum, Notice { text: "3 grabaciones automáticas desde las 10:42".into(), id: "fr-3".into() });
        assert!(n.due(t0 + 601).is_none());
        // Una nueva justo después del resumen no avisa al momento (hace nada que avisó): espera su plazo.
        assert!(n.record(t0 + 650, "Excel se colgó", "10:53", "fr-4").is_none());
        assert!(n.due(t0 + 1000).is_none());
        let single = n.due(t0 + 1250).unwrap();
        assert_eq!(single, Notice { text: "Excel se colgó a las 10:53 · ver grabación".into(), id: "fr-4".into() });
    }

    #[test]
    fn una_sola_en_diez_minutos_no_manda_resumen_y_la_siguiente_avisa_al_momento() {
        let mut n = Notifier::default();
        assert!(n.record(1000, "A se colgó", "00:16", "fr-1").is_some());
        // Nada que resumir al vencer.
        assert!(n.due(1600).is_none());
        // Pasado el plazo, la siguiente vuelve a avisar al momento.
        let again = n.record(1700, "B se colgó", "00:28", "fr-2").unwrap();
        assert_eq!(again.id, "fr-2");
        // Si llega otra tras vencer sin que nadie llamara a `due`, el resumen sale en esa llamada y ninguna se pierde.
        let mut m = Notifier::default();
        assert!(m.record(0, "A se colgó", "10:00", "fr-1").is_some());
        assert!(m.record(100, "B se colgó", "10:02", "fr-2").is_none());
        let out = m.record(700, "C se colgó", "10:12", "fr-3").unwrap();
        assert_eq!(out.text, "2 grabaciones automáticas desde las 10:00");
        // Y la C espera su plazo, sin avisar ya.
        assert!(m.due(1300).is_some_and(|x| x.id == "fr-3"));
    }

    #[test]
    fn nunca_dos_avisos_seguidos() {
        let mut n = Notifier::default();
        let mut sent: Vec<u64> = Vec::new();
        for i in 0..200u64 {
            let now = i * 37;
            if n.record(now, "X se colgó", "12:00", &format!("fr-{i}")).is_some() {
                sent.push(now);
            }
            if i % 3 == 0 {
                // Entre grabación y grabación también se va comprobando el plazo, como hace el vigilante.
                for s in 0..37 {
                    if n.due(now + s).is_some() {
                        sent.push(now + s);
                    }
                }
            }
        }
        for w in sent.windows(2) {
            assert!(w[1] - w[0] >= NOTICE_SECS, "{} y {} están a menos de {NOTICE_SECS} s", w[0], w[1]);
        }
        assert!(sent.len() >= 2);
    }

    #[test]
    fn lee_un_evento_de_error_de_aplicacion() {
        let xml = "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='Application Error'/><EventID Qualifiers='0'>1000</EventID><Level>2</Level><TimeCreated SystemTime='2026-11-23T14:42:11.123Z'/><EventRecordID>48211</EventRecordID><Channel>Application</Channel></System><EventData><Data>EXCEL.EXE</Data><Data>16.0.17328.20068</Data></EventData></Event>";
        assert_eq!(parse_event_xml(xml), Some(AppEvent { record_id: 48211, event_id: 1000, program: "EXCEL.EXE".into() }));
        let hang = "<Event><System><EventID>1002</EventID><EventRecordID>7</EventRecordID></System><EventData><Data Name='AppName'>Mi &amp; Programa.exe</Data></EventData></Event>";
        assert_eq!(parse_event_xml(hang), Some(AppEvent { record_id: 7, event_id: 1002, program: "Mi & Programa.exe".into() }));
        // Sin datos, sin número de registro o roto: nada.
        assert_eq!(parse_event_xml("<Event><System><EventID>1000</EventID><EventRecordID>7</EventRecordID></System></Event>"), None);
        assert_eq!(parse_event_xml("<Event><System><EventID>1000</EventID></System><EventData><Data>X</Data></EventData></Event>"), None);
        assert_eq!(parse_event_xml(""), None);
        assert_eq!(parse_event_xml("<Event><System><EventID>mil</EventID><EventRecordID>7</EventRecordID></System><EventData><Data>X</Data></EventData></Event>"), None);
    }

    /// Solo lectura: pregunta de verdad al registro Application.
    #[cfg(windows)]
    #[test]
    #[ignore = "usa el Visor de eventos real"]
    fn lee_de_verdad_el_registro_de_aplicacion() {
        let mut p = EventPoller::default();
        assert!(p.poll().is_empty());
        println!("REGISTRO: último evento {:?}", p.last);
        assert!(p.last.is_some(), "el canal Application no tiene ningún evento o no se pudo leer");
        // Con un filtro que mira desde el principio, los 1000/1002 que haya salen bien formados.
        let old = windows_log::new_app_events(0);
        println!("REGISTRO: {} eventos 1000/1002 en total (máx. 50): {:?}", old.len(), old.iter().take(3).collect::<Vec<_>>());
        assert!(old.iter().all(|e| (e.event_id == 1000 || e.event_id == 1002) && !e.program.is_empty()));
        // Y un segundo sondeo sin nada nuevo no devuelve nada.
        assert!(p.poll().is_empty());
    }
}
