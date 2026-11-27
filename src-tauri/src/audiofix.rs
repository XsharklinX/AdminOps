//! «No me oyen en Teams» (y «no se oye»): lo que suele pasar de verdad no es el servicio de audio,
//! sino que el micrófono predeterminado es el de la webcam del cajón, que está al 4 % o en silencio,
//! o que Windows no deja usarlo a las aplicaciones. Aquí:
//!
//! - cada micrófono y altavoz con su nivel, su volumen y si es el predeterminado;
//! - escuchar unos segundos a todos los micrófonos a la vez («habla ahora»), con una barra de voz en
//!   vivo por cada uno (`audio://levels`), para ver cuál te oye;
//! - un pitido de prueba por un altavoz concreto;
//! - «Usar este para todo»: predeterminado para todo y para las llamadas, volumen y sin silencio;
//! - el permiso de micrófono (el del equipo, el de las aplicaciones de escritorio y el de Teams nuevo).
//!
//! Lo predeterminado y los permisos de aplicaciones son de la cuenta: se cambian los del usuario con
//! la sesión abierta (ver `target_user`), salvo lo predeterminado, que Windows guarda para la cuenta
//! que ejecuta AdminOps.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

pub const LEVELS_EVENT: &str = "audio://levels";
static LISTENING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AudioDev {
    pub id: String,
    pub name: String,
    /// Predeterminado para todo (consola/multimedia).
    pub default: bool,
    /// Predeterminado para las llamadas (comunicaciones).
    pub default_comm: bool,
    /// 0–100.
    pub volume: u32,
    pub muted: bool,
}

#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MicPrivacy {
    /// «Acceso al micrófono» del equipo (HKLM).
    pub device: bool,
    /// «Permitir que las aplicaciones accedan al micrófono» del usuario.
    pub apps: bool,
    /// «Permitir que las aplicaciones de escritorio accedan» (Teams clásico, Zoom, el navegador…).
    pub desktop: bool,
    /// Teams nuevo (MSTeams de la Tienda), si está: ¿tiene permiso?
    pub teams_new: Option<bool>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AudioView {
    pub capture: Vec<AudioDev>,
    pub render: Vec<AudioDev>,
    pub privacy: MicPrivacy,
}

const CONSENT: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";
const TEAMS_PKG: &str = "MSTeams_8wekyb3d8bbwe";

/// La rama del usuario con la sesión abierta (HKU\<sid>) o HKCU.
fn user_root() -> String {
    match crate::target_user::hkcu_redirect() {
        Some(sid) => format!(r"HKU\{sid}"),
        None => "HKCU".into(),
    }
}

fn allowed(path: &str) -> Option<bool> {
    crate::tweaks::registry::read_string(path, "Value").map(|v| !v.eq_ignore_ascii_case("Deny"))
}

pub fn privacy() -> MicPrivacy {
    let u = format!(r"{}\{CONSENT}", user_root());
    let teams_key = format!(r"{u}\{TEAMS_PKG}");
    MicPrivacy {
        // Sin valor = lo de fábrica, que es permitir.
        device: allowed(&format!(r"HKLM\{CONSENT}")).unwrap_or(true),
        apps: allowed(&u).unwrap_or(true),
        desktop: allowed(&format!(r"{u}\NonPackaged")).unwrap_or(true),
        teams_new: if crate::tweaks::registry::key_exists(&teams_key) { Some(allowed(&teams_key).unwrap_or(true)) } else { None },
    }
}

