//! OneDrive y Teams que no sincronizan. Lo que suele romper OneDrive: rutas
//! demasiado largas, nombres que SharePoint no admite, archivos abiertos, copias
//! en conflicto y las carpetas protegidas (Escritorio, Documentos) a medias. De
//! Teams, la caché dañada y la versión clásica conviviendo con la nueva.

use crate::troubleshoot::{finding, fix, fix_confirm, Finding};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// SharePoint y OneDrive no admiten rutas de más de 400 caracteres.
pub const MAX_PATH: usize = 400;
/// No se recorre más que esto (una carpeta de OneDrive puede tener millones de archivos).
const MAX_FILES: usize = 200_000;
const MAX_TIME: Duration = Duration::from_secs(25);

/// Por qué OneDrive no subirá un archivo con ese nombre, si no lo hará.
pub fn bad_name(name: &str) -> Option<&'static str> {
    if name.starts_with("~$") || name.eq_ignore_ascii_case("desktop.ini") {
        return None; // temporales de Office y archivos de Windows: OneDrive los ignora
    }
    if name.chars().any(|c| "\"*:<>?/\\|".contains(c)) {
        return Some("lleva un carácter no permitido");
    }
    if name != name.trim_start() || name.ends_with(' ') {
        return Some("empieza o termina con un espacio");
    }
    if name.ends_with('.') {
        return Some("termina en punto");
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str()) || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        return Some("es un nombre reservado de Windows");
    }
    if name.to_ascii_lowercase().contains("_vti_") {
        return Some("contiene «_vti_», que SharePoint reserva");
    }
    if name.eq_ignore_ascii_case(".lock") || name.eq_ignore_ascii_case("forms") {
        return Some("es un nombre que SharePoint reserva");
    }
    None
}

/// Un nombre que OneDrive sí admite, lo más parecido al original.
pub fn safe_name(name: &str) -> String {
    let mut s: String = name.chars().map(|c| if "\"*:<>?/\\|".contains(c) { '_' } else { c }).collect();
    s = s.trim().trim_end_matches('.').trim().to_string();
    s = s.replace("_vti_", "_vti-");
    if s.is_empty() {
        s = "archivo".into();
    }
    if bad_name(&s).is_some() {
        // Nombres reservados: se les añade algo delante de la extensión.
        match s.split_once('.') {
            Some((a, b)) => s = format!("{a}_.{b}"),
            None => s.push('_'),
        }
    }
    s
}

