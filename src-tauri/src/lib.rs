mod elevation;
mod metrics;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(metrics::MetricsState::new())
        .invoke_handler(tauri::generate_handler![
            elevation::is_admin,
            elevation::relaunch_as_admin,
            metrics::get_system_info,
            metrics::get_live_metrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