/// Da todos los permisos de micrófono que falten. Devuelve qué cambió.
pub fn allow_mic() -> Result<Vec<String>, String> {
    use crate::tweaks::model::{RegData, RegKind};
    use crate::tweaks::registry::write;
    let u = format!(r"{}\{CONSENT}", user_root());
    let p = privacy();
    let mut done = Vec::new();
    let allow = |path: &str| write(path, "Value", RegKind::String, &RegData::Str("Allow".into()));
    if !p.device {
        if !crate::elevation::is_elevated() {
            return Err("El micrófono está apagado para todo el equipo: hace falta abrir AdminOps como administrador para encenderlo.".into());
        }
        allow(&format!(r"HKLM\{CONSENT}"))?;
        done.push("Micrófono encendido para el equipo".to_string());
    }
    if !p.apps {
        allow(&u)?;
        done.push("Las aplicaciones pueden usar el micrófono".to_string());
    }
    if !p.desktop {
        allow(&format!(r"{u}\NonPackaged"))?;
        done.push("Las aplicaciones de escritorio pueden usar el micrófono".to_string());
    }
    if p.teams_new == Some(false) {
        allow(&format!(r"{u}\{TEAMS_PKG}"))?;
        done.push("Teams puede usar el micrófono".to_string());
    }
    Ok(done)
}

