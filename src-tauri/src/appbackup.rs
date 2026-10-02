//! Copia de seguridad cifrada de los datos de AdminOps (lo que viaja en el USB:
//! ajustes, contactos, clientes, routers, soluciones…) a otra unidad o a una
//! carpeta sincronizada con la nube, y su restauración. Además, la salud del
//! almacenamiento: espacio, copias recientes, clave de cifrado presente.
//!
//! El .zip va cifrado con AES-256 y una contraseña elegida por el técnico.

use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Carpetas que no se copian: caché del navegador interno (enorme y se regenera),
/// registros y restos de restauraciones anteriores.
const SKIP_DIRS: &[&str] = &["webview", "EBWebView", "logs", "antes-de-restaurar"];
const MARKER: &str = "adminops-copia.json";
const PREFS: &str = "preferencias.json";
const LAST: &str = "ultima-copia.txt";

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Archivos a copiar (ruta relativa con `/`, ruta real).
fn files(root: &Path, machines: bool, reports: bool) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                let top = dir == root;
                let skip = SKIP_DIRS.iter().any(|s| name.to_lowercase().starts_with(&s.to_lowercase()))
                    || (top && !machines && (name == "equipos" || name == "snapshots"))
                    || (top && !reports && name == "Informes");
                if !skip {
                    stack.push(p);
                }
            } else if ft.is_file() && !name.ends_with(".tmp") {
                if let Ok(rel) = p.strip_prefix(root) {
                    out.push((rel.to_string_lossy().replace('\\', "/"), p));
                }
            }
        }
    }
    out.sort();
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDone {
    path: String,
    files: usize,
    bytes: u64,
}

