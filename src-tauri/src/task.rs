//! Tareas largas visibles en la interfaz: emiten su progreso como evento
//! `task-progress` y se pueden cancelar con `cancel_task`.

use crate::ps;
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

/// Las tareas que duran más que esto avisan con una notificación de Windows al
/// terminar, si la ventana de AdminOps no está en primer plano.
const NOTIFY_AFTER: Duration = Duration::from_secs(20);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    task: &'a str,
    message: &'a str,
}

pub struct Task {
    app: Option<tauri::AppHandle>,
    key: String,
    /// Nombre para la notificación ("Instalación de programas", "Comprobar archivos (SFC)"…).
    name: String,
    started: Instant,
}

/// Nombre por defecto según la clave de la tarea.
fn default_name(key: &str) -> &'static str {
    match key.split(':').next().unwrap_or("") {
        "install-apps" => "Instalación de programas",
        "software" => "Actualización de programas",
        "migrate" => "Copia de datos",
        "drivers-backup" => "Copia de drivers",
        "apps" => "Quitar aplicaciones",
        "reinstall" => "Reinstalar aplicación",
        "restore-point" => "Punto de restauración",
        "profile" => "Aplicar perfil",
        "session" => "Sesión de servicio",
        "pawnio" => "Instalación de PawnIO",
        _ => "Tarea",
    }
}

impl Task {
    pub fn new(app: &tauri::AppHandle, key: impl Into<String>) -> Self {
        let key = key.into();
        ps::finish_task(&key); // por si quedó marcada de una cancelación anterior
        Task { app: Some(app.clone()), name: default_name(&key).into(), key, started: Instant::now() }
    }

    /// Tarea sin interfaz (tests, modo línea de comandos).
    pub fn detached(key: impl Into<String>) -> Self {
        let key = key.into();
        Task { app: None, name: default_name(&key).into(), key, started: Instant::now() }
    }

    /// Nombre que verá el usuario en la notificación de "terminado".
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Informa de la etapa actual a la interfaz y al registro.
    pub fn step(&self, message: impl AsRef<str>) {
        let message = message.as_ref();
        log::info!("[{}] {message}", self.key);
        if let Some(app) = &self.app {
            let _ = app.emit("task-progress", Progress { task: &self.key, message });
        }
    }

    pub fn opts(&self, timeout: Option<Duration>) -> ps::Opts<'_> {
        ps::Opts::task(&self.key, timeout)
    }

    pub fn cancelled(&self) -> bool {
        ps::is_cancelled(&self.key)
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        let cancelled = ps::is_cancelled(&self.key);
        ps::finish_task(&self.key);
        if let Some(app) = &self.app {
            let elapsed = self.started.elapsed();
            if elapsed >= NOTIFY_AFTER {
                notify_done(app, &self.name, elapsed, cancelled);
            }
        }
    }
}

fn human(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 60 { format!("{} min {} s", s / 60, s % 60) } else { format!("{s} s") }
}

/// Notificación de Windows, solo si el técnico está en otra ventana.
fn notify_done(app: &tauri::AppHandle, name: &str, elapsed: Duration, cancelled: bool) {
    use tauri_plugin_notification::NotificationExt;
    let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    if focused {
        return;
    }
    let body = if cancelled { format!("{name}: cancelada.") } else { format!("{name}: terminada ({}).", human(elapsed)) };
    if let Err(e) = app.notification().builder().title("AdminOps").body(body).show() {
        log::warn!("No se pudo mostrar la notificación: {e}");
    }
}

#[tauri::command]
pub fn cancel_task(task: String) -> bool {
    ps::cancel(&task)
}
