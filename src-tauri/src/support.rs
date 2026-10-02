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
        "{}Modo: {reason}\nNavegador interno: {}\nMicrosoft 365: {}\nModo auditoría: {}\n\n{}",
        info_text(app),
        if crate::paths::browser_on_usb() { "en el pendrive" } else { "en el disco de este equipo" },
        serde_json::to_value(crate::graph::graph_status(app.clone())).ok().map_or("—".into(), |v| if v["connected"] == true { "conectado" } else if v["configured"] == true { "configurado, sin conectar" } else { "no configurado" }.to_string()),
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

/// Crea el .zip junto a los informes y lo muestra en el Explorador. Devuelve su ruta.
#[tauri::command(async)]
pub fn support_package(app: tauri::AppHandle) -> Result<String, String> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let staging = std::env::temp_dir().join(format!("adminops-soporte-{stamp}"));
    let result = (|| -> Result<PathBuf, String> {
        std::fs::create_dir_all(staging.join("registro")).map_err(|e| e.to_string())?;
        std::fs::write(staging.join("info.txt"), info_text(&app)).map_err(|e| e.to_string())?;
        std::fs::write(staging.join("resumen.txt"), summary(&app)).map_err(|e| e.to_string())?;
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
    let zip = result?;
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", zip.display())).spawn();
    log::info!("Paquete de soporte creado");
    Ok(zip.display().to_string())
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
}