fn write_zip(root: &Path, out: &Path, password: &str, prefs: &serde_json::Value, machines: bool, reports: bool) -> Result<(usize, u64), String> {
    let file = std::fs::File::create(out).map_err(|e| format!("No se pudo crear la copia: {e}"))?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).with_aes_encryption(zip::AesMode::Aes256, password);
    let marker = serde_json::json!({ "app": "AdminOps", "version": env!("CARGO_PKG_VERSION"), "created": now(), "portable": crate::paths::is_portable() });
    zip.start_file(MARKER, opts).map_err(|e| e.to_string())?;
    zip.write_all(marker.to_string().as_bytes()).map_err(|e| e.to_string())?;
    zip.start_file(PREFS, opts).map_err(|e| e.to_string())?;
    zip.write_all(prefs.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let (mut n, mut bytes) = (0, 0);
    for (rel, p) in files(root, machines, reports) {
        // Un archivo en uso o sin permiso no debe estropear toda la copia.
        let Ok(data) = std::fs::read(&p) else { continue };
        zip.start_file(format!("datos/{rel}"), opts).map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
        n += 1;
        bytes += data.len() as u64;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok((n, bytes))
}

#[tauri::command(async)]
pub fn backup_app_data(app: tauri::AppHandle, password: String, prefs: serde_json::Value, machines: bool, reports: bool) -> Result<Option<BackupDone>, String> {
    use tauri_plugin_dialog::DialogExt;
    if password.chars().count() < 8 {
        return Err("La contraseña debe tener al menos 8 caracteres.".into());
    }
    let name = format!("AdminOps-copia-{}.zip", chrono::Local::now().format("%Y-%m-%d"));
    let Some(out) = app.dialog().file().set_file_name(&name).add_filter("Copia de AdminOps", &["zip"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let root = crate::paths::shared_data_dir(&app);
    // No guardar la copia dentro de lo que se copia.
    if out.starts_with(&root) {
        return Err("Guarda la copia fuera de la carpeta de datos de AdminOps (otra unidad, OneDrive…).".into());
    }
    let task = crate::task::Task::new(&app, "app-backup").named("Copia de seguridad");
    task.step("Cifrando y guardando los datos…");
    let (files, bytes) = write_zip(&root, &out, &password, &prefs, machines, reports)?;
    let _ = std::fs::write(root.join(LAST), now().to_string());
    log::info!("Copia de seguridad de AdminOps: {files} archivos");
    Ok(Some(BackupDone { path: out.display().to_string(), files, bytes }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    files: usize,
    created: u64,
    /// Preferencias de la interfaz de la copia (las aplica la interfaz).
    prefs: serde_json::Value,
}

/// Restaura en `root`. Lo que se sobrescribe se guarda antes en `antes-de-restaurar-<fecha>`.
fn restore_zip(root: &Path, zip_path: &Path, password: &str) -> Result<Restored, String> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(|e| format!("No se pudo abrir: {e}"))?).map_err(|_| "No es una copia de AdminOps válida.".to_string())?;
    let read_entry = |z: &mut zip::ZipArchive<std::fs::File>, name: &str| -> Result<Vec<u8>, String> {
        let i = z.index_for_name(name).ok_or("No es una copia de AdminOps.")?;
        let mut f = match z.by_index_decrypt(i, password.as_bytes()) {
            Ok(f) => f,
            Err(zip::result::ZipError::InvalidPassword) => return Err("Contraseña incorrecta.".into()),
            Err(e) => return Err(e.to_string()),
        };
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|_| "Contraseña incorrecta o copia dañada.".to_string())?;
        Ok(buf)
    };
    let marker: serde_json::Value = serde_json::from_slice(&read_entry(&mut z, MARKER)?).map_err(|_| "No es una copia de AdminOps.".to_string())?;
    if marker["app"] != "AdminOps" {
        return Err("No es una copia de AdminOps.".into());
    }
    let prefs = serde_json::from_slice(&read_entry(&mut z, PREFS).unwrap_or_default()).unwrap_or(serde_json::Value::Null);
    let safety = root.join(format!("antes-de-restaurar-{}", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    let mut n = 0;
    for i in 0..z.len() {
        let mut f = z.by_index_decrypt(i, password.as_bytes()).map_err(|e| e.to_string())?;
        // Solo rutas seguras dentro de «datos/».
        let Some(rel) = f.enclosed_name().and_then(|p| p.strip_prefix("datos").ok().map(Path::to_path_buf)) else { continue };
        if f.is_dir() || rel.as_os_str().is_empty() {
            continue;
        }
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|_| "La copia está dañada.".to_string())?;
        let target = root.join(&rel);
        if target.is_file() {
            let keep = safety.join(&rel);
            if let Some(d) = keep.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::copy(&target, keep);
        }
        if let Some(d) = target.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&target, data).map_err(|e| format!("No se pudo restaurar {}: {e}", rel.display()))?;
        n += 1;
    }
    Ok(Restored { files: n, created: marker["created"].as_u64().unwrap_or(0), prefs })
}

#[tauri::command(async)]
pub fn restore_app_data(app: tauri::AppHandle, password: String) -> Result<Option<Restored>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(file) = app.dialog().file().add_filter("Copia de AdminOps", &["zip"]).blocking_pick_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let r = restore_zip(&crate::paths::shared_data_dir(&app), &file, &password)?;
    log::info!("Datos restaurados desde una copia: {} archivos", r.files);
    Ok(Some(r))
}

// ---------- Copia automática ----------

const AUTO: &str = "copia-automatica.json";

/// Copia automática: a una carpeta (OneDrive, otro disco) cada N días, en el
/// equipo donde se configuró (en un pendrive, en otro PC esa carpeta sería la
/// de otra persona). La contraseña del .zip se guarda cifrada como las demás.
#[derive(Serialize, serde::Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
struct AutoStored {
    folder: String,
    password: String,
    every_days: u32,
    keep: u32,
    host: String,
    last_error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoBackupInfo {
    enabled: bool,
    folder: String,
    every_days: u32,
    keep: u32,
    /// Se configuró en este equipo (solo aquí se hace sola).
    here: bool,
    configured_on: String,
    last_backup: Option<u64>,
    next_due: Option<u64>,
    last_error: String,
    /// Carpeta de OneDrive de este usuario, si la hay (para proponerla).
    onedrive: Option<String>,
}

fn auto_load(root: &Path) -> AutoStored {
    crate::paths::read_json(&root.join(AUTO))
}

fn last_backup(root: &Path) -> Option<u64> {
    std::fs::read_to_string(root.join(LAST)).ok().and_then(|s| s.trim().parse().ok())
}

/// ¿Toca copia? (Nunca hecha, o hace `every` días o más.)
pub fn backup_due(last: Option<u64>, every_days: u32, at: u64) -> bool {
    every_days > 0 && last.is_none_or(|l| at.saturating_sub(l) >= u64::from(every_days) * 86_400)
}

/// Deja solo las `keep` copias automáticas más recientes de la carpeta.
fn prune(folder: &Path, keep: u32) {
    let mut copies: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("AdminOps-copia-") && n.ends_with(".zip")))
        .collect();
    copies.sort();
    let extra = copies.len().saturating_sub(keep.max(1) as usize);
    for p in copies.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
}

