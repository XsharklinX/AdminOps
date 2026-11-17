//! «Pregúntale al equipo»: unas cuantas preguntas fijas, escritas a mano (no un
//! chat), con la respuesta en una frase y las pruebas debajo. Todo sale de lo
//! que AdminOps ya lee: arranques y apagados, pantallazos, rendimiento,
//! programas instalados, cambios de cada día y la comprobación de impresoras.
//! Si no hay pruebas, dice «No lo sé» y qué mirar. Solo lee.

use crate::bootlog::BootEvent;
use crate::perfhistory::Sample;
use crate::whatchanged::Change;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    /// Cuándo (segundos Unix), si es un hecho con fecha.
    pub t: Option<u64>,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub question: String,
    /// Una frase.
    pub answer: String,
    /// false: «No lo sé».
    pub known: bool,
    pub evidence: Vec<Evidence>,
    /// Si no se sabe, qué mirar.
    pub look_at: String,
    /// Pantalla donde seguir (id de página y de sección).
    pub page: Option<String>,
    pub focus: Option<String>,
}

fn ev(t: Option<u64>, text: impl Into<String>) -> Evidence {
    Evidence { t, text: text.into() }
}

fn unknown(question: &str, why: &str, look_at: &str, page: Option<(&str, &str)>, evidence: Vec<Evidence>) -> Answer {
    Answer {
        question: question.into(),
        answer: format!("No lo sé: {why}"),
        known: false,
        evidence,
        look_at: look_at.into(),
        page: page.map(|p| p.0.into()),
        focus: page.map(|p| p.1.into()),
    }
}

fn known(question: &str, answer: String, evidence: Vec<Evidence>, page: Option<(&str, &str)>) -> Answer {
    Answer { question: question.into(), answer, known: true, evidence, look_at: String::new(), page: page.map(|p| p.0.into()), focus: page.map(|p| p.1.into()) }
}

/// «16/11/2026 03:12», en hora local.
fn when(t: u64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(t as i64, 0).single().map(|d| d.format("%d/%m/%Y %H:%M").to_string()).unwrap_or_default()
}

// ---------- ¿Por qué se reinició? ¿Se apagó o se colgó? ----------

/// Cómo terminó la sesión anterior a un arranque.
#[derive(Debug, PartialEq)]
pub enum End {
    /// Pantallazo azul: cuándo, código y nombre.
    Bsod(u64, String, String),
    /// Se cortó sin cerrar Windows (corte de luz o cuelgue total).
    Unexpected(u64),
    /// Apagado o reinicio normal.
    Clean(u64),
    Unknown,
}

pub fn end_before(events: &[BootEvent], boot_t: u64) -> End {
    // El pantallazo y el apagón se apuntan al volver a arrancar: poco antes o poco después.
    let near = |e: &&BootEvent| e.t + 600 >= boot_t && e.t <= boot_t + 1800;
    if let Some(e) = events.iter().filter(near).find(|e| e.kind == "bsod" || (e.kind == "unexpected" && e.code.is_some())) {
        let code = e.code.clone().unwrap_or_default();
        return End::Bsod(e.t, code, e.name.clone().unwrap_or_default());
    }
    if let Some(e) = events.iter().filter(near).find(|e| e.kind == "unexpected") {
        return End::Unexpected(e.t);
    }
    match events.iter().filter(|e| e.kind == "shutdown" && e.t <= boot_t && boot_t - e.t <= 6 * 3600).max_by_key(|e| e.t) {
        Some(e) => End::Clean(e.t),
        None => End::Unknown,
    }
}

