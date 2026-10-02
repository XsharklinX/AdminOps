//! Discos: salud, reparación y rescate de archivos.
//!
//! «Me sale "Reparar disco" y hay archivos que no se copian» es de las
//! consultas más habituales, y la respuesta depende de qué está roto:
//!
//! - **El sistema de archivos** (Windows lo marcó como dañado, suele pasar al
//!   quitar un disco externo sin expulsarlo): se repara sin perder lo que está
//!   bien (`Repair-Volume`, lo mismo que chkdsk /f).
//! - **La superficie del disco** (sectores que ya no se leen: SMART con
//!   sectores pendientes o no corregibles): eso no lo arregla ningún programa.
//!   El propio disco aparta los sectores malos; lo que se puede hacer es
//!   **copiar primero lo que aún se lee** y cambiar el disco. Pasar chkdsk /r
//!   antes de copiar obliga a leer justo las zonas dañadas y puede empeorarlo.
//! - **La conexión** (errores CRC: cable, puerto o la caja USB del disco externo).
//!
//! Por eso aquí se da primero un veredicto por disco y cada acción dice qué hace.

use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;
use tauri::State;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Volume {
    pub letter: String,
    pub label: String,
    pub fs: String,
    pub size: u64,
    pub free: u64,
    pub health: String,
    /// Windows lo marcó como dañado (el «Reparar disco» de Windows). None: no se pudo mirar.
    pub dirty: Option<bool>,
    /// Es el volumen de Windows.
    pub system: bool,
    /// BitLocker: "" sin cifrar o no se sabe · on · suspended · encrypting · decrypting.
    pub bitlocker: String,
    /// Porcentaje cifrado (mientras cifra o descifra).
    pub bitlocker_percent: i64,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Disk {
    pub number: i64,
    pub model: String,
    /// USB, SATA, NVMe, SAS, SD…
    pub bus: String,
    /// HDD, SSD o Unspecified.
    pub media: String,
    pub size: u64,
    /// Healthy, Warning o Unhealthy (lo que dice Windows).
    pub health: String,
    pub system: bool,
    /// -1: no se sabe (casi todo esto necesita administrador).
    pub temperature: i64,
    pub hours: i64,
    pub read_errors: i64,
    pub write_errors: i64,
    /// Desgaste de un SSD en %.
    pub wear: i64,
    pub volumes: Vec<Volume>,
    // SMART (solo discos ATA/SATA y con administrador).
    pub predict_failure: bool,
    pub reallocated: Option<u64>,
    pub pending: Option<u64>,
    pub uncorrectable: Option<u64>,
    pub crc_errors: Option<u64>,
    #[serde(skip_deserializing)]
    pub verdict: Verdict,
    /// Cifras de los últimos días (una por día) para ver si va a peor.
    #[serde(skip_deserializing)]
    pub trend: Vec<Point>,
    /// Lo que ha subido en el último mes («Sectores apartados: 8 → 40 en 7 días»).
    #[serde(skip_deserializing)]
    pub rising: Vec<String>,
}

/// Una foto diaria de las cifras que indican desgaste.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Point {
    /// Días desde 1970.
    pub day: u32,
    pub reallocated: Option<u64>,
    pub pending: Option<u64>,
    pub uncorrectable: Option<u64>,
    pub crc: Option<u64>,
    pub read_errors: i64,
    pub wear: i64,
}

type History = std::collections::BTreeMap<String, Vec<Point>>;
/// Por disco (modelo), lo que ha subido.
type News = Vec<(String, Vec<String>)>;

fn today() -> u32 {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) / 86_400) as u32
}

fn history_key(d: &Disk) -> String {
    format!("{}|{}", d.model.trim(), d.size)
}

fn point(d: &Disk, day: u32) -> Point {
    Point { day, reallocated: d.reallocated, pending: d.pending, uncorrectable: d.uncorrectable, crc: d.crc_errors, read_errors: d.read_errors, wear: d.wear }
}

/// Qué ha subido entre dos fotos, en palabras.
pub fn rising(from: &Point, to: &Point) -> Vec<String> {
    let days = to.day.saturating_sub(from.day);
    let when = match days {
        0 => "hoy".to_string(),
        1 => "en 1 día".to_string(),
        n => format!("en {n} días"),
    };
    let mut out = Vec::new();
    let mut up = |name: &str, a: Option<u64>, b: Option<u64>| {
        if let (Some(a), Some(b)) = (a, b) {
            if b > a {
                out.push(format!("{name}: {a} → {b} {when}"));
            }
        }
    };
    up("Sectores apartados", from.reallocated, to.reallocated);
    up("Sectores pendientes", from.pending, to.pending);
    up("Errores no corregibles", from.uncorrectable, to.uncorrectable);
    up("Errores de conexión", from.crc, to.crc);
    let known = |x: i64| (x >= 0).then_some(x as u64);
    up("Errores de lectura", known(from.read_errors), known(to.read_errors));
    out
}

