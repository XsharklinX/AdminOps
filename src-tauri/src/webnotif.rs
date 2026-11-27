//! Avisos falsos de virus: webs a las que el navegador dejó mandar notificaciones a Windows. Las que
//! fingen alertas de virus o premios llegan ahí cuando alguien pulsó «Permitir» en una página dudosa.
//!
//! Solo navegadores Chromium (Chrome, Edge, Brave y Opera): los permisos están en el JSON de
//! `Preferences` de cada perfil. Firefox los guarda en `permissions.sqlite` y el proyecto no tiene
//! ninguna biblioteca de SQLite (y no se añade solo para esto): se deja fuera.
//!
//! Quitar un aviso = ponerlo en «bloqueado» (así la web no vuelve a pedirlo). Con el navegador
//! cerrado: abierto, al cerrarse reescribe sus ajustes y deshace el cambio.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use tauri::State;

/// Un navegador y dónde guarda sus datos.
#[derive(Debug, Clone, Copy)]
pub struct Browser {
    /// Para la lista de «A fondo» del cohete.
    pub id: &'static str,
    /// Lo que ve el usuario («Chrome»); también identifica al navegador en `web_notifications_remove`.
    pub label: &'static str,
    pub exe: &'static str,
    /// Carpeta de datos, desde AppData\Local (o AppData\Roaming si `roaming`).
    pub rel: &'static str,
    pub roaming: bool,
}

pub const CHROMIUM: [Browser; 4] = [
    Browser { id: "chrome", label: "Chrome", exe: "chrome.exe", rel: r"Google\Chrome\User Data", roaming: false },
    Browser { id: "edge", label: "Edge", exe: "msedge.exe", rel: r"Microsoft\Edge\User Data", roaming: false },
    Browser { id: "brave", label: "Brave", exe: "brave.exe", rel: r"BraveSoftware\Brave-Browser\User Data", roaming: false },
    // Opera guarda sus Preferences directamente en la carpeta, sin perfiles aparte.
    Browser { id: "opera", label: "Opera", exe: "opera.exe", rel: r"Opera Software\Opera Stable", roaming: true },
];

impl Browser {
    pub fn root(&self) -> Option<PathBuf> {
        let u = crate::target_user::user_dirs()?;
        Some(if self.roaming { u.app_data } else { u.local_app_data }.join(self.rel))
    }

    /// Los perfiles que existen: (nombre, carpeta). Opera es uno solo, «Principal».
    pub fn profiles(&self) -> Vec<(String, PathBuf)> {
        let Some(root) = self.root() else { return vec![] };
        if self.roaming {
            return if root.join("Preferences").is_file() { vec![("Principal".to_string(), root)] } else { vec![] };
        }
        let Ok(rd) = std::fs::read_dir(&root) else { return vec![] };
        let mut v: Vec<(String, PathBuf)> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                (is_profile_dir(&name) && e.path().is_dir()).then(|| (name, e.path()))
            })
            .collect();
        v.sort();
        v
    }
}

/// «Default» o «Profile 3».
fn is_profile_dir(name: &str) -> bool {
    name == "Default" || name.strip_prefix("Profile ").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Nombres (en minúsculas) de los programas en marcha.
pub fn running_exes() -> HashSet<String> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().map(|p| p.name().to_string_lossy().to_ascii_lowercase()).collect()
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WebNotif {
    /// «Chrome», «Edge», «Brave» u «Opera».
    pub browser: String,
    pub profile: String,
    /// La clave tal como la guarda el navegador («https://sitio.com:443,*»): sirve para quitarla.
    pub origin: String,
    /// Solo el sitio («sitio.com»), para enseñarlo.
    pub host: String,
    pub suspicious: bool,
    pub trusted: bool,
}

/// «https://sub.sitio.com:443,*» o «[*.]sitio.com,*» → «sub.sitio.com» / «sitio.com».
pub fn host_of(key: &str) -> String {
    let origin = key.split(',').next().unwrap_or(key);
    let rest = origin.split_once("://").map_or(origin, |(_, r)| r);
    let rest = rest.trim_start_matches("[*.]").split('/').next().unwrap_or("");
    // Sin el puerto (y sin romper una dirección IPv6 entre corchetes).
    let host = match rest.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => rest.split(':').next().unwrap_or(""),
    };
    host.to_ascii_lowercase()
}

