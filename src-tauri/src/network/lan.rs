//! Mi red: la conexión actual, el router (acceso a su panel, credenciales
//! guardadas cifradas, chequeo de seguridad, doble NAT) y los dispositivos
//! conectados a la red local.

use super::tools::echo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Mutex;
use std::time::Duration;

static FILE_LOCK: Mutex<()> = Mutex::new(());

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

// ---------- La red actual ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LanInfo {
    pub adapter: String,
    pub description: String,
    pub index: u32,
    pub wireless: bool,
    pub link_speed: String,
    /// MAC de este equipo en esa red (para encenderlo con Wake-on-LAN).
    pub mac: String,
    pub ip: String,
    pub prefix: u32,
    pub gateway: String,
    pub gateway_mac: String,
    pub dns: Vec<String>,
    /// Nombre de la red para Windows (en Wi-Fi, el SSID).
    pub network: String,
    /// Public | Private | DomainAuthenticated
    pub category: String,
    pub dhcp: bool,
    pub ssid: Option<String>,
    pub wifi_password: Option<String>,
    pub wifi_auth: String,
    /// Identifica esta red para recordar su router y sus dispositivos.
    pub key: String,
}

const INFO_SCRIPT: &str = r#"
$c = Get-NetIPConfiguration -ErrorAction SilentlyContinue |
  Where-Object { $_.IPv4DefaultGateway -and $_.NetAdapter.Status -eq 'Up' } |
  Sort-Object { $_.NetIPv4Interface.InterfaceMetric } | Select-Object -First 1
if (-not $c) { 'null'; return }
$gw = "$(@($c.IPv4DefaultGateway)[0].NextHop)"
$ip = @($c.IPv4Address)[0]
$n = Get-NetNeighbor -IPAddress $gw -ErrorAction SilentlyContinue | Select-Object -First 1
$p = Get-NetConnectionProfile -InterfaceIndex $c.InterfaceIndex -ErrorAction SilentlyContinue | Select-Object -First 1
$a = $c.NetAdapter
[pscustomobject]@{
  adapter = "$($c.InterfaceAlias)"; description = "$($a.InterfaceDescription)"; index = [int]$c.InterfaceIndex
  wireless = [bool]("$($a.PhysicalMediaType)" -match '802\.11' -or "$($a.InterfaceDescription)" -match 'Wi-?Fi|Wireless|WLAN|802\.11')
  linkSpeed = "$($a.LinkSpeed)"; mac = "$($a.MacAddress)"
  ip = "$($ip.IPAddress)"; prefix = [int]$ip.PrefixLength
  gateway = $gw; gatewayMac = if ($n) { "$($n.LinkLayerAddress)" } else { '' }
  dns = @($c.DNSServer | Where-Object { $_.AddressFamily -eq 2 } | ForEach-Object { $_.ServerAddresses } | Where-Object { $_ })
  network = if ($p) { "$($p.Name)" } else { '' }
  category = if ($p) { "$($p.NetworkCategory)" } else { '' }
  dhcp = [bool]("$($c.NetIPv4Interface.Dhcp)" -eq 'Enabled')
} | ConvertTo-Json -Compress -Depth 3
"#;

/// "AA-BB-CC-DD-EE-FF" → "aa:bb:cc:dd:ee:ff"
fn norm_mac(mac: &str) -> String {
    mac.trim().replace('-', ":").to_ascii_lowercase()
}

fn network_key(gateway: &str, gateway_mac: &str) -> String {
    let mac = norm_mac(gateway_mac);
    if mac.len() == 17 && mac != "00:00:00:00:00:00" {
        format!("mac-{mac}")
    } else {
        format!("gw-{gateway}")
    }
}

pub fn current() -> Result<Option<LanInfo>, String> {
    let out = crate::ps::powershell(INFO_SCRIPT)?;
    let mut info: Option<LanInfo> = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    if let Some(i) = info.as_mut() {
        i.gateway_mac = norm_mac(&i.gateway_mac);
        i.mac = norm_mac(&i.mac);
        i.key = network_key(&i.gateway, &i.gateway_mac);
        if i.wireless {
            if let Some(w) = super::wifi::list().ok().and_then(|l| l.into_iter().find(|w| w.connected)) {
                i.ssid = Some(w.ssid);
                i.wifi_password = w.password;
                i.wifi_auth = w.authentication;
            }
        }
    }
    Ok(info)
}

#[tauri::command(async)]
pub fn lan_info() -> Result<Option<LanInfo>, String> {
    current()
}

// ---------- IP pública ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct PublicIp {
    pub ip: String,
    pub city: String,
    pub region: String,
    pub country: String,
    /// Proveedor ("AS1234 Claro…").
    pub org: String,
}

#[tauri::command]
pub async fn public_ip() -> Result<PublicIp, String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(6)).build().map_err(|e| e.to_string())?;
    let mut ip: PublicIp = client
        .get("https://ipinfo.io/json")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|_| "No se pudo consultar la IP pública (¿hay Internet?).".to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    // "AS6400 Compañía Dominicana de Teléfonos" → sin el número de sistema autónomo.
    if let Some((asn, name)) = ip.org.split_once(' ') {
        if asn.starts_with("AS") {
            ip.org = name.to_string();
        }
    }
    Ok(ip)
}

