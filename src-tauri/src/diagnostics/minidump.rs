//! Análisis de los minivolcados de pantallazos azules (`C:\Windows\Minidump\*.dmp`).
//!
//! Formato de volcado de kernel de 64 bits ("PAGEDU64") de tipo triaje: una
//! cabecera de 0x2000 bytes con el código de error y, detrás, una estructura
//! con la lista de drivers cargados y una copia de la pila del hilo que falló.
//! El driver "probable" es el primero de la pila que no es el núcleo de
//! Windows, igual que hacen BlueScreenView y similares. Es una pista, no un
//! veredicto: si algo no cuadra, se devuelve el código sin culpable.

use serde::{Deserialize, Serialize};

const HEADER_SIZE: usize = 0x2000;
const OFF_BUGCHECK: usize = 0x38;
const OFF_PARAMS: usize = 0x40;
const OFF_DUMP_TYPE: usize = 0xF98;
const DUMP_TYPE_TRIAGE: u32 = 4;
const KERNEL_SPACE: u64 = 0xFFFF_8000_0000_0000;
/// Módulos del núcleo: aparecen en casi todas las pilas y no señalan a nadie.
const CORE: &[&str] = &["ntoskrnl.exe", "ntkrnlmp.exe", "ntkrnlpa.exe", "ntkrpamp.exe", "hal.dll"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DumpAnalysis {
    /// p. ej. "0x0000009F"
    pub bugcheck: Option<String>,
    /// Driver más probable (p. ej. "nvlddmkm.sys").
    pub culprit: Option<String>,
    /// Qué es ese driver, si se conoce.
    pub culprit_hint: Option<String>,
    /// Drivers que no son del núcleo encontrados en la pila, por orden.
    pub stack_drivers: Vec<String>,
}

struct Module {
    name: String,
    base: u64,
    size: u64,
}

fn u32_at(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn u64_at(b: &[u8], off: usize) -> Option<u64> {
    b.get(off..off + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap()))
}

/// Cadena del "string pool": longitud en caracteres (u32) + UTF-16.
fn pool_string(b: &[u8], off: usize, pool: std::ops::Range<usize>) -> Option<String> {
    if !pool.contains(&off) {
        return None;
    }
    let len = u32_at(b, off)? as usize;
    if len == 0 || len > 260 {
        return None;
    }
    let bytes = b.get(off + 4..off + 4 + len * 2)?;
    let words: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let s = String::from_utf16(&words).ok()?;
    let lower = s.to_ascii_lowercase();
    (lower.ends_with(".sys") || lower.ends_with(".dll") || lower.ends_with(".exe")).then_some(s)
}

/// Lista de drivers. El tamaño de cada entrada y la posición de la base
/// cambian entre versiones de Windows, así que se prueban las combinaciones
/// habituales y se acepta la primera en la que TODAS las entradas son coherentes.
fn modules(b: &[u8], triage: usize) -> Option<Vec<Module>> {
    let list = u32_at(b, triage + 0x30)? as usize;
    let count = u32_at(b, triage + 0x34)? as usize;
    let pool = u32_at(b, triage + 0x38)? as usize;
    let pool_size = u32_at(b, triage + 0x3C)? as usize;
    if count == 0 || count > 2000 || pool.checked_add(pool_size)? > b.len() {
        return None;
    }
    let pool_range = pool..pool + pool_size;
    // (desplazamiento de la entrada LDR dentro del registro, tamaño del registro)
    for ldr in [8usize, 4] {
        for stride in (0x90..=0xE0).step_by(8) {
            let parsed: Option<Vec<Module>> = (0..count)
                .map(|i| {
                    let e = list + i * stride;
                    let name = pool_string(b, u32_at(b, e)? as usize, pool_range.clone())?;
                    let base = u64_at(b, e + ldr + 0x30)?;
                    let size = u32_at(b, e + ldr + 0x40)? as u64;
                    (base >= KERNEL_SPACE && size > 0 && size < 0x2000_0000).then_some(Module { name, base, size })
                })
                .collect();
            if let Some(m) = parsed {
                return Some(m);
            }
        }
    }
    None
}

fn hint(driver: &str) -> Option<&'static str> {
    let d = driver.to_ascii_lowercase();
    let table: &[(&str, &str)] = &[
        ("nvlddmkm", "Driver de la tarjeta gráfica NVIDIA: reinstálalo (limpio) o prueba otra versión."),
        ("atikmdag", "Driver de la tarjeta gráfica AMD: reinstálalo o prueba otra versión."),
        ("amdkmdag", "Driver de la tarjeta gráfica AMD: reinstálalo o prueba otra versión."),
        ("igdkmd", "Driver de gráficos Intel: actualízalo desde el fabricante del equipo o de Intel."),
        ("dxgkrnl", "Subsistema gráfico de Windows: casi siempre lo provoca el driver de la tarjeta gráfica."),
        ("dxgmms", "Subsistema gráfico de Windows: casi siempre lo provoca el driver de la tarjeta gráfica."),
        ("netwtw", "Driver de Wi-Fi Intel: actualízalo."),
        ("netwbw", "Driver de Wi-Fi Intel: actualízalo."),
        ("rtwlan", "Driver de Wi-Fi Realtek: actualízalo."),
        ("rt640", "Driver de red Realtek: actualízalo."),
        ("rtux64", "Adaptador de red USB Realtek: actualiza su driver o prueba otro puerto."),
        ("e1d", "Driver de red Intel: actualízalo."),
        ("iastor", "Controlador de discos Intel RST: actualízalo o revisa el disco."),
        ("storahci", "Controlador de discos: revisa el estado SMART del disco y los cables."),
        ("stornvme", "Controlador NVMe: revisa el SSD (SMART) y su firmware."),
        ("ntfs", "Sistema de archivos: revisa el disco (SMART) y ejecuta chkdsk."),
        ("wdfilter", "Microsoft Defender: suele indicar un conflicto con otro antivirus."),
        ("klif", "Antivirus Kaspersky: actualízalo o desinstálalo para probar."),
        ("asw", "Antivirus Avast/AVG: actualízalo o desinstálalo para probar."),
        ("bdselfpr", "Antivirus Bitdefender: actualízalo o desinstálalo para probar."),
        ("mfe", "Antivirus McAfee: desinstálalo con su herramienta oficial para probar."),
        ("tcpip", "Pila de red de Windows: suele venir de un driver de red, VPN o antivirus."),
        ("ndis", "Red de Windows: suele venir de un driver de red, VPN o antivirus."),
        ("usbxhci", "Controlador USB: prueba sin dispositivos USB y actualiza el chipset."),
        ("win32k", "Parte gráfica de Windows: revisa el driver de la tarjeta gráfica."),
        ("vbox", "Drivers de VirtualBox: actualiza VirtualBox."),
        ("vmm", "Drivers de virtualización: actualiza el programa de máquinas virtuales."),
    ];
    table.iter().find(|(k, _)| d.starts_with(k)).map(|(_, h)| *h)
}

