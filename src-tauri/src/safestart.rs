//! Abrir a salvo (1.5, idea 10). Si AdminOps se cierra de golpe dos veces
//! seguidas, la siguiente vez abre sin nada en segundo plano (sin vigilar, sin
//! monitor en vivo y sin tareas nocturnas) y dice qué estaba haciendo.
//!
//! Cómo se sabe: al arrancar se apunta «abierta» en `session.json`; al salir
//! bien, «cerrada bien». Si al arrancar sigue «abierta», la vez anterior no
//! salió bien. No cuenta si Windows se ha reiniciado desde entonces (apagar el
//! equipo con AdminOps abierta no es un fallo suyo) ni si aquella AdminOps
//! sigue viva (otra ventana abierta a la vez).

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Cierres de golpe seguidos para abrir a salvo.
pub const BAD_CLOSES_FOR_SAFE: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    /// Abierta y sin salir bien todavía.
    pub open: bool,
    /// El proceso que la abrió.
    pub pid: u32,
    /// Cuándo arrancó (segundos Unix).
    pub started: u64,
    /// La pantalla en la que estaba (para «estaba en Pantallas»).
    pub page: String,
    /// Lo que hacía en segundo plano (grabar, vigilar, una tarea nocturna…).
    pub activities: Vec<String>,
    /// Cierres de golpe seguidos.
    pub bad_closes: u32,
    /// Qué estaba haciendo en cada uno de ellos (el último al final).
    pub bad_doing: Vec<String>,
}

/// Lo que se decide al arrancar.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    /// Abre a salvo.
    pub safe: bool,
    pub bad_closes: u32,
    /// Qué estaba haciendo en los cierres de golpe (vacío si no se sabe).
    pub doing: Vec<String>,
}

/// Lo que hacía una sesión, en una frase: «grabando la pantalla, en Pantallas».
pub fn doing_of(s: &Session) -> String {
    let mut parts: Vec<String> = s.activities.iter().filter(|a| !a.trim().is_empty()).cloned().collect();
    if !s.page.trim().is_empty() {
        parts.push(format!("en «{}»", s.page.trim()));
    }
    parts.join(", ")
}

/// Decide al arrancar. `prev`: lo que quedó apuntado; `boot`: cuándo arrancó Windows;
/// `prev_alive`: si el proceso de aquella sesión sigue vivo.
pub fn decide(prev: &Session, boot: u64, prev_alive: bool) -> (StartInfo, Session) {
    let crashed = prev.open && !prev_alive && prev.started >= boot && prev.started > 0;
    let (bad, mut doing) = if crashed {
        let mut d = prev.bad_doing.clone();
        d.push(doing_of(prev));
        (prev.bad_closes.saturating_add(1), d)
    } else if prev.open && prev_alive {
        // Otra AdminOps abierta a la vez: no es un cierre; se conserva la cuenta.
        (prev.bad_closes, prev.bad_doing.clone())
    } else {
        (0, Vec::new())
    };
    // Solo los dos últimos: es lo que se cuenta.
    if doing.len() > BAD_CLOSES_FOR_SAFE as usize {
        doing.drain(..doing.len() - BAD_CLOSES_FOR_SAFE as usize);
    }
    let safe = bad >= BAD_CLOSES_FOR_SAFE;
    let info = StartInfo { safe, bad_closes: bad, doing: if safe { doing.clone() } else { Vec::new() } };
    let next = Session { open: true, bad_closes: bad, bad_doing: doing, ..Default::default() };
    (info, next)
}

/// El texto del aviso: «Las dos veces estaba …» o «La primera vez … y la segunda …».
pub fn explain(doing: &[String]) -> String {
    let known: Vec<&String> = doing.iter().filter(|d| !d.trim().is_empty()).collect();
    match (doing.len(), known.len()) {
        (_, 0) => String::new(),
        (2, 2) if doing[0] == doing[1] => format!("Las dos veces estaba {}.", doing[0]),
        (2, 2) => format!("La primera vez estaba {} y la segunda, {}.", doing[0], doing[1]),
        _ => format!("La última vez estaba {}.", known[known.len() - 1]),
    }
}

// ---------- En disco ----------

static STATE: Mutex<Option<(std::path::PathBuf, Session)>> = Mutex::new(None);
static INFO: Mutex<Option<StartInfo>> = Mutex::new(None);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn save(path: &std::path::Path, s: &Session) {
    if let Err(e) = crate::paths::write_json(path, s) {
        log::warn!("Abrir a salvo: no se pudo guardar la sesión: {e}");
    }
}

fn alive(pid: u32) -> bool {
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    let mut sys = sysinfo::System::new();
    let p = sysinfo::Pid::from_u32(pid);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
    // Que sea AdminOps: el número de un proceso que murió puede tenerlo ahora otro programa.
    sys.process(p).and_then(|x| x.exe()).and_then(|e| e.file_name()).is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with("adminops"))
}

/// Al arrancar, antes de lanzar nada en segundo plano.
pub fn begin(app: &tauri::AppHandle) -> StartInfo {
    let path = crate::paths::machine_data_dir(app).join("session.json");
    let prev: Session = crate::paths::read_json(&path);
    let (info, mut next) = decide(&prev, sysinfo::System::boot_time(), alive(prev.pid));
    next.pid = std::process::id();
    next.started = now();
    save(&path, &next);
    if info.safe {
        let detail = explain(&info.doing);
        log::warn!("AdminOps se cerró de golpe {} veces seguidas: se abre a salvo. {detail}", info.bad_closes);
        crate::errorlog::note(app, "cierre", "arranque", &format!("Se cerró de golpe {} veces seguidas. {detail}", info.bad_closes));
    } else if info.bad_closes > 0 {
        log::warn!("La vez anterior AdminOps no se cerró bien ({})", doing_of(&prev));
    }
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some((path, next));
    *INFO.lock().unwrap_or_else(|e| e.into_inner()) = Some(info.clone());
    info
}

