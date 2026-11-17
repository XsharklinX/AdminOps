//! Impresoras y escáneres RICOH y EPSON: inventario con modelo, serie y páginas
//! (SNMP), estado del driver frente a lo que hay en Windows Update y en la
//! biblioteca local, actualización del driver con copia previa y deshacer,
//! recogida de lo que se baja de la web oficial y prueba de escaneo por WIA.
//!
//! Fuentes de drivers, en este orden: Windows Update (oficiales y firmados) →
//! biblioteca local (carpeta o pendrive) → página oficial abierta por AdminOps.
//! Nunca se baja nada a ciegas ni de webs de terceros.
//!
//! La lógica que no toca el equipo (leer un `.inf`, comparar versiones, marca y
//! modelo, estado del driver) vive en `inf.rs` y `logic.rs`, con sus pruebas.

mod inf;
mod logic;

use crate::fixes::printdeep::version_text;
use crate::ps::{text_var, Opts};
use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use base64::Engine;
use inf::{decode_inf, parse_inf, InfInfo, InfModel};
use logic::{
    best_version, brand_key, classify_driver, clean_model, compare_versions, connection_of, driver_status, extract_version, hw_match, identify, pick_model, same_model, scan_software_id, usb_id,
    usable_version, wia_error, Connection, Status, Vendor,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

const NEED_ADMIN: &str = "Requiere ejecutar AdminOps como administrador.";

// ---------- Utilidades ----------

fn now_epoch() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Datos de este equipo (caché de Windows Update, deshacer).
fn machine_dir(app: &AppHandle) -> PathBuf {
    crate::paths::machine_data_dir(app).join("printvendor")
}

/// Datos que viajan con AdminOps (carpeta de la biblioteca, índice).
fn shared_dir(app: &AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("printvendor")
}

fn run_ps(script: &str, secs: u64) -> Result<String, String> {
    crate::ps::powershell_opts(script, Opts { timeout: Some(Duration::from_secs(secs)), task: None })
}

fn valid_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.len() > 300 {
        return Err("Nombre de impresora no válido.".into());
    }
    Ok(())
}

/// Texto de PowerShell que puede venir como nada, un texto o una lista de textos.
fn lenient_vec<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        Many(Vec<Option<String>>),
        One(String),
    }
    Ok(match Option::<OneOrMany>::deserialize(d)? {
        Some(OneOrMany::Many(v)) => v.into_iter().flatten().collect(),
        Some(OneOrMany::One(s)) => vec![s],
        None => Vec::new(),
    })
}

/// Un nombre apto para una carpeta de Windows.
fn safe_name(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let cleaned: String = cleaned.trim().trim_matches('.').chars().take(80).collect();
    if cleaned.is_empty() {
        "paquete".into()
    } else {
        cleaned
    }
}

/// Identificador estable corto (FNV-1a) para paquetes y modelos.
fn short_id(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.to_ascii_lowercase().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn read_inf_file(path: &Path) -> Option<InfInfo> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 16 * 1024 * 1024 {
        return None;
    }
    Some(parse_inf(&decode_inf(&std::fs::read(path).ok()?)))
}

// ---------- Tipos que ve la interfaz ----------

/// De qué aparato se habla (lo que devuelve el inventario, tal cual).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceRef {
    /// "printer" | "scanner" | cualquier otra cosa (aparato sin instalar).
    pub kind: String,
    pub name: String,
    /// "ricoh" | "epson" | "otra"
    pub vendor: String,
    /// Marca en minúsculas si se conoce («hp», «brother»…), vacío si no.
    pub brand: String,
    pub model: String,
    pub hardware_ids: Vec<String>,
    pub driver_name: String,
    pub driver_version: String,
    pub driver_manufacturer: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriverState {
    pub status: Status,
    /// Texto corto para el chip («Al día», «Viejo»…).
    pub label: String,
    /// El motivo, para la tarjeta que sale al pasar el ratón.
    pub reason: String,
    /// Versión más nueva conocida (si hay).
    pub new_version: Option<String>,
    /// De dónde sale: «Windows Update» o «la biblioteca».
    pub source: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriverInfo {
    pub name: String,
    pub version: String,
    pub date: String,
    pub manufacturer: String,
    /// Solo el nombre del `.inf`, sin ruta.
    pub inf: String,
    /// "fabricante" | "generico" | "otraMarca" | "ninguno"
    pub kind: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnmpInfo {
    pub model: String,
    pub serial: String,
    pub pages: Option<u64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VendorPrinterInfo {
    pub name: String,
    /// "ricoh" | "epson" | "otra"
    pub vendor: String,
    pub brand: String,
    pub model: String,
    pub connection: Connection,
    pub driver: DriverInfo,
    pub default: bool,
    pub shared: bool,
    pub hardware_ids: Vec<String>,
    /// Lo que dice la propia impresora (solo las de red que contestan).
    pub snmp: Option<SnmpInfo>,
    pub state: DriverState,
    /// Para pasárselo tal cual a `driver_candidates`.
    pub device: DeviceRef,
}

/// Aparato conectado al que le falta el driver (o con un problema de instalación).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DetectedDevice {
    pub name: String,
    pub instance_id: String,
    pub class: String,
    /// Código de problema de Windows (28 = sin driver).
    pub problem: u32,
    pub vendor: String,
    pub model: String,
    /// «04B8:1100», útil si Windows no le pone nombre.
    pub usb_id: String,
    pub hardware_ids: Vec<String>,
    pub device: DeviceRef,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInventory {
    pub printers: Vec<VendorPrinterInfo>,
    pub not_installed: Vec<DetectedDevice>,
    /// Cuándo fue la última búsqueda en Windows Update (segundos desde 1970), si la hubo.
    pub windows_update_at: Option<u64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScannerInfo {
    pub name: String,
    pub vendor: String,
    pub model: String,
    pub instance_id: String,
    pub hardware_ids: Vec<String>,
    /// "funciona" | "falta"
    pub health: String,
    pub problem: u32,
    pub driver: Option<DriverInfo>,
    /// Interfaces de escaneo detectadas: "WIA", "TWAIN", "PaperStream IP".
    pub interfaces: Vec<String>,
    pub state: DriverState,
    pub device: DeviceRef,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanSoftware {
    /// "epsonscan2" | "paperstream-capture" | "paperstream-ip" | "scansnap-home"
    pub id: String,
    pub name: String,
    /// "epson" | "ricoh"
    pub vendor: String,
    pub installed: bool,
    pub version: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScannerInventory {
    pub scanners: Vec<ScannerInfo>,
    pub not_installed: Vec<DetectedDevice>,
    pub software: Vec<ScanSoftware>,
    pub windows_update_at: Option<u64>,
}

// ---------- Lectura del equipo ----------

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawPrinter {
    name: String,
    driver: String,
    port: String,
    default: bool,
    shared: bool,
    network: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawDriver {
    name: String,
    manufacturer: String,
    version: u64,
    inf: String,
    env: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawPort {
    name: String,
    host: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct RawPnp {
    name: String,
    device_id: String,
    #[serde(deserialize_with = "lenient_vec")]
    hardware_id: Vec<String>,
    class: String,
    problem: u32,
    present: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawInventory {
    printers: Vec<RawPrinter>,
    drivers: Vec<RawDriver>,
    ports: Vec<RawPort>,
    pnp: Vec<RawPnp>,
}

const PRINTERS_SCRIPT: &str = r#"
$ports = @(Get-PrinterPort -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ name = "$($_.Name)"; host = "$($_.PrinterHostAddress)" } })
$printers = @(Get-CimInstance Win32_Printer -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ name = "$($_.Name)"; driver = "$($_.DriverName)"; port = "$($_.PortName)"; default = [bool]$_.Default; shared = [bool]$_.Shared; network = [bool]$_.Network } })
$drivers = @(Get-PrinterDriver -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ name = "$($_.Name)"; manufacturer = "$($_.Manufacturer)"; version = [uint64]$_.DriverVersion; inf = "$($_.InfPath)"; env = "$($_.PrinterEnvironment)" } })
$pnp = @(Get-CimInstance Win32_PnPEntity -Filter "PNPClass = 'Printer' OR PNPClass = 'PrintQueue' OR ConfigManagerErrorCode <> 0" -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ name = "$($_.Name)"; deviceId = "$($_.DeviceID)"; hardwareId = @(@($_.HardwareID) | Where-Object { $_ }); class = "$($_.PNPClass)"; problem = [int]$_.ConfigManagerErrorCode; present = if ($null -eq $_.Present) { $true } else { [bool]$_.Present } }
})
ConvertTo-Json -InputObject ([pscustomobject]@{ printers = $printers; drivers = $drivers; ports = $ports; pnp = $pnp }) -Depth 5 -Compress
"#;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawSigned {
    device_id: String,
    name: String,
    provider: String,
    manufacturer: String,
    version: String,
    date: String,
    inf: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawSoft {
    name: String,
    version: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawScanners {
    images: Vec<RawPnp>,
    drivers: Vec<RawSigned>,
    twain: Vec<String>,
    software: Vec<RawSoft>,
    bad: Vec<RawPnp>,
}

const SCANNERS_SCRIPT: &str = r#"
$pnp = { param($filter) @(Get-CimInstance Win32_PnPEntity -Filter $filter -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ name = "$($_.Name)"; deviceId = "$($_.DeviceID)"; hardwareId = @(@($_.HardwareID) | Where-Object { $_ }); class = "$($_.PNPClass)"; problem = [int]$_.ConfigManagerErrorCode; present = if ($null -eq $_.Present) { $true } else { [bool]$_.Present } }
}) }
$images = @(& $pnp "PNPClass = 'Image'")
$bad = @(& $pnp "ConfigManagerErrorCode <> 0")
$drivers = @(Get-CimInstance Win32_PnPSignedDriver -Filter "DeviceClass = 'IMAGE'" -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ deviceId = "$($_.DeviceID)"; name = "$($_.DeviceName)"; provider = "$($_.DriverProviderName)"; manufacturer = "$($_.Manufacturer)"; version = "$($_.DriverVersion)"
    date = if ($_.DriverDate) { ([datetime]$_.DriverDate).ToString('yyyy-MM-dd') } else { '' }; inf = "$($_.InfName)" }
})
$twain = @(Get-ChildItem (Join-Path $env:windir 'twain_32') -Directory -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Name)" })
$keys = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
$software = @(foreach ($k in $keys) {
  Get-ItemProperty $k -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match 'Epson ?Scan|PaperStream|ScanSnap' } | ForEach-Object { [pscustomobject]@{ name = "$($_.DisplayName)"; version = "$($_.DisplayVersion)" } }
})
ConvertTo-Json -InputObject ([pscustomobject]@{ images = $images; drivers = $drivers; twain = $twain; software = $software; bad = $bad }) -Depth 5 -Compress
"#;

fn is_print_class(class: &str) -> bool {
    class.eq_ignore_ascii_case("printer") || class.eq_ignore_ascii_case("printqueue")
}

/// Aparatos con problema de instalación: con ID de RICOH/EPSON/Fujitsu y cualquier
/// problema que no sea «desactivado», o impresoras con el código 28 (sin driver).
fn detect_not_installed(pnp: &[RawPnp]) -> Vec<DetectedDevice> {
    const DISABLED: u32 = 22;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for d in pnp {
        if d.problem == 0 || d.problem == DISABLED || d.present == Some(false) {
            continue;
        }
        let by_id = d.hardware_id.iter().find_map(|h| logic::brand_from_hwid(h)).or_else(|| logic::brand_from_hwid(&d.device_id));
        if by_id.is_none() && !(is_print_class(&d.class) && d.problem == 28) {
            continue;
        }
        if !seen.insert(d.device_id.to_ascii_lowercase()) {
            continue;
        }
        let brand = by_id.or_else(|| brand_key(&d.name));
        let vendor = Vendor::from_brand(brand);
        let model = if brand_key(&d.name).is_some() { clean_model(&d.name) } else { String::new() };
        let usb = d.hardware_id.iter().chain(std::iter::once(&d.device_id)).find_map(|h| usb_id(h)).unwrap_or_default();
        let device = DeviceRef {
            kind: "other".into(),
            name: d.name.clone(),
            vendor: vendor.id().into(),
            brand: brand.unwrap_or_default().into(),
            model: model.clone(),
            hardware_ids: d.hardware_id.clone(),
            ..Default::default()
        };
        out.push(DetectedDevice {
            name: d.name.clone(),
            instance_id: d.device_id.clone(),
            class: d.class.clone(),
            problem: d.problem,
            vendor: vendor.id().into(),
            model,
            usb_id: usb,
            hardware_ids: d.hardware_id.clone(),
            device,
        });
    }
    out.sort_by_key(|d| (d.vendor == "otra", d.name.to_lowercase()));
    out
}

/// El driver de una impresora por su nombre, prefiriendo el de 64 bits.
fn pick_driver<'a>(drivers: &'a [RawDriver], name: &str) -> Option<&'a RawDriver> {
    let mut same = drivers.iter().filter(|d| d.name.eq_ignore_ascii_case(name));
    let first = same.next()?;
    Some(std::iter::once(first).chain(same).find(|d| d.env.contains("x64")).unwrap_or(first))
}

/// Modelo «limpio»: el del primer texto que diga una marca; si ninguno la dice, el primero que sirva.
fn best_model(texts: &[&str]) -> String {
    texts
        .iter()
        .filter(|t| brand_key(t).is_some())
        .map(|t| clean_model(t))
        .find(|m| !m.is_empty())
        .or_else(|| texts.iter().map(|t| clean_model(t)).find(|m| !m.is_empty()))
        .unwrap_or_default()
}