// ---------- Router ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// ok | info | warn | bad
    pub level: &'static str,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RouterCheck {
    /// Dirección del panel (http o https, según responda).
    pub url: String,
    pub title: String,
    pub brand: Option<String>,
    /// Pista de usuario y contraseña de fábrica según la marca.
    pub default_hint: String,
    pub open_ports: Vec<u16>,
    /// normal | double | provider | cgnat | unknown
    pub nat: String,
    pub notes: Vec<Note>,
}

fn private_v4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

fn cgnat(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (64..128).contains(&o[1])
}

/// Dirección IPv4 privada (la de un router o equipo de la red local).
pub fn is_private_host(host: &str) -> bool {
    host.parse::<Ipv4Addr>().is_ok_and(private_v4)
}

fn port_open(ip: Ipv4Addr, port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::from((ip, port)), Duration::from_millis(600)).is_ok()
}

const BRANDS: &[(&str, &str)] = &[
    ("tp-link", "TP-Link"),
    ("tplink", "TP-Link"),
    ("mercusys", "Mercusys"),
    ("huawei", "Huawei"),
    ("zte", "ZTE"),
    ("netgear", "Netgear"),
    ("linksys", "Linksys"),
    ("d-link", "D-Link"),
    ("dlink", "D-Link"),
    ("asus", "ASUS"),
    ("tenda", "Tenda"),
    ("mikrotik", "MikroTik"),
    ("routeros", "MikroTik"),
    ("ubiquiti", "Ubiquiti"),
    ("unifi", "Ubiquiti"),
    ("technicolor", "Technicolor"),
    ("arris", "Arris"),
    ("sagemcom", "Sagemcom"),
    ("nokia", "Nokia"),
    ("fiberhome", "FiberHome"),
    ("xiaomi", "Xiaomi"),
    ("mi router", "Xiaomi"),
    ("cisco", "Cisco"),
    ("zyxel", "Zyxel"),
    ("openwrt", "OpenWrt"),
    ("luci", "OpenWrt"),
    ("fritz", "AVM FRITZ!Box"),
];

pub fn brand_of(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    BRANDS.iter().find(|(k, _)| t.contains(k)).map(|(_, b)| *b)
}

fn default_hint(brand: Option<&str>) -> String {
    let specific = match brand {
        Some("TP-Link") | Some("Mercusys") => "En los TP-Link antiguos: admin / admin. Los nuevos piden crear la contraseña la primera vez.",
        Some("Netgear") => "Netgear suele venir con admin / password.",
        Some("Linksys") => "Linksys suele venir con admin / admin (o la contraseña en blanco).",
        Some("D-Link") => "D-Link suele venir con admin y la contraseña en blanco.",
        Some("ASUS") => "ASUS suele venir con admin / admin (los nuevos piden cambiarla al entrar).",
        Some("MikroTik") => "MikroTik antiguos: admin sin contraseña. Los nuevos la traen en la etiqueta.",
        Some("Ubiquiti") => "Ubiquiti antiguos: ubnt / ubnt.",
        Some("Huawei") | Some("ZTE") | Some("FiberHome") | Some("Nokia") | Some("Sagemcom") | Some("Technicolor") | Some("Arris") => {
            "Los routers que instala el proveedor traen usuario y contraseña propios en la etiqueta."
        }
        _ => "",
    };
    let base = "Mira la etiqueta del router: suele traer la dirección, el usuario y la contraseña.";
    if specific.is_empty() {
        base.into()
    } else {
        format!("{specific} {base}")
    }
}

fn html_title(html: &str) -> String {
    let lower = html.to_lowercase();
    let Some(start) = lower.find("<title") else { return String::new() };
    let Some(open_end) = lower[start..].find('>') else { return String::new() };
    let from = start + open_end + 1;
    let to = lower[from..].find("</title").map_or(from, |e| from + e);
    html.get(from..to).unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect()
}

/// Pide la página del panel (sin seguir a otros servidores) y devuelve (título + cabeceras) para reconocer la marca.
async fn fetch_panel(url: &str) -> Option<(String, String)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        // El panel del router casi siempre usa un certificado propio.
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .ok()?;
    let r = client.get(url).send().await.ok()?;
    let server = r.headers().get("server").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let realm = r.headers().get("www-authenticate").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let body = r.text().await.unwrap_or_default();
    let title = html_title(&body);
    let hay = format!("{title} {server} {realm} {}", body.chars().take(20_000).collect::<String>());
    Some((title, hay))
}

/// Título de la página web de un dispositivo de la red local (vacío si no tiene).
pub async fn page_title(url: &str) -> String {
    fetch_panel(url).await.map(|(t, _)| t).unwrap_or_default()
}

