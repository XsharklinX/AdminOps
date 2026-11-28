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
    /// Hora (ms desde 1970) a la que AdminOps avisa de la nota. Al avisar se quita.
    pub remind_at: Option<u64>,
}

/// Para que guardar desde la interfaz y quitar el aviso desde el hilo no se pisen.
static FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Las notas cuyo aviso ya venció: les quita el aviso y las devuelve.
fn take_due(all: &mut [Note], at: u64) -> Vec<Note> {
    let mut out = Vec::new();
    for n in all.iter_mut() {
        if n.remind_at.is_some_and(|r| r <= at) {
            n.remind_at = None;
            out.push(n.clone());
        }
    }
    out
}

fn notify(app: &tauri::AppHandle, n: &Note) {
    use tauri_plugin_notification::NotificationExt;
    let title = if !n.title.trim().is_empty() { n.title.trim().to_string() } else { n.text.lines().next().unwrap_or("").trim().to_string() };
    let title = if title.is_empty() { "Nota".to_string() } else { title.chars().take(120).collect() };
    let body = "Aviso de tu cuaderno de notas";
    let handle = app.clone();
    // Al pulsar el aviso se abre Conocimiento → Notas.
    let shown = crate::toast::show(app, &title, body, &[], move |_| {
        use tauri::Emitter;
        // En «solo el cohete» no hay ventana principal: se abre AdminOps entera (este proceso se cierra).
        if crate::rocketplus::rocket_only() {
            let _ = crate::rocketplus::open_full(&handle);
            return;
        }
        crate::tray::show_main(&handle);
        let _ = handle.emit("tray-action", "notebook");
    });
    if !shown {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

fn tick(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _x = crate::paths::CrossProcessLock::acquire("AdminOpsNotas");
    let p = path(app);
    let at = now_ms();
    let peek: Vec<Note> = crate::paths::read_json(&p);
    if !peek.iter().any(|n| n.remind_at.is_some_and(|r| r <= at)) {
        return;
    }
    // Con las notas cifradas y bloqueadas no se toca nada: se reintenta en la siguiente pasada.
    let Ok(mut all) = load_for_write(&p) else { return };
    let due = take_due(&mut all, at);
    if due.is_empty() {
        return;
    }
    if let Err(e) = crate::paths::write_json(&p, &all) {
        log::warn!("Notas: no se pudo quitar el aviso: {e}");
        return;
    }
    drop(_x);
    drop(_g);
    for n in &due {
        notify(app, n);
    }
    let _ = app.emit("notes-changed", ());
}

/// Vigila los avisos de las notas mientras AdminOps está abierta (cada 30 s).
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(15));
        loop {
            // En «solo el cohete» avisa únicamente si la app completa no está abierta (si lo está, avisa ella).
            if !crate::rocketplus::rocket_only() || !crate::rocketplus::full_running() {
                tick(&app);
            }
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    });
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
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _x = crate::paths::CrossProcessLock::acquire("AdminOpsNotas");
    let p = path(&app);
    let mut all = load_for_write(&p)?;
    let t = now_ms();
    let saved = match all.iter_mut().find(|n| !note.id.is_empty() && n.id == note.id) {
        Some(n) => {
            n.title = note.title.trim().to_string();
            n.text = note.text;
            n.images = note.images;
            n.pinned = note.pinned;
            // Un aviso ya pasado que llega desde la interfaz es una copia vieja (el hilo ya avisó): no vuelve a sonar.
            n.remind_at = note.remind_at.filter(|r| *r > t);
            n.updated = t;
            n.clone()
        }
        None => {
            let n = Note { id: format!("n{t}"), title: note.title.trim().to_string(), text: note.text, created: t, updated: t, pinned: note.pinned, images: note.images, remind_at: note.remind_at.filter(|r| *r > t) };
            all.push(n.clone());
            n
        }
    };
    crate::paths::write_json(&p, &all)?;
    Ok(saved)
}

#[tauri::command]
pub fn notes_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _x = crate::paths::CrossProcessLock::acquire("AdminOpsNotas");
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

    #[test]
    fn avisos_vencidos_se_quitan() {
        let n = |id: &str, r: Option<u64>| Note { id: id.into(), remind_at: r, ..Default::default() };
        let mut v = vec![n("a", Some(100)), n("b", Some(900)), n("c", None)];
        let due = take_due(&mut v, 500);
        assert_eq!(due.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["a"]);
        assert_eq!(v[0].remind_at, None);
        assert_eq!(v[1].remind_at, Some(900));
    }

    #[test]
    fn notas_antiguas_sin_aviso_se_leen() {
        let n: Note = serde_json::from_str(r#"{"id":"x","title":"t"}"#).unwrap();
        assert_eq!(n.remind_at, None);
    }
}
