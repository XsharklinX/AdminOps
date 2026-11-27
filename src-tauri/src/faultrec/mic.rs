//! La voz del técnico: el micrófono por defecto de Windows (WASAPI compartido) pasado a PCM de 16 bits,
//! 48 000 Hz y un canal, que el hilo de captura mete en la segunda pista (AAC) del MP4.
//! También el sonido del equipo (lo que suena por el altavoz predeterminado, WASAPI en modo «loopback»), con
//! el mismo formato: se mezcla con la voz en esa misma pista.
//!
//! El micrófono vive en su propio hilo (con su COM) y manda trozos de muestras por un canal. Si no arranca
//! (no hay micrófono, Windows no deja usarlo, un error), se graba igual sin voz y la nota lo cuenta.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread::JoinHandle;

/// Muestras por segundo de la voz.
pub const RATE: u32 = 48_000;
/// Muestras de cada décima de segundo (lo que mide el volumen).
pub const TENTH: usize = (RATE / 10) as usize;
/// Si el audio va más de esto por detrás del vídeo, se rellena con silencio (200 ms).
pub const MAX_LAG: u64 = (RATE / 5) as u64;

/// Con el sonido del equipo, no se escribe un trozo de menos de 10 ms (no merece la pena una muestra por llamada).
pub const MIN_BLOCK: usize = (RATE / 100) as usize;

const E_ACCESSDENIED: u32 = 0x8007_0005;
const E_NOTFOUND: u32 = 0x8007_0490;

/// Por qué no se pudo grabar la voz.
#[derive(Clone, Debug, PartialEq)]
pub enum MicError {
    /// Windows no deja usar el micrófono a las aplicaciones (privacidad).
    Denied,
    NoDevice,
    Other(String),
}

impl MicError {
    /// De un código de Windows (y su texto) a lo que se le cuenta al técnico.
    pub fn classify(code: u32, text: &str) -> MicError {
        match code {
            E_ACCESSDENIED => MicError::Denied,
            E_NOTFOUND => MicError::NoDevice,
            _ => MicError::Other(text.to_string()),
        }
    }

    /// La nota que se guarda con la grabación.
    pub fn note(&self) -> String {
        match self {
            MicError::Denied => "No se grabó la voz: Windows no deja usar el micrófono a las aplicaciones (Configuración → Privacidad y seguridad → Micrófono).".to_string(),
            MicError::NoDevice => "No se grabó la voz: no hay ningún micrófono.".to_string(),
            MicError::Other(e) => format!("No se grabó la voz: {}.", e.trim().trim_end_matches('.')),
        }
    }
}

impl MicError {
    /// La nota cuando falla el sonido del equipo (no hay permiso de privacidad que pedir: solo puede faltar la salida).
    pub fn note_system(&self) -> String {
        match self {
            MicError::Denied => "No se grabó el sonido del equipo: Windows no dejó abrir la salida de audio.".to_string(),
            MicError::NoDevice => "No se grabó el sonido del equipo: no hay ninguna salida de audio.".to_string(),
            MicError::Other(e) => format!("No se grabó el sonido del equipo: {}.", e.trim().trim_end_matches('.')),
        }
    }
}

