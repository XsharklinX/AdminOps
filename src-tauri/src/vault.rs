//! Caja fuerte: un disco virtual (.vhdx) cifrado con BitLocker y contraseña.
//! Es un formato de Windows: se abre con doble clic (Windows pide la
//! contraseña) aunque AdminOps ya no esté instalado.
//!
//! Carpeta cifrada: para Windows Home (sin BitLocker), un .zip con AES-256
//! que abren 7-Zip, WinRAR o el propio AdminOps.
//!
//! Las contraseñas nunca se guardan ni van al registro: los scripts que las
//! llevan empiezan por una línea neutra (es la que se anota) y las reciben en
//! base64 (ver `ps::text_var`).

use crate::ps::text_var;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

static FILE_LOCK: Mutex<()> = Mutex::new(());

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn check_password(pw: &str) -> Result<(), String> {
    if pw.chars().count() < 8 {
        return Err("La contraseña debe tener al menos 8 caracteres.".into());
    }
    if pw.chars().count() > 128 || pw.chars().any(char::is_control) {
        return Err("La contraseña no es válida (máximo 128 caracteres, sin saltos de línea).".into());
    }
    Ok(())
}

// ---------- Disponibilidad ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Support {
    /// Se puede crear una caja fuerte con BitLocker (Windows Pro, Enterprise, Education).
    pub bitlocker: bool,
    pub edition: String,
    pub elevated: bool,
}

fn edition() -> String {
    crate::tweaks::registry::read_string(r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "EditionID").unwrap_or_default()
}

#[tauri::command]
pub fn vault_support() -> Support {
    let edition = edition();
    // Home: "Core", "CoreSingleLanguage", "CoreN", "CoreCountrySpecific".
    let bitlocker = !edition.is_empty() && !edition.starts_with("Core");
    Support { bitlocker, edition, elevated: crate::elevation::is_elevated() }
}

// ---------- Cajas fuertes registradas ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Vault {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size_gb: u32,
    pub created: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    #[serde(flatten)]
    pub vault: Vault,
    pub exists: bool,
    pub mounted: bool,
    /// Letter with colon ("S:") when mounted.
    pub letter: Option<String>,
    pub unlocked: bool,
    /// Tamaño actual del archivo (crece según se llena).
    pub file_size: u64,
}

fn vaults_path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("vaults.json")
}

fn load(app: &tauri::AppHandle) -> Vec<Vault> {
    crate::paths::read_json(&vaults_path(app))
}

fn find(app: &tauri::AppHandle, id: &str) -> Result<Vault, String> {
    load(app).into_iter().find(|v| v.id == id).ok_or_else(|| "Esa caja fuerte ya no está en la lista.".into())
}

fn save_list(app: &tauri::AppHandle, list: &[Vault]) -> Result<(), String> {
    crate::paths::write_json(&vaults_path(app), &list)
}

const STATUS_SCRIPT: &str = r#"
$r = @(foreach ($p in @(ConvertFrom-Json $json)) {
  $o = [ordered]@{ path = $p; mounted = $false; letter = $null; unlocked = $false }
  if (Test-Path -LiteralPath $p) {
    $img = Get-DiskImage -ImagePath $p -ErrorAction SilentlyContinue
    if ($img -and $img.Attached) {
      $o.mounted = $true
      $part = $img | Get-Disk -ErrorAction SilentlyContinue | Get-Partition -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter -and $_.DriveLetter -ne [char]0 } | Select-Object -First 1
      if ($part) {
        $o.letter = "$($part.DriveLetter):"
        $bl = Get-BitLockerVolume -MountPoint $o.letter -ErrorAction SilentlyContinue
        $o.unlocked = (-not $bl) -or ("$($bl.LockStatus)" -ne 'Locked')
      }
    }
  }
  [pscustomobject]$o
})
ConvertTo-Json -InputObject $r -Compress
"#;

