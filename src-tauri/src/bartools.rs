//! Las herramientas opcionales de la barra del cohete (apagadas de fábrica, se encienden en «Personalizar»):
//! pantallas, Wi-Fi y Bluetooth, mis carpetas, bloquear y reiniciar sin miedo. Contactos, la impresora y el
//! teclado usan los comandos que ya tiene AdminOps. Todo funciona sin administrador: es lo que verá el usuario.

use std::sync::atomic::{AtomicU64, Ordering};

/// Lo que hace Win+P: duplicar, extender, solo esta pantalla o solo la otra.
#[tauri::command(async)]
pub fn bar_display_mode(mode: String) -> Result<(), String> {
    let arg = match mode.as_str() {
        "clone" => "/clone",
        "extend" => "/extend",
        "internal" => "/internal",
        "external" => "/external",
        _ => return Err("Modo de pantalla no válido.".into()),
    };
    crate::ps::exec("DisplaySwitch.exe", &[arg]).map(|_| ()).map_err(|e| format!("No se pudo cambiar las pantallas: {}", crate::ps::clean_error(&e)))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Wireless {
    /// None: no hay o no se pudo leer.
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
    /// «Wi-Fi «Oficina-5G» · señal 68 %» o «Cable (o sin Wi-Fi)».
    pub line: String,
}

#[tauri::command(async)]
pub fn bar_wireless() -> Wireless {
    Wireless {
        wifi: crate::network::wifictl::radio_kind("WiFi", ""),
        bluetooth: crate::network::wifictl::radio_kind("Bluetooth", ""),
        line: crate::rocketbar::network_line(&crate::ps::exec("netsh", &["wlan", "show", "interfaces"]).unwrap_or_default()),
    }
}

/// Enciende o apaga la Wi-Fi o el Bluetooth (el interruptor de Configuración, no la tarjeta).
#[tauri::command(async)]
pub fn bar_set_radio(kind: String, on: bool) -> Result<bool, String> {
    let (k, name) = match kind.as_str() {
        "wifi" => ("WiFi", "la Wi-Fi"),
        "bluetooth" => ("Bluetooth", "el Bluetooth"),
        _ => return Err("Radio no válida.".into()),
    };
    let now = crate::network::wifictl::radio_kind(k, if on { "On" } else { "Off" }).ok_or_else(|| format!("No se pudo cambiar {name} (¿está en modo avión o lo bloquea el equipo?)."))?;
    // Windows puede aceptar la orden y dejarla como estaba (modo avión, interruptor físico, política).
    radio_result(now, on, name)
}

fn radio_result(now: bool, wanted: bool, name: &str) -> Result<bool, String> {
    if now == wanted {
        Ok(now)
    } else {
        let state = match (now, name.starts_with("la ")) {
            (true, true) => "encendida",
            (false, true) => "apagada",
            (true, false) => "encendido",
            (false, false) => "apagado",
        };
        Err(format!("Windows no dejó {} {name}: sigue {state} (¿modo avión o un interruptor del equipo?).", if wanted { "encender" } else { "apagar" }))
    }
}

/// El perfil de la Wi-Fi conectada, de `netsh wlan show interfaces` («Profile» o «Perfil»).
pub fn wifi_profile(netsh: &str) -> Option<String> {
    netsh.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        let k = k.trim();
        (k.eq_ignore_ascii_case("Profile") || k.eq_ignore_ascii_case("Perfil")).then(|| v.trim().to_string()).filter(|v| !v.is_empty())
    })
}

/// «Reconectar la Wi-Fi»: se desconecta y vuelve a la misma red, lo que arregla la Wi-Fi «colgada».
#[tauri::command(async)]
pub fn bar_wifi_reconnect() -> Result<String, String> {
    let out = crate::ps::exec("netsh", &["wlan", "show", "interfaces"]).unwrap_or_default();
    let profile = wifi_profile(&out).ok_or("No hay ninguna Wi-Fi conectada a la que volver.")?;
    let _ = crate::ps::exec("netsh", &["wlan", "disconnect"]);
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let name = format!("name={profile}");
    crate::ps::exec("netsh", &["wlan", "connect", &name]).map_err(|e| format!("No se pudo volver a «{profile}»: {}", crate::ps::clean_error(&e)))?;
    Ok(format!("Reconectando a «{profile}»…"))
}

