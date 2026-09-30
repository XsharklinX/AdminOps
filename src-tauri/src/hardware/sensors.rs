//! Temperaturas, ventiladores y cargas con LibreHardwareMonitor (MPL-2.0).
//!
//! La librería .NET se carga en un PowerShell persistente (ver pspool) y el
//! objeto `Computer` queda abierto en `$global:AdminOpsLhm`: la primera lectura
//! tarda ~1 s y las siguientes ~50 ms.
//!
//! Sin administrador se leen las GPU. CPU y placa necesitan administrador y el
//! driver PawnIO (firmado, libre); si falta se puede instalar desde la app.

use crate::ps;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sensor {
    pub hardware: String,
    pub hardware_type: String,
    pub name: String,
    pub kind: String,
    pub value: f64,
    pub max: Option<f64>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuSensors {
    pub name: String,
    pub temperature: Option<f64>,
    pub hotspot: Option<f64>,
    pub load: Option<f64>,
    pub fan_rpm: Option<f64>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Sensors {
    pub cpu_name: Option<String>,
    pub cpu_temp: Option<f64>,
    pub cpu_temp_max: Option<f64>,
    pub cpu_power: Option<f64>,
    pub gpus: Vec<GpuSensors>,
    /// (nombre, °C) de placa y discos.
    pub other_temps: Vec<(String, f64)>,
    pub fans: Vec<(String, f64)>,
    /// La CPU existe pero no da temperatura: falta administrador o PawnIO.
    pub cpu_needs_driver: bool,
    pub pawnio_installed: bool,
    pub all: Vec<Sensor>,
}

/// Quita el prefijo `\\?\` que devuelve Tauri: PowerShell (Join-Path, Add-Type)
/// no entiende esas rutas ("the value of argument drive is null").
fn plain(p: PathBuf) -> PathBuf {
    let s = p.display().to_string();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p,
    }
}

fn lhm_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from));
    [app.path().resource_dir().ok().map(|r| r.join("lhm")), exe_dir.clone().map(|d| d.join("lhm")), exe_dir.map(|d| d.join("resources").join("lhm"))]
        .into_iter()
        .flatten()
        .map(plain)
        .find(|d| d.join("LibreHardwareMonitorLib.dll").is_file())
}

pub fn pawnio_installed() -> bool {
    crate::tweaks::registry::read_u32(r"HKLM\SYSTEM\CurrentControlSet\Services\PawnIO", "Start").is_some()
}

/// Quita la marca «descargado de Internet» (flujo `Zone.Identifier`) a las DLL
/// de LibreHardwareMonitor. Al descomprimir el portable con el Explorador, Windows
/// marca cada archivo y .NET se niega a cargarlos (0x80131515). Es lo mismo que
/// «Propiedades → Desbloquear», y solo en la carpeta de AdminOps que lo necesita.
fn unblock(dir: &std::path::Path) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t) if t.is_file() => {
                    let mut ads = p.into_os_string();
                    ads.push(":Zone.Identifier");
                    let _ = std::fs::remove_file(&ads);
                }
                _ => {}
            }
        }
    }
}

/// Mensaje claro para los fallos típicos al cargar la biblioteca de sensores.
fn explain(err: &str) -> String {
    if err.contains("0x80131515") || err.to_lowercase().contains("operación no admitida") || err.contains("Operation is not supported") {
        return "Windows bloqueó el lector de temperaturas porque AdminOps se descargó de Internet. Cierra AdminOps, haz clic derecho en el .zip → Propiedades → «Desbloquear», y vuelve a descomprimirlo.".into();
    }
    if err.to_lowercase().contains("pawnio") {
        return err.into();
    }
    // Sin rutas (llevan el nombre del usuario): solo el motivo.
    let first = err.lines().next().unwrap_or(err);
    match first.find(" '") {
        Some(i) if first.contains(":\\") => format!("No se pudo iniciar el lector de temperaturas ({}).", first[..i].trim()),
        _ => first.to_string(),
    }
}

/// Tras un fallo, no reintentar durante un minuto (el Panel lee cada 5 s).
static LAST_FAILURE: std::sync::Mutex<Option<(std::time::Instant, String)>> = std::sync::Mutex::new(None);

