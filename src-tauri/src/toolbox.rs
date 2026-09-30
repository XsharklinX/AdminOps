//! Herramientas: accesos rápidos a las utilidades de Windows (catálogo
//! incrustado en `tools/shortcuts.toml`), favoritos y accesos propios del técnico.
//!
//! La UI solo envía el `id` de un acceso: el programa, sus argumentos y la
//! forma de abrirlo salen del catálogo o del archivo de accesos propios.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use tauri::State;

const CATALOG: &str = include_str!("../tools/shortcuts.toml");
#[cfg(test)]
const GROUPS: &[&str] = &["console", "admin", "diag", "panel", "settings", "folders", "boot", "remote"];

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct ToolDef {
    id: String,
    name: String,
    description: String,
    group: String,
    icon: String,
    #[serde(default)]
    keywords: String,
    run: Option<String>,
    #[serde(default)]
    args: String,
    open: Option<String>,
    #[serde(default)]
    elevate: bool,
    #[serde(default)]
    as_user: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    admin: bool,
    requires: Option<String>,
    confirm: Option<String>,
}

#[derive(Deserialize)]
struct Catalog {
    tool: Vec<ToolDef>,
}

static TOOLS: LazyLock<Vec<ToolDef>> =
    LazyLock::new(|| toml::from_str::<Catalog>(CATALOG).expect("tools/shortcuts.toml inválido").tool);

/// Los ids del catálogo de herramientas (lo usan los tests que comprueban que
/// los botones de las soluciones apuntan a algo que existe).
#[cfg(test)]
pub fn catalog_ids() -> Vec<String> {
    TOOLS.iter().map(|t| t.id.clone()).collect()
}

// ---------- Variables y disponibilidad ----------

/// Sustituye `{System32}`, `{UserTemp}`… por las rutas de este equipo y del
/// usuario destino. `None` si falta alguna (p. ej. no se pudo leer el perfil).
fn expand(s: &str) -> Option<String> {
    if !s.contains('{') || s.starts_with("shell:") {
        return Some(s.to_string());
    }
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let user = crate::target_user::user_dirs();
    let dir = |p: Option<&PathBuf>| p.map(|p| p.display().to_string());
    let vars: [(&str, Option<String>); 8] = [
        ("{System32}", Some(format!(r"{windows}\System32"))),
        ("{Windows}", Some(windows.clone())),
        ("{ProgramData}", std::env::var("ProgramData").ok()),
        ("{ProgramFiles}", std::env::var("ProgramFiles").ok()),
        ("{UserProfile}", dir(user.as_ref().map(|u| &u.profile))),
        ("{UserAppData}", dir(user.as_ref().map(|u| &u.app_data))),
        ("{UserLocalAppData}", dir(user.as_ref().map(|u| &u.local_app_data))),
        ("{UserTemp}", dir(user.as_ref().map(|u| &u.temp))),
    ];
    let mut out = s.to_string();
    for (k, v) in vars {
        if out.contains(k) {
            out = out.replace(k, &v?);
        }
    }
    (!out.contains('{')).then_some(out)
}

/// ¿Existe lo que el acceso necesita? `Err` con el motivo para mostrarlo.
fn availability(t: &ToolDef) -> Result<(), String> {
    let Some(req) = &t.requires else { return Ok(()) };
    match expand(req) {
        Some(p) if Path::new(&p).exists() => Ok(()),
        Some(_) if req.ends_with(".msc") => Err("No incluido en esta edición de Windows (suele faltar en Home).".into()),
        _ => Err("No está disponible en este equipo.".into()),
    }
}