/// Apunta la foto de hoy de cada disco y devuelve, por disco, lo que ha subido
/// en el último mes (para la tarjeta) y desde la foto anterior (para avisar).
fn record(history: &mut History, disks: &mut [Disk], day: u32) -> News {
    let mut news = Vec::new();
    for d in disks.iter_mut() {
        let list = history.entry(history_key(d)).or_default();
        let now = point(d, day);
        if let Some(prev) = list.iter().rev().find(|p| p.day < day) {
            let fresh = rising(prev, &now);
            if !fresh.is_empty() {
                news.push((d.model.clone(), fresh));
            }
        }
        list.retain(|p| p.day != day);
        list.push(now.clone());
        list.sort_by_key(|p| p.day);
        let keep = list.len().saturating_sub(365);
        list.drain(..keep);
        if let Some(oldest) = list.iter().find(|p| p.day + 30 >= day && p.day < day) {
            d.rising = rising(oldest, &now);
        }
        d.trend = list.iter().filter(|p| p.day + 90 >= day).cloned().collect();
    }
    news
}

/// Un disco que va a peor no está «sano», aunque hoy sus cifras aún sean bajas.
fn apply_trend(d: &mut Disk) {
    if d.rising.is_empty() || d.verdict.level == "bad" {
        return;
    }
    d.verdict = v(
        "warn",
        "Va a peor",
        format!("{}. Es la señal de un disco que empieza a fallar.", d.rising.join(" · ")),
        &["Ten una copia al día de lo importante (Rescatar archivos o la copia de seguridad).", "Si sigue subiendo en los próximos días, cámbialo."],
    );
}

fn history_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    crate::paths::machine_data_dir(app).join("discos-historial.json")
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    /// ok | warn | bad
    pub level: String,
    pub title: String,
    pub text: String,
    /// Qué hacer, por orden.
    pub advice: Vec<String>,
}

fn v(level: &str, title: impl Into<String>, text: impl Into<String>, advice: &[&str]) -> Verdict {
    Verdict { level: level.into(), title: title.into(), text: text.into(), advice: advice.iter().map(|s| s.to_string()).collect() }
}

/// Qué le pasa a un disco, por orden de gravedad: primero la superficie (se
/// pierden datos), luego la conexión, el sistema de archivos y el resto.
pub fn verdict(d: &Disk) -> Verdict {
    let external = d.bus.eq_ignore_ascii_case("USB");
    let pending = d.pending.unwrap_or(0);
    let uncorrectable = d.uncorrectable.unwrap_or(0);
    if d.predict_failure || pending > 0 || uncorrectable > 0 || d.health.eq_ignore_ascii_case("Unhealthy") || d.read_errors > 0 {
        let mut detalle = Vec::new();
        if pending > 0 {
            detalle.push(format!("{pending} sectores que no se pueden leer"));
        }
        if uncorrectable > 0 {
            detalle.push(format!("{uncorrectable} errores que el disco no pudo corregir"));
        }
        if d.read_errors > 0 {
            detalle.push(format!("{} errores de lectura sin corregir", d.read_errors));
        }
        if d.predict_failure {
            detalle.push("el propio disco avisa de que va a fallar".into());
        }
        let text = if detalle.is_empty() { "Windows lo da por averiado.".to_string() } else { format!("El disco tiene {}.", detalle.join(", ")) };
        return v(
            "bad",
            "El disco se está estropeando",
            format!("{text} Es la superficie del disco, no un error de Windows: ningún programa la repara. Por eso hay archivos que no se copian."),
            &[
                "Copia ya lo importante a otro disco con «Rescatar archivos» (salta lo que no se puede leer y te dice qué se quedó).",
                "No pases «Buscar sectores dañados» (chkdsk /r) antes de copiar: obliga a leer justo las zonas dañadas y puede empeorarlo.",
                "Después, cambia el disco. Si hay algo imprescindible que no salió, un servicio de recuperación de datos (no sigas usándolo mientras tanto).",
            ],
        );
    }
    let crc = d.crc_errors.unwrap_or(0);
    if crc > 0 {
        return v(
            "warn",
            "Errores de conexión con el disco",
            format!("{crc} errores de comunicación (CRC): los datos se estropean por el camino, no en el disco."),
            if external {
                &["Cambia el cable USB y prueba en otro puerto (mejor uno trasero del equipo).", "Si sigue, suele ser la caja o el adaptador USB del disco externo, no el disco."]
            } else {
                &["Cambia el cable SATA o el puerto de la placa.", "Si sigue apareciendo tras cambiarlo, revisa la fuente de alimentación."]
            },
        );
    }
    let reallocated = d.reallocated.unwrap_or(0);
    if reallocated > 0 || d.health.eq_ignore_ascii_case("Warning") {
        return v(
            "warn",
            if reallocated > 0 { format!("Ya ha tenido sectores dañados ({reallocated} apartados)") } else { "Windows avisa de un problema con el disco".into() },
            "El disco apartó esos sectores y de momento funciona, pero es la señal típica de un disco que empieza a fallar.",
            &["Ten una copia al día de lo importante.", "Vuelve a mirarlo en unas semanas: si la cifra sube, cámbialo."],
        );
    }
    let dirty: Vec<&str> = d.volumes.iter().filter(|x| x.dirty == Some(true)).map(|x| x.letter.as_str()).collect();
    if !dirty.is_empty() {
        let letras = dirty.iter().map(|l| format!("{l}:")).collect::<Vec<_>>().join(", ");
        return v(
            "warn",
            format!("El sistema de archivos de {letras} está marcado como dañado"),
            "Es lo que hace que Windows diga «Reparar disco». El disco en sí está bien: se estropeó el índice de archivos, casi siempre por quitarlo sin expulsarlo o por un corte de luz.",
            &["Pulsa «Reparar sistema de archivos»: arregla el índice sin tocar los archivos buenos.", if external { "A partir de ahora, «Expulsar» antes de desconectarlo." } else { "Si vuelve a pasar sin cortes de luz, revisa el disco a fondo." }],
        );
    }
    if d.wear >= 90 {
        return v("warn", format!("El SSD está al {} % de su vida útil", d.wear), "Los SSD se gastan con las escrituras; a partir de aquí puede empezar a fallar.", &["Ten una copia al día y planifica cambiarlo."]);
    }
    let hot = if d.media.eq_ignore_ascii_case("SSD") { 70 } else { 55 };
    if d.temperature >= hot {
        return v("warn", format!("El disco está caliente ({} °C)", d.temperature), "Con calor constante duran menos.", &["Revisa la ventilación del equipo o de la caja externa."]);
    }
    let base = if d.hours > 0 { format!("{} horas encendido.", d.hours) } else { String::new() };
    v("ok", "Sano", format!("Sin errores de lectura ni sectores dañados. {base}").trim().to_string(), &[])
}

