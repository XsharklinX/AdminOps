//! El caso de ahora: el ticket que se está atendiendo.
//!
//! Mientras hay un caso abierto, todo lo que se hace en AdminOps queda apuntado
//! solo: no hace falta registrarlo aparte, porque el diario ya guarda cada
//! cambio con su título. Al cerrar el caso se redacta la resolución a partir de
//! ese diario, para pegarla en el ticket en vez de escribirla de memoria al
//! final del día.
//!
//! Solo hay un caso abierto a la vez: es «el de ahora». Se guardan con los datos
//! del técnico (en portable, en el USB), porque son su trabajo, no del equipo.

use crate::tweaks::journal::{Entry, Op};
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

static FILE_LOCK: Mutex<()> = Mutex::new(());
/// Casos que se conservan (los más recientes).
const KEEP: usize = 500;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Case {
    pub id: String,
    /// Número o referencia del ticket («#4521»), si lo hay.
    pub ticket: String,
    /// A quién se atiende (nombre para mostrar).
    pub person: String,
    /// Su usuario del dominio, si se abrió desde su ficha.
    pub sam: String,
    /// El equipo, si lo hay.
    pub machine: String,
    pub notes: String,
    /// Segundos Unix.
    pub started: u64,
    /// 0 mientras está abierto.
    pub ended: u64,
    /// La resolución tal y como quedó al cerrarlo (la que se pegó en el ticket).
    pub resolution: String,
}

/// La misma resolución contada de tres maneras.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToneSet {
    pub brief: String,
    pub friendly: String,
    pub technical: String,
}

/// Una acción apuntada durante el caso (para la barra del caso).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CaseAction {
    pub at: u64,
    pub title: String,
    pub ok: bool,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("cases.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Case> {
    crate::paths::read_json(&path(app))
}

fn save(app: &tauri::AppHandle, list: &[Case]) -> Result<(), String> {
    crate::paths::write_json(&path(app), &list)
}

/// Texto libre de un campo del caso: sin controles (salvo saltos de línea en
/// las notas) y con un tope razonable.
fn clean(v: &str, max: usize, multiline: bool) -> String {
    // Primero fuera los controles y después los espacios de los extremos: al
    // revés, un espacio pegado a un carácter invisible se quedaba dentro.
    let limpio: String = v.chars().filter(|c| !c.is_control() || (multiline && *c == '\n')).collect();
    limpio.trim().chars().take(max).collect()
}

fn validate(mut c: Case) -> Case {
    c.ticket = clean(&c.ticket, 60, false);
    c.person = clean(&c.person, 120, false);
    c.sam = clean(&c.sam, 64, false);
    c.machine = clean(&c.machine, 64, false).to_uppercase();
    c.notes = clean(&c.notes, 2000, true);
    c
}

/// Lo que hay en el diario que merece contarse en el ticket. Las consultas de
/// contraseñas (LAPS, BitLocker) no: no cambian nada y no se le cuentan al
/// usuario.
fn counts(e: &Entry) -> bool {
    !e.title.starts_with("Dominio: consultad")
}