// ---------- Lanzar ----------

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// ShellExecute: abre programas, consolas .msc y .cpl con sus asociaciones y
/// muestra el aviso de UAC cuando hace falta. No pasa por ninguna consola, así
/// que los argumentos nunca se interpretan como comandos.
#[cfg(windows)]
fn shell_execute(file: &str, args: &str, dir: Option<&Path>, elevate: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let verb = wide(if elevate { "runas" } else { "open" });
    let file_w = wide(file);
    let args_w = wide(args);
    let dir_w = dir.map(|d| wide(&d.display().to_string()));
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file_w.as_ptr(),
            if args.is_empty() { std::ptr::null() } else { args_w.as_ptr() },
            dir_w.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
            SW_SHOWNORMAL,
        )
    } as isize;
    // ShellExecute devuelve un valor > 32 si tuvo éxito.
    match code {
        c if c > 32 => Ok(()),
        2 | 3 => Err("No se encontró el programa o la ruta.".into()),
        5 => Err("Se canceló el aviso de administrador (UAC) o Windows denegó el acceso.".into()),
        31 => Err("No hay ningún programa asociado para abrirlo.".into()),
        c => Err(format!("Windows no pudo abrirlo (código {c}).")),
    }
}

#[cfg(not(windows))]
fn shell_execute(_: &str, _: &str, _: Option<&Path>, _: bool) -> Result<(), String> {
    Err("Solo disponible en Windows.".into())
}

/// Lo abre el Explorador ya en marcha, que es del usuario con la sesión
/// abierta: carpetas, `shell:`, `ms-settings:` y programas sin administrador.
fn open_as_user(target: &str) -> Result<(), String> {
    std::process::Command::new("explorer.exe").arg(target).spawn().map(|_| ()).map_err(|e| format!("No se pudo abrir: {e}"))
}

fn is_path(s: &str) -> bool {
    s.len() > 2 && s.as_bytes()[1] == b':'
}

fn launch_builtin(t: &ToolDef, tweaks: &TweakState) -> Result<(), String> {
    availability(t)?;
    if t.admin && !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let unreadable = || "No se pudo resolver la ruta del usuario del equipo.".to_string();
    if let Some(open) = &t.open {
        let target = expand(open).ok_or_else(unreadable)?;
        if is_path(&target) && !Path::new(&target).exists() {
            return Err(format!("La carpeta no existe en este equipo: {}", t.name));
        }
        return open_as_user(&target);
    }
    let run = expand(t.run.as_deref().unwrap_or_default()).ok_or_else(unreadable)?;
    let args = expand(&t.args).ok_or_else(unreadable)?;
    if t.as_user {
        return open_as_user(&run);
    }
    if t.hidden {
        let parts: Vec<&str> = args.split_whitespace().collect();
        let result = crate::ps::exec(&run, &parts).map(|_| ());
        tweaks.record(Op::Run, &format!("Herramientas: {}", t.name), &result);
        return result;
    }
    let home = crate::target_user::user_dirs().map(|u| u.profile);
    shell_execute(&run, &args, home.as_deref(), t.elevate)
}

// ---------- Accesos propios y favoritos ----------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum CustomKind {
    Program,
    Folder,
    Url,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CustomTool {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub kind: CustomKind,
    pub target: String,
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub icon: String,
}

#[derive(Serialize, Deserialize, Default)]
struct ToolboxData {
    #[serde(default)]
    favorites: Vec<String>,
    #[serde(default)]
    custom: Vec<CustomTool>,
}

/// Serializa lectura-modificación-escritura del archivo.
static FILE_LOCK: Mutex<()> = Mutex::new(());

fn data_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("toolbox.json")
}

