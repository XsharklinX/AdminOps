//! Pantallazos azules: del driver culpable a la solución. Se agrupan todos los
//! volcados por driver, se enseña la versión y la fecha de ese driver, si los
//! fallos empezaron cuando se instaló, y si hay una versión anterior en el
//! almacén de drivers a la que volver. Para los rebeldes, Driver Verifier
//! guiado: solo sobre los drivers sospechosos y con su desactivación a mano.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoreDriver {
    pub published: String,
    pub original: String,
    pub provider: String,
    pub class: String,
    pub date: String,
    pub version: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CrashGroup {
    /// Driver señalado («nvlddmkm.sys»), o «» si los volcados no señalan a nadie.
    pub driver: String,
    pub hint: String,
    pub count: usize,
    pub codes: Vec<String>,
    pub first: u64,
    pub last: u64,
    /// Lo que se sabe del driver instalado.
    pub device: String,
    pub version: String,
    pub driver_date: String,
    /// Cuándo se escribió el archivo del driver (≈ cuándo se instaló esta versión).
    pub installed: u64,
    /// Todos los fallos son posteriores a la instalación y antes no había ninguno.
    pub since_install: bool,
    /// Paquete del driver en uso (oemNN.inf) y versiones anteriores en el almacén.
    pub inf: String,
    pub older: Vec<StoreDriver>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub groups: Vec<CrashGroup>,
    /// Pantallazos sin volcado que leer (solo el evento).
    pub without_dump: usize,
    pub verifier: Vec<String>,
}

/// Bloques de `pnputil /enum-drivers`: el orden de los campos es el mismo en todos los idiomas.
pub fn parse_pnputil(text: &str) -> Vec<StoreDriver> {
    let mut out = Vec::new();
    for block in text.replace('\r', "").split("\n\n") {
        let values: Vec<&str> = block.lines().filter_map(|l| l.split_once(':').map(|(_, v)| v.trim())).collect();
        if values.len() < 5 || !values[0].to_ascii_lowercase().starts_with("oem") {
            continue;
        }
        // «Versión del controlador: 10/03/2024 31.0.15.5222»
        let dv = values.iter().find(|v| v.split_whitespace().count() == 2 && v.contains('/') && v.contains('.')).copied().unwrap_or("");
        let (date, version) = dv.split_once(' ').unwrap_or(("", dv));
        out.push(StoreDriver { published: values[0].into(), original: values[1].into(), provider: values[2].into(), class: values[3].into(), date: date.into(), version: version.into() });
    }
    out
}

/// Compara dos versiones «a.b.c.d» por partes.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let p = |s: &str| s.split('.').map(|x| x.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    p(a).cmp(&p(b))
}

/// Versiones anteriores del mismo paquete que hay en el almacén.
pub fn older_versions(store: &[StoreDriver], current_inf: &str) -> Vec<StoreDriver> {
    let Some(cur) = store.iter().find(|d| d.published.eq_ignore_ascii_case(current_inf)) else { return vec![] };
    let mut v: Vec<StoreDriver> = store.iter().filter(|d| d.original.eq_ignore_ascii_case(&cur.original) && !d.published.eq_ignore_ascii_case(current_inf) && version_cmp(&d.version, &cur.version).is_lt()).cloned().collect();
    v.sort_by(|a, b| version_cmp(&b.version, &a.version));
    v
}

