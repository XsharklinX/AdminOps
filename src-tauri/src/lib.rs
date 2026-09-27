mod apps;
mod diagnostics;
mod domain;
mod drivers;
mod elevation;
mod hardware;
mod keys;
mod maintenance;
mod metrics;
mod migrate;
mod network;
mod paths;
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
mod applock;
mod appcare;
mod winwatch;
mod remote;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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

    // Portable: la caché de WebView2 al USB (WebView2 respeta esta variable).
    if let Some(dir) = paths::portable_webview_dir() {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir);
    }

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
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    window_state::save(window);
                }
            }
        })
        .manage(metrics::MetricsState::new())
        .manage(processes::ProcessState::new())
        .setup(|app| {
            pspool::warm_up();
            log::info!(
                "AdminOps {} iniciado · admin={} · portable={}",
                app.package_info().version,
                elevation::is_elevated(),
                paths::is_portable()
            );
            app.manage(tweaks::TweakState::new(app.handle()));
            window_state::restore(app.handle());
            let settings = workflow::settings(app.handle());
            tweaks::set_restore_point_policy(&settings.restore_points);
            // Arranque con Windows: minimizada en la barra de tareas.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            // Vigilancia de errores de Windows (Ajustes → General).
            winwatch::start(app.handle().clone());
            // Limpieza de datos antiguos, si está activada (en segundo plano).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                use tauri::Manager;
                let state = handle.state::<tweaks::TweakState>();
                appcare::auto_cleanup(&handle, &state);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            elevation::is_admin,
            elevation::relaunch_as_admin,
            metrics::get_system_info,
            metrics::get_live_metrics,
            tweaks::list_tweaks,
            tweaks::tweak_index,
            tweaks::apply_tweak,
            tweaks::revert_tweak,
            tweaks::revert_entry,
            tweaks::run_action,
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
            portals::portal_nav,
            portals::portal_open_window,
            portals::portal_open_external,
            paths::read_log,
            support::support_package,
            paths::log_frontend_error,
            paths::open_logs_folder,
            paths::open_app_folder,
            task::cancel_task,
            drivers::backup_drivers,
            hardware::hardware_inventory,
            hardware::memory_test_result,
            hardware::smart::smart_status,
            hardware::sensors::read_sensors,
            hardware::sensors::install_pawnio,
            hardware::sensors::open_third_party_notices,
            workflow::get_settings,
            workflow::save_settings,
            workflow::list_clients,
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
            space::space_children,
            space::reveal_in_explorer,
            software::list_software_updates,
            software::upgrade_software,
            apps::app_catalog,
            apps::installed_apps,
            apps::search_apps,
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
            processes::list_processes,
            processes::kill_process,
            processes::open_process_location,
            network::diag::network_diagnostics,
            network::speedtest::run_speedtest,
            network::speedtest::cancel_speedtest,
            network::speedtest::list_speedtests,
            network::wifi::list_wifi_profiles,
            network::wifi::forget_wifi_profile,
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
            office::open_quick_assist,
            office::list_shares,
            office::create_share,
            office::remove_share,
            office::enable_file_sharing,
            office::ip_conflicts,
            office::export_csv,
            applock::lock_status,
            applock::lock_verify,
            applock::lock_set,
            applock::lock_set_idle,
            applock::lock_disable,
            applock::lock_verify_windows,
            window_state::set_ui_zoom,
            appcare::autostart_enabled,
            appcare::set_autostart,
            appcare::data_usage,
            appcare::data_cleanup,
            appcare::check_update,
            appcare::open_release_page,
            winwatch::list_windows_alerts,
            winwatch::mark_windows_alerts_read,
            winwatch::clear_windows_alerts,
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
            tweaks::profiles::list_profiles,
            tweaks::profiles::apply_profile,
            tweaks::profiles::revert_profile,
            tweaks::profiles::save_custom_profile,
            tweaks::profiles::delete_custom_profile,
            tweaks::profiles::export_custom_profiles,
            tweaks::profiles::import_custom_profiles,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