fn dev_brand(dev: &DeviceRef) -> Option<String> {
    if !dev.brand.is_empty() {
        Some(dev.brand.to_ascii_lowercase())
    } else if dev.vendor == "ricoh" || dev.vendor == "epson" {
        Some(dev.vendor.clone())
    } else {
        brand_key(&dev.model).or_else(|| brand_key(&dev.name)).map(String::from)
    }
}

// ---------- Estado y candidatos ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Candidate {
    /// "lib:<paquete>|<modelo>" o "wu:<id de la actualización>|<modelo>"; se devuelve tal cual a `printer_driver_update`.
    pub id: String,
    /// "biblioteca" | "windowsUpdate"
    pub source: String,
    pub title: String,
    pub version: String,
    pub date: String,
    /// Nombre del driver que se instalaría (el modelo del inf).
    pub model: String,
    /// "hardwareId" | "modelo" | "mismoDriver" | "universal"
    pub matched_by: String,
    /// Se puede comparar con lo instalado (el universal no es comparable con un driver de modelo).
    pub comparable: bool,
    /// Más nueva que lo instalado (None: no se puede comparar).
    pub newer: Option<bool>,
    /// Carpeta o paquete de la biblioteca.
    pub package: String,
}

fn source_label(source: &str) -> &'static str {
    if source == "windowsUpdate" {
        "Windows Update"
    } else {
        "la biblioteca"
    }
}

fn match_rank(by: &str) -> u8 {
    match by {
        "hardwareId" => 0,
        "modelo" => 1,
        "mismoDriver" => 2,
        _ => 3,
    }
}

fn finish_candidates(mut cands: Vec<Candidate>, dev: &DeviceRef) -> Vec<Candidate> {
    for c in &mut cands {
        c.comparable = c.matched_by != "universal" || c.model.eq_ignore_ascii_case(&dev.driver_name);
        c.newer = if usable_version(&c.version) && usable_version(&dev.driver_version) { compare_versions(&c.version, &dev.driver_version).map(|o| o == Ordering::Greater) } else { None };
    }
    cands.sort_by(|a, b| {
        let by_version = match (usable_version(&a.version), usable_version(&b.version)) {
            (true, true) => compare_versions(&b.version, &a.version).unwrap_or(Ordering::Equal),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => Ordering::Equal,
        };
        by_version.then(match_rank(&a.matched_by).cmp(&match_rank(&b.matched_by))).then(a.source.cmp(&b.source))
    });
    cands.dedup_by(|a, b| a.id == b.id);
    cands
}

fn state_from(dev: &DeviceRef, cands: &[Candidate], searched_at: Option<u64>) -> DriverState {
    let brand = dev_brand(dev);
    let best = best_version(cands.iter().filter(|c| c.comparable).map(|c| c.version.as_str()));
    let status = driver_status(brand.as_deref(), &dev.driver_name, &dev.driver_manufacturer, &dev.driver_version, best);
    let best_c = best.and_then(|b| cands.iter().find(|c| c.comparable && c.version == b));
    let installed = if dev.driver_version.is_empty() { "versión desconocida".to_string() } else { dev.driver_version.clone() };
    let source = best_c.map(|c| source_label(&c.source));
    let thing = if dev.kind == "scanner" { "del escáner" } else { "del modelo" };
    let reason = match status {
        Status::Falta => "No tiene ningún driver instalado.".to_string(),
        Status::OtraMarca => format!("Usa «{}», que es de otra marca. Con el de otra marca suele imprimir mal o no funcionar: conviene el del fabricante.", dev.driver_name),
        Status::Generico => format!("Usa «{}», un driver genérico: funciona, pero sin las funciones propias {thing} (bandejas, dúplex, nivel de tóner).", dev.driver_name),
        Status::Viejo => format!("Instalada: {installed}. Hay una más nueva: {} (de {}).", best.unwrap_or_default(), source.unwrap_or("la biblioteca")),
        Status::AlDia => format!("Instalada: {installed}. Lo más nuevo que se conoce es {} (de {}): no hay nada más reciente.", best.unwrap_or_default(), source.unwrap_or("la biblioteca")),
        Status::SinComparar => {
            if !usable_version(&dev.driver_version) {
                "No se pudo leer la versión del driver instalado, así que no se puede comparar.".to_string()
            } else if searched_at.is_none() {
                "No hay con qué comparar: la biblioteca no tiene un paquete para este modelo y aún no se ha buscado en Windows Update.".to_string()
            } else {
                "No hay con qué comparar: ni la biblioteca ni Windows Update tienen un driver para este modelo.".to_string()
            }
        }
    };
    DriverState { status, label: status.label().into(), reason, new_version: matches!(status, Status::Viejo).then(|| best.unwrap_or_default().to_string()), source: source.map(String::from) }
}

// ---------- Inventario de impresoras ----------

fn snmp_parallel(ips: &[IpAddr]) -> HashMap<IpAddr, crate::snmp::PrinterIds> {
    std::thread::scope(|s| {
        let handles: Vec<_> = ips.iter().map(|&ip| s.spawn(move || (ip, crate::snmp::printer_ids(ip, Duration::from_secs(2))))).collect();
        handles.into_iter().filter_map(|h| h.join().ok()).filter_map(|(ip, ids)| ids.map(|i| (ip, i))).collect()
    })
}

fn build_printer_inventory(app: &AppHandle, raw: RawInventory) -> PrinterInventory {
    let printers: Vec<&RawPrinter> = raw.printers.iter().filter(|p| !crate::printers::is_virtual(&p.port, &p.driver)).collect();
    let conns: Vec<Connection> = printers
        .iter()
        .map(|p| {
            let host = raw.ports.iter().find(|x| x.name.eq_ignore_ascii_case(&p.port)).map(|x| x.host.as_str()).unwrap_or("");
            connection_of(&p.port, host, p.network)
        })
        .collect();
    // SNMP a todas las de red a la vez: una apagada no retrasa a las demás.
    let mut ips: Vec<IpAddr> = conns.iter().filter_map(|c| c.ip.as_deref().and_then(|s| s.parse::<IpAddr>().ok())).filter(IpAddr::is_ipv4).collect();
    ips.sort();
    ips.dedup();
    let snmp = snmp_parallel(&ips);
    let (idx, wu) = (index_if_any(app), read_wu_cache(app));
    let searched_at = (wu.searched_at > 0).then_some(wu.searched_at);

    let mut out = Vec::new();
    for (p, conn) in printers.iter().zip(&conns) {
        let ids = conn.ip.as_deref().and_then(|s| s.parse::<IpAddr>().ok()).and_then(|ip| snmp.get(&ip));
        let drv = pick_driver(&raw.drivers, &p.driver);
        let inf_info = drv.and_then(|d| read_inf_file(Path::new(&d.inf)));
        let inf_ver = inf_info.as_ref().map(|i| i.driver_ver.clone()).unwrap_or_default();
        let version = if usable_version(&inf_ver.version) { inf_ver.version.clone() } else { drv.map(|d| version_text(d.version)).unwrap_or_default() };
        let manufacturer = drv.map(|d| d.manufacturer.clone()).filter(|m| !m.is_empty()).or_else(|| inf_info.as_ref().map(|i| i.provider.clone())).unwrap_or_default();
        let hardware_ids: Vec<String> = raw.pnp.iter().filter(|d| is_print_class(&d.class) && d.name.eq_ignore_ascii_case(&p.name)).flat_map(|d| d.hardware_id.clone()).collect();
        let snmp_model = ids.map(|i| i.model.as_str()).unwrap_or("");
        let texts = [snmp_model, p.name.as_str(), p.driver.as_str()];
        let brand = identify(&hardware_ids, &texts);
        let vendor = Vendor::from_brand(brand);
        let device = DeviceRef {
            kind: "printer".into(),
            name: p.name.clone(),
            vendor: vendor.id().into(),
            brand: brand.unwrap_or_default().into(),
            model: best_model(&texts),
            hardware_ids: hardware_ids.clone(),
            driver_name: p.driver.clone(),
            driver_version: version.clone(),
            driver_manufacturer: manufacturer.clone(),
        };
        let mut cands = library_candidates(&idx, &device);
        cands.extend(wu_candidates(&wu, &device));
        let cands = finish_candidates(cands, &device);
        let driver = DriverInfo {
            name: p.driver.clone(),
            version,
            date: inf_ver.date,
            manufacturer,
            inf: drv.and_then(|d| Path::new(&d.inf).file_name()).map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            kind: classify_driver(dev_brand(&device).as_deref(), &p.driver, &device.driver_manufacturer).id().into(),
        };
        out.push(VendorPrinterInfo {
            name: p.name.clone(),
            vendor: vendor.id().into(),
            brand: device.brand.clone(),
            model: device.model.clone(),
            connection: conn.clone(),
            driver,
            default: p.default,
            shared: p.shared,
            hardware_ids,
            snmp: ids.map(|i| SnmpInfo { model: i.model.clone(), serial: i.serial.clone(), pages: i.pages }),
            state: state_from(&device, &cands, searched_at),
            device,
        });
    }
    out.sort_by_key(|p| (p.vendor == "otra", !p.default, p.name.to_lowercase()));
    PrinterInventory { printers: out, not_installed: detect_not_installed(&raw.pnp), windows_update_at: searched_at }
}

/// Impresoras instaladas (no virtuales) con marca, modelo, conexión, driver y su estado;
/// por SNMP, modelo, serie y páginas de las de red; y los aparatos conectados sin instalar.
#[tauri::command(async)]
pub fn printer_inventory(app: AppHandle) -> Result<PrinterInventory, String> {
    let out = run_ps(PRINTERS_SCRIPT, 90)?;
    let raw: RawInventory = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    Ok(build_printer_inventory(&app, raw))
}

// ---------- Inventario de escáneres ----------

/// Marca de un aparato de imagen: solo interesan RICOH, EPSON y PFU/Fujitsu (RICOH).
fn scanner_brand(p: &RawPnp) -> Option<&'static str> {
    p.hardware_id.iter().find_map(|h| logic::brand_from_hwid(h)).or_else(|| logic::brand_from_hwid(&p.device_id)).or_else(|| brand_key(&p.name).filter(|b| *b == "ricoh" || *b == "epson"))
}

/// Qué interfaces de escaneo hay para ese aparato (a partir de lo instalado; es una pista, no una prueba).
fn scan_interfaces(brand: &str, has_driver: bool, twain: &[String], software: &[ScanSoftware]) -> Vec<String> {
    let mut v = Vec::new();
    if has_driver {
        v.push("WIA".to_string());
    }
    let keys: &[&str] = if brand == "epson" { &["epson", "escn"] } else { &["pfu", "fujitsu", "paperstream", "scansnap", "ricoh", "fi"] };
    if twain.iter().any(|t| {
        let t = t.to_ascii_lowercase();
        keys.iter().any(|k| t.contains(k))
    }) {
        v.push("TWAIN".to_string());
    }
    if brand == "ricoh" && software.iter().any(|s| s.id == "paperstream-ip" && s.installed) {
        v.push("PaperStream IP".to_string());
    }
    v
}

fn build_scanner_inventory(app: &AppHandle, raw: RawScanners) -> ScannerInventory {
    let known = [
        ("epsonscan2", "Epson Scan 2", "epson"),
        ("paperstream-capture", "PaperStream Capture", "ricoh"),
        ("paperstream-ip", "PaperStream IP", "ricoh"),
        ("scansnap-home", "ScanSnap Home", "ricoh"),
    ];
    let software: Vec<ScanSoftware> = known
        .iter()
        .map(|(id, name, vendor)| {
            let found = raw.software.iter().find(|s| scan_software_id(&s.name) == Some(*id));
            ScanSoftware { id: (*id).into(), name: (*name).into(), vendor: (*vendor).into(), installed: found.is_some(), version: found.map(|f| f.version.clone()).unwrap_or_default() }
        })
        .collect();
    let (idx, wu) = (index_if_any(app), read_wu_cache(app));
    let searched_at = (wu.searched_at > 0).then_some(wu.searched_at);

    let mut scanners = Vec::new();
    for p in &raw.images {
        let Some(brand) = scanner_brand(p) else { continue };
        let signed = raw.drivers.iter().find(|d| d.device_id.eq_ignore_ascii_case(&p.device_id));
        let health = if p.problem == 0 && p.present != Some(false) { "funciona" } else { "falta" };
        let ver = signed.map(|d| d.version.clone()).unwrap_or_default();
        let provider = signed.map(|d| if d.provider.is_empty() { d.manufacturer.clone() } else { d.provider.clone() }).unwrap_or_default();
        let driver_name = signed.map(|d| d.name.clone()).filter(|n| !n.is_empty()).unwrap_or_default();
        let device = DeviceRef {
            kind: "scanner".into(),
            name: p.name.clone(),
            vendor: Vendor::from_brand(Some(brand)).id().into(),
            brand: brand.into(),
            model: clean_model(&p.name),
            hardware_ids: p.hardware_id.clone(),
            driver_name: if health == "funciona" { driver_name.clone() } else { String::new() },
            driver_version: ver.clone(),
            driver_manufacturer: provider.clone(),
        };
        let mut cands = library_candidates(&idx, &device);
        cands.extend(wu_candidates(&wu, &device));
        let cands = finish_candidates(cands, &device);
        let driver = signed.filter(|_| health == "funciona").map(|d| DriverInfo {
            name: driver_name.clone(),
            version: ver.clone(),
            date: d.date.clone(),
            manufacturer: provider.clone(),
            inf: d.inf.clone(),
            kind: classify_driver(Some(brand), &driver_name, &provider).id().into(),
        });
        scanners.push(ScannerInfo {
            name: p.name.clone(),
            vendor: device.vendor.clone(),
            model: device.model.clone(),
            instance_id: p.device_id.clone(),
            hardware_ids: p.hardware_id.clone(),
            health: health.into(),
            problem: p.problem,
            interfaces: scan_interfaces(brand, driver.is_some(), &raw.twain, &software),
            state: state_from(&device, &cands, searched_at),
            driver,
            device,
        });
    }
    scanners.sort_by_key(|s| s.name.to_lowercase());
    ScannerInventory { scanners, not_installed: detect_not_installed(&raw.bad), software, windows_update_at: searched_at }
}

