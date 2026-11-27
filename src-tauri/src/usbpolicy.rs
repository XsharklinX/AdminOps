//! Memorias USB: permitidas, solo lectura (se abren, no se copia nada a ellas) o bloqueadas, equipo
//! por equipo. Con las directivas de «Acceso al almacenamiento extraíble» de Windows: solo afectan a
//! discos extraíbles y teléfonos o cámaras conectados como almacenamiento; teclado, ratón, impresora,
//! escáner y lector de huella siguen igual.
//!
//! Las directivas no admiten excepciones por dispositivo (eso es Intune). Para usar un pendrive un
//! momento: «Permitir 15 minutos», que deja una tarea programada que vuelve a poner lo que había
//! aunque AdminOps se cierre o el equipo se reinicie.

use serde::Serialize;

const BASE: &str = r"HKLM\SOFTWARE\Policies\Microsoft\Windows\RemovableStorageDevices";
/// Discos extraíbles y los dos tipos de dispositivos portátiles (WPD: teléfonos, cámaras, reproductores).
const CLASSES: [&str; 3] = ["{53f5630d-b6bf-11d0-94f2-00a0c91efb8b}", "{6AC27878-A6FA-4155-BA85-F98F491D4F33}", "{F33FDC04-D1AC-4E8E-9A30-19BBD4B108AE}"];
const WRITE_PROTECT: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\StorageDevicePolicies";
const TASK: &str = "AdminOps - volver a bloquear USB";

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbState {
    /// allowed | readonly | blocked
    pub mode: String,
    /// Hay un «Permitir 15 minutos» en marcha: cuándo vuelve (hora local, «14:35»).
    pub until: Option<String>,
    pub admin: bool,
}

pub fn valid_mode(m: &str) -> bool {
    matches!(m, "allowed" | "readonly" | "blocked")
}

fn read_mode() -> String {
    use crate::tweaks::registry::read_u32;
    let on = |name: &str| CLASSES.iter().any(|c| read_u32(&format!(r"{BASE}\{c}"), name) == Some(1));
    if on("Deny_Read") {
        "blocked".into()
    } else if on("Deny_Write") || read_u32(WRITE_PROTECT, "WriteProtect") == Some(1) {
        "readonly".into()
    } else {
        "allowed".into()
    }
}

