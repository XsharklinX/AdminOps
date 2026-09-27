//! HTML → PDF con Microsoft Edge en modo headless.
//!
//! Edge viene instalado en todo Windows 10/11, así que no hace falta incluir
//! un motor de PDF: se imprime el mismo HTML del informe con su CSS de impresión.

use crate::ps;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(90);
/// Tras salir el proceso lanzado, cuánto se espera a la instancia que lo sustituye.
const HANDOFF: Duration = Duration::from_secs(30);

fn find_edge() -> Option<PathBuf> {
    let from_registry = [
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
        r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
    ]
    .iter()
    .filter_map(|k| crate::tweaks::registry::read_string(k, ""))
    .map(PathBuf::from);

    let from_defaults = ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|p| PathBuf::from(p).join(r"Microsoft\Edge\Application\msedge.exe"));

    from_registry.chain(from_defaults).find(|p| p.is_file())
}

/// `file:///` con los caracteres no seguros codificados (espacios, tildes, ñ…).
fn file_url(path: &Path) -> String {
    let raw = path.display().to_string().replace('\\', "/");
    let mut url = String::from("file:///");
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || b"/:-_.~".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// Convierte `html` en el PDF `out`.
pub fn html_to_pdf(html: &str, out: &Path) -> Result<(), String> {
    let edge = find_edge().ok_or("No se encontró Microsoft Edge para generar el PDF.")?;

    let work = std::env::temp_dir().join(format!("adminops-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let src = work.join("informe.html");
    std::fs::write(&src, html).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(out);

    // Perfil propio: no interfiere con un Edge que el usuario tenga abierto.
    let result = (|| {
        let mut child = ps::hidden(&edge)
            .arg("--headless=new")
            .arg("--disable-gpu")
            .arg("--no-first-run")
            // Que no se relance sin privilegios (el trabajo lo haría otro proceso).
            .arg("--do-not-de-elevate")
            .arg("--no-default-browser-check")
            .arg("--disable-extensions")
            .arg(format!("--user-data-dir={}", work.join("profile").display()))
            .arg("--no-pdf-header-footer")
            .arg(format!("--print-to-pdf={}", out.display()))
            .arg(file_url(&src))
            .spawn()
            .map_err(|e| format!("No se pudo iniciar Edge: {e}"))?;

        // Edge lanzado desde un proceso elevado puede volver a abrirse sin
        // privilegios y cerrar el original enseguida: lo que manda es que el
        // PDF aparezca completo, no que el proceso lanzado termine.
        let start = Instant::now();
        let mut exited: Option<Instant> = None;
        let mut last_size = 0u64;
        loop {
            if exited.is_none() {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    log::debug!("Edge (PDF) terminó con {:?}", status.code());
                    exited = Some(Instant::now());
                }
            }
            let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
            // Completo: existe, no crece y el proceso ya terminó (o lleva un rato estable).
            if size > 0 && size == last_size && (exited.is_some() || start.elapsed() > Duration::from_secs(20)) {
                return Ok(());
            }
            last_size = size;
            if exited.is_some_and(|t| t.elapsed() > HANDOFF) && size == 0 {
                return Err("Edge no generó el PDF.".into());
            }
            if start.elapsed() > TIMEOUT {
                let _ = child.kill();
                return Err("Edge tardó demasiado en generar el PDF.".into());
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    })();

    let _ = std::fs::remove_dir_all(&work);
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_url_encodes_spaces_and_accents() {
        let u = super::file_url(std::path::Path::new(r"C:\Users\José\Mis Informes\a.html"));
        assert_eq!(u, "file:///C:/Users/Jos%C3%A9/Mis%20Informes/a.html");
    }

    /// Genera un PDF real con Edge: `cargo test pdf_real -- --ignored`
    #[test]
    #[ignore]
    fn pdf_real() {
        let out = std::env::temp_dir().join("adminops-test.pdf");
        super::html_to_pdf("<h1>Prueba ñ</h1>", &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        let _ = std::fs::remove_file(out);
    }
}
