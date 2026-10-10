//! «Algo no funciona»: el técnico elige el síntoma (no hay Internet, no suena…),
//! AdminOps comprueba en orden lo típico y ofrece la reparación de cada problema.
//! Incluye la reparación de red en un clic con comparación antes/después.
//!
//! Los scripts solo recogen datos; la interpretación se hace aquí (probada y sin
//! palabras que alarmen a los antivirus).

use crate::network::wifictl;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use tauri::State;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub id: String,
    pub label: String,
    pub admin: bool,
    /// Texto de confirmación si la reparación corta algo (la red, la pantalla…).
    pub confirm: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// ok | info | warn | bad
    pub level: &'static str,
    pub title: String,
    pub detail: String,
    pub fixes: Vec<Fix>,
    /// Página de AdminOps donde seguir.
    pub page: Option<&'static str>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub symptom: String,
    pub findings: Vec<Finding>,
}

pub(crate) fn fix(id: impl Into<String>, label: &str, admin: bool) -> Fix {
    Fix { id: id.into(), label: label.into(), admin, confirm: None }
}

pub(crate) fn fix_confirm(id: impl Into<String>, label: &str, admin: bool, confirm: &str) -> Fix {
    Fix { confirm: Some(confirm.into()), ..fix(id, label, admin) }
}

pub(crate) fn finding(level: &'static str, title: impl Into<String>, detail: impl Into<String>) -> Finding {
    Finding { level, title: title.into(), detail: detail.into(), fixes: vec![], page: None }
}

impl Finding {
    pub(crate) fn fixes(mut self, f: Vec<Fix>) -> Self {
        self.fixes = f;
        self
    }
    pub(crate) fn page(mut self, p: &'static str) -> Self {
        self.page = Some(p);
        self
    }
}

pub(crate) fn open(uri: &str, label: &str) -> Fix {
    fix(format!("open:{uri}"), label, false)
}

// ---------- Red ----------

const NET_CUTS: &str = "La conexión de red se cortará unos segundos. Si estás conectado a este equipo en remoto, la sesión puede caerse.";

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
struct NetAdapter {
    name: String,
    status: String,
    wifi: bool,
    ipv4: Vec<String>,
    gateway: Vec<String>,
    dhcp: bool,
    dns: Vec<String>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct NetRaw {
    adapters: Vec<NetAdapter>,
    vpn: Vec<String>,
    proxy_enable: u32,
    proxy_server: String,
    auto_config: String,
}

const NET_SCRIPT: &str = r#"
$ie = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction SilentlyContinue
$ad = @(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | ForEach-Object {
  $cfg = Get-NetIPConfiguration -InterfaceIndex $_.ifIndex -ErrorAction SilentlyContinue
  $ifc = Get-NetIPInterface -InterfaceIndex $_.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue
  [pscustomobject]@{
    name = "$($_.Name)"; status = "$($_.Status)"
    wifi = ("$($_.PhysicalMediaType)" -match '802\.11') -or ("$($_.InterfaceDescription)" -match 'Wi-?Fi|Wireless|WLAN')
    ipv4 = @($cfg.IPv4Address | ForEach-Object { "$($_.IPAddress)" } | Where-Object { $_ })
    gateway = @($cfg.IPv4DefaultGateway | ForEach-Object { "$($_.NextHop)" } | Where-Object { $_ })
    dhcp = [bool]($ifc -and "$($ifc.Dhcp)" -eq 'Enabled')
    dns = @($cfg.DNSServer | Where-Object { $_.AddressFamily -eq 2 } | ForEach-Object { $_.ServerAddresses } | Where-Object { $_ })
  }
})
$vpn = @(Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { "$($_.Status)" -eq 'Up' -and "$($_.InterfaceDescription)" -match 'VPN|TAP-|WireGuard|Fortinet|AnyConnect|OpenVPN|Tailscale|ZeroTier|PANGP|GlobalProtect' } | ForEach-Object { "$($_.InterfaceDescription)" })
[pscustomobject]@{ adapters = $ad; vpn = $vpn; proxyEnable = [int]$ie.ProxyEnable; proxyServer = "$($ie.ProxyServer)"; autoConfig = "$($ie.AutoConfigURL)" } | ConvertTo-Json -Depth 4 -Compress
"#;

fn net_raw() -> Result<NetRaw, String> {
    let out = crate::pspool::query(NET_SCRIPT, Some(Duration::from_secs(40)), "Solucionar: red")?;
    serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))
}

/// Estado de la conexión en cuatro preguntas (para el antes/después).
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetCheck {
    /// Hay un adaptador conectado con IP válida.
    pub connected: bool,
    /// Tiene IP 169.254.x: el router no le dio dirección.
    pub no_dhcp_address: bool,
    pub adapter: String,
    /// El router responde (None: no hay router).
    pub gateway: Option<bool>,
    pub internet: bool,
    pub dns: bool,
    // Lo que hace falta para explicar el resultado, no solo para marcarlo.
    /// IPv4 del adaptador activo.
    pub ip: String,
    /// Puerta de enlace (el router).
    pub gateway_ip: String,
    /// Servidores DNS en uso.
    pub dns_servers: Vec<String>,
    /// La IP la da el router (DHCP) o está puesta a mano.
    pub dhcp: bool,
    pub wifi: bool,
    /// Proxy configurado en Windows (vacío si no hay).
    pub proxy: String,
    /// Adaptadores de VPN activos.
    pub vpn: Vec<String>,
}

fn tcp_ok(addr: &str) -> bool {
    addr.parse::<SocketAddr>().is_ok_and(|a| TcpStream::connect_timeout(&a, Duration::from_secs(3)).is_ok())
}

fn dns_ok() -> bool {
    ["www.microsoft.com", "www.google.com"].iter().any(|h| (*h, 443).to_socket_addrs().is_ok_and(|mut a| a.next().is_some()))
}

fn active(raw: &NetRaw) -> Option<&NetAdapter> {
    let up = |a: &&NetAdapter| a.status.eq_ignore_ascii_case("Up");
    raw.adapters.iter().filter(up).find(|a| !a.gateway.is_empty()).or_else(|| raw.adapters.iter().find(up))
}

fn net_check_from(raw: &NetRaw) -> NetCheck {
    let a = active(raw);
    let valid_ip = a.is_some_and(|a| a.ipv4.iter().any(|ip| !ip.starts_with("169.254.")));
    let gateway = a.and_then(|a| a.gateway.first()).map(|g| crate::network::diag::ping("router", g, 2).received > 0);
    let (internet, dns) = std::thread::scope(|s| {
        let i = s.spawn(|| tcp_ok("1.1.1.1:443") || tcp_ok("8.8.8.8:443"));
        let d = s.spawn(dns_ok);
        (i.join().unwrap_or(false), d.join().unwrap_or(false))
    });
    let proxy = if raw.proxy_enable == 1 && !raw.proxy_server.is_empty() {
        raw.proxy_server.clone()
    } else if !raw.auto_config.is_empty() {
        raw.auto_config.clone()
    } else {
        String::new()
    };
    NetCheck {
        connected: valid_ip,
        no_dhcp_address: a.is_some_and(|a| !a.ipv4.is_empty() && a.ipv4.iter().all(|ip| ip.starts_with("169.254."))),
        adapter: a.map(|a| a.name.clone()).unwrap_or_default(),
        gateway,
        internet,
        dns,
        ip: a.and_then(|a| a.ipv4.first().cloned()).unwrap_or_default(),
        gateway_ip: a.and_then(|a| a.gateway.first().cloned()).unwrap_or_default(),
        dns_servers: a.map(|a| a.dns.clone()).unwrap_or_default(),
        dhcp: a.is_some_and(|a| a.dhcp),
        wifi: a.is_some_and(|a| a.wifi),
        proxy,
        vpn: raw.vpn.clone(),
    }
}

