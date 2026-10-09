mod agenda;
mod bootlog;
mod comms;
mod context;
mod perfhistory;
mod apps;
mod boottime;
mod diagnostics;
mod domain;
mod drivers;
mod elevation;
mod hardware;
mod keys;
mod maintenance;
mod metrics;
mod migrate;
mod minimon;
mod network;
mod cases;
mod followups;
mod snmp;
mod storage;
mod company;
mod webview_orphans;
mod disks;
mod disktools;
mod diskscan;
mod diskscanio;
mod diskwatch;
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
mod ocr;
mod discovery;
mod quicknote;
mod paths;
mod people;
mod portals;
mod printers;
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
mod winupdate;
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
mod remote;
mod accounts;
mod appbackup;
mod audit;
mod contacts;
mod library;
mod officemap;
mod sheet;
mod shellopen;
mod stations;
mod secrets;
mod smoke;
mod timeline;
mod troubleshoot;

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
    let orphans = boottime::step("Navegador de sesiones anteriores", webview_orphans::kill_orphans);
    if orphans > 0 {
        std::env::set_var("ADMINOPS_ORPHANS", orphans.to_string());
    }
    if let Some(dir) = boottime::step("Carpeta de datos de WebView2", || paths::portable_webview_dir().or_else(paths::installed_webview_dir)) {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir);
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
            if window.label() != "main" {
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
                }
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
            // Historial de rendimiento de 7 días: una muestra por minuto mientras AdminOps está abierta.
            perfhistory::start(app.handle().clone());
            appbackup::start(app.handle().clone());
            // Nota de llamada con Ctrl+Alt+N, aunque AdminOps esté minimizado.
            quicknote::start(app.handle().clone());
            if std::env::args().any(|a| a == "--auditoria") {
                audit::set(true);
            }
            // Limpieza de datos antiguos, si está activada (en segundo plano).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
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
            hardware::memory_test_result,
            hardware::smart::smart_status,
            disks::disks_status,
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
            window_state::ui_ready,
            window_state::ui_booting,
            window_state::quit_app,
            minimon::open_mini_monitor,
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
        ]))
        .run(context)
        .expect("error while running tauri application");
}