fn validate_custom(mut c: CustomTool) -> Result<CustomTool, String> {
    c.name = c.name.trim().to_string();
    c.target = c.target.trim().trim_matches('"').to_string();
    c.args = c.args.trim().to_string();
    if c.name.is_empty() || c.name.chars().count() > 60 {
        return Err("El nombre es obligatorio (máximo 60 caracteres).".into());
    }
    if c.target.is_empty() || c.target.contains(['\n', '\r']) || c.args.contains(['\n', '\r']) {
        return Err("Indica el destino en una sola línea.".into());
    }
    match c.kind {
        CustomKind::Program => {
            if !Path::new(&c.target).is_file() {
                return Err("El programa no existe. Usa «Examinar…» para elegirlo.".into());
            }
            if c.args.len() > 500 {
                return Err("Los argumentos son demasiado largos.".into());
            }
        }
        CustomKind::Folder => {
            if !Path::new(&c.target).is_dir() {
                return Err("La carpeta no existe.".into());
            }
            c.args.clear();
        }
        CustomKind::Url => {
            let lower = c.target.to_ascii_lowercase();
            if !(lower.starts_with("https://") || lower.starts_with("http://")) || c.target.contains(char::is_whitespace) {
                return Err("La dirección debe empezar por https:// (o http://) y no llevar espacios.".into());
            }
            c.args.clear();
        }
    }
    if c.icon.trim().is_empty() {
        c.icon = match c.kind {
            CustomKind::Program => "app",
            CustomKind::Folder => "folder",
            CustomKind::Url => "globe",
        }
        .into();
    }
    Ok(c)
}

fn launch_custom(c: &CustomTool) -> Result<(), String> {
    match c.kind {
        CustomKind::Program => {
            if !Path::new(&c.target).is_file() {
                return Err(format!("«{}» ya no existe en este equipo.", c.target));
            }
            shell_execute(&c.target, &c.args, Path::new(&c.target).parent(), false)
        }
        CustomKind::Folder => {
            if !Path::new(&c.target).is_dir() {
                return Err("La carpeta no existe en este equipo.".into());
            }
            open_as_user(&c.target)
        }
        CustomKind::Url => open_as_user(&c.target),
    }
}

// ---------- Comandos ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolView {
    id: String,
    name: String,
    description: String,
    group: String,
    icon: String,
    keywords: String,
    /// Se abre con permisos de administrador.
    as_admin: bool,
    /// Sin administrador no hace nada útil.
    needs_admin: bool,
    confirm: Option<String>,
    /// `None` si está disponible; si no, el motivo.
    unavailable: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolboxView {
    tools: Vec<ToolView>,
    custom: Vec<CustomTool>,
    favorites: Vec<String>,
    /// Los accesos propios se abren con los permisos de AdminOps.
    elevated: bool,
}

#[tauri::command(async)]
pub fn list_tools(app: tauri::AppHandle) -> ToolboxView {
    let elevated = crate::elevation::is_elevated();
    let tools = TOOLS
        .iter()
        .map(|t| ToolView {
            id: t.id.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
            group: t.group.clone(),
            icon: t.icon.clone(),
            keywords: t.keywords.clone(),
            as_admin: t.run.is_some() && !t.as_user && !t.hidden && (t.elevate || elevated),
            needs_admin: t.admin,
            confirm: t.confirm.clone(),
            unavailable: availability(t).err(),
        })
        .collect();
    let data: ToolboxData = crate::paths::read_json(&data_path(&app));
    ToolboxView { tools, custom: data.custom, favorites: data.favorites, elevated }
}

#[tauri::command(async)]
pub fn launch_tool(app: tauri::AppHandle, id: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    let result = if let Some(t) = TOOLS.iter().find(|t| t.id == id) {
        launch_builtin(t, &tweaks)
    } else {
        let data: ToolboxData = crate::paths::read_json(&data_path(&app));
        let c = data.custom.iter().find(|c| c.id == id).ok_or("Ese acceso ya no existe.")?;
        launch_custom(c)
    };
    match &result {
        Ok(()) => log::info!("Herramientas: abierto «{id}»"),
        Err(e) => log::warn!("Herramientas: «{id}» → {e}"),
    }
    result
}

#[tauri::command]
pub fn set_tool_favorite(app: tauri::AppHandle, id: String, favorite: bool) -> Result<Vec<String>, String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = data_path(&app);
    let mut data: ToolboxData = crate::paths::read_json(&path);
    data.favorites.retain(|f| f != &id);
    if favorite {
        data.favorites.push(id);
    }
    crate::paths::write_json(&path, &data)?;
    Ok(data.favorites)
}