// ---------- Leer el estado ----------

const STATUS_SCRIPT: &str = r#"
$sys = "$env:SystemDrive".TrimEnd(':')
# BitLocker (solo con administrador y en ediciones que lo tienen).
$bl = @{}
try { Get-BitLockerVolume -ErrorAction Stop | ForEach-Object { $bl["$($_.MountPoint)".TrimEnd(':', '\')] = $_ } } catch {}
$r = @(Get-PhysicalDisk -ErrorAction Stop | ForEach-Object {
  $pd = $_
  $rel = $null
  try { $rel = $pd | Get-StorageReliabilityCounter -ErrorAction Stop } catch {}
  $disk = Get-Disk -ErrorAction SilentlyContinue | Where-Object { "$($_.Number)" -eq "$($pd.DeviceId)" } | Select-Object -First 1
  $vols = @()
  if ($disk) {
    $vols = @(Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter } | ForEach-Object {
      $vo = $_ | Get-Volume -ErrorAction SilentlyContinue
      $l = "$($_.DriveLetter)"
      $b = $bl[$l]
      if ($vo) { [pscustomobject]@{ letter = $l; label = "$($vo.FileSystemLabel)"; fs = "$($vo.FileSystem)"; size = [uint64]$vo.Size; free = [uint64]$vo.SizeRemaining; health = "$($vo.HealthStatus)"; system = ($l -eq $sys)
        bitlocker = if ($b) { "$($b.ProtectionStatus)|$($b.VolumeStatus)" } else { '' }; bitlockerPercent = if ($b) { [int64]$b.EncryptionPercentage } else { -1 } } }
    })
  }
  $n = { param($x) if ($null -eq $x) { -1 } else { [int64]$x } }
  [pscustomobject]@{
    number = [int64]$pd.DeviceId; model = "$($pd.FriendlyName)".Trim(); bus = "$($pd.BusType)"; media = "$($pd.MediaType)"; size = [uint64]$pd.Size
    health = "$($pd.HealthStatus)"; system = [bool]($disk -and ($disk.IsSystem -or $disk.IsBoot))
    temperature = & $n $rel.Temperature; hours = & $n $rel.PowerOnHours; readErrors = & $n $rel.ReadErrorsUncorrected
    writeErrors = & $n $rel.WriteErrorsUncorrected; wear = & $n $rel.Wear
    volumes = $vols
  }
})
ConvertTo-Json -InputObject $r -Depth 4 -Compress
"#;

/// ¿Windows marcó el volumen como dañado? (FSCTL_IS_VOLUME_DIRTY, necesita administrador.)
#[cfg(windows)]
pub fn is_dirty(letter: &str) -> Option<bool> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;
    const FSCTL_IS_VOLUME_DIRTY: u32 = 0x0009_0078;
    const VOLUME_IS_DIRTY: u32 = 1;
    let c = letter.chars().next().filter(|c| c.is_ascii_alphabetic())?;
    let path: Vec<u16> = format!(r"\\.\{c}:").encode_utf16().chain(Some(0)).collect();
    // SAFETY: se abre el volumen solo para consultar, se consulta y se cierra.
    unsafe {
        let h = CreateFileW(path.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut flags = 0u32;
        let mut n = 0u32;
        let ok = DeviceIoControl(h, FSCTL_IS_VOLUME_DIRTY, std::ptr::null(), 0, (&mut flags as *mut u32).cast(), 4, &mut n, std::ptr::null_mut());
        CloseHandle(h);
        (ok != 0).then_some(flags & VOLUME_IS_DIRTY != 0)
    }
}

#[cfg(not(windows))]
pub fn is_dirty(_: &str) -> Option<bool> {
    None
}

/// «On|FullyEncrypted» → on; lo que no es cifrado → "".
pub fn bitlocker_state(raw: &str) -> String {
    let (protection, status) = raw.split_once('|').unwrap_or(("", raw));
    match status {
        "FullyEncrypted" if protection.eq_ignore_ascii_case("On") => "on",
        "FullyEncrypted" => "suspended",
        "EncryptionInProgress" | "EncryptionPaused" => "encrypting",
        "DecryptionInProgress" | "DecryptionPaused" => "decrypting",
        _ => "",
    }
    .to_string()
}

pub fn same_model(a: &str, b: &str) -> bool {
    let n = |s: &str| s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let (a, b) = (n(a), n(b));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

/// Todos los discos con sus volúmenes, su veredicto y su tendencia.
#[tauri::command(async)]
pub fn disks_status(app: tauri::AppHandle) -> Result<Vec<Disk>, String> {
    status_and_record(&app).map(|(d, _)| d)
}

/// Lee los discos y apunta la foto de hoy. Devuelve también lo que subió desde la anterior.
fn status_and_record(app: &tauri::AppHandle) -> Result<(Vec<Disk>, News), String> {
    let mut disks = read_status()?;
    let path = history_path(app);
    let mut history: History = crate::paths::read_json(&path);
    let news = record(&mut history, &mut disks, today());
    let _ = crate::paths::write_json(&path, &history);
    for d in &mut disks {
        apply_trend(d);
    }
    Ok((disks, news))
}

/// Una vez al día, con AdminOps abierto: si un disco va a peor, avisa.
pub fn start_watch(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10 * 60));
        let mut last = 0u32;
        loop {
            if today() != last {
                last = today();
                if let Ok((_, news)) = status_and_record(&app) {
                    let alerts = news
                        .into_iter()
                        .map(|(model, changes)| crate::winwatch::Alert {
                            key: format!("disk-trend:{model}:{}", changes.join(",")),
                            level: "warn".into(),
                            title: format!("El disco {model} va a peor"),
                            detail: changes.join(" · "),
                            explanation: "Las cifras de desgaste del disco han subido desde ayer. Es la señal típica de un disco que empieza a fallar.".into(),
                            advice: "Haz una copia de lo importante y míralo en Discos → Salud y reparación.".into(),
                            page: Some("space".into()),
                            count: 1,
                            time: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
                            ..Default::default()
                        })
                        .collect::<Vec<_>>();
                    if !alerts.is_empty() {
                        crate::winwatch::push_alerts(&app, alerts);
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(3600));
        }
    });
}

fn read_status() -> Result<Vec<Disk>, String> {
    let out = crate::ps::powershell_opts(STATUS_SCRIPT, crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None })?;
    let mut disks: Vec<Disk> = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    // SMART (solo con administrador): se cruza por modelo.
    let smart = crate::hardware::smart::read().unwrap_or_default();
    for d in &mut disks {
        if let Some(s) = smart.iter().find(|s| same_model(&s.model, &d.model)) {
            d.predict_failure = s.predict_failure;
            d.reallocated = s.reallocated;
            d.pending = s.pending;
            d.uncorrectable = s.uncorrectable;
            d.crc_errors = s.crc_errors;
            if d.temperature < 0 {
                d.temperature = s.temperature.map_or(-1, |t| t as i64);
            }
            if d.hours < 0 {
                d.hours = s.power_on_hours.map_or(-1, |h| h as i64);
            }
        }
        for vol in &mut d.volumes {
            vol.dirty = is_dirty(&vol.letter);
            vol.bitlocker = bitlocker_state(&vol.bitlocker);
        }
        d.verdict = verdict(d);
    }
    disks.sort_by_key(|d| (!d.system, d.number));
    Ok(disks)
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryKey {
    pub id: String,
    pub password: String,
}

/// Las claves de recuperación de BitLocker de un volumen de ESTE equipo
/// (necesita administrador). Queda en el diario que se consultaron.
#[tauri::command(async)]
pub fn bitlocker_local_key(letter: String, tweaks: State<'_, TweakState>) -> Result<Vec<RecoveryKey>, String> {
    let l = letter_ok(&letter)?;
    need_admin()?;
    let script = format!(
        "@((Get-BitLockerVolume -MountPoint '{l}:' -ErrorAction Stop).KeyProtector | Where-Object {{ \"$($_.KeyProtectorType)\" -eq 'RecoveryPassword' }} | ForEach-Object {{ \"$($_.KeyProtectorId)|$($_.RecoveryPassword)\" }}) -join \"`n\""
    );
    let result = crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(60)), task: None }).map(|out| {
        out.lines()
            .filter_map(|x| x.trim().split_once('|'))
            .map(|(id, pw)| RecoveryKey { id: id.trim_matches(['{', '}']).to_string(), password: pw.to_string() })
            .collect::<Vec<_>>()
    });
    let journal = result.as_ref().map(|_| ()).map_err(Clone::clone);
    tweaks.record(Op::Run, &format!("Consultada la clave de recuperación de BitLocker de {l}:"), &journal);
    let keys = result?;
    if keys.is_empty() {
        return Err(format!("{l}: no tiene clave de recuperación (o no está cifrado con BitLocker)."));
    }
    Ok(keys)
}

