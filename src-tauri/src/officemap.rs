//! Mapa de la oficina: qué es cada dispositivo de una red (función, responsable,
//! notas) y vigilancia de los dispositivos clave (impresora, servidor, NAS…):
//! si dejan de responder mientras AdminOps está abierta, avisa.
//!
//! Se guarda por red (MAC del router) y por dispositivo (su MAC), en los datos
//! que viajan con AdminOps.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

static FILE_LOCK: Mutex<()> = Mutex::new(());
/// Estado de la vigilancia: (red, MAC) → (responde, fallos seguidos, desde).
static STATE: Mutex<Option<HashMap<(String, String), WatchState>>> = Mutex::new(None);

const POLL: Duration = Duration::from_secs(60);
/// Fallos seguidos antes de avisar (evita avisos por un ping perdido).
const FAILS_TO_ALERT: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceMeta {
    /// Para qué sirve: «Impresora de contabilidad», «Servidor de archivos»…
    pub role: String,
    /// Contacto responsable (id de la agenda).
    pub contact_id: String,
    pub notes: String,
    /// Avisar si deja de responder.
    pub watch: bool,
    /// Última IP y nombre conocidos (para vigilarlo y mostrarlo aunque no esté).
    pub ip: String,
    pub name: String,
    pub updated: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchState {
    pub up: bool,
    #[serde(skip)]
    fails: u32,
    #[serde(skip)]
    established: bool,
    /// Desde cuándo está en ese estado.
    pub since: u64,
    pub checked: u64,
}

type Store = HashMap<String, HashMap<String, DeviceMeta>>;

fn path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("mapa-oficina.json")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn norm_mac(mac: &str) -> String {
    mac.trim().to_lowercase().replace('-', ":")
}

#[tauri::command]
pub fn office_map(app: tauri::AppHandle, key: String) -> HashMap<String, DeviceMeta> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let store: Store = crate::paths::read_json(&path(&app));
    store.get(&key).cloned().unwrap_or_default()
}

#[tauri::command]
pub fn save_device_meta(app: tauri::AppHandle, key: String, mac: String, meta: DeviceMeta) -> Result<(), String> {
    if key.is_empty() || mac.is_empty() {
        return Err("Falta la red o el dispositivo.".into());
    }
    let cut = |s: &str, n: usize| s.trim().chars().take(n).collect::<String>();
    let meta = DeviceMeta {
        role: cut(&meta.role, 120),
        contact_id: cut(&meta.contact_id, 64),
        notes: meta.notes.chars().take(4000).collect(),
        watch: meta.watch,
        ip: cut(&meta.ip, 45),
        name: cut(&meta.name, 120),
        updated: now(),
    };
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store: Store = crate::paths::read_json(&path(&app));
    let net = store.entry(key).or_default();
    let empty = meta.role.is_empty() && meta.contact_id.is_empty() && meta.notes.is_empty() && !meta.watch;
    if empty {
        net.remove(&norm_mac(&mac));
    } else {
        net.insert(norm_mac(&mac), meta);
    }
    crate::paths::write_json(&path(&app), &store)
}

/// Tras una búsqueda: actualiza la IP de los dispositivos anotados (el DHCP la cambia).
#[tauri::command]
pub fn refresh_device_ips(app: tauri::AppHandle, key: String, seen: Vec<(String, String)>) -> Result<(), String> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store: Store = crate::paths::read_json(&path(&app));
    let Some(net) = store.get_mut(&key) else { return Ok(()) };
    let mut changed = false;
    for (mac, ip) in seen {
        if let Some(m) = net.get_mut(&norm_mac(&mac)) {
            if m.ip != ip {
                m.ip = ip;
                changed = true;
            }
        }
    }
    if changed {
        crate::paths::write_json(&path(&app), &store)?;
    }
    Ok(())
}

/// Estado de la vigilancia en esta red (MAC → estado).
#[tauri::command]
pub fn watch_status(key: String) -> HashMap<String, WatchState> {
    let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .map(|m| m.iter().filter(|((k, _), _)| *k == key).map(|((_, mac), s)| (mac.clone(), s.clone())).collect())
        .unwrap_or_default()
}