#[tauri::command]
pub fn save_custom_tool(app: tauri::AppHandle, tool: CustomTool) -> Result<CustomTool, String> {
    let mut c = validate_custom(tool)?;
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = data_path(&app);
    let mut data: ToolboxData = crate::paths::read_json(&path);
    match data.custom.iter_mut().find(|x| !c.id.is_empty() && x.id == c.id) {
        Some(x) => *x = c.clone(),
        None => {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            c.id = format!("custom-{stamp:x}");
            data.custom.push(c.clone());
        }
    }
    crate::paths::write_json(&path, &data)?;
    Ok(c)
}

#[tauri::command]
pub fn delete_custom_tool(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = data_path(&app);
    let mut data: ToolboxData = crate::paths::read_json(&path);
    data.custom.retain(|c| c.id != id);
    data.favorites.retain(|f| f != &id);
    crate::paths::write_json(&path, &data)
}

/// Selector nativo de Windows para elegir el destino de un acceso propio.
#[tauri::command(async)]
pub fn pick_tool_target(app: tauri::AppHandle, kind: CustomKind) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().file();
    let picked = match kind {
        CustomKind::Folder => dialog.blocking_pick_folder(),
        _ => dialog
            .add_filter("Programas", &["exe", "bat", "cmd", "msc", "cpl", "lnk"])
            .add_filter("Todos los archivos", &["*"])
            .blocking_pick_file(),
    };
    picked.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_is_valid() {
        let mut ids = HashSet::new();
        for t in TOOLS.iter() {
            assert!(ids.insert(&t.id), "id repetido: {}", t.id);
            assert!(GROUPS.contains(&t.group.as_str()), "{}: grupo desconocido {}", t.id, t.group);
            assert!(t.run.is_some() != t.open.is_some(), "{}: necesita run u open (solo uno)", t.id);
            assert!(!t.as_user || t.args.is_empty(), "{}: as_user no admite argumentos", t.id);
            assert!(!(t.as_user && t.elevate), "{}: as_user y elevate se excluyen", t.id);
            assert!(!t.name.is_empty() && !t.description.is_empty() && !t.icon.is_empty(), "{}", t.id);
            for s in [t.run.as_deref(), t.open.as_deref(), Some(t.args.as_str()), t.requires.as_deref()].into_iter().flatten() {
                assert!(expand(s).is_some(), "{}: variable desconocida en {s}", t.id);
            }
        }
        assert!(TOOLS.len() >= 60, "catálogo demasiado corto: {}", TOOLS.len());
    }

    /// Todo lo que no declara `requires` tiene que existir en cualquier Windows 10/11.
    #[test]
    fn programs_exist_on_this_machine() {
        for t in TOOLS.iter().filter(|t| t.requires.is_none()) {
            if let Some(run) = &t.run {
                let p = expand(run).unwrap();
                assert!(Path::new(&p).is_file(), "{}: no existe {p}", t.id);
            }
            if let Some(open) = t.open.as_deref().and_then(expand).filter(|o| is_path(o)) {
                assert!(Path::new(&open).exists(), "{}: no existe {open}", t.id);
            }
        }
    }

    #[test]
    fn validates_custom_tools() {
        let mk = |kind, target: &str| CustomTool { id: String::new(), name: "x".into(), kind, target: target.into(), args: String::new(), icon: String::new() };
        let windows = std::env::var("SystemRoot").unwrap();
        assert!(validate_custom(mk(CustomKind::Program, &format!(r"{windows}\notepad.exe"))).is_ok());
        assert!(validate_custom(mk(CustomKind::Program, r"C:\no\existe.exe")).is_err());
        assert!(validate_custom(mk(CustomKind::Folder, &windows)).is_ok());
        assert!(validate_custom(mk(CustomKind::Url, "https://example.com/a?b=1&c=2")).is_ok());
        assert!(validate_custom(mk(CustomKind::Url, "file:///C:/Windows")).is_err());
        assert!(validate_custom(mk(CustomKind::Url, "javascript:alert(1)")).is_err());
        let mut named = mk(CustomKind::Url, "https://a.b");
        named.name = "   ".into();
        assert!(validate_custom(named).is_err());
    }
}