/// ¿Empezaron los fallos con esta versión del driver?
pub fn started_after(crashes: &[u64], installed: u64) -> bool {
    installed > 0 && crashes.len() >= 2 && crashes.iter().all(|t| *t >= installed)
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
struct DriverInfo {
    name: String,
    device: String,
    version: String,
    driver_date: String,
    installed: u64,
    inf: String,
}

const INFO_SCRIPT: &str = r#"
$out = @(foreach ($n in $names) {
  $f = Join-Path $env:SystemRoot "System32\drivers\$n"
  $item = Get-Item -LiteralPath $f -ErrorAction SilentlyContinue
  $svc = Get-CimInstance Win32_SystemDriver -ErrorAction SilentlyContinue | Where-Object { "$($_.PathName)" -like "*\$n" } | Select-Object -First 1
  $dev = $null; $pnp = $null
  if ($svc) { $dev = Get-PnpDevice -ErrorAction SilentlyContinue | Where-Object { $_.Service -eq $svc.Name } | Select-Object -First 1 }
  if ($dev) { $pnp = Get-CimInstance Win32_PnPSignedDriver -ErrorAction SilentlyContinue | Where-Object { $_.DeviceID -eq $dev.InstanceId } | Select-Object -First 1 }
  [pscustomobject]@{
    name = $n; device = if ($dev) { "$($dev.FriendlyName)" } elseif ($svc) { "$($svc.DisplayName)" } else { '' }
    version = if ($pnp) { "$($pnp.DriverVersion)" } elseif ($item) { "$($item.VersionInfo.FileVersion)" } else { '' }
    driverDate = if ($pnp -and $pnp.DriverDate) { $pnp.DriverDate.ToString('yyyy-MM-dd') } else { '' }
    installed = if ($item) { [int64](($item.LastWriteTimeUtc - [datetime]'1970-01-01').TotalSeconds) } else { 0 }
    inf = if ($pnp) { "$($pnp.InfName)" } else { '' }
  }
})
ConvertTo-Json -InputObject @($out) -Compress
"#;

fn verifier_drivers() -> Vec<String> {
    crate::tweaks::registry::read_string(r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management", "VerifyDrivers").map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default()
}

#[tauri::command(async)]
pub fn crash_report() -> Result<CrashReport, String> {
    let log = crate::bootlog::boot_history()?;
    let mut groups: Vec<CrashGroup> = Vec::new();
    let mut times: Vec<(String, u64)> = Vec::new();
    for d in &log.dumps {
        let Some(a) = &d.analysis else { continue };
        let driver = a.culprit.clone().unwrap_or_default();
        let code = a.bugcheck.clone().unwrap_or_default();
        times.push((driver.clone(), d.t));
        match groups.iter_mut().find(|g| g.driver == driver) {
            Some(g) => {
                g.count += 1;
                g.first = g.first.min(d.t);
                g.last = g.last.max(d.t);
                if !code.is_empty() && !g.codes.contains(&code) {
                    g.codes.push(code);
                }
            }
            None => groups.push(CrashGroup { hint: a.culprit_hint.clone().unwrap_or_default(), count: 1, codes: if code.is_empty() { vec![] } else { vec![code] }, first: d.t, last: d.t, driver, ..Default::default() }),
        }
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.count));
    let names: Vec<String> = groups.iter().filter(|g| !g.driver.is_empty()).take(4).map(|g| g.driver.clone()).collect();
    if !names.is_empty() {
        let list = names.iter().filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))).map(|n| format!("'{n}'")).collect::<Vec<_>>().join(",");
        let script = format!("$names = @({list})\n{INFO_SCRIPT}");
        if let Ok(out) = crate::pspool::query(&script, Some(Duration::from_secs(60)), "Pantallazos: drivers") {
            let infos: Vec<DriverInfo> = serde_json::from_str(out.trim()).unwrap_or_default();
            let store = crate::ps::exec("pnputil.exe", &["/enum-drivers"]).map(|t| parse_pnputil(&t)).unwrap_or_default();
            for g in groups.iter_mut() {
                if let Some(i) = infos.iter().find(|i| i.name.eq_ignore_ascii_case(&g.driver)) {
                    g.device = i.device.clone();
                    g.version = i.version.clone();
                    g.driver_date = i.driver_date.clone();
                    g.installed = i.installed;
                    g.inf = i.inf.clone();
                    let t: Vec<u64> = times.iter().filter(|(d, _)| d == &g.driver).map(|(_, t)| *t).collect();
                    g.since_install = started_after(&t, i.installed);
                    if !i.inf.is_empty() {
                        g.older = older_versions(&store, &i.inf);
                    }
                }
            }
        }
    }
    let bsods = log.events.iter().filter(|e| e.kind == "bsod").count();
    Ok(CrashReport { without_dump: bsods.saturating_sub(log.dumps.len()), groups, verifier: verifier_drivers() })
}

fn valid_inf(inf: &str) -> bool {
    let l = inf.to_ascii_lowercase();
    l.starts_with("oem") && l.ends_with(".inf") && l[3..l.len() - 4].chars().all(|c| c.is_ascii_digit())
}