/// Escáneres RICOH, EPSON y PFU/Fujitsu: driver, interfaces, estado y programas de escaneo instalados.
#[tauri::command(async)]
pub fn scanner_inventory(app: AppHandle) -> Result<ScannerInventory, String> {
    let out = run_ps(SCANNERS_SCRIPT, 120)?;
    let raw: RawScanners = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    Ok(build_scanner_inventory(&app, raw))
}

// ---------- Biblioteca ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LibDriver {
    pub id: String,
    /// Carpeta de primer nivel de la biblioteca donde está.
    pub package: String,
    pub file: String,
    /// Ruta dentro de la biblioteca, con «/».
    pub rel_path: String,
    pub class: String,
    pub provider: String,
    /// "ricoh" | "epson" | "otra"
    pub vendor: String,
    pub version: String,
    pub date: String,
    pub models: Vec<String>,
    pub model_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct IndexedDriver {
    driver: LibDriver,
    entries: Vec<InfModel>,
}

/// Instalador del fabricante que no se pudo abrir como zip (se guarda tal cual).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LibInstaller {
    /// Para `run_vendor_installer`.
    pub id: String,
    pub name: String,
    pub rel_path: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct LibIndex {
    dir: String,
    scanned_at: u64,
    drivers: Vec<IndexedDriver>,
    installers: Vec<LibInstaller>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Config {
    library_dir: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    /// La carpeta elegida por el técnico (no se da la de por defecto: lleva el nombre de usuario).
    pub custom_dir: Option<String>,
    pub is_default: bool,
    /// La carpeta existe (un pendrive sin conectar no existe).
    pub exists: bool,
    pub scanned_at: u64,
    pub drivers: Vec<LibDriver>,
    pub installers: Vec<LibInstaller>,
}

const MAX_SCAN_ENTRIES: usize = 60_000;
const INSTALLERS_DIR: &str = "Instaladores";

fn config_path(app: &AppHandle) -> PathBuf {
    shared_dir(app).join("config.json")
}

fn index_path(app: &AppHandle) -> PathBuf {
    shared_dir(app).join("library-index.json")
}

fn custom_library_dir(app: &AppHandle) -> Option<PathBuf> {
    let c: Config = crate::paths::read_json(&config_path(app));
    let d = c.library_dir.trim();
    (!d.is_empty()).then(|| PathBuf::from(d))
}

fn library_dir(app: &AppHandle) -> PathBuf {
    custom_library_dir(app).unwrap_or_else(|| crate::paths::shared_data_dir(app).join("drivers"))
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")).unwrap_or_default()
}

fn index_inf(path: &Path, rel: &str) -> Option<IndexedDriver> {
    let info = read_inf_file(path)?;
    if !info.arch_x64 || info.models.is_empty() {
        return None;
    }
    let class = info.class.to_ascii_lowercase();
    if !matches!(class.as_str(), "printer" | "printqueue" | "image" | "scanner") {
        return None;
    }
    let brand = brand_key(&info.provider).or_else(|| info.models.iter().find_map(|m| brand_key(m)));
    let package = rel.split_once('/').map(|(p, _)| p.to_string()).unwrap_or_else(|| "(raíz)".into());
    let driver = LibDriver {
        id: short_id(rel),
        package,
        file: rel.rsplit('/').next().unwrap_or(rel).to_string(),
        rel_path: rel.to_string(),
        class: info.class.clone(),
        provider: info.provider.clone(),
        vendor: Vendor::from_brand(brand).id().into(),
        version: info.driver_ver.version.clone(),
        date: info.driver_ver.date.clone(),
        model_count: info.models.len(),
        models: info.models.iter().take(200).cloned().collect(),
    };
    Some(IndexedDriver { driver, entries: info.entries })
}

/// Recorre la biblioteca y lee sus `.inf` (solo 64 bits, clase impresora o imagen).
fn scan_library(dir: &Path) -> LibIndex {
    let mut idx = LibIndex { dir: dir.display().to_string(), scanned_at: now_epoch(), ..Default::default() };
    let mut stack = vec![(dir.to_path_buf(), 0u32)];
    let mut visited = 0usize;
    'walk: while let Some((cur, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&cur) else { continue };
        for e in entries.flatten() {
            visited += 1;
            if visited > MAX_SCAN_ENTRIES {
                break 'walk;
            }
            let path = e.path();
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                if depth < 8 {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            let ext = path.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
            let rel = rel_of(dir, &path);
            if ext == "inf" {
                if let Some(d) = index_inf(&path, &rel) {
                    idx.drivers.push(d);
                }
            } else if ext == "exe" && (!rel.contains('/') || rel.starts_with(&format!("{INSTALLERS_DIR}/"))) {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                idx.installers.push(LibInstaller { id: short_id(&rel), name: rel.rsplit('/').next().unwrap_or(&rel).to_string(), rel_path: rel, size });
            }
        }
    }
    idx.drivers.sort_by(|a, b| (&a.driver.package, &a.driver.rel_path).cmp(&(&b.driver.package, &b.driver.rel_path)));
    idx.installers.sort_by_key(|i| i.name.to_lowercase());
    idx
}

/// Índice guardado, si es de esta misma carpeta (sin recorrer nada).
fn index_if_any(app: &AppHandle) -> LibIndex {
    let idx: LibIndex = crate::paths::read_json(&index_path(app));
    if idx.dir == library_dir(app).display().to_string() {
        idx
    } else {
        LibIndex::default()
    }
}

/// El índice, o uno nuevo si se pide, no existe o la carpeta cambió.
fn load_index(app: &AppHandle, force: bool) -> LibIndex {
    if !force {
        let idx = index_if_any(app);
        if idx.scanned_at > 0 {
            return idx;
        }
    }
    let dir = library_dir(app);
    if !dir.is_dir() {
        return LibIndex { dir: dir.display().to_string(), ..Default::default() };
    }
    let idx = scan_library(&dir);
    if let Err(e) = crate::paths::write_json(&index_path(app), &idx) {
        log::warn!("Biblioteca de drivers: no se pudo guardar el índice: {e}");
    }
    idx
}

fn library_view(app: &AppHandle, idx: LibIndex) -> Library {
    let custom = custom_library_dir(app);
    Library {
        is_default: custom.is_none(),
        // La carpeta que eligió el técnico, con el nombre de usuario de Windows tapado si lo lleva.
        custom_dir: custom.as_ref().map(|d| crate::escalate::scrub(&d.display().to_string())),
        exists: library_dir(app).is_dir(),
        scanned_at: idx.scanned_at,
        drivers: idx.drivers.into_iter().map(|d| d.driver).collect(),
        installers: idx.installers.into_iter().map(|i| LibInstaller { id: short_id(&i.rel_path), ..i }).collect(),
    }
}

/// Paquetes de la biblioteca (lee el índice guardado; con `rescan` vuelve a recorrer la carpeta).
#[tauri::command(async)]
pub fn driver_library(app: AppHandle, rescan: Option<bool>) -> Library {
    let idx = load_index(&app, rescan.unwrap_or(false));
    library_view(&app, idx)
}

/// Cambia la carpeta de la biblioteca (`None` o vacío: la de por defecto). Solo guarda
/// un ajuste de AdminOps; después hay que llamar a `driver_library(true)`.
#[tauri::command(async)]
pub fn set_driver_library_dir(app: AppHandle, dir: Option<String>) -> Result<(), String> {
    let dir = dir.unwrap_or_default();
    let dir = dir.trim();
    if !dir.is_empty() {
        let p = Path::new(dir);
        if !p.is_absolute() {
            return Err("La carpeta debe ser una ruta completa.".into());
        }
        std::fs::create_dir_all(p).map_err(|_| "No se puede usar esa carpeta (¿el pendrive está conectado y no está protegido contra escritura?).".to_string())?;
        let probe = p.join(".adminops-write-test");
        let writable = std::fs::write(&probe, b"ok").is_ok();
        let _ = std::fs::remove_file(&probe);
        if !writable {
            return Err("No se puede escribir en esa carpeta.".into());
        }
    }
    crate::paths::write_json(&config_path(&app), &Config { library_dir: dir.to_string() })
}

const MAX_COPY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_COPY_FILES: usize = 50_000;

fn copy_tree(src: &Path, dst: &Path, bytes: &mut u64, files: &mut usize) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("No se pudo crear la carpeta de destino: {e}"))?;
    for e in std::fs::read_dir(src).map_err(|e| format!("No se pudo leer la carpeta: {e}"))?.flatten() {
        let Ok(kind) = e.file_type() else { continue };
        let target = dst.join(e.file_name());
        if kind.is_dir() {
            copy_tree(&e.path(), &target, bytes, files)?;
        } else if kind.is_file() {
            *files += 1;
            *bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            if *files > MAX_COPY_FILES || *bytes > MAX_COPY_BYTES {
                return Err("La carpeta es demasiado grande para una biblioteca de drivers.".into());
            }
            std::fs::copy(e.path(), &target).map_err(|e| format!("No se pudo copiar un archivo: {e}"))?;
        }
    }
    Ok(())
}

/// Saca un zip a una carpeta, sin salirse de ella (zip-slip) y con tope de tamaño.
fn extract_zip<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, dest: &Path) -> Result<(), String> {
    use std::io::Read;
    if zip.len() > MAX_COPY_FILES {
        return Err("El archivo tiene demasiados ficheros.".into());
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| format!("No se pudo leer el archivo comprimido: {e}"))?;
        let Some(rel) = f.enclosed_name() else { continue };
        let out = dest.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        total += f.size();
        if total > MAX_COPY_BYTES {
            return Err("El archivo descomprimido es demasiado grande.".into());
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = std::fs::File::create(&out).map_err(|e| format!("No se pudo escribir un archivo: {e}"))?;
        std::io::copy(&mut f.by_ref().take(MAX_COPY_BYTES), &mut file).map_err(|e| format!("No se pudo extraer un archivo: {e}"))?;
    }
    Ok(())
}

fn zip_has_inf<R: std::io::Read + std::io::Seek>(zip: &zip::ZipArchive<R>) -> bool {
    zip.file_names().any(|n| n.to_ascii_lowercase().ends_with(".inf"))
}

/// Un nombre libre dentro de `dir` («Paquete», «Paquete (2)»…).
fn free_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    (2..1000).map(|n| format!("{name} ({n})")).find(|c| !dir.join(c).exists()).unwrap_or_else(|| format!("{name} ({})", now_epoch()))
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    /// "carpeta" | "zip" | "exeAbierto" | "instalador"
    pub kind: String,
    /// Nombre con el que quedó en la biblioteca.
    pub name: String,
    /// Drivers de impresora o escáner (64 bits) que se encontraron dentro.
    pub drivers_found: usize,
    pub message: String,
}

/// Añade un paquete a la biblioteca: una carpeta, un `.zip` o un `.exe` autoextraíble
/// que se pueda abrir como zip; si no se puede abrir, se guarda tal cual como instalador
/// del fabricante. Solo copia archivos dentro de la carpeta de AdminOps: no instala nada.
#[tauri::command(async)]
pub fn driver_library_add(app: AppHandle, path: String) -> Result<AddResult, String> {
    let raw = path.trim();
    // «dl:…» es una descarga que AdminOps vio en Descargas; si no, una ruta que eligió el técnico con el diálogo.
    let src = if raw.starts_with("dl:") { download_path(raw).ok_or("Esa descarga ya no está disponible. Vuelve a vigilar Descargas.")? } else { PathBuf::from(raw) };
    if !src.is_absolute() || !src.exists() {
        return Err("No se encuentra ese archivo o carpeta.".into());
    }
    let lib = library_dir(&app);
    std::fs::create_dir_all(&lib).map_err(|_| "No se puede usar la carpeta de la biblioteca (¿el pendrive está conectado?).".to_string())?;
    let stem = safe_name(&src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let ext = src.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();

    let (kind, name) = if src.is_dir() {
        if lib.starts_with(&src) || src.starts_with(&lib) {
            return Err("Esa carpeta es la propia biblioteca (o está dentro de ella).".into());
        }
        let name = free_name(&lib, &stem);
        let (mut bytes, mut files) = (0u64, 0usize);
        if let Err(e) = copy_tree(&src, &lib.join(&name), &mut bytes, &mut files) {
            let _ = std::fs::remove_dir_all(lib.join(&name));
            return Err(e);
        }
        ("carpeta", name)
    } else if ext == "zip" || ext == "exe" {
        let file = std::fs::File::open(&src).map_err(|e| format!("No se pudo abrir el archivo: {e}"))?;
        match zip::ZipArchive::new(file) {
            Ok(mut z) if zip_has_inf(&z) => {
                let name = free_name(&lib, &stem);
                if let Err(e) = extract_zip(&mut z, &lib.join(&name)) {
                    let _ = std::fs::remove_dir_all(lib.join(&name));
                    return Err(e);
                }
                (if ext == "zip" { "zip" } else { "exeAbierto" }, name)
            }
            _ if ext == "zip" => return Err("Ese .zip no se puede abrir o no trae ningún driver (.inf).".into()),
            _ => {
                if std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0) > MAX_COPY_BYTES {
                    return Err("El instalador es demasiado grande para la biblioteca.".into());
                }
                let dir = lib.join(INSTALLERS_DIR);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let file_name = src.file_name().map(|n| safe_name(&n.to_string_lossy())).unwrap_or_else(|| "instalador.exe".into());
                let name = free_name(&dir, &file_name);
                if let Err(e) = std::fs::copy(&src, dir.join(&name)) {
                    let _ = std::fs::remove_file(dir.join(&name));
                    return Err(format!("No se pudo copiar el instalador: {e}"));
                }
                ("instalador", name)
            }
        }
    } else {
        return Err("Solo se pueden añadir carpetas, archivos .zip o instaladores .exe.".into());
    };

    let idx = load_index(&app, true);
    let drivers_found = idx.drivers.iter().filter(|d| d.driver.package == name).count();
    let message = match kind {
        "instalador" => "Guardado como instalador del fabricante: no se puede abrir para leer sus drivers, así que no se compara con lo instalado.".to_string(),
        _ if drivers_found == 0 => "Añadido, pero no se encontró ningún driver de impresora o escáner de 64 bits dentro.".to_string(),
        _ => format!("Añadido: {drivers_found} driver(s) de impresora o escáner."),
    };
    Ok(AddResult { kind: kind.into(), name, drivers_found, message })
}