#[tauri::command(async)]
pub fn quick_net_check() -> Result<NetCheck, String> {
    Ok(net_check_from(&net_raw()?))
}

fn check_internet() -> Result<Vec<Finding>, String> {
    let raw = net_raw()?;
    let c = net_check_from(&raw);
    let mut out = Vec::new();
    let quick = fix_confirm("net.quick", "Reparar la red", true, NET_CUTS);
    let deep = fix_confirm(
        "net.deep",
        "Reparar a fondo (Winsock y TCP/IP)",
        true,
        "Además restablece Winsock y la pila TCP/IP: se pierde la IP fija si la hay y hay que reiniciar el equipo.",
    );

    // La Wi-Fi rota es la causa más clara: se dice primero.
    if let Ok(w) = wifictl::wifi_state() {
        if w.level == "error" || (w.level == "off" && active(&raw).is_none()) {
            // Con Internet por cable, la Wi-Fi rota es un aviso, no la causa.
            let (level, title) = if c.internet { ("warn", "La Wi-Fi no funciona (la conexión va por cable)") } else if w.level == "error" { ("bad", "Problema con la Wi-Fi") } else { ("warn", "La Wi-Fi está apagada") };
            out.push(finding(level, title, w.summary.clone()).fixes(wifi_fixes(&w)).page("network"));
        }
    }
    if active(&raw).is_none() {
        let cable = raw.adapters.iter().any(|a| !a.wifi);
        out.push(
            finding(
                "bad",
                "No hay ninguna conexión de red activa",
                if cable { "Ni el cable ni la Wi-Fi están conectados. Revisa que el cable esté bien enchufado (luces del puerto) o conéctate a una Wi-Fi." } else { "Conéctate a una red Wi-Fi o enchufa un cable de red." },
            )
            .fixes(vec![open("ms-settings:network-wifi", "Abrir redes Wi-Fi")]),
        );
        return Ok(out);
    }
    if c.no_dhcp_address {
        out.push(
            finding("bad", "El router no le da dirección IP a este equipo", "Tiene una IP 169.254.x: pidió dirección al router y no obtuvo respuesta. Suele arreglarse renovando la IP; si no, reinicia el router.")
                .fixes(vec![quick.clone()]),
        );
    } else if c.gateway.is_none() {
        out.push(
            finding("bad", "La conexión no tiene puerta de enlace (router)", "Está conectado pero no sabe por dónde salir. Si tiene IP fija, falta la puerta de enlace; si es automática, renueva la IP.")
                .fixes(vec![quick.clone()])
                .page("nettools"),
        );
    } else if c.gateway == Some(false) && !c.internet {
        out.push(
            finding("bad", "El router no responde", "El equipo está conectado pero el router no contesta. Reinicia el router (desenchufa 30 segundos) o revisa el cable entre el equipo y el router.")
                .fixes(vec![quick.clone()])
                .page("router"),
        );
    }
    if !c.internet && c.connected && c.gateway != Some(false) {
        out.push(finding(
            "bad",
            "No hay salida a Internet",
            if c.gateway == Some(true) { "El router responde pero no llega a Internet: el problema está en el router o en el proveedor. Reinicia el router y, si sigue, llama al proveedor." } else { "No se llega a Internet." },
        )
        .fixes(vec![quick.clone(), deep.clone()])
        .page("router"));
    }
    if c.internet && !c.dns {
        out.push(
            finding("bad", "Hay Internet pero los nombres de las webs no se resuelven (DNS)", "Las páginas no cargan aunque la conexión funciona. Vaciar la caché de DNS o usar un DNS público (1.1.1.1, 8.8.8.8) suele arreglarlo.")
                .fixes(vec![fix("net.flushdns", "Vaciar la caché de DNS", false), quick.clone()])
                .page("nettools"),
        );
    }
    if raw.proxy_enable == 1 || !raw.auto_config.is_empty() {
        let what = if raw.proxy_enable == 1 { raw.proxy_server.clone() } else { raw.auto_config.clone() };
        out.push(
            finding("warn", "Hay un proxy configurado", format!("Todo el tráfico web pasa por «{what}». Si no lo puso la empresa, puede ser un resto de un programa o malware y romper la navegación."))
                .fixes(vec![fix_confirm("proxy.off", "Quitar el proxy", false, "Se quitará el proxy de este usuario. En una empresa que lo exige, dejarás de navegar: hazlo solo si no debería estar.")]),
        );
    }
    if !raw.vpn.is_empty() {
        out.push(finding("info", "Hay una VPN conectada", format!("{}. Si la navegación va mal, prueba a desconectarla.", raw.vpn.join(", "))));
    }
    if out.iter().all(|f| f.level == "info" || f.level == "warn") && c.internet && c.dns {
        out.insert(
            0,
            finding("ok", "La conexión funciona", format!("Conectado por «{}»: el router responde, hay Internet y los nombres se resuelven. Si una web concreta falla, puede ser la propia web.", c.adapter))
                .page("network"),
        );
    }
    Ok(out)
}

/// Reparación de red en un clic, con comprobación antes y después.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetRepair {
    pub before: NetCheck,
    pub after: NetCheck,
    pub steps: Vec<Step>,
    pub reboot: bool,
    /// Qué pasa y qué hacer ahora. Lo importante del resultado: marcar cuatro
    /// casillas está bien para ver si mejoró, pero no dice dónde está el problema.
    pub verdict: Verdict,
}

/// Diagnóstico en cristiano de cómo quedó la red.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    /// "ok" | "warn" | "bad"
    pub level: String,
    pub title: String,
    /// Qué pasa y, sobre todo, qué hacer a continuación.
    pub text: String,
    /// Qué botones ofrecer: "router" | "dns" | "deep" | "wifi" | "proxy" | "speed".
    pub next: Vec<String>,
}

