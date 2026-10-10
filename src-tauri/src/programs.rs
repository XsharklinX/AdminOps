//! Desinstalador de programas (Win32): lista del registro, desinstalación
//! silenciosa (MSI, Inno Setup, NSIS o la que declara el programa) o con su
//! asistente, reparación de MSI y limpieza de restos.
//!
//! Restos: carpetas, accesos directos del escritorio y del menú Inicio, y claves
//! del registro con el nombre exacto del programa. Las carpetas y accesos van a
//! la papelera; las claves se copian antes en un .reg. Solo se borran rutas que
//! calculó el propio backend para ese programa, nunca rutas enviadas por la
//! interfaz.

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::State;
use winreg::enums::*;
use winreg::RegKey;

const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
/// Carpetas y claves compartidas por muchos programas: nunca son "restos" de uno solo.
const SHARED: &[&str] = &[
    "microsoft", "windows", "common files", "intel", "amd", "nvidia", "nvidia corporation", "google", "mozilla", "packages", "temp",
    "programs", "microsoft shared", "windowsapps", "package cache", "installer", "system32", "classes", "policies", "wow6432node",
    "clients", "startup", "accessories", "administrative tools", "system tools", "windows powershell", "maintenance",
];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    /// hive|vista|clave: identifica la entrada del registro.
    id: String,
    pub(crate) name: String,
    pub(crate) publisher: Option<String>,
    pub(crate) version: Option<String>,
    /// AAAA-MM-DD
    installed: Option<String>,
    /// Bytes (estimación del propio instalador).
    size: Option<u64>,
    install_location: Option<String>,
    /// Se puede desinstalar sin ventanas.
    silent: bool,
    /// Cómo: MSI | Inno Setup | NSIS | Propio (vacío si no hay modo silencioso).
    silent_kind: String,
    /// MSI: se puede reparar.
    repairable: bool,
    /// Runtimes, redistribuibles, drivers…: se pueden ocultar en la lista.
    component: bool,
    /// El desinstalador ya no existe: entrada huérfana.
    orphan: bool,
    /// Solo para este usuario (HKCU).
    per_user: bool,
    #[serde(skip)]
    uninstall: Option<String>,
    #[serde(skip)]
    quiet: Option<String>,
    #[serde(skip)]
    msi: Option<String>,
}

struct Root {
    tag: &'static str,
    key: RegKey,
    path: String,
    flags: u32,
    per_user: bool,
}

fn roots() -> Vec<Root> {
    let mut v = vec![
        Root { tag: "HKLM64", key: RegKey::predef(HKEY_LOCAL_MACHINE), path: UNINSTALL.into(), flags: KEY_WOW64_64KEY, per_user: false },
        Root { tag: "HKLM32", key: RegKey::predef(HKEY_LOCAL_MACHINE), path: UNINSTALL.into(), flags: KEY_WOW64_32KEY, per_user: false },
    ];
    // Programas del usuario con la sesión abierta, aunque AdminOps se haya elevado con otra cuenta.
    match crate::target_user::hkcu_redirect() {
        Some(sid) => v.push(Root { tag: "HKU", key: RegKey::predef(HKEY_USERS), path: format!(r"{sid}\{UNINSTALL}"), flags: 0, per_user: true }),
        None => v.push(Root { tag: "HKCU", key: RegKey::predef(HKEY_CURRENT_USER), path: UNINSTALL.into(), flags: 0, per_user: true }),
    }
    v
}

fn open(root: &Root, sub: &str, write: bool) -> Option<RegKey> {
    let access = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
    root.key.open_subkey_with_flags(format!(r"{}\{sub}", root.path), access | root.flags).ok()
}

/// Separa un comando del registro en (programa, argumentos).
pub fn split_command(cmd: &str) -> (String, String) {
    let c = cmd.trim();
    if let Some(rest) = c.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_string(), rest[end + 1..].trim().to_string());
        }
    }
    let lower = c.to_ascii_lowercase();
    if let Some(i) = lower.find(".exe") {
        let end = i + 4;
        return (c[..end].trim().to_string(), c[end..].trim().to_string());
    }
    match c.split_once(char::is_whitespace) {
        Some((a, b)) => (a.to_string(), b.trim().to_string()),
        None => (c.to_string(), String::new()),
    }
}