// ---------- Windows Update ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct WuUpdate {
    update_id: String,
    title: String,
    hardware_id: String,
    model: String,
    manufacturer: String,
    provider: String,
    class: String,
    date: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct WuCache {
    /// Segundos desde 1970 de la última búsqueda (0: nunca).
    searched_at: u64,
    updates: Vec<WuUpdate>,
}

const WU_SEARCH_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$session = New-Object -ComObject Microsoft.Update.Session
$searcher = $session.CreateUpdateSearcher()
$searcher.ServerSelection = 2
$result = $searcher.Search("IsInstalled=0 and Type='Driver'")
$list = @()
foreach ($u in $result.Updates) {
  $date = ''
  try { $date = ([datetime]$u.DriverVerDate).ToString('yyyy-MM-dd') } catch {}
  $list += [pscustomobject]@{
    updateId = "$($u.Identity.UpdateID)"; title = "$($u.Title)"; hardwareId = "$($u.DriverHardwareID)"; model = "$($u.DriverModel)"
    manufacturer = "$($u.DriverManufacturer)"; provider = "$($u.DriverProvider)"; class = "$($u.DriverClass)"; date = $date; size = [uint64]$u.MaxDownloadSize
  }
}
ConvertTo-Json -InputObject @($list) -Depth 3 -Compress
"#;

const WU_INSTALL_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
if ($id -notmatch '^[0-9a-fA-F-]{36}$') { throw 'Identificador de actualización no válido.' }
$session = New-Object -ComObject Microsoft.Update.Session
$searcher = $session.CreateUpdateSearcher()
$searcher.ServerSelection = 2
$result = $searcher.Search("UpdateID='$id' and IsInstalled=0")
if ($result.Updates.Count -eq 0) { throw 'Windows Update ya no ofrece ese driver. Vuelve a buscar.' }
$col = New-Object -ComObject Microsoft.Update.UpdateColl
foreach ($u in $result.Updates) { if (-not $u.EulaAccepted) { $u.AcceptEula() }; [void]$col.Add($u) }
$dl = $session.CreateUpdateDownloader()
$dl.Updates = $col
$r = $dl.Download()
if ($r.ResultCode -ne 2 -and $r.ResultCode -ne 3) { throw "La descarga desde Windows Update falló (código $($r.ResultCode))." }
$ins = $session.CreateUpdateInstaller()
$ins.Updates = $col
$r = $ins.Install()
if ($r.ResultCode -ne 2 -and $r.ResultCode -ne 3) { throw "La instalación desde Windows Update falló (código $($r.ResultCode))." }
if ($r.RebootRequired) { 'reboot' } else { 'ok' }
"#;

fn wu_cache_path(app: &AppHandle) -> PathBuf {
    machine_dir(app).join("wu-cache.json")
}

fn read_wu_cache(app: &AppHandle) -> WuCache {
    crate::paths::read_json(&wu_cache_path(app))
}

static WU_BUSY: AtomicBool = AtomicBool::new(false);

struct BusyGuard;

impl Drop for BusyGuard {
    fn drop(&mut self) {
        WU_BUSY.store(false, AtomicOrdering::SeqCst);
    }
}

/// Busca drivers pendientes en Windows Update (tarda; en un proceso propio y cancelable,
/// con límite de 3 minutos) y guarda el resultado con su hora.
fn wu_search(app: &AppHandle) -> Result<WuCache, String> {
    if WU_BUSY.swap(true, AtomicOrdering::SeqCst) {
        return Err("Ya hay una búsqueda en Windows Update en marcha.".into());
    }
    let _guard = BusyGuard;
    let task = Task::new(app, "printvendor-wu").named("Buscar drivers en Windows Update");
    task.step("Buscando drivers en Windows Update (puede tardar un par de minutos)…");
    let out = crate::ps::powershell_opts(WU_SEARCH_SCRIPT, task.opts(Some(Duration::from_secs(180)))).map_err(|e| format!("No se pudo buscar en Windows Update: {e}"))?;
    let updates: Vec<WuUpdate> = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada de Windows Update: {e}"))?;
    let cache = WuCache { searched_at: now_epoch(), updates };
    if let Err(e) = crate::paths::write_json(&wu_cache_path(app), &cache) {
        log::warn!("Windows Update: no se pudo guardar la búsqueda: {e}");
    }
    Ok(cache)
}

fn class_ok(class: &str, kind: &str) -> bool {
    let c = class.to_ascii_lowercase();
    match kind {
        "scanner" => c == "image" || c == "scanner",
        "printer" => c == "printer" || c == "printqueue",
        _ => true,
    }
}

fn library_candidates(idx: &LibIndex, dev: &DeviceRef) -> Vec<Candidate> {
    let brand = dev_brand(dev);
    idx.drivers
        .iter()
        .filter_map(|d| {
            if !class_ok(&d.driver.class, &dev.kind) {
                return None;
            }
            let provider_brand = brand_key(&d.driver.provider);
            if let (Some(b), Some(pb)) = (brand.as_deref(), provider_brand) {
                if b != pb {
                    return None;
                }
            }
            let (model, by) = if let Some(e) = d.entries.iter().find(|e| e.hardware_ids.iter().any(|h| hw_match(&dev.hardware_ids, h))) {
                (e.name.clone(), "hardwareId")
            } else {
                let names: Vec<String> = d.entries.iter().map(|e| e.name.clone()).collect();
                let (m, by) = pick_model(&names, &dev.model, &dev.driver_name)?;
                // El universal solo vale si se sabe de qué marca es el aparato y el driver.
                if by == "universal" && (brand.is_none() || provider_brand.is_none()) {
                    return None;
                }
                (m, by)
            };
            Some(Candidate {
                id: format!("lib:{}|{model}", d.driver.id),
                source: "biblioteca".into(),
                title: d.driver.package.clone(),
                version: d.driver.version.clone(),
                date: d.driver.date.clone(),
                model,
                matched_by: by.into(),
                package: d.driver.package.clone(),
                ..Default::default()
            })
        })
        .collect()
}

fn wu_candidates(cache: &WuCache, dev: &DeviceRef) -> Vec<Candidate> {
    let brand = dev_brand(dev);
    cache
        .updates
        .iter()
        .filter_map(|u| {
            if !u.class.is_empty() && !class_ok(&u.class, &dev.kind) {
                return None;
            }
            let update_brand = brand_key(&u.manufacturer).or_else(|| brand_key(&u.provider)).or_else(|| brand_key(&u.title));
            if let (Some(b), Some(ub)) = (brand.as_deref(), update_brand) {
                if b != ub {
                    return None;
                }
            }
            let by = if hw_match(&dev.hardware_ids, &u.hardware_id) {
                "hardwareId"
            } else if !dev.model.is_empty() && !u.model.is_empty() && same_model(&u.model, &dev.model) {
                "modelo"
            } else {
                return None;
            };
            let version = extract_version(&u.title).unwrap_or_default();
            let model = if u.model.is_empty() { dev.driver_name.clone() } else { u.model.clone() };
            Some(Candidate {
                id: format!("wu:{}|{model}", u.update_id),
                source: "windowsUpdate".into(),
                title: u.title.clone(),
                version,
                date: u.date.clone(),
                model,
                matched_by: by.into(),
                ..Default::default()
            })
        })
        .collect()
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WuInfo {
    /// Se buscó ahora en Windows Update.
    pub searched: bool,
    /// Se usó la última búsqueda guardada.
    pub from_cache: bool,
    /// Hora de la búsqueda usada (segundos desde 1970).
    pub searched_at: Option<u64>,
    /// Por qué falló la búsqueda, en español (la biblioteca se usa igualmente).
    pub error: Option<String>,
    /// Drivers pendientes que ofrece Windows Update en total (de cualquier equipo conectado).
    pub pending_total: usize,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Candidates {
    pub candidates: Vec<Candidate>,
    pub state: DriverState,
    pub windows_update: WuInfo,
}

/// Lo que hay para ese aparato en la biblioteca y en Windows Update, y su estado. La
/// búsqueda en Windows Update tarda (hasta 3 minutos), por eso solo se hace con
/// `search_update = true`; si no, se usa la última guardada.
#[tauri::command(async)]
pub fn driver_candidates(app: AppHandle, device: DeviceRef, search_update: Option<bool>) -> Result<Candidates, String> {
    let idx = load_index(&app, false);
    let mut searched = false;
    let mut error = None;
    let cache = if search_update.unwrap_or(false) {
        match wu_search(&app) {
            Ok(c) => {
                searched = true;
                c
            }
            Err(e) => {
                error = Some(e);
                read_wu_cache(&app)
            }
        }
    } else {
        read_wu_cache(&app)
    };
    let mut cands = library_candidates(&idx, &device);
    cands.extend(wu_candidates(&cache, &device));
    let cands = finish_candidates(cands, &device);
    let searched_at = (cache.searched_at > 0).then_some(cache.searched_at);
    let state = state_from(&device, &cands, searched_at);
    Ok(Candidates {
        candidates: cands,
        state,
        windows_update: WuInfo { searched, from_cache: !searched && searched_at.is_some(), searched_at, error, pending_total: cache.updates.len() },
    })
}

// ---------- Actualizar el driver y deshacer ----------

type StepTable = &'static [(&'static str, &'static str)];

const STEPS: StepTable = &[
    ("backup", "Copia del driver actual"),
    ("install", "Instalar el driver nuevo"),
    ("driver", "Registrar el driver en Windows"),
    ("assign", "Asignarlo a la impresora"),
    ("test", "Página de prueba"),
];

/// Progreso de la actualización (evento `printvendor:step`).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct StepEvent<'a> {
    printer: &'a str,
    /// "backup" | "install" | "driver" | "assign" | "test"
    step: &'a str,
    /// 1 a 5
    index: u8,
    total: u8,
    /// "running" | "ok" | "skipped" | "error"
    state: &'a str,
    message: &'a str,
}

const INSTALL_STEPS: StepTable = &[("install", "Instalar el driver"), ("driver", "Registrar el driver en Windows"), ("queue", "Comprobar que aparece la impresora")];

const INSTALLER_STEPS: StepTable = &[("run", "Ejecutar el instalador del fabricante")];

struct Steps<'a> {
    app: &'a AppHandle,
    printer: &'a str,
    table: StepTable,
}