fn raw(app: &tauri::AppHandle) -> Result<Vec<Sensor>, String> {
    let dir = lhm_dir(app).ok_or("No se encontró LibreHardwareMonitor junto a AdminOps.")?;
    if let Some((when, err)) = LAST_FAILURE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if when.elapsed() < Duration::from_secs(60) {
            return Err(err.clone());
        }
    }
    unblock(&dir);
    let r = raw_from_dir(&dir).map_err(|e| {
        log::warn!("Sensores: {e}");
        explain(&e)
    });
    *LAST_FAILURE.lock().unwrap_or_else(|e| e.into_inner()) = r.as_ref().err().map(|e| (std::time::Instant::now(), e.clone()));
    r
}

/// Resolución de dependencias en C#: LibreHardwareMonitor carga ensamblados
/// desde sus propios hilos, donde un scriptblock de PowerShell no puede
/// ejecutarse (tumba el proceso). Compilado una vez por proceso.
const RESOLVER: &str = r#"
using System; using System.IO; using System.Reflection;
public static class AdminOpsResolver {
  static string dir;
  public static void Install(string d) {
    if (dir != null) return;
    dir = d;
    AppDomain.CurrentDomain.AssemblyResolve += (s, e) => {
      var f = Path.Combine(dir, new AssemblyName(e.Name).Name + ".dll");
      return File.Exists(f) ? Assembly.LoadFrom(f) : null;
    };
  }
}
"#;

pub fn raw_from_dir(dir: &std::path::Path) -> Result<Vec<Sensor>, String> {
    let dir = dir.display().to_string().replace('\'', "''");
    let resolver = RESOLVER.replace('\'', "''");
    let script = format!(
        r#"
if (-not $global:AdminOpsLhm) {{
  $dir = '{dir}'
  if (-not ('AdminOpsResolver' -as [type])) {{ Add-Type -TypeDefinition '{resolver}' }}
  [AdminOpsResolver]::Install($dir)
  Add-Type -Path (Join-Path $dir 'LibreHardwareMonitorLib.dll')
  $c = New-Object LibreHardwareMonitor.Hardware.Computer
  $c.IsCpuEnabled = $true; $c.IsGpuEnabled = $true; $c.IsMotherboardEnabled = $true
  $c.IsStorageEnabled = $true; $c.IsControllerEnabled = $true
  $c.Open()
  $global:AdminOpsLhm = $c
}}
$out = foreach ($h in $global:AdminOpsLhm.Hardware) {{
  $h.Update()
  foreach ($sub in $h.SubHardware) {{ $sub.Update() }}
  foreach ($s in @($h.Sensors) + @($h.SubHardware | ForEach-Object {{ $_.Sensors }})) {{
    $k = "$($s.SensorType)"
    if ($null -ne $s.Value -and $k -in 'Temperature', 'Fan', 'Load', 'Power') {{
      [pscustomobject]@{{ hardware = $h.Name; hardwareType = "$($h.HardwareType)"; name = $s.Name; kind = $k
        value = [double]$s.Value; max = if ($null -ne $s.Max) {{ [double]$s.Max }} else {{ $null }} }}
    }}
  }}
}}
ConvertTo-Json -InputObject @($out) -Compress
"#
    );
    let out = ps::powershell_opts(&script, ps::Opts { timeout: Some(Duration::from_secs(45)), task: None })?;
    if out.is_empty() {
        return Ok(vec![]);
    }
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada de los sensores: {e}"))
}