/// Nuevo estado de un dispositivo tras un ping. Devuelve si hay que avisar
/// (Some(false): dejó de responder; Some(true): volvió).
fn step(s: &mut WatchState, reached: bool, t: u64) -> Option<bool> {
    s.checked = t;
    if reached {
        s.fails = 0;
        if !s.established {
            // Primera respuesta: se empieza a vigilar, sin aviso.
            s.established = true;
            s.up = true;
            s.since = t;
            return None;
        }
        if !s.up {
            s.up = true;
            s.since = t;
            return Some(true);
        }
        return None;
    }
    s.fails += 1;
    if s.fails >= FAILS_TO_ALERT && (s.up || !s.established) {
        s.established = true;
        s.up = false;
        s.since = t;
        return Some(false);
    }
    None
}

fn poll(app: &tauri::AppHandle) {
    let store: Store = {
        let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::paths::read_json(&path(app))
    };
    if !store.values().any(|n| n.values().any(|m| m.watch)) {
        return;
    }
    let Ok(Some(info)) = crate::network::lan::current() else { return };
    let Some(net) = store.get(&info.key) else { return };
    let watched: Vec<(&String, &DeviceMeta)> = net.iter().filter(|(_, m)| m.watch && !m.ip.is_empty()).collect();
    let results: Vec<(String, bool)> = std::thread::scope(|s| {
        let h: Vec<_> = watched.iter().map(|(mac, m)| s.spawn(move || ((*mac).clone(), crate::network::diag::ping("vigilar", &m.ip, 2).received > 0))).collect();
        h.into_iter().filter_map(|x| x.join().ok()).collect()
    });
    let t = now();
    let mut alerts = Vec::new();
    {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(HashMap::new);
        for (mac, reached) in results {
            let s = map.entry((info.key.clone(), mac.clone())).or_insert(WatchState { since: t, checked: t, ..Default::default() });
            if let Some(up) = step(s, reached, t) {
                let m = &net[&mac];
                let label = if m.role.is_empty() { if m.name.is_empty() { m.ip.clone() } else { m.name.clone() } } else { m.role.clone() };
                alerts.push(crate::winwatch::Alert {
                    level: if up { "info" } else { "bad" }.into(),
                    key: format!("watch:{mac}:{up}"),
                    time: t,
                    title: if up { format!("{label} vuelve a responder") } else { format!("{label} no responde") },
                    detail: format!("{} · {}", m.ip, info.network),
                    explanation: if up {
                        "El dispositivo vigilado vuelve a estar disponible en la red.".into()
                    } else {
                        "El dispositivo vigilado ha dejado de responder en la red: puede estar apagado, sin cable, sin papel en el caso de una impresora bloqueada o con otra IP.".into()
                    },
                    advice: if up { String::new() } else { "Comprueba que esté encendido y conectado. Si cambió de IP, búscalo de nuevo en Dispositivos en la red.".into() },
                    page: Some("devices".into()),
                    count: 1,
                    ..Default::default()
                });
            }
        }
    }
    if !alerts.is_empty() {
        crate::winwatch::push_alerts(app, alerts);
    }
}

/// Hilo de vigilancia de dispositivos clave (solo trabaja si hay alguno marcado).
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            poll(&app);
            std::thread::sleep(POLL);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerts_after_two_failures_and_on_recovery() {
        let mut s = WatchState::default();
        // Primer ping bien: empieza a vigilarse, sin aviso.
        assert_eq!(step(&mut s, true, 1), None);
        assert!(s.up);
        // Un fallo suelto no avisa.
        assert_eq!(step(&mut s, false, 2), None);
        assert_eq!(step(&mut s, true, 3), None);
        // Dos seguidos, sí.
        assert_eq!(step(&mut s, false, 4), None);
        assert_eq!(step(&mut s, false, 5), Some(false));
        assert_eq!(step(&mut s, false, 6), None);
        assert_eq!(step(&mut s, true, 7), Some(true));
    }

    #[test]
    fn device_already_down_when_watching_starts() {
        let mut s = WatchState::default();
        assert_eq!(step(&mut s, false, 1), None);
        assert_eq!(step(&mut s, false, 2), Some(false));
    }
}
