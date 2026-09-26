//! Detectar, aplicar y revertir ajustes. Sin estado: el diario y la política
//! de puntos de restauración viven en `mod.rs`.

use super::journal::Backup;
use super::model::{Kind, RegData, Startup, Tweak};
use super::{registry, service};
use crate::ps;
use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Applied,
    NotApplied,
    /// Solo parte de las acciones coinciden (alguien cambió algo a mano, o una actualización lo revirtió).
    Partial,
    /// Los servicios que toca no existen en este equipo.
    Unavailable,
    /// No se pudo determinar (falló el script de detección).
    Unknown,
    /// Tarea puntual, sin estado.
    Action,
}

fn script_true(out: &str) -> bool {
    out.lines().last().is_some_and(|l| l.trim().eq_ignore_ascii_case("true"))
}

pub fn detect(t: &Tweak) -> Status {
    if t.kind == Kind::Action {
        return Status::Action;
    }
    let mut checks: Vec<bool> = t.registry.iter().map(registry::is_applied).collect();
    for s in &t.service {
        if let Some(current) = service::startup(&s.name) {
            checks.push(current == s.startup);
        }
    }
    if let Some(script) = t.script.as_ref().and_then(|s| s.detect.as_deref()) {
        match ps::powershell(script) {
            Ok(out) => checks.push(script_true(&out)),
            Err(_) => return Status::Unknown,
        }
    }
    match (checks.is_empty(), checks.iter().filter(|&&c| c).count()) {
        (true, _) if !t.service.is_empty() => Status::Unavailable,
        (true, _) => Status::Unknown,
        (_, 0) => Status::NotApplied,
        (_, n) if n == checks.len() => Status::Applied,
        _ => Status::Partial,
    }
}

/// Aplica el ajuste. Si algo falla a mitad, deshace lo ya hecho antes de devolver el error.
pub fn apply(t: &Tweak) -> Result<Vec<Backup>, String> {
    let mut done: Vec<Backup> = Vec::new();
    let result = (|| {
        for r in &t.registry {
            done.push(Backup::Registry {
                path: r.path.clone(),
                name: r.name.clone(),
                previous: registry::read_raw(&r.path, &r.name),
            });
            registry::write(&r.path, &r.name, r.kind, &r.value)?;
        }
        for s in &t.service {
            let Some(current) = service::startup(&s.name) else { continue };
            let was_running = service::is_running(&s.name);
            service::set_startup(&s.name, s.startup)?;
            done.push(Backup::Service { name: s.name.clone(), startup: current, was_running });
            if s.stop {
                service::stop(&s.name);
            }
        }
        if let Some(script) = t.script.as_ref().and_then(|s| s.apply.as_deref()) {
            let out = ps::powershell(script).map_err(|e| format!("Script: {e}"))?;
            let previous = out.lines().last().map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
            done.push(Backup::Script { previous });
        }
        Ok(())
    })();

    match result {
        Ok(()) => Ok(done),
        Err(e) => {
            // `Backup::Script` solo se añade si el script tuvo éxito, así que esto
            // restaura únicamente el registro y los servicios ya tocados.
            let _ = restore(Some(t), &done);
            Err(e)
        }
    }
}

/// Restaura un conjunto de copias guardadas en el diario. `t` es el ajuste del
/// catálogo que las generó, si lo hay (cambios de Inicio o apps no tienen).
pub fn restore(t: Option<&Tweak>, backups: &[Backup]) -> Result<(), String> {
    let mut errors = Vec::new();
    for b in backups.iter().rev() {
        let r = match b {
            Backup::Registry { path, name, previous: Some(raw) } => registry::write_raw(path, name, raw),
            Backup::Registry { path, name, previous: None } => registry::delete(path, name),
            Backup::Service { name, startup, was_running } => {
                let r = service::set_startup(name, *startup);
                if r.is_ok() && *was_running {
                    service::start(name);
                }
                r
            }
            Backup::Script { previous } => match t {
                Some(t) => run_revert_script(t, previous.as_deref()),
                None => Err("Falta el ajuste de origen del script".into()),
            },
            Backup::Task { path, name, was_enabled } => super::startup::set_task_enabled(path, name, *was_enabled),
            Backup::Appx { name, store_id: Some(id) } => super::appx::reinstall(name, id),
            Backup::Appx { name, store_id: None } => Err(format!("{name} no se puede reinstalar automáticamente")),
        };
        if let Err(e) = r {
            errors.push(e);
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join(" · ")) }
}

