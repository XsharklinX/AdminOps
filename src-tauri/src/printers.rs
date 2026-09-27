//! Impresoras: estado, trabajos en cola, vaciar la cola, página de prueba,
//! predeterminada y quitar impresoras (las "fantasma" de equipos o drivers viejos).

use crate::ps::text_var;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Printer {
    name: String,
    driver: Option<String>,
    port: Option<String>,
    default: bool,
    network: bool,
    shared: bool,
    offline: bool,
    /// Código de Win32_Printer.PrinterStatus (3 = inactiva, 4 = imprimiendo, 7 = sin conexión…).
    status: u32,
    /// Estado de error detectado (atasco, sin papel, sin tóner…), si lo hay.
    error: Option<String>,
    jobs: u32,
    /// Impresoras de software: PDF, XPS, OneNote, Fax.
    #[serde(rename = "virtual")]
    is_virtual: bool,
}

const LIST_SCRIPT: &str = r#"
$errors = @{ 1='Otro error'; 2='Sin papel'; 3='Poco papel'; 4='Sin papel'; 5='Poco tóner'; 6='Sin tóner'; 7='Puerta abierta'; 8='Atasco de papel'; 9='Sin conexión'; 10='Requiere atención'; 11='Bandeja de salida llena' }
$r = @(Get-CimInstance Win32_Printer -ErrorAction Stop | ForEach-Object {
  $jobs = 0
  try { $jobs = @(Get-PrintJob -PrinterName $_.Name -ErrorAction Stop).Count } catch {}
  $err = [int]$_.DetectedErrorState
  [pscustomobject]@{
    name = $_.Name; driver = $_.DriverName; port = $_.PortName
    default = [bool]$_.Default; network = [bool]$_.Network; shared = [bool]$_.Shared
    offline = [bool]$_.WorkOffline -or [int]$_.PrinterStatus -eq 7
    status = [int]$_.PrinterStatus
    error = if ($err -gt 2 -or $err -eq 1) { $errors[$err] } else { $null }
    jobs = $jobs
    virtual = [bool]($_.PortName -match '^(PORTPROMPT:|nul:|SHRFAX:|XPSPort:)' -or $_.DriverName -match 'Microsoft (Print To PDF|XPS Document Writer)|OneNote|Fax')
  }
})
ConvertTo-Json -InputObject $r -Compress
"#;

#[tauri::command(async)]
pub fn list_printers() -> Result<Vec<Printer>, String> {
    let out = crate::ps::powershell_opts(LIST_SCRIPT, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None })?;
    let mut v: Vec<Printer> = serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    v.sort_by_key(|p| (p.is_virtual, !p.default, p.name.to_lowercase()));
    Ok(v)
}

/// Ejecuta una acción sobre la impresora `name` (el nombre viaja en base64).
fn on_printer(name: &str, body: &str, title: &str, tweaks: &TweakState) -> Result<(), String> {
    if name.is_empty() || name.len() > 300 {
        return Err("Nombre de impresora no válido.".into());
    }
    let script = format!(
        "{}$p = Get-CimInstance Win32_Printer | Where-Object {{ $_.Name -eq $name }}\n\
         if (-not $p) {{ throw 'La impresora ya no existe. Actualiza la lista.' }}\n{body}\n'ok'",
        text_var("name", name)
    );
    let result = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None }).map(|_| ());
    tweaks.record(Op::Run, &format!("{title}: {name}"), &result);
    result
}

#[tauri::command(async)]
pub fn clear_printer_queue(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    on_printer(&name, "Get-PrintJob -PrinterName $name | Remove-PrintJob", "Vaciar la cola de impresión", &tweaks)
}

#[tauri::command(async)]
pub fn print_test_page(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    on_printer(
        &name,
        "$r = Invoke-CimMethod -InputObject $p -MethodName PrintTestPage\n\
         if ($r.ReturnValue -ne 0) { throw \"Windows no pudo enviar la página de prueba (código $($r.ReturnValue)).\" }",
        "Página de prueba",
        &tweaks,
    )
}

#[tauri::command(async)]
pub fn set_default_printer(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    // Windows 10/11 cambia solo la predeterminada si "Permitir que Windows administre" está activo.
    on_printer(
        &name,
        "Set-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows NT\\CurrentVersion\\Windows' -Name LegacyDefaultPrinterMode -Value 1 -Type DWord\n\
         $r = Invoke-CimMethod -InputObject $p -MethodName SetDefaultPrinter\n\
         if ($r.ReturnValue -ne 0) { throw \"Windows no pudo cambiar la predeterminada (código $($r.ReturnValue)).\" }",
        "Impresora predeterminada",
        &tweaks,
    )
}

#[tauri::command(async)]
pub fn remove_printer(name: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    on_printer(
        &name,
        "Get-PrintJob -PrinterName $name -ErrorAction SilentlyContinue | Remove-PrintJob -ErrorAction SilentlyContinue\nRemove-Printer -Name $name",
        "Quitar impresora",
        &tweaks,
    )
}

#[cfg(test)]
mod tests {
    /// Solo lectura: lista las impresoras reales.
    #[test]
    fn lists_real_printers() {
        let v = super::list_printers().unwrap();
        println!("{v:#?}");
        assert!(v.iter().any(|p| p.is_virtual), "Microsoft Print to PDF suele existir siempre");
    }

    #[test]
    fn embedded_scripts_parse() {
        let errors = crate::ps::parse_errors(super::LIST_SCRIPT);
        assert!(errors.is_empty(), "LIST_SCRIPT: {errors}");
    }
}