pub fn analyze(b: &[u8]) -> Option<DumpAnalysis> {
    if b.len() < HEADER_SIZE || &b[0..4] != b"PAGE" || &b[4..8] != b"DU64" {
        return None; // no es un volcado de 64 bits
    }
    let code = u32_at(b, OFF_BUGCHECK)?;
    let mut a = DumpAnalysis { bugcheck: Some(format!("0x{code:08X}")), ..Default::default() };
    if u32_at(b, OFF_DUMP_TYPE) != Some(DUMP_TYPE_TRIAGE) {
        return Some(a);
    }
    let Some(mods) = modules(b, HEADER_SIZE) else { return Some(a) };
    let owner = |addr: u64| mods.iter().find(|m| addr >= m.base && addr < m.base + m.size);

    let stack = u32_at(b, HEADER_SIZE + 0x28).map(|o| o as usize);
    let stack_size = u32_at(b, HEADER_SIZE + 0x2C).map(|s| s as usize);
    if let (Some(off), Some(size)) = (stack, stack_size) {
        if let Some(bytes) = b.get(off..off.saturating_add(size.min(0x10000))) {
            for chunk in bytes.chunks_exact(8) {
                let v = u64::from_le_bytes(chunk.try_into().unwrap());
                if let Some(m) = owner(v) {
                    let lower = m.name.to_ascii_lowercase();
                    if !CORE.contains(&lower.as_str()) && !a.stack_drivers.contains(&m.name) {
                        a.stack_drivers.push(m.name.clone());
                    }
                }
            }
        }
    }
    // Si la pila solo tiene el núcleo, mirar las direcciones de los parámetros del error.
    let from_params = || {
        (0..4)
            .filter_map(|i| u64_at(b, OFF_PARAMS + i * 8))
            .filter_map(owner)
            .map(|m| m.name.clone())
            .find(|n| !CORE.contains(&n.to_ascii_lowercase().as_str()))
    };
    a.culprit = a.stack_drivers.first().cloned().or_else(from_params);
    a.culprit_hint = a.culprit.as_deref().and_then(hint).map(String::from);
    Some(a)
}