fn is_guid(s: &str) -> bool {
    s.len() == 38 && s.starts_with('{') && s.ends_with('}') && s[1..37].chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// «MsiExec.exe /I{GUID}» o «/X{GUID}»: es un MSI aunque la entrada no lo diga.
fn msi_guid_in(cmd: &str) -> Option<String> {
    let l = cmd.to_ascii_lowercase();
    if !l.contains("msiexec") {
        return None;
    }
    let i = cmd.find('{')?;
    let guid = cmd.get(i..i + 38)?;
    is_guid(guid).then(|| guid.to_uppercase())
}

fn expand(path: &str) -> String {
    let mut p = path.to_string();
    for var in ["SystemRoot", "windir", "ProgramFiles", "ProgramFiles(x86)", "ProgramData"] {
        if let Ok(v) = std::env::var(var) {
            p = p.replace(&format!("%{var}%"), &v);
        }
    }
    p
}

fn exe_exists(cmd: &str) -> bool {
    let (exe, _) = split_command(cmd);
    let p = PathBuf::from(expand(&exe));
    // Sin ruta (msiexec, rundll32…) = está en el PATH del sistema.
    !p.is_absolute() || p.exists()
}

/// Tipo de desinstalador por su archivo, y sus argumentos para hacerlo en silencio.
/// Inno Setup (unins000.exe) y NSIS cubren la mayoría de programas gratuitos.
fn installer_kind(uninstall: &str) -> Option<(&'static str, &'static str)> {
    let (exe, _) = split_command(uninstall);
    let path = PathBuf::from(expand(&exe));
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name.starts_with("unins") && name.ends_with(".exe") && name[5..name.len() - 4].chars().all(|c| c.is_ascii_digit()) && name.len() == 12 {
        return Some(("Inno Setup", "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART"));
    }
    if name.contains("uninst") {
        // NSIS deja su firma en el ejecutable; solo se leen desinstaladores pequeños,
        // y una sola vez (caché por ruta, tamaño y fecha).
        let meta = std::fs::metadata(&path).ok()?;
        let stamp = (meta.len(), meta.modified().ok());
        let mut cache = NSIS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let nsis = match cache.get(&path) {
            Some((s, v)) if *s == stamp => *v,
            _ => {
                let v = meta.len() < 16 * 1024 * 1024 && std::fs::read(&path).is_ok_and(|b| b.windows(8).any(|w| w == b"Nullsoft"));
                cache.insert(path.clone(), (stamp, v));
                v
            }
        };
        if nsis {
            return Some(("NSIS", "/S"));
        }
    }
    None
}

type Stamp = (u64, Option<std::time::SystemTime>);
static NSIS_CACHE: LazyLock<Mutex<HashMap<PathBuf, (Stamp, bool)>>> = LazyLock::new(Default::default);

/// Runtimes, redistribuibles y controladores: normalmente no se tocan.
fn is_component(name: &str, publisher: Option<&str>) -> bool {
    let n = name.to_lowercase();
    let p = publisher.unwrap_or("").to_lowercase();
    [
        "visual c++",
        "redistributable",
        ".net ",
        ".net framework",
        "windows desktop runtime",
        "asp.net core",
        "directx",
        "microsoft update health",
        "windows sdk",
        "driver",
        "chipset",
        "vulkan run",
        "webview2 runtime",
        "microsoft edge update",
        "windows software development",
        "ms-mpi",
    ]
    .iter()
    .any(|k| n.contains(k))
        || (p.contains("advanced micro devices") && n.contains("amd "))
        || (p.starts_with("intel") && (n.contains("management engine") || n.contains("serial io") || n.contains("graphics")))
}

fn read_program(root: &Root, name: &str) -> Option<Program> {
    let k = open(root, name, false)?;
    let s = |v: &str| k.get_value::<String, _>(v).ok().map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    let n = |v: &str| k.get_value::<u32, _>(v).ok();
    let display = s("DisplayName")?;
    if n("SystemComponent") == Some(1) || s("ParentKeyName").is_some() {
        return None;
    }
    if matches!(s("ReleaseType").as_deref(), Some("Update" | "Hotfix" | "Security Update")) {
        return None;
    }
    let uninstall = s("UninstallString");
    let quiet = s("QuietUninstallString");
    let msi = if n("WindowsInstaller") == Some(1) && is_guid(name) {
        Some(name.to_string())
    } else {
        uninstall.as_deref().and_then(msi_guid_in)
    };
    if uninstall.is_none() && msi.is_none() {
        return None; // no se puede desinstalar desde aquí (componentes, drivers…)
    }
    let installed = s("InstallDate").filter(|d| d.len() == 8 && d.chars().all(|c| c.is_ascii_digit())).map(|d| format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]));
    let orphan = msi.is_none() && uninstall.as_deref().is_some_and(|u| !exe_exists(u));
    let silent_kind = if msi.is_some() {
        "MSI"
    } else if quiet.is_some() {
        "Propio"
    } else if orphan {
        ""
    } else {
        uninstall.as_deref().and_then(installer_kind).map_or("", |(k, _)| k)
    };
    let publisher = s("Publisher");
    Some(Program {
        id: format!("{}|{name}", root.tag),
        component: is_component(&display, publisher.as_deref()),
        publisher,
        version: s("DisplayVersion"),
        installed,
        size: n("EstimatedSize").map(|kb| kb as u64 * 1024).filter(|b| *b > 0),
        install_location: s("InstallLocation").map(|l| l.trim_matches('"').trim_end_matches('\\').to_string()),
        silent: !silent_kind.is_empty(),
        silent_kind: silent_kind.into(),
        repairable: msi.is_some(),
        orphan,
        per_user: root.per_user,
        uninstall,
        quiet,
        msi,
        name: display,
    })
}

