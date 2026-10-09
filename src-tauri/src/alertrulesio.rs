//! Reglas de alerta: leer las cifras, vigilarlas en segundo plano y los comandos de la
//! pantalla. La decisión de qué salta está en `alertrules.rs`.

use crate::alertrules::{self, Readings, Rule, State};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Lecturas lentas (impresoras): se guardan un rato para no preguntar a la red cada minuto.
type SlowCache = HashMap<String, (Instant, Option<f64>)>;
static SLOW: Mutex<Option<SlowCache>> = Mutex::new(None);
const SLOW_TTL: Duration = Duration::from_secs(30 * 60);

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("reglas-alerta.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Rule> {
    crate::paths::read_json(&path(app))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[derive(Serialize)]
pub struct Template {
    pub name: String,
    pub rules: Vec<Rule>,
}

#[tauri::command]
pub fn alert_rules_get(app: tauri::AppHandle) -> Vec<Rule> {
    load(&app)
}

#[tauri::command]
pub fn alert_rules_templates() -> Vec<Template> {
    alertrules::templates().into_iter().map(|(name, rules)| Template { name: name.into(), rules }).collect()
}

#[tauri::command]
pub fn alert_rules_save(app: tauri::AppHandle, rules: Vec<Rule>) -> Result<Vec<Rule>, String> {
    let mut seen = std::collections::HashSet::new();
    let out: Vec<Rule> = rules
        .into_iter()
        .take(60)
        .enumerate()
        .map(|(i, mut r)| {
            if r.id.trim().is_empty() || !seen.insert(r.id.clone()) {
                r.id = format!("r{:x}{i}", now());
                seen.insert(r.id.clone());
            }
            r.clean()
        })
        .collect();
    crate::paths::write_json(&path(&app), &out)?;
    // Las reglas cambiadas empiezan de cero.
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(out)
}

fn disk_free_pct(target: &str) -> Option<f64> {
    let letter = if target.trim().is_empty() { std::env::var("SystemDrive").ok()?.chars().next()? } else { target.trim().chars().next()? }.to_ascii_uppercase();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let d = disks.iter().find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with(&format!("{letter}:")))?;
    (d.total_space() > 0).then(|| d.available_space() as f64 * 100.0 / d.total_space() as f64)
}

fn cpu_pct() -> f64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();
    f64::from(sys.global_cpu_usage())
}

fn ram_pct() -> f64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    if sys.total_memory() == 0 {
        0.0
    } else {
        sys.used_memory() as f64 * 100.0 / sys.total_memory() as f64
    }
}

/// 1 si el servicio está en marcha, 0 si no. Una sola consulta para todos.
fn services(names: &[String]) -> HashMap<String, f64> {
    let list = names.iter().map(|n| crate::ps::ps_literal(n)).collect::<Vec<_>>().join(",");
    let script = format!("foreach ($n in @({list})) {{ $s = Get-Service -Name $n -ErrorAction SilentlyContinue; \"$n|$(if ($s -and $s.Status -eq 'Running') {{ 1 }} elseif ($s) {{ 0 }} else {{ -1 }})\" }}");
    let mut out = HashMap::new();
    if let Ok(text) = crate::pspool::query(&script, Some(Duration::from_secs(20)), "Reglas de alerta") {
        for line in text.lines() {
            if let Some((n, v)) = line.trim().split_once('|') {
                // -1: el servicio no existe en este equipo; no se avisa de algo que no está.
                if let Ok(v) = v.trim().parse::<f64>() {
                    if v >= 0.0 {
                        out.insert(n.to_lowercase(), v);
                    }
                }
            }
        }
    }
    out
}

fn backup_age(app: &tauri::AppHandle, target: &str) -> Option<f64> {
    let t = target.trim().to_lowercase();
    let ages: Vec<f64> = crate::backupio::last_statuses(app)
        .into_iter()
        .filter(|s| t.is_empty() || s.id.to_lowercase() == t || s.name.to_lowercase() == t)
        .filter_map(|s| s.newest)
        .map(|n| now().saturating_sub(n) as f64 / 86_400.0)
        .collect();
    // La copia más atrasada: una regla de «copia atrasada» debe saltar si cualquiera lo está.
    ages.into_iter().reduce(f64::max).map(|oldest| oldest.max(0.0))
}