#[derive(Deserialize)]
struct RawStatus {
    path: String,
    mounted: bool,
    letter: Option<String>,
    unlocked: bool,
}

#[tauri::command(async)]
pub fn vault_list(app: tauri::AppHandle) -> Result<Vec<VaultStatus>, String> {
    let list = load(&app);
    if list.is_empty() {
        return Ok(vec![]);
    }
    let paths: Vec<&str> = list.iter().map(|v| v.path.as_str()).collect();
    let script = format!("{}{STATUS_SCRIPT}", text_var("json", &serde_json::to_string(&paths).unwrap_or_default()));
    let out = crate::pspool::query(&script, Some(Duration::from_secs(40)), "Caja fuerte: estado")?;
    let raw: Vec<RawStatus> = serde_json::from_str(out.trim()).unwrap_or_default();
    Ok(list
        .into_iter()
        .map(|v| {
            let st = raw.iter().find(|r| r.path == v.path);
            let file = std::fs::metadata(&v.path).ok();
            VaultStatus {
                exists: file.is_some(),
                file_size: file.map_or(0, |m| m.len()),
                mounted: st.is_some_and(|s| s.mounted),
                letter: st.and_then(|s| s.letter.clone()),
                unlocked: st.is_some_and(|s| s.unlocked && s.letter.is_some()),
                vault: v,
            }
        })
        .collect())
}

// ---------- Crear ----------

/// Crea un .vhdx dinámico (crece según se llena) con la API de Windows: no
/// necesita Hyper-V ni depende de la codificación de diskpart.
#[cfg(windows)]
fn create_vhdx(path: &Path, bytes: u64) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Storage::Vhd::{
        CreateVirtualDisk, CREATE_VIRTUAL_DISK_FLAG_NONE, CREATE_VIRTUAL_DISK_PARAMETERS, CREATE_VIRTUAL_DISK_PARAMETERS_0_1, CREATE_VIRTUAL_DISK_VERSION_2,
        VIRTUAL_DISK_ACCESS_NONE, VIRTUAL_STORAGE_TYPE, VIRTUAL_STORAGE_TYPE_DEVICE_VHDX, VIRTUAL_STORAGE_TYPE_VENDOR_MICROSOFT,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let kind = VIRTUAL_STORAGE_TYPE { DeviceId: VIRTUAL_STORAGE_TYPE_DEVICE_VHDX, VendorId: VIRTUAL_STORAGE_TYPE_VENDOR_MICROSOFT };
    let mut params = CREATE_VIRTUAL_DISK_PARAMETERS { Version: CREATE_VIRTUAL_DISK_VERSION_2, ..Default::default() };
    params.Anonymous.Version2 = CREATE_VIRTUAL_DISK_PARAMETERS_0_1 { MaximumSize: bytes, ..Default::default() };
    let mut handle = std::ptr::null_mut();
    let r = unsafe {
        CreateVirtualDisk(&kind, wide.as_ptr(), VIRTUAL_DISK_ACCESS_NONE, std::ptr::null_mut(), CREATE_VIRTUAL_DISK_FLAG_NONE, 0, &params, std::ptr::null(), &mut handle)
    };
    if r != 0 {
        return Err(format!("No se pudo crear el disco virtual (error {r}): {}", std::io::Error::from_raw_os_error(r as i32)));
    }
    unsafe { CloseHandle(handle) };
    Ok(())
}

#[cfg(not(windows))]
fn create_vhdx(_: &Path, _: u64) -> Result<(), String> {
    Err("Solo disponible en Windows.".into())
}

