//! Paquete de soporte: un .zip con el registro de actividad, el último
//! diagnóstico y la versión, para enviarlo cuando algo falla. Empieza por
//! `resumen.txt`: lo que hace falta para entender un problema en otro equipo
//! sin ir hasta él (versión, dónde guarda las cosas, cuánto tarda en arrancar
//! y los últimos avisos y errores).

use std::path::PathBuf;
use std::time::Duration;

/// Lo importante del registro: los últimos arranques (con sus tiempos) y los
/// últimos avisos y errores, en orden.
pub fn digest(log: &str, starts: usize, problems: usize) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let pick = |pred: &dyn Fn(&str) -> bool, n: usize| -> Vec<&str> {
        let v: Vec<&str> = lines.iter().copied().filter(|l| pred(l)).collect();
        v[v.len().saturating_sub(n)..].to_vec()
    };
    let boots = pick(&|l| l.contains("Tiempos:") || l.contains(" iniciado · "), starts * 2);
    let bad = pick(&|l| l.contains("][WARN][") || l.contains("][ERROR]["), problems);
    let cut = |l: &&str| l.chars().take(400).collect::<String>();
    format!(
        "== Últimos arranques ==\n{}\n\n== Últimos avisos y errores ==\n{}\n",
        if boots.is_empty() { "(ninguno en el registro)".to_string() } else { boots.iter().map(cut).collect::<Vec<_>>().join("\n") },
        if bad.is_empty() { "(ninguno)".to_string() } else { bad.iter().map(cut).collect::<Vec<_>>().join("\n") }
    )
}

fn summary(app: &tauri::AppHandle) -> String {
    let log = std::fs::read_to_string(crate::paths::logs_dir(app).join(format!("{}.log", crate::paths::LOG_FILE))).unwrap_or_default();
    let reason = match crate::paths::portable_reason() {
        "removable" => "instalado en un pendrive (datos junto al programa)",
        "marker" => "portable (datos junto al programa)",
        _ => "instalado (datos en este equipo)",
    };
    format!(
        "{}Modo: {reason}\nNavegador interno: {}\nModo auditoría: {}\n\n{}",
        info_text(app),
        if crate::paths::browser_on_usb() { "en el pendrive" } else { "en el disco de este equipo" },
        if crate::audit::active() { "activo" } else { "no" },
        digest(&log, 5, 40)
    )
}

fn info_text(app: &tauri::AppHandle) -> String {
    format!(
        "AdminOps {}\nFecha: {}\nWindows: {} (build {})\nArquitectura: {}\nAdministrador: {}\nPortable: {}\nWebView2: {}\n",
        app.package_info().version,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        sysinfo::System::long_os_version().unwrap_or_default(),
        sysinfo::System::kernel_version().unwrap_or_default(),
        std::env::consts::ARCH,
        if crate::elevation::is_elevated() { "sí" } else { "no" },
        if crate::paths::is_portable() { "sí" } else { "no" },
        tauri::webview_version().unwrap_or_else(|_| "desconocida".into()),
    )
}

/// A dónde van los avisos de fallos: el correo del autor.
pub const SUPPORT_EMAIL: &str = "Contactoyerlindavid@gmail.com";

/// Crea el .zip junto a los informes. Con `description`, lleva dentro lo que
/// contó el técnico (`descripcion.txt`).
fn build_package(app: &tauri::AppHandle, description: Option<&str>) -> Result<PathBuf, String> {
    let app = app.clone();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let staging = std::env::temp_dir().join(format!("adminops-soporte-{stamp}"));
    let result = (|| -> Result<PathBuf, String> {
        std::fs::create_dir_all(staging.join("registro")).map_err(|e| e.to_string())?;
        std::fs::write(staging.join("info.txt"), info_text(&app)).map_err(|e| e.to_string())?;
        std::fs::write(staging.join("resumen.txt"), summary(&app)).map_err(|e| e.to_string())?;
        if let Some(text) = description {
            std::fs::write(staging.join("descripcion.txt"), text).map_err(|e| e.to_string())?;
        }
        for entry in std::fs::read_dir(crate::paths::logs_dir(&app)).into_iter().flatten().flatten() {
            let _ = std::fs::copy(entry.path(), staging.join("registro").join(entry.file_name()));
        }
        // Último diagnóstico guardado.
        let snaps = crate::paths::machine_data_dir(&app).join("snapshots");
        let latest = std::fs::read_dir(&snaps)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .max_by_key(|e| e.file_name());
        if let Some(e) = latest {
            let _ = std::fs::copy(e.path(), staging.join("ultimo-diagnostico.json"));
        }
        let out_dir: PathBuf = crate::paths::reports_dir(&app).parent().map_or_else(|| crate::paths::reports_dir(&app), PathBuf::from).join("Soporte");
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let zip = out_dir.join(format!("AdminOps-soporte-{stamp}.zip"));
        let script = format!(
            "{}{}Compress-Archive -Path (Join-Path $src '*') -DestinationPath $dst -Force\n'ok'",
            crate::ps::text_var("src", &staging.display().to_string()),
            crate::ps::text_var("dst", &zip.display().to_string())
        );
        crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(120)), task: None })?;
        Ok(zip)
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Crea el paquete de soporte y lo muestra en el Explorador. Devuelve su ruta.
#[tauri::command(async)]
pub fn support_package(app: tauri::AppHandle) -> Result<String, String> {
    let zip = build_package(&app, None)?;
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", zip.display())).spawn();
    log::info!("Paquete de soporte creado");
    Ok(zip.display().to_string())
}

