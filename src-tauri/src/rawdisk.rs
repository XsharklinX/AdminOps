//! Acceso directo a un disco físico (`\\.\PhysicalDriveN`) o a un archivo de
//! imagen, con la misma interfaz, para que el mapa de superficie, el escaneo de
//! particiones, la recuperación por firmas y el clonado funcionen igual sobre
//! un disco real que sobre una imagen (y se puedan probar sin disco).
//!
//! Leer un disco físico necesita administrador. Escribir en él, además, solo lo
//! hace el módulo de particiones, tras copiar la tabla y con confirmación.

use std::io::{Read, Seek, SeekFrom};

/// Algo de lo que se pueden leer bloques.
pub trait BlockSource {
    /// Tamaño total en bytes.
    fn len(&self) -> u64;
    /// Tamaño de sector (512 o 4096).
    fn sector(&self) -> u32;
    /// Lee `buf.len()` bytes desde `offset`. `Err` lleva el código de error del sistema (0 si no hay).
    /// Devuelve cuántos bytes se leyeron (menos que `buf.len()` solo al final).
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, u32>;
}

/// Búfer alineado a 4 KB (la E/S sin caché de Windows lo exige).
pub struct Aligned {
    raw: Vec<u8>,
    off: usize,
    len: usize,
}

impl Aligned {
    pub fn new(len: usize) -> Self {
        let raw = vec![0u8; len + 4096];
        let off = (4096 - (raw.as_ptr() as usize % 4096)) % 4096;
        Aligned { raw, off, len }
    }
    pub fn slice(&mut self, len: usize) -> &mut [u8] {
        &mut self.raw[self.off..self.off + len.min(self.len)]
    }
}

/// Un archivo (imagen de disco o dispositivo en Linux, para las pruebas).
pub struct FileSource {
    file: std::fs::File,
    len: u64,
    sector: u32,
}

impl FileSource {
    pub fn open(path: &std::path::Path, sector: u32) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        Ok(FileSource { file, len, sector })
    }
}

impl BlockSource for FileSource {
    fn len(&self) -> u64 {
        self.len
    }
    fn sector(&self) -> u32 {
        self.sector
    }
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, u32> {
        self.file.seek(SeekFrom::Start(offset)).map_err(|e| e.raw_os_error().unwrap_or(0) as u32)?;
        let mut done = 0;
        while done < buf.len() {
            match self.file.read(&mut buf[done..]) {
                Ok(0) => break,
                Ok(n) => done += n,
                Err(e) => return Err(e.raw_os_error().unwrap_or(0) as u32),
            }
        }
        Ok(done)
    }
}

/// Trozos de memoria (para las pruebas): `bad` son rangos que dan error al leer.
#[cfg(test)]
pub struct MemSource {
    pub data: Vec<u8>,
    pub sector: u32,
    pub bad: Vec<(u64, u64)>,
}

#[cfg(test)]
impl BlockSource for MemSource {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }
    fn sector(&self) -> u32 {
        self.sector
    }
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, u32> {
        let end = offset + buf.len() as u64;
        if self.bad.iter().any(|(a, b)| offset < *b && end > *a) {
            return Err(23);
        }
        if offset >= self.data.len() as u64 {
            return Ok(0);
        }
        let n = buf.len().min(self.data.len() - offset as usize);
        buf[..n].copy_from_slice(&self.data[offset as usize..offset as usize + n]);
        Ok(n)
    }
}

// ---------- Disco físico de Windows ----------

#[cfg(windows)]
mod win {
    use super::BlockSource;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, ReadFile, SetFilePointerEx, WriteFile, FILE_BEGIN, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x0007_405C;
    const IOCTL_DISK_GET_DRIVE_GEOMETRY_EX: u32 = 0x0007_00A0;

    pub struct RawDisk {
        h: HANDLE,
        len: u64,
        sector: u32,
        number: u32,
    }

    // SAFETY: el identificador de Windows se puede usar desde otro hilo; cada hilo usa el suyo.
    unsafe impl Send for RawDisk {}

    #[derive(Clone, Copy, PartialEq)]
    pub enum Mode {
        /// Solo órdenes (SMART, información): sin acceso a los datos.
        Query,
        /// Lectura de datos, sin pasar por la caché.
        Read,
        /// Lectura y escritura (solo particiones).
        Write,
    }