/// Ordena los sensores en lo que la interfaz necesita.
pub fn summarize(all: Vec<Sensor>) -> Sensors {
    let valid = |s: &&Sensor| s.value > 0.0 && s.value < 150.0;
    let temps = |ty: &'static str| all.iter().filter(move |s| s.kind == "Temperature" && s.hardware_type == ty);

    // CPU: Tctl/Tdie en AMD, Package en Intel; si no, la media de núcleos.
    let cpu_temp = ["Core (Tctl/Tdie)", "CPU Package", "Package", "Core Average", "Core Max"]
        .iter()
        .find_map(|n| temps("Cpu").filter(valid).find(|s| s.name == *n))
        .or_else(|| temps("Cpu").find(valid));
    let cpu_name = all.iter().find(|s| s.hardware_type == "Cpu").map(|s| s.hardware.clone());
    let cpu_power = all.iter().find(|s| s.hardware_type == "Cpu" && s.kind == "Power" && s.name.contains("Package")).map(|s| s.value);

    let mut gpus: Vec<GpuSensors> = Vec::new();
    for s in all.iter().filter(|s| s.hardware_type.starts_with("Gpu")) {
        let g = match gpus.iter_mut().find(|g| g.name == s.hardware) {
            Some(g) => g,
            None => {
                gpus.push(GpuSensors { name: s.hardware.clone(), ..Default::default() });
                gpus.last_mut().unwrap()
            }
        };
        match (s.kind.as_str(), s.name.as_str()) {
            ("Temperature", "GPU Core") if s.value > 0.0 => g.temperature = Some(s.value),
            ("Temperature", "GPU Hot Spot") if s.value > 0.0 => g.hotspot = Some(s.value),
            ("Load", "GPU Core") => g.load = Some(s.value),
            ("Fan", _) if g.fan_rpm.is_none() => g.fan_rpm = Some(s.value),
            _ => {}
        }
    }
    let other_temps = all
        .iter()
        .filter(|s| s.kind == "Temperature" && matches!(s.hardware_type.as_str(), "Motherboard" | "SuperIO" | "Storage") && s.value > 0.0 && s.value < 120.0)
        .map(|s| (if s.hardware_type == "Storage" { s.hardware.clone() } else { format!("{} · {}", s.hardware, s.name) }, s.value))
        .collect();
    let fans = all.iter().filter(|s| s.kind == "Fan" && s.value > 0.0 && !s.hardware_type.starts_with("Gpu")).map(|s| (s.name.clone(), s.value)).collect();

    Sensors {
        cpu_temp_max: cpu_temp.and_then(|s| s.max).filter(|m| *m > 0.0),
        cpu_temp: cpu_temp.map(|s| s.value),
        cpu_needs_driver: cpu_name.is_some() && cpu_temp.is_none(),
        cpu_name,
        cpu_power,
        gpus,
        other_temps,
        fans,
        pawnio_installed: pawnio_installed(),
        all,
    }
}

/// Última lectura, para que las páginas que miran temperaturas a la vez (Panel
/// y Hardware) compartan una sola y no dupliquen el trabajo de PowerShell.
static LAST: std::sync::Mutex<Option<(std::time::Instant, Sensors)>> = std::sync::Mutex::new(None);
const SHARE_FOR: Duration = Duration::from_millis(2000);

pub fn read(app: &tauri::AppHandle) -> Result<Sensors, String> {
    if let Some((t, v)) = LAST.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if t.elapsed() < SHARE_FOR {
            return Ok(v.clone());
        }
    }
    let v = raw(app).map(summarize)?;
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((std::time::Instant::now(), v.clone()));
    Ok(v)
}

#[tauri::command(async)]
pub fn read_sensors(app: tauri::AppHandle) -> Result<Sensors, String> {
    read(&app)
}