// ---------- Acciones ----------

fn letter_ok(letter: &str) -> Result<char, String> {
    let mut c = letter.trim().trim_end_matches(['\\', ':']).chars();
    match (c.next(), c.next()) {
        (Some(l), None) if l.is_ascii_alphabetic() => Ok(l.to_ascii_uppercase()),
        _ => Err("Unidad no válida.".into()),
    }
}

fn is_system(letter: char) -> bool {
    std::env::var("SystemDrive").ok().and_then(|s| s.chars().next()).is_some_and(|s| s.eq_ignore_ascii_case(&letter))
}

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Requiere ejecutar AdminOps como administrador.".into())
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    /// ok | warn | bad
    pub level: String,
    pub text: String,
}

/// Lo que devuelve Repair-Volume, en español.
pub fn repair_status(raw: &str, fixing: bool) -> CheckResult {
    let r = raw.trim();
    let c = |level: &str, text: &str| CheckResult { level: level.into(), text: text.into() };
    match r {
        "NoErrorsFound" => c("ok", "Sin errores en el sistema de archivos."),
        "ErrorsFixed" | "Fixed" => c("ok", "Se encontraron errores y se repararon."),
        "SpotFixNeeded" | "FullRepairNeeded" | "ScanNeeded" if !fixing => c("warn", "Hay errores en el sistema de archivos: pulsa «Reparar sistema de archivos»."),
        "SpotFixNeeded" | "FullRepairNeeded" | "ScanNeeded" => c("bad", "No se pudieron reparar todos los errores. Si el disco tiene sectores dañados, copia antes lo importante."),
        _ if r.is_empty() => c("ok", "Comprobación terminada."),
        other => c("warn", &format!("Resultado: {other}")),
    }
}