const CHEAP_TLDS: [&str; 13] = ["xyz", "top", "click", "live", "online", "site", "buzz", "icu", "rest", "monster", "cfd", "sbs", "bond"];
const BAIT_WORDS: [&str; 12] = ["virus", "clean", "security-alert", "prize", "premio", "win", "captcha", "robot", "verify", "push", "notify", "allow"];
const TRUSTED: [&str; 16] = [
    "office.com", "office365.com", "outlook.com", "live.com", "microsoft.com", "microsoftonline.com", "google.com", "gmail.com", "whatsapp.com", "zoom.us", "zoom.com", "slack.com",
    "teams.microsoft.com", "dgii.gov.do", "gob.do", "gov.do",
];

/// ¿Es de una lista corta de sitios conocidos (el dominio o uno de sus subdominios)?
pub fn trusted(host: &str) -> bool {
    let h = host.trim_end_matches('.');
    TRUSTED.iter().any(|d| h == *d || h.strip_suffix(d).is_some_and(|p| p.ends_with('.')))
}

/// Heurística de «parece un aviso falso»: dominio de primer nivel barato, palabras de gancho o un
/// subdominio de solo números. Los de confianza nunca lo son.
pub fn suspicious(host: &str) -> bool {
    let h = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if h.is_empty() || trusted(&h) {
        return false;
    }
    let labels: Vec<&str> = h.split('.').collect();
    if labels.len() > 1 && labels.last().is_some_and(|t| CHEAP_TLDS.contains(t)) {
        return true;
    }
    if BAIT_WORDS.iter().any(|w| h.contains(w)) {
        return true;
    }
    // Una etiqueta (que no sea la última) de solo números y larga: «83921746.sitio.com».
    labels.len() > 1 && labels[..labels.len() - 1].iter().any(|l| l.len() >= 5 && l.bytes().all(|b| b.is_ascii_digit()))
}

/// Las claves de `profile.content_settings.exceptions.notifications` con su valor de `setting`.
pub fn notification_entries(prefs: &Value) -> Vec<(String, i64)> {
    let Some(map) = prefs.pointer("/profile/content_settings/exceptions/notifications").and_then(Value::as_object) else { return vec![] };
    map.iter().filter_map(|(k, v)| Some((k.clone(), v.get("setting")?.as_i64()?))).collect()
}

/// Pone en «bloqueado» (2) las claves dadas. Devuelve cuántas cambió; conserva todo lo demás.
pub fn block_keys(prefs: &mut Value, keys: &[String]) -> u32 {
    let Some(map) = prefs.pointer_mut("/profile/content_settings/exceptions/notifications").and_then(Value::as_object_mut) else { return 0 };
    let mut n = 0;
    for k in keys {
        if let Some(obj) = map.get_mut(k).and_then(Value::as_object_mut) {
            if obj.get("setting").and_then(Value::as_i64) != Some(2) {
                obj.insert("setting".into(), Value::from(2));
                n += 1;
            }
        }
    }
    n
}

fn read_prefs(dir: &std::path::Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(dir.join("Preferences")).ok()?).ok()
}

/// Las webs con permiso para mandar avisos, en todos los navegadores y perfiles.
#[tauri::command(async)]
pub fn web_notifications() -> Vec<WebNotif> {
    let mut out = Vec::new();
    for b in CHROMIUM {
        for (profile, dir) in b.profiles() {
            let Some(prefs) = read_prefs(&dir) else { continue };
            for (key, setting) in notification_entries(&prefs) {
                if setting != 1 {
                    continue;
                }
                let host = host_of(&key);
                if host.is_empty() {
                    continue;
                }
                let trusted = trusted(&host);
                out.push(WebNotif { browser: b.label.into(), profile: profile.clone(), origin: key, suspicious: suspicious(&host), trusted, host });
            }
        }
    }
    // Primero las sospechosas, luego el resto por sitio.
    out.sort_by(|a, b| b.suspicious.cmp(&a.suspicious).then_with(|| a.host.cmp(&b.host)).then_with(|| a.browser.cmp(&b.browser)));
    out
}

/// Bloquea los avisos de esas webs. Cada elemento: (navegador, perfil, clave tal como la dio `web_notifications`).
#[tauri::command(async)]
pub fn web_notifications_remove(tweaks: State<'_, TweakState>, items: Vec<(String, String, String)>) -> Result<u32, String> {
    let r = remove(&items);
    if let Ok(n) = &r {
        if *n > 0 {
            let title = if *n == 1 { "Bloquear 1 aviso de una web".to_string() } else { format!("Bloquear {n} avisos de webs") };
            tweaks.record(Op::Run, &title, &Ok::<(), String>(()));
        }
    }
    r
}