/// Segundo salto hacia Internet: otro router privado = doble NAT; 100.64/10 = CGNAT del proveedor.
fn nat_kind(gateway: Ipv4Addr) -> (String, Option<Ipv4Addr>) {
    let target = Ipv4Addr::new(1, 1, 1, 1);
    let (_, first, _) = echo(target, 1, 1000);
    if first.is_some_and(|f| f != gateway) {
        return ("unknown".into(), None);
    }
    for ttl in 2..=3u8 {
        let (status, from, _) = echo(target, ttl, 1200);
        if let Some(hop) = from {
            if status == 0 && hop == target {
                return ("normal".into(), Some(hop));
            }
            if cgnat(hop) {
                return ("cgnat".into(), Some(hop));
            }
            // 192.168.x.x es lo típico de otro router doméstico; 10.x y 172.16-31
            // también las usan los proveedores dentro de su red.
            if hop.octets()[0] == 192 && hop.octets()[1] == 168 {
                return ("double".into(), Some(hop));
            }
            if private_v4(hop) {
                return ("provider".into(), Some(hop));
            }
            return ("normal".into(), Some(hop));
        }
    }
    ("unknown".into(), None)
}

#[tauri::command]
pub async fn router_check(gateway: String, wifi_auth: String) -> Result<RouterCheck, String> {
    let gw: Ipv4Addr = gateway.trim().parse().map_err(|_| "La puerta de enlace no es una IPv4.".to_string())?;
    if !private_v4(gw) {
        return Err("La puerta de enlace no es una dirección de red local.".into());
    }
    const PORTS: [u16; 7] = [80, 443, 8080, 8443, 23, 21, 22];
    let ports = tauri::async_runtime::spawn_blocking(move || {
        std::thread::scope(|s| {
            let handles: Vec<_> = PORTS.iter().map(|&p| s.spawn(move || (p, port_open(gw, p)))).collect();
            handles.into_iter().filter_map(|h| h.join().ok()).filter(|(_, open)| *open).map(|(p, _)| p).collect::<Vec<u16>>()
        })
    });
    let nat = tauri::async_runtime::spawn_blocking(move || nat_kind(gw));
    let open_ports = ports.await.map_err(|e| e.to_string())?;
    let (nat, hop) = nat.await.map_err(|e| e.to_string())?;

    let mut url = String::new();
    let mut title = String::new();
    let mut hay = String::new();
    let candidates = [(443, format!("https://{gw}/")), (80, format!("http://{gw}/")), (8443, format!("https://{gw}:8443/")), (8080, format!("http://{gw}:8080/"))];
    for (port, u) in candidates.iter().filter(|(p, _)| open_ports.contains(p)) {
        if let Some((t, h)) = fetch_panel(u).await {
            url = u.clone();
            title = t;
            hay = h;
            // Preferir https si responde; si no, el primero que conteste.
            if *port == 443 || *port == 8443 {
                break;
            }
            if !title.is_empty() {
                break;
            }
        }
    }
    if url.is_empty() {
        url = format!("http://{gw}/");
    }
    let brand = brand_of(&hay).map(String::from);

    let mut notes = Vec::new();
    if open_ports.contains(&23) {
        notes.push(Note { level: "bad", text: "Telnet abierto en el router: permite entrar sin cifrar. Desactívalo en el panel.".into() });
    }
    if open_ports.contains(&21) {
        notes.push(Note { level: "warn", text: "FTP abierto en el router (compartir USB): conviene desactivarlo si no se usa.".into() });
    }
    if url.starts_with("http://") {
        notes.push(Note { level: "info", text: "El panel usa HTTP sin cifrar: entra solo desde esta red, nunca desde una Wi-Fi de invitados.".into() });
    }
    match nat.as_str() {
        "double" => notes.push(Note {
            level: "warn",
            text: format!(
                "Doble NAT: hay otro router después de este ({}). Puede dar problemas con juegos, cámaras y VPN; pon el del proveedor en modo puente.",
                hop.map(|h| h.to_string()).unwrap_or_default()
            ),
        }),
        "provider" => notes.push(Note {
            level: "info",
            text: format!(
                "Después del router hay una dirección privada ({}): suele ser la red interna del proveedor, con IP compartida (CGNAT) o doble NAT. Si necesitas abrir puertos, compara la IP WAN del router con la IP pública.",
                hop.map(|h| h.to_string()).unwrap_or_default()
            ),
        }),
        "cgnat" => notes.push(Note {
            level: "info",
            text: "El proveedor usa CGNAT (IP compartida): no se pueden abrir puertos hacia dentro (cámaras, servidores) sin pedirle una IP pública.".into(),
        }),
        "normal" => notes.push(Note { level: "ok", text: "Un solo router hasta Internet (sin doble NAT).".into() }),
        _ => {}
    }
    let auth = wifi_auth.to_lowercase();
    if !auth.is_empty() {
        let (level, text) = if auth == "open" {
            ("bad", "La Wi-Fi está abierta, sin contraseña: cualquiera puede conectarse y ver el tráfico.")
        } else if auth.contains("wep") || auth == "shared" {
            ("bad", "La Wi-Fi usa WEP, un cifrado que se rompe en minutos. Cámbialo a WPA2 o WPA3.")
        } else if auth.starts_with("wpa") && !auth.contains("wpa2") && !auth.contains("wpa3") {
            ("warn", "La Wi-Fi usa WPA (el primero), ya inseguro. Cámbialo a WPA2 o WPA3.")
        } else if auth.contains("wpa3") {
            ("ok", "Wi-Fi con WPA3, el cifrado más seguro.")
        } else {
            ("ok", "Wi-Fi con WPA2: correcto.")
        };
        notes.push(Note { level, text: text.into() });
    }
    Ok(RouterCheck { default_hint: default_hint(brand.as_deref()), url, title, brand, open_ports, nat, notes })
}