/// ¿Es una copia en conflicto que OneDrive crea con el nombre del equipo («Informe-PC-01.docx»)?
pub fn conflict_copy(name: &str, computer: &str) -> bool {
    if computer.is_empty() {
        return false;
    }
    let stem = name.rsplit_once('.').map(|(a, _)| a).unwrap_or(name);
    stem.to_ascii_lowercase().ends_with(&format!("-{}", computer.to_ascii_lowercase()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub path: String,
    pub why: String,
    /// Se puede renombrar solo (si no, hay que acortar a mano).
    pub renamable: bool,
}

#[derive(Debug, Default)]
pub struct Scan {
    pub roots: Vec<String>,
    pub files: usize,
    pub problems: Vec<Problem>,
    /// Archivos abiertos en Office ahora mismo.
    pub open: Vec<String>,
    pub conflicts: Vec<String>,
    pub truncated: bool,
}

/// Carpetas de OneDrive del usuario (personal y de empresa).
pub fn roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let base = r"HKCU\Software\Microsoft\OneDrive\Accounts";
    for account in ["Personal", "Business1", "Business2", "Business3"] {
        if let Some(p) = crate::tweaks::registry::read_string(&format!(r"{base}\{account}"), "UserFolder") {
            let p = PathBuf::from(p);
            if p.is_dir() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Recorre las carpetas de OneDrive buscando lo que no se subirá.
pub fn scan(roots: &[PathBuf], computer: &str) -> Scan {
    let started = Instant::now();
    let mut s = Scan { roots: roots.iter().map(|r| r.display().to_string()).collect(), ..Default::default() };
    let mut stack: Vec<PathBuf> = roots.to_vec();
    while let Some(dir) = stack.pop() {
        if s.files >= MAX_FILES || started.elapsed() > MAX_TIME {
            s.truncated = true;
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let full = path.display().to_string();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                stack.push(path.clone());
            } else {
                s.files += 1;
            }
            if let Some(owner) = name.strip_prefix("~$") {
                // «~$forme.docx» al lado de «Informe.docx»: está abierto en Office.
                if let Some(real) = std::fs::read_dir(&dir).ok().and_then(|rd| rd.flatten().map(|x| x.file_name().to_string_lossy().into_owned()).find(|n| n.len() > 2 && n.ends_with(owner) && !n.starts_with("~$"))) {
                    s.open.push(dir.join(real).display().to_string());
                }
                continue;
            }
            if let Some(why) = bad_name(&name) {
                s.problems.push(Problem { path: full.clone(), why: why.into(), renamable: true });
            } else if full.chars().count() > MAX_PATH {
                s.problems.push(Problem { path: full.clone(), why: format!("ruta de {} caracteres (máximo {MAX_PATH})", full.chars().count()), renamable: false });
            }
            if !is_dir && conflict_copy(&name, computer) {
                s.conflicts.push(full);
            }
        }
    }
    s
}

fn short(path: &str, roots: &[String]) -> String {
    for r in roots {
        if let Some(rest) = path.strip_prefix(r.as_str()) {
            let name = Path::new(r).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return format!("{name}{rest}");
        }
    }
    path.to_string()
}

/// Hallazgos a partir del recorrido y del estado de OneDrive y Teams.
pub fn findings(s: &Scan, running: bool, kfm_mixed: Option<String>, teams: &TeamsState) -> Vec<Finding> {
    let mut out = Vec::new();
    if s.roots.is_empty() {
        out.push(finding("info", "OneDrive no está configurado en este usuario", "No hay ninguna carpeta de OneDrive. Si se usa, inicia sesión desde el icono de la nube junto al reloj."));
    } else {
        if !running {
            out.push(finding("warn", "OneDrive no está abierto", "Sin OneDrive en marcha no se sincroniza nada. Ábrelo desde el menú Inicio o reinícialo.").fixes(vec![fix("od.start", "Abrir OneDrive", false)]));
        }
        if !s.problems.is_empty() {
            let mut detail = format!("{} archivo(s) o carpeta(s) que OneDrive no subirá:", s.problems.len());
            for p in s.problems.iter().take(6) {
                detail.push_str(&format!("\n· {} — {}", short(&p.path, &s.roots), p.why));
            }
            if s.problems.len() > 6 {
                detail.push_str(&format!("\n· y {} más", s.problems.len() - 6));
            }
            let mut fixes: Vec<_> = s.problems.iter().filter(|p| p.renamable).take(3).map(|p| fix(format!("od.rename:{}", p.path), &format!("Renombrar «{}»", Path::new(&p.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()), false)).collect();
            if let Some(p) = s.problems.iter().find(|p| !p.renamable) {
                fixes.push(fix(format!("reveal:{}", p.path), "Abrir la carpeta de la ruta larga", false));
            }
            out.push(finding("bad", format!("{} elemento(s) no se sincronizan", s.problems.len()), detail).fixes(fixes));
        }
        if !s.open.is_empty() {
            let names: Vec<String> = s.open.iter().take(4).map(|p| short(p, &s.roots)).collect();
            out.push(finding("info", format!("{} archivo(s) abiertos en Office", s.open.len()), format!("Se suben al cerrarlos: {}.", names.join(", "))));
        }
        if !s.conflicts.is_empty() {
            let names: Vec<String> = s.conflicts.iter().take(4).map(|p| short(p, &s.roots)).collect();
            out.push(finding("warn", format!("{} copia(s) en conflicto", s.conflicts.len()), format!("OneDrive guardó dos versiones del mismo archivo (la copia lleva el nombre del equipo): {}. Compáralas y quédate con una.", names.join(", "))).fixes(vec![fix(format!("reveal:{}", s.conflicts[0]), "Abrir la carpeta", false)]));
        }
        if let Some(m) = kfm_mixed {
            out.push(finding("warn", "Las carpetas protegidas están a medias", m));
        }
        if s.truncated {
            out.push(finding("info", "Revisión parcial", format!("Hay muchísimos archivos: se revisaron {} y se paró para no tardar. Lo encontrado es lo que hay en esos.", s.files)));
        }
        out.push(finding("info", "Si sigue sin sincronizar", "Restablecer OneDrive no borra archivos: vuelve a comprobar todo desde cero (tarda un rato con muchas carpetas).").fixes(vec![fix_confirm("od.reset", "Restablecer OneDrive", false, "OneDrive se cerrará, se restablecerá y volverá a abrirse. Los archivos no se borran; la primera sincronización tardará más.")]));
    }
    if teams.classic && teams.new_app {
        out.push(finding("warn", "Conviven Teams clásico y el nuevo", "El clásico ya no tiene soporte y a veces abre el que no toca o duplica avisos. Desinstala «Microsoft Teams (clásico)» desde Programas.").page("uninstall"));
    }
    if teams.classic || teams.new_app {
        out.push(
            finding("info", "Teams: caché", format!("Caché de Teams: {}. Si Teams no carga, se queda en blanco o no muestra mensajes nuevos, vaciarla lo arregla (se cierra Teams; no se pierden chats).", super::gb(teams.cache_bytes)))
                .fixes(vec![fix_confirm("teams.cache", "Vaciar la caché de Teams", false, "Teams se cerrará y tardará un poco más la próxima vez que se abra. No se pierden chats ni archivos.")]),
        );
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(0, finding("ok", "OneDrive y Teams no muestran problemas", format!("{} archivo(s) revisados en {} carpeta(s) de OneDrive.", s.files, s.roots.len())));
    }
    out
}

#[derive(Debug, Default)]
pub struct TeamsState {
    pub classic: bool,
    pub new_app: bool,
    pub cache_bytes: u64,
}

fn dir_size(p: &Path, budget: &mut usize) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    let mut total = 0;
    for e in rd.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        match e.file_type() {
            Ok(t) if t.is_dir() => total += dir_size(&e.path(), budget),
            Ok(_) => total += e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => {}
        }
    }
    total
}

/// Carpetas de caché de Teams (clásico y nuevo) del usuario.
fn teams_cache_dirs() -> Vec<PathBuf> {
    let Some(d) = crate::target_user::user_dirs() else { return vec![] };
    let classic = d.app_data.join(r"Microsoft\Teams");
    let mut out: Vec<PathBuf> = ["Cache", "blob_storage", "databases", "GPUCache", "IndexedDB", "Local Storage", "tmp", "Code Cache"].iter().map(|s| classic.join(s)).collect();
    out.push(d.local_app_data.join(r"Packages\MSTeams_8wekyb3d8bbwe\LocalCache\Microsoft\MSTeams"));
    out
}

fn teams_state() -> TeamsState {
    let Some(d) = crate::target_user::user_dirs() else { return TeamsState::default() };
    let mut budget = 50_000;
    TeamsState {
        classic: d.app_data.join(r"Microsoft\Teams").is_dir(),
        new_app: d.local_app_data.join(r"Packages\MSTeams_8wekyb3d8bbwe").is_dir(),
        cache_bytes: teams_cache_dirs().iter().map(|p| dir_size(p, &mut budget)).sum(),
    }
}

/// Escritorio y Documentos: si uno está en OneDrive y el otro no, la protección quedó a medias.
fn kfm_mixed(roots: &[PathBuf]) -> Option<String> {
    if roots.is_empty() {
        return None;
    }
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
    let names = [("Desktop", "Escritorio"), ("Personal", "Documentos"), ("My Pictures", "Imágenes")];
    let inside: Vec<(&str, bool)> = names
        .iter()
        .filter_map(|(v, label)| {
            let p = crate::tweaks::registry::read_string(key, v)?;
            Some((*label, roots.iter().any(|r| p.to_ascii_lowercase().starts_with(&r.display().to_string().to_ascii_lowercase()))))
        })
        .collect();
    let yes: Vec<&str> = inside.iter().filter(|(_, i)| *i).map(|(l, _)| *l).collect();
    let no: Vec<&str> = inside.iter().filter(|(_, i)| !*i).map(|(l, _)| *l).collect();
    (!yes.is_empty() && !no.is_empty()).then(|| format!("{} se guarda(n) en OneDrive y {} no. Lo que no está en OneDrive no tiene copia en la nube. Termina la protección desde OneDrive → Configuración → Sincronización y copia de seguridad.", yes.join(" y "), no.join(" y ")))
}

fn onedrive_running() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes().values().any(|p| p.name().to_string_lossy().eq_ignore_ascii_case("OneDrive.exe"))
}

pub fn check() -> Result<Vec<Finding>, String> {
    let roots = roots();
    let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
    let s = scan(&roots, &computer);
    Ok(findings(&s, onedrive_running(), kfm_mixed(&roots), &teams_state()))
}

fn onedrive_exe() -> Option<PathBuf> {
    let d = crate::target_user::user_dirs()?;
    [d.local_app_data.join(r"Microsoft\OneDrive\OneDrive.exe"), PathBuf::from(r"C:\Program Files\Microsoft OneDrive\OneDrive.exe"), PathBuf::from(r"C:\Program Files (x86)\Microsoft OneDrive\OneDrive.exe")].into_iter().find(|p| p.is_file())
}

fn rename(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    let inside = roots().iter().any(|r| p.starts_with(r));
    if !inside || !p.exists() {
        return Err("Solo se renombra lo que está dentro de OneDrive.".into());
    }
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("Ruta sin nombre.")?;
    let new = safe_name(&name);
    let target = p.with_file_name(&new);
    if target.exists() {
        return Err(format!("Ya existe «{new}» en esa carpeta: renómbralo a mano."));
    }
    std::fs::rename(p, &target).map_err(|e| format!("No se pudo renombrar: {e}"))?;
    Ok(format!("«{name}» ahora se llama «{new}»."))
}

fn kill(names: &[&str]) {
    for n in names {
        let _ = crate::ps::exec("taskkill.exe", &["/IM", n, "/F"]);
    }
}

pub fn run(kind: &str, arg: &str) -> Option<Result<String, String>> {
    Some(match kind {
        "od.rename" => rename(arg),
        "od.start" => onedrive_exe().ok_or_else(|| "No se encuentra OneDrive.".to_string()).and_then(|exe| crate::shellopen::run_as_user(&exe.display().to_string(), "/background")).map(|()| "OneDrive abierto.".into()),
        "od.reset" => (|| {
            let exe = onedrive_exe().ok_or("No se encuentra OneDrive.")?.display().to_string();
            crate::shellopen::run_as_user(&exe, "/reset")?;
            std::thread::sleep(Duration::from_secs(8));
            crate::shellopen::run_as_user(&exe, "")?;
            Ok("OneDrive restablecido: vuelve a comprobar los archivos (puede tardar).".into())
        })(),
        "teams.cache" => {
            kill(&["ms-teams.exe", "Teams.exe"]);
            std::thread::sleep(Duration::from_millis(1500));
            let mut freed = 0u64;
            for d in teams_cache_dirs() {
                let mut budget = 200_000;
                freed += dir_size(&d, &mut budget);
                let _ = std::fs::remove_dir_all(&d);
            }
            Ok(format!("Caché de Teams vaciada ({}). Ábrelo de nuevo.", super::gb(freed)))
        }
        _ => return None,
    })
}

pub fn title(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "od.rename" => "OneDrive: renombrar un archivo que no se subía",
        "od.reset" => "Restablecer OneDrive",
        "teams.cache" => "Vaciar la caché de Teams",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_que_onedrive_no_admite() {
        assert!(bad_name("Presupuesto*.xlsx").is_some());
        assert!(bad_name(" informe.docx").is_some());
        assert!(bad_name("informe.docx ").is_some());
        assert!(bad_name("notas.").is_some());
        assert!(bad_name("CON.txt").is_some());
        assert!(bad_name("com1").is_some());
        assert!(bad_name("_vti_cnf").is_some());
        assert!(bad_name("Informe final (copia) [2].docx").is_none());
        assert!(bad_name("~$forme.docx").is_none());
        assert!(bad_name("desktop.ini").is_none());
        assert!(bad_name("Comida.txt").is_none());
    }

    #[test]
    fn nombre_seguro() {
        assert_eq!(safe_name("Presupuesto*.xlsx"), "Presupuesto_.xlsx");
        assert_eq!(safe_name(" informe.docx  "), "informe.docx");
        assert_eq!(safe_name("notas..."), "notas");
        assert_eq!(safe_name("CON.txt"), "CON_.txt");
        assert!(bad_name(&safe_name("a<b>:c")).is_none());
    }

    #[test]
    fn copias_en_conflicto() {
        assert!(conflict_copy("Informe-PC-CONTA-01.docx", "PC-CONTA-01"));
        assert!(!conflict_copy("Informe.docx", "PC-CONTA-01"));
        assert!(!conflict_copy("Informe-PC.docx", ""));
    }

    #[test]
    fn recorre_y_encuentra_lo_que_no_sube() {
        let dir = std::env::temp_dir().join(format!("adminops-od-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("bien.txt"), "x").unwrap();
        // Windows no deja crear «CON.txt»: se prueba con otro nombre que SharePoint rechaza.
        std::fs::write(dir.join("sub").join("_vti_notas.txt"), "x").unwrap();
        std::fs::write(dir.join("Informe.docx"), "x").unwrap();
        std::fs::write(dir.join("~$forme.docx"), "x").unwrap();
        std::fs::write(dir.join("Plan-PC-01.xlsx"), "x").unwrap();
        let s = scan(std::slice::from_ref(&dir), "PC-01");
        assert_eq!(s.problems.len(), 1, "{:?}", s.problems);
        assert!(s.problems[0].path.ends_with("_vti_notas.txt"));
        assert_eq!(s.open.len(), 1);
        assert_eq!(s.conflicts.len(), 1);
        let f = findings(&s, true, None, &TeamsState::default());
        assert!(f.iter().any(|x| x.level == "bad" && x.fixes.iter().any(|y| y.id.starts_with("od.rename:"))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
