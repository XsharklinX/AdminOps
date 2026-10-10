//! «Ver lo que va a cambiar antes de aplicarlo»: para cada ajuste, el valor
//! actual de cada cosa que tocará, el que tendrá después y si se puede deshacer.
//! Solo lee: no cambia nada.

use super::model::{Kind, RegData, RegKind, Startup, Tweak};
use super::{registry, service, TweakState};
use serde::Serialize;
use tauri::State;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLine {
    pub what: String,
    pub before: String,
    pub after: String,
    pub undoable: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TweakPreview {
    pub id: String,
    pub name: String,
    pub reboot: bool,
    pub lines: Vec<PreviewLine>,
}

pub fn startup_label(s: Startup) -> &'static str {
    match s {
        Startup::Automatic => "Automático",
        Startup::Delayed => "Automático (retrasado)",
        Startup::Manual => "Manual",
        Startup::Disabled => "Deshabilitado",
    }
}

pub fn data_label(d: &RegData) -> String {
    match d {
        RegData::Int(n) => n.to_string(),
        RegData::Str(s) if s.is_empty() => "(vacío)".into(),
        RegData::Str(s) => s.clone(),
    }
}

fn current_value(path: &str, name: &str, kind: RegKind) -> String {
    let v = match kind {
        RegKind::Dword => registry::read_u32(path, name).map(|n| n.to_string()),
        RegKind::Qword => registry::read_raw(path, name).filter(|r| r.bytes.len() == 8).map(|r| u64::from_le_bytes(r.bytes[..8].try_into().unwrap_or([0; 8])).to_string()),
        RegKind::String | RegKind::Expand => registry::read_string(path, name),
    };
    v.unwrap_or_else(|| "(no existe)".into())
}

/// Lo que cambiará un ajuste, leído del equipo ahora mismo.
pub fn preview_of(t: &Tweak) -> Vec<PreviewLine> {
    let mut out = Vec::new();
    if t.kind == Kind::Action {
        out.push(PreviewLine { what: "Tarea puntual".into(), before: "—".into(), after: t.description.clone(), undoable: false });
        return out;
    }
    for r in &t.registry {
        let short = r.path.rsplit('\\').next().unwrap_or(&r.path);
        out.push(PreviewLine { what: format!("Registro: {short}\\{}", r.name), before: current_value(&r.path, &r.name, r.kind), after: data_label(&r.value), undoable: true });
    }
    for s in &t.service {
        let before = service::startup(&s.name).map(startup_label).unwrap_or("(no instalado)");
        let after = format!("{}{}", startup_label(s.startup), if s.stop { " y se detiene" } else { "" });
        out.push(PreviewLine { what: format!("Servicio {}", s.name), before: before.into(), after, undoable: true });
    }
    if let Some(sc) = &t.script {
        if sc.apply.is_some() {
            out.push(PreviewLine { what: "Cambio por script".into(), before: "Lo que haya ahora".into(), after: t.description.clone(), undoable: sc.revert.is_some() });
        }
    }
    out
}

/// Vista previa de uno o varios ajustes (un perfil entero).
#[tauri::command(async)]
pub fn tweak_preview(ids: Vec<String>, state: State<'_, TweakState>) -> Vec<TweakPreview> {
    ids.iter()
        .filter_map(|id| state.find(id).ok())
        .map(|t| TweakPreview { id: t.id.clone(), name: t.name.clone(), reboot: t.reboot, lines: preview_of(t) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etiquetas_legibles() {
        assert_eq!(startup_label(Startup::Disabled), "Deshabilitado");
        assert_eq!(data_label(&RegData::Int(0)), "0");
        assert_eq!(data_label(&RegData::Str(String::new())), "(vacío)");
    }

    #[test]
    fn cada_ajuste_del_catalogo_tiene_vista_previa() {
        for t in super::super::catalog::load() {
            let lines = preview_of(&t);
            assert!(!lines.is_empty() || (t.registry.is_empty() && t.service.is_empty()), "«{}» no explica qué cambia", t.id);
        }
    }
}
