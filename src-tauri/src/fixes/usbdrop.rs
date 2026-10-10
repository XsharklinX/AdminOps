//! USB, teclados y ratones que se desconectan. Windows apunta cada vez que un
//! dispositivo arranca (evento 410 de Kernel-PnP): un ratón que «arranca» catorce
//! veces en un día es un ratón que se ha desconectado catorce veces. Se busca el
//! patrón: el ahorro de energía del USB, un puerto concreto o el mismo
//! dispositivo en cualquier puerto. Y se ajusta el ahorro de energía con un clic.

use super::parse;
use crate::troubleshoot::{finding, fix, Finding};
use crate::tweaks::TweakState;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;

/// Más arranques que esto en dos días ya no son enchufar y desenchufar a mano.
const MANY: usize = 6;
const SUB_USB: &str = "2a737441-1930-4402-8d77-b2bebba308a3";
const SET_SUSPEND: &str = "48e6b7a6-50f5-4782-a5d4-53bb8f07e226";

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Start {
    pub at: i64,
    pub instance: String,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct UsbRaw {
    pub starts: Vec<Start>,
    /// InstanceId → nombre amable.
    pub names: BTreeMap<String, String>,
    pub suspend_ac: Option<u32>,
    pub suspend_dc: Option<u32>,
}

const SCRIPT: &str = r#"
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-Kernel-PnP/Configuration'; Id = 410; StartTime = (Get-Date).AddDays(-2) } -MaxEvents 3000 -ErrorAction SilentlyContinue)
$starts = @($ev | ForEach-Object { $id = "$($_.Properties[0].Value)"; if ($id -match '^(USB|HID)\\') { [pscustomobject]@{ at = [int64](($_.TimeCreated.ToUniversalTime() - [datetime]'1970-01-01').TotalSeconds); instance = $id } } })
$names = @{}
foreach ($i in ($starts | Select-Object -ExpandProperty instance -Unique)) { $d = Get-PnpDevice -InstanceId $i -ErrorAction SilentlyContinue; if ($d) { $names[$i] = "$($d.FriendlyName)" } }
$q = "$(powercfg.exe /q SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226)"
$hex = @([regex]::Matches($q, '0x0000000([01])') | ForEach-Object { [int]$_.Groups[1].Value })
[pscustomobject]@{ starts = $starts; names = $names; suspendAc = if ($hex.Count -ge 2) { $hex[$hex.Count - 2] } else { $null }; suspendDc = if ($hex.Count -ge 1) { $hex[$hex.Count - 1] } else { $null } } | ConvertTo-Json -Depth 4 -Compress
"#;

/// «USB\VID_046D&PID_C52B\5&2a1f…» → «USB\VID_046D&PID_C52B» (el dispositivo, sin el puerto).
pub fn device_of(instance: &str) -> String {
    let mut parts = instance.splitn(3, '\\');
    let bus = parts.next().unwrap_or("");
    let id = parts.next().unwrap_or("");
    let id = id.split("&MI_").next().unwrap_or(id).split("&COL").next().unwrap_or(id);
    format!("{bus}\\{id}").to_ascii_uppercase()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drops {
    pub device: String,
    pub name: String,
    pub starts: usize,
    pub ports: usize,
    pub last: i64,
}

/// Dispositivos que arrancan una y otra vez, el que más primero.
pub fn analyze(raw: &UsbRaw) -> Vec<Drops> {
    let mut by: BTreeMap<String, (Vec<i64>, std::collections::BTreeSet<String>, String)> = BTreeMap::new();
    for s in &raw.starts {
        let dev = device_of(&s.instance);
        let e = by.entry(dev).or_default();
        e.0.push(s.at);
        e.1.insert(s.instance.to_ascii_uppercase());
        if e.2.is_empty() {
            if let Some(n) = raw.names.get(&s.instance).filter(|n| !n.is_empty()) {
                e.2 = n.clone();
            }
        }
    }
    let mut out: Vec<Drops> = by
        .into_iter()
        .filter(|(_, (t, _, _))| t.len() >= MANY)
        .map(|(device, (t, ports, name))| Drops { name: if name.is_empty() { device.clone() } else { name }, starts: t.len(), ports: ports.len(), last: t.into_iter().max().unwrap_or(0), device })
        .collect();
    out.sort_by_key(|d| std::cmp::Reverse(d.starts));
    out
}

pub fn findings(raw: &UsbRaw) -> Vec<Finding> {
    let mut out = Vec::new();
    let drops = analyze(raw);
    let suspend_on = raw.suspend_ac == Some(1) || raw.suspend_dc == Some(1);
    for d in drops.iter().take(5) {
        let why = if d.ports > 1 {
            "Pasa en varios puertos: el problema va con el dispositivo (o su cable o receptor), no con el equipo."
        } else if suspend_on {
            "Siempre en el mismo puerto y con el ahorro de energía del USB activo: lo más probable es que Windows lo apague para ahorrar. Prueba primero a quitar el ahorro; si sigue, otro puerto (mejor uno trasero, directo a la placa)."
        } else {
            "Siempre en el mismo puerto: prueba otro (mejor uno trasero, directo a la placa) y evita concentradores sin alimentación."
        };
        out.push(finding("warn", format!("«{}» se ha reconectado {} veces en dos días", d.name, d.starts), why.to_string()).fixes(if suspend_on { vec![fix("usb.suspend.off", "Quitar el ahorro de energía del USB", true)] } else { vec![] }));
    }
    if suspend_on && drops.is_empty() {
        out.push(finding("info", "El ahorro de energía del USB está activo", "Windows apaga los puertos USB que no se usan. Si un ratón o un lector se desconecta tras un rato sin tocarlo, quítalo.").fixes(vec![fix("usb.suspend.off", "Quitar el ahorro de energía del USB", true)]));
    } else if !suspend_on && raw.suspend_ac.is_some() {
        out.push(finding("info", "El ahorro de energía del USB está desactivado", "Windows no apaga los puertos USB.").fixes(vec![fix("usb.suspend.on", "Volver a activarlo", true)]));
    }
    if !out.iter().any(|f| f.level == "warn" || f.level == "bad") {
        out.insert(0, finding("ok", "Ningún dispositivo USB se desconecta a menudo", format!("{} arranques de dispositivos en los últimos dos días, todos normales.", raw.starts.len())));
    }
    out
}

pub fn check() -> Result<Vec<Finding>, String> {
    let out = crate::pspool::query(SCRIPT, Some(Duration::from_secs(60)), "Solucionar: USB")?;
    Ok(findings(&parse::<UsbRaw>(&out)?))
}

fn set_suspend(on: bool) -> Result<String, String> {
    let v = if on { "1" } else { "0" };
    crate::ps::exec("powercfg.exe", &["/setacvalueindex", "SCHEME_CURRENT", SUB_USB, SET_SUSPEND, v])?;
    crate::ps::exec("powercfg.exe", &["/setdcvalueindex", "SCHEME_CURRENT", SUB_USB, SET_SUSPEND, v])?;
    crate::ps::exec("powercfg.exe", &["/setactive", "SCHEME_CURRENT"])?;
    Ok(if on { "Ahorro de energía del USB activado de nuevo." } else { "Windows ya no apagará los puertos USB para ahorrar energía." }.into())
}

pub fn run(_tweaks: &TweakState, kind: &str, _arg: &str) -> Option<Result<String, String>> {
    Some(match kind {
        "usb.suspend.off" => set_suspend(false),
        "usb.suspend.on" => set_suspend(true),
        _ => return None,
    })
}

pub fn title(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "usb.suspend.off" => "Quitar el ahorro de energía del USB",
        "usb.suspend.on" => "Activar el ahorro de energía del USB",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispositivo_sin_puerto() {
        assert_eq!(device_of(r"USB\VID_046D&PID_C52B\5&2a1f0c3&0&2"), r"USB\VID_046D&PID_C52B");
        assert_eq!(device_of(r"HID\VID_046D&PID_C52B&MI_01&COL02\7&1"), r"HID\VID_046D&PID_C52B");
    }

    fn starts(instance: &str, n: usize) -> Vec<Start> {
        (0..n).map(|i| Start { at: 1000 + i as i64, instance: instance.into() }).collect()
    }

    #[test]
    fn mismo_puerto_con_ahorro() {
        let mut raw = UsbRaw { starts: starts(r"USB\VID_046D&PID_C52B\5&1&0&2", 14), suspend_ac: Some(1), suspend_dc: Some(1), ..Default::default() };
        raw.names.insert(r"USB\VID_046D&PID_C52B\5&1&0&2".into(), "Receptor Logitech".into());
        raw.starts.extend(starts(r"USB\VID_1234&PID_0001\5&9", 2));
        let d = analyze(&raw);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].name, "Receptor Logitech");
        let f = findings(&raw);
        assert!(f[0].title.contains("14 veces"));
        assert!(f[0].detail.contains("ahorro"));
        assert_eq!(f[0].fixes[0].id, "usb.suspend.off");
    }

    #[test]
    fn varios_puertos_es_el_dispositivo() {
        let mut s = starts(r"USB\VID_AAAA&PID_0001\5&1", 4);
        s.extend(starts(r"USB\VID_AAAA&PID_0001\5&2", 4));
        let raw = UsbRaw { starts: s, suspend_ac: Some(0), suspend_dc: Some(0), ..Default::default() };
        let f = findings(&raw);
        assert!(f[0].detail.contains("varios puertos"));
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
