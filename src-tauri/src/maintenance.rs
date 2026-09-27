//! Mantenimiento a fondo: qué retrasa el arranque, espacio de los puntos de
//! restauración, restaurar drivers de una copia y limpieza programada.

use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::State;

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() { Ok(()) } else { Err("Requiere ejecutar AdminOps como administrador.".into()) }
}

fn query<T: serde::de::DeserializeOwned>(script: &str, secs: u64) -> Result<T, String> {
    let out = crate::ps::powershell_opts(script, crate::ps::Opts { timeout: Some(Duration::from_secs(secs)), task: None })?;
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))
}

// ---------- Arranque ----------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootRecord {
    time: String,
    /// Milisegundos hasta el escritorio usable.
    total_ms: u64,
    main_ms: u64,
    post_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Culprit {
    /// app | driver | service | device | other
    kind: String,
    name: String,
    times: u32,
    /// Milisegundos que añade de media al arranque.
    avg_delay_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootAnalysis {
    boots: Vec<BootRecord>,
    culprits: Vec<Culprit>,
}

const BOOT_SCRIPT: &str = r#"
function Get-EventFields($e) { $d = @{}; ([xml]$e.ToXml()).Event.EventData.Data | ForEach-Object { $d[$_.Name] = $_.'#text' }; $d }
$log = 'Microsoft-Windows-Diagnostics-Performance/Operational'
$boots = @(Get-WinEvent -FilterHashtable @{ LogName = $log; Id = 100 } -MaxEvents 10 -ErrorAction Stop | ForEach-Object {
  $d = Get-EventFields $_
  [pscustomobject]@{ time = $_.TimeCreated.ToString('o'); totalMs = [uint64]$d.BootTime; mainMs = [uint64]$d.MainPathBootTime; postMs = [uint64]$d.BootPostBootTime }
})
$kinds = @{ 101 = 'app'; 102 = 'driver'; 103 = 'service'; 106 = 'other'; 109 = 'device' }
$raw = @(Get-WinEvent -FilterHashtable @{ LogName = $log; Id = 101, 102, 103, 106, 109; StartTime = (Get-Date).AddDays(-60) } -ErrorAction SilentlyContinue | ForEach-Object {
  $d = Get-EventFields $_
  $name = if ($d.FriendlyName) { $d.FriendlyName } elseif ($d.Name) { $d.Name } else { $null }
  if ($name) { [pscustomobject]@{ kind = $kinds[$_.Id]; name = $name; delay = [uint64]$d.DegradationTime } }
})
$culprits = @($raw | Group-Object kind, name | ForEach-Object {
  [pscustomobject]@{ kind = $_.Group[0].kind; name = $_.Group[0].name; times = $_.Count; avgDelayMs = [uint64](($_.Group | Measure-Object delay -Average).Average) }
} | Sort-Object avgDelayMs -Descending | Select-Object -First 15)
ConvertTo-Json -Depth 3 -Compress -InputObject ([pscustomobject]@{ boots = $boots; culprits = $culprits })
"#;

#[tauri::command(async)]
pub fn boot_analysis() -> Result<BootAnalysis, String> {
    query(BOOT_SCRIPT, 90).map_err(|e| {
        if e.to_lowercase().contains("no events") || e.contains("No se encontraron eventos") {
            "Windows aún no ha registrado ningún arranque analizado en este equipo.".into()
        } else if !crate::elevation::is_elevated() {
            "Requiere ejecutar AdminOps como administrador para leer el registro de arranque.".into()
        } else {
            e
        }
    })
}

// ---------- Puntos de restauración ----------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStorage {
    used: u64,
    max: u64,
    count: u32,
    oldest: Option<String>,
    newest: Option<String>,
}

