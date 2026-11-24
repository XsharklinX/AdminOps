//! Windows Update: la causa de un fallo, comprobada en este equipo.
//!
//! Un código como 0x800f0922 dice poco; aquí se mira el equipo (espacio de la partición
//! reservada y de C:, servicios, reinicio pendiente, componentes, reloj) y, según el código y
//! lo medido, se elige **una** causa con su frase y el arreglo que toca. `pick` es lógica
//! pura (se prueba con datos); `gather` es lo que lee del equipo, y solo lo que el código pide.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Lo que hace falta libre en la partición reservada del sistema (EFI) para que Windows
/// actualice sus archivos de arranque. Umbral práctico: por debajo de esto falla con 0x800f0922.
pub const ESP_MIN_MB: u64 = 15;
/// Espacio libre en el disco del sistema por debajo del cual un fallo de espacio es la causa.
const DISK_LOW_GB: f64 = 10.0;
/// Por debajo de esto, sea cual sea el código, el espacio ya es un problema.
const DISK_VERY_LOW_GB: f64 = 5.0;
/// Desfase del reloj a partir del cual los certificados de Microsoft dejan de valer.
const CLOCK_SKEW_MIN: i64 = 10;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceFact {
    pub name: String,
    /// Auto | Manual | Disabled
    pub mode: String,
    /// Running | Stopped…
    pub state: String,
}

/// Lo que se ha podido medir. `None` = no se pudo comprobar (no se afirma nada).
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Facts {
    /// Bytes libres de la partición del sistema EFI.
    pub esp_free: Option<u64>,
    /// Bytes libres del disco del sistema.
    pub c_free: Option<u64>,
    pub services: Vec<ServiceFact>,
    pub reboot_pending: Option<bool>,
    pub components_damaged: Option<bool>,
    /// Segundos que el reloj del equipo va por delante (+) o por detrás (−) del de Microsoft.
    pub clock_skew_sec: Option<i64>,
}

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Cause {
    /// esp | disk | service | reboot | components | clock
    pub kind: &'static str,
    /// Una frase con el dato medido.
    pub sentence: String,
    /// El arreglo que toca primero (id del catálogo). Sin arreglo automático: `None`.
    pub fix: Option<&'static str>,
}

fn norm(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

const ESP_CODES: &[&str] = &["0X800F0922"];
const DISK_CODES: &[&str] = &["0X80070070"];
const COMPONENT_CODES: &[&str] = &["0X80073712", "0X800F081F", "0X800F0831", "0X800F0988", "0X8007000D"];
const CLOCK_CODES: &[&str] = &["0X80072F8F"];

fn is_one_of(code: &str, list: &[&str]) -> bool {
    let c = norm(code);
    list.iter().any(|k| c.starts_with(k))
}

/// Lo que merece la pena medir para este código (DISM y el reloj tardan: solo si hacen falta).
pub fn wants_components(code: &str) -> bool {
    is_one_of(code, COMPONENT_CODES)
}

pub fn wants_clock(code: &str) -> bool {
    is_one_of(code, CLOCK_CODES)
}

fn mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

fn gb(bytes: u64) -> String {
    crate::measure::fmt_num(bytes as f64 / 1_073_741_824.0)
}

fn service_name(name: &str) -> &'static str {
    match name.to_ascii_lowercase().as_str() {
        "wuauserv" => "Windows Update",
        "bits" => "Transferencia inteligente en segundo plano (BITS)",
        "cryptsvc" => "Servicios criptográficos",
        _ => "de Windows Update",
    }
}

fn service_problem(f: &Facts) -> Option<Cause> {
    for s in &f.services {
        let known = matches!(s.name.to_ascii_lowercase().as_str(), "wuauserv" | "bits" | "cryptsvc");
        if !known {
            continue;
        }
        if s.mode.eq_ignore_ascii_case("Disabled") {
            return Some(Cause { kind: "service", sentence: format!("El servicio «{}» está deshabilitado: Windows Update no puede trabajar sin él.", service_name(&s.name)), fix: Some("repair.wu-services") });
        }
        // Los dos primeros arrancan a demanda (parados es lo normal); el de criptografía tiene que estar siempre en marcha.
        if s.name.eq_ignore_ascii_case("cryptsvc") && s.mode.eq_ignore_ascii_case("Auto") && s.state.eq_ignore_ascii_case("Stopped") {
            return Some(Cause { kind: "service", sentence: format!("El servicio «{}» está parado.", service_name(&s.name)), fix: Some("repair.wu-services") });
        }
    }
    None
}

fn esp_cause(f: &Facts) -> Option<Cause> {
    let free = mb(f.esp_free?);
    (free < ESP_MIN_MB).then(|| Cause {
        kind: "esp",
        sentence: format!("No cabe en la partición reservada del sistema: quedan {free} MB y Windows necesita al menos {ESP_MIN_MB} MB."),
        fix: Some("repair.esp-fonts"),
    })
}

