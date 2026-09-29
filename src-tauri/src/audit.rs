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
    "wipe_free_space",
    "recover_install",
    "recover_run",
];

pub fn active() -> bool {
    ON.load(Ordering::Relaxed)
}

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
}