fn remove(items: &[(String, String, String)]) -> Result<u32, String> {
    // Los navegadores implicados, para comprobar que ninguno está abierto antes de tocar nada.
    let mut browsers: Vec<&Browser> = Vec::new();
    for (name, _, _) in items {
        let b = CHROMIUM.iter().find(|b| b.label == name).ok_or("Ese navegador no es compatible.")?;
        if !browsers.iter().any(|x| x.id == b.id) {
            browsers.push(b);
        }
    }
    let running = running_exes();
    for b in &browsers {
        if running.contains(b.exe) {
            return Err(format!("Cierra {} para quitar los avisos (si no, al cerrarse vuelve a escribir sus ajustes)", b.label));
        }
    }
    let mut total = 0;
    for b in browsers {
        let profiles = b.profiles();
        // Los perfiles se buscan en la lista real: el nombre que llega no se usa como ruta.
        for (pname, dir) in &profiles {
            let keys: Vec<String> = items.iter().filter(|(n, p, _)| n == b.label && p == pname).map(|(_, _, k)| k.clone()).collect();
            if keys.is_empty() {
                continue;
            }
            let file = dir.join("Preferences");
            let raw = std::fs::read(&file).map_err(|e| format!("No se pudo leer los ajustes de {}: {e}", b.label))?;
            let mut prefs: Value = serde_json::from_slice(&raw).map_err(|_| format!("Los ajustes de {} no se pudieron leer.", b.label))?;
            let n = block_keys(&mut prefs, &keys);
            if n == 0 {
                continue;
            }
            let out = serde_json::to_vec(&prefs).map_err(|e| e.to_string())?;
            // Copia de seguridad del archivo tal como estaba, antes de escribir nada.
            std::fs::write(dir.join("Preferences.adminops-bak"), &raw).map_err(|e| format!("No se pudo hacer la copia de seguridad: {e}"))?;
            std::fs::write(&file, out).map_err(|e| format!("No se pudo guardar los ajustes de {}: {e}", b.label))?;
            total += n;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitio_de_la_clave() {
        assert_eq!(host_of("https://Sitio.com:443,*"), "sitio.com");
        assert_eq!(host_of("[*.]ejemplo.org,*"), "ejemplo.org");
        assert_eq!(host_of("https://a.b.com,*"), "a.b.com");
        assert_eq!(host_of("http://localhost:8080,*"), "localhost");
    }

    #[test]
    fn sospechosas_y_de_confianza() {
        assert!(suspicious("alerta-premio.xyz"));
        assert!(suspicious("clean-your-pc.com"));
        assert!(suspicious("virus-detected.net"));
        assert!(suspicious("83921746.sitio.com"));
        assert!(suspicious("tu-captcha.live"));
        assert!(!suspicious("sitio.com"));
        assert!(!suspicious("12.sitio.com"));
        // De confianza: nunca sospechosas, aunque lleven una palabra de gancho.
        assert!(trusted("outlook.office.com") && !suspicious("outlook.office.com"));
        assert!(trusted("teams.microsoft.com"));
        assert!(trusted("dgii.gov.do"));
        assert!(trusted("web.whatsapp.com"));
        // Un dominio que solo contiene el nombre no es de confianza.
        assert!(!trusted("microsoft-virus-alert.xyz"));
        assert!(!trusted("fakegoogle.com"));
        assert!(suspicious("microsoft-virus-alert.xyz"));
    }

    #[test]
    fn leer_y_bloquear() {
        let mut v: Value = serde_json::from_str(
            r#"{"x":1,"profile":{"name":"A","content_settings":{"exceptions":{"notifications":{
                "https://bueno.com:443,*":{"setting":1,"last_modified":"1"},
                "https://malo.xyz:443,*":{"setting":1},
                "https://no.com:443,*":{"setting":2}}}}}}"#,
        )
        .unwrap();
        let mut e = notification_entries(&v);
        e.sort();
        assert_eq!(e.len(), 3);
        assert_eq!(e.iter().filter(|(_, s)| *s == 1).count(), 2);
        let n = block_keys(&mut v, &["https://malo.xyz:443,*".to_string(), "https://no.com:443,*".to_string(), "no existe".to_string()]);
        // La ya bloqueada y la que no existe no cuentan.
        assert_eq!(n, 1);
        assert_eq!(v["profile"]["content_settings"]["exceptions"]["notifications"]["https://malo.xyz:443,*"]["setting"], 2);
        assert_eq!(v["profile"]["content_settings"]["exceptions"]["notifications"]["https://bueno.com:443,*"]["last_modified"], "1");
        assert_eq!(v["x"], 1);
        assert_eq!(v["profile"]["name"], "A");
        assert!(notification_entries(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn carpetas_de_perfil() {
        assert!(is_profile_dir("Default") && is_profile_dir("Profile 12"));
        assert!(!is_profile_dir("Profile ") && !is_profile_dir("Profile x") && !is_profile_dir("System Profile") && !is_profile_dir("Guest Profile"));
    }
}
