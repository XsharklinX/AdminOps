//! Lo que se hace con una grabación ya guardada (1.4, G4): sacar fotogramas como JPEG (la miniatura de la lista y
//! los tres del informe) y exportar un trozo a un MP4 aparte sin volver a codificarlo.
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

#[cfg(not(windows))]
pub fn frames_jpeg(_path: &Path, _times_ms: &[u64], _max_w: u32) -> Result<Vec<Vec<u8>>, String> {
    Err("Solo funciona en Windows.".into())
}

#[cfg(not(windows))]
pub fn export(_src: &Path, _dst: &Path, _from_ms: u64, _to_ms: u64) -> Result<u64, String> {
    Err("Solo funciona en Windows.".into())
}

#[cfg(windows)]
pub use imp::{export, frames_jpeg};

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
}
