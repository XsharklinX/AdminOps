//! Catálogo de ajustes incrustado en el binario: la UI solo puede pedir ajustes
//! por `id`, nunca enviar scripts propios.

use super::model::{CatalogFile, Kind, Tweak};

const FILES: &[(&str, &str)] = &[
    ("privacy.toml", include_str!("../../tweaks/privacy.toml")),
    ("services.toml", include_str!("../../tweaks/services.toml")),
    ("cleanup.toml", include_str!("../../tweaks/cleanup.toml")),
    ("performance.toml", include_str!("../../tweaks/performance.toml")),
    ("repair.toml", include_str!("../../tweaks/repair.toml")),
    ("security.toml", include_str!("../../tweaks/security.toml")),
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
        // Los scripts deben actuar sobre el usuario destino, no sobre la cuenta
        // que elevó AdminOps (ver target_user::script_prelude).
        if let Some(sc) = script {
            for body in [&sc.detect, &sc.apply, &sc.revert, &sc.run].into_iter().flatten() {
                let lower = body.to_ascii_lowercase();
                for bad in ["$env:temp", "$env:tmp", "$env:localappdata", "$env:appdata", "$env:userprofile", "hkcu:", "clear-recyclebin"] {
                    if lower.contains(bad) {
                        errors.push(format!("{}: usa {bad}; usa las variables $User* del usuario destino", t.id));
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

    /// Analiza (sin ejecutar) todos los scripts del catálogo con el parser de
    /// PowerShell: un error de sintaxis se detecta aquí y no en el equipo de un cliente.
    #[test]
    fn scripts_parse() {
        let parse = |code: &str| {
            let probe = format!(
                "{}$e = $null; [void][System.Management.Automation.Language.Parser]::ParseInput($code, [ref]$null, [ref]$e)
                 ($e | ForEach-Object {{ \"$($_.Extent.StartLineNumber): $($_.Message)\" }}) -join ' | '",
                crate::ps::text_var("code", code)
            );
            crate::pspool::query(&probe, None, "t").unwrap()
        };
        // La comprobación de verdad detecta errores.
        assert!(!parse("if ($x -eq 1 { 'a' }").trim().is_empty());
        let mut checked = 0;
        for t in super::load() {
            let Some(sc) = &t.script else { continue };
            for (part, code) in [("detect", &sc.detect), ("apply", &sc.apply), ("revert", &sc.revert), ("run", &sc.run)] {
                let Some(code) = code else { continue };
                let errors = parse(code);
                assert!(errors.trim().is_empty(), "{} ({part}): {errors}", t.id);
                checked += 1;
            }
        }
        assert!(checked >= 25, "solo {checked} scripts");
    }
}

#[cfg(test)]
mod solution_links {
    /// Las soluciones que trae AdminOps llevan botones «Hacerlo ahora» que
    /// apuntan a ajustes del catálogo y a herramientas de Windows. Si un id no
    /// existe, el técnico se encontraría un botón roto delante del cliente.
    #[test]
    fn solution_buttons_point_somewhere_real() {
        let ts = include_str!("../../../src/lib/solutionsCatalog.ts");
        let tweaks: Vec<String> = super::load().into_iter().map(|t| t.id).collect();
        let tools: Vec<String> = crate::toolbox::catalog_ids();

        let mut bad = Vec::new();
        for line in ts.lines().map(str::trim) {
            let Some(kind) = ["fix", "tool"].into_iter().find(|k| line.contains(&format!("kind: \"{k}\""))) else { continue };
            let Some(rest) = line.split_once("id: \"") else { continue };
            let Some((id, _)) = rest.1.split_once('"') else { continue };
            let known = if kind == "fix" { tweaks.iter().any(|t| t == id) } else { tools.iter().any(|t| t == id) };
            if !known {
                bad.push(format!("{kind}: {id}"));
            }
        }
        assert!(bad.is_empty(), "las soluciones apuntan a ids que no existen: {bad:#?}");
    }
}