fn valid_sys(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".sys") && l.len() < 64 && l.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// Vuelve al driver anterior: quita el paquete en uso y Windows instala la
/// versión anterior que queda en el almacén.
#[tauri::command(async)]
pub fn driver_rollback(inf: String, state: State<'_, TweakState>) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if !valid_inf(&inf) {
        return Err("Paquete de driver no válido.".into());
    }
    let r = crate::ps::exec("pnputil.exe", &["/delete-driver", &inf, "/uninstall", "/force"]).and_then(|_| crate::ps::exec("pnputil.exe", &["/scan-devices"])).map(|_| ());
    state.record(Op::Run, &format!("Volver al driver anterior (quitar {inf})"), &r);
    r.map(|()| "Driver nuevo quitado: Windows ha vuelto a la versión anterior. Reinicia y comprueba si siguen los pantallazos.".into())
}

/// Activa Driver Verifier solo para esos drivers (hace falta reiniciar).
#[tauri::command(async)]
pub fn verifier_enable(drivers: Vec<String>, state: State<'_, TweakState>) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if drivers.is_empty() || drivers.len() > 6 || !drivers.iter().all(|d| valid_sys(d)) {
        return Err("Elige entre uno y seis drivers (.sys).".into());
    }
    let mut args = vec!["/standard", "/driver"];
    args.extend(drivers.iter().map(String::as_str));
    let r = crate::ps::exec("verifier.exe", &args).map(|_| ());
    state.record(Op::Run, &format!("Driver Verifier: vigilar {}", drivers.join(", ")), &r);
    r.map(|()| "Driver Verifier activado. Reinicia y usa el equipo con normalidad: si el driver falla, el pantallazo lo señalará con nombre. Desactívalo aquí en cuanto lo tengas (o si el equipo no arranca: modo seguro → «verifier /reset»).".into())
}

#[tauri::command(async)]
pub fn verifier_disable(state: State<'_, TweakState>) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let r = crate::ps::exec("verifier.exe", &["/reset"]).map(|_| ());
    state.record(Op::Run, "Driver Verifier: desactivar", &r);
    r.map(|()| "Driver Verifier desactivado. Reinicia para que deje de vigilar.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNP: &str = "Utilidad de PnP de Microsoft\r\n\r\nNombre publicado:     oem12.inf\r\nNombre original:      nv_dispi.inf\r\nNombre del proveedor: NVIDIA\r\nNombre de clase:      Adaptadores de pantalla\r\nGUID de clase:        {4d36e968-e325-11ce-bfc1-08002be10318}\r\nVersión del controlador: 10/03/2024 31.0.15.6603\r\nNombre del firmante:  Microsoft Windows Hardware Compatibility Publisher\r\n\r\nNombre publicado:     oem8.inf\r\nNombre original:      nv_dispi.inf\r\nNombre del proveedor: NVIDIA\r\nNombre de clase:      Adaptadores de pantalla\r\nGUID de clase:        {4d36e968-e325-11ce-bfc1-08002be10318}\r\nVersión del controlador: 05/14/2024 31.0.15.5222\r\nNombre del firmante:  Microsoft Windows Hardware Compatibility Publisher\r\n";

    #[test]
    fn lee_el_almacen_de_drivers() {
        let s = parse_pnputil(PNP);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].published, "oem12.inf");
        assert_eq!(s[0].original, "nv_dispi.inf");
        assert_eq!(s[0].version, "31.0.15.6603");
        let older = older_versions(&s, "oem12.inf");
        assert_eq!(older.len(), 1);
        assert_eq!(older[0].published, "oem8.inf");
        assert!(older_versions(&s, "oem8.inf").is_empty());
    }

    #[test]
    fn desde_la_instalacion() {
        assert!(started_after(&[200, 300, 400], 100));
        assert!(!started_after(&[50, 300], 100));
        assert!(!started_after(&[300], 100));
    }

    #[test]
    fn valida_lo_que_pasa_a_pnputil_y_verifier() {
        assert!(valid_inf("oem12.inf"));
        assert!(!valid_inf("nv_dispi.inf"));
        assert!(!valid_inf("oem1.inf & calc"));
        assert!(valid_sys("nvlddmkm.sys"));
        assert!(!valid_sys("a.sys; x"));
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(INFO_SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