fn run_auto(app: &tauri::AppHandle) -> Result<BackupDone, String> {
    let root = crate::paths::shared_data_dir(app);
    let cfg = auto_load(&root);
    if cfg.folder.is_empty() {
        return Err("La copia automática no está configurada.".into());
    }
    let folder = PathBuf::from(&cfg.folder);
    if !folder.is_dir() {
        return Err("La carpeta de la copia no está disponible (¿OneDrive o el disco, desconectados?).".into());
    }
    let password = crate::secrets::open(&cfg.password)?;
    let out = folder.join(format!("AdminOps-copia-{}.zip", chrono::Local::now().format("%Y-%m-%d")));
    let (files, bytes) = write_zip(&root, &out, &password, &serde_json::json!({}), false, false)?;
    let _ = std::fs::write(root.join(LAST), now().to_string());
    prune(&folder, if cfg.keep == 0 { 5 } else { cfg.keep });
    log::info!("Copia automática de AdminOps: {files} archivos");
    Ok(BackupDone { path: out.display().to_string(), files, bytes })
}

#[tauri::command]
pub fn autobackup_info(app: tauri::AppHandle) -> AutoBackupInfo {
    let root = crate::paths::shared_data_dir(&app);
    let cfg = auto_load(&root);
    let last = last_backup(&root);
    let every = if cfg.every_days == 0 { 7 } else { cfg.every_days };
    AutoBackupInfo {
        enabled: !cfg.folder.is_empty(),
        here: cfg.host.is_empty() || cfg.host == crate::paths::host(),
        configured_on: cfg.host.clone(),
        next_due: (!cfg.folder.is_empty()).then(|| last.map_or(now(), |l| l + u64::from(every) * 86_400)),
        folder: cfg.folder,
        every_days: every,
        keep: if cfg.keep == 0 { 5 } else { cfg.keep },
        last_backup: last,
        last_error: cfg.last_error,
        onedrive: std::env::var("OneDrive").ok().filter(|p| Path::new(p).is_dir()).map(|p| Path::new(&p).join("AdminOps copias").display().to_string()),
    }
}

/// Configura la copia automática (carpeta vacía: la desactiva).
#[tauri::command]
pub fn autobackup_set(app: tauri::AppHandle, folder: String, password: String, every_days: u32, keep: u32) -> Result<(), String> {
    let root = crate::paths::shared_data_dir(&app);
    let path = root.join(AUTO);
    if folder.trim().is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let dir = PathBuf::from(folder.trim());
    if dir.starts_with(&root) {
        return Err("Elige una carpeta fuera de los datos de AdminOps (OneDrive, otro disco…).".into());
    }
    let old = auto_load(&root);
    // Sin contraseña nueva, se mantiene la que había.
    let sealed = if password.is_empty() {
        if old.password.is_empty() {
            return Err("Pon una contraseña para cifrar la copia (al menos 8 caracteres).".into());
        }
        old.password
    } else {
        if password.chars().count() < 8 {
            return Err("La contraseña debe tener al menos 8 caracteres.".into());
        }
        crate::secrets::seal(&password)?
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear la carpeta: {e}"))?;
    let cfg = AutoStored { folder: dir.display().to_string(), password: sealed, every_days: every_days.clamp(1, 60), keep: keep.clamp(1, 30), host: crate::paths::host(), last_error: String::new() };
    crate::paths::write_json(&path, &cfg)
}

#[tauri::command(async)]
pub fn autobackup_run_now(app: tauri::AppHandle) -> Result<BackupDone, String> {
    let r = run_auto(&app);
    let root = crate::paths::shared_data_dir(&app);
    let mut cfg = auto_load(&root);
    if !cfg.folder.is_empty() {
        cfg.last_error = r.as_ref().err().cloned().unwrap_or_default();
        let _ = crate::paths::write_json(&root.join(AUTO), &cfg);
    }
    r
}

/// Cada hora: la copia automática si toca; una vez al día, la salud de la
/// unidad de datos (el pendrive) con aviso si algo va mal.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3 * 60));
        let mut checked_day = 0u64;
        loop {
            let root = crate::paths::shared_data_dir(&app);
            let cfg = auto_load(&root);
            let every = if cfg.every_days == 0 { 7 } else { cfg.every_days };
            if !cfg.folder.is_empty() && cfg.host == crate::paths::host() && backup_due(last_backup(&root), every, now()) {
                let r = run_auto(&app);
                let mut cfg = cfg.clone();
                cfg.last_error = r.as_ref().err().cloned().unwrap_or_default();
                let _ = crate::paths::write_json(&root.join(AUTO), &cfg);
                if let Err(e) = r {
                    log::warn!("Copia automática: {e}");
                }
            }
            let day = now() / 86_400;
            if crate::paths::is_portable() && day != checked_day {
                checked_day = day;
                let h = storage_health(app.clone());
                let alerts: Vec<crate::winwatch::Alert> = h
                    .warnings
                    .iter()
                    .map(|w| crate::winwatch::Alert {
                        key: format!("usb:{}", w.chars().take(40).collect::<String>()),
                        level: if w.contains("dañado") || w.contains("problema") { "bad".into() } else { "warn".into() },
                        title: "Los datos de AdminOps en el pendrive".into(),
                        detail: w.clone(),
                        explanation: "El pendrive donde va AdminOps es lo más frágil de todo: si se estropea o se pierde, se van tus clientes, contactos y agenda.".into(),
                        advice: "Ajustes → Datos: copia automática a OneDrive u otro disco. Y en Discos, su salud.".into(),
                        page: Some("settings".into()),
                        count: 1,
                        time: now(),
                        ..Default::default()
                    })
                    .collect();
                if !alerts.is_empty() {
                    crate::winwatch::push_alerts(&app, alerts);
                }
            }
            std::thread::sleep(Duration::from_secs(3600));
        }
    });
}