/// Comprueba el sistema de archivos sin cambiar nada (chkdsk /scan, en línea).
#[tauri::command(async)]
pub fn disk_check(letter: String) -> Result<CheckResult, String> {
    let l = letter_ok(&letter)?;
    need_admin()?;
    let out = crate::ps::powershell_opts(&format!("\"$(Repair-Volume -DriveLetter {l} -Scan)\""), crate::ps::Opts { timeout: Some(Duration::from_secs(30 * 60)), task: None })?;
    Ok(repair_status(&out, false))
}

/// Repara el sistema de archivos (chkdsk /f). En el disco de Windows se hace al
/// reiniciar (con Windows en marcha no se puede); en los demás, ahora mismo.
#[tauri::command(async)]
pub fn disk_repair(letter: String, tweaks: State<'_, TweakState>) -> Result<CheckResult, String> {
    let l = letter_ok(&letter)?;
    need_admin()?;
    let result = if is_system(l) {
        // Marca el volumen para que Windows lo revise y repare al arrancar (autochk).
        crate::ps::exec("fsutil.exe", &["dirty", "set", &format!("{l}:")])
            .map(|_| CheckResult { level: "warn".into(), text: format!("{l}: es el disco de Windows: se revisará y reparará al reiniciar el equipo (tarda unos minutos antes de que arranque).") })
    } else {
        crate::ps::powershell_opts(&format!("\"$(Repair-Volume -DriveLetter {l} -OfflineScanAndFix)\""), crate::ps::Opts { timeout: Some(Duration::from_secs(2 * 3600)), task: None }).map(|out| repair_status(&out, true))
    };
    let journal = result.as_ref().map(|_| ()).map_err(Clone::clone);
    tweaks.record(Op::Run, &format!("Reparar sistema de archivos de {l}:"), &journal);
    result
}

