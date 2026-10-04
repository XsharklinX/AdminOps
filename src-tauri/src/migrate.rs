//! Copia de datos del usuario a un USB o disco, y restauración en el equipo nuevo.
//!
//! Copia las carpetas personales (respetando las redirigidas, p. ej. el
//! Escritorio dentro de OneDrive), los marcadores de los navegadores y las
//! redes Wi-Fi. Deja un `adminops-backup.json` que describe la copia.
//!
//! Nunca sobrescribe al restaurar: si ya existe un archivo distinto con el
//! mismo nombre, guarda el copiado como "nombre (AdminOps).ext".

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{Emitter, State};

const MANIFEST: &str = "adminops-backup.json";
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x40000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x400000;

/// (id, nombre visible, valor en "User Shell Folders", carpeta por defecto)
const FOLDERS: &[(&str, &str, &str, &str)] = &[
    ("desktop", "Escritorio", "Desktop", "Desktop"),
    ("documents", "Documentos", "Personal", "Documents"),
    ("pictures", "Imágenes", "My Pictures", "Pictures"),
    ("music", "Música", "My Music", "Music"),
    ("videos", "Vídeos", "My Video", "Videos"),
    ("downloads", "Descargas", "{374DE290-123F-4565-9164-39C4925E467B}", "Downloads"),
];

/// Navegadores basados en Chromium: (id, nombre, carpeta dentro de LocalAppData, proceso).
const CHROMIUM: &[(&str, &str, &str, &str)] = &[
    ("chrome", "Google Chrome", r"Google\Chrome\User Data", "chrome.exe"),
    ("edge", "Microsoft Edge", r"Microsoft\Edge\User Data", "msedge.exe"),
    ("brave", "Brave", r"BraveSoftware\Brave-Browser\User Data", "brave.exe"),
];

// ---------- Perfiles y carpetas ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRef {
    sid: String,
    name: String,
    /// Es el usuario con la sesión abierta (el único al que se puede restaurar).
    is_target: bool,
    #[serde(skip)]
    path: PathBuf,
}

fn expand_system_drive(p: &str) -> String {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    p.replace("%SystemDrive%", &drive)
}

/// Perfiles de usuario del equipo (cuentas locales y de Microsoft).
fn profiles() -> Vec<ProfileRef> {
    use winreg::enums::*;
    let target = crate::target_user::get().map(|t| t.sid.clone());
    let Ok(list) = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList", KEY_READ | KEY_WOW64_64KEY)
    else {
        return vec![];
    };
    let mut out: Vec<ProfileRef> = list
        .enum_keys()
        .filter_map(Result::ok)
        .filter(|sid| sid.starts_with("S-1-5-21-") && !sid.ends_with(".bak"))
        .filter_map(|sid| {
            let raw: String = list.open_subkey(&sid).ok()?.get_value("ProfileImagePath").ok()?;
            let path = PathBuf::from(expand_system_drive(&raw));
            let name = path.file_name()?.to_string_lossy().into_owned();
            path.is_dir().then(|| ProfileRef { name, is_target: target.as_deref() == Some(sid.as_str()), sid, path })
        })
        .collect();
    out.sort_by_key(|p| (!p.is_target, p.name.to_lowercase()));
    out
}

fn find_profile(sid: &str) -> Result<ProfileRef, String> {
    profiles().into_iter().find(|p| p.sid == sid).ok_or_else(|| "Ese perfil ya no existe.".into())
}

/// Carpeta personal real (puede estar redirigida a OneDrive u otra unidad).
fn known_folder(p: &ProfileRef, reg_name: &str, default: &str) -> PathBuf {
    let key = format!(r"HKU\{}\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders", p.sid);
    let fallback = p.path.join(default);
    let Some(raw) = crate::tweaks::registry::read_raw(&key, reg_name).and_then(|r| crate::tweaks::registry::raw_to_string(&r)) else {
        return fallback; // colmena del usuario no cargada: carpeta estándar
    };
    let profile = p.path.display().to_string();
    let lower = raw.to_ascii_lowercase();
    let expanded = match lower.find("%userprofile%") {
        Some(i) => format!("{}{}{}", &raw[..i], profile, &raw[i + "%userprofile%".len()..]),
        None => raw,
    };
    let expanded = expand_system_drive(&expanded);
    if expanded.contains('%') { fallback } else { PathBuf::from(expanded) }
}

