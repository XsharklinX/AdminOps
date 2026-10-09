//! Clonado e imágenes de disco: los comandos de la interfaz. El algoritmo
//! (tres pasadas, ddrescue) está en `clone.rs`.
//!
//! El origen solo se lee. El destino es un archivo de imagen o un disco entero;
//! en un disco el contenido anterior se pierde, así que se pide escribir su
//! número, no vale el disco de Windows y se pone fuera de línea mientras se copia.

use crate::clone::{self, BlockSink, CloneState, FileSink};
use crate::rawdisk::{BlockSource, Mode, RawDisk};
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Target {
    /// image · disk
    pub kind: String,
    pub path: String,
    pub number: u32,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloneLive {
    pub cells: String,
    pub percent: u8,
    pub pass: u8,
    pub good: u64,
    pub lost: u64,
    pub pending: u64,
    pub mbps: f32,
    pub eta_secs: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloneResult {
    pub finished: bool,
    pub cells: String,
    pub size: u64,
    pub good: u64,
    pub lost: u64,
    pub pending: u64,
    pub lost_ranges: u32,
    /// ok · warn · bad
    pub level: String,
    pub text: String,
    pub target: String,
}

/// 1536 MB → «1,5 GB».
fn fmt(n: u64) -> String {
    let gb = n as f64 / 1_073_741_824.0;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", n as f64 / 1_048_576.0)
    }
}

static LIVE: Mutex<Option<HashMap<u32, CloneLive>>> = Mutex::new(None);

#[tauri::command]
pub fn disk_clone_live(number: u32) -> Option<CloneLive> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&number).cloned())
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    target: String,
    state: CloneState,
}

fn state_path(app: &tauri::AppHandle, number: u32) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join(format!("clonado-{number}.json"))
}

fn target_key(t: &Target) -> String {
    if t.kind == "disk" {
        format!("disco {}", t.number)
    } else {
        t.path.clone()
    }
}

fn describe(st: &CloneState, target: String) -> CloneResult {
    let (good, lost, pending) = st.totals();
    let finished = st.pass >= 4;
    let (level, text) = if !finished {
        ("ok", format!("Parado: se puede seguir donde se quedó (pasada {} de 3).", st.pass))
    } else if lost == 0 {
        ("ok", "Copia completa: se pudo leer todo el disco.".to_string())
    } else {
        ("warn", format!("Copiado el {:.2} %. No se pudo leer {} en {} zonas: en el destino están a ceros. Lo demás está a salvo.", good as f64 * 100.0 / st.size.max(1) as f64, fmt(lost), st.lost.len()))
    };
    CloneResult { finished, cells: st.cells(2000), size: st.size, good, lost, pending, lost_ranges: st.lost.len() as u32, level: level.into(), text, target }
}

/// Lo último que se hizo con este disco (terminado o a medias).
#[tauri::command]
pub fn disk_clone_last(app: tauri::AppHandle, number: u32) -> Option<CloneResult> {
    let s: Saved = crate::paths::read_json(&state_path(&app, number));
    (s.state.size > 0).then(|| describe(&s.state, s.target))
}

struct DiskSink(RawDisk);

impl BlockSink for DiskSink {
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u32> {
        self.0.write_at(offset, data)
    }
}

fn free_letter_disk(path: &str) -> Option<u32> {
    let l = path.trim().chars().next().filter(|c| c.is_ascii_alphabetic())?;
    crate::ps::powershell(&format!("(Get-Partition -DriveLetter {l} -ErrorAction Stop).DiskNumber")).ok()?.trim().parse().ok()
}

