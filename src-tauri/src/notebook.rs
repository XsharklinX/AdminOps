//! «Notas»: lo que se apunta con la nota rápida (Ctrl+Alt+N o el botón de la barra del cohete), guardado en un
//! cuaderno que se ve entero en Conocimiento → Notas. Son datos de AdminOps (no cambian el equipo).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub text: String,
    /// Milisegundos desde 1970.
    pub created: u64,
    pub updated: u64,
    pub pinned: bool,
    /// Imágenes pegadas en la nota (capturas), como `data:image/…;base64,…`.
    pub images: Vec<String>,
}

/// Como mucho 6 imágenes por nota y 3 MB cada una: el cuaderno es un solo archivo y se lee entero.
const MAX_IMAGES: usize = 6;
const MAX_IMAGE: usize = 3 * 1024 * 1024;

fn valid_images(images: &[String]) -> Result<(), String> {
    if images.len() > MAX_IMAGES {
        return Err(format!("Como mucho {MAX_IMAGES} imágenes por nota."));
    }
    if images.iter().any(|i| !i.starts_with("data:image/") || i.len() > MAX_IMAGE) {
        return Err("Una de las imágenes no vale o es demasiado grande (3 MB como mucho).".into());
    }
    Ok(())
}

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("notas.json")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Las notas para cambiarlas. Si `notas.json` existe y no se entiende (dañado), se aparta a `notas-dañado-….json`
/// antes de seguir: así guardar una nota nueva no pisa las demás. Cifrado y bloqueado, error (nada se toca).
fn load_for_write(p: &std::path::Path) -> Result<Vec<Note>, String> {
    let Ok(bytes) = std::fs::read(p) else { return Ok(Vec::new()) };
    let plain = crate::datacrypt::read(bytes).ok_or("Las notas están cifradas y bloqueadas: desbloquéalas en Ajustes → Seguridad.")?;
    match serde_json::from_slice(&plain) {
        Ok(v) => Ok(v),
        Err(e) => {
            let aside = p.with_file_name(format!("notas-dañado-{}.json", now_ms()));
            std::fs::rename(p, &aside).map_err(|err| format!("El cuaderno de notas está dañado y no se pudo apartar: {err}"))?;
            log::warn!("notas.json dañado ({e}); apartado en {}", aside.display());
            Ok(Vec::new())
        }
    }
}

/// Fijadas primero; luego la última tocada.
pub fn sorted(mut v: Vec<Note>) -> Vec<Note> {
    v.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated.cmp(&a.updated)));
    v
}

/// Para buscar (Ctrl+K): las notas sin sus imágenes, que pueden pesar mucho.
#[tauri::command]
pub fn notes_index(app: tauri::AppHandle) -> Vec<Note> {
    notes_list(app).into_iter().map(|n| Note { images: Vec::new(), ..n }).collect()
}

#[tauri::command]
pub fn notes_list(app: tauri::AppHandle) -> Vec<Note> {
    sorted(crate::paths::read_json(&path(&app)))
}

/// Crea (sin id) o guarda una nota. Una nota sin título ni texto no se guarda.
#[tauri::command]
pub fn notes_save(app: tauri::AppHandle, note: Note) -> Result<Note, String> {
    if note.title.trim().is_empty() && note.text.trim().is_empty() && note.images.is_empty() {
        return Err("La nota está vacía.".into());
    }
    if note.text.len() > 200_000 {
        return Err("La nota es demasiado larga.".into());
    }
    valid_images(&note.images)?;
    let p = path(&app);
    let mut all = load_for_write(&p)?;
    let t = now_ms();
    let saved = match all.iter_mut().find(|n| !note.id.is_empty() && n.id == note.id) {
        Some(n) => {
            n.title = note.title.trim().to_string();
            n.text = note.text;
            n.images = note.images;
            n.pinned = note.pinned;
            n.updated = t;
            n.clone()
        }
        None => {
            let n = Note { id: format!("n{t}"), title: note.title.trim().to_string(), text: note.text, created: t, updated: t, pinned: note.pinned, images: note.images };
            all.push(n.clone());
            n
        }
    };
    crate::paths::write_json(&p, &all)?;
    Ok(saved)
}

#[tauri::command]
pub fn notes_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let p = path(&app);
    let mut all = load_for_write(&p)?;
    all.retain(|n| n.id != id);
    crate::paths::write_json(&p, &all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fijadas_primero_y_luego_lo_ultimo() {
        let n = |id: &str, updated: u64, pinned: bool| Note { id: id.into(), updated, pinned, ..Default::default() };
        let v = sorted(vec![n("a", 1, false), n("b", 5, false), n("c", 2, true)]);
        assert_eq!(v.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["c", "b", "a"]);
    }
}
