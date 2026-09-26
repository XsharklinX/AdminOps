//! Puntos de restauración del sistema.

use crate::ps;
use serde::{Deserialize, Serialize};

/// Crea un punto de restauración evitando las dos trampas de Windows:
/// - Por defecto solo permite uno cada 24 h (`SystemRestorePointCreationFrequency`):
///   lo ponemos a 0 durante la llamada y lo dejamos como estaba.
/// - Si la Protección del sistema está desactivada en C:, la activamos.
pub fn create(description: &str, task: &crate::task::Task) -> Result<(), String> {
    let desc = description.replace('\'', "''");
    let script = format!(
        r#"
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
$old = (Get-ItemProperty $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue).SystemRestorePointCreationFrequency
Set-ItemProperty $k -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord
try {{
  Enable-ComputerRestore -Drive "$env:SystemDrive\"
  Checkpoint-Computer -Description '{desc}' -RestorePointType MODIFY_SETTINGS
}} finally {{
  if ($null -eq $old) {{ Remove-ItemProperty $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue }}
  else {{ Set-ItemProperty $k -Name SystemRestorePointCreationFrequency -Value $old -Type DWord }}
}}
"#
    );
    ps::powershell_opts(&script, task.opts(Some(std::time::Duration::from_secs(10 * 60)))).map(|_| ())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RestorePoint {
    #[serde(rename(deserialize = "SequenceNumber"))]
    pub sequence: u32,
    #[serde(rename(deserialize = "Description"))]
    pub description: String,
    /// ISO 8601
    #[serde(rename(deserialize = "Created"))]
    pub created: String,
}

pub fn list() -> Result<Vec<RestorePoint>, String> {
    let out = ps::powershell(
        r#"
$p = @(Get-ComputerRestorePoint | Sort-Object SequenceNumber -Descending | Select-Object SequenceNumber, Description,
  @{n='Created'; e={ $_.ConvertToDateTime($_.CreationTime).ToString('o') }})
ConvertTo-Json -InputObject $p -Compress
"#,
    )?;
    if out.is_empty() {
        return Ok(vec![]);
    }
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))
}

/// Abre el asistente "Restaurar sistema" de Windows.
pub fn open_wizard() -> Result<(), String> {
    std::process::Command::new("rstrui.exe")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("No se pudo abrir Restaurar sistema: {e}"))
}