/// Clona el disco `number` (solo lectura) a una imagen o a otro disco, en tres pasadas.
/// Con `resume`, sigue donde se quedó. Tarea «disk-clone:N».
#[tauri::command(async)]
pub fn disk_clone(app: tauri::AppHandle, number: u32, target: Target, retries: u32, resume: bool, tweaks: tauri::State<'_, TweakState>) -> Result<CloneResult, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let mut src = RawDisk::open(number, Mode::Read)?;
    let size = src.len();
    let sector = src.sector();
    if size == 0 {
        return Err("No se pudo saber el tamaño del disco de origen.".into());
    }
    let tkey = target_key(&target);
    let mut dst: Box<dyn BlockSink + Send>;
    let mut offline: Option<u32> = None;
    if target.kind == "disk" {
        if target.number == number {
            return Err("El destino no puede ser el mismo disco de origen.".into());
        }
        if crate::partitionsio::is_system_disk(target.number) {
            return Err("El destino es el disco de Windows: se perdería el sistema.".into());
        }
        let probe = RawDisk::open(target.number, Mode::Read)?;
        if probe.len() < size {
            return Err(format!("El disco de destino ({}) es más pequeño que el de origen ({}).", fmt(probe.len()), fmt(size)));
        }
        drop(probe);
        // Fuera de línea: sin letras ni volúmenes montados que impidan escribir en bruto.
        crate::ps::powershell(&format!("Set-Disk -Number {} -IsOffline $true -ErrorAction Stop", target.number)).map_err(|e| format!("No se pudo poner el disco {} fuera de línea: {e}", target.number))?;
        offline = Some(target.number);
        match RawDisk::open(target.number, Mode::Write) {
            Ok(d) => dst = Box::new(DiskSink(d)),
            Err(e) => {
                let _ = crate::ps::powershell(&format!("Set-Disk -Number {} -IsOffline $false", target.number));
                return Err(e);
            }
        }
    } else {
        let path = std::path::PathBuf::from(target.path.trim());
        if path.as_os_str().is_empty() || path.is_dir() {
            return Err("Escribe el nombre del archivo de imagen (por ejemplo E:\\Imagenes\\cliente.img).".into());
        }
        if path.parent().is_none_or(|p| !p.is_dir()) {
            return Err("La carpeta del archivo de imagen no existe.".into());
        }
        if free_letter_disk(&target.path) == Some(number) {
            return Err("La imagen está en el mismo disco que se va a copiar. Elige otro disco para guardarla.".into());
        }
        dst = Box::new(FileSink::create(&path, size).map_err(|e| format!("No se pudo crear la imagen: {e}"))?);
    }

    let path = state_path(&app, number);
    let mut saved: Saved = crate::paths::read_json(&path);
    let mut st = if resume && saved.target == tkey && saved.state.size == size && saved.state.pass < 4 { saved.state.clone() } else { CloneState::new(size, sector) };
    saved.target = tkey.clone();

    let task = crate::task::Task::new(&app, format!("disk-clone:{number}")).named("Clonar el disco");
    task.step("Copiando lo que se lee bien primero…");
    let began = Instant::now();
    let start_good = st.totals().0;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut last_save = Instant::now();
    let finished = clone::clone(&mut src, &mut dst, &mut st, retries.clamp(1, 20), |s| {
        if last_emit.elapsed() >= Duration::from_millis(900) {
            last_emit = Instant::now();
            let (good, lost, pending) = s.totals();
            let secs = began.elapsed().as_secs_f64().max(0.5);
            let rate = good.saturating_sub(start_good) as f64 / secs;
            let eta = if rate > 1.0 { (pending as f64 / rate) as u64 } else { 0 };
            let pct = ((good + lost) * 100 / s.size.max(1)) as u8;
            LIVE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(number, CloneLive { cells: s.cells(2000), percent: pct, pass: s.pass, good, lost, pending, mbps: (rate / 1_048_576.0) as f32, eta_secs: eta });
            task.step(format!("Pasada {} de 3 · {pct} % · {:.0} MB/s", s.pass.min(3), rate / 1_048_576.0));
        }
        if last_save.elapsed() >= Duration::from_secs(30) {
            last_save = Instant::now();
            saved.state = s.clone();
            let _ = crate::paths::write_json(&path, &saved);
        }
        !task.cancelled()
    });
    if let Some(m) = LIVE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(&number);
    }
    saved.state = st.clone();
    let _ = crate::paths::write_json(&path, &saved);
    drop(dst);
    if let Some(n) = offline {
        let _ = crate::ps::powershell(&format!("Set-Disk -Number {n} -IsOffline $false -ErrorAction SilentlyContinue; Update-Disk -Number {n} -ErrorAction SilentlyContinue"));
    }
    let r = describe(&st, tkey.clone());
    let journal: Result<(), String> = Ok(());
    tweaks.record(Op::Run, &format!("Clonar el disco {number} a {tkey}: {}", if finished { r.text.clone() } else { "parado".into() }), &journal);
    Ok(r)
}
