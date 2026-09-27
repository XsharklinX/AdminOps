//! Control de la Wi-Fi: estado real de la tarjeta (con el problema explicado),
//! encender/apagar la radio, reiniciar la tarjeta (arregla muchos «código 10»),
//! quitar adaptadores fantasma y evitar que Windows la apague para ahorrar energía.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WifiAdapter {
    pub name: String,
    pub description: String,
    pub instance_id: String,
    /// El dispositivo está conectado físicamente (no es un fantasma de una instalación anterior).
    pub present: bool,
    /// Up | Disconnected | Disabled | Not Present…
    pub status: String,
    /// Código de error de Windows (0: sin problema).
    pub problem: u32,
    pub problem_text: String,
    /// Windows puede apagarla para ahorrar energía.
    pub power_saving: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WifiState {
    pub adapters: Vec<WifiAdapter>,
    /// Estado de la radio (el interruptor de Configuración). None: no se pudo leer.
    pub radio_on: Option<bool>,
    pub service_running: bool,
    /// Resumen en lenguaje claro de lo que pasa.
    pub summary: String,
    /// ok | off | error | none
    pub level: String,
}

const STATE_SCRIPT: &str = r#"
$wlan = Get-Service WlanSvc -ErrorAction SilentlyContinue
$pnp = @(Get-CimInstance Win32_PnPEntity -Filter "PNPClass = 'Net'" -ErrorAction SilentlyContinue | Where-Object { "$($_.Name)" -match 'Wireless|Wi-?Fi|WLAN|802\.11' })
$present = @{}
Get-PnpDevice -Class Net -ErrorAction SilentlyContinue | ForEach-Object { $present["$($_.InstanceId)"] = [bool]$_.Present }
$nets = @(Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue)
$adapters = @(foreach ($d in $pnp) {
  $n = $nets | Where-Object { "$($_.PnPDeviceID)" -eq "$($d.PNPDeviceID)" } | Select-Object -First 1
  $pm = if ($n) { Get-NetAdapterPowerManagement -Name $n.Name -ErrorAction SilentlyContinue } else { $null }
  [pscustomobject]@{
    name = if ($n) { "$($n.Name)" } else { "$($d.Name)" }
    description = "$($d.Name)"
    instanceId = "$($d.PNPDeviceID)"
    present = if ($present.ContainsKey("$($d.PNPDeviceID)")) { $present["$($d.PNPDeviceID)"] } else { $true }
    status = if ($n) { "$($n.Status)" } else { '' }
    problem = [int]$d.ConfigManagerErrorCode
    powerSaving = if ($pm -and "$($pm.AllowComputerToTurnOffDevice)" -ne 'Unsupported') { "$($pm.AllowComputerToTurnOffDevice)" -eq 'Enabled' } else { $null }
  }
})
[pscustomobject]@{ adapters = $adapters; serviceRunning = [bool]($wlan -and "$($wlan.Status)" -eq 'Running') } | ConvertTo-Json -Depth 4 -Compress
"#;

/// Interruptor de radio de Windows (el mismo de Configuración) mediante WinRT.
const RADIO_SCRIPT: &str = r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = @([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, $type) { $t = $asTask.MakeGenericMethod($type).Invoke($null, @($op)); [void]$t.Wait(10000); $t.Result }
[void][Windows.Devices.Radios.Radio, Windows.System.Devices, ContentType = WindowsRuntime]
[void](Await ([Windows.Devices.Radios.Radio]::RequestAccessAsync()) ([Windows.Devices.Radios.RadioAccessStatus]))
$radios = Await ([Windows.Devices.Radios.Radio]::GetRadiosAsync()) ([System.Collections.Generic.IReadOnlyList[Windows.Devices.Radios.Radio]])
$wifi = @($radios | Where-Object { "$($_.Kind)" -eq $kind })
if ($set -ne '') { foreach ($r in $wifi) { [void](Await ($r.SetStateAsync($set)) ([Windows.Devices.Radios.RadioAccessStatus])) } }
if ($wifi.Count -eq 0) { 'none' } elseif (@($wifi | Where-Object { "$($_.State)" -eq 'On' }).Count -gt 0) { 'on' } else { 'off' }
"#;

pub fn problem_text(code: u32) -> &'static str {
    match code {
        0 => "",
        10 => "El dispositivo no puede iniciarse (código 10): el driver falló al arrancar la tarjeta.",
        1 | 18 => "El driver está mal instalado: hay que reinstalarlo.",
        3 => "El driver está dañado o falta memoria para cargarlo.",
        12 => "No tiene recursos libres (conflicto con otro dispositivo).",
        14 => "Necesita reiniciar el equipo para funcionar.",
        19 => "Su configuración en el registro está dañada: reinstala el driver.",
        22 => "El dispositivo está deshabilitado (código 22).",
        24 => "No está presente o no funciona bien (código 24).",
        28 => "No tiene driver instalado (código 28).",
        31 => "Windows no puede cargar el driver (código 31).",
        39 => "El driver está dañado o no es compatible (código 39).",
        43 => "Windows lo detuvo porque informó de un problema (código 43).",
        45 => "No está conectado ahora mismo (código 45).",
        52 => "Windows no puede verificar la firma de su driver (código 52).",
        _ => "Windows informa de un problema con el dispositivo.",
    }
}

