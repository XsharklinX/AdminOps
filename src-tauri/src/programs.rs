//! Desinstalador de programas (Win32): lista del registro, desinstalación
//! silenciosa o con el asistente del programa, y limpieza de restos.
//!
//! Los restos se buscan por nombre exacto en las carpetas habituales y van a la
//! papelera (se pueden recuperar). Solo se borran rutas que calculó el propio
//! backend para ese programa, nunca rutas enviadas por la interfaz.

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
/// Carpetas compartidas por muchos programas: nunca son "restos" de uno solo.
const SHARED: &[&str] = &[
    "microsoft", "windows", "common files", "intel", "amd", "nvidia", "nvidia corporation", "google", "mozilla", "packages", "temp",
    "programs", "microsoft shared", "windowsapps", "package cache", "installer", "system32",
];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    /// hive|vista|clave: identifica la entrada del registro.
    id: String,
    name: String,
    publisher: Option<String>,
    version: Option<String>,
    /// AAAA-MM-DD
    installed: Option<String>,
    /// Bytes (estimación del propio instalador).
    size: Option<u64>,
    install_location: Option<String>,
    /// Se puede desinstalar sin ventanas (MSI o desinstalador silencioso).
    silent: bool,
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

fn exe_exists(cmd: &str) -> bool {
    let (exe, _) = split_command(cmd);
    let exe = std::env::var("SystemRoot").map(|w| exe.replace("%SystemRoot%", &w).replace("%windir%", &w)).unwrap_or(exe);
    let p = Path::new(&exe);
    // Sin ruta (msiexec, rundll32…) = está en el PATH del sistema.
    !p.is_absolute() || p.exists()
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
    let msi = (n("WindowsInstaller") == Some(1) && is_guid(name)).then(|| name.to_string());
    if uninstall.is_none() && msi.is_none() {
        return None; // no se puede desinstalar desde aquí (componentes, drivers…)
    }
    let installed = s("InstallDate").filter(|d| d.len() == 8 && d.chars().all(|c| c.is_ascii_digit())).map(|d| format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]));
    let orphan = msi.is_none() && uninstall.as_deref().is_some_and(|u| !exe_exists(u));
    Some(Program {
        id: format!("{}|{name}", root.tag),
        publisher: s("Publisher"),
        version: s("DisplayVersion"),
        installed,
        size: n("EstimatedSize").map(|kb| kb as u64 * 1024).filter(|b| *b > 0),
        install_location: s("InstallLocation").map(|l| l.trim_matches('"').trim_end_matches('\\').to_string()),
        silent: msi.is_some() || quiet.is_some(),
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

fn safe_leaf(p: &Path) -> bool {
    p.file_name().map(|n| n.to_string_lossy().to_lowercase()).is_some_and(|n| !SHARED.contains(&n.as_str()) && n.len() >= 3)
}

/// Carpetas que suelen quedar tras desinstalar, que existen y son solo de ese programa.
fn leftover_candidates(p: &Program) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramData"].iter().filter_map(|v| std::env::var(v).ok()).map(PathBuf::from).collect();
    if let Some(u) = crate::target_user::user_dirs() {
        bases.push(u.app_data);
        bases.push(u.local_app_data);
    }
    let mut names = vec![p.name.clone(), base_name(&p.name)];
    if let Some(publisher) = &p.publisher {
        names.push(format!(r"{publisher}\{}", base_name(&p.name)));
    }
    names.retain(|n| !n.trim().is_empty());
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(loc) = &p.install_location {
        let loc = PathBuf::from(loc);
        // La ubicación declarada no debe ser una carpeta raíz ni una de las carpetas base.
        if loc.components().count() > 2 && !bases.contains(&loc) {
            out.push(loc);
        }
    }
    for b in &bases {
        for n in &names {
            out.push(b.join(n));
        }
    }
    let mut seen = Vec::new();
    out.into_iter()
        .filter(|d| d.is_dir() && safe_leaf(d))
        .filter(|d| {
            let k = d.display().to_string().to_lowercase();
            let new = !seen.contains(&k);
            seen.push(k);
            new
        })
        .collect()
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    path: String,
    size: u64,
    files: u64,
}

/// Restos calculados en la última desinstalación (lo único que se puede borrar).
static LEFTOVERS: LazyLock<Mutex<HashMap<String, Vec<String>>>> = LazyLock::new(Default::default);

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

#[tauri::command(async)]
pub fn uninstall_program(app: tauri::AppHandle, id: String, silent: bool, tweaks: State<'_, TweakState>) -> Result<UninstallResult, String> {
    let p = list().into_iter().find(|p| p.id == id).ok_or("El programa ya no está instalado. Actualiza la lista.")?;
    if !p.per_user && !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if p.orphan {
        return Err("Su desinstalador ya no existe: usa «Quitar de la lista».".into());
    }
    let (exe, args) = match (&p.msi, silent, &p.quiet, &p.uninstall) {
        (Some(guid), true, _, _) => ("msiexec.exe".to_string(), format!("/x {guid} /qn /norestart")),
        (Some(guid), false, _, _) => ("msiexec.exe".to_string(), format!("/x {guid}")),
        (None, true, Some(q), _) => split_command(q),
        (None, _, _, Some(u)) => split_command(u),
        _ => return Err("No hay forma conocida de desinstalar este programa.".into()),
    };
    let task = Task::new(&app, format!("uninstall:{id}")).named(format!("Desinstalar {}", p.name));
    task.step(format!("Desinstalando {}{}…", p.name, if silent { " en silencio" } else { " (sigue el asistente del programa)" }));
    let script = format!(
        "{}{}$p = if ($a) {{ Start-Process -FilePath $exe -ArgumentList $a -PassThru }} else {{ Start-Process -FilePath $exe -PassThru }}\n\
         $p.WaitForExit()\n$p.ExitCode",
        crate::ps::text_var("exe", &exe),
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
    // Muchos desinstaladores se copian a Temp y terminan al instante: esperar a que la entrada desaparezca.
    let wait = Duration::from_secs(if silent { 180 } else { 15 * 60 });
    let start = Instant::now();
    while still_installed(&id) && start.elapsed() < wait && !task.cancelled() && matches!(code, 0 | 3010) {
        task.step(format!("Esperando a que termine el desinstalador de {}…", p.name));
        std::thread::sleep(Duration::from_secs(2));
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
    let leftovers: Vec<Leftover> = if removed {
        leftover_candidates(&p)
            .into_iter()
            .map(|d| {
                let (size, files) = crate::space::folder_size(&d);
                Leftover { path: d.display().to_string(), size, files }
            })
            .collect()
    } else {
        vec![]
    };
    LEFTOVERS.lock().unwrap_or_else(|e| e.into_inner()).insert(id, leftovers.iter().map(|l| l.path.clone()).collect());
    Ok(UninstallResult { removed, message, leftovers })
}

/// Envía a la papelera los restos elegidos (solo los calculados para ese programa).
#[tauri::command(async)]
pub fn remove_leftovers(id: String, paths: Vec<String>, tweaks: State<'_, TweakState>) -> Result<usize, String> {
    let allowed = LEFTOVERS.lock().unwrap_or_else(|e| e.into_inner()).get(&id).cloned().unwrap_or_default();
    let chosen: Vec<&String> = paths.iter().filter(|p| allowed.contains(p)).collect();
    if chosen.len() != paths.len() {
        return Err("Alguna carpeta no corresponde a los restos detectados.".into());
    }
    let mut removed = 0;
    let mut errors = Vec::new();
    for p in chosen {
        let script = format!(
            "{}Add-Type -AssemblyName Microsoft.VisualBasic\n\
             [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($p, 'OnlyErrorDialogs', 'SendToRecycleBin')\n'ok'",
            crate::ps::text_var("p", p)
        );
        match crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(300)), task: None }) {
            Ok(_) => removed += 1,
            Err(e) => errors.push(format!("{p}: {e}")),
        }
    }
    let result: Result<(), String> = if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) };
    tweaks.record(Op::Run, &format!("Restos de programa a la papelera ({removed} carpetas)"), &result);
    result.map(|_| removed)
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

    /// Solo lectura: lista real.
    #[test]
    fn lists_real_programs() {
        let v = list();
        println!("{} programas; huérfanos: {:?}", v.len(), v.iter().filter(|p| p.orphan).map(|p| &p.name).collect::<Vec<_>>());
        assert!(v.len() > 5);
    }
}
