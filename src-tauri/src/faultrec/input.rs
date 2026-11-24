//! Clics y teclas de la grabación: se apuntan en una línea de tiempo para enseñarlos sobre el vídeo.
//!
//! Se mira el estado del ratón y del teclado cada 8 ms con `GetAsyncKeyState`: ni ganchos
//! (`SetWindowsHookEx`) ni nada que los antivirus miren mal. **Las letras que se escriben nunca se guardan**:
//! cualquier tecla que escribe algo se apunta como «•••», sea una contraseña o no. Solo se guardan los clics
//! (con su posición dentro de lo grabado) y los atajos y teclas especiales por su nombre («Ctrl + P», «Intro»).
//!
//! La lógica (nombres de teclas, qué es escribir, juntar los «•••», dobles clics, posiciones) es pura y tiene
//! pruebas; el hilo que sondea solo existe en Windows.

use serde::{Deserialize, Serialize};

/// Lo más que se apunta en una grabación; lo siguiente se ignora.
pub const MAX_EVENTS: usize = 2000;
/// Lo que se apunta cuando se escribe.
pub const TYPING: &str = "•••";
/// Varias pulsaciones de escritura seguidas con menos de esto entre una y otra son una sola marca.
const MERGE_TYPING_MS: u32 = 1500;
/// Dos clics a menos de esta distancia (píxeles) pueden ser un doble clic.
const DOUBLE_CLICK_PX: i32 = 6;

/// Un clic o un grupo de teclas.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct InputEvent {
    /// Milisegundos desde el primer fotograma.
    pub t: u32,
    /// "click" | "keys"
    pub kind: String,
    /// Dentro de lo grabado, de 0 a 1 (solo clics).
    pub x: f32,
    pub y: f32,
    /// Clics: "Clic", "Clic derecho", "Clic central", "Doble clic". Teclas: "Ctrl + P", "Intro", "•••".
    pub label: String,
}

/// Un rectángulo del escritorio, en píxeles físicos.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// De un punto del escritorio a «de 0 a 1 dentro del rectángulo»; `None` si cae fuera (el borde derecho e inferior no cuentan).
pub fn normalize(point: (i32, i32), area: Area) -> Option<(f32, f32)> {
    if area.w <= 0 || area.h <= 0 {
        return None;
    }
    let (dx, dy) = (point.0 - area.x, point.1 - area.y);
    if dx < 0 || dy < 0 || dx >= area.w || dy >= area.h {
        return None;
    }
    Some((dx as f32 / area.w as f32, dy as f32 / area.h as f32))
}

// ---------- Teclas ----------

/// Los modificadores que están pulsados.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

const VK_SPACE: u32 = 0x20;

/// Las teclas que escriben algo: letras, números, signos, espacio y teclado numérico.
fn is_printable(vk: u32) -> bool {
    matches!(vk, 0x30..=0x39 | 0x41..=0x5A | VK_SPACE | 0x60..=0x6F | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2)
}

/// ¿Esta pulsación escribe algo? Sí si es una tecla de escribir sin Ctrl, Alt ni Win, o con Ctrl+Alt a la vez
/// (así se escriben «@» o «€» con AltGr) o solo con Mayús.
pub fn is_typing(vk: u32, m: Mods) -> bool {
    if !is_printable(vk) || m.win {
        return false;
    }
    (!m.ctrl && !m.alt) || (m.ctrl && m.alt)
}

/// El nombre de una tecla que no escribe, o `None` si no se sabe cuál es.
fn key_name(vk: u32) -> Option<String> {
    let fixed = match vk {
        0x08 => "Retroceso",
        0x09 => "Tab",
        0x0D => "Intro",
        0x14 => "Bloq Mayús",
        0x1B => "Esc",
        VK_SPACE => "Espacio",
        0x21 => "Re Pág",
        0x22 => "Av Pág",
        0x23 => "Fin",
        0x24 => "Inicio",
        0x25 => "←",
        0x26 => "↑",
        0x27 => "→",
        0x28 => "↓",
        0x2C => "Impr Pant",
        0x2D => "Insert",
        0x2E => "Supr",
        _ => "",
    };
    if !fixed.is_empty() {
        return Some(fixed.to_string());
    }
    match vk {
        // F1 … F12.
        0x70..=0x7B => Some(format!("F{}", vk - 0x70 + 1)),
        // Cifras y letras: en mayúscula, sea cual sea el idioma del teclado.
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).map(|c| c.to_string()),
        // Signos y teclado numérico: su posición cambia con el idioma, no se adivina.
        _ if is_printable(vk) => Some("tecla".to_string()),
        _ => None,
    }
}

