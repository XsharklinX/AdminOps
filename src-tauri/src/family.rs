//! Control parental con lo que trae Windows:
//! - Filtro de navegación por DNS (Cloudflare for Families, CleanBrowsing):
//!   bloquea malware y contenido adulto en todo el equipo y todos los navegadores.
//! - Sitios bloqueados concretos en el archivo hosts (sección propia de AdminOps).
//! - Horario de inicio de sesión por usuario local (logon hours de Windows).

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

// ---------- Filtro por DNS ----------

/// (id, nombre, servidores)
const FILTERS: &[(&str, &str, [&str; 2])] = &[
    ("malware", "Malware y phishing", ["1.1.1.2", "1.0.0.2"]),
    ("family", "Malware y contenido adulto", ["1.1.1.3", "1.0.0.3"]),
    ("strict", "Estricto: adultos, proxies y búsqueda segura", ["185.228.168.168", "185.228.169.168"]),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterStatus {
    /// none | malware | family | strict | custom
    pub active: String,
    pub adapters: Vec<String>,
}

fn physical_adapters() -> Result<Vec<crate::network::tools::DnsAdapter>, String> {
    Ok(crate::network::tools::dns_adapters()?.into_iter().filter(|a| !a.is_virtual).collect())
}

#[tauri::command(async)]
pub fn dns_filter_status() -> Result<FilterStatus, String> {
    let adapters = physical_adapters()?;
    let preset = |a: &crate::network::tools::DnsAdapter| -> String {
        if !a.manual {
            return "none".into();
        }
        FILTERS.iter().find(|(_, _, s)| a.dns.first().map(String::as_str) == Some(s[0])).map_or("custom".into(), |(id, _, _)| id.to_string())
    };
    let kinds: Vec<String> = adapters.iter().map(preset).collect();
    let active = match kinds.first() {
        None => "none".to_string(),
        Some(k) if kinds.iter().all(|x| x == k) => k.clone(),
        _ => "custom".into(),
    };
    Ok(FilterStatus { active, adapters: adapters.into_iter().map(|a| a.name).collect() })
}

#[tauri::command(async)]
pub fn set_dns_filter(tweaks: State<'_, TweakState>, filter: String) -> Result<(), String> {
    let servers: Vec<String> = match filter.as_str() {
        "none" => vec![],
        id => FILTERS.iter().find(|(f, _, _)| *f == id).ok_or("Filtro desconocido.")?.2.iter().map(|s| s.to_string()).collect(),
    };
    let adapters = physical_adapters()?;
    if adapters.is_empty() {
        return Err("No hay ningún adaptador de red conectado.".into());
    }
    for a in adapters {
        crate::network::tools::set_dns(a.index, servers.clone(), tweaks.clone())?;
    }
    Ok(())
}

// ---------- Sitios bloqueados (hosts) ----------

const BEGIN: &str = "# >>> AdminOps: sitios bloqueados (no editar esta sección a mano)";
const END: &str = "# <<< AdminOps";

fn valid_domain(d: &str) -> bool {
    d.len() <= 253 && d.contains('.') && !d.starts_with('.') && !d.ends_with('.') && d.split('.').all(|l| !l.is_empty() && l.len() <= 63 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

/// "https://www.Ejemplo.com/ruta" → "ejemplo.com"
pub fn clean_domain(s: &str) -> Option<String> {
    let s = s.trim().to_ascii_lowercase();
    let s = s.split("://").last().unwrap_or(&s);
    let s = s.split(['/', '?', '#', ':']).next().unwrap_or("");
    let s = s.strip_prefix("www.").unwrap_or(s).to_string();
    valid_domain(&s).then_some(s)
}

fn blocked_in(hosts: &str) -> Vec<String> {
    let Some(start) = hosts.find(BEGIN) else { return vec![] };
    let end = hosts[start..].find(END).map_or(hosts.len(), |e| start + e);
    let mut v: Vec<String> = hosts[start..end]
        .lines()
        .filter(|l| l.trim_start().starts_with("0.0.0.0"))
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter(|d| !d.starts_with("www."))
        .map(String::from)
        .collect();
    v.dedup();
    v
}

/// El archivo hosts con la sección de AdminOps sustituida (el resto intacto).
fn with_blocked(hosts: &str, domains: &[String]) -> String {
    let mut rest = hosts.to_string();
    if let Some(start) = rest.find(BEGIN) {
        let end = rest[start..].find(END).map_or(rest.len(), |e| start + e + END.len());
        rest.replace_range(start..end, "");
    }
    let mut out = rest.trim_end().to_string();
    if !domains.is_empty() {
        out.push_str("\r\n\r\n");
        out.push_str(BEGIN);
        out.push_str("\r\n");
        for d in domains {
            out.push_str(&format!("0.0.0.0 {d}\r\n0.0.0.0 www.{d}\r\n"));
        }
        out.push_str(END);
    }
    out.push_str("\r\n");
    out
}

#[tauri::command(async)]
pub fn blocked_sites() -> Result<Vec<String>, String> {
    let hosts = std::fs::read_to_string(crate::network::tools::hosts_path()).map_err(|e| format!("No se pudo leer el archivo hosts: {e}"))?;
    Ok(blocked_in(&hosts))
}

#[tauri::command(async)]
pub fn set_blocked_sites(tweaks: State<'_, TweakState>, sites: Vec<String>) -> Result<Vec<String>, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let mut domains = Vec::new();
    for s in &sites {
        let d = clean_domain(s).ok_or_else(|| format!("«{}» no es un sitio válido (ejemplo: tiktok.com).", s.trim()))?;
        if !domains.contains(&d) {
            domains.push(d);
        }
    }
    if domains.len() > 500 {
        return Err("Demasiados sitios (máximo 500).".into());
    }
    let path = crate::network::tools::hosts_path();
    let hosts = std::fs::read_to_string(&path).map_err(|e| format!("No se pudo leer el archivo hosts: {e}"))?;
    let result = std::fs::write(&path, with_blocked(&hosts, &domains)).map_err(|e| format!("No se pudo escribir el archivo hosts (¿lo bloquea el antivirus?): {e}"));
    if result.is_ok() {
        let _ = crate::ps::powershell("Clear-DnsClientCache");
    }
    tweaks.record(Op::Run, &format!("Sitios bloqueados: {}", domains.len()), &result);
    result.map(|_| domains)
}

// ---------- Horario de uso ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserHours {
    pub name: String,
    pub full_name: String,
    pub admin: bool,
    pub microsoft: bool,
    /// Es la cuenta con la que se ejecuta AdminOps (no se puede restringir).
    pub is_self: bool,
    /// 7 días (lunes primero) × 24 horas, en hora local. `true` = puede iniciar sesión.
    pub hours: Vec<Vec<bool>>,
    pub restricted: bool,
}

const USERS_SCRIPT: &str = r#"
$admins = @(Get-LocalGroupMember -SID 'S-1-5-32-544' -ErrorAction SilentlyContinue | ForEach-Object { "$($_.SID)" })
$r = @(Get-LocalUser | Where-Object { $_.Enabled -and "$($_.SID)" -notmatch '-(500|501|503|504)$' } | ForEach-Object {
  [pscustomobject]@{ name = $_.Name; fullName = "$($_.FullName)"; sid = "$($_.SID)"; admin = $admins -contains "$($_.SID)"; microsoft = "$($_.PrincipalSource)" -eq 'MicrosoftAccount' }
})
ConvertTo-Json -InputObject $r -Compress
"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUser {
    name: String,
    full_name: String,
    admin: bool,
    microsoft: bool,
}

/// Diferencia de la hora local con UTC, en horas.
fn utc_offset_hours() -> i32 {
    use chrono::Offset;
    chrono::Local::now().offset().fix().local_minus_utc() / 3600
}

/// Bits de Windows (168, domingo 00:00 UTC primero) → cuadrícula local lunes-domingo.
fn to_grid(bits: &[u8; 21], offset: i32) -> Vec<Vec<bool>> {
    (0..7)
        .map(|ui_day| {
            let sunday_based = (ui_day + 1) % 7;
            (0..24)
                .map(|h| {
                    let utc = (sunday_based * 24 + h - offset).rem_euclid(168) as usize;
                    bits[utc / 8] & (1 << (utc % 8)) != 0
                })
                .collect()
        })
        .collect()
}

fn to_bits(grid: &[Vec<bool>], offset: i32) -> [u8; 21] {
    let mut bits = [0u8; 21];
    for (ui_day, row) in grid.iter().enumerate().take(7) {
        let sunday_based = (ui_day as i32 + 1) % 7;
        for (h, allowed) in row.iter().enumerate().take(24) {
            if *allowed {
                let utc = (sunday_based * 24 + h as i32 - offset).rem_euclid(168) as usize;
                bits[utc / 8] |= 1 << (utc % 8);
            }
        }
    }
    bits
}

#[cfg(windows)]
fn get_hours(name: &str) -> Option<[u8; 21]> {
    use windows_sys::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetUserGetInfo, USER_INFO_11};
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut buf: *mut u8 = std::ptr::null_mut();
    let r = unsafe { NetUserGetInfo(std::ptr::null(), wide.as_ptr(), 11, &mut buf) };
    if r != 0 || buf.is_null() {
        return None;
    }
    let info = unsafe { &*(buf as *const USER_INFO_11) };
    let mut out = [0xFFu8; 21];
    if !info.usri11_logon_hours.is_null() && info.usri11_units_per_week == 168 {
        unsafe { std::ptr::copy_nonoverlapping(info.usri11_logon_hours, out.as_mut_ptr(), 21) };
    }
    unsafe { NetApiBufferFree(buf.cast()) };
    Some(out)
}

