//! Lectura de archivos `.inf` de drivers: clase, fabricante, fecha y versión,
//! modelos que trae y si sirve para Windows de 64 bits.
//!
//! Solo lo que hace falta para comparar versiones y emparejar un driver con
//! una impresora o un escáner. Tolera UTF-16 (con o sin BOM), UTF-8 y ANSI,
//! comentarios `;`, líneas partidas con `\` y cadenas `%Nombre%` que salen de
//! `[Strings]`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DriverVer {
    /// Fecha del driver en formato AAAA-MM-DD (o el texto original si no se entiende).
    pub date: String,
    /// Versión de cuatro números («4.42.0.0»), vacía si el inf no la dice.
    pub version: String,
}

/// Un modelo del inf con los ID de hardware que lo activan.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct InfModel {
    pub name: String,
    pub hardware_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct InfInfo {
    pub class: String,
    pub provider: String,
    pub driver_ver: DriverVer,
    /// Nombres de modelo (solo los de 64 bits), sin repetir.
    pub models: Vec<String>,
    /// Los mismos modelos con sus ID de hardware.
    pub entries: Vec<InfModel>,
    /// El inf trae drivers para Windows de 64 bits (o no distingue arquitectura).
    pub arch_x64: bool,
}

/// Pasa los bytes de un `.inf` a texto: UTF-16 con BOM, UTF-16 LE sin BOM, UTF-8
/// (con o sin BOM) y, si no es UTF-8, ANSI (cada byte es un carácter Latin-1).
pub fn decode_inf(bytes: &[u8]) -> String {
    fn utf16(b: &[u8], le: bool) -> String {
        let units: Vec<u16> = b.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    }
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ if looks_utf16_le(bytes) => utf16(bytes, true),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => bytes.iter().map(|&b| b as char).collect(),
        },
    }
}

/// UTF-16 LE sin BOM: casi todos los bytes en posición impar son cero (texto latino).
fn looks_utf16_le(b: &[u8]) -> bool {
    let n = b.len().min(256) & !1;
    n >= 8 && (1..n).step_by(2).filter(|&i| b[i] == 0).count() * 10 >= (n / 2) * 8
}

type Section = (String, Vec<String>);

/// Quita el comentario (`;` fuera de comillas) y los espacios.
fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => return line[..i].trim(),
            _ => {}
        }
    }
    line.trim()
}

/// Secciones en minúsculas con sus líneas ya sin comentarios ni continuaciones.
fn sections(text: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut pending = String::new();
    for raw in text.lines() {
        let mut line = strip_comment(raw).to_string();
        if !pending.is_empty() {
            line = format!("{pending}{line}");
            pending.clear();
        }
        if let Some(head) = line.strip_suffix('\\') {
            pending = format!("{} ", head.trim_end());
            continue;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            out.push((line[1..line.len() - 1].trim().to_ascii_lowercase(), Vec::new()));
        } else if let Some(last) = out.last_mut() {
            last.1.push(line.to_string());
        }
    }
    out
}

/// Todas las líneas de las secciones que se llaman `name` (puede haber varias).
fn lines_of<'a>(secs: &'a [Section], name: &str) -> Vec<&'a String> {
    secs.iter().filter(|(n, _)| n == name).flat_map(|(_, l)| l.iter()).collect()
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(s)
}

/// `[Strings]` (y las de idioma, que solo rellenan lo que falte).
fn strings_map(secs: &[Section]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut order: Vec<&Section> = secs.iter().filter(|(n, _)| n == "strings" || n.starts_with("strings.")).collect();
    order.sort_by_key(|(n, _)| n != "strings");
    for (_, lines) in order {
        for l in lines {
            if let Some((k, v)) = split_kv(l) {
                m.entry(k.to_ascii_lowercase()).or_insert_with(|| unquote(v).to_string());
            }
        }
    }
    m
}