pub fn list() -> Vec<Program> {
    let mut out: Vec<Program> = Vec::new();
    for root in roots() {
        let Ok(list) = root.key.open_subkey_with_flags(&root.path, KEY_READ | root.flags) else { continue };
        for name in list.enum_keys().filter_map(Result::ok) {
            if let Some(p) = read_program(&root, &name) {
                // El mismo programa puede aparecer en las vistas de 32 y 64 bits.
                if !out.iter().any(|x| x.name == p.name && x.version == p.version) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

fn find(id: &str) -> Result<(Root, String), String> {
    let (tag, key) = id.split_once('|').ok_or("Identificador no válido.")?;
    let root = roots().into_iter().find(|r| r.tag == tag).ok_or("Identificador no válido.")?;
    if key.is_empty() || key.contains('\\') {
        return Err("Identificador no válido.".into());
    }
    Ok((root, key.to_string()))
}

fn still_installed(id: &str) -> bool {
    find(id).is_ok_and(|(root, key)| open(&root, &key, false).is_some())
}

// ---------- Restos ----------

/// "7-Zip 25.01 (x64)" → "7-Zip"; "Mozilla Firefox (x64 es-ES)" → "Mozilla Firefox".
pub fn base_name(name: &str) -> String {
    let cut = name.find(" (").unwrap_or(name.len());
    let mut words: Vec<&str> = name[..cut].split_whitespace().collect();
    while words.len() > 1 && words.last().is_some_and(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()) || w.eq_ignore_ascii_case("x64") || w.eq_ignore_ascii_case("x86")) {
        words.pop();
    }
    words.join(" ")
}

fn safe_name(n: &str) -> bool {
    let n = n.trim().to_lowercase();
    n.len() >= 3 && !SHARED.contains(&n.as_str())
}

fn safe_leaf(p: &Path) -> bool {
    p.file_name().map(|n| n.to_string_lossy().to_string()).is_some_and(|n| safe_name(&n))
}

/// Nombres con los que el programa suele dejar carpetas y claves.
fn names_of(p: &Program) -> Vec<String> {
    let mut names = vec![p.name.clone(), base_name(&p.name)];
    if let Some(publisher) = &p.publisher {
        let publisher = publisher.trim_end_matches('.').trim();
        if !publisher.is_empty() && !publisher.contains(['\\', '/']) {
            names.push(format!(r"{publisher}\{}", base_name(&p.name)));
        }
    }
    names.retain(|n| safe_name(n.rsplit('\\').next().unwrap_or(n)) && !n.contains(['/', ':', '*', '?', '"', '<', '>', '|']));
    names.dedup();
    names
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    /// folder | shortcut | registry
    kind: String,
    path: String,
    size: u64,
    files: u64,
}

fn known_folders() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let env = |v: &str| std::env::var(v).ok().map(PathBuf::from);
    let mut data: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramData"].iter().filter_map(|v| env(v)).collect();
    // Menús Inicio y escritorios (común y del usuario).
    let mut shortcuts: Vec<PathBuf> = Vec::new();
    if let Some(pd) = env("ProgramData") {
        shortcuts.push(pd.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(public) = env("PUBLIC") {
        shortcuts.push(public.join("Desktop"));
    }
    if let Some(u) = crate::target_user::user_dirs() {
        data.push(u.app_data.clone());
        data.push(u.local_app_data.clone());
        data.push(u.local_app_data.join("Programs"));
        shortcuts.push(u.app_data.join(r"Microsoft\Windows\Start Menu\Programs"));
        shortcuts.push(u.profile.join("Desktop"));
    }
    (data, shortcuts)
}

/// Carpetas y accesos directos que existen y son solo de ese programa.
fn file_leftovers(p: &Program) -> Vec<Leftover> {
    let (data, shortcut_dirs) = known_folders();
    let names = names_of(p);
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(loc) = &p.install_location {
        let loc = PathBuf::from(expand(loc));
        // La ubicación declarada no debe ser una carpeta raíz ni una de las carpetas base.
        if loc.components().count() > 2 && !data.contains(&loc) {
            dirs.push(loc);
        }
    }
    for b in data.iter().chain(&shortcut_dirs) {
        for n in &names {
            dirs.push(b.join(n));
        }
    }
    let mut out: Vec<Leftover> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for d in dirs {
        let key = d.display().to_string().to_lowercase();
        if seen.contains(&key) || !d.is_dir() || !safe_leaf(&d) {
            continue;
        }
        seen.push(key);
        let (size, files) = crate::space::folder_size(&d);
        out.push(Leftover { kind: "folder".into(), path: d.display().to_string(), size, files });
    }
    // Accesos directos sueltos con el nombre del programa.
    let stems: Vec<String> = [p.name.clone(), base_name(&p.name)].iter().map(|n| n.to_lowercase()).collect();
    for dir in &shortcut_dirs {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = e.path();
            let is_lnk = path.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk") || x.eq_ignore_ascii_case("url"));
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if is_lnk && stems.contains(&stem) && safe_name(&stem) {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(Leftover { kind: "shortcut".into(), path: path.display().to_string(), size, files: 1 });
            }
        }
    }
    out
}

/// Claves del registro con el nombre del programa (HKLM, su vista de 32 bits y el usuario).
fn registry_leftovers(p: &Program) -> Vec<Leftover> {
    let user_hive: (&str, RegKey, String) = match crate::target_user::hkcu_redirect() {
        Some(sid) => ("HKU", RegKey::predef(HKEY_USERS), format!(r"{sid}\Software")),
        None => ("HKCU", RegKey::predef(HKEY_CURRENT_USER), "Software".into()),
    };
    let bases: Vec<(String, RegKey, String, u32)> = vec![
        ("HKLM".into(), RegKey::predef(HKEY_LOCAL_MACHINE), "SOFTWARE".into(), KEY_WOW64_64KEY),
        ("HKLM".into(), RegKey::predef(HKEY_LOCAL_MACHINE), r"SOFTWARE\WOW6432Node".into(), KEY_WOW64_64KEY),
        (user_hive.0.into(), user_hive.1, user_hive.2, 0),
    ];
    let mut out = Vec::new();
    for (hive, key, base, flags) in &bases {
        for n in names_of(p) {
            let sub = format!(r"{base}\{n}");
            if key.open_subkey_with_flags(&sub, KEY_READ | flags).is_ok() {
                let path = format!(r"{hive}\{sub}");
                if !out.iter().any(|l: &Leftover| l.path.eq_ignore_ascii_case(&path)) {
                    out.push(Leftover { kind: "registry".into(), path, size: 0, files: 0 });
                }
            }
        }
    }
    out
}

/// ¿Pertenece la carpeta a OTRO programa instalado? (plugins que declaran como
/// suya la carpeta del programa principal, suites que comparten carpeta…)
fn belongs_to_other(path: &Path, others: &[Program]) -> bool {
    let norm = |p: &Path| p.display().to_string().trim_end_matches('\\').to_lowercase();
    let cand = norm(path);
    let leaf = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    others.iter().filter(|o| !o.orphan).any(|o| {
        let by_location = o.install_location.as_deref().map(|l| norm(Path::new(&expand(l)))).is_some_and(|loc| {
            !loc.is_empty() && (loc == cand || loc.starts_with(&format!("{cand}\\")) || cand.starts_with(&format!("{loc}\\")))
        });
        // El desinstalador de otro programa está dentro.
        let by_uninstaller = o.uninstall.as_deref().map(|u| expand(&split_command(u).0).to_lowercase()).is_some_and(|exe| exe.starts_with(&format!("{cand}\\")));
        by_location || by_uninstaller || base_name(&o.name).to_lowercase() == leaf
    })
}

fn leftovers_of(p: &Program) -> Vec<Leftover> {
    let others: Vec<Program> = list().into_iter().filter(|o| o.id != p.id).collect();
    let mut v: Vec<Leftover> = file_leftovers(p).into_iter().filter(|l| l.kind != "folder" || !belongs_to_other(Path::new(&l.path), &others)).collect();
    v.extend(registry_leftovers(p).into_iter().filter(|l| {
        // Tampoco la clave con el nombre de otro programa instalado.
        let leaf = l.path.rsplit('\\').next().unwrap_or("").to_lowercase();
        !others.iter().any(|o| base_name(&o.name).to_lowercase() == leaf)
    }));
    v
}

/// Restos calculados para cada programa (lo único que se puede borrar).
static LEFTOVERS: LazyLock<Mutex<HashMap<String, Vec<Leftover>>>> = LazyLock::new(Default::default);

fn remember(id: &str, items: &[Leftover]) {
    LEFTOVERS.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), items.to_vec());
}

// ---------- Comandos ----------

#[tauri::command(async)]
pub fn list_programs() -> Vec<Program> {
    list()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResult {
    removed: bool,
    message: String,
    leftovers: Vec<Leftover>,
}

/// Programa, argumentos y si va en silencio.
fn command_for(p: &Program, silent: bool) -> Result<(String, String), String> {
    if let Some(guid) = &p.msi {
        return Ok(("msiexec.exe".into(), if silent { format!("/x {guid} /qn /norestart") } else { format!("/x {guid}") }));
    }
    if silent {
        if let Some(q) = &p.quiet {
            return Ok(split_command(q));
        }
        if let Some((_, extra)) = p.uninstall.as_deref().and_then(installer_kind) {
            let (exe, args) = split_command(p.uninstall.as_deref().unwrap_or_default());
            return Ok((exe, format!("{args} {extra}").trim().to_string()));
        }
    }
    p.uninstall.as_deref().map(split_command).ok_or_else(|| "No hay forma conocida de desinstalar este programa.".into())
}

#[tauri::command(async)]
pub fn uninstall_program(app: tauri::AppHandle, id: String, silent: bool, tweaks: State<'_, TweakState>) -> Result<UninstallResult, String> {
    let p = list().into_iter().find(|p| p.id == id).ok_or("El programa ya no está instalado. Actualiza la lista.")?;
    if !p.per_user && !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if p.orphan {
        return Err("Su desinstalador ya no existe: usa «Quitar de la lista».".into());
    }
    let silent = silent && p.silent;
    let (exe, args) = command_for(&p, silent)?;
    let task = Task::new(&app, format!("uninstall:{id}")).named(format!("Desinstalar {}", p.name));
    task.step(format!("Desinstalando {}{}…", p.name, if silent { " en silencio" } else { " (sigue el asistente del programa)" }));
    let script = format!(
        "{}{}$p = if ($a) {{ Start-Process -FilePath $exe -ArgumentList $a -PassThru }} else {{ Start-Process -FilePath $exe -PassThru }}\n\
         $p.WaitForExit()\n$p.ExitCode",
        crate::ps::text_var("exe", &expand(&exe)),
        crate::ps::text_var("a", &args)
    );
    let limit = Duration::from_secs(if silent { 20 * 60 } else { 60 * 60 });
    let code = crate::ps::powershell_opts(&script, task.opts(Some(limit))).map(|o| o.trim().parse::<i64>().unwrap_or(-1));
    let code = match code {
        Ok(c) => c,
        Err(e) => {
            tweaks.record(Op::Run, &format!("Desinstalar {}", p.name), &Err::<(), _>(e.clone()));
            return Err(e);
        }
    };
    // Muchos desinstaladores (NSIS, Inno) se copian a Temp y terminan al instante:
    // se espera a que la entrada desaparezca.
    let wait = Duration::from_secs(if silent { 180 } else { 15 * 60 });
    let start = Instant::now();
    while still_installed(&id) && start.elapsed() < wait && !task.cancelled() && matches!(code, 0 | 3010) {
        task.step(format!("Esperando a que termine el desinstalador de {}…", p.name));
        std::thread::sleep(Duration::from_millis(1500));
    }
    let removed = !still_installed(&id);
    let message = match (removed, code) {
        (true, 3010) => "Desinstalado. Reinicia el equipo para terminar.".to_string(),
        (true, _) => "Desinstalado.".to_string(),
        (false, 1602) => "Cancelado en el asistente del programa.".to_string(),
        (false, 1605) => "Windows dice que ya no estaba instalado.".to_string(),
        (false, 0) if !silent => "El asistente se cerró sin desinstalar (o sigue abierto).".to_string(),
        (false, c) => format!("El desinstalador terminó con código {c} y el programa sigue instalado."),
    };
    let result: Result<(), String> = if removed { Ok(()) } else { Err(message.clone()) };
    tweaks.record(Op::Run, format!("Desinstalar {} {}", p.name, p.version.clone().unwrap_or_default()).trim(), &result);
    if removed {
        crate::apps::invalidate_installed();
    }
    let leftovers = if removed {
        task.step("Buscando restos…");
        leftovers_of(&p)
    } else {
        vec![]
    };
    remember(&id, &leftovers);
    Ok(UninstallResult { removed, message, leftovers })
}

/// Restos de un programa sin desinstalarlo (p. ej. una entrada huérfana).
#[tauri::command(async)]
pub fn scan_leftovers(id: String) -> Result<Vec<Leftover>, String> {
    let p = list().into_iter().find(|p| p.id == id).ok_or("Esa entrada ya no existe.")?;
    if !p.orphan {
        return Err("El programa sigue instalado: desinstálalo primero.".into());
    }
    let v = leftovers_of(&p);
    remember(&id, &v);
    Ok(v)
}

fn delete_registry_key(path: &str, backup_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;
    let leaf: String = path.rsplit('\\').next().unwrap_or("clave").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let file = backup_dir.join(format!("{}-{leaf}.reg", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    crate::ps::exec("reg.exe", &["export", path, &file.display().to_string(), "/y", "/reg:64"])?;
    crate::ps::exec("reg.exe", &["delete", path, "/f", "/reg:64"]).map(|_| ())
}

/// Envía a la papelera los restos elegidos (solo los calculados para ese programa).
/// Las claves del registro se copian antes en un .reg.
#[tauri::command(async)]
pub fn remove_leftovers(app: tauri::AppHandle, id: String, paths: Vec<String>, tweaks: State<'_, TweakState>) -> Result<usize, String> {
    let allowed = LEFTOVERS.lock().unwrap_or_else(|e| e.into_inner()).get(&id).cloned().unwrap_or_default();
    let chosen: Vec<&Leftover> = allowed.iter().filter(|l| paths.contains(&l.path)).collect();
    if chosen.len() != paths.len() {
        return Err("Algún resto no corresponde a los detectados.".into());
    }
    let backup_dir = crate::paths::machine_data_dir(&app).join("registro-copias");
    let mut removed = 0;
    let mut errors = Vec::new();
    for l in chosen {
        let r = match l.kind.as_str() {
            "registry" => delete_registry_key(&l.path, &backup_dir),
            _ => {
                let method = if l.kind == "folder" { "DeleteDirectory" } else { "DeleteFile" };
                let script = format!(
                    "{}Add-Type -AssemblyName Microsoft.VisualBasic\n\
                     [Microsoft.VisualBasic.FileIO.FileSystem]::{method}($p, 'OnlyErrorDialogs', 'SendToRecycleBin')\n'ok'",
                    crate::ps::text_var("p", &l.path)
                );
                crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(300)), task: None }).map(|_| ())
            }
        };
        match r {
            Ok(()) => removed += 1,
            Err(e) => errors.push(format!("{}: {e}", l.path)),
        }
    }
    let result: Result<(), String> = if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) };
    tweaks.record(Op::Run, &format!("Restos de programa: {removed} eliminados (papelera y copia .reg)"), &result);
    result.map(|_| removed)
}

