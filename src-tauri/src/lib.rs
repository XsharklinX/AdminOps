mod elevation;
mod metrics;
mod ps;
mod target_user;
mod tweaks;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(metrics::MetricsState::new())
        .setup(|app| {
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