impl Steps<'_> {
    fn emit(&self, index: u8, state: &str, message: &str) {
        let (step, _) = self.table[usize::from(index) - 1];
        let message = crate::escalate::scrub(message);
        let _ = self.app.emit("printvendor:step", StepEvent { printer: self.printer, step, index, total: self.table.len() as u8, state, message: &message });
    }

    /// Ejecuta un paso avisando de cuándo empieza y cómo acaba. El cierre devuelve el
    /// valor, si el paso se saltó y el mensaje.
    fn run<T>(&self, index: u8, f: impl FnOnce() -> Result<(T, bool, String), String>) -> Result<T, String> {
        self.emit(index, "running", self.table[usize::from(index) - 1].1);
        match f() {
            Ok((v, skipped, msg)) => {
                self.emit(index, if skipped { "skipped" } else { "ok" }, &msg);
                Ok(v)
            }
            Err(e) => {
                self.emit(index, "error", &e);
                Err(e)
            }
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CurrentConfig {
    driver: String,
    port: String,
    shared: bool,
    is_default: bool,
    connection: bool,
    inf: String,
    version: u64,
}

const CURRENT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$p = Get-Printer | Where-Object { $_.Name -eq $name } | Select-Object -First 1
if (-not $p) { throw 'La impresora ya no existe. Actualiza la lista.' }
$w = Get-CimInstance Win32_Printer | Where-Object { $_.Name -eq $name } | Select-Object -First 1
$d = @(Get-PrinterDriver -ErrorAction SilentlyContinue | Where-Object { $_.Name -eq $p.DriverName })
$x = $d | Where-Object { $_.PrinterEnvironment -like '*x64*' } | Select-Object -First 1
if (-not $x) { $x = $d | Select-Object -First 1 }
[pscustomobject]@{ driver = "$($p.DriverName)"; port = "$($p.PortName)"; shared = [bool]$p.Shared; isDefault = [bool]$w.Default; connection = ("$($p.Type)" -eq 'Connection'); inf = "$($x.InfPath)"; version = [uint64]$x.DriverVersion } | ConvertTo-Json -Compress
"#;

/// Nombre publicado (oemNN.inf) del driver cuyo inf original es `$inf`; vacío si es de Windows.
const PUBLISHED_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$m = Get-WindowsDriver -Online | Where-Object { $_.OriginalFileName -eq $inf } | Select-Object -First 1
if ($m) { "$($m.Driver)" } else { '' }
"#;

const REGISTER_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$done = $false
if ($infPath) { try { Add-PrinterDriver -Name $model -InfPath $infPath; $done = $true } catch { $first = $_ } }
if (-not $done) { Add-PrinterDriver -Name $model }
$d = @(Get-PrinterDriver -ErrorAction SilentlyContinue | Where-Object { $_.Name -eq $model })
$x = $d | Where-Object { $_.PrinterEnvironment -like '*x64*' } | Select-Object -First 1
if (-not $x) { $x = $d | Select-Object -First 1 }
if (-not $x) { throw "Windows no registró el driver «$model»." }
"$([uint64]$x.DriverVersion)"
"#;

/// Cambia el driver conservando nombre, puerto, compartida y predeterminada (y lo comprueba).
const ASSIGN_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function Get-Exact { Get-Printer | Where-Object { $_.Name -eq $name } | Select-Object -First 1 }
$p = Get-Exact
if (-not $p) { throw 'La impresora ya no existe. Actualiza la lista.' }
$w = Get-CimInstance Win32_Printer | Where-Object { $_.Name -eq $name } | Select-Object -First 1
$wasDefault = [bool]$w.Default
$port = $p.PortName
$shared = [bool]$p.Shared
$share = $p.ShareName
$p | Set-Printer -DriverName $model
$after = Get-Exact
if ($after.PortName -ne $port) { $after | Set-Printer -PortName $port }
if ($shared -ne [bool]$after.Shared) {
  if ($shared) { $after | Set-Printer -Shared $true -ShareName $share } else { $after | Set-Printer -Shared $false }
}
if ($wasDefault) {
  $w2 = Get-CimInstance Win32_Printer | Where-Object { $_.Name -eq $name } | Select-Object -First 1
  if (-not $w2.Default) { [void](Invoke-CimMethod -InputObject $w2 -MethodName SetDefaultPrinter) }
}
$final = Get-Exact
if ($final.DriverName -ne $model) { throw "Windows no cambió el driver de la impresora (sigue con «$($final.DriverName)»)." }
"$($final.DriverName)"
"#;

const TEST_PAGE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$p = Get-CimInstance Win32_Printer | Where-Object { $_.Name -eq $name } | Select-Object -First 1
if (-not $p) { throw 'La impresora ya no existe.' }
$r = Invoke-CimMethod -InputObject $p -MethodName PrintTestPage
if ($r.ReturnValue -ne 0) { throw "Windows no pudo enviar la página de prueba (código $($r.ReturnValue))." }
'ok'
"#;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct UndoRecord {
    id: String,
    printer: String,
    previous_driver: String,
    new_driver: String,
    /// Cómo estaba la impresora antes del cambio.
    port: String,
    shared: bool,
    is_default: bool,
    created: u64,
    done: bool,
    /// Nombre de la carpeta con la copia del driver anterior (vacío si era de Windows).
    backup: String,
}

fn undo_path(app: &AppHandle) -> PathBuf {
    machine_dir(app).join("undo.json")
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct UndoFile {
    records: Vec<UndoRecord>,
}

fn save_undo(app: &AppHandle, record: UndoRecord) -> Result<(), String> {
    let mut file: UndoFile = crate::paths::read_json(&undo_path(app));
    file.records.push(record);
    let keep = file.records.len().saturating_sub(100);
    file.records.drain(..keep);
    crate::paths::write_json(&undo_path(app), &file)
}

/// Lo que hay que instalar, ya resuelto desde el id del candidato.
enum Plan {
    Library { inf: PathBuf, model: String },
    Update { update_id: String, model: String },
}

impl Plan {
    fn model(&self) -> &str {
        match self {
            Plan::Library { model, .. } | Plan::Update { model, .. } => model,
        }
    }
}

/// Quita el prefijo `\\?\` que pone `canonicalize` (pnputil y PowerShell lo llevan peor).
fn plain_path(p: PathBuf) -> PathBuf {
    let stripped = p.to_str().and_then(|s| s.strip_prefix(r"\\?\")).filter(|r| !r.starts_with("UNC\\")).map(PathBuf::from);
    stripped.unwrap_or(p)
}

/// Resuelve `rel` (ruta relativa con «/») dentro de la biblioteca, siguiendo enlaces y
/// «..», y exige que el resultado siga dentro de ella. `None` si se sale o no existe.
fn inside_library(lib: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.contains(':') || rel.split('/').any(|p| p.is_empty() || p == "..") {
        return None;
    }
    let lib = std::fs::canonicalize(lib).ok()?;
    let full = std::fs::canonicalize(lib.join(rel.replace('/', "\\"))).ok()?;
    full.starts_with(&lib).then(|| plain_path(full))
}

fn resolve_candidate(app: &AppHandle, candidate: &str) -> Result<Plan, String> {
    let (kind_and_id, model) = candidate.split_once('|').ok_or("Candidato no válido.")?;
    let (kind, id) = kind_and_id.split_once(':').ok_or("Candidato no válido.")?;
    if model.trim().is_empty() || model.len() > 300 {
        return Err("Candidato no válido.".into());
    }
    match kind {
        "lib" => {
            let idx = load_index(app, false);
            let d = idx.drivers.iter().find(|d| d.driver.id == id).ok_or("Ese paquete ya no está en la biblioteca. Vuelve a buscar.")?;
            let rel = &d.driver.rel_path;
            if !rel.to_ascii_lowercase().ends_with(".inf") {
                return Err("Candidato no válido.".into());
            }
            if !library_dir(app).join(rel.replace('/', "\\")).is_file() {
                return Err("El paquete ya no está en la biblioteca (¿el pendrive está conectado?).".into());
            }
            let inf = inside_library(&library_dir(app), rel).filter(|p| p.is_file()).ok_or("Candidato no válido.")?;
            Ok(Plan::Library { inf, model: model.to_string() })
        }
        "wu" => {
            if id.len() != 36 || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                return Err("Candidato no válido.".into());
            }
            Ok(Plan::Update { update_id: id.to_string(), model: model.to_string() })
        }
        _ => Err("Candidato no válido.".into()),
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    /// Para `printer_driver_undo`. Siempre es un cambio guardado: si no se pudo guardar cómo deshacerlo, no se cambia nada.
    pub undo_id: String,
    pub printer: String,
    pub previous_driver: String,
    pub previous_version: String,
    pub new_driver: String,
    pub new_version: String,
    /// El driver o su versión cambiaron de verdad.
    pub changed: bool,
    /// Windows aceptó la página de prueba (que salga bien en papel lo dice el técnico).
    pub test_page_sent: bool,
    /// Windows pide reiniciar para terminar.
    pub reboot_required: bool,
    pub message: String,
}

/// Copia el driver actual con `pnputil /export-driver`. `None`: es de Windows y no hace falta.
fn backup_current(app: &AppHandle, task: &Task, printer: &str, inf: &str) -> Result<Option<PathBuf>, String> {
    if inf.trim().is_empty() {
        return Ok(None);
    }
    let script = format!("{}{PUBLISHED_SCRIPT}", text_var("inf", inf));
    let published = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(180)))).map_err(|e| format!("No se pudo localizar el driver actual: {e}"))?;
    let published = published.trim();
    if published.is_empty() {
        return Ok(None);
    }
    let digits = published.strip_prefix("oem").and_then(|r| r.strip_suffix(".inf")).filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
    if digits.is_none() {
        return Err("Windows dio un nombre de driver inesperado.".into());
    }
    let dest = crate::paths::drivers_backup_dir(app).join(format!("Impresora_{}_{}", safe_name(printer), chrono::Local::now().format("%Y-%m-%d_%H%M")));
    std::fs::create_dir_all(&dest).map_err(|e| format!("No se pudo crear la carpeta de la copia: {e}"))?;
    crate::ps::exec_opts("pnputil.exe", &["/export-driver", published, &dest.display().to_string()], task.opts(Some(Duration::from_secs(300)))).map_err(|e| {
        let _ = std::fs::remove_dir_all(&dest);
        format!("No se pudo copiar el driver actual: {e}")
    })?;
    Ok(Some(dest))
}

fn assign_driver(task: Option<&Task>, name: &str, model: &str) -> Result<String, String> {
    let script = format!("{}{}{ASSIGN_SCRIPT}", text_var("name", name), text_var("model", model));
    match task {
        Some(t) => crate::ps::powershell_opts(&script, t.opts(Some(Duration::from_secs(120)))),
        None => run_ps(&script, 120),
    }
}

/// Mete el driver del plan en el almacén de Windows. Devuelve si pide reiniciar y el mensaje.
fn install_plan(task: &Task, plan: &Plan) -> Result<(bool, bool, String), String> {
    match plan {
        Plan::Library { inf, .. } => {
            let r = crate::ps::exec_opts("pnputil.exe", &["/add-driver", &inf.display().to_string(), "/install"], task.opts(Some(Duration::from_secs(600))));
            match r {
                Ok(_) => Ok((false, false, "Driver añadido al almacén de Windows.".to_string())),
                // 3010: instalado, pide reiniciar.
                Err(e) if e.contains("3010") => Ok((true, false, "Driver añadido; Windows pide reiniciar para terminar.".to_string())),
                Err(e) => Err(format!("No se pudo instalar el driver: {e}")),
            }
        }
        Plan::Update { update_id, .. } => {
            let script = format!("{}{WU_INSTALL_SCRIPT}", text_var("id", update_id));
            let out = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(15 * 60)))).map_err(|e| format!("No se pudo instalar desde Windows Update: {e}"))?;
            Ok((out.trim() == "reboot", false, "Driver instalado desde Windows Update.".to_string()))
        }
    }
}

/// Registra el driver del plan como driver de impresora y devuelve su versión empaquetada.
fn register_driver(task: &Task, plan: &Plan) -> Result<u64, String> {
    let inf = if let Plan::Library { inf, .. } = plan { inf.display().to_string() } else { String::new() };
    let script = format!("{}{}{REGISTER_SCRIPT}", text_var("model", plan.model()), text_var("infPath", &inf));
    let out = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(180)))).map_err(|e| format!("No se pudo registrar el driver: {e}"))?;
    Ok(out.trim().lines().last().and_then(|l| l.trim().parse().ok()).unwrap_or(0))
}

fn mark_undo_done(app: &AppHandle, id: &str) {
    let mut file: UndoFile = crate::paths::read_json(&undo_path(app));
    if let Some(r) = file.records.iter_mut().find(|r| r.id == id) {
        r.done = true;
    }
    let _ = crate::paths::write_json(&undo_path(app), &file);
}

fn do_update(app: &AppHandle, printer: &str, candidate: &str) -> Result<UpdateResult, String> {
    valid_name(printer)?;
    let plan = resolve_candidate(app, candidate)?;
    let task = Task::new(app, "printvendor-update").named("Actualizar el driver de la impresora");
    let steps = Steps { app, printer, table: STEPS };

    let cur_out = crate::ps::powershell_opts(&format!("{}{CURRENT_SCRIPT}", text_var("name", printer)), task.opts(Some(Duration::from_secs(60))))
        .map_err(|e| format!("No se pudo leer la impresora «{printer}»: {e}"))?;
    let cur: CurrentConfig = serde_json::from_str(cur_out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    if cur.connection {
        return Err("Es una impresora conectada desde un servidor: el driver se actualiza en el servidor, no aquí.".into());
    }
    let (prev_driver, prev_version) = (cur.driver.clone(), version_text(cur.version));

    // 1. Copia del driver actual.
    let backup = steps.run(1, || {
        let b = backup_current(app, &task, printer, &cur.inf)?;
        let msg = if b.is_some() { "Copia guardada en la carpeta de copias de drivers.".to_string() } else { "El driver actual viene con Windows: no necesita copia.".to_string() };
        Ok((b.clone(), b.is_none(), msg))
    })?;

    // 2. Instalar.
    let reboot = steps.run(2, || install_plan(&task, &plan))?;

    // 3. Registrar el driver de impresora.
    let model = plan.model().to_string();
    let new_packed = steps.run(3, || Ok((register_driver(&task, &plan)?, false, format!("Driver «{model}» registrado."))))?;

    // 4. Guardar cómo deshacer y, solo entonces, asignar el driver. Si falla, se intenta volver al anterior.
    let undo_id = format!("{}-{}", now_epoch(), short_id(&format!("{printer}{model}")).get(..6).unwrap_or("x"));
    let record = UndoRecord {
        id: undo_id.clone(),
        printer: printer.to_string(),
        previous_driver: prev_driver.clone(),
        new_driver: model.clone(),
        port: cur.port.clone(),
        shared: cur.shared,
        is_default: cur.is_default,
        created: now_epoch(),
        done: false,
        backup: backup.as_ref().and_then(|b| b.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
    };
    steps.run(4, || {
        save_undo(app, record).map_err(|e| format!("No se pudo guardar cómo deshacer el cambio, así que no se ha cambiado nada: {e}"))?;
        match assign_driver(Some(&task), printer, &model) {
            Ok(_) => Ok(((), false, "Driver cambiado; nombre, puerto, compartida y predeterminada se conservan.".to_string())),
            Err(e) => {
                let tail = if prev_driver.is_empty() {
                    String::new()
                } else {
                    match assign_driver(Some(&task), printer, &prev_driver) {
                        Ok(_) => {
                            mark_undo_done(app, &undo_id);
                            format!(" Se volvió al driver anterior («{prev_driver}»).")
                        }
                        Err(e2) => format!(" Además no se pudo volver al driver anterior ({e2}): usa «Deshacer»."),
                    }
                };
                Err(format!("No se pudo cambiar el driver de la impresora: {e}{tail}"))
            }
        }
    })?;

    // 5. Página de prueba (si falla, el driver ya está cambiado: se avisa y se puede deshacer).
    let test_page_sent = steps
        .run(5, || {
            let script = format!("{}{TEST_PAGE_SCRIPT}", text_var("name", printer));
            crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(60)))).map(|_| (true, false, "Página de prueba enviada a la impresora.".to_string()))
        })
        .unwrap_or(false);

    let new_version = version_text(new_packed);
    let changed = !prev_driver.eq_ignore_ascii_case(&model) || new_packed != cur.version;
    let message = if changed {
        format!("«{printer}» ahora usa «{model}»{}.", if new_version.is_empty() { String::new() } else { format!(" ({new_version})") })
    } else {
        "El driver es el mismo y Windows conservó la misma versión: no había nada nuevo que aplicar.".to_string()
    };
    Ok(UpdateResult {
        undo_id,
        printer: printer.to_string(),
        previous_driver: prev_driver,
        previous_version: prev_version,
        new_driver: model,
        new_version,
        changed,
        test_page_sent,
        reboot_required: reboot,
        message,
    })
}

