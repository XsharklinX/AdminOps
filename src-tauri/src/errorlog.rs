//! Registro de errores de la propia AdminOps (1.2.9): lo que falla dentro de la
//! aplicación (un fallo de pantalla, una orden que revienta) se apunta aquí,
//! agrupado y sin datos personales, para saber qué falla y cuánto, y poder pegar
//! un informe en un aviso de fallo. Es local: nada se envía solo.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Cuántos grupos distintos se guardan (los más antiguos se van).
const MAX_GROUPS: usize = 100;
const MAX_MESSAGE: usize = 300;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// «pantalla» (la interfaz falló) o «orden» (una orden del sistema reventó).
    pub source: String,
    /// Dónde: la pantalla o la orden.
    pub place: String,
    /// El mensaje, ya sin rutas ni nombres de usuario.
    pub message: String,
    pub count: u32,
    pub first: u64,
    pub last: u64,
    /// La versión de AdminOps en la que ocurrió por última vez.
    pub version: String,
}

/// El mensaje sin lo que cambia de una vez a otra (números, rutas, identificadores):
/// dos fallos iguales caen en el mismo grupo.
pub fn normalize(message: &str) -> String {
    let first: String = message.trim().lines().next().unwrap_or("").chars().take(MAX_MESSAGE).collect();
    let text = crate::escalate::scrub(&first);
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Una ruta de disco: «C:\…» hasta un espacio o una comilla.
        if c.is_ascii_alphabetic() && chars.get(i + 1) == Some(&':') && matches!(chars.get(i + 2), Some('\\' | '/')) && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            out.push_str("<ruta>");
            let stop = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | ')');
            loop {
                while i < chars.len() && !stop(chars[i]) {
                    i += 1;
                }
                // Los nombres con espacios («Mis documentos\x.docx»): la palabra siguiente sigue siendo
                // la ruta si lleva una barra o termina en una extensión.
                let mut k = i;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                let mut e = k;
                while e < chars.len() && !stop(chars[e]) {
                    e += 1;
                }
                let word: String = chars[k..e].iter().collect();
                let ext = word.rsplit_once('.').is_some_and(|(_, x)| (1..=5).contains(&x.len()) && x.chars().all(|c| c.is_ascii_alphanumeric()));
                if k > i && e > k && (word.contains(['\\', '/']) || ext) {
                    i = e;
                } else {
                    break;
                }
            }
            continue;
        }
        // Números (y 0x…): «#».
        if c.is_ascii_digit() {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
            out.push('#');
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// La clave del grupo: de dónde viene y qué dice (sin lo variable).
fn key(source: &str, place: &str, message: &str) -> String {
    format!("{source}|{place}|{}", normalize(message))
}

/// Apunta un fallo: si ya había uno igual, suma; si no, abre un grupo.
pub fn record(groups: &mut Vec<Group>, source: &str, place: &str, message: &str, now: u64, version: &str) {
    let msg = normalize(message);
    if msg.is_empty() {
        return;
    }
    let place: String = place.chars().take(60).collect();
    let source = if source == "orden" { "orden" } else { "pantalla" };
    let k = key(source, &place, message);
    if let Some(g) = groups.iter_mut().find(|g| key(&g.source, &g.place, &g.message) == k) {
        g.count = g.count.saturating_add(1);
        g.last = now;
        g.version = version.to_string();
        return;
    }
    groups.push(Group { source: source.into(), place, message: msg, count: 1, first: now, last: now, version: version.to_string() });
    if groups.len() > MAX_GROUPS {
        groups.sort_by_key(|g| std::cmp::Reverse(g.last));
        groups.truncate(MAX_GROUPS);
    }
}

/// Los grupos del más repetido al menos, y a igualdad, el más reciente.
pub fn sorted(mut groups: Vec<Group>) -> Vec<Group> {
    groups.sort_by_key(|g| (std::cmp::Reverse(g.count), std::cmp::Reverse(g.last)));
    groups
}

/// El texto para pegar en un aviso de fallo.
pub fn report(groups: &[Group], version: &str, os: &str) -> String {
    let mut out = format!("AdminOps {version} · {os}\nErrores de la aplicación: {}\n", groups.len());
    for g in groups.iter().take(30) {
        out.push_str(&format!("\n- [{}] {} · {} {} · {}\n  {}\n", g.source, g.place, g.count, if g.count == 1 { "vez" } else { "veces" }, g.version, g.message));
    }
    out
}

// ---------- En disco ----------

static LOCK: Mutex<()> = Mutex::new(());

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("errors.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Group> {
    crate::paths::read_json(&path(app))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Apunta un fallo en el registro (lo llaman la interfaz y el registro técnico).
pub fn note(app: &tauri::AppHandle, source: &str, place: &str, message: &str) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut groups = load(app);
    record(&mut groups, source, place, message, now(), &app.package_info().version.to_string());
    let _ = crate::paths::write_json(&path(app), &groups);
}

#[tauri::command(async)]
pub fn error_log_record(app: tauri::AppHandle, source: String, place: String, message: String) {
    note(&app, &source, &place, &message);
}

#[tauri::command(async)]
pub fn error_log_list(app: tauri::AppHandle) -> Vec<Group> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    sorted(load(&app))
}