#[cfg(not(windows))]
fn get_hours(_: &str) -> Option<[u8; 21]> {
    None
}

#[cfg(windows)]
fn set_hours(name: &str, bits: &mut [u8; 21]) -> Result<(), String> {
    use windows_sys::Win32::NetworkManagement::NetManagement::{NetUserSetInfo, USER_INFO_1020};
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let info = USER_INFO_1020 { usri1020_units_per_week: 168, usri1020_logon_hours: bits.as_mut_ptr() };
    let mut parm = 0u32;
    let r = unsafe { NetUserSetInfo(std::ptr::null(), wide.as_ptr(), 1020, (&info as *const USER_INFO_1020).cast(), &mut parm) };
    match r {
        0 => Ok(()),
        5 => Err("Acceso denegado: ejecuta AdminOps como administrador.".into()),
        2221 => Err("No existe ese usuario.".into()),
        e => Err(format!("Windows no aceptó el horario (error {e}).")),
    }
}

#[cfg(not(windows))]
fn set_hours(_: &str, _: &mut [u8; 21]) -> Result<(), String> {
    Err("Solo disponible en Windows.".into())
}

fn current_user() -> String {
    std::env::var("USERNAME").unwrap_or_default()
}

#[tauri::command(async)]
pub fn logon_hours() -> Result<Vec<UserHours>, String> {
    let out = crate::pspool::query(USERS_SCRIPT, Some(Duration::from_secs(30)), "Horario de uso: usuarios")?;
    let users: Vec<RawUser> = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let offset = utc_offset_hours();
    let me = current_user();
    Ok(users
        .into_iter()
        .map(|u| {
            let bits = get_hours(&u.name).unwrap_or([0xFF; 21]);
            UserHours {
                restricted: bits.iter().any(|b| *b != 0xFF),
                hours: to_grid(&bits, offset),
                is_self: u.name.eq_ignore_ascii_case(&me),
                name: u.name,
                full_name: u.full_name,
                admin: u.admin,
                microsoft: u.microsoft,
            }
        })
        .collect())
}