/// Actualiza el driver de una impresora con el candidato elegido (el `id` que dio
/// `driver_candidates`): copia del actual, instalar, registrar, asignar conservando
/// nombre, puerto, compartida y predeterminada, y página de prueba. Emite
/// `printvendor:step` en cada paso. Requiere administrador.
#[tauri::command(async)]
pub fn printer_driver_update(app: AppHandle, printer: String, candidate: String, tweaks: State<'_, TweakState>) -> Result<UpdateResult, String> {
    let result = if crate::elevation::is_elevated() { do_update(&app, &printer, &candidate) } else { Err(NEED_ADMIN.to_string()) };
    let result = result.map_err(|e| crate::escalate::scrub(&e));
    tweaks.record(Op::Run, &format!("Actualizar el driver de la impresora: {printer}"), &result);
    result
}

fn do_undo(app: &AppHandle, id: &str) -> Result<String, String> {
    let file: UndoFile = crate::paths::read_json(&undo_path(app));
    let rec = file.records.iter().find(|r| r.id == id).cloned().ok_or("Ese cambio ya no se puede deshacer.")?;
    if rec.done {
        return Err("Ese cambio ya se deshizo.".into());
    }
    let check = format!(
        "{}{}$ErrorActionPreference = 'Stop'\nif (-not (Get-Printer | Where-Object {{ $_.Name -eq $name }})) {{ throw 'La impresora ya no existe.' }}\nif (-not (Get-PrinterDriver | Where-Object {{ $_.Name -eq $model }})) {{ throw 'El driver anterior ya no está instalado en Windows.' }}\n'ok'",
        text_var("name", &rec.printer),
        text_var("model", &rec.previous_driver)
    );
    run_ps(&check, 60).map_err(|e| {
        if rec.backup.is_empty() {
            e
        } else {
            format!("{e} Hay una copia en la carpeta de copias de drivers («{}»): se puede restaurar desde Mantenimiento.", rec.backup)
        }
    })?;
    assign_driver(None, &rec.printer, &rec.previous_driver)?;
    mark_undo_done(app, id);
    Ok(format!("«{}» vuelve a usar «{}».", rec.printer, rec.previous_driver))
}

/// Vuelve al driver anterior de una actualización (`undo_id`). El driver viejo no se
/// desinstala al actualizar, así que deshacer es inmediato. Requiere administrador.
#[tauri::command(async)]
pub fn printer_driver_undo(app: AppHandle, id: String, tweaks: State<'_, TweakState>) -> Result<String, String> {
    let result = if crate::elevation::is_elevated() { do_undo(&app, &id) } else { Err(NEED_ADMIN.to_string()) };
    let result = result.map_err(|e| crate::escalate::scrub(&e));
    tweaks.record(Op::Run, "Deshacer la actualización del driver de una impresora", &result);
    result
}

// ---------- Instalar una impresora conectada y sin instalar ----------

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct PrinterSnapshot {
    #[serde(deserialize_with = "lenient_vec")]
    names: Vec<String>,
    #[serde(deserialize_with = "lenient_vec")]
    used_ports: Vec<String>,
    #[serde(deserialize_with = "lenient_vec")]
    usb_ports: Vec<String>,
}

const SNAPSHOT_SCRIPT: &str = r#"
$names = @(Get-CimInstance Win32_Printer -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Name)" })
$used = @(Get-CimInstance Win32_Printer -ErrorAction SilentlyContinue | ForEach-Object { "$($_.PortName)" })
$usb = @(Get-PrinterPort -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^USB\d+$' } | ForEach-Object { "$($_.Name)" })
ConvertTo-Json -InputObject ([pscustomobject]@{ names = $names; usedPorts = $used; usbPorts = $usb }) -Compress
"#;

const CREATE_QUEUE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Printer -Name $qname -DriverName $model -PortName $port
"$qname"
"#;

fn printer_snapshot() -> Result<PrinterSnapshot, String> {
    let out = run_ps(SNAPSHOT_SCRIPT, 60)?;
    serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))
}

/// Un nombre de cola que no choque con las que ya hay («X», «X (2)»…).
fn free_queue_name(existing: &[String], base: &str) -> String {
    let taken = |n: &str| existing.iter().any(|e| e.eq_ignore_ascii_case(n));
    if !taken(base) {
        return base.to_string();
    }
    (2..1000).map(|n| format!("{base} ({n})")).find(|c| !taken(c)).unwrap_or_else(|| format!("{base} ({})", now_epoch()))
}

/// El puerto USB libre que corresponde al aparato: debe haber exactamente uno sin impresora.
fn pick_usb_port(usb: &[String], used: &[String]) -> Result<String, String> {
    let free: Vec<&String> = usb.iter().filter(|p| !used.iter().any(|u| u.eq_ignore_ascii_case(p))).collect();
    match free.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err("Windows no tiene ningún puerto USB libre para este aparato. Desconecta el cable, vuelve a conectarlo y reinténtalo.".into()),
        _ => Err("Hay varios puertos USB libres y no se sabe cuál es de este aparato. Desconecta los demás aparatos USB de impresión y reinténtalo.".into()),
    }
}

/// Nombre para la cola nueva: el del aparato si dice la marca; si no, «MARCA modelo».
fn queue_base_name(dev: &DeviceRef) -> String {
    let name = dev.name.trim();
    if brand_key(name).is_some() && !name.is_empty() {
        return name.to_string();
    }
    let vendor = match dev.vendor.as_str() {
        "ricoh" => "RICOH",
        "epson" => "EPSON",
        _ => "Impresora",
    };
    if dev.model.trim().is_empty() {
        format!("{vendor} USB")
    } else {
        format!("{vendor} {}", dev.model.trim())
    }
}

fn is_usb_device(dev: &DeviceRef) -> bool {
    dev.hardware_ids.iter().any(|h| h.to_ascii_uppercase().starts_with("USB"))
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// Nombre de la impresora que quedó instalada.
    pub printer: String,
    /// La cola la creó AdminOps (Windows no la creó solo).
    pub created_queue: bool,
    pub driver: String,
    pub reboot_required: bool,
    pub message: String,
}

fn do_install(app: &AppHandle, dev: &DeviceRef, candidate: &str) -> Result<InstallResult, String> {
    if dev.name.trim().is_empty() || dev.name.len() > 300 {
        return Err("Aparato no válido.".into());
    }
    let plan = resolve_candidate(app, candidate)?;
    let task = Task::new(app, "printvendor-install").named("Instalar una impresora");
    let steps = Steps { app, printer: &dev.name, table: INSTALL_STEPS };
    let before = printer_snapshot()?;
    let model = plan.model().to_string();

    let reboot = steps.run(1, || install_plan(&task, &plan))?;

    let register = steps.run(2, || {
        Ok(match register_driver(&task, &plan) {
            Ok(_) => (None, false, format!("Driver «{model}» registrado.")),
            Err(e) => (Some(e.clone()), true, format!("No se pudo registrar ahora ({e}); si Windows crea la impresora sola, no hace falta.")),
        })
    })?;

    let (printer, created) = steps.run(3, || {
        let mut found: Option<String> = None;
        for i in 0..8 {
            if i > 0 {
                std::thread::sleep(Duration::from_secs(2));
            }
            let now = printer_snapshot()?;
            let new: Vec<&String> = now.names.iter().filter(|n| !before.names.iter().any(|b| b.eq_ignore_ascii_case(n))).collect();
            found = new.iter().find(|n| brand_key(n).is_some()).or(new.first()).map(|n| (*n).clone());
            if found.is_some() {
                break;
            }
        }
        if let Some(n) = found {
            return Ok(((n.clone(), false), false, format!("Windows creó la impresora «{n}» por sí sola.")));
        }
        if !is_usb_device(dev) {
            return Err("Windows no creó la impresora. Si es de red, añádela desde la búsqueda de impresoras en la red.".into());
        }
        if let Some(e) = &register {
            return Err(format!("No se pudo crear la impresora porque el driver no se registró: {e}"));
        }
        let snap = printer_snapshot()?;
        let port = pick_usb_port(&snap.usb_ports, &snap.used_ports)?;
        let qname = free_queue_name(&snap.names, &queue_base_name(dev));
        let script = format!("{}{}{}{CREATE_QUEUE_SCRIPT}", text_var("qname", &qname), text_var("model", &model), text_var("port", &port));
        crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(90)))).map_err(|e| format!("No se pudo crear la impresora: {e}"))?;
        Ok(((qname.clone(), true), false, format!("Impresora «{qname}» creada en el puerto {port}.")))
    })?;

    Ok(InstallResult {
        message: if created { format!("«{printer}» instalada y lista en el puerto USB.") } else { format!("«{printer}» instalada.") },
        printer,
        created_queue: created,
        driver: model,
        reboot_required: reboot,
    })
}

/// Instala una impresora conectada pero sin instalar (el `device` de `notInstalled`) con el
/// candidato elegido: añade el driver, comprueba que aparezca la cola y, si es USB y Windows
/// no la crea sola, la crea en el puerto USB libre. Emite `printvendor:step`. Requiere administrador.
#[tauri::command(async)]
pub fn printer_install(app: AppHandle, device: DeviceRef, candidate: String, tweaks: State<'_, TweakState>) -> Result<InstallResult, String> {
    let result = if crate::elevation::is_elevated() { do_install(&app, &device, &candidate) } else { Err(NEED_ADMIN.to_string()) };
    let result = result.map_err(|e| crate::escalate::scrub(&e));
    tweaks.record(Op::Run, &format!("Instalar la impresora: {}", device.name), &result);
    result
}

// ---------- Instaladores del fabricante ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallerResult {
    pub name: String,
    /// El instalador terminó (false: sigue abierto pasados 15 minutos).
    pub finished: bool,
    pub exit_code: Option<i32>,
    pub reboot_required: bool,
    pub message: String,
}

const INSTALLER_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$a = @{ FilePath = $path; PassThru = $true }
if ($runas) { $a.Verb = 'RunAs' }
$p = Start-Process @a
if ($p.WaitForExit(900000)) { "$($p.ExitCode)" } else { 'timeout' }
"#;

fn do_run_installer(app: &AppHandle, id: &str) -> Result<InstallerResult, String> {
    let idx = load_index(app, false);
    let inst = idx.installers.iter().find(|i| short_id(&i.rel_path) == id).ok_or("Ese instalador ya no está en la biblioteca.")?;
    if !inst.rel_path.to_ascii_lowercase().ends_with(".exe") {
        return Err("Instalador no válido.".into());
    }
    let path = inside_library(&library_dir(app), &inst.rel_path).filter(|p| p.is_file()).ok_or("El instalador ya no está en la biblioteca (¿el pendrive está conectado?).")?;
    let name = inst.name.clone();
    let task = Task::new(app, "printvendor-installer").named("Instalador del fabricante");
    let steps = Steps { app, printer: &name, table: INSTALLER_STEPS };
    let runas = if crate::elevation::is_elevated() { "$false" } else { "$true" };
    let script = format!("{}$runas = {runas}\n{INSTALLER_SCRIPT}", text_var("path", &path.display().to_string()));
    steps.run(1, || {
        let out = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(16 * 60)))).map_err(|e| {
            if e.to_ascii_lowercase().contains("cancel") {
                "Se canceló el permiso de administrador para abrir el instalador.".to_string()
            } else {
                format!("No se pudo abrir el instalador: {e}")
            }
        })?;
        let out = out.trim();
        let (finished, code) = if out == "timeout" { (false, None) } else { (true, out.parse::<i32>().ok()) };
        let reboot = code == Some(3010);
        let message = match (finished, code) {
            (false, _) => "El instalador sigue abierto: AdminOps deja de esperarlo, pero no lo cierra.".to_string(),
            (true, Some(0)) => "El instalador terminó.".to_string(),
            (true, Some(3010)) => "El instalador terminó; Windows pide reiniciar.".to_string(),
            (true, Some(c)) => format!("El instalador terminó con el código {c}."),
            (true, None) => "El instalador terminó.".to_string(),
        };
        Ok((InstallerResult { name: name.clone(), finished, exit_code: code, reboot_required: reboot, message: message.clone() }, false, message))
    })
}