/// Lo que se apunta al pulsar `vk` con esos modificadores: «•••» si se escribe, «Ctrl + P» o «Intro» si es un
/// atajo o una tecla especial, y `None` si no es nada que apuntar (un modificador solo, una tecla desconocida).
pub fn key_label(vk: u32, m: Mods) -> Option<String> {
    if is_typing(vk, m) {
        return Some(TYPING.to_string());
    }
    let name = key_name(vk)?;
    let mut parts: Vec<&str> = Vec::new();
    for (on, n) in [(m.ctrl, "Ctrl"), (m.alt, "Alt"), (m.shift, "Mayús"), (m.win, "Win")] {
        if on {
            parts.push(n);
        }
    }
    parts.push(&name);
    Some(parts.join(" + "))
}

// ---------- Lo apuntado ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// Va juntando los eventos con las reglas de dobles clics, de «•••» seguidos y del máximo.
#[derive(Default)]
pub struct Recorder {
    events: Vec<InputEvent>,
    /// El último clic izquierdo que se apuntó: instante, posición y lugar en la lista.
    last_left: Option<(u32, (i32, i32), usize)>,
}

impl Recorder {
    pub fn new() -> Recorder {
        Recorder::default()
    }

    /// Un clic en el instante `t` (ms), en `pos` (escritorio) y, ya pasado a 0..1, en `norm`. `dbl_ms`: lo que
    /// tarda Windows en dar un doble clic.
    pub fn click(&mut self, t: u32, button: Button, pos: (i32, i32), norm: (f32, f32), dbl_ms: u32) {
        if button == Button::Left {
            if let Some((t0, p0, i)) = self.last_left {
                let (dx, dy) = (pos.0 - p0.0, pos.1 - p0.1);
                if t.saturating_sub(t0) <= dbl_ms && dx * dx + dy * dy < DOUBLE_CLICK_PX * DOUBLE_CLICK_PX && self.events.get(i).is_some_and(|e| e.label == "Clic") {
                    self.events[i].label = "Doble clic".to_string();
                    self.last_left = None;
                    return;
                }
            }
        }
        if self.events.len() >= MAX_EVENTS {
            return;
        }
        let label = match button {
            Button::Left => "Clic",
            Button::Right => "Clic derecho",
            Button::Middle => "Clic central",
        };
        self.events.push(InputEvent { t, kind: "click".into(), x: norm.0, y: norm.1, label: label.into() });
        self.last_left = (button == Button::Left).then_some((t, pos, self.events.len() - 1));
    }

    /// Un atajo, una tecla especial o «•••».
    pub fn keys(&mut self, t: u32, label: String) {
        if self.events.len() >= MAX_EVENTS {
            return;
        }
        if label == TYPING {
            if let Some(last) = self.events.last() {
                if last.kind == "keys" && last.label == TYPING && t.saturating_sub(last.t) < MERGE_TYPING_MS {
                    return;
                }
            }
        }
        self.events.push(InputEvent { t, kind: "keys".into(), label, ..Default::default() });
    }

    pub fn finish(self) -> Vec<InputEvent> {
        self.events
    }

    /// Lo apuntado hasta ahora, sin cerrar el apunte (Vigilar lo guarda en un búfer circular).
    pub fn snapshot(&self) -> Vec<InputEvent> {
        self.events.clone()
    }

    /// Quita lo anterior a `t_ms` (para el búfer circular de Vigilar). Un doble clic cuyo primer clic se fue ya no se junta.
    pub fn trim_before(&mut self, t_ms: u32) {
        let n = self.events.iter().take_while(|e| e.t < t_ms).count();
        if n == 0 {
            return;
        }
        self.events.drain(..n);
        self.last_left = self.last_left.and_then(|(t, p, i)| i.checked_sub(n).map(|i| (t, p, i)));
    }
}

// ---------- El hilo que mira (solo Windows) ----------

/// Qué se está grabando, para saber dónde cae cada clic.
#[derive(Clone, Debug, Default)]
pub struct Watch {
    /// Nombre GDI del monitor (vacío = el principal).
    pub gdi: String,
    pub window: Option<isize>,
    /// La zona ya recortada a lo que cabe en el monitor.
    pub region: Option<super::Region>,
}

