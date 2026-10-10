//! Todo lo que arranca con Windows, como Autoruns: claves del registro,
//! carpetas de Inicio, tareas programadas, servicios y drivers que no son de
//! Microsoft, el Winlogon, los «depuradores» de programas (IFEO), AppInit,
//! extensiones del Explorador y suscripciones WMI. Cada entrada con su editor,
//! si está firmada y si el archivo existe. Se resalta lo que no lleva firma, lo
//! que apunta a carpetas temporales y lo huérfano. Desactivar no borra nada: lo
//! que se toca queda en el diario con su «Deshacer».

use crate::tweaks::journal::Backup;
use crate::tweaks::model::{RegData, RegKind, Startup};
use crate::tweaks::{registry, TweakState};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
struct RawItem {
    category: String,
    /// Dónde está (clave, carpeta, ruta de la tarea…).
    location: String,
    name: String,
    command: String,
    /// Imagen ya resuelta si el script la sabe (servicios, drivers, accesos directos).
    image: String,
    enabled: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
struct Signature {
    path: String,
    status: String,
    signer: String,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct Raw {
    items: Vec<RawItem>,
    signatures: Vec<Signature>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Autorun {
    pub id: String,
    /// run · startup · task · service · driver · winlogon · ifeo · appinit · explorer · wmi
    pub category: String,
    pub location: String,
    pub name: String,
    pub command: String,
    pub image: String,
    pub exists: bool,
    pub signed: bool,
    pub publisher: String,
    pub enabled: bool,
    /// Lo que llama la atención («sin firma», «en Temp», «el archivo no existe»…).
    pub flags: Vec<String>,
    /// none · low · high
    pub concern: &'static str,
    pub can_disable: bool,
}

const SCRIPT: &str = r#"
$items = New-Object System.Collections.ArrayList
function Add($cat, $loc, $name, $cmd, $img, $en) { [void]$items.Add([pscustomobject]@{ category = $cat; location = $loc; name = "$name"; command = "$cmd"; image = "$img"; enabled = [bool]$en }) }
$runs = @("$UserHive\Software\Microsoft\Windows\CurrentVersion\Run", "$UserHive\Software\Microsoft\Windows\CurrentVersion\RunOnce", 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run', 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce', 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run', "$UserHive\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run")
foreach ($k in $runs) {
  $p = Get-ItemProperty $k -ErrorAction SilentlyContinue
  if (-not $p) { continue }
  foreach ($v in $p.PSObject.Properties) { if ($v.Name -notlike 'PS*') { Add 'run' ($k -replace '^Registry::HKEY_USERS\\[^\\]+', 'HKCU' -replace '^HKCU:', 'HKCU' -replace '^HKLM:', 'HKLM') $v.Name $v.Value '' $true } }
}
$sh = New-Object -ComObject WScript.Shell
foreach ($d in @((Join-Path $UserAppData 'Microsoft\Windows\Start Menu\Programs\Startup'), "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup")) {
  foreach ($f in @(Get-ChildItem $d -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne 'desktop.ini' })) {
    $t = if ($f.Extension -eq '.lnk') { try { $sh.CreateShortcut($f.FullName).TargetPath } catch { '' } } else { $f.FullName }
    Add 'startup' $d $f.Name $f.FullName $t $true
  }
}
foreach ($t in @(Get-ScheduledTask -ErrorAction SilentlyContinue | Where-Object { $_.TaskPath -notlike '\Microsoft\*' })) {
  foreach ($a in @($t.Actions | Where-Object { $_.Execute })) { Add 'task' $t.TaskPath $t.TaskName "$($a.Execute) $($a.Arguments)".Trim() ([Environment]::ExpandEnvironmentVariables($a.Execute)) ("$($t.State)" -ne 'Disabled') }
}
foreach ($s in @(Get-CimInstance Win32_Service -ErrorAction SilentlyContinue | Where-Object { $_.StartMode -eq 'Auto' })) { Add 'service' 'Servicios' $s.Name "$($s.PathName)" '' $true }
foreach ($s in @(Get-CimInstance Win32_SystemDriver -ErrorAction SilentlyContinue | Where-Object { $_.StartMode -in 'Boot', 'System', 'Auto' })) { Add 'driver' 'Drivers' $s.Name "$($s.PathName)" '' $true }
$wl = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' -ErrorAction SilentlyContinue
if ($wl) { Add 'winlogon' 'HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' 'Shell' $wl.Shell '' $true; Add 'winlogon' 'HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon' 'Userinit' $wl.Userinit '' $true }
foreach ($k in @(Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options' -ErrorAction SilentlyContinue)) {
  $dbg = (Get-ItemProperty $k.PSPath -ErrorAction SilentlyContinue).Debugger
  if ($dbg) { Add 'ifeo' ($k.Name -replace '^HKEY_LOCAL_MACHINE', 'HKLM') $k.PSChildName $dbg '' $true }
}
foreach ($k in @('HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows NT\CurrentVersion\Windows')) {
  $a = (Get-ItemProperty $k -ErrorAction SilentlyContinue).AppInit_DLLs
  if ($a) { Add 'appinit' ($k -replace '^HKLM:', 'HKLM') 'AppInit_DLLs' $a '' $true }
}
foreach ($k in @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Browser Helper Objects', 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\ShellServiceObjectDelayLoad')) {
  foreach ($c in @(Get-ChildItem $k -ErrorAction SilentlyContinue)) {
    $clsid = $c.PSChildName
    $dll = (Get-ItemProperty "Registry::HKEY_CLASSES_ROOT\CLSID\$clsid\InprocServer32" -ErrorAction SilentlyContinue).'(default)'
    Add 'explorer' ($k -replace '^HKLM:', 'HKLM') $clsid "$dll" "$dll" $true
  }
}
try {
  foreach ($c in @(Get-CimInstance -Namespace root\subscription -ClassName CommandLineEventConsumer -ErrorAction Stop)) { Add 'wmi' 'root\subscription' $c.Name "$($c.CommandLineTemplate)" "$($c.ExecutablePath)" $true }
  foreach ($c in @(Get-CimInstance -Namespace root\subscription -ClassName ActiveScriptEventConsumer -ErrorAction Stop)) { Add 'wmi' 'root\subscription' $c.Name "Script $($c.ScriptingEngine)" '' $true }
} catch { }
$paths = @($items | ForEach-Object { $_.image } | Where-Object { $_ } | Select-Object -Unique | Select-Object -First 500)
$sigs = @($paths | ForEach-Object {
  $s = Get-AuthenticodeSignature -LiteralPath $_ -ErrorAction SilentlyContinue
  $subj = "$($s.SignerCertificate.Subject)"
  $o = if ($subj -match 'O="?([^",]+)') { $Matches[1] } elseif ($subj -match 'CN="?([^",]+)') { $Matches[1] } else { '' }
  [pscustomobject]@{ path = $_; status = "$($s.Status)"; signer = $o }
})
[pscustomobject]@{ items = $items; signatures = $sigs } | ConvertTo-Json -Depth 4 -Compress
"#;

/// Expande las variables de entorno habituales de una ruta.
pub fn expand(path: &str) -> String {
    let mut out = path.to_string();
    for var in ["SystemRoot", "windir", "ProgramFiles", "ProgramFiles(x86)", "ProgramData", "SystemDrive", "LOCALAPPDATA", "APPDATA", "USERPROFILE", "CommonProgramFiles"] {
        let pat = format!("%{var}%");
        if out.to_ascii_lowercase().contains(&pat.to_ascii_lowercase()) {
            if let Ok(v) = std::env::var(var) {
                let i = out.to_ascii_lowercase().find(&pat.to_ascii_lowercase()).unwrap_or(0);
                out.replace_range(i..i + pat.len(), &v);
            }
        }
    }
    out
}

/// El ejecutable de una línea de órdenes («"C:\A B\x.exe" -arg», «C:\x.exe /s», «rundll32 a.dll,Entrada»).
pub fn exe_of(command: &str) -> String {
    let c = command.trim();
    let raw = if let Some(rest) = c.strip_prefix('"') {
        rest.split('"').next().unwrap_or("").to_string()
    } else {
        // Sin comillas: hasta el primer «.exe»/«.dll»… o el primer espacio.
        let lower = c.to_ascii_lowercase();
        let cut = [".exe", ".dll", ".sys", ".bat", ".cmd", ".ps1", ".vbs", ".js", ".com", ".scr"].iter().filter_map(|e| lower.find(e).map(|i| i + e.len())).min();
        match cut {
            Some(i) => c[..i].to_string(),
            None => c.split_whitespace().next().unwrap_or("").to_string(),
        }
    };
    let raw = raw.trim_start_matches(r"\??\").replace(r"\SystemRoot\", &format!("{}\\", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())));
    let mut p = expand(&raw);
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    if p.to_ascii_lowercase().starts_with(r"system32\") {
        p = format!(r"{root}\{p}");
    } else if !p.is_empty() && !p.contains(':') && !p.starts_with('\\') {
        // «explorer.exe», «rundll32.exe»: se buscan donde los busca Windows.
        if let Some(found) = [format!(r"{root}\System32\{p}"), format!(r"{root}\{p}")].into_iter().find(|c| std::path::Path::new(c).exists()) {
            p = found;
        }
    }
    p
}

/// Lo que llama la atención de una entrada y cuánto preocupa.
pub fn assess(category: &str, name: &str, command: &str, image: &str, exists: bool, signed: bool, publisher: &str) -> (Vec<String>, &'static str) {
    let mut flags = Vec::new();
    let lower = format!("{} {}", command.to_ascii_lowercase(), image.to_ascii_lowercase());
    let mut high = false;
    if !image.is_empty() && !exists {
        flags.push("el archivo no existe (entrada huérfana)".to_string());
    }
    if exists && !signed {
        flags.push("sin firma digital".to_string());
    }
    if lower.contains(r"\temp\") || lower.contains(r"\appdata\local\temp") || lower.contains(r"\windows\temp") {
        flags.push("arranca desde una carpeta temporal".to_string());
        high = true;
    } else if lower.contains(r"\appdata\roaming\") && !signed {
        flags.push("en AppData sin firma".to_string());
        high = true;
    }
    if lower.contains("powershell") && (lower.contains(" -e ") || lower.contains("-enc") || lower.contains("hidden")) {
        flags.push("PowerShell oculto o codificado".to_string());
        high = true;
    }
    match category {
        "winlogon" => {
            let ok = match name {
                "Shell" => command.trim().eq_ignore_ascii_case("explorer.exe"),
                _ => command.trim().trim_end_matches(',').to_ascii_lowercase().ends_with(r"system32\userinit.exe"),
            };
            if !ok {
                flags.push(format!("{name} no es el de Windows"));
                high = true;
            }
        }
        "ifeo" => {
            flags.push(format!("«{name}» se abre a través de otro programa"));
            high = true;
        }
        "appinit" => {
            flags.push("DLL cargada en todos los programas".to_string());
            high = true;
        }
        "wmi" => {
            flags.push("suscripción WMI: algo se ejecuta ante un evento".to_string());
            high = true;
        }
        _ => {}
    }
    let concern = if high {
        "high"
    } else if flags.is_empty() || (signed && publisher.to_ascii_lowercase().contains("microsoft")) {
        "none"
    } else {
        "low"
    };
    (flags, concern)
}

fn microsoft(publisher: &str) -> bool {
    publisher.to_ascii_lowercase().contains("microsoft")
}

fn list_raw() -> Result<Raw, String> {
    let script = format!("{}{SCRIPT}", crate::target_user::script_prelude());
    let out = crate::pspool::query(&script, Some(Duration::from_secs(180)), "Todo lo que arranca con Windows")?;
    serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))
}

fn build(raw: Raw) -> Vec<Autorun> {
    let mut out = Vec::new();
    for (i, it) in raw.items.into_iter().enumerate() {
        let image = if it.image.is_empty() { exe_of(&it.command) } else { expand(&it.image) };
        let sig = raw.signatures.iter().find(|s| s.path.eq_ignore_ascii_case(&image));
        let exists = !image.is_empty() && std::path::Path::new(&image).exists();
        let signed = sig.is_some_and(|s| s.status == "Valid");
        let publisher = sig.map(|s| s.signer.clone()).unwrap_or_default();
        // Servicios y drivers de Microsoft firmados: son cientos y no aportan nada.
        if matches!(it.category.as_str(), "service" | "driver") && signed && microsoft(&publisher) {
            continue;
        }
        let (flags, concern) = assess(&it.category, &it.name, &it.command, &image, exists, signed, &publisher);
        let can_disable = matches!(it.category.as_str(), "run" | "startup" | "task" | "service" | "ifeo" | "appinit") && it.enabled;
        out.push(Autorun { id: format!("{}:{}:{}", it.category, i, it.name), category: it.category, location: it.location, name: it.name, command: it.command, image, exists, signed, publisher, enabled: it.enabled, flags, concern, can_disable });
    }
    out.sort_by_key(|a| (match a.concern { "high" => 0, "low" => 1, _ => 2 }, a.category.clone(), a.name.to_ascii_lowercase()));
    out
}

#[tauri::command(async)]
pub fn autoruns_list() -> Result<Vec<Autorun>, String> {
    Ok(build(list_raw()?))
}

const APPROVED_FOLDER_USER: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";
const APPROVED_FOLDER_ALL: &str = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";

/// StartupApproved: mismo formato que el Administrador de tareas (impar = deshabilitado).
fn approved_off() -> Vec<u8> {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut b = vec![3, 0, 0, 0];
    b.extend_from_slice(&((unix.as_secs() + 11_644_473_600) * 10_000_000).to_le_bytes());
    b
}

fn disable(a: &Autorun) -> Result<Vec<Backup>, String> {
    match a.category.as_str() {
        "run" => {
            let previous = registry::read_raw(&a.location, &a.name);
            registry::delete(&a.location, &a.name)?;
            Ok(vec![Backup::Registry { path: a.location.clone(), name: a.name.clone(), previous }])
        }
        "startup" => {
            let key = if a.location.to_ascii_lowercase().contains("programdata") { APPROVED_FOLDER_ALL } else { APPROVED_FOLDER_USER };
            let previous = registry::read_raw(key, &a.name);
            registry::write_raw(key, &a.name, &registry::RawValue { vtype: 3, bytes: approved_off() })?;
            Ok(vec![Backup::Registry { path: key.into(), name: a.name.clone(), previous }])
        }
        "task" => {
            crate::tweaks::startup::set_task_enabled(&a.location, &a.name, false)?;
            Ok(vec![Backup::Task { path: a.location.clone(), name: a.name.clone(), was_enabled: true }])
        }
        "service" => {
            let startup = crate::tweaks::service::startup(&a.name).unwrap_or(Startup::Automatic);
            let was_running = crate::tweaks::service::is_running(&a.name);
            crate::tweaks::service::set_startup(&a.name, Startup::Disabled)?;
            crate::tweaks::service::stop(&a.name);
            Ok(vec![Backup::Service { name: a.name.clone(), startup, was_running }])
        }
        "ifeo" => {
            let previous = registry::read_raw(&a.location, "Debugger");
            registry::delete(&a.location, "Debugger")?;
            Ok(vec![Backup::Registry { path: a.location.clone(), name: "Debugger".into(), previous }])
        }
        "appinit" => {
            let previous = registry::read_raw(&a.location, "AppInit_DLLs");
            registry::write(&a.location, "AppInit_DLLs", RegKind::String, &RegData::Str(String::new()))?;
            Ok(vec![Backup::Registry { path: a.location.clone(), name: "AppInit_DLLs".into(), previous }])
        }
        _ => Err("Esta entrada no se desactiva desde aquí (se enseña para que sepas que existe).".into()),
    }
}

/// Desactiva una entrada (sin borrar el programa). Queda en el diario con «Deshacer».
#[tauri::command(async)]
pub fn autorun_disable(id: String, state: State<'_, TweakState>) -> Result<String, String> {
    let all = build(list_raw()?);
    // El id lleva la posición en la lista: se busca por categoría y nombre por si la lista cambió.
    let (cat, rest) = id.split_once(':').ok_or("Entrada no válida.")?;
    let name = rest.split_once(':').map(|(_, n)| n).unwrap_or(rest);
    let a = all.iter().find(|a| a.id == id).or_else(|| all.iter().find(|a| a.category == cat && a.name == name)).ok_or("La entrada ya no existe (vuelve a cargar la lista).")?;
    if !a.can_disable {
        return Err("Esta entrada no se puede desactivar desde aquí.".into());
    }
    let needs_admin = !a.location.to_ascii_uppercase().starts_with("HKCU") && a.category != "startup" || a.location.to_ascii_lowercase().contains("programdata");
    if needs_admin && !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let r = disable(a);
    let title = format!("Arranque: desactivar «{}»", a.name);
    let (backups, result) = match r {
        Ok(b) => (b, Ok(())),
        Err(e) => (vec![], Err(e)),
    };
    state.log_change(&title, backups, &result);
    result.map(|()| format!("«{}» ya no arrancará con Windows. Se puede deshacer desde el aviso o el Historial.", a.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saca_el_ejecutable() {
        assert_eq!(exe_of(r#""C:\Program Files\App\app.exe" --minimized"#), r"C:\Program Files\App\app.exe");
        assert_eq!(exe_of(r"C:\Tools\x.exe /s"), r"C:\Tools\x.exe");
        assert_eq!(exe_of(r"C:\Program Files\App Sin Comillas\app.exe -a"), r"C:\Program Files\App Sin Comillas\app.exe");
        assert!(exe_of(r"system32\drivers\x.sys").to_ascii_lowercase().ends_with(r"system32\drivers\x.sys"));
    }

    #[test]
    fn valora_lo_que_preocupa() {
        let (f, c) = assess("run", "Updater", r"C:\Users\a\AppData\Local\Temp\sh.exe", r"C:\Users\a\AppData\Local\Temp\sh.exe", true, false, "");
        assert_eq!(c, "high");
        assert!(f.iter().any(|x| x.contains("temporal")));
        let (_, c) = assess("run", "OneDrive", "x", r"C:\Program Files\Microsoft OneDrive\OneDrive.exe", true, true, "Microsoft Corporation");
        assert_eq!(c, "none");
        let (f, c) = assess("run", "Viejo", "x", r"C:\No\Existe.exe", false, false, "");
        assert_eq!(c, "low");
        assert!(f[0].contains("huérfana"));
        let (_, c) = assess("winlogon", "Shell", "explorer.exe", "", false, false, "");
        assert_eq!(c, "none");
        let (_, c) = assess("winlogon", "Shell", r"explorer.exe, C:\x\y.exe", "", false, false, "");
        assert_eq!(c, "high");
        assert_eq!(assess("ifeo", "sethc.exe", "cmd.exe", "", true, true, "Microsoft").1, "high");
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
