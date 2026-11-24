//! «Pausar 2 h» para OneDrive (Red → Uso de la red).
//!
//! Cierra OneDrive del usuario con la sesión abierta (`OneDrive.exe /shutdown`, en su sesión: si
//! AdminOps corre con otra cuenta de administrador, con una tarea programada interactiva de ese usuario) y deja
//! una tarea programada de un solo uso que lo vuelve a abrir (`OneDrive.exe /background`) a las 2 horas, así
//! vuelve aunque AdminOps se cierre. Va al diario con su «Deshacer»: «Reanudar ahora» abre OneDrive y borra la tarea.

use crate::netusage::OneDrivePause;
use crate::tweaks::journal::Backup;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::State;

/// Lo que dura la pausa.
pub const PAUSE_HOURS: u64 = 2;
pub const TASK_PATH: &str = "\\AdminOps\\";
pub const TASK_NAME: &str = "Reanudar OneDrive";
const TITLE: &str = "Pausar OneDrive 2 h";

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PauseFile {
    until_ms: u64,
    entry_id: u64,
}

static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

pub fn start(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

fn file() -> Option<PathBuf> {
    APP.get().map(|a| crate::paths::machine_data_dir(a).join("onedrive_pause.json"))
}

fn read_file() -> Option<PauseFile> {
    let p = file()?;
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok())
}

fn clear_file() {
    if let Some(p) = file() {
        let _ = std::fs::remove_file(p);
    }
}

/// La pausa en marcha, si la hay. Una pausa cuya hora ya pasó se olvida.
pub fn pause_info() -> Option<OneDrivePause> {
    let f = read_file()?;
    if f.until_ms <= crate::netusage::now_ms() {
        clear_file();
        return None;
    }
    Some(OneDrivePause { until_ms: f.until_ms, entry_id: f.entry_id })
}

/// Dónde está OneDrive.exe: el del usuario con la sesión abierta o el de todo el equipo.
pub fn find_exe() -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();
    if let Some(d) = crate::target_user::user_dirs() {
        c.push(d.local_app_data.join(r"Microsoft\OneDrive\OneDrive.exe"));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(p) = std::env::var_os(var) {
            c.push(PathBuf::from(p).join(r"Microsoft OneDrive\OneDrive.exe"));
        }
    }
    c.into_iter().find(|p| p.is_file())
}

fn target_sid() -> Option<String> {
    crate::target_user::get().map(|t| t.sid.clone()).filter(|s| !s.is_empty())
}

/// Líneas que ponen `$user` con el nombre de la cuenta del usuario con la sesión abierta.
fn user_lines(sid: Option<&str>) -> String {
    match sid {
        Some(s) => format!("{}$user = ([Security.Principal.SecurityIdentifier]$sid).Translate([Security.Principal.NTAccount]).Value\n", crate::ps::text_var("sid", s)),
        None => "$user = [Security.Principal.WindowsIdentity]::GetCurrent().Name\n".to_string(),
    }
}

/// El argumento de `cmd.exe` que abre `OneDrive.exe` con `args` y termina enseguida: `OneDrive.exe /background` se queda
/// abierto, y una tarea que no acaba se mataría con su límite de tiempo (o seguiría «en ejecución» horas). Con `start`,
/// `cmd` lo lanza aparte y sale. Es una expresión de PowerShell que arma el texto desde `$exe` (las comillas van dentro).
fn start_argument(args: &str) -> String {
    format!(r#"('/c start "" "' + $exe + '" ' + {})"#, crate::ps::ps_literal(args))
}

/// Los procesos de OneDrive que son de `$user` (y no los de otros usuarios con sesión en el equipo). Con CIM y `GetOwner`
/// en vez de `Get-Process -IncludeUserName`, que solo funciona como administrador y aquí no se eleva.
const OWN_PROCESSES: &str = r#"function OneDriveDelUsuario {
  @(Get-CimInstance Win32_Process -Filter "Name='OneDrive.exe'" -ErrorAction SilentlyContinue | Where-Object {
    $o = Invoke-CimMethod -InputObject $_ -MethodName GetOwner -ErrorAction SilentlyContinue
    $o -and ("$($o.Domain)\$($o.User)" -eq $user)
  })
}
"#;

