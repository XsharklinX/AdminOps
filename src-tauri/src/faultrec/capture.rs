//! La grabación en sí: captura de pantalla (un monitor, una ventana o una zona de un monitor) con
//! `Windows.Graphics.Capture` y codificación a MP4 (H.264 y, si se pide la voz, una segunda pista AAC) con el
//! codificador de Media Foundation que trae Windows. Sin ffmpeg ni nada que instalar.
//!
//! Todo ocurre en un hilo propio (COM multihilo, `MFStartup`/`MFShutdown` emparejados). Cada décima de
//! segundo se toma el último fotograma que entregó Windows (si la pantalla no cambió, se repite el
//! anterior, porque Windows solo entrega fotogramas cuando algo cambia) y se escribe con la marca de
//! tiempo real. Al monitor se le deja su borde amarillo de Windows: es el aviso al usuario de que se graba.
//! La voz (si la hay) se escribe en cada décima con lo que haya llegado del micrófono, con su propio tiempo.

use super::mic::{self, Mic, VoiceMeter};
use super::{Region, Started};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Fotogramas por segundo del vídeo.
pub const FPS: u32 = 10;
/// Ancho máximo del vídeo.
pub const MAX_WIDTH: u32 = 1920;
/// Bits por segundo del H.264.
pub const BITRATE: u32 = 2_500_000;
/// Lo que se espera al primer fotograma antes de rendirse.
pub const FIRST_FRAME_WAIT: Duration = Duration::from_secs(5);

/// Qué grabar y dónde.
pub struct Job {
    /// Nombre GDI del monitor (`\\.\DISPLAY1`); vacío = el principal. Con `window` no se usa.
    pub gdi: String,
    /// Grabar solo esta ventana (su `HWND`) en vez de un monitor.
    pub window: Option<isize>,
    /// Grabar solo esta zona del monitor (píxeles desde su esquina superior izquierda).
    pub region: Option<Region>,
    /// El micrófono ya abierto, si se graba la voz.
    pub mic: Option<Mic>,
    pub out: PathBuf,
    pub stop: Arc<AtomicBool>,
    /// Se rellena al llegar el primer fotograma: de ahí cuentan los segundos del vídeo y de la línea de tiempo.
    pub first: Arc<Mutex<Option<Started>>>,
    pub max: Duration,
}

/// Lo que dejó la grabación.
#[derive(Debug, Clone, PartialEq)]
pub struct Done {
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    /// El monitor se desconectó (o la ventana se cerró) a mitad: el vídeo termina ahí.
    pub lost: bool,
    /// El volumen de cada décima de segundo (de 0 a 100); vacío sin voz.
    pub voice: Vec<u8>,
    /// El MP4 lleva de verdad la pista de voz.
    pub with_voice: bool,
}

/// Lo menos que mide, por lado, una zona que se graba.
pub const MIN_REGION: u32 = 64;

/// Sin fotogramas nuevos durante este tiempo se mira si el monitor sigue conectado.
pub const QUIET_CHECK: Duration = Duration::from_secs(3);

pub const UNSUPPORTED: &str = "Este Windows no permite grabar la pantalla (hace falta Windows 10 1903 o posterior).";

// ---------- Tamaño y escalado (puro, con pruebas) ----------

/// La zona que de verdad se graba de una imagen de `content_w`×`content_h`: se recorta a lo que cabe y devuelve
/// `(x, y, ancho, alto)`. `None` si queda fuera o por debajo del mínimo (64×64).
pub fn crop_rect(content_w: u32, content_h: u32, region: Region) -> Option<(u32, u32, u32, u32)> {
    let (x0, y0) = (i64::from(region.x).max(0), i64::from(region.y).max(0));
    let x1 = (i64::from(region.x) + i64::from(region.w)).min(i64::from(content_w));
    let y1 = (i64::from(region.y) + i64::from(region.h)).min(i64::from(content_h));
    if x1 - x0 < i64::from(MIN_REGION) || y1 - y0 < i64::from(MIN_REGION) {
        return None;
    }
    Some((x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// Tamaño del vídeo: el de la pantalla hasta 1920 de ancho (manteniendo la proporción) y siempre par.
pub fn output_size(w: u32, h: u32) -> (u32, u32) {
    output_size_max(w, h, MAX_WIDTH)
}

/// Lo mismo con otro ancho máximo (Vigilar usa 1280).
pub fn output_size_max(w: u32, h: u32, max_width: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let max_width = max_width.max(2);
    let (ow, oh) = if w > max_width { (max_width, ((u64::from(h) * u64::from(max_width) + u64::from(w) / 2) / u64::from(w)) as u32) } else { (w, h) };
    ((ow & !1).max(2), (oh & !1).max(2))
}

/// Pasa una imagen BGRA (`sw`×`sh`, con `stride` bytes por fila) a `dw`×`dh` en `dst` (BGRA, sin hueco entre filas).
/// Al reducir se promedian hasta 3×3 puntos de cada zona: el texto sigue legible. El alfa queda opaco.
pub fn scale_bgra(src: &[u8], sw: u32, sh: u32, stride: usize, dw: u32, dh: u32, dst: &mut Vec<u8>) {
    let (sw, sh, dw, dh) = (sw as usize, sh as usize, dw as usize, dh as usize);
    dst.resize(dw * dh * 4, 0);
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 || src.len() < stride * (sh - 1) + sw * 4 {
        return;
    }
    if sw == dw && sh == dh {
        for y in 0..dh {
            let row = &src[y * stride..y * stride + sw * 4];
            let out = &mut dst[y * dw * 4..(y + 1) * dw * 4];
            out.copy_from_slice(row);
            for px in out.chunks_exact_mut(4) {
                px[3] = 255;
            }
        }
        return;
    }
    let (kx, ky) = (sw.div_ceil(dw).clamp(1, 3), sh.div_ceil(dh).clamp(1, 3));
    // Las columnas de origen de cada columna de destino, una sola vez.
    let cols: Vec<[usize; 3]> = (0..dw)
        .map(|dx| {
            let (x0, x1) = (dx * sw / dw, ((dx + 1) * sw / dw).max(dx * sw / dw + 1).min(sw));
            let mut c = [0usize; 3];
            for (i, slot) in c.iter_mut().enumerate().take(kx) {
                *slot = (x0 + ((2 * i + 1) * (x1 - x0)) / (2 * kx)).min(sw - 1);
            }
            c
        })
        .collect();
    for dy in 0..dh {
        let (y0, y1) = (dy * sh / dh, ((dy + 1) * sh / dh).max(dy * sh / dh + 1).min(sh));
        let mut rows = [0usize; 3];
        for (i, slot) in rows.iter_mut().enumerate().take(ky) {
            *slot = (y0 + ((2 * i + 1) * (y1 - y0)) / (2 * ky)).min(sh - 1);
        }
        for (dx, c) in cols.iter().enumerate() {
            let (mut b, mut g, mut r) = (0u32, 0u32, 0u32);
            for &sy in rows.iter().take(ky) {
                for &sx in c.iter().take(kx) {
                    let o = sy * stride + sx * 4;
                    b += u32::from(src[o]);
                    g += u32::from(src[o + 1]);
                    r += u32::from(src[o + 2]);
                }
            }
            let n = (kx * ky) as u32;
            let o = (dy * dw + dx) * 4;
            dst[o] = (b / n) as u8;
            dst[o + 1] = (g / n) as u8;
            dst[o + 2] = (r / n) as u8;
            dst[o + 3] = 255;
        }
    }
}

#[cfg(not(windows))]
pub fn record(_job: Job) -> Result<Done, String> {
    Err("Grabar el fallo solo funciona en Windows.".into())
}

/// Mantiene vivo el COM multihilo de todo el proceso desde la primera captura hasta que AdminOps se cierra.
///
/// Sin esto, cuando el último hilo que lo usaba (la captura, el vigilante) llama a `CoUninitialize`, Windows lo
/// desmonta mientras `Windows.Graphics.Capture` aún está soltando sus fotogramas en sus propios hilos, y el proceso
/// se cierra con 0xC0000005 al parar (pasaba 3 de cada 20 veces). `CoIncrementMTAUsage` es lo que Windows ofrece
/// para esto; la «galleta» no se devuelve nunca, a propósito.
#[cfg(windows)]
pub fn keep_mta_alive() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: sin punteros de entrada; si falla, todo sigue como antes (cada hilo con su CoInitializeEx).
        // La galleta es un simple identificador: con no pasarla nunca a CoDecrementMTAUsage basta.
        if let Err(e) = unsafe { windows::Win32::System::Com::CoIncrementMTAUsage() } {
            log::warn!("Grabar el fallo: no se pudo fijar el COM multihilo: {e}");
        }
    });
}

