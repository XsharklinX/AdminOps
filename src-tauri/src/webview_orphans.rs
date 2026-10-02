//! Procesos de WebView2 que se quedaron vivos de una sesión anterior.
//!
//! Cuando AdminOps se cierra de golpe (el instalador lo cierra para
//! actualizar, se cuelga, se apaga el equipo a medias…), a veces los procesos
//! del navegador interno (`msedgewebview2.exe`) siguen vivos con su perfil
//! abierto. El siguiente AdminOps que usa ese perfil se engancha a ellos, y si
//! están atascados, Correo y Teams se quedan en blanco para siempre, sesión
//! tras sesión. Pasó con la 1.1.8 en un pendrive.
//!
//! Al arrancar, antes de crear ninguna vista, se cierran los de los perfiles de
//! AdminOps, pero solo si no hay otro AdminOps abierto (serían suyos).

/// ¿Es un proceso de WebView2 con un perfil de AdminOps? Por su línea de
/// órdenes: `--user-data-dir=…\AdminOps\WebView…` o `…\AdminOps-data\…`.
pub fn uses_adminops_profile(cmdline: &str) -> bool {
    let l = cmdline.to_lowercase().replace('/', "\\");
    let Some(i) = l.find("--user-data-dir=") else { return false };
    let dir = &l[i + "--user-data-dir=".len()..];
    let dir = dir.trim_start_matches('"');
    let dir = dir.split('"').next().unwrap_or(dir);
    dir.contains("\\adminops\\webview") || dir.contains("\\adminops-data\\") || dir.contains("\\temp\\adminops\\webview")
}

/// Cierra los procesos de WebView2 huérfanos de AdminOps. Devuelve cuántos.
pub fn kill_orphans() -> usize {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet));
    let me = sysinfo::Pid::from_u32(std::process::id());
    // Otro AdminOps abierto: sus vistas no se tocan.
    let other = sys.processes().values().any(|p| p.pid() != me && p.name().to_string_lossy().eq_ignore_ascii_case("adminops.exe"));
    if other {
        return 0;
    }
    let mut n = 0;
    for p in sys.processes().values() {
        if !p.name().to_string_lossy().eq_ignore_ascii_case("msedgewebview2.exe") {
            continue;
        }
        let cmd = p.cmd().iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ");
        if uses_adminops_profile(&cmd) && p.kill() {
            n += 1;
        }
    }
    if n > 0 {
        // Que terminen de soltar el perfil antes de que WebView2 lo abra.
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura: cuántos procesos de WebView2 con perfil de AdminOps hay ahora.
    #[test]
    #[ignore = "mira los procesos reales"]
    fn lists_real_candidates() {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        let mut sys = System::new();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet));
        let found: Vec<_> = sys
            .processes()
            .values()
            .filter(|p| p.name().to_string_lossy().eq_ignore_ascii_case("msedgewebview2.exe"))
            .filter(|p| uses_adminops_profile(&p.cmd().iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ")))
            .map(|p| p.pid())
            .collect();
        let readable = sys
            .processes()
            .values()
            .filter(|p| p.name().to_string_lossy().eq_ignore_ascii_case("msedgewebview2.exe"))
            .filter(|p| p.cmd().iter().any(|a| a.to_string_lossy().starts_with("--user-data-dir=")))
            .count();
        println!("candidatos: {} · WebView2 con perfil legible: {readable}", found.len());
    }

    #[test]
    fn recognises_only_adminops_profiles() {
        assert!(uses_adminops_profile(r#"msedgewebview2.exe --embedded-browser-webview=1 --user-data-dir="G:\AdminOps\AdminOps-data\equipos\PC1\webview\EBWebView" --noerrdialogs"#));
        assert!(uses_adminops_profile(r#"msedgewebview2.exe --type=renderer --user-data-dir="C:\Users\x\AppData\Local\AdminOps\WebView\EBWebView" --lang=es"#));
        assert!(!uses_adminops_profile(r#"msedgewebview2.exe --user-data-dir="C:\Users\x\AppData\Local\Packages\5319275A.WhatsAppDesktop_cv1g1gvanyjgm\LocalCache\EBWebView""#));
        assert!(!uses_adminops_profile("msedgewebview2.exe --type=crashpad-handler"));
    }
}
