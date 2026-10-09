//! Copias de seguridad: los comandos de la interfaz y la comprobación diaria. Qué se
//! comprueba y cómo está en `backupcheck.rs`.

use crate::backupcheck::{self, SetCfg, Status};
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use std::path::Path;
use std::time::Duration;

fn cfg_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("copias-config.json")
}

fn status_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("copias-estado.json")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Las últimas comprobaciones (para el informe).
pub fn last_statuses(app: &tauri::AppHandle) -> Vec<Status> {
    crate::paths::read_json(&status_path(app))
}

fn load(app: &tauri::AppHandle) -> Vec<SetCfg> {
    crate::paths::read_json(&cfg_path(app))
}

#[tauri::command]
pub fn backups_get(app: tauri::AppHandle) -> Vec<SetCfg> {
    load(&app)
}

#[tauri::command]
pub fn backups_status(app: tauri::AppHandle) -> Vec<Status> {
    last_statuses(&app)
}

/// Las carpetas que casi siempre merece la pena proteger, si existen en este equipo.
#[tauri::command]
pub fn backups_defaults() -> Vec<String> {
    let home = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default();
    let local = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    backupcheck::default_sources(&home, local.as_deref())
}

fn clean(s: &str, n: usize) -> String {
    s.trim().chars().filter(|c| !c.is_control()).take(n).collect()
}

#[tauri::command]
pub fn backups_save(app: tauri::AppHandle, sets: Vec<SetCfg>) -> Result<Vec<SetCfg>, String> {
    let out: Vec<SetCfg> = sets
        .into_iter()
        .take(20)
        .enumerate()
        .map(|(i, s)| SetCfg {
            id: if s.id.is_empty() { format!("c{:x}{i}", now()) } else { clean(&s.id, 32) },
            name: clean(&s.name, 60),
            dest: clean(&s.dest, 260),
            sources: s.sources.iter().map(|p| clean(p, 260)).filter(|p| !p.is_empty()).take(12).collect(),
        })
        .filter(|s| !s.dest.is_empty())
        .collect();
    crate::paths::write_json(&cfg_path(&app), &out)?;
    // Lo que ya no existe deja de aparecer en el estado.
    let keep: Vec<Status> = last_statuses(&app).into_iter().filter(|s| out.iter().any(|c| c.id == s.id)).collect();
    let _ = crate::paths::write_json(&status_path(&app), &keep);
    Ok(out)
}

/// Número del disco físico en el que está una unidad (None: red u otra cosa).
fn disk_of(path: &str) -> Option<u32> {
    let l = path.trim().chars().next().filter(|c| c.is_ascii_alphabetic())?;
    if path.trim().chars().nth(1) != Some(':') {
        return None;
    }
    crate::ps::powershell(&format!("(Get-Partition -DriveLetter {l} -ErrorAction Stop).DiskNumber")).ok()?.trim().parse().ok()
}

fn same_disk(cfg: &SetCfg) -> Option<bool> {
    // Una carpeta de red siempre es «otro disco».
    if cfg.dest.trim_start().starts_with("\\\\") {
        return Some(false);
    }
    let dest = disk_of(&cfg.dest)?;
    let src = cfg.sources.iter().find_map(|s| disk_of(s))?;
    Some(dest == src)
}

fn run_checks(app: &tauri::AppHandle, only: Option<&str>) -> Vec<Status> {
    let sets = load(app);
    let mut all = last_statuses(app);
    for cfg in sets.iter().filter(|c| only.is_none_or(|id| id == c.id)) {
        let st = backupcheck::check_set(cfg, now(), same_disk(cfg));
        all.retain(|s| s.id != cfg.id);
        all.push(st);
    }
    all.retain(|s| sets.iter().any(|c| c.id == s.id));
    all.sort_by(|a, b| a.name.cmp(&b.name));
    let _ = crate::paths::write_json(&status_path(app), &all);
    all
}