fn toner(target: &str) -> Option<f64> {
    if target.trim().is_empty() {
        return None;
    }
    let key = target.trim().to_lowercase();
    if let Some((at, v)) = SLOW.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&key)).copied() {
        if at.elapsed() < SLOW_TTL {
            return v;
        }
    }
    let v = crate::printers::check_printer(target.trim().to_string())
        .ok()
        .and_then(|c| c.device)
        .and_then(|d| d.supplies.iter().filter(|s| s.consumable).filter_map(|s| s.percent).min())
        .map(f64::from);
    SLOW.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(key, (Instant::now(), v));
    v
}

fn disk_score(app: &tauri::AppHandle) -> Option<f64> {
    crate::disks::last_saved(app).disks.iter().filter(|d| !d.bus.eq_ignore_ascii_case("USB")).map(|d| f64::from(d.score)).reduce(f64::min)
}

/// Lee la cifra que necesita una regla.
fn read_one(app: &tauri::AppHandle, r: &Rule, svc: &HashMap<String, f64>) -> Option<f64> {
    match r.metric.as_str() {
        "disk_free" => disk_free_pct(&r.target),
        "cpu" => Some(cpu_pct()),
        "ram" => Some(ram_pct()),
        "service_running" => svc.get(&r.target.trim().to_lowercase()).copied(),
        "backup_age" => backup_age(app, &r.target),
        "toner" => toner(&r.target),
        "disk_score" => disk_score(app),
        _ => None,
    }
}

fn collect(app: &tauri::AppHandle, rules: &[Rule]) -> Readings {
    let names: Vec<String> = rules.iter().filter(|r| r.enabled && r.metric == "service_running" && !r.target.trim().is_empty()).map(|r| r.target.trim().to_string()).collect();
    let svc = if names.is_empty() { HashMap::new() } else { services(&names) };
    let mut out = Readings::new();
    for r in rules.iter().filter(|r| r.enabled) {
        let key = r.reading_key();
        if out.contains_key(&key) {
            continue;
        }
        if let Some(v) = read_one(app, r, &svc) {
            out.insert(key, v);
        }
    }
    out
}

/// «Ahora» de una regla, para enseñarlo mientras se escribe.
#[tauri::command(async)]
pub fn alert_rules_reading(app: tauri::AppHandle, rule: Rule) -> Option<f64> {
    let r = rule.clean();
    let svc = if r.metric == "service_running" { services(std::slice::from_ref(&r.target)) } else { HashMap::new() };
    read_one(&app, &r, &svc)
}

/// Cada minuto, con AdminOps abierto, mira las reglas activas y avisa de las que saltan.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(90));
        loop {
            let rules = load(&app);
            if rules.iter().any(|r| r.enabled) {
                let readings = collect(&app, &rules);
                let prev = STATE.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
                let (fired, next) = alertrules::evaluate(&rules, &readings, &prev, now());
                *STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(next);
                if !fired.is_empty() {
                    use tauri::Manager;
                    let alerts = fired
                        .into_iter()
                        .map(|f| {
                            let ok: Result<(), String> = Ok(());
                            app.state::<crate::tweaks::TweakState>().record(crate::tweaks::journal::Op::Run, &format!("Regla de alerta: {} ({})", f.title, f.detail), &ok);
                            crate::winwatch::Alert {
                                key: format!("rule:{}", f.rule_id),
                                level: f.level,
                                title: f.title,
                                detail: f.detail,
                                explanation: "Se cumple una regla de alerta que creaste en Ajustes → Alertas.".into(),
                                advice: "Revisa lo que dice la regla; puedes cambiarla o quitarla en Ajustes → Alertas.".into(),
                                page: Some("settings".into()),
                                count: 1,
                                time: now(),
                                ..Default::default()
                            }
                        })
                        .collect();
                    crate::winwatch::push_alerts(&app, alerts);
                }
            }
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}
