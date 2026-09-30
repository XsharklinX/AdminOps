//! Agenda del técnico: personas a las que llamar o escribir (extensiones,
//! correos, para qué). Viaja con AdminOps (en portable, en el USB), así que es
//! la misma en todos los equipos.
//!
//! - Papelera: lo borrado se recupera durante 30 días.
//! - Copia automática semanal (se guardan las 5 últimas).
//! - Etiquetas con color, renombrar/fusionar/borrar en todos los contactos.
//! - Importa CSV (Excel, Outlook) y vCard; exporta vCard.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

static FILE_LOCK: Mutex<()> = Mutex::new(());
const MAX: usize = 5000;
const TRASH_DAYS: u64 = 30;
const BACKUP_EVERY: u64 = 7 * 86_400;
const BACKUPS_KEPT: usize = 5;

/// Teléfono o correo adicional con su etiqueta («guardia», «personal»…).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Channel {
    /// phone | email
    pub kind: String,
    pub label: String,
    pub value: String,
}

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
    /// Más teléfonos y correos.
    pub channels: Vec<Channel>,
    /// Para qué se le llama («impresoras», «altas de usuarios», «proveedor de Internet»…).
    pub reason: String,
    /// Cuándo está («solo mañanas», «guardia fines de semana»…).
    pub availability: String,
    /// A quién llamar si no está (id de otro contacto).
    pub substitute_id: String,
    /// Cliente de AdminOps al que pertenece.
    pub client_id: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    /// Veces que se ha usado (copiar, llamar, escribir): para «Más usados».
    pub uses: u32,
    pub last_used: u64,
    pub created: u64,
    pub updated: u64,
    /// En la papelera desde (segundos Unix).
    pub deleted: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TagMeta {
    pub name: String,
    pub color: String,
}

fn path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("contacts.json")
}

fn tags_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("contacts-tags.json")
}

fn backups_dir(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("contacts-copias")
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

fn cut(s: &str, n: usize) -> String {
    s.trim().chars().take(n).collect()
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags.iter().map(|t| cut(t.trim_start_matches('#'), 40)).filter(|t| !t.is_empty()) {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    out.sort_by_key(|t| t.to_lowercase());
    out
}

fn clean(mut c: Contact) -> Result<Contact, String> {
    c.name = cut(&c.name, 120);
    c.role = cut(&c.role, 120);
    c.company = cut(&c.company, 120);
    c.extension = cut(&c.extension, 30);
    c.phone = cut(&c.phone, 40);
    c.mobile = cut(&c.mobile, 40);
    c.email = cut(&c.email, 160);
    c.reason = cut(&c.reason, 300);
    c.availability = cut(&c.availability, 200);
    c.notes = c.notes.chars().take(4000).collect();
    c.tags = clean_tags(&c.tags);
    c.channels = c
        .channels
        .into_iter()
        .map(|ch| Channel { kind: if ch.kind == "email" { "email".into() } else { "phone".into() }, label: cut(&ch.label, 40), value: cut(&ch.value, 160) })
        .filter(|ch| !ch.value.is_empty())
        .take(20)
        .collect();
    if c.name.is_empty() && c.email.is_empty() && c.company.is_empty() {
        return Err("Pon al menos un nombre, una empresa o un correo.".into());
    }
    if c.name.is_empty() {
        c.name = if c.company.is_empty() { c.email.clone() } else { c.company.clone() };
    }
    if c.substitute_id == c.id {
        c.substitute_id.clear();
    }
    Ok(c)
}

// ---------- Almacenamiento con papelera y copias ----------

/// Lee la agenda y vacía de la papelera lo que lleva más de 30 días.
fn load(app: &tauri::AppHandle) -> Vec<Contact> {
    let mut list: Vec<Contact> = crate::paths::read_json(&path(app));
    let limit = now().saturating_sub(TRASH_DAYS * 86_400);
    let before = list.len();
    list.retain(|c| c.deleted.is_none_or(|d| d >= limit));
    if list.len() != before {
        let _ = crate::paths::write_json(&path(app), &list);
    }
    list
}

fn backup_files(app: &tauri::AppHandle) -> Vec<(u64, PathBuf)> {
    let mut v: Vec<(u64, PathBuf)> = std::fs::read_dir(backups_dir(app))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let ts: u64 = p.file_stem()?.to_str()?.strip_prefix("contactos-")?.parse().ok()?;
            Some((ts, p))
        })
        .collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.0));
    v
}

