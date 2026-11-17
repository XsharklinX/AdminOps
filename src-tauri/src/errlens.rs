//! Lector de errores: del recorte o la foto de un mensaje de error a lo que
//! significa. Lee el texto de la imagen con el OCR de Windows (`ocr.rs`), busca
//! los códigos que aparecen (0x80070005, error 809, AADSTS50126, un nombre de
//! pantallazo…) y los explica con el diccionario de `errcodes.rs`.
//!
//! También funciona con un texto pegado. Solo lee: nada se cambia.

use crate::errcodes::{explain, explain_aad, explain_ras, explain_win32, find_names, ErrorInfo};
use serde::Serialize;

/// Tamaño máximo de una imagen (una foto de móvil ronda los 5 MB).
const MAX_IMAGE: usize = 30 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// Tal como aparece en el mensaje, ya normalizado («0x0000011B»).
    pub raw: String,
    /// None: el código está en el mensaje pero el diccionario no lo conoce.
    pub info: Option<ErrorInfo>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Lens {
    /// El texto leído (sin nombres de usuario en las rutas).
    pub text: String,
    pub codes: Vec<Found>,
}

/// Lo que se encontró en el texto, antes de explicarlo.
#[derive(Clone, Debug, PartialEq)]
enum Hit {
    /// Hexadecimal o número de 32 bits: 0x80070005
    Hex(u32),
    /// «error 809» o «código de error 5»
    Decimal(u32),
    /// AADSTS50126
    Aad(u32),
}

/// El OCR confunde la O con el 0 y la l con el 1 dentro de un código.
fn fix_hex(c: char) -> Option<char> {
    match c {
        '0'..='9' | 'a'..='f' | 'A'..='F' => Some(c),
        'O' | 'o' => Some('0'),
        'l' | 'I' | '|' => Some('1'),
        _ => None,
    }
}

fn boundary(chars: &[char], i: usize) -> bool {
    i == 0 || !chars[i - 1].is_alphanumeric()
}

fn scan(text: &str) -> Vec<Hit> {
    let chars: Vec<char> = text.chars().collect();
    let mut hits: Vec<Hit> = Vec::new();
    let mut push = |h: Hit| {
        if !hits.contains(&h) {
            hits.push(h);
        }
    };
    let n = chars.len();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        // 0x… (el OCR a veces lee «Ox»)
        if (c == '0' || c == 'O' || c == 'o') && i + 2 < n && (chars[i + 1] == 'x' || chars[i + 1] == 'X') && boundary(&chars, i) {
            let mut hex = String::new();
            let mut j = i + 2;
            while j < n && hex.len() < 8 {
                match fix_hex(chars[j]) {
                    Some(h) => hex.push(h),
                    None => break,
                }
                j += 1;
            }
            // Un código pegado a más letras («0xford») no es un código.
            let glued = j < n && chars[j].is_alphanumeric();
            if hex.len() >= 3 && !glued {
                if let Ok(v) = u32::from_str_radix(&hex, 16) {
                    push(Hit::Hex(v));
                }
            }
            i = j.max(i + 1);
            continue;
        }
        // AADSTS50126
        if (c == 'A' || c == 'a') && boundary(&chars, i) && i + 5 < n {
            let word: String = chars[i..i + 5].iter().collect::<String>().to_ascii_uppercase();
            if word == "AADST" && chars.get(i + 5).is_some_and(|x| x.eq_ignore_ascii_case(&'S')) {
                let digits: String = chars[i + 6..].iter().take_while(|d| d.is_ascii_digit()).collect();
                if (4..=8).contains(&digits.len()) {
                    if let Ok(v) = digits.parse() {
                        push(Hit::Aad(v));
                    }
                }
                i += 6 + digits.len();
                continue;
            }
        }
        // 80070005 / c000021a sueltos
        if (c == '8' || c == 'c' || c == 'C') && boundary(&chars, i) && i + 8 <= n {
            let word: String = chars[i..i + 8].iter().collect();
            let after = chars.get(i + 8).is_some_and(|x| x.is_alphanumeric());
            if !after && word.chars().all(|x| x.is_ascii_hexdigit()) && word.chars().filter(|x| x.is_ascii_digit()).count() >= 4 {
                if let Ok(v) = u32::from_str_radix(&word, 16) {
                    push(Hit::Hex(v));
                }
                i += 8;
                continue;
            }
        }
        // -2147024891
        if c == '-' && i + 1 < n && chars[i + 1].is_ascii_digit() && boundary(&chars, i) {
            let digits: String = chars[i + 1..].iter().take_while(|d| d.is_ascii_digit()).collect();
            if (9..=10).contains(&digits.len()) {
                if let Ok(v) = digits.parse::<i64>() {
                    push(Hit::Hex((-v) as i32 as u32));
                }
            }
            i += 1 + digits.len();
            continue;
        }
        i += 1;
    }
    // «error 809», «código de error: 5», «error code 0»
    let words: Vec<String> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
    for (k, w) in words.iter().enumerate() {
        if !matches!(w.as_str(), "error" | "code" | "codigo" | "código") {
            continue;
        }
        let filler = |x: &str| matches!(x, "de" | "error" | "code" | "codigo" | "código" | "numero" | "número" | "no" | "n" | "nº" | "num");
        let mut j = k + 1;
        while j < words.len() && j <= k + 3 && filler(&words[j]) {
            j += 1;
        }
        if let Some(d) = words.get(j) {
            if (1..=5).contains(&d.len()) && d.chars().all(|x| x.is_ascii_digit()) {
                if let Ok(v) = d.parse::<u32>() {
                    if v > 0 {
                        push(Hit::Decimal(v));
                    }
                }
            }
        }
    }
    hits
}