fn local_app_data(p: &ProfileRef) -> PathBuf {
    known_folder(p, "Local AppData", r"AppData\Local")
}

fn roaming_app_data(p: &ProfileRef) -> PathBuf {
    known_folder(p, "AppData", r"AppData\Roaming")
}

/// Marcadores de los navegadores: (id del navegador, carpeta de perfil, archivo).
fn bookmark_files(p: &ProfileRef) -> Vec<(String, String, PathBuf)> {
    let mut out = Vec::new();
    let local = local_app_data(p);
    for (id, _, dir, _) in CHROMIUM {
        let root = local.join(dir);
        for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "Default" || name.starts_with("Profile ") {
                let file = entry.path().join("Bookmarks");
                if file.is_file() {
                    out.push((id.to_string(), name, file));
                }
            }
        }
    }
    // Firefox: la copia de seguridad de marcadores más reciente de cada perfil.
    let ff = roaming_app_data(p).join(r"Mozilla\Firefox\Profiles");
    for entry in std::fs::read_dir(&ff).into_iter().flatten().flatten() {
        let backups = entry.path().join("bookmarkbackups");
        let latest = std::fs::read_dir(&backups)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .max_by_key(|(t, _)| *t);
        if let Some((_, file)) = latest {
            out.push(("firefox".into(), entry.file_name().to_string_lossy().into_owned(), file));
        }
    }
    out
}

// ---------- Copia de archivos ----------

/// Prefijo `\\?\` para rutas de más de 260 caracteres.
fn long(p: &Path) -> PathBuf {
    let s = p.display().to_string();
    if s.starts_with(r"\\") { p.to_path_buf() } else { PathBuf::from(format!(r"\\?\{s}")) }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    item: String,
    done_bytes: u64,
    total_bytes: u64,
    files: u64,
}

struct Ctx<'a> {
    app: Option<&'a tauri::AppHandle>,
    task: &'a Task,
    restore: bool,
    item: String,
    done_bytes: u64,
    total_bytes: u64,
    files: u64,
    item_files: u64,
    item_bytes: u64,
    cloud_only: u64,
    unchanged: u64,
    renamed: u64,
    errors: Vec<String>,
    last_emit: Instant,
}

impl Ctx<'_> {
    fn emit(&mut self, force: bool) {
        if !force && self.last_emit.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last_emit = Instant::now();
        let Some(app) = self.app else { return };
        let _ = app.emit(
            "migrate-progress",
            Progress { item: self.item.clone(), done_bytes: self.done_bytes, total_bytes: self.total_bytes, files: self.files },
        );
    }

    fn error(&mut self, path: &Path, e: impl std::fmt::Display) {
        if self.errors.len() < 200 {
            self.errors.push(format!("{}: {e}", path.display()));
        }
    }
}

/// "foto.jpg" → "foto (AdminOps).jpg" (o " 2", " 3"… si también existe).
fn free_name(dst: &Path) -> PathBuf {
    let stem = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = dst.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (1..)
        .map(|n| {
            let tag = if n == 1 { " (AdminOps)".to_string() } else { format!(" (AdminOps {n})") };
            dst.with_file_name(format!("{stem}{tag}{ext}"))
        })
        .find(|p| !p.exists())
        .unwrap()
}

fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.len() == b.len() && a.modified().ok() == b.modified().ok()
}

fn copy_tree(src: &Path, dst: &Path, ctx: &mut Ctx) {
    use std::os::windows::fs::MetadataExt;
    if ctx.task.cancelled() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(long(dst)) {
        ctx.error(dst, e);
        return;
    }
    let entries = match std::fs::read_dir(long(src)) {
        Ok(e) => e,
        Err(e) => {
            ctx.error(src, e);
            return;
        }
    };
    for entry in entries.flatten() {
        if ctx.task.cancelled() {
            return;
        }
        let name = entry.file_name();
        let (from, to) = (src.join(&name), dst.join(&name));
        let Ok(meta) = entry.metadata() else {
            ctx.error(&from, "no se pudo leer");
            continue;
        };
        let attrs = meta.file_attributes();
        if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 && meta.is_dir() {
            continue; // uniones del sistema ("Mis imágenes" dentro de Documentos…)
        }
        if meta.is_dir() {
            copy_tree(&from, &to, ctx);
            continue;
        }
        if attrs & (FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0 {
            ctx.cloud_only += 1; // solo en OneDrive: copiarlo lo descargaría
            ctx.done_bytes += meta.len();
            continue;
        }
        let mut target = to;
        if ctx.restore {
            if let Ok(existing) = std::fs::metadata(long(&target)) {
                if same_file(&existing, &meta) {
                    ctx.unchanged += 1;
                    ctx.done_bytes += meta.len();
                    continue;
                }
                target = free_name(&target);
                ctx.renamed += 1;
            }
        }
        match std::fs::copy(long(&from), long(&target)) {
            Ok(n) => {
                ctx.files += 1;
                ctx.item_files += 1;
                ctx.item_bytes += n;
            }
            Err(e) => ctx.error(&from, e),
        }
        ctx.done_bytes += meta.len();
        ctx.emit(false);
    }
}

fn free_space(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    let w: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0u64;
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(w.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut())
    };
    (ok != 0).then_some(free)
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect()
}