/// Analiza un archivo (lee como máximo 64 MB: los minivolcados ocupan de 0,2 a 3 MB).
pub fn analyze_file(path: &std::path::Path) -> Option<DumpAnalysis> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path).ok()?.take(64 << 20).read_to_end(&mut buf).ok()?;
    analyze(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye un minivolcado sintético con `drivers` (nombre, base, tamaño)
    /// y una pila que contiene las direcciones indicadas.
    fn synthetic(code: u32, drivers: &[(&str, u64, u32)], stack: &[u64], stride: usize) -> Vec<u8> {
        let mut b = vec![0u8; 0x8000];
        b[0..4].copy_from_slice(b"PAGE");
        b[4..8].copy_from_slice(b"DU64");
        b[OFF_BUGCHECK..OFF_BUGCHECK + 4].copy_from_slice(&code.to_le_bytes());
        b[OFF_DUMP_TYPE..OFF_DUMP_TYPE + 4].copy_from_slice(&DUMP_TYPE_TRIAGE.to_le_bytes());
        let t = HEADER_SIZE;
        let (list, pool, stack_off) = (0x2100usize, 0x4000usize, 0x6000usize);
        let put32 = |b: &mut Vec<u8>, o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let put64 = |b: &mut Vec<u8>, o: usize, v: u64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
        put32(&mut b, t + 0x28, stack_off as u32);
        put32(&mut b, t + 0x2C, (stack.len() * 8) as u32);
        put32(&mut b, t + 0x30, list as u32);
        put32(&mut b, t + 0x34, drivers.len() as u32);
        put32(&mut b, t + 0x38, pool as u32);
        let mut p = pool;
        for (i, (name, base, size)) in drivers.iter().enumerate() {
            let e = list + i * stride;
            put32(&mut b, e, p as u32);
            put64(&mut b, e + 8 + 0x30, *base);
            put32(&mut b, e + 8 + 0x40, *size);
            let w: Vec<u16> = name.encode_utf16().collect();
            put32(&mut b, p, w.len() as u32);
            for (k, c) in w.iter().enumerate() {
                b[p + 4 + k * 2..p + 6 + k * 2].copy_from_slice(&c.to_le_bytes());
            }
            p += 4 + w.len() * 2 + 2;
            p = (p + 7) & !7;
        }
        put32(&mut b, t + 0x3C, (p - pool) as u32);
        for (i, v) in stack.iter().enumerate() {
            put64(&mut b, stack_off + i * 8, *v);
        }
        b
    }

    const NT: u64 = 0xFFFF_F800_0000_0000;
    const NV: u64 = 0xFFFF_F801_1000_0000;
    const NET: u64 = 0xFFFF_F801_2000_0000;

    #[test]
    fn finds_first_third_party_driver_on_stack() {
        let drivers = [("ntoskrnl.exe", NT, 0x100_0000), ("nvlddmkm.sys", NV, 0x200_0000), ("Netwtw10.sys", NET, 0x10_0000)];
        for stride in [0xA0, 0xA8, 0xB0] {
            let b = synthetic(0x116, &drivers, &[0x1234, NT + 0x500, NV + 0x42, NET + 0x10, NV + 0x99], stride);
            let a = analyze(&b).unwrap();
            assert_eq!(a.bugcheck.as_deref(), Some("0x00000116"));
            assert_eq!(a.culprit.as_deref(), Some("nvlddmkm.sys"), "stride {stride:#x}");
            assert_eq!(a.stack_drivers, vec!["nvlddmkm.sys", "Netwtw10.sys"]);
            assert!(a.culprit_hint.unwrap().contains("NVIDIA"));
        }
    }

    #[test]
    fn only_kernel_on_stack_means_no_culprit() {
        let b = synthetic(0x1A, &[("ntoskrnl.exe", NT, 0x100_0000)], &[NT + 1, NT + 2], 0xA8);
        let a = analyze(&b).unwrap();
        assert_eq!(a.bugcheck.as_deref(), Some("0x0000001A"));
        assert_eq!(a.culprit, None);
    }

    #[test]
    fn garbage_is_rejected_without_panicking() {
        assert_eq!(analyze(b"hola"), None);
        assert_eq!(analyze(&vec![0u8; 0x3000]), None);
        let mut b = synthetic(0x50, &[("x.sys", NV, 0x1000)], &[NV], 0xA8);
        // Lista de drivers que apunta fuera del archivo: código sí, culpable no.
        b[HEADER_SIZE + 0x30..HEADER_SIZE + 0x34].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
        let a = analyze(&b).unwrap();
        assert_eq!(a.bugcheck.as_deref(), Some("0x00000050"));
        assert_eq!(a.culprit, None);
        // Recortado a mitad: nunca debe entrar en pánico.
        for len in [0x2000, 0x2040, 0x4010, 0x6004] {
            let _ = analyze(&b[..len]);
        }
    }
}
