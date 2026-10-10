//! Navegador secuestrado o lento: Chrome, Edge, Brave y Firefox a la vez.
//! Extensiones que no vienen de la tienda o que puso un programa, buscador y
//! página de inicio cambiados, directivas que bloquean ajustes («Administrado
//! por tu organización» en un equipo que no es de empresa) y perfiles enormes.
//! Se lee todo desde aquí (sin PowerShell): los archivos de preferencias del
//! navegador y el registro.

use crate::troubleshoot::{finding, fix, fix_confirm, Finding};
use crate::tweaks::registry;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Buscadores conocidos: cualquier otro como predeterminado es sospechoso.
const KNOWN_ENGINES: &[&str] = &["google.", "bing.com", "duckduckgo.com", "ecosia.org", "yahoo.", "yandex.", "startpage.com", "search.brave.com", "qwant.com", "ask.com", "baidu.com", "naver.com", "perplexity.ai"];

#[derive(Debug, Clone, PartialEq)]
pub struct Ext {
    pub id: String,
    pub name: String,
    pub location: i64,
    pub from_store: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ProfileInfo {
    pub browser: String,
    pub profile: String,
    pub search_url: String,
    pub start_urls: Vec<String>,
    pub homepage: String,
    pub extensions: Vec<Ext>,
    pub size: u64,
}

/// Host de una URL («https://www.google.es/search?q=» → «www.google.es»).
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.split(['/', '?', '#', ':']).next().unwrap_or("").to_ascii_lowercase()
}

/// ¿Es un buscador conocido?
pub fn known_engine(url: &str) -> bool {
    let h = host_of(url);
    h.is_empty() || KNOWN_ENGINES.iter().any(|k| h.contains(k))
}

/// Por qué una extensión es sospechosa, si lo es (por dónde llegó al navegador).
pub fn ext_concern(e: &Ext) -> Option<&'static str> {
    match e.location {
        5 | 10 => None,                                           // componentes del propio navegador
        2 | 3 | 6 => Some("la instaló un programa, no la persona"), // preferencias externas o registro
        7 | 9 => Some("la impone una directiva"),
        4 | 8 => Some("cargada a mano, sin pasar por la tienda"),
        1 if !e.from_store => Some("no viene de la tienda"),
        _ => None,
    }
}

/// Lee lo que importa de las preferencias de un perfil de Chrome, Edge o Brave.
pub fn chromium_profile(prefs: &Value, secure: &Value) -> ProfileInfo {
    let mut p = ProfileInfo {
        search_url: prefs.pointer("/default_search_provider_data/template_url_data/url").and_then(Value::as_str).unwrap_or("").to_string(),
        homepage: prefs.get("homepage").and_then(Value::as_str).unwrap_or("").to_string(),
        ..Default::default()
    };
    if prefs.pointer("/session/restore_on_startup").and_then(Value::as_i64) == Some(4) {
        p.start_urls = prefs.pointer("/session/startup_urls").and_then(Value::as_array).map(|a| a.iter().filter_map(|u| u.as_str().map(String::from)).collect()).unwrap_or_default();
    }
    // La lista de extensiones está en «Secure Preferences» (o, en versiones viejas, en «Preferences»).
    for src in [secure, prefs] {
        if let Some(map) = src.pointer("/extensions/settings").and_then(Value::as_object) {
            for (id, s) in map {
                if p.extensions.iter().any(|e| &e.id == id) {
                    continue;
                }
                let name = s.pointer("/manifest/name").and_then(Value::as_str).unwrap_or("").to_string();
                p.extensions.push(Ext { id: id.clone(), name, location: s.get("location").and_then(Value::as_i64).unwrap_or(1), from_store: s.get("from_webstore").and_then(Value::as_bool).unwrap_or(false) });
            }
        }
    }
    p
}

/// Nombre de la extensión desde su manifest (si las preferencias no lo dicen).
fn manifest_name(profile: &Path, id: &str) -> Option<String> {
    let dir = profile.join("Extensions").join(id);
    let ver = std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).max()?;
    let m: Value = serde_json::from_str(&std::fs::read_to_string(ver.join("manifest.json")).ok()?).ok()?;
    let name = m.get("name")?.as_str()?.to_string();
    if name.starts_with("__MSG_") {
        let key = name.trim_start_matches("__MSG_").trim_end_matches("__");
        let loc = m.get("default_locale").and_then(Value::as_str).unwrap_or("en");
        let msgs: Value = serde_json::from_str(&std::fs::read_to_string(ver.join("_locales").join(loc).join("messages.json")).ok()?).ok()?;
        let obj = msgs.as_object()?;
        return obj.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).and_then(|(_, v)| v.get("message")?.as_str().map(String::from));
    }
    Some(name)
}