/// Porcentaje de la última línea de progreso de chkdsk («… 37 percent», «37 %»).
pub fn percent_in(text: &str) -> Option<u32> {
    let bytes = text.as_bytes();
    let mut best = None;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let rest = text[i..].trim_start();
            if rest.starts_with('%') || rest.to_lowercase().starts_with("percent") || rest.to_lowercase().starts_with("por ciento") {
                if let Ok(n) = text[start..i].parse::<u32>() {
                    if n <= 100 {
                        best = Some(n);
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    best
}

/// Busca sectores dañados en toda la superficie (chkdsk /r). Solo en discos
/// que no son el de Windows. Tarda mucho (horas en discos grandes).
#[tauri::command(async)]
pub fn disk_surface_scan(app: tauri::AppHandle, letter: String, tweaks: State<'_, TweakState>) -> Result<CheckResult, String> {
    let l = letter_ok(&letter)?;
    need_admin()?;
    if is_system(l) {
        return Err("En el disco de Windows no se puede con Windows en marcha. Usa «Reparar sistema de archivos» (se hace al reiniciar).".into());
    }
    let task = crate::task::Task::new(&app, format!("disk-surface:{l}")).named(format!("Buscar sectores dañados en {l}:"));
    task.step("Empezando… (el disco se desconecta un momento de Windows)");
    let mut child = crate::ps::hidden("chkdsk.exe")
        .args([&format!("{l}:"), "/r", "/x"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("No se pudo iniciar chkdsk: {e}"))?;
    let mut out = child.stdout.take().ok_or("Sin salida de chkdsk")?;
    let (tx, rx) = std::sync::mpsc::channel::<u32>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 512];
        let mut tail = String::new();
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 {
                break;
            }
            tail.push_str(&String::from_utf8_lossy(&buf[..n]));
            if tail.len() > 400 {
                tail = tail[tail.len() - 200..].to_string();
            }
            if let Some(p) = percent_in(&tail) {
                let _ = tx.send(p);
            }
        }
    });
    let mut last = None;
    let code = loop {
        while let Ok(p) = rx.try_recv() {
            if last != Some(p) {
                last = Some(p);
                task.step(format!("Revisando la superficie… {p} %"));
            }
        }
        if let Ok(Some(status)) = child.try_wait() {
            break status.code();
        }
        if task.cancelled() {
            let _ = child.kill();
            let r: Result<(), String> = Err("Cancelado".into());
            tweaks.record(Op::Run, &format!("Buscar sectores dañados en {l}:"), &r);
            return Err("Cancelado. El disco vuelve a estar disponible en unos segundos.".into());
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    // chkdsk: 0 sin errores, 1 corregidos, 2 limpieza, 3 no se pudo comprobar o quedaron errores.
    let result = match code {
        Some(0) => Ok(CheckResult { level: "ok".into(), text: "Sin sectores dañados ni errores.".into() }),
        Some(1) | Some(2) => Ok(CheckResult { level: "warn".into(), text: "Se encontraron y corrigieron errores; si había sectores dañados, quedaron apartados. Mira el veredicto del disco: si tiene sectores dañados, cámbialo.".into() }),
        Some(c) => Err(format!("chkdsk no pudo terminar la revisión (código {c}). Si el disco está en uso por otro programa, ciérralo y vuelve a intentarlo.")),
        None => Err("chkdsk se detuvo.".into()),
    };
    let journal = result.as_ref().map(|_| ()).map_err(Clone::clone);
    tweaks.record(Op::Run, &format!("Buscar sectores dañados en {l}:"), &journal);
    result
}

// ---------- Rescatar archivos ----------

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RescueResult {
    pub copied: u64,
    /// Archivos que no se pudieron leer (se quedaron en el disco).
    pub failed: Vec<String>,
    pub bytes: u64,
}

/// Rutas con error del registro de robocopy («… ERROR 23 (0x00000017) … X:\ruta»).
pub fn failed_paths(log: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in log.lines() {
        let Some(i) = line.find("(0x") else { continue };
        let rest = &line[i..];
        let Some(close) = rest.find(')') else { continue };
        let after = &rest[close + 1..];
        let Some(colon) = after.find(":\\") else { continue };
        if colon == 0 {
            continue;
        }
        let path = after[colon - 1..].trim().to_string();
        if !path.ends_with('\\') && !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

fn inside(child: &std::path::Path, parent: &std::path::Path) -> bool {
    let n = |p: &std::path::Path| p.display().to_string().to_lowercase().trim_end_matches('\\').to_string() + "\\";
    n(child).starts_with(&n(parent))
}

/// Copia todo lo que se pueda leer de `source` a `dest`, saltando enseguida lo
/// ilegible (un reintento de 1 s en vez de los millones por defecto) y diciendo
/// qué se quedó. Es lo primero que hay que hacer con un disco que falla.
#[tauri::command(async)]
pub fn disk_rescue(app: tauri::AppHandle, source: String, dest: String, tweaks: State<'_, TweakState>) -> Result<RescueResult, String> {
    let (src, dst) = (std::path::PathBuf::from(source.trim()), std::path::PathBuf::from(dest.trim()));
    if !src.is_dir() {
        return Err("La carpeta de origen no existe o no se puede abrir.".into());
    }
    if dst.as_os_str().is_empty() || inside(&dst, &src) || inside(&src, &dst) {
        return Err("El destino tiene que ser otra carpeta, fuera de la de origen (mejor en otro disco).".into());
    }
    std::fs::create_dir_all(&dst).map_err(|e| format!("No se pudo crear el destino: {e}"))?;
    let log = std::env::temp_dir().join(format!("adminops-rescate-{}.log", std::process::id()));
    let task = crate::task::Task::new(&app, "disk-rescue").named("Rescatar archivos");
    task.step("Copiando lo que se puede leer…");
    let mut child = crate::ps::hidden("robocopy.exe")
        .arg(&src)
        .arg(&dst)
        .args(["/E", "/COPY:DAT", "/DCOPY:T", "/R:1", "/W:1", "/XJ", "/NP", "/NDL", "/NJH", "/BYTES", "/TEE"])
        .arg(format!("/UNILOG:{}", log.display()))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("No se pudo iniciar la copia: {e}"))?;
    let mut out = child.stdout.take().ok_or("Sin salida de la copia")?;
    let (tx, rx) = std::sync::mpsc::channel::<u64>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut lines = 0u64;
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 {
                break;
            }
            lines += buf[..n].iter().filter(|b| **b == b'\n').count() as u64;
            let _ = tx.send(lines);
        }
    });
    let mut shown = 0;
    let code = loop {
        while let Ok(n) = rx.try_recv() {
            if n / 25 != shown / 25 {
                task.step(format!("Copiando… {n} archivos revisados"));
            }
            shown = n;
        }
        if let Ok(Some(status)) = child.try_wait() {
            break status.code().unwrap_or(16);
        }
        if task.cancelled() {
            let _ = child.kill();
            let _ = std::fs::remove_file(&log);
            return Err("Copia cancelada. Lo ya copiado queda en el destino.".into());
        }
        std::thread::sleep(Duration::from_millis(400));
    };
    let text = std::fs::read(&log).map(|b| {
        // /UNILOG escribe UTF-16 LE.
        let u: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        String::from_utf16_lossy(&u)
    });
    let _ = std::fs::remove_file(&log);
    let failed = text.as_deref().map(failed_paths).unwrap_or_default();
    let result = if code >= 16 {
        Err("La copia no pudo empezar: revisa que el origen se lea y que haya espacio en el destino.".into())
    } else {
        let bytes = walk_size(&dst);
        Ok(RescueResult { copied: bytes.1, bytes: bytes.0, failed })
    };
    let journal = result.as_ref().map(|_| ()).map_err(Clone::clone);
    tweaks.record(Op::Run, "Rescatar archivos de un disco", &journal);
    result
}

