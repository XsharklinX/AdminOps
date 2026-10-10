//! Outlook y Office a fondo: tamaño de los archivos de datos (OST/PST) frente a
//! su límite, perfiles de correo, complementos que retrasan el arranque (Outlook
//! apunta cuánto tarda cada uno), la licencia y las credenciales guardadas. Y la
//! reparación rápida u online de Office con un clic.

use super::{gb, parse};
use crate::troubleshoot::{finding, fix, fix_confirm, Finding};
use serde::Deserialize;
use std::time::Duration;

/// Outlook avisa y se vuelve lento al acercarse a los 50 GB (el límite de fábrica).
pub const DATA_LIMIT: u64 = 50 * 1024 * 1024 * 1024;
const DATA_WARN: u64 = 40 * 1024 * 1024 * 1024;
/// Un complemento que tarda más de esto en cargar se nota al abrir Outlook.
const SLOW_ADDIN_MS: u32 = 1000;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct DataFile {
    pub path: String,
    pub size: u64,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Addin {
    pub prog_id: String,
    pub name: String,
    /// Clave del registro (HKCU\… o HKLM\…).
    pub key: String,
    pub load_behavior: u32,
    /// Tiempos de carga que apunta Outlook (ms), si los hay.
    pub load_times: Vec<u32>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct OutlookRaw {
    pub installed: bool,
    pub version: String,
    pub products: String,
    pub platform: String,
    pub culture: String,
    pub install_path: String,
    pub data_files: Vec<DataFile>,
    pub max_large_file_gb: u32,
    pub profiles: Vec<String>,
    pub default_profile: String,
    pub addins: Vec<Addin>,
    pub disabled_items: u32,
    pub license_files: u32,
    pub license_status: String,
    pub office_creds: u32,
    pub outlook_running: bool,
}

const SCRIPT: &str = r#"
$c2r = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Office\ClickToRun\Configuration' -ErrorAction SilentlyContinue
$files = @()
foreach ($d in @("$UserLocalAppData\Microsoft\Outlook", "$UserProfile\Documents\Outlook Files", "$UserProfile\Documents\Archivos de Outlook")) {
  if (Test-Path $d) { $files += @(Get-ChildItem $d -File -ErrorAction SilentlyContinue | Where-Object { $_.Extension -in '.ost', '.pst' } | ForEach-Object { [pscustomobject]@{ path = $_.FullName; size = [int64]$_.Length } }) }
}
$ol = "$UserHive\Software\Microsoft\Office\16.0\Outlook"
$profiles = @(Get-ChildItem "$ol\Profiles" -ErrorAction SilentlyContinue | ForEach-Object { $_.PSChildName })
$def = "$((Get-ItemProperty $ol -ErrorAction SilentlyContinue).DefaultProfile)"
$max = [int]((Get-ItemProperty "$ol\PST" -ErrorAction SilentlyContinue).MaxLargeFileSize / 1024)
$times = Get-ItemProperty "$ol\AddInLoadTimes" -ErrorAction SilentlyContinue
$addins = @()
foreach ($root in @("$UserHive\Software\Microsoft\Office\Outlook\Addins", 'HKLM:\Software\Microsoft\Office\Outlook\Addins', 'HKLM:\Software\WOW6432Node\Microsoft\Office\Outlook\Addins')) {
  Get-ChildItem $root -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
    $id = $_.PSChildName
    $t = @()
    if ($times -and $times.$id) { $b = [byte[]]$times.$id; for ($i = 0; $i + 3 -lt $b.Length; $i += 4) { $t += [BitConverter]::ToUInt32($b, $i) } }
    $addins += [pscustomobject]@{ progId = $id; name = "$($p.FriendlyName)"; key = ($_.Name -replace '^HKEY_CURRENT_USER', 'HKCU' -replace '^HKEY_USERS\\[^\\]+', 'HKCU' -replace '^HKEY_LOCAL_MACHINE', 'HKLM'); loadBehavior = [int]$p.LoadBehavior; loadTimes = $t }
  }
}
$dis = @((Get-Item "$ol\Resiliency\DisabledItems" -ErrorAction SilentlyContinue).Property).Count
$lic = @(Get-ChildItem "$UserLocalAppData\Microsoft\Office\Licenses" -Recurse -File -ErrorAction SilentlyContinue).Count
$status = ''
try { $p = Get-CimInstance SoftwareLicensingProduct -Filter "ApplicationId='0ff1ce15-a989-479d-af46-f275c6370663' AND PartialProductKey IS NOT NULL" -ErrorAction Stop | Select-Object -First 1; if ($p) { $status = "$($p.LicenseStatus)" } } catch { }
$creds = @((cmdkey.exe /list) -match 'MicrosoftOffice1').Count
[pscustomobject]@{
  installed = [bool]($c2r -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Office\16.0\Outlook'))
  version = "$($c2r.VersionToReport)"; products = "$($c2r.ProductReleaseIds)"; platform = "$($c2r.Platform)"; culture = "$($c2r.ClientCulture)"; installPath = "$($c2r.InstallationPath)"
  dataFiles = $files; maxLargeFileGb = $max; profiles = $profiles; defaultProfile = $def; addins = $addins; disabledItems = $dis
  licenseFiles = $lic; licenseStatus = $status; officeCreds = $creds
  outlookRunning = [bool](Get-Process OUTLOOK -ErrorAction SilentlyContinue)
} | ConvertTo-Json -Depth 4 -Compress
"#;

/// Tiempo de carga típico de un complemento: Outlook guarda los de los últimos
/// arranques; a veces el primer número es cuántos hay.
pub fn typical_load_ms(times: &[u32]) -> Option<u32> {
    let list = match times {
        [n, rest @ ..] if *n as usize == rest.len() && !rest.is_empty() => rest,
        all => all,
    };
    let valid: Vec<u32> = list.iter().copied().filter(|t| *t > 0 && *t < 120_000).collect();
    if valid.is_empty() {
        return None;
    }
    Some(valid.iter().sum::<u32>() / valid.len() as u32)
}

/// ¿El complemento está activo (se carga al abrir Outlook)?
pub fn addin_active(load_behavior: u32) -> bool {
    load_behavior & 1 == 1
}

/// Nombre que se enseña de un complemento.
fn addin_name(a: &Addin) -> String {
    if a.name.trim().is_empty() {
        a.prog_id.clone()
    } else {
        a.name.trim().to_string()
    }
}

/// Lo que se puede decir de Outlook con lo leído (sin tocar nada).
pub fn findings(raw: &OutlookRaw) -> Vec<Finding> {
    let mut out = Vec::new();
    if !raw.installed {
        out.push(finding("info", "No se encuentra Office en este equipo", "Si se usa Outlook en la web o la aplicación «Outlook (nuevo)», estas comprobaciones no aplican."));
        return out;
    }
    let limit = if raw.max_large_file_gb > 0 { raw.max_large_file_gb as u64 * 1024 * 1024 * 1024 } else { DATA_LIMIT };
    let warn_at = if raw.max_large_file_gb > 0 { limit * 8 / 10 } else { DATA_WARN };
    for f in &raw.data_files {
        let name = f.path.rsplit('\\').next().unwrap_or(&f.path);
        if f.size >= limit {
            out.push(finding("bad", format!("{name} ha llegado al límite ({})", gb(f.size)), format!("Outlook deja de guardar correo nuevo en ese archivo. Archiva lo antiguo o reduce el tiempo que se descarga sin conexión (Cuenta → Configuración → «Correo para mantener sin conexión»). Límite: {}.", gb(limit))).fixes(vec![fix(format!("reveal:{}", f.path), "Abrir la carpeta", false)]));
        } else if f.size >= warn_at {
            out.push(finding("warn", format!("{name} se acerca al límite ({})", gb(f.size)), format!("Con más de 40 GB Outlook va lento al abrir y buscar. Límite: {}.", gb(limit))).fixes(vec![fix(format!("reveal:{}", f.path), "Abrir la carpeta", false)]));
        }
    }
    let mut slow: Vec<(&Addin, u32)> = raw.addins.iter().filter(|a| addin_active(a.load_behavior)).filter_map(|a| typical_load_ms(&a.load_times).map(|t| (a, t))).filter(|(_, t)| *t >= SLOW_ADDIN_MS).collect();
    slow.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    for (a, t) in slow.iter().take(4) {
        out.push(
            finding("warn", format!("El complemento «{}» tarda {:.1} s al abrir Outlook", addin_name(a), *t as f64 / 1000.0).replace('.', ","), "Cada vez que se abre Outlook espera a que cargue. Si no se usa, desactívalo: se puede volver a activar aquí mismo.")
                .fixes(vec![fix(format!("ol.addin.off:{}", a.key), "Desactivar el complemento", a.key.starts_with("HKLM"))]),
        );
    }
    let off: Vec<&Addin> = raw.addins.iter().filter(|a| !addin_active(a.load_behavior) && !a.prog_id.is_empty()).collect();
    if !off.is_empty() {
        let names: Vec<String> = off.iter().take(5).map(|a| addin_name(a)).collect();
        out.push(
            finding("info", format!("{} complemento(s) desactivados", off.len()), format!("{}. Si alguno hace falta (firma, CRM…), actívalo de nuevo.", names.join(", ")))
                .fixes(off.iter().take(3).map(|a| fix(format!("ol.addin.on:{}", a.key), &format!("Activar «{}»", addin_name(a)), a.key.starts_with("HKLM"))).collect()),
        );
    }
    if raw.disabled_items > 0 {
        out.push(finding("warn", "Outlook desactivó complementos por fallar", format!("{} elemento(s) en la lista negra de Outlook: dejaron de cargarse tras un cierre inesperado. Se ven en Archivo → Opciones → Complementos → «Elementos deshabilitados».", raw.disabled_items)));
    }
    if raw.profiles.len() > 1 {
        out.push(finding("info", format!("{} perfiles de correo", raw.profiles.len()), format!("Predeterminado: «{}». Si Outlook no abre o pide la contraseña sin parar, crear un perfil nuevo (conservando el antiguo) suele arreglarlo.", raw.default_profile)).fixes(vec![fix("ol.profiles", "Abrir los perfiles de correo", false)]));
    } else {
        out.push(finding("info", "Perfil de correo", "Si Outlook no abre, se queda «Procesando» o pide la contraseña sin parar, crear un perfil nuevo suele arreglarlo (el antiguo se conserva).").fixes(vec![fix("ol.profiles", "Abrir los perfiles de correo", false)]));
    }
    if raw.office_creds > 3 {
        out.push(
            finding("warn", format!("{} credenciales de Office guardadas", raw.office_creds), "Muchas credenciales viejas hacen que Outlook y Teams pidan la contraseña una y otra vez. Borrarlas obliga a iniciar sesión una vez y suele cortar el bucle.")
                .fixes(vec![fix_confirm("ol.creds", "Borrar las credenciales de Office", false, "Se borran las contraseñas de Office guardadas en Windows. Outlook, Teams y OneDrive pedirán iniciar sesión una vez.")]),
        );
    }
    let licensed = raw.license_files > 0 || raw.license_status == "1";
    if !licensed && !raw.products.is_empty() {
        out.push(finding("warn", "No se ve una licencia de Office activa", format!("Productos instalados: {}. Si Office dice «Producto sin licencia», inicia sesión con la cuenta que tiene la licencia (Archivo → Cuenta).", raw.products)));
    }
    if !raw.install_path.is_empty() {
        let quick = fix_confirm("ol.repair:quick", "Reparación rápida de Office", false, "Office se cerrará. La reparación rápida tarda unos minutos y no necesita Internet.");
        let online = fix_confirm("ol.repair:online", "Reparación online (a fondo)", false, "Office se cerrará y se volverá a descargar entero (unos 3 GB). Úsala si la rápida no bastó.");
        out.push(finding("info", format!("Office {}", raw.version), "Si Office se cierra solo, no abre o va raro, la reparación rápida suele bastar; la online lo reinstala entero.").fixes(vec![quick, online]));
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(0, finding("ok", "Outlook y Office no muestran problemas", format!("{} archivo(s) de datos, {} complemento(s) activos.", raw.data_files.len(), raw.addins.iter().filter(|a| addin_active(a.load_behavior)).count())));
    }
    out
}

pub fn check() -> Result<Vec<Finding>, String> {
    let script = format!("{}{SCRIPT}", crate::target_user::script_prelude());
    let out = crate::pspool::query(&script, Some(Duration::from_secs(60)), "Solucionar: Outlook y Office")?;
    Ok(findings(&parse::<OutlookRaw>(&out)?))
}

/// La clave del registro es la de un complemento de Outlook (no se toca otra cosa).
pub fn valid_addin_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    (k.starts_with("hkcu\\") || k.starts_with("hklm\\"))
        && (k.contains("\\software\\microsoft\\office\\outlook\\addins\\") || k.contains("\\software\\wow6432node\\microsoft\\office\\outlook\\addins\\"))
        && !k.contains("..")
}

fn set_addin(key: &str, on: bool) -> Result<String, String> {
    if !valid_addin_key(key) {
        return Err("Esa clave no es de un complemento de Outlook.".into());
    }
    // Por el módulo del registro de AdminOps: HKCU es el del usuario con la sesión abierta.
    use crate::tweaks::model::{RegData, RegKind};
    crate::tweaks::registry::write(key, "LoadBehavior", RegKind::Dword, &RegData::Int(if on { 3 } else { 0 }))?;
    Ok(if on { "Complemento activado: se cargará la próxima vez que se abra Outlook." } else { "Complemento desactivado. Cierra y abre Outlook para notarlo." }.into())
}

fn office_paths() -> (String, String, String) {
    let get = |name: &str| crate::tweaks::registry::read_string(r"HKLM\SOFTWARE\Microsoft\Office\ClickToRun\Configuration", name).unwrap_or_default();
    (get("InstallationPath"), get("Platform"), get("ClientCulture"))
}

fn repair(online: bool) -> Result<String, String> {
    let (install, platform, culture) = office_paths();
    if install.is_empty() {
        return Err("No se encuentra una instalación de Office «Hacer clic y ejecutar» que reparar.".into());
    }
    let exe = r"C:\Program Files\Common Files\microsoft shared\ClickToRun\OfficeClickToRun.exe";
    let platform = if platform.is_empty() { "x64".to_string() } else { platform };
    let culture = if culture.is_empty() { "es-es".to_string() } else { culture };
    std::process::Command::new(exe)
        .args(["scenario=Repair", &format!("platform={platform}"), &format!("culture={culture}"), &format!("RepairType={}", if online { "FullRepair" } else { "QuickRepair" }), "DisplayLevel=True", "forceappshutdown=True"])
        .spawn()
        .map_err(|e| format!("No se pudo abrir el reparador de Office: {e}"))?;
    Ok("Reparación de Office iniciada: sigue los pasos en su ventana.".into())
}

fn open_profiles() -> Result<String, String> {
    let (install, _, _) = office_paths();
    let cpl = format!(r"{install}\root\Office16\MLCFG32.CPL");
    if !install.is_empty() && std::path::Path::new(&cpl).exists() {
        std::process::Command::new("rundll32.exe").args(["shell32.dll,Control_RunDLL", &cpl]).spawn().map_err(|e| e.to_string())?;
    } else {
        std::process::Command::new("control.exe").arg("mlcfg32.cpl").spawn().map_err(|e| e.to_string())?;
    }
    Ok("En «Mostrar perfiles…» → «Agregar» se crea uno nuevo; marca «Solicitar un perfil» para poder volver al antiguo.".into())
}

/// Credenciales de Office en la salida de `cmdkey /list`.
pub fn office_targets(cmdkey: &str) -> Vec<String> {
    cmdkey
        .lines()
        .filter_map(|l| l.split_once(':').map(|(_, v)| v.trim()))
        .filter(|v| v.contains("MicrosoftOffice1"))
        .map(|v| v.split_once("target=").map(|(_, r)| r).unwrap_or(v).to_string())
        .collect()
}

fn clear_creds() -> Result<String, String> {
    let list = crate::ps::exec("cmdkey.exe", &["/list"])?;
    let targets = office_targets(&list);
    let mut n = 0;
    for t in &targets {
        if crate::ps::exec("cmdkey.exe", &[&format!("/delete:{t}")]).is_ok() {
            n += 1;
        }
    }
    Ok(format!("{n} credencial(es) de Office borradas. Abre Outlook e inicia sesión una vez."))
}

pub fn run(kind: &str, arg: &str) -> Option<Result<String, String>> {
    Some(match kind {
        "ol.addin.off" => set_addin(arg, false),
        "ol.addin.on" => set_addin(arg, true),
        "ol.repair" => repair(arg == "online"),
        "ol.profiles" => open_profiles(),
        "ol.creds" => clear_creds(),
        _ => return None,
    })
}

pub fn title(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "ol.addin.off" => "Outlook: desactivar un complemento",
        "ol.addin.on" => "Outlook: activar un complemento",
        "ol.repair" => "Reparar Office",
        "ol.creds" => "Borrar las credenciales de Office",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiempos_de_carga() {
        assert_eq!(typical_load_ms(&[3, 1000, 2000, 3000]), Some(2000));
        assert_eq!(typical_load_ms(&[800, 1200]), Some(1000));
        assert_eq!(typical_load_ms(&[]), None);
        assert_eq!(typical_load_ms(&[0, 0]), None);
    }

    #[test]
    fn complementos_lentos_y_archivos_grandes() {
        let raw = OutlookRaw {
            installed: true,
            version: "16.0.17928".into(),
            products: "O365BusinessRetail".into(),
            install_path: r"C:\Program Files\Microsoft Office".into(),
            data_files: vec![DataFile { path: r"C:\Users\x\AppData\Local\Microsoft\Outlook\a@b.ost".into(), size: 47 * 1024 * 1024 * 1024 }],
            addins: vec![
                Addin { prog_id: "Firma.Connect".into(), name: "Firma Corporativa".into(), key: r"HKCU\Software\Microsoft\Office\Outlook\Addins\Firma.Connect".into(), load_behavior: 3, load_times: vec![3800, 3700] },
                Addin { prog_id: "Rapido".into(), name: String::new(), key: r"HKCU\Software\Microsoft\Office\Outlook\Addins\Rapido".into(), load_behavior: 3, load_times: vec![50] },
            ],
            license_files: 2,
            ..Default::default()
        };
        let f = findings(&raw);
        assert!(f.iter().any(|x| x.level == "warn" && x.title.contains("a@b.ost")), "{f:?}");
        let slow: Vec<_> = f.iter().filter(|x| x.title.contains("tarda")).collect();
        assert_eq!(slow.len(), 1);
        assert!(slow[0].title.contains("Firma Corporativa") && slow[0].title.contains("3,"), "{}", slow[0].title);
        assert!(slow[0].fixes[0].id.starts_with("ol.addin.off:HKCU"));
    }

    #[test]
    fn sin_office_no_hay_nada_que_mirar() {
        let f = findings(&OutlookRaw::default());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].level, "info");
    }

    #[test]
    fn solo_toca_claves_de_complementos() {
        assert!(valid_addin_key(r"HKCU\Software\Microsoft\Office\Outlook\Addins\Firma.Connect"));
        assert!(valid_addin_key(r"HKLM\Software\WOW6432Node\Microsoft\Office\Outlook\Addins\X"));
        assert!(!valid_addin_key(r"HKLM\SYSTEM\CurrentControlSet\Services\X"));
        assert!(!valid_addin_key(r"HKCU\Software\Microsoft\Office\Outlook\Addins\..\..\Run"));
    }

    #[test]
    fn credenciales_de_office() {
        let out = "Destino actual: MicrosoftOffice16_Data:SSPI:ana@empresa.com\n    Tipo: Genérico\nDestino actual: WindowsLive:target=virtualapp/didlogical\nTarget: LegacyGeneric:target=MicrosoftOffice15_Data:orgid:ana@empresa.com\n";
        assert_eq!(office_targets(out), vec!["MicrosoftOffice16_Data:SSPI:ana@empresa.com", "MicrosoftOffice15_Data:orgid:ana@empresa.com"]);
    }

    #[test]
    fn script_parses() {
        let e = crate::ps::parse_errors(SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