/// Una tarea sin horario que se lanza ya, en la sesión de ese usuario, y se borra. Va por `cmd /c start` para que la tarea
/// termine sola (ver `start_argument`).
fn run_as_user_lines(args: &str) -> String {
    format!(
        "$accion = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument {a}\n\
         $principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive\n\
         $una = 'AdminOps OneDrive un momento'\n\
         Register-ScheduledTask -TaskPath {path} -TaskName $una -Action $accion -Principal $principal -Settings (New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5)) -Force | Out-Null\n\
         Start-ScheduledTask -TaskPath {path} -TaskName $una\n\
         Start-Sleep -Seconds 2\n\
         for ($i = 0; $i -lt 20; $i++) {{ if ((Get-ScheduledTask -TaskPath {path} -TaskName $una).State -ne 'Running') {{ break }}; Start-Sleep -Seconds 1 }}\n\
         Unregister-ScheduledTask -TaskPath {path} -TaskName $una -Confirm:$false -ErrorAction SilentlyContinue\n",
        a = start_argument(args),
        path = crate::ps::ps_literal(TASK_PATH),
    )
}

/// El script de la pausa: cerrar OneDrive, comprobar que se cerró y dejar la tarea que lo vuelve a abrir.
///
/// OneDrive se cierra y se abre siempre como el usuario del escritorio y sin elevar (con una tarea interactiva de
/// ese usuario): desde un PowerShell elevado `OneDrive.exe /shutdown` no llega a la instancia del usuario y
/// `OneDrive.exe /background` arrancaría elevado, y OneDrive no lo admite.
pub fn pause_script(exe: &str, sid: Option<&str>, at_secs: u64) -> String {
    let mut s = String::new();
    s.push_str(&crate::ps::text_var("exe", exe));
    s.push_str(&user_lines(sid));
    s.push_str(OWN_PROCESSES);
    s.push_str(&format!("$a = [DateTimeOffset]::FromUnixTimeSeconds({at_secs}).LocalDateTime\n"));
    s.push_str("# Cerrar OneDrive en la sesión del usuario, sin elevar (sus archivos no se tocan; solo deja de sincronizar)\n");
    s.push_str(&run_as_user_lines("/shutdown"));
    s.push_str("# Comprobar que de verdad se cerró (el de este usuario); si no, no se da la pausa por hecha\n");
    s.push_str("for ($i = 0; $i -lt 15 -and (OneDriveDelUsuario).Count -gt 0; $i++) { Start-Sleep -Seconds 1 }\n");
    s.push_str("if ((OneDriveDelUsuario).Count -gt 0) { throw 'OneDrive no se cerró (puede que el usuario no tenga la sesión abierta). No se ha pausado nada.' }\n");
    s.push_str("# Tarea de un solo uso que lo vuelve a abrir a su hora, aunque AdminOps esté cerrado\n");
    s.push_str(&format!("$accion = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument {}\n", start_argument("/background")));
    s.push_str("$disparador = New-ScheduledTaskTrigger -Once -At $a\n");
    s.push_str("$disparador.EndBoundary = $a.AddHours(12).ToString('s')\n");
    s.push_str("$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive\n");
    s.push_str("$ajustes = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 10) -DeleteExpiredTaskAfter (New-TimeSpan -Minutes 1)\n");
    s.push_str(&format!(
        "Register-ScheduledTask -TaskPath {} -TaskName {} -Action $accion -Trigger $disparador -Principal $principal -Settings $ajustes -Force | Out-Null\n",
        crate::ps::ps_literal(TASK_PATH),
        crate::ps::ps_literal(TASK_NAME)
    ));
    s
}

