//! Paquete de soporte: un .zip con el registro de actividad, el último
//! diagnóstico y la versión, para enviarlo cuando algo falla.

use std::path::PathBuf;
use std::time::Duration;

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