/// Estado (y cambio si `set` es On/Off) de la radio `kind`: WiFi | Bluetooth.
pub fn radio_kind(kind: &str, set: &str) -> Option<bool> {
    let script = format!("{}{}{RADIO_SCRIPT}", crate::ps::text_var("set", set), crate::ps::text_var("kind", kind));
    match crate::pspool::query(&script, Some(Duration::from_secs(30)), "Radio").ok()?.trim() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

fn radio(set: &str) -> Option<bool> {
    radio_kind("WiFi", set)
}

#[tauri::command(async)]
pub fn wifi_state() -> Result<WifiState, String> {
    let out = crate::pspool::query(STATE_SCRIPT, Some(Duration::from_secs(40)), "Wi-Fi: estado")?;
    let mut s: WifiState = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    for a in &mut s.adapters {
        a.problem_text = problem_text(a.problem).into();
    }
    s.radio_on = radio("");
    let real: Vec<&WifiAdapter> = s.adapters.iter().filter(|a| a.present).collect();
    let (level, summary) = if real.is_empty() {
        ("none", "Este equipo no tiene tarjeta Wi-Fi (o Windows no la ve).".to_string())
    } else if let Some(bad) = real.iter().find(|a| a.problem != 0 && a.problem != 22) {
        ("error", format!("La tarjeta Wi-Fi no funciona: {} Prueba «Reiniciar la tarjeta»; si vuelve a pasar, actualiza su driver desde la web del fabricante y desactiva el ahorro de energía.", bad.problem_text))
    } else if real.iter().all(|a| a.problem == 22 || a.status.eq_ignore_ascii_case("Disabled")) {
        ("off", "La tarjeta Wi-Fi está deshabilitada en Windows.".to_string())
    } else if !s.service_running {
        ("error", "El servicio de Wi-Fi de Windows (Configuración automática de WLAN) está detenido.".to_string())
    } else if s.radio_on == Some(false) {
        ("off", "La Wi-Fi está apagada (interruptor de Windows o modo avión).".to_string())
    } else {
        ("ok", "La Wi-Fi funciona.".to_string())
    };
    s.level = level.into();
    s.summary = summary;
    Ok(s)
}

#[tauri::command(async)]
pub fn set_wifi_radio(tweaks: State<'_, TweakState>, on: bool) -> Result<bool, String> {
    let r = radio(if on { "On" } else { "Off" }).ok_or("No se pudo cambiar la Wi-Fi (¿la tarjeta funciona?).".to_string());
    tweaks.record(Op::Run, if on { "Wi-Fi encendida" } else { "Wi-Fi apagada" }, &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r
}

pub fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

pub fn check_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 300 || id.contains(['\n', '\r', '"']) {
        return Err("Dispositivo no válido.".into());
    }
    Ok(())
}

/// Deshabilita y vuelve a habilitar la tarjeta (reinicia su driver). Arregla muchos «código 10».
#[tauri::command(async)]
pub fn restart_wifi_adapter(tweaks: State<'_, TweakState>, instance_id: String) -> Result<(), String> {
    let r = restart_device(&instance_id);
    tweaks.record(Op::Run, "Reiniciar la tarjeta Wi-Fi", &r);
    r
}

/// Deshabilita y vuelve a habilitar un dispositivo (reinicia su driver). También habilita uno deshabilitado.
pub fn restart_device(instance_id: &str) -> Result<(), String> {
    need_admin()?;
    check_id(instance_id)?;
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n{}Disable-PnpDevice -InstanceId $id -Confirm:$false -ErrorAction SilentlyContinue\nStart-Sleep -Seconds 2\nEnable-PnpDevice -InstanceId $id -Confirm:$false\nStart-Sleep -Seconds 3\n'ok'",
        crate::ps::text_var("id", instance_id)
    );
    crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None }).map(|_| ())
}

/// Quita los adaptadores de red que ya no existen (restos de drivers reinstalados).
#[tauri::command(async)]
pub fn remove_ghost_wifi(tweaks: State<'_, TweakState>) -> Result<usize, String> {
    need_admin()?;
    let state = wifi_state()?;
    let ghosts: Vec<&WifiAdapter> = state.adapters.iter().filter(|a| !a.present).collect();
    let mut removed = 0;
    for g in &ghosts {
        check_id(&g.instance_id)?;
        if crate::ps::exec_opts("pnputil.exe", &["/remove-device", &g.instance_id], crate::ps::Opts { timeout: Some(Duration::from_secs(30)), task: None }).is_ok() {
            removed += 1;
        }
    }
    tweaks.record(Op::Run, &format!("Quitar {removed} adaptadores Wi-Fi fantasma"), &Ok::<(), String>(()));
    Ok(removed)
}

/// Que Windows no apague la tarjeta para ahorrar energía (causa típica de fallos intermitentes).
#[tauri::command(async)]
pub fn wifi_power_saving_off(tweaks: State<'_, TweakState>, name: String) -> Result<(), String> {
    need_admin()?;
    check_id(&name)?;
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n{}Set-NetAdapterPowerManagement -Name $n -AllowComputerToTurnOffDevice Disabled\n'ok'",
        crate::ps::text_var("n", &name)
    );
    let r = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, "Wi-Fi: desactivar el ahorro de energía de la tarjeta", &r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_parse() {
        for (name, s) in [("STATE", STATE_SCRIPT.to_string()), ("RADIO", format!("$set = ''\n$kind = 'WiFi'\n{RADIO_SCRIPT}"))] {
            let e = crate::ps::parse_errors(&s);
            assert!(e.is_empty(), "{name}: {e}");
        }
    }

    /// Equipo real: `cargo test wifictl_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn wifictl_real() {
        let s = wifi_state().unwrap();
        println!("{} · radio {:?} · servicio {}", s.summary, s.radio_on, s.service_running);
        for a in s.adapters {
            println!("  {} · presente {} · {} · problema {} · ahorro {:?}", a.name, a.present, a.status, a.problem, a.power_saving);
        }
    }
}