/// Las órdenes `reg` que dejan el modo puesto (también sirven para la tarea de vuelta).
pub fn reg_commands(mode: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (read, write) = match mode {
        "blocked" => (true, true),
        "readonly" => (false, true),
        _ => (false, false),
    };
    for c in CLASSES {
        let key = format!(r"{BASE}\{c}");
        for (name, on) in [("Deny_Read", read), ("Deny_Write", write), ("Deny_Execute", read)] {
            out.push(if on { format!(r#"reg add "{key}" /v {name} /t REG_DWORD /d 1 /f"#) } else { format!(r#"reg delete "{key}" /v {name} /f"#) });
        }
    }
    out.push(if write { format!(r#"reg add "{WRITE_PROTECT}" /v WriteProtect /t REG_DWORD /d 1 /f"#) } else { format!(r#"reg delete "{WRITE_PROTECT}" /v WriteProtect /f"#) });
    out
}

fn set_mode(mode: &str) -> Result<(), String> {
    use crate::tweaks::model::{RegData, RegKind};
    use crate::tweaks::registry::{delete, read_u32, write};
    let (read, wr) = match mode {
        "blocked" => (true, true),
        "readonly" => (false, true),
        _ => (false, false),
    };
    let one = RegData::Int(1);
    for c in CLASSES {
        let key = format!(r"{BASE}\{c}");
        for (name, on) in [("Deny_Read", read), ("Deny_Write", wr), ("Deny_Execute", read)] {
            if on {
                write(&key, name, RegKind::Dword, &one)?;
            } else if read_u32(&key, name).is_some() {
                delete(&key, name)?;
            }
        }
    }
    if wr {
        write(WRITE_PROTECT, "WriteProtect", RegKind::Dword, &one)?;
    } else if read_u32(WRITE_PROTECT, "WriteProtect").is_some() {
        delete(WRITE_PROTECT, "WriteProtect")?;
    }
    Ok(())
}

/// El script de la tarea de vuelta: deja el modo de antes y se borra a sí misma.
pub fn restore_script(mode: &str) -> String {
    let mut s: Vec<String> = reg_commands(mode).into_iter().map(|c| format!("{c} 2>$null | Out-Null")).collect();
    s.push(format!("Unregister-ScheduledTask -TaskName {} -Confirm:$false -ErrorAction SilentlyContinue", crate::ps::ps_literal(TASK)));
    s.join("\n")
}

/// Para `-EncodedCommand`: UTF-16LE en base64.
fn encode_ps(script: &str) -> String {
    use base64::Engine;
    let wide: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(wide)
}

fn task_until() -> Option<String> {
    let ps = format!("$i = Get-ScheduledTaskInfo -TaskName {} -ErrorAction SilentlyContinue; if ($i -and $i.NextRunTime) {{ $i.NextRunTime.ToString('HH:mm') }}", crate::ps::ps_literal(TASK));
    crate::ps::powershell(&ps).ok().map(|s| s.trim().to_string()).filter(|s| s.len() == 5)
}

fn cancel_task() {
    let _ = crate::ps::powershell(&format!("Unregister-ScheduledTask -TaskName {} -Confirm:$false -ErrorAction SilentlyContinue", crate::ps::ps_literal(TASK)));
}

#[tauri::command]
pub async fn usb_state() -> Result<UsbState, String> {
    tauri::async_runtime::spawn_blocking(|| Ok(UsbState { mode: read_mode(), until: task_until(), admin: crate::elevation::is_elevated() })).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn usb_set(app: tauri::AppHandle, mode: String) -> Result<UsbState, String> {
    let label = match mode.as_str() {
        "allowed" => "permitidas",
        "readonly" => "solo lectura",
        _ => "bloqueadas",
    };
    let r = usb_set_inner(mode).await;
    crate::tweaks::record_app(&app, &format!("Memorias USB: {label}"), &r);
    r
}

async fn usb_set_inner(mode: String) -> Result<UsbState, String> {
    if !valid_mode(&mode) {
        return Err("Modo de USB no válido.".into());
    }
    if !crate::elevation::is_elevated() {
        return Err("Hace falta abrir AdminOps como administrador.".into());
    }
    let m = mode.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cancel_task();
        set_mode(&m)
    })
    .await
    .map_err(|e| e.to_string())??;
    log::info!("Memorias USB: {mode}");
    usb_state().await
}

/// Permite las memorias `minutes` minutos y deja programada la vuelta a lo que había.
#[tauri::command]
pub async fn usb_allow_for(app: tauri::AppHandle, minutes: u32) -> Result<UsbState, String> {
    let r = usb_allow_for_inner(minutes).await;
    crate::tweaks::record_app(&app, &format!("Memorias USB permitidas {} min", minutes.clamp(1, 240)), &r);
    r
}

async fn usb_allow_for_inner(minutes: u32) -> Result<UsbState, String> {
    if !crate::elevation::is_elevated() {
        return Err("Hace falta abrir AdminOps como administrador.".into());
    }
    let minutes = minutes.clamp(1, 240);
    tauri::async_runtime::spawn_blocking(move || {
        let before = read_mode();
        if before == "allowed" {
            return Err("Las memorias USB ya están permitidas.".to_string());
        }
        // Las órdenes de vuelta van dentro de la propia tarea (codificadas), no en un archivo: un archivo en
        // una carpeta que un usuario pudiera tocar lo ejecutaría SYSTEM, o se podría borrar y las memorias
        // se quedarían permitidas para siempre.
        let encoded = encode_ps(&restore_script(&before));
        // Con PowerShell y no con schtasks: la fecha de schtasks cambia con el idioma del equipo.
        // StartWhenAvailable: si el equipo estaba apagado a esa hora, se hace al arrancar.
        let ps = format!(
            "$a = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand {encoded}'
             $t = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes({minutes})
             $s = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
             Register-ScheduledTask -TaskName {task} -Action $a -Trigger $t -Settings $s -User 'SYSTEM' -RunLevel Highest -Force | Out-Null
             (Get-Date).AddMinutes({minutes}).ToString('HH:mm')",
            task = crate::ps::ps_literal(TASK),
        );
        let at = crate::ps::powershell(&ps).map_err(|e| format!("No se pudo programar la vuelta: {}", crate::ps::clean_error(&e)))?.trim().to_string();
        set_mode("allowed")?;
        log::info!("Memorias USB permitidas {minutes} min; vuelven a «{before}» a las {at}");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    usb_state().await
}

// ---------- Las memorias que se han conectado ----------

const USBSTOR: &str = r"HKLM\SYSTEM\CurrentControlSet\Enum\USBSTOR";

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsbSeen {
    pub name: String,
    pub serial: String,
    /// Última conexión, hora local («2026-11-24 09:15»); no siempre se sabe.
    pub last: Option<String>,
}

/// El número de serie de un InstanceId (`USBSTOR\DISK&VEN_X&PROD_Y&REV_1\<serie>&0` → `<serie>`).
fn serial_of(instance: &str) -> String {
    let tail = instance.rsplit('\\').next().unwrap_or(instance);
    tail.strip_suffix("&0").unwrap_or(tail).to_string()
}

/// Lee lo que devuelve PowerShell: un objeto suelto (un solo dispositivo) o una lista. Devuelve
/// (serie, última conexión, nombre).
fn parse_arrivals(json: &str) -> Vec<(String, Option<String>, Option<String>)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json.trim()) else { return vec![] };
    let items = match v {
        serde_json::Value::Array(a) => a,
        o @ serde_json::Value::Object(_) => vec![o],
        _ => vec![],
    };
    let text = |o: &serde_json::Value, k: &str| o.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    items.iter().filter_map(|o| Some((serial_of(&text(o, "id")?), text(o, "last"), text(o, "name")))).collect()
}

/// Ordena de la conexión más reciente a la más antigua; las que no tienen fecha, al final.
fn sort_seen(v: &mut [UsbSeen]) {
    v.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.name.cmp(&b.name)));
}

/// «Disk&Ven_Kingston&Prod_DataTraveler&Rev_1.00» → «Kingston DataTraveler» (si no hay nombre amable).
fn name_from_key(key: &str) -> String {
    let part = |p: &str| key.split('&').find_map(|s| s.strip_prefix(p)).map(|s| s.replace('_', " "));
    let n = [part("Ven_"), part("Prod_")].into_iter().flatten().collect::<Vec<_>>().join(" ");
    if n.trim().is_empty() { "Memoria USB".into() } else { n }
}

/// Las memorias y discos USB que Windows ha visto en este equipo (están o no conectados ahora).
#[tauri::command(async)]
pub fn usb_history() -> Vec<UsbSeen> {
    use crate::tweaks::registry::{read_string, subkeys};
    let mut out: Vec<UsbSeen> = Vec::new();
    for dev in subkeys(USBSTOR) {
        for inst in subkeys(&format!(r"{USBSTOR}\{dev}")) {
            let name = read_string(&format!(r"{USBSTOR}\{dev}\{inst}"), "FriendlyName").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| name_from_key(&dev));
            out.push(UsbSeen { name, serial: serial_of(&inst), last: None });
        }
    }
    if !out.is_empty() {
        // Una sola consulta: la fecha de la última conexión no está en el registro de forma sencilla.
        let ps = "Get-PnpDevice -Class DiskDrive -PresentOnly:$false | Where-Object InstanceId -like 'USBSTOR*' | ForEach-Object { $d = (Get-PnpDeviceProperty -InstanceId $_.InstanceId -KeyName DEVPKEY_Device_LastArrivalDate -ErrorAction SilentlyContinue).Data; [pscustomobject]@{ id=$_.InstanceId; name=$_.FriendlyName; last= if ($d) { $d.ToString('yyyy-MM-dd HH:mm') } else { $null } } } | ConvertTo-Json -Compress";
        if let Ok(raw) = crate::ps::powershell_opts(ps, crate::ps::Opts { timeout: Some(std::time::Duration::from_secs(60)), task: None }) {
            for (serial, last, _) in parse_arrivals(&raw) {
                for s in out.iter_mut().filter(|s| s.serial.eq_ignore_ascii_case(&serial)) {
                    s.last = last.clone();
                }
            }
        }
    }
    sort_seen(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historial_objeto_y_lista() {
        let uno = r#"{"id":"USBSTOR\\DISK&VEN_KINGSTON&PROD_DT&REV_1.00\\60A44C42&0","name":"Kingston DT","last":"2026-11-24 09:15"}"#;
        let r = parse_arrivals(uno);
        assert_eq!(r, vec![("60A44C42".to_string(), Some("2026-11-24 09:15".to_string()), Some("Kingston DT".to_string()))]);
        let lista = format!("[{uno},{{\"id\":\"USBSTOR\\\\DISK&VEN_X&PROD_Y&REV_1\\\\ABC&0\",\"name\":null,\"last\":null}}]");
        let r = parse_arrivals(&lista);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1], ("ABC".to_string(), None, None));
        assert!(parse_arrivals("").is_empty());
        assert!(parse_arrivals("basura").is_empty());
    }

    #[test]
    fn serie_y_orden() {
        assert_eq!(serial_of(r"USBSTOR\DISK&VEN_A&PROD_B&REV_1\6&2a3b&0"), "6&2a3b");
        assert_eq!(serial_of("XYZ"), "XYZ");
        let mk = |n: &str, l: Option<&str>| UsbSeen { name: n.into(), serial: n.into(), last: l.map(str::to_string) };
        let mut v = vec![mk("a", None), mk("b", Some("2026-01-01 10:00")), mk("c", Some("2026-05-01 10:00"))];
        sort_seen(&mut v);
        assert_eq!(v.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["c", "b", "a"]);
        assert_eq!(name_from_key("Disk&Ven_Kingston&Prod_Data_Traveler&Rev_1.00"), "Kingston Data Traveler");
    }

    #[test]
    fn ordenes_de_vuelta() {
        let b = reg_commands("blocked");
        assert_eq!(b.len(), 10);
        assert!(b.iter().filter(|c| c.starts_with("reg add")).count() == 10);
        let r = reg_commands("readonly");
        assert!(r.iter().any(|c| c.contains("Deny_Write /t REG_DWORD /d 1")));
        assert!(r.iter().any(|c| c.contains("reg delete") && c.contains("Deny_Read")));
        assert!(r.last().unwrap().contains("WriteProtect /t REG_DWORD /d 1"));
        assert!(reg_commands("allowed").iter().all(|c| c.starts_with("reg delete")));
    }

    #[test]
    fn la_tarea_de_vuelta_no_usa_archivos() {
        let s = restore_script("blocked");
        assert_eq!(s.lines().count(), 11);
        assert!(s.ends_with("-Confirm:$false -ErrorAction SilentlyContinue"));
        assert!(!s.contains(".cmd"));
        // Cabe de sobra en el argumento de una tarea programada.
        assert!(encode_ps(&s).len() < 8000);
    }

    #[test]
    fn modos() {
        assert!(valid_mode("readonly") && !valid_mode("solo"));
    }
}