const STORAGE_SCRIPT: &str = r#"
$vol = Get-CimInstance Win32_Volume -Filter "DriveLetter='$env:SystemDrive'"
$st = Get-CimInstance Win32_ShadowStorage -ErrorAction SilentlyContinue | Where-Object { $_.Volume.DeviceID -eq $vol.DeviceID } | Select-Object -First 1
$copies = @(Get-CimInstance Win32_ShadowCopy -ErrorAction SilentlyContinue | Where-Object { $_.VolumeName -eq $vol.DeviceID } | Sort-Object InstallDate)
[pscustomobject]@{
  used = if ($st) { [uint64]$st.UsedSpace } else { 0 }; max = if ($st) { [uint64]$st.MaxSpace } else { 0 }
  count = $copies.Count
  oldest = if ($copies) { $copies[0].InstallDate.ToString('o') } else { $null }
  newest = if ($copies) { $copies[-1].InstallDate.ToString('o') } else { $null }
} | ConvertTo-Json -Compress
"#;

#[tauri::command(async)]
pub fn restore_storage() -> Result<RestoreStorage, String> {
    need_admin()?;
    query(STORAGE_SCRIPT, 60)
}

/// Borra los puntos de restauración antiguos del disco del sistema y conserva el más reciente.
#[tauri::command(async)]
pub fn delete_old_restore_points(tweaks: State<'_, TweakState>) -> Result<u32, String> {
    need_admin()?;
    let script = r#"
$vol = Get-CimInstance Win32_Volume -Filter "DriveLetter='$env:SystemDrive'"
$old = @(Get-CimInstance Win32_ShadowCopy | Where-Object { $_.VolumeName -eq $vol.DeviceID } | Sort-Object InstallDate | Select-Object -SkipLast 1)
$old | Remove-CimInstance
$old.Count
"#;
    let result = crate::ps::powershell_opts(script, crate::ps::Opts { timeout: Some(Duration::from_secs(300)), task: None })
        .map(|o| o.trim().parse::<u32>().unwrap_or(0));
    let title = match &result {
        Ok(n) => format!("Borrar puntos de restauración antiguos ({n})"),
        Err(_) => "Borrar puntos de restauración antiguos".into(),
    };
    tweaks.record(Op::Run, &title, &result);
    result
}

// ---------- Drivers ----------

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriverBackup {
    name: String,
    drivers: u32,
    size: u64,
}

#[tauri::command(async)]
pub fn list_driver_backups(app: tauri::AppHandle) -> Vec<DriverBackup> {
    let mut v: Vec<DriverBackup> = std::fs::read_dir(crate::paths::drivers_backup_dir(&app))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let (size, _) = crate::space::folder_size(&e.path());
            let drivers = std::fs::read_dir(e.path()).map(|d| d.flatten().filter(|x| x.path().is_dir()).count() as u32).unwrap_or(0);
            DriverBackup { name: e.file_name().to_string_lossy().into_owned(), drivers, size }
        })
        .collect();
    v.sort_by(|a, b| b.name.cmp(&a.name));
    v
}

/// Instala todos los drivers de una copia hecha con AdminOps (`pnputil /add-driver /subdirs /install`).
#[tauri::command(async)]
pub fn restore_drivers(app: tauri::AppHandle, name: String, tweaks: State<'_, TweakState>) -> Result<String, String> {
    need_admin()?;
    if name.contains(['\\', '/']) || name.contains("..") || name.is_empty() {
        return Err("Copia no válida.".into());
    }
    let dir: PathBuf = crate::paths::drivers_backup_dir(&app).join(&name);
    if !dir.is_dir() {
        return Err("Esa copia ya no existe.".into());
    }
    let task = Task::new(&app, "drivers-restore").named("Restaurar drivers");
    task.step("Instalando los drivers de la copia (puede tardar varios minutos)…");
    let pattern = dir.join("*.inf").display().to_string();
    let r = crate::ps::exec_opts("pnputil.exe", &["/add-driver", &pattern, "/subdirs", "/install"], task.opts(Some(Duration::from_secs(30 * 60))));
    // pnputil devuelve error si alguno ya estaba instalado o no aplica: se considera bien si terminó.
    let result = match r {
        Ok(_) => Ok("Drivers de la copia instalados.".to_string()),
        Err(e) if e.contains("3010") => Ok("Drivers instalados. Reinicia el equipo para terminar.".to_string()),
        Err(e) if e.contains("259") => Ok("Todos los drivers de la copia ya estaban instalados o no aplican a este equipo.".to_string()),
        Err(e) => Err(e),
    };
    tweaks.record(Op::Run, &format!("Restaurar drivers de la copia {name}"), &result);
    result
}

