//! Programas que arrancan con Windows: claves Run, carpetas Inicio y tareas
//! programadas al iniciar sesión.
//!
//! Para Run y carpetas usamos el mismo mecanismo que el Administrador de tareas
//! (`Explorer\StartupApproved`): no se borra nada, solo se marca deshabilitado,
//! así que es 100% reversible y Windows lo muestra igual en su propia UI.

use super::journal::{entry, Backup, Op};
use super::registry;
use super::TweakState;
use crate::ps;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

struct RegSource {
    run: &'static str,
    approved: &'static str,
    label: &'static str,
}

const REG_SOURCES: &[RegSource] = &[
    RegSource { run: r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", approved: "HKCU|Run", label: "Registro (usuario)" },
    RegSource { run: r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run", approved: "HKLM|Run", label: "Registro (equipo)" },
    RegSource {
        run: r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
        approved: "HKLM|Run32",
        label: "Registro (equipo, 32 bits)",
    },
];

fn approved_key(spec: &str) -> String {
    let (hive, sub) = spec.split_once('|').unwrap();
    format!(r"{hive}\{APPROVED}\{sub}")
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    id: String,
    name: String,
    command: String,
    location: String,
    source: &'static str,
    enabled: bool,
    needs_admin: bool,
    publisher: Option<String>,
    description: Option<String>,
    /// Ejecutable resuelto (sigue accesos directos). `None` si no existe: entrada huérfana.
    target: Option<String>,
    #[serde(skip)]
    approved: Option<(String, String)>,
    #[serde(skip)]
    task: Option<(String, String)>,
}

/// StartupApproved: primer byte par = habilitado, impar = deshabilitado. Sin valor = habilitado.
fn approved_enabled(key: &str, name: &str) -> bool {
    registry::read_raw(key, name).is_none_or(|raw| raw.bytes.first().is_none_or(|b| b & 1 == 0))
}

fn user_startup_folder() -> Option<std::path::PathBuf> {
    let profile = match crate::target_user::hkcu_redirect() {
        Some(sid) => registry::read_string(
            &format!(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}"),
            "ProfileImagePath",
        )
        .map(|p| std::path::PathBuf::from(p).join(r"AppData\Roaming"))?,
        None => std::path::PathBuf::from(std::env::var_os("APPDATA")?),
    };
    Some(profile.join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
}

fn common_startup_folder() -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from(std::env::var_os("ProgramData")?).join(r"Microsoft\Windows\Start Menu\Programs\StartUp"))
}

fn collect_local() -> Vec<StartupItem> {
    let mut items = Vec::new();
    for src in REG_SOURCES {
        let key = approved_key(src.approved);
        for (name, raw) in registry::values(src.run) {
            let Some(command) = registry::raw_to_string(&raw).filter(|c| !name.is_empty() && !c.is_empty()) else {
                continue;
            };
            items.push(StartupItem {
                id: format!("reg|{}|{name}", src.run),
                enabled: approved_enabled(&key, &name),
                name: name.clone(),
                command,
                location: src.label.into(),
                source: "registry",
                needs_admin: src.run.starts_with("HKLM"),
                publisher: None,
                description: None,
                target: None,
                approved: Some((key.clone(), name)),
                task: None,
            });
        }
    }
    let folders = [
        (user_startup_folder(), "HKCU|StartupFolder", "Carpeta Inicio (usuario)", false),
        (common_startup_folder(), "HKLM|StartupFolder", "Carpeta Inicio (todos)", true),
    ];
    for (dir, spec, label, admin) in folders {
        let Some(dir) = dir else { continue };
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        let key = approved_key(spec);
        for f in read.flatten() {
            let file = f.file_name().to_string_lossy().into_owned();
            if file.eq_ignore_ascii_case("desktop.ini") || !f.path().is_file() {
                continue;
            }
            let display = std::path::Path::new(&file).file_stem().map_or(file.clone(), |s| s.to_string_lossy().into_owned());
            items.push(StartupItem {
                id: format!("folder|{}|{file}", dir.display()),
                enabled: approved_enabled(&key, &file),
                name: display,
                command: f.path().display().to_string(),
                location: label.into(),
                source: "folder",
                needs_admin: admin,
                publisher: None,
                description: None,
                target: None,
                approved: Some((key.clone(), file)),
                task: None,
            });
        }
    }
    items
}

#[derive(Serialize)]
struct Probe<'a> {
    i: &'a str,
    c: &'a str,
}

#[derive(Deserialize)]
struct ProbeResult {
    i: String,
    p: Option<String>,
    co: Option<String>,
    d: Option<String>,
    e: bool,
}

