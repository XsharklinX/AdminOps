//! Prueba de ida y vuelta del catálogo: `adminops.exe --roundtrip informe.json`.
//!
//! Para cada ajuste compatible: foto del estado → aplicar → comprobar que quedó
//! aplicado → deshacer → comprobar que el registro, los servicios y la
//! detección vuelven EXACTAMENTE a como estaban.
//!
//! Modifica el sistema: está pensada para Windows Sandbox o la CI, no para el
//! PC del técnico. Código de salida: 0 = todo reversible, 1 = algún ajuste no
//! vuelve a su estado original, 2 = no se pudo ejecutar.

use super::engine::{self, Status};
use super::model::{Kind, Tweak};
use super::{apply_logged, registry, revert_logged, service, TweakState};
use crate::task::Task;
use serde::Serialize;

/// Valor de registro en bruto (tipo, bytes); `None` si no existe.
type RawReg = Option<(u32, Vec<u8>)>;

/// Estado observable de un ajuste: todo lo que toca, en bruto.
#[derive(PartialEq, Debug, Serialize)]
struct Snapshot {
    status: Status,
    registry: Vec<(String, RawReg)>,
    services: Vec<(String, Option<String>)>,
}

fn snapshot(t: &Tweak) -> Snapshot {
    Snapshot {
        status: engine::detect(t),
        registry: t
            .registry
            .iter()
            .map(|r| (format!("{}\\{}", r.path, r.name), registry::read_raw(&r.path, &r.name).map(|v| (v.vtype, v.bytes))))
            .collect(),
        services: t
            .service
            .iter()
            .map(|s| (s.name.clone(), service::startup(&s.name).map(|st| format!("{st:?}"))))
            .collect(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseResult {
    id: String,
    /// ok | not_reversible | apply_failed | not_applied | skipped
    outcome: &'static str,
    detail: Option<String>,
    before: Option<Snapshot>,
    after_revert: Option<Snapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    version: String,
    windows_build: u32,
    total: usize,
    ok: usize,
    not_reversible: usize,
    warnings: usize,
    cases: Vec<CaseResult>,
}

fn case(t: &Tweak, state: &TweakState, task: &Task) -> CaseResult {
    let result = |outcome, detail: Option<String>, before, after| CaseResult {
        id: t.id.clone(),
        outcome,
        detail,
        before,
        after_revert: after,
    };
    if !t.supported_on(state.build) {
        return result("skipped", Some("No compatible con esta versión de Windows".into()), None, None);
    }
    let before = snapshot(t);
    if before.status == Status::Unavailable {
        return result("skipped", Some("Servicios no presentes en este equipo".into()), None, None);
    }

    if let Err(e) = apply_logged(state, t, Some(task)) {
        // El motor deshace lo parcial al fallar: comprobar que así fue.
        let after = snapshot(t);
        return if after == before {
            result("apply_failed", Some(e), Some(before), None)
        } else {
            result("not_reversible", Some(format!("Falló al aplicar y no restauró: {e}")), Some(before), Some(after))
        };
    }
    let applied = engine::detect(t);
    let revert = revert_logged(state, t, Some(task));
    let after = snapshot(t);

    if after != before {
        let why = match revert {
            Err(e) => format!("Deshacer falló: {e}"),
            Ok(_) => "El estado tras deshacer no coincide con el original".into(),
        };
        return result("not_reversible", Some(why), Some(before), Some(after));
    }
    if applied != Status::Applied {
        return result("not_applied", Some(format!("Tras aplicar, la detección dio {applied:?}")), Some(before), None);
    }
    result("ok", None, None, None)
}

/// Punto de entrada del modo `--roundtrip`. Devuelve el código de salida.
pub fn run(report_path: &str, only: Option<&str>) -> i32 {
    if !crate::elevation::is_elevated() {
        eprintln!("--roundtrip requiere administrador");
        return 2;
    }
    let dir = std::env::temp_dir().join(format!("adminops-roundtrip-{}", std::process::id()));
    let state = TweakState::with_data_dir(dir.clone());
    let task = Task::detached("roundtrip");

    let cases: Vec<CaseResult> = state
        .catalog
        .iter()
        .filter(|t| t.kind == Kind::Toggle)
        .filter(|t| only.is_none_or(|o| o.split(',').any(|id| id == t.id)))
        .map(|t| {
            task.step(format!("Probando {}", t.id));
            case(t, &state, &task)
        })
        .collect();

    let count = |o: &str| cases.iter().filter(|c| c.outcome == o).count();
    let report = Report {
        version: env!("CARGO_PKG_VERSION").into(),
        windows_build: state.build,
        total: cases.len(),
        ok: count("ok"),
        not_reversible: count("not_reversible"),
        warnings: count("apply_failed") + count("not_applied"),
        cases,
    };
    let _ = std::fs::remove_dir_all(dir);
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Err(e) = std::fs::write(report_path, json) {
        eprintln!("No se pudo escribir {report_path}: {e}");
        return 2;
    }
    if report.not_reversible > 0 { 1 } else { 0 }
}