fn read_json(p: &Path) -> Value {
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
}

fn dir_size(p: &Path, budget: &mut usize) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    let mut total = 0;
    for e in rd.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        match e.file_type() {
            Ok(t) if t.is_dir() => total += dir_size(&e.path(), budget),
            Ok(_) => total += e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => {}
        }
    }
    total
}

/// Navegadores basados en Chromium: nombre, carpeta de datos, ejecutable y clave de directivas.
pub const CHROMIUM: &[(&str, &str, &str, &str)] = &[
    ("Chrome", r"Google\Chrome\User Data", "chrome.exe", r"SOFTWARE\Policies\Google\Chrome"),
    ("Edge", r"Microsoft\Edge\User Data", "msedge.exe", r"SOFTWARE\Policies\Microsoft\Edge"),
    ("Brave", r"BraveSoftware\Brave-Browser\User Data", "brave.exe", r"SOFTWARE\Policies\BraveSoftware\Brave"),
];

fn profiles() -> Vec<(ProfileInfo, PathBuf)> {
    let Some(d) = crate::target_user::user_dirs() else { return vec![] };
    let mut out = Vec::new();
    for (browser, data, _, _) in CHROMIUM {
        let root = d.local_app_data.join(data);
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name != "Default" && !name.starts_with("Profile ") {
                continue;
            }
            let dir = e.path();
            let mut p = chromium_profile(&read_json(&dir.join("Preferences")), &read_json(&dir.join("Secure Preferences")));
            for x in p.extensions.iter_mut().filter(|x| x.name.is_empty()) {
                x.name = manifest_name(&dir, &x.id).unwrap_or_else(|| x.id.clone());
            }
            p.browser = browser.to_string();
            p.profile = name;
            let mut budget = 40_000;
            p.size = dir_size(&dir, &mut budget);
            out.push((p, dir));
        }
    }
    // Firefox: extensiones de fuera del perfil y página de inicio.
    if let Ok(rd) = std::fs::read_dir(d.app_data.join(r"Mozilla\Firefox\Profiles")) {
        for e in rd.flatten() {
            let dir = e.path();
            let mut p = ProfileInfo { browser: "Firefox".into(), profile: e.file_name().to_string_lossy().into_owned(), ..Default::default() };
            let j = read_json(&dir.join("extensions.json"));
            for a in j.get("addons").and_then(Value::as_array).into_iter().flatten() {
                if a.get("type").and_then(Value::as_str) != Some("extension") {
                    continue;
                }
                let loc = a.get("location").and_then(Value::as_str).unwrap_or("");
                if loc.starts_with("app-system") || loc == "app-builtin" {
                    continue;
                }
                let name = a.pointer("/defaultLocale/name").and_then(Value::as_str).unwrap_or("").to_string();
                // app-profile: la puso la persona; el resto (registro, carpeta global): un programa.
                p.extensions.push(Ext { id: a.get("id").and_then(Value::as_str).unwrap_or("").into(), name, location: if loc == "app-profile" { 1 } else { 3 }, from_store: true });
            }
            if let Ok(prefs) = std::fs::read_to_string(dir.join("prefs.js")) {
                p.homepage = firefox_pref(&prefs, "browser.startup.homepage").unwrap_or_default();
            }
            let mut budget = 40_000;
            p.size = dir_size(&dir, &mut budget);
            out.push((p, dir));
        }
    }
    out
}

/// Valor de una preferencia de Firefox en prefs.js.
pub fn firefox_pref(prefs: &str, name: &str) -> Option<String> {
    let needle = format!("user_pref(\"{name}\", \"");
    let start = prefs.find(&needle)? + needle.len();
    let end = prefs[start..].find("\");")?;
    Some(prefs[start..start + end].to_string())
}