// ---------- Manifiesto ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestItem {
    id: String,
    label: String,
    files: u64,
    bytes: u64,
    #[serde(default)]
    cloud_only: u64,
    #[serde(default)]
    errors: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    app: String,
    version: String,
    /// ISO 8601
    created: String,
    computer: String,
    user: String,
    items: Vec<ManifestItem>,
}

// ---------- Comandos ----------

#[tauri::command(async)]
pub fn migrate_profiles() -> Vec<ProfileRef> {
    profiles()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    id: String,
    label: String,
    bytes: u64,
    files: u64,
    /// Detalle (navegadores encontrados, "requiere administrador"…).
    note: Option<String>,
    available: bool,
}

#[tauri::command(async)]
pub fn migrate_estimate(sid: String) -> Result<Vec<Estimate>, String> {
    let p = find_profile(&sid)?;
    let mut out: Vec<Estimate> = FOLDERS
        .iter()
        .map(|(id, label, reg, def)| {
            let dir = known_folder(&p, reg, def);
            let (bytes, files) = if dir.is_dir() { crate::space::folder_size(&dir) } else { (0, 0) };
            // Redirigida: no es la carpeta estándar (p. ej. C:\Users\x\OneDrive\Escritorio).
            let redirected = !dir.as_os_str().eq_ignore_ascii_case(p.path.join(def).as_os_str());
            Estimate {
                id: id.to_string(),
                label: label.to_string(),
                bytes,
                files,
                note: redirected.then(|| "Redirigida (OneDrive u otra unidad): los archivos solo en la nube no se copian".into()),
                available: dir.is_dir(),
            }
        })
        .collect();
    let marks = bookmark_files(&p);
    let mut browsers: Vec<&str> = marks
        .iter()
        .map(|(id, _, _)| CHROMIUM.iter().find(|c| c.0 == id).map_or("Firefox", |c| c.1))
        .collect();
    browsers.dedup();
    out.push(Estimate {
        id: "browsers".into(),
        label: "Marcadores de navegadores".into(),
        bytes: marks.iter().filter_map(|(_, _, f)| f.metadata().ok()).map(|m| m.len()).sum(),
        files: marks.len() as u64,
        note: Some(if browsers.is_empty() { "No se encontraron navegadores con marcadores".into() } else { browsers.join(", ") }),
        available: !marks.is_empty(),
    });
    let wifi = crate::network::wifi::export_all();
    let elevated = crate::elevation::is_elevated();
    out.push(Estimate {
        id: "wifi".into(),
        label: "Redes Wi-Fi".into(),
        bytes: 0,
        files: wifi.as_ref().map_or(0, |v| v.len() as u64),
        note: Some(match &wifi {
            Err(e) => e.clone(),
            Ok(_) if !elevated => "Requiere administrador para copiar las contraseñas".into(),
            Ok(v) => format!("{} redes con su contraseña (la copia las guarda en texto: protege el USB)", v.len()),
        }),
        available: wifi.is_ok_and(|v| !v.is_empty()) && elevated,
    });
    Ok(out)
}

#[tauri::command(async)]
pub fn migrate_pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateSummary {
    /// Carpeta de la copia (en la restauración, la de origen).
    folder: String,
    files: u64,
    bytes: u64,
    cloud_only: u64,
    unchanged: u64,
    renamed: u64,
    errors: Vec<String>,
    notes: Vec<String>,
    cancelled: bool,
}

