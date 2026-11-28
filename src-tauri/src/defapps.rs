//! Programas predeterminados: qué abre cada tipo de archivo que importa en la oficina (PDF, la web,
//! CSV, Word y Excel, imágenes, el correo) y cambiarlo «como en la oficina».
//!
//! Windows 11 no deja que un programa cambie lo predeterminado de otro: la elección del usuario va
//! firmada (UserChoice con huella) y el controlador UCPD bloquea el PDF y el navegador. Lo que sí
//! respeta Windows:
//! - en un equipo **unido a un dominio**, la directiva «archivo de asociaciones predeterminadas»
//!   (`DefaultAssociationsConfiguration`): Windows aplica el XML al iniciar sesión, a todos los
//!   usuarios, y lo vuelve a poner si una actualización lo cambia;
//! - fuera de dominio, la página de Configuración de ese programa, donde un clic en «Establecer
//!   predeterminado» lo deja puesto (lo abre AdminOps ya en esa página).
//!
//! Lo que la oficina quiere para cada tipo se elige una vez y se guarda con los datos de AdminOps
//! (`defapps.json`), por nombre de programa: así vale en cualquier equipo.

use serde::Serialize;
use std::collections::BTreeMap;
use tauri::Manager;

/// Los tipos que se enseñan, con las extensiones o protocolos que agrupan.
pub const GROUPS: &[(&str, &str, &[&str])] = &[
    ("pdf", "PDF", &[".pdf"]),
    ("web", "Páginas web", &["http", "https", ".htm", ".html"]),
    ("csv", "CSV", &[".csv"]),
    ("excel", "Excel", &[".xlsx", ".xls", ".xlsm"]),
    ("word", "Word", &[".docx", ".doc", ".rtf"]),
    ("image", "Imágenes", &[".jpg", ".jpeg", ".png", ".gif", ".bmp"]),
    ("mail", "Correo (mailto)", &["mailto"]),
];

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub name: String,
    /// Registrado para todo el equipo (si no, solo para este usuario).
    pub machine: bool,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub label: String,
    /// Qué lo abre ahora (nombre del programa), o vacío si Windows no tiene nada elegido.
    pub current: String,
    /// Los programas que lo pueden abrir.
    pub candidates: Vec<Candidate>,
    /// Lo que quiere la oficina para este tipo, si se eligió.
    pub office: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DefaultsView {
    pub rows: Vec<Row>,
    /// Unido a un dominio: «Como en la oficina» se aplica con la directiva.
    pub domain: bool,
    /// Ya hay una directiva de AdminOps puesta (se aplica al iniciar sesión).
    pub policy: bool,
    pub admin: bool,
}

/// Un programa registrado y lo que declara que abre (identificador → ProgId).
#[derive(Debug, Clone)]
struct App {
    name: String,
    machine: bool,
    assoc: BTreeMap<String, String>,
}

const POLICY_KEY: &str = r"HKLM\SOFTWARE\Policies\Microsoft\Windows\System";
const POLICY_VALUE: &str = "DefaultAssociationsConfiguration";

fn policy_file() -> Option<std::path::PathBuf> {
    std::env::var_os("ProgramData").map(|p| std::path::Path::new(&p).join(r"AdminOps\asociaciones.xml"))
}

fn user_root() -> String {
    match crate::target_user::hkcu_redirect() {
        Some(sid) => format!(r"HKU\{sid}"),
        None => "HKCU".into(),
    }
}