/// ¿El equipo lo gestiona una empresa (dominio o MDM)? Ahí las directivas son legítimas.
pub fn managed_machine() -> bool {
    registry::read_string(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Group Policy\State\Machine", "Distinguished-Name").is_some_and(|s| !s.is_empty())
        || registry::subkeys(r"HKLM\SOFTWARE\Microsoft\Enrollments").iter().any(|k| registry::read_string(&format!(r"HKLM\SOFTWARE\Microsoft\Enrollments\{k}"), "ProviderID").is_some_and(|p| p.contains("MS DM")))
}

/// Directivas del navegador (HKLM y HKCU) que se notan: buscador, inicio, extensiones forzadas, proxy.
fn policies(key: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for hive in ["HKLM", "HKCU"] {
        let base = format!(r"{hive}\{key}");
        if !registry::key_exists(&base) {
            continue;
        }
        for (name, raw) in registry::values(&base) {
            let v = registry::raw_to_string(&raw).unwrap_or_else(|| "(valor)".into());
            out.push((format!("{hive}: {name}"), v));
        }
        for sub in registry::subkeys(&base) {
            out.push((format!("{hive}: {sub}"), "(lista)".into()));
        }
    }
    out
}

/// Entradas del registro con las que un programa instala extensiones en Chrome o Edge.
fn external_entries() -> Vec<String> {
    let mut out = Vec::new();
    for base in [r"HKLM\SOFTWARE\Google\Chrome\Extensions", r"HKLM\SOFTWARE\WOW6432Node\Google\Chrome\Extensions", r"HKCU\SOFTWARE\Google\Chrome\Extensions", r"HKLM\SOFTWARE\Microsoft\Edge\Extensions", r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Edge\Extensions", r"HKCU\SOFTWARE\Microsoft\Edge\Extensions"] {
        for id in registry::subkeys(base) {
            out.push(format!(r"{base}\{id}"));
        }
    }
    out
}

pub fn findings(profiles: &[ProfileInfo], pol: &[(String, Vec<(String, String)>)], managed: bool, external: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for p in profiles {
        let who = if p.profile == "Default" || p.browser == "Firefox" { p.browser.clone() } else { format!("{} ({})", p.browser, p.profile) };
        if !p.search_url.is_empty() && !known_engine(&p.search_url) {
            out.push(finding("bad", format!("{who}: el buscador es «{}»", host_of(&p.search_url)), "No es un buscador conocido: suele cambiarlo un programa instalado de regalo con otro. Restablece la configuración del navegador.").fixes(vec![fix(format!("br.reset:{}", p.browser), &format!("Restablecer {}", p.browser), false)]));
        }
        let odd_start: Vec<String> = p.start_urls.iter().chain(std::iter::once(&p.homepage)).filter(|u| !u.is_empty() && !known_engine(u) && !u.starts_with("chrome://") && !u.starts_with("edge://") && !u.starts_with("about:")).map(|u| host_of(u)).collect();
        if !odd_start.is_empty() {
            out.push(finding("warn", format!("{who}: se abre con {}", odd_start.join(", ")), "Si nadie la eligió, la puso un programa. Restablecer el navegador la quita sin borrar marcadores ni contraseñas.").fixes(vec![fix(format!("br.reset:{}", p.browser), &format!("Restablecer {}", p.browser), false)]));
        }
        for e in &p.extensions {
            if let Some(why) = ext_concern(e) {
                let name = if e.name.is_empty() { &e.id } else { &e.name };
                out.push(finding("warn", format!("{who}: extensión «{name}»"), format!("{why}. Si no la reconoces, quítala desde la página de extensiones del navegador.")));
            }
        }
        if p.size > 3 * 1024 * 1024 * 1024 {
            out.push(finding("info", format!("{who}: el perfil ocupa {}", super::gb(p.size)), "Un perfil muy grande hace el navegador lento. Vaciar la caché no borra contraseñas, marcadores ni historial.").fixes(vec![fix_confirm(format!("br.cache:{}", p.browser), "Vaciar la caché", false, &format!("Se cerrará {} y se vaciará su caché. No se borran contraseñas, marcadores ni historial.", p.browser))]));
        }
    }
    for (browser, list) in pol {
        if list.is_empty() {
            continue;
        }
        let shown: Vec<String> = list.iter().take(5).map(|(k, v)| format!("{k} = {v}")).collect();
        if managed {
            out.push(finding("info", format!("{browser}: directivas de la empresa"), format!("El equipo está en un dominio o gestionado: «Administrado por tu organización» es normal. {}", shown.join(" · "))));
        } else {
            out.push(
                finding("bad", format!("{browser}: «Administrado por tu organización» sin ser de empresa"), format!("Este equipo no está en un dominio ni gestionado, pero tiene directivas que bloquean ajustes del navegador (lo hacen algunos programas no deseados): {}", shown.join(" · ")))
                    .fixes(vec![fix_confirm(format!("br.policies:{browser}"), "Quitar las directivas", true, "Se guarda una copia de las directivas y se borran. El navegador recupera sus ajustes al volver a abrirlo.")]),
            );
        }
    }
    if !external.is_empty() && !managed {
        out.push(
            finding("warn", format!("{} extensión(es) instaladas desde el registro", external.len()), "Un programa las añadió al navegador y vuelve a ponerlas aunque se quiten desde el navegador. Quitar la entrada lo evita.")
                .fixes(external.iter().take(3).map(|k| fix_confirm(format!("br.extreg:{k}"), &format!("Quitar {}", k.rsplit('\\').next().unwrap_or(k)), k.starts_with("HKLM"), "Se guarda una copia de la entrada y se borra. Después quita la extensión también desde el navegador.")).collect()),
        );
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        let names: Vec<&str> = profiles.iter().map(|p| p.browser.as_str()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        out.insert(0, finding("ok", "Los navegadores no muestran nada raro", format!("Revisados: {}. Buscador, página de inicio, extensiones y directivas en orden.", if names.is_empty() { "ninguno instalado".to_string() } else { names.join(", ") })));
    }
    out
}

pub fn check() -> Result<Vec<Finding>, String> {
    let profiles = profiles();
    let mut pol: Vec<(String, Vec<(String, String)>)> = CHROMIUM.iter().map(|(b, _, _, key)| (b.to_string(), policies(key))).collect();
    pol.push(("Firefox".into(), policies(r"SOFTWARE\Policies\Mozilla\Firefox")));
    let list: Vec<ProfileInfo> = profiles.into_iter().map(|(p, _)| p).collect();
    Ok(findings(&list, &pol, managed_machine(), &external_entries()))
}

fn exe_path(browser: &str) -> Option<String> {
    let exe = CHROMIUM.iter().find(|(b, _, _, _)| *b == browser)?.2;
    registry::read_string(&format!(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}"), "").or_else(|| registry::read_string(&format!(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}"), ""))
}