const CREATE_SCRIPT: &str = r#"
$disk = Mount-DiskImage -ImagePath $path -StorageType VHDX -PassThru | Get-Disk
Initialize-Disk -Number $disk.Number -PartitionStyle GPT -ErrorAction Stop
# Sin letra hasta formatear: así el Explorador no ofrece "formatear el disco".
$part = New-Partition -DiskNumber $disk.Number -UseMaximumSize
Format-Volume -Partition $part -FileSystem NTFS -NewFileSystemLabel $label -Confirm:$false | Out-Null
$part | Add-PartitionAccessPath -AssignDriveLetter
$letter = "$((Get-Partition -DiskNumber $disk.Number -PartitionNumber $part.PartitionNumber).DriveLetter):"
$sec = ConvertTo-SecureString $pw -AsPlainText -Force
Enable-BitLocker -MountPoint $letter -EncryptionMethod XtsAes256 -UsedSpaceOnly -PasswordProtector -Password $sec | Out-Null
Add-BitLockerKeyProtector -MountPoint $letter -RecoveryPasswordProtector | Out-Null
$t = [Diagnostics.Stopwatch]::StartNew()
do { Start-Sleep -Milliseconds 700; $v = Get-BitLockerVolume -MountPoint $letter } while ("$($v.VolumeStatus)" -ne 'FullyEncrypted' -and $t.Elapsed.TotalMinutes -lt 5)
$key = @($v.KeyProtector | Where-Object { "$($_.KeyProtectorType)" -eq 'RecoveryPassword' })[0].RecoveryPassword
[pscustomobject]@{ letter = $letter; recovery = "$key" } | ConvertTo-Json -Compress
"#;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub id: String,
    pub letter: String,
    /// Clave de recuperación de 48 dígitos: se muestra una vez y no se guarda.
    pub recovery: String,
}

#[tauri::command(async)]
pub fn vault_create(app: tauri::AppHandle, tweaks: State<'_, TweakState>, name: String, path: String, size_gb: u32, password: String) -> Result<Created, String> {
    if !vault_support().bitlocker {
        return Err("Esta edición de Windows no puede crear unidades con BitLocker. Usa la carpeta cifrada.".into());
    }
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    check_password(&password)?;
    if !(1..=2048).contains(&size_gb) {
        return Err("El tamaño debe estar entre 1 y 2048 GB.".into());
    }
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(32).collect();
    let name = if name.is_empty() { "Caja fuerte".to_string() } else { name };
    let file = PathBuf::from(path.trim());
    if !file.extension().is_some_and(|e| e.eq_ignore_ascii_case("vhdx")) || !file.is_absolute() {
        return Err("Elige dónde guardar el archivo .vhdx.".into());
    }
    if file.exists() {
        return Err("Ya existe un archivo con ese nombre.".into());
    }
    let task = crate::task::Task::new(&app, "vault").named("Crear caja fuerte");
    task.step("Creando el disco virtual…");
    create_vhdx(&file, size_gb as u64 * 1024 * 1024 * 1024)?;
    task.step("Formateando y cifrando con BitLocker…");
    // La etiqueta NTFS admite 32 caracteres.
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n{}{}{}{CREATE_SCRIPT}",
        text_var("path", &file.display().to_string()),
        text_var("label", &name),
        text_var("pw", &password)
    );
    let result = crate::ps::powershell_opts(&script, task.opts(Some(Duration::from_secs(420))));
    let out = match result {
        Ok(o) => o,
        Err(e) => {
            // No dejar un disco a medio hacer: desmontar y borrar.
            let _ = crate::pspool::query(&format!("{}Dismount-DiskImage -ImagePath $path -ErrorAction SilentlyContinue | Out-Null", text_var("path", &file.display().to_string())), Some(Duration::from_secs(30)), "Caja fuerte: deshacer");
            let _ = std::fs::remove_file(&file);
            tweaks.record(Op::Run, "Caja fuerte: crear", &Err::<(), _>(e.clone()));
            return Err(e);
        }
    };
    #[derive(Deserialize)]
    struct Out {
        letter: String,
        recovery: String,
    }
    let o: Out = serde_json::from_str(out.trim()).map_err(|e| format!("Respuesta inesperada: {e}"))?;
    let id = format!("v{:x}", now());
    {
        let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut list = load(&app);
        list.push(Vault { id: id.clone(), name: name.clone(), path: file.display().to_string(), size_gb, created: now() });
        save_list(&app, &list)?;
    }
    tweaks.record(Op::Run, &format!("Caja fuerte «{name}» creada ({size_gb} GB)"), &Ok::<(), String>(()));
    let _ = std::process::Command::new("explorer.exe").arg(format!("{}\\", o.letter)).spawn();
    Ok(Created { id, letter: o.letter, recovery: o.recovery })
}

