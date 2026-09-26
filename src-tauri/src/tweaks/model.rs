//! Esquema declarativo de un ajuste ("tweak"). Los ajustes viven en
//! `src-tauri/tweaks/*.toml` y se incrustan en el binario al compilar.
//!
//! Un ajuste combina cualquier número de acciones de registro y de servicio
//! (que el motor sabe detectar, respaldar y revertir solo) y, como vía de
//! escape, un script de PowerShell con `detect`/`apply`/`revert` o `run`.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Estado persistente que se puede activar y desactivar.
    #[default]
    Toggle,
    /// Tarea puntual (limpiar, vaciar caché…). No tiene estado ni se deshace.
    Action,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum RegData {
    Int(i64),
    Str(String),
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RegKind {
    Dword,
    Qword,
    String,
    Expand,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RegistryAction {
    /// Ruta completa, p. ej. `HKLM\SOFTWARE\Policies\...` o `HKCU\Software\...`.
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: RegKind,
    pub value: RegData,
    /// Valor de fábrica de Windows. Si falta, revertir sin historial borra el valor.
    #[serde(default)]
    pub default: Option<RegData>,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Startup {
    Automatic,
    Delayed,
    Manual,
    Disabled,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ServiceAction {
    pub name: String,
    pub startup: Startup,
    /// Tipo de inicio de fábrica, usado si no hay copia de seguridad.
    pub default: Startup,
    /// Detener el servicio al aplicar (además de cambiar su inicio).
    #[serde(default)]
    pub stop: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct ScriptAction {
    /// Debe imprimir `True` si el ajuste ya está aplicado.
    pub detect: Option<String>,
    /// Su última línea de salida se guarda y `revert` la recibe como `$Previous`.
    pub apply: Option<String>,
    /// `$Previous` es `$null` si se revierte sin historial.
    pub revert: Option<String>,
    /// Para `kind = "action"`: lo que se ejecuta. Su última línea se muestra al usuario.
    pub run: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Tweak {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub risk: Risk,
    #[serde(default)]
    pub kind: Kind,
    /// Aviso visible (efectos secundarios, ediciones de Windows donde no aplica…).
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub reboot: bool,
    #[serde(default)]
    pub min_build: u32,
    #[serde(default)]
    pub max_build: Option<u32>,
    /// Forzar que requiera admin (para scripts; registro HKLM y servicios lo infieren solos).
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub registry: Vec<RegistryAction>,
    #[serde(default)]
    pub service: Vec<ServiceAction>,
    #[serde(default)]
    pub script: Option<ScriptAction>,
}

impl Tweak {
    pub fn needs_admin(&self) -> bool {
        self.admin
            || !self.service.is_empty()
            || self.registry.iter().any(|r| !r.path.to_ascii_uppercase().starts_with("HKCU"))
    }

    pub fn supported_on(&self, build: u32) -> bool {
        build >= self.min_build && self.max_build.is_none_or(|max| build <= max)
    }
}

#[derive(Deserialize)]
pub struct CatalogFile {
    pub tweak: Vec<Tweak>,
}