fn reset(browser: &str) -> Result<String, String> {
    let url = match browser {
        "Chrome" | "Brave" => "chrome://settings/resetProfileSettings",
        "Edge" => "edge://settings/resetProfileSettings",
        _ => return Err("En Firefox: Ayuda → Más información para solucionar problemas → «Restablecer Firefox».".into()),
    };
    let exe = exe_path(browser).ok_or_else(|| format!("No se encuentra {browser}."))?;
    crate::shellopen::run_as_user(&exe, url)?;
    Ok(format!("{browser} abierto en «Restablecer configuración»: pulsa «Restablecer». No se borran marcadores ni contraseñas."))
}

fn remove_policies(browser: &str) -> Result<String, String> {
    if managed_machine() {
        return Err("El equipo lo gestiona una empresa: sus directivas no se quitan desde aquí.".into());
    }
    let key = if browser == "Firefox" { r"SOFTWARE\Policies\Mozilla\Firefox" } else { CHROMIUM.iter().find(|(b, _, _, _)| *b == browser).ok_or("Navegador desconocido.")?.3 };
    let mut done = 0;
    for hive in ["HKLM", "HKCU"] {
        let path = format!(r"{hive}\{key}");
        if registry::key_exists(&path) {
            let copy = super::reg_backup(&path, &format!("directivas-{}-{}", browser.to_lowercase(), hive.to_lowercase()))?;
            registry::delete_tree(&path)?;
            log::info!("Directivas de {browser} ({hive}) borradas; copia en {}", copy.display());
            done += 1;
        }
    }
    Ok(if done > 0 { format!("Directivas de {browser} quitadas (hay copia en la carpeta de datos de AdminOps, «copias-registro»). Cierra y abre {browser}.") } else { "No había directivas que quitar.".into() })
}

/// Solo entradas de extensiones externas de Chrome o Edge.
pub fn valid_external(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    (k.starts_with("hklm\\") || k.starts_with("hkcu\\")) && (k.contains("\\google\\chrome\\extensions\\") || k.contains("\\microsoft\\edge\\extensions\\")) && !k.contains("..")
}

fn remove_external(key: &str) -> Result<String, String> {
    if !valid_external(key) {
        return Err("Esa clave no es de una extensión del navegador.".into());
    }
    super::reg_backup(key, "extension-externa")?;
    registry::delete_tree(key)?;
    Ok("Entrada quitada (hay copia). Quita también la extensión desde la página de extensiones del navegador.".into())
}

