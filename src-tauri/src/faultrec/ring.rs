//! Los búferes circulares de Vigilar (lógica pura, con pruebas): el vídeo ya comprimido, las medidas de cada
//! segundo y los clics y teclas. Todo está en memoria; nada se escribe en disco hasta que se pide guardar.
//!
//! El vídeo se guarda en trozos que empiezan en un fotograma clave (un GOP): así lo que se guarda siempre se
//! puede reproducir desde el principio, sin volver a codificar.

use super::analysis::Tick;
use super::input::InputEvent;
use std::collections::VecDeque;

/// Unidades de 100 ns que tiene un segundo (el tiempo de Media Foundation).
pub const HNS: i64 = 10_000_000;

/// Una muestra de vídeo ya comprimida (H.264, con los códigos de inicio delante de cada NAL).
#[derive(Clone, Debug, PartialEq)]
pub struct VSample {
    pub data: Vec<u8>,
    /// Instante en unidades de 100 ns desde el principio de la sesión de Vigilar.
    pub time: i64,
    pub duration: i64,
    /// Fotograma clave (`MFSampleExtension_CleanPoint`).
    pub key: bool,
}

impl VSample {
    pub fn end(&self) -> i64 {
        self.time + self.duration.max(0)
    }
}

/// El vídeo de los últimos segundos: como mucho `keep` (contado desde un fotograma clave) y `max_bytes` de memoria.
#[derive(Debug)]
pub struct VideoRing {
    samples: VecDeque<VSample>,
    bytes: usize,
    keep: i64,
    max_bytes: usize,
}

impl VideoRing {
    pub fn new(keep_secs: u32, max_bytes: usize) -> VideoRing {
        VideoRing { samples: VecDeque::new(), bytes: 0, keep: i64::from(keep_secs) * HNS, max_bytes }
    }

    /// Una muestra más. Lo que llega antes del primer fotograma clave no se puede reproducir: se tira.
    pub fn push(&mut self, s: VSample) {
        if self.samples.is_empty() && !s.key {
            return;
        }
        self.bytes += s.data.len();
        self.samples.push_back(s);
        self.trim();
    }