/// Todo lo que el diccionario sabe de los códigos que aparecen en un texto.
pub fn lens_text(text: &str) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    let mut add = |raw: String, info: Option<ErrorInfo>| {
        if !out.iter().any(|f| f.raw == raw) {
            out.push(Found { raw, info });
        }
    };
    for h in scan(text) {
        match h {
            Hit::Hex(v) => add(if v <= 0xFFFF { format!("0x{v:X}") } else { format!("0x{v:08X}") }, explain(v)),
            Hit::Aad(v) => add(format!("AADSTS{v}"), explain_aad(v)),
            Hit::Decimal(v) => {
                let info = if (600..=950).contains(&v) { explain_ras(v) } else { None }.or_else(|| explain_win32(v));
                add(format!("error {v}"), info)
            }
        }
    }
    for info in find_names(text) {
        add(info.name.clone(), Some(info));
    }
    // Lo que el diccionario conoce, primero.
    out.sort_by_key(|f| f.info.is_none());
    out
}

fn finish(text: String) -> Lens {
    let codes = lens_text(&text);
    let clean: String = crate::escalate::scrub(&text).chars().take(4000).collect();
    Lens { text: clean, codes }
}

/// Un texto pegado.
#[tauri::command]
pub fn error_lens_text(text: String) -> Lens {
    finish(text.chars().take(20000).collect())
}

/// Una imagen pegada o soltada, en base64 (el recorte o la foto tal cual).
#[tauri::command(async)]
pub fn error_lens_image(data: String) -> Result<Lens, String> {
    use base64::Engine;
    let b64 = data.rsplit_once(',').map_or(data.as_str(), |(_, rest)| rest);
    if b64.len() > MAX_IMAGE * 4 / 3 + 8 {
        return Err("La imagen es demasiado grande (máximo 30 MB).".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| "La imagen no se pudo leer.".to_string())?;
    Ok(finish(crate::ocr::text_from_bytes(bytes)?))
}

/// Una imagen del disco (la que se suelta sobre la ventana).
#[tauri::command(async)]
pub fn error_lens_file(path: String) -> Result<Lens, String> {
    let p = std::path::Path::new(&path);
    let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp"].contains(&ext.as_str()) {
        return Err("Eso no es una imagen (PNG, JPG, BMP…).".into());
    }
    let len = std::fs::metadata(p).map_err(|_| "No se encuentra el archivo.".to_string())?.len();
    if len as usize > MAX_IMAGE {
        return Err("La imagen es demasiado grande (máximo 30 MB).".into());
    }
    let bytes = std::fs::read(p).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    Ok(finish(crate::ocr::text_from_bytes(bytes)?))
}

/// El recorte que hay ahora en el portapapeles (Win + Mayús + S).
#[tauri::command(async)]
pub fn error_lens_clipboard() -> Result<Lens, String> {
    Ok(finish(crate::ocr::text_from_clipboard()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raws(t: &str) -> Vec<String> {
        lens_text(t).into_iter().map(|f| f.raw).collect()
    }

    #[test]
    fn encuentra_el_codigo_de_la_impresora() {
        let t = "Windows no se puede conectar a la impresora. La operación no se pudo completar (error 0x0000011b). Aceptar";
        let f = lens_text(t);
        assert_eq!(f[0].raw, "0x11B");
        assert_eq!(f[0].info.as_ref().unwrap().page, "printers");
    }

    #[test]
    fn perdona_los_errores_del_ocr() {
        assert_eq!(raws("Ox8007OOO5"), vec!["0x80070005"]);
        assert_eq!(raws("código 0xc000O21a"), vec!["0xC000021A"]);
    }

    #[test]
    fn varios_formatos() {
        assert_eq!(raws("80070005"), vec!["0x80070005"]);
        assert_eq!(raws("-2147024891"), vec!["0x80070005"]);
        assert_eq!(raws("Error 809: no se pudo establecer la conexión"), vec!["error 809"]);
        assert!(lens_text("Error 809")[0].info.is_some());
        assert_eq!(raws("AADSTS50126: Error validating credentials")[0], "AADSTS50126");
        assert!(lens_text("AADSTS50126")[0].info.is_some());
        assert_eq!(raws("DPC_WATCHDOG_VIOLATION")[0], "DPC_WATCHDOG_VIOLATION");
        assert_eq!(raws("error 5")[0], "error 5");
        assert!(lens_text("error 5")[0].info.is_some());
    }

    #[test]
    fn no_inventa_codigos() {
        assert!(raws("Oxford y 0xford, la fecha 12345678 y el pedido 20240101").is_empty());
        assert!(raws("").is_empty());
        // Un código que el diccionario no conoce se enseña igual, sin explicación y al final.
        let f = lens_text("0x80070005 y 0x1234abcd");
        assert!(f[0].info.is_some() && f[1].info.is_none());
    }

    #[test]
    fn el_texto_sale_sin_el_usuario() {
        let l = finish(r"No se encuentra C:\Users\ana\Documentos\a.xlsx (error 0x80070002)".into());
        assert!(!l.text.contains("ana"));
        assert_eq!(l.codes.len(), 1);
    }

    #[test]
    fn rechaza_lo_que_no_es_imagen() {
        assert!(error_lens_file("C:\\x\\a.exe".into()).is_err());
        assert!(error_lens_image("%%%".into()).is_err());
    }
}