// ---------- Credenciales del router (cifradas con DPAPI) ----------

#[cfg(windows)]
pub(crate) fn dpapi(data: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let ok = unsafe {
        if protect {
            CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut out)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut out)
        }
    };
    if ok == 0 || out.pbData.is_null() {
        return Err(if protect { "No se pudo cifrar la contraseña." } else { "La contraseña se guardó con otro usuario o en otro equipo." }.into());
    }
    let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
    unsafe { LocalFree(out.pbData.cast()) };
    Ok(bytes)
}

#[cfg(not(windows))]
pub(crate) fn dpapi(_: &[u8], _: bool) -> Result<Vec<u8>, String> {
    Err("Solo disponible en Windows.".into())
}

fn encrypt(secret: &str) -> Result<String, String> {
    crate::secrets::seal(secret)
}

fn decrypt(stored: &str) -> Result<String, String> {
    crate::secrets::open(stored)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct StoredRouter {
    key: String,
    name: String,
    url: String,
    username: String,
    /// Cifrada con DPAPI (solo este usuario de Windows puede leerla).
    password: String,
    notes: String,
    updated: u64,
}

/// Lo que ve la interfaz: la contraseña ya descifrada.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RouterProfile {
    pub key: String,
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    pub updated: u64,
    /// No se pudo descifrar la contraseña (otro equipo u otro usuario).
    pub locked: bool,
}

fn routers_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("routers.json")
}

fn view(s: StoredRouter) -> RouterProfile {
    let (password, locked) = match decrypt(&s.password) {
        Ok(p) => (p, false),
        Err(_) => (String::new(), true),
    };
    RouterProfile { key: s.key, name: s.name, url: s.url, username: s.username, password, notes: s.notes, updated: s.updated, locked }
}

#[tauri::command]
pub fn list_routers(app: tauri::AppHandle) -> Vec<RouterProfile> {
    let mut list: Vec<StoredRouter> = crate::paths::read_json(&routers_path(&app));
    // Portable: las guardadas en el equipo (DPAPI) pasan al formato del USB.
    let mut changed = false;
    for r in &mut list {
        if let Some(p) = crate::secrets::upgrade(&r.password) {
            r.password = p;
            changed = true;
        }
    }
    if changed {
        let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = crate::paths::write_json(&routers_path(&app), &list);
    }
    let mut v: Vec<RouterProfile> = list.into_iter().map(view).collect();
    v.sort_by_key(|r| std::cmp::Reverse(r.updated));
    v
}

fn check_router_url(url: &str) -> Result<String, String> {
    let u = url.trim();
    let u = if u.contains("://") { u.to_string() } else { format!("http://{u}") };
    let parsed = tauri::Url::parse(&u).map_err(|_| "La dirección del router no es válida.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("La dirección del router debe empezar por http:// o https://.".into());
    }
    Ok(u)
}

#[tauri::command]
pub fn save_router(app: tauri::AppHandle, profile: RouterProfile) -> Result<RouterProfile, String> {
    if profile.key.trim().is_empty() {
        return Err("Falta la red a la que pertenece el router.".into());
    }
    let url = check_router_url(&profile.url)?;
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<StoredRouter> = crate::paths::read_json(&routers_path(&app));
    let existing = list.iter().position(|r| r.key == profile.key);
    // Contraseña que no se pudo descifrar y no se cambió: se conserva la guardada.
    let password = match (profile.locked && profile.password.is_empty(), existing) {
        (true, Some(i)) => list[i].password.clone(),
        _ => encrypt(&profile.password)?,
    };
    let stored = StoredRouter {
        key: profile.key.trim().into(),
        name: profile.name.trim().chars().take(60).collect(),
        url,
        username: profile.username.trim().into(),
        password,
        notes: profile.notes,
        updated: now(),
    };
    match existing {
        Some(i) => list[i] = stored.clone(),
        None => list.push(stored.clone()),
    }
    crate::paths::write_json(&routers_path(&app), &list)?;
    Ok(view(stored))
}

#[tauri::command]
pub fn delete_router(app: tauri::AppHandle, key: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<StoredRouter> = crate::paths::read_json(&routers_path(&app));
    list.retain(|r| r.key != key);
    crate::paths::write_json(&routers_path(&app), &list)
}