/// Abre un instalador del fabricante guardado en la biblioteca (el `id` de `Library.installers`),
/// con elevación si AdminOps no la tiene (el técnico ve la ventana), y espera a que termine o
/// 15 minutos como mucho. Solo ids de la biblioteca, nunca rutas.
#[tauri::command(async)]
pub fn run_vendor_installer(app: AppHandle, id: String, tweaks: State<'_, TweakState>) -> Result<InstallerResult, String> {
    let result = do_run_installer(&app, &id).map_err(|e| crate::escalate::scrub(&e));
    tweaks.record(Op::Run, "Ejecutar un instalador del fabricante de impresoras", &result);
    result
}

// ---------- Páginas oficiales y descargas ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VendorPage {
    pub url: String,
    /// El modelo quedó en el portapapeles.
    pub copied: bool,
}

fn copy_text(text: &str) -> Result<(), String> {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let bytes = wide.len() * 2;
    // SAFETY: el bloque se reserva con el tamaño exacto, se copia y se entrega al
    // portapapeles (que pasa a ser su dueño); si no lo acepta, se libera.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if h.is_null() {
            return Err("Sin memoria para el portapapeles.".into());
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return Err("Sin memoria para el portapapeles.".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), p, bytes);
        GlobalUnlock(h);
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !opened {
            GlobalFree(h);
            return Err("El portapapeles está ocupado por otro programa.".into());
        }
        EmptyClipboard();
        let ok = !SetClipboardData(CF_UNICODETEXT, h).is_null();
        CloseClipboard();
        if !ok {
            GlobalFree(h);
            return Err("Windows no aceptó el texto en el portapapeles.".into());
        }
    }
    Ok(())
}

/// Abre la página oficial de descargas (RICOH o EPSON) y copia el modelo al portapapeles
/// para pegarlo en su buscador.
#[tauri::command(async)]
pub fn open_vendor_page(brand: String, model: String) -> Result<VendorPage, String> {
    let url = match brand.trim().to_ascii_lowercase().as_str() {
        "ricoh" => "https://www.ricoh-usa.com/en/support-and-download",
        "epson" => "https://epson.com/Support/sl/s",
        _ => return Err("Solo hay página oficial para RICOH y EPSON.".into()),
    };
    let model: String = model.chars().filter(|c| !c.is_control()).take(200).collect();
    let model = model.trim();
    let copied = !model.is_empty() && copy_text(model).is_ok();
    crate::shellopen::open(url)?;
    Ok(VendorPage { url: url.into(), copied })
}

/// La carpeta Descargas del usuario según Windows (puede estar redirigida).
fn downloads_dir() -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Downloads, SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    // SAFETY: la cadena la reserva Windows y se libera con CoTaskMemFree tras copiarla.
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const std::ffi::c_void));
        s.map(PathBuf::from)
    }
}

#[derive(Default, Debug)]
struct PackageInfo {
    readable: bool,
    version: String,
    provider: String,
    vendor: String,
    models: Vec<String>,
}

/// Mira dentro de un `.zip` o `.exe` autoextraíble (si se abre como zip) y lee sus `.inf`.
fn inspect_package(path: &Path) -> PackageInfo {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(path) else { return PackageInfo::default() };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return PackageInfo::default() };
    let mut out = PackageInfo { readable: true, ..Default::default() };
    let mut versions: Vec<String> = Vec::new();
    let mut read = 0;
    for i in 0..zip.len() {
        if read >= 60 {
            break;
        }
        let Ok(f) = zip.by_index(i) else { continue };
        if !f.name().to_ascii_lowercase().ends_with(".inf") || f.size() > 4 * 1024 * 1024 {
            continue;
        }
        read += 1;
        let mut bytes = Vec::new();
        if f.take(4 * 1024 * 1024).read_to_end(&mut bytes).is_err() {
            continue;
        }
        let info = parse_inf(&decode_inf(&bytes));
        if !info.arch_x64 || !matches!(info.class.to_ascii_lowercase().as_str(), "printer" | "printqueue" | "image" | "scanner") {
            continue;
        }
        if out.provider.is_empty() {
            out.provider = info.provider.clone();
        }
        if !info.driver_ver.version.is_empty() {
            versions.push(info.driver_ver.version.clone());
        }
        for m in info.models {
            if out.models.len() < 40 && !out.models.contains(&m) {
                out.models.push(m);
            }
        }
    }
    out.version = best_version(versions.iter().map(String::as_str)).unwrap_or_default().to_string();
    let brand = brand_key(&out.provider).or_else(|| out.models.iter().find_map(|m| brand_key(m)));
    out.vendor = Vendor::from_brand(brand).id().into();
    out
}

/// Un paquete nuevo en Descargas (evento `printvendor:download`).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct DownloadEvent {
    name: String,
    /// Identificador opaco («dl:…») para pasarlo a `driver_library_add`. No hay ruta: lleva el nombre de usuario.
    id: String,
    size: u64,
    /// "exe" | "zip"
    kind: String,
    /// Se pudo abrir como zip y leer sus `.inf`.
    readable: bool,
    version: String,
    provider: String,
    /// "ricoh" | "epson" | "otra"
    vendor: String,
    models: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct WatchEvent {
    /// "started" | "ended" | "stopped"
    state: &'static str,
    minutes: u32,
}

static WATCH_GEN: AtomicU64 = AtomicU64::new(0);

/// Descargas vistas: identificador opaco → ruta (solo en memoria).
static DOWNLOADS: LazyLock<Mutex<HashMap<String, PathBuf>>> = LazyLock::new(Default::default);

fn remember_download(path: &Path) -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let id = format!("dl:{}", short_id(&format!("{}{nanos}", path.display())));
    let mut map = DOWNLOADS.lock().unwrap_or_else(|e| e.into_inner());
    if map.len() >= 50 {
        map.clear();
    }
    map.insert(id.clone(), path.to_path_buf());
    id
}

fn download_path(id: &str) -> Option<PathBuf> {
    DOWNLOADS.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
}

fn is_package_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".exe") || lower.ends_with(".zip")
}

/// Vigila la carpeta Descargas `minutes` minutos (1 a 60) y emite `printvendor:download`
/// por cada `.exe` o `.zip` nuevo que termine de bajarse. Llamarla otra vez reinicia la
/// vigilancia; con 0 se para. Emite `printvendor:watch` al empezar y al acabar.
#[tauri::command(async)]
pub fn watch_downloads(app: AppHandle, minutes: u32) -> Result<(), String> {
    let generation = WATCH_GEN.fetch_add(1, AtomicOrdering::SeqCst) + 1;
    if minutes == 0 {
        let _ = app.emit("printvendor:watch", WatchEvent { state: "stopped", minutes: 0 });
        return Ok(());
    }
    let minutes = minutes.min(60);
    let dir = downloads_dir().ok_or("No se encuentra la carpeta Descargas de este usuario.")?;
    if !dir.is_dir() {
        return Err("La carpeta Descargas no existe.".into());
    }
    let mut seen: HashSet<String> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    let _ = app.emit("printvendor:watch", WatchEvent { state: "started", minutes });
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(u64::from(minutes) * 60);
        let mut pending: HashMap<String, u64> = HashMap::new();
        while Instant::now() < deadline {
            if WATCH_GEN.load(AtomicOrdering::SeqCst) != generation {
                return;
            }
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if seen.contains(&name) || !is_package_name(&name) {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                if !meta.is_file() || meta.len() == 0 {
                    continue;
                }
                let path = e.path();
                // Terminó de bajarse si pesa lo mismo que hace un momento y se puede abrir.
                if pending.get(&name) == Some(&meta.len()) && std::fs::File::open(&path).is_ok() {
                    let info = inspect_package(&path);
                    let kind = if name.to_ascii_lowercase().ends_with(".zip") { "zip" } else { "exe" };
                    let _ = app.emit(
                        "printvendor:download",
                        DownloadEvent {
                            name: name.clone(),
                            id: remember_download(&path),
                            size: meta.len(),
                            kind: kind.into(),
                            readable: info.readable,
                            version: info.version,
                            provider: info.provider,
                            vendor: if info.readable { info.vendor } else { Vendor::from_brand(brand_key(&name)).id().into() },
                            models: info.models,
                        },
                    );
                    seen.insert(name.clone());
                    pending.remove(&name);
                } else {
                    pending.insert(name, meta.len());
                }
            }
            std::thread::sleep(Duration::from_millis(1500));
        }
        if WATCH_GEN.load(AtomicOrdering::SeqCst) == generation {
            let _ = app.emit("printvendor:watch", WatchEvent { state: "ended", minutes });
        }
    });
    Ok(())
}

// ---------- Prueba de escaneo ----------

/// Nombres de los escáneres WIA conectados.
const WIA_LIST_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$dm = New-Object -ComObject WIA.DeviceManager
$names = @(foreach ($d in $dm.DeviceInfos) { if ($d.Type -eq 1) { try { "$($d.Properties.Item('Name').Value)" } catch {} } })
ConvertTo-Json -InputObject @($names) -Compress
"#;

/// Elige el escáner WIA: el de nombre exacto; si no hay, el único que se le parezca;
/// si hay varios (con el mismo nombre o parecidos), error pidiendo elegir.
fn pick_wia_device(names: &[String], want: &str) -> Result<String, String> {
    let want = want.trim().to_lowercase();
    let exact: Vec<&String> = names.iter().filter(|n| n.trim().to_lowercase() == want).collect();
    match exact.as_slice() {
        [one] => return Ok((*one).clone()),
        [_, ..] => return Err("Hay varios escáneres con ese mismo nombre. Desconecta los que no uses y reinténtalo.".into()),
        [] => {}
    }
    let partial: Vec<&String> = names
        .iter()
        .filter(|n| {
            let n = n.trim().to_lowercase();
            !n.is_empty() && !want.is_empty() && (n.contains(&want) || want.contains(&n))
        })
        .collect();
    match partial.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(wia_error("80210015")),
        many => Err(format!("Hay varios escáneres que se parecen a ese ({}). Elige uno de la lista.", many.iter().map(|n| format!("«{n}»")).collect::<Vec<_>>().join(", "))),
    }
}

const SCAN_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
try {
  $dm = New-Object -ComObject WIA.DeviceManager
  $info = $null
  $want = $name.ToLower()
  foreach ($d in $dm.DeviceInfos) {
    if ($d.Type -ne 1) { continue }
    $n = ''
    try { $n = "$($d.Properties.Item('Name').Value)".ToLower() } catch {}
    if ($n -and $n -eq $want) { $info = $d; break }
  }
  if (-not $info) { throw 'WIA:80210015' }
  $dev = $info.Connect()
  $item = $dev.Items.Item(1)
  try { $item.Properties.Item(6147).Value = 100; $item.Properties.Item(6148).Value = 100 } catch {}
  $img = $item.Transfer('{B96B3CAF-0728-11D3-9D7B-0000F81EF32E}')
  $img.SaveFile($path)
  'ok'
} catch {
  $m = "$($_.Exception.Message)"
  if ($m -like 'WIA:*') { throw $m }
  $h = $_.Exception.HResult
  if ($null -eq $h) { $h = 0 }
  throw ('WIA:{0:X8}' -f [int]$h)
}
"#;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanTest {
    /// La imagen escaneada como `data:image/png;base64,…`, para enseñarla.
    pub data_url: String,
    pub bytes: usize,
}