// ---------- Salud del almacenamiento ----------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StorageHealth {
    pub portable: bool,
    pub drive: String,
    pub label: String,
    pub file_system: String,
    /// Healthy | Warning | Unhealthy (lo que informa Windows del volumen).
    pub health: String,
    pub removable: bool,
    pub total: u64,
    pub free: u64,
    pub data_bytes: u64,
    /// Portable: la clave de las contraseñas está en el USB.
    pub key_present: Option<bool>,
    /// Portable: esa clave va protegida con el PIN del bloqueo.
    pub key_protected: Option<bool>,
    pub last_backup: Option<u64>,
    pub last_contacts_backup: Option<u64>,
    pub warnings: Vec<String>,
}

fn dir_bytes(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                // El navegador interno son miles de archivos: recorrerlo en un
                // pendrive tarda segundos, y no son «tus datos».
                Ok(t) if t.is_dir() && SKIP_DIRS.iter().any(|s| e.file_name().to_string_lossy().eq_ignore_ascii_case(s)) => {}
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

#[tauri::command(async)]
pub fn storage_health(app: tauri::AppHandle) -> StorageHealth {
    let root = crate::paths::shared_data_dir(&app);
    let drive: String = root.to_string_lossy().chars().take(1).collect();
    let mut h = StorageHealth { portable: crate::paths::is_portable(), drive: format!("{drive}:"), ..Default::default() };
    if drive.chars().all(|c| c.is_ascii_alphabetic()) && !drive.is_empty() {
        let script = format!(
            "$v = Get-Volume -DriveLetter '{drive}' -ErrorAction SilentlyContinue\n[pscustomobject]@{{ label = \"$($v.FileSystemLabel)\"; fs = \"$($v.FileSystem)\"; health = \"$($v.HealthStatus)\"; type = \"$($v.DriveType)\"; size = [uint64]$v.Size; free = [uint64]$v.SizeRemaining }} | ConvertTo-Json -Compress"
        );
        if let Ok(out) = crate::pspool::query(&script, Some(Duration::from_secs(20)), "Salud del almacenamiento") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(out.trim()) {
                h.label = v["label"].as_str().unwrap_or("").into();
                h.file_system = v["fs"].as_str().unwrap_or("").into();
                h.health = v["health"].as_str().unwrap_or("").into();
                h.removable = v["type"].as_str().is_some_and(|t| t.eq_ignore_ascii_case("Removable"));
                h.total = v["size"].as_u64().unwrap_or(0);
                h.free = v["free"].as_u64().unwrap_or(0);
            }
        }
    }
    h.data_bytes = dir_bytes(&root);
    if h.portable {
        h.key_present = Some(root.join(".clave").is_file());
        h.key_protected = Some(crate::secrets::key_is_protected(&root));
    }
    h.last_backup = std::fs::read_to_string(root.join(LAST)).ok().and_then(|s| s.trim().parse().ok());
    h.last_contacts_backup = std::fs::read_dir(root.join("contacts-copias"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_prefix("contactos-")?.strip_suffix(".json")?.parse::<u64>().ok())
        .max();

    let days = |t: Option<u64>| t.map(|t| now().saturating_sub(t) / 86_400);
    if !h.health.is_empty() && !h.health.eq_ignore_ascii_case("Healthy") {
        h.warnings.push(format!("Windows informa de un problema en la unidad ({}). Copia los datos cuanto antes y cambia el USB.", h.health));
    }
    if h.file_system.eq_ignore_ascii_case("FAT32") {
        h.warnings.push("La unidad es FAT32: se corrompe con facilidad si se desconecta sin «Expulsar». Mejor exFAT o NTFS.".into());
    }
    if h.total > 0 && h.free < 500 * 1024 * 1024 {
        h.warnings.push("Queda muy poco espacio en la unidad.".into());
    }
    // En un pendrive, perderlo es perderlo todo: la copia se pide antes.
    let limit = if h.portable { 15 } else { 30 };
    match days(h.last_backup) {
        None => h.warnings.push("Nunca se ha hecho una copia de seguridad cifrada de estos datos.".into()),
        Some(d) if d > limit => h.warnings.push(format!("La última copia de seguridad es de hace {d} días.")),
        _ => {}
    }
    if h.portable && drive.chars().next().is_some_and(|c| crate::disks::is_dirty(&c.to_string()) == Some(true)) {
        h.warnings.push("Windows marcó el sistema de archivos de esta unidad como dañado: repáralo en Discos → Salud y reparación y haz una copia cuanto antes.".into());
    }
    if h.key_present == Some(false) && std::fs::read_to_string(root.join("routers.json")).is_ok_and(|s| s.contains("p1:")) {
        h.warnings.push("Falta la clave del USB: las contraseñas de routers guardadas no se pueden leer.".into());
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_a_backup_is_due() {
        let day = 86_400;
        assert!(backup_due(None, 7, 100 * day), "nunca hecha");
        assert!(!backup_due(Some(95 * day), 7, 100 * day));
        assert!(backup_due(Some(93 * day), 7, 100 * day));
        assert!(!backup_due(None, 0, 100 * day), "desactivada");
    }

    #[test]
    fn keeps_only_the_newest_copies() {
        let dir = std::env::temp_dir().join(format!("adminops-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for d in ["2026-09-01", "2026-09-08", "2026-09-15", "2026-09-22"] {
            std::fs::write(dir.join(format!("AdminOps-copia-{d}.zip")), b"x").unwrap();
        }
        std::fs::write(dir.join("otra-cosa.zip"), b"x").unwrap();
        prune(&dir, 2);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["AdminOps-copia-2026-09-15.zip", "AdminOps-copia-2026-09-22.zip", "otra-cosa.zip"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let base = std::env::temp_dir().join(format!("adminops-appbackup-{}", std::process::id()));
        let src = base.join("src");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(src.join("webview")).unwrap();
        std::fs::create_dir_all(src.join("equipos/PC1")).unwrap();
        std::fs::write(src.join("contacts.json"), "[1]").unwrap();
        std::fs::write(src.join(".clave"), [7u8; 32]).unwrap();
        std::fs::write(src.join("webview/cache"), "x").unwrap();
        std::fs::write(src.join("equipos/PC1/journal.json"), "[]").unwrap();
        let zip = base.join("copia.zip");
        let (n, _) = write_zip(&src, &zip, "clave segura", &serde_json::json!({ "zoom": 1.1 }), false, false).unwrap();
        // Ni la caché del navegador ni lo de cada equipo (no se pidió).
        assert_eq!(n, 2);

        let dst = base.join("dst");
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join("contacts.json"), "[0]").unwrap();
        assert!(restore_zip(&dst, &zip, "otra").is_err());
        let r = restore_zip(&dst, &zip, "clave segura").unwrap();
        assert_eq!(r.files, 2);
        assert_eq!(r.prefs["zoom"], 1.1);
        assert_eq!(std::fs::read_to_string(dst.join("contacts.json")).unwrap(), "[1]");
        // Lo sobrescrito se guardó antes.
        let safety = std::fs::read_dir(&dst).unwrap().flatten().find(|e| e.file_name().to_string_lossy().starts_with("antes-de-restaurar")).unwrap();
        assert_eq!(std::fs::read_to_string(safety.path().join("contacts.json")).unwrap(), "[0]");
        let _ = std::fs::remove_dir_all(&base);
    }
}
