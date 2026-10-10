//! Correo que no sale o no llega, para dominios propios: registros MX, SPF,
//! DKIM y DMARC del dominio, conexión a los servidores de salida y entrada con
//! los puertos que tocan, y si la IP de la oficina está en una lista negra.
//! Dice por qué los correos acaban en spam o se rechazan, sin siglas.

use serde::{Deserialize, Serialize};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// ok · info · warn · bad
    pub level: &'static str,
    pub what: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PortCheck {
    pub host: String,
    pub port: u16,
    pub label: String,
    pub ok: bool,
    pub ms: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MailReport {
    pub domain: String,
    pub mx: Vec<String>,
    pub spf: Option<String>,
    pub dmarc: Option<String>,
    pub dkim: Vec<String>,
    pub ports: Vec<PortCheck>,
    pub public_ip: String,
    pub blacklists: Vec<(String, String)>,
    pub lines: Vec<Line>,
    /// Texto listo para mandar al proveedor del dominio o del correo.
    pub provider_text: String,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct DnsRaw {
    mx: Vec<String>,
    txt: Vec<String>,
    dmarc: Vec<String>,
    dkim: Vec<String>,
    lists: Vec<DnsList>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct DnsList {
    name: String,
    answer: String,
}

/// Selectores de DKIM habituales (Microsoft 365, Google, cPanel y otros).
const SELECTORS: &[&str] = &["selector1", "selector2", "google", "default", "k1", "s1", "s2", "mail", "dkim", "smtp"];
const LISTS: &[&str] = &["zen.spamhaus.org", "bl.spamcop.net", "b.barracudacentral.org"];

const SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$mx = @(Resolve-DnsName -Name $domain -Type MX -DnsOnly | Where-Object { $_.NameExchange } | Sort-Object Preference | ForEach-Object { "$($_.NameExchange)" })
$txt = @(Resolve-DnsName -Name $domain -Type TXT -DnsOnly | Where-Object { $_.Strings } | ForEach-Object { $_.Strings -join '' })
$dmarc = @(Resolve-DnsName -Name "_dmarc.$domain" -Type TXT -DnsOnly | Where-Object { $_.Strings } | ForEach-Object { $_.Strings -join '' })
$dkim = @(foreach ($s in $selectors) { $r = Resolve-DnsName -Name "$s._domainkey.$domain" -Type TXT -DnsOnly | Where-Object { $_.Strings -or $_.NameHost }; if ($r) { $s } })
$lists = @(foreach ($l in $lists) { if ($rev) { $a = Resolve-DnsName -Name "$rev.$l" -Type A -DnsOnly | Where-Object { $_.IPAddress } | Select-Object -First 1; [pscustomobject]@{ name = $l; answer = "$($a.IPAddress)" } } })
[pscustomobject]@{ mx = $mx; txt = $txt; dmarc = $dmarc; dkim = $dkim; lists = $lists } | ConvertTo-Json -Depth 4 -Compress
"#;

/// Política final de un registro SPF: «-all» (estricta), «~all» (suave), «?all» o «+all».
pub fn spf_all(spf: &str) -> Option<&'static str> {
    let s = spf.to_ascii_lowercase();
    ["-all", "~all", "?all", "+all"].into_iter().find(|a| s.split_whitespace().any(|w| w == *a))
}

/// Los «include:» de un SPF (qué servicios pueden enviar en nombre del dominio).
pub fn spf_includes(spf: &str) -> Vec<String> {
    spf.split_whitespace().filter_map(|w| w.strip_prefix("include:")).map(String::from).collect()
}

/// Política de DMARC (p=none, quarantine o reject).
pub fn dmarc_policy(dmarc: &str) -> Option<String> {
    dmarc.split(';').map(str::trim).find_map(|kv| kv.strip_prefix("p=")).map(|v| v.trim().to_ascii_lowercase())
}

/// «203.0.113.7» → «7.113.0.203» (para preguntar a las listas negras).
pub fn reverse_ip(ip: &str) -> Option<String> {
    let parts: Vec<&str> = ip.split('.').collect();
    (parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok())).then(|| parts.iter().rev().copied().collect::<Vec<_>>().join("."))
}

/// Qué significa la respuesta de una lista negra.
pub fn list_verdict(answer: &str) -> &'static str {
    if answer.is_empty() {
        "limpia"
    } else if answer.starts_with("127.255.255.") || answer == "127.0.0.255" {
        "no se pudo consultar"
    } else if answer.starts_with("127.") {
        "en la lista"
    } else {
        "no se pudo consultar"
    }
}

