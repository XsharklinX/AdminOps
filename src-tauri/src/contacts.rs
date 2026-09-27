//! Agenda del técnico: personas a las que llamar o escribir (extensiones,
//! correos, para qué). Viaja con AdminOps (en portable, en el USB), así que es
//! la misma en todos los equipos. Importa y exporta CSV (Excel, Outlook).

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

static FILE_LOCK: Mutex<()> = Mutex::new(());
const MAX: usize = 5000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Contact {
    pub id: String,
    pub name: String,
    /// Cargo, área o departamento.
    pub role: String,
    pub company: String,
    pub extension: String,
    pub phone: String,
    pub mobile: String,
    pub email: String,
    /// Para qué se le llama («impresoras», «altas de usuarios», «proveedor de Internet»…).
    pub reason: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    pub updated: u64,
}

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("contacts.json")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("c{:x}{:x}", nanos, SEQ.fetch_add(1, Ordering::Relaxed))
}

fn clean(mut c: Contact) -> Result<Contact, String> {
    let cut = |s: &str, n: usize| s.trim().chars().take(n).collect::<String>();
    c.name = cut(&c.name, 120);
    c.role = cut(&c.role, 120);
    c.company = cut(&c.company, 120);
    c.extension = cut(&c.extension, 30);
    c.phone = cut(&c.phone, 40);
    c.mobile = cut(&c.mobile, 40);
    c.email = cut(&c.email, 160);
    c.reason = cut(&c.reason, 300);
    c.notes = c.notes.chars().take(4000).collect();
    let mut tags: Vec<String> = c.tags.iter().map(|t| cut(t, 40)).filter(|t| !t.is_empty()).collect();
    tags.sort_by_key(|t| t.to_lowercase());
    tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    c.tags = tags;
    if c.name.is_empty() && c.email.is_empty() && c.company.is_empty() {
        return Err("Pon al menos un nombre, una empresa o un correo.".into());
    }
    if c.name.is_empty() {
        c.name = if c.company.is_empty() { c.email.clone() } else { c.company.clone() };
    }
    Ok(c)
}

#[tauri::command]
pub fn list_contacts(app: tauri::AppHandle) -> Vec<Contact> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    crate::paths::read_json(&path(&app))
}

#[tauri::command]
pub fn save_contact(app: tauri::AppHandle, contact: Contact) -> Result<Contact, String> {
    let mut c = clean(contact)?;
    c.updated = now();
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<Contact> = crate::paths::read_json(&path(&app));
    match list.iter().position(|x| !c.id.is_empty() && x.id == c.id) {
        Some(i) => list[i] = c.clone(),
        None => {
            if list.len() >= MAX {
                return Err(format!("La agenda admite hasta {MAX} contactos."));
            }
            c.id = new_id();
            list.push(c.clone());
        }
    }
    crate::paths::write_json(&path(&app), &list)?;
    Ok(c)
}

#[tauri::command]
pub fn delete_contact(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<Contact> = crate::paths::read_json(&path(&app));
    list.retain(|c| c.id != id);
    crate::paths::write_json(&path(&app), &list)
}

/// Abre el programa de correo con un mensaje nuevo para `email`.
#[tauri::command]
pub fn write_email(email: String) -> Result<(), String> {
    let e = email.trim();
    let valid = e.len() < 160 && e.contains('@') && e.chars().all(|c| c.is_alphanumeric() || "@._-+".contains(c));
    if !valid {
        return Err("El correo no es válido.".into());
    }
    std::process::Command::new("explorer.exe").arg(format!("mailto:{e}")).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ---------- CSV ----------

/// Separa un CSV respetando comillas (Excel en español usa `;`).
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let sep = [';', ',', '\t'].into_iter().max_by_key(|s| first.matches(*s).count()).unwrap_or(',');
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => row.push(std::mem::take(&mut field)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    rows
}

fn norm(h: &str) -> String {
    h.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' => 'a',
            'é' | 'è' => 'e',
            'í' => 'i',
            'ó' | 'ò' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            c => c,
        })
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Qué campo es cada columna (nombres en español, inglés y los de Outlook).
fn field_of(header: &str) -> Option<&'static str> {
    Some(match norm(header).as_str() {
        "nombre" | "name" | "nombrecompleto" | "fullname" | "contacto" | "displayname" => "name",
        "firstname" | "nombrepila" => "first",
        "lastname" | "apellido" | "apellidos" => "last",
        "cargo" | "puesto" | "rol" | "area" | "departamento" | "jobtitle" | "title" | "department" | "role" => "role",
        "empresa" | "compania" | "company" | "organizacion" | "organization" | "proveedor" => "company",
        "extension" | "ext" | "anexo" | "interno" => "extension",
        "telefono" | "phone" | "businessphone" | "telefonotrabajo" | "fijo" | "telefonoprincipal" | "primaryphone" => "phone",
        "movil" | "celular" | "mobile" | "mobilephone" | "telefonomovil" | "whatsapp" => "mobile",
        "correo" | "email" | "mail" | "emailaddress" | "correoelectronico" | "email1" => "email",
        "motivo" | "paraque" | "reason" | "llamarpara" | "asunto" | "servicio" => "reason",
        "etiquetas" | "tags" | "categorias" | "categories" | "grupo" => "tags",
        "notas" | "notes" | "observaciones" | "comentarios" => "notes",
        _ => return None,
    })
}

