//! Catálogo de ajustes incrustado en el binario: la UI solo puede pedir ajustes
//! por `id`, nunca enviar scripts propios.

use super::model::{CatalogFile, Kind, Tweak};

const FILES: &[(&str, &str)] = &[
    ("privacy.toml", include_str!("../../tweaks/privacy.toml")),
    ("services.toml", include_str!("../../tweaks/services.toml")),
    ("cleanup.toml", include_str!("../../tweaks/cleanup.toml")),
    ("performance.toml", include_str!("../../tweaks/performance.toml")),
];

pub fn load() -> Vec<Tweak> {
    let tweaks: Vec<Tweak> = FILES
        .iter()
        .flat_map(|(name, src)| {
            toml::from_str::<CatalogFile>(src)
                .unwrap_or_else(|e| panic!("Catálogo {name} inválido: {e}"))
                .tweak
        })
        .collect();
    debug_assert!(validate(&tweaks).is_empty(), "{:#?}", validate(&tweaks));
    tweaks
}

/// Reglas que el formato TOML no puede expresar. Se comprueban en los tests.
pub fn validate(tweaks: &[Tweak]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for t in tweaks {
        if !ids.insert(&t.id) {
            errors.push(format!("{}: id duplicado", t.id));
        }
        if !t.id.starts_with(&format!("{}.", t.category)) {
            errors.push(format!("{}: el id debe empezar por la categoría '{}.'", t.id, t.category));
        }
        let script = t.script.as_ref();
        match t.kind {
            Kind::Action => {
                if script.and_then(|s| s.run.as_ref()).is_none() {
                    errors.push(format!("{}: una acción necesita script.run", t.id));
                }
            }
            Kind::Toggle => {
                if t.registry.is_empty() && t.service.is_empty() && script.and_then(|s| s.apply.as_ref()).is_none() {
                    errors.push(format!("{}: el ajuste no hace nada", t.id));
                }
                if let Some(s) = script {
                    if s.apply.is_some() && (s.detect.is_none() || s.revert.is_none()) {
                        errors.push(format!("{}: script.apply necesita detect y revert", t.id));
                    }
                }
            }
        }
        for r in &t.registry {
            if !r.path.contains('\\') {
                errors.push(format!("{}: ruta de registro inválida {}", t.id, r.path));
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_is_valid() {
        let tweaks = super::load();
        assert!(!tweaks.is_empty());
        let errors = super::validate(&tweaks);
        assert!(errors.is_empty(), "{errors:#?}");
    }
}