// ---------- Abrir, cerrar, contraseña ----------

const OPEN_SCRIPT: &str = r#"
$img = Get-DiskImage -ImagePath $path
$attachedHere = $false
if (-not $img.Attached) { $img = Mount-DiskImage -ImagePath $path -PassThru; $attachedHere = $true; Start-Sleep -Milliseconds 800 }
$disk = $img | Get-Disk
$part = Get-Partition -DiskNumber $disk.Number | Where-Object { $_.Type -eq 'Basic' } | Select-Object -First 1
if (-not $part) { throw 'El disco virtual no tiene una partición de datos.' }
if (-not $part.DriveLetter -or $part.DriveLetter -eq [char]0) {
  $part | Add-PartitionAccessPath -AssignDriveLetter
  $part = Get-Partition -DiskNumber $disk.Number -PartitionNumber $part.PartitionNumber
}
$letter = "$($part.DriveLetter):"
$bl = Get-BitLockerVolume -MountPoint $letter -ErrorAction SilentlyContinue
if ($bl -and "$($bl.LockStatus)" -eq 'Locked') {
  try {
    Unlock-BitLocker -MountPoint $letter -Password (ConvertTo-SecureString $pw -AsPlainText -Force) -ErrorAction Stop | Out-Null
  } catch {
    if ($attachedHere) { Dismount-DiskImage -ImagePath $path -ErrorAction SilentlyContinue | Out-Null }
    throw 'Contraseña incorrecta.'
  }
}
[pscustomobject]@{ letter = $letter } | ConvertTo-Json -Compress
"#;

fn open_inner(v: &Vault, password: &str) -> Result<String, String> {
    if !Path::new(&v.path).is_file() {
        return Err("No se encuentra el archivo de la caja fuerte (¿se movió o está en un USB desconectado?).".into());
    }
    let script = format!("$ErrorActionPreference = 'Stop'\n{}{}{OPEN_SCRIPT}", text_var("path", &v.path), text_var("pw", password));
    let out = crate::pspool::query(&script, Some(Duration::from_secs(90)), "Caja fuerte: abrir")?;
    #[derive(Deserialize)]
    struct Out {
        letter: String,
    }
    serde_json::from_str::<Out>(out.trim()).map(|o| o.letter).map_err(|e| format!("Respuesta inesperada: {e}"))
}

#[tauri::command(async)]
pub fn vault_open(app: tauri::AppHandle, id: String, password: String) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador (o abre el archivo .vhdx con doble clic).".into());
    }
    let v = find(&app, &id)?;
    let letter = open_inner(&v, &password)?;
    let _ = std::process::Command::new("explorer.exe").arg(format!("{letter}\\")).spawn();
    log::info!("Caja fuerte «{}» abierta en {letter}", v.name);
    Ok(letter)
}

const CLOSE_SCRIPT: &str = r#"
$img = Get-DiskImage -ImagePath $path -ErrorAction SilentlyContinue
if (-not $img -or -not $img.Attached) { return }
$part = $img | Get-Disk | Get-Partition | Where-Object { $_.DriveLetter -and $_.DriveLetter -ne [char]0 } | Select-Object -First 1
if ($part) {
  $letter = "$($part.DriveLetter):"
  $bl = Get-BitLockerVolume -MountPoint $letter -ErrorAction SilentlyContinue
  if ($bl -and "$($bl.LockStatus)" -ne 'Locked') {
    try { Lock-BitLocker -MountPoint $letter -ErrorAction Stop | Out-Null }
    catch { throw 'Hay archivos abiertos en la caja fuerte: ciérralos y vuelve a intentarlo.' }
  }
}
Dismount-DiskImage -ImagePath $path | Out-Null
"#;