/// «@%ProgramFiles%\…\x.exe,-123» o «@{Paquete?ms-resource://…}» → el texto de verdad.
#[cfg(windows)]
fn indirect(s: &str) -> String {
    use windows_sys::Win32::UI::Shell::SHLoadIndirectString;
    if !s.starts_with('@') {
        return s.to_string();
    }
    let src: Vec<u16> = s.encode_utf16().chain(Some(0)).collect();
    let mut buf = [0u16; 512];
    // SAFETY: búferes propios con su tamaño.
    let hr = unsafe { SHLoadIndirectString(src.as_ptr(), buf.as_mut_ptr(), buf.len() as u32, std::ptr::null()) };
    if hr < 0 {
        return String::new();
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(not(windows))]
fn indirect(s: &str) -> String {
    s.to_string()
}

fn registered_apps() -> Vec<App> {
    use crate::tweaks::registry::{raw_to_string, read_string, values};
    let mut out: Vec<App> = Vec::new();
    for (root, machine) in [("HKLM".to_string(), true), (user_root(), false)] {
        for (reg_name, raw) in values(&format!(r"{root}\SOFTWARE\RegisteredApplications")) {
            let Some(cap) = raw_to_string(&raw) else { continue };
            let cap = format!(r"{root}\{}", cap.trim_start_matches('\\'));
            let name = read_string(&cap, "ApplicationName").map(|n| indirect(&n)).filter(|n| !n.trim().is_empty()).unwrap_or(reg_name);
            let mut assoc = BTreeMap::new();
            for sub in ["FileAssociations", "URLAssociations"] {
                for (id, v) in values(&format!(r"{cap}\{sub}")) {
                    if let Some(p) = raw_to_string(&v) {
                        assoc.insert(id.to_ascii_lowercase(), p);
                    }
                }
            }
            if !assoc.is_empty() && !out.iter().any(|a| a.name == name) {
                out.push(App { name, machine, assoc });
            }
        }
    }
    out
}

/// El ProgId elegido ahora para ese identificador (el de Windows 11 reciente primero).
fn current_progid(id: &str) -> Option<String> {
    use crate::tweaks::registry::read_string;
    let u = user_root();
    let base = if id.starts_with('.') {
        format!(r"{u}\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\{id}")
    } else {
        format!(r"{u}\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\{id}")
    };
    ["UserChoiceLatest", "UserChoice"]
        .iter()
        .find_map(|k| read_string(&format!(r"{base}\{k}"), "ProgId"))
        .filter(|p| !p.is_empty())
        // Sin elección del usuario: lo que Windows tiene asociado para todos (p. ej. Excel para .csv).
        .or_else(|| if id.starts_with('.') { read_string(&format!(r"HKCR\{id}"), "").filter(|p| !p.is_empty()) } else { None })
}

/// Nombre de un ProgId: el programa que lo registró o lo que diga el propio ProgId.
fn progid_name(progid: &str, apps: &[App]) -> String {
    use crate::tweaks::registry::read_string;
    if let Some(a) = apps.iter().find(|a| a.assoc.values().any(|p| p.eq_ignore_ascii_case(progid))) {
        return a.name.clone();
    }
    read_string(&format!(r"HKCR\{progid}\Application"), "ApplicationName")
        .map(|n| indirect(&n))
        .filter(|n| !n.trim().is_empty())
        .or_else(|| read_string(&format!(r"HKCR\{progid}"), "").map(|n| indirect(&n)).filter(|n| !n.trim().is_empty()))
        .unwrap_or_else(|| progid.to_string())
}

#[cfg(windows)]
pub fn domain_joined() -> bool {
    use windows_sys::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetGetJoinInformation, NetSetupDomainName};
    let mut name: windows_sys::core::PWSTR = std::ptr::null_mut();
    let mut status = 0;
    // SAFETY: el nombre lo reserva la API y se libera con NetApiBufferFree.
    unsafe {
        let r = NetGetJoinInformation(std::ptr::null(), &mut name, &mut status);
        if !name.is_null() {
            NetApiBufferFree(name.cast());
        }
        r == 0 && status == NetSetupDomainName
    }
}

#[cfg(not(windows))]
pub fn domain_joined() -> bool {
    false
}

type Office = BTreeMap<String, String>;

fn office_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("defapps.json")
}

fn view(app: &tauri::AppHandle) -> DefaultsView {
    let apps = registered_apps();
    let office: Office = crate::paths::read_json(&office_path(app));
    let rows = GROUPS
        .iter()
        .map(|(id, label, ids)| {
            let current = current_progid(ids[0]).map(|p| progid_name(&p, &apps)).unwrap_or_default();
            let mut candidates: Vec<Candidate> = apps.iter().filter(|a| a.assoc.contains_key(ids[0])).map(|a| Candidate { name: a.name.clone(), machine: a.machine }).collect();
            candidates.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            Row { id: (*id).into(), label: (*label).into(), current, candidates, office: office.get(*id).cloned() }
        })
        .collect();
    let policy = crate::tweaks::registry::read_string(POLICY_KEY, POLICY_VALUE).is_some_and(|p| policy_file().is_some_and(|f| std::path::Path::new(&p) == f));
    DefaultsView { rows, domain: domain_joined(), policy, admin: crate::elevation::is_elevated() }
}

