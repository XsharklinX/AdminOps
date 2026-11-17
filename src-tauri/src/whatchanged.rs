//! «¿Qué cambió?»: cada día AdminOps guarda una foto pequeña del equipo
//! (programas, drivers, actualizaciones, servicios, lo que arranca solo y los
//! ajustes de energía) y dice qué es distinto desde la fecha que se elija.
//!
//! Para que ocupe unos KB por día, no se guarda una foto entera cada día: se
//! guarda la última foto completa y, cada día, solo lo que cambió respecto a la
//! anterior (60 días). «Desde el martes» es la suma neta de esos cambios: lo que
//! se instaló y se quitó después no aparece. Solo lee; todo queda en los datos
//! de este equipo.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// Días de cambios que se guardan.
const KEEP_DAYS: i64 = 60;

static FILE: Mutex<()> = Mutex::new(());
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Tipos de cosas que se vigilan, en el orden en que se enseñan.
const KINDS: &[&str] = &["program", "driver", "update", "service", "startup", "power"];

/// Una cosa del equipo: un programa y su versión, un servicio y su modo de inicio…
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Item {
    /// program · driver · update · service · startup · power
    pub kind: String,
    pub key: String,
    pub label: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
struct Snapshot {
    /// Día de esta foto (AAAA-MM-DD).
    day: String,
    /// Primer día en que se hizo una foto en este equipo.
    first_day: String,
    taken: u64,
    items: Vec<Item>,
}

/// Qué cambió entre dos fotos.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Change {
    pub kind: String,
    pub key: String,
    pub label: String,
    /// Valor antes (None: no existía).
    pub old: Option<String>,
    /// Valor después (None: ya no está).
    pub new: Option<String>,
    /// Día en que se vio el cambio.
    pub day: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
struct Delta {
    day: String,
    taken: u64,
    changes: Vec<Change>,
}

// ---------- Lógica pura ----------

/// Lo que cambió de una foto a la siguiente.
pub fn diff(old: &[Item], new: &[Item], day: &str) -> Vec<Change> {
    let key = |i: &Item| (i.kind.clone(), i.key.clone());
    let a: BTreeMap<_, _> = old.iter().map(|i| (key(i), i)).collect();
    let b: BTreeMap<_, _> = new.iter().map(|i| (key(i), i)).collect();
    let mut out = Vec::new();
    for (k, n) in &b {
        match a.get(k) {
            None => out.push(Change { kind: k.0.clone(), key: k.1.clone(), label: n.label.clone(), old: None, new: Some(n.value.clone()), day: day.into() }),
            Some(o) if o.value != n.value => out.push(Change { kind: k.0.clone(), key: k.1.clone(), label: n.label.clone(), old: Some(o.value.clone()), new: Some(n.value.clone()), day: day.into() }),
            _ => {}
        }
    }
    for (k, o) in &a {
        if !b.contains_key(k) {
            out.push(Change { kind: k.0.clone(), key: k.1.clone(), label: o.label.clone(), old: Some(o.value.clone()), new: None, day: day.into() });
        }
    }
    out
}

/// Si una categoría no se pudo leer hoy (sin ningún elemento) pero ayer sí, se
/// conserva la de ayer: no es que se haya quitado todo.
pub fn keep_unreadable(old: &[Item], mut new: Vec<Item>) -> Vec<Item> {
    for k in KINDS {
        if !new.iter().any(|i| i.kind == *k) {
            new.extend(old.iter().filter(|i| i.kind == *k).cloned());
        }
    }
    new
}

/// La suma neta de los cambios de los días `>= since` (ordenados por día): lo
/// que se instaló y se quitó después, o se cambió y se devolvió, no cuenta.
fn net_since(deltas: &[Delta], since: &str) -> Vec<Change> {
    let mut days: Vec<&Delta> = deltas.iter().filter(|d| d.day.as_str() >= since).collect();
    days.sort_by(|a, b| a.day.cmp(&b.day));
    // (tipo, clave) → primer «antes», último «después» y último cambio visto.
    let mut acc: BTreeMap<(String, String), Change> = BTreeMap::new();
    for d in days {
        for c in &d.changes {
            let k = (c.kind.clone(), c.key.clone());
            match acc.get_mut(&k) {
                Some(e) => {
                    e.new = c.new.clone();
                    e.label = c.label.clone();
                    e.day = c.day.clone();
                }
                None => {
                    acc.insert(k, c.clone());
                }
            }
        }
    }
    let mut v: Vec<Change> = acc.into_values().filter(|c| c.old != c.new).collect();
    v.sort_by(|a, b| {
        let ka = KINDS.iter().position(|k| *k == a.kind).unwrap_or(99);
        let kb = KINDS.iter().position(|k| *k == b.kind).unwrap_or(99);
        ka.cmp(&kb).then(b.day.cmp(&a.day)).then(a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    v
}

/// Dónde se arregla en AdminOps, si ya hay un sitio.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub label: String,
    pub page: String,
    pub focus: Option<String>,
}

fn fix_for(c: &Change) -> Option<Fix> {
    let fix = |label: &str, page: &str, focus: &str| Some(Fix { label: label.into(), page: page.into(), focus: Some(focus.into()) });
    let removed = c.new.is_none();
    match c.kind.as_str() {
        "program" if !removed => fix("Desinstalar", "apps", "uninstall"),
        "driver" if !removed => fix("Volver al driver anterior", "machine", "boots"),
        "service" => fix("Devolver el servicio", "tweaks", "services"),
        "startup" if !removed => fix("Quitar del inicio", "tweaks", "startup"),
        "update" if !removed => fix("Ver actualizaciones", "apps", "winupdate"),
        _ => None,
    }
}

/// Un cambio tal como se enseña en la interfaz.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeView {
    pub kind: String,
    /// added · removed · changed
    pub change: &'static str,
    pub label: String,
    pub old: Option<String>,
    pub new: Option<String>,
    pub day: String,
    pub fix: Option<Fix>,
}

fn view(c: Change) -> ChangeView {
    let change = match (&c.old, &c.new) {
        (None, _) => "added",
        (_, None) => "removed",
        _ => "changed",
    };
    let fix = fix_for(&c);
    let clean = |s: Option<String>| s.map(|t| crate::escalate::scrub(&t));
    ChangeView { kind: c.kind, change, label: crate::escalate::scrub(&c.label), old: clean(c.old), new: clean(c.new), day: c.day, fix }
}

// ---------- Ajustes de energía ----------

/// Nombre de los ajustes de energía que más se tocan (subgrupo/ajuste).
fn power_label(id: &str) -> Option<(&'static str, bool)> {
    // (nombre, es un tiempo en segundos)
    Some(match id.to_ascii_lowercase().as_str() {
        "238c9fa8-0aad-41ed-83f4-97be242c8f20/29f6c1db-86da-48c5-9fdb-f2b67b1f44da" => ("Suspender tras estar inactivo", true),
        "238c9fa8-0aad-41ed-83f4-97be242c8f20/9d7815a6-7ee4-497e-8888-515a05f02364" => ("Hibernar tras estar inactivo", true),
        "7516b95f-f776-4464-8c53-06167f40cc99/3c0bc021-c8a8-4e07-a973-6b14cbcb2b7e" => ("Apagar la pantalla tras", true),
        "0012ee47-9041-4b5d-9b77-535fba8b1442/6738e2c4-e8a5-4a42-b16a-e040e769756e" => ("Apagar el disco duro tras", true),
        "54533251-82be-4824-96c1-47b60b740d00/893dee8e-2bef-41e0-89c6-b55d0929964c" => ("Estado mínimo del procesador (%)", false),
        "54533251-82be-4824-96c1-47b60b740d00/bc5038f7-23e0-4960-96da-33abaf5935ec" => ("Estado máximo del procesador (%)", false),
        "4f971e89-eebd-4455-a8de-9e59040e7347/5ca83367-6e45-459f-a27b-476b1d01c936" => ("Al cerrar la tapa", false),
        "4f971e89-eebd-4455-a8de-9e59040e7347/7648efa3-dd9c-4e3e-b566-50f929386280" => ("Al pulsar el botón de encendido", false),
        _ => return None,
    })
}

fn secs(v: &str) -> String {
    match v.trim().parse::<u64>() {
        Ok(0) => "nunca".into(),
        Ok(s) if s % 60 == 0 && s >= 60 => format!("{} min", s / 60),
        Ok(s) => format!("{s} s"),
        Err(_) => v.to_string(),
    }
}

/// «CA|CC» crudo → «con corriente: 30 min · con batería: 10 min».
pub fn power_value(id: &str, raw: &str) -> String {
    let (ac, dc) = raw.split_once('|').unwrap_or((raw, ""));
    let timed = power_label(id).is_some_and(|(_, t)| t);
    let f = |v: &str| if v.is_empty() { None } else { Some(if timed { secs(v) } else { v.to_string() }) };
    match (f(ac), f(dc)) {
        (Some(a), Some(d)) => format!("con corriente: {a} · con batería: {d}"),
        (Some(a), None) => format!("con corriente: {a}"),
        (None, Some(d)) => format!("con batería: {d}"),
        (None, None) => String::new(),
    }
}

// ---------- Leer el equipo ----------

const SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
function Item($k, $l, $v) { [pscustomobject]@{ k = "$k"; l = "$l"; v = "$v" } }
$drivers = @(Get-CimInstance Win32_PnPSignedDriver | Where-Object { $_.DeviceName -and $_.DriverVersion } | Sort-Object DeviceName -Unique | ForEach-Object { Item $_.DeviceName $_.DeviceName $_.DriverVersion })
$updates = @(Get-CimInstance Win32_QuickFixEngineering | Where-Object { $_.HotFixID } | ForEach-Object { Item $_.HotFixID ("$($_.HotFixID) $($_.Description)") $(if ($_.InstalledOn) { $_.InstalledOn.ToString('yyyy-MM-dd') } else { '' }) })
$services = @(Get-CimInstance Win32_Service | ForEach-Object { Item $_.Name $_.DisplayName $_.StartMode })
$startup = @()
foreach ($k in 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run', 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run') {
  $p = Get-ItemProperty -LiteralPath $k
  if ($p) { foreach ($n in $p.PSObject.Properties) { if ($n.Name -notlike 'PS*') { $startup += Item ($k + '|' + $n.Name) $n.Name $n.Value } } }
}
foreach ($d in @([Environment]::GetFolderPath('CommonStartup'), [Environment]::GetFolderPath('Startup'))) {
  if ($d) { Get-ChildItem -LiteralPath $d -File | ForEach-Object { $startup += Item ($d + '|' + $_.Name) $_.Name $_.Name } }
}
$power = @()
$schemes = 'HKLM:\SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes'
$active = (Get-ItemProperty -LiteralPath $schemes).ActivePowerScheme
$name = ''
try { $o = (& powercfg.exe /getactivescheme) -join ' '; if ($o -match '\(([^)]+)\)') { $name = $Matches[1] } } catch { }
if ($name) { $power += Item 'plan' 'Plan de energía' $name }
if ($active) {
  foreach ($sub in Get-ChildItem -LiteralPath ($schemes + '\' + $active)) {
    foreach ($set in Get-ChildItem -LiteralPath $sub.PSPath) {
      $p = Get-ItemProperty -LiteralPath $set.PSPath
      if ($null -ne $p.ACSettingIndex -or $null -ne $p.DCSettingIndex) {
        $id = $sub.PSChildName + '/' + $set.PSChildName
        $power += Item $id $id ("$($p.ACSettingIndex)|$($p.DCSettingIndex)")
      }
    }
  }
}
$fast = (Get-ItemProperty -LiteralPath 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Power').HiberbootEnabled
if ($null -ne $fast) { $power += Item 'faststartup' 'Inicio rápido de Windows' $(if ($fast -eq 1) { 'activado' } else { 'desactivado' }) }
ConvertTo-Json -InputObject ([pscustomobject]@{ drivers = $drivers; updates = $updates; services = $services; startup = $startup; power = $power }) -Depth 4 -Compress
"#;

#[derive(Deserialize, Default)]
struct RawItem {
    k: String,
    l: String,
    v: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Raw {
    drivers: Vec<RawItem>,
    updates: Vec<RawItem>,
    services: Vec<RawItem>,
    startup: Vec<RawItem>,
    power: Vec<RawItem>,
}

/// Una lista de PowerShell con un solo elemento puede llegar como objeto suelto.
fn lenient(json: &str) -> Raw {
    let mut v: serde_json::Value = serde_json::from_str(json.trim()).unwrap_or_default();
    if let Some(o) = v.as_object_mut() {
        for val in o.values_mut() {
            if val.is_object() {
                *val = serde_json::Value::Array(vec![val.take()]);
            }
        }
    }
    serde_json::from_value(v).unwrap_or_default()
}

fn items_from(raw: Raw) -> Vec<Item> {
    let mut out = Vec::new();
    let mut add = |kind: &str, list: Vec<RawItem>, f: &dyn Fn(&RawItem) -> RawItem| {
        for r in list.iter().filter(|r| !r.k.trim().is_empty()) {
            let r = f(r);
            out.push(Item { kind: kind.into(), key: r.k, label: r.l, value: r.v });
        }
    };
    let same = |r: &RawItem| RawItem { k: r.k.clone(), l: r.l.clone(), v: r.v.clone() };
    add("driver", raw.drivers, &same);
    add("update", raw.updates, &same);
    add("service", raw.services, &same);
    add("startup", raw.startup, &same);
    add("power", raw.power, &|r| match power_label(&r.k) {
        Some((name, _)) => RawItem { k: r.k.clone(), l: name.into(), v: power_value(&r.k, &r.v) },
        None if r.k.contains('/') => RawItem { k: r.k.clone(), l: format!("Ajuste de energía ({})", r.k.chars().take(8).collect::<String>()), v: power_value(&r.k, &r.v) },
        None => same(r),
    });
    out
}

fn collect() -> Vec<Item> {
    let mut items: Vec<Item> = crate::programs::list()
        .into_iter()
        .filter(|p| !p.name.trim().is_empty())
        .map(|p| Item { kind: "program".into(), key: p.name.clone(), label: p.name, value: p.version.unwrap_or_default() })
        .collect();
    if let Ok(out) = crate::pspool::query(SCRIPT, Some(Duration::from_secs(180)), "¿Qué cambió?: foto del equipo") {
        items.extend(items_from(lenient(&out)));
    }
    items.sort_by(|a, b| (&a.kind, &a.key).cmp(&(&b.kind, &b.key)));
    items.dedup_by(|a, b| a.kind == b.kind && a.key == b.key);
    items
}

// ---------- Archivos ----------

fn dir(app: &tauri::AppHandle) -> PathBuf {
    paths::machine_data_dir(app).join("whatchanged")
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn valid_day(d: &str) -> bool {
    chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok()
}

fn load_deltas(dir: &std::path::Path) -> Vec<Delta> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<Delta> = rd
        .filter_map(Result::ok)
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.ends_with(".json") && valid_day(n.trim_end_matches(".json"))
        })
        .map(|e| paths::read_json::<Delta>(&e.path()))
        .filter(|d| valid_day(&d.day))
        .collect();
    v.sort_by(|a, b| a.day.cmp(&b.day));
    v
}

fn prune(dir: &std::path::Path) {
    let Some(limit) = chrono::Local::now().date_naive().checked_sub_signed(chrono::Duration::days(KEEP_DAYS)) else { return };
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(Result::ok) {
        let n = e.file_name().to_string_lossy().to_string();
        if let Ok(d) = chrono::NaiveDate::parse_from_str(n.trim_end_matches(".json"), "%Y-%m-%d") {
            if d < limit {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Hace la foto de hoy y apunta lo que cambió desde la anterior.
fn take(app: &tauri::AppHandle) -> Result<(), String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let r = (|| {
        let day = today();
        let d = dir(app);
        let old: Snapshot = paths::read_json(&d.join("current.json"));
        let items = keep_unreadable(&old.items, collect());
        if items.is_empty() {
            return Err("No se pudo leer el equipo.".to_string());
        }
        let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
        if !old.day.is_empty() {
            let changes = diff(&old.items, &items, &day);
            if !changes.is_empty() {
                let path = d.join(format!("{day}.json"));
                let mut delta: Delta = paths::read_json(&path);
                delta.day = day.clone();
                delta.taken = now();
                delta.changes.extend(changes);
                paths::write_json(&path, &delta)?;
            }
        }
        let first_day = if old.first_day.is_empty() { day.clone() } else { old.first_day };
        paths::write_json(&d.join("current.json"), &Snapshot { day, first_day, taken: now(), items })?;
        prune(&d);
        Ok(())
    })();
    RUNNING.store(false, Ordering::SeqCst);
    r
}

/// Una foto al día: unos minutos después de abrir AdminOps y, si sigue abierta, de nuevo al cambiar el día.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(150));
        loop {
            let done = {
                let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
                let s: Snapshot = paths::read_json(&dir(&app).join("current.json"));
                s.day == today()
            };
            if !done {
                if let Err(e) = take(&app) {
                    log::warn!("¿Qué cambió?: {e}");
                }
            }
            std::thread::sleep(Duration::from_secs(30 * 60));
        }
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatChangedInfo {
    /// Primer día con foto en este equipo (vacío: todavía ninguna).
    pub first_day: String,
    /// Último día con foto.
    pub last_day: String,
    /// Días en que cambió algo (los últimos 60).
    pub days: Vec<String>,
}

fn info_of(app: &tauri::AppHandle) -> WhatChangedInfo {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let d = dir(app);
    let s: Snapshot = paths::read_json(&d.join("current.json"));
    WhatChangedInfo { first_day: s.first_day, last_day: s.day, days: load_deltas(&d).into_iter().map(|x| x.day).collect() }
}

#[tauri::command(async)]
pub fn whatchanged_info(app: tauri::AppHandle) -> WhatChangedInfo {
    info_of(&app)
}

/// Hace la foto ahora (si hoy ya hay una, apunta lo que cambió desde ella).
#[tauri::command(async)]
pub fn whatchanged_snapshot(app: tauri::AppHandle) -> Result<WhatChangedInfo, String> {
    take(&app)?;
    Ok(info_of(&app))
}

/// Lo que es distinto desde ese día (AAAA-MM-DD), con su arreglo si ya existe en AdminOps.
#[tauri::command(async)]
pub fn whatchanged_since(app: tauri::AppHandle, day: String) -> Result<Vec<ChangeView>, String> {
    if !valid_day(&day) {
        return Err("Fecha no válida.".into());
    }
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    Ok(net_since(&load_deltas(&dir(&app)), &day).into_iter().map(view).collect())
}

/// Los cambios netos desde ese día (para «Pregúntale al equipo»).
pub fn changes_since(app: &tauri::AppHandle, day: &str) -> Vec<Change> {
    if !valid_day(day) {
        return vec![];
    }
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    net_since(&load_deltas(&dir(app)), day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(kind: &str, key: &str, value: &str) -> Item {
        Item { kind: kind.into(), key: key.into(), label: key.into(), value: value.into() }
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }

    #[test]
    fn diff_finds_added_removed_and_changed() {
        let old = vec![it("program", "A", "1"), it("program", "B", "1"), it("service", "S", "Auto")];
        let new = vec![it("program", "A", "2"), it("program", "C", "1"), it("service", "S", "Auto")];
        let d = diff(&old, &new, "2026-11-10");
        assert_eq!(d.len(), 3);
        assert!(d.iter().any(|c| c.key == "A" && c.old.as_deref() == Some("1") && c.new.as_deref() == Some("2")));
        assert!(d.iter().any(|c| c.key == "B" && c.new.is_none()));
        assert!(d.iter().any(|c| c.key == "C" && c.old.is_none()));
    }

    #[test]
    fn an_unreadable_category_is_not_a_mass_removal() {
        let old = vec![it("driver", "GPU", "1"), it("program", "A", "1")];
        let new = keep_unreadable(&old, vec![it("program", "A", "1")]);
        assert!(new.iter().any(|i| i.kind == "driver"));
        assert!(diff(&old, &new, "d").is_empty());
    }

    fn delta(day: &str, changes: Vec<Change>) -> Delta {
        Delta { day: day.into(), taken: 0, changes }
    }
    fn ch(key: &str, old: Option<&str>, new: Option<&str>, day: &str) -> Change {
        Change { kind: "program".into(), key: key.into(), label: key.into(), old: old.map(String::from), new: new.map(String::from), day: day.into() }
    }

    #[test]
    fn net_sum_ignores_what_was_undone_and_what_is_older() {
        let deltas = vec![
            delta("2026-11-01", vec![ch("Viejo", None, Some("1"), "2026-11-01")]),
            delta("2026-11-10", vec![ch("X", None, Some("1"), "2026-11-10"), ch("Y", Some("1"), Some("2"), "2026-11-10")]),
            delta("2026-11-12", vec![ch("X", Some("1"), None, "2026-11-12"), ch("Y", Some("2"), Some("3"), "2026-11-12")]),
        ];
        let n = net_since(&deltas, "2026-11-10");
        // X se instaló y se quitó: nada. Y pasó de 1 a 3. «Viejo» es anterior.
        assert_eq!(n.len(), 1);
        assert_eq!((n[0].old.as_deref(), n[0].new.as_deref(), n[0].day.as_str()), (Some("1"), Some("3"), "2026-11-12"));
        assert_eq!(net_since(&deltas, "2026-11-13").len(), 0);
    }

    #[test]
    fn net_sum_returns_to_the_start_means_no_change() {
        let deltas = vec![delta("2026-11-10", vec![ch("Y", Some("1"), Some("2"), "2026-11-10")]), delta("2026-11-11", vec![ch("Y", Some("2"), Some("1"), "2026-11-11")])];
        assert!(net_since(&deltas, "2026-11-01").is_empty());
    }

    #[test]
    fn fixes_exist_only_where_the_app_can_do_something() {
        let added = Change { kind: "program".into(), key: "k".into(), label: "k".into(), old: None, new: Some("1".into()), day: "d".into() };
        assert_eq!(fix_for(&added).map(|f| f.page), Some("apps".into()));
        let removed = Change { new: None, old: Some("1".into()), ..added.clone() };
        assert!(fix_for(&removed).is_none());
        let svc = Change { kind: "service".into(), old: Some("Auto".into()), new: Some("Disabled".into()), ..added };
        assert_eq!(fix_for(&svc).map(|f| f.page), Some("tweaks".into()));
    }

    #[test]
    fn power_values_read_in_minutes() {
        let id = "238c9fa8-0aad-41ed-83f4-97be242c8f20/29f6c1db-86da-48c5-9fdb-f2b67b1f44da";
        assert_eq!(power_value(id, "1800|600"), "con corriente: 30 min · con batería: 10 min");
        assert_eq!(power_value(id, "0|0"), "con corriente: nunca · con batería: nunca");
        assert_eq!(power_value("x/y", "5|"), "con corriente: 5");
    }

    #[test]
    fn a_single_object_still_reads_as_a_list() {
        let raw = lenient(r#"{"drivers":{"k":"GPU","l":"GPU","v":"1.0"},"updates":[],"services":[],"startup":[],"power":[]}"#);
        assert_eq!(raw.drivers.len(), 1);
    }

    #[test]
    fn day_validation() {
        assert!(valid_day("2026-11-16"));
        assert!(!valid_day("../x"));
        assert!(!valid_day("2026-13-01"));
    }
}
