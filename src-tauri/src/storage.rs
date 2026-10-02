//! Llevar AdminOps en un pendrive con todo dentro.
//!
//! Instalar AdminOps en el pendrive (en vez de usar el portable) facilita
//! actualizar: se vuelve a pasar el instalador y listo, y los datos siguen ahí,
//! porque el desinstalador solo borra los archivos que instaló. Para que los
//! datos viajen con el pendrive, AdminOps se pone en modo portable:
//!
//! - **Solo**, si el programa está en una unidad extraíble (un pendrive).
//! - **A petición** (Ajustes → Datos), en cualquier otra carpeta: deja el
//!   archivo `AdminOps.portable` junto al programa.
//!
//! La primera vez, lo que AdminOps tenía guardado en este equipo se trae al
//! pendrive (ajustes, clientes, contactos, agenda, portales…), las contraseñas
//! guardadas se vuelven a cifrar con la clave del pendrive (las de Windows solo
//! se pueden leer en este equipo) y la sesión de los portales de este equipo se
//! conserva. Se hace al arrancar, antes de que se abra ninguna vista web, para
//! que ningún archivo esté en uso.
//!
//! Lo que no puede viajar: las sesiones de los portales (Correo, Teams…) están
//! cifradas por Windows para cada equipo. Por eso el perfil del navegador se
//! guarda por equipo dentro del pendrive: hay que entrar una vez en cada PC, y
//! luego se mantiene. Llevarse la sesión de un equipo a otro es justo lo que
//! Microsoft detecta como robo de sesión.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Archivos y carpetas que son de un equipo concreto (van a `equipos\<equipo>`).
const MACHINE_ONLY: &[&str] = &["snapshots", "maintenance.json", "speedtests.json", "registro-copias", "rdp", "window.json", "windows-alerts.json", "session.json"];

fn is_machine_only(name: &str) -> bool {
    MACHINE_ONLY.iter().any(|m| m.eq_ignore_ascii_case(name)) || name.to_lowercase().starts_with("journal")
}

/// Cachés del navegador: se regeneran solas y pesan mucho.
fn is_cache(name: &str) -> bool {
    ["cache", "code cache", "gpucache", "grshadercache", "shadercache", "dawncache", "dawngraphitecache", "crashpad"].contains(&name.to_lowercase().as_str())
}

/// ¿Está el programa en una unidad extraíble (un pendrive)?
#[cfg(windows)]
pub fn on_removable_drive(exe_dir: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    const DRIVE_REMOVABLE: u32 = 2;
    let Some(root) = exe_dir.components().next() else { return false };
    let root: PathBuf = [root.as_os_str(), std::ffi::OsStr::new("\\")].iter().collect();
    let wide: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: cadena terminada en cero; solo consulta el tipo de unidad.
    unsafe { GetDriveTypeW(wide.as_ptr()) == DRIVE_REMOVABLE }
}

#[cfg(not(windows))]
pub fn on_removable_drive(_: &Path) -> bool {
    false
}

/// Copia una carpeta entera sin pisar lo que ya exista en el destino.
fn copy_tree(from: &Path, to: &Path, skip: &dyn Fn(&str) -> bool) -> u64 {
    let Ok(rd) = std::fs::read_dir(from) else { return 0 };
    let _ = std::fs::create_dir_all(to);
    let mut n = 0;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if skip(&name) {
            continue;
        }
        let dst = to.join(&name);
        match e.file_type() {
            Ok(t) if t.is_dir() => n += copy_tree(&e.path(), &dst, skip),
            Ok(_) if !dst.exists() && std::fs::copy(e.path(), &dst).is_ok() => n += 1,
            _ => {}
        }
    }
    n
}

/// Las contraseñas cifradas con DPAPI (solo legibles en este equipo) se cifran
/// otra vez con la clave del pendrive. Recorre todos los textos del JSON.
fn reseal(v: &mut serde_json::Value, root: &Path) -> usize {
    match v {
        serde_json::Value::String(s) if crate::secrets::is_dpapi(s) => match crate::secrets::open(s).and_then(|p| crate::secrets::seal_in(root, &p)) {
            Ok(new) => {
                *s = new;
                1
            }
            Err(_) => 0,
        },
        serde_json::Value::Array(a) => a.iter_mut().map(|x| reseal(x, root)).sum(),
        serde_json::Value::Object(o) => o.values_mut().map(|x| reseal(x, root)).sum(),
        _ => 0,
    }
}

fn reseal_files(dir: &Path, root: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let mut n = 0;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if !is_cache(&e.file_name().to_string_lossy()) && e.file_name() != "webview" {
                n += reseal_files(&p, root);
            }
            continue;
        }
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let changed = reseal(&mut v, root);
        if changed > 0 && crate::paths::write_json(&p, &v).is_ok() {
            n += changed;
        }
    }
    n
}

/// Lo que se trajo en la migración (para el registro y Ajustes).
#[derive(Serialize, serde::Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Migrated {
    pub at: u64,
    pub files: u64,
    pub secrets: usize,
    pub browser: bool,
    pub from_host: String,
}

const MIGRATED_FILE: &str = "traido-del-equipo.json";