/// Quién pidió un reinicio, según el programa que lo hizo (evento 1074).
pub fn who_asked(process: &str) -> String {
    let p = process.to_lowercase();
    let leaf = p.rsplit(['\\', '/']).next().unwrap_or(&p).to_string();
    if ["trustedinstaller", "musnotification", "usoclient", "wuauclt", "tiworker", "windowsupdate", "mousocoreworker"].iter().any(|k| leaf.contains(k)) {
        "Windows Update, para terminar de instalar actualizaciones".into()
    } else if ["explorer", "shutdown", "logonui", "winlogon", "sihost"].iter().any(|k| leaf.contains(k)) {
        "alguien (o una tarea programada) desde Windows".into()
    } else if leaf.is_empty() {
        "un programa que Windows no nombra".into()
    } else {
        format!("el programa {}", process.rsplit(['\\', '/']).next().unwrap_or(process))
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ShutdownReason {
    pub t: u64,
    pub process: String,
    pub reason: String,
    pub action: String,
}

const REASONS_SCRIPT: &str = r#"
$since = (Get-Date).AddDays(-60)
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; Id = 1074; StartTime = $since } -ErrorAction SilentlyContinue | ForEach-Object {
  $p = $_.Properties
  [pscustomobject]@{ t = [int64]([DateTimeOffset]$_.TimeCreated).ToUnixTimeSeconds(); process = "$($p[0].Value)"; reason = "$($p[2].Value)"; action = "$($p[4].Value)" }
})
ConvertTo-Json -InputObject $ev -Depth 3 -Compress
"#;

fn shutdown_reasons() -> Vec<ShutdownReason> {
    let Ok(out) = crate::pspool::query(REASONS_SCRIPT, Some(Duration::from_secs(60)), "Pregúntale al equipo: motivos de reinicio") else { return vec![] };
    serde_json::from_str(out.trim()).unwrap_or_default()
}

pub fn restart_answer(question: &str, events: &[BootEvent], reasons: &[ShutdownReason], now: u64, only_kind: bool) -> Answer {
    let page = Some(("machine", "boots"));
    let Some(boot) = events.iter().filter(|e| e.kind == "boot").max_by_key(|e| e.t) else {
        return unknown(question, "Windows no apunta ningún arranque en los últimos 60 días.", "Mira «Arranques y cuelgues» y el Visor de eventos del sistema.", page, vec![]);
    };
    let mut evidence = vec![ev(Some(boot.t), "Windows arrancó")];
    let end = end_before(events, boot.t);
    // Quién lo pidió: el motivo apuntado justo antes del apagado.
    let reason = reasons.iter().filter(|r| r.t <= boot.t && boot.t - r.t <= 1800).max_by_key(|r| r.t);
    let core = match &end {
        End::Bsod(t, code, name) => {
            let what = if name.is_empty() { code.clone() } else { format!("{name} ({code})") };
            evidence.push(ev(Some(*t), format!("Pantallazo azul {what}")));
            if let Some(h) = events.iter().find(|e| e.kind == "bsod" && e.hint.is_some()).and_then(|e| e.hint.clone()) {
                evidence.push(ev(None, h));
            }
            if only_kind {
                format!("se colgó con un pantallazo azul ({what})")
            } else {
                format!("se reinició por un pantallazo azul ({what})")
            }
        }
        End::Unexpected(t) => {
            evidence.push(ev(Some(*t), "Windows avisa de que se apagó sin cerrarse bien (evento 41, Kernel-Power)"));
            if only_kind {
                "se colgó o se fue la luz: Windows no llegó a apagarse".to_string()
            } else {
                "se apagó de golpe, sin cerrar Windows: un corte de luz, el botón de encendido o un cuelgue total".to_string()
            }
        }
        End::Clean(t) => {
            evidence.push(ev(Some(*t), "Windows se apagó con normalidad"));
            match reason {
                Some(r) => {
                    let mut text = format!("Lo pidió {}", who_asked(&r.process));
                    if !r.reason.trim().is_empty() {
                        text.push_str(&format!(" · motivo: {}", r.reason.trim()));
                    }
                    evidence.push(ev(Some(r.t), text));
                    if only_kind {
                        "se apagó con normalidad (no se colgó)".to_string()
                    } else {
                        format!("lo reinició {}", who_asked(&r.process))
                    }
                }
                None => {
                    if only_kind {
                        "se apagó con normalidad (no se colgó)".to_string()
                    } else {
                        "fue un reinicio o apagado normal; Windows no apunta quién lo pidió".to_string()
                    }
                }
            }
        }
        End::Unknown => {
            return unknown(
                question,
                "hay un arranque, pero no veo cómo terminó la sesión anterior.",
                "Mira «Arranques y cuelgues»: puede que el registro se haya recortado.",
                page,
                evidence,
            );
        }
    };
    let ago = now.saturating_sub(boot.t);
    let lead = if only_kind {
        format!("La última vez ({}) ", when(boot.t))
    } else if ago > 36 * 3600 {
        format!("Anoche no se reinició; el último reinicio fue el {}: ", when(boot.t))
    } else {
        format!("El último reinicio fue el {}: ", when(boot.t))
    };
    let mut text = format!("{lead}{core}.");
    if !only_kind && ago <= 36 * 3600 {
        text = text.replacen("El último reinicio", "Se reinició", 1);
    }
    known(question, text, evidence, page)
}