#[tauri::command(async)]
pub fn vault_close(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let v = find(&app, &id)?;
    let script = format!("$ErrorActionPreference = 'Stop'\n{}{CLOSE_SCRIPT}", text_var("path", &v.path));
    crate::pspool::query(&script, Some(Duration::from_secs(60)), "Caja fuerte: cerrar").map(|_| ())
}

const PASSWORD_SCRIPT: &str = r#"
$old = @((Get-BitLockerVolume -MountPoint $letter).KeyProtector | Where-Object { "$($_.KeyProtectorType)" -eq 'Password' })
Add-BitLockerKeyProtector -MountPoint $letter -PasswordProtector -Password (ConvertTo-SecureString $pw -AsPlainText -Force) | Out-Null
foreach ($k in $old) { Remove-BitLockerKeyProtector -MountPoint $letter -KeyProtectorId $k.KeyProtectorId | Out-Null }
"#;

#[tauri::command(async)]
pub fn vault_change_password(app: tauri::AppHandle, tweaks: State<'_, TweakState>, id: String, old: String, new: String) -> Result<(), String> {
    check_password(&new)?;
    let v = find(&app, &id)?;
    // Abrirla con la contraseña actual comprueba que es la buena.
    let letter = open_inner(&v, &old)?;
    let script = format!("$ErrorActionPreference = 'Stop'\n{}{}{PASSWORD_SCRIPT}", text_var("letter", &letter), text_var("pw", &new));
    let r = crate::pspool::query(&script, Some(Duration::from_secs(60)), "Caja fuerte: cambiar contraseña").map(|_| ());
    tweaks.record(Op::Run, &format!("Caja fuerte «{}»: contraseña cambiada", v.name), &r);
    r
}