/// El script de «Reanudar ahora»: abrir OneDrive como el usuario del escritorio, sin elevar, y solo cuando se ve abierto,
/// borrar la tarea que lo iba a abrir a su hora. Si no se abre, la tarea se queda y el error sale (el diario no se da por deshecho).
pub fn resume_script(exe: &str, sid: Option<&str>) -> String {
    let mut s = String::new();
    s.push_str(&crate::ps::text_var("exe", exe));
    s.push_str(&user_lines(sid));
    s.push_str(OWN_PROCESSES);
    s.push_str("# Abrir OneDrive otra vez, como el usuario y sin elevar\n");
    s.push_str(&run_as_user_lines("/background"));
    s.push_str("# Comprobar que se abrió (hasta 20 s); si no, la tarea de reapertura se queda\n");
    s.push_str("$abierto = $false\n");
    s.push_str("for ($i = 0; $i -lt 20; $i++) { if ((OneDriveDelUsuario).Count -gt 0) { $abierto = $true; break }; Start-Sleep -Seconds 1 }\n");
    s.push_str("if (-not $abierto) { throw 'OneDrive no se abrió. La tarea que lo abre a su hora se queda programada; también puedes abrirlo a mano.' }\n");
    s.push_str("# Ya está abierto: quitar la tarea que lo iba a abrir a su hora\n");
    s.push_str(&format!("Unregister-ScheduledTask -TaskPath {} -TaskName {} -Confirm:$false -ErrorAction SilentlyContinue\n", crate::ps::ps_literal(TASK_PATH), crate::ps::ps_literal(TASK_NAME)));
    s
}

/// «Reanudar ahora» (y deshacer la pausa): lo usa el diario.
pub fn resume(exe: &str, sid: Option<&str>) -> Result<(), String> {
    let script = resume_script(exe, sid);
    crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None }).map(|_| ()).map_err(|e| format!("No se pudo reanudar OneDrive: {e}"))?;
    clear_file();
    Ok(())
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PauseResult {
    pub until_ms: u64,
    pub entry_id: u64,
    pub message: String,
}

/// «Pausar 2 h»: cierra OneDrive y programa que se abra a las 2 horas.
#[tauri::command(async)]
pub fn onedrive_pause(state: State<'_, TweakState>) -> Result<PauseResult, String> {
    let exe = find_exe().ok_or("OneDrive no está instalado en este equipo.")?;
    let exe = exe.to_string_lossy().into_owned();
    let sid = target_sid();
    let until_ms = crate::netusage::now_ms() + PAUSE_HOURS * 3_600_000;
    let script = pause_script(&exe, sid.as_deref(), until_ms / 1000);
    let result = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None }).map(|_| ());
    let backups = vec![Backup::OneDrivePause { task: TASK_NAME.to_string(), exe: exe.clone(), sid: sid.clone() }];
    let entry_id = state.log_change(TITLE, backups, &result);
    result?;
    if let Some(p) = file() {
        let _ = crate::paths::write_json(&p, &PauseFile { until_ms, entry_id });
    }
    Ok(PauseResult { until_ms, entry_id, message: "OneDrive en pausa: se vuelve a abrir solo dentro de 2 horas.".into() })
}