#[tauri::command]
pub async fn defapps_view(app: tauri::AppHandle) -> Result<DefaultsView, String> {
    tauri::async_runtime::spawn_blocking(move || view(&app)).await.map_err(|e| e.to_string())
}

/// Guarda lo que quiere la oficina para un tipo (vacío: sin preferencia).
#[tauri::command]
pub fn defapps_set_office(app: tauri::AppHandle, id: String, name: String) -> Result<(), String> {
    if !GROUPS.iter().any(|(g, _, _)| *g == id) {
        return Err("Tipo de archivo no válido.".into());
    }
    let path = office_path(&app);
    let mut office: Office = crate::paths::read_json(&path);
    if name.trim().is_empty() {
        office.remove(&id);
    } else {
        office.insert(id, name.trim().to_string());
    }
    crate::paths::write_json(&path, &office)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

/// El XML de la directiva para lo elegido: cada identificador del grupo con el ProgId que ese programa
/// declara para él (si no lo declara, ese identificador se queda como esté).
pub fn build_xml(choices: &[(&[&str], String, &BTreeMap<String, String>)]) -> (String, usize) {
    let mut lines = Vec::new();
    for (ids, name, assoc) in choices {
        for id in ids.iter() {
            if let Some(p) = assoc.get(*id) {
                lines.push(format!("  <Association Identifier=\"{}\" ProgId=\"{}\" ApplicationName=\"{}\" />", xml_escape(id), xml_escape(p), xml_escape(name)));
            }
        }
    }
    let n = lines.len();
    (format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<DefaultAssociations>\n{}\n</DefaultAssociations>\n", lines.join("\n")), n)
}

/// «Como en la oficina» en un equipo de dominio: escribe el XML y la directiva. Devuelve cuántas
/// asociaciones quedaron puestas. Se aplica al volver a iniciar sesión.
#[tauri::command]
pub async fn defapps_apply(app: tauri::AppHandle) -> Result<usize, String> {
    let before = crate::tweaks::registry::read_string(POLICY_KEY, POLICY_VALUE);
    // Si ya apuntaba al archivo de AdminOps (aplicado antes), ese archivo se sobrescribe: volver a «como estaba» no
    // cambiaría nada. Entonces no se ofrece deshacer.
    let own = before.as_deref().is_some_and(|b| policy_file().is_some_and(|f| std::path::Path::new(b) == f));
    let r = defapps_apply_inner(app.clone()).await;
    if own {
        crate::tweaks::record_app(&app, "Programas predeterminados como en la oficina", &r);
    } else {
        crate::tweaks::record_undoable(&app, "Programas predeterminados como en la oficina", vec![crate::tweaks::journal::Backup::DefaultAppsPolicy { previous: before }], &r);
    }
    r
}

async fn defapps_apply_inner(app: tauri::AppHandle) -> Result<usize, String> {
    if !crate::elevation::is_elevated() {
        return Err("Hace falta abrir AdminOps como administrador.".into());
    }
    if !domain_joined() {
        return Err("Fuera de un dominio Windows no aplica la directiva: cámbialo desde Configuración con «Cambiar».".into());
    }
    // Si IT ya puso su propio archivo de asociaciones, no se pisa.
    if let Some(cur) = crate::tweaks::registry::read_string(POLICY_KEY, POLICY_VALUE).filter(|c| !c.trim().is_empty()) {
        if policy_file().is_none_or(|f| std::path::Path::new(&cur) != f) {
            return Err("Este equipo ya tiene un archivo de asociaciones puesto por una directiva de la empresa: AdminOps no lo cambia.".into());
        }
    }
    tauri::async_runtime::spawn_blocking(move || {
        let (xml, n) = office_xml(&app)?;
        let file = policy_file().ok_or("No se encuentra la carpeta de datos de programas (ProgramData).")?;
        write_protected(&file, &xml)?;
        use crate::tweaks::model::{RegData, RegKind};
        crate::tweaks::registry::write(POLICY_KEY, POLICY_VALUE, RegKind::String, &RegData::Str(file.to_string_lossy().into_owned()))?;
        log::info!("Programas predeterminados: directiva con {n} de la lista");
        Ok(n)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// El XML con lo elegido «en la oficina» para los programas que hay en este equipo.
fn office_xml(app: &tauri::AppHandle) -> Result<(String, usize), String> {
    let apps = registered_apps();
    let office: Office = crate::paths::read_json(&office_path(app));
    let choices: Vec<(&[&str], String, &BTreeMap<String, String>)> = GROUPS
        .iter()
        .filter_map(|(id, _, ids)| {
            let name = office.get(*id)?;
            let a = apps.iter().find(|a| &a.name == name)?;
            Some((*ids, name.clone(), &a.assoc))
        })
        .collect();
    let (xml, n) = build_xml(&choices);
    if n == 0 {
        return Err("No hay nada elegido «en la oficina», o esos programas no están en este equipo.".into());
    }
    // Lo que se cuenta al técnico son los tipos de la lista (Páginas web, PDF…) que de verdad quedan puestos, no cada
    // asociación (Páginas web son cuatro: http, https, .htm y .html) ni los que ese programa no sabe abrir.
    let kinds = choices.iter().filter(|(ids, _, assoc)| ids.iter().any(|id| assoc.contains_key(*id))).count();
    Ok((xml, kinds))
}

/// ¿Es un enlace (junction o enlace simbólico)? Se mira sin seguirlo.
fn is_link(p: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        std::fs::symlink_metadata(p).is_ok_and(|m| m.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(windows))]
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

/// ¿Hay algún enlace dentro (en cualquier nivel)? Se recorre sin seguirlos.
fn has_links_inside(dir: &std::path::Path) -> bool {
    let Ok(list) = std::fs::read_dir(dir) else { return false };
    list.flatten().any(|e| {
        let p = e.path();
        is_link(&p) || (p.is_dir() && has_links_inside(&p))
    })
}

/// Escribe en la carpeta de AdminOps en ProgramData, que Windows lee con privilegios: la carpeta, solo de
/// administradores y SYSTEM (lectura para el resto), aunque alguien la hubiera creado antes.
///
/// Los permisos se ponen solo en la carpeta (sin /T, Windows los hereda hacia dentro) y nunca si ella o algo de dentro
/// es un enlace: un usuario puede crear cosas en ProgramData y un icacls como administrador que siguiera un enlace
/// cambiaría los permisos de otra carpeta (System32). El archivo se crea de nuevo: uno que dejara otro sería suyo.
fn write_protected(file: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        if is_link(dir) || has_links_inside(dir) {
            return Err(format!("La carpeta {} tiene enlaces a otros sitios: no se toca. Bórralos y vuelve a probar.", dir.display()));
        }
        let d = dir.to_string_lossy().into_owned();
        crate::ps::exec("icacls", &[&d, "/setowner", "*S-1-5-32-544", "/C", "/Q"]).map_err(|e| format!("No se pudo proteger la carpeta: {e}"))?;
        crate::ps::exec("icacls", &[&d, "/inheritance:r", "/grant:r", "*S-1-5-32-544:(OI)(CI)F", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-545:(OI)(CI)RX", "/C", "/Q"])
            .map_err(|e| format!("No se pudo proteger la carpeta: {e}"))?;
    }
    // remove_file quita el enlace si lo fuera, no lo que hay detrás; el nuevo es de los administradores.
    let _ = std::fs::remove_file(file);
    std::fs::write(file, text).map_err(|e| format!("No se pudo escribir el archivo de asociaciones: {e}"))
}

/// «Como en la oficina» para los usuarios que se creen después en este equipo (también fuera de dominio):
/// `DISM /Import-DefaultAppAssociations` deja el XML como lo de fábrica de los perfiles nuevos.
#[tauri::command]
pub async fn defapps_apply_new_users(app: tauri::AppHandle) -> Result<usize, String> {
    if !crate::elevation::is_elevated() {
        return Err("Hace falta abrir AdminOps como administrador.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let (xml, n) = office_xml(&app)?;
        let file = std::env::var_os("ProgramData")
            .map(|p| std::path::Path::new(&p).join(r"AdminOps\asociaciones-usuarios-nuevos.xml"))
            .ok_or("No se encuentra la carpeta de datos de programas (ProgramData).")?;
        write_protected(&file, &xml)?;
        let arg = format!("/Import-DefaultAppAssociations:{}", file.to_string_lossy());
        crate::ps::exec_opts("dism.exe", &["/Online", &arg], crate::ps::Opts { timeout: Some(std::time::Duration::from_secs(120)), task: None })
            .map_err(|e| format!("Windows no aceptó las asociaciones: {}", crate::ps::clean_error(&e)))?;
        if let Some(state) = app.try_state::<crate::tweaks::TweakState>() {
            state.record(crate::tweaks::journal::Op::Run, &format!("Programas predeterminados para usuarios nuevos ({n} de la lista)"), &Ok::<(), String>(()));
        }
        log::info!("Programas predeterminados: {n} de la lista para los usuarios nuevos");
        Ok(n)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Quita la directiva de AdminOps (lo predeterminado vuelve a ser cosa de cada usuario).
#[tauri::command]
pub fn defapps_remove_policy(app: tauri::AppHandle) -> Result<(), String> {
    let before = crate::tweaks::registry::read_string(POLICY_KEY, POLICY_VALUE);
    let r = defapps_remove_policy_inner();
    crate::tweaks::record_undoable(&app, "Programas predeterminados: quitada la directiva de AdminOps", vec![crate::tweaks::journal::Backup::DefaultAppsPolicy { previous: before }], &r);
    r
}

/// «Volver a como estaba» desde el historial: la directiva de antes (o ninguna). El archivo de AdminOps se queda: sin
/// la directiva apuntándolo, Windows no lo usa.
pub fn restore_policy(previous: Option<&str>) -> Result<(), String> {
    match previous {
        Some(p) => crate::tweaks::registry::write(POLICY_KEY, POLICY_VALUE, crate::tweaks::model::RegKind::String, &crate::tweaks::model::RegData::Str(p.to_string())),
        None => {
            if crate::tweaks::registry::read_string(POLICY_KEY, POLICY_VALUE).is_some() {
                crate::tweaks::registry::delete(POLICY_KEY, POLICY_VALUE)?;
            }
            Ok(())
        }
    }
}

fn defapps_remove_policy_inner() -> Result<(), String> {
    if crate::tweaks::registry::read_string(POLICY_KEY, POLICY_VALUE).is_some() {
        crate::tweaks::registry::delete(POLICY_KEY, POLICY_VALUE)?;
    }
    if let Some(f) = policy_file() {
        let _ = std::fs::remove_file(f);
    }
    Ok(())
}

/// Abre la página de Configuración de ese programa («Establecer predeterminado»), como el usuario.
#[tauri::command]
pub fn defapps_open_settings(name: String, machine: bool) -> Result<(), String> {
    let enc: String = name
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    let key = if machine { "registeredAppMachine" } else { "registeredAppUser" };
    crate::shellopen::open(&format!("ms-settings:defaultapps?{key}={enc}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_de_la_directiva() {
        let mut acro = BTreeMap::new();
        acro.insert(".pdf".to_string(), "Acrobat.Document.DC".to_string());
        let mut chrome = BTreeMap::new();
        chrome.insert("http".to_string(), "ChromeHTML".to_string());
        chrome.insert("https".to_string(), "ChromeHTML".to_string());
        chrome.insert(".html".to_string(), "ChromeHTML".to_string());
        let web: &[&str] = &["http", "https", ".htm", ".html"];
        let pdf: &[&str] = &[".pdf"];
        let (xml, n) = build_xml(&[(pdf, "Adobe Acrobat".into(), &acro), (web, "Google Chrome & Co".into(), &chrome)]);
        assert_eq!(n, 4);
        assert!(xml.contains("<Association Identifier=\".pdf\" ProgId=\"Acrobat.Document.DC\" ApplicationName=\"Adobe Acrobat\" />"));
        // .htm no lo declara Chrome: se queda fuera.
        assert!(!xml.contains("\".htm\""));
        assert!(xml.contains("Google Chrome &amp; Co"));
    }
}
