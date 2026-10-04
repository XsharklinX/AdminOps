//! Lo que se ve en la barra de arriba de AdminOps: nombre del equipo, dominio o
//! grupo de trabajo, espacio libre en el disco del sistema y si hay Internet.
//!
//! Se lee directamente de Windows (sin PowerShell) porque se consulta al abrir y
//! cada poco: tiene que tardar milisegundos. El estado a fondo del dominio
//! (relación de confianza, controlador) sigue en Usuarios y cuentas → Dominio.

use serde::Serialize;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MachineContext {
    pub computer_name: String,
    /// "domain", "workgroup" o "unknown".
    pub join: String,
    /// Dominio (nombre DNS si Windows lo sabe; si no, el NetBIOS) o grupo de trabajo.
    pub join_name: Option<String>,
    /// Unidad del sistema ("C:").
    pub system_drive: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InternetProbe {
    pub online: bool,
    /// Lo que tardó en conectar, si conectó.
    pub ms: Option<u32>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Lee una cadena de Windows terminada en cero.
unsafe fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

fn computer_name_ex(kind: windows_sys::Win32::System::SystemInformation::COMPUTER_NAME_FORMAT) -> Option<String> {
    use windows_sys::Win32::System::SystemInformation::GetComputerNameExW;
    let mut size: u32 = 0;
    // Primera llamada: cuánto sitio hace falta.
    unsafe { GetComputerNameExW(kind, std::ptr::null_mut(), &mut size) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u16; size as usize];
    let ok = unsafe { GetComputerNameExW(kind, buf.as_mut_ptr(), &mut size) };
    if ok == 0 {
        return None;
    }
    let s = String::from_utf16_lossy(&buf[..size as usize]);
    (!s.trim().is_empty()).then(|| s.trim().to_string())
}

/// Dominio o grupo de trabajo, como lo cuenta Windows.
fn join_info() -> (String, Option<String>) {
    use windows_sys::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetGetJoinInformation, NetSetupDomainName, NetSetupWorkgroupName};
    let mut name: *mut u16 = std::ptr::null_mut();
    let mut status = 0;
    let rc = unsafe { NetGetJoinInformation(std::ptr::null(), &mut name, &mut status) };
    if rc != 0 {
        return ("unknown".into(), None);
    }
    let netbios = unsafe { from_wide(name) };
    unsafe { NetApiBufferFree(name as *const _) };
    use windows_sys::Win32::System::SystemInformation::ComputerNameDnsDomain;
    if status == NetSetupDomainName {
        ("domain".into(), computer_name_ex(ComputerNameDnsDomain).or(Some(netbios)))
    } else if status == NetSetupWorkgroupName {
        ("workgroup".into(), Some(netbios))
    } else {
        ("unknown".into(), None)
    }
}

fn disk(drive: &str) -> (u64, u64) {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let path = wide(&format!("{drive}\\"));
    let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(path.as_ptr(), &mut free, &mut total, &mut total_free) };
    if ok == 0 {
        (0, 0)
    } else {
        (free, total)
    }
}

pub fn read() -> MachineContext {
    use windows_sys::Win32::System::SystemInformation::ComputerNamePhysicalNetBIOS;
    let system_drive = std::env::var("SystemDrive").ok().filter(|d| d.len() == 2 && d.ends_with(':')).unwrap_or_else(|| "C:".into());
    let (free_bytes, total_bytes) = disk(&system_drive);
    let (join, join_name) = join_info();
    MachineContext {
        computer_name: computer_name_ex(ComputerNamePhysicalNetBIOS).or_else(|| std::env::var("COMPUTERNAME").ok()).unwrap_or_default(),
        join,
        join_name,
        system_drive,
        free_bytes,
        total_bytes,
    }
}

/// ¿Hay Internet? Una conexión TCP a un servidor DNS público conocido (sin enviar
/// nada), con un tope de 2 s. Es lo mismo que comprueba quien abre una web.
pub fn probe() -> InternetProbe {
    let targets: [SocketAddr; 2] = [([1, 1, 1, 1], 443).into(), ([8, 8, 8, 8], 443).into()];
    for t in targets {
        let start = Instant::now();
        if TcpStream::connect_timeout(&t, Duration::from_secs(2)).is_ok() {
            return InternetProbe { online: true, ms: Some(start.elapsed().as_millis().min(u32::MAX as u128) as u32) };
        }
    }
    InternetProbe { online: false, ms: None }
}

#[tauri::command(async)]
pub fn machine_context() -> MachineContext {
    read()
}

#[tauri::command(async)]
pub fn internet_probe() -> InternetProbe {
    probe()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_this_machine_quickly() {
        let t = Instant::now();
        let c = read();
        assert!(!c.computer_name.is_empty());
        assert!(c.system_drive.ends_with(':'));
        assert!(c.total_bytes >= c.free_bytes);
        assert!(["domain", "workgroup", "unknown"].contains(&c.join.as_str()));
        // Sin PowerShell: tiene que ser casi instantáneo.
        assert!(t.elapsed() < Duration::from_secs(2), "tardó {:?}", t.elapsed());
    }
}
