//! Modo auditoría: AdminOps solo mira. Se bloquea en un único punto (el
//! despachador de comandos) todo lo que cambia el equipo, así ningún botón se
//! escapa aunque la interfaz no lo desactive. Lo propio de AdminOps (contactos,
//! notas, informes, diagnósticos) sigue funcionando.
//!
//! Dura mientras AdminOps esté abierta: al volver a abrirla empieza desactivado,
//! salvo que se arranque con `--auditoria`.

use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

pub const MESSAGE: &str = "Modo auditoría activo: AdminOps no hace cambios en este equipo. Desactívalo en la barra superior para continuar.";

/// Comandos que cambian el equipo (sistema, programas, usuarios, red, archivos).
const BLOCKED: &[&str] = &[
    // Ajustes, reparaciones y perfiles
    "apply_tweak",
    "revert_tweak",
    "revert_entry",
    "run_action",
    "fix_finding",
    "apply_profile",
    "revert_profile",
    "create_restore_point",
    "delete_old_restore_points",
    "set_maintenance_schedule",
    "troubleshoot_fix",
    "repair_network",
    // Discos: reparar el sistema de archivos, chkdsk /r y copiar archivos a otra carpeta
    "disk_repair",
    "disk_surface_scan",
    "disk_rescue",
    "disk_capacity_test",
    "disk_format",
    "disk_eject",
    "smart_selftest",
    "partition_table_restore",
    "partition_restore",
    "boot_repair",
    "carve_recover",
    "disk_clone",
    // Programas y Windows
    "remove_apps",
    "reinstall_app",
    "set_startup_enabled",
    "upgrade_software",
    "install_apps",
    "uninstall_program",
    "remove_leftovers",
    "remove_orphan_entry",
    "repair_program",
    "install_pawnio",
    "restore_drivers",
    "update_pause",
    "set_update_hidden",
    "disable_task",
    "kill_process",
    "set_autostart",
    "migrate_restore",
    // Dominio, usuarios y seguridad
    "domain_join",
    "domain_leave",
    "domain_repair",
    "rename_computer",
    "create_user",
    "set_user_password",
    "set_user_enabled",
    "set_user_admin",
    "delete_user",
    "rename_user",
    "unlock_account",
    "reset_domain_password",
    "set_dns_filter",
    "set_blocked_sites",
    "set_logon_hours",
    "leave_azure_ad",
    "remove_work_account",
    "office_sign_out",
    "delete_credential",
    "sign_out_windows",
    // Red y Wi-Fi
    "set_dns",
    "save_hosts",
    "forget_wifi_profile",
    "set_wifi_radio",
    "restart_wifi_adapter",
    "remove_ghost_wifi",
    "wifi_power_saving_off",
    "enable_wake_on_lan",
    "set_remote_desktop",
    "set_rdp_user",
    "set_connection_password",
    "install_remote_tool",
    "station_action",
    "create_share",
    "remove_share",
    "enable_file_sharing",
    // Descarga la versión nueva y abre su instalador
    "install_update",
    "share_grant",
    "share_revoke",
    "map_network_drive",
    "unmap_network_drive",
    "reconnect_network_drive",
    "set_share_backup",
    "remove_share_backup",
    "run_share_backup",
    // Impresoras
    "clear_printer_queue",
    "set_default_printer",
    "remove_printer",
    // Archivos del equipo
    "vault_create",
    "vault_open",
    "vault_close",
    "vault_change_password",
    "vault_add_existing",
    "vault_remove",
    "encrypt_folder",
    "decrypt_archive",
    "wipe_items",
    "space_recycle",
    "wipe_free_space",
    "recover_install",
    "recover_run",
];

