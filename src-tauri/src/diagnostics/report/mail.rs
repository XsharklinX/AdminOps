//! El correo con el informe adjunto.

use super::*;

// ---------- Envío por correo ----------

pub(super) fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Base64 en líneas de 76 caracteres (RFC 2045).
pub(super) fn b64_lines(bytes: &[u8]) -> String {
    let s = b64(bytes);
    s.as_bytes().chunks(76).map(|c| std::str::from_utf8(c).unwrap_or("")).collect::<Vec<_>>().join("\r\n")
}

pub(super) fn header_text(s: &str) -> String {
    if s.is_ascii() {
        s.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", b64(s.as_bytes()))
    }
}

pub(super) fn check_mail_fields(to: &str, subject: &str) -> Result<(), String> {
    if [to, subject].iter().any(|s| s.contains(['\r', '\n'])) || to.len() > 320 || subject.len() > 300 {
        return Err("Revisa el destinatario y el asunto.".into());
    }
    if !to.trim().is_empty() && !to.split([',', ';']).all(|a| a.trim().contains('@')) {
        return Err("El correo del destinatario no es válido.".into());
    }
    Ok(())
}

/// Correo en formato .eml, marcado como borrador para que el programa de correo lo abra listo para enviar.
pub(crate) fn build_eml(to: &str, subject: &str, body: &str, file_name: &str, attachment: &[u8]) -> String {
    let boundary = format!("adminops-{:x}", attachment.len() ^ 0x5eed);
    let body = body.replace("\r\n", "\n").replace('\n', "\r\n");
    let lower = file_name.to_lowercase();
    let mime = if lower.ends_with(".pdf") {
        "application/pdf"
    } else if lower.ends_with(".zip") {
        "application/zip"
    } else {
        "text/html"
    };
    format!(
        "X-Unsent: 1\r\nTo: {to}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n\
         --{boundary}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n\
         --{boundary}\r\nContent-Type: {mime}; name=\"{file_name}\"\r\nContent-Disposition: attachment; filename=\"{file_name}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n\
         --{boundary}--\r\n",
        header_text(subject),
        b64_lines(body.as_bytes()),
        b64_lines(attachment)
    )
}
