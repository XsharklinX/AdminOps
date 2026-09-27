mod apps;
mod diagnostics;
mod drivers;
mod elevation;
mod hardware;
mod metrics;
mod migrate;
mod network;
mod paths;
mod printers;
mod processes;
mod software;
mod space;
mod bench;
mod ps;
mod pspool;
mod task;
mod target_user;
mod toolbox;
mod tweaks;
mod users;
mod workflow;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Modo prueba: `adminops.exe --roundtrip informe.json [--only id1,id2]`.
    // No abre ventana; ver tweaks/roundtrip.rs.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--roundtrip") {
        let out = args.get(i + 1).map_or("roundtrip.json", String::as_str);
        let only = args.iter().position(|a| a == "--only").and_then(|j| args.get(j + 1)).map(String::as_str);
        std::process::exit(tweaks::roundtrip::run(out, only));
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            elevation::is_admin,
            elevation::relaunch_as_admin,
            metrics::get_system_info,
            metrics::get_live_metrics,
            tweaks::list_tweaks,
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
            diagnostics::open_system_tool,
            diagnostics::report::generate_report,
            diagnostics::report::open_report,
            diagnostics::report::reveal_report,
            paths::get_app_info,
            paths::read_log,
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
            processes::list_processes,
            processes::kill_process,
            processes::open_process_location,
            network::diag::network_diagnostics,
            network::speedtest::run_speedtest,
            network::speedtest::cancel_speedtest,
            network::speedtest::list_speedtests,
            network::wifi::list_wifi_profiles,
            network::wifi::forget_wifi_profile,
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