/// Abre el aviso de licencias de los componentes de terceros incluidos.
#[tauri::command]
pub fn open_third_party_notices(app: tauri::AppHandle) -> Result<(), String> {
    let file = lhm_dir(&app).map(|d| d.join("THIRD-PARTY-NOTICES.txt")).filter(|f| f.is_file()).ok_or("No se encontró el aviso de licencias.")?;
    std::process::Command::new("explorer.exe").arg(file).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Instala el driver PawnIO (libre y firmado) con winget. Tras instalarlo hay
/// que reabrir AdminOps para que LibreHardwareMonitor lo use.
#[tauri::command(async)]
pub fn install_pawnio(app: tauri::AppHandle) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let task = crate::task::Task::new(&app, "pawnio");
    task.step("Instalando PawnIO con winget…");
    let script = "winget install --id namazso.PawnIO --exact --silent --accept-package-agreements --accept-source-agreements \
                  --disable-interactivity | Out-String\n\"EXIT:$LASTEXITCODE\"";
    let out = ps::powershell_opts(script, task.opts(Some(Duration::from_secs(10 * 60))))?;
    if pawnio_installed() || out.trim_end().ends_with("EXIT:0") {
        Ok("PawnIO instalado. Reinicia AdminOps para ver las temperaturas de CPU y placa.".into())
    } else {
        Err("winget no pudo instalar PawnIO. Puedes instalarlo manualmente desde pawnio.eu.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_blocked_dll_without_paths() {
        let raw = r"No se puede cargar el archivo o ensamblado 'file:///C:\Users\admin\Downloads\x\lhm\LibreHardwareMonitorLib.dll' ni una de sus dependencias. Operación no admitida. (Excepción de HRESULT: 0x80131515)";
        let m = explain(raw);
        assert!(m.contains("Desbloquear"), "{m}");
        assert!(!m.contains("admin"));
        let other = explain(r"No se puede cargar 'C:\Users\ana\x.dll' por otra cosa");
        assert!(!other.contains("ana"), "{other}");
    }

    #[test]
    fn unblock_removes_zone_identifier() {
        let dir = std::env::temp_dir().join(format!("adminops-motw-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let f = dir.join("sub").join("a.dll");
        std::fs::write(&f, b"x").unwrap();
        let mut ads = f.clone().into_os_string();
        ads.push(":Zone.Identifier");
        // En unidades sin flujos alternativos (FAT) no hay nada que quitar.
        if std::fs::write(&ads, b"[ZoneTransfer]\r\nZoneId=3\r\n").is_ok() {
            assert!(std::fs::metadata(&ads).is_ok());
            unblock(&dir);
            assert!(std::fs::metadata(&ads).is_err());
            assert!(f.exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn s(hw: &str, ty: &str, name: &str, kind: &str, v: f64) -> Sensor {
        Sensor { hardware: hw.into(), hardware_type: ty.into(), name: name.into(), kind: kind.into(), value: v, max: None }
    }

    #[test]
    fn summarizes_amd_and_nvidia() {
        let r = summarize(vec![
            s("Ryzen 5 5500", "Cpu", "Core (Tctl/Tdie)", "Temperature", 52.0),
            s("GTX 1080", "GpuNvidia", "GPU Core", "Temperature", 44.0),
            s("GTX 1080", "GpuNvidia", "GPU Hot Spot", "Temperature", 55.4),
            s("GTX 1080", "GpuNvidia", "GPU Core", "Load", 12.0),
        ]);
        assert_eq!(r.cpu_temp, Some(52.0));
        assert!(!r.cpu_needs_driver);
        assert_eq!(r.gpus[0].temperature, Some(44.0));
        assert_eq!(r.gpus[0].hotspot, Some(55.4));
    }

    /// Lee los sensores reales dos veces en el mismo PowerShell persistente
    /// (la segunda reutiliza el objeto abierto): `cargo test sensors_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sensors_real() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join("lhm");
        for i in 0..2 {
            let t = std::time::Instant::now();
            let all = raw_from_dir(&dir).unwrap();
            let s = summarize(all);
            println!("lectura {i}: {} ms · CPU {:?} · GPUs {:?}", t.elapsed().as_millis(), s.cpu_temp, s.gpus.iter().map(|g| (&g.name, g.temperature)).collect::<Vec<_>>());
        }
    }

    #[test]
    fn strips_verbatim_prefix() {
        assert_eq!(plain(PathBuf::from(r"\\?\C:\Program Files\AdminOps\lhm")), PathBuf::from(r"C:\Program Files\AdminOps\lhm"));
        assert_eq!(plain(PathBuf::from(r"C:\x")), PathBuf::from(r"C:\x"));
    }

    #[test]
    fn zero_cpu_temp_means_driver_needed() {
        let r = summarize(vec![s("Ryzen", "Cpu", "Core (Tctl/Tdie)", "Temperature", 0.0), s("Ryzen", "Cpu", "CPU Total", "Load", 5.0)]);
        assert_eq!(r.cpu_temp, None);
        assert!(r.cpu_needs_driver);
    }
}