    impl RawDisk {
        pub fn open(number: u32, mode: Mode) -> Result<Self, String> {
            let path: Vec<u16> = format!(r"\\.\PhysicalDrive{number}").encode_utf16().chain(Some(0)).collect();
            let (access, flags) = match mode {
                Mode::Query => (GENERIC_READ | GENERIC_WRITE, 0),
                Mode::Read => (GENERIC_READ, FILE_FLAG_NO_BUFFERING),
                Mode::Write => (GENERIC_READ | GENERIC_WRITE, FILE_FLAG_NO_BUFFERING),
            };
            // SAFETY: ruta terminada en cero; se abre, se consulta y se cierra al soltarlo.
            let h = unsafe { CreateFileW(path.as_ptr(), access, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, flags, std::ptr::null_mut()) };
            if h == INVALID_HANDLE_VALUE {
                // SAFETY: solo lee el último error del hilo.
                let e = unsafe { GetLastError() };
                return Err(match e {
                    5 => "Windows no deja abrir el disco: ejecuta AdminOps como administrador.".to_string(),
                    2 | 3 => format!("El disco {number} ya no está conectado."),
                    32 => "Otro programa tiene el disco en uso exclusivo.".to_string(),
                    _ => format!("No se pudo abrir el disco {number} (error {e})."),
                });
            }
            let mut d = RawDisk { h, len: 0, sector: 512, number };
            let mut out = [0u8; 8];
            if let Ok(8) = d.ioctl(IOCTL_DISK_GET_LENGTH_INFO, &[], &mut out) {
                d.len = u64::from_le_bytes(out);
            }
            let mut geo = [0u8; 128];
            if d.ioctl(IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, &[], &mut geo).is_ok() {
                let s = u32::from_le_bytes([geo[20], geo[21], geo[22], geo[23]]);
                if s == 512 || s == 4096 {
                    d.sector = s;
                }
            }
            Ok(d)
        }

        pub fn number(&self) -> u32 {
            self.number
        }

        /// `DeviceIoControl`. Devuelve los bytes que dio el disco, o el código de error.
        pub fn ioctl(&self, code: u32, input: &[u8], output: &mut [u8]) -> Result<u32, u32> {
            let mut returned = 0u32;
            // SAFETY: los búferes son válidos durante la llamada y sus tamaños se pasan tal cual.
            let ok = unsafe {
                DeviceIoControl(
                    self.h,
                    code,
                    if input.is_empty() { std::ptr::null() } else { input.as_ptr().cast() },
                    input.len() as u32,
                    if output.is_empty() { std::ptr::null_mut() } else { output.as_mut_ptr().cast() },
                    output.len() as u32,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            if ok != 0 {
                Ok(returned)
            } else {
                // SAFETY: solo lee el último error del hilo.
                Err(unsafe { GetLastError() })
            }
        }

        /// Escribe `buf` (múltiplo del sector y alineado) en `offset`.
        pub fn write_at(&self, offset: u64, buf: &[u8]) -> Result<(), u32> {
            // SAFETY: el identificador es válido y el búfer vive durante la llamada.
            unsafe {
                if SetFilePointerEx(self.h, offset as i64, std::ptr::null_mut(), FILE_BEGIN) == 0 {
                    return Err(GetLastError());
                }
                let mut n = 0u32;
                if WriteFile(self.h, buf.as_ptr().cast(), buf.len() as u32, &mut n, std::ptr::null_mut()) == 0 || n as usize != buf.len() {
                    return Err(GetLastError());
                }
            }
            Ok(())
        }
    }

    impl BlockSource for RawDisk {
        fn len(&self) -> u64 {
            self.len
        }
        fn sector(&self) -> u32 {
            self.sector
        }
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, u32> {
            // SAFETY: el identificador es válido y el búfer vive durante la llamada.
            unsafe {
                if SetFilePointerEx(self.h, offset as i64, std::ptr::null_mut(), FILE_BEGIN) == 0 {
                    return Err(GetLastError());
                }
                let mut n = 0u32;
                if ReadFile(self.h, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n, std::ptr::null_mut()) == 0 {
                    return Err(GetLastError());
                }
                Ok(n as usize)
            }
        }
    }

    impl Drop for RawDisk {
        fn drop(&mut self) {
            // SAFETY: se cierra una sola vez el identificador abierto en `open`.
            unsafe {
                CloseHandle(self.h);
            }
        }
    }
}

#[cfg(windows)]
pub use win::{Mode, RawDisk};

/// Sin Windows no hay discos físicos que abrir (solo existe para que el resto compile).
#[cfg(not(windows))]
pub mod stub {
    use super::BlockSource;
    #[derive(Clone, Copy, PartialEq)]
    pub enum Mode {
        Query,
        Read,
        Write,
    }
    pub struct RawDisk;
    impl RawDisk {
        pub fn open(_: u32, _: Mode) -> Result<Self, String> {
            Err("Los discos físicos solo se pueden abrir en Windows.".into())
        }
        pub fn number(&self) -> u32 {
            0
        }
        pub fn ioctl(&self, _: u32, _: &[u8], _: &mut [u8]) -> Result<u32, u32> {
            Err(1)
        }
        pub fn write_at(&self, _: u64, _: &[u8]) -> Result<(), u32> {
            Err(1)
        }
    }
    impl BlockSource for RawDisk {
        fn len(&self) -> u64 {
            0
        }
        fn sector(&self) -> u32 {
            512
        }
        fn read_at(&mut self, _: u64, _: &mut [u8]) -> Result<usize, u32> {
            Err(1)
        }
    }
}

#[cfg(not(windows))]
pub use stub::{Mode, RawDisk};

/// Para elegir en tiempo de ejecución entre un disco físico y una imagen.
impl BlockSource for Box<dyn BlockSource + Send> {
    fn len(&self) -> u64 {
        (**self).len()
    }
    fn sector(&self) -> u32 {
        (**self).sector()
    }
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, u32> {
        (**self).read_at(offset, buf)
    }
}