/// Dónde se corta la cadena: adaptador → IP → router → Internet → DNS. Se mira
/// en ese orden porque cada eslabón depende del anterior, y así el técnico sabe
/// a qué atenerse en vez de volver a probar a ciegas.
fn verdict_for(c: &NetCheck, deep_done: bool) -> Verdict {
    let v = |level: &str, title: &str, text: String, next: &[&str]| Verdict {
        level: level.into(),
        title: title.into(),
        text,
        next: next.iter().map(|s| s.to_string()).collect(),
    };
    let equipo = if c.adapter.is_empty() { "este equipo".to_string() } else { format!("«{}»", c.adapter) };

    if c.adapter.is_empty() {
        return v("bad", "No hay ninguna tarjeta de red conectada", "Ni cable ni Wi-Fi. Revisa que el cable esté puesto en los dos extremos y con la luz encendida, o que la Wi-Fi esté activada. Si la tarjeta no aparece, mírala en Estado del equipo → Piezas.".into(), &["wifi"]);
    }
    if c.no_dhcp_address {
        return v(
            "bad",
            "El router no le está dando dirección",
            format!("{equipo} se ha puesto una dirección 169.254.x.x, que es lo que hace Windows cuando nadie le contesta. Casi siempre es el cable, el puerto del switch o que el router está colgado: apágalo 30 segundos y vuelve a encenderlo. Si hay varios equipos igual, es el router."),
            &["router"],
        );
    }
    if !c.connected {
        return v("bad", "Sin dirección IP válida", format!("{equipo} no tiene una IPv4 utilizable. Revisa el cable o la Wi-Fi y, si la dirección está puesta a mano, compruébala."), &["wifi", "router"]);
    }
    if c.gateway == Some(false) {
        return v(
            "bad",
            "El equipo tiene IP pero el router no responde",
            format!("Tiene la dirección {} y su puerta de enlace es {}, pero esa dirección no contesta. El router puede estar apagado o colgado, el cable en un puerto que no toca, o {equipo} en otra red distinta de la del router.{}",
                c.ip, c.gateway_ip,
                if c.dhcp { "" } else { " Ojo: la IP está puesta a mano, no la da el router; si la red cambió, ya no vale." }),
            &["router"],
        );
    }
    if !c.internet {
        let vpn = if c.vpn.is_empty() { String::new() } else { format!(" Hay una VPN activa ({}): pruébalo también con la VPN desconectada.", c.vpn.join(", ")) };
        return v(
            "warn",
            "Llega al router, pero no sale a Internet",
            format!("La red local funciona: el problema está del router hacia fuera, así que no es de este equipo. Entra en el panel del router y mira si tiene línea; si no la tiene, es del proveedor. Comprueba también si la red pide aceptar unas condiciones en el navegador (hoteles, wifis públicas).{vpn}"),
            &["router", "speed"],
        );
    }
    if !c.dns {
        let actuales = if c.dns_servers.is_empty() { "ninguno configurado".to_string() } else { c.dns_servers.join(", ") };
        return v(
            "warn",
            "Hay Internet, pero no resuelve nombres",
            format!("Se llega a las direcciones pero no se traducen los nombres: por eso «no carga ninguna web» aunque la conexión esté bien. Los DNS en uso son {actuales}. Cámbialos a 1.1.1.1 y 8.8.8.8 y vuelve a probar."),
            &["dns"],
        );
    }
    if !c.proxy.is_empty() {
        return v(
            "warn",
            "La red funciona, pero hay un proxy configurado",
            format!("Todo responde, pero Windows tiene un proxy puesto ({}). Si el equipo ya no está en la red donde hacía falta, las webs seguirán sin cargar aunque la conexión esté bien.", c.proxy),
            &["proxy"],
        );
    }
    v(
        "ok",
        "La red funciona",
        format!("{equipo} tiene dirección {}, llega al router {} y resuelve nombres.{}", c.ip, c.gateway_ip, if deep_done { " Reinicia el equipo para terminar de aplicar el restablecimiento." } else { "" }),
        &["speed"],
    )
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub title: String,
    pub ok: bool,
    pub detail: String,
}

fn step(title: &str, r: Result<String, String>) -> Step {
    match r {
        Ok(_) => Step { title: title.into(), ok: true, detail: String::new() },
        Err(e) => Step { title: title.into(), ok: false, detail: e.chars().take(200).collect() },
    }
}

const RESTART_ADAPTERS: &str = r#"
$n = 0
Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { "$($_.Status)" -ne 'Disabled' -and "$($_.Status)" -ne 'Not Present' } | ForEach-Object { Restart-NetAdapter -Name $_.Name -Confirm:$false -ErrorAction SilentlyContinue; $n++ }
"$n"
"#;

pub fn network_repair(deep: bool) -> Result<NetRepair, String> {
    wifictl::need_admin()?;
    let before = quick_net_check()?;
    let mut steps = Vec::new();
    let opts = || crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None };
    steps.push(step("Vaciar la caché de DNS", crate::ps::exec_opts("ipconfig.exe", &["/flushdns"], opts())));
    // La tabla ARP se queda con la MAC vieja del router cuando lo cambian o se
    // reinicia: el equipo tiene IP, ve la red y aun así no llega a la puerta de
    // enlace. Vaciarla es inofensivo y se rehace sola.
    steps.push(step("Vaciar la tabla de direcciones de la red (ARP)", crate::ps::exec_opts("netsh.exe", &["interface", "ip", "delete", "arpcache"], opts())));
    if deep {
        steps.push(step("Restablecer Winsock", crate::ps::exec_opts("netsh.exe", &["winsock", "reset"], opts())));
        steps.push(step("Restablecer la pila TCP/IP", crate::ps::exec_opts("netsh.exe", &["int", "ip", "reset"], opts())));
        // Un proxy heredado de otra red deja el equipo «conectado sin Internet».
        steps.push(step("Quitar el proxy de las descargas del sistema", crate::ps::exec_opts("netsh.exe", &["winhttp", "reset", "proxy"], opts())));
    }
    steps.push(step("Reiniciar los adaptadores de red", crate::ps::powershell_opts(RESTART_ADAPTERS, opts())));
    // Tras reiniciar el adaptador, esperar a que vuelva la conexión antes de renovar.
    std::thread::sleep(Duration::from_secs(4));
    // Con IP fija /renew falla sin importancia: se informa como aviso, no como error.
    let renew = crate::ps::exec_opts("ipconfig.exe", &["/renew"], opts());
    steps.push(match renew {
        Ok(_) => step("Renovar la dirección IP", Ok(String::new())),
        Err(_) => Step { title: "Renovar la dirección IP".into(), ok: true, detail: "Algún adaptador tiene IP fija o sin conexión: no se renovó.".into() },
    });
    let start = Instant::now();
    let mut after = quick_net_check()?;
    while !(after.internet && after.dns) && start.elapsed() < Duration::from_secs(25) {
        std::thread::sleep(Duration::from_secs(3));
        after = quick_net_check()?;
    }
    let verdict = verdict_for(&after, deep);
    Ok(NetRepair { before, after, steps, reboot: deep, verdict })
}