/// Al arrancar en portable: si el pendrive aún no tiene datos y este equipo sí
/// (AdminOps estaba instalado aquí), se traen. Antes de abrir WebView2.
pub fn migrate_if_needed(root: &Path, host: &str) -> Option<Migrated> {
    let fresh = !root.join("settings.json").exists();
    let forced = crate::paths::marker_requests_migration();
    if !fresh && !forced {
        return None;
    }
    let appdata = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("com.adminops.app"))?;
    if !appdata.join("settings.json").exists() || appdata == root {
        crate::paths::clear_migration_request();
        return None;
    }
    let machine = root.join("equipos").join(host);
    let mut files = copy_tree(&appdata, root, &|name| is_machine_only(name) || name == "logs" || name == "EBWebView");
    // Lo propio de este equipo, a su carpeta dentro del pendrive.
    if let Ok(rd) = std::fs::read_dir(&appdata) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !is_machine_only(&name) {
                continue;
            }
            let dst = machine.join(&name);
            if e.path().is_dir() {
                files += copy_tree(&e.path(), &dst, &|_| false);
            } else if !dst.exists() {
                let _ = std::fs::create_dir_all(&machine);
                if std::fs::copy(e.path(), &dst).is_ok() {
                    files += 1;
                }
            }
        }
    }
    let secrets = reseal_files(root, root);
    // La sesión de los portales se queda en el disco de este equipo (ver
    // paths::browser_on_usb): allí sigue, sin copiarla.
    let browser = false;
    crate::paths::clear_migration_request();
    let m = Migrated { at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()), files, secrets, browser, from_host: host.to_string() };
    let _ = crate::paths::write_json(&root.join(MIGRATED_FILE), &m);
    Some(m)
}

// ---------- Ajustes ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    /// Los datos viajan con el programa (portable).
    portable: bool,
    /// "removable" (está en un pendrive), "marker" (se pidió) o "" (instalado).
    reason: String,
    /// Letra de la unidad del programa («E:»), nunca la ruta (lleva el usuario).
    drive: String,
    /// Se puede pasar a portable (la carpeta del programa se puede escribir).
    can_switch: bool,
    /// Última vez que se trajeron datos de un equipo.
    migrated: Option<Migrated>,
    /// El navegador interno (sesiones de los portales) se guarda en el pendrive.
    browser_on_usb: bool,
}

#[tauri::command]
pub fn storage_info() -> StorageInfo {
    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)).unwrap_or_default();
    let drive = exe.components().next().map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default();
    let reason = crate::paths::portable_reason().to_string();
    StorageInfo {
        portable: crate::paths::is_portable(),
        reason,
        drive,
        can_switch: crate::paths::exe_dir_writable(),
        migrated: crate::paths::portable_data_root().map(|r| crate::paths::read_json::<Migrated>(&r.join(MIGRATED_FILE))).filter(|m| m.at > 0),
        browser_on_usb: crate::paths::browser_on_usb(),
    }
}

/// Guardar (o no) el navegador interno en el pendrive. Surte efecto al volver a abrir AdminOps.
#[tauri::command]
pub fn storage_set_browser_on_usb(on: bool) -> Result<(), String> {
    crate::paths::set_browser_on_usb(on)
}

/// Deja el archivo `AdminOps.portable` junto al programa pidiendo traer los
/// datos de este equipo. Surte efecto al volver a abrir AdminOps.
#[tauri::command]
pub fn storage_make_portable() -> Result<(), String> {
    crate::paths::request_portable_with_migration()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_files_stay_per_machine() {
        assert!(is_machine_only("journal.json"));
        assert!(is_machine_only("journal-2026-09.json"));
        assert!(is_machine_only("window.json"));
        assert!(is_machine_only("snapshots"));
        assert!(!is_machine_only("contacts.json"));
        assert!(!is_machine_only("settings.json"));
        assert!(is_cache("Code Cache") && is_cache("GPUCache") && !is_cache("Default"));
    }

    /// Una instalación con datos, un pendrive vacío: se trae todo, lo del equipo
    /// a su carpeta, y lo que ya había en el pendrive no se pisa.
    #[test]
    fn copies_without_overwriting() {
        let base = std::env::temp_dir().join(format!("adminops-storage-{}", std::process::id()));
        let (from, to) = (base.join("appdata"), base.join("usb"));
        std::fs::create_dir_all(from.join("contacts-copias")).unwrap();
        std::fs::write(from.join("contacts.json"), "[1]").unwrap();
        std::fs::write(from.join("contacts-copias").join("a.json"), "[]").unwrap();
        std::fs::write(from.join("journal.json"), "{}").unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(to.join("contacts.json"), "[2]").unwrap();
        let n = copy_tree(&from, &to, &|name| is_machine_only(name));
        assert_eq!(n, 1, "solo la copia de contactos; contacts.json ya estaba");
        assert_eq!(std::fs::read_to_string(to.join("contacts.json")).unwrap(), "[2]");
        assert!(!to.join("journal.json").exists(), "el diario es del equipo");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Una contraseña de Windows (DPAPI) dentro de un JSON pasa a la clave del pendrive.
    #[cfg(windows)]
    #[test]
    fn dpapi_secrets_are_resealed() {
        let base = std::env::temp_dir().join(format!("adminops-reseal-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let dpapi = {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(crate::network::lan::dpapi(b"clave del router", true).unwrap())
        };
        assert!(crate::secrets::is_dpapi(&dpapi));
        let mut v = serde_json::json!({ "routers": [{ "name": "Casa", "password": dpapi }], "otro": "AQAA no es" });
        assert_eq!(reseal(&mut v, &base), 1);
        let nuevo = v["routers"][0]["password"].as_str().unwrap().to_string();
        assert!(nuevo.starts_with("p1:"));
        assert_eq!(v["otro"], "AQAA no es");
        assert_eq!(crate::secrets::open_in(&base, &nuevo).unwrap(), "clave del router");
        let _ = std::fs::remove_dir_all(&base);
    }
}