/// Comandos revisados uno a uno que **no cambian el equipo del cliente**: leen,
/// abren una herramienta de Windows, o solo tocan los datos del propio AdminOps
/// (clientes, contactos, portales, ajustes de la aplicación).
///
/// No es decoración: junto a `BLOCKED` forma la clasificación completa, y la
/// prueba `every_command_is_classified` falla en cuanto se registra un comando
/// nuevo que no esté en ninguna de las dos listas. Así el modo auditoría no se
/// puede escapar por olvido al añadir una función.
///
/// Al añadir un comando, la pregunta es una sola: **¿deja el equipo del cliente
/// distinto a como estaba?** Si la respuesta es sí (o «depende»), va a `BLOCKED`.
/// (Solo se compila en las pruebas: su trabajo es hacerlas fallar, no viajar en el programa.)
#[cfg(test)]
const SAFE: &[&str] = &[
    // accounts
    "accounts_status",
    // agenda
    "list_agenda", "save_visit", "set_visit_status", "delete_visit", "postpone_visit",
    // appbackup
    "backup_app_data", "restore_app_data", "storage_health",
    // appcare
    "autostart_enabled", "data_usage", "data_cleanup", "check_update", "open_release_page",
    // applock
    "lock_status", "lock_verify", "lock_set", "lock_set_idle", "lock_disable", "lock_verify_windows",
    // apps
    "app_catalog", "set_catalog_view", "installed_apps", "search_apps", "install_preflight", "save_app_list", "delete_app_list",
    // audit
    "audit_mode", "set_audit_mode",
    // boottime
    "startup_timing", "log_timing",
    // contacts
    "list_contacts", "save_contact", "touch_contact", "bulk_contacts", "merge_contacts", "contact_tag_colors", "set_contact_tag_color",
    "rename_contact_tag", "delete_contact_tag", "list_contact_backups", "backup_contacts_now", "restore_contact_backup", "open_teams", "call_number",
    "save_vcard", "import_contacts", "write_email",
    // diagnostics
    "run_diagnostics", "list_snapshots", "latest_findings", "open_system_tool",
    // diagnostics::report
    "generate_report", "preview_report", "open_report", "reveal_report", "email_report", "email_report_manual",
    // domain
    "domain_status", "domain_check",
    // drivers
    "backup_drivers",
    // elevation
    "is_admin", "relaunch_as_admin",
    // family
    "dns_filter_status", "blocked_sites", "logon_hours",
    // hardware
    "hardware_inventory", "memory_test_result",
    // hardware::sensors
    "read_sensors", "battery_history", "open_third_party_notices",
    // hardware::smart
    "smart_status", "disks_status", "disk_check", "disk_pick_folder", "bitlocker_local_key", "disk_speed_test", "autobackup_info", "autobackup_set", "autobackup_run_now", "storage_info", "storage_make_portable", "storage_set_browser_on_usb", "company_export", "company_import_preview", "company_import_apply",
    // keys
    "try_shortcut",
    // library
    "library_list", "library_save", "library_delete", "library_touch", "this_place",
    // maintenance
    "boot_analysis", "restore_storage", "list_driver_backups", "maintenance_schedule",
    // metrics
    "get_system_info", "get_live_metrics",
    // migrate
    "migrate_profiles", "migrate_estimate", "migrate_pick_folder", "migrate_backup", "migrate_read_backup",
    // network::diag
    "network_diagnostics",
    // network::lan
    "lan_info", "public_ip", "router_check", "list_routers", "save_router", "delete_router", "wifi_qr", "scan_lan", "set_device_alias",
    "identify_lan", "open_device_page", "lookup_vendors",
    // network::speedtest
    "run_speedtest", "cancel_speedtest", "list_speedtests",
    // network::tools
    "start_ping", "start_trace", "stop_probe", "list_ports", "list_dns_adapters", "read_hosts",
    // network::wifi
    "list_wifi_profiles",
    // network::wifictl
    "wifi_state",
    // office
    "wake_on_lan", "remote_status", "open_remote_desktop", "open_remote_assistance", "list_shares", "ip_conflicts", "export_csv",
    // shares (consultas; abrir una ruta de red en el Explorador no cambia nada)
    "share_explain", "network_drives", "open_network_path", "remote_shares", "share_sizes", "share_backups",
    // officemap
    "office_map", "save_device_meta", "refresh_device_ips", "watch_status",
    // followups y nota de llamada (datos y ventanas del propio técnico)
    "list_followups", "add_followup", "set_followup_done", "snooze_followup", "delete_followup",
    "open_quick_note", "close_quick_note", "open_screen_clip", "redact_clipboard_image",
    // cases
    "case_current", "case_open", "case_update", "case_actions", "case_draft", "case_close", "case_discard", "cases_for_person",
    // people
    "search_people", "person_details", "laps_password", "bitlocker_recovery",
    // paths
    "get_app_info", "read_log", "log_frontend_error", "open_logs_folder", "open_app_folder",
    // portals
    "list_portals", "save_portal", "delete_portal", "portal_show", "portal_bounds", "portal_hide_all", "portal_reset", "portal_insert_text", "portal_nav", "portal_open_window",
    "portal_open_external", "portal_preload", "portal_close_idle", "portal_go", "portal_allow_domain", "portal_zoom", "portal_find", "portal_login_get", "portal_login_set",
    "portal_compose", "portal_teams", "portal_sign_out", "portal_download_open", "portal_download_reveal", "router_portal", "portal_hide",
    // printers
    "list_printers", "print_test_page", "check_printer", "find_network_printers",
    // processes
    "list_processes", "open_process_location",
    // programs
    "list_programs", "scan_leftovers",
    // recover
    "recover_status",
    // remote
    "list_connections", "save_connection", "delete_connection", "connect_rdp", "connect_saved", "test_connection", "remote_tools", "connect_tool_id",
    "open_remote_tool", "rdp_server",
    // security
    "security_audit", "bitlocker_status", "bitlocker_keys", "bitlocker_export", "suspicious_items", "browser_extensions",
    // sheet
    "machine_sheet", "open_warranty",
    // software
    "list_software_updates", "ignored_updates", "cached_software_updates", "set_update_ignored",
    // space
    "scan_space", "cancel_space_scan", "space_freeable", "space_folder", "space_files", "space_kind_files", "space_duplicates", "reveal_in_explorer",
    // stations
    "check_stations",
    // support
    "support_package",
    // Prepara un correo (lo envía el técnico) y enseña los términos: no cambian el equipo.
    "report_problem",
    // Guarda un PDF donde elija el técnico: no cambia el equipo.
    "export_journal_pdf",
    "terms_of_use",
    // target_user
    "get_target_user",
    // task
    "cancel_task",
    // timeline
    "machine_timeline",
    // toolbox
    "list_tools", "launch_tool", "set_tool_favorite", "save_custom_tool", "delete_custom_tool", "pick_tool_target",
    // troubleshoot
    "troubleshoot_check", "quick_net_check",
    // context (barra de arriba)
    "machine_context", "internet_probe",
    // historial de rendimiento, arranques, abrir Teams/Correo, vigilante de la conexión
    "perf_history", "boot_history", "comm_apps", "open_comm",
    // «Ya lo sé» del diagnóstico: una nota del técnico, no cambia el equipo
    "diag_accepted", "diag_accept", "diag_unaccept",
    "netwatch_start", "netwatch_stop", "netwatch_status", "netwatch_clear",
    // tweaks
    "list_tweaks", "tweak_index", "get_journal", "list_restore_points", "open_system_restore",
    // tweaks::appx
    "list_apps",
    // tweaks::profiles
    "list_profiles", "save_custom_profile", "delete_custom_profile", "export_custom_profiles", "import_custom_profiles",
    // tweaks::startup
    "list_startup",
    // users
    "list_users", "user_profile_size",
    // vault
    "vault_support", "vault_list", "vault_recovery_key", "vault_save_recovery", "vault_pick_location", "pick_folder", "pick_encrypted_zip",
    // window_state
    "set_ui_zoom", "ui_ready", "ui_booting", "quit_app", "open_mini_monitor",
    // winupdate
    "update_history", "update_pause_state", "pending_updates",
    // winwatch
    "list_windows_alerts", "mark_windows_alerts_read", "clear_windows_alerts", "check_windows_now", "unread_windows_alerts",
    "dismiss_windows_alert", "mute_windows_alert", "unmute_windows_alert", "muted_windows_alerts",
    // wipe
    "wipe_pick", "open_ms_settings",
    // discos a fondo: solo leen (o guardan copias y ajustes propios de AdminOps)
    "diskwatch_get", "diskwatch_set", "smart_full", "smart_selftest_status", "disk_scan", "disk_scan_live", "disk_scan_last",
    "partition_layout", "partition_backup", "partition_backups", "partition_find_lost",
    "carve_scan", "carve_found", "carve_preview", "disk_clone_live", "disk_clone_last",
    // workflow
    "get_settings", "save_settings", "list_clients", "visit_changes", "compare_client_machines", "save_client", "delete_client", "get_session",
    "start_session", "update_session", "cancel_session", "finish_session", "set_next_maintenance", "export_config", "import_config",
    "inventory_add_this", "inventory_remove", "save_network_map",
];