/// Tamaño y número de archivos de lo que quedó en el destino.
fn walk_size(dir: &std::path::Path) -> (u64, u64) {
    let (mut bytes, mut files) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(_) => {
                    files += 1;
                    bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                }
                Err(_) => {}
            }
        }
    }
    (bytes, files)
}

#[tauri::command(async)]
pub fn disk_pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk() -> Disk {
        Disk { model: "WD Elements".into(), bus: "USB".into(), media: "HDD".into(), health: "Healthy".into(), temperature: 35, hours: 1200, read_errors: 0, write_errors: 0, wear: -1, ..Default::default() }
    }

    /// Lo que se estropea (la superficie) manda sobre lo que se arregla (el índice).
    #[test]
    fn verdict_puts_data_loss_first() {
        assert_eq!(verdict(&disk()).level, "ok");
        let dirty = Disk { volumes: vec![Volume { letter: "E".into(), dirty: Some(true), ..Default::default() }], ..disk() };
        let r = verdict(&dirty);
        assert_eq!(r.level, "warn");
        assert!(r.title.contains("E:"), "{}", r.title);
        assert!(r.advice[0].contains("Reparar sistema de archivos"));

        let failing = Disk { pending: Some(12), ..dirty.clone() };
        let r = verdict(&failing);
        assert_eq!(r.level, "bad");
        assert!(r.text.contains("12 sectores"), "{}", r.text);
        assert!(r.advice[0].contains("Rescatar archivos"), "primero copiar");
        assert!(r.advice[1].contains("No pases"), "y no chkdsk /r antes");

        let cable = Disk { crc_errors: Some(40), ..disk() };
        let r = verdict(&cable);
        assert!(r.title.contains("conexión"));
        assert!(r.advice[0].contains("cable USB"), "externo: el cable y la caja");

        let tired = Disk { reallocated: Some(8), ..disk() };
        assert!(verdict(&tired).title.contains("8 apartados"));
        assert_eq!(verdict(&Disk { media: "SSD".into(), wear: 95, ..disk() }).level, "warn");
        assert_eq!(verdict(&Disk { health: "Unhealthy".into(), ..disk() }).level, "bad");
    }

    /// La foto de cada día se apunta una vez; lo que sube se dice con cifras y
    /// hace que un disco «sano» pase a «va a peor».
    #[test]
    fn trend_catches_a_disk_getting_worse() {
        let mut h = History::new();
        let mut d = vec![Disk { reallocated: Some(8), pending: Some(0), ..disk() }];
        assert!(record(&mut h, &mut d, 100).is_empty(), "la primera foto no compara con nada");
        d[0].reallocated = Some(40);
        d[0].pending = Some(3);
        let news = record(&mut h, &mut d, 107);
        assert_eq!(news[0].1, ["Sectores apartados: 8 → 40 en 7 días", "Sectores pendientes: 0 → 3 en 7 días"]);
        assert_eq!(d[0].rising.len(), 2);
        // El mismo día otra vez: no se duplica la foto.
        record(&mut h, &mut d, 107);
        assert_eq!(h.values().next().unwrap().len(), 2);
        d[0].verdict = verdict(&Disk { pending: None, ..d[0].clone() });
        apply_trend(&mut d[0]);
        assert_eq!(d[0].verdict.title, "Va a peor");
        // Sin cambios: nada que decir.
        let quieto = Point { day: 1, reallocated: Some(5), ..Default::default() };
        assert!(rising(&quieto, &Point { day: 9, ..quieto.clone() }).is_empty());
    }

    #[test]
    fn bitlocker_states() {
        assert_eq!(bitlocker_state("On|FullyEncrypted"), "on");
        assert_eq!(bitlocker_state("Off|FullyEncrypted"), "suspended");
        assert_eq!(bitlocker_state("Off|EncryptionInProgress"), "encrypting");
        assert_eq!(bitlocker_state("Off|FullyDecrypted"), "");
        assert_eq!(bitlocker_state(""), "");
    }

    #[test]
    fn chkdsk_progress_in_any_language() {
        assert_eq!(percent_in("Stage 4: Looking for bad clusters in user file data ...\r  37 percent complete."), Some(37));
        assert_eq!(percent_in("Fase 4: buscando clústeres defectuosos...\r Progreso: 12 de 900 hecho; fase:  58 %; total:  40 %"), Some(40));
        assert_eq!(percent_in("Sin números"), None);
        assert_eq!(percent_in("250 %"), None);
    }

    #[test]
    fn robocopy_failures_are_listed() {
        let log = "\
\t    Nuevo archivo  \t\t  1024\tE:\\Fotos\\a.jpg\r\n\
2026/09/30 10:00:01 ERROR 23 (0x00000017) Copiando archivo E:\\Fotos\\b.jpg\r\n\
Error de datos (comprobación de redundancia cíclica).\r\n\
2026/09/30 10:00:05 ERROR 23 (0x00000017) Copying File E:\\Docs\\informe final.docx\r\n\
2026/09/30 10:00:06 ERROR 5 (0x00000005) Scanning Source Directory E:\\System Volume Information\\\r\n";
        assert_eq!(failed_paths(log), [r"E:\Fotos\b.jpg", r"E:\Docs\informe final.docx"]);
    }

    #[test]
    fn rescue_paths_cannot_overlap() {
        use std::path::Path;
        assert!(inside(Path::new(r"E:\Fotos\copia"), Path::new(r"E:\Fotos")));
        assert!(!inside(Path::new(r"D:\Rescate"), Path::new(r"E:\Fotos")));
        assert!(!inside(Path::new(r"E:\Fotos2"), Path::new(r"E:\Fotos")));
    }

    #[test]
    fn drive_letters() {
        assert_eq!(letter_ok("e").unwrap(), 'E');
        assert_eq!(letter_ok("E:\\").unwrap(), 'E');
        assert!(letter_ok("EE").is_err());
        assert!(letter_ok("1").is_err());
        assert!(letter_ok("").is_err());
    }

    #[test]
    fn repair_results_in_spanish() {
        assert_eq!(repair_status("NoErrorsFound", false).level, "ok");
        assert_eq!(repair_status("SpotFixNeeded", false).level, "warn");
        assert_eq!(repair_status("FullRepairNeeded", true).level, "bad");
    }

    #[test]
    fn embedded_script_parses() {
        let e = crate::ps::parse_errors(STATUS_SCRIPT);
        assert!(e.is_empty(), "{e}");
    }
}