// ---------- QR de la Wi-Fi ----------

/// Escapa `\ ; , : "` como pide el formato WIFI: de los QR.
fn wifi_escape(s: &str) -> String {
    s.chars().fold(String::new(), |mut acc, c| {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            acc.push('\\');
        }
        acc.push(c);
        acc
    })
}

fn wifi_payload(ssid: &str, password: &str, auth: &str) -> String {
    let a = auth.to_lowercase();
    let kind = if password.is_empty() || a == "open" { "nopass" } else if a.contains("wep") { "WEP" } else { "WPA" };
    format!("WIFI:T:{kind};S:{};P:{};;", wifi_escape(ssid), wifi_escape(password))
}

/// Código QR (SVG) para conectarse a la Wi-Fi con la cámara del móvil.
#[tauri::command]
pub fn wifi_qr(ssid: String, password: String, auth: String) -> Result<String, String> {
    use qrcode::render::svg;
    if ssid.is_empty() {
        return Err("No hay red Wi-Fi.".into());
    }
    let code = qrcode::QrCode::new(wifi_payload(&ssid, &password, &auth).as_bytes()).map_err(|e| e.to_string())?;
    Ok(code.render::<svg::Color<'_>>().min_dimensions(220, 220).quiet_zone(true).dark_color(svg::Color("#111315")).light_color(svg::Color("#ffffff")).build())
}

// ---------- Dispositivos de la red ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Device {
    pub ip: String,
    pub mac: String,
    pub name: String,
    pub vendor: String,
    pub alias: String,
    pub gateway: bool,
    pub this_pc: bool,
    /// MAC aleatoria (móviles modernos): no identifica al fabricante.
    pub private_mac: bool,
    /// Respondió al ping (algunos equipos solo contestan a ARP).
    pub ms: Option<u32>,
    /// No se había visto antes en esta red.
    pub new: bool,
    pub first_seen: u64,
    /// De la identificación a fondo (se recuerda entre búsquedas).
    pub kind: String,
    pub manufacturer: String,
    pub model: String,
    pub friendly: String,
    pub os: String,
    pub services: Vec<String>,
    pub ports: Vec<u16>,
    pub netbios: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct Known {
    alias: String,
    name: String,
    vendor: String,
    first_seen: u64,
    last_seen: u64,
    info: Option<super::identify::DeviceInfo>,
}

type Memory = HashMap<String, HashMap<String, Known>>;

fn memory_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::shared_data_dir(app).join("lan-devices.json")
}

fn private_mac(mac: &str) -> bool {
    // Bit "administrado localmente" del primer octeto: x2, x6, xA, xE.
    mac.chars().nth(1).is_some_and(|c| matches!(c.to_ascii_lowercase(), '2' | '6' | 'a' | 'e'))
}

/// Direcciones a revisar: la subred del equipo (como mucho su bloque /24).
fn hosts(ip: Ipv4Addr, prefix: u32) -> Vec<Ipv4Addr> {
    let prefix = prefix.clamp(24, 30);
    let mask = u32::MAX << (32 - prefix);
    let net = u32::from(ip) & mask;
    let size = 1u32 << (32 - prefix);
    (1..size - 1).map(|i| Ipv4Addr::from(net + i)).filter(|h| *h != ip).collect()
}

#[cfg(windows)]
fn reverse_name(ip: Ipv4Addr) -> Option<String> {
    use windows_sys::Win32::Networking::WinSock::{GetNameInfoW, AF_INET, IN_ADDR, IN_ADDR_0, NI_NAMEREQD, SOCKADDR, SOCKADDR_IN};
    let addr = SOCKADDR_IN { sin_family: AF_INET, sin_port: 0, sin_addr: IN_ADDR { S_un: IN_ADDR_0 { S_addr: u32::from_ne_bytes(ip.octets()) } }, sin_zero: [0; 8] };
    let mut buf = [0u16; 256];
    let r = unsafe {
        GetNameInfoW(
            (&addr as *const SOCKADDR_IN).cast::<SOCKADDR>(),
            std::mem::size_of::<SOCKADDR_IN>() as i32,
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null_mut(),
            0,
            NI_NAMEREQD as i32,
        )
    };
    if r != 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let name = String::from_utf16_lossy(&buf[..len]);
    let name = name.trim_end_matches(".local").trim_end_matches(".lan").trim_end_matches(".home").to_string();
    (!name.is_empty() && name != ip.to_string()).then_some(name)
}