/// Escaneo de prueba por WIA (baja resolución) a un archivo temporal que se borra al leerlo.
/// `device` es el nombre que dio `scanner_inventory`.
#[tauri::command(async)]
pub fn scanner_test(app: AppHandle, device: String) -> Result<ScanTest, String> {
    let device = device.trim();
    if device.is_empty() || device.len() > 300 {
        return Err("Nombre de escáner no válido.".into());
    }
    let scan_error = |e: &str| match e.split("WIA:").nth(1).map(|c| c.trim().chars().take(8).collect::<String>()) {
        Some(c) => wia_error(&c),
        None if e.contains("tardó más") => "El escáner tardó demasiado en responder.".to_string(),
        None => wia_error(""),
    };
    // Primero se mira qué escáneres WIA hay y se elige uno sin ambigüedad.
    let listed = run_ps(WIA_LIST_SCRIPT, 60).map_err(|e| scan_error(&e))?;
    let names: Vec<String> = serde_json::from_str(listed.trim()).map_err(|_| wia_error(""))?;
    let exact = pick_wia_device(&names, device)?;
    let path = std::env::temp_dir().join(format!("adminops-scan-{}-{}.png", std::process::id(), now_epoch()));
    let script = format!("{}{}{SCAN_SCRIPT}", text_var("name", &exact), text_var("path", &path.display().to_string()));
    let task = Task::new(&app, "printvendor-scan").named("Prueba de escaneo");
    task.step("Escaneando una página de prueba…");
    let run = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(120))));
    let bytes = std::fs::read(&path);
    let _ = std::fs::remove_file(&path);
    if let Err(e) = run {
        return Err(scan_error(&e));
    }
    let bytes = bytes.map_err(|_| "El escáner no entregó ninguna imagen.".to_string())?;
    if bytes.is_empty() {
        return Err("El escáner no entregó ninguna imagen.".into());
    }
    Ok(ScanTest { data_url: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)), bytes: bytes.len() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(model: &str, driver: &str, version: &str) -> DeviceRef {
        DeviceRef {
            kind: "printer".into(),
            name: format!("RICOH {model}"),
            vendor: "ricoh".into(),
            brand: "ricoh".into(),
            model: model.into(),
            driver_name: driver.into(),
            driver_version: version.into(),
            driver_manufacturer: "RICOH".into(),
            ..Default::default()
        }
    }

    fn lib_index(models: &[&str], version: &str) -> LibIndex {
        let entries: Vec<InfModel> = models.iter().map(|m| InfModel { name: (*m).into(), hardware_ids: vec![] }).collect();
        let driver = LibDriver {
            id: short_id("paquete/x.inf"),
            package: "paquete".into(),
            class: "Printer".into(),
            provider: "RICOH".into(),
            vendor: "ricoh".into(),
            version: version.into(),
            rel_path: "paquete/x.inf".into(),
            ..Default::default()
        };
        LibIndex { dir: "x".into(), scanned_at: 1, drivers: vec![IndexedDriver { driver, entries }], installers: vec![] }
    }

    #[test]
    fn library_matches_by_model_and_the_state_compares_versions() {
        let idx = lib_index(&["RICOH PCL6 Driver for Universal Print", "RICOH IM C3000 PCL 6"], "4.42.0.0");
        let d = dev("IM C3000", "RICOH IM C3000 PCL 6", "4.30.0.0");
        let cands = finish_candidates(library_candidates(&idx, &d), &d);
        assert_eq!(cands.len(), 1, "{cands:?}");
        assert_eq!((cands[0].matched_by.as_str(), cands[0].model.as_str(), cands[0].newer), ("modelo", "RICOH IM C3000 PCL 6", Some(true)));
        let st = state_from(&d, &cands, None);
        assert_eq!(st.status, Status::Viejo);
        assert_eq!(st.new_version.as_deref(), Some("4.42.0.0"));

        // La misma versión: al día; ninguna: sin comparar, nunca «al día».
        let same = dev("IM C3000", "RICOH IM C3000 PCL 6", "4.42.0.0");
        assert_eq!(state_from(&same, &finish_candidates(library_candidates(&idx, &same), &same), None).status, Status::AlDia);
        assert_eq!(state_from(&same, &[], Some(5)).status, Status::SinComparar);
        assert_eq!(state_from(&same, &[], None).status, Status::SinComparar);
    }

    #[test]
    fn the_universal_driver_is_not_compared_with_a_model_driver() {
        let idx = lib_index(&["RICOH PCL6 Driver for Universal Print"], "4.42.0.0");
        // Un modelo que el inf no trae: sale el universal, pero no se compara con su driver propio.
        let d = dev("IM C4500", "RICOH IM C4500 PCL 6", "3.0.0.0");
        let cands = finish_candidates(library_candidates(&idx, &d), &d);
        assert_eq!((cands.len(), cands[0].matched_by.as_str(), cands[0].comparable), (1, "universal", false));
        assert_eq!(state_from(&d, &cands, None).status, Status::SinComparar);
        // Si ya usa el universal, sí se compara.
        let u = dev("IM C4500", "RICOH PCL6 Driver for Universal Print", "4.0.0.0");
        let cands = finish_candidates(library_candidates(&idx, &u), &u);
        assert_eq!((cands[0].matched_by.as_str(), cands[0].comparable), ("mismoDriver", true));
        assert_eq!(state_from(&u, &cands, None).status, Status::Viejo);
        // Una impresora de otra marca no recibe drivers de RICOH.
        let hp = DeviceRef { kind: "printer".into(), brand: "hp".into(), vendor: "otra".into(), model: "IM C3000".into(), ..Default::default() };
        assert!(library_candidates(&idx, &hp).is_empty());
        // Un escáner no usa drivers de impresora.
        let sc = DeviceRef { kind: "scanner".into(), ..dev("IM C3000", "", "") };
        assert!(library_candidates(&idx, &sc).is_empty());
    }

    #[test]
    fn windows_update_matches_by_hardware_id_or_model() {
        let cache = WuCache {
            searched_at: 10,
            updates: vec![
                WuUpdate { update_id: "11111111-1111-1111-1111-111111111111".into(), title: "RICOH - Printer - 4.42.0.0".into(), hardware_id: "USBPRINT\\RICOHIM_C3000".into(), model: "RICOH IM C3000 PCL 6".into(), manufacturer: "RICOH".into(), class: "Printer".into(), ..Default::default() },
                WuUpdate { update_id: "22222222-2222-2222-2222-222222222222".into(), title: "EPSON - Printer - 1.0.0.0".into(), model: "EPSON ET-2820 Series".into(), manufacturer: "EPSON".into(), class: "Printer".into(), ..Default::default() },
            ],
        };
        let mut d = dev("IM C3000", "RICOH IM C3000 PCL 6", "4.30.0.0");
        let c = wu_candidates(&cache, &d);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].matched_by.as_str(), c[0].version.as_str(), c[0].source.as_str()), ("modelo", "4.42.0.0", "windowsUpdate"));
        d.model = "Otro".into();
        d.hardware_ids = vec!["usbprint\\ricohim_c3000".into()];
        assert_eq!(wu_candidates(&cache, &d)[0].matched_by, "hardwareId");
        d.hardware_ids.clear();
        assert!(wu_candidates(&cache, &d).is_empty());
    }

    #[test]
    fn printers_without_a_driver_or_with_a_wrong_one_are_flagged() {
        let none = dev("IM C3000", "", "");
        assert_eq!(state_from(&none, &[], None).status, Status::Falta);
        let wrong = dev("IM C3000", "EPSON ET-2820 Series", "1.0.0.0");
        assert_eq!(state_from(&wrong, &[], None).status, Status::OtraMarca);
        let generic = dev("IM C3000", "Microsoft IPP Class Driver", "10.0.0.1");
        let st = state_from(&generic, &[], None);
        assert_eq!(st.status, Status::Generico);
        assert!(st.reason.contains("genérico"));
    }

    #[test]
    fn unplugged_devices_are_found_by_vid_or_printer_class_with_problem_28() {
        let p = |name: &str, hw: &str, class: &str, problem: u32| RawPnp { name: name.into(), device_id: format!("USB\\{name}"), hardware_id: vec![hw.into()], class: class.into(), problem, present: None };
        let found = detect_not_installed(&[
            p("EPSON ET-2820 Series", r"USB\VID_04B8&PID_1100", "", 28),
            p("Dispositivo desconocido", r"USB\VID_05CA&PID_0418", "", 28),
            p("HP LaserJet", r"USBPRINT\HPLASERJET", "Printer", 28),
            p("Webcam", r"USB\VID_046D&PID_0825", "Image", 28),
            p("RICOH ya instalada", r"USB\VID_05CA&PID_0001", "Printer", 0),
            p("RICOH desactivada", r"USB\VID_05CA&PID_0002", "", 22),
        ]);
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Dispositivo desconocido", "EPSON ET-2820 Series", "HP LaserJet"], "{names:?}");
        assert_eq!(found[0].usb_id, "05CA:0418");
        assert_eq!(found[1].model, "ET-2820");
        assert_eq!(found[2].vendor, "otra");
    }

    #[test]
    fn candidate_ids_cannot_escape_the_library() {
        assert_eq!(short_id("A/b.inf"), short_id("a/B.inf"), "no distingue mayúsculas");
        assert_eq!(safe_name("a/b:c*d"), "a_b_c_d");
        assert_eq!(safe_name("..."), "paquete");
        assert_eq!(free_name(Path::new("Z:\\no-existe"), "X"), "X");
        assert!(is_package_name("Setup.EXE") && is_package_name("x.zip") && !is_package_name("x.crdownload"));
    }

    #[test]
    fn json_from_powershell_tolerates_null_single_and_list() {
        let a: RawPnp = serde_json::from_str(r#"{"name":"x","hardwareId":"USB\\VID_1","problem":28}"#).unwrap();
        assert_eq!(a.hardware_id, ["USB\\VID_1"]);
        let b: RawPnp = serde_json::from_str(r#"{"hardwareId":null}"#).unwrap();
        assert!(b.hardware_id.is_empty());
        let c: RawPnp = serde_json::from_str(r#"{"hardwareId":["a",null,"b"]}"#).unwrap();
        assert_eq!(c.hardware_id, ["a", "b"]);
    }

    #[test]
    fn library_paths_cannot_escape() {
        let base = std::env::temp_dir().join(format!("adminops-lib-test-{}", std::process::id()));
        let lib = base.join("lib");
        let other = base.join("lib2");
        std::fs::create_dir_all(lib.join("pkg")).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(lib.join("pkg").join("a.inf"), "x").unwrap();
        std::fs::write(other.join("evil.inf"), "x").unwrap();
        let ok = inside_library(&lib, "pkg/a.inf").expect("dentro");
        assert!(ok.is_file() && !ok.to_string_lossy().starts_with(r"\\?\"), "{ok:?}");
        assert!(inside_library(&lib, "../lib2/evil.inf").is_none(), "con ..");
        assert!(inside_library(&lib, "pkg/../../lib2/evil.inf").is_none());
        assert!(inside_library(&lib, "C:/Windows/win.ini").is_none(), "unidad");
        assert!(inside_library(&lib, "/pkg/a.inf").is_none(), "absoluta");
        assert!(inside_library(&lib, "pkg/no-existe.inf").is_none());
        assert!(inside_library(&lib, "").is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn wia_device_is_chosen_without_guessing() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(pick_wia_device(&n(&["EPSON DS-530", "EPSON DS-530 II"]), "epson ds-530").unwrap(), "EPSON DS-530", "gana el exacto");
        assert_eq!(pick_wia_device(&n(&["EPSON DS-530"]), "EPSON DS-530 Series").unwrap(), "EPSON DS-530", "único parecido");
        assert!(pick_wia_device(&n(&["EPSON DS-530", "EPSON DS-5300"]), "EPSON DS-53").unwrap_err().contains("Elige"));
        assert!(pick_wia_device(&n(&["A", "A"]), "A").unwrap_err().contains("mismo nombre"));
        assert!(pick_wia_device(&n(&[]), "A").unwrap_err().contains("disponible"));
        assert!(pick_wia_device(&n(&["fi-7160"]), "").is_err());
    }

    #[test]
    fn usb_queue_helpers() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(pick_usb_port(&s(&["USB001", "USB002"]), &s(&["usb001"])).unwrap(), "USB002");
        assert!(pick_usb_port(&s(&["USB001"]), &s(&["USB001"])).is_err());
        assert!(pick_usb_port(&s(&["USB001", "USB002"]), &[]).unwrap_err().contains("varios"));
        assert_eq!(free_queue_name(&s(&["EPSON L3250", "EPSON L3250 (2)"]), "epson l3250"), "epson l3250 (3)");
        assert_eq!(free_queue_name(&[], "X"), "X");
        let d = DeviceRef { name: "Dispositivo desconocido".into(), vendor: "epson".into(), model: "L3250".into(), hardware_ids: vec![r"USB\VID_04B8&PID_1".into()], ..Default::default() };
        assert_eq!(queue_base_name(&d), "EPSON L3250");
        assert!(is_usb_device(&d));
        assert_eq!(queue_base_name(&DeviceRef { name: "EPSON ET-2820 Series".into(), ..Default::default() }), "EPSON ET-2820 Series");
    }

    #[test]
    fn download_ids_are_opaque() {
        let path = PathBuf::from(r"C:\Users\ana\Downloads\x.exe");
        let id = remember_download(&path);
        assert!(id.starts_with("dl:") && !id.contains("ana"));
        assert_eq!(download_path(&id), Some(path));
        assert!(download_path("dl:nope").is_none());
    }

    #[test]
    fn scan_interfaces_follow_what_is_installed() {
        let sw = |id: &str| ScanSoftware { id: id.into(), name: id.into(), vendor: "ricoh".into(), installed: true, version: String::new() };
        let twain = vec!["EPSON Scan 2".to_string(), "PFU".to_string()];
        assert_eq!(scan_interfaces("epson", true, &twain, &[]), ["WIA", "TWAIN"]);
        assert_eq!(scan_interfaces("ricoh", true, &twain, &[sw("paperstream-ip")]), ["WIA", "TWAIN", "PaperStream IP"]);
        assert!(scan_interfaces("ricoh", false, &[], &[]).is_empty());
    }

    #[test]
    fn embedded_scripts_parse() {
        for (nombre, script) in [
            ("PRINTERS_SCRIPT", PRINTERS_SCRIPT),
            ("SCANNERS_SCRIPT", SCANNERS_SCRIPT),
            ("WU_SEARCH_SCRIPT", WU_SEARCH_SCRIPT),
            ("WU_INSTALL_SCRIPT", WU_INSTALL_SCRIPT),
            ("CURRENT_SCRIPT", CURRENT_SCRIPT),
            ("PUBLISHED_SCRIPT", PUBLISHED_SCRIPT),
            ("REGISTER_SCRIPT", REGISTER_SCRIPT),
            ("ASSIGN_SCRIPT", ASSIGN_SCRIPT),
            ("TEST_PAGE_SCRIPT", TEST_PAGE_SCRIPT),
            ("SCAN_SCRIPT", SCAN_SCRIPT),
            ("WIA_LIST_SCRIPT", WIA_LIST_SCRIPT),
            ("SNAPSHOT_SCRIPT", SNAPSHOT_SCRIPT),
            ("CREATE_QUEUE_SCRIPT", CREATE_QUEUE_SCRIPT),
            ("INSTALLER_SCRIPT", INSTALLER_SCRIPT),
        ] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{nombre}: {errors}");
        }
    }

    /// Solo lectura, en este equipo: `cargo test real_inventory -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_inventory() {
        let out = run_ps(PRINTERS_SCRIPT, 90).unwrap();
        let raw: RawInventory = serde_json::from_str(out.trim()).unwrap();
        println!("{} impresoras, {} drivers, {} dispositivos", raw.printers.len(), raw.drivers.len(), raw.pnp.len());
        let out = run_ps(SCANNERS_SCRIPT, 120).unwrap();
        let raw: RawScanners = serde_json::from_str(out.trim()).unwrap();
        println!("{} aparatos de imagen, {} programas", raw.images.len(), raw.software.len());
    }
}
