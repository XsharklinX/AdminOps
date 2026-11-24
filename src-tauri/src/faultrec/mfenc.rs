//! El codificador H.264 de Media Foundation usado directamente como transformación (MFT), para Vigilar, y
//! el guardado de lo ya comprimido en un MP4 sin volver a codificarlo.
//!
//! Se usa el codificador **por programa y síncrono** (`MFT_ENUM_FLAG_SYNCMFT`): los de tarjeta gráfica son
//! asíncronos (hay que atender sus eventos) y aquí no hacen falta, a 5 fotogramas por segundo. Entra NV12 (la
//! conversión desde BGRA se hace aquí) y salen muestras H.264 con códigos de inicio (Annex B), que se guardan tal
//! cual en el búfer circular y se escriben luego con `SinkWriter` sin recodificar.
//!
//! La conversión de color y la búsqueda de la cabecera (SPS y PPS) son lógica pura con pruebas.

// ---------- Lógica pura ----------

/// De BGRA (`w`×`h`, sin hueco entre filas, `w` y `h` pares) a NV12: el plano de luminosidad y después los colores
/// intercalados (U, V) a media resolución. BT.601 de rango limitado, que es lo que espera el codificador.
pub fn bgra_to_nv12(bgra: &[u8], w: u32, h: u32, out: &mut Vec<u8>) {
    let (w, h) = (w as usize, h as usize);
    out.clear();
    out.resize(w * h * 3 / 2, 0);
    if w < 2 || h < 2 || bgra.len() < w * h * 4 {
        return;
    }
    let (y_plane, uv_plane) = out.split_at_mut(w * h);
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            let (b, g, r) = (i32::from(bgra[o]), i32::from(bgra[o + 1]), i32::from(bgra[o + 2]));
            y_plane[y * w + x] = (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16).clamp(0, 255) as u8;
        }
    }
    for cy in 0..h / 2 {
        for cx in 0..w / 2 {
            let (mut sb, mut sg, mut sr) = (0i32, 0i32, 0i32);
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let o = ((cy * 2 + dy) * w + cx * 2 + dx) * 4;
                sb += i32::from(bgra[o]);
                sg += i32::from(bgra[o + 1]);
                sr += i32::from(bgra[o + 2]);
            }
            let (b, g, r) = ((sb + 2) / 4, (sg + 2) / 4, (sr + 2) / 4);
            let u = (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
            let v = (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
            uv_plane[cy * w + cx * 2] = u;
            uv_plane[cy * w + cx * 2 + 1] = v;
        }
    }
}

/// Dónde empieza cada NAL de un flujo Annex B: `(posición del código de inicio, posición del primer byte del NAL)`.
fn nal_starts(data: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            // Un cero de más delante también es parte del código de inicio.
            let start = if i > 0 && data[i - 1] == 0 { i - 1 } else { i };
            out.push((start, i + 3));
            i += 3;
        } else {
            i += 1;
        }
    }
    out
}

/// El tipo de cada NAL de un flujo Annex B (7 = SPS, 8 = PPS, 5 = fotograma clave).
#[cfg(test)]
pub fn nal_types(data: &[u8]) -> Vec<u8> {
    nal_starts(data).into_iter().filter_map(|(_, p)| data.get(p).map(|b| b & 0x1F)).collect()
}

/// Las cabeceras (SPS y PPS, con su código de inicio de cuatro bytes) que lleve un fotograma clave. `None` si no
/// lleva las dos.
pub fn extract_sps_pps(data: &[u8]) -> Option<Vec<u8>> {
    let starts = nal_starts(data);
    let (mut sps, mut pps): (Option<Vec<u8>>, Option<Vec<u8>>) = (None, None);
    for (i, &(_, p)) in starts.iter().enumerate() {
        let Some(&b) = data.get(p) else { continue };
        let end = starts.get(i + 1).map_or(data.len(), |n| n.0);
        let mut nal = vec![0, 0, 0, 1];
        nal.extend_from_slice(&data[p..end]);
        match b & 0x1F {
            7 if sps.is_none() => sps = Some(nal),
            8 if pps.is_none() => pps = Some(nal),
            _ => {}
        }
    }
    let mut out = sps?;
    out.extend(pps?);
    Some(out)
}

/// Lo que hace falta saber del vídeo comprimido para escribirlo en un MP4.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StreamInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
    /// SPS y PPS (`MF_MT_MPEG_SEQUENCE_HEADER`).
    pub header: Vec<u8>,
}