/// Redacta la resolución del caso a partir de lo que se hizo.
///
/// No se inventa nada: cada línea es una acción real del diario, con su título,
/// en el orden en que se hizo. Lo que falló va aparte, porque también es
/// información útil para quien lea el ticket después.
pub fn compose(case: &Case, entries: &[Entry], at: u64) -> String {
    let mut out = Vec::new();
    let cabecera: Vec<&str> = [case.ticket.as_str(), case.person.as_str(), case.machine.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
    if !cabecera.is_empty() {
        out.push(format!("Caso {}", cabecera.join(" · ")));
        out.push(String::new());
    }

    // Acciones seguidas iguales (reintentos) se cuentan una vez: «(2 veces)».
    let mut hechas: Vec<(String, usize)> = Vec::new();
    let mut fallidas: Vec<String> = Vec::new();
    for e in entries.iter().filter(|e| counts(e)) {
        if e.ok {
            match hechas.last_mut() {
                Some((t, n)) if *t == e.title => *n += 1,
                _ => hechas.push((e.title.clone(), 1)),
            }
        } else {
            let porque = e.message.as_deref().map(|m| m.lines().next().unwrap_or("").chars().take(90).collect::<String>()).filter(|m| !m.is_empty());
            fallidas.push(match porque {
                Some(m) => format!("{} ({m})", e.title),
                None => e.title.clone(),
            });
        }
    }

    if hechas.is_empty() {
        out.push("No se hizo ningún cambio en el equipo desde AdminOps.".into());
    } else {
        out.push("Acciones realizadas:".into());
        for (t, n) in &hechas {
            out.push(if *n > 1 { format!("• {t} ({n} veces)") } else { format!("• {t}") });
        }
    }
    if !fallidas.is_empty() {
        out.push(String::new());
        out.push("No se pudo:".into());
        out.extend(fallidas.iter().map(|f| format!("• {f}")));
    }
    if !case.notes.is_empty() {
        out.push(String::new());
        out.push(format!("Notas: {}", case.notes));
    }
    let minutos = at.saturating_sub(case.started).div_ceil(60).max(1);
    out.push(String::new());
    out.push(format!("Tiempo dedicado: {minutos} min."));
    out.join("\n")
}

/// Los hechos del caso para contarlos en otro tono.
fn facts(case: &Case, entries: &[Entry], at: u64, findings: Vec<(String, Option<String>)>) -> crate::casetones::Facts {
    let mut done: Vec<(String, usize)> = Vec::new();
    let mut failed = Vec::new();
    for e in entries.iter().filter(|e| counts(e)) {
        if e.ok {
            match done.last_mut() {
                Some((t, n)) if *t == e.title => *n += 1,
                _ => done.push((e.title.clone(), 1)),
            }
        } else {
            let why = e.message.as_deref().map(|m| m.lines().next().unwrap_or("").chars().take(90).collect::<String>()).filter(|m| !m.is_empty());
            failed.push(match why {
                Some(m) => format!("{} ({m})", e.title),
                None => e.title.clone(),
            });
        }
    }
    crate::casetones::Facts {
        ticket: case.ticket.clone(),
        person: case.person.clone(),
        machine: case.machine.clone(),
        notes: case.notes.clone(),
        minutes: at.saturating_sub(case.started).div_ceil(60).max(1),
        done,
        failed,
        findings,
    }
}

fn open_case(list: &[Case]) -> Option<&Case> {
    list.iter().find(|c| c.ended == 0)
}

// ---------- Comandos ----------

/// El caso abierto ahora, si lo hay.
#[tauri::command]
pub fn case_current(app: tauri::AppHandle) -> Option<Case> {
    open_case(&load(&app)).cloned()
}

/// Abre un caso. Solo puede haber uno abierto: el de ahora.
#[tauri::command]
pub fn case_open(app: tauri::AppHandle, case: Case) -> Result<Case, String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    if let Some(c) = open_case(&list) {
        let de = if c.person.is_empty() { c.ticket.clone() } else { c.person.clone() };
        return Err(format!("Ya hay un caso abierto ({de}). Ciérralo antes de abrir otro."));
    }
    let mut c = validate(case);
    c.id = format!("c{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
    c.started = now();
    c.ended = 0;
    c.resolution = String::new();
    list.push(c.clone());
    let sobran = list.len().saturating_sub(KEEP);
    list.drain(..sobran);
    save(&app, &list)?;
    Ok(c)
}

/// Cambia los datos del caso abierto (ticket, persona, equipo, notas).
#[tauri::command]
pub fn case_update(app: tauri::AppHandle, case: Case) -> Result<Case, String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let cur = list.iter_mut().find(|c| c.ended == 0).ok_or("No hay ningún caso abierto.")?;
    let nuevo = validate(case);
    cur.ticket = nuevo.ticket;
    cur.person = nuevo.person;
    cur.sam = nuevo.sam;
    cur.machine = nuevo.machine;
    cur.notes = nuevo.notes;
    let out = cur.clone();
    save(&app, &list)?;
    Ok(out)
}