/// Repara un programa MSI (vuelve a copiar sus archivos y claves).
#[tauri::command(async)]
pub fn repair_program(app: tauri::AppHandle, id: String, tweaks: State<'_, TweakState>) -> Result<String, String> {
    let p = list().into_iter().find(|p| p.id == id).ok_or("El programa ya no está instalado.")?;
    let guid = p.msi.clone().ok_or("Solo se pueden reparar desde aquí los programas MSI.")?;
    if !p.per_user && !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let task = Task::new(&app, format!("repair:{id}")).named(format!("Reparar {}", p.name));
    task.step(format!("Reparando {}…", p.name));
    let script = format!(
        "{}$p = Start-Process -FilePath msiexec.exe -ArgumentList \"/fa $g /qb! /norestart\" -PassThru\n$p.WaitForExit()\n$p.ExitCode",
        crate::ps::text_var("g", &guid)
    );
    let r = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(30 * 60)))).and_then(|o| match o.trim().parse::<i64>().unwrap_or(-1) {
        0 => Ok("Reparado.".to_string()),
        3010 => Ok("Reparado. Reinicia el equipo para terminar.".to_string()),
        1605 => Err("Windows dice que no está instalado.".to_string()),
        c => Err(format!("La reparación terminó con código {c}.")),
    });
    tweaks.record(Op::Run, &format!("Reparar {}", p.name), &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r
}