/// Valida un dominio sencillo (sin espacios ni rarezas que vayan a un script).
pub fn valid_domain(d: &str) -> bool {
    let d = d.trim();
    d.len() >= 4 && d.len() < 200 && d.contains('.') && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && !d.starts_with('.') && !d.ends_with('.')
}

/// Conclusiones a partir de lo leído del DNS.
pub fn verdict(domain: &str, mx: &[String], txt: &[String], dmarc: &[String], dkim: &[String]) -> (Option<String>, Option<String>, Vec<Line>) {
    let mut lines = Vec::new();
    let spf = txt.iter().find(|t| t.to_ascii_lowercase().starts_with("v=spf1")).cloned();
    let spf_count = txt.iter().filter(|t| t.to_ascii_lowercase().starts_with("v=spf1")).count();
    let dm = dmarc.iter().find(|t| t.to_ascii_lowercase().starts_with("v=dmarc1")).cloned();
    let l = |level, what: &str, text: String| Line { level, what: what.into(), text };
    if mx.is_empty() {
        lines.push(l("bad", "MX", format!("{domain} no tiene registros MX: nadie sabe a qué servidor entregar su correo. Sin esto no llega ningún correo.")));
    } else {
        lines.push(l("ok", "MX", format!("El correo de {domain} se entrega a {}.", mx.join(", "))));
    }
    match (&spf, spf_count) {
        (None, _) => lines.push(l("bad", "SPF", "No hay SPF: no se dice qué servidores pueden enviar en nombre del dominio, y Gmail y Outlook mandan esos correos a spam o los rechazan.".into())),
        (Some(_), n) if n > 1 => lines.push(l("bad", "SPF", "Hay más de un registro SPF: es como no tener ninguno. Hay que juntarlos en uno solo.".into())),
        (Some(s), _) => {
            let inc = spf_includes(s);
            match spf_all(s) {
                Some("+all") => lines.push(l("bad", "SPF", "El SPF termina en «+all»: permite que cualquiera envíe en nombre del dominio. Cámbialo a «~all» o «-all».".into())),
                Some("?all") | None => lines.push(l("warn", "SPF", format!("El SPF existe pero no dice qué hacer con el resto ({}). Termínalo en «~all».", if inc.is_empty() { "sin servicios incluidos".to_string() } else { inc.join(", ") }))),
                Some(a) => lines.push(l("ok", "SPF", format!("Correcto ({a}). Pueden enviar: {}. Si usas otro servicio para boletines o facturas, tiene que estar aquí.", if inc.is_empty() { "solo los servidores indicados".to_string() } else { inc.join(", ") }))),
            }
        }
    }
    if dkim.is_empty() {
        lines.push(l("warn", "DKIM", "No se encontró la firma DKIM con los nombres habituales. Puede tenerla con otro nombre; si no, el proveedor del correo la activa en un minuto.".into()));
    } else {
        lines.push(l("ok", "DKIM", format!("Firma encontrada ({}).", dkim.join(", "))));
    }
    match dm.as_deref().and_then(dmarc_policy).as_deref() {
        None => lines.push(l("warn", "DMARC", "No hay DMARC: cualquiera puede suplantar el dominio con más facilidad y algunos servidores desconfían de sus correos.".into())),
        Some("none") => lines.push(l("info", "DMARC", "DMARC en modo «solo vigilar» (p=none). Bien para empezar; con SPF y DKIM correctos, pasa a «quarantine».".into())),
        Some(p) => lines.push(l("ok", "DMARC", format!("Política «{p}»: los correos que suplantan el dominio se apartan o se rechazan."))),
    }
    (spf, dm, lines)
}