/// Cambia cada `%Nombre%` por su valor de `[Strings]` (`%%` es un `%`). Lo que no
/// se puede resolver se deja tal cual.
fn resolve(s: &str, strings: &HashMap<String, String>) -> String {
    let s = unquote(s);
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('%') {
            Some(0) => {
                out.push('%');
                rest = &after[1..];
            }
            Some(j) => {
                let key = &after[..j];
                match strings.get(&key.to_ascii_lowercase()) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push('%');
                        out.push_str(key);
                        out.push('%');
                    }
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    unquote(&out).trim().to_string()
}

/// `07/15/2025` → `2025-07-15`; si no se entiende, se deja el texto.
fn iso_date(d: &str) -> String {
    let p: Vec<&str> = d.split('/').collect();
    if let [m, day, y] = p.as_slice() {
        if let (Ok(m), Ok(day), Ok(y)) = (m.trim().parse::<u32>(), day.trim().parse::<u32>(), y.trim().parse::<u32>()) {
            if (1..=12).contains(&m) && (1..=31).contains(&day) && y >= 1980 {
                return format!("{y:04}-{m:02}-{day:02}");
            }
        }
    }
    d.trim().to_string()
}

fn parse_driver_ver(v: &str) -> DriverVer {
    let mut parts = v.split(',');
    let date = iso_date(parts.next().unwrap_or(""));
    let version = parts.next().unwrap_or("").trim();
    let version = if !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.') { version.to_string() } else { String::new() };
    DriverVer { date, version }
}

/// Lee el texto de un `.inf`.
pub fn parse_inf(text: &str) -> InfInfo {
    let text = text.trim_start_matches('\u{feff}');
    let secs = sections(text);
    let strings = strings_map(&secs);
    let mut info = InfInfo::default();

    for l in lines_of(&secs, "version") {
        let Some((k, v)) = split_kv(l) else { continue };
        match k.to_ascii_lowercase().as_str() {
            "class" => info.class = resolve(v, &strings),
            "provider" => info.provider = resolve(v, &strings),
            "driverver" => info.driver_ver = parse_driver_ver(&resolve(v, &strings)),
            _ => {}
        }
    }

    // [Manufacturer]: `%Marca% = Modelos, NTamd64, NTamd64.6.1` → secciones `modelos.ntamd64`…
    let mut model_sections: Vec<String> = Vec::new();
    for l in lines_of(&secs, "manufacturer") {
        let Some((_, v)) = split_kv(l) else { continue };
        let mut parts = v.split(',').map(str::trim);
        let base = unquote(parts.next().unwrap_or("")).to_ascii_lowercase();
        if base.is_empty() {
            continue;
        }
        let decorations: Vec<String> = parts.map(|p| p.to_ascii_lowercase()).collect();
        if decorations.is_empty() {
            // Sin decoración vale para cualquier arquitectura.
            info.arch_x64 = true;
            model_sections.push(base);
            continue;
        }
        for d in decorations {
            if d.starts_with("ntamd64") || d == "nt" || d.starts_with("nt.") {
                info.arch_x64 = true;
                model_sections.push(format!("{base}.{d}"));
            }
        }
    }

    for name in &model_sections {
        for l in lines_of(&secs, name) {
            let Some((k, v)) = split_kv(l) else { continue };
            let model = resolve(k, &strings);
            if model.is_empty() {
                continue;
            }
            let hardware_ids: Vec<String> = v.split(',').skip(1).map(|h| resolve(h, &strings)).filter(|h| !h.is_empty()).collect();
            if !info.models.contains(&model) {
                info.models.push(model.clone());
            }
            info.entries.push(InfModel { name: model, hardware_ids });
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
; Driver de ejemplo
[Version]
Signature="$Windows NT$"
Class=Printer
ClassGuid={4D36E979-E325-11CE-BFC1-08002BE10318}
Provider=%MFG%
DriverVer=07/15/2025,4.42.0.0   ; fecha y versión

[Manufacturer]
%MFG%=RICOH,NTamd64,NTx86

[RICOH.NTamd64]
%PCL6%=PCL6_Install, USBPRINT\RICOHIM_C3000_6A3C, RICOH_IM_C3000_PCL6
"RICOH IM C3000 PS" = PS_Install, RICOH_IM_C3000_PS ; con comentario
%Dup%=PCL6_Install
%Dup%=PCL6_Install

[RICOH.NTx86]
"RICOH Solo 32 bits" = X86_Install, X86ID

[Strings]
MFG = "RICOH"
PCL6 = "RICOH IM C3000 PCL 6"
Dup = "RICOH Universal"
"#;

    #[test]
    fn reads_version_class_provider_and_models() {
        let i = parse_inf(SAMPLE);
        assert_eq!(i.class, "Printer");
        assert_eq!(i.provider, "RICOH");
        assert_eq!(i.driver_ver, DriverVer { date: "2025-07-15".into(), version: "4.42.0.0".into() });
        assert!(i.arch_x64);
        // Solo los de 64 bits y sin repetir.
        assert_eq!(i.models, ["RICOH IM C3000 PCL 6", "RICOH IM C3000 PS", "RICOH Universal"]);
        assert_eq!(i.entries[0].hardware_ids, ["USBPRINT\\RICOHIM_C3000_6A3C", "RICOH_IM_C3000_PCL6"]);
    }

    #[test]
    fn utf16_with_bom_is_read_like_ansi() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in SAMPLE.encode_utf16() {
            bytes.extend(u.to_le_bytes());
        }
        let text = decode_inf(&bytes);
        assert_eq!(parse_inf(&text), parse_inf(SAMPLE));
        // Sin BOM también.
        let no_bom = &bytes[2..];
        assert_eq!(parse_inf(&decode_inf(no_bom)).models.len(), 3);
        // UTF-8 con BOM y ANSI (é = 0xE9).
        let mut utf8 = vec![0xEF, 0xBB, 0xBF];
        utf8.extend(SAMPLE.as_bytes());
        assert_eq!(parse_inf(&decode_inf(&utf8)).class, "Printer");
        assert_eq!(decode_inf(&[b'a', 0xE9, b'b']), "aéb");
    }

    #[test]
    fn only_32_bit_infs_are_flagged_and_have_no_models() {
        let text = "[Version]\nClass=Printer\n[Manufacturer]\n\"X\"=Mod,NTx86\n[Mod.NTx86]\n\"X 1\"=a,b\n";
        let i = parse_inf(text);
        assert!(!i.arch_x64);
        assert!(i.models.is_empty());
        // Sin decoración: sirve para todo.
        let i = parse_inf("[Manufacturer]\n\"X\"=Mod\n[Mod]\n\"X 1\"=a,b\n");
        assert!(i.arch_x64);
        assert_eq!(i.models, ["X 1"]);
    }

    #[test]
    fn comments_continuations_and_unresolved_strings() {
        let text = "[Version]\nClass = Image ; escáner\nProvider = %Missing%\nDriverVer = 03/01/2021\n[Manufacturer]\n%A% = M,\\\n NTamd64\n[M.NTamd64]\n\"Esc;aner\" = i, id\n[Strings]\nA = \"EPSON\"\n";
        let i = parse_inf(text);
        assert_eq!(i.class, "Image");
        assert_eq!(i.provider, "%Missing%");
        assert_eq!(i.driver_ver, DriverVer { date: "2021-03-01".into(), version: String::new() });
        assert_eq!(i.models, ["Esc;aner"], "el ; entre comillas no es un comentario");
        assert_eq!(resolve("100%%", &HashMap::new()), "100%");
        assert_eq!(parse_inf("").models.len(), 0);
    }
}