    /// Quita el GOP más viejo mientras lo que queda siga cubriendo `keep`, o mientras se pase de memoria.
    fn trim(&mut self) {
        loop {
            // El segundo fotograma clave marca dónde acaba el primer GOP.
            let Some(second) = self.samples.iter().skip(1).position(|s| s.key).map(|i| i + 1) else {
                // Sin segundo fotograma clave no se puede recortar por GOP, y el que se está llenando se respeta. Pero
                // si ese único GOP ya dura más que todo lo que se guarda (un codificador que no da claves; lo normal es
                // una cada 2 s) y se pasa de memoria, se vacía: el siguiente fotograma clave vuelve a empezar.
                let span = self.samples.back().map_or(0, VSample::end) - self.samples.front().map_or(0, |f| f.time);
                if self.bytes > self.max_bytes && span > self.keep {
                    self.samples.clear();
                    self.bytes = 0;
                }
                return;
            };
            let end = self.samples.back().map_or(0, VSample::end);
            let still_covers = end - self.samples[second].time >= self.keep;
            if !still_covers && self.bytes <= self.max_bytes {
                return;
            }
            for _ in 0..second {
                if let Some(old) = self.samples.pop_front() {
                    self.bytes -= old.data.len();
                }
            }
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Suelta todo (al apagar Vigilar).
    pub fn clear(&mut self) {
        self.samples = VecDeque::new();
        self.bytes = 0;
    }

    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Cuántos segundos hay guardados.
    pub fn buffered_secs(&self) -> u32 {
        match (self.samples.front(), self.samples.back()) {
            (Some(a), Some(b)) => ((b.end() - a.time).max(0) / HNS) as u32,
            _ => 0,
        }
    }

    /// El instante en que acaba lo último guardado.
    pub fn end(&self) -> Option<i64> {
        self.samples.back().map(VSample::end)
    }

    /// Lo guardado entre `from` y `to` (unidades de 100 ns de la sesión), empezando en el fotograma clave igual o
    /// anterior a `from` (o en el primero que haya, si `from` queda antes). `None` si no hay nada que enseñar.
    pub fn window(&self, from: i64, to: i64) -> Option<Vec<VSample>> {
        self.samples.front()?;
        let start = self.samples.iter().rposition(|s| s.key && s.time <= from).unwrap_or(0);
        let out: Vec<VSample> = self.samples.iter().skip(start).take_while(|s| s.time < to).cloned().collect();
        // Siempre hay al menos el fotograma clave de salida, aunque `to` sea anterior a todo.
        if out.is_empty() {
            return self.samples.get(start).map(|s| vec![s.clone()]);
        }
        Some(out)
    }
}

/// Quita de la cola lo que empezó antes de `min_t` (segundos de la sesión).
pub fn trim_ticks(ticks: &mut VecDeque<Tick>, min_t: u32) {
    while ticks.front().is_some_and(|t| t.t < min_t) {
        ticks.pop_front();
    }
}

/// Las medidas del tramo que empieza en el segundo `from_s` y llega hasta `to_s` (sin incluirlo), con el tiempo
/// llevado a 0 en `from_s`.
pub fn window_ticks(ticks: &VecDeque<Tick>, from_s: u32, to_s: u32) -> Vec<Tick> {
    ticks.iter().filter(|t| t.t >= from_s && t.t < to_s).map(|t| Tick { t: t.t - from_s, ..t.clone() }).collect()
}

/// Los clics y teclas entre `from_ms` y `to_ms` (milisegundos de la sesión), con el tiempo llevado a 0 en `from_ms`.
pub fn window_input(events: &[InputEvent], from_ms: u32, to_ms: u32) -> Vec<InputEvent> {
    events.iter().filter(|e| e.t >= from_ms && e.t <= to_ms).map(|e| InputEvent { t: e.t - from_ms, ..e.clone() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_segundo_fotograma_clave_no_pasa_del_tope_de_memoria() {
        // Un fotograma cada 10 s y ninguna clave más: a partir de los 75 s y del tope, se vacía.
        let mut r = VideoRing::new(75, 1000);
        r.push(VSample { data: vec![0; 400], time: 0, duration: HNS, key: true });
        for i in 1..20 {
            r.push(VSample { data: vec![0; 400], time: i * 10 * HNS, duration: HNS, key: false });
            assert!(r.bytes() <= 1000 || i * 10 <= 75, "{} bytes a los {} s", r.bytes(), i * 10);
        }
    }

    /// Un fotograma cada 200 ms (5 por segundo) y un clave cada 10.
    fn frame(i: i64, size: usize) -> VSample {
        VSample { data: vec![0; size], time: i * HNS / 5, duration: HNS / 5, key: i % 10 == 0 }
    }

    #[test]
    fn nada_antes_del_primer_fotograma_clave() {
        let mut r = VideoRing::new(75, 1 << 30);
        r.push(frame(3, 10));
        r.push(frame(4, 10));
        assert!(r.is_empty());
        assert_eq!(r.bytes(), 0);
        r.push(frame(10, 10));
        r.push(frame(11, 10));
        assert_eq!(r.len(), 2);
        assert_eq!(r.bytes(), 20);
    }

    #[test]
    fn guarda_75_segundos_empezando_en_un_clave() {
        let mut r = VideoRing::new(75, 1 << 30);
        // 200 s de vídeo.
        for i in 0..1000 {
            r.push(frame(i, 100));
        }
        let first = r.samples.front().unwrap();
        assert!(first.key);
        let covered = r.end().unwrap() - first.time;
        assert!(covered >= 75 * HNS, "cubre {covered}");
        // Y no se guarda más de la cuenta: como mucho un GOP (2 s) de más.
        assert!(covered < 77 * HNS + HNS / 5, "cubre {covered}");
        assert_eq!(r.buffered_secs() as i64, covered / HNS);
        // Cuenta bien los bytes.
        assert_eq!(r.bytes(), r.len() * 100);
    }

    #[test]
    fn con_menos_de_75_segundos_no_quita_nada() {
        let mut r = VideoRing::new(75, 1 << 30);
        for i in 0..200 {
            r.push(frame(i, 10));
        }
        assert_eq!(r.len(), 200);
        assert_eq!(r.buffered_secs(), 40);
    }

    #[test]
    fn el_tope_de_memoria_quita_el_gop_mas_viejo() {
        // 10 fotogramas de 1000 bytes por GOP: el tope deja 3 GOP y algo.
        let mut r = VideoRing::new(75, 35_000);
        for i in 0..300 {
            r.push(frame(i, 1000));
        }
        assert!(r.bytes() <= 35_000 + 10_000, "{}", r.bytes());
        assert!(r.samples.front().unwrap().key);
        // El GOP que se está llenando nunca se quita aunque solo quede ese.
        let mut tiny = VideoRing::new(75, 10);
        for i in 0..25 {
            tiny.push(frame(i, 1000));
        }
        assert!(!tiny.is_empty());
        assert!(tiny.samples.front().unwrap().key);
        assert!(tiny.len() >= 5);
    }

    #[test]
    fn una_ventana_empieza_en_el_clave_anterior() {
        let mut r = VideoRing::new(75, 1 << 30);
        for i in 0..100 {
            r.push(frame(i, 10));
        }
        // Claves en 0, 2, 4, 6… s. Desde 5,5 s hasta 9 s: empieza en el de 4 s.
        let w = r.window(11 * HNS / 2, 9 * HNS).unwrap();
        assert_eq!(w.first().unwrap().time, 4 * HNS);
        assert!(w.first().unwrap().key);
        assert!(w.last().unwrap().time < 9 * HNS);
        assert_eq!(w.len(), 25);
        // Justo en un clave: empieza ahí.
        assert_eq!(r.window(6 * HNS, 7 * HNS).unwrap().first().unwrap().time, 6 * HNS);
        // Antes de todo lo guardado: desde el primero.
        assert_eq!(r.window(-5 * HNS, 3 * HNS).unwrap().first().unwrap().time, 0);
        // Un «hasta» anterior al clave de salida deja al menos ese fotograma.
        assert_eq!(r.window(6 * HNS, 0).unwrap().len(), 1);
        // Vacío: nada.
        assert!(VideoRing::new(75, 100).window(0, HNS).is_none());
    }

    #[test]
    fn la_ventana_tras_recortar_sigue_empezando_en_clave() {
        let mut r = VideoRing::new(75, 1 << 30);
        for i in 0..1000 {
            r.push(frame(i, 10));
        }
        let end = r.end().unwrap();
        let w = r.window(end - 60 * HNS, end).unwrap();
        assert!(w[0].key);
        assert!(w.last().unwrap().end() <= end);
        let covered = w.last().unwrap().end() - w[0].time;
        assert!((60 * HNS..62 * HNS + HNS / 5).contains(&covered), "{covered}");
    }

    fn tick(t: u32) -> Tick {
        Tick { t, cpu: t as f32, ..Default::default() }
    }

    #[test]
    fn las_medidas_se_recortan_y_se_llevan_a_cero() {
        let mut q: VecDeque<Tick> = (0..100).map(tick).collect();
        trim_ticks(&mut q, 20);
        assert_eq!(q.front().unwrap().t, 20);
        assert_eq!(q.len(), 80);
        let w = window_ticks(&q, 30, 35);
        assert_eq!(w.iter().map(|t| t.t).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
        assert_eq!(w[0].cpu, 30.0);
        assert!(window_ticks(&q, 500, 600).is_empty());
        trim_ticks(&mut q, 1000);
        assert!(q.is_empty());
    }

    #[test]
    fn los_clics_de_la_ventana_se_llevan_a_cero() {
        let ev = |t: u32, label: &str| InputEvent { t, kind: "keys".into(), label: label.into(), ..Default::default() };
        let all = vec![ev(500, "a"), ev(1500, "b"), ev(2500, "c"), ev(9000, "d")];
        let w = window_input(&all, 1000, 3000);
        assert_eq!(w.iter().map(|e| (e.t, e.label.as_str())).collect::<Vec<_>>(), vec![(500, "b"), (1500, "c")]);
        assert!(window_input(&all, 10_000, 20_000).is_empty());
    }
}