fn disk_cause(f: &Facts, limit: f64) -> Option<Cause> {
    let free = f.c_free?;
    ((free as f64 / 1_073_741_824.0) < limit).then(|| Cause {
        kind: "disk",
        sentence: format!("Quedan {} GB libres en el disco del sistema: Windows necesita varios GB para descargar e instalar.", gb(free)),
        fix: Some("cleanup.windows-update-cache"),
    })
}

fn components_cause(f: &Facts) -> Option<Cause> {
    (f.components_damaged == Some(true)).then(|| Cause {
        kind: "components",
        sentence: "Windows encuentra dañado su almacén de componentes (la comprobación rápida de DISM lo confirma).".into(),
        fix: Some("repair.dism"),
    })
}

fn clock_cause(f: &Facts) -> Option<Cause> {
    let skew = f.clock_skew_sec?;
    let min = skew.abs() / 60;
    (min >= CLOCK_SKEW_MIN).then(|| Cause {
        kind: "clock",
        sentence: format!("El reloj del equipo va {min} min {} de la hora real: los servidores de Microsoft rechazan la conexión.", if skew > 0 { "adelantado respecto a" } else { "atrasado respecto a" }),
        fix: Some("repair.time-sync"),
    })
}

fn reboot_cause(f: &Facts) -> Option<Cause> {
    (f.reboot_pending == Some(true)).then(|| Cause { kind: "reboot", sentence: "Hay un reinicio pendiente: Windows no instala nada más hasta que se reinicie.".into(), fix: None })
}

/// Elige la causa más probable para este código con lo medido. `None`: no se puede
/// afirmar ninguna (la interfaz enseña la frase del código y «Ver en Soluciones»).
pub fn pick(code: &str, f: &Facts) -> Option<Cause> {
    // Primero lo que el propio código señala…
    let specific = if is_one_of(code, ESP_CODES) {
        esp_cause(f)
    } else if is_one_of(code, DISK_CODES) {
        disk_cause(f, DISK_LOW_GB)
    } else if is_one_of(code, COMPONENT_CODES) {
        components_cause(f)
    } else if is_one_of(code, CLOCK_CODES) {
        clock_cause(f)
    } else {
        None
    };
    // …y si no, lo que esté mal en el equipo y estropee cualquier actualización.
    specific.or_else(|| service_problem(f)).or_else(|| reboot_cause(f)).or_else(|| disk_cause(f, DISK_VERY_LOW_GB)).or_else(|| components_cause(f))
}