/// El texto del aviso: lo que pasó, cómo repetirlo y a quién contestar.
fn problem_text(what: &str, steps: &str, contact: &str) -> String {
    let clip = |s: &str, n: usize| s.trim().chars().take(n).collect::<String>();
    let mut out = format!("QUÉ PASÓ\n{}\n", clip(what, 4000));
    let steps = clip(steps, 4000);
    if !steps.is_empty() {
        out.push_str(&format!("\nQUÉ ESTABA HACIENDO / CÓMO SE REPITE\n{steps}\n"));
    }
    let contact = clip(contact, 200);
    if !contact.is_empty() {
        out.push_str(&format!("\nPARA CONTESTAR\n{contact}\n"));
    }
    out
}

/// Asunto del correo: la versión y el principio de lo que pasó.
fn problem_subject(version: &str, what: &str) -> String {
    let first: String = what.trim().lines().next().unwrap_or("").chars().take(70).collect();
    format!("AdminOps {version}: {first}")
}

/// Reportar un problema: prepara un correo para el autor con la descripción y
/// el paquete de soporte. `manual`: sin adjunto (correo web); se abre el correo
/// con el texto y la carpeta del archivo para arrastrarlo. AdminOps no envía
/// nada: el correo sale cuando el técnico lo envía.
#[tauri::command(async)]
pub fn report_problem(app: tauri::AppHandle, what: String, steps: String, contact: String, manual: bool) -> Result<(), String> {
    use crate::diagnostics::report::{build_eml, shell_open, url_encode};
    if what.trim().chars().count() < 10 {
        return Err("Cuenta qué pasó con un poco más de detalle.".into());
    }
    let text = problem_text(&what, &steps, &contact);
    let zip = build_package(&app, Some(&text))?;
    let subject = problem_subject(&app.package_info().version.to_string(), &what);
    let body = format!("{text}\n---\n{}", info_text(&app));
    if manual {
        let short: String = body.chars().take(1500).collect();
        let url = format!("mailto:{SUPPORT_EMAIL}?subject={}&body={}", url_encode(&subject), url_encode(&short.replace('\n', "\r\n")));
        shell_open(&[std::ffi::OsStr::new(&url)])?;
        let arg = format!("/select,{}", zip.display());
        shell_open(&[std::ffi::OsStr::new(&arg)])?;
    } else {
        let data = std::fs::read(&zip).map_err(|e| format!("No se pudo leer el archivo de diagnóstico: {e}"))?;
        if data.len() > 20 * 1024 * 1024 {
            return Err("El archivo de diagnóstico es demasiado grande para adjuntarlo. Usa «Sin adjunto» y arrástralo al correo.".into());
        }
        let name = zip.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "AdminOps-soporte.zip".into());
        let eml = build_eml(SUPPORT_EMAIL, &subject, &body, &name, &data);
        let out = zip.with_extension("eml");
        std::fs::write(&out, eml).map_err(|e| format!("No se pudo preparar el correo: {e}"))?;
        shell_open(&[out.as_os_str()])?;
    }
    log::info!("Aviso de un problema preparado para enviar ({})", if manual { "sin adjunto" } else { "con adjunto" });
    Ok(())
}

/// Los términos de uso, tal como van en el instalador.
#[tauri::command]
pub fn terms_of_use() -> &'static str {
    include_str!("../terminos.txt")
}

#[cfg(test)]
mod tests {
    #[test]
    fn digest_picks_starts_and_problems() {
        let log = "\
[2026-09-30][16:13:36][INFO][adminops_lib] AdminOps 1.1.8 iniciado · admin=true · portable=true
[2026-09-30][16:13:56][INFO][adminops_lib::boottime] Tiempos: ventana 11948 ms
[2026-09-30][16:13:57][INFO][adminops_lib::ps] PowerShell ok
[2026-09-30][16:14:22][WARN][adminops_lib::portals] Portal p1: vista destruida porque no llegó a arrancar
[2026-09-30][16:15:00][ERROR][adminops_lib] Interfaz: algo
";
        let d = super::digest(log, 5, 40);
        assert!(d.contains("iniciado") && d.contains("Tiempos:"));
        assert!(d.contains("no llegó a arrancar") && d.contains("[ERROR]"));
        assert!(!d.contains("PowerShell ok"), "lo normal no hace ruido");
        assert!(super::digest("", 5, 5).contains("(ninguno)"));
    }

    #[test]
    fn problem_reports_are_readable() {
        let t = super::problem_text("  Tickets no carga\nSale en blanco ", "Abrí Tickets", " ana@example.com ");
        assert!(t.starts_with("QUÉ PASÓ\nTickets no carga\nSale en blanco\n"));
        assert!(t.contains("CÓMO SE REPITE\nAbrí Tickets") && t.contains("PARA CONTESTAR\nana@example.com"));
        // Sin pasos ni contacto, no salen sus títulos.
        let solo = super::problem_text("Algo falla al abrir", "", "  ");
        assert!(!solo.contains("REPITE") && !solo.contains("CONTESTAR"));
        assert_eq!(super::problem_subject("1.2.3", "Tickets no carga\nmás detalle"), "AdminOps 1.2.3: Tickets no carga");
        assert!(super::terms_of_use().contains("Sin garantía") && super::terms_of_use().contains(super::SUPPORT_EMAIL));
    }
}