#[derive(Deserialize)]
struct RawTask {
    path: String,
    name: String,
    enabled: bool,
    command: String,
}

#[derive(Deserialize)]
struct Enriched {
    tasks: Vec<RawTask>,
    info: Vec<ProbeResult>,
}

/// Una sola llamada a PowerShell: lista tareas de inicio de sesión y resuelve
/// editor/descripción del ejecutable de cada entrada (incluidas las tareas).
fn enrich(items: &[StartupItem]) -> Result<Enriched, String> {
    let probes: Vec<Probe> = items.iter().map(|i| Probe { i: &i.id, c: &i.command }).collect();
    let json = serde_json::to_string(&probes).unwrap().replace('\'', "''");
    let script = format!(
        r#"
$items = [System.Collections.ArrayList]::new()
foreach ($x in (ConvertFrom-Json '{json}')) {{ [void]$items.Add($x) }}
# Programador de tareas por COM (≈50 ms) en vez de Get-ScheduledTask (≈1 s):
# se salta \Microsoft entero y solo se leen los disparadores de cada tarea.
$tasks = [System.Collections.ArrayList]::new()
$svc = New-Object -ComObject Schedule.Service
$svc.Connect()
$pending = [System.Collections.Queue]::new()
$pending.Enqueue($svc.GetFolder('\'))
while ($pending.Count) {{
  $folder = $pending.Dequeue()
  foreach ($sub in $folder.GetFolders(0)) {{ if ($sub.Path -notlike '\Microsoft*') {{ $pending.Enqueue($sub) }} }}
  $taskPath = if ($folder.Path -eq '\') {{ '\' }} else {{ $folder.Path + '\' }}
  foreach ($task in $folder.GetTasks(1)) {{
    $def = $task.Definition
    # 8 = al arrancar, 9 = al iniciar sesión
    if (-not @($def.Triggers | Where-Object {{ $_.Type -in 8, 9 }}).Count) {{ continue }}
    $cmd = (@($def.Actions | Where-Object {{ $_.Type -eq 0 -and $_.Path }} | ForEach-Object {{ ('"' + $_.Path.Trim('"') + '" ' + $_.Arguments).Trim() }}) -join ' ; ')
    [void]$tasks.Add([pscustomobject]@{{ path = $taskPath; name = $task.Name; enabled = [bool]$task.Enabled; command = "$cmd" }})
    [void]$items.Add([pscustomobject]@{{ i = "task|$taskPath|$($task.Name)"; c = "$cmd" }})
  }}
}}
$tasks = @($tasks)
$sh = New-Object -ComObject WScript.Shell
$info = @(foreach ($it in $items) {{
  $c = [Environment]::ExpandEnvironmentVariables("$($it.c)").Trim()
  $p = $null
  if ($c.StartsWith('"')) {{ $end = $c.IndexOf('"', 1); if ($end -gt 1) {{ $p = $c.Substring(1, $end - 1) }} }}
  else {{
    $m = [regex]::Match($c, '^(.+?\.(exe|com|bat|cmd|lnk|vbs|ps1|url))(\s|$)', 'IgnoreCase')
    $p = if ($m.Success) {{ $m.Groups[1].Value }} else {{ ($c -split ' ')[0] }}
  }}
  if ($p -and $p -like '*.lnk' -and (Test-Path -LiteralPath $p)) {{ try {{ $p = $sh.CreateShortcut($p).TargetPath }} catch {{}} }}
  $exists = [bool]($p -and (Test-Path -LiteralPath $p))
  $vi = if ($exists -and $p -notlike '*.url') {{ (Get-Item -LiteralPath $p).VersionInfo }} else {{ $null }}
  [pscustomobject]@{{ i = $it.i; p = $p; co = $vi.CompanyName; d = $vi.FileDescription; e = $exists }}
}})
ConvertTo-Json -InputObject ([pscustomobject]@{{ tasks = $tasks; info = $info }}) -Depth 4 -Compress
"#
    );
    let out = ps::powershell(&script)?;
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada al listar Inicio: {e}"))
}

fn list() -> Result<Vec<StartupItem>, String> {
    let mut items = collect_local();
    let enriched = enrich(&items)?;
    for t in enriched.tasks {
        items.push(StartupItem {
            id: format!("task|{}|{}", t.path, t.name),
            name: t.name.clone(),
            command: t.command,
            location: match t.path.trim_matches('\\') {
                "" => "Tarea programada".into(),
                folder => format!("Tarea programada ({folder})"),
            },
            source: "task",
            enabled: t.enabled,
            needs_admin: true,
            publisher: None,
            description: None,
            target: None,
            approved: None,
            task: Some((t.path, t.name)),
        });
    }
    let info: HashMap<String, ProbeResult> = enriched.info.into_iter().map(|p| (p.i.clone(), p)).collect();
    for it in &mut items {
        if let Some(p) = info.get(&it.id) {
            let clean = |s: &Option<String>| s.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            it.publisher = clean(&p.co);
            it.description = clean(&p.d);
            it.target = if p.e { p.p.clone() } else { None };
        }
    }
    items.sort_by_key(|i| (!i.enabled, i.name.to_lowercase()));
    Ok(items)
}

/// Lo que arranca con Windows: los nombres (para el informe) y cuántos son de
/// terceros. Lo de Microsoft (OneDrive, Seguridad, Teams…) viene con Windows u
/// Office: lo que alarga el arranque y se puede quitar es lo demás.
pub fn enabled_overview() -> Result<(Vec<String>, usize), String> {
    let items: Vec<StartupItem> = list()?.into_iter().filter(|i| i.enabled).collect();
    let third_party = items.iter().filter(|i| !is_microsoft(i.publisher.as_deref())).count();
    Ok((items.into_iter().map(|i| i.description.unwrap_or(i.name)).collect(), third_party))
}

fn is_microsoft(publisher: Option<&str>) -> bool {
    publisher.is_some_and(|p| p.to_lowercase().starts_with("microsoft"))
}

/// Nombres visibles de lo que arranca con Windows (para el informe).
pub fn enabled_names() -> Result<Vec<String>, String> {
    Ok(list()?.into_iter().filter(|i| i.enabled).map(|i| i.description.unwrap_or(i.name)).collect())
}

#[tauri::command(async)]
pub fn list_startup() -> Result<Vec<StartupItem>, String> {
    list()
}

pub fn set_task_enabled(path: &str, name: &str, enabled: bool) -> Result<(), String> {
    let verb = if enabled { "Enable" } else { "Disable" };
    let (p, n) = (path.replace('\'', "''"), name.replace('\'', "''"));
    ps::powershell(&format!("{verb}-ScheduledTask -TaskPath '{p}' -TaskName '{n}' | Out-Null")).map(|_| ())
}

/// Mismo formato que escribe el Administrador de tareas: 4 bytes de estado + FILETIME.
fn approved_bytes(enabled: bool) -> Vec<u8> {
    let mut b = vec![if enabled { 2 } else { 3 }, 0, 0, 0];
    let filetime = if enabled {
        0
    } else {
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        (unix.as_secs() + 11_644_473_600) * 10_000_000
    };
    b.extend_from_slice(&filetime.to_le_bytes());
    b
}

#[tauri::command(async)]
pub fn set_startup_enabled(id: String, enabled: bool, state: State<'_, TweakState>) -> Result<(), String> {
    // El id solo sirve para buscar: se vuelve a listar y se usa la entrada real.
    let item = list()?.into_iter().find(|i| i.id == id).ok_or("La entrada ya no existe")?;
    if item.needs_admin && !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let title = format!("Inicio: {} {}", if enabled { "activar" } else { "desactivar" }, item.name);
    let mut e = entry(Op::Apply, None, &title);
    let result = if let Some((key, name)) = &item.approved {
        let previous = registry::read_raw(key, name);
        let raw = registry::RawValue { vtype: 3, bytes: approved_bytes(enabled) };
        registry::write_raw(key, name, &raw)
            .map(|_| vec![Backup::Registry { path: key.clone(), name: name.clone(), previous }])
    } else if let Some((path, name)) = &item.task {
        set_task_enabled(path, name, enabled)
            .map(|_| vec![Backup::Task { path: path.clone(), name: name.clone(), was_enabled: item.enabled }])
    } else {
        Err("Tipo de entrada desconocido".into())
    };
    match result {
        Ok(backups) => {
            e.backups = backups;
            state.log(e);
            Ok(())
        }
        Err(err) => {
            e.ok = false;
            e.message = Some(err.clone());
            state.log(e);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn approved_bytes_match_task_manager_format() {
        let on = super::approved_bytes(true);
        let off = super::approved_bytes(false);
        assert_eq!(on.len(), 12);
        assert_eq!(on[0] & 1, 0);
        assert_eq!(off[0] & 1, 1);
    }

    /// Lee el Inicio real del equipo (solo lectura): `cargo test -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn list_real_startup() {
        let items = super::list().unwrap();
        for i in &items {
            println!("{:<5} {:<30} {:<28} {:<25} {}", i.enabled, i.name, i.location, i.publisher.clone().unwrap_or_default(), i.target.is_some());
        }
        assert!(!items.is_empty());
    }
}