/// Quita de la lista una entrada cuyo desinstalador ya no existe (copia la clave antes en un .reg).
#[tauri::command(async)]
pub fn remove_orphan_entry(app: tauri::AppHandle, id: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    let p = list().into_iter().find(|p| p.id == id).ok_or("Esa entrada ya no existe.")?;
    if !p.orphan {
        return Err("Ese programa tiene desinstalador: desinstálalo normalmente.".into());
    }
    let (root, key) = find(&id)?;
    if !root.per_user && !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let hive = match root.tag {
        "HKLM64" | "HKLM32" => "HKLM",
        "HKU" => "HKU",
        _ => "HKCU",
    };
    let full = format!(r"{hive}\{}\{key}", root.path);
    let dir = crate::paths::machine_data_dir(&app).join("registro-copias");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("{}-{}.reg", chrono::Local::now().format("%Y%m%d-%H%M%S"), key.trim_matches(['{', '}'])));
    let view = if root.tag == "HKLM32" { "/reg:32" } else { "/reg:64" };
    crate::ps::exec("reg.exe", &["export", &full, &file.display().to_string(), "/y", view])?;
    let result = root
        .key
        .open_subkey_with_flags(&root.path, KEY_READ | KEY_WRITE | root.flags)
        .and_then(|parent| parent.delete_subkey_all(&key))
        .map_err(|e| format!("No se pudo quitar: {e}"));
    tweaks.record(Op::Run, &format!("Quitar entrada huérfana: {} (copia en {})", p.name, file.display()), &result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_commands() {
        assert_eq!(split_command(r#""C:\Program Files\X\unins000.exe" /SILENT"#), (r"C:\Program Files\X\unins000.exe".into(), "/SILENT".into()));
        assert_eq!(split_command(r"C:\Program Files\X\uninst.exe /S"), (r"C:\Program Files\X\uninst.exe".into(), "/S".into()));
        assert_eq!(split_command("MsiExec.exe /X{1234}"), ("MsiExec.exe".into(), "/X{1234}".into()));
        assert_eq!(split_command(r"C:\x\setup.exe"), (r"C:\x\setup.exe".into(), String::new()));
    }

    #[test]
    fn base_names_and_guards() {
        assert_eq!(base_name("7-Zip 25.01 (x64)"), "7-Zip");
        assert_eq!(base_name("Mozilla Firefox (x64 es-ES)"), "Mozilla Firefox");
        assert_eq!(base_name("VLC media player"), "VLC media player");
        assert_eq!(base_name("Notepad++ 8.6 x64"), "Notepad++");
        assert!(!safe_leaf(Path::new(r"C:\ProgramData\Microsoft")));
        assert!(!safe_leaf(Path::new(r"C:\Program Files\Common Files")));
        assert!(safe_leaf(Path::new(r"C:\Program Files\VideoLAN")));
        assert!(is_guid("{23170F69-40C1-2702-2501-000001000000}"));
        assert!(!is_guid("7-Zip"));
    }

    #[test]
    fn detects_msi_in_uninstall_string_and_inno() {
        assert_eq!(msi_guid_in("MsiExec.exe /I{23170F69-40C1-2702-2501-000001000000}").as_deref(), Some("{23170F69-40C1-2702-2501-000001000000}"));
        assert!(msi_guid_in(r"C:\x\uninst.exe").is_none());
        // Inno Setup por el nombre (no hace falta que exista para reconocerlo).
        assert_eq!(installer_kind(r#""C:\Program Files\X\unins000.exe""#).map(|k| k.0), Some("Inno Setup"));
        assert!(installer_kind(r"C:\x\uninstaller-tool.exe").is_none());
    }

    #[test]
    fn components_and_names() {
        assert!(is_component("Microsoft Visual C++ 2015-2022 Redistributable (x64)", Some("Microsoft Corporation")));
        assert!(is_component("Microsoft .NET Framework 4.8 SDK", None));
        assert!(!is_component("VLC media player", Some("VideoLAN")));
        let p = Program {
            id: String::new(),
            name: "VLC media player".into(),
            publisher: Some("VideoLAN".into()),
            version: None,
            installed: None,
            size: None,
            install_location: None,
            silent: false,
            silent_kind: String::new(),
            repairable: false,
            component: false,
            orphan: false,
            per_user: false,
            uninstall: None,
            quiet: None,
            msi: None,
        };
        assert_eq!(names_of(&p), vec!["VLC media player".to_string(), r"VideoLAN\VLC media player".to_string()]);
        // Nunca una carpeta compartida.
        let shared = Program { name: "Microsoft".into(), publisher: None, ..p };
        assert!(names_of(&shared).is_empty());
    }

    /// Restos de las entradas huérfanas reales (solo lectura):
    /// `cargo test leftovers_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn leftovers_real() {
        for p in list().into_iter().filter(|p| p.orphan) {
            println!("== {}", p.name);
            for l in leftovers_of(&p) {
                println!("  [{}] {} ({} bytes)", l.kind, l.path, l.size);
            }
        }
    }

    #[test]
    fn never_another_programs_folder() {
        let prog = |name: &str, loc: Option<&str>, uninstall: Option<&str>| Program {
            id: name.into(),
            name: name.into(),
            publisher: None,
            version: None,
            installed: None,
            size: None,
            install_location: loc.map(String::from),
            silent: false,
            silent_kind: String::new(),
            repairable: false,
            component: false,
            orphan: false,
            per_user: false,
            uninstall: uninstall.map(String::from),
            quiet: None,
            msi: None,
        };
        let others = vec![prog("Audacity 3.7.1", Some(r"C:\Program Files\Audacity"), Some(r"C:\Program Files\Audacity\unins000.exe"))];
        assert!(belongs_to_other(Path::new(r"C:\Program Files\Audacity"), &others));
        assert!(belongs_to_other(Path::new(r"C:\Program Files\Audacity\Plug-Ins"), &others));
        assert!(belongs_to_other(Path::new(r"C:\Program Files"), &others));
        assert!(belongs_to_other(Path::new(r"C:\Users\x\AppData\Roaming\audacity"), &others));
        assert!(!belongs_to_other(Path::new(r"C:\Program Files\OpenVINO"), &others));
    }

    /// Solo lectura: lista real (dos veces: la segunda, con la caché).
    /// Equipo real (depende de su estado): `cargo test lists_real_programs -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn lists_real_programs() {
        let _ = list();
        let t = Instant::now();
        let v = list();
        println!(
            "{} programas en {} ms; silenciosos {}; por tipo {:?}; huérfanos: {:?}",
            v.len(),
            t.elapsed().as_millis(),
            v.iter().filter(|p| p.silent).count(),
            ["MSI", "Propio", "Inno Setup", "NSIS"].map(|k| (k, v.iter().filter(|p| p.silent_kind == k).count())),
            v.iter().filter(|p| p.orphan).map(|p| &p.name).collect::<Vec<_>>()
        );
        assert!(v.len() > 5);
    }
}
