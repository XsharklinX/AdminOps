//! Prueba de humo: ejecuta de verdad todas las lecturas de AdminOps en este
//! equipo (PowerShell, WMI, registro, red…) y cuenta las que fallan.
//! `cargo test --lib smoke_all -- --ignored --nocapture`
//! Sin administrador algunas fallan a propósito (claves de BitLocker, Wi-Fi…).

#[cfg(test)]
mod tests {
    use std::time::Instant;

    fn check<T, E: std::fmt::Display>(name: &str, fails: &mut Vec<String>, f: impl FnOnce() -> Result<T, E>) {
        let t = Instant::now();
        let r = f();
        let ms = t.elapsed().as_millis();
        match r {
            Ok(_) => println!("  ok   {ms:>6} ms  {name}"),
            Err(e) => {
                let msg: String = e.to_string().chars().take(160).collect();
                println!("  FALLA {ms:>5} ms  {name}: {msg}");
                fails.push(format!("{name}: {msg}"));
            }
        }
    }

    #[test]
    #[ignore]
    fn smoke_all() {
        let elevated = crate::elevation::is_elevated();
        println!("Administrador: {elevated}");
        let mut f = Vec::new();
        let total = Instant::now();

        // Diagnóstico
        check("diagnóstico: discos", &mut f, crate::diagnostics::collect::disks);
        check("diagnóstico: estabilidad", &mut f, crate::diagnostics::collect::stability);
        check("diagnóstico: drivers", &mut f, crate::diagnostics::collect::drivers);
        check("diagnóstico: batería", &mut f, crate::diagnostics::collect::battery);
        check("diagnóstico: sistema", &mut f, crate::diagnostics::collect::system);
        // Hardware
        check("hardware: inventario", &mut f, crate::hardware::inventory);
        check("hardware: SMART", &mut f, crate::hardware::smart::read);
        check("hardware: prueba de memoria", &mut f, crate::hardware::memory_test);
        // Seguridad
        check("seguridad: extra", &mut f, crate::security::extra);
        check("seguridad: cuentas", &mut f, || crate::security::accounts().ok_or("sin datos"));
        check("seguridad: auditoría", &mut f, crate::security::audit);
        check("seguridad: BitLocker", &mut f, crate::security::bitlocker_status);
        check("seguridad: elementos sospechosos", &mut f, crate::security::suspicious_items);
        check("seguridad: extensiones", &mut f, crate::security::browser_extensions);
        // Programas y Windows
        check("programas: actualizaciones (winget)", &mut f, crate::software::list);
        check("programas: instalados", &mut f, || Ok::<_, String>(crate::programs::list()));
        check("inicio de Windows", &mut f, crate::tweaks::startup::list_startup);
        check("bloatware: apps", &mut f, crate::tweaks::appx::list_apps);
        check("windows update: historial", &mut f, crate::winupdate::history);
        check("windows update: pausa", &mut f, || Ok::<_, String>(crate::winupdate::update_pause_state()));
        check("mantenimiento: arranque", &mut f, crate::maintenance::boot_analysis);
        check("mantenimiento: puntos de restauración", &mut f, crate::maintenance::restore_storage);
        check("puntos de restauración", &mut f, crate::tweaks::list_restore_points);
        // Administración
        check("usuarios locales", &mut f, crate::users::list);
        check("dominio", &mut f, crate::domain::status);
        check("impresoras", &mut f, crate::printers::list_printers);
        check("carpetas compartidas", &mut f, crate::office::list_shares);
        check("control parental: horarios", &mut f, crate::family::logon_hours);
        check("control parental: filtro DNS", &mut f, crate::family::dns_filter_status);
        check("control parental: sitios", &mut f, crate::family::blocked_sites);
        check("recuperar: unidades", &mut f, crate::recover::recover_status);
        // Red
        check("red: información", &mut f, || crate::network::lan::current().and_then(|i| i.ok_or("sin red".into())));
        check("red: puertos", &mut f, crate::network::tools::list_ports);
        check("red: DNS", &mut f, crate::network::tools::dns_adapters);
        check("red: hosts", &mut f, crate::network::tools::read_hosts);
        check("red: Wi-Fi", &mut f, crate::network::wifi::list);
        check("red: estado de la tarjeta Wi-Fi", &mut f, crate::network::wifictl::wifi_state);
        check("red: conflictos de IP", &mut f, crate::office::ip_conflicts);
        check("acceso remoto: estado", &mut f, crate::office::remote_status);
        check("acceso remoto: Escritorio remoto", &mut f, crate::remote::rdp_server);
        check("acceso remoto: herramientas", &mut f, || Ok::<_, String>(crate::remote::remote_tools()));
        // 1.1.2
        check("ficha del equipo", &mut f, crate::sheet::machine_sheet);
        check("este equipo y esta red (notas)", &mut f, || {
            let p = crate::library::place_uncached();
            if p.machine.is_empty() { Err("sin clave de equipo") } else { Ok(p) }
        });
        check("comprobar puestos (este equipo)", &mut f, || crate::stations::check_stations(vec!["127.0.0.1".into()], false));
        // Solucionar problemas
        for s in ["internet", "wifi", "audio", "bluetooth", "display", "printer", "slow", "winupdate"] {
            check(&format!("solucionar: {s}"), &mut f, || crate::troubleshoot::troubleshoot_check(s.into()));
        }

        println!("\n{} lecturas en {:.1} s, {} fallos", 52, total.elapsed().as_secs_f64(), f.len());
        for x in &f {
            println!("  - {x}");
        }
    }
}