#[tauri::command(async)]
pub fn migrate_backup(
    app: tauri::AppHandle,
    sid: String,
    items: Vec<String>,
    dest: String,
    tweaks: State<'_, TweakState>,
) -> Result<MigrateSummary, String> {
    let p = find_profile(&sid)?;
    let dest = PathBuf::from(dest.trim());
    if !dest.is_dir() {
        return Err("La carpeta de destino no existe.".into());
    }
    if dest.starts_with(&p.path) {
        return Err("Elige un destino fuera del perfil que vas a copiar (un USB u otro disco).".into());
    }
    let task = Task::new(&app, "migrate");
    task.step("Calculando tamaño…");
    let folders: Vec<(&str, &str, PathBuf)> = FOLDERS
        .iter()
        .filter(|f| items.iter().any(|i| i == f.0))
        .map(|(id, label, reg, def)| (*id, *label, known_folder(&p, reg, def)))
        .filter(|(_, _, d)| d.is_dir())
        .collect();
    let total: u64 = folders.iter().map(|(_, _, d)| crate::space::folder_size(d).0).sum();
    if let Some(free) = free_space(&dest) {
        if total > free {
            return Err(format!(
                "No cabe: la copia ocupa {:.1} GB y en el destino quedan {:.1} GB libres.",
                total as f64 / 1e9,
                free as f64 / 1e9
            ));
        }
    }
    let stamp = chrono::Local::now();
    let root = dest.join(safe_name(&format!("AdminOps - Copia de {} - {}", p.name, stamp.format("%Y-%m-%d %H%M"))));
    std::fs::create_dir_all(&root).map_err(|e| format!("No se pudo crear la carpeta de la copia: {e}"))?;

    let mut ctx = Ctx {
        app: Some(&app),
        task: &task,
        restore: false,
        item: String::new(),
        done_bytes: 0,
        total_bytes: total,
        files: 0,
        item_files: 0,
        item_bytes: 0,
        cloud_only: 0,
        unchanged: 0,
        renamed: 0,
        errors: vec![],
        last_emit: Instant::now(),
    };
    let mut manifest_items = Vec::new();
    let mut notes = Vec::new();
    for (id, label, dir) in &folders {
        task.step(format!("Copiando {label}…"));
        ctx.item = label.to_string();
        let (errors_before, cloud_before) = (ctx.errors.len(), ctx.cloud_only);
        ctx.item_files = 0;
        ctx.item_bytes = 0;
        copy_tree(dir, &root.join(label), &mut ctx);
        manifest_items.push(ManifestItem {
            id: id.to_string(),
            label: label.to_string(),
            files: ctx.item_files,
            bytes: ctx.item_bytes,
            cloud_only: ctx.cloud_only - cloud_before,
            errors: (ctx.errors.len() - errors_before) as u64,
        });
    }
    if items.iter().any(|i| i == "browsers") && !task.cancelled() {
        task.step("Copiando marcadores…");
        let marks = bookmark_files(&p);
        for (browser, profile, file) in &marks {
            let to = root.join("Navegadores").join(browser).join(safe_name(profile));
            let r = std::fs::create_dir_all(&to).and_then(|_| std::fs::copy(file, to.join(file.file_name().unwrap_or_default())));
            if let Err(e) = r {
                ctx.error(file, e);
            }
        }
        manifest_items.push(ManifestItem { id: "browsers".into(), label: "Marcadores".into(), files: marks.len() as u64, bytes: 0, cloud_only: 0, errors: 0 });
    }
    if items.iter().any(|i| i == "wifi") && !task.cancelled() {
        task.step("Copiando redes Wi-Fi…");
        match crate::network::wifi::export_all() {
            Ok(list) => {
                let dir = root.join("Wi-Fi");
                let _ = std::fs::create_dir_all(&dir);
                let saved = list.iter().filter(|(name, xml)| std::fs::write(dir.join(format!("{}.xml", safe_name(name))), xml).is_ok()).count();
                manifest_items.push(ManifestItem { id: "wifi".into(), label: "Redes Wi-Fi".into(), files: saved as u64, bytes: 0, cloud_only: 0, errors: 0 });
                notes.push("Las contraseñas Wi-Fi quedan en texto dentro de la copia: guarda el USB en un lugar seguro.".into());
            }
            Err(e) => notes.push(format!("Wi-Fi: {e}")),
        }
    }
    ctx.emit(true);
    let cancelled = task.cancelled();
    let manifest = Manifest {
        app: "AdminOps".into(),
        version: app.package_info().version.to_string(),
        created: stamp.to_rfc3339(),
        computer: sysinfo::System::host_name().unwrap_or_default(),
        user: p.name.clone(),
        items: manifest_items,
    };
    crate::paths::write_json(&root.join(MANIFEST), &manifest)?;
    if ctx.cloud_only > 0 {
        notes.push(format!("{} archivos están solo en OneDrive y no se copiaron (seguirán en la nube).", ctx.cloud_only));
    }
    let summary = MigrateSummary {
        folder: root.display().to_string(),
        files: ctx.files,
        bytes: ctx.done_bytes,
        cloud_only: ctx.cloud_only,
        unchanged: 0,
        renamed: 0,
        errors: ctx.errors,
        notes,
        cancelled,
    };
    let result: Result<(), String> = if cancelled { Err(crate::ps::CANCELLED_MSG.into()) } else { Ok(()) };
    tweaks.record(Op::Run, &format!("Copia de datos de «{}»: {} archivos", p.name, summary.files), &result);
    Ok(summary)
}