// ---------- ¿Por qué va lento? ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Cause {
    /// Menor, más probable.
    pub rank: u8,
    pub text: String,
    pub evidence: Vec<Evidence>,
}

fn avg(v: &[&Sample], f: impl Fn(&Sample) -> f32) -> f32 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().map(|s| f(s)).sum::<f32>() / v.len() as f32
    }
}

/// Lo que explica que vaya lento desde `since`, de más a menos probable.
pub fn slow_causes(samples: &[Sample], since: u64, changes: &[Change], installed_since: &[String], events: &[(String, usize)], offset_secs: i64) -> Vec<Cause> {
    let after: Vec<&Sample> = samples.iter().filter(|s| s.t >= since).collect();
    let before: Vec<&Sample> = samples.iter().filter(|s| s.t < since).collect();
    let mut out = Vec::new();
    let recent: Vec<Sample> = after.iter().map(|s| (*s).clone()).collect();
    let ins = crate::perfinsight::insights(&recent, offset_secs);
    if let Some(l) = ins.leaks.iter().find(|l| l.to_mb >= l.from_mb + 200) {
        out.push(Cause {
            rank: 1,
            text: format!("{} va ocupando cada vez más memoria ({} → {} MB en {:.0} h)", l.name, l.from_mb, l.to_mb, l.hours),
            evidence: vec![ev(Some(l.at), format!("{}: de {} MB a {} MB sin soltarla", l.name, l.from_mb, l.to_mb))],
        });
    }
    if ins.saturated_minutes >= 30 {
        let who = ins.disk_hogs.first().map(|h| format!("; lo que más lo usaba: {}", h.name)).unwrap_or_default();
        let mut evidence = vec![ev(None, format!("El disco estuvo saturado {} minutos{who}", ins.saturated_minutes))];
        for h in ins.disk_hogs.iter().take(3) {
            evidence.push(ev(None, format!("{} leía o escribía unos {} KB/s mientras el disco estaba al límite ({} min)", h.name, h.avg_kbs, h.minutes)));
        }
        out.push(Cause { rank: 2, text: format!("el disco ha estado saturado {} minutos{who}", ins.saturated_minutes), evidence });
    }
    if after.len() >= 30 {
        let (ram_a, ram_b) = (avg(&after, |s| s.ram), avg(&before, |s| s.ram));
        if ram_a >= 85.0 || (!before.is_empty() && ram_a >= ram_b + 15.0) {
            let cmp = if before.is_empty() { String::new() } else { format!(" (antes {ram_b:.0} %)") };
            out.push(Cause {
                rank: 3,
                text: format!("la memoria va al {ram_a:.0} % de media{cmp}"),
                evidence: vec![ev(None, format!("Memoria en uso de media desde entonces: {ram_a:.0} %{cmp}"))],
            });
        }
        let (cpu_a, cpu_b) = (avg(&after, |s| s.cpu), avg(&before, |s| s.cpu));
        if cpu_a >= 60.0 || (!before.is_empty() && cpu_a >= cpu_b + 20.0) {
            let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
            for s in &after {
                if let Some(t) = &s.top {
                    *counts.entry(t.as_str()).or_default() += 1;
                }
            }
            let top = counts.iter().max_by_key(|(_, n)| **n).map(|(n, _)| (*n).to_string());
            let who = top.as_ref().map(|n| format!("; el que más procesador usaba: {n}")).unwrap_or_default();
            out.push(Cause { rank: 3, text: format!("el procesador va al {cpu_a:.0} % de media{who}"), evidence: vec![ev(None, format!("Procesador de media desde entonces: {cpu_a:.0} %{who}"))] });
        }
    }
    let mut changed: Vec<String> = changes
        .iter()
        .filter(|c| matches!(c.kind.as_str(), "program" | "driver" | "update" | "startup" | "service") && c.new.is_some())
        .map(|c| c.label.clone())
        .collect();
    for n in installed_since {
        if !changed.contains(n) {
            changed.push(n.clone());
        }
    }
    if !changed.is_empty() {
        let list = changed.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
        let more = if changed.len() > 4 { format!(" y {} más", changed.len() - 4) } else { String::new() };
        out.push(Cause {
            rank: 4,
            text: format!("desde entonces se instaló o cambió: {list}{more}"),
            evidence: changed.iter().take(8).map(|n| ev(None, format!("Instalado o cambiado: {n}"))).collect(),
        });
    }
    if let Some((title, n)) = events.first() {
        out.push(Cause {
            rank: 5,
            text: format!("Windows registró «{title}» {n} {}", if *n == 1 { "vez" } else { "veces" }),
            evidence: events.iter().take(3).map(|(t, n)| ev(None, format!("{t}: {n} {}", if *n == 1 { "vez" } else { "veces" }))).collect(),
        });
    }
    out.sort_by_key(|c| c.rank);
    out
}