/// Lo que va quedando apuntado en el caso abierto.
#[tauri::command]
pub fn case_actions(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Vec<CaseAction> {
    let Some(c) = open_case(&load(&app)).cloned() else { return Vec::new() };
    tweaks.journal_since(c.started).into_iter().filter(counts).map(|e| CaseAction { at: e.timestamp, title: e.title, ok: e.ok }).collect()
}

/// Borrador de la resolución del caso abierto (sin cerrarlo).
#[tauri::command]
pub fn case_draft(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Result<String, String> {
    let c = open_case(&load(&app)).cloned().ok_or("No hay ningún caso abierto.")?;
    Ok(compose(&c, &tweaks.journal_since(c.started), now()))
}

/// La resolución del caso abierto en tres tonos: breve, para la persona y técnica.
/// Lo "encontrado" sale del último análisis del equipo (solo lo importante).
#[tauri::command]
pub fn case_tones(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Result<ToneSet, String> {
    use crate::diagnostics::Severity;
    let c = open_case(&load(&app)).cloned().ok_or("No hay ningún caso abierto.")?;
    let entries = tweaks.journal_since(c.started);
    let findings: Vec<(String, Option<String>)> = crate::diagnostics::latest_snapshot(&app)
        .map(|d| d.findings.into_iter().filter(|f| matches!(f.severity, Severity::Bad | Severity::Warn)).take(4).map(|f| (f.title, f.detail)).collect())
        .unwrap_or_default();
    let t = crate::casetones::tones(&facts(&c, &entries, now(), findings));
    Ok(ToneSet { brief: t.brief, friendly: t.friendly, technical: compose(&c, &entries, now()) })
}

/// Cierra el caso abierto guardando la resolución tal y como quedó.
#[tauri::command]
pub fn case_close(app: tauri::AppHandle, resolution: String, tweaks: State<'_, TweakState>) -> Result<Case, String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let cur = list.iter_mut().find(|c| c.ended == 0).ok_or("No hay ningún caso abierto.")?;
    cur.ended = now();
    cur.resolution = clean(&resolution, 8000, true);
    let out = cur.clone();
    save(&app, &list)?;
    let que = if out.ticket.is_empty() { out.person.clone() } else { out.ticket.clone() };
    tweaks.record(Op::Run, &format!("Caso cerrado: {que}"), &Ok::<(), String>(()));
    Ok(out)
}

/// Descarta el caso abierto sin guardarlo (abierto por error).
#[tauri::command]
pub fn case_discard(app: tauri::AppHandle) -> Result<(), String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    list.retain(|c| c.ended != 0);
    save(&app, &list)
}

/// Casos anteriores de una persona (para su ficha), los más recientes primero.
#[tauri::command]
pub fn cases_for_person(app: tauri::AppHandle, sam: String, person: String) -> Vec<Case> {
    let (sam, person) = (sam.trim().to_lowercase(), person.trim().to_lowercase());
    let mut v: Vec<Case> = load(&app)
        .into_iter()
        .filter(|c| c.ended != 0 && ((!sam.is_empty() && c.sam.to_lowercase() == sam) || (!person.is_empty() && c.person.to_lowercase() == person)))
        .collect();
    v.sort_by_key(|c| std::cmp::Reverse(c.started));
    v.truncate(20);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(t: u64, title: &str, ok: bool, message: Option<&str>) -> Entry {
        Entry { id: t, timestamp: t, op: Op::Run, tweak_id: None, title: title.into(), ok, message: message.map(String::from), backups: Vec::new(), reverted: false, undoable: false }
    }

    fn caso() -> Case {
        Case { ticket: "#4521".into(), person: "María Pérez".into(), machine: "PC-CONTA-03".into(), started: 1_000, ..Default::default() }
    }

    /// La resolución cuenta lo que se hizo de verdad, en orden, y nada más.
    #[test]
    fn resolution_lists_real_actions_in_order() {
        let e = vec![
            entry(1_010, "Vaciar la cola de impresión: HP-Recepción", true, None),
            entry(1_200, "Reparar la red", true, None),
            entry(1_250, "Dominio: desbloquear la cuenta de maria.perez", true, None),
        ];
        let t = compose(&caso(), &e, 1_000 + 14 * 60);
        assert!(t.starts_with("Caso #4521 · María Pérez · PC-CONTA-03"), "{t}");
        let i_cola = t.find("Vaciar la cola").unwrap();
        let i_red = t.find("Reparar la red").unwrap();
        assert!(i_cola < i_red, "en el orden en que se hizo");
        assert!(t.contains("Tiempo dedicado: 14 min."), "{t}");
        assert!(!t.contains("No se pudo"), "{t}");
    }

    /// Lo que falló también se cuenta, aparte y con el motivo.
    #[test]
    fn failures_are_listed_separately_with_their_reason() {
        let e = vec![entry(1_010, "Reparar la red", true, None), entry(1_020, "Restablecer Winsock", false, Some("Acceso denegado.\nDetalle largo"))];
        let t = compose(&caso(), &e, 1_100);
        assert!(t.contains("No se pudo:\n• Restablecer Winsock (Acceso denegado.)"), "{t}");
    }

    /// Un reintento no se cuenta como dos cosas distintas.
    #[test]
    fn repeated_actions_are_counted_once() {
        let e = vec![entry(1_010, "Reparar la red", true, None), entry(1_020, "Reparar la red", true, None)];
        let t = compose(&caso(), &e, 1_100);
        assert!(t.contains("• Reparar la red (2 veces)"), "{t}");
    }

    /// Consultar una contraseña LAPS no es algo que se cuente en el ticket.
    #[test]
    fn password_lookups_are_not_told() {
        let e = vec![entry(1_010, "Dominio: consultada la contraseña LAPS de PC-CONTA-03", true, None)];
        let t = compose(&caso(), &e, 1_100);
        assert!(!t.contains("LAPS"), "{t}");
        assert!(t.contains("No se hizo ningún cambio"), "{t}");
    }

    /// Sin ticket ni persona no queda una cabecera vacía, y el tiempo nunca es 0.
    #[test]
    fn minimal_case_reads_well() {
        let c = Case { started: 1_000, notes: "Volver el jueves".into(), ..Default::default() };
        let t = compose(&c, &[], 1_000);
        assert!(!t.starts_with("Caso"), "{t}");
        assert!(t.contains("Notas: Volver el jueves"), "{t}");
        assert!(t.contains("Tiempo dedicado: 1 min."), "{t}");
    }

    #[test]
    fn fields_are_cleaned() {
        let c = validate(Case { ticket: "  #4521 \u{7}".into(), machine: "pc-conta-03".into(), notes: "línea 1\nlínea 2".into(), ..Default::default() });
        assert_eq!(c.ticket, "#4521");
        assert_eq!(c.machine, "PC-CONTA-03");
        assert_eq!(c.notes, "línea 1\nlínea 2");
    }
}
