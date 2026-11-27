//! Lo que se hace con una grabación ya guardada (1.4, G4): sacar fotogramas como JPEG (la miniatura de la lista y
//! los tres del informe) y exportar un trozo a un MP4 aparte sin volver a codificarlo. También la copia pequeña
//! «Para enviar» (720p, 15 fotogramas por segundo) y un GIF corto.
//!
//! Todo con lo que trae Windows: Media Foundation lee el vídeo y el codificador de imágenes de WinRT hace el JPEG.
//! Cada función inicia y cierra COM y Media Foundation en su hilo, y lo suelta todo antes de cerrarlos.

use std::path::Path;

/// Ancho de la miniatura de la lista.
pub const THUMB_W: u32 = 320;
/// Ancho de los fotogramas del informe.
pub const REPORT_W: u32 = 800;

/// Los tres momentos del informe dentro de `[from, to]` (milisegundos): las marcas que caen dentro (hasta tres, por
/// orden) y, si faltan, el principio, la mitad y el final del trozo. Siempre en orden y sin repetir.
pub fn report_times(from: u64, to: u64, marks_ms: &[u64]) -> Vec<u64> {
    let (from, to) = (from.min(to), from.max(to));
    let mut out: Vec<u64> = marks_ms.iter().copied().filter(|t| (from..=to).contains(t)).collect();
    out.sort_unstable();
    out.dedup();
    out.truncate(3);
    for t in [from, from + (to - from) / 2, to.saturating_sub(200).max(from)] {
        if out.len() >= 3 {
            break;
        }
        // Lo bastante lejos de lo que ya hay (medio segundo) para que no salgan dos iguales.
        if out.iter().all(|x| x.abs_diff(t) >= 500) {
            out.push(t);
        }
    }
    out.sort_unstable();
    out
}

/// Tamaño de salida con ancho máximo `max_w`, manteniendo la proporción y par.
pub fn fit(w: u32, h: u32, max_w: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let (ow, oh) = if w > max_w { (max_w, ((u64::from(h) * u64::from(max_w)) / u64::from(w)) as u32) } else { (w, h) };
    ((ow & !1).max(2), (oh & !1).max(2))
}


/// Ancho y alto máximos de la copia «Para enviar» (720p) y sus fotogramas por segundo.
pub const SEND_MAX_W: u32 = 1280;
pub const SEND_MAX_H: u32 = 720;
pub const SEND_FPS: u32 = 15;
/// El GIF: como mucho 8 segundos, 480 de ancho y 10 fotogramas por segundo.
pub const GIF_MAX_MS: u64 = 8_000;
pub const GIF_W: u32 = 480;
pub const GIF_FPS: u32 = 10;

/// Tamaño de salida dentro de una caja de `max_w`×`max_h`, manteniendo la proporción, sin agrandar y par.
pub fn fit_box(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    if w <= max_w && h <= max_h {
        return ((w & !1).max(2), (h & !1).max(2));
    }
    // La razón más estricta de las dos manda.
    let (ow, oh) = if u64::from(max_w) * u64::from(h) <= u64::from(max_h) * u64::from(w) {
        (max_w, (u64::from(h) * u64::from(max_w) / u64::from(w)) as u32)
    } else {
        ((u64::from(w) * u64::from(max_h) / u64::from(h)) as u32, max_h)
    };
    ((ow & !1).max(2), (oh & !1).max(2))
}

/// El trozo del GIF: el que se pide, pero de 8 segundos como mucho (se queda con el principio).
pub fn gif_range(from_ms: u64, to_ms: u64) -> (u64, u64) {
    let (a, b) = (from_ms.min(to_ms), from_ms.max(to_ms));
    (a, b.min(a + GIF_MAX_MS))
}

#[cfg(not(windows))]
pub fn frames_jpeg(_path: &Path, _times_ms: &[u64], _max_w: u32) -> Result<Vec<Vec<u8>>, String> {
    Err("Solo funciona en Windows.".into())
}

#[cfg(not(windows))]
pub fn export(_src: &Path, _dst: &Path, _from_ms: u64, _to_ms: u64) -> Result<u64, String> {
    Err("Solo funciona en Windows.".into())
}


#[cfg(not(windows))]
pub fn shrink(_src: &Path, _dst: &Path) -> Result<u64, String> {
    Err("Solo funciona en Windows.".into())
}

#[cfg(not(windows))]
pub fn gif(_src: &Path, _dst: &Path, _from_ms: u64, _to_ms: u64) -> Result<u64, String> {
    Err("Solo funciona en Windows.".into())
}