/// Comprueba una copia (o todas) y devuelve el estado de todas.
#[tauri::command(async)]
pub fn backups_check(app: tauri::AppHandle, id: Option<String>) -> Vec<Status> {
    let task = crate::task::Task::new(&app, "backup-check").named("Comprobar las copias de seguridad");
    task.step("Mirando las copias…");
    run_checks(&app, id.as_deref())
}

/// Copia ahora lo que falte: archivos nuevos o cambiados, sin borrar nada del destino.
#[tauri::command(async)]
pub fn backups_run(app: tauri::AppHandle, id: String, tweaks: tauri::State<'_, TweakState>) -> Result<String, String> {
    let cfg = load(&app).into_iter().find(|c| c.id == id).ok_or("No se encuentra esa copia.")?;
    if cfg.sources.is_empty() {
        return Err("Esa copia no tiene carpetas de origen.".into());
    }
    let task = crate::task::Task::new(&app, "backup-run").named("Copiar ahora");
    let mut copied = 0;
    let mut failed: Vec<String> = Vec::new();
    for src in &cfg.sources {
        let p = Path::new(src);
        if !p.is_dir() {
            continue;
        }
        if Path::new(&cfg.dest).starts_with(p) {
            return Err("El destino está dentro de una carpeta de origen: elige otro sitio para la copia.".into());
        }
        let name = p.file_name().map_or("Copia".into(), |n| n.to_string_lossy().into_owned());
        let target = Path::new(&cfg.dest).join(&name);
        task.step(format!("Copiando {name}…"));
        // /XO: no pisa lo más nuevo del destino. Sin /MIR: nunca borra nada.
        let mut child = crate::ps::hidden("robocopy.exe")
            .args([src.as_str(), &target.display().to_string(), "/E", "/XO", "/R:1", "/W:1", "/NP", "/NFL", "/NDL", "/NJH", "/NJS", "/XJ"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("No se pudo iniciar la copia: {e}"))?;
        let code = loop {
            if let Ok(Some(status)) = child.try_wait() {
                break status.code();
            }
            if task.cancelled() {
                let _ = child.kill();
                return Err("Cancelado.".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        };
        // robocopy: 0 a 7 es que fue bien (con o sin archivos copiados); 8 o más, que algo falló.
        match code {
            Some(c) if c < 8 => copied += 1,
            Some(c) => failed.push(format!("{name}: robocopy terminó con el código {c}")),
            None => failed.push(format!("{name}: la copia se detuvo")),
        }
    }
    let result: Result<(), String> = if failed.is_empty() { Ok(()) } else { Err(failed.join(" · ")) };
    tweaks.record(Op::Run, &format!("Copia de seguridad «{}»: {copied} carpetas copiadas", cfg.name), &result);
    result.map(|_| format!("{copied} carpetas copiadas a {}.", cfg.dest))
}

/// Una vez al día, con AdminOps abierto: comprueba las copias y avisa de las que fallan.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(6 * 60));
        loop {
            if !load(&app).is_empty() {
                let day = now() / 86_400;
                let alerts: Vec<crate::winwatch::Alert> = run_checks(&app, None)
                    .into_iter()
                    .filter(|s| s.level != "ok")
                    .map(|s| {
                        let worst = s.checks.iter().find(|c| c.level == s.level).map(|c| c.text.clone()).unwrap_or_default();
                        crate::winwatch::Alert {
                            key: format!("backup:{}:{day}", s.id),
                            level: s.level.clone(),
                            title: format!("La copia «{}» necesita atención", s.name),
                            detail: worst,
                            explanation: "Una copia que no está al día, que está en el mismo disco o que no cubre lo importante no protege de verdad.".into(),
                            advice: "Mira Datos del equipo → Copias de seguridad: dice qué falla y deja copiar ahora.".into(),
                            page: Some("data".into()),
                            count: 1,
                            time: now(),
                            ..Default::default()
                        }
                    })
                    .collect();
                if !alerts.is_empty() {
                    crate::winwatch::push_alerts(&app, alerts);
                }
            }
            std::thread::sleep(Duration::from_secs(24 * 3600));
        }
    });
}