// ---------- Mantenimiento programado ----------

const TASK_PATH: &str = r"\AdminOps\";
const TASK_NAME: &str = "Mantenimiento";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Schedule {
    /// 1 = semanal, 2 = quincenal, 4 = cada cuatro semanas.
    weeks: u32,
    /// Monday…Sunday (inglés, como lo espera el Programador de tareas).
    day: String,
    /// HH:MM
    time: String,
    tasks: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleView {
    enabled: bool,
    schedule: Schedule,
    available: Vec<(String, String)>,
    last_run: Option<String>,
    last_result: Option<i64>,
    next_run: Option<String>,
    /// En modo portable no se ofrece: la tarea apuntaría a un USB que no siempre está.
    portable: bool,
}

fn schedule_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("maintenance.json")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskInfo {
    exists: bool,
    last_run: Option<String>,
    last_result: Option<i64>,
    next_run: Option<String>,
}

#[tauri::command(async)]
pub fn maintenance_schedule(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Result<ScheduleView, String> {
    let info: TaskInfo = query(
        &format!(
            "$t = Get-ScheduledTask -TaskPath '{TASK_PATH}' -TaskName '{TASK_NAME}' -ErrorAction SilentlyContinue\n\
             $i = if ($t) {{ $t | Get-ScheduledTaskInfo }} else {{ $null }}\n\
             $d = {{ param($x) if ($x -and $x.Year -gt 2000) {{ $x.ToString('o') }} else {{ $null }} }}\n\
             [pscustomobject]@{{ exists = [bool]$t; lastRun = & $d $i.LastRunTime; lastResult = if ($i -and $i.LastRunTime.Year -gt 2000) {{ [int64]$i.LastTaskResult }} else {{ $null }}; nextRun = & $d $i.NextRunTime }} | ConvertTo-Json -Compress"
        ),
        60,
    )?;
    let mut schedule: Schedule = crate::paths::read_json(&schedule_path(&app));
    if schedule.weeks == 0 {
        schedule = Schedule {
            weeks: 1,
            day: "Sunday".into(),
            time: "03:00".into(),
            tasks: vec!["cleanup.user-temp".into(), "cleanup.windows-temp".into(), "cleanup.browser-cache".into(), "cleanup.delivery-optimization".into()],
        };
    }
    Ok(ScheduleView {
        enabled: info.exists,
        schedule,
        available: tweaks.maintenance_tasks(),
        last_run: info.last_run,
        last_result: info.last_result,
        next_run: info.next_run,
        portable: crate::paths::is_portable(),
    })
}

fn validate(s: &Schedule, tweaks: &TweakState) -> Result<(), String> {
    if ![1, 2, 4].contains(&s.weeks) {
        return Err("Frecuencia no válida.".into());
    }
    if !["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"].contains(&s.day.as_str()) {
        return Err("Día no válido.".into());
    }
    let ok_time = s.time.len() == 5
        && s.time.as_bytes()[2] == b':'
        && s.time[..2].parse::<u32>().is_ok_and(|h| h < 24)
        && s.time[3..].parse::<u32>().is_ok_and(|m| m < 60);
    if !ok_time {
        return Err("Hora no válida (formato HH:MM).".into());
    }
    let allowed: Vec<String> = tweaks.maintenance_tasks().into_iter().map(|(id, _)| id).collect();
    if s.tasks.is_empty() || s.tasks.iter().any(|t| !allowed.contains(t)) {
        return Err("Elige al menos una limpieza de la lista.".into());
    }
    Ok(())
}

/// Crea o actualiza la tarea programada (se ejecuta como SYSTEM, sin ventana).
#[tauri::command(async)]
pub fn set_maintenance_schedule(app: tauri::AppHandle, schedule: Option<Schedule>, tweaks: State<'_, TweakState>) -> Result<(), String> {
    need_admin()?;
    let result = match &schedule {
        None => crate::ps::powershell(&format!(
            "Unregister-ScheduledTask -TaskPath '{TASK_PATH}' -TaskName '{TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue\n'ok'"
        ))
        .map(|_| ()),
        Some(s) => {
            if crate::paths::is_portable() {
                return Err("En modo portable no se puede programar: instala AdminOps en este equipo.".into());
            }
            validate(s, &tweaks)?;
            let exe = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string();
            let data = crate::paths::machine_data_dir(&app).display().to_string();
            let args = format!("--maintenance --data \"{data}\" --only {}", s.tasks.join(","));
            let script = format!(
                "{}{}$action = New-ScheduledTaskAction -Execute $exe -Argument $a\n\
                 $trigger = New-ScheduledTaskTrigger -Weekly -WeeksInterval {weeks} -DaysOfWeek {day} -At '{time}'\n\
                 $principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest\n\
                 $settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Hours 2)\n\
                 Register-ScheduledTask -TaskPath '{TASK_PATH}' -TaskName '{TASK_NAME}' -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description 'Limpieza programada con AdminOps' -Force | Out-Null\n'ok'",
                crate::ps::text_var("exe", &exe),
                crate::ps::text_var("a", &args),
                weeks = s.weeks,
                day = s.day,
                time = s.time,
            );
            crate::ps::powershell(&script).map(|_| ()).and_then(|_| crate::paths::write_json(&schedule_path(&app), s))
        }
    };
    let title = match &schedule {
        None => "Desactivar el mantenimiento programado".to_string(),
        Some(s) => format!("Mantenimiento programado: cada {} semana(s), {} a las {} ({} limpiezas)", s.weeks, s.day, s.time, s.tasks.len()),
    };
    tweaks.record(Op::Run, &title, &result);
    result
}

/// Modo línea de comandos: `adminops.exe --maintenance --data <carpeta> --only id1,id2`.
/// Lo lanza la tarea programada. Código de salida: 0 bien, 1 alguna limpieza falló, 2 argumentos.
pub fn run_cli(args: &[String]) -> i32 {
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let (Some(data), Some(only)) = (value("--data"), value("--only")) else { return 2 };
    let dir = PathBuf::from(data);
    if !dir.is_dir() {
        return 2;
    }
    let state = TweakState::with_data_dir(dir);
    let mut failed = false;
    for id in only.split(',').filter(|s| !s.is_empty()) {
        failed |= state.run_maintenance_task(id).is_err();
    }
    i32::from(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_schedules() {
        let state = TweakState::with_data_dir(std::env::temp_dir());
        let ok = Schedule { weeks: 1, day: "Sunday".into(), time: "03:00".into(), tasks: vec!["cleanup.user-temp".into()] };
        assert!(validate(&ok, &state).is_ok());
        assert!(validate(&Schedule { weeks: 3, ..ok.clone() }, &state).is_err());
        assert!(validate(&Schedule { day: "Domingo".into(), ..ok.clone() }, &state).is_err());
        assert!(validate(&Schedule { time: "25:00".into(), ..ok.clone() }, &state).is_err());
        assert!(validate(&Schedule { tasks: vec!["privacy.telemetry".into()], ..ok.clone() }, &state).is_err());
        assert!(validate(&Schedule { tasks: vec![], ..ok }, &state).is_err());
        // Solo limpiezas: nada que cambie ajustes del sistema.
        assert!(state.maintenance_tasks().iter().all(|(id, _)| id.starts_with("cleanup.")));
    }

    #[test]
    fn cli_rejects_bad_arguments() {
        assert_eq!(run_cli(&["x".into(), "--maintenance".into()]), 2);
        assert_eq!(run_cli(&["--data".into(), r"C:\no\existe".into(), "--only".into(), "cleanup.user-temp".into()]), 2);
    }

    #[test]
    fn embedded_scripts_parse() {
        for (name, script) in [("BOOT_SCRIPT", BOOT_SCRIPT), ("STORAGE_SCRIPT", STORAGE_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }
}