#[cfg(not(windows))]
pub fn keep_mta_alive() {}

#[cfg(windows)]
pub use imp::{record, Grabber};
#[cfg(all(windows, test))]
pub(crate) use imp::video_info;

#[cfg(windows)]
mod imp {
    use super::super::win::{self, monitor_handle};
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::atomic::Ordering;
    use std::time::Instant;
    use windows::core::{factory, Interface, PCWSTR};
    use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
    use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
    use windows::Graphics::DirectX::DirectXPixelFormat;
    use windows::Graphics::SizeInt32;
    use windows::Win32::Foundation::{HMODULE, HWND};
    use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
        D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
    use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

    fn e(what: &str, err: windows::core::Error) -> String {
        format!("{what}: {err}")
    }

    fn pack(hi: u32, lo: u32) -> u64 {
        (u64::from(hi) << 32) | u64::from(lo)
    }

    // ---------- Codificador ----------

    struct Encoder {
        writer: IMFSinkWriter,
        stream: u32,
        /// La pista de voz (AAC), si el MP4 la lleva.
        audio: Option<u32>,
    }

    impl Encoder {
        /// Prepara el MP4. Con `hardware` se deja que Windows use el codificador de la tarjeta gráfica; con `audio`,
        /// el archivo lleva una segunda pista AAC (48 000 Hz, un canal) que se alimenta con `write_audio`.
        unsafe fn new(path: &std::path::Path, w: u32, h: u32, hardware: bool, audio: bool) -> Result<Encoder, String> {
            unsafe {
                let mut attrs: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut attrs, 3).map_err(|x| e("No se pudo preparar el codificador", x))?;
                let attrs = attrs.ok_or("No se pudo preparar el codificador.")?;
                attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, u32::from(hardware)).map_err(|x| e("Codificador", x))?;
                attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1).map_err(|x| e("Codificador", x))?;
                // El archivo se escribe con la extensión «.part»: se le dice el contenedor a mano.
                attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4).map_err(|x| e("Codificador", x))?;
                let url: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                let writer = MFCreateSinkWriterFromURL(PCWSTR(url.as_ptr()), None::<&IMFByteStream>, &attrs).map_err(|x| e("No se pudo crear el vídeo", x))?;

                let out = MFCreateMediaType().map_err(|x| e("Codificador", x))?;
                out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Codificador", x))?;
                out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(|x| e("Codificador", x))?;
                out.SetUINT32(&MF_MT_AVG_BITRATE, BITRATE).map_err(|x| e("Codificador", x))?;
                out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(|x| e("Codificador", x))?;
                out.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h)).map_err(|x| e("Codificador", x))?;
                out.SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1)).map_err(|x| e("Codificador", x))?;
                out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(|x| e("Codificador", x))?;
                let stream = writer.AddStream(&out).map_err(|x| e("No se pudo crear el vídeo H.264", x))?;

                let input = MFCreateMediaType().map_err(|x| e("Codificador", x))?;
                input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Codificador", x))?;
                input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|x| e("Codificador", x))?;
                input.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(|x| e("Codificador", x))?;
                input.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h)).map_err(|x| e("Codificador", x))?;
                input.SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1)).map_err(|x| e("Codificador", x))?;
                input.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(|x| e("Codificador", x))?;
                // Stride positivo: la imagen va de arriba abajo, como la deja la captura.
                input.SetUINT32(&MF_MT_DEFAULT_STRIDE, w * 4).map_err(|x| e("Codificador", x))?;
                writer.SetInputMediaType(stream, &input, None::<&IMFAttributes>).map_err(|x| e("El codificador no acepta la imagen", x))?;

                let audio = if audio { Some(Self::add_audio(&writer)?) } else { None };
                writer.BeginWriting().map_err(|x| e("No se pudo empezar a escribir el vídeo", x))?;
                Ok(Encoder { writer, stream, audio })
            }
        }

        /// La segunda pista: entra PCM de 16 bits, 48 000 Hz y un canal; sale AAC a 96 kbit/s.
        unsafe fn add_audio(writer: &IMFSinkWriter) -> Result<u32, String> {
            unsafe {
                let out = MFCreateMediaType().map_err(|x| e("Codificador de voz", x))?;
                out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(|x| e("Codificador de voz", x))?;
                out.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).map_err(|x| e("Codificador de voz", x))?;
                out.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(|x| e("Codificador de voz", x))?;
                out.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, mic::RATE).map_err(|x| e("Codificador de voz", x))?;
                out.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1).map_err(|x| e("Codificador de voz", x))?;
                out.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 12_000).map_err(|x| e("Codificador de voz", x))?;
                out.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 1).map_err(|x| e("Codificador de voz", x))?;
                out.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0).map_err(|x| e("Codificador de voz", x))?;
                out.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29).map_err(|x| e("Codificador de voz", x))?;
                let stream = writer.AddStream(&out).map_err(|x| e("No se pudo crear la pista de voz (AAC)", x))?;

                let input = MFCreateMediaType().map_err(|x| e("Codificador de voz", x))?;
                input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(|x| e("Codificador de voz", x))?;
                input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(|x| e("Codificador de voz", x))?;
                input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(|x| e("Codificador de voz", x))?;
                input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, mic::RATE).map_err(|x| e("Codificador de voz", x))?;
                input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1).map_err(|x| e("Codificador de voz", x))?;
                input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2).map_err(|x| e("Codificador de voz", x))?;
                input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, mic::RATE * 2).map_err(|x| e("Codificador de voz", x))?;
                writer.SetInputMediaType(stream, &input, None::<&IMFAttributes>).map_err(|x| e("El codificador no acepta la voz", x))?;
                Ok(stream)
            }
        }

        /// Como `new`, pero si el Windows de este equipo no tiene el codificador de audio, el vídeo se graba sin voz.
        unsafe fn open(path: &std::path::Path, w: u32, h: u32, hardware: bool, audio: bool) -> Result<Encoder, String> {
            unsafe {
                match Encoder::new(path, w, h, hardware, audio) {
                    Err(x) if audio => {
                        log::info!("Grabar el fallo: no se pudo preparar la pista de voz ({x}); se graba sin voz");
                        let _ = std::fs::remove_file(path);
                        Encoder::new(path, w, h, hardware, false)
                    }
                    r => r,
                }
            }
        }

        /// Escribe un trozo de voz (PCM de 16 bits) que empieza en `time` (unidades de 100 ns).
        unsafe fn write_audio(&self, pcm: &[i16], time: i64) -> Result<(), String> {
            unsafe {
                let Some(stream) = self.audio else { return Ok(()) };
                let bytes = pcm.len() * 2;
                let buffer = MFCreateMemoryBuffer(bytes as u32).map_err(|x| e("Memoria de la voz", x))?;
                let mut ptr: *mut u8 = std::ptr::null_mut();
                buffer.Lock(&mut ptr, None, None).map_err(|x| e("Memoria de la voz", x))?;
                std::ptr::copy_nonoverlapping(pcm.as_ptr() as *const u8, ptr, bytes);
                buffer.Unlock().map_err(|x| e("Memoria de la voz", x))?;
                buffer.SetCurrentLength(bytes as u32).map_err(|x| e("Memoria de la voz", x))?;
                let sample = MFCreateSample().map_err(|x| e("Voz", x))?;
                sample.AddBuffer(&buffer).map_err(|x| e("Voz", x))?;
                sample.SetSampleTime(time).map_err(|x| e("Voz", x))?;
                sample.SetSampleDuration(mic::hns_of(pcm.len() as u64)).map_err(|x| e("Voz", x))?;
                self.writer.WriteSample(stream, &sample).map_err(|x| e("No se pudo escribir la voz", x))
            }
        }

        unsafe fn write(&self, bgra: &[u8], time: i64, duration: i64) -> Result<(), String> {
            unsafe {
                let buffer = MFCreateMemoryBuffer(bgra.len() as u32).map_err(|x| e("Memoria del vídeo", x))?;
                let mut ptr: *mut u8 = std::ptr::null_mut();
                buffer.Lock(&mut ptr, None, None).map_err(|x| e("Memoria del vídeo", x))?;
                std::ptr::copy_nonoverlapping(bgra.as_ptr(), ptr, bgra.len());
                buffer.Unlock().map_err(|x| e("Memoria del vídeo", x))?;
                buffer.SetCurrentLength(bgra.len() as u32).map_err(|x| e("Memoria del vídeo", x))?;
                let sample = MFCreateSample().map_err(|x| e("Fotograma", x))?;
                sample.AddBuffer(&buffer).map_err(|x| e("Fotograma", x))?;
                sample.SetSampleTime(time).map_err(|x| e("Fotograma", x))?;
                sample.SetSampleDuration(duration).map_err(|x| e("Fotograma", x))?;
                self.writer.WriteSample(self.stream, &sample).map_err(|x| e("No se pudo escribir un fotograma", x))
            }
        }

        unsafe fn finish(self) -> Result<(), String> {
            unsafe { self.writer.Finalize().map_err(|x| e("No se pudo cerrar el vídeo", x)) }
        }
    }

    /// Crea el codificador: primero con la ayuda de la tarjeta gráfica y, si no puede, por programa.
    /// Devuelve también si es el de la tarjeta gráfica.
    unsafe fn open_encoder(path: &std::path::Path, w: u32, h: u32, audio: bool) -> Result<(Encoder, bool), String> {
        unsafe {
            match Encoder::open(path, w, h, true, audio) {
                Ok(enc) => Ok((enc, true)),
                Err(first) => {
                    log::info!("Grabar el fallo: el codificador por hardware no sirvió ({first}); se prueba por programa");
                    let _ = std::fs::remove_file(path);
                    Encoder::open(path, w, h, false, audio).map(|enc| (enc, false))
                }
            }
        }
    }

    // ---------- Captura ----------

    unsafe fn make_device() -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
        unsafe {
            let mut last = String::new();
            for kind in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
                let mut device: Option<ID3D11Device> = None;
                let mut ctx: Option<ID3D11DeviceContext> = None;
                match D3D11CreateDevice(None::<&IDXGIAdapter>, kind, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut ctx)) {
                    Ok(()) => {
                        if let (Some(d), Some(c)) = (device, ctx) {
                            return Ok((d, c));
                        }
                    }
                    Err(x) => last = x.to_string(),
                }
            }
            Err(format!("No se pudo preparar la tarjeta gráfica para grabar: {last}"))
        }
    }

    unsafe fn staging_texture(device: &ID3D11Device, w: u32, h: u32) -> Result<ID3D11Texture2D, String> {
        unsafe {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut tex: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&desc, None, Some(&mut tex)).map_err(|x| e("No se pudo reservar memoria de imagen", x))?;
            tex.ok_or_else(|| "No se pudo reservar memoria de imagen.".to_string())
        }
    }

    fn now_unix() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
    }

    /// Graba hasta que se pida parar o pase `max`. Se llama desde un hilo propio.
    pub fn record(job: Job) -> Result<Done, String> {
        keep_mta_alive();
        // SAFETY: COM y Media Foundation se inician y se cierran en este mismo hilo; todo lo que se crea
        // dentro de `record_inner` se suelta antes de `MFShutdown`.
        unsafe {
            let com = CoInitializeEx(None, COINIT_MULTITHREADED);
            let started = MFStartup(MF_VERSION, MFSTARTUP_FULL);
            let result = match &started {
                Ok(()) => record_inner(&job),
                Err(x) => Err(e("No se pudo iniciar el codificador de vídeo de Windows", x.clone())),
            };
            if started.is_ok() {
                let _ = MFShutdown();
            }
            // El micrófono se suelta antes de cerrar COM del todo (tiene su propio hilo y su propio COM).
            if let Some(m) = job.mic {
                m.finish();
            }
            if com.is_ok() {
                CoUninitialize();
            }
            result
        }
    }

    unsafe fn record_inner(job: &Job) -> Result<Done, String> {
        unsafe {
            if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
                return Err(UNSUPPORTED.into());
            }
            let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(|x| e(UNSUPPORTED, x))?;
            let item: GraphicsCaptureItem = match job.window {
                Some(h) => {
                    if !win::is_window(h) {
                        return Err("Esa ventana ya no está abierta.".into());
                    }
                    interop.CreateForWindow(HWND(h as *mut std::ffi::c_void)).map_err(|x| e("No se pudo capturar esa ventana", x))?
                }
                None => interop.CreateForMonitor(monitor_handle(&job.gdi)?).map_err(|x| e("No se pudo capturar ese monitor", x))?,
            };
            let (device, ctx) = make_device()?;
            let dxgi: IDXGIDevice = device.cast().map_err(|x| e("Tarjeta gráfica", x))?;
            let d3d: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi).map_err(|x| e("Tarjeta gráfica", x))?.cast().map_err(|x| e("Tarjeta gráfica", x))?;
            let format = DirectXPixelFormat::B8G8R8A8UIntNormalized;
            let size = item.Size().map_err(|x| e("Tamaño del monitor", x))?;
            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&d3d, format, 3, size).map_err(|x| e("No se pudo empezar la captura", x))?;
            let session = pool.CreateCaptureSession(&item).map_err(|x| e("No se pudo empezar la captura", x))?;
            let _ = session.SetIsCursorCaptureEnabled(true);
            session.StartCapture().map_err(|x| e("No se pudo empezar la captura", x))?;
            let result = capture_loop(job, &pool, &d3d, &device, &ctx, size);
            // Primero se para la captura y luego se cierra lo demás.
            let _ = session.Close();
            let _ = pool.Close();
            result
        }
    }

    unsafe fn capture_loop(job: &Job, pool: &Direct3D11CaptureFramePool, d3d: &IDirect3DDevice, device: &ID3D11Device, ctx: &ID3D11DeviceContext, size: SizeInt32) -> Result<Done, String> {
        unsafe {
            let format = DirectXPixelFormat::B8G8R8A8UIntNormalized;
            // El tamaño del vídeo se fija con el del primer fotograma; si el monitor cambia, se escala a él.
            let (base_w, base_h) = match job.region {
                Some(r) => crop_rect(size.Width.max(1) as u32, size.Height.max(1) as u32, r).map(|(_, _, w, h)| (w, h)).ok_or("La zona es demasiado pequeña.")?,
                None => (size.Width.max(2) as u32, size.Height.max(2) as u32),
            };
            let (out_w, out_h) = output_size(base_w, base_h);
            let (mut encoder, mut hardware) = open_encoder(&job.out, out_w, out_h, job.mic.is_some())?;
            // La voz: lo que ya se escribió (en muestras), lo que espera, y el volumen de cada décima.
            let mut audio_samples: u64 = 0;
            let mut audio_failed = false;
            let mut pending: Vec<i16> = Vec::new();
            let mut meter = VoiceMeter::default();
            let tick = Duration::from_millis(1000 / u64::from(FPS));
            let tick_hns = (tick.as_nanos() / 100) as i64;

            let mut pool_size = size;
            let mut staging: Option<(ID3D11Texture2D, u32, u32)> = None;
            let mut image: Vec<u8> = Vec::new();
            let mut have_image = false;
            let mut start: Option<Instant> = None;
            let waiting = Instant::now();
            let mut last_hns: i64 = -1;
            let mut frames: u64 = 0;
            let mut failure: Option<String> = None;
            let mut n: u32 = 0;
            let mut lost = false;
            let mut last_new = Instant::now();
            let mut last_check = Instant::now();

            loop {
                // Lo más reciente que entregó Windows desde la última vez.
                let mut latest = None;
                while let Ok(frame) = pool.TryGetNextFrame() {
                    if let Some(old) = latest.replace(frame) {
                        let _ = old.Close();
                    }
                }
                if let Some(frame) = latest {
                    let copied = (|| -> Result<(), String> {
                        let content = frame.ContentSize().map_err(|x| e("Fotograma", x))?;
                        let surface = frame.Surface().map_err(|x| e("Fotograma", x))?;
                        let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(|x| e("Fotograma", x))?;
                        let texture: ID3D11Texture2D = access.GetInterface().map_err(|x| e("Fotograma", x))?;
                        let mut desc = D3D11_TEXTURE2D_DESC::default();
                        texture.GetDesc(&mut desc);
                        if staging.as_ref().is_none_or(|(_, w, h)| *w != desc.Width || *h != desc.Height) {
                            staging = Some((staging_texture(device, desc.Width, desc.Height)?, desc.Width, desc.Height));
                        }
                        let (tex, _, _) = staging.as_ref().ok_or("Sin memoria de imagen.")?;
                        ctx.CopyResource(tex, &texture);
                        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                        ctx.Map(tex, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).map_err(|x| e("No se pudo leer la imagen", x))?;
                        let (cw, ch) = ((content.Width.max(1) as u32).min(desc.Width), (content.Height.max(1) as u32).min(desc.Height));
                        let src = std::slice::from_raw_parts(mapped.pData as *const u8, mapped.RowPitch as usize * desc.Height as usize);
                        // Con una zona, solo se escala ese trozo: el puntero de origen se desplaza a su esquina.
                        let (sx, sy, sw, sh) = job.region.and_then(|r| crop_rect(cw, ch, r)).unwrap_or((0, 0, cw, ch));
                        let offset = (sy as usize * mapped.RowPitch as usize + sx as usize * 4).min(src.len());
                        scale_bgra(&src[offset..], sw, sh, mapped.RowPitch as usize, out_w, out_h, &mut image);
                        ctx.Unmap(tex, 0);
                        // Si el monitor cambió de tamaño, la reserva de Windows se ajusta al nuevo.
                        if content.Width != pool_size.Width || content.Height != pool_size.Height {
                            pool_size = content;
                            let _ = pool.Recreate(d3d, format, 3, content);
                        }
                        Ok(())
                    })();
                    let _ = frame.Close();
                    match copied {
                        Ok(()) => {
                            have_image = true;
                            last_new = Instant::now();
                            if start.is_none() {
                                let now = Instant::now();
                                start = Some(now);
                                n = 0;
                                *job.first.lock().unwrap_or_else(|x| x.into_inner()) = Some(Started { at: now, unix: now_unix() });
                                // La voz que llegó antes del primer fotograma no cuenta: el audio empieza con el vídeo.
                                if let Some(m) = &job.mic {
                                    while m.rx.try_recv().is_ok() {}
                                }
                            }
                        }
                        Err(x) => {
                            failure = Some(x);
                            break;
                        }
                    }
                }

                if let Some(t0) = start {
                    // Un fotograma por décima de segundo, con la marca de tiempo real (nunca hacia atrás).
                    let mut hns = (t0.elapsed().as_nanos() / 100) as i64;
                    if hns <= last_hns {
                        hns = last_hns + 1;
                    }
                    let mut wrote = encoder.write(&image, hns, tick_hns);
                    if let (Err(x), true, 0) = (&wrote, hardware, frames) {
                        // Hay codificadores de tarjeta que se dejan crear y fallan al primer fotograma: por programa.
                        log::info!("Grabar el fallo: el codificador por hardware falló al escribir ({x}); se prueba por programa");
                        // Primero se suelta el de la tarjeta: tiene abierto el mismo archivo.
                        let _ = encoder.finish();
                        let _ = std::fs::remove_file(&job.out);
                        encoder = Encoder::open(&job.out, out_w, out_h, false, job.mic.is_some())?;
                        hardware = false;
                        wrote = encoder.write(&image, hns, tick_hns);
                    }
                    if let Err(x) = wrote {
                        failure = Some(x);
                        break;
                    }
                    last_hns = hns;
                    frames += 1;

                    // La voz de esta décima, con su propio tiempo (el de sus muestras). Si el micrófono se queda
                    // atrás (quitado, bloqueado), se rellena con silencio: el codificador espera a las dos pistas.
                    if let (Some(m), true) = (&job.mic, encoder.audio.is_some() && !audio_failed) {
                        while let Ok(chunk) = m.rx.try_recv() {
                            pending.extend_from_slice(&chunk);
                        }
                        let due = (t0.elapsed().as_nanos() * u128::from(mic::RATE) / 1_000_000_000) as u64;
                        let gap = mic::silence_needed(audio_samples, pending.len() as u64, due) as usize;
                        if gap > 0 {
                            let mut padded = vec![0i16; gap];
                            padded.append(&mut pending);
                            pending = padded;
                        }
                        if !pending.is_empty() {
                            match encoder.write_audio(&pending, mic::hns_of(audio_samples)) {
                                Ok(()) => {
                                    audio_samples += pending.len() as u64;
                                    meter.push(&pending);
                                }
                                Err(x) => {
                                    // Sin voz, pero el vídeo sigue.
                                    log::warn!("Grabar el fallo: se dejó de grabar la voz: {x}");
                                    audio_failed = true;
                                }
                            }
                            pending.clear();
                        }
                    }
                } else if waiting.elapsed() > FIRST_FRAME_WAIT && !have_image {
                    failure = Some(if job.window.is_some() { "Windows no entregó imagen de esa ventana (¿está oculta o protegida?)." } else { "Windows no entregó imagen de ese monitor (¿está apagado o protegido?)." }.into());
                    break;
                }

                if job.stop.load(Ordering::SeqCst) || start.is_some_and(|t0| t0.elapsed() >= job.max) {
                    break;
                }
                // Windows solo entrega fotogramas cuando algo cambia; si lleva rato sin hacerlo, se mira si el
                // monitor (o la ventana) sigue ahí: si no, el vídeo se queda congelado y es mejor parar y decirlo.
                if (job.window.is_some() || !job.gdi.is_empty()) && start.is_some() && last_new.elapsed() >= QUIET_CHECK && last_check.elapsed() >= Duration::from_secs(1) {
                    last_check = Instant::now();
                    let gone = match job.window {
                        Some(h) => !win::is_window(h),
                        None => monitor_handle(&job.gdi).is_err(),
                    };
                    if gone {
                        lost = true;
                        break;
                    }
                }
                // Hasta el siguiente fotograma, sin acumular retraso (y atento a la orden de parar).
                n = n.saturating_add(1);
                let next = start.unwrap_or(waiting) + tick * n;
                loop {
                    let now = Instant::now();
                    if now >= next || job.stop.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep((next - now).min(Duration::from_millis(20)));
                }
            }

            let duration_ms = start.map_or(0, |t0| t0.elapsed().as_millis() as u64);
            match failure {
                Some(msg) => {
                    let _ = encoder.finish();
                    Err(msg)
                }
                None => {
                    if frames == 0 {
                        let _ = encoder.finish();
                        return Err("No se grabó ningún fotograma.".into());
                    }
                    encoder.finish()?;
                    let with_voice = audio_samples > 0;
                    Ok(Done { duration_ms, width: out_w, height: out_h, lost, voice: if with_voice { meter.finish() } else { Vec::new() }, with_voice })
                }
            }
        }
    }

    // ---------- Capturar sin escribir (Vigilar) ----------

    /// La captura de un monitor entero sin codificar nada: da el último fotograma ya escalado (BGRA) cuando Windows
    /// entrega uno nuevo. Hay que crearla, usarla y soltarla en un mismo hilo con COM iniciado.
    pub struct Grabber {
        pool: Direct3D11CaptureFramePool,
        session: GraphicsCaptureSession,
        d3d: IDirect3DDevice,
        device: ID3D11Device,
        ctx: ID3D11DeviceContext,
        pool_size: SizeInt32,
        staging: Option<(ID3D11Texture2D, u32, u32)>,
        pub out_w: u32,
        pub out_h: u32,
        /// El último fotograma, escalado a `out_w`×`out_h` (BGRA, sin hueco entre filas).
        pub image: Vec<u8>,
        /// Ya hay un fotograma en `image`.
        pub have_image: bool,
        pub gdi: String,
    }

    impl Grabber {
        /// Empieza a capturar el monitor `gdi` (vacío = el principal) con un ancho de vídeo de como mucho `max_width`.
        pub unsafe fn open_monitor(gdi: &str, max_width: u32) -> Result<Grabber, String> {
            unsafe {
                if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
                    return Err(UNSUPPORTED.into());
                }
                let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(|x| e(UNSUPPORTED, x))?;
                let item: GraphicsCaptureItem = interop.CreateForMonitor(monitor_handle(gdi)?).map_err(|x| e("No se pudo capturar ese monitor", x))?;
                let (device, ctx) = make_device()?;
                let dxgi: IDXGIDevice = device.cast().map_err(|x| e("Tarjeta gráfica", x))?;
                let d3d: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi).map_err(|x| e("Tarjeta gráfica", x))?.cast().map_err(|x| e("Tarjeta gráfica", x))?;
                let size = item.Size().map_err(|x| e("Tamaño del monitor", x))?;
                let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&d3d, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size).map_err(|x| e("No se pudo empezar la captura", x))?;
                let session = pool.CreateCaptureSession(&item).map_err(|x| e("No se pudo empezar la captura", x))?;
                let _ = session.SetIsCursorCaptureEnabled(true);
                session.StartCapture().map_err(|x| e("No se pudo empezar la captura", x))?;
                let (out_w, out_h) = output_size_max(size.Width.max(2) as u32, size.Height.max(2) as u32, max_width);
                Ok(Grabber { pool, session, d3d, device, ctx, pool_size: size, staging: None, out_w, out_h, image: Vec::new(), have_image: false, gdi: gdi.to_string() })
            }
        }

        /// Toma lo más reciente que entregó Windows. `true` si había un fotograma nuevo (ya está en `image`).
        pub unsafe fn poll(&mut self) -> Result<bool, String> {
            unsafe {
                let mut latest = None;
                while let Ok(frame) = self.pool.TryGetNextFrame() {
                    if let Some(old) = latest.replace(frame) {
                        let _ = old.Close();
                    }
                }
                let Some(frame) = latest else { return Ok(false) };
                let copied = (|| -> Result<(), String> {
                    let content = frame.ContentSize().map_err(|x| e("Fotograma", x))?;
                    let surface = frame.Surface().map_err(|x| e("Fotograma", x))?;
                    let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(|x| e("Fotograma", x))?;
                    let texture: ID3D11Texture2D = access.GetInterface().map_err(|x| e("Fotograma", x))?;
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    texture.GetDesc(&mut desc);
                    if self.staging.as_ref().is_none_or(|(_, w, h)| *w != desc.Width || *h != desc.Height) {
                        self.staging = Some((staging_texture(&self.device, desc.Width, desc.Height)?, desc.Width, desc.Height));
                    }
                    let (tex, _, _) = self.staging.as_ref().ok_or("Sin memoria de imagen.")?;
                    self.ctx.CopyResource(tex, &texture);
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    self.ctx.Map(tex, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).map_err(|x| e("No se pudo leer la imagen", x))?;
                    let (cw, ch) = ((content.Width.max(1) as u32).min(desc.Width), (content.Height.max(1) as u32).min(desc.Height));
                    let src = std::slice::from_raw_parts(mapped.pData as *const u8, mapped.RowPitch as usize * desc.Height as usize);
                    scale_bgra(src, cw, ch, mapped.RowPitch as usize, self.out_w, self.out_h, &mut self.image);
                    self.ctx.Unmap(tex, 0);
                    // Si el monitor cambió de tamaño, la reserva de Windows se ajusta al nuevo.
                    if content.Width != self.pool_size.Width || content.Height != self.pool_size.Height {
                        self.pool_size = content;
                        let _ = self.pool.Recreate(&self.d3d, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, content);
                    }
                    Ok(())
                })();
                let _ = frame.Close();
                copied?;
                self.have_image = true;
                Ok(true)
            }
        }

        /// ¿Sigue conectado el monitor?
        pub fn monitor_alive(&self) -> bool {
            self.gdi.is_empty() || monitor_handle(&self.gdi).is_ok()
        }
    }

    impl Drop for Grabber {
        fn drop(&mut self) {
            // Primero se para la captura y luego se cierra lo demás.
            let _ = self.session.Close();
            let _ = self.pool.Close();
        }
    }

    // ---------- Comprobar un vídeo ya hecho (lo usa la prueba real) ----------

    /// Lo que se lee de un MP4: duración en milisegundos y tamaño.
    #[cfg(test)]
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct VideoInfo {
        pub duration_ms: u64,
        pub width: u32,
        pub height: u32,
        /// El MP4 lleva una pista de audio.
        pub has_audio: bool,
    }

    #[cfg(test)]
    pub(crate) fn video_info(path: &std::path::Path) -> Result<VideoInfo, String> {
        // SAFETY: igual que `record`: COM y Media Foundation de este hilo, soltando todo antes de cerrarlos.
        unsafe {
            let com = CoInitializeEx(None, COINIT_MULTITHREADED);
            let started = MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|x| e("MFStartup", x));
            let result = started.and_then(|()| {
                let r = (|| {
                    let url: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                    let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), None::<&IMFAttributes>).map_err(|x| e("No se pudo abrir el vídeo", x))?;
                    let dur = reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION).map_err(|x| e("Duración", x))?;
                    let hns = dur.Anonymous.Anonymous.Anonymous.uhVal;
                    let media = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32).map_err(|x| e("Tipo de vídeo", x))?;
                    let size = media.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|x| e("Tamaño del vídeo", x))?;
                    let has_audio = reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32).is_ok();
                    Ok(VideoInfo { duration_ms: (hns / 10_000) as u64, width: (size >> 32) as u32, height: (size & 0xFFFF_FFFF) as u32, has_audio })
                })();
                let _ = MFShutdown();
                r
            });
            if com.is_ok() {
                CoUninitialize();
            }
            result
        }
    }

    /// Prueba con un vídeo inventado (mitad de arriba roja, mitad de abajo azul) de que el codificador no
    /// lo da la vuelta: se codifica con el mismo `Encoder` y se vuelve a leer con Windows.
    #[cfg(test)]
    mod orientation {
        use super::*;

        #[test]
        #[ignore = "usa el codificador de vídeo de este equipo"]
        fn el_video_no_sale_del_reves() {
            let (w, h) = (320u32, 240u32);
            let path = std::env::temp_dir().join(format!("adminops-faultrec-orientacion-{}.mp4", std::process::id()));
            let _ = std::fs::remove_file(&path);
            let mut frame = vec![0u8; (w * h * 4) as usize];
            for (i, px) in frame.chunks_exact_mut(4).enumerate() {
                let y = i as u32 / w;
                let bgra = if y < h / 2 { [0, 0, 255, 255] } else { [255, 0, 0, 255] };
                px.copy_from_slice(&bgra);
            }
            // SAFETY: COM y Media Foundation de este hilo, soltando todo antes de cerrarlos.
            let (top, bottom) = unsafe {
                let com = CoInitializeEx(None, COINIT_MULTITHREADED);
                MFStartup(MF_VERSION, MFSTARTUP_FULL).unwrap();
                let enc = Encoder::new(&path, w, h, false, false).unwrap();
                for i in 0..20 {
                    enc.write(&frame, i * 1_000_000, 1_000_000).unwrap();
                }
                enc.finish().unwrap();

                let mut attrs: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut attrs, 1).unwrap();
                let attrs = attrs.unwrap();
                attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).unwrap();
                let url: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), &attrs).unwrap();
                let want = MFCreateMediaType().unwrap();
                want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
                want.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).unwrap();
                let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
                reader.SetCurrentMediaType(stream, None, &want).unwrap();
                let stride = reader.GetCurrentMediaType(stream).unwrap().GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap() as i32;
                let mut sample: Option<IMFSample> = None;
                for _ in 0..5 {
                    let mut flags = 0u32;
                    reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample)).unwrap();
                    if sample.is_some() {
                        break;
                    }
                }
                let buffer = sample.expect("un fotograma").ConvertToContiguousBuffer().unwrap();
                let mut ptr: *mut u8 = std::ptr::null_mut();
                let mut len = 0u32;
                buffer.Lock(&mut ptr, None, Some(&mut len)).unwrap();
                let data = std::slice::from_raw_parts(ptr, len as usize);
                let pitch = stride.unsigned_abs() as usize;
                // Con stride negativo la primera fila de la memoria es la de abajo.
                let mem_row = |image_row: usize| if stride < 0 { h as usize - 1 - image_row } else { image_row };
                let at = |image_row: usize| {
                    let o = mem_row(image_row) * pitch + (w as usize / 2) * 4;
                    [data[o], data[o + 1], data[o + 2]]
                };
                let result = (at(10), at(h as usize - 10));
                buffer.Unlock().unwrap();
                drop((reader, buffer));
                let _ = MFShutdown();
                if com.is_ok() {
                    CoUninitialize();
                }
                result
            };
            let _ = std::fs::remove_file(&path);
            println!("ORIENTACION: arriba (B,G,R) = {top:?}, abajo = {bottom:?}");
            assert!(top[2] > 150 && top[0] < 100, "arriba debería ser rojo: {top:?}");
            assert!(bottom[0] > 150 && bottom[2] < 100, "abajo debería ser azul: {bottom:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamano_par_y_con_ancho_maximo() {
        assert_eq!(output_size(1920, 1080), (1920, 1080));
        assert_eq!(output_size(2560, 1440), (1920, 1080));
        assert_eq!(output_size(3840, 2160), (1920, 1080));
        // Pantalla pequeña: no se agranda y queda par.
        assert_eq!(output_size(1366, 768), (1366, 768));
        assert_eq!(output_size(1365, 767), (1364, 766));
        // Panorámica y vertical.
        assert_eq!(output_size(3440, 1440), (1920, 804));
        assert_eq!(output_size(1080, 1920), (1080, 1920));
        for (w, h) in [(2561, 1441), (5120, 1440), (1921, 1081), (7, 5), (0, 0)] {
            let (ow, oh) = output_size(w, h);
            assert!(ow % 2 == 0 && oh % 2 == 0 && ow >= 2 && oh >= 2 && ow <= 1920, "{w}x{h} -> {ow}x{oh}");
        }
    }

    fn solid(w: usize, h: usize, bgr: [u8; 3]) -> Vec<u8> {
        (0..w * h).flat_map(|_| [bgr[0], bgr[1], bgr[2], 0]).collect()
    }

    #[test]
    fn escalar_un_color_liso_lo_conserva_y_deja_el_alfa_opaco() {
        let src = solid(8, 6, [10, 20, 30]);
        let mut dst = Vec::new();
        scale_bgra(&src, 8, 6, 8 * 4, 4, 2, &mut dst);
        assert_eq!(dst.len(), 4 * 2 * 4);
        assert!(dst.chunks_exact(4).all(|p| p == [10, 20, 30, 255]));
        // Mismo tamaño: copia fiel.
        let mut same = Vec::new();
        scale_bgra(&src, 8, 6, 8 * 4, 8, 6, &mut same);
        assert!(same.chunks_exact(4).all(|p| p == [10, 20, 30, 255]));
    }

    #[test]
    fn escalar_promedia_y_respeta_la_fila_con_relleno() {
        // Tablero de 2x2 reducido a 1x1: la media de negro y blanco.
        let mut src = vec![0u8; 2 * 2 * 4];
        for (i, px) in src.chunks_exact_mut(4).enumerate() {
            let v = if i == 0 || i == 3 { 255 } else { 0 };
            px.copy_from_slice(&[v, v, v, 0]);
        }
        let mut dst = Vec::new();
        scale_bgra(&src, 2, 2, 8, 1, 1, &mut dst);
        assert!((120..=135).contains(&dst[0]), "{}", dst[0]);
        // Con relleno al final de cada fila (stride mayor que el ancho).
        let mut padded = vec![7u8; 16 * 2];
        for row in 0..2 {
            for x in 0..2 {
                padded[row * 16 + x * 4..row * 16 + x * 4 + 4].copy_from_slice(&[50, 60, 70, 0]);
            }
        }
        let mut out = Vec::new();
        scale_bgra(&padded, 2, 2, 16, 2, 2, &mut out);
        assert!(out.chunks_exact(4).all(|p| p == [50, 60, 70, 255]));
    }

    fn zone(x: i32, y: i32, w: u32, h: u32) -> Region {
        Region { x, y, w, h }
    }

    #[test]
    fn la_zona_se_recorta_a_lo_que_cabe_en_el_monitor() {
        // Dentro: tal cual.
        assert_eq!(crop_rect(1920, 1080, zone(100, 50, 800, 600)), Some((100, 50, 800, 600)));
        // Todo el monitor y justo en los bordes.
        assert_eq!(crop_rect(1920, 1080, zone(0, 0, 1920, 1080)), Some((0, 0, 1920, 1080)));
        assert_eq!(crop_rect(1920, 1080, zone(1856, 1016, 64, 64)), Some((1856, 1016, 64, 64)));
        // Se sale por la derecha y por abajo: se queda con lo que cabe.
        assert_eq!(crop_rect(1920, 1080, zone(1500, 800, 800, 600)), Some((1500, 800, 420, 280)));
        // Se sale por la izquierda y por arriba.
        assert_eq!(crop_rect(1920, 1080, zone(-100, -50, 400, 300)), Some((0, 0, 300, 250)));
        // El mínimo es 64×64 (ya recortada).
        assert_eq!(crop_rect(1920, 1080, zone(10, 10, 64, 64)), Some((10, 10, 64, 64)));
        assert_eq!(crop_rect(1920, 1080, zone(10, 10, 63, 200)), None);
        assert_eq!(crop_rect(1920, 1080, zone(10, 10, 200, 63)), None);
        assert_eq!(crop_rect(1920, 1080, zone(1900, 10, 200, 200)), None);
        // Fuera del todo, o con un monitor que ya no es tan grande.
        assert_eq!(crop_rect(1920, 1080, zone(2000, 0, 300, 300)), None);
        assert_eq!(crop_rect(1920, 1080, zone(-500, 0, 300, 300)), None);
        assert_eq!(crop_rect(800, 600, zone(1000, 1000, 300, 300)), None);
        assert_eq!(crop_rect(0, 0, zone(0, 0, 300, 300)), None);
        // Valores enormes no desbordan.
        assert_eq!(crop_rect(1920, 1080, zone(i32::MAX, i32::MAX, u32::MAX, u32::MAX)), None);
        assert_eq!(crop_rect(1920, 1080, zone(i32::MIN, i32::MIN, u32::MAX, u32::MAX)), Some((0, 0, 1920, 1080)));
    }

    #[test]
    fn el_video_de_una_zona_sale_del_tamano_de_la_zona() {
        let (_, _, w, h) = crop_rect(2560, 1440, zone(0, 0, 801, 601)).unwrap();
        assert_eq!(output_size(w, h), (800, 600));
        let (_, _, w, h) = crop_rect(3840, 2160, zone(0, 0, 3840, 1000)).unwrap();
        assert_eq!(output_size(w, h), (1920, 500));
    }

    #[test]
    fn escalar_un_trozo_con_el_origen_desplazado_toma_los_puntos_de_ese_trozo() {
        // Imagen de 8×4 con el trozo de 4×2 desde (2,1) de color distinto.
        let (w, h) = (8usize, 4usize);
        let mut src = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let inside = (2..6).contains(&x) && (1..3).contains(&y);
                let o = (y * w + x) * 4;
                src[o..o + 4].copy_from_slice(&if inside { [200, 100, 50, 0] } else { [1, 2, 3, 0] });
            }
        }
        let stride = w * 4;
        let offset = stride + 2 * 4;
        let mut dst = Vec::new();
        scale_bgra(&src[offset..], 4, 2, stride, 4, 2, &mut dst);
        assert!(dst.chunks_exact(4).all(|p| p == [200, 100, 50, 255]));
    }

    #[test]
    fn escalar_con_datos_insuficientes_no_revienta() {
        let mut dst = Vec::new();
        scale_bgra(&[0u8; 8], 100, 100, 400, 50, 50, &mut dst);
        assert_eq!(dst.len(), 50 * 50 * 4);
    }
}