#[tauri::command]
pub async fn audio_view() -> Result<AudioView, String> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(windows)]
        {
            let _com = imp::Com::init();
            Ok(AudioView { capture: imp::list(true)?, render: imp::list(false)?, privacy: privacy() })
        }
        #[cfg(not(windows))]
        Err("Solo en Windows.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Escucha todos los micrófonos `secs` segundos y manda su nivel cada décima (`audio://levels`:
/// lista de `[id, pico 0–1]`; `null` si ese micrófono no se pudo abrir).
#[tauri::command]
pub fn audio_listen(app: tauri::AppHandle, secs: u32) -> Result<(), String> {
    if LISTENING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let secs = secs.clamp(2, 20);
    std::thread::spawn(move || {
        #[cfg(windows)]
        imp::listen(&app, secs);
        #[cfg(not(windows))]
        let _ = (&app, secs);
        LISTENING.store(false, Ordering::SeqCst);
        let _ = app.emit(LEVELS_EVENT, Option::<Vec<(String, Option<f32>)>>::None);
    });
    Ok(())
}

/// Un pitido corto por ese altavoz.
#[tauri::command]
pub async fn audio_beep(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            let _com = imp::Com::init();
            imp::beep(&id)
        }
        #[cfg(not(windows))]
        {
            let _ = id;
            Err("Solo en Windows.".to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// «Usar este para todo»: predeterminado (también para llamadas), volumen y sin silencio; en un
/// micrófono, además, los permisos que falten. Devuelve lo que hizo, frase a frase.
#[tauri::command]
pub async fn audio_use(app: tauri::AppHandle, id: String, capture: bool, volume: u32) -> Result<Vec<String>, String> {
    let r = audio_use_inner(id, capture, volume).await;
    crate::tweaks::record_app(&app, if capture { "Audio: micrófono para todo" } else { "Audio: altavoz para todo" }, &r);
    r
}

async fn audio_use_inner(id: String, capture: bool, volume: u32) -> Result<Vec<String>, String> {
    let volume = volume.clamp(10, 100);
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            let _com = imp::Com::init();
            let mut done = imp::make_default(&id)?;
            done.extend(imp::set_volume(&id, volume)?);
            if capture {
                done.extend(allow_mic()?);
            }
            log::info!("Audio: {} → {}", if capture { "micrófono" } else { "altavoz" }, done.join("; "));
            Ok(done)
        }
        #[cfg(not(windows))]
        {
            let _ = (id, capture, volume);
            Err("Solo en Windows.".to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// «Micrófono (Realtek(R) Audio)» → «Realtek(R) Audio»: lo que va entre el último paréntesis de fuera.
pub fn gadget(name: &str) -> String {
    let n = name.trim();
    if !n.ends_with(')') {
        return n.to_string();
    }
    let mut depth = 0;
    for (i, ch) in n.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    let inner = n[i + 1..n.len() - 1].trim();
                    return if inner.is_empty() { n.to_string() } else { inner.to_string() };
                }
            }
            _ => {}
        }
    }
    n.to_string()
}

/// Para «Preparar llamada»: qué micrófono usarán las llamadas y si puede usarse.
pub fn call_mic_step() -> crate::rocketplus::CallStep {
    use crate::rocketplus::CallStep;
    #[cfg(windows)]
    {
        let _com = imp::Com::init();
        let devs = imp::list(true).unwrap_or_default();
        let p = privacy();
        let Some(d) = devs.iter().find(|d| d.default_comm).or_else(|| devs.iter().find(|d| d.default)) else {
            return CallStep { ok: false, text: "No hay ningún micrófono conectado".into() };
        };
        let name = gadget(&d.name);
        if !(p.device && p.apps && p.desktop && p.teams_new != Some(false)) {
            return CallStep { ok: false, text: format!("Micrófono «{name}»: falta el permiso (Solucionar problemas → No se oye o no me oyen)") };
        }
        if d.muted || d.volume < 50 {
            return CallStep { ok: false, text: format!("Micrófono «{name}» {}", if d.muted { "en silencio".to_string() } else { format!("al {} %", d.volume) }) };
        }
        CallStep { ok: true, text: format!("Micrófono para las llamadas: «{name}» al {} %", d.volume) }
    }
    #[cfg(not(windows))]
    CallStep { ok: false, text: "Solo en Windows".into() }
}

/// «Escúchate como en una llamada»: graba `secs` segundos de ese micrófono y los reproduce por ese
/// altavoz o cascos. Devuelve el pico de la grabación (0–1) para decir si se oyó algo.
#[tauri::command]
pub async fn audio_echo(mic: String, speaker: String, secs: u32) -> Result<f32, String> {
    if LISTENING.load(Ordering::SeqCst) {
        return Err("Espera a que termine de escuchar los micrófonos.".into());
    }
    let secs = secs.clamp(2, 10);
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            let _com = imp::Com::init();
            let samples = imp::record(&mic, secs)?;
            let peak = samples.iter().map(|s| (f32::from(*s) / f32::from(i16::MAX)).abs()).fold(0.0, f32::max);
            imp::play(&speaker, &samples)?;
            Ok(peak)
        }
        #[cfg(not(windows))]
        {
            let _ = (mic, speaker, secs);
            Err("Solo en Windows.".to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn audio_allow_mic(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let r = allow_mic();
    crate::tweaks::record_app(&app, "Audio: permiso de micrófono para las aplicaciones", &r);
    r
}

#[cfg(windows)]
mod imp {
    use super::{AudioDev, LEVELS_EVENT};
    use std::time::{Duration, Instant};
    use tauri::Emitter;
    use windows::core::{GUID, HRESULT, PCWSTR};
    use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioMeterInformation};
    use windows::Win32::Media::Audio::{
        eCapture, eCommunications, eConsole, eMultimedia, eRender, IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, DEVICE_STATE_ACTIVE, WAVEFORMATEX, WAVE_FORMAT_PCM,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantToBSTR;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
    use windows::Win32::Foundation::PROPERTYKEY;

    /// COM de este hilo, mientras dure.
    pub struct Com(bool);
    impl Com {
        pub fn init() -> Com {
            // SAFETY: se empareja con CoUninitialize en Drop si tuvo éxito.
            Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
        }
    }
    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: este hilo inicializó COM.
                unsafe { CoUninitialize() };
            }
        }
    }

    const PKEY_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };

    fn err(e: windows::core::Error) -> String {
        if e.code().0 as u32 == 0x8007_0005 {
            "Windows no deja usar ese dispositivo (privacidad del micrófono).".into()
        } else {
            format!("Error de audio de Windows: {}", e.message().trim())
        }
    }

    fn enumerator() -> Result<IMMDeviceEnumerator, String> {
        // SAFETY: COM inicializado por quien llama.
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(err)
    }

    fn id_of(d: &IMMDevice) -> String {
        // SAFETY: GetId reserva la cadena con CoTaskMemAlloc; se libera aquí.
        unsafe {
            match d.GetId() {
                Ok(p) => {
                    let s = p.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(p.0 as _));
                    s
                }
                Err(_) => String::new(),
            }
        }
    }

    fn name_of(d: &IMMDevice) -> String {
        // SAFETY: lectura de una propiedad del dispositivo.
        unsafe {
            d.OpenPropertyStore(STGM_READ)
                .and_then(|s| s.GetValue(&PKEY_FRIENDLY_NAME))
                .and_then(|v| PropVariantToBSTR(&v))
                .map(|b| b.to_string())
                .unwrap_or_else(|_| "Dispositivo de audio".into())
        }
    }

    fn device(id: &str) -> Result<IMMDevice, String> {
        let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        // SAFETY: cadena terminada en cero que vive durante la llamada.
        unsafe { enumerator()?.GetDevice(PCWSTR(wide.as_ptr())) }.map_err(|_| "Ese dispositivo ya no está conectado.".to_string())
    }

    pub fn list(capture: bool) -> Result<Vec<AudioDev>, String> {
        let en = enumerator()?;
        let flow = if capture { eCapture } else { eRender };
        // SAFETY: llamadas de lectura sobre objetos COM vivos en este hilo.
        unsafe {
            let def = en.GetDefaultAudioEndpoint(flow, eConsole).ok().map(|d| id_of(&d));
            let comm = en.GetDefaultAudioEndpoint(flow, eCommunications).ok().map(|d| id_of(&d));
            let col = en.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE).map_err(err)?;
            let n = col.GetCount().map_err(err)?;
            let mut out = Vec::new();
            for i in 0..n {
                let Ok(d) = col.Item(i) else { continue };
                let id = id_of(&d);
                let (volume, muted) = match d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                    Ok(v) => ((v.GetMasterVolumeLevelScalar().unwrap_or(0.0) * 100.0).round() as u32, v.GetMute().map(|m| m.as_bool()).unwrap_or(false)),
                    Err(_) => (0, false),
                };
                out.push(AudioDev { name: name_of(&d), default: def.as_deref() == Some(id.as_str()), default_comm: comm.as_deref() == Some(id.as_str()), volume, muted, id });
            }
            Ok(out)
        }
    }

    /// Abre una captura de cada micrófono (si no, el medidor de Windows marca cero) y manda los picos.
    pub fn listen(app: &tauri::AppHandle, secs: u32) {
        let _com = Com::init();
        let Ok(en) = enumerator() else { return };
        let mics: Vec<(String, Option<(IAudioClient, IAudioMeterInformation)>)> = unsafe {
            let Ok(col) = en.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) else { return };
            let n = col.GetCount().unwrap_or(0);
            (0..n)
                .filter_map(|i| col.Item(i).ok())
                .map(|d| {
                    let id = id_of(&d);
                    let opened = (|| -> windows::core::Result<(IAudioClient, IAudioMeterInformation)> {
                        let client: IAudioClient = d.Activate(CLSCTX_ALL, None)?;
                        let fmt = client.GetMixFormat()?;
                        let r = client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 2_000_000, 0, fmt, None);
                        CoTaskMemFree(Some(fmt as _));
                        r?;
                        client.Start()?;
                        let meter: IAudioMeterInformation = d.Activate(CLSCTX_ALL, None)?;
                        Ok((client, meter))
                    })();
                    (id, opened.ok())
                })
                .collect()
        };
        let end = Instant::now() + Duration::from_secs(u64::from(secs));
        while Instant::now() < end {
            // SAFETY: medidores vivos en este hilo.
            let levels: Vec<(String, Option<f32>)> = mics.iter().map(|(id, m)| (id.clone(), m.as_ref().and_then(|(_, meter)| unsafe { meter.GetPeakValue() }.ok()))).collect();
            let _ = app.emit(LEVELS_EVENT, Some(levels));
            std::thread::sleep(Duration::from_millis(100));
        }
        for (_, m) in &mics {
            if let Some((client, _)) = m {
                // SAFETY: cliente vivo, se para antes de soltarlo.
                let _ = unsafe { client.Stop() };
            }
        }
    }

    /// 0,6 s de un «ding» a 660 Hz con caída suave, en 16 bits estéreo a 48 kHz (Windows lo convierte).
    pub fn beep(id: &str) -> Result<(), String> {
        const RATE: u32 = 48_000;
        let d = device(id)?;
        // SAFETY: objetos COM de este hilo; el búfer se pide y se suelta con su tamaño.
        unsafe {
            let client: IAudioClient = d.Activate(CLSCTX_ALL, None).map_err(err)?;
            let fmt = WAVEFORMATEX { wFormatTag: WAVE_FORMAT_PCM as u16, nChannels: 2, nSamplesPerSec: RATE, nAvgBytesPerSec: RATE * 4, nBlockAlign: 4, wBitsPerSample: 16, cbSize: 0 };
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, 10_000_000, 0, &fmt, None).map_err(err)?;
            let render: IAudioRenderClient = client.GetService().map_err(err)?;
            let size = client.GetBufferSize().map_err(err)?;
            let frames = (RATE as usize * 6 / 10).min(size as usize);
            let buf = render.GetBuffer(frames as u32).map_err(err)? as *mut i16;
            for i in 0..frames {
                let t = i as f32 / RATE as f32;
                let env = (1.0 - i as f32 / frames as f32).powi(2);
                let s = ((t * 660.0 * std::f32::consts::TAU).sin() * env * 0.35 * f32::from(i16::MAX)) as i16;
                *buf.add(i * 2) = s;
                *buf.add(i * 2 + 1) = s;
            }
            render.ReleaseBuffer(frames as u32, 0).map_err(err)?;
            client.Start().map_err(err)?;
            std::thread::sleep(Duration::from_millis(750));
            let _ = client.Stop();
        }
        Ok(())
    }

    const ECHO_RATE: u32 = 48_000;

    fn mono_format() -> WAVEFORMATEX {
        WAVEFORMATEX { wFormatTag: WAVE_FORMAT_PCM as u16, nChannels: 1, nSamplesPerSec: ECHO_RATE, nAvgBytesPerSec: ECHO_RATE * 2, nBlockAlign: 2, wBitsPerSample: 16, cbSize: 0 }
    }

    /// Graba `secs` segundos de ese micrófono.
    pub fn record(id: &str, secs: u32) -> Result<Vec<i16>, String> {
        use windows::Win32::Media::Audio::{IAudioCaptureClient, AUDCLNT_BUFFERFLAGS_SILENT};
        let d = device(id)?;
        let want = (ECHO_RATE * secs) as usize;
        let mut out: Vec<i16> = Vec::with_capacity(want);
        // SAFETY: objetos COM de este hilo; cada búfer se lee con su tamaño y se suelta.
        unsafe {
            let client: IAudioClient = d.Activate(CLSCTX_ALL, None).map_err(err)?;
            let fmt = mono_format();
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, 10_000_000, 0, &fmt, None).map_err(err)?;
            let capture: IAudioCaptureClient = client.GetService().map_err(err)?;
            client.Start().map_err(err)?;
            let end = Instant::now() + Duration::from_secs(u64::from(secs) + 2);
            while out.len() < want && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(10));
                loop {
                    let n = capture.GetNextPacketSize().map_err(err)?;
                    if n == 0 {
                        break;
                    }
                    let (mut data, mut frames, mut flags) = (std::ptr::null_mut::<u8>(), 0u32, 0u32);
                    capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None).map_err(err)?;
                    if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                        out.extend(std::iter::repeat_n(0i16, frames as usize));
                    } else {
                        out.extend_from_slice(std::slice::from_raw_parts(data as *const i16, frames as usize));
                    }
                    let _ = capture.ReleaseBuffer(frames);
                }
            }
            let _ = client.Stop();
        }
        out.truncate(want);
        Ok(out)
    }

    /// Reproduce la grabación por ese altavoz o cascos y espera a que termine.
    pub fn play(id: &str, samples: &[i16]) -> Result<(), String> {
        let d = device(id)?;
        // SAFETY: objetos COM de este hilo; solo se escribe dentro del hueco que da GetBuffer.
        unsafe {
            let client: IAudioClient = d.Activate(CLSCTX_ALL, None).map_err(err)?;
            let fmt = mono_format();
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, 5_000_000, 0, &fmt, None).map_err(err)?;
            let render: IAudioRenderClient = client.GetService().map_err(err)?;
            let size = client.GetBufferSize().map_err(err)? as usize;
            let mut pos = 0usize;
            let mut started = false;
            let end = Instant::now() + Duration::from_secs((samples.len() / ECHO_RATE as usize) as u64 + 3);
            while pos < samples.len() && Instant::now() < end {
                let pad = client.GetCurrentPadding().map_err(err)? as usize;
                let free = size.saturating_sub(pad).min(samples.len() - pos);
                if free > 0 {
                    let buf = render.GetBuffer(free as u32).map_err(err)? as *mut i16;
                    std::ptr::copy_nonoverlapping(samples[pos..].as_ptr(), buf, free);
                    render.ReleaseBuffer(free as u32, 0).map_err(err)?;
                    pos += free;
                }
                if !started {
                    client.Start().map_err(err)?;
                    started = true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            // Que suene lo que queda en el búfer.
            while client.GetCurrentPadding().map_or(0, |p| p) > 0 && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = client.Stop();
        }
        Ok(())
    }

    /// IPolicyConfig (sin documentar, pero el que usan Windows y todas las herramientas de audio):
    /// SetDefaultEndpoint es el método 13 de su tabla.
    pub fn make_default(id: &str) -> Result<Vec<String>, String> {
        const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
        const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
        type SetDefault = unsafe extern "system" fn(*mut core::ffi::c_void, PCWSTR, i32) -> HRESULT;
        let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        // SAFETY: objeto COM creado aquí; se llama a su método 13 con la firma de IPolicyConfig y se suelta.
        unsafe {
            let unknown: windows::core::IUnknown = CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL).map_err(err)?;
            let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
            windows::core::Interface::query(&unknown, &IID_POLICY_CONFIG, &mut raw).ok().map_err(err)?;
            let vtbl = *(raw as *const *const usize);
            let f: SetDefault = std::mem::transmute(*vtbl.add(13));
            let mut res = Ok(());
            for role in [eConsole, eMultimedia, eCommunications] {
                let hr = f(raw, PCWSTR(wide.as_ptr()), role.0);
                if hr.is_err() {
                    res = Err(err(hr.into()));
                }
            }
            // Release (método 2 de IUnknown).
            let release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 = std::mem::transmute(*vtbl.add(2));
            release(raw);
            res?;
        }
        let name = name_of(&device(id)?);
        Ok(vec![format!("«{name}» es el predeterminado, también para las llamadas")])
    }

    pub fn set_volume(id: &str, volume: u32) -> Result<Vec<String>, String> {
        let d = device(id)?;
        let mut done = Vec::new();
        // SAFETY: control de volumen del dispositivo, en este hilo.
        unsafe {
            let v: IAudioEndpointVolume = d.Activate(CLSCTX_ALL, None).map_err(err)?;
            if v.GetMute().map(|m| m.as_bool()).unwrap_or(false) {
                v.SetMute(false, std::ptr::null()).map_err(err)?;
                done.push("Quitado el silencio".to_string());
            }
            let now = (v.GetMasterVolumeLevelScalar().unwrap_or(0.0) * 100.0).round() as u32;
            // Solo se sube: si ya estaba más alto, se deja como lo tenía.
            if now < volume {
                v.SetMasterVolumeLevelScalar(volume as f32 / 100.0, std::ptr::null()).map_err(err)?;
                done.push(format!("Nivel del {now} % al {volume} %"));
            }
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn nombre_del_aparato() {
        assert_eq!(super::gadget("Micrófono (Realtek(R) Audio)"), "Realtek(R) Audio");
        assert_eq!(super::gadget("Micrófono (Jabra Evolve 20)"), "Jabra Evolve 20");
        assert_eq!(super::gadget("Altavoces"), "Altavoces");
    }
}