pub fn slow_answer(question: &str, samples: &[Sample], since: u64, causes: &[Cause]) -> Answer {
    let page = Some(("machine", "performance"));
    let covered = samples.iter().filter(|s| s.t >= since).count();
    if causes.is_empty() {
        let why = if covered < 30 { "AdminOps lleva poco midiendo desde esa fecha (se mide mientras está abierta)." } else { "no veo nada raro en el rendimiento ni en lo instalado desde esa fecha." };
        let ev = vec![self::ev(None, format!("{covered} minutos medidos desde esa fecha"))];
        return unknown(question, why, "Mira el Rendimiento de los últimos 7 días y el Historial del equipo.", page, ev);
    }
    let mut text = format!("Lo más probable: {}.", causes[0].text);
    if causes.len() > 1 {
        text.push_str(&format!(" Además, {}.", causes[1].text));
    }
    let evidence = causes.iter().flat_map(|c| c.evidence.clone()).collect();
    known(question, text, evidence, page)
}

// ---------- ¿Quién instaló esto? ----------

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct MsiEvent {
    pub t: u64,
    pub user: String,
}

const MSI_SCRIPT: &str = r#"
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'Application'; ProviderName = 'MsiInstaller'; Id = 1033, 11707 } -ErrorAction SilentlyContinue | Where-Object { $_.Message -and $_.Message.IndexOf($name, [StringComparison]::OrdinalIgnoreCase) -ge 0 } | Select-Object -First 5 | ForEach-Object {
  $u = ''
  try { $u = $_.UserId.Translate([Security.Principal.NTAccount]).Value } catch { }
  [pscustomobject]@{ t = [int64]([DateTimeOffset]$_.TimeCreated).ToUnixTimeSeconds(); user = "$u" }
})
ConvertTo-Json -InputObject $ev -Depth 3 -Compress
"#;