/// Clave de recuperación (la caja debe estar abierta).
#[tauri::command(async)]
pub fn vault_recovery_key(app: tauri::AppHandle, id: String, password: String) -> Result<String, String> {
    let v = find(&app, &id)?;
    let letter = open_inner(&v, &password)?;
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n{}@((Get-BitLockerVolume -MountPoint $letter).KeyProtector | Where-Object {{ \"$($_.KeyProtectorType)\" -eq 'RecoveryPassword' }})[0].RecoveryPassword",
        text_var("letter", &letter)
    );
    let key = crate::pspool::query(&script, Some(Duration::from_secs(30)), "Caja fuerte: clave de recuperación")?;
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Esta caja fuerte no tiene clave de recuperación.".into());
    }
    Ok(key)
}

/// Guarda la clave de recuperación en un .txt donde elija el usuario (mejor un USB).
#[tauri::command(async)]
pub fn vault_save_recovery(app: tauri::AppHandle, name: String, key: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    if !key.chars().all(|c| c.is_ascii_digit() || c == '-') || key.len() < 40 {
        return Err("Clave no válida.".into());
    }
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let Some(file) = app.dialog().file().set_file_name(format!("Clave de recuperación - {safe}.txt")).add_filter("Texto", &["txt"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let text = format!(
        "CLAVE DE RECUPERACIÓN DE LA CAJA FUERTE «{name}»\r\nFecha: {}\r\n\r\nSi olvidas la contraseña, esta clave de 48 dígitos abre la caja fuerte.\r\nGuárdala fuera del equipo (USB, papel). Quien la tenga puede abrirla.\r\n\r\n{key}\r\n",
        chrono::Local::now().format("%d/%m/%Y %H:%M")
    );
    std::fs::write(&file, text).map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(Some(file.display().to_string()))
}

/// Añade a la lista una caja fuerte que ya existe (otro equipo, un USB…).
#[tauri::command(async)]
pub fn vault_add_existing(app: tauri::AppHandle) -> Result<Option<Vault>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(file) = app.dialog().file().add_filter("Disco virtual", &["vhdx", "vhd"]).blocking_pick_file().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    let path = file.display().to_string();
    if let Some(v) = list.iter().find(|v| v.path.eq_ignore_ascii_case(&path)) {
        return Ok(Some(v.clone()));
    }
    let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    let v = Vault {
        id: format!("v{:x}", now()),
        name: file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Caja fuerte".into()),
        size_gb: (size / (1024 * 1024 * 1024)) as u32,
        path,
        created: now(),
    };
    list.push(v.clone());
    save_list(&app, &list)?;
    Ok(Some(v))
}

/// Dónde guardar una caja fuerte nueva.
#[tauri::command(async)]
pub fn vault_pick_location(app: tauri::AppHandle, name: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let safe: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let safe = if safe.trim().is_empty() { "Caja fuerte".to_string() } else { safe };
    app.dialog()
        .file()
        .set_file_name(format!("{safe}.vhdx"))
        .add_filter("Disco virtual", &["vhdx"])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vhdx")) { p } else { p.with_extension("vhdx") })
        .map(|p| p.display().to_string())
}

/// Quita la caja de la lista y, si se pide, borra el archivo (se cierra antes).
#[tauri::command(async)]
pub fn vault_remove(app: tauri::AppHandle, tweaks: State<'_, TweakState>, id: String, delete_file: bool) -> Result<(), String> {
    let v = find(&app, &id)?;
    if delete_file && Path::new(&v.path).exists() {
        vault_close(app.clone(), id.clone())?;
        let r = std::fs::remove_file(&v.path).map_err(|e| format!("No se pudo borrar el archivo: {e}"));
        tweaks.record(Op::Run, &format!("Caja fuerte «{}» eliminada", v.name), &r);
        r?;
    }
    let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load(&app);
    list.retain(|x| x.id != id);
    save_list(&app, &list)
}

// ---------- Carpeta cifrada (.zip AES-256) ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipResult {
    pub path: String,
    pub files: u64,
    pub bytes: u64,
}

fn free_name(base: PathBuf) -> PathBuf {
    if !base.exists() {
        return base;
    }
    let stem = base.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = base.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..1000).map(|i| base.with_file_name(format!("{stem} ({i}){ext}"))).find(|p| !p.exists()).unwrap_or(base)
}

/// Archivos de `root` (sin seguir enlaces ni uniones), con su ruta relativa.
fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, String, bool)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        if ft.is_dir() {
            out.push((path.clone(), format!("{rel}/"), true));
            walk(root, &path, out)?;
        } else if ft.is_file() {
            out.push((path, rel, false));
        }
    }
    Ok(())
}

#[tauri::command(async)]
pub fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[tauri::command(async)]
pub fn pick_encrypted_zip(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().add_filter("Carpeta cifrada", &["zip"]).blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

fn protected_folder(p: &Path) -> bool {
    let s = p.display().to_string().to_lowercase();
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()).to_lowercase();
    let profile = std::env::var("USERPROFILE").unwrap_or_default().to_lowercase();
    p.parent().is_none()
        || s.starts_with(&windows)
        || s.contains(r"\program files")
        || s.trim_end_matches('\\') == profile
        || s.trim_end_matches('\\').ends_with(r"\programdata")
}

