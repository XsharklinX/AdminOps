//! Redes Wi-Fi guardadas en el equipo, con su contraseña.
//!
//! Usa la API nativa de WLAN (no `netsh wlan export`, que escribe las claves
//! en archivos XML en disco). Windows solo devuelve la clave en claro a un
//! proceso con administrador; sin él la red aparece con la clave protegida.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WifiProfile {
    pub name: String,
    pub ssid: String,
    /// open | WPA2PSK | WPA3SAE | WPA2 (empresa)…
    pub authentication: String,
    /// `None` si la red es abierta o Windows no la dio (sin administrador o red de empresa).
    pub password: Option<String>,
    /// La clave existe pero Windows no la dio en claro.
    pub protected: bool,
    /// Se conecta sola cuando está al alcance.
    pub auto_connect: bool,
    pub connected: bool,
}

/// Contenido del primer `<tag>…</tag>` (sin atributos), con las entidades XML decodificadas.
fn tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))? + start;
    Some(
        xml[start..end]
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

pub fn parse_profile(xml: &str) -> WifiProfile {
    let name = tag(xml, "name").unwrap_or_default();
    let ssid = tag(xml, "SSIDConfig").and_then(|s| tag(&s, "name")).unwrap_or_else(|| name.clone());
    let authentication = tag(xml, "authentication").unwrap_or_default();
    let protected = tag(xml, "protected").as_deref() == Some("true");
    let key = tag(xml, "keyMaterial").filter(|k| !k.is_empty());
    WifiProfile {
        password: if protected { None } else { key.clone() },
        protected: protected && key.is_some(),
        auto_connect: tag(xml, "connectionMode").as_deref() != Some("manual"),
        connected: false,
        authentication,
        ssid,
        name,
    }
}

#[cfg(windows)]
mod native {
    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::NetworkManagement::WiFi::*;

    const ERROR_SERVICE_NOT_ACTIVE: u32 = 1062;

    fn wstr(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// Sesión de WLAN que se cierra sola.
    pub struct Session(HANDLE);

    impl Drop for Session {
        fn drop(&mut self) {
            unsafe { WlanCloseHandle(self.0, std::ptr::null()) };
        }
    }

    impl Session {
        pub fn open() -> Result<Self, String> {
            let mut version = 0;
            let mut handle: HANDLE = std::ptr::null_mut();
            match unsafe { WlanOpenHandle(2, std::ptr::null(), &mut version, &mut handle) } {
                0 => Ok(Session(handle)),
                ERROR_SERVICE_NOT_ACTIVE => Err("Este equipo no tiene Wi-Fi o el servicio de configuración inalámbrica (WLAN AutoConfig) está detenido.".into()),
                e => Err(format!("No se pudo acceder al Wi-Fi (error {e}).")),
            }
        }

        pub fn interfaces(&self) -> Vec<GUID> {
            let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            if unsafe { WlanEnumInterfaces(self.0, std::ptr::null(), &mut list) } != 0 || list.is_null() {
                return vec![];
            }
            let out = unsafe {
                let n = (*list).dwNumberOfItems as usize;
                let first = (*list).InterfaceInfo.as_ptr();
                (0..n).map(|i| (*first.add(i)).InterfaceGuid).collect()
            };
            unsafe { WlanFreeMemory(list.cast()) };
            out
        }

        pub fn connected_profile(&self, guid: &GUID) -> Option<String> {
            let mut size = 0;
            let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut kind = 0;
            let rc = unsafe {
                WlanQueryInterface(self.0, guid, wlan_intf_opcode_current_connection, std::ptr::null(), &mut size, &mut data, &mut kind)
            };
            if rc != 0 || data.is_null() {
                return None;
            }
            let name = unsafe { wstr(&(*(data as *const WLAN_CONNECTION_ATTRIBUTES)).strProfileName) };
            unsafe { WlanFreeMemory(data) };
            Some(name).filter(|n| !n.is_empty())
        }

        pub fn profile_names(&self, guid: &GUID) -> Vec<String> {
            let mut list: *mut WLAN_PROFILE_INFO_LIST = std::ptr::null_mut();
            if unsafe { WlanGetProfileList(self.0, guid, std::ptr::null(), &mut list) } != 0 || list.is_null() {
                return vec![];
            }
            let out = unsafe {
                let n = (*list).dwNumberOfItems as usize;
                let first = (*list).ProfileInfo.as_ptr();
                (0..n).map(|i| wstr(&(*first.add(i)).strProfileName)).collect()
            };
            unsafe { WlanFreeMemory(list.cast()) };
            out
        }

        pub fn profile_xml(&self, guid: &GUID, name: &str) -> Option<String> {
            let name_w = wide(name);
            let mut xml: windows_sys::core::PWSTR = std::ptr::null_mut();
            let mut flags = WLAN_PROFILE_GET_PLAINTEXT_KEY;
            let mut access = 0;
            let rc = unsafe { WlanGetProfile(self.0, guid, name_w.as_ptr(), std::ptr::null(), &mut xml, &mut flags, &mut access) };
            if rc != 0 || xml.is_null() {
                return None;
            }
            let text = unsafe {
                let len = (0..).take_while(|&i| *xml.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(xml, len))
            };
            unsafe { WlanFreeMemory(xml.cast()) };
            Some(text)
        }

        /// Crea o reemplaza un perfil a partir de su XML. Devuelve el código de error (0 = bien).
        pub fn set_profile(&self, guid: &GUID, xml: &str) -> u32 {
            let xml_w = wide(xml);
            let mut reason = 0;
            unsafe { WlanSetProfile(self.0, guid, 0, xml_w.as_ptr(), std::ptr::null(), 1, std::ptr::null(), &mut reason) }
        }

        pub fn delete(&self, guid: &GUID, name: &str) -> u32 {
            let name_w = wide(name);
            unsafe { WlanDeleteProfile(self.0, guid, name_w.as_ptr(), std::ptr::null()) }
        }
    }
}

#[cfg(windows)]
pub fn list() -> Result<Vec<WifiProfile>, String> {
    let s = native::Session::open()?;
    let interfaces = s.interfaces();
    if interfaces.is_empty() {
        return Err("Este equipo no tiene ningún adaptador Wi-Fi.".into());
    }
    let mut out: Vec<WifiProfile> = Vec::new();
    for guid in &interfaces {
        let connected = s.connected_profile(guid);
        for name in s.profile_names(guid) {
            let Some(xml) = s.profile_xml(guid, &name) else { continue };
            let mut p = parse_profile(&xml);
            p.name = name;
            p.connected = connected.as_deref() == Some(p.name.as_str());
            // El mismo perfil puede estar en varios adaptadores.
            match out.iter_mut().find(|x| x.name == p.name) {
                Some(x) => x.connected |= p.connected,
                None => out.push(p),
            }
        }
    }
    out.sort_by_key(|p| (!p.connected, p.name.to_lowercase()));
    Ok(out)
}

/// XML de cada perfil con la clave en claro (con administrador), para copiarlos a otro equipo.
#[cfg(windows)]
pub fn export_all() -> Result<Vec<(String, String)>, String> {
    let s = native::Session::open()?;
    let mut out: Vec<(String, String)> = Vec::new();
    for guid in s.interfaces() {
        for name in s.profile_names(&guid) {
            if out.iter().any(|(n, _)| *n == name) {
                continue;
            }
            if let Some(xml) = s.profile_xml(&guid, &name) {
                out.push((name, xml));
            }
        }
    }
    Ok(out)
}

/// Importa perfiles (XML) en el primer adaptador Wi-Fi. Devuelve cuántos se importaron.
#[cfg(windows)]
pub fn import_all(profiles: &[String]) -> Result<usize, String> {
    let s = native::Session::open()?;
    let guid = s.interfaces().into_iter().next().ok_or("Este equipo no tiene ningún adaptador Wi-Fi.")?;
    Ok(profiles.iter().filter(|xml| s.set_profile(&guid, xml) == 0).count())
}

#[cfg(not(windows))]
pub fn export_all() -> Result<Vec<(String, String)>, String> {
    Err("Solo disponible en Windows.".into())
}

#[cfg(not(windows))]
pub fn import_all(_: &[String]) -> Result<usize, String> {
    Err("Solo disponible en Windows.".into())
}

#[cfg(not(windows))]
pub fn list() -> Result<Vec<WifiProfile>, String> {
    Err("Solo disponible en Windows.".into())
}

#[tauri::command(async)]
pub fn list_wifi_profiles() -> Result<Vec<WifiProfile>, String> {
    list()
}

/// Olvida una red guardada en todos los adaptadores.
#[tauri::command(async)]
pub fn forget_wifi_profile(name: String, tweaks: tauri::State<'_, crate::tweaks::TweakState>) -> Result<(), String> {
    #[cfg(windows)]
    let result = (|| {
        let s = native::Session::open()?;
        let mut deleted = false;
        let mut last = 0;
        for guid in s.interfaces() {
            match s.delete(&guid, &name) {
                0 => deleted = true,
                e => last = e,
            }
        }
        match (deleted, last) {
            (true, _) => Ok(()),
            (false, 5) => Err("Acceso denegado: ejecuta AdminOps como administrador.".to_string()),
            _ => Err(format!("No se encontró la red «{name}».")),
        }
    })();
    #[cfg(not(windows))]
    let result: Result<(), String> = Err("Solo disponible en Windows.".into());
    tweaks.record(crate::tweaks::journal::Op::Run, &format!("Wi-Fi: olvidar la red «{name}»"), &result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
	<name>Casa &amp; Oficina</name>
	<SSIDConfig><SSID><hex>43617361</hex><name>Casa &amp; Oficina</name></SSID></SSIDConfig>
	<connectionType>ESS</connectionType>
	<connectionMode>auto</connectionMode>
	<MSM><security>
		<authEncryption><authentication>WPA2PSK</authentication><encryption>AES</encryption></authEncryption>
		<sharedKey><keyType>passPhrase</keyType><protected>false</protected><keyMaterial>clave&lt;segura&gt;</keyMaterial></sharedKey>
	</security></MSM>
</WLANProfile>"#;

    #[test]
    fn parses_profiles() {
        let p = parse_profile(XML);
        assert_eq!(p.name, "Casa & Oficina");
        assert_eq!(p.ssid, "Casa & Oficina");
        assert_eq!(p.authentication, "WPA2PSK");
        assert_eq!(p.password.as_deref(), Some("clave<segura>"));
        assert!(!p.protected && p.auto_connect);

        let locked = parse_profile(&XML.replace("<protected>false", "<protected>true").replace("clave&lt;segura&gt;", "01000000D08C9DDF"));
        assert_eq!(locked.password, None);
        assert!(locked.protected);

        let open = parse_profile(&XML.replace("WPA2PSK", "open").replace("<connectionMode>auto", "<connectionMode>manual"));
        assert!(!open.auto_connect);
    }

    /// Solo lectura. En equipos sin Wi-Fi devuelve el error explicativo.
    /// Equipo real (depende de su estado): `cargo test lists_real_profiles -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn lists_real_profiles() {
        match list() {
            Ok(v) => println!("{} redes", v.len()),
            Err(e) => println!("sin Wi-Fi: {e}"),
        }
    }
}