/// Datos de un programa instalado que hacen falta para contestar.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgramInfo {
    pub name: String,
    pub publisher: String,
    pub version: String,
    /// AAAA-MM-DD
    pub installed: String,
    pub per_user: bool,
}

fn programs_info() -> Vec<ProgramInfo> {
    crate::programs::list()
        .iter()
        .filter_map(|p| serde_json::to_value(p).ok())
        .map(|v| ProgramInfo {
            name: v["name"].as_str().unwrap_or_default().into(),
            publisher: v["publisher"].as_str().unwrap_or_default().into(),
            version: v["version"].as_str().unwrap_or_default().into(),
            installed: v["installed"].as_str().unwrap_or_default().into(),
            per_user: v["perUser"].as_bool().unwrap_or(false),
        })
        .filter(|p| !p.name.is_empty())
        .collect()
}

/// Elige el programa que se quiso decir: el nombre exacto, o el que lo contiene.
pub fn pick_program<'a>(list: &'a [ProgramInfo], q: &str) -> Option<&'a ProgramInfo> {
    let q = q.trim().to_lowercase();
    if q.len() < 2 {
        return None;
    }
    list.iter().find(|p| p.name.to_lowercase() == q).or_else(|| list.iter().find(|p| p.name.to_lowercase().contains(&q)))
}

fn account_text(user: &str) -> String {
    let u = user.trim();
    if u.eq_ignore_ascii_case("NT AUTHORITY\\SYSTEM") || u.eq_ignore_ascii_case("NT AUTORIDAD\\SYSTEM") || u.to_uppercase().ends_with("\\SYSTEM") {
        "el propio sistema (una instalación automática o de la empresa, sin nadie delante)".into()
    } else {
        u.to_string()
    }
}

pub fn installed_answer(question: &str, list: &[ProgramInfo], q: &str, msi: &[MsiEvent]) -> Answer {
    let page = Some(("apps", "uninstall"));
    let Some(p) = pick_program(list, q) else {
        return unknown(question, &format!("no encuentro ningún programa instalado que se llame «{}».", q.trim()), "Escribe parte del nombre tal como sale en Programas → Desinstalar.", page, vec![]);
    };
    let mut evidence = vec![ev(None, format!("Programa: {}{}", p.name, if p.version.is_empty() { String::new() } else { format!(" {}", p.version) }))];
    if !p.publisher.is_empty() {
        evidence.push(ev(None, format!("Editor: {}", p.publisher)));
    }
    if !p.installed.is_empty() {
        evidence.push(ev(None, format!("Fecha de instalación: {}", p.installed)));
    }
    let others = list.iter().filter(|x| x.name != p.name && x.name.to_lowercase().contains(&q.trim().to_lowercase())).take(3).map(|x| x.name.clone()).collect::<Vec<_>>();
    if !others.is_empty() {
        evidence.push(ev(None, format!("Otros con nombre parecido: {}", others.join(", "))));
    }
    if let Some(m) = msi.iter().filter(|m| !m.user.trim().is_empty()).max_by_key(|m| m.t) {
        evidence.push(ev(Some(m.t), format!("Windows Installer apuntó la instalación a nombre de {}", account_text(&m.user))));
        let date = if p.installed.is_empty() { when(m.t) } else { format!("el {}", p.installed) };
        return known(question, format!("{} lo instaló {}, {date}.", p.name, account_text(&m.user)), evidence, page);
    }
    if p.per_user {
        evidence.push(ev(None, "Está registrado solo para un usuario (no para todo el equipo)."));
        let date = if p.installed.is_empty() { String::new() } else { format!(" el {}", p.installed) };
        return known(question, format!("{} lo instaló el usuario que lo usa (está instalado solo para su perfil){date}; Windows no guarda cuál.", p.name), evidence, page);
    }
    let when_txt = if p.installed.is_empty() { "no consta cuándo".to_string() } else { format!("se instaló el {}", p.installed) };
    unknown(
        question,
        &format!("Windows no apunta quién instaló {} ({when_txt}).", p.name),
        "Mira el Historial del equipo de esa fecha, o pregunta a quien tuviera sesión abierta ese día.",
        page,
        evidence,
    )
}

