//! Tareas largas visibles en la interfaz: emiten su progreso como evento
//! `task-progress` y se pueden cancelar con `cancel_task`.

use crate::ps;
use serde::Serialize;
use std::time::Duration;
use tauri::Emitter;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    task: &'a str,
    message: &'a str,
}

pub struct Task {
    app: Option<tauri::AppHandle>,
    key: String,
}

impl Task {
    pub fn new(app: &tauri::AppHandle, key: impl Into<String>) -> Self {
        let key = key.into();
        ps::finish_task(&key); // por si quedó marcada de una cancelación anterior
        Task { app: Some(app.clone()), key }
    }

    /// Tarea sin interfaz (tests, modo línea de comandos).
    pub fn detached(key: impl Into<String>) -> Self {
        Task { app: None, key: key.into() }
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
        ps::finish_task(&self.key);
    }
}

#[tauri::command]
pub fn cancel_task(task: String) -> bool {
    ps::cancel(&task)
}