fn provider_text(domain: &str, lines: &[Line]) -> String {
    let todo: Vec<String> = lines.iter().filter(|l| l.level == "bad" || l.level == "warn").map(|l| format!("- {}: {}", l.what, l.text)).collect();
    if todo.is_empty() {
        return format!("El dominio {domain} tiene MX, SPF, DKIM y DMARC correctos.");
    }
    let mut t = format!("Hola. Revisando el correo del dominio {domain} hemos visto lo siguiente:\n\n{}\n", todo.join("\n"));
    if lines.iter().any(|l| l.what == "DMARC" && l.level != "ok") {
        t.push_str(&format!("\nPara DMARC, un registro TXT en _dmarc.{domain} con: v=DMARC1; p=none; rua=mailto:dmarc@{domain}\n"));
    }
    t.push_str("\n¿Podéis revisarlo? Gracias.");
    t
}

fn tcp(host: &str, port: u16) -> (bool, u64) {
    let t = Instant::now();
    let ok = (host, port).to_socket_addrs().ok().and_then(|mut a| a.next()).map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(4)).is_ok()).unwrap_or(false);
    (ok, t.elapsed().as_millis() as u64)
}

/// Revisión completa del correo de un dominio. `server`: el servidor de envío y
/// recepción de la oficina (si se sabe); si no, se prueba el MX.
#[tauri::command]
pub async fn mail_domain_check(domain: String, server: Option<String>) -> Result<MailReport, String> {
    let domain = domain.trim().trim_start_matches('@').to_ascii_lowercase();
    let domain = domain.rsplit('@').next().unwrap_or(&domain).to_string();
    if !valid_domain(&domain) {
        return Err("Escribe un dominio como empresa.com.".into());
    }
    let server = server.map(|s| s.trim().to_string()).filter(|s| valid_domain(s));
    let ip = crate::network::lan::public_ip().await.map(|p| p.ip).unwrap_or_default();
    let rev = reverse_ip(&ip).unwrap_or_default();
    let d2 = domain.clone();
    let raw: DnsRaw = tokio::task::spawn_blocking(move || {
        let script = format!(
            "{}{}{}{}{SCRIPT}",
            crate::ps::text_var("domain", &d2),
            crate::ps::text_var("rev", &rev),
            format_args!("$selectors = @({})\n", SELECTORS.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(",")),
            format_args!("$lists = @({})\n", LISTS.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(","))
        );
        crate::pspool::query(&script, Some(Duration::from_secs(60)), "Correo del dominio").and_then(|o| super::parse::<DnsRaw>(&o))
    })
    .await
    .map_err(|e| e.to_string())??;
    let (spf, dmarc, mut lines) = verdict(&domain, &raw.mx, &raw.txt, &raw.dmarc, &raw.dkim);

    // Puertos: el servidor de la oficina (o el MX) para enviar y recibir.
    let target = server.clone().or_else(|| raw.mx.first().cloned()).unwrap_or_default();
    let mut ports = Vec::new();
    if !target.is_empty() {
        let t2 = target.clone();
        let checks: Vec<(u16, &str)> = vec![(25, "SMTP entre servidores"), (587, "Envío (SMTP con autenticación)"), (465, "Envío cifrado (SMTPS)"), (993, "Recepción IMAP cifrada"), (995, "Recepción POP3 cifrada")];
        ports = tokio::task::spawn_blocking(move || checks.into_iter().map(|(port, label)| {
            let (ok, ms) = tcp(&t2, port);
            PortCheck { host: t2.clone(), port, label: label.into(), ok, ms }
        }).collect::<Vec<_>>()).await.map_err(|e| e.to_string())?;
        let submit_ok = ports.iter().any(|p| (p.port == 587 || p.port == 465) && p.ok);
        if server.is_some() && !submit_ok {
            lines.push(Line { level: "bad", what: "Envío".into(), text: format!("Desde este equipo no se llega a {target} por los puertos de envío (587/465): un cortafuegos o el antivirus lo bloquea, o el nombre del servidor no es ese.") });
        }
        if ports.iter().any(|p| p.port == 25 && !p.ok) && server.is_none() {
            lines.push(Line { level: "info", what: "Puerto 25".into(), text: "Muchos proveedores de Internet bloquean el puerto 25 desde casa y oficinas: es normal. Para enviar se usan el 587 o el 465.".into() });
        }
    }
    let blacklists: Vec<(String, String)> = raw.lists.iter().map(|l| (l.name.clone(), list_verdict(&l.answer).to_string())).collect();
    let listed: Vec<&str> = blacklists.iter().filter(|(_, v)| v == "en la lista").map(|(n, _)| n.as_str()).collect();
    if !listed.is_empty() {
        lines.push(Line { level: "bad", what: "Listas negras".into(), text: format!("La IP pública de la oficina ({ip}) está en: {}. Los correos enviados desde aquí (sin pasar por el servidor del proveedor) se rechazan. Suele ser un equipo infectado que envía spam o una IP heredada; se pide la baja en la web de cada lista.", listed.join(", ")) });
    } else if !ip.is_empty() && !blacklists.is_empty() {
        lines.push(Line { level: "ok", what: "Listas negras".into(), text: format!("La IP pública de la oficina ({ip}) no está en las listas negras principales.") });
    }
    let provider_text = provider_text(&domain, &lines);
    Ok(MailReport { domain, mx: raw.mx, spf, dmarc, dkim: raw.dkim, ports, public_ip: ip, blacklists, lines, provider_text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spf_y_dmarc() {
        assert_eq!(spf_all("v=spf1 include:spf.protection.outlook.com -all"), Some("-all"));
        assert_eq!(spf_all("v=spf1 include:_spf.google.com ~all"), Some("~all"));
        assert_eq!(spf_all("v=spf1 a mx"), None);
        assert_eq!(spf_includes("v=spf1 include:a.com include:b.net ~all"), vec!["a.com", "b.net"]);
        assert_eq!(dmarc_policy("v=DMARC1; p=quarantine; rua=mailto:x@y.com").as_deref(), Some("quarantine"));
        assert_eq!(dmarc_policy("v=DMARC1"), None);
    }

    #[test]
    fn listas_negras() {
        assert_eq!(reverse_ip("203.0.113.7").as_deref(), Some("7.113.0.203"));
        assert_eq!(reverse_ip("hola"), None);
        assert_eq!(list_verdict(""), "limpia");
        assert_eq!(list_verdict("127.0.0.2"), "en la lista");
        assert_eq!(list_verdict("127.255.255.254"), "no se pudo consultar");
    }

    #[test]
    fn conclusiones() {
        let (_, _, l) = verdict("empresa.com", &["mail.empresa.com".into()], &["v=spf1 include:spf.protection.outlook.com".into()], &[], &[]);
        assert!(l.iter().any(|x| x.what == "SPF" && x.level == "warn"));
        assert!(l.iter().any(|x| x.what == "DMARC" && x.level == "warn"));
        let (_, _, l) = verdict("empresa.com", &[], &["v=spf1 -all".into(), "v=spf1 ~all".into()], &[], &[]);
        assert!(l.iter().any(|x| x.what == "MX" && x.level == "bad"));
        assert!(l.iter().any(|x| x.what == "SPF" && x.level == "bad"));
        let t = provider_text("empresa.com", &l);
        assert!(t.contains("_dmarc.empresa.com"));
    }

    #[test]
    fn dominios_validos() {
        assert!(valid_domain("empresa.com"));
        assert!(valid_domain("mi-empresa.co.uk"));
        assert!(!valid_domain("empresa"));
        assert!(!valid_domain("a.com; calc"));
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