// ---------- ¿Por qué no imprime? ----------

pub fn printer_answer(question: &str, name: &str, check: Result<crate::printers::PrinterCheck, String>) -> Answer {
    let page = Some(("printers", "printers"));
    match check {
        Err(e) => unknown(question, &format!("no pude revisar «{name}»: {e}"), "Comprueba que la impresora sigue instalada en este equipo.", page, vec![]),
        Ok(c) => {
            let mut evidence = vec![ev(None, c.text.clone()), ev(None, if c.spooler { "La cola de impresión de Windows está en marcha" } else { "La cola de impresión de Windows está parada" })];
            if !c.host.is_empty() {
                let r = match c.reachable {
                    Some(true) => "responde",
                    Some(false) => "no responde",
                    None => "no se pudo probar",
                };
                evidence.push(ev(None, format!("La impresora en {} {r}", crate::escalate::scrub(&c.host))));
            }
            if c.jobs > 0 {
                evidence.push(ev(None, format!("{} trabajo(s) en la cola; el más viejo lleva {} min", c.jobs, c.oldest_job_min)));
            }
            if c.level == "ok" {
                known(question, format!("No veo nada que lo impida: {}.", c.title.to_lowercase()), evidence, page)
            } else {
                known(question, format!("{}.", c.title), evidence, page)
            }
        }
    }
}

// ---------- La pregunta ----------

const Q_RESTART: &str = "¿Por qué se reinició anoche?";
const Q_CRASH: &str = "¿Se apagó o se colgó?";
const Q_INSTALLED: &str = "¿Quién instaló esto?";
const Q_PRINTER: &str = "¿Por qué no imprime?";

fn start_of_day(day: &str) -> Result<u64, String> {
    use chrono::TimeZone;
    let d = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| "Fecha no válida.".to_string())?;
    let dt = d.and_hms_opt(0, 0, 0).ok_or("Fecha no válida.")?;
    chrono::Local.from_local_datetime(&dt).earliest().map(|t| t.timestamp().max(0) as u64).ok_or_else(|| "Fecha no válida.".to_string())
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Contesta una de las preguntas fijas: `restart`, `crash`, `slow` (con `since`),
/// `installed` (con el nombre en `arg`) o `printer` (con el nombre de la impresora).
#[tauri::command(async)]
pub fn ask_pc(app: tauri::AppHandle, question: String, arg: Option<String>, since: Option<String>) -> Result<Answer, String> {
    let arg = arg.unwrap_or_default();
    match question.as_str() {
        "restart" | "crash" => {
            let log = crate::bootlog::boot_history()?;
            let reasons = if question == "restart" { shutdown_reasons() } else { vec![] };
            Ok(restart_answer(if question == "restart" { Q_RESTART } else { Q_CRASH }, &log.events, &reasons, now(), question == "crash"))
        }
        "slow" => {
            let day = since.ok_or("Elige desde qué día.")?;
            let t = start_of_day(&day)?;
            let samples = crate::perfhistory::samples();
            let changes = crate::whatchanged::changes_since(&app, &day);
            let installed: Vec<String> = programs_info().into_iter().filter(|p| p.installed.as_str() >= day.as_str() && !p.installed.is_empty()).map(|p| p.name).collect();
            let days = ((now().saturating_sub(t)) / 86_400 + 1).clamp(1, 30) as u32;
            let events: Vec<(String, usize)> = crate::eventdigest::event_digest(days)
                .unwrap_or_default()
                .into_iter()
                .filter(|g| g.weight == "matter")
                .take(3)
                .map(|g| (g.title, g.count))
                .collect();
            let offset = chrono::Local::now().offset().local_minus_utc() as i64;
            let causes = slow_causes(&samples, t, &changes, &installed, &events, offset);
            let q = format!("¿Por qué va lento desde el {}?", weekday(&day));
            Ok(slow_answer(&q, &samples, t, &causes))
        }
        "installed" => {
            if arg.trim().len() < 2 {
                return Err("Escribe el nombre del programa.".into());
            }
            let list = programs_info();
            let msi = match pick_program(&list, &arg) {
                Some(p) => {
                    let script = format!("{}{MSI_SCRIPT}", crate::ps::text_var("name", &p.name));
                    crate::pspool::query(&script, Some(Duration::from_secs(60)), "Pregúntale al equipo: quién instaló")
                        .ok()
                        .and_then(|o| serde_json::from_str::<Vec<MsiEvent>>(o.trim()).ok())
                        .unwrap_or_default()
                }
                None => vec![],
            };
            Ok(installed_answer(Q_INSTALLED, &list, &arg, &msi))
        }
        "printer" => {
            if arg.trim().is_empty() {
                return Err("Elige una impresora.".into());
            }
            Ok(printer_answer(Q_PRINTER, &arg, crate::printers::check_printer(arg.clone())))
        }
        _ => Err("Pregunta desconocida.".into()),
    }
}