/// ¿Abrió a salvo? (para no arrancar lo de segundo plano).
pub fn is_safe() -> bool {
    INFO.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|i| i.safe)
}

fn update(f: impl FnOnce(&mut Session)) {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((path, s)) = g.as_mut() {
        let before = s.clone();
        f(s);
        if *s != before {
            save(path, s);
        }
    }
}

/// Empieza o termina algo en segundo plano que conviene recordar si se cierra de golpe.
pub fn activity(text: &str, on: bool) {
    update(|s| {
        s.activities.retain(|a| a != text);
        if on {
            s.activities.push(text.to_string());
        }
    });
}

/// Salió bien: la próxima vez arranca normal.
pub fn end_clean() {
    update(|s| {
        if s.pid == std::process::id() {
            *s = Session { pid: s.pid, started: s.started, ..Default::default() };
        }
    });
}

#[tauri::command]
pub fn safe_start_info() -> StartInfo {
    INFO.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
}

/// La pantalla en la que está (la interfaz lo dice al cambiar).
#[tauri::command]
pub fn session_page(label: String) {
    let label: String = label.chars().take(60).collect();
    update(|s| s.page = label);
}

/// «Abrir normal»: olvida los cierres y reinicia AdminOps. `request_restart` sale por
/// `RunEvent::Exit` (como cerrar): se cierra bien lo que haya en marcha y se apunta «cerrada bien».
/// (`restart` desde el hilo principal no pasa por Exit y el reinicio contaba como cierre de golpe.)
#[tauri::command(async)]
pub fn safe_start_leave(app: tauri::AppHandle) {
    update(|s| {
        s.bad_closes = 0;
        s.bad_doing.clear();
    });
    log::info!("Abrir a salvo: se reinicia en modo normal");
    app.request_restart();
}

/// «Copiar el informe del fallo»: lo de los cierres y el registro de errores.
#[tauri::command(async)]
pub fn safe_start_report(app: tauri::AppHandle) -> String {
    let info = safe_start_info();
    let mut out = format!("AdminOps se cerró de golpe {} veces seguidas y se abrió a salvo.\n", info.bad_closes);
    let detail = explain(&info.doing);
    if !detail.is_empty() {
        out.push_str(&detail);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&crate::errorlog::error_log_report(app));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(started: u64, bad: u32, doing: &[&str], activities: &[&str], page: &str) -> Session {
        Session {
            open: true,
            pid: 4242,
            started,
            page: page.into(),
            activities: activities.iter().map(|s| s.to_string()).collect(),
            bad_closes: bad,
            bad_doing: doing.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn dos_cierres_de_golpe_seguidos_abren_a_salvo() {
        let boot = 1000;
        // Primer cierre de golpe: aún normal.
        let (i1, s1) = decide(&open(2000, 0, &[], &["vigilando la pantalla (Grabar el fallo)"], "Grabar el fallo"), boot, false);
        assert!(!i1.safe);
        assert_eq!(i1.bad_closes, 1);
        assert!(s1.open && s1.activities.is_empty() && s1.page.is_empty());
        // Segundo: a salvo, con lo que hacía las dos veces.
        let prev = Session { started: 3000, activities: vec!["vigilando la pantalla (Grabar el fallo)".into()], page: "Grabar el fallo".into(), ..s1 };
        let (i2, _) = decide(&prev, boot, false);
        assert!(i2.safe);
        assert_eq!(i2.doing.len(), 2);
        assert_eq!(explain(&i2.doing), "Las dos veces estaba vigilando la pantalla (Grabar el fallo), en «Grabar el fallo».");
    }

    #[test]
    fn salir_bien_o_reiniciar_windows_pone_la_cuenta_a_cero() {
        let boot = 5000;
        // Cerrada bien.
        let (i, s) = decide(&Session { bad_closes: 1, ..Default::default() }, boot, false);
        assert!(!i.safe && i.bad_closes == 0 && s.bad_doing.is_empty());
        // Abierta pero de antes de reiniciar Windows: no es culpa de AdminOps.
        let (i, _) = decide(&open(4000, 1, &["x"], &[], ""), boot, false);
        assert!(!i.safe && i.bad_closes == 0);
    }

    #[test]
    fn otra_adminops_abierta_no_cuenta_como_cierre() {
        let (i, s) = decide(&open(6000, 1, &["grabando"], &[], ""), 1000, true);
        assert!(!i.safe);
        assert_eq!(i.bad_closes, 1);
        assert_eq!(s.bad_doing, vec!["grabando".to_string()]);
    }

    #[test]
    fn el_aviso_dice_lo_que_sabe() {
        assert_eq!(explain(&["".into(), "".into()]), "");
        assert_eq!(explain(&["grabando la pantalla".into(), "en «Discos»".into()]), "La primera vez estaba grabando la pantalla y la segunda, en «Discos».");
        assert_eq!(explain(&["".into(), "en «Discos»".into()]), "La última vez estaba en «Discos».");
        assert_eq!(doing_of(&open(1, 0, &[], &["grabando la pantalla"], "Pantallas")), "grabando la pantalla, en «Pantallas»");
    }

    #[test]
    fn solo_se_recuerdan_los_dos_ultimos() {
        let (i, s) = decide(&open(2000, 2, &["a", "b"], &["c"], ""), 1000, false);
        assert!(i.safe);
        assert_eq!(i.bad_closes, 3);
        assert_eq!(s.bad_doing, vec!["b".to_string(), "c".to_string()]);
    }
}
