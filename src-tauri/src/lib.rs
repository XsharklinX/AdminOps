mod agenda;
mod bootlog;
mod comms;
mod context;
mod perfhistory;
mod problems;
mod apps;
mod boottime;
mod diagnostics;
mod domain;
mod drivers;
mod elevation;
mod connected;
mod displays;
mod netusage;
mod faultrec;
mod onedrive;
mod scanfolder;
mod kbdlang;
mod unsaved;
mod hardware;
mod keys;
mod maintenance;
mod metrics;
mod migrate;
mod mini;
mod minimon;
mod network;
mod nightjobs;
mod cases;
mod actioninfo;
mod measure;
mod planrun;
mod followups;
mod snmp;
mod storage;
mod company;
mod webview_orphans;
mod diskerrors;
mod disks;
mod disktools;
mod casetones;
mod diskreport;
mod labels;
mod partsadvice;
mod partsio;
mod lifeio;
mod lifespan;
mod labelsio;
mod diskscan;
mod diskscanio;
mod diskwatch;
mod alertrules;
mod alertrulesio;
mod backupcheck;
mod backupio;
mod carve;
mod carveio;
mod clone;
mod cloneio;
mod partitions;
mod partitionsio;
mod rawdisk;
mod smartio;
mod smartx;
mod toast;
mod evtwatch;
mod games;
mod safestart;
mod ocr;
mod discovery;
mod quicknote;
mod paths;
mod people;
mod portable_exit;
mod portals;
mod printers;
mod printvendor;
mod processes;
mod programs;
mod security;
mod software;
mod support;
mod space;
mod bench;
mod ps;
mod pspool;
mod task;
mod target_user;
mod toolbox;
mod tray;
mod tweaks;
mod users;
mod window_state;
mod conpty;
mod winupdate;
mod wucause;
mod wingetout;
mod installq;
mod workflow;
mod vault;
mod wipe;
mod recover;
mod family;
mod office;
mod shares;
mod applock;
mod appcare;
mod winwatch;
mod audiofix;
mod defapps;
mod power;
mod remote;
mod rocket;
mod rocketplus;
mod usbpolicy;
mod camfix;
mod lostwin;
mod rocketbar;
mod webnotif;
mod wingetfix;
mod accounts;
mod appbackup;
mod audit;
mod contacts;
mod library;
mod officemap;
mod sheet;
mod shellopen;
mod stations;
mod whatchanged;
mod askpc;
mod secrets;
mod smoke;
mod timeline;
mod troubleshoot;
mod uxio;
mod fixes;
mod errcodes;
mod autoruns;
mod crashes;
mod stress;
mod perfinsight;
mod eventdigest;
mod learned;
mod escalate;
mod checksums;
mod datacrypt;
mod errorlog;
mod outbound;
mod psquote;
mod rescue;
mod vulndb;
mod waitchain;
mod filelocks;
mod errlens;
mod testuser;
mod culprit;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    boottime::mark_start();
    // Un fallo inesperado queda en el registro (con dónde ocurrió) en lugar de perderse.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("Fallo interno: {info}");
        default_hook(info);
    }));
    // Modo prueba: `adminops.exe --roundtrip informe.json [--only id1,id2]`.
    // No abre ventana; ver tweaks/roundtrip.rs.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--roundtrip") {
        let out = args.get(i + 1).map_or("roundtrip.json", String::as_str);
        let only = args.iter().position(|a| a == "--only").and_then(|j| args.get(j + 1)).map(String::as_str);
        std::process::exit(tweaks::roundtrip::run(out, only));
    }
    // Mantenimiento programado (lo lanza la tarea del sistema, sin ventana).
    if args.iter().any(|a| a == "--maintenance") {
        std::process::exit(maintenance::run_cli(&args));
    }

    // Carpeta de datos de WebView2 (WebView2 respeta esta variable). En portable
    // va al USB; instalado, una carpeta propia y escribible por este usuario, para
    // no depender de la de por defecto (que puede quedar de otro usuario y hacer
    // que WebView2 no abra: «can't read and write to its data directory»).
    // AdminOps en un pendrive por primera vez: se trae lo de este equipo antes
    // de abrir nada (ver storage.rs). El registro aún no existe: se anota luego.
    let migrated = boottime::step("Datos al pendrive", paths::migrate_to_portable_if_needed);
    if let Some(m) = &migrated {
        std::env::set_var("ADMINOPS_MIGRATED", format!("{} archivos, {} contraseñas, sesión del navegador: {}", m.files, m.secrets, if m.browser { "sí" } else { "no" }));
    }
    // Procesos del navegador interno que quedaron vivos de una sesión anterior:
    // si siguen con el perfil abierto, las vistas nuevas se enganchan a ellos y
    // no arrancan nunca (ver webview_orphans.rs).
    // `--rocket`: solo la ventanita del cohete, con AdminOps cerrada (arranca con Windows).
    // No toca los procesos del navegador interno: podrían ser los de una AdminOps abierta.
    let rocket_only = args.iter().any(|a| a == "--rocket");
    if rocket_only {
        rocketplus::mark_rocket_only();
    }
    let orphans = if rocket_only { 0 } else { boottime::step("Navegador de sesiones anteriores", webview_orphans::kill_orphans) };
    if orphans > 0 {
        std::env::set_var("ADMINOPS_ORPHANS", orphans.to_string());
    }
    if let Some(dir) = boottime::step("Carpeta de datos de WebView2", || paths::portable_webview_dir().or_else(paths::installed_webview_dir)) {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
        portable_exit::set_webview_dir(dir);
    }

    let context = boottime::step("Configuración de la ventana", || tauri::generate_context!());
    // Intranets de la empresa (p. ej. *.empresa.local): el navegador interno entra
    // con la cuenta de Windows sin pedir usuario y contraseña. Solo los dominios
    // de los portales del técnico. Mismos argumentos para todas las vistas: WebView2
    // no admite dos configuraciones distintas en la misma carpeta de datos.
    if let Some(auth) = boottime::step("Portales con la cuenta de Windows", portals::integrated_auth_arg) {
        let base = context.config().app.windows.first().and_then(|w| w.additional_browser_args.clone()).unwrap_or_default();
        // Los portales leen esta misma variable, para arrancar con argumentos
        // idénticos a los de la ventana principal (ver portals::browser_args).
        std::env::set_var(portals::ARGS_ENV, portals::merged_browser_args(&base, Some(&auth)));
    }

    // A partir de aquí manda Tauri: crear la ventana y arrancar WebView2. Esa
    // parte no se puede medir desde dentro, pero sí cuándo empieza: la
    // diferencia con «Preparar PowerShell» (ya en setup) es lo que cuesta.
    boottime::step("Listo para crear la ventana", || ());
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(tauri_plugin_log::Target::new(paths::log_target()))
                .level(log::LevelFilter::Info)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .max_file_size(1_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            // El cohete: se guarda dónde lo dejan.
            if window.label() == rocket::LABEL {
                if let tauri::WindowEvent::Moved(p) = event {
                    rocket::on_moved(window.app_handle(), p.x, p.y);
                }
                return;
            }
            if window.label() != "main" || rocketplus::rocket_only() {
                return;
            }
            // Queda por escrito cómo y cuándo se cierra. Sin esto, una app que
            // desaparece sola no deja ni rastro en el registro y no hay forma de
            // saber si la cerró el usuario, Windows, o se murió ella.
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    window_state::save(window);
                    // Ajustes → General: la X solo la quita de en medio.
                    if window_state::minimizes_on_close() {
                        api.prevent_close();
                        // Con el icono junto al reloj se esconde ahí; sin él, a la barra de tareas.
                        if tray::visible() {
                            let _ = window.hide();
                            tray::tell_where(window.app_handle());
                        } else {
                            let _ = window.minimize();
                        }
                        log::info!("Cierre pedido: se minimiza (ajuste «Al cerrar, minimizar»)");
                    } else {
                        log::info!("Cierre pedido a los {} ms", boottime::since_start_ms());
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    log::info!("Ventana destruida a los {} ms", boottime::since_start_ms());
                    minimon::close(window.app_handle());
                    mini::close(window.app_handle());
                    rocket::close(window.app_handle());
                    // Si el cohete arranca con Windows, se queda él solo al cerrar AdminOps.
                    rocketplus::leave_rocket_behind(window.app_handle());
                    // Un cambio de pantalla sin confirmar se deshace antes de salir.
                    displays::restore_pending_on_exit(window.app_handle());
                    // La sesión de ETW y la espera de un escaneo se paran con la ventana.
                    netusage::shutdown();
                    scanfolder::shutdown();
                    // Una grabación en marcha se cierra bien (el MP4 queda entero) y las sin caso se borran.
                    faultrec::shutdown(window.app_handle());
                }
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => window_state::watch_monitor(window),
                _ => {}
            }
        })
        .manage(metrics::MetricsState::new())
        .manage(processes::ProcessState::new())
        .setup(|app| {
            boottime::step("Preparar PowerShell", pspool::warm_up);
            log::info!(
                "AdminOps {} iniciado · admin={} · portable={}",
                app.package_info().version,
                elevation::is_elevated(),
                paths::is_portable()
            );
            if let Ok(n) = std::env::var("ADMINOPS_ORPHANS") {
                log::warn!("Cerrados {n} procesos del navegador interno que seguían vivos de una sesión anterior");
            }
            if let Ok(m) = std::env::var("ADMINOPS_MIGRATED") {
                log::info!("Datos de este equipo traídos al pendrive: {m}");
            }
            let state =boottime::step("Catálogo de ajustes e historial", || tweaks::TweakState::new(app.handle()));
            app.manage(state);
            if rocketplus::rocket_only() {
                // Solo el cohete: sin ventana principal ni nada en segundo plano. Primero el cohete y luego
                // se quita la principal: si en algún momento no quedara ninguna ventana, el proceso se cerraría.
                if let Err(e) = rocket::open(app.handle()) {
                    log::warn!("Solo el cohete: {e}");
                    std::process::exit(1);
                }
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.destroy();
                }
                rocketplus::start_hotkey(app.handle().clone());
                rocketbar::start_hotkey(app.handle().clone());
                rocketplus::start_brain(app.handle().clone());
                rocketplus::watch_full(app.handle().clone());
                return Ok(());
            }
            rocketplus::mark_full_running();
            boottime::step("Tamaño y posición de la ventana", || window_state::restore(app.handle()));
            // La ventana se crea oculta y se enseña cuando la interfaz ya tiene
            // algo pintado, para que nunca se vea un rectángulo negro.
            window_state::watch_first_paint(app.handle().clone());
            let settings = boottime::step("Ajustes", || workflow::settings(app.handle()));
            tweaks::set_restore_point_policy(&settings.restore_points);
            window_state::set_close_minimizes(settings.close_minimizes);
            if settings.tray_icon {
                tray::set_visible(app.handle(), true);
            }
            // Arranque con Windows: minimizada en la barra de tareas.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            // Dos cierres de golpe seguidos: se abre a salvo, sin nada en segundo plano (ver safestart.rs).
            let safe = boottime::step("¿Se cerró bien la última vez?", || safestart::begin(app.handle())).safe;
            if !safe {
                // Carga en segundo plano los módulos de PowerShell más lentos (Defender,
                // BitLocker), para que el primer diagnóstico del día no pague esa carga.
                security::warm_up();
                // Vigilancia de errores de Windows (Ajustes → General).
                boottime::step("Vigilancia de errores de Windows", || winwatch::start(app.handle().clone()));
                // Vigilancia de dispositivos clave de la red (solo si hay alguno marcado).
                boottime::step("Vigilancia de la red", || officemap::start(app.handle().clone()));
                // Agenda de mantenimientos: resumen del día y aviso antes de cada visita.
                agenda::start(app.handle().clone());
                disks::start_watch(app.handle().clone());
            }
            uxio::watch_drops(app.handle());
            if !safe {
                backupio::start(app.handle().clone());
                alertrulesio::start(app.handle().clone());
                // Sin prisa y en orden, cuando la primera pantalla ya está pintada (o a los 20 s):
                // el historial de rendimiento de 7 días (una muestra por minuto mientras AdminOps está abierta)
                // y lo apuntado para la noche (superficie de un disco, reparar Windows…).
                let late = app.handle().clone();
                std::thread::spawn(move || {
                    window_state::wait_until_ready(std::time::Duration::from_secs(20));
                    // El cohete en el escritorio (Ajustes → General).
                    if workflow::settings(&late).rocket == "desktop" {
                        rocket::apply_mode(&late, "desktop");
                    }
                    perfhistory::start(late.clone());
                    // Una foto al día del equipo para «¿Qué cambió?».
                    whatchanged::start(late.clone());
                    nightjobs::start(late);
                });
                appbackup::start(app.handle().clone());
            }
            // Nota de llamada con Ctrl+Alt+N, aunque AdminOps esté minimizado.
            quicknote::start(app.handle().clone());
            // AdminOps en miniatura con Ctrl+Alt+A.
            mini::start(app.handle().clone());
            // El cohete: Ctrl+Alt+O, programa colgado y despegue automático.
            rocketplus::start_hotkey(app.handle().clone());
                rocketbar::start_hotkey(app.handle().clone());
            rocketplus::start_brain(app.handle().clone());
            // Lo que se enchufa y se quita (Hardware → Conectados). También a salvo: su ventana oculta
            // es la que avisa a Pantallas de que cambió un monitor.
            connected::start(app.handle().clone());
            // El aviso de que cambió la pantalla lo da la misma ventana oculta; aquí se guarda a quién avisar.
            displays::start(app.handle().clone());
            // Uso de la red y escanear a carpeta: se guarda la app y se deja como estaba lo que una ejecución anterior dejó a medias.
            netusage::start(app.handle().clone());
            onedrive::start(app.handle().clone());
            scanfolder::start(app.handle().clone());
            // Grabar el fallo: borra lo que quedó a medias o de un caso que ya no está.
            faultrec::start(app.handle().clone());
            if std::env::args().any(|a| a == "--auditoria") {
                audit::set(true);
            }
            // Limpieza de datos antiguos, si está activada (en segundo plano).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if safestart::is_safe() {
                    return;
                }
                use tauri::Manager;
                // Recorre carpetas: se deja para cuando la app ya esté abierta.
                std::thread::sleep(std::time::Duration::from_secs(20));
                let state = handle.state::<tweaks::TweakState>();
                appcare::auto_cleanup(&handle, &state);
            });
            boottime::step("Fin del arranque del programa", || ());
            Ok(())
        })
        // Modo auditoría: lo que cambia el equipo se rechaza en un único punto.
        .invoke_handler(audit::guard(tauri::generate_handler![
            elevation::is_admin,
            boottime::startup_timing,
            boottime::log_timing,
            elevation::relaunch_as_admin,
            metrics::get_system_info,
            metrics::get_live_metrics,
            tweaks::list_tweaks,
            tweaks::tweak_index,
            tweaks::apply_tweak,
            tweaks::revert_tweak,
            tweaks::revert_entry,
            tweaks::run_action,
            tweaks::fix_finding,
            tweaks::get_journal,
            tweaks::create_restore_point,
            tweaks::list_restore_points,
            tweaks::open_system_restore,
            tweaks::appx::list_apps,
            tweaks::appx::remove_apps,
            tweaks::appx::reinstall_app,
            tweaks::startup::list_startup,
            tweaks::startup::set_startup_enabled,
            actioninfo::action_command,
            actioninfo::action_save_script,
            measure::measure_read,
            measure::measure_text,
            measure::measure_record,
            planrun::plan_state_save,
            planrun::plan_state_load,
            planrun::plan_state_clear,
            planrun::plan_check,
            planrun::plan_reboot,
            target_user::get_target_user,
            diagnostics::run_diagnostics,
            diagnostics::list_snapshots,
            diagnostics::latest_findings,
            diagnostics::open_system_tool,
            diagnostics::report::generate_report,
            diagnostics::report::preview_report,
            diagnostics::report::open_report,
            diagnostics::report::reveal_report,
            diagnostics::report::email_report,
            diagnostics::report::email_report_manual,
            paths::get_app_info,
            domain::domain_status,
            domain::domain_check,
            domain::domain_join,
            domain::domain_leave,
            domain::domain_repair,
            domain::rename_computer,
            portals::list_portals,
            portals::save_portal,
            portals::delete_portal,
            portals::portal_show,
            portals::portal_bounds,
            portals::portal_hide_all,
            portals::portal_reset,
            portals::portal_insert_text,
            cases::case_current,
            cases::case_open,
            cases::case_update,
            cases::case_actions,
            cases::case_template_draft,
            cases::case_draft,
            cases::case_close,
            cases::case_discard,
            cases::cases_for_person,
            followups::list_followups,
            followups::add_followup,
            followups::set_followup_done,
            followups::snooze_followup,
            followups::delete_followup,
            quicknote::open_quick_note,
            quicknote::close_quick_note,
            quicknote::open_screen_clip,
            quicknote::redact_clipboard_image,
            mini::open_adminops_mini,
            rocket::rocket_boost,
            rocket::rocket_size,
            rocket::rocket_peek,
            rocket::rocket_memory,
            rocket::rocket_dock_state,
            rocketplus::rocket_deep_scan,
            rocketplus::rocket_deep_clean,
            rocketplus::rocket_restart_hung,
            rocketplus::rocket_prepare_call,
            rocketplus::rocket_startup_enabled,
            rocketplus::rocket_set_startup,
            lostwin::lost_windows,
            audiofix::audio_echo,
            defapps::defapps_apply_new_users,
            camfix::cam_view,
            camfix::cam_allow,
            camfix::cam_enable,
            rocket::rocket_bar,
            rocketbar::bar_fixes,
            rocketbar::bar_open_comm,
            rocketbar::bar_quick_assist,
            rocketbar::bar_screen_clip,
            faultrec::faultrec_attach_case,
            faultrec::faultrec_copy_file,
            faultrec::faultrec_open_file,
            lostwin::bring_windows,
            webnotif::web_notifications,
            webnotif::web_notifications_remove,
            rocket::rocket_set_mode,
            rocket::rocket_open_main,
            power::power_state,
            power::power_profiles,
            power::power_set,
            power::power_wake,
            audiofix::audio_view,
            audiofix::audio_listen,
            audiofix::audio_beep,
            audiofix::audio_use,
            audiofix::audio_allow_mic,
            wingetfix::winget_check,
            wingetfix::winget_repair,
            defapps::defapps_view,
            defapps::defapps_set_office,
            defapps::defapps_apply,
            defapps::defapps_remove_policy,
            defapps::defapps_open_settings,
            usbpolicy::usb_state,
            usbpolicy::usb_history,
            usbpolicy::usb_set,
            usbpolicy::usb_allow_for,
            mini::close_adminops_mini,
            portals::portal_nav,
            portals::portal_open_window,
            portals::portal_open_external,
            portals::portal_preload,
            portals::portal_close_idle,
            portals::portal_go,
            portals::portal_allow_domain,
            portals::portal_zoom,
            portals::portal_find,
            portals::portal_login_get,
            portals::portal_login_set,
            portals::portal_compose,
            portals::portal_teams,
            portals::portal_sign_out,
            portals::portal_download_open,
            portals::portal_download_reveal,
            paths::read_log,
            support::support_package,
            diagnostics::report::export_journal_pdf,
            support::report_problem,
            support::terms_of_use,
            paths::log_frontend_error,
            paths::open_logs_folder,
            paths::open_app_folder,
            task::cancel_task,
            drivers::backup_drivers,
            hardware::hardware_inventory,
            connected::connected_list,
            connected::connected_recent,
            connected::connected_action,
            displays::displays_list,
            displays::display_apply,
            displays::display_confirm,
            displays::display_undo,
            displays::display_restart_driver,
            netusage::netusage_start,
            netusage::netusage_snapshot,
            netusage::netusage_stop,
            netusage::netusage_icon,
            faultrec::faultrec_monitors,
            faultrec::faultrec_start,
            faultrec::faultrec_status,
            faultrec::faultrec_stop,
            faultrec::faultrec_list,
            faultrec::faultrec_get,
            faultrec::faultrec_delete,
            faultrec::faultrec_windows,
            faultrec::faultrec_pick_region,
            faultrec::faultrec_region_done,
            faultrec::faultrec_watch_start,
            faultrec::faultrec_watch_stop,
            faultrec::faultrec_watch_status,
            faultrec::faultrec_just_happened,
            faultrec::faultrec_auto_get,
            faultrec::faultrec_auto_set,
            faultrec::faultrec_auto_ignore,
            faultrec::faultrec_auto_unignore,
            faultrec::faultrec_thumb,
            faultrec::faultrec_to_report,
            faultrec::faultrec_report_remove,
            faultrec::faultrec_export,
            onedrive::onedrive_pause,
            scanfolder::scanfolder_status,
            scanfolder::scanfolder_setup,
            scanfolder::scanfolder_new_password,
            scanfolder::scanfolder_private_network,
            scanfolder::scanfolder_disable_signing,
            scanfolder::scanfolder_watch_start,
            scanfolder::scanfolder_watch_status,
            scanfolder::scanfolder_watch_stop,
            scanfolder::scanfolder_diagnose,
            scanfolder::scanfolder_open_folder,
            scanfolder::scanfolder_desktop_shortcut,
            kbdlang::kbd_state,
            kbdlang::kbd_only_spanish,
            unsaved::unsaved_list,
            unsaved::unsaved_preview,
            unsaved::unsaved_versions,
            unsaved::unsaved_pick,
            unsaved::unsaved_copy,
            hardware::memory_test_result,
            hardware::smart::smart_status,
            disks::disks_status,
            diskerrors::disk_error_history,
            problems::problems_list,
            nightjobs::night_jobs,
            nightjobs::night_job_add,
            nightjobs::night_job_remove,
            nightjobs::night_set_time,
            nightjobs::night_run_now,
            nightjobs::night_seen,
            disks::disk_check,
            disks::disk_repair,
            disks::disk_surface_scan,
            disks::disk_rescue,
            disks::disk_pick_folder,
            disks::bitlocker_local_key,
            disks::diskwatch_get,
            disks::diskwatch_set,
            smartio::smart_full,
            smartio::smart_selftest,
            smartio::smart_selftest_status,
            diskscanio::disk_scan,
            diskscanio::disk_scan_live,
            diskscanio::disk_scan_last,
            partitionsio::partition_layout,
            partitionsio::partition_backup,
            partitionsio::partition_backups,
            partitionsio::partition_table_restore,
            partitionsio::partition_find_lost,
            partitionsio::partition_restore,
            partitionsio::boot_repair,
            carveio::carve_scan,
            carveio::carve_found,
            carveio::carve_preview,
            carveio::carve_recover,
            cloneio::disk_clone,
            cloneio::disk_clone_live,
            cloneio::disk_clone_last,
            cases::case_tones,
            alertrulesio::alert_rules_get,
            alertrulesio::alert_rules_templates,
            alertrulesio::alert_rules_save,
            alertrulesio::alert_rules_reading,
            backupio::backups_get,
            backupio::backups_status,
            backupio::backups_defaults,
            backupio::backups_save,
            backupio::backups_check,
            backupio::backups_run,
            lifeio::life_report,
            partsio::parts_advice,
            uxio::media_in_use,
            uxio::describe_file,
            uxio::reveal_path,
            uxio::open_dropped_pdf,
            uxio::compose_mail,
            tweaks::preview::tweak_preview,
            errcodes::error_lookup,
            autoruns::autoruns_list,
            autoruns::autorun_disable,
            crashes::crash_report,
            crashes::driver_rollback,
            crashes::verifier_enable,
            crashes::verifier_disable,
            stress::stress_run,
            stress::stress_live,
            stress::stress_history,
            perfinsight::perf_insights,
            eventdigest::event_digest,
            learned::learned_record,
            learned::learned_stats,
            escalate::escalation_pack,
            vulndb::vulnerable_programs,
            datacrypt::data_crypt_status,
            datacrypt::data_crypt_enable,
            datacrypt::data_crypt_disable,
            datacrypt::data_crypt_unlock_recovery,
            outbound::outbound_list,
            errorlog::error_log_record,
            errorlog::error_log_list,
            errorlog::error_log_report,
            errorlog::error_log_clear,
            rescue::rescue_status,
            rescue::rescue_create,
            uxio::save_text_file,
            uxio::open_text_file,
            errcodes::error_codes,
            fixes::maildns::mail_domain_check,
            labelsio::label_qr,
            labelsio::label_sheet,
            disktools::disk_speed_test,
            disktools::disk_capacity_test,
            disktools::disk_eject,
            disktools::disk_format,
            appbackup::autobackup_info,
            appbackup::autobackup_set,
            appbackup::autobackup_run_now,
            storage::storage_info,
            storage::storage_make_portable,
            storage::storage_set_browser_on_usb,
            company::company_export,
            company::company_import_preview,
            company::company_import_apply,
            hardware::sensors::read_sensors,
            hardware::battery::battery_history,
            hardware::sensors::install_pawnio,
            hardware::sensors::open_third_party_notices,
            workflow::get_settings,
            workflow::save_settings,
            workflow::list_clients,
            workflow::visit_changes,
            workflow::compare_client_machines,
            agenda::list_agenda,
            agenda::save_visit,
            agenda::set_visit_status,
            agenda::delete_visit,
            agenda::postpone_visit,
            workflow::save_client,
            workflow::delete_client,
            workflow::get_session,
            workflow::start_session,
            workflow::update_session,
            workflow::cancel_session,
            workflow::finish_session,
            workflow::set_next_maintenance,
            space::scan_space,
            space::cancel_space_scan,
            space::space_freeable,
            space::space_recycle,
            space::space_folder,
            space::space_files,
            space::space_kind_files,
            space::space_duplicates,
            space::reveal_in_explorer,
            software::list_software_updates,
            software::upgrade_software,
            software::ignored_updates,
            software::cached_software_updates,
            software::set_update_ignored,
            apps::app_catalog,
            apps::set_catalog_view,
            apps::installed_apps,
            apps::search_apps,
            apps::install_preflight,
            apps::install_apps,
            installq::install_enqueue,
            installq::install_snapshot,
            installq::install_clear,
            installq::install_skip,
            installq::install_retry,
            installq::install_blockers,
            installq::install_close_and_retry,
            apps::save_app_list,
            apps::delete_app_list,
            migrate::migrate_profiles,
            migrate::migrate_estimate,
            migrate::migrate_pick_folder,
            migrate::migrate_backup,
            migrate::migrate_read_backup,
            migrate::migrate_restore,
            printers::list_printers,
            printers::check_printer,
            printers::find_network_printers,
            printers::clear_printer_queue,
            printers::print_test_page,
            printers::set_default_printer,
            printers::remove_printer,
            printvendor::printer_inventory,
            printvendor::scanner_inventory,
            printvendor::driver_library,
            printvendor::driver_library_add,
            printvendor::set_driver_library_dir,
            printvendor::driver_candidates,
            printvendor::printer_driver_update,
            printvendor::printer_driver_undo,
            printvendor::open_vendor_page,
            printvendor::watch_downloads,
            printvendor::scanner_test,
            printvendor::printer_install,
            printvendor::run_vendor_installer,
            maintenance::boot_analysis,
            maintenance::restore_storage,
            maintenance::delete_old_restore_points,
            maintenance::list_driver_backups,
            maintenance::restore_drivers,
            maintenance::maintenance_schedule,
            maintenance::set_maintenance_schedule,
            keys::try_shortcut,
            security::security_audit,
            security::bitlocker_status,
            security::bitlocker_keys,
            security::bitlocker_export,
            security::suspicious_items,
            security::disable_task,
            security::browser_extensions,
            winupdate::update_history,
            winupdate::update_pause_state,
            winupdate::update_pause,
            winupdate::pending_updates,
            winupdate::set_update_hidden,
            winupdate::update_install,
            winupdate::update_install_running,
            wucause::update_cause,
            programs::list_programs,
            programs::uninstall_program,
            programs::remove_leftovers,
            programs::remove_orphan_entry,
            programs::scan_leftovers,
            programs::repair_program,
            processes::list_processes,
            processes::kill_process,
            processes::open_process_location,
            network::diag::network_diagnostics,
            network::speedtest::run_speedtest,
            network::speedtest::cancel_speedtest,
            network::speedtest::list_speedtests,
            network::wifi::list_wifi_profiles,
            network::wifi::forget_wifi_profile,
            network::wifictl::wifi_state,
            network::wifictl::set_wifi_radio,
            network::wifictl::restart_wifi_adapter,
            network::wifictl::remove_ghost_wifi,
            network::wifictl::wifi_power_saving_off,
            network::lan::lan_info,
            network::lan::public_ip,
            network::lan::router_check,
            network::lan::list_routers,
            network::lan::save_router,
            network::lan::delete_router,
            network::lan::wifi_qr,
            network::lan::scan_lan,
            network::lan::set_device_alias,
            network::lan::identify_lan,
            network::lan::open_device_page,
            network::lan::lookup_vendors,
            vault::vault_support,
            vault::vault_list,
            vault::vault_create,
            vault::vault_open,
            vault::vault_close,
            vault::vault_change_password,
            vault::vault_recovery_key,
            vault::vault_save_recovery,
            vault::vault_add_existing,
            vault::vault_pick_location,
            vault::vault_remove,
            vault::pick_folder,
            vault::pick_encrypted_zip,
            vault::encrypt_folder,
            vault::decrypt_archive,
            wipe::wipe_pick,
            wipe::wipe_items,
            wipe::wipe_free_space,
            wipe::open_ms_settings,
            recover::recover_status,
            recover::recover_install,
            recover::recover_run,
            family::dns_filter_status,
            family::set_dns_filter,
            family::blocked_sites,
            family::set_blocked_sites,
            family::logon_hours,
            family::set_logon_hours,
            office::wake_on_lan,
            office::remote_status,
            office::enable_wake_on_lan,
            office::set_remote_desktop,
            office::open_remote_desktop,
            office::open_remote_assistance,
            office::list_shares,
            office::create_share,
            office::remove_share,
            office::enable_file_sharing,
            office::ip_conflicts,
            shares::share_grant,
            shares::share_revoke,
            shares::share_explain,
            shares::network_drives,
            shares::map_network_drive,
            shares::unmap_network_drive,
            shares::reconnect_network_drive,
            shares::open_network_path,
            shares::remote_shares,
            shares::share_sizes,
            shares::share_backups,
            shares::set_share_backup,
            shares::remove_share_backup,
            shares::run_share_backup,
            office::export_csv,
            applock::lock_status,
            applock::lock_verify,
            applock::lock_set,
            applock::lock_set_idle,
            applock::lock_disable,
            applock::lock_verify_windows,
            window_state::set_ui_zoom,
            window_state::current_monitor_name,
            window_state::ui_ready,
            window_state::ui_booting,
            window_state::quit_app,
            minimon::open_mini_monitor,
            tray::tray_run_fix,
            tray::tray_fixes_available,
            portable_exit::portable_leftovers,
            portable_exit::portable_cleanup,
            appcare::autostart_enabled,
            appcare::set_autostart,
            appcare::data_usage,
            appcare::data_cleanup,
            appcare::check_update,
            appcare::install_update,
            appcare::open_release_page,
            winwatch::list_windows_alerts,
            troubleshoot::troubleshoot_check,
            troubleshoot::troubleshoot_fix,
            troubleshoot::repair_network,
            troubleshoot::quick_net_check,
            context::machine_context,
            context::internet_probe,
            perfhistory::perf_history,
            bootlog::boot_history,
            diagnostics::diag_accepted,
            diagnostics::diag_accept,
            diagnostics::diag_unaccept,
            comms::comm_apps,
            comms::open_comm,
            network::watch::netwatch_start,
            network::watch::netwatch_stop,
            network::watch::netwatch_status,
            network::watch::netwatch_clear,
            timeline::machine_timeline,
            contacts::list_contacts,
            contacts::save_contact,
            contacts::touch_contact,
            contacts::bulk_contacts,
            contacts::merge_contacts,
            contacts::contact_tag_colors,
            contacts::set_contact_tag_color,
            contacts::rename_contact_tag,
            contacts::delete_contact_tag,
            contacts::list_contact_backups,
            contacts::backup_contacts_now,
            contacts::restore_contact_backup,
            contacts::open_teams,
            contacts::call_number,
            contacts::save_vcard,
            contacts::import_contacts,
            contacts::write_email,
            winwatch::mark_windows_alerts_read,
            winwatch::clear_windows_alerts,
            winwatch::dismiss_windows_alert,
            winwatch::fix_windows_alert,
            winwatch::mute_windows_alert,
            winwatch::unmute_windows_alert,
            winwatch::muted_windows_alerts,
            winwatch::check_windows_now,
            winwatch::unread_windows_alerts,
            remote::list_connections,
            remote::save_connection,
            remote::delete_connection,
            remote::set_connection_password,
            remote::connect_rdp,
            remote::connect_saved,
            remote::test_connection,
            remote::remote_tools,
            remote::connect_tool_id,
            remote::open_remote_tool,
            remote::install_remote_tool,
            remote::rdp_server,
            remote::set_rdp_user,
            workflow::export_config,
            workflow::import_config,
            workflow::inventory_add_this,
            workflow::inventory_remove,
            workflow::save_network_map,
            portals::router_portal,
            portals::portal_hide,
            network::tools::start_ping,
            network::tools::start_trace,
            network::tools::stop_probe,
            network::tools::list_ports,
            network::tools::list_dns_adapters,
            network::tools::set_dns,
            network::tools::read_hosts,
            network::tools::save_hosts,
            toolbox::list_tools,
            toolbox::launch_tool,
            toolbox::set_tool_favorite,
            toolbox::save_custom_tool,
            toolbox::delete_custom_tool,
            toolbox::pick_tool_target,
            users::list_users,
            users::create_user,
            users::set_user_password,
            users::set_user_enabled,
            users::set_user_admin,
            users::user_profile_size,
            users::delete_user,
            users::rename_user,
            people::search_people,
            people::person_details,
            people::unlock_account,
            people::reset_domain_password,
            people::laps_password,
            people::bitlocker_recovery,
            tweaks::profiles::list_profiles,
            tweaks::profiles::apply_profile,
            tweaks::profiles::revert_profile,
            tweaks::profiles::save_custom_profile,
            tweaks::profiles::delete_custom_profile,
            tweaks::profiles::export_custom_profiles,
            tweaks::profiles::import_custom_profiles,
            library::library_list,
            library::library_save,
            library::library_delete,
            library::library_touch,
            library::this_place,
            officemap::office_map,
            officemap::save_device_meta,
            officemap::refresh_device_ips,
            officemap::watch_status,
            stations::check_stations,
            stations::check_stations_stream,
            stations::review_station,
            whatchanged::whatchanged_info,
            whatchanged::whatchanged_snapshot,
            whatchanged::whatchanged_since,
            askpc::ask_pc,
            stations::station_action,
            sheet::machine_sheet,
            sheet::open_warranty,
            appbackup::backup_app_data,
            appbackup::restore_app_data,
            appbackup::storage_health,
            audit::audit_mode,
            audit::set_audit_mode,
            accounts::accounts_status,
            accounts::leave_azure_ad,
            accounts::remove_work_account,
            accounts::office_sign_out,
            accounts::delete_credential,
            accounts::sign_out_windows,
            waitchain::why_not_responding,
            filelocks::file_holders,
            filelocks::holder_drives,
            filelocks::holder_pick,
            filelocks::close_holder,
            filelocks::close_smb_file,
            errlens::error_lens_text,
            errlens::error_lens_image,
            errlens::error_lens_file,
            errlens::error_lens_clipboard,
            testuser::test_user_status,
            testuser::create_test_user,
            testuser::delete_test_user,
            culprit::bisect_note,
            appcare::previous_version,
            appcare::rollback_previous,
            safestart::safe_start_info,
            safestart::session_page,
            safestart::safe_start_leave,
            safestart::safe_start_report,
        ]))
        .build(context)
        .expect("error while building tauri application")
        .run(|app, event| {
            // Salir por cualquier camino (cerrar, reiniciar con administrador…): si había un cambio de pantalla sin confirmar, se deshace.
            if let tauri::RunEvent::Exit = event {
                // Salió bien: la próxima vez arranca normal (ver safestart.rs). Lo primero: si Windows
                // o un instalador matan el proceso mientras se cierra lo demás, ya no cuenta como fallo.
                safestart::end_clean();
                displays::restore_pending_on_exit(app);
                netusage::shutdown();
                scanfolder::shutdown();
                faultrec::shutdown(app);
            }
        });
}