#[cfg(windows)]
pub use imp::{save_mp4, H264Mft};

#[cfg(not(windows))]
pub fn save_mp4(_path: &std::path::Path, _samples: &[super::ring::VSample], _info: &StreamInfo) -> Result<(), String> {
    Err("Vigilar solo funciona en Windows.".into())
}

#[cfg(windows)]
mod imp {
    use super::super::ring::VSample;
    use super::*;
    use std::mem::ManuallyDrop;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_MULTITHREADED};
    use windows::Win32::System::Variant::{VARIANT, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4};

    fn e(what: &str, err: windows::core::Error) -> String {
        format!("{what}: {err}")
    }

    fn pack(hi: u32, lo: u32) -> u64 {
        (u64::from(hi) << 32) | u64::from(lo)
    }

    fn variant_u32(v: u32) -> VARIANT {
        let mut var = VARIANT::default();
        var.Anonymous.Anonymous = ManuallyDrop::new(VARIANT_0_0 { vt: VT_UI4, wReserved1: 0, wReserved2: 0, wReserved3: 0, Anonymous: VARIANT_0_0_0 { ulVal: v } });
        var
    }

    fn variant_bool(v: bool) -> VARIANT {
        let mut var = VARIANT::default();
        var.Anonymous.Anonymous = ManuallyDrop::new(VARIANT_0_0 { vt: VT_BOOL, wReserved1: 0, wReserved2: 0, wReserved3: 0, Anonymous: VARIANT_0_0_0 { boolVal: windows::Win32::Foundation::VARIANT_BOOL::from(v) } });
        var
    }

    /// El codificador H.264. Se crea, se usa y se suelta en el mismo hilo (con COM y Media Foundation iniciados).
    pub struct H264Mft {
        mft: IMFTransform,
        w: u32,
        h: u32,
        fps: u32,
        bitrate: u32,
        provides_samples: bool,
        /// La muestra donde el codificador deja lo que sale (se reutiliza).
        out_sample: Option<IMFSample>,
        nv12: Vec<u8>,
        header: Option<Vec<u8>>,
        /// Para pedir un fotograma clave cuando el grupo de imágenes se alarga en tiempo (equipo lento).
        api: Option<ICodecAPI>,
        /// El instante (100 ns) del último fotograma clave pedido o recibido.
        last_key: Option<i64>,
        max_gap: i64,
    }

    impl H264Mft {
        /// Busca el codificador por programa y lo deja listo para `w`×`h` a `fps` fotogramas por segundo, `bitrate`
        /// bits por segundo y un fotograma clave cada `gop` fotogramas.
        pub unsafe fn new(w: u32, h: u32, fps: u32, bitrate: u32, gop: u32) -> Result<H264Mft, String> {
            unsafe {
                let mft = find_encoder()?;
                // Lo que el codificador admita de esto se aplica; lo que no, se anota y se sigue.
                let enc_api = mft.cast::<ICodecAPI>().ok();
                if let Some(api) = &enc_api {
                    let set = |name: &str, key: &windows::core::GUID, v: VARIANT| {
                        if let Err(x) = api.SetValue(key, &v) {
                            log::info!("Vigilar: el codificador no aceptó {name}: {x}");
                        }
                    };
                    set("tamaño del grupo de imágenes", &CODECAPI_AVEncMPVGOPSize, variant_u32(gop));
                    set("la tasa media", &CODECAPI_AVEncCommonMeanBitRate, variant_u32(bitrate));
                    set("baja latencia", &CODECAPI_AVLowLatencyMode, variant_bool(true));
                }
                let out = MFCreateMediaType().map_err(|x| e("Codificador", x))?;
                out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Codificador", x))?;
                out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(|x| e("Codificador", x))?;
                out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate).map_err(|x| e("Codificador", x))?;
                out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(|x| e("Codificador", x))?;
                out.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h)).map_err(|x| e("Codificador", x))?;
                out.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1)).map_err(|x| e("Codificador", x))?;
                out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(|x| e("Codificador", x))?;
                // Perfil base: sin fotogramas B, así el orden de salida es el de entrada y los tiempos no se mezclan.
                out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32).map_err(|x| e("Codificador", x))?;
                mft.SetOutputType(0, &out, 0).map_err(|x| e("El codificador no acepta la salida H.264", x))?;

                let input = MFCreateMediaType().map_err(|x| e("Codificador", x))?;
                input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Codificador", x))?;
                input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).map_err(|x| e("Codificador", x))?;
                input.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(|x| e("Codificador", x))?;
                input.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h)).map_err(|x| e("Codificador", x))?;
                input.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1)).map_err(|x| e("Codificador", x))?;
                input.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(|x| e("Codificador", x))?;
                mft.SetInputType(0, &input, 0).map_err(|x| e("El codificador no acepta la imagen", x))?;

                let info = mft.GetOutputStreamInfo(0).map_err(|x| e("Codificador", x))?;
                let provides = info.dwFlags & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32) != 0;
                let out_sample = if provides {
                    None
                } else {
                    let sample = MFCreateSample().map_err(|x| e("Codificador", x))?;
                    // Un fotograma clave de pantalla puede pesar bastante: nunca menos de 1 MB.
                    let buffer = MFCreateMemoryBuffer(info.cbSize.max(1 << 20)).map_err(|x| e("Codificador", x))?;
                    sample.AddBuffer(&buffer).map_err(|x| e("Codificador", x))?;
                    Some(sample)
                };
                mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0).map_err(|x| e("Codificador", x))?;
                mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0).map_err(|x| e("Codificador", x))?;
                let api = enc_api;
                let mut enc = H264Mft { mft, w, h, fps, bitrate, provides_samples: provides, out_sample, nv12: Vec::new(), header: None, api, last_key: None, max_gap: i64::from(gop) * 10_000_000 / i64::from(fps.max(1)) };
                enc.read_header();
                Ok(enc)
            }
        }

        /// SPS y PPS, cuando se conocen (el codificador los da con su tipo de salida o con el primer fotograma clave).
        pub fn stream_info(&self) -> Option<StreamInfo> {
            self.header.clone().map(|header| StreamInfo { width: self.w, height: self.h, fps: self.fps, bitrate: self.bitrate, header })
        }

        unsafe fn read_header(&mut self) {
            unsafe {
                let Ok(t) = self.mft.GetOutputCurrentType(0) else { return };
                let Ok(n) = t.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) else { return };
                if n == 0 {
                    return;
                }
                let mut buf = vec![0u8; n as usize];
                if t.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut buf, None).is_ok() {
                    self.header = Some(buf);
                }
            }
        }

        /// Codifica una imagen BGRA (`w`×`h`) con su instante y duración (100 ns) y deja en `out` lo que salga
        /// (puede no salir nada todavía, o varias muestras).
        pub unsafe fn encode(&mut self, bgra: &[u8], time: i64, duration: i64, out: &mut Vec<VSample>) -> Result<(), String> {
            unsafe {
                bgra_to_nv12(bgra, self.w, self.h, &mut self.nv12);
                let buffer = MFCreateMemoryBuffer(self.nv12.len() as u32).map_err(|x| e("Memoria del vídeo", x))?;
                let mut ptr: *mut u8 = std::ptr::null_mut();
                buffer.Lock(&mut ptr, None, None).map_err(|x| e("Memoria del vídeo", x))?;
                std::ptr::copy_nonoverlapping(self.nv12.as_ptr(), ptr, self.nv12.len());
                buffer.Unlock().map_err(|x| e("Memoria del vídeo", x))?;
                buffer.SetCurrentLength(self.nv12.len() as u32).map_err(|x| e("Memoria del vídeo", x))?;
                let sample = MFCreateSample().map_err(|x| e("Fotograma", x))?;
                sample.AddBuffer(&buffer).map_err(|x| e("Fotograma", x))?;
                sample.SetSampleTime(time).map_err(|x| e("Fotograma", x))?;
                sample.SetSampleDuration(duration).map_err(|x| e("Fotograma", x))?;
                // El grupo de imágenes se cuenta en fotogramas: si el equipo va lento y llegan menos, los claves se
                // espaciarían más de la cuenta. Se piden por tiempo.
                if self.last_key.is_none_or(|k| time - k >= self.max_gap) {
                    if let Some(api) = &self.api {
                        if api.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &variant_u32(1)).is_ok() {
                            self.last_key = Some(time);
                        }
                    }
                }
                if let Err(x) = self.mft.ProcessInput(0, &sample, 0) {
                    // Si el codificador tiene cosas sin recoger, se recogen y se vuelve a intentar.
                    if x.code() != MF_E_NOTACCEPTING {
                        return Err(e("No se pudo codificar un fotograma", x));
                    }
                    self.pull(out)?;
                    self.mft.ProcessInput(0, &sample, 0).map_err(|x| e("No se pudo codificar un fotograma", x))?;
                }
                self.pull(out)
            }
        }

        /// Recoge lo que haya listo.
        unsafe fn pull(&mut self, out: &mut Vec<VSample>) -> Result<(), String> {
            unsafe {
                loop {
                    let given = if self.provides_samples { None } else { self.out_sample.clone() };
                    if let Some(s) = &given {
                        if let Ok(b) = s.GetBufferByIndex(0) {
                            let _ = b.SetCurrentLength(0);
                        }
                    }
                    let mut bufs = [MFT_OUTPUT_DATA_BUFFER { dwStreamID: 0, pSample: ManuallyDrop::new(given), dwStatus: 0, pEvents: ManuallyDrop::new(None) }];
                    let mut status = 0u32;
                    let r = self.mft.ProcessOutput(0, &mut bufs, &mut status);
                    // Lo que devuelve el codificador pasa a ser nuestro, para soltarlo en su sitio.
                    let sample = ManuallyDrop::take(&mut bufs[0].pSample);
                    let _events = ManuallyDrop::take(&mut bufs[0].pEvents);
                    match r {
                        Ok(()) => {
                            let Some(sample) = sample else { return Ok(()) };
                            let v = read_sample(&sample)?;
                            if v.key {
                                self.last_key = Some(v.time);
                            }
                            if self.header.is_none() {
                                self.read_header();
                            }
                            if self.header.is_none() && v.key {
                                self.header = extract_sps_pps(&v.data);
                            }
                            out.push(v);
                        }
                        Err(x) if x.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                        Err(x) if x.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                            // El tipo de salida cambió (rarísimo): se acepta el que ofrece.
                            let t = self.mft.GetOutputAvailableType(0, 0).map_err(|x| e("Codificador", x))?;
                            self.mft.SetOutputType(0, &t, 0).map_err(|x| e("Codificador", x))?;
                            self.read_header();
                        }
                        Err(x) => return Err(e("No se pudo recoger un fotograma codificado", x)),
                    }
                }
            }
        }
    }

    /// Lee bytes, instante, duración y si es clave de una muestra que dejó el codificador.
    unsafe fn read_sample(sample: &IMFSample) -> Result<VSample, String> {
        unsafe {
            let buffer = sample.ConvertToContiguousBuffer().map_err(|x| e("Fotograma codificado", x))?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut len = 0u32;
            buffer.Lock(&mut ptr, None, Some(&mut len)).map_err(|x| e("Fotograma codificado", x))?;
            let data = std::slice::from_raw_parts(ptr, len as usize).to_vec();
            buffer.Unlock().map_err(|x| e("Fotograma codificado", x))?;
            let key = sample.GetUINT32(&MFSampleExtension_CleanPoint).map(|v| v != 0).unwrap_or(false);
            Ok(VSample { data, time: sample.GetSampleTime().unwrap_or(0), duration: sample.GetSampleDuration().unwrap_or(0), key })
        }
    }

    /// El primer codificador H.264 por programa (síncrono) que se deja crear.
    unsafe fn find_encoder() -> Result<IMFTransform, String> {
        unsafe {
            let info = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
            let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
            let mut count = 0u32;
            MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_SORTANDFILTER, None, Some(&info), &mut list, &mut count).map_err(|x| e("No se pudo buscar el codificador de vídeo", x))?;
            let mut found: Option<IMFTransform> = None;
            let mut last = String::new();
            for i in 0..count as usize {
                // Cada elemento se toma (y se suelta) uno a uno: la lista la reservó Windows con una referencia por elemento.
                let activate = std::ptr::read(list.add(i));
                if found.is_none() {
                    if let Some(a) = &activate {
                        match a.ActivateObject::<IMFTransform>() {
                            Ok(t) => found = Some(t),
                            Err(x) => last = x.to_string(),
                        }
                    }
                }
            }
            if !list.is_null() {
                CoTaskMemFree(Some(list as *const std::ffi::c_void));
            }
            found.ok_or_else(|| if last.is_empty() { "Este Windows no tiene un codificador de vídeo H.264 por programa.".to_string() } else { format!("No se pudo preparar el codificador de vídeo: {last}") })
        }
    }

    /// Escribe `samples` (empiezan en un fotograma clave) en un MP4 sin recodificar, con los tiempos llevados a 0.
    /// Inicia y cierra por sí misma COM y Media Foundation en este hilo.
    pub fn save_mp4(path: &std::path::Path, samples: &[VSample], info: &StreamInfo) -> Result<(), String> {
        if samples.is_empty() {
            return Err("No hay nada que guardar.".into());
        }
        if info.header.is_empty() {
            return Err("Todavía no se conoce la cabecera del vídeo.".into());
        }
        // SAFETY: COM y Media Foundation se inician y se cierran en este hilo; todo lo creado en `write` se suelta antes.
        unsafe {
            let com = CoInitializeEx(None, COINIT_MULTITHREADED);
            let started = MFStartup(MF_VERSION, MFSTARTUP_FULL);
            let result = match &started {
                Ok(()) => write(path, samples, info),
                Err(x) => Err(e("No se pudo iniciar el codificador de vídeo de Windows", x.clone())),
            };
            if started.is_ok() {
                let _ = MFShutdown();
            }
            if com.is_ok() {
                CoUninitialize();
            }
            result
        }
    }

    unsafe fn write(path: &std::path::Path, samples: &[VSample], info: &StreamInfo) -> Result<(), String> {
        unsafe {
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 3).map_err(|x| e("No se pudo preparar el vídeo", x))?;
            let attrs = attrs.ok_or("No se pudo preparar el vídeo.")?;
            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 0).map_err(|x| e("Vídeo", x))?;
            attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1).map_err(|x| e("Vídeo", x))?;
            // El archivo se escribe con la extensión «.part»: se le dice el contenedor a mano.
            attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4).map_err(|x| e("Vídeo", x))?;
            let url: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            let writer = MFCreateSinkWriterFromURL(PCWSTR(url.as_ptr()), None::<&IMFByteStream>, &attrs).map_err(|x| e("No se pudo crear el vídeo", x))?;

            // El mismo tipo a la entrada y a la salida: el escritor no recodifica nada.
            let ty = MFCreateMediaType().map_err(|x| e("Vídeo", x))?;
            ty.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|x| e("Vídeo", x))?;
            ty.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(|x| e("Vídeo", x))?;
            ty.SetUINT32(&MF_MT_AVG_BITRATE, info.bitrate).map_err(|x| e("Vídeo", x))?;
            ty.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(|x| e("Vídeo", x))?;
            ty.SetUINT64(&MF_MT_FRAME_SIZE, pack(info.width, info.height)).map_err(|x| e("Vídeo", x))?;
            ty.SetUINT64(&MF_MT_FRAME_RATE, pack(info.fps, 1)).map_err(|x| e("Vídeo", x))?;
            ty.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(|x| e("Vídeo", x))?;
            ty.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32).map_err(|x| e("Vídeo", x))?;
            ty.SetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &info.header).map_err(|x| e("Vídeo", x))?;
            let stream = writer.AddStream(&ty).map_err(|x| e("No se pudo crear el vídeo H.264", x))?;
            writer.SetInputMediaType(stream, &ty, None::<&IMFAttributes>).map_err(|x| e("El escritor no acepta el vídeo ya codificado", x))?;
            writer.BeginWriting().map_err(|x| e("No se pudo empezar a escribir el vídeo", x))?;

            let t0 = samples[0].time;
            for (i, s) in samples.iter().enumerate() {
                // La duración llega hasta la muestra siguiente (así nada se solapa ni deja huecos).
                let duration = samples.get(i + 1).map_or(s.duration, |n| n.time - s.time).max(1);
                let buffer = MFCreateMemoryBuffer(s.data.len() as u32).map_err(|x| e("Memoria del vídeo", x))?;
                let mut ptr: *mut u8 = std::ptr::null_mut();
                buffer.Lock(&mut ptr, None, None).map_err(|x| e("Memoria del vídeo", x))?;
                std::ptr::copy_nonoverlapping(s.data.as_ptr(), ptr, s.data.len());
                buffer.Unlock().map_err(|x| e("Memoria del vídeo", x))?;
                buffer.SetCurrentLength(s.data.len() as u32).map_err(|x| e("Memoria del vídeo", x))?;
                let sample = MFCreateSample().map_err(|x| e("Fotograma", x))?;
                sample.AddBuffer(&buffer).map_err(|x| e("Fotograma", x))?;
                sample.SetSampleTime(s.time - t0).map_err(|x| e("Fotograma", x))?;
                sample.SetSampleDuration(duration).map_err(|x| e("Fotograma", x))?;
                if s.key {
                    sample.SetUINT32(&MFSampleExtension_CleanPoint, 1).map_err(|x| e("Fotograma", x))?;
                }
                if i == 0 {
                    sample.SetUINT32(&MFSampleExtension_Discontinuity, 1).map_err(|x| e("Fotograma", x))?;
                }
                writer.WriteSample(stream, &sample).map_err(|x| e("No se pudo escribir un fotograma", x))?;
            }
            writer.Finalize().map_err(|x| e("No se pudo cerrar el vídeo", x))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, bgr: [u8; 3]) -> Vec<u8> {
        (0..w * h).flat_map(|_| [bgr[0], bgr[1], bgr[2], 255]).collect()
    }

    fn convert(bgr: [u8; 3]) -> (u8, u8, u8) {
        let mut out = Vec::new();
        bgra_to_nv12(&solid(4, 4, bgr), 4, 4, &mut out);
        assert_eq!(out.len(), 4 * 4 * 3 / 2);
        // Todos los puntos de un color liso salen iguales.
        assert!(out[..16].iter().all(|&y| y == out[0]));
        assert!(out[16..].chunks(2).all(|uv| uv == &out[16..18]));
        (out[0], out[16], out[17])
    }

    #[test]
    fn nv12_de_colores_conocidos() {
        assert_eq!(convert([0, 0, 0]), (16, 128, 128));
        let (y, u, v) = convert([255, 255, 255]);
        assert_eq!((y, u, v), (235, 128, 128));
        // Rojo (en BGRA: B=0, G=0, R=255): luminosidad baja, mucho V.
        let (y, u, v) = convert([0, 0, 255]);
        assert!((80..=84).contains(&y) && (88..=92).contains(&u) && (238..=241).contains(&v), "{y} {u} {v}");
        // Azul: mucho U.
        let (y, u, v) = convert([255, 0, 0]);
        assert!((39..=42).contains(&y) && (238..=241).contains(&u) && (108..=112).contains(&v), "{y} {u} {v}");
    }

    #[test]
    fn nv12_conserva_la_orientacion_y_promedia_el_color() {
        // Arriba blanco, abajo negro, en 4×4: los dos planos lo respetan.
        let mut img = Vec::new();
        for y in 0..4 {
            img.extend(solid(4, 1, if y < 2 { [255, 255, 255] } else { [0, 0, 0] }));
        }
        let mut out = Vec::new();
        bgra_to_nv12(&img, 4, 4, &mut out);
        assert_eq!((out[0], out[15]), (235, 16));
        // Un tablero de 2×2 (blanco y negro) da gris en el color y sin color propio.
        let one: Vec<u8> = [[255u8, 255, 255, 255], [0, 0, 0, 255], [0, 0, 0, 255], [255, 255, 255, 255]].concat();
        bgra_to_nv12(&one, 2, 2, &mut out);
        assert_eq!(out.len(), 6);
        assert_eq!((out[4], out[5]), (128, 128));
        // Datos de menos o medidas imposibles: no revienta.
        bgra_to_nv12(&[0u8; 8], 4, 4, &mut out);
        assert_eq!(out.len(), 24);
        bgra_to_nv12(&[], 0, 0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn busca_la_cabecera_en_un_flujo_annex_b() {
        let sps = [0x67, 0x42, 0xC0, 0x1F, 0xDA];
        let pps = [0x68, 0xCE, 0x3C, 0x80];
        let idr = [0x65, 0x88, 0x84, 0x00, 0x10];
        let mut stream = vec![0, 0, 0, 1];
        stream.extend(sps);
        stream.extend([0, 0, 1]);
        stream.extend(pps);
        stream.extend([0, 0, 0, 1]);
        stream.extend(idr);
        assert_eq!(nal_types(&stream), vec![7, 8, 5]);
        let mut want = vec![0, 0, 0, 1];
        want.extend(sps);
        want.extend([0, 0, 0, 1]);
        want.extend(pps);
        assert_eq!(extract_sps_pps(&stream), Some(want));
        // Sin SPS o sin PPS, nada.
        let mut only_idr = vec![0, 0, 0, 1];
        only_idr.extend(idr);
        assert_eq!(extract_sps_pps(&only_idr), None);
        assert_eq!(nal_types(&only_idr), vec![5]);
        assert_eq!(extract_sps_pps(&[]), None);
        assert_eq!(extract_sps_pps(&[0, 0, 1]), None);
    }
}