#[cfg(windows)]
pub use imp::{run, run_ring};

#[cfg(not(windows))]
pub fn run(_: std::sync::Arc<std::sync::Mutex<Option<super::Started>>>, _: std::sync::Arc<std::sync::atomic::AtomicBool>, _: Watch) -> Vec<InputEvent> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn run_ring(_: super::Started, _: std::sync::Arc<std::sync::atomic::AtomicBool>, _: Watch, _: std::sync::Arc<std::sync::Mutex<Recorder>>, _: u32) {}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::faultrec::{win, Started};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetDoubleClickTime, VK_CONTROL, VK_LBUTTON, VK_LWIN, VK_MBUTTON, VK_MENU, VK_RBUTTON, VK_RWIN, VK_SHIFT};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_SWAPBUTTON};

    /// Cada cuánto se mira.
    const POLL: Duration = Duration::from_millis(8);

    fn down(vk: u32) -> bool {
        // SAFETY: sin punteros; el bit alto dice si la tecla está pulsada ahora.
        (unsafe { GetAsyncKeyState(vk as i32) } as u16) & 0x8000 != 0
    }

    /// Las teclas que se miran: de Retroceso a las de signos, sin los botones del ratón ni los modificadores
    /// (esos se leen aparte).
    fn watched(vk: u32) -> bool {
        let modifier = matches!(vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5);
        ((0x08..=0xDF).contains(&vk) || vk == 0xE2) && !modifier
    }

    /// El rectángulo que se está grabando ahora mismo (la ventana se puede mover).
    fn area_now(w: &Watch) -> Option<Area> {
        if let Some(h) = w.window {
            return win::window_frame(h);
        }
        let m = win::monitor_rect(&w.gdi)?;
        Some(match w.region {
            Some(r) => Area { x: m.x + r.x, y: m.y + r.y, w: r.w as i32, h: r.h as i32 },
            None => m,
        })
    }

    pub fn run(first: Arc<Mutex<Option<Started>>>, stop: Arc<AtomicBool>, watch: Watch) -> Vec<InputEvent> {
        // Hasta el primer fotograma no se apunta nada.
        let started = loop {
            if stop.load(Ordering::SeqCst) {
                return Vec::new();
            }
            if let Some(s) = *first.lock().unwrap_or_else(|e| e.into_inner()) {
                break s;
            }
            std::thread::sleep(POLL);
        };
        let rec = Mutex::new(Recorder::new());
        poll(started, &stop, &watch, &rec, None);
        rec.into_inner().unwrap_or_else(|e| e.into_inner()).finish()
    }

    /// Lo mismo, pero sin fin y guardando solo los últimos `keep_ms` milisegundos en `rec` (Vigilar). `started` es el
    /// instante del que cuentan los milisegundos.
    pub fn run_ring(started: Started, stop: Arc<AtomicBool>, watch: Watch, rec: Arc<Mutex<Recorder>>, keep_ms: u32) {
        poll(started, &stop, &watch, &rec, Some(keep_ms));
    }

    /// El sondeo: cada 8 ms mira los botones del ratón y las teclas y lo apunta en `rec`. Con `keep_ms`, cada
    /// segundo quita lo más viejo.
    fn poll(started: Started, stop: &AtomicBool, watch: &Watch, rec: &Mutex<Recorder>, keep_ms: Option<u32>) {
        let lock = || rec.lock().unwrap_or_else(|e| e.into_inner());
        // Lo que ya estaba pulsado al empezar no cuenta.
        let mut was = [false; 256];
        for (vk, slot) in was.iter_mut().enumerate() {
            *slot = down(vk as u32);
        }
        // SAFETY: sin punteros.
        let swapped = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
        let (left, right) = if swapped { (Button::Right, Button::Left) } else { (Button::Left, Button::Right) };
        let buttons = [(VK_LBUTTON.0 as u32, left), (VK_RBUTTON.0 as u32, right), (VK_MBUTTON.0 as u32, Button::Middle)];
        let mut last_trim = 0u32;

        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(POLL);
            let t = started.at.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
            for (vk, button) in buttons {
                let now = down(vk);
                let before = std::mem::replace(&mut was[vk as usize], now);
                if now && !before {
                    let mut p = POINT::default();
                    // SAFETY: `p` vive durante la llamada.
                    if unsafe { GetCursorPos(&mut p) }.is_err() {
                        continue;
                    }
                    let norm = area_now(watch).and_then(|a| normalize((p.x, p.y), a));
                    if let Some(n) = norm {
                        // SAFETY: sin punteros.
                        lock().click(t, button, (p.x, p.y), n, unsafe { GetDoubleClickTime() });
                    }
                }
            }
            let mods = Mods { ctrl: down(VK_CONTROL.0 as u32), alt: down(VK_MENU.0 as u32), shift: down(VK_SHIFT.0 as u32), win: down(VK_LWIN.0 as u32) || down(VK_RWIN.0 as u32) };
            for vk in (0x08u32..=0xDF).chain(std::iter::once(0xE2)) {
                if !watched(vk) {
                    continue;
                }
                let now = down(vk);
                let before = std::mem::replace(&mut was[vk as usize], now);
                if now && !before {
                    if let Some(label) = key_label(vk, mods) {
                        lock().keys(t, label);
                    }
                }
            }
            if let Some(keep) = keep_ms {
                if t.saturating_sub(last_trim) >= 1000 {
                    last_trim = t;
                    lock().trim_before(t.saturating_sub(keep));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods { ctrl: false, alt: false, shift: false, win: false };
    const CTRL: Mods = Mods { ctrl: true, ..NONE };
    const ALT: Mods = Mods { alt: true, ..NONE };
    const SHIFT: Mods = Mods { shift: true, ..NONE };
    const WIN: Mods = Mods { win: true, ..NONE };
    const A: u32 = 0x41;

    #[test]
    fn la_posicion_va_de_0_a_1_dentro_y_nada_fuera() {
        let a = Area { x: 100, y: 50, w: 200, h: 100 };
        assert_eq!(normalize((100, 50), a), Some((0.0, 0.0)));
        assert_eq!(normalize((200, 100), a), Some((0.5, 0.5)));
        assert_eq!(normalize((299, 149), a), Some((0.995, 0.99)));
        // Fuera por cualquier lado, y el borde derecho/inferior ya es fuera.
        for p in [(99, 60), (100, 49), (300, 60), (150, 150), (-5, -5)] {
            assert_eq!(normalize(p, a), None, "{p:?}");
        }
        // Un rectángulo sin tamaño no tiene dentro.
        assert_eq!(normalize((0, 0), Area::default()), None);
        // Monitores a la izquierda del principal (coordenadas negativas).
        let left = Area { x: -1920, y: 0, w: 1920, h: 1080 };
        assert_eq!(normalize((-960, 540), left), Some((0.5, 0.5)));
    }

    #[test]
    fn escribir_se_apunta_como_puntos_y_nunca_la_letra() {
        for vk in [A, 0x5A, 0x30, 0x39, VK_SPACE, 0x60, 0x6B, 0xBA, 0xC0, 0xDB, 0xE2] {
            assert!(is_typing(vk, NONE), "{vk:#x}");
            assert!(is_typing(vk, SHIFT), "{vk:#x} con Mayús");
            assert_eq!(key_label(vk, NONE).as_deref(), Some("•••"));
            assert_eq!(key_label(vk, SHIFT).as_deref(), Some("•••"));
        }
        // Ctrl+Alt a la vez es AltGr: se escribe.
        assert!(is_typing(A, Mods { ctrl: true, alt: true, ..NONE }));
        assert!(is_typing(A, Mods { ctrl: true, alt: true, shift: true, win: false }));
        // Con Ctrl, Alt o Win solos es un atajo.
        for m in [CTRL, ALT, WIN, Mods { ctrl: true, win: true, ..NONE }, Mods { ctrl: true, alt: true, win: true, shift: false }] {
            assert!(!is_typing(A, m), "{m:?}");
        }
        // Lo que no escribe, tampoco.
        for vk in [0x0D, 0x1B, 0x08, 0x70, 0x26, 0x2E] {
            assert!(!is_typing(vk, NONE), "{vk:#x}");
        }
        // Ninguna etiqueta guarda la letra pulsada.
        for vk in 0x08u32..=0xFE {
            for m in [NONE, SHIFT] {
                if let Some(l) = key_label(vk, m) {
                    assert!(!(l.chars().count() == 1 && l.chars().all(char::is_alphanumeric)), "{vk:#x} -> {l}");
                }
            }
        }
    }

    #[test]
    fn los_atajos_y_teclas_especiales_llevan_su_nombre_en_espanol() {
        assert_eq!(key_label(0x50, CTRL).as_deref(), Some("Ctrl + P"));
        assert_eq!(key_label(0x43, CTRL).as_deref(), Some("Ctrl + C"));
        assert_eq!(key_label(0x1B, Mods { ctrl: true, shift: true, ..NONE }).as_deref(), Some("Ctrl + Mayús + Esc"));
        assert_eq!(key_label(0x73, ALT).as_deref(), Some("Alt + F4"));
        assert_eq!(key_label(0x44, WIN).as_deref(), Some("Win + D"));
        assert_eq!(key_label(0x31, CTRL).as_deref(), Some("Ctrl + 1"));
        assert_eq!(key_label(0x09, ALT).as_deref(), Some("Alt + Tab"));
        assert_eq!(key_label(0x09, SHIFT).as_deref(), Some("Mayús + Tab"));
        // El orden de los modificadores es siempre Ctrl, Alt, Mayús, Win.
        assert_eq!(key_label(0x1B, Mods { ctrl: true, alt: true, shift: true, win: true }).as_deref(), Some("Ctrl + Alt + Mayús + Win + Esc"));
        for (vk, n) in [(0x0D, "Intro"), (0x09, "Tab"), (0x1B, "Esc"), (0x08, "Retroceso"), (0x2E, "Supr"), (0x2D, "Insert"), (0x24, "Inicio"), (0x23, "Fin"), (0x21, "Re Pág"), (0x22, "Av Pág"), (0x2C, "Impr Pant"), (0x14, "Bloq Mayús")] {
            assert_eq!(key_label(vk, NONE).as_deref(), Some(n));
        }
        for (vk, n) in [(0x26, "↑"), (0x28, "↓"), (0x25, "←"), (0x27, "→")] {
            assert_eq!(key_label(vk, NONE).as_deref(), Some(n));
        }
        for i in 0..12u32 {
            assert_eq!(key_label(0x70 + i, NONE), Some(format!("F{}", i + 1)));
        }
        // Un signo con Ctrl: su nombre cambia con el idioma del teclado, no se adivina.
        assert_eq!(key_label(0xBB, CTRL).as_deref(), Some("Ctrl + tecla"));
        assert_eq!(key_label(0x6B, Mods { ctrl: true, shift: true, ..NONE }).as_deref(), Some("Ctrl + Mayús + tecla"));
        assert_eq!(key_label(VK_SPACE, CTRL).as_deref(), Some("Ctrl + Espacio"));
        // Un modificador solo o una tecla desconocida no apuntan nada.
        for vk in [0x10, 0x11, 0x12, 0x5B, 0x5C, 0xA0, 0xA2, 0xA4, 0x01, 0x02, 0xFF, 0x07] {
            assert_eq!(key_label(vk, CTRL), None, "{vk:#x}");
        }
    }

    #[test]
    fn los_puntos_seguidos_se_juntan_si_van_pegados() {
        let mut r = Recorder::new();
        r.keys(1000, "•••".into());
        r.keys(1200, "•••".into());
        r.keys(2400, "•••".into());
        // Se cuenta desde el primero que se guardó (no desde el último que se descartó): a 1,5 s ya es otro.
        r.keys(2500, "•••".into());
        // Un atajo en medio corta la tanda.
        r.keys(2600, "Ctrl + S".into());
        r.keys(2700, "•••".into());
        let ev = r.finish();
        let v: Vec<(u32, &str)> = ev.iter().map(|e| (e.t, e.label.as_str())).collect();
        assert_eq!(v, vec![(1000, "•••"), (2500, "•••"), (2600, "Ctrl + S"), (2700, "•••")]);
        // Un clic también corta la tanda.
        let mut r = Recorder::new();
        r.keys(0, "•••".into());
        r.click(100, Button::Left, (5, 5), (0.1, 0.1), 500);
        r.keys(200, "•••".into());
        assert_eq!(r.finish().len(), 3);
    }

    #[test]
    fn dos_clics_izquierdos_seguidos_y_juntos_son_un_doble_clic() {
        let mut r = Recorder::new();
        r.click(1000, Button::Left, (100, 100), (0.1, 0.2), 500);
        r.click(1300, Button::Left, (103, 98), (0.1, 0.2), 500);
        let ev = r.finish();
        assert_eq!(ev.len(), 1);
        assert_eq!((ev[0].t, ev[0].label.as_str(), ev[0].kind.as_str()), (1000, "Doble clic", "click"));
        assert_eq!((ev[0].x, ev[0].y), (0.1, 0.2));

        // Muy tarde, muy lejos o con otro botón: son clics aparte.
        for (b2, t2, p2) in [(Button::Left, 1600, (100, 100)), (Button::Left, 1300, (110, 100)), (Button::Right, 1300, (100, 100)), (Button::Left, 1300, (106, 100))] {
            let mut r = Recorder::new();
            r.click(1000, Button::Left, (100, 100), (0.5, 0.5), 500);
            r.click(t2, b2, p2, (0.5, 0.5), 500);
            let ev = r.finish();
            assert_eq!(ev.len(), 2, "{b2:?} {t2} {p2:?}");
            assert_eq!(ev[0].label, "Clic");
        }

        // Tres clics seguidos: doble y uno nuevo (el tercero no se pega al doble).
        let mut r = Recorder::new();
        for t in [1000, 1200, 1400] {
            r.click(t, Button::Left, (50, 50), (0.5, 0.5), 500);
        }
        let ev = r.finish();
        assert_eq!(ev.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), vec!["Doble clic", "Clic"]);
    }

    #[test]
    fn los_nombres_de_los_clics() {
        let mut r = Recorder::new();
        r.click(0, Button::Right, (1, 1), (0.0, 0.0), 500);
        r.click(10, Button::Middle, (1, 1), (0.0, 0.0), 500);
        r.click(20, Button::Left, (1, 1), (0.0, 0.0), 500);
        let ev = r.finish();
        assert_eq!(ev.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), vec!["Clic derecho", "Clic central", "Clic"]);
    }

    #[test]
    fn como_mucho_dos_mil_eventos() {
        let mut r = Recorder::new();
        for i in 0..2500u32 {
            if i % 2 == 0 {
                r.click(i * 10, Button::Right, (0, 0), (0.0, 0.0), 500);
            } else {
                r.keys(i * 10, "Intro".into());
            }
        }
        assert_eq!(r.finish().len(), MAX_EVENTS);
        // Un doble clic sobre el último que cabe sigue valiendo.
        let mut r = Recorder::new();
        for i in 0..MAX_EVENTS as u32 - 1 {
            r.keys(i, "Esc".into());
        }
        r.click(5000, Button::Left, (9, 9), (0.0, 0.0), 500);
        r.click(5100, Button::Left, (9, 9), (0.0, 0.0), 500);
        let ev = r.finish();
        assert_eq!((ev.len(), ev.last().unwrap().label.as_str()), (MAX_EVENTS, "Doble clic"));
    }

    #[test]
    fn el_json_va_en_camel_case_y_las_marcas_viejas_se_leen() {
        let e = InputEvent { t: 5, kind: "click".into(), x: 0.5, y: 0.25, label: "Clic".into() };
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(s, r#"{"t":5,"kind":"click","x":0.5,"y":0.25,"label":"Clic"}"#);
        assert_eq!(serde_json::from_str::<InputEvent>(r#"{"t":9}"#).unwrap().t, 9);
    }

    #[test]
    fn el_buffer_circular_quita_lo_viejo_y_los_dobles_clics_siguen_valiendo() {
        let mut r = Recorder::new();
        r.keys(100, "Esc".into());
        r.keys(900, "Tab".into());
        r.click(2000, Button::Left, (5, 5), (0.1, 0.1), 500);
        r.trim_before(1000);
        assert_eq!(r.snapshot().iter().map(|e| e.t).collect::<Vec<_>>(), vec![2000]);
        // El primer clic sigue siendo el último clic izquierdo: el segundo lo vuelve doble.
        r.click(2200, Button::Left, (5, 5), (0.1, 0.1), 500);
        assert_eq!(r.snapshot().len(), 1);
        assert_eq!(r.snapshot()[0].label, "Doble clic");
        // Si se va el primer clic, el segundo ya no se junta con él.
        let mut r = Recorder::new();
        r.click(100, Button::Left, (5, 5), (0.1, 0.1), 500);
        r.trim_before(150);
        assert!(r.snapshot().is_empty());
        r.click(200, Button::Left, (5, 5), (0.1, 0.1), 500);
        assert_eq!(r.snapshot().iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), vec!["Clic"]);
        // Sin nada que quitar, no pasa nada.
        r.trim_before(0);
        assert_eq!(r.snapshot().len(), 1);
    }
}