const FACTS_SCRIPT: &str = r#"
$o = [ordered]@{}
try {
  # La partición EFI no tiene letra: se pregunta por su ruta de volumen
  if (-not ('AdminOpsWu.WuDisk' -as [type])) {
    Add-Type -MemberDefinition '[DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern bool GetDiskFreeSpaceEx(string d, out ulong a, out ulong t, out ulong f);' -Name WuDisk -Namespace AdminOpsWu -ErrorAction Stop
  }
  $esp = Get-Partition -ErrorAction Stop | Where-Object { $_.GptType -eq '{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}' } | Select-Object -First 1
  foreach ($path in @($esp.AccessPaths | Where-Object { $_ -like '\\?\Volume{*' })) {
    $p = if ($path.EndsWith('\')) { $path } else { $path + '\' }
    $a = [uint64]0; $t = [uint64]0; $f = [uint64]0
    if ([AdminOpsWu.WuDisk]::GetDiskFreeSpaceEx($p, [ref]$a, [ref]$t, [ref]$f)) { $o.espFree = $f; break }
    # Si Windows no responde por la ruta de volumen, se pregunta por el volumen mismo
    try { $v = Get-Volume -UniqueId $p -ErrorAction Stop; if ($null -ne $v.SizeRemaining) { $o.espFree = [uint64]$v.SizeRemaining; break } } catch {}
    try { $v = Get-CimInstance Win32_Volume -Filter ("DeviceID='" + $p.Replace('\', '\\') + "'") -ErrorAction Stop; if ($v -and $null -ne $v.FreeSpace) { $o.espFree = [uint64]$v.FreeSpace; break } } catch {}
  }
} catch {}
try { $o.cFree = [uint64](Get-PSDrive -Name ($env:SystemDrive.TrimEnd(':')) -ErrorAction Stop).Free } catch {}
try {
  $o.services = @(Get-CimInstance Win32_Service -Filter "Name='wuauserv' OR Name='bits' OR Name='cryptsvc'" -ErrorAction Stop | ForEach-Object { [pscustomobject]@{ name = $_.Name; mode = $_.StartMode; state = $_.State } })
} catch {}
$o.rebootPending = (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')
ConvertTo-Json -InputObject $o -Compress -Depth 4
"#;

const COMPONENTS_SCRIPT: &str = r#"
dism.exe /Online /Cleanup-Image /CheckHealth | Out-String
"#;

/// Lo que dice `DISM /CheckHealth` (en inglés o en español; los acentos pueden llegar rotos por la
/// consola, así que se miran sin ellos): `Some(false)` sano, `Some(true)` dañado, `None` no se sabe.
pub fn dism_damaged(output: &str) -> Option<bool> {
    let t: String = output.to_lowercase().chars().map(|c| if c.is_ascii() { c } else { '?' }).collect();
    if t.contains("no component store corruption") || t.contains("no se detect") {
        return Some(false);
    }
    let damaged = ["repairable", "reparable", "se puede reparar", "puede repararse", "damaged", "corrupt", "da?ad", "da?o"];
    damaged.iter().any(|k| t.contains(k)).then_some(true)
}

const CLOCK_SCRIPT: &str = r#"
try {
  $r = Invoke-WebRequest -Uri 'http://www.msftconnecttest.com/connecttest.txt' -Method Head -UseBasicParsing -TimeoutSec 6
  $d = [DateTimeOffset]::Parse([string]$r.Headers['Date'])
  [int64]([DateTimeOffset]::UtcNow - $d).TotalSeconds
} catch { 'unknown' }
"#;

/// Lee del equipo lo que este código necesita para decidir. Cada lectura que falla se queda en `None`.
pub fn gather(code: &str, admin: bool) -> Facts {
    let ps = |script: &str, secs: u64| crate::ps::powershell_opts(script, crate::ps::Opts { timeout: Some(Duration::from_secs(secs)), task: None });
    let mut facts: Facts = ps(FACTS_SCRIPT, 60).ok().and_then(|o| serde_json::from_str(o.trim()).ok()).unwrap_or_default();
    if admin && wants_components(code) {
        facts.components_damaged = ps(COMPONENTS_SCRIPT, 180).ok().and_then(|t| dism_damaged(&t));
        if facts.components_damaged.is_none() {
            log::info!("[wucause] DISM /CheckHealth no dio una respuesta que se pueda interpretar");
        }
    }
    if facts.esp_free.is_none() && is_one_of(code, ESP_CODES) {
        log::info!("[wucause] no se pudo medir el espacio libre de la partición EFI: no se afirma ninguna causa");
    }
    if wants_clock(code) {
        facts.clock_skew_sec = ps(CLOCK_SCRIPT, 20).ok().and_then(|o| o.trim().parse::<i64>().ok());
    }
    facts
}

/// La causa de un fallo de Windows Update en este equipo, o nada si no se puede afirmar ninguna.
#[tauri::command(async)]
pub fn update_cause(code: String) -> Option<Cause> {
    let c = norm(&code);
    if !(c.starts_with("0X") && c.len() <= 10 && c[2..].chars().all(|x| x.is_ascii_hexdigit())) {
        return None;
    }
    pick(&c, &gather(&c, crate::elevation::is_elevated()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;

    fn svc(name: &str, mode: &str, state: &str) -> ServiceFact {
        ServiceFact { name: name.into(), mode: mode.into(), state: state.into() }
    }

    fn healthy() -> Facts {
        Facts {
            esp_free: Some(60 * MB),
            c_free: Some(80 * GB),
            services: vec![svc("wuauserv", "Manual", "Stopped"), svc("bits", "Manual", "Stopped"), svc("cryptsvc", "Auto", "Running")],
            reboot_pending: Some(false),
            components_damaged: Some(false),
            clock_skew_sec: Some(2),
        }
    }

    #[test]
    fn a_healthy_machine_gives_no_cause() {
        for code in ["0x800f0922", "0x80070070", "0x80073712", "0x80072F8F", "0x80240034"] {
            assert_eq!(pick(code, &healthy()), None, "{code}");
        }
    }

    #[test]
    fn full_reserved_partition_explains_0x800f0922() {
        let f = Facts { esp_free: Some(12 * MB), ..healthy() };
        let c = pick("0x800F0922", &f).unwrap();
        assert_eq!(c.kind, "esp");
        assert_eq!(c.fix, Some("repair.esp-fonts"));
        assert_eq!(c.sentence, "No cabe en la partición reservada del sistema: quedan 12 MB y Windows necesita al menos 15 MB.");
    }

    #[test]
    fn unknown_reserved_partition_space_claims_nothing() {
        let f = Facts { esp_free: None, ..healthy() };
        assert_eq!(pick("0x800f0922", &f), None);
    }

    #[test]
    fn low_disk_explains_0x80070070_but_only_very_low_explains_other_codes() {
        let f = Facts { c_free: Some(7 * GB + GB / 2), ..healthy() };
        let c = pick("0x80070070", &f).unwrap();
        assert_eq!((c.kind, c.fix), ("disk", Some("cleanup.windows-update-cache")));
        assert!(c.sentence.contains("7,5 GB"), "{}", c.sentence);
        assert_eq!(pick("0x80240034", &f), None, "con 7,5 GB libres otro código no es cuestión de espacio");
        let f = Facts { c_free: Some(3 * GB), ..healthy() };
        assert_eq!(pick("0x80240034", &f).unwrap().kind, "disk");
    }

    #[test]
    fn disabled_services_win_over_the_code() {
        let mut f = healthy();
        f.services[0] = svc("wuauserv", "Disabled", "Stopped");
        let c = pick("0x80070422", &f).unwrap();
        assert_eq!((c.kind, c.fix), ("service", Some("repair.wu-services")));
        assert!(c.sentence.contains("Windows Update") && c.sentence.contains("deshabilitado"), "{}", c.sentence);
        // Parado a demanda no es un fallo; criptográficos parado, sí.
        assert_eq!(pick("0x80070422", &healthy()), None);
        let mut f = healthy();
        f.services[2] = svc("cryptsvc", "Auto", "Stopped");
        assert_eq!(pick("0x80070422", &f).unwrap().kind, "service");
    }

    #[test]
    fn pending_reboot_has_no_automatic_fix() {
        let f = Facts { reboot_pending: Some(true), ..healthy() };
        let c = pick("0x8024200B", &f).unwrap();
        assert_eq!((c.kind, c.fix), ("reboot", None));
    }

    #[test]
    fn damaged_components_lead_to_dism() {
        let f = Facts { components_damaged: Some(true), ..healthy() };
        let c = pick("0x800F081F", &f).unwrap();
        assert_eq!((c.kind, c.fix), ("components", Some("repair.dism")));
        // Sin haber podido comprobarlo, no se afirma.
        assert_eq!(pick("0x800F081F", &Facts { components_damaged: None, ..healthy() }), None);
    }

    #[test]
    fn clock_skew_for_certificate_errors() {
        let f = Facts { clock_skew_sec: Some(-3 * 3600), ..healthy() };
        let c = pick("0x80072F8F", &f).unwrap();
        assert_eq!((c.kind, c.fix), ("clock", Some("repair.time-sync")));
        assert!(c.sentence.contains("180 min atrasado"), "{}", c.sentence);
        assert_eq!(pick("0x80072F8F", &Facts { clock_skew_sec: Some(40), ..healthy() }), None);
    }

    #[test]
    fn dism_output_in_english_and_spanish() {
        for t in ["No component store corruption detected.\r\nThe operation completed successfully.", "No se detect\u{f3} da\u{f1}o en el almac\u{e9}n de componentes.", "No se detectaron da\u{f1}os.", "No se detect? ning?n da?o"] {
            assert_eq!(dism_damaged(t), Some(false), "{t}");
        }
        for t in [
            "The component store is repairable.",
            "Se puede reparar el almac\u{e9}n de componentes.",
            "El almac\u{e9}n de componentes se puede reparar.",
            "El almac\u{e9}n de componentes es reparable.",
            "El almac\u{e9}n de componentes est\u{e1} da\u{f1}ado.",
            "El almac?n de componentes est? da?ado.",
            "The component store is damaged.",
        ] {
            assert_eq!(dism_damaged(t), Some(true), "{t}");
        }
        assert_eq!(dism_damaged("Error: 740\r\nElevated permissions are required to run DISM."), None);
        assert_eq!(dism_damaged(""), None);
    }

    #[test]
    fn only_slow_checks_the_code_asks_for() {
        assert!(wants_components("0x800f081f") && !wants_components("0x800f0922"));
        assert!(wants_clock("0X80072F8F") && !wants_clock("0x80070070"));
    }

    #[test]
    fn facts_script_parses_and_reads_json() {
        for (name, s) in [("FACTS", FACTS_SCRIPT), ("COMPONENTS", COMPONENTS_SCRIPT), ("CLOCK", CLOCK_SCRIPT)] {
            let e = crate::ps::parse_errors(s);
            assert!(e.is_empty(), "{name}: {e}");
        }
        let f: Facts = serde_json::from_str(r#"{"espFree":12582912,"cFree":5000,"services":[{"name":"bits","mode":"Manual","state":"Stopped"}],"rebootPending":false}"#).unwrap();
        assert_eq!(f.esp_free, Some(12 * MB));
        assert_eq!(f.services.len(), 1);
        assert_eq!(f.components_damaged, None);
    }

    /// Equipo real: `cargo test real_facts -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_facts() {
        let f = gather("0x800f0922", crate::elevation::is_elevated());
        println!("{f:#?}");
        println!("{:?}", pick("0x800f0922", &f));
    }
}