fn contacts_from_csv(text: &str) -> Result<Vec<Contact>, String> {
    let rows = parse_csv(text);
    let (header, body) = rows.split_first().ok_or("El archivo está vacío.")?;
    let fields: Vec<Option<&str>> = header.iter().map(|h| field_of(h)).collect();
    if !fields.iter().any(|f| matches!(f, Some("name" | "first" | "email"))) {
        return Err("No se reconocen las columnas. La primera fila debe tener los títulos: Nombre, Cargo, Empresa, Extensión, Teléfono, Móvil, Correo, Motivo, Etiquetas, Notas.".into());
    }
    let mut out = Vec::new();
    for r in body {
        let mut c = Contact::default();
        let (mut first, mut last) = (String::new(), String::new());
        for (v, f) in r.iter().zip(&fields) {
            let v = v.trim().to_string();
            match f {
                Some("name") => c.name = v,
                Some("first") => first = v,
                Some("last") => last = v,
                Some("role") if c.role.is_empty() => c.role = v,
                Some("company") => c.company = v,
                Some("extension") => c.extension = v,
                Some("phone") if c.phone.is_empty() => c.phone = v,
                Some("mobile") => c.mobile = v,
                Some("email") if c.email.is_empty() => c.email = v,
                Some("reason") => c.reason = v,
                Some("tags") => c.tags = v.split([',', ';', '|']).map(|t| t.trim().to_string()).collect(),
                Some("notes") => c.notes = v,
                _ => {}
            }
        }
        if c.name.is_empty() {
            c.name = format!("{first} {last}").trim().to_string();
        }
        if let Ok(c) = clean(c) {
            out.push(c);
        }
    }
    Ok(out)
}

/// Clave para no duplicar al importar dos veces el mismo archivo.
fn same(a: &Contact, b: &Contact) -> bool {
    a.name.eq_ignore_ascii_case(&b.name) && (a.email.eq_ignore_ascii_case(&b.email) || (a.email.is_empty() || b.email.is_empty()) && a.extension == b.extension)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    added: usize,
    updated: usize,
}

#[tauri::command(async)]
pub fn import_contacts(app: tauri::AppHandle) -> Result<Option<ImportResult>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(file) = app.dialog().file().add_filter("CSV (Excel, Outlook)", &["csv", "txt"]).blocking_pick_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let bytes = std::fs::read(&file).map_err(|e| format!("No se pudo leer: {e}"))?;
    // Excel guarda a veces en Windows-1252: se aceptan los dos.
    let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
    let incoming = contacts_from_csv(&text)?;
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<Contact> = crate::paths::read_json(&path(&app));
    let (mut added, mut updated) = (0, 0);
    for mut c in incoming {
        c.updated = now();
        if let Some(existing) = list.iter_mut().find(|x| same(x, &c)) {
            // Rellena lo que falte sin borrar lo que ya había.
            let fill = |dst: &mut String, src: String| {
                if dst.is_empty() {
                    *dst = src;
                }
            };
            fill(&mut existing.role, c.role);
            fill(&mut existing.company, c.company);
            fill(&mut existing.extension, c.extension);
            fill(&mut existing.phone, c.phone);
            fill(&mut existing.mobile, c.mobile);
            fill(&mut existing.email, c.email);
            fill(&mut existing.reason, c.reason);
            fill(&mut existing.notes, c.notes);
            for t in c.tags {
                if !existing.tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                    existing.tags.push(t);
                }
            }
            updated += 1;
        } else if list.len() < MAX {
            c.id = new_id();
            list.push(c);
            added += 1;
        }
    }
    crate::paths::write_json(&path(&app), &list)?;
    Ok(Some(ImportResult { added, updated }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_spanish_excel_csv() {
        let csv = "\u{feff}Nombre;Cargo;Extensión;Correo;Motivo;Etiquetas\r\nAna López;Sistemas;2104;ana@empresa.com;Altas de usuarios;TI, urgente\r\n\"Pérez; Juan\";Compras;;juan@empresa.com;\"Pedidos \"\"urgentes\"\"\";\r\n";
        let c = contacts_from_csv(csv).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].extension, "2104");
        assert_eq!(c[0].tags, vec!["TI", "urgente"]);
        assert_eq!(c[1].name, "Pérez; Juan");
        assert_eq!(c[1].reason, "Pedidos \"urgentes\"");
    }

    #[test]
    fn imports_outlook_csv() {
        let csv = "First Name,Last Name,E-mail Address,Business Phone,Mobile Phone,Job Title,Company\nLuis,Gómez,luis@x.com,+34 900 000 000,600 000 000,Soporte,Proveedor SA\n";
        let c = contacts_from_csv(csv).unwrap();
        assert_eq!(c[0].name, "Luis Gómez");
        assert_eq!(c[0].phone, "+34 900 000 000");
        assert_eq!(c[0].company, "Proveedor SA");
    }

    #[test]
    fn rejects_unknown_columns_and_empty_contacts() {
        assert!(contacts_from_csv("a;b\n1;2\n").is_err());
        assert!(clean(Contact::default()).is_err());
        assert_eq!(clean(Contact { email: "x@y.z".into(), ..Default::default() }).unwrap().name, "x@y.z");
    }
}