/// El comando que se enseña en «Ver el comando».
pub fn action_script() -> Result<(&'static str, String), String> {
    let mut s = String::from("# Pausar OneDrive 2 horas: lo cierra y deja una tarea que lo vuelve a abrir\n\n");
    s.push_str("# Dónde está OneDrive.exe (el del usuario o el de todo el equipo)\n");
    s.push_str("$exe = @(\"$env:LOCALAPPDATA\\Microsoft\\OneDrive\\OneDrive.exe\", \"$env:ProgramFiles\\Microsoft OneDrive\\OneDrive.exe\") | Where-Object { Test-Path $_ } | Select-Object -First 1\n");
    s.push_str("$user = [Security.Principal.WindowsIdentity]::GetCurrent().Name\n");
    s.push_str("$a = (Get-Date).AddHours(2)\n");
    s.push_str("# Cerrar OneDrive (sus archivos no se tocan; solo deja de sincronizar). AdminOps lo hace con una tarea interactiva del usuario para no elevarlo\n& $exe /shutdown\n");
    s.push_str("# Tarea de un solo uso que lo vuelve a abrir a su hora, aunque AdminOps esté cerrado\n");
    s.push_str(&format!("$accion = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument {}\n", start_argument("/background")));
    s.push_str("$disparador = New-ScheduledTaskTrigger -Once -At $a\n");
    s.push_str("$disparador.EndBoundary = $a.AddHours(12).ToString('s')\n");
    s.push_str("$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive\n");
    s.push_str("$ajustes = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 10) -DeleteExpiredTaskAfter (New-TimeSpan -Minutes 1)\n");
    s.push_str(&format!("Register-ScheduledTask -TaskPath {} -TaskName {} -Action $accion -Trigger $disparador -Principal $principal -Settings $ajustes -Force | Out-Null\n", crate::ps::ps_literal(TASK_PATH), crate::ps::ps_literal(TASK_NAME)));
    s.push_str("\n# Para reanudar antes de tiempo:\n");
    s.push_str(&format!("# Unregister-ScheduledTask -TaskPath {} -TaskName {} -Confirm:$false\n# Start-Process -FilePath $exe -ArgumentList '/background'\n", crate::ps::ps_literal(TASK_PATH), crate::ps::ps_literal(TASK_NAME)));
    Ok((TITLE, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_scripts_son_powershell_valido() {
        let p = pause_script(r"C:\Program Files\Microsoft OneDrive\OneDrive.exe", Some("S-1-5-21-1-2-3-1001"), 1_800_000_000);
        let e = crate::ps::parse_errors(&p);
        assert!(e.trim().is_empty(), "pausa: {e}\n{p}");
        for sid in [Some("S-1-5-21-1-2-3-1001"), None] {
            let r = resume_script(r"C:\Program Files\Microsoft OneDrive\OneDrive.exe", sid);
            let e = crate::ps::parse_errors(&r);
            assert!(e.trim().is_empty(), "reanudar: {e}\n{r}");
        }
        let (_, doc) = action_script().unwrap();
        let e = crate::ps::parse_errors(&doc);
        assert!(e.trim().is_empty(), "{e}");
    }

    /// OneDrive no arranca elevado: ni al cerrarlo ni al abrirlo se ejecuta desde el PowerShell de AdminOps.
    #[test]
    fn onedrive_se_abre_y_se_cierra_como_el_usuario_sin_elevar() {
        let p = pause_script("x.exe", None, 1);
        let r = resume_script("x.exe", None);
        for (name, s) in [("pausa", &p), ("reanudar", &r)] {
            assert!(!s.contains("Start-Process"), "{name}: arranca elevado\n{s}");
            assert!(!s.lines().any(|l| l.trim_start().starts_with("& $exe")), "{name}: ejecuta OneDrive directamente\n{s}");
            assert!(s.contains("New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive") && s.contains("Start-ScheduledTask"), "{name}\n{s}");
            assert!(!s.contains("RunLevel Highest"), "{name}\n{s}");
        }
        assert!(p.contains("+ '/shutdown')") && r.contains("+ '/background')"));
    }

    /// OneDrive.exe /background no termina: las tareas lo lanzan con `cmd /c start` para acabar enseguida y que ningún límite de tiempo las mate.
    #[test]
    fn las_tareas_lanzan_onedrive_con_start_para_terminar_enseguida() {
        let p = pause_script(r"C:\Program Files\Microsoft OneDrive\OneDrive.exe", Some("S-1-5-21-1-2-3-1001"), 1);
        let r = resume_script("x.exe", None);
        let (_, doc) = action_script().unwrap();
        for (name, s) in [("pausa", &p), ("reanudar", &r), ("comando", &doc)] {
            assert!(!s.contains("-Execute $exe"), "{name}: la tarea ejecuta OneDrive.exe directamente\n{s}");
            assert!(s.contains(r#"-Execute 'cmd.exe' -Argument ('/c start "" "' + $exe + '" ' + "#), "{name}\n{s}");
        }
        // La tarea de reapertura (la de la hora) también va por cmd.
        assert!(p.contains(r#"-Argument ('/c start "" "' + $exe + '" ' + '/background')"#) && p.contains("-Trigger $disparador"), "{p}");
        // La tarea de un momento lleva ajustes (batería y 5 minutos) y no se queda con los de por defecto.
        for s in [&p, &r] {
            assert!(s.contains("-Settings (New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5))"), "{s}");
        }
    }

    /// Los procesos que se comprueban son solo los del usuario (y sin pedir administrador).
    #[test]
    fn solo_se_miran_los_procesos_de_onedrive_del_usuario() {
        let p = pause_script("x.exe", Some("S-1-5-21-1-2-3-1001"), 1);
        assert!(p.contains("GetOwner") && p.contains("-eq $user"), "{p}");
        assert!(!p.contains("Get-Process") && !p.contains("-IncludeUserName"), "{p}");
    }

    /// «Reanudar»: primero se abre, luego se espera a verlo abierto y solo entonces se borra la tarea de reapertura.
    #[test]
    fn reanudar_borra_la_tarea_solo_si_onedrive_se_abrio() {
        let r = resume_script("x.exe", Some("S-1-5-21-1-2-3-1001"));
        let abrir = r.find("Start-ScheduledTask").expect("abre");
        let esperar = r.find("$abierto = $true").expect("espera");
        let fallo = r.find("throw 'OneDrive no se abrió").expect("falla si no se abre");
        let borrar = r.find("Unregister-ScheduledTask -TaskPath '\\AdminOps\\' -TaskName 'Reanudar OneDrive'").expect("borra la tarea");
        assert!(abrir < esperar && esperar < fallo && fallo < borrar, "{r}");
        assert!(r.contains("$i -lt 20"), "{r}");
    }

    /// Si OneDrive sigue abierto tras cerrarlo, el script falla antes de dejar la tarea de reabrirlo: la pausa no se da por hecha.
    #[test]
    fn si_onedrive_no_se_cerro_la_pausa_no_se_da_por_hecha() {
        let p = pause_script("x.exe", None, 1);
        let comprobar = p.find("if ((OneDriveDelUsuario).Count -gt 0)").expect("comprueba que se cerró");
        let throw_ = p.find("throw 'OneDrive no se cerró").expect("falla");
        let tarea = p.find("Register-ScheduledTask -TaskPath '\\AdminOps\\' -TaskName 'Reanudar OneDrive'").expect("deja la tarea");
        assert!(comprobar < throw_ && throw_ < tarea, "la tarea de reapertura se crea antes de comprobar\n{p}");
    }

    #[test]
    fn una_ruta_con_comillas_no_se_sale_del_texto() {
        let p = pause_script(r"C:\Users\O'Brien\OneDrive.exe", None, 1);
        assert!(p.contains("$exe = 'C:\\Users\\O''Brien\\OneDrive.exe'"), "{p}");
        let e = crate::ps::parse_errors(&p);
        assert!(e.trim().is_empty(), "{e}");
    }

    #[test]
    fn la_tarea_se_borra_sola_y_se_abre_en_segundo_plano() {
        let p = pause_script("x.exe", None, 1);
        assert!(p.contains("/shutdown") && p.contains("/background"));
        assert!(p.contains("-DeleteExpiredTaskAfter") && p.contains("EndBoundary"));
        assert!(p.contains("-Once -At $a"));
        let r = resume_script("x.exe", None);
        assert!(r.contains("Unregister-ScheduledTask") && r.contains("/background"));
    }
}