fn clear_cache(browser: &str) -> Result<String, String> {
    let d = crate::target_user::user_dirs().ok_or("No se encuentra el perfil del usuario.")?;
    let (exe, roots): (&str, Vec<PathBuf>) = if browser == "Firefox" {
        ("firefox.exe", std::fs::read_dir(d.local_app_data.join(r"Mozilla\Firefox\Profiles")).map(|rd| rd.flatten().map(|e| e.path().join("cache2")).collect()).unwrap_or_default())
    } else {
        let (_, data, exe, _) = CHROMIUM.iter().find(|(b, _, _, _)| *b == browser).ok_or("Navegador desconocido.")?;
        let root = d.local_app_data.join(data);
        let profiles: Vec<PathBuf> = std::fs::read_dir(&root).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.join("Preferences").is_file()).collect()).unwrap_or_default();
        (exe, profiles.iter().flat_map(|p| ["Cache", "Code Cache", "GPUCache", r"Service Worker\CacheStorage"].map(|s| p.join(s))).collect())
    };
    let _ = crate::ps::exec("taskkill.exe", &["/IM", exe, "/F"]);
    std::thread::sleep(std::time::Duration::from_millis(1200));
    let mut freed = 0;
    for r in roots {
        let mut budget = 300_000;
        freed += dir_size(&r, &mut budget);
        let _ = std::fs::remove_dir_all(&r);
    }
    Ok(format!("Caché de {browser} vaciada ({}).", super::gb(freed)))
}

pub fn run(kind: &str, arg: &str) -> Option<Result<String, String>> {
    Some(match kind {
        "br.reset" => reset(arg),
        "br.policies" => remove_policies(arg),
        "br.extreg" => remove_external(arg),
        "br.cache" => clear_cache(arg),
        _ => return None,
    })
}

pub fn title(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "br.policies" => "Navegador: quitar directivas puestas por un programa",
        "br.extreg" => "Navegador: quitar una extensión instalada desde el registro",
        "br.cache" => "Navegador: vaciar la caché",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn buscadores_conocidos() {
        assert!(known_engine("https://www.google.es/search?q={searchTerms}"));
        assert!(known_engine("https://www.bing.com/search?q=x"));
        assert!(!known_engine("https://search-nova.xyz/?q={searchTerms}"));
        assert_eq!(host_of("http://Ejemplo.COM:8080/a"), "ejemplo.com");
    }

    #[test]
    fn extensiones_sospechosas() {
        let e = |location, from_store| Ext { id: "x".into(), name: "X".into(), location, from_store };
        assert!(ext_concern(&e(1, true)).is_none());
        assert!(ext_concern(&e(5, false)).is_none());
        assert!(ext_concern(&e(3, false)).unwrap().contains("programa"));
        assert!(ext_concern(&e(4, false)).is_some());
        assert!(ext_concern(&e(1, false)).is_some());
    }

    #[test]
    fn lee_las_preferencias_de_chrome() {
        let prefs = json!({
            "default_search_provider_data": { "template_url_data": { "url": "https://search-nova.xyz/?q={searchTerms}" } },
            "homepage": "https://inicio-raro.example/",
            "session": { "restore_on_startup": 4, "startup_urls": ["https://inicio-raro.example/"] }
        });
        let secure = json!({ "extensions": { "settings": {
            "aaa": { "location": 1, "from_webstore": true, "manifest": { "name": "uBlock" } },
            "bbb": { "location": 3, "manifest": { "name": "PDF Converter Pro" } }
        } } });
        let mut p = chromium_profile(&prefs, &secure);
        p.browser = "Chrome".into();
        p.profile = "Default".into();
        assert_eq!(p.extensions.len(), 2);
        let f = findings(&[p], &[], false, &[]);
        assert!(f.iter().any(|x| x.level == "bad" && x.title.contains("search-nova.xyz")), "{f:?}");
        assert!(f.iter().any(|x| x.title.contains("PDF Converter Pro")));
        assert!(!f.iter().any(|x| x.title.contains("uBlock")));
    }

    #[test]
    fn directivas_segun_el_equipo() {
        let pol = vec![("Chrome".to_string(), vec![("HKLM: DefaultSearchProviderSearchURL".to_string(), "https://x.example".to_string())])];
        assert!(findings(&[], &pol, false, &[]).iter().any(|f| f.level == "bad" && !f.fixes.is_empty()));
        assert!(findings(&[], &pol, true, &[]).iter().all(|f| f.level != "bad"));
    }

    #[test]
    fn preferencias_de_firefox() {
        let prefs = "user_pref(\"browser.startup.homepage\", \"https://raro.example\");\nuser_pref(\"x\", 1);";
        assert_eq!(firefox_pref(prefs, "browser.startup.homepage").as_deref(), Some("https://raro.example"));
        assert_eq!(firefox_pref(prefs, "nada"), None);
    }

    #[test]
    fn solo_toca_extensiones_externas() {
        assert!(valid_external(r"HKLM\SOFTWARE\Google\Chrome\Extensions\abcdef"));
        assert!(!valid_external(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"));
    }
}