#[tauri::command(async)]
pub fn repair_network(tweaks: State<'_, TweakState>, deep: bool) -> Result<NetRepair, String> {
    let r = network_repair(deep);
    let title = if deep { "Reparar la red a fondo" } else { "Reparar la red" };
    tweaks.record(Op::Run, title, &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r
}

// ---------- Wi-Fi ----------

fn wifi_fixes(w: &wifictl::WifiState) -> Vec<Fix> {
    let mut f = Vec::new();
    if w.radio_on == Some(false) {
        f.push(fix("wifi.radio.on", "Encender la Wi-Fi", false));
    }
    for a in w.adapters.iter().filter(|a| a.present) {
        if a.problem != 0 || w.level == "error" {
            f.push(fix(format!("dev.restart:{}", a.instance_id), if a.problem == 22 { "Habilitar la tarjeta" } else { "Reiniciar la tarjeta" }, true));
        }
        if a.power_saving == Some(true) {
            f.push(fix(format!("wifi.power:{}", a.name), "No apagarla para ahorrar energía", true));
        }
    }
    if w.adapters.iter().any(|a| !a.present) {
        f.push(fix("wifi.ghosts", "Quitar adaptadores fantasma", true));
    }
    f
}

fn check_wifi() -> Result<Vec<Finding>, String> {
    let w = wifictl::wifi_state()?;
    let level = match w.level.as_str() {
        "ok" => "ok",
        "off" => "warn",
        "none" => "info",
        _ => "bad",
    };
    let mut out = vec![finding(level, "Tarjeta Wi-Fi", w.summary.clone()).fixes(wifi_fixes(&w)).page("network")];
    if w.level == "ok" {
        out.push(
            finding("info", "Si la Wi-Fi funciona pero va mal", "Acércate al router, prueba la banda de 5 GHz y mira en Mi red y router la señal. Olvidar la red y volver a conectarse arregla contraseñas cambiadas.")
                .fixes(vec![open("ms-settings:network-wifi", "Abrir redes Wi-Fi")])
                .page("router"),
        );
    }
    Ok(out)
}

// ---------- Dispositivos (sonido, Bluetooth) ----------

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct Dev {
    id: String,
    name: String,
    class: String,
    code: u32,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct Svc {
    name: String,
    status: String,
    start: String,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct DevRaw {
    services: Vec<Svc>,
    devices: Vec<Dev>,
}

fn dev_script(classes: &[&str], services: &[&str]) -> String {
    let cls = classes.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(",");
    let svc = services.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(",");
    format!(
        r#"
$svc = @(Get-Service {svc} -ErrorAction SilentlyContinue | ForEach-Object {{ [pscustomobject]@{{ name = "$($_.Name)"; status = "$($_.Status)"; start = "$($_.StartType)" }} }})
$dev = @(Get-CimInstance Win32_PnPEntity -ErrorAction SilentlyContinue | Where-Object {{ @({cls}) -contains "$($_.PNPClass)" }} | ForEach-Object {{ [pscustomobject]@{{ id = "$($_.PNPDeviceID)"; name = "$($_.Name)"; class = "$($_.PNPClass)"; code = [int]$_.ConfigManagerErrorCode }} }})
[pscustomobject]@{{ services = $svc; devices = $dev }} | ConvertTo-Json -Depth 4 -Compress
"#
    )
}

fn dev_raw(classes: &[&str], services: &[&str]) -> Result<DevRaw, String> {
    let out = crate::pspool::query(&dev_script(classes, services), Some(Duration::from_secs(40)), "Solucionar: dispositivos")?;
    serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))
}

fn stopped<'a>(raw: &'a DevRaw, name: &str) -> Option<&'a Svc> {
    raw.services.iter().find(|s| s.name.eq_ignore_ascii_case(name) && !s.status.eq_ignore_ascii_case("Running"))
}

/// Un aviso por dispositivo con error (sin los deshabilitados a propósito: 22).
fn device_problems(devs: &[&Dev], what: &str) -> Vec<Finding> {
    devs.iter()
        .filter(|d| d.code != 0 && d.code != 22 && d.code != 45)
        .map(|d| {
            finding("bad", format!("{what}: {} no funciona", d.name), wifictl::problem_text(d.code))
                .fixes(vec![fix(format!("dev.restart:{}", d.id), "Reiniciar el dispositivo", true)])
                .page("diagnostics")
        })
        .collect()
}

fn check_audio() -> Result<Vec<Finding>, String> {
    let raw = dev_raw(&["MEDIA", "AudioEndpoint"], &["Audiosrv", "AudioEndpointBuilder"])?;
    let mut out = Vec::new();
    let restart = fix("audio.restart", "Reiniciar el audio de Windows", true);
    for s in ["Audiosrv", "AudioEndpointBuilder"] {
        if let Some(s) = stopped(&raw, s) {
            out.push(
                finding("bad", "El servicio de audio de Windows está detenido", format!("{} ({}). Sin él no suena nada.", s.name, s.status))
                    .fixes(vec![restart.clone()]),
            );
            break;
        }
    }
    let cards: Vec<&Dev> = raw.devices.iter().filter(|d| d.class == "MEDIA").collect();
    let endpoints: Vec<&Dev> = raw.devices.iter().filter(|d| d.class == "AudioEndpoint").collect();
    out.extend(device_problems(&cards, "Tarjeta de sonido"));
    if cards.is_empty() && endpoints.is_empty() {
        out.push(finding("bad", "Windows no ve ninguna tarjeta de sonido", "Falta el driver de audio o está desactivado en la BIOS. Instala el driver de sonido del fabricante del equipo.").page("diagnostics"));
    }
    let disabled: Vec<&&Dev> = endpoints.iter().filter(|d| d.code == 22).collect();
    if !disabled.is_empty() {
        out.push(
            finding("warn", "Hay salidas o micrófonos deshabilitados", disabled.iter().map(|d| d.name.as_str()).collect::<Vec<_>>().join(", "))
                .fixes(disabled.iter().map(|d| fix(format!("dev.restart:{}", d.id), &format!("Habilitar {}", short(&d.name)), true)).collect()),
        );
    }
    if !out.iter().any(|f| f.level == "bad") {
        out.insert(
            0,
            finding("ok", "El audio de Windows funciona", format!("{} dispositivos de sonido. Si no suena: revisa el volumen y la salida elegida (altavoces, auriculares, HDMI) y que la app no esté silenciada en el mezclador.", endpoints.len()))
                .fixes(vec![open("ms-settings:sound", "Configuración de sonido"), open("ms-settings:apps-volume", "Mezclador de volumen"), restart]),
        );
    }
    Ok(out)
}

fn short(s: &str) -> String {
    let s: String = s.chars().take(40).collect();
    s
}

fn check_bluetooth() -> Result<Vec<Finding>, String> {
    let raw = dev_raw(&["Bluetooth"], &["bthserv"])?;
    let mut out = Vec::new();
    let devs: Vec<&Dev> = raw.devices.iter().collect();
    if devs.is_empty() {
        out.push(finding("info", "Este equipo no tiene Bluetooth", "O está desactivado en la BIOS o en un interruptor del portátil. Un adaptador USB Bluetooth es la solución más barata."));
        return Ok(out);
    }
    if let Some(s) = stopped(&raw, "bthserv") {
        out.push(
            finding("bad", "El servicio de Bluetooth está detenido", format!("Estado: {} · Inicio: {}", s.status, s.start))
                .fixes(vec![fix("bt.service", "Iniciar el servicio de Bluetooth", true)]),
        );
    }
    out.extend(device_problems(&devs, "Bluetooth"));
    match wifictl::radio_kind("Bluetooth", "") {
        Some(false) => out.push(finding("warn", "El Bluetooth está apagado", "Está desactivado en Windows (o en modo avión).").fixes(vec![fix("bt.radio.on", "Encender el Bluetooth", false)])),
        None if out.is_empty() => out.push(finding("warn", "Windows no ve la radio Bluetooth", "El adaptador existe pero no responde. Prueba a reiniciarlo.")),
        _ => {}
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(
            0,
            finding("ok", "El Bluetooth funciona", "Si un aparato concreto no conecta: quítalo en Configuración, ponlo en modo emparejamiento y vuelve a añadirlo. Los auriculares suelen fallar si están conectados a otro dispositivo a la vez.")
                .fixes(vec![open("ms-settings:bluetooth", "Configuración de Bluetooth")]),
        );
    }
    Ok(out)
}

