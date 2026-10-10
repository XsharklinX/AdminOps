//! Pendrive de rescate arrancable: un Windows mínimo (WinPE) con el menú de
//! rescate de AdminOps, para los equipos que no arrancan. Copiar los datos de un
//! usuario, ver la salud de los discos, reparar el arranque, comprobar el disco
//! de Windows, quitar un driver que cuelga el equipo o desactivar Driver
//! Verifier, todo sin que el Windows dañado esté en marcha.
//!
//! Se crea con las herramientas oficiales de Microsoft (Windows ADK y su
//! complemento WinPE): copype, DISM y MakeWinPEMedia. La interfaz gráfica de
//! AdminOps necesita WebView2, que WinPE no trae: en el pendrive va el menú de
//! rescate en consola, que no depende de nada más.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Componentes de WinPE que necesita el menú (PowerShell y almacenamiento).
pub const COMPONENTS: &[&str] = &["WinPE-WMI", "WinPE-NetFX", "WinPE-Scripting", "WinPE-PowerShell", "WinPE-StorageWMI", "WinPE-DismCmdlets"];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UsbDisk {
    pub number: u32,
    pub name: String,
    pub size: u64,
    pub letters: Vec<String>,
    pub system: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RescueStatus {
    pub adk: bool,
    pub winpe: bool,
    pub kits_root: String,
    pub usb: Vec<UsbDisk>,
}

fn kits_root() -> Option<PathBuf> {
    crate::tweaks::registry::read_string(r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows Kits\Installed Roots", "KitsRoot10")
        .or_else(|| crate::tweaks::registry::read_string(r"HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots", "KitsRoot10"))
        .map(PathBuf::from)
}

fn adk_paths(root: &Path) -> (PathBuf, PathBuf) {
    let adk = root.join("Assessment and Deployment Kit");
    (adk.join(r"Deployment Tools\DandISetEnv.bat"), adk.join("Windows Preinstallation Environment"))
}

const USB_SCRIPT: &str = r#"
$sysDisk = (Get-Partition -DriveLetter $env:SystemDrive.TrimEnd(':') -ErrorAction SilentlyContinue).DiskNumber
$out = @(Get-Disk -ErrorAction SilentlyContinue | Where-Object { $_.BusType -eq 'USB' } | ForEach-Object {
  [pscustomobject]@{ number = [int]$_.Number; name = "$($_.FriendlyName)"; size = [int64]$_.Size; letters = @(Get-Partition -DiskNumber $_.Number -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter } | ForEach-Object { "$($_.DriveLetter)" }); system = ($_.Number -eq $sysDisk) }
})
ConvertTo-Json -InputObject @($out) -Depth 3 -Compress
"#;

fn usb_disks() -> Vec<UsbDisk> {
    crate::pspool::query(USB_SCRIPT, Some(Duration::from_secs(30)), "Pendrive de rescate: USB").ok().and_then(|o| serde_json::from_str(o.trim()).ok()).unwrap_or_default()
}

#[tauri::command(async)]
pub fn rescue_status() -> RescueStatus {
    let root = kits_root();
    let (env, pe) = root.as_deref().map(adk_paths).unzip();
    RescueStatus {
        adk: env.as_ref().is_some_and(|p| p.is_file()),
        winpe: pe.as_ref().is_some_and(|p| p.join("copype.cmd").is_file()),
        kits_root: root.map(|r| r.display().to_string()).unwrap_or_default(),
        usb: usb_disks(),
    }
}

/// ¿Se puede usar este disco para el pendrive? (USB, no el del sistema, ni enano ni enorme).
pub fn usable(d: &UsbDisk) -> Result<(), String> {
    if d.system {
        return Err("Ese es el disco del sistema: no se toca.".into());
    }
    if d.size < 1 << 30 {
        return Err("El pendrive tiene que tener al menos 1 GB.".into());
    }
    if d.size > 256u64 << 30 {
        return Err("Más de 256 GB parece un disco externo, no un pendrive: por seguridad, usa uno más pequeño.".into());
    }
    if d.letters.is_empty() {
        return Err("El pendrive no tiene ninguna letra de unidad: dale formato una vez desde el Explorador.".into());
    }
    Ok(())
}

/// Lo que arranca WinPE: prepara el sistema y busca el menú en cualquier unidad.
pub const STARTNET: &str = "wpeinit\r\n@echo off\r\nfor %%d in (C D E F G H I J K L M N O P Q R S T U V W X Y Z) do if exist %%d:\\AdminOps-Rescate\\rescate.cmd (call %%d:\\AdminOps-Rescate\\rescate.cmd & goto :eof)\r\necho No se encuentra el menu de rescate de AdminOps.\r\n";

pub const RESCUE_CMD: &str = "@echo off\r\ntitle AdminOps - Rescate\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File \"%~dp0rescate.ps1\"\r\n";

/// El menú de rescate (PowerShell dentro de WinPE).
pub const RESCUE_PS1: &str = r#"
$ErrorActionPreference = 'Continue'
function Find-Windows {
  foreach ($l in [char[]](67..90)) { if (Test-Path "${l}:\Windows\System32\config\SYSTEM") { return "${l}:" } }
  return $null
}
function Pause-Rescue { Write-Host ''; Read-Host 'Pulsa Intro para volver al menu' | Out-Null }
while ($true) {
  Clear-Host
  $win = Find-Windows
  Write-Host '=== AdminOps · Pendrive de rescate ===' -ForegroundColor Cyan
  Write-Host ("Windows encontrado en: " + $(if ($win) { $win } else { 'ninguna unidad' }))
  Write-Host ''
  Write-Host ' 1. Ver los discos y su salud'
  Write-Host ' 2. Copiar los datos de un usuario a otro disco'
  Write-Host ' 3. Reparar el arranque de Windows'
  Write-Host ' 4. Comprobar y reparar el disco de Windows (chkdsk)'
  Write-Host ' 5. Desactivar Driver Verifier del Windows instalado'
  Write-Host ' 6. Quitar un driver del Windows instalado'
  Write-Host ' 7. Abrir el Bloc de notas (para ver y copiar archivos)'
  Write-Host ' 8. Simbolo del sistema'
  Write-Host ' 9. Reiniciar        0. Apagar'
  $c = Read-Host 'Elige una opcion'
  switch ($c) {
    '1' { Get-PhysicalDisk | Format-Table FriendlyName, MediaType, HealthStatus, OperationalStatus, @{ n = 'GB'; e = { [math]::Round($_.Size / 1GB) } } -AutoSize; Get-Volume | Where-Object DriveLetter | Format-Table DriveLetter, FileSystemLabel, FileSystem, HealthStatus, @{ n = 'Libre GB'; e = { [math]::Round($_.SizeRemaining / 1GB, 1) } } -AutoSize; Pause-Rescue }
    '2' {
      if (-not $win) { Write-Host 'No hay Windows en ninguna unidad.'; Pause-Rescue; break }
      Get-ChildItem "$win\Users" -Directory | Where-Object { $_.Name -notin 'Public', 'Default', 'Default User', 'All Users' } | ForEach-Object { Write-Host " - $($_.Name)" }
      $u = Read-Host 'Usuario a copiar'
      $dest = Read-Host 'Unidad de destino (por ejemplo E)'
      $target = "$($dest.TrimEnd(':')):\Rescate-$u"
      robocopy "$win\Users\$u" $target /E /R:1 /W:1 /XJ /MT:8 /NFL /NDL /XD 'AppData\Local\Temp' 'AppData\Local\Microsoft\Windows\INetCache'
      Write-Host "Copiado en $target (los archivos que no se pudieron leer salen arriba como errores)."
      Pause-Rescue
    }
    '3' { if ($win) { bcdboot "$win\Windows" /f ALL; Write-Host 'Si dice que se crearon los archivos de arranque, reinicia sin el pendrive.' } else { Write-Host 'No hay Windows en ninguna unidad.' }; Pause-Rescue }
    '4' { if ($win) { chkdsk $win /f } else { Write-Host 'No hay Windows en ninguna unidad.' }; Pause-Rescue }
    '5' {
      if (-not $win) { Write-Host 'No hay Windows en ninguna unidad.'; Pause-Rescue; break }
      reg load HKLM\OFFSYS "$win\Windows\System32\config\SYSTEM" | Out-Null
      foreach ($cs in 'ControlSet001', 'ControlSet002') { reg delete "HKLM\OFFSYS\$cs\Control\Session Manager\Memory Management" /v VerifyDrivers /f 2>$null | Out-Null; reg delete "HKLM\OFFSYS\$cs\Control\Session Manager\Memory Management" /v VerifyDriverLevel /f 2>$null | Out-Null }
      reg unload HKLM\OFFSYS | Out-Null
      Write-Host 'Driver Verifier desactivado en el Windows instalado. Reinicia sin el pendrive.'
      Pause-Rescue
    }
    '6' {
      if (-not $win) { Write-Host 'No hay Windows en ninguna unidad.'; Pause-Rescue; break }
      dism /Image:"$win\" /Get-Drivers /Format:Table
      $d = Read-Host 'Nombre publicado del driver a quitar (por ejemplo oem12.inf; vacio para volver)'
      if ($d -match '^oem\d+\.inf$') { dism /Image:"$win\" /Remove-Driver /Driver:$d } elseif ($d) { Write-Host 'Escribe un nombre como oem12.inf.' }
      Pause-Rescue
    }
    '7' { Start-Process notepad.exe }
    '8' { cmd.exe }
    '9' { wpeutil reboot }
    '0' { wpeutil shutdown }
  }
}
"#;

fn cmd_with_env(env: &Path, line: &str, timeout: u64, task: &str) -> Result<String, String> {
    let script = format!("call \"{}\" >nul && {line}", env.display());
    crate::ps::exec_opts("cmd.exe", &["/d", "/c", &script], crate::ps::Opts { timeout: Some(Duration::from_secs(timeout)), task: Some(task) })
}

/// Crea el pendrive (se BORRA lo que tenga). Tarea «rescue».
#[tauri::command(async)]
pub fn rescue_create(app: tauri::AppHandle, number: u32) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let root = kits_root().ok_or("Falta el Windows ADK: instálalo primero (botón de esta tarjeta).")?;
    let (env, pe) = adk_paths(&root);
    if !env.is_file() || !pe.join("copype.cmd").is_file() {
        return Err("Faltan el Windows ADK o su complemento WinPE: instálalos primero.".into());
    }
    let disk = usb_disks().into_iter().find(|d| d.number == number).ok_or("Ese pendrive ya no está conectado.")?;
    usable(&disk)?;
    let letter = disk.letters[0].clone();
    let task = crate::task::Task::new(&app, "rescue").named("Pendrive de rescate");
    let work = std::env::temp_dir().join("AdminOps-WinPE");
    let _ = std::fs::remove_dir_all(&work);
    let mount = work.join("mount");
    let result = (|| -> Result<(), String> {
        task.step("Preparando WinPE (1 de 5)");
        cmd_with_env(&env, &format!("copype amd64 \"{}\"", work.display()), 600, "rescue")?;
        task.step("Añadiendo PowerShell y almacenamiento (2 de 5)");
        let wim = work.join(r"media\sources\boot.wim");
        crate::ps::exec_opts("dism.exe", &["/Mount-Image", &format!("/ImageFile:{}", wim.display()), "/Index:1", &format!("/MountDir:{}", mount.display())], crate::ps::Opts { timeout: Some(Duration::from_secs(900)), task: Some("rescue") })?;
        let ocs = pe.join(r"amd64\WinPE_OCs");
        let commit = (|| -> Result<(), String> {
            for c in COMPONENTS {
                if task.cancelled() {
                    return Err("Cancelado.".into());
                }
                task.step(format!("Añadiendo {c} (2 de 5)"));
                crate::ps::exec_opts("dism.exe", &[&format!("/Image:{}", mount.display()), "/Add-Package", &format!("/PackagePath:{}", ocs.join(format!("{c}.cab")).display())], crate::ps::Opts { timeout: Some(Duration::from_secs(900)), task: Some("rescue") })?;
            }
            std::fs::write(mount.join(r"Windows\System32\startnet.cmd"), STARTNET).map_err(|e| e.to_string())
        })();
        task.step("Cerrando la imagen (3 de 5)");
        let unmount = crate::ps::exec_opts("dism.exe", &["/Unmount-Image", &format!("/MountDir:{}", mount.display()), if commit.is_ok() { "/Commit" } else { "/Discard" }], crate::ps::Opts { timeout: Some(Duration::from_secs(900)), task: None });
        commit?;
        unmount?;
        task.step(format!("Copiando al pendrive {letter}: (4 de 5): se borra lo que tenía"));
        cmd_with_env(&env, &format!("MakeWinPEMedia /UFD /F \"{}\" {letter}:", work.display()), 1200, "rescue")?;
        task.step("Copiando el menú de rescate (5 de 5)");
        let dir = PathBuf::from(format!("{letter}:\\AdminOps-Rescate"));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("rescate.cmd"), RESCUE_CMD).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("rescate.ps1"), RESCUE_PS1).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("LEEME.txt"), "Pendrive de rescate de AdminOps.\r\nArranca el equipo desde este pendrive (tecla de menu de arranque: F12, F8, F11 o Esc segun el fabricante).\r\nEl menu se abre solo. Si no, ejecuta AdminOps-Rescate\\rescate.cmd.\r\n").map_err(|e| e.to_string())?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    {
        use tauri::Manager;
        app.state::<crate::tweaks::TweakState>().record(crate::tweaks::journal::Op::Run, &format!("Crear pendrive de rescate en {letter}:"), &result);
    }
    result.map(|()| format!("Pendrive de rescate listo en {letter}:. Arranca el equipo desde él (F12, F8, F11 o Esc al encender, según la marca)."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_pendrives_razonables() {
        let d = |size: u64, system: bool, letters: &[&str]| UsbDisk { number: 2, name: "Kingston".into(), size, letters: letters.iter().map(|s| s.to_string()).collect(), system };
        assert!(usable(&d(16 << 30, false, &["E"])).is_ok());
        assert!(usable(&d(16 << 30, true, &["E"])).is_err());
        assert!(usable(&d(512 << 20, false, &["E"])).is_err());
        assert!(usable(&d(1 << 40, false, &["E"])).is_err());
        assert!(usable(&d(16 << 30, false, &[])).is_err());
    }

    #[test]
    fn scripts_parse() {
        for s in [USB_SCRIPT, RESCUE_PS1] {
            let e = crate::ps::parse_errors(s);
            assert!(e.is_empty(), "{e}");
        }
    }

    #[test]
    fn startnet_busca_el_menu() {
        assert!(STARTNET.starts_with("wpeinit"));
        assert!(STARTNET.contains(r"AdminOps-Rescate\rescate.cmd"));
    }
}
