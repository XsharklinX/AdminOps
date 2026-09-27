//! Diario de cambios persistente (`%APPDATA%\com.adminops.app\journal.json`).
//!
//! Cada aplicación guarda el estado previo exacto de lo que toca, así "Deshacer"
//! restaura el valor real que tenía ese PC, no un valor de fábrica supuesto.

use super::model::Startup;
use super::registry::RawValue;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 1000;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Backup {
    #[serde(rename_all = "camelCase")]
    Registry { path: String, name: String, previous: Option<RawValue> },
    #[serde(rename_all = "camelCase")]
    Service { name: String, startup: Startup, was_running: bool },
    /// El ajuste incluía un script: al deshacer se ejecuta su `revert`, que
    /// recibe en `$Previous` la última línea que imprimió `apply` (p. ej. el plan
    /// de energía que estaba activo).
    Script {
        #[serde(default)]
        previous: Option<String>,
    },
    /// Tarea programada habilitada/deshabilitada.
    #[serde(rename_all = "camelCase")]
    Task { path: String, name: String, was_enabled: bool },
    /// App (Appx) quitada. Deshacer = reinstalar desde Microsoft Store con winget.
    #[serde(rename_all = "camelCase")]
    Appx { name: String, store_id: Option<String> },
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    Apply,
    Revert,
    Run,
    RestorePoint,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u64,
    /// Segundos desde epoch (UTC).
    pub timestamp: u64,
    pub op: Op,
    pub tweak_id: Option<String>,
    pub title: String,
    pub ok: bool,
    pub message: Option<String>,
    #[serde(default)]
    pub backups: Vec<Backup>,
    /// Para `Apply`: ya se deshizo (desde el ajuste o desde el historial).
    #[serde(default)]
    pub reverted: bool,
    /// Se puede deshacer desde el historial (se calcula al guardar).
    #[serde(default)]
    pub undoable: bool,
}

pub struct Journal {
    path: PathBuf,
    entries: Vec<Entry>,
}

impl Journal {
    pub fn load(path: PathBuf) -> Self {
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, entries }
    }

    fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Escritura atómica: un corte de luz no debe dejar el diario corrupto.
        let tmp = self.path.with_extension("json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(&self.entries) {
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    pub fn push(&mut self, mut e: Entry) -> u64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let last = self.entries.last().map_or(0, |l| l.id);
        e.id = (now.as_millis() as u64).max(last + 1);
        e.timestamp = now.as_secs();
        e.undoable = e.op == Op::Apply
            && e.ok
            && !e.backups.is_empty()
            && !e.backups.iter().any(|b| matches!(b, Backup::Appx { store_id: None, .. }));
        let id = e.id;
        self.entries.push(e);
        if self.entries.len() > MAX_ENTRIES {
            let extra = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..extra);
        }
        self.save();
        id
    }

    /// Última aplicación exitosa y aún no deshecha de un ajuste.
    pub fn pending_apply(&self, tweak_id: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.op == Op::Apply && e.ok && !e.reverted && e.tweak_id.as_deref() == Some(tweak_id))
    }

    pub fn get(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn mark_reverted(&mut self, id: u64) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.reverted = true;
            self.save();
        }
    }

    pub fn newest_first(&self) -> Vec<Entry> {
        self.entries.iter().rev().cloned().collect()
    }

    /// Borra las entradas anteriores a `ts`, salvo los ajustes aplicados que aún se
    /// pueden deshacer (guardan los valores originales). Devuelve cuántas borró.
    pub fn prune_before(&mut self, ts: u64) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| e.timestamp >= ts || (e.op == Op::Apply && e.ok && !e.reverted));
        let removed = before - self.entries.len();
        if removed > 0 {
            self.save();
        }
        removed
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

pub fn entry(op: Op, tweak_id: Option<&str>, title: &str) -> Entry {
    Entry {
        id: 0,
        timestamp: 0,
        op,
        tweak_id: tweak_id.map(str::to_string),
        title: title.to_string(),
        ok: true,
        message: None,
        backups: Vec::new(),
        reverted: false,
        undoable: false,
    }
}
