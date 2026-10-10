//! Qué sale de este equipo (1.2.9): la lista honesta de los pocos sitios con los
//! que AdminOps habla por su cuenta, qué se envía a cada uno y cuándo, con un
//! interruptor para cada servicio que se puede apagar. Todo lo demás está bloqueado
//! por la política de contenido de la ventana. Los portales que configuras
//! (tickets, correo, Teams) son webs que abres tú dentro de AdminOps: no están aquí.

use serde::Serialize;
use std::collections::HashMap;

pub struct Entry {
    pub id: &'static str,
    pub host: &'static str,
    pub what: &'static str,
    /// Lo que viaja en la petición.
    pub sends: &'static str,
    pub when: &'static str,
    /// Se puede apagar desde Ajustes. El resto es una prueba de conectividad o va por el propio Windows.
    pub switchable: bool,
}

pub const ENTRIES: &[Entry] = &[
    Entry { id: "updates", host: "api.github.com · github.com", what: "Buscar y descargar versiones nuevas de AdminOps", sends: "Solo la petición (tu IP y la versión de AdminOps). No se envía nada de tus datos.", when: "Al abrir (si lo tienes activado en Avisos) y cuando pulsas «Buscar»", switchable: true },
    Entry { id: "speedtest", host: "speed.cloudflare.com · 1.1.1.1", what: "Test de velocidad de Internet", sends: "Datos de prueba de ida y vuelta. Tu IP la ve el servidor, como en cualquier web.", when: "Solo cuando lanzas el test", switchable: true },
    Entry { id: "publicip", host: "ipinfo.io", what: "Saber tu IP pública y tu proveedor", sends: "Solo la petición. Devuelve tu IP, ciudad y proveedor.", when: "Solo cuando lo pides (Red) y durante el test de velocidad", switchable: true },
    Entry { id: "vendors", host: "api.macvendors.com", what: "Averiguar el fabricante de un dispositivo de la red por su MAC", sends: "Los 3 primeros bytes de la MAC (el fabricante), nunca la MAC entera ni el nombre del equipo.", when: "Al escanear la red, con los fabricantes que aún no se conocen (se guardan para no repetir)", switchable: true },
    Entry { id: "connectivity", host: "www.msftconnecttest.com · 1.1.1.1 · 8.8.8.8", what: "Comprobar si hay Internet (la prueba que hace el propio Windows)", sends: "Una petición de prueba y pings. Nada de tus datos.", when: "Al diagnosticar la red y en «Solucionar problemas»", switchable: false },
    Entry { id: "winget", host: "Orígenes de winget (Microsoft)", what: "Buscar, instalar y actualizar programas", sends: "Lo gestiona winget, no AdminOps: AdminOps solo le pide el programa que elijas.", when: "Solo cuando buscas, instalas o actualizas programas", switchable: false },
];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub host: String,
    pub what: String,
    pub sends: String,
    pub when: String,
    pub switchable: bool,
    pub enabled: bool,
    /// Segundos desde 1970 de la última vez que AdminOps habló con él (0: nunca desde que se lleva la cuenta).
    pub last_used: u64,
}

/// ¿Está este servicio encendido? Lo que no se puede apagar, siempre.
pub fn is_enabled(disabled: &[String], id: &str) -> bool {
    !ENTRIES.iter().any(|e| e.id == id && e.switchable) || !disabled.iter().any(|d| d == id)
}

/// La lista que ve el técnico, con el estado de cada interruptor y la última vez que se usó.
pub fn services(disabled: &[String], last: &HashMap<String, u64>) -> Vec<Service> {
    ENTRIES
        .iter()
        .map(|e| Service {
            id: e.id.into(),
            host: e.host.into(),
            what: e.what.into(),
            sends: e.sends.into(),
            when: e.when.into(),
            switchable: e.switchable,
            enabled: is_enabled(disabled, e.id),
            last_used: last.get(e.id).copied().unwrap_or(0),
        })
        .collect()
}

pub const OFF_MESSAGE: &str = "Está apagado en Ajustes → Seguridad → «Qué sale de este equipo».";

fn last_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("outbound.json")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Antes de hablar con un servicio: si está apagado, no se habla; si no, se apunta cuándo.
pub fn guard(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let settings = crate::workflow::settings(app);
    if !is_enabled(&settings.net_off, id) {
        let what = ENTRIES.iter().find(|e| e.id == id).map_or("Este servicio", |e| e.what);
        return Err(format!("{what}: {OFF_MESSAGE}"));
    }
    let path = last_path(app);
    let mut last: HashMap<String, u64> = crate::paths::read_json(&path);
    last.insert(id.to_string(), now());
    let _ = crate::paths::write_json(&path, &last);
    Ok(())
}

#[tauri::command(async)]
pub fn outbound_list(app: tauri::AppHandle) -> Vec<Service> {
    let last: HashMap<String, u64> = crate::paths::read_json(&last_path(&app));
    services(&crate::workflow::settings(&app).net_off, &last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_switchable_services_can_be_turned_off() {
        let off = vec!["updates".to_string(), "connectivity".to_string(), "inventado".to_string()];
        assert!(!is_enabled(&off, "updates"));
        assert!(is_enabled(&off, "speedtest"));
        // Lo que no se puede apagar sigue encendido aunque alguien lo escriba en el archivo de ajustes.
        assert!(is_enabled(&off, "connectivity"));
        assert!(is_enabled(&off, "desconocido"));
    }

    #[test]
    fn the_list_is_complete_and_honest() {
        let l = services(&["vendors".to_string()], &HashMap::from([("speedtest".to_string(), 1_700_000_000)]));
        assert_eq!(l.len(), ENTRIES.len());
        let ids: Vec<&str> = l.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), ids.iter().collect::<std::collections::HashSet<_>>().len(), "sin repetidos");
        assert!(!l.iter().find(|s| s.id == "vendors").unwrap().enabled);
        assert_eq!(l.iter().find(|s| s.id == "speedtest").unwrap().last_used, 1_700_000_000);
        for s in &l {
            assert!(!s.host.is_empty() && !s.what.is_empty() && !s.sends.is_empty() && !s.when.is_empty(), "{} sin explicar", s.id);
        }
        // Cada servicio de red que AdminOps usa por su cuenta está en la lista.
        for id in ["updates", "speedtest", "publicip", "vendors"] {
            assert!(l.iter().any(|s| s.id == id && s.switchable), "{id}");
        }
    }
}