/// Una carpeta del usuario con la sesión abierta, también si OneDrive la redirige.
fn shell_folder(value: &str, fallback: &str) -> Option<std::path::PathBuf> {
    let u = crate::target_user::user_dirs()?;
    let root = match crate::target_user::hkcu_redirect() {
        Some(sid) => format!(r"HKU\{sid}"),
        None => "HKCU".into(),
    };
    let raw = crate::tweaks::registry::read_string(&format!(r"{root}\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders"), value);
    let p = raw
        .map(|r| r.replace("%USERPROFILE%", &u.profile.to_string_lossy()))
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| u.profile.join(fallback));
    p.is_dir().then_some(p)
}

/// «Mis carpetas»: Escritorio, Descargas, Documentos o una carpeta elegida (ruta completa).
#[tauri::command(async)]
pub fn bar_open_folder(which: String) -> Result<(), String> {
    let path = match which.as_str() {
        "desktop" => shell_folder("Desktop", "Desktop"),
        "downloads" => shell_folder("{374DE290-123F-4565-9164-39C4925E467B}", "Downloads"),
        "documents" => shell_folder("Personal", "Documents"),
        other => {
            let p = std::path::PathBuf::from(other);
            (p.is_absolute() && p.is_dir()).then_some(p)
        }
    }
    .ok_or("Esa carpeta ya no existe o no está conectada.")?;
    crate::shellopen::open(&path.to_string_lossy())
}

/// La consola: «cmd» y «powershell» como el usuario (sin elevar); «cmd-admin» como administrador (con
/// AdminOps elevado se abre directamente; si no, Windows pide la contraseña con su aviso).
#[tauri::command(async)]
pub fn bar_console(kind: String) -> Result<(), String> {
    match kind.as_str() {
        "cmd" => crate::shellopen::open("cmd.exe"),
        "powershell" => crate::shellopen::open("powershell.exe"),
        "cmd-admin" => run_admin("cmd.exe"),
        _ => Err("Consola no válida.".into()),
    }
}

#[cfg(windows)]
fn run_admin(program: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    // Que empiece en la carpeta del usuario y no en System32.
    let dir = crate::target_user::user_dirs().map(|u| u.profile.to_string_lossy().into_owned()).unwrap_or_default();
    let (verb, file, wdir) = (wide("runas"), wide(program), wide(&dir));
    // SAFETY: cadenas terminadas en cero, vivas durante la llamada.
    let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), if dir.is_empty() { std::ptr::null() } else { wdir.as_ptr() }, SW_SHOWNORMAL) };
    if r as isize > 32 {
        Ok(())
    } else if r as isize == 5 {
        Err("Cancelado: Windows no dio permiso de administrador.".into())
    } else {
        Err(format!("No se pudo abrir la consola (código {}).", r as isize))
    }
}

#[cfg(not(windows))]
fn run_admin(_: &str) -> Result<(), String> {
    Err("Solo en Windows.".into())
}

/// Bloquear el equipo (Win+L).
#[tauri::command(async)]
pub fn bar_lock() -> Result<(), String> {
    crate::ps::exec("rundll32.exe", &["user32.dll,LockWorkStation"]).map(|_| ()).map_err(|e| format!("No se pudo bloquear: {}", crate::ps::clean_error(&e)))
}

/// Reinicio programado para esta noche (segundos Unix; 0 = ninguno). Lo lleva AdminOps, no Windows: así
/// el reinicio nunca fuerza el cierre de los programas (`shutdown /t` mayor que 0 implica /f).
static TONIGHT: AtomicU64 = AtomicU64::new(0);

/// Las 22:00 de hoy; pasadas las 21:45, las 2:00 de mañana.
pub fn tonight_at(now: chrono::DateTime<chrono::Local>) -> chrono::DateTime<chrono::Local> {
    use chrono::{Duration, TimeZone, Timelike};
    let day = now.date_naive();
    let at = |d: chrono::NaiveDate, h: u32| chrono::Local.from_local_datetime(&d.and_hms_opt(h, 0, 0).unwrap_or_default()).earliest().unwrap_or(now + Duration::hours(1));
    if now.hour() < 21 || (now.hour() == 21 && now.minute() < 45) {
        at(day, 22)
    } else {
        at(day + Duration::days(1), 2)
    }
}

/// ¿Toca reiniciar ya? Solo a la hora o poco después (el hilo mira cada 30 s). Pasada más de una hora (el
/// equipo estuvo suspendido), no.
pub const RESTART_SKIPPED_EVENT: &str = "rocketbar://restart-skipped";

