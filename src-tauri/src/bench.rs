//! Mediciones de rendimiento (solo lectura): `cargo test --release bench -- --ignored --nocapture`

#[cfg(test)]
mod tests {
    use std::time::Instant;

    fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
        let t = Instant::now();
        let r = f();
        println!("{label:<40} {:>6} ms", t.elapsed().as_millis());
        r
    }

    #[test]
    #[ignore]
    fn bench() {
        time("10 × PowerShell trivial (secuencial)", || {
            for _ in 0..10 {
                crate::ps::powershell("1").unwrap();
            }
        });
        let state = crate::tweaks::TweakState::with_data_dir(std::env::temp_dir().join("adminops-bench"));
        time("Detectar todo el catálogo", || state.applied_count());
        time("Detectar todo el catálogo (2ª vez)", || state.applied_count());
        time("Listar Inicio", || crate::tweaks::startup::enabled_names().unwrap());
        time("Listar apps", || crate::tweaks::appx::recommended_installed().unwrap());
    }
}