#[tauri::command(async)]
pub fn error_log_report(app: tauri::AppHandle) -> String {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    report(&sorted(load(&app)), &app.package_info().version.to_string(), &sysinfo::System::long_os_version().unwrap_or_default())
}

#[tauri::command(async)]
pub fn error_log_clear(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    crate::paths::write_json(&path(&app), &Vec::<Group>::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_failure_with_different_numbers_is_one_group() {
        let mut g = Vec::new();
        record(&mut g, "orden", "disk_smart", "No se pudo leer SMART del disco 1 (error 0x80070005)", 100, "1.2.9");
        record(&mut g, "orden", "disk_smart", "No se pudo leer SMART del disco 2 (error 0x80041003)", 200, "1.2.9");
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].count, g[0].first, g[0].last), (2, 100, 200));
        assert_eq!(g[0].message, "No se pudo leer SMART del disco # (error #)");
    }

    #[test]
    fn different_places_or_texts_are_different_groups() {
        let mut g = Vec::new();
        record(&mut g, "orden", "a", "falló", 1, "1");
        record(&mut g, "orden", "b", "falló", 1, "1");
        record(&mut g, "pantalla", "a", "falló", 1, "1");
        record(&mut g, "orden", "a", "otra cosa", 1, "1");
        assert_eq!(g.len(), 4);
    }

    #[test]
    fn no_personal_data_is_kept() {
        let m = normalize("No se pudo abrir C:\\Users\\ana.garcia\\Documentos\\Cliente Perez\\x.docx porque no existe");
        assert!(!m.contains("ana") && !m.contains("Perez") && !m.contains("Documentos"), "{m}");
        assert_eq!(m, "No se pudo abrir <ruta> porque no existe");
        let m = normalize("Error en D:/Datos/clientes/acme.json (línea 12)");
        assert_eq!(m, "Error en <ruta> (línea #)");
        // Solo la primera línea: lo de debajo (pilas, rutas de código) no se guarda.
        assert_eq!(normalize("Algo falló\n    at C:\\Users\\bob\\x.js:3:4"), "Algo falló");
    }

    #[test]
    fn long_or_empty_messages_are_handled() {
        assert!(normalize(&"x".repeat(5000)).chars().count() <= MAX_MESSAGE);
        let mut g = Vec::new();
        record(&mut g, "orden", "a", "   \n  ", 1, "1");
        assert!(g.is_empty());
        record(&mut g, "raro", "a", "falló", 1, "1");
        assert_eq!(g[0].source, "pantalla", "una fuente desconocida se toma por la interfaz");
    }

    #[test]
    fn keeps_a_bounded_number_of_groups_dropping_the_oldest() {
        let mut g = Vec::new();
        for i in 0..(MAX_GROUPS as u64 + 30) {
            record(&mut g, "orden", "a", &format!("fallo tipo {}", "x".repeat(i as usize % 7) + &"y".repeat(i as usize)), i, "1");
        }
        assert_eq!(g.len(), MAX_GROUPS);
        assert!(g.iter().all(|x| x.last >= 30), "se van los más antiguos");
    }

    #[test]
    fn sorted_by_how_often_then_how_recent_and_the_report_reads_well() {
        let mk = |msg: &str, count, last| Group { source: "orden".into(), place: "p".into(), message: msg.into(), count, first: 1, last, version: "1.2.9".into() };
        let s = sorted(vec![mk("a", 1, 50), mk("b", 6, 10), mk("c", 1, 90)]);
        assert_eq!(s.iter().map(|g| g.message.as_str()).collect::<Vec<_>>(), ["b", "c", "a"]);
        let r = report(&s, "1.2.9", "Windows 11");
        assert!(r.starts_with("AdminOps 1.2.9 · Windows 11\nErrores de la aplicación: 3"));
        assert!(r.contains("6 veces") && r.contains("1 vez"));
    }
}