#[tauri::command(async)]
pub fn set_logon_hours(tweaks: State<'_, TweakState>, name: String, hours: Vec<Vec<bool>>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if name.eq_ignore_ascii_case(&current_user()) {
        return Err("No se puede limitar la cuenta con la que se está usando AdminOps.".into());
    }
    if hours.len() != 7 || hours.iter().any(|d| d.len() != 24) {
        return Err("Horario no válido.".into());
    }
    let mut bits = to_bits(&hours, utc_offset_hours());
    let r = set_hours(&name, &mut bits);
    let all = bits.iter().all(|b| *b == 0xFF);
    tweaks.record(Op::Run, &format!("Horario de uso de «{name}»: {}", if all { "sin límite" } else { "limitado" }), &r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains() {
        assert_eq!(clean_domain("https://www.TikTok.com/@x?y").as_deref(), Some("tiktok.com"));
        assert_eq!(clean_domain("m.facebook.com").as_deref(), Some("m.facebook.com"));
        assert_eq!(clean_domain("roblox.com:443").as_deref(), Some("roblox.com"));
        assert!(clean_domain("no es un sitio").is_none());
        assert!(clean_domain("localhost").is_none());
    }

    #[test]
    fn hosts_section_is_replaced_and_rest_kept() {
        let original = "# hosts de Windows\r\n127.0.0.1 miservidor\r\n";
        let a = with_blocked(original, &["tiktok.com".into(), "roblox.com".into()]);
        assert!(a.contains("127.0.0.1 miservidor"));
        assert!(a.contains("0.0.0.0 www.tiktok.com"));
        assert_eq!(blocked_in(&a), vec!["tiktok.com", "roblox.com"]);
        let b = with_blocked(&a, &["roblox.com".into()]);
        assert_eq!(blocked_in(&b), vec!["roblox.com"]);
        assert!(!b.contains("tiktok"));
        let c = with_blocked(&b, &[]);
        assert!(!c.contains("AdminOps"));
        assert!(c.contains("127.0.0.1 miservidor"));
    }

    #[test]
    fn logon_hours_roundtrip_with_timezones() {
        for offset in [-4, 0, 1, 5, -11] {
            let mut grid = vec![vec![false; 24]; 7];
            // Lunes a viernes de 8 a 20; sábado de 10 a 22.
            for day in grid.iter_mut().take(5) {
                day[8..20].fill(true);
            }
            grid[5][10..22].fill(true);
            let bits = to_bits(&grid, offset);
            assert_eq!(to_grid(&bits, offset), grid, "offset {offset}");
        }
        // Sin restricción: todos los bits.
        let all = vec![vec![true; 24]; 7];
        assert_eq!(to_bits(&all, -4), [0xFF; 21]);
        // Domingo 00:00 UTC es el bit 0: con offset 0, domingo (índice 6 en la cuadrícula) a las 0.
        let mut one = vec![vec![false; 24]; 7];
        one[6][0] = true;
        assert_eq!(to_bits(&one, 0)[0], 1);
    }

    /// Equipo real: `cargo test family_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn family_real() {
        for u in logon_hours().unwrap() {
            println!("{} admin={} ms={} self={} restringido={}", u.name, u.admin, u.microsoft, u.is_self, u.restricted);
        }
        let f = dns_filter_status().unwrap();
        println!("filtro: {} en {:?}", f.active, f.adapters);
        println!("bloqueados: {:?}", blocked_sites().unwrap());
        println!("edición: {:?} bitlocker={}", crate::vault::vault_support().edition, crate::vault::vault_support().bitlocker);
        println!("winfr: {:?}", crate::recover::recover_status().map(|s| (s.installed, s.drives.len())));
    }

    #[test]
    fn users_script_parses() {
        assert!(crate::ps::parse_errors(USERS_SCRIPT).is_empty());
    }
}