pub fn tonight_due(now: u64, at: u64) -> bool {
    now >= at && now - at <= 3600
}

/// Reinicia sin /f: si algo está sin guardar, Windows lo enseña y deja cancelar; nunca se pierde nada.
fn restart_now() -> Result<(), String> {
    crate::ps::exec("shutdown.exe", &["/r", "/t", "0"]).map(|_| ()).map_err(|e| format!("Windows no dejó reiniciar: {}", crate::ps::clean_error(&e)))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartState {
    /// Segundos Unix del reinicio de esta noche, si hay uno.
    pub tonight: Option<u64>,
}

/// «now», «tonight», «cancel» o «» (solo leer el estado).
#[tauri::command(async)]
pub fn bar_restart(app: tauri::AppHandle, when: String) -> Result<RestartState, String> {
    match when.as_str() {
        "now" => restart_now()?,
        "tonight" => {
            let at = tonight_at(chrono::Local::now()).timestamp().max(0) as u64;
            TONIGHT.store(at, Ordering::SeqCst);
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(30));
                let t = TONIGHT.load(Ordering::SeqCst);
                // Cancelado u otro programado después: este hilo se retira.
                if t != at {
                    return;
                }
                let now = chrono::Local::now().timestamp().max(0) as u64;
                if now >= at {
                    TONIGHT.store(0, Ordering::SeqCst);
                    if !tonight_due(now, at) {
                        // El portátil estuvo suspendido y despierta por la mañana: reiniciar ahora le quitaría el
                        // trabajo de delante. Se cancela; se puede volver a programar.
                        log::info!("Reinicio de esta noche cancelado: el equipo despertó {} min después de la hora.", (now - at) / 60);
                        // La barra lo dice, para que no se dé por hecho que se reinició.
                        let _ = tauri::Emitter::emit(&app, RESTART_SKIPPED_EVENT, ());
                        return;
                    }
                    if let Err(e) = restart_now() {
                        log::warn!("Reinicio de esta noche: {e}");
                    }
                    return;
                }
            });
        }
        "cancel" => TONIGHT.store(0, Ordering::SeqCst),
        "" => {}
        _ => return Err("Opción no válida.".into()),
    }
    let t = TONIGHT.load(Ordering::SeqCst);
    Ok(RestartState { tonight: (t > 0).then_some(t) })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Timelike};

    #[test]
    fn perfil_de_la_wifi() {
        let en = "    Name                   : Wi-Fi\n    State                  : connected\n    SSID                   : Oficina-5G\n    Profile                : Oficina-5G\n";
        assert_eq!(super::wifi_profile(en).as_deref(), Some("Oficina-5G"));
        let es = "    Nombre                 : Wi-Fi\n    Perfil                 : Casa Pérez\n";
        assert_eq!(super::wifi_profile(es).as_deref(), Some("Casa Pérez"));
        assert_eq!(super::wifi_profile("    Estado : desconectado\n"), None);
    }

    #[test]
    fn la_radio_tiene_que_quedar_como_se_pidio() {
        assert_eq!(super::radio_result(true, true, "la Wi-Fi"), Ok(true));
        let e = super::radio_result(false, true, "la Wi-Fi").unwrap_err();
        assert!(e.contains("encender la Wi-Fi") && e.contains("sigue apagada"), "{e}");
        let e = super::radio_result(true, false, "el Bluetooth").unwrap_err();
        assert!(e.contains("sigue encendido"), "{e}");
    }

    #[test]
    fn reinicio_de_esta_noche_tras_suspender() {
        // A su hora (o con un minuto de retraso del hilo) se reinicia; horas después, al despertar, no.
        assert!(super::tonight_due(1000, 1000));
        assert!(super::tonight_due(1000 + 60, 1000));
        assert!(!super::tonight_due(999, 1000));
        assert!(!super::tonight_due(1000 + 3 * 3600, 1000));
    }

    #[test]
    fn esta_noche() {
        let tarde = chrono::Local.with_ymd_and_hms(2026, 11, 27, 15, 0, 0).unwrap();
        let t = super::tonight_at(tarde);
        assert_eq!((t.date_naive(), t.hour()), (tarde.date_naive(), 22));
        let tarde_noche = chrono::Local.with_ymd_and_hms(2026, 11, 27, 23, 0, 0).unwrap();
        let t = super::tonight_at(tarde_noche);
        assert_eq!(t.hour(), 2);
        assert!(t > tarde_noche);
    }
}