/// Revierte a los valores de fábrica declarados en el catálogo (sin historial).
pub fn revert_to_defaults(t: &Tweak) -> Result<(), String> {
    let mut errors = Vec::new();
    for r in &t.registry {
        let res = match &r.default {
            Some(d) => registry::write(&r.path, &r.name, r.kind, d),
            None => registry::delete(&r.path, &r.name),
        };
        if let Err(e) = res {
            errors.push(e);
        }
    }
    for s in &t.service {
        if service::exists(&s.name) {
            if let Err(e) = service::set_startup(&s.name, s.default) {
                errors.push(e);
            } else if s.default != Startup::Disabled && s.default != Startup::Manual {
                service::start(&s.name);
            }
        }
    }
    if t.script.as_ref().is_some_and(|s| s.revert.is_some()) {
        if let Err(e) = run_revert_script(t, None) {
            errors.push(e);
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join(" · ")) }
}

fn run_revert_script(t: &Tweak, previous: Option<&str>) -> Result<(), String> {
    match t.script.as_ref().and_then(|s| s.revert.as_deref()) {
        Some(script) => {
            let prelude = match previous {
                Some(p) => format!("$Previous = '{}'\n", p.replace('\'', "''")),
                None => "$Previous = $null\n".to_string(),
            };
            ps::powershell(&format!("{prelude}{script}")).map(|_| ()).map_err(|e| format!("Script: {e}"))
        }
        None => Err("Este ajuste no tiene script para deshacer".into()),
    }
}

/// Ejecuta una tarea puntual y devuelve la última línea de su salida como resumen.
pub fn run_action(t: &Tweak) -> Result<String, String> {
    let script = t
        .script
        .as_ref()
        .and_then(|s| s.run.as_deref())
        .ok_or("Esta tarea no tiene nada que ejecutar")?;
    let out = ps::powershell(script)?;
    Ok(out.lines().last().unwrap_or("Completado").trim().to_string())
}

/// Descripción legible de lo que toca el ajuste, para que el técnico vea exactamente qué cambia.
pub fn describe(t: &Tweak) -> Vec<String> {
    let startup = |s: Startup| match s {
        Startup::Automatic => "Automático",
        Startup::Delayed => "Automático (retrasado)",
        Startup::Manual => "Manual",
        Startup::Disabled => "Deshabilitado",
    };
    let data = |d: &RegData| match d {
        RegData::Int(n) => n.to_string(),
        RegData::Str(s) => format!("\"{s}\""),
    };
    let mut out: Vec<String> = t
        .registry
        .iter()
        .map(|r| format!("{}\\{} = {}", r.path, r.name, data(&r.value)))
        .collect();
    out.extend(t.service.iter().map(|s| {
        format!("Servicio {} → {}{}", s.name, startup(s.startup), if s.stop { " y detener" } else { "" })
    }));
    if let Some(s) = &t.script {
        if s.apply.is_some() || s.run.is_some() {
            out.push("Script de PowerShell".into());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tweaks::model::{CatalogFile, RegKind};

    const KEY: &str = r"HKCU\Software\AdminOpsTest";

    fn tweak() -> Tweak {
        let src = format!(
            r#"
[[tweak]]
id = "test.roundtrip"
name = "t"
description = "t"
category = "test"
risk = "low"

[[tweak.registry]]
path = '{KEY}'
name = "Existing"
type = "dword"
value = 0

[[tweak.registry]]
path = '{KEY}'
name = "Missing"
type = "string"
value = "on"
"#
        );
        toml::from_str::<CatalogFile>(&src).unwrap().tweak.remove(0)
    }

    #[test]
    fn apply_then_restore_is_exact() {
        let _ = registry::delete(KEY, "Missing");
        // Valor previo con un tipo distinto al del ajuste: debe volver tal cual.
        registry::write(KEY, "Existing", RegKind::Qword, &RegData::Int(7)).unwrap();
        let t = tweak();
        assert_eq!(detect(&t), Status::NotApplied);

        let backups = apply(&t).unwrap();
        assert_eq!(detect(&t), Status::Applied);

        restore(Some(&t), &backups).unwrap();
        assert_eq!(detect(&t), Status::NotApplied);
        let raw = registry::read_raw(KEY, "Existing").unwrap();
        assert_eq!(raw.vtype, 11); // REG_QWORD
        assert_eq!(raw.bytes, 7u64.to_le_bytes());
        assert!(registry::read_raw(KEY, "Missing").is_none());

        let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).delete_subkey_all(r"Software\AdminOpsTest");
    }
}