// ---------- Pantalla ----------

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct Gpu {
    name: String,
    code: u32,
    driver: String,
    driver_date: String,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct DisplayRaw {
    gpus: Vec<Gpu>,
    monitors: Vec<String>,
    tdr: u32,
}

const DISPLAY_SCRIPT: &str = r#"
$gpus = @(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ name = "$($_.Name)"; code = [int]$_.ConfigManagerErrorCode; driver = "$($_.DriverVersion)"; driverDate = if ($_.DriverDate) { $_.DriverDate.ToString('yyyy-MM-dd') } else { '' } } })
$mon = @(Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue | ForEach-Object { $n = -join ($_.UserFriendlyName | Where-Object { $_ -gt 0 } | ForEach-Object { [char]$_ }); if ($n) { $n } else { 'Pantalla' } })
$tdr = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Display'; Id = 4101; StartTime = (Get-Date).AddDays(-7) } -MaxEvents 50 -ErrorAction SilentlyContinue).Count
[pscustomobject]@{ gpus = $gpus; monitors = $mon; tdr = $tdr } | ConvertTo-Json -Depth 4 -Compress
"#;

fn check_display() -> Result<Vec<Finding>, String> {
    let out = crate::pspool::query(DISPLAY_SCRIPT, Some(Duration::from_secs(40)), "Solucionar: pantalla")?;
    let raw: DisplayRaw = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let reset = fix_confirm("display.reset", "Reiniciar el driver de la gráfica", false, "La pantalla se pondrá negra un segundo y se oirá un pitido: es normal.");
    let project = fix("display.project", "Elegir cómo usar las pantallas (Win+P)", false);
    let mut out = Vec::new();
    for g in &raw.gpus {
        let l = g.name.to_lowercase();
        if l.contains("basic display") || l.contains("básico de microsoft") || l.contains("basico de microsoft") {
            out.push(
                finding("bad", "La tarjeta gráfica no tiene driver", format!("Windows usa «{}»: sin driver no hay buena resolución, varios monitores ni aceleración. Instala el driver del fabricante (Intel, AMD, NVIDIA o el del portátil).", g.name))
                    .page("diagnostics"),
            );
        } else if g.code != 0 {
            out.push(finding("bad", format!("{} no funciona", g.name), wifictl::problem_text(g.code)).fixes(vec![reset.clone()]).page("diagnostics"));
        }
    }
    if raw.tdr > 0 {
        out.push(finding("warn", "El driver de la gráfica se ha colgado estos días", format!("{} veces en los últimos 7 días (pantalla negra o parpadeo). Actualiza el driver y revisa la temperatura.", raw.tdr)).page("hardware"));
    }
    let old = raw.gpus.iter().filter(|g| g.driver_date.len() == 10 && g.driver_date.as_str() < two_years_ago().as_str()).collect::<Vec<_>>();
    for g in old {
        out.push(finding("info", format!("El driver de {} es antiguo", g.name), format!("Versión {} del {}. Si hay problemas de pantalla, actualízalo.", g.driver, g.driver_date)));
    }
    let monitors = if raw.monitors.is_empty() { "No se pudieron leer los monitores.".to_string() } else { format!("{} pantalla(s): {}.", raw.monitors.len(), raw.monitors.join(", ")) };
    if !out.iter().any(|f| f.level == "bad") {
        out.insert(
            0,
            finding("ok", "La gráfica funciona", format!("{monitors} Si un monitor no se ve: revisa el cable y la entrada del monitor, pulsa Win+P o reinicia el driver de la gráfica."))
                .fixes(vec![reset, project, open("ms-settings:display", "Configuración de pantalla")]),
        );
    }
    Ok(out)
}

fn two_years_ago() -> String {
    (chrono::Local::now() - chrono::Duration::days(730)).format("%Y-%m-%d").to_string()
}