pub fn active() -> bool {{
    ON.load(Ordering::Relaxed)
}}

/// ¿Hay que rechazar este comando ahora?
pub fn blocks(command: &str) -> bool {
    active() && BLOCKED.contains(&command)
}

pub fn set(on: bool) {
    ON.store(on, Ordering::Relaxed);
    log::info!("Modo auditoría: {}", if on { "activado" } else { "desactivado" });
}

/// Envuelve el despachador de comandos: con el modo activo, rechaza los que cambian el equipo.
pub fn guard<R: tauri::Runtime, F>(handler: F) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
where
    F: Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
{
    move |invoke| {
        if blocks(invoke.message.command()) {
            log::info!("Modo auditoría: bloqueado {}", invoke.message.command());
            invoke.resolver.reject(MESSAGE);
            return true;
        }
        handler(invoke)
    }
}

#[tauri::command]
pub fn audit_mode() -> bool {
    active()
}

#[tauri::command]
pub fn set_audit_mode(on: bool) {
    set(on);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_only_system_changes_when_active() {
        set(false);
        assert!(!blocks("apply_tweak"));
        set(true);
        assert!(blocks("apply_tweak"));
        assert!(blocks("domain_join"));
        assert!(!blocks("run_diagnostics"));
        assert!(!blocks("save_contact"));
        assert!(!blocks("set_audit_mode"));
        set(false);
    }

    /// Todo comando bloqueado existe de verdad (una errata lo dejaría sin bloquear).
    #[test]
    fn every_blocked_command_is_registered() {
        let lib = include_str!("lib.rs");
        for c in BLOCKED {
            assert!(lib.contains(&format!("::{c},")), "«{c}» no está registrado en lib.rs");
        }
    }

    /// Nombres de los comandos registrados en `lib.rs`, tal y como los recibe el
    /// despachador (el último tramo tras `::`).
    fn registered() -> Vec<String> {
        let lib = include_str!("lib.rs");
        let block = lib.split("generate_handler![").nth(1).expect("lib.rs registra los comandos");
        let block = block.split("])").next().expect("la lista de comandos se cierra");
        block
            .lines()
            .map(|l| l.trim().trim_end_matches(',').trim())
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
            .map(|l| l.rsplit("::").next().unwrap_or(l).to_string())
            .collect()
    }

    /// **La prueba que sostiene el modo auditoría.** Cada comando registrado está
    /// clasificado: o cambia el equipo (`BLOCKED`) o no (`SAFE`). Si esta prueba
    /// falla es porque se añadió un comando y nadie decidió en cuál va; decidirlo
    /// es parte de añadirlo, no un detalle que se pueda dejar para después.
    #[test]
    fn every_command_is_classified() {
        let sin_clasificar: Vec<String> = registered().into_iter().filter(|c| !BLOCKED.contains(&c.as_str()) && !SAFE.contains(&c.as_str())).collect();
        assert!(
            sin_clasificar.is_empty(),
            "Comandos sin clasificar en audit.rs: {sin_clasificar:?}.
¿Cambian el equipo del cliente? Sí → BLOCKED · No → SAFE."
        );
    }

    /// Ningún comando en las dos listas a la vez, y ninguno que ya no exista.
    #[test]
    fn the_two_lists_are_consistent() {
        let reg = registered();
        for c in SAFE {
            assert!(!BLOCKED.contains(c), "«{c}» está en SAFE y en BLOCKED a la vez");
            assert!(reg.iter().any(|r| r == c), "«{c}» está en SAFE pero ya no se registra en lib.rs");
        }
        assert_eq!(reg.len(), BLOCKED.len() + SAFE.len(), "las dos listas deben cubrir exactamente los comandos registrados");
    }
}