/// «2026-11-16» → «lunes».
pub fn weekday(day: &str) -> String {
    use chrono::Datelike;
    let Ok(d) = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d") else { return day.to_string() };
    ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"][d.weekday().num_days_from_monday() as usize].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn be(t: u64, kind: &str, code: Option<&str>) -> BootEvent {
        BootEvent { t, kind: kind.into(), code: code.map(String::from), name: None, hint: None }
    }

    #[test]
    fn scripts_parse() {
        assert!(crate::ps::parse_errors(REASONS_SCRIPT).is_empty());
        let e = crate::ps::parse_errors(&format!("$name = 'x'\n{MSI_SCRIPT}"));
        assert!(e.is_empty(), "{e}");
    }

    #[test]
    fn a_clean_restart_for_updates_is_explained() {
        let now = 100_000;
        let events = vec![be(now - 3000, "boot", None), be(now - 3200, "shutdown", None)];
        let reasons = vec![ShutdownReason { t: now - 3250, process: r"C:\Windows\System32\MusNotification.exe".into(), reason: "Operating System: Recovery (Planned)".into(), action: "restart".into() }];
        let a = restart_answer(Q_RESTART, &events, &reasons, now, false);
        assert!(a.known);
        assert!(a.answer.contains("Windows Update"), "{}", a.answer);
        assert!(a.evidence.len() >= 3);
    }

    #[test]
    fn a_blue_screen_and_a_power_cut_are_told_apart() {
        let now = 100_000;
        let bsod = vec![be(now - 3000, "boot", None), be(now - 2990, "bsod", Some("0x0000009F"))];
        let a = restart_answer(Q_RESTART, &bsod, &[], now, false);
        assert!(a.answer.contains("pantallazo azul"));
        let cut = vec![be(now - 3000, "boot", None), be(now - 2980, "unexpected", None)];
        let a = restart_answer(Q_CRASH, &cut, &[], now, true);
        assert!(a.answer.contains("se colgó o se fue la luz"));
        let clean = vec![be(now - 3000, "boot", None), be(now - 3100, "shutdown", None)];
        let a = restart_answer(Q_CRASH, &clean, &[], now, true);
        assert!(a.answer.contains("normalidad"));
    }

    #[test]
    fn without_evidence_it_says_it_does_not_know() {
        let a = restart_answer(Q_RESTART, &[], &[], 100, false);
        assert!(!a.known && a.answer.starts_with("No lo sé"));
        assert!(!a.look_at.is_empty());
        let b = restart_answer(Q_RESTART, &[be(50, "boot", None)], &[], 100, false);
        assert!(!b.known);
    }

    #[test]
    fn old_restarts_are_not_last_night() {
        let now = 1_000_000;
        let events = vec![be(now - 5 * 86_400, "boot", None), be(now - 5 * 86_400 - 60, "shutdown", None)];
        assert!(restart_answer(Q_RESTART, &events, &[], now, false).answer.starts_with("Anoche no se reinició"));
    }

    #[test]
    fn who_asked_names_the_cause() {
        assert!(who_asked(r"C:\Windows\servicing\TrustedInstaller.exe").contains("Windows Update"));
        assert!(who_asked(r"C:\Windows\explorer.exe").contains("alguien"));
        assert_eq!(who_asked(r"C:\Program Files\X\setup.exe"), "el programa setup.exe");
    }

    fn sample(t: u64, ram: f32, mem: Vec<(&str, u32)>) -> Sample {
        Sample { t, cpu: 10.0, ram, disk: 40.0, mem: mem.into_iter().map(|(n, m)| (n.to_string(), m)).collect(), ..Default::default() }
    }

    #[test]
    fn slow_picks_the_leak_first_and_says_what_changed() {
        let samples: Vec<Sample> = (0..180).map(|i| sample(1_000 + i * 60, 60.0, vec![("chrome.exe", 500 + i as u32 * 10)])).collect();
        let changes = vec![Change { kind: "program".into(), key: "X".into(), label: "Foo".into(), old: None, new: Some("1".into()), day: "d".into() }];
        let c = slow_causes(&samples, 1_000, &changes, &[], &[], 0);
        assert!(c[0].text.contains("chrome.exe"), "{c:?}");
        assert!(c.iter().any(|x| x.text.contains("Foo")));
        let a = slow_answer("¿Por qué va lento?", &samples, 1_000, &c);
        assert!(a.known && a.answer.starts_with("Lo más probable"));
    }

    #[test]
    fn slow_with_nothing_found_says_so() {
        let samples: Vec<Sample> = (0..100).map(|i| sample(1_000 + i * 60, 50.0, vec![])).collect();
        let c = slow_causes(&samples, 1_000, &[], &[], &[], 0);
        assert!(c.is_empty());
        let a = slow_answer("q", &samples, 1_000, &c);
        assert!(!a.known && a.answer.contains("no veo nada raro"));
        let few = slow_answer("q", &[], 1_000, &[]);
        assert!(few.answer.contains("poco midiendo"));
    }

    fn prog(name: &str, per_user: bool, installed: &str) -> ProgramInfo {
        ProgramInfo { name: name.into(), publisher: "Acme".into(), version: "1.0".into(), installed: installed.into(), per_user }
    }

    #[test]
    fn who_installed_uses_the_installer_log_then_the_registry() {
        let list = vec![prog("7-Zip", false, "2026-11-10"), prog("Zoom", true, "2026-11-12")];
        let a = installed_answer(Q_INSTALLED, &list, "7zip", &[]);
        assert!(!a.known, "no se encuentra «7zip» sin guion");
        let a = installed_answer(Q_INSTALLED, &list, "7-zip", &[MsiEvent { t: 1, user: r"EMPRESA\ana".into() }]);
        assert!(a.known && a.answer.contains(r"EMPRESA\ana"));
        let a = installed_answer(Q_INSTALLED, &list, "zoom", &[]);
        assert!(a.known && a.answer.contains("solo para su perfil"));
        let a = installed_answer(Q_INSTALLED, &list, "7-zip", &[]);
        assert!(!a.known && a.answer.starts_with("No lo sé"));
        let a = installed_answer(Q_INSTALLED, &list, "7-zip", &[MsiEvent { t: 1, user: r"NT AUTHORITY\SYSTEM".into() }]);
        assert!(a.answer.contains("propio sistema"));
    }

    #[test]
    fn printer_failure_is_not_guessed() {
        let a = printer_answer(Q_PRINTER, "HP", Err("La impresora ya no existe.".into()));
        assert!(!a.known && a.answer.contains("No lo sé"));
    }

    #[test]
    fn weekday_names() {
        assert_eq!(weekday("2026-11-16"), "lunes");
        assert_eq!(weekday("2026-11-22"), "domingo");
    }
}