/// Cifra una carpeta en un .zip con AES-256 junto a ella. Si `delete_original`,
/// borra la carpeta original de forma segura después de comprobar el .zip.
#[tauri::command(async)]
pub fn encrypt_folder(app: tauri::AppHandle, tweaks: State<'_, TweakState>, folder: String, password: String, delete_original: bool) -> Result<ZipResult, String> {
    check_password(&password)?;
    let src = PathBuf::from(folder.trim());
    if !src.is_dir() {
        return Err("Elige una carpeta.".into());
    }
    if protected_folder(&src) {
        return Err("Esa carpeta es del sistema o demasiado general: elige una carpeta concreta.".into());
    }
    let task = crate::task::Task::new(&app, "encrypt").named("Cifrar carpeta");
    task.step("Leyendo la carpeta…");
    let mut entries = Vec::new();
    walk(&src, &src, &mut entries).map_err(|e| format!("No se pudo leer la carpeta: {e}"))?;
    let total = entries.iter().filter(|e| !e.2).count() as u64;
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Carpeta".into());
    let out = free_name(src.with_file_name(format!("{name}.zip")));

    let write = || -> Result<u64, String> {
        let file = std::fs::File::create(&out).map_err(|e| format!("No se pudo crear el archivo: {e}"))?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true)
            .with_aes_encryption(zip::AesMode::Aes256, &password);
        let mut done = 0u64;
        let mut bytes = 0u64;
        let mut buf = vec![0u8; 1 << 20];
        for (path, rel, is_dir) in &entries {
            if task.cancelled() {
                return Err(crate::ps::CANCELLED_MSG.into());
            }
            if *is_dir {
                zip.add_directory(rel.as_str(), opts).map_err(|e| e.to_string())?;
                continue;
            }
            zip.start_file(rel.as_str(), opts).map_err(|e| e.to_string())?;
            let mut f = std::fs::File::open(path).map_err(|e| format!("No se pudo leer «{rel}»: {e}"))?;
            loop {
                let n = f.read(&mut buf).map_err(|e| format!("No se pudo leer «{rel}»: {e}"))?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                bytes += n as u64;
            }
            done += 1;
            if done.is_multiple_of(50) || done == total {
                task.step(format!("Cifrando… {done} de {total} archivos"));
            }
        }
        zip.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())?;
        Ok(bytes)
    };
    let bytes = match write() {
        Ok(b) => b,
        Err(e) => {
            let _ = std::fs::remove_file(&out);
            return Err(e);
        }
    };

    // Comprobar que se puede leer con la contraseña antes de tocar la original.
    task.step("Comprobando el archivo cifrado…");
    let verify = || -> Result<(), String> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if z.len() != entries.len() {
            return Err("El archivo cifrado no contiene todos los archivos.".into());
        }
        for i in 0..z.len() {
            let mut f = z.by_index_decrypt(i, password.as_bytes()).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut std::io::sink()).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    if let Err(e) = verify() {
        let _ = std::fs::remove_file(&out);
        return Err(format!("La comprobación del archivo cifrado falló, no se tocó la carpeta original: {e}"));
    }
    if delete_original {
        task.step("Borrando la carpeta original de forma segura…");
        crate::wipe::wipe_path(&src, &task)?;
    }
    tweaks.record(
        Op::Run,
        &format!("Carpeta cifrada: «{name}» ({total} archivos){}", if delete_original { ", original borrada" } else { "" }),
        &Ok::<(), String>(()),
    );
    Ok(ZipResult { path: out.display().to_string(), files: total, bytes })
}

