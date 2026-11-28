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
    /// Memorias USB: el modo que había (allowed | readonly | blocked).
    #[serde(rename_all = "camelCase")]
    UsbMode { previous: String },
    /// Energía: pantalla, suspensión y tapa que había, enchufado y con batería.
    #[serde(rename_all = "camelCase")]
    PowerSides { ac: crate::power::Side, dc: crate::power::Side },
    /// Programas predeterminados: la directiva que había (None = ninguna).
    #[serde(rename_all = "camelCase")]
    DefaultAppsPolicy { previous: Option<String> },
    /// Audio: el dispositivo predeterminado que había.
    #[serde(rename_all = "camelCase")]
    AudioDefault { id: String },
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
    /// La lista de idiomas de teclado del usuario. Deshacer = volver a ponerla (Set-WinUserLanguageList).
    #[serde(rename_all = "camelCase")]
    Languages { list: Vec<crate::kbdlang::LanguageEntry> },
    /// «Pausar OneDrive 2 h»: deshacer = borrar la tarea que lo iba a abrir y abrirlo ahora.
    #[serde(rename_all = "camelCase")]
    OneDrivePause { task: String, exe: String, sid: Option<String> },
    /// «Escanear a carpeta»: deshacer = quitar el recurso compartido, el usuario (si lo creó AdminOps) y lo que se
    /// abrió del cortafuegos; la carpeta se queda si tiene archivos.
    #[serde(rename_all = "camelCase")]
    ScanFolder {
        created_user: bool,
        firewall: Vec<String>,
        /// El recurso «Escaneos» ya existía (en la misma carpeta): al deshacer no se borra, se le devuelven sus permisos.
        #[serde(default)]
        share_existed: bool,
        #[serde(default)]
        share_access: Vec<crate::scanfolder::ShareAce>,
        /// La carpeta la creó AdminOps: solo entonces se borra al deshacer (y si está vacía).
        #[serde(default)]
        folder_created: bool,
        /// El usuario «escaner» ya existía y no era de AdminOps: cómo estaba (activado y su entrada para ocultarlo).
        /// Los diarios viejos no lo tienen: entonces al deshacer no se toca.
        #[serde(default)]
        user_prev: Option<crate::scanfolder::UserPrev>,
        /// El SID del usuario del equipo al que AdminOps dio permiso en la carpeta (no lo tenía antes). Sin él, no se quita nada.
        #[serde(default)]
        client_sid: Option<String>,
    },
    /// La firma SMB obligatoria que se quitó. Deshacer = volver a ponerla.
    #[serde(rename_all = "camelCase")]
    SmbSigning { previous: bool },
    /// La red que se puso como privada. Deshacer = volver a su categoría.
    #[serde(rename_all = "camelCase")]
    NetworkCategory { interface_index: u32, previous: String },
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
    /// Lo que midió la acción antes y después («C: 3,1 GB → 24,8 GB libres»). Los
    /// diarios de antes no lo tienen: se leen igual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<crate::measure::Measure>,
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
        let tmp = crate::paths::tmp_for(&self.path);
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
        measure: None,
    }
}