/// Copia de la agenda actual (antes de sobrescribirla), como mucho una por semana.
fn backup(app: &tauri::AppHandle, force: bool) -> Result<(), String> {
    let files = backup_files(app);
    if !force && files.first().is_some_and(|(ts, _)| now().saturating_sub(*ts) < BACKUP_EVERY) {
        return Ok(());
    }
    let current = path(app);
    if !current.is_file() {
        return Ok(());
    }
    let dir = backups_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(&current, dir.join(format!("contactos-{}.json", now()))).map_err(|e| format!("No se pudo hacer la copia: {e}"))?;
    for (_, old) in backup_files(app).into_iter().skip(BACKUPS_KEPT) {
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

fn store(app: &tauri::AppHandle, list: &[Contact]) -> Result<(), String> {
    if let Err(e) = backup(app, false) {
        log::warn!("Contactos: {e}");
    }
    crate::paths::write_json(&path(app), &list)
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------- Comandos básicos ----------

/// Todos los contactos, también los de la papelera (con `deleted`).
#[tauri::command]
pub fn list_contacts(app: tauri::AppHandle) -> Vec<Contact> {
    let _g = lock();
    load(&app)
}

#[tauri::command]
pub fn save_contact(app: tauri::AppHandle, contact: Contact) -> Result<Contact, String> {
    let mut c = clean(contact)?;
    let _g = lock();
    let mut list = load(&app);
    c.updated = now();
    match list.iter().position(|x| !c.id.is_empty() && x.id == c.id) {
        Some(i) => {
            // Lo que lleva la cuenta AdminOps no lo cambia el formulario.
            let old = &list[i];
            c.uses = old.uses;
            c.last_used = old.last_used;
            c.created = old.created;
            c.deleted = old.deleted;
            list[i] = c.clone();
        }
        None => {
            if list.iter().filter(|x| x.deleted.is_none()).count() >= MAX {
                return Err(format!("La agenda admite hasta {MAX} contactos."));
            }
            c.id = new_id();
            c.created = now();
            c.uses = 0;
            c.last_used = 0;
            c.deleted = None;
            list.push(c.clone());
        }
    }
    store(&app, &list)?;
    Ok(c)
}

/// Se usó un contacto (copiar, llamar, escribir): sube en «Más usados».
#[tauri::command]
pub fn touch_contact(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _g = lock();
    let mut list = load(&app);
    if let Some(c) = list.iter_mut().find(|c| c.id == id) {
        c.uses = c.uses.saturating_add(1);
        c.last_used = now();
        crate::paths::write_json(&path(&app), &list)?;
    }
    Ok(())
}

#[derive(Deserialize, Debug)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Bulk {
    AddTag { tag: String },
    RemoveTag { tag: String },
    Favorite { value: bool },
    /// A la papelera.
    Delete,
    Restore,
    /// Borrar para siempre (solo desde la papelera).
    Purge,
}

fn apply_bulk(list: &mut Vec<Contact>, ids: &[String], op: &Bulk) -> usize {
    let t = now();
    let mut n = 0;
    if let Bulk::Purge = op {
        let before = list.len();
        list.retain(|c| !(ids.contains(&c.id) && c.deleted.is_some()));
        return before - list.len();
    }
    for c in list.iter_mut().filter(|c| ids.contains(&c.id)) {
        match op {
            Bulk::AddTag { tag } => {
                let mut tags = c.tags.clone();
                tags.push(tag.clone());
                c.tags = clean_tags(&tags);
            }
            Bulk::RemoveTag { tag } => c.tags.retain(|x| !x.eq_ignore_ascii_case(tag)),
            Bulk::Favorite { value } => c.favorite = *value,
            Bulk::Delete => c.deleted = Some(t),
            Bulk::Restore => c.deleted = None,
            Bulk::Purge => {}
        }
        if !matches!(op, Bulk::Delete | Bulk::Restore) {
            c.updated = t;
        }
        n += 1;
    }
    n
}

/// Una acción sobre varios contactos a la vez. Devuelve cuántos cambió.
#[tauri::command]
pub fn bulk_contacts(app: tauri::AppHandle, ids: Vec<String>, action: Bulk) -> Result<usize, String> {
    let _g = lock();
    let mut list = load(&app);
    let n = apply_bulk(&mut list, &ids, &action);
    store(&app, &list)?;
    Ok(n)
}

/// Une varios contactos en `keep`: rellena lo que le falte, junta etiquetas,
/// teléfonos y correos. Los demás van a la papelera (se pueden recuperar).
fn merge(list: &mut [Contact], keep: &str, others: &[String]) -> Result<(), String> {
    let absorbed: Vec<Contact> = list.iter().filter(|c| others.contains(&c.id) && c.id != keep).cloned().collect();
    let k = list.iter_mut().find(|c| c.id == keep).ok_or("Contacto no encontrado.")?;
    for o in &absorbed {
        let fill = |dst: &mut String, src: &str| {
            if dst.is_empty() {
                *dst = src.to_string();
            }
        };
        fill(&mut k.role, &o.role);
        fill(&mut k.company, &o.company);
        fill(&mut k.extension, &o.extension);
        fill(&mut k.reason, &o.reason);
        fill(&mut k.availability, &o.availability);
        fill(&mut k.substitute_id, &o.substitute_id);
        fill(&mut k.client_id, &o.client_id);
        if !o.notes.is_empty() && !k.notes.contains(&o.notes) {
            k.notes = [k.notes.as_str(), o.notes.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
        }
        let mut channels: Vec<Channel> = o.channels.clone();
        for (kind, label, value) in [("phone", "Teléfono", &o.phone), ("phone", "Móvil", &o.mobile), ("email", "Correo", &o.email)] {
            channels.push(Channel { kind: kind.into(), label: label.into(), value: value.clone() });
        }
        for ch in channels.into_iter().filter(|ch| !ch.value.is_empty()) {
            let v = ch.value.to_lowercase();
            let known = [&k.phone, &k.mobile, &k.email].iter().any(|x| x.to_lowercase() == v) || k.channels.iter().any(|x| x.value.to_lowercase() == v);
            if known {
                continue;
            }
            match ch.kind.as_str() {
                "email" if k.email.is_empty() => k.email = ch.value,
                "phone" if k.phone.is_empty() => k.phone = ch.value,
                "phone" if k.mobile.is_empty() && ch.label == "Móvil" => k.mobile = ch.value,
                _ => k.channels.push(ch),
            }
        }
        let mut tags = k.tags.clone();
        tags.extend(o.tags.iter().cloned());
        k.tags = clean_tags(&tags);
        k.favorite |= o.favorite;
        k.uses = k.uses.saturating_add(o.uses);
        k.last_used = k.last_used.max(o.last_used);
    }
    k.updated = now();
    let t = now();
    for c in list.iter_mut().filter(|c| others.contains(&c.id) && c.id != keep) {
        c.deleted = Some(t);
    }
    Ok(())
}

#[tauri::command]
pub fn merge_contacts(app: tauri::AppHandle, keep: String, others: Vec<String>) -> Result<(), String> {
    let _g = lock();
    let mut list = load(&app);
    merge(&mut list, &keep, &others)?;
    store(&app, &list)
}

// ---------- Etiquetas ----------

#[tauri::command]
pub fn contact_tag_colors(app: tauri::AppHandle) -> Vec<TagMeta> {
    crate::paths::read_json(&tags_path(&app))
}

#[tauri::command]
pub fn set_contact_tag_color(app: tauri::AppHandle, name: String, color: String) -> Result<(), String> {
    let _g = lock();
    let mut meta: Vec<TagMeta> = crate::paths::read_json(&tags_path(&app));
    meta.retain(|m| !m.name.eq_ignore_ascii_case(&name));
    if !color.is_empty() {
        meta.push(TagMeta { name: cut(&name, 40), color: cut(&color, 20) });
    }
    crate::paths::write_json(&tags_path(&app), &meta)
}

/// Renombra una etiqueta en todos los contactos. Si `to` ya existe, se fusionan.
fn rename_tag(list: &mut [Contact], from: &str, to: &str) -> usize {
    let mut n = 0;
    for c in list.iter_mut().filter(|c| c.tags.iter().any(|t| t.eq_ignore_ascii_case(from))) {
        let tags: Vec<String> = c.tags.iter().map(|t| if t.eq_ignore_ascii_case(from) { to.to_string() } else { t.clone() }).collect();
        c.tags = clean_tags(&tags);
        n += 1;
    }
    n
}

#[tauri::command]
pub fn rename_contact_tag(app: tauri::AppHandle, from: String, to: String) -> Result<usize, String> {
    let to = cut(to.trim_start_matches('#'), 40);
    if to.is_empty() {
        return Err("Escribe el nuevo nombre.".into());
    }
    let _g = lock();
    let mut list = load(&app);
    let n = rename_tag(&mut list, &from, &to);
    store(&app, &list)?;
    let mut meta: Vec<TagMeta> = crate::paths::read_json(&tags_path(&app));
    let color = meta.iter().find(|m| m.name.eq_ignore_ascii_case(&from)).map(|m| m.color.clone());
    meta.retain(|m| !m.name.eq_ignore_ascii_case(&from));
    if let Some(color) = color {
        if !meta.iter().any(|m| m.name.eq_ignore_ascii_case(&to)) {
            meta.push(TagMeta { name: to, color });
        }
    }
    crate::paths::write_json(&tags_path(&app), &meta)?;
    Ok(n)
}

#[tauri::command]
pub fn delete_contact_tag(app: tauri::AppHandle, name: String) -> Result<usize, String> {
    let _g = lock();
    let mut list = load(&app);
    let ids: Vec<String> = list.iter().map(|c| c.id.clone()).collect();
    let n = apply_bulk(&mut list, &ids, &Bulk::RemoveTag { tag: name.clone() });
    store(&app, &list)?;
    let mut meta: Vec<TagMeta> = crate::paths::read_json(&tags_path(&app));
    meta.retain(|m| !m.name.eq_ignore_ascii_case(&name));
    crate::paths::write_json(&tags_path(&app), &meta)?;
    Ok(n)
}

// ---------- Copias ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    id: u64,
    contacts: usize,
}

#[tauri::command]
pub fn list_contact_backups(app: tauri::AppHandle) -> Vec<BackupInfo> {
    backup_files(&app)
        .into_iter()
        .map(|(id, p)| BackupInfo { id, contacts: crate::paths::read_json::<Vec<Contact>>(&p).iter().filter(|c| c.deleted.is_none()).count() })
        .collect()
}

#[tauri::command]
pub fn backup_contacts_now(app: tauri::AppHandle) -> Result<(), String> {
    let _g = lock();
    backup(&app, true)
}

/// Vuelve a una copia. Antes guarda la agenda actual como otra copia (se puede volver).
#[tauri::command]
pub fn restore_contact_backup(app: tauri::AppHandle, id: u64) -> Result<usize, String> {
    let _g = lock();
    let (_, file) = backup_files(&app).into_iter().find(|(ts, _)| *ts == id).ok_or("Copia no encontrada.")?;
    let list: Vec<Contact> = crate::paths::read_json(&file);
    backup(&app, true)?;
    crate::paths::write_json(&path(&app), &list)?;
    Ok(list.iter().filter(|c| c.deleted.is_none()).count())
}

// ---------- Abrir correo, Teams y llamadas ----------

fn valid_email(e: &str) -> bool {
    e.len() < 160 && e.contains('@') && e.chars().all(|c| c.is_alphanumeric() || "@._-+".contains(c))
}

/// Teams, correo o teléfono se abren como el usuario, no como administrador.
fn open(uri: &str) -> Result<(), String> {
    crate::shellopen::open(uri)
}

// ¿Hay un programa registrado para este protocolo (msteams:, tel:…)?
use crate::shellopen::protocol_registered;

/// Enlace de Teams: la app si está instalada; si no, Teams en el navegador.
/// La «@» va codificada: sin codificar, el Explorador no lo reconoce como
/// dirección web y abre una carpeta (Documentos).
fn teams_uri(email: &str, call: bool, app: bool) -> String {
    let kind = if call { "call" } else { "chat" };
    let user = email.replace('@', "%40");
    if app {
        format!("msteams:/l/{kind}/0/0?users={user}")
    } else {
        format!("https://teams.microsoft.com/l/{kind}/0/0?users={user}")
    }
}

/// Abre el programa de correo con un mensaje nuevo para `email`.
#[tauri::command]
pub fn write_email(email: String) -> Result<(), String> {
    let e = email.trim();
    if !valid_email(e) {
        return Err("El correo no es válido.".into());
    }
    open(&format!("mailto:{e}"))
}

/// Chat o llamada de Teams con esa persona (por su correo).
#[tauri::command]
pub fn open_teams(email: String, call: bool) -> Result<(), String> {
    let e = email.trim();
    if !valid_email(e) {
        return Err("El correo no es válido.".into());
    }
    open(&teams_uri(e, call, protocol_registered("msteams")))
}

/// Llama con la aplicación de teléfono del equipo (Teams, Enlace Móvil, softphone…).
#[tauri::command]
pub fn call_number(number: String) -> Result<(), String> {
    let n: String = number.chars().filter(|c| c.is_ascii_digit() || *c == '+' || *c == '*' || *c == '#').collect();
    if n.is_empty() || n.len() > 30 {
        return Err("El número no es válido.".into());
    }
    open(&format!("tel:{n}"))
}

/// Guarda un vCard (.vcf) elegido por el usuario.
#[tauri::command(async)]
pub fn save_vcard(app: tauri::AppHandle, name: String, content: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_".contains(c) { c } else { '_' }).collect();
    let Some(file) = app.dialog().file().set_file_name(format!("{safe}.vcf")).add_filter("vCard", &["vcf"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::write(&file, content).map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(Some(file.display().to_string()))
}

// ---------- Importar: CSV ----------

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
        "disponibilidad" | "horario" | "availability" => "availability",
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
                Some("availability") => c.availability = v,
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

// ---------- Importar: vCard ----------

fn vcard_unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

/// Contactos de un .vcf (Outlook, Android, iPhone, Google).
fn contacts_from_vcard(text: &str) -> Vec<Contact> {
    // Las líneas que empiezan por espacio continúan la anterior.
    let mut lines: Vec<String> = Vec::new();
    for l in text.trim_start_matches('\u{feff}').lines() {
        match l.strip_prefix(' ').or_else(|| l.strip_prefix('\t')) {
            Some(rest) if !lines.is_empty() => lines.last_mut().expect("hay línea").push_str(rest),
            _ => lines.push(l.to_string()),
        }
    }
    let mut out = Vec::new();
    let mut cur: Option<(Contact, String)> = None;
    for l in lines {
        let upper = l.to_uppercase();
        if upper.starts_with("BEGIN:VCARD") {
            cur = Some((Contact::default(), String::new()));
            continue;
        }
        if upper.starts_with("END:VCARD") {
            if let Some((mut c, n)) = cur.take() {
                if c.name.is_empty() {
                    let parts: Vec<&str> = n.split(';').collect();
                    c.name = [parts.get(1).copied().unwrap_or(""), parts.first().copied().unwrap_or("")].join(" ").trim().to_string();
                }
                if let Ok(c) = clean(c) {
                    out.push(c);
                }
            }
            continue;
        }
        let Some((c, n)) = cur.as_mut() else { continue };
        let Some((key, value)) = l.split_once(':') else { continue };
        let mut params = key.split(';');
        let prop = params.next().unwrap_or("").to_uppercase();
        // Grupos de Apple («item1.TEL»).
        let prop = prop.rsplit('.').next().unwrap_or("").to_string();
        let params = params.collect::<Vec<_>>().join(";").to_uppercase();
        let value = vcard_unescape(value.trim());
        match prop.as_str() {
            "FN" => c.name = value,
            "N" => *n = value,
            "ORG" => c.company = value.split(';').next().unwrap_or("").to_string(),
            "TITLE" | "ROLE" if c.role.is_empty() => c.role = value,
            "NOTE" => c.notes = value,
            "CATEGORIES" => c.tags = value.split(',').map(|t| t.trim().to_string()).collect(),
            "EMAIL" => {
                if c.email.is_empty() {
                    c.email = value;
                } else {
                    c.channels.push(Channel { kind: "email".into(), label: "Correo".into(), value });
                }
            }
            "TEL" => {
                let cell = params.contains("CELL");
                if cell && c.mobile.is_empty() {
                    c.mobile = value;
                } else if !cell && c.phone.is_empty() {
                    c.phone = value;
                } else {
                    c.channels.push(Channel { kind: "phone".into(), label: if cell { "Móvil".into() } else { "Teléfono".into() }, value });
                }
            }
            _ => {}
        }
    }
    out
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

fn import_into(list: &mut Vec<Contact>, incoming: Vec<Contact>) -> ImportResult {
    let (mut added, mut updated) = (0, 0);
    for mut c in incoming {
        c.updated = now();
        if let Some(existing) = list.iter_mut().find(|x| x.deleted.is_none() && same(x, &c)) {
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
            fill(&mut existing.availability, c.availability);
            fill(&mut existing.notes, c.notes);
            let mut tags = existing.tags.clone();
            tags.extend(c.tags);
            existing.tags = clean_tags(&tags);
            updated += 1;
        } else if list.len() < MAX {
            c.id = new_id();
            c.created = now();
            list.push(c);
            added += 1;
        }
    }
    ImportResult { added, updated }
}

#[tauri::command(async)]
pub fn import_contacts(app: tauri::AppHandle) -> Result<Option<ImportResult>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Contactos (CSV, vCard)", &["csv", "txt", "vcf"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let bytes = std::fs::read(&file).map_err(|e| format!("No se pudo leer: {e}"))?;
    // Excel guarda a veces en Windows-1252: se aceptan los dos.
    let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
    let incoming = if text.trim_start_matches('\u{feff}').trim_start().to_uppercase().starts_with("BEGIN:VCARD") {
        let v = contacts_from_vcard(&text);
        if v.is_empty() {
            return Err("El archivo vCard no tiene contactos.".into());
        }
        v
    } else {
        contacts_from_csv(&text)?
    };
    let _g = lock();
    let mut list = load(&app);
    let r = import_into(&mut list, incoming);
    store(&app, &list)?;
    Ok(Some(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, name: &str) -> Contact {
        Contact { id: id.into(), name: name.into(), ..Default::default() }
    }

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
    fn imports_vcard() {
        let vcf = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:López;Ana;;;\r\nFN:Ana López\r\nORG:Empresa SA;Sistemas\r\nTITLE:Técnica\r\nTEL;TYPE=WORK:910000000\r\nTEL;TYPE=CELL:600000000\r\nTEL;TYPE=CELL:611111111\r\nEMAIL;TYPE=INTERNET:ana@empresa.com\r\nNOTE:Guardia los\r\n  lunes\\, martes\r\nCATEGORIES:TI,Urgencias\r\nEND:VCARD\r\nBEGIN:VCARD\r\nN:Pérez;Juan\r\nitem1.EMAIL:juan@x.com\r\nEND:VCARD\r\n";
        let v = contacts_from_vcard(vcf);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "Ana López");
        assert_eq!(v[0].company, "Empresa SA");
        assert_eq!(v[0].phone, "910000000");
        assert_eq!(v[0].mobile, "600000000");
        assert_eq!(v[0].channels.len(), 1);
        assert_eq!(v[0].notes, "Guardia los lunes, martes");
        assert_eq!(v[0].tags, vec!["TI", "Urgencias"]);
        assert_eq!(v[1].name, "Juan Pérez");
        assert_eq!(v[1].email, "juan@x.com");
    }

    #[test]
    fn rejects_unknown_columns_and_empty_contacts() {
        assert!(contacts_from_csv("a;b\n1;2\n").is_err());
        assert!(clean(Contact::default()).is_err());
        assert_eq!(clean(Contact { email: "x@y.z".into(), ..Default::default() }).unwrap().name, "x@y.z");
    }

    #[test]
    fn teams_links() {
        assert_eq!(teams_uri("ana@x.com", false, true), "msteams:/l/chat/0/0?users=ana%40x.com");
        assert_eq!(teams_uri("ana@x.com", true, false), "https://teams.microsoft.com/l/call/0/0?users=ana%40x.com");
        assert!(protocol_registered("mailto"));
        assert!(!protocol_registered("no-existe-adminops"));
    }

    #[test]
    fn tags_are_unique_and_clean() {
        assert_eq!(clean_tags(&["TI".into(), "#ti".into(), " Urgencias ".into(), "".into()]), vec!["TI", "Urgencias"]);
    }

    #[test]
    fn bulk_trash_restore_and_purge() {
        let mut l = vec![c("a", "Ana"), c("b", "Bea")];
        let ids = vec!["a".to_string()];
        assert_eq!(apply_bulk(&mut l, &ids, &Bulk::AddTag { tag: "TI".into() }), 1);
        assert_eq!(l[0].tags, vec!["TI"]);
        // Solo se borra para siempre lo que está en la papelera.
        assert_eq!(apply_bulk(&mut l, &ids, &Bulk::Purge), 0);
        apply_bulk(&mut l, &ids, &Bulk::Delete);
        assert!(l[0].deleted.is_some());
        apply_bulk(&mut l, &ids, &Bulk::Restore);
        assert!(l[0].deleted.is_none());
        apply_bulk(&mut l, &ids, &Bulk::Delete);
        assert_eq!(apply_bulk(&mut l, &ids, &Bulk::Purge), 1);
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn rename_merges_tags() {
        let mut l = vec![Contact { tags: vec!["Sistemas".into(), "TI".into()], ..c("a", "Ana") }];
        assert_eq!(rename_tag(&mut l, "sistemas", "TI"), 1);
        assert_eq!(l[0].tags, vec!["TI"]);
    }

    #[test]
    fn merge_keeps_everything() {
        let mut l = vec![
            Contact { email: "ana@x.com".into(), tags: vec!["TI".into()], uses: 2, ..c("a", "Ana López") },
            Contact { email: "ana.lopez@x.com".into(), extension: "2104".into(), phone: "910".into(), tags: vec!["Urgencias".into()], uses: 3, ..c("b", "Ana") },
        ];
        merge(&mut l, "a", &["b".into()]).unwrap();
        let a = &l[0];
        assert_eq!(a.extension, "2104");
        assert_eq!(a.phone, "910");
        assert_eq!(a.channels, vec![Channel { kind: "email".into(), label: "Correo".into(), value: "ana.lopez@x.com".into() }]);
        assert_eq!(a.tags, vec!["TI", "Urgencias"]);
        assert_eq!(a.uses, 5);
        assert!(l[1].deleted.is_some());
    }

    #[test]
    fn reimport_does_not_duplicate() {
        let mut l = vec![Contact { email: "ana@x.com".into(), ..c("a", "Ana") }];
        let r = import_into(&mut l, vec![Contact { email: "ana@x.com".into(), extension: "21".into(), ..c("", "ana") }]);
        assert_eq!((r.added, r.updated), (0, 1));
        assert_eq!(l[0].extension, "21");
    }
}