/// Un micrófono (o el sonido del equipo) en marcha.
pub struct Mic {
    /// Trozos de muestras (16 bits, 48 000 Hz, un canal) a medida que llegan.
    pub rx: Receiver<Vec<i16>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Mic {
    /// Lo para y espera a que suelte el dispositivo.
    pub fn finish(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Mic {
    fn drop(&mut self) {
        // Si no se llegó a `finish`, el hilo se entera igual y se va solo.
        self.stop.store(true, Ordering::SeqCst);
    }
}

// ---------- Cuánto suena (puro, con pruebas) ----------

/// El volumen de un trozo de muestras, de 0 a 100: el valor eficaz (RMS) con una curva de raíz cuadrada,
/// para que una voz normal (un 3–15 % del máximo) se vea y el ruido de fondo casi no.
pub fn level(samples: &[i16]) -> u8 {
    if samples.is_empty() {
        return 0;
    }
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    let rms = (sum / samples.len() as f64).sqrt() / 32768.0;
    (rms.sqrt() * 100.0).round().clamp(0.0, 100.0) as u8
}

/// Va midiendo el volumen de cada décima de segundo de lo que se escribe.
#[derive(Default)]
pub struct VoiceMeter {
    buf: Vec<i16>,
    levels: Vec<u8>,
}

impl VoiceMeter {
    pub fn push(&mut self, samples: &[i16]) {
        self.buf.extend_from_slice(samples);
        let whole = self.buf.len() / TENTH * TENTH;
        for chunk in self.buf[..whole].chunks_exact(TENTH) {
            self.levels.push(level(chunk));
        }
        self.buf.drain(..whole);
    }

    /// Lo medido; lo que sobra de la última décima cuenta como una más.
    pub fn finish(mut self) -> Vec<u8> {
        if !self.buf.is_empty() {
            self.levels.push(level(&self.buf));
        }
        self.levels
    }
}

/// Cuánto silencio hay que meter para que el audio no se quede por detrás del vídeo: `written` muestras ya
/// escritas, `pending` esperando, y `due` las que tocarían por el tiempo del vídeo. Solo se rellena si el
/// retraso pasa de `MAX_LAG` (así un micrófono que va con su ritmo no se toca).
pub fn silence_needed(written: u64, pending: u64, due: u64) -> u64 {
    let have = written + pending;
    if due > have + MAX_LAG {
        due - have
    } else {
        0
    }
}

/// Las marcas de tiempo (en unidades de 100 ns) de la muestra número `samples`.
pub fn hns_of(samples: u64) -> i64 {
    (u128::from(samples) * 10_000_000 / u128::from(RATE)) as i64
}

/// Suma dos pistas muestra a muestra con saturación (nunca da la vuelta al pasarse). La más corta se completa con
/// silencio.
pub fn mix(a: &[i16], b: &[i16]) -> Vec<i16> {
    (0..a.len().max(b.len()))
        .map(|i| {
            let s = i32::from(a.get(i).copied().unwrap_or(0)) + i32::from(b.get(i).copied().unwrap_or(0));
            s.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
        })
        .collect()
}

/// Saca de `pending` las primeras `n` muestras; si hay menos, se completa con silencio (el sonido del equipo no
/// entrega nada mientras no suena nada, y la pista tiene que seguir el reloj del vídeo).
pub fn take_padded(pending: &mut Vec<i16>, n: usize) -> Vec<i16> {
    let k = n.min(pending.len());
    let mut out: Vec<i16> = pending.drain(..k).collect();
    out.resize(n, 0);
    out
}

/// Si una pista va más de un segundo por delante de lo que toca, se tira lo más viejo: así un retraso no crece sin fin.
pub fn trim_backlog(pending: &mut Vec<i16>, n: usize) {
    let keep = n + RATE as usize;
    if pending.len() > keep {
        let extra = pending.len() - keep;
        pending.drain(..extra);
    }
}

// ---------- El micrófono de Windows ----------

#[cfg(not(windows))]
pub fn start() -> Result<Mic, MicError> {
    Err(MicError::Other("Windows es el único sistema donde se graba la voz".into()))
}

#[cfg(not(windows))]
pub fn start_system() -> Result<Mic, MicError> {
    Err(MicError::Other("Windows es el único sistema donde se graba el sonido del equipo".into()))
}

#[cfg(windows)]
pub use imp::{start, start_system};

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::mpsc::{channel, Sender};
    use std::time::Duration;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, WAVEFORMATEX,
        WAVE_FORMAT_PCM,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

    fn classify(x: &windows::core::Error) -> MicError {
        MicError::classify(x.code().0 as u32, &x.message())
    }

    /// Abre el micrófono por defecto (o, con `system`, el altavoz predeterminado en modo «loopback»: lo que suena en
    /// el equipo) y empieza a capturar. Devuelve el cliente y el lector.
    unsafe fn open(system: bool) -> Result<(IAudioClient, IAudioCaptureClient), MicError> {
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|x| classify(&x))?;
            let device = enumerator.GetDefaultAudioEndpoint(if system { eRender } else { eCapture }, eConsole).map_err(|x| classify(&x))?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(|x| classify(&x))?;
            let format = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16,
                nChannels: 1,
                nSamplesPerSec: RATE,
                nAvgBytesPerSec: RATE * 2,
                nBlockAlign: 2,
                wBitsPerSample: 16,
                cbSize: 0,
            };
            // Que Windows convierta lo que dé el micrófono a lo pedido, y con búfer de un segundo.
            // En «loopback» Windows solo entrega datos mientras algo suena: la mezcla con el vídeo rellena los huecos.
            let flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY | if system { AUDCLNT_STREAMFLAGS_LOOPBACK } else { 0 };
            client
                .Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 10_000_000, 0, &format, None)
                .map_err(|x| classify(&x))?;
            let capture: IAudioCaptureClient = client.GetService().map_err(|x| classify(&x))?;
            client.Start().map_err(|x| classify(&x))?;
            Ok((client, capture))
        }
    }

    unsafe fn read_loop(capture: &IAudioCaptureClient, tx: &Sender<Vec<i16>>, stop: &AtomicBool) {
        unsafe {
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(10));
                loop {
                    let Ok(n) = capture.GetNextPacketSize() else { return };
                    if n == 0 {
                        break;
                    }
                    let (mut data, mut frames, mut flags) = (std::ptr::null_mut::<u8>(), 0u32, 0u32);
                    if capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                        return;
                    }
                    let mut v = vec![0i16; frames as usize];
                    if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 == 0 && !data.is_null() {
                        std::ptr::copy_nonoverlapping(data, v.as_mut_ptr() as *mut u8, frames as usize * 2);
                    }
                    let _ = capture.ReleaseBuffer(frames);
                    // Si ya no hay quien reciba (la grabación terminó), se acaba.
                    if tx.send(v).is_err() {
                        return;
                    }
                }
            }
        }
    }

    /// Arranca el micrófono por defecto. Tarda como mucho unos segundos en saber si funciona.
    pub fn start() -> Result<Mic, MicError> {
        start_source(false)
    }

    /// Arranca la captura de lo que suena por el altavoz predeterminado.
    pub fn start_system() -> Result<Mic, MicError> {
        start_source(true)
    }

    fn start_source(system: bool) -> Result<Mic, MicError> {
        let (tx, rx) = channel::<Vec<i16>>();
        let (ready_tx, ready_rx) = channel::<Result<(), MicError>>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let handle = std::thread::Builder::new()
            .name(if system { "faultrec-sysaudio" } else { "faultrec-mic" }.into())
            .spawn(move || {
                // SAFETY: COM se inicia y se cierra en este mismo hilo, y todo lo que se crea se suelta antes.
                unsafe {
                    let com = CoInitializeEx(None, COINIT_MULTITHREADED);
                    match open(system) {
                        Ok((client, capture)) => {
                            let _ = ready_tx.send(Ok(()));
                            read_loop(&capture, &tx, &stop2);
                            let _ = client.Stop();
                            drop(capture);
                            drop(client);
                        }
                        Err(e) => {
                            let _ = ready_tx.send(Err(e));
                        }
                    }
                    if com.is_ok() {
                        CoUninitialize();
                    }
                }
            })
            .map_err(|e| MicError::Other(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Mic { rx, stop, handle: Some(handle) }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                Err(MicError::Other("Windows tardó demasiado en abrir el audio".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_volumen_va_de_0_a_100() {
        assert_eq!(level(&[]), 0);
        assert_eq!(level(&[0; 4800]), 0);
        assert_eq!(level(&[i16::MAX; 4800]), 100);
        assert_eq!(level(&[i16::MIN; 4800]), 100);
        // Una onda cuadrada de la mitad de alto: un 50 % de eficaz, raíz de 0,5 = 71.
        assert_eq!(level(&[16384, -16384, 16384, -16384]), 71);
        // Una voz normal (un 5 % del máximo) se ve claramente; el ruido de fondo (0,1 %) casi nada.
        let voice = level(&[1638, -1638, 1638, -1638]);
        let noise = level(&[33, -33, 33, -33]);
        assert!((18..=26).contains(&voice), "{voice}");
        assert!(noise <= 4, "{noise}");
        // Siempre sube con el volumen.
        let mut last = 0;
        for amp in (0..=32767).step_by(500) {
            let l = level(&[amp as i16, -(amp as i16)]);
            assert!(l >= last, "{amp}");
            last = l;
        }
    }

    #[test]
    fn el_medidor_da_un_valor_por_decima_de_segundo() {
        let mut m = VoiceMeter::default();
        // 0,2 s de silencio y 0,3 s de sonido, en trozos que no coinciden con las décimas.
        m.push(&[0; 4000]);
        m.push(&[0; 5600]);
        m.push(&vec![i16::MAX; 14_400]);
        // 24 000 muestras = 5 décimas justas.
        assert_eq!(m.levels.len(), 5);
        let l = m.finish();
        assert_eq!(l, vec![0, 0, 100, 100, 100]);
        // Lo que sobra cuenta como una décima más; sin nada, no hay nada.
        let mut m = VoiceMeter::default();
        m.push(&vec![i16::MAX; TENTH + 100]);
        assert_eq!(m.finish(), vec![100, 100]);
        assert!(VoiceMeter::default().finish().is_empty());
    }

    #[test]
    fn el_silencio_solo_se_mete_si_el_audio_va_muy_atrasado() {
        let due = 48_000u64;
        // Al día, o con menos de 200 ms de retraso: no se toca.
        assert_eq!(silence_needed(48_000, 0, due), 0);
        assert_eq!(silence_needed(40_000, 0, due), 0);
        assert_eq!(silence_needed(38_400, 0, due), 0);
        // Con más de 200 ms por detrás se pone al día del todo.
        assert_eq!(silence_needed(38_000, 0, due), 10_000);
        assert_eq!(silence_needed(0, 0, due), 48_000);
        // Lo que está esperando cuenta como audio que hay.
        assert_eq!(silence_needed(20_000, 10_000, due), 18_000);
        assert_eq!(silence_needed(30_000, 10_000, due), 0);
        assert_eq!(silence_needed(30_000, 18_000, due), 0);
        // Un audio adelantado no se recorta.
        assert_eq!(silence_needed(60_000, 0, due), 0);
    }

    #[test]
    fn las_marcas_de_tiempo_salen_de_las_muestras() {
        assert_eq!(hns_of(0), 0);
        assert_eq!(hns_of(48_000), 10_000_000);
        assert_eq!(hns_of(4_800), 1_000_000);
        assert_eq!(hns_of(24_000), 5_000_000);
    }

    #[test]
    fn los_fallos_del_microfono_se_cuentan_en_claro() {
        assert_eq!(MicError::classify(0x8007_0005, "x"), MicError::Denied);
        assert_eq!(MicError::classify(0x8007_0490, "x"), MicError::NoDevice);
        assert_eq!(MicError::classify(1, "dispositivo en uso"), MicError::Other("dispositivo en uso".into()));
        assert_eq!(MicError::NoDevice.note(), "No se grabó la voz: no hay ningún micrófono.");
        assert_eq!(MicError::Denied.note(), "No se grabó la voz: Windows no deja usar el micrófono a las aplicaciones (Configuración → Privacidad y seguridad → Micrófono).");
        assert_eq!(MicError::Other("dispositivo en uso.".into()).note(), "No se grabó la voz: dispositivo en uso.");
        assert_eq!(MicError::Other("fallo ".into()).note(), "No se grabó la voz: fallo.");
    }

    #[test]
    fn la_mezcla_suma_con_saturacion_y_completa_la_mas_corta() {
        assert_eq!(mix(&[100, -100, 5], &[1, 2]), vec![101, -98, 5]);
        assert_eq!(mix(&[i16::MAX, i16::MIN], &[1000, -1000]), vec![i16::MAX, i16::MIN]);
        assert_eq!(mix(&[], &[3, 4]), vec![3, 4]);
        assert!(mix(&[], &[]).is_empty());
    }

    #[test]
    fn se_saca_lo_pedido_y_se_rellena_con_silencio() {
        let mut p = vec![1, 2, 3, 4, 5];
        assert_eq!(take_padded(&mut p, 3), vec![1, 2, 3]);
        assert_eq!(p, vec![4, 5]);
        assert_eq!(take_padded(&mut p, 4), vec![4, 5, 0, 0]);
        assert!(p.is_empty());
        assert_eq!(take_padded(&mut p, 2), vec![0, 0]);
    }

    #[test]
    fn el_atraso_no_crece_sin_fin() {
        let mut p = vec![7i16; 200_000];
        trim_backlog(&mut p, 4_800);
        assert_eq!(p.len(), 4_800 + RATE as usize);
        let mut q = vec![1i16; 100];
        trim_backlog(&mut q, 4_800);
        assert_eq!(q.len(), 100);
    }

    #[test]
    fn el_fallo_del_sonido_del_equipo_se_cuenta() {
        assert_eq!(MicError::NoDevice.note_system(), "No se grabó el sonido del equipo: no hay ninguna salida de audio.");
        assert_eq!(MicError::Other("x.".into()).note_system(), "No se grabó el sonido del equipo: x.");
    }
}