#[cfg(windows)]
pub use imp::{export, frames_jpeg, gif, shrink};

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{GUID, PCWSTR};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    use windows::Win32::System::Variant::VT_I8;

    fn e(what: &str, err: windows::core::Error) -> String {
        format!("{what}: {}", err.message())
    }

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// COM y Media Foundation para lo que haga `f`, en este hilo.
    fn with_mf<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        super::super::capture::keep_mta_alive();
        // SAFETY: se inician y se cierran en este hilo; lo que crea `f` se suelta al salir de ella, antes de cerrarlos.
        unsafe {
            let com = CoInitializeEx(None, COINIT_MULTITHREADED);
            let started = MFStartup(MF_VERSION, MFSTARTUP_FULL);
            let r = match &started {
                Ok(()) => f(),
                Err(x) => Err(e("No se pudo iniciar Media Foundation", x.clone())),
            };
            if started.is_ok() {
                let _ = MFShutdown();
            }
            if com.is_ok() {
                CoUninitialize();
            }
            r
        }
    }

    /// Un PROPVARIANT con un tiempo en unidades de 100 ns, para `SetCurrentPosition`.
    fn position(hns: i64) -> PROPVARIANT {
        let mut pv = PROPVARIANT::default();
        // SAFETY: se escribe el tipo y su valor en la unión, como manda VT_I8.
        unsafe {
            (*pv.Anonymous.Anonymous).vt = VT_I8;
            (*pv.Anonymous.Anonymous).Anonymous.hVal = hns;
        }
        pv
    }

    /// Una imagen BGRA (de arriba abajo, sin huecos) a JPEG con el codificador de Windows.
    fn jpeg(bgra: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
        use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapEncoder, BitmapPixelFormat};
        use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
        let fail = |x: windows::core::Error| e("No se pudo hacer la imagen", x);
        let stream = InMemoryRandomAccessStream::new().map_err(fail)?;
        let enc = BitmapEncoder::CreateAsync(BitmapEncoder::JpegEncoderId().map_err(fail)?, &stream).map_err(fail)?.get().map_err(fail)?;
        enc.SetPixelData(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore, w, h, 96.0, 96.0, bgra).map_err(fail)?;
        enc.FlushAsync().map_err(fail)?.get().map_err(fail)?;
        let size = stream.Size().map_err(fail)? as u32;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).map_err(fail)?).map_err(fail)?;
        reader.LoadAsync(size).map_err(fail)?.get().map_err(fail)?;
        let mut out = vec![0u8; size as usize];
        reader.ReadBytes(&mut out).map_err(fail)?;
        Ok(out)
    }

    /// Los fotogramas de `times_ms` como JPEG de `max_w` de ancho como mucho. Si uno no se puede leer (más allá del
    /// final), se usa el último que se leyó; si no se lee ninguno, error.
    pub fn frames_jpeg(path: &Path, times_ms: &[u64], max_w: u32) -> Result<Vec<Vec<u8>>, String> {
        with_mf(|| unsafe { frames_inner(path, times_ms, max_w) })
    }

    unsafe fn frames_inner(path: &Path, times_ms: &[u64], max_w: u32) -> Result<Vec<Vec<u8>>, String> {
        unsafe {
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 1).map_err(|x| e("Lector de vídeo", x))?;
            let attrs = attrs.ok_or("Lector de vídeo")?;
            // Que Windows decodifique y pase a RGB32.
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).map_err(|x| e("Lector de vídeo", x))?;
            let url = wide(path);
            let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), &attrs).map_err(|x| e("No se pudo abrir el vídeo", x))?;
            let video = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let _ = reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
            reader.SetStreamSelection(video, true).map_err(|x| e("Lector de vídeo", x))?;
            let want = MFCreateMediaType().map_err(|x| e("Lector de vídeo", x))?;
            want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Lector de vídeo", x))?;
            want.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|x| e("Lector de vídeo", x))?;
            reader.SetCurrentMediaType(video, None, &want).map_err(|x| e("Lector de vídeo", x))?;
            let current = reader.GetCurrentMediaType(video).map_err(|x| e("Lector de vídeo", x))?;
            let size = current.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|x| e("Tamaño del vídeo", x))?;
            let (w, h) = ((size >> 32) as u32, (size & 0xFFFF_FFFF) as u32);
            let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| s as i32).unwrap_or((w * 4) as i32);
            let (ow, oh) = fit(w, h, max_w);

            let mut out = Vec::with_capacity(times_ms.len());
            let mut last: Option<Vec<u8>> = None;
            let mut top_down: Vec<u8> = Vec::new();
            let mut scaled: Vec<u8> = Vec::new();
            for &t in times_ms {
                let target = (t as i64) * 10_000;
                let pv = position(target);
                let _ = reader.SetCurrentPosition(&GUID::zeroed(), &pv);
                // Se decodifica desde el fotograma clave anterior: se leen fotogramas hasta llegar al momento pedido.
                let mut got: Option<IMFSample> = None;
                for _ in 0..400 {
                    let (mut flags, mut ts) = (0u32, 0i64);
                    let mut sample: Option<IMFSample> = None;
                    if reader.ReadSample(video, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample)).is_err() {
                        break;
                    }
                    if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                        break;
                    }
                    if let Some(s) = sample {
                        let reached = ts + 1_000_000 / 10 >= target;
                        got = Some(s);
                        if reached {
                            break;
                        }
                    }
                }
                let Some(sample) = got else {
                    if let Some(l) = &last {
                        out.push(l.clone());
                        continue;
                    }
                    return Err("No se pudo leer ningún fotograma del vídeo.".into());
                };
                let buffer = sample.ConvertToContiguousBuffer().map_err(|x| e("Fotograma", x))?;
                let mut ptr: *mut u8 = std::ptr::null_mut();
                let mut len = 0u32;
                buffer.Lock(&mut ptr, None, Some(&mut len)).map_err(|x| e("Fotograma", x))?;
                let data = std::slice::from_raw_parts(ptr, len as usize);
                let pitch = stride.unsigned_abs() as usize;
                let row = (w * 4) as usize;
                top_down.resize(row * h as usize, 0);
                if pitch >= row && data.len() >= pitch * (h as usize - 1) + row {
                    for y in 0..h as usize {
                        // Con stride negativo la primera fila de la memoria es la de abajo.
                        let src_row = if stride < 0 { h as usize - 1 - y } else { y };
                        top_down[y * row..(y + 1) * row].copy_from_slice(&data[src_row * pitch..src_row * pitch + row]);
                    }
                }
                let _ = buffer.Unlock();
                super::super::capture::scale_bgra(&top_down, w, h, row, ow, oh, &mut scaled);
                let img = jpeg(&scaled, ow, oh)?;
                last = Some(img.clone());
                out.push(img);
            }
            Ok(out)
        }
    }


    // ---------- «Para enviar» y GIF ----------

    /// Un vídeo abierto para leerlo fotograma a fotograma (RGB32) y, si se pide, su audio ya decodificado (PCM).
    struct Source {
        reader: IMFSourceReader,
        /// Los números de pista reales.
        video: u32,
        audio: Option<u32>,
        /// El tipo con el que sale el audio decodificado.
        audio_type: Option<IMFMediaType>,
        w: u32,
        h: u32,
        stride: i32,
    }

    unsafe fn open_source(path: &Path, with_audio: bool) -> Result<Source, String> {
        unsafe {
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 1).map_err(|x| e("Lector de vídeo", x))?;
            let attrs = attrs.ok_or("Lector de vídeo")?;
            // Que Windows decodifique y pase a RGB32.
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1).map_err(|x| e("Lector de vídeo", x))?;
            let url = wide(path);
            let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), &attrs).map_err(|x| e("No se pudo abrir el vídeo", x))?;
            // Qué pista es cada cosa.
            let (mut video, mut audio) = (None, None);
            for i in 0..8u32 {
                let Ok(native) = reader.GetNativeMediaType(i, 0) else { break };
                let major = native.GetMajorType().map_err(|x| e("Lector de vídeo", x))?;
                if major == MFMediaType_Video && video.is_none() {
                    video = Some(i);
                } else if major == MFMediaType_Audio && audio.is_none() {
                    audio = Some(i);
                }
            }
            let video = video.ok_or("La grabación no tiene vídeo.")?;
            let _ = reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
            reader.SetStreamSelection(video, true).map_err(|x| e("Lector de vídeo", x))?;
            let want = MFCreateMediaType().map_err(|x| e("Lector de vídeo", x))?;
            want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Lector de vídeo", x))?;
            want.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|x| e("Lector de vídeo", x))?;
            reader.SetCurrentMediaType(video, None, &want).map_err(|x| e("Lector de vídeo", x))?;
            let current = reader.GetCurrentMediaType(video).map_err(|x| e("Lector de vídeo", x))?;
            let size = current.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|x| e("Tamaño del vídeo", x))?;
            let (w, h) = ((size >> 32) as u32, (size & 0xFFFF_FFFF) as u32);
            let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| s as i32).unwrap_or((w * 4) as i32);
            // El audio, si lo hay y se quiere: si Windows no lo puede decodificar, la copia va sin él.
            let (mut audio_idx, mut audio_type) = (None, None);
            if let (true, Some(a)) = (with_audio, audio) {
                let setup = (|| -> Result<IMFMediaType, String> {
                    reader.SetStreamSelection(a, true).map_err(|x| e("Audio", x))?;
                    let want = MFCreateMediaType().map_err(|x| e("Audio", x))?;
                    want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(|x| e("Audio", x))?;
                    want.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(|x| e("Audio", x))?;
                    reader.SetCurrentMediaType(a, None, &want).map_err(|x| e("Audio", x))?;
                    reader.GetCurrentMediaType(a).map_err(|x| e("Audio", x))
                })();
                match setup {
                    Ok(t) => {
                        audio_idx = Some(a);
                        audio_type = Some(t);
                    }
                    Err(x) => {
                        log::info!("Grabar el fallo: la copia para enviar va sin audio ({x})");
                        let _ = reader.SetStreamSelection(a, false);
                    }
                }
            }
            Ok(Source { reader, video, audio: audio_idx, audio_type, w, h, stride })
        }
    }

    /// El fotograma de un `sample` RGB32 pasado a BGRA de arriba abajo y sin huecos entre filas, en `out`.
    unsafe fn top_down(sample: &IMFSample, src: &Source, out: &mut Vec<u8>) -> Result<(), String> {
        unsafe {
            let buffer = sample.ConvertToContiguousBuffer().map_err(|x| e("Fotograma", x))?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut len = 0u32;
            buffer.Lock(&mut ptr, None, Some(&mut len)).map_err(|x| e("Fotograma", x))?;
            let data = std::slice::from_raw_parts(ptr, len as usize);
            let (w, h) = (src.w as usize, src.h as usize);
            let pitch = src.stride.unsigned_abs() as usize;
            let row = w * 4;
            out.resize(row * h, 0);
            if pitch >= row && data.len() >= pitch * (h - 1) + row {
                for y in 0..h {
                    // Con stride negativo la primera fila de la memoria es la de abajo.
                    let src_row = if src.stride < 0 { h - 1 - y } else { y };
                    out[y * row..(y + 1) * row].copy_from_slice(&data[src_row * pitch..src_row * pitch + row]);
                }
            }
            let _ = buffer.Unlock();
            Ok(())
        }
    }

    unsafe fn write_bgra(writer: &IMFSinkWriter, stream: u32, bgra: &[u8], time: i64, duration: i64) -> Result<(), String> {
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
            writer.WriteSample(stream, &sample).map_err(|x| e("No se pudo escribir un fotograma", x))
        }
    }

    fn pack(hi: u32, lo: u32) -> u64 {
        (u64::from(hi) << 32) | u64::from(lo)
    }

    /// La copia pequeña para mandar por Teams o el correo: se decodifica la grabación y se vuelve a codificar a 720p
    /// como mucho y 15 fotogramas por segundo (la grabación original va a 10, así que no se tira ninguno), con el
    /// audio si lo lleva. Codifica por programa (no por la tarjeta gráfica): es lo que funciona en cualquier equipo.
    /// Devuelve lo que dura, en milisegundos.
    pub fn shrink(src: &Path, dst: &Path) -> Result<u64, String> {
        let r = with_mf(|| unsafe { shrink_inner(src, dst) });
        if r.is_err() {
            let _ = std::fs::remove_file(dst);
        }
        r
    }

    unsafe fn shrink_inner(src: &Path, dst: &Path) -> Result<u64, String> {
        unsafe {
            let input = open_source(src, true)?;
            let (ow, oh) = fit_box(input.w, input.h, SEND_MAX_W, SEND_MAX_H);
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 3).map_err(|x| e("Para enviar", x))?;
            let attrs = attrs.ok_or("Para enviar")?;
            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 0).map_err(|x| e("Para enviar", x))?;
            attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1).map_err(|x| e("Para enviar", x))?;
            attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4).map_err(|x| e("Para enviar", x))?;
            let out = wide(dst);
            let writer = MFCreateSinkWriterFromURL(PCWSTR(out.as_ptr()), None::<&IMFByteStream>, &attrs).map_err(|x| e("No se pudo crear la copia", x))?;

            let vout = MFCreateMediaType().map_err(|x| e("Para enviar", x))?;
            vout.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Para enviar", x))?;
            vout.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(|x| e("Para enviar", x))?;
            // Unos 4 MB por minuto: cabe en un correo o en un chat.
            vout.SetUINT32(&MF_MT_AVG_BITRATE, 600_000).map_err(|x| e("Para enviar", x))?;
            vout.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(|x| e("Para enviar", x))?;
            vout.SetUINT64(&MF_MT_FRAME_SIZE, pack(ow, oh)).map_err(|x| e("Para enviar", x))?;
            vout.SetUINT64(&MF_MT_FRAME_RATE, pack(SEND_FPS, 1)).map_err(|x| e("Para enviar", x))?;
            vout.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(|x| e("Para enviar", x))?;
            let vstream = writer.AddStream(&vout).map_err(|x| e("No se pudo crear el vídeo H.264", x))?;
            let vin = MFCreateMediaType().map_err(|x| e("Para enviar", x))?;
            vin.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Para enviar", x))?;
            vin.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|x| e("Para enviar", x))?;
            vin.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(|x| e("Para enviar", x))?;
            vin.SetUINT64(&MF_MT_FRAME_SIZE, pack(ow, oh)).map_err(|x| e("Para enviar", x))?;
            vin.SetUINT64(&MF_MT_FRAME_RATE, pack(SEND_FPS, 1)).map_err(|x| e("Para enviar", x))?;
            vin.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(|x| e("Para enviar", x))?;
            // Stride positivo: la imagen va de arriba abajo, como la deja `top_down`.
            vin.SetUINT32(&MF_MT_DEFAULT_STRIDE, ow * 4).map_err(|x| e("Para enviar", x))?;
            writer.SetInputMediaType(vstream, &vin, None::<&IMFAttributes>).map_err(|x| e("El codificador no acepta la imagen", x))?;

            // El audio, en AAC con los mismos canales y frecuencia que traiga. Si no se puede preparar, va sin él.
            let mut astream: Option<u32> = None;
            let mut reading_audio = input.audio;
            if let Some(cur) = &input.audio_type {
                let setup = (|| -> Result<u32, String> {
                    let rate = cur.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).map_err(|x| e("Audio", x))?;
                    let channels = cur.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).map_err(|x| e("Audio", x))?;
                    let aout = MFCreateMediaType().map_err(|x| e("Audio", x))?;
                    aout.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(|x| e("Audio", x))?;
                    aout.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).map_err(|x| e("Audio", x))?;
                    aout.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(|x| e("Audio", x))?;
                    aout.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate).map_err(|x| e("Audio", x))?;
                    aout.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels).map_err(|x| e("Audio", x))?;
                    aout.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, if channels > 1 { 16_000 } else { 12_000 }).map_err(|x| e("Audio", x))?;
                    aout.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 1).map_err(|x| e("Audio", x))?;
                    aout.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0).map_err(|x| e("Audio", x))?;
                    aout.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29).map_err(|x| e("Audio", x))?;
                    let s = writer.AddStream(&aout).map_err(|x| e("No se pudo crear la pista de audio", x))?;
                    writer.SetInputMediaType(s, cur, None::<&IMFAttributes>).map_err(|x| e("El codificador no acepta el audio", x))?;
                    Ok(s)
                })();
                match setup {
                    Ok(s) => astream = Some(s),
                    Err(x) => {
                        log::info!("Grabar el fallo: la copia para enviar va sin audio ({x})");
                        reading_audio = None;
                        let _ = input.reader.SetStreamSelection(input.audio.unwrap_or(0), false);
                    }
                }
            }
            writer.BeginWriting().map_err(|x| e("No se pudo empezar la copia", x))?;

            let min_gap: i64 = 10_000_000 / i64::from(SEND_FPS);
            let mut last_kept = i64::MIN / 2;
            let mut last_end: i64 = 0;
            let (mut frame, mut scaled) = (Vec::new(), Vec::new());
            let (mut video_done, mut audio_done) = (false, reading_audio.is_none());
            let mut guard = 0u32;
            while !(video_done && audio_done) && guard < 400_000 {
                guard += 1;
                let (mut index, mut flags, mut ts) = (0u32, 0u32, 0i64);
                let mut sample: Option<IMFSample> = None;
                input
                    .reader
                    .ReadSample(MF_SOURCE_READER_ANY_STREAM.0 as u32, 0, Some(&mut index), Some(&mut flags), Some(&mut ts), Some(&mut sample))
                    .map_err(|x| e("No se pudo leer la grabación", x))?;
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    if index == input.video {
                        video_done = true;
                    } else {
                        audio_done = true;
                    }
                    continue;
                }
                let Some(sample) = sample else { continue };
                if index == input.video {
                    // Nunca más de 15 por segundo.
                    if ts - last_kept < min_gap {
                        continue;
                    }
                    last_kept = ts;
                    top_down(&sample, &input, &mut frame)?;
                    super::super::capture::scale_bgra(&frame, input.w, input.h, (input.w * 4) as usize, ow, oh, &mut scaled);
                    write_bgra(&writer, vstream, &scaled, ts, min_gap)?;
                    last_end = ts + min_gap;
                } else if let (Some(a), true) = (astream, Some(index) == reading_audio) {
                    sample.SetSampleTime(ts).map_err(|x| e("Audio", x))?;
                    writer.WriteSample(a, &sample).map_err(|x| e("No se pudo escribir el audio", x))?;
                }
            }
            if last_end == 0 {
                let _ = writer.Finalize();
                return Err("La grabación no tiene ningún fotograma que se pueda copiar.".into());
            }
            writer.Finalize().map_err(|x| e("No se pudo cerrar la copia", x))?;
            Ok((last_end / 10_000) as u64)
        }
    }

    /// Un GIF del trozo `[from_ms, to_ms]` (de 8 segundos como mucho), de 480 de ancho y 10 fotogramas por segundo,
    /// hecho con el codificador de imágenes de Windows. Devuelve lo que dura, en milisegundos.
    pub fn gif(src: &Path, dst: &Path, from_ms: u64, to_ms: u64) -> Result<u64, String> {
        let r = with_mf(|| unsafe { gif_inner(src, dst, from_ms, to_ms) });
        if r.is_err() {
            let _ = std::fs::remove_file(dst);
        }
        r
    }

    unsafe fn gif_inner(src: &Path, dst: &Path, from_ms: u64, to_ms: u64) -> Result<u64, String> {
        use windows::core::HSTRING;
        use windows::Foundation::{PropertyType, PropertyValue};
        use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapEncoder, BitmapPixelFormat, BitmapPropertySet, BitmapTypedValue};
        use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
        unsafe {
            let fail = |x: windows::core::Error| e("No se pudo hacer el GIF", x);
            let (from_ms, to_ms) = gif_range(from_ms, to_ms);
            let input = open_source(src, false)?;
            let (ow, oh) = fit(input.w, input.h, GIF_W);
            let (from, to) = ((from_ms as i64) * 10_000, (to_ms as i64) * 10_000);
            let _ = input.reader.SetCurrentPosition(&GUID::zeroed(), &position(from));

            let stream = InMemoryRandomAccessStream::new().map_err(fail)?;
            let enc = BitmapEncoder::CreateAsync(BitmapEncoder::GifEncoderId().map_err(fail)?, &stream).map_err(fail)?.get().map_err(fail)?;
            // Que se repita sin parar (extensión «NETSCAPE2.0» del GIF). Si el codificador no la admite, se ve una vez.
            let looped = (|| -> windows::core::Result<()> {
                let props = BitmapPropertySet::new()?;
                props.Insert(&HSTRING::from("/appext/Application"), &BitmapTypedValue::Create(&PropertyValue::CreateUInt8Array(b"NETSCAPE2.0")?, PropertyType::UInt8Array)?)?;
                props.Insert(&HSTRING::from("/appext/Data"), &BitmapTypedValue::Create(&PropertyValue::CreateUInt8Array(&[3, 1, 0, 0, 0])?, PropertyType::UInt8Array)?)?;
                enc.BitmapContainerProperties()?.SetPropertiesAsync(&props)?.get()
            })();
            if let Err(x) = looped {
                log::info!("Grabar el fallo: el GIF no se repetirá solo ({x})");
            }

            let min_gap: i64 = 10_000_000 / i64::from(GIF_FPS) * 9 / 10;
            let delay = (100 / GIF_FPS) as u16;
            let (mut frame, mut scaled) = (Vec::new(), Vec::new());
            let mut last_kept = i64::MIN / 2;
            let mut last_ts = from;
            let mut count = 0u32;
            let mut guard = 0u32;
            while guard < 4_000 {
                guard += 1;
                let (mut flags, mut ts) = (0u32, 0i64);
                let mut sample: Option<IMFSample> = None;
                input.reader.ReadSample(input.video, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample)).map_err(|x| e("No se pudo leer la grabación", x))?;
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                let Some(sample) = sample else { continue };
                // Al buscar, Windows decodifica desde el fotograma clave anterior: lo de antes del trozo no cuenta.
                if ts < from {
                    continue;
                }
                if ts > to {
                    break;
                }
                if ts - last_kept < min_gap {
                    continue;
                }
                last_kept = ts;
                top_down(&sample, &input, &mut frame)?;
                super::super::capture::scale_bgra(&frame, input.w, input.h, (input.w * 4) as usize, ow, oh, &mut scaled);
                if count > 0 {
                    enc.GoToNextFrameAsync().map_err(fail)?.get().map_err(fail)?;
                }
                let props = BitmapPropertySet::new().map_err(fail)?;
                let one = BitmapTypedValue::Create(&PropertyValue::CreateUInt16(delay).map_err(fail)?, PropertyType::UInt16).map_err(fail)?;
                props.Insert(&HSTRING::from("/grctlext/Delay"), &one).map_err(fail)?;
                enc.BitmapProperties().map_err(fail)?.SetPropertiesAsync(&props).map_err(fail)?.get().map_err(fail)?;
                enc.SetPixelData(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore, ow, oh, 96.0, 96.0, &scaled).map_err(fail)?;
                count += 1;
                last_ts = ts;
            }
            if count == 0 {
                return Err("En ese trozo no hay ningún fotograma para el GIF.".into());
            }
            enc.FlushAsync().map_err(fail)?.get().map_err(fail)?;
            let size = stream.Size().map_err(fail)? as u32;
            let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).map_err(fail)?).map_err(fail)?;
            reader.LoadAsync(size).map_err(fail)?.get().map_err(fail)?;
            let mut bytes = vec![0u8; size as usize];
            reader.ReadBytes(&mut bytes).map_err(fail)?;
            std::fs::write(dst, &bytes).map_err(|x| format!("No se pudo guardar el GIF: {x}"))?;
            Ok(((last_ts - from).max(0) / 10_000) as u64 + 1000 / u64::from(GIF_FPS))
        }
    }

    /// Copia `[from_ms, to_ms]` de `src` a `dst` (MP4) sin volver a codificar: el vídeo empieza en el fotograma clave
    /// anterior a `from_ms` (así se ve desde el principio) y la voz, si la hay, va con él. Devuelve lo que dura.
    pub fn export(src: &Path, dst: &Path, from_ms: u64, to_ms: u64) -> Result<u64, String> {
        let r = with_mf(|| unsafe { export_inner(src, dst, from_ms, to_ms) });
        if r.is_err() {
            let _ = std::fs::remove_file(dst);
        }
        r
    }

    unsafe fn export_inner(src: &Path, dst: &Path, from_ms: u64, to_ms: u64) -> Result<u64, String> {
        unsafe {
            let url = wide(src);
            let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), None::<&IMFAttributes>).map_err(|x| e("No se pudo abrir la grabación", x))?;
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 1).map_err(|x| e("Exportar", x))?;
            let attrs = attrs.ok_or("Exportar")?;
            attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4).map_err(|x| e("Exportar", x))?;
            let out = wide(dst);
            let writer = MFCreateSinkWriterFromURL(PCWSTR(out.as_ptr()), None::<&IMFByteStream>, &attrs).map_err(|x| e("No se pudo crear el archivo", x))?;

            // Cada pista del original, tal cual (vídeo H.264 y, si la hay, la voz en AAC).
            struct Track {
                src: u32,
                out: u32,
                video: bool,
                done: bool,
            }
            let mut tracks: Vec<Track> = Vec::new();
            for i in 0..8u32 {
                let Ok(native) = reader.GetNativeMediaType(i, 0) else { break };
                let major = native.GetMajorType().map_err(|x| e("Exportar", x))?;
                let video = major == MFMediaType_Video;
                if !video && major != MFMediaType_Audio {
                    let _ = reader.SetStreamSelection(i, false);
                    continue;
                }
                reader.SetStreamSelection(i, true).map_err(|x| e("Exportar", x))?;
                let idx = writer.AddStream(&native).map_err(|x| e("No se pudo preparar el archivo", x))?;
                writer.SetInputMediaType(idx, &native, None::<&IMFAttributes>).map_err(|x| e("No se pudo preparar el archivo", x))?;
                tracks.push(Track { src: i, out: idx, video, done: false });
            }
            if !tracks.iter().any(|t| t.video) {
                return Err("La grabación no tiene vídeo.".into());
            }
            writer.BeginWriting().map_err(|x| e("No se pudo empezar el archivo", x))?;

            let (from, to) = ((from_ms.min(to_ms) as i64) * 10_000, (from_ms.max(to_ms) as i64) * 10_000);
            let pv = position(from);
            let _ = reader.SetCurrentPosition(&GUID::zeroed(), &pv);
            // El primer fotograma clave que sale marca el cero del archivo nuevo.
            let mut base: Option<i64> = None;
            let mut last_end: i64 = 0;
            let mut guard = 0u32;
            while tracks.iter().any(|t| !t.done) && guard < 200_000 {
                guard += 1;
                let (mut index, mut flags, mut ts) = (0u32, 0u32, 0i64);
                let mut sample: Option<IMFSample> = None;
                reader
                    .ReadSample(MF_SOURCE_READER_ANY_STREAM.0 as u32, 0, Some(&mut index), Some(&mut flags), Some(&mut ts), Some(&mut sample))
                    .map_err(|x| e("No se pudo leer la grabación", x))?;
                let Some(track) = tracks.iter_mut().find(|t| t.src == index) else { continue };
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    track.done = true;
                    continue;
                }
                let Some(sample) = sample else { continue };
                if ts > to {
                    track.done = true;
                    continue;
                }
                if base.is_none() {
                    // Hasta el primer fotograma clave de vídeo no se escribe nada.
                    let key = track.video && sample.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) != 0;
                    if !key {
                        continue;
                    }
                    base = Some(ts);
                }
                let b = base.unwrap_or(0);
                if ts < b {
                    continue;
                }
                sample.SetSampleTime(ts - b).map_err(|x| e("Exportar", x))?;
                let dur = sample.GetSampleDuration().unwrap_or(0);
                last_end = last_end.max(ts - b + dur);
                writer.WriteSample(track.out, &sample).map_err(|x| e("No se pudo escribir el archivo", x))?;
            }
            if base.is_none() {
                let _ = writer.Finalize();
                return Err("En ese trozo no hay ningún fotograma que se pueda guardar.".into());
            }
            writer.Finalize().map_err(|x| e("No se pudo cerrar el archivo", x))?;
            Ok((last_end / 10_000) as u64)
        }
    }

    #[cfg(test)]
    mod real {
        use super::*;

        /// Graba 4 s de la pantalla, saca tres fotogramas y exporta el segundo 1 al 3.
        #[test]
        #[ignore = "graba la pantalla de este equipo"]
        fn fotogramas_y_exportar_de_verdad() {
            use std::sync::atomic::AtomicBool;
            use std::sync::{Arc, Mutex};
            let dir = std::env::temp_dir().join(format!("adminops-clip-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let src = dir.join("origen.mp4");
            let job = super::super::super::capture::Job {
                gdi: String::new(),
                window: None,
                region: None,
                mic: None,
                sys: None,
                pause: Arc::new(AtomicBool::new(false)),
                out: src.clone(),
                stop: Arc::new(AtomicBool::new(false)),
                first: Arc::new(Mutex::new(None)),
                max: std::time::Duration::from_secs(4),
            };
            super::super::super::capture::record(job).expect("grabar");
            let frames = frames_jpeg(&src, &report_times(500, 3500, &[]), REPORT_W).expect("fotogramas");
            assert_eq!(frames.len(), 3);
            assert!(frames.iter().all(|f| f.len() > 1000 && f[0] == 0xFF && f[1] == 0xD8), "son JPEG");
            let dst = dir.join("trozo.mp4");
            let ms = export(&src, &dst, 1000, 3000).expect("exportar");
            let info = super::super::super::capture::video_info(&dst).expect("leer el trozo");
            println!("TROZO: {ms} ms escritos · MP4 de {} ms · {}x{}", info.duration_ms, info.width, info.height);
            assert!((1_500..=4_200).contains(&info.duration_ms), "{}", info.duration_ms);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Graba 4 s de la pantalla con el sonido del equipo (si Windows lo deja), con una pausa de 1 s por en medio,
        /// y de ahí saca la copia «Para enviar» y un GIF.
        #[test]
        #[ignore = "graba la pantalla y el sonido de este equipo"]
        fn para_enviar_y_gif_de_verdad() {
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::{Arc, Mutex};
            let dir = std::env::temp_dir().join(format!("adminops-small-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let src = dir.join("origen.mp4");
            let sys = super::super::super::mic::start_system();
            println!("SONIDO DEL EQUIPO: {}", if sys.is_ok() { "abierto".to_string() } else { format!("{:?}", sys.as_ref().err()) });
            let pause = Arc::new(AtomicBool::new(false));
            let stop = Arc::new(AtomicBool::new(false));
            let job = super::super::super::capture::Job {
                gdi: String::new(),
                window: None,
                region: None,
                mic: None,
                sys: sys.ok(),
                pause: pause.clone(),
                out: src.clone(),
                stop: stop.clone(),
                first: Arc::new(Mutex::new(None)),
                max: std::time::Duration::from_secs(60),
            };
            let h = std::thread::spawn(move || super::super::super::capture::record(job));
            std::thread::sleep(std::time::Duration::from_millis(2_000));
            pause.store(true, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(1_500));
            pause.store(false, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(2_000));
            stop.store(true, Ordering::SeqCst);
            let done = h.join().expect("hilo").expect("grabar");
            let orig = super::super::super::capture::video_info(&src).expect("leer el original");
            println!("ORIGINAL: {} ms (medida {} ms) · audio {} · voz {}", orig.duration_ms, done.duration_ms, orig.has_audio, done.with_voice);
            // 5,5 s de reloj (menos lo que tarda el primer fotograma, ~1 s) con 1,5 s de pausa: ~3 s, no ~4,5.
            assert!((2_000..=3_800).contains(&orig.duration_ms), "la pausa no debería contar: {} ms", orig.duration_ms);
            let small = dir.join("pequeno.mp4");
            let ms = shrink(&src, &small).expect("para enviar");
            let info = super::super::super::capture::video_info(&small).expect("leer la copia");
            println!("PARA ENVIAR: {ms} ms · {}x{} · {} ms · audio {} · {} bytes", info.width, info.height, info.duration_ms, info.has_audio, std::fs::metadata(&small).map(|m| m.len()).unwrap_or(0));
            assert!(info.height <= 720 && info.width <= 1280);
            assert!((2_000..=4_500).contains(&info.duration_ms), "{}", info.duration_ms);
            assert_eq!(info.has_audio, orig.has_audio);
            let g = dir.join("trozo.gif");
            let gms = gif(&src, &g, 500, 3_500).expect("gif");
            let bytes = std::fs::read(&g).expect("leer el gif");
            println!("GIF: {gms} ms · {} bytes", bytes.len());
            assert!(bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a"));
            assert!(bytes.len() > 2_000);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_tres_momentos_del_informe() {
        // Sin marcas: principio, mitad y casi el final.
        assert_eq!(report_times(13_000, 27_000, &[]), vec![13_000, 20_000, 26_800]);
        // Las marcas que caen dentro mandan, y se completa con lo que falte.
        assert_eq!(report_times(13_000, 27_000, &[14_000, 22_000, 40_000]), vec![13_000, 14_000, 22_000]);
        assert_eq!(report_times(13_000, 27_000, &[14_000, 19_000, 22_000, 25_000]), vec![14_000, 19_000, 22_000]);
        // Al revés y repetidas: igual.
        assert_eq!(report_times(27_000, 13_000, &[22_000, 22_000]), vec![13_000, 20_000, 22_000]);
        // Un trozo muy corto no repite fotogramas casi iguales.
        assert_eq!(report_times(5_000, 5_300, &[]), vec![5_000]);
    }

    #[test]
    fn tamano_con_ancho_maximo_y_par() {
        assert_eq!(fit(1920, 1080, 320), (320, 180));
        assert_eq!(fit(1366, 768, 800), (800, 448));
        assert_eq!(fit(300, 200, 800), (300, 200));
        assert_eq!(fit(0, 0, 320), (2, 2));
    }

    #[test]
    fn la_copia_para_enviar_cabe_en_720p_sin_agrandar() {
        assert_eq!(fit_box(1920, 1080, 1280, 720), (1280, 720));
        assert_eq!(fit_box(1366, 768, 1280, 720), (1280, 718));
        assert_eq!(fit_box(1280, 720, 1280, 720), (1280, 720));
        // Más pequeña: se queda como está (y par).
        assert_eq!(fit_box(800, 600, 1280, 720), (800, 600));
        assert_eq!(fit_box(801, 601, 1280, 720), (800, 600));
        // Vertical y panorámica: manda la razón más estricta.
        assert_eq!(fit_box(1080, 1920, 1280, 720), (404, 720));
        assert_eq!(fit_box(3440, 1440, 1280, 720), (1280, 534));
        assert_eq!(fit_box(0, 0, 1280, 720), (2, 2));
        for (w, h) in [(2561, 1441), (5120, 1440), (1921, 1081), (7, 5)] {
            let (ow, oh) = fit_box(w, h, 1280, 720);
            assert!(ow % 2 == 0 && oh % 2 == 0 && ow <= 1280 && oh <= 720, "{w}x{h} -> {ow}x{oh}");
        }
    }

    #[test]
    fn el_gif_dura_como_mucho_ocho_segundos() {
        assert_eq!(gif_range(10_000, 14_000), (10_000, 14_000));
        assert_eq!(gif_range(10_000, 40_000), (10_000, 18_000));
        // Al revés también.
        assert_eq!(gif_range(40_000, 10_000), (10_000, 18_000));
        assert_eq!(gif_range(0, 8_000), (0, 8_000));
    }
}