#[tauri::command(async)]
pub fn migrate_read_backup(folder: String) -> Result<Manifest, String> {
    let path = PathBuf::from(folder.trim()).join(MANIFEST);
    let text = std::fs::read_to_string(&path).map_err(|_| "Esa carpeta no es una copia de AdminOps (falta adminops-backup.json).".to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("La descripción de la copia está dañada: {e}"))
}

fn process_running(exe: &str) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| p.name().eq_ignore_ascii_case(exe))
}

/// Restaura en el usuario con la sesión abierta.
#[tauri::command(async)]
pub fn migrate_restore(app: tauri::AppHandle, folder: String, items: Vec<String>, tweaks: State<'_, TweakState>) -> Result<MigrateSummary, String> {
    let root = PathBuf::from(folder.trim());
    let manifest = migrate_read_backup(folder.clone())?;
    let target = crate::target_user::get().ok_or("No se pudo identificar al usuario con la sesión abierta.")?;
    let p = find_profile(&target.sid)?;
    let task = Task::new(&app, "migrate");
    let total: u64 = manifest.items.iter().filter(|i| items.contains(&i.id)).map(|i| i.bytes).sum();
    let mut ctx = Ctx {
        app: Some(&app),
        task: &task,
        restore: true,
        item: String::new(),
        done_bytes: 0,
        total_bytes: total,
        files: 0,
        item_files: 0,
        item_bytes: 0,
        cloud_only: 0,
        unchanged: 0,
        renamed: 0,
        errors: vec![],
        last_emit: Instant::now(),
    };
    let mut notes = Vec::new();
    for (_, label, reg, def) in FOLDERS.iter().filter(|f| items.iter().any(|i| i == f.0)) {
        let src = root.join(label);
        if !src.is_dir() {
            continue;
        }
        task.step(format!("Restaurando {label}…"));
        ctx.item = label.to_string();
        copy_tree(&src, &known_folder(&p, reg, def), &mut ctx);
    }
    if items.iter().any(|i| i == "browsers") && !task.cancelled() {
        task.step("Restaurando marcadores…");
        let base = root.join("Navegadores");
        let local = local_app_data(&p);
        for (id, name, dir, exe) in CHROMIUM {
            let saved = base.join(id);
            if !saved.is_dir() {
                continue;
            }
            if process_running(exe) {
                notes.push(format!("{name} está abierto: ciérralo y vuelve a restaurar los marcadores."));
                continue;
            }
            for prof in std::fs::read_dir(&saved).into_iter().flatten().flatten() {
                let file = prof.path().join("Bookmarks");
                let dest_dir = local.join(dir).join(prof.file_name());
                if !file.is_file() {
                    continue;
                }
                if !dest_dir.is_dir() {
                    notes.push(format!("{name}: abre el navegador una vez y vuelve a restaurar los marcadores."));
                    continue;
                }
                let dest = dest_dir.join("Bookmarks");
                if dest.is_file() {
                    let _ = std::fs::copy(&dest, dest_dir.join("Bookmarks.adminops.bak"));
                }
                match std::fs::copy(&file, &dest) {
                    Ok(_) => notes.push(format!("{name}: marcadores restaurados.")),
                    Err(e) => ctx.error(&dest, e),
                }
            }
        }
        let ff = base.join("firefox");
        if ff.is_dir() {
            let desk = known_folder(&p, "Desktop", "Desktop").join("Marcadores de Firefox");
            ctx.item = "Marcadores".into();
            copy_tree(&ff, &desk, &mut ctx);
            notes.push("Firefox: la copia quedó en el Escritorio, carpeta «Marcadores de Firefox». En Firefox: Marcadores → Administrar marcadores → Importar y respaldar → Restaurar → Elegir archivo.".into());
        }
    }
    if items.iter().any(|i| i == "wifi") && !task.cancelled() {
        task.step("Restaurando redes Wi-Fi…");
        let xmls: Vec<String> = std::fs::read_dir(root.join("Wi-Fi"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .collect();
        match crate::network::wifi::import_all(&xmls) {
            Ok(n) => notes.push(format!("{n} de {} redes Wi-Fi importadas.", xmls.len())),
            Err(e) => notes.push(format!("Wi-Fi: {e}")),
        }
    }
    ctx.emit(true);
    let cancelled = task.cancelled();
    let summary = MigrateSummary {
        folder: root.display().to_string(),
        files: ctx.files,
        bytes: ctx.done_bytes,
        cloud_only: 0,
        unchanged: ctx.unchanged,
        renamed: ctx.renamed,
        errors: ctx.errors,
        notes,
        cancelled,
    };
    let result: Result<(), String> = if cancelled { Err(crate::ps::CANCELLED_MSG.into()) } else { Ok(()) };
    tweaks.record(Op::Run, &format!("Restaurar copia de «{}» en «{}»: {} archivos", manifest.user, p.name, summary.files), &result);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(free_name(Path::new(r"C:\no\existe\foto.jpg")), PathBuf::from(r"C:\no\existe\foto (AdminOps).jpg"));
        assert_eq!(free_name(Path::new(r"C:\no\existe\LEEME")), PathBuf::from(r"C:\no\existe\LEEME (AdminOps)"));
        assert_eq!(safe_name("Copia: a/b"), "Copia_ a_b");
        assert_eq!(long(Path::new(r"C:\a")), PathBuf::from(r"\\?\C:\a"));
        assert_eq!(long(Path::new(r"\\server\share")), PathBuf::from(r"\\server\share"));
    }

    /// Equipo real (depende de su estado): `cargo test real_profiles_and_folders -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_profiles_and_folders() {
        let list = profiles();
        assert!(!list.is_empty());
        let p = list.iter().find(|p| p.is_target).unwrap_or(&list[0]);
        for (_, label, reg, def) in FOLDERS {
            println!("{label}: {}", known_folder(p, reg, def).display());
        }
        println!("marcadores: {}", bookmark_files(p).len());
    }

    /// Copia real en una carpeta temporal: sin sobrescribir al restaurar.
    #[test]
    fn copy_and_restore_without_overwriting() {
        let base = std::env::temp_dir().join(format!("adminops-migrate-{}", std::process::id()));
        let (src, dst) = (base.join("src"), base.join("dst"));
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), "uno").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), "dos").unwrap();
        let task = Task::detached("t-migrate");
        let mk = |restore| Ctx {
            app: None,
            task: &task,
            restore,
            item: String::new(),
            done_bytes: 0,
            total_bytes: 0,
            files: 0,
            item_files: 0,
            item_bytes: 0,
            cloud_only: 0,
            unchanged: 0,
            renamed: 0,
            errors: vec![],
            last_emit: Instant::now(),
        };
        let mut c = mk(false);
        copy_tree(&src, &dst, &mut c);
        assert_eq!(c.files, 2);
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        assert_eq!(std::fs::read_to_string(dst.join("sub").join("b.txt")).unwrap(), "dos");

        // Restaurar sobre el destino: lo idéntico se salta, lo distinto se guarda aparte.
        std::fs::write(dst.join("a.txt"), "cambiado").unwrap();
        let mut r = mk(true);
        copy_tree(&src, &dst, &mut r);
        assert_eq!(r.unchanged, 1);
        assert_eq!(r.renamed, 1);
        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "cambiado");
        assert_eq!(std::fs::read_to_string(dst.join("a (AdminOps).txt")).unwrap(), "uno");
        std::fs::remove_dir_all(&base).unwrap();
    }
}