// ---------- Impresoras ----------

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct PrinterRaw {
    name: String,
    default: bool,
    offline: bool,
    error: u32,
    jobs: u32,
    stuck: u32,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct PrintRaw {
    spooler: String,
    printers: Vec<PrinterRaw>,
}

const PRINT_SCRIPT: &str = r#"
$sp = Get-Service Spooler -ErrorAction SilentlyContinue
$jobs = @(Get-CimInstance Win32_PrintJob -ErrorAction SilentlyContinue)
$pr = @(if ("$($sp.Status)" -eq 'Running') { Get-CimInstance Win32_Printer -ErrorAction SilentlyContinue | ForEach-Object {
  $n = "$($_.Name)"
  $mine = @($jobs | Where-Object { "$($_.Name)" -like "$n,*" })
  [pscustomobject]@{ name = $n; default = [bool]$_.Default; offline = [bool]$_.WorkOffline -or [int]$_.PrinterStatus -eq 7; error = [int]$_.DetectedErrorState; jobs = $mine.Count
    stuck = @($mine | Where-Object { "$($_.JobStatus)" -match 'Error|Blocked|Bloq' -or ($_.TimeSubmitted -and ((Get-Date) - $_.TimeSubmitted).TotalMinutes -gt 10) }).Count }
} })
[pscustomobject]@{ spooler = "$($sp.Status)"; printers = $pr } | ConvertTo-Json -Depth 4 -Compress
"#;

fn is_virtual(name: &str) -> bool {
    let l = name.to_lowercase();
    ["pdf", "xps", "onenote", "fax"].iter().any(|v| l.contains(v))
}

fn check_printer() -> Result<Vec<Finding>, String> {
    let out = crate::pspool::query(PRINT_SCRIPT, Some(Duration::from_secs(40)), "Solucionar: impresoras")?;
    let raw: PrintRaw = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let queue = fix("print.queue", "Desatascar la cola de impresión", true);
    let mut out = Vec::new();
    if !raw.spooler.eq_ignore_ascii_case("Running") {
        out.push(finding("bad", "La cola de impresión de Windows está detenida", "Sin ella no se puede imprimir con ninguna impresora.").fixes(vec![queue]).page("printers"));
        return Ok(out);
    }
    let real: Vec<&PrinterRaw> = raw.printers.iter().filter(|p| !is_virtual(&p.name)).collect();
    if real.is_empty() {
        out.push(finding("warn", "No hay ninguna impresora instalada", "Solo hay impresoras virtuales (PDF, XPS…). Añade la impresora desde Configuración o desde el programa del fabricante.").fixes(vec![open("ms-settings:printers", "Impresoras y escáneres")]).page("printers"));
    }
    if let Some(d) = raw.printers.iter().find(|p| p.default) {
        if is_virtual(&d.name) && !real.is_empty() {
            out.push(finding("warn", "La impresora predeterminada no es una impresora real", format!("Es «{}»: al imprimir se genera un archivo en vez de papel.", d.name)).page("printers"));
        }
    }
    for p in &real {
        if p.stuck > 0 {
            out.push(finding("bad", format!("Hay trabajos atascados en {}", p.name), format!("{} trabajo(s) bloqueados: impiden imprimir lo siguiente.", p.stuck)).fixes(vec![queue.clone()]).page("printers"));
        } else if p.offline {
            out.push(
                finding("warn", format!("{} está sin conexión", p.name), "Comprueba que esté encendida y conectada (cable o Wi-Fi). Si cambió de IP, hay que volver a añadirla.")
                    .fixes(vec![open("ms-settings:printers", "Impresoras y escáneres")])
                    .page("printers"),
            );
        } else if p.error > 2 {
            out.push(finding("warn", format!("{} informa de un problema", p.name), printer_error(p.error)).page("printers"));
        }
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(0, finding("ok", "Las impresoras están bien", format!("{} impresora(s). Imprime una página de prueba desde Impresoras.", real.len())).fixes(vec![queue]).page("printers"));
    }
    Ok(out)
}

fn printer_error(code: u32) -> String {
    match code {
        3 => "Poco papel.",
        4 => "Sin papel.",
        5 => "Poco tóner o tinta.",
        6 => "Sin tóner o tinta.",
        7 => "Tapa abierta.",
        8 => "Papel atascado.",
        9 => "Sin conexión.",
        10 => "Necesita atención (revisa la pantalla de la impresora).",
        _ => "Revisa la pantalla de la impresora.",
    }
    .into()
}

// ---------- Lentitud ----------

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct SlowRaw {
    uptime_days: f64,
    free_gb: f64,
    total_gb: f64,
    media: String,
    power_saver: bool,
    pending_reboot: bool,
}

const SLOW_SCRIPT: &str = r#"
$os = Get-CimInstance Win32_OperatingSystem
$sys = $env:SystemDrive.TrimEnd(':')
$v = Get-Volume -DriveLetter $sys -ErrorAction SilentlyContinue
$media = ''
try { $dn = (Get-Partition -DriveLetter $sys -ErrorAction Stop | Get-Disk -ErrorAction Stop).Number; $media = "$((Get-PhysicalDisk -ErrorAction Stop | Where-Object { "$($_.DeviceId)" -eq "$dn" } | Select-Object -First 1).MediaType)" } catch { }
$scheme = "$(powercfg.exe /getactivescheme)"
$pending = (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')
[pscustomobject]@{ uptimeDays = ((Get-Date) - $os.LastBootUpTime).TotalDays; freeGb = if ($v) { $v.SizeRemaining / 1GB } else { 0 }; totalGb = if ($v) { $v.Size / 1GB } else { 0 }; media = $media; powerSaver = $scheme -match 'a1841308-3541-4fab-bc81-f71556f20b4a'; pendingReboot = [bool]$pending } | ConvertTo-Json -Compress
"#;

fn check_slow() -> Result<Vec<Finding>, String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    // Los programas de inicio tardan: se leen a la vez que lo demás.
    let startup = std::thread::spawn(crate::tweaks::startup::enabled_names);
    let out = crate::pspool::query(SLOW_SCRIPT, Some(Duration::from_secs(40)), "Solucionar: lentitud")?;
    let raw: SlowRaw = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let mut sys = System::new();
    let kind = ProcessRefreshKind::nothing().with_cpu().with_memory();
    sys.refresh_cpu_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    std::thread::sleep(Duration::from_millis(1200));
    sys.refresh_cpu_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    sys.refresh_memory();
    let cpu = sys.global_cpu_usage();
    let mem = sys.used_memory() as f64 / sys.total_memory().max(1) as f64 * 100.0;
    let cores = sys.cpus().len().max(1) as f32;
    let top_cpu = sys.processes().values().max_by(|a, b| a.cpu_usage().total_cmp(&b.cpu_usage())).map(|p| (p.name().to_string_lossy().to_string(), p.cpu_usage() / cores));
    let top_mem = sys.processes().values().max_by_key(|p| p.memory()).map(|p| (p.name().to_string_lossy().to_string(), p.memory() as f64 / 1024f64.powi(3)));

    let mut out = Vec::new();
    if cpu > 80.0 {
        let who = top_cpu.map(|(n, c)| format!(" El que más consume: {n} ({c:.0} %).")).unwrap_or_default();
        out.push(finding("bad", format!("El procesador está al {cpu:.0} %"), format!("Algo lo está saturando.{who}")).page("processes"));
    }
    if mem > 85.0 {
        let who = top_mem.map(|(n, g)| format!(" El que más usa: {n} ({g:.1} GB).")).unwrap_or_default();
        out.push(finding("bad", format!("La memoria está al {mem:.0} %"), format!("Windows tiene que usar el disco como memoria y todo va lento.{who} Si pasa siempre, amplía la RAM.")).page("processes"));
    }
    if raw.total_gb > 0.0 && (raw.free_gb < 10.0 || raw.free_gb / raw.total_gb < 0.1) {
        out.push(
            finding("bad", "Queda poco espacio en el disco del sistema", format!("{:.1} GB libres de {:.0} GB. Windows necesita espacio libre para ir fluido y actualizarse.", raw.free_gb, raw.total_gb))
                .fixes(vec![fix("cleanup.temp", "Borrar archivos temporales", false)])
                .page("space"),
        );
    }
    if raw.media.eq_ignore_ascii_case("HDD") {
        out.push(finding("warn", "El disco del sistema es mecánico (HDD)", "Es la causa más habitual de lentitud en equipos de hace unos años. Cambiarlo por un SSD es la mejora más notable que se puede hacer.").page("hardware"));
    }
    if raw.power_saver {
        out.push(finding("warn", "Está activo el plan de energía «Economizador»", "Limita el procesador para ahorrar batería.").fixes(vec![fix("power.balanced", "Usar el plan Equilibrado", false)]));
    }
    if raw.uptime_days > 7.0 {
        out.push(finding("warn", format!("Lleva {:.0} días sin reiniciar", raw.uptime_days), "«Apagar» con el inicio rápido no reinicia de verdad. Un reinicio libera memoria y termina actualizaciones."));
    } else if raw.pending_reboot {
        out.push(finding("warn", "Hay un reinicio pendiente", "Windows necesita reiniciar para terminar de instalar actualizaciones."));
    }
    if let Ok(Ok(names)) = startup.join() {
        if names.len() > 12 {
            out.push(finding("warn", format!("Arrancan {} programas con Windows", names.len()), "Cuantos más, más tarda en arrancar y más memoria se usa. Desactiva los que no hagan falta.").page("startup"));
        }
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(
            0,
            finding("ok", "No se ve nada que lo frene ahora mismo", format!("Procesador al {cpu:.0} %, memoria al {mem:.0} %, {:.0} GB libres. Si la lentitud es a ratos, mira Procesos cuando ocurra o pasa un Diagnóstico completo.", raw.free_gb))
                .page("diagnostics"),
        );
    }
    Ok(out)
}

// ---------- Windows Update ----------

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct WuRaw {
    wu_start: String,
    failures: u32,
    last_code: String,
    free_gb: f64,
    pending_reboot: bool,
}

const WU_SCRIPT: &str = r#"
$wu = Get-Service wuauserv -ErrorAction SilentlyContinue
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-WindowsUpdateClient'; Id = 20; StartTime = (Get-Date).AddDays(-30) } -MaxEvents 50 -ErrorAction SilentlyContinue)
$code = if ($ev.Count) { "$($ev[0].Properties[0].Value)" } else { '' }
$v = Get-Volume -DriveLetter $env:SystemDrive.TrimEnd(':') -ErrorAction SilentlyContinue
$pending = (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')
[pscustomobject]@{ wuStart = "$($wu.StartType)"; failures = $ev.Count; lastCode = $code; freeGb = if ($v) { $v.SizeRemaining / 1GB } else { 0 }; pendingReboot = [bool]$pending } | ConvertTo-Json -Compress
"#;

fn check_winupdate() -> Result<Vec<Finding>, String> {
    let out = crate::pspool::query(WU_SCRIPT, Some(Duration::from_secs(40)), "Solucionar: Windows Update")?;
    let raw: WuRaw = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let repair = fix_confirm("wu.repair", "Reparar Windows Update", true, "Detiene las actualizaciones y regenera su caché. Se pierde el historial que muestra Configuración (no las actualizaciones instaladas).");
    let mut out = Vec::new();
    if raw.wu_start.eq_ignore_ascii_case("Disabled") {
        out.push(finding("bad", "El servicio de Windows Update está deshabilitado", "Alguien (o un programa «optimizador») lo desactivó: el equipo no recibe parches de seguridad.").page("services"));
    }
    if raw.failures > 0 {
        let code = u32::try_from(raw.last_code.trim().parse::<i64>().unwrap_or(0) & 0xFFFF_FFFF).map(|c| format!(" Último error: 0x{c:08X}.")).unwrap_or_default();
        out.push(finding("warn", format!("{} actualizaciones fallidas en 30 días", raw.failures), format!("Windows no consigue instalar alguna actualización.{code} En Windows Update se ve cuál y qué significa el error.")).fixes(vec![repair.clone(), fix("time.sync", "Sincronizar la hora", true)]).page("winupdate"));
    }
    if raw.free_gb > 0.0 && raw.free_gb < 15.0 {
        out.push(finding("warn", "Poco espacio para actualizar", format!("{:.1} GB libres: las actualizaciones grandes necesitan 15-20 GB.", raw.free_gb)).fixes(vec![fix("cleanup.temp", "Borrar archivos temporales", false)]).page("space"));
    }
    if raw.pending_reboot {
        out.push(finding("warn", "Hay un reinicio pendiente", "Reinicia para que se terminen de instalar; mientras, no se instalarán más."));
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(0, finding("ok", "Windows Update no muestra problemas", "Sin fallos en los últimos 30 días.").fixes(vec![repair]).page("winupdate"));
    }
    Ok(out)
}

// ---------- Comandos ----------

#[tauri::command(async)]
pub fn troubleshoot_check(symptom: String) -> Result<CheckResult, String> {
    let findings = match symptom.as_str() {
        "internet" => check_internet(),
        "wifi" => check_wifi(),
        "audio" => check_audio(),
        "bluetooth" => check_bluetooth(),
        "display" => check_display(),
        "printer" => check_printer().map(|mut v| {
            v.extend(crate::fixes::printdeep::check());
            drop_stale_ok(v)
        }),
        "slow" => check_slow(),
        "winupdate" => check_winupdate().map(|mut v| {
            v.extend(crate::fixes::wudeep::check());
            drop_stale_ok(v)
        }),
        other => crate::fixes::check(other).unwrap_or_else(|| Err("Síntoma desconocido.".into())),
    }?;
    Ok(CheckResult { symptom, findings })
}

/// Si lo añadido a fondo encontró problemas, el «todo bien» de la comprobación básica sobra.
fn drop_stale_ok(mut v: Vec<Finding>) -> Vec<Finding> {
    if v.iter().any(|f| f.level == "bad" || f.level == "warn") {
        v.retain(|f| f.level != "ok");
    }
    v
}

fn allowed_uri(uri: &str) -> bool {
    uri.starts_with("ms-settings:") && uri.len() < 60 && uri.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-')
}

fn run_fix(tweaks: &TweakState, id: &str) -> Result<String, String> {
    let (kind, arg) = id.split_once(':').unwrap_or((id, ""));
    let hotkey = |codes: &[u16]| {
        std::thread::sleep(Duration::from_millis(250));
        crate::keys::send(codes)
    };
    Ok(match kind {
        "net.quick" | "net.deep" => {
            let r = network_repair(kind == "net.deep")?;
            let ok = r.after.internet && r.after.dns;
            match (ok, r.reboot) {
                (true, true) => "Red reparada. Reinicia el equipo para completar el restablecimiento.".into(),
                (true, false) => "Red reparada: hay Internet.".into(),
                (false, true) => "Hecho. Reinicia el equipo: el restablecimiento se completa al reiniciar.".into(),
                (false, false) => "Hecho, pero sigue sin Internet: prueba a reiniciar el router o la reparación a fondo.".into(),
            }
        }
        "net.flushdns" => {
            crate::ps::exec("ipconfig.exe", &["/flushdns"])?;
            "Caché de DNS vaciada.".into()
        }
        "proxy.off" => {
            crate::ps::powershell(
                r"$k = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
Set-ItemProperty $k -Name ProxyEnable -Value 0
Remove-ItemProperty $k -Name AutoConfigURL -ErrorAction SilentlyContinue
'ok'",
            )?;
            "Proxy quitado.".into()
        }
        "wifi.radio.on" | "wifi.radio.off" => {
            wifictl::radio_kind("WiFi", if kind.ends_with("on") { "On" } else { "Off" }).ok_or("No se pudo cambiar la Wi-Fi.")?;
            if kind.ends_with("on") { "Wi-Fi encendida." } else { "Wi-Fi apagada." }.into()
        }
        "bt.radio.on" | "bt.radio.off" => {
            wifictl::radio_kind("Bluetooth", if kind.ends_with("on") { "On" } else { "Off" }).ok_or("No se pudo cambiar el Bluetooth.")?;
            if kind.ends_with("on") { "Bluetooth encendido." } else { "Bluetooth apagado." }.into()
        }
        "bt.service" => {
            wifictl::need_admin()?;
            crate::ps::powershell("$ErrorActionPreference = 'Stop'\nif ((Get-Service bthserv).StartType -eq 'Disabled') { Set-Service bthserv -StartupType Manual }\nStart-Service bthserv\n'ok'")?;
            "Servicio de Bluetooth iniciado.".into()
        }
        "dev.restart" => {
            wifictl::restart_device(arg)?;
            "Dispositivo reiniciado.".into()
        }
        "wifi.power" => {
            wifictl::need_admin()?;
            wifictl::check_id(arg)?;
            crate::ps::powershell(&format!("$ErrorActionPreference = 'Stop'\n{}Set-NetAdapterPowerManagement -Name $n -AllowComputerToTurnOffDevice Disabled\n'ok'", crate::ps::text_var("n", arg)))?;
            "Windows ya no apagará la tarjeta para ahorrar energía.".into()
        }
        "wifi.ghosts" => {
            wifictl::need_admin()?;
            let state = wifictl::wifi_state()?;
            let mut n = 0;
            for g in state.adapters.iter().filter(|a| !a.present) {
                if crate::ps::exec("pnputil.exe", &["/remove-device", &g.instance_id]).is_ok() {
                    n += 1;
                }
            }
            format!("{n} adaptador(es) fantasma quitados.")
        }
        "audio.restart" => tweaks.run_catalog("repair.audio")?,
        "print.queue" => tweaks.run_catalog("repair.print-queue")?,
        "wu.repair" => tweaks.run_catalog("repair.windows-update")?,
        "time.sync" => tweaks.run_catalog("repair.time-sync")?,
        "cleanup.temp" => tweaks.run_catalog("cleanup.user-temp")?,
        "power.balanced" => {
            crate::ps::exec("powercfg.exe", &["/setactive", "381b4222-f694-41f0-9685-ff5bb260df2e"])?;
            "Plan Equilibrado activado.".into()
        }
        // Win+Ctrl+Shift+B: Windows reinicia el driver de vídeo sin cerrar nada.
        "display.reset" => {
            hotkey(&[0x5B, 0x11, 0x10, b'B' as u16])?;
            "Driver de la gráfica reiniciado.".into()
        }
        "display.project" => {
            hotkey(&[0x5B, b'P' as u16])?;
            "Elige el modo en el panel de Windows.".into()
        }
        "open" if allowed_uri(arg) => {
            crate::shellopen::open(arg)?;
            return Ok(String::new());
        }
        _ => return crate::fixes::run(tweaks, kind, arg).unwrap_or_else(|| Err("Reparación desconocida.".into())),
    })
}

fn fix_title(id: &str) -> &'static str {
    match id.split(':').next().unwrap_or("") {
        "net.quick" => "Reparar la red",
        "net.deep" => "Reparar la red a fondo",
        "net.flushdns" => "Vaciar la caché de DNS",
        "proxy.off" => "Quitar el proxy",
        "wifi.radio.on" => "Encender la Wi-Fi",
        "wifi.radio.off" => "Apagar la Wi-Fi",
        "bt.radio.on" => "Encender el Bluetooth",
        "bt.radio.off" => "Apagar el Bluetooth",
        "bt.service" => "Iniciar el servicio de Bluetooth",
        "dev.restart" => "Reiniciar un dispositivo",
        "wifi.power" => "Wi-Fi: desactivar el ahorro de energía de la tarjeta",
        "wifi.ghosts" => "Quitar adaptadores Wi-Fi fantasma",
        "power.balanced" => "Plan de energía Equilibrado",
        "display.reset" => "Reiniciar el driver de la gráfica",
        other => crate::fixes::title(other),
    }
}

#[tauri::command(async)]
pub fn troubleshoot_fix(tweaks: State<'_, TweakState>, id: String) -> Result<String, String> {
    let r = run_fix(&tweaks, &id);
    // Las del catálogo ya quedan en el diario; abrir Configuración no es un cambio.
    let title = fix_title(&id);
    if !title.is_empty() {
        tweaks.record(Op::Run, title, &r.as_ref().map(|_| ()).map_err(Clone::clone));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYMPTOMS: &[&str] = &["internet", "wifi", "audio", "bluetooth", "display", "printer", "slow", "winupdate"];

    /// El diagnóstico de la red señala el primer eslabón roto de la cadena, no
    /// el último síntoma: es la diferencia entre «no hay Internet» y «el router
    /// no te está dando dirección, reinícialo».
    #[test]
    fn network_verdict_points_at_the_broken_link() {
        let base = NetCheck {
            connected: true,
            adapter: "Ethernet".into(),
            gateway: Some(true),
            internet: true,
            dns: true,
            ip: "192.168.1.50".into(),
            gateway_ip: "192.168.1.1".into(),
            dhcp: true,
            ..Default::default()
        };
        // Todo bien.
        assert_eq!(verdict_for(&base, false).level, "ok");

        // Sin tarjeta: ni se mira el resto.
        let v = verdict_for(&NetCheck::default(), false);
        assert_eq!(v.level, "bad");
        assert!(v.title.contains("tarjeta"), "{}", v.title);

        // 169.254: lo dice el router, no el equipo.
        let v = verdict_for(&NetCheck { no_dhcp_address: true, connected: false, ..base.clone() }, false);
        assert!(v.title.contains("router"), "{}", v.title);
        assert_eq!(v.next, vec!["router"]);

        // Con IP pero el router no contesta.
        let v = verdict_for(&NetCheck { gateway: Some(false), ..base.clone() }, false);
        assert_eq!(v.level, "bad");
        assert!(v.text.contains("192.168.1.1"), "debe decir cuál es la puerta de enlace: {}", v.text);

        // Llega al router pero no sale: el problema no es del equipo.
        let v = verdict_for(&NetCheck { internet: false, ..base.clone() }, false);
        assert_eq!(v.level, "warn");
        assert!(v.text.contains("no es de este equipo"), "{}", v.text);

        // Hay Internet pero no resuelve nombres: se proponen otros DNS.
        let v = verdict_for(&NetCheck { dns: false, dns_servers: vec!["10.0.0.9".into()], ..base.clone() }, false);
        assert_eq!(v.next, vec!["dns"]);
        assert!(v.text.contains("10.0.0.9"), "debe decir qué DNS se están usando: {}", v.text);

        // Todo responde pero queda un proxy de otra red.
        let v = verdict_for(&NetCheck { proxy: "proxy.empresa.local:8080".into(), ..base.clone() }, false);
        assert_eq!(v.level, "warn");
        assert!(v.text.contains("proxy.empresa.local:8080"), "{}", v.text);

        // Tras la reparación a fondo se avisa del reinicio.
        assert!(verdict_for(&base, true).text.contains("Reinicia"));
    }

    #[test]
    fn scripts_parse() {
        let all = [
            ("NET", NET_SCRIPT.to_string()),
            ("DEV", dev_script(&["MEDIA", "AudioEndpoint"], &["Audiosrv"])),
            ("DISPLAY", DISPLAY_SCRIPT.to_string()),
            ("PRINT", PRINT_SCRIPT.to_string()),
            ("SLOW", SLOW_SCRIPT.to_string()),
            ("WU", WU_SCRIPT.to_string()),
            ("RESTART", RESTART_ADAPTERS.to_string()),
        ];
        for (name, s) in all {
            let e = crate::ps::parse_errors(&s);
            assert!(e.is_empty(), "{name}: {e}");
        }
    }

    #[test]
    fn only_settings_uris_open() {
        assert!(allowed_uri("ms-settings:sound"));
        assert!(allowed_uri("ms-settings:network-wifi"));
        assert!(!allowed_uri("file:///C:/x.exe"));
        assert!(!allowed_uri("ms-settings:x & calc"));
        assert!(!allowed_uri("C:\\Windows\\notepad.exe"));
    }

    #[test]
    fn every_fix_is_known() {
        // Cada botón que pueden ofrecer las comprobaciones tiene su reparación.
        for id in ["net.quick", "net.deep", "net.flushdns", "proxy.off", "wifi.radio.on", "bt.radio.on", "bt.service", "wifi.ghosts", "power.balanced", "display.reset"] {
            assert!(!fix_title(id).is_empty(), "{id}");
        }
    }

    /// Equipo real (solo lectura): `cargo test troubleshoot_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn troubleshoot_real() {
        for s in SYMPTOMS {
            let t = Instant::now();
            match troubleshoot_check(s.to_string()) {
                Ok(r) => {
                    println!("== {s} ({} ms)", t.elapsed().as_millis());
                    for f in r.findings {
                        println!("  [{}] {} · {} {:?}", f.level, f.title, f.detail, f.fixes.iter().map(|x| &x.id).collect::<Vec<_>>());
                    }
                }
                Err(e) => println!("== {s}: ERROR {e}"),
            }
        }
    }
}
