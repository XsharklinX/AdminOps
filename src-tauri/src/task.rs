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

/// Inicio y fin de una tarea, para el indicador de tareas de la barra superior.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Lifecycle<'a> {
    task: &'a str,
    name: &'a str,
    /// Solo al terminar.
    seconds: u64,
    cancelled: bool,
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
        "diagnostics" => "Diagnóstico",
        "app-backup" => "Copia cifrada de AdminOps",
        "encrypt" | "decrypt" | "vault" => "Caja fuerte",
        "disk-rescue" => "Rescate de archivos",
        "disk-scan" => "Mapa de superficie",
        "drivers-restore" => "Restaurar drivers",
        "inventory" => "Inventario",
        "lan-scan" | "lan-identify" => "Búsqueda en la red",
        "recover" | "recover-install" => "Recuperar archivos",
        "remote-install" => "Instalar herramienta remota",
        "update" => "Actualización de AdminOps",
        "wipe" | "wipe-free" => "Borrado seguro",
        "wu-search" => "Buscar actualizaciones de Windows",
        "uninstall" => "Desinstalar",
        "space" => "Análisis de espacio",
        "speedtest" => "Prueba de velocidad",
        _ => "Tarea",
    }
}

impl Task {
    pub fn new(app: &tauri::AppHandle, key: impl Into<String>) -> Self {
        let key = key.into();
        ps::finish_task(&key); // por si quedó marcada de una cancelación anterior
        let task = Task { app: Some(app.clone()), name: default_name(&key).into(), key, started: Instant::now() };
        task.announce();
        task
    }

    fn announce(&self) {
        if let Some(app) = &self.app {
            let _ = app.emit("task-started", Lifecycle { task: &self.key, name: &self.name, seconds: 0, cancelled: false });
        }
    }

    /// Tarea sin interfaz (tests, modo línea de comandos).
    pub fn detached(key: impl Into<String>) -> Self {
        let key = key.into();
        Task { app: None, name: default_name(&key).into(), key, started: Instant::now() }
    }

    /// Nombre que verá el usuario en la notificación de "terminado".
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        // Se vuelve a anunciar con el nombre bueno («Desinstalar VLC» en vez de «Tarea»).
        self.announce();
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
            let _ = app.emit("task-finished", Lifecycle { task: &self.key, name: &self.name, seconds: elapsed.as_secs(), cancelled });
            if elapsed >= NOTIFY_AFTER && crate::workflow::settings(app).notify_tasks {
                notify_done(app, &self.name, elapsed, cancelled);
            }
        }
    }
}

fn human(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 60 { format!("{} min {} s", s / 60, s % 60) } else { format!("{s} s") }
}

/// Para lo largo que no va por `Task` (análisis de espacio, prueba de velocidad):
/// el mismo aviso, con las mismas reglas (más de 20 s, activado en Ajustes, y
/// solo si el técnico está en otra ventana).
pub fn notify_if_long(app: &tauri::AppHandle, name: &str, elapsed: Duration, cancelled: bool) {
    if elapsed >= NOTIFY_AFTER && crate::workflow::settings(app).notify_tasks {
        notify_done(app, name, elapsed, cancelled);
    }
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
