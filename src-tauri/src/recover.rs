//! Recuperar archivos borrados con Windows File Recovery (winfr), la
//! herramienta gratuita de Microsoft (Microsoft Store). AdminOps la instala,
//! arma el comando según lo que se busca y abre la carpeta con lo recuperado.

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::State;

/// Identificador de Windows File Recovery en la Microsoft Store.
const STORE_ID: &str = "9N26S50LN705";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub letter: String,
    pub label: String,
    pub fs: String,
    pub size: u64,
    pub free: u64,
    /// Fixed | Removable
    pub kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverStatus {
    pub installed: bool,
    pub drives: Vec<Drive>,
}

const DRIVES_SCRIPT: &str = r#"
$r = @(Get-Volume -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter -and $_.DriveType -in 'Fixed', 'Removable' -and $_.Size -gt 0 } |
  Sort-Object DriveLetter | ForEach-Object {
    [pscustomobject]@{ letter = "$($_.DriveLetter)"; label = "$($_.FileSystemLabel)"; fs = "$($_.FileSystemType)"; size = [uint64]$_.Size; free = [uint64]$_.SizeRemaining; kind = "$($_.DriveType)" }
  })
ConvertTo-Json -InputObject $r -Compress
"#;

fn winfr_path() -> Option<PathBuf> {
    // Las apps de la Store se ejecutan por su alias en WindowsApps.
    let alias = PathBuf::from(std::env::var("LOCALAPPDATA").ok()?).join(r"Microsoft\WindowsApps\winfr.exe");
    alias.exists().then_some(alias)
}

#[tauri::command(async)]
pub fn recover_status() -> Result<RecoverStatus, String> {
    let out = crate::ps::powershell(DRIVES_SCRIPT)?;
    let drives: Vec<Drive> = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    Ok(RecoverStatus { installed: winfr_path().is_some(), drives })
}

#[tauri::command(async)]
pub fn recover_install(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Result<(), String> {
    let task = Task::new(&app, "recover-install").named("Instalar Windows File Recovery");
    task.step("Instalando Windows File Recovery desde la Microsoft Store…");
    let r = crate::ps::exec_opts(
        "winget",
        &["install", "--id", STORE_ID, "--source", "msstore", "--accept-package-agreements", "--accept-source-agreements", "--silent", "--disable-interactivity"],
        task.opts(Some(Duration::from_secs(600))),
    )
    .map(|_| ());
    let r = match r {
        Ok(()) if winfr_path().is_none() => Err("La instalación terminó pero no se encuentra winfr. Ábrelo una vez desde la Microsoft Store.".into()),
        other => other,
    };
    tweaks.record(Op::Run, "Instalar Windows File Recovery", &r);
    r
}

/// Qué buscar: tipos de archivo o una carpeta concreta.
fn filters(kinds: &[String], folder: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for k in kinds {
        let ext: &[&str] = match k.as_str() {
            "documents" => &["*.doc", "*.docx", "*.xls", "*.xlsx", "*.ppt", "*.pptx", "*.pdf", "*.txt", "*.odt", "*.ods", "*.rtf", "*.csv"],
            "photos" => &["*.jpg", "*.jpeg", "*.png", "*.heic", "*.gif", "*.bmp", "*.webp", "*.raw", "*.cr2", "*.nef"],
            "videos" => &["*.mp4", "*.mov", "*.avi", "*.mkv", "*.wmv", "*.3gp"],
            "music" => &["*.mp3", "*.wav", "*.m4a", "*.flac", "*.wma", "*.ogg"],
            "archives" => &["*.zip", "*.rar", "*.7z"],
            _ => return Err(format!("Tipo desconocido: {k}")),
        };
        out.extend(ext.iter().map(|e| e.to_string()));
    }
    let folder = folder.trim();
    if !folder.is_empty() {
        // winfr quiere la carpeta sin la letra, empezando y terminando por "\".
        let f = folder.get(2..).filter(|_| folder.chars().nth(1) == Some(':')).unwrap_or(folder).replace('/', "\\");
        let f = format!("\\{}\\", f.trim_matches('\\'));
        if f.contains(['"', '\n', '\r']) || f.contains("..") {
            return Err("La carpeta no es válida.".into());
        }
        out.push(f);
    }
    if out.is_empty() {
        return Err("Elige qué tipo de archivos buscar o una carpeta.".into());
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovered {
    /// Carpeta con lo recuperado.
    pub folder: String,
    pub files: u64,
}

fn count_files(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| if e.path().is_dir() { count_files(&e.path()) } else { 1 }).sum())
        .unwrap_or(0)
}

#[tauri::command(async)]
pub fn recover_run(app: tauri::AppHandle, tweaks: State<'_, TweakState>, source: String, dest: String, extensive: bool, kinds: Vec<String>, folder: String) -> Result<Recovered, String> {
    let exe = winfr_path().ok_or("Primero instala Windows File Recovery.")?;
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let letter = |d: &str| {
        let l = d.trim().trim_end_matches(['\\', ':']).to_ascii_uppercase();
        (l.len() == 1 && l.chars().all(|c| c.is_ascii_uppercase())).then_some(l)
    };
    let (src, dst) = (letter(&source).ok_or("Unidad de origen no válida.")?, letter(&dest).ok_or("Unidad de destino no válida.")?);
    if src == dst {
        return Err("Lo recuperado debe guardarse en otra unidad: escribir en la misma sobrescribiría lo que se busca.".into());
    }
    let out_dir = PathBuf::from(format!("{dst}:\\Recuperados AdminOps"));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("No se pudo preparar el destino: {e}"))?;
    let before: Vec<PathBuf> = std::fs::read_dir(&out_dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();

    let mut args: Vec<String> = vec![format!("{src}:"), out_dir.display().to_string(), if extensive { "/extensive".into() } else { "/regular".into() }];
    for f in filters(&kinds, &folder)? {
        args.push("/n".into());
        args.push(f);
    }
    // Acepta las confirmaciones de winfr (es una consola interactiva).
    args.push("/a".into());

    let task = Task::new(&app, "recover").named("Recuperar archivos");
    task.step(format!("Buscando archivos borrados en {src}: ({})…", if extensive { "a fondo, puede tardar mucho" } else { "rápido" }));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let r = crate::ps::exec_opts(&exe.display().to_string(), &refs, task.opts(None));
    tweaks.record(Op::Run, &format!("Recuperar archivos de {src}: en {dst}:"), &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r?;
    // winfr crea una carpeta Recovery_<fecha> por búsqueda.
    let created = std::fs::read_dir(&out_dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !before.contains(p)).collect::<Vec<_>>()).unwrap_or_default();
    let target = created.into_iter().next().unwrap_or(out_dir);
    let files = count_files(&target);
    let _ = std::process::Command::new("explorer.exe").arg(&target).spawn();
    Ok(Recovered { folder: target.display().to_string(), files })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_filters() {
        let f = filters(&["photos".into()], "").unwrap();
        assert!(f.contains(&"*.jpg".to_string()));
        assert_eq!(filters(&[], r"C:\Users\Ana\Documents").unwrap(), vec![r"\Users\Ana\Documents\"]);
        assert_eq!(filters(&[], r"Users\Ana\Desktop\").unwrap(), vec![r"\Users\Ana\Desktop\"]);
        assert!(filters(&[], "").is_err());
        assert!(filters(&["x".into()], "").is_err());
        assert!(filters(&[], r"C:\a\..\b").is_err());
    }

    #[test]
    fn drives_script_parses() {
        assert!(crate::ps::parse_errors(DRIVES_SCRIPT).is_empty());
    }
}