/// Descifra un .zip de AdminOps (o cualquiera con AES o ZipCrypto) en una carpeta nueva junto a él.
#[tauri::command(async)]
pub fn decrypt_archive(app: tauri::AppHandle, path: String, password: String) -> Result<ZipResult, String> {
    let file = PathBuf::from(path.trim());
    let mut z = zip::ZipArchive::new(std::fs::File::open(&file).map_err(|e| format!("No se pudo abrir: {e}"))?).map_err(|_| "No es un archivo .zip válido.".to_string())?;
    let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Carpeta".into());
    let dest = free_name(file.with_file_name(&stem));
    let task = crate::task::Task::new(&app, "decrypt").named("Descifrar carpeta");
    let total = z.len();
    let mut files = 0u64;
    let mut bytes = 0u64;
    let run = |z: &mut zip::ZipArchive<std::fs::File>, files: &mut u64, bytes: &mut u64| -> Result<(), String> {
        for i in 0..total {
            if task.cancelled() {
                return Err(crate::ps::CANCELLED_MSG.into());
            }
            let mut f = match z.by_index_decrypt(i, password.as_bytes()) {
                Ok(f) => f,
                Err(zip::result::ZipError::InvalidPassword) => return Err("Contraseña incorrecta.".into()),
                Err(e) => return Err(e.to_string()),
            };
            // Nunca fuera de la carpeta de destino (rutas con "..").
            let Some(rel) = f.enclosed_name() else { continue };
            let target = dest.join(rel);
            if f.is_dir() {
                std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
            *bytes += std::io::copy(&mut f, &mut out).map_err(|e| match e.kind() {
                std::io::ErrorKind::InvalidData => "Contraseña incorrecta o archivo dañado.".to_string(),
                _ => e.to_string(),
            })?;
            *files += 1;
            if files.is_multiple_of(50) {
                task.step(format!("Descifrando… {files} archivos"));
            }
        }
        Ok(())
    };
    if let Err(e) = run(&mut z, &mut files, &mut bytes) {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(e);
    }
    let _ = std::process::Command::new("explorer.exe").arg(&dest).spawn();
    Ok(ZipResult { path: dest.display().to_string(), files, bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_and_paths() {
        assert!(check_password("corta").is_err());
        assert!(check_password("una contraseña larga").is_ok());
        assert!(check_password("con\nsalto de línea").is_err());
        assert!(protected_folder(Path::new(r"C:\")));
        assert!(protected_folder(Path::new(r"C:\Windows\System32")));
        assert!(protected_folder(Path::new(r"C:\Program Files\App")));
        assert!(!protected_folder(Path::new(r"D:\Clientes\Facturas")));
    }

    #[test]
    fn embedded_scripts_parse() {
        for (name, script) in [("STATUS", STATUS_SCRIPT), ("CREATE", CREATE_SCRIPT), ("OPEN", OPEN_SCRIPT), ("CLOSE", CLOSE_SCRIPT), ("PASSWORD", PASSWORD_SCRIPT)] {
            let errors = crate::ps::parse_errors(script);
            assert!(errors.is_empty(), "{name}: {errors}");
        }
    }

    #[test]
    fn encrypted_zip_roundtrip() {
        let base = std::env::temp_dir().join(format!("adminops-zip-{}", std::process::id()));
        let src = base.join("Facturas ñ");
        std::fs::create_dir_all(src.join("2026")).unwrap();
        std::fs::write(src.join("a.txt"), "hola").unwrap();
        std::fs::write(src.join("2026").join("b.bin"), vec![7u8; 300_000]).unwrap();
        let out = base.join("Facturas ñ.zip");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&out).unwrap());
            let opts = zip::write::SimpleFileOptions::default().with_aes_encryption(zip::AesMode::Aes256, "clave segura");
            let mut entries = Vec::new();
            walk(&src, &src, &mut entries).unwrap();
            assert_eq!(entries.len(), 3);
            for (p, rel, dir) in &entries {
                if *dir {
                    zip.add_directory(rel.as_str(), opts).unwrap();
                } else {
                    zip.start_file(rel.as_str(), opts).unwrap();
                    zip.write_all(&std::fs::read(p).unwrap()).unwrap();
                }
            }
            zip.finish().unwrap();
        }
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let bad = z.by_index_decrypt(z.index_for_name("a.txt").unwrap(), b"otra clave").map(|mut f| std::io::copy(&mut f, &mut std::io::sink()));
        assert!(!matches!(bad, Ok(Ok(_))), "una contraseña incorrecta no debe descifrar");
        let mut s = String::new();
        z.by_index_decrypt(z.index_for_name("a.txt").unwrap(), b"clave segura").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "hola");
        let _ = std::fs::remove_dir_all(&base);
    }
}