#[cfg(not(windows))]
fn reverse_name(_: Ipv4Addr) -> Option<String> {
    None
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Neighbor {
    ip: String,
    mac: String,
}

const NEIGHBORS_SCRIPT: &str = r#"
$r = @(Get-NetNeighbor -AddressFamily IPv4 -InterfaceIndex $ifIndex -ErrorAction SilentlyContinue |
  Where-Object { $_.State -notin 'Unreachable', 'Incomplete' -and $_.LinkLayerAddress -and $_.LinkLayerAddress -ne '00-00-00-00-00-00' -and $_.LinkLayerAddress -ne 'FF-FF-FF-FF-FF-FF' } |
  ForEach-Object { [pscustomobject]@{ ip = "$($_.IPAddress)"; mac = "$($_.LinkLayerAddress)" } })
ConvertTo-Json -InputObject $r -Compress
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scan {
    pub key: String,
    pub devices: Vec<Device>,
    /// Se revisó solo el bloque /24 del equipo (la red es más grande).
    pub truncated: bool,
}

#[tauri::command(async)]
pub fn scan_lan(app: tauri::AppHandle) -> Result<Scan, String> {
    let info = current()?.ok_or("El equipo no está conectado a ninguna red con router.")?;
    let ip: Ipv4Addr = info.ip.parse().map_err(|_| "El equipo no tiene una IPv4 en esta red.".to_string())?;
    let gateway: Ipv4Addr = info.gateway.parse().unwrap_or(Ipv4Addr::UNSPECIFIED);
    let task = crate::task::Task::new(&app, "lan-scan").named("Buscar dispositivos en la red");
    let targets = hosts(ip, info.prefix);
    task.step(format!("Buscando equipos en {} direcciones…", targets.len()));

    // Ping a toda la subred en paralelo: además de las respuestas, rellena la
    // tabla ARP con los equipos que no contestan al ping pero existen.
    let pings: HashMap<Ipv4Addr, u32> = std::thread::scope(|s| {
        let chunks: Vec<_> = targets
            .chunks(targets.len().div_ceil(48).max(1))
            .map(|chunk| {
                let task = &task;
                s.spawn(move || {
                    chunk
                        .iter()
                        .filter(|_| !task.cancelled())
                        .filter_map(|h| {
                            let (status, from, ms) = echo(*h, 64, 700);
                            (status == 0 && from == Some(*h)).then_some((*h, ms))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        chunks.into_iter().filter_map(|h| h.join().ok()).flatten().collect()
    });
    if task.cancelled() {
        return Err(crate::ps::CANCELLED_MSG.into());
    }

    task.step("Leyendo la tabla de vecinos…");
    // El índice es un número: se puede escribir tal cual en el script.
    let ps_if = format!("$ifIndex = {}
", info.index);
    let out = crate::ps::powershell_opts(&format!("{ps_if}{NEIGHBORS_SCRIPT}"), task.opts(Some(Duration::from_secs(30))))?;
    let neighbors: Vec<Neighbor> = serde_json::from_str(out.trim()).unwrap_or_default();
    let in_net = |a: Ipv4Addr| hosts(ip, info.prefix).contains(&a);
    let mut found: HashMap<Ipv4Addr, String> = neighbors.into_iter().filter_map(|n| Some((n.ip.parse::<Ipv4Addr>().ok()?, norm_mac(&n.mac)))).filter(|(a, _)| in_net(*a)).collect();
    for a in pings.keys() {
        found.entry(*a).or_default();
    }

    task.step(format!("Identificando {} dispositivos…", found.len() + 1));
    let mut list: Vec<(Ipv4Addr, String)> = found.into_iter().collect();
    let own_mac = crate::ps::powershell(&format!("{ps_if}(Get-NetAdapter -InterfaceIndex $ifIndex).MacAddress")).unwrap_or_default();
    list.push((ip, norm_mac(&own_mac)));
    let names: HashMap<Ipv4Addr, String> = std::thread::scope(|s| {
        let hs: Vec<_> = list.iter().map(|(a, _)| *a).map(|a| s.spawn(move || reverse_name(a).map(|n| (a, n)))).collect();
        hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    });

    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory: Memory = crate::paths::read_json(&memory_path(&app));
    let first_scan = !memory.contains_key(&info.key);
    let known = memory.entry(info.key.clone()).or_default();
    let t = now();
    let mut devices: Vec<Device> = list
        .into_iter()
        .map(|(a, mac)| {
            let name = names.get(&a).cloned().unwrap_or_default();
            let id = if mac.is_empty() { format!("ip-{a}") } else { mac.clone() };
            let is_new = !known.contains_key(&id);
            let k = known.entry(id).or_insert_with(|| Known { first_seen: t, ..Default::default() });
            k.last_seen = t;
            if !name.is_empty() {
                k.name = name.clone();
            }
            let info = k.info.clone().unwrap_or_default();
            Device {
                kind: info.kind,
                manufacturer: info.manufacturer,
                model: info.model,
                friendly: info.friendly,
                os: info.os,
                services: info.services,
                ports: info.ports,
                netbios: info.netbios,
                ip: a.to_string(),
                private_mac: private_mac(&mac),
                name: if name.is_empty() { k.name.clone() } else { name },
                vendor: k.vendor.clone(),
                alias: k.alias.clone(),
                gateway: a == gateway,
                this_pc: a == ip,
                ms: pings.get(&a).copied(),
                new: is_new && !first_scan && a != ip,
                first_seen: k.first_seen,
                mac,
            }
        })
        .collect();
    crate::paths::write_json(&memory_path(&app), &memory)?;
    devices.sort_by_key(|d| u32::from(d.ip.parse::<Ipv4Addr>().unwrap_or(Ipv4Addr::UNSPECIFIED)));
    log::info!("Red local: {} dispositivos encontrados", devices.len());
    Ok(Scan { key: info.key, devices, truncated: info.prefix < 24 })
}

/// Abre la página web de un dispositivo de la red local (impresora, cámara…) en el navegador.
#[tauri::command]
pub fn open_device_page(ip: String) -> Result<(), String> {
    let addr: Ipv4Addr = ip.trim().parse().map_err(|_| "IP no válida.".to_string())?;
    if !private_v4(addr) {
        return Err("Solo se abren direcciones de la red local.".into());
    }
    // A través del Explorador: el navegador no hereda los permisos de administrador.
    std::process::Command::new("explorer.exe").arg(format!("http://{addr}/")).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Identificación a fondo de los dispositivos encontrados: tipo, fabricante, modelo…
#[tauri::command]
pub async fn identify_lan(app: tauri::AppHandle, key: String, devices: Vec<Device>) -> Result<Vec<Device>, String> {
    tauri::async_runtime::spawn_blocking(move || identify_blocking(&app, key, devices)).await.map_err(|e| format!("La identificación falló: {e}"))?
}

fn identify_blocking(app: &tauri::AppHandle, key: String, devices: Vec<Device>) -> Result<Vec<Device>, String> {
    let app = app.clone();
    let task = crate::task::Task::new(&app, "lan-identify").named("Identificar dispositivos");
    task.step(format!("Preguntando a {} dispositivos qué son…", devices.len()));
    let parsed: Vec<(Ipv4Addr, &Device)> = devices.iter().filter_map(|d| d.ip.parse().ok().map(|ip| (ip, d))).filter(|(ip, _)| private_v4(*ip)).collect();
    let targets: Vec<(Ipv4Addr, super::identify::Hints)> = parsed
        .iter()
        .map(|(ip, d)| (*ip, super::identify::Hints { gateway: d.gateway, this_pc: d.this_pc, private_mac: d.private_mac, vendor: &d.vendor, name: &d.name }))
        .collect();
    let local = current().ok().flatten().and_then(|i| i.ip.parse().ok()).unwrap_or(Ipv4Addr::UNSPECIFIED);
    let infos = super::identify::identify(local, &targets);

    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory: Memory = crate::paths::read_json(&memory_path(&app));
    let known = memory.entry(key).or_default();
    let out: Vec<Device> = devices
        .into_iter()
        .map(|mut d| {
            if let Some(i) = infos.iter().find(|i| i.ip == d.ip) {
                d.kind = i.kind.clone();
                d.manufacturer = i.manufacturer.clone();
                d.model = i.model.clone();
                d.friendly = i.friendly.clone();
                d.os = i.os.clone();
                d.services = i.services.clone();
                d.ports = i.ports.clone();
                d.netbios = i.netbios.clone();
                if d.name.is_empty() && !i.netbios.is_empty() {
                    d.name = i.netbios.clone();
                }
                let id = if d.mac.is_empty() { format!("ip-{}", d.ip) } else { d.mac.clone() };
                if let Some(k) = known.get_mut(&id) {
                    k.info = Some(i.clone());
                }
            }
            d
        })
        .collect();
    crate::paths::write_json(&memory_path(&app), &memory)?;
    log::info!("Red local: {} dispositivos identificados", infos.iter().filter(|i| i.kind != "unknown").count());
    Ok(out)
}

#[tauri::command]
pub fn set_device_alias(app: tauri::AppHandle, key: String, mac: String, alias: String) -> Result<(), String> {
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory: Memory = crate::paths::read_json(&memory_path(&app));
    let k = memory.entry(key).or_default().entry(mac).or_insert_with(|| Known { first_seen: now(), ..Default::default() });
    k.alias = alias.trim().chars().take(40).collect();
    crate::paths::write_json(&memory_path(&app), &memory)
}

/// Fabricante de cada MAC, consultando en Internet solo el prefijo (OUI) y
/// guardando la respuesta para no repetirla. Las MAC aleatorias no se consultan.
#[tauri::command]
pub async fn lookup_vendors(app: tauri::AppHandle, key: String, macs: Vec<String>) -> Result<HashMap<String, String>, String> {
    let cache_path = crate::paths::shared_data_dir(&app).join("oui-cache.json");
    let mut cache: HashMap<String, String> = crate::paths::read_json(&cache_path);
    let client = reqwest::Client::builder().timeout(Duration::from_secs(6)).build().map_err(|e| e.to_string())?;
    let mut result = HashMap::new();
    let mut asked = 0;
    for mac in macs.iter().map(|m| norm_mac(m)).filter(|m| m.len() == 17 && !private_mac(m)) {
        let oui: String = mac.chars().take(8).collect();
        if !cache.contains_key(&oui) && asked < 40 {
            // El servicio gratuito admite una consulta por segundo.
            if asked > 0 {
                tokio::time::sleep(Duration::from_millis(1100)).await;
            }
            asked += 1;
            match client.get(format!("https://api.macvendors.com/{oui}")).send().await {
                Ok(r) if r.status().is_success() => {
                    cache.insert(oui.clone(), r.text().await.unwrap_or_default().trim().chars().take(60).collect());
                }
                Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => {
                    cache.insert(oui.clone(), String::new());
                }
                _ => {}
            }
        }
        if let Some(v) = cache.get(&oui).filter(|v| !v.is_empty()) {
            result.insert(mac.clone(), v.clone());
        }
    }
    crate::paths::write_json(&cache_path, &cache)?;
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory: Memory = crate::paths::read_json(&memory_path(&app));
    if let Some(known) = memory.get_mut(&key) {
        for (mac, vendor) in &result {
            if let Some(k) = known.get_mut(mac) {
                k.vendor = vendor.clone();
            }
        }
    }
    crate::paths::write_json(&memory_path(&app), &memory)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_hosts() {
        let h = hosts("192.168.1.37".parse().unwrap(), 24);
        assert_eq!(h.len(), 253);
        assert_eq!(h[0], Ipv4Addr::new(192, 168, 1, 1));
        assert!(!h.contains(&Ipv4Addr::new(192, 168, 1, 37)));
        assert!(!h.contains(&Ipv4Addr::new(192, 168, 1, 255)));
        // Redes grandes: solo el /24 del equipo.
        assert_eq!(hosts("10.0.5.9".parse().unwrap(), 16).len(), 253);
        assert_eq!(hosts("10.0.5.9".parse().unwrap(), 30).len(), 1);
    }

    #[test]
    fn macs_and_keys() {
        assert_eq!(norm_mac("AA-BB-CC-00-11-22"), "aa:bb:cc:00:11:22");
        assert!(private_mac("da:a1:19:00:00:01"));
        assert!(!private_mac("f4:f2:6d:00:00:01"));
        assert_eq!(network_key("192.168.1.1", "AA-BB-CC-00-11-22"), "mac-aa:bb:cc:00:11:22");
        assert_eq!(network_key("192.168.1.1", ""), "gw-192.168.1.1");
        assert!(is_private_host("192.168.0.1") && is_private_host("10.1.1.1") && !is_private_host("8.8.8.8") && !is_private_host("router.local"));
        assert!(cgnat("100.72.3.4".parse().unwrap()) && !cgnat("100.200.0.1".parse().unwrap()));
    }

    #[test]
    fn brands_titles_and_wifi_qr() {
        assert_eq!(brand_of("Opening... TP-LINK Wireless N Router"), Some("TP-Link"));
        assert_eq!(brand_of("HG8245H Huawei"), Some("Huawei"));
        assert_eq!(brand_of("Router"), None);
        assert_eq!(html_title("<html><TITLE> Mi\n Router </title>"), "Mi Router");
        assert_eq!(html_title("sin título"), "");
        assert_eq!(wifi_payload("Casa;1", "a:b\"c", "WPA2PSK"), "WIFI:T:WPA;S:Casa\\;1;P:a\\:b\\\"c;;");
        assert_eq!(wifi_payload("Libre", "", "open"), "WIFI:T:nopass;S:Libre;P:;;");
        assert!(wifi_qr("Casa".into(), "clave".into(), "WPA2PSK".into()).unwrap().starts_with("<?xml"));
    }

    #[test]
    fn router_urls() {
        assert_eq!(check_router_url("192.168.1.1").unwrap(), "http://192.168.1.1");
        assert!(check_router_url("https://192.168.0.1:8443/").is_ok());
        assert!(check_router_url("ftp://192.168.1.1").is_err());
    }

    /// Usa la red real: `cargo test lan_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn lan_real() {
        let info = current().unwrap().unwrap();
        println!("{} {}/{} gw {} {} dns {:?} red «{}» wifi {}", info.adapter, info.ip, info.prefix, info.gateway, info.gateway_mac, info.dns, info.network, info.wireless);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let r = rt.block_on(router_check(info.gateway.clone(), info.wifi_auth.clone())).unwrap();
        println!("{} «{}» {:?} puertos {:?} nat {}", r.url, r.title, r.brand, r.open_ports, r.nat);
        for n in r.notes {
            println!("  [{}] {}", n.level, n.text);
        }
    }

    #[test]
    fn embedded_scripts_parse() {
        for (name, script) in [("INFO_SCRIPT", INFO_SCRIPT.to_string()), ("NEIGHBORS_SCRIPT", format!("$ifIndex = 7
{NEIGHBORS_SCRIPT}"))] {
            let errors = crate::ps::parse_errors(&script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }

    #[test]
    #[cfg(windows)]
    fn dpapi_roundtrip() {
        let enc = encrypt("clave ñ 123").unwrap();
        assert_ne!(enc, "clave ñ 123");
        assert_eq!(decrypt(&enc).unwrap(), "clave ñ 123");
        assert_eq!(decrypt("").unwrap(), "");
        assert!(decrypt("AAAA").is_err());
    }
}
