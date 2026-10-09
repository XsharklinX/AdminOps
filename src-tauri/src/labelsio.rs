//! Etiquetas con QR: los comandos de la interfaz. El contenido y la hoja están en `labels.rs`.

use crate::labels::{payload, qr_svg, sheet_html, LabelInfo};

/// Vista previa de una etiqueta en pantalla (solo el QR; el texto lo pinta la interfaz).
#[tauri::command]
pub fn label_qr(info: LabelInfo) -> Result<String, String> {
    if info.host.trim().is_empty() {
        return Err("Falta el nombre del equipo.".into());
    }
    qr_svg(&payload(&info), 220)
}

/// Crea el PDF de etiquetas y lo abre para imprimirlo.
#[tauri::command(async)]
pub fn label_sheet(app: tauri::AppHandle, labels: Vec<LabelInfo>, cols: usize, rows: usize) -> Result<String, String> {
    if labels.iter().all(|l| l.host.trim().is_empty()) {
        return Err("No hay equipos para las etiquetas.".into());
    }
    let company = crate::workflow::settings(&app).company;
    let html = sheet_html(&labels, cols, rows, &company)?;
    let dir = crate::paths::reports_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let pdf = dir.join(format!("Etiquetas_{stamp}.pdf"));
    let path = match crate::diagnostics::pdf::html_to_pdf(&html, &pdf) {
        Ok(()) => pdf,
        Err(e) => {
            log::warn!("No se pudo crear el PDF de etiquetas, se guarda en HTML: {e}");
            let p = dir.join(format!("Etiquetas_{stamp}.html"));
            std::fs::write(&p, &html).map_err(|w| format!("{e} · {w}"))?;
            p
        }
    };
    crate::diagnostics::report::shell_open(&[path.as_os_str()])?;
    Ok(path.display().to_string())
}

