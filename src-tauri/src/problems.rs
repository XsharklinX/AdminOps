//! Problemas abiertos de este equipo: cada análisis completo era una foto suelta.
//! Aquí cada problema tiene memoria: cuándo apareció, si se resolvió (dejó de
//! salir en un análisis que sí lo miró), si volvió, y qué arreglos tiene. Lo que
//! se probó para arreglarlo sale del Historial (el diario), por esos arreglos.

use crate::diagnostics::{Action, Diagnostics, Severity};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProblemEvent {
    pub at: u64,
    /// detected · resolved · reopened
    pub kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub key: String,
    pub title: String,
    pub area: String,
    /// bad · warn
    pub severity: String,
    pub first_seen: u64,
    pub last_seen: u64,
    pub resolved_at: Option<u64>,
    /// Arreglos del catálogo que ofrece (para buscar en el Historial lo que se probó).
    #[serde(default)]
    pub fixes: Vec<String>,
    #[serde(default)]
    pub events: Vec<ProblemEvent>,
}

/// Cuánto se guardan los ya resueltos.
const KEEP_RESOLVED_DAYS: u64 = 60;
static FILE: Mutex<()> = Mutex::new(());

/// Pone la lista al día con un análisis completo. `unchecked`: lo que ese
/// análisis no pudo mirar; lo de esas áreas no se da por resuelto por no salir.
pub fn update(list: &mut Vec<Problem>, d: &Diagnostics, at: u64) {
    let unchecked: Vec<String> = d.unchecked.iter().map(|u| u.what.to_lowercase()).collect();
    let present: Vec<&crate::diagnostics::Finding> = d.findings.iter().filter(|f| f.severity != Severity::Info && !f.key.is_empty()).collect();
    for f in &present {
        let fixes: Vec<String> = f.actions.iter().filter_map(|a| if let Action::Fix { id, .. } = a { Some(id.clone()) } else { None }).collect();
        let severity = if f.severity == Severity::Bad { "bad" } else { "warn" }.to_string();
        match list.iter_mut().find(|p| p.key == f.key) {
            Some(p) => {
                if p.resolved_at.is_some() {
                    p.resolved_at = None;
                    p.events.push(ProblemEvent { at, kind: "reopened".into() });
                }
                p.title = f.title.clone();
                p.severity = severity;
                p.last_seen = at;
                p.fixes = fixes;
            }
            None => list.push(Problem {
                key: f.key.clone(),
                title: f.title.clone(),
                area: f.area.clone(),
                severity,
                first_seen: at,
                last_seen: at,
                resolved_at: None,
                fixes,
                events: vec![ProblemEvent { at, kind: "detected".into() }],
            }),
        }
    }
    for p in list.iter_mut().filter(|p| p.resolved_at.is_none()) {
        let still = present.iter().any(|f| f.key == p.key);
        let area_unchecked = unchecked.iter().any(|u| u.contains(&p.area.to_lowercase()));
        if !still && !area_unchecked {
            p.resolved_at = Some(at);
            p.events.push(ProblemEvent { at, kind: "resolved".into() });
        }
    }
    list.retain(|p| p.resolved_at.is_none_or(|r| at.saturating_sub(r) < KEEP_RESOLVED_DAYS * 86_400));
    list.sort_by(|a, b| a.resolved_at.is_some().cmp(&b.resolved_at.is_some()).then(b.last_seen.cmp(&a.last_seen)));
}

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("problemas.json")
}

/// Lo llama el diagnóstico al guardar un análisis completo.
pub fn record(app: &tauri::AppHandle, d: &Diagnostics) {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<Problem> = crate::paths::read_json(&path(app));
    update(&mut list, d, d.timestamp);
    if let Err(e) = crate::paths::write_json(&path(app), &list) {
        log::warn!("No se pudo guardar la lista de problemas: {e}");
    }
}

#[tauri::command(async)]
pub fn problems_list(app: tauri::AppHandle) -> Vec<Problem> {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    crate::paths::read_json(&path(&app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Finding;

    fn diag(findings: Vec<Finding>, unchecked: &[&str]) -> Diagnostics {
        Diagnostics {
            findings,
            unchecked: unchecked.iter().map(|w| crate::diagnostics::Unchecked { what: (*w).into(), why: String::new(), action: None }).collect(),
            ..Default::default()
        }
    }
    fn f(key: &str, area: &str, sev: Severity) -> Finding {
        Finding { severity: sev, area: area.into(), title: key.into(), detail: None, actions: vec![], key: key.into(), proof: vec![] }
    }

    #[test]
    fn a_problem_is_born_resolved_and_comes_back() {
        let mut l = vec![];
        update(&mut l, &diag(vec![f("disco-lleno", "Discos", Severity::Warn), f("nota", "Hardware", Severity::Info)], &[]), 100);
        assert_eq!(l.len(), 1, "las sugerencias no son problemas");
        assert_eq!(l[0].first_seen, 100);
        // Sigue: se actualiza, no se duplica.
        update(&mut l, &diag(vec![f("disco-lleno", "Discos", Severity::Bad)], &[]), 200);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].last_seen, l[0].severity.as_str()), (200, "bad"));
        // Deja de salir: resuelto.
        update(&mut l, &diag(vec![], &[]), 300);
        assert_eq!(l[0].resolved_at, Some(300));
        // Vuelve.
        update(&mut l, &diag(vec![f("disco-lleno", "Discos", Severity::Warn)], &[]), 400);
        assert_eq!(l[0].resolved_at, None);
        assert_eq!(l[0].events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["detected", "resolved", "reopened"]);
    }

    #[test]
    fn what_could_not_be_checked_is_not_resolved() {
        let mut l = vec![];
        update(&mut l, &diag(vec![f("antivirus", "Seguridad", Severity::Bad)], &[]), 100);
        update(&mut l, &diag(vec![], &["Seguridad (requiere administrador)"]), 200);
        assert_eq!(l[0].resolved_at, None);
    }

    #[test]
    fn old_resolved_problems_are_forgotten() {
        let mut l = vec![];
        update(&mut l, &diag(vec![f("x", "Sistema", Severity::Warn)], &[]), 100);
        update(&mut l, &diag(vec![], &[]), 200);
        update(&mut l, &diag(vec![], &[]), 200 + 61 * 86_400);
        assert!(l.is_empty());
    }
}
