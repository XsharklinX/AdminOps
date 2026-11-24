//! Análisis de espacio leyendo la MFT de NTFS de una vez (como WizTree).
//!
//! En vez de abrir carpeta por carpeta, se lee seguido el índice de archivos del disco (la Master File
//! Table: un registro de 1 KB por archivo o carpeta, con su nombre, su carpeta y su tamaño) y el árbol se
//! monta en memoria. En un SSD, millones de archivos en segundos.
//!
//! Solo lectura, y solo con AdminOps abierto como administrador (se abre el volumen `\\.\C:`). Si no se
//! puede (sin permisos, el disco no es NTFS, unidad de red, carpeta montada…), `scan` devuelve `None` y el
//! análisis sigue con el recorrido normal carpeta por carpeta.
//!
//! Igual que el recorrido normal: no se cuenta nada con «reparse» (enlaces, uniones, carpetas y archivos de
//! OneDrive y similares), ni System Volume Information, ni los archivos internos de NTFS ($MFT, $LogFile,
//! $Extend…). Diferencias, a propósito: un archivo con varios nombres (enlaces duros, como los de WinSxS) se
//! cuenta una vez, en la carpeta de su primer nombre; y no hay carpetas sin permiso.
//!
//! Para el disco entero se usa siempre. Para una subcarpeta, solo en discos sin cabezal (SSD): en un disco
//! mecánico, leer toda la MFT para una carpeta pequeña tardaría más que recorrerla.

use super::{kind_of, Counters, Node, Tops, CANCEL, KINDS, KIND_MIN, KIND_TOP, MIN_NODE, TOP_FILES};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

// ---------- Lectura de registros (puro, con pruebas) ----------

const MASK48: u64 = 0xFFFF_FFFF_FFFF;
/// Atributos de NTFS que se leen.
const ATTR_STANDARD_INFORMATION: u32 = 0x10;
const ATTR_FILE_NAME: u32 = 0x30;
const ATTR_DATA: u32 = 0x80;
const ATTR_END: u32 = 0xFFFF_FFFF;
/// Espacio de nombres «DOS» (el nombre corto 8.3): solo si no hay otro.
const NS_DOS: u8 = 2;
const REPARSE: u32 = 0x400;
/// Carpeta raíz de todo volumen NTFS.
const ROOT_RECORD: u32 = 5;
/// Los primeros registros son de NTFS ($MFT, $LogFile, $Extend…), salvo la raíz.
const FIRST_USER_RECORD: u32 = 24;

fn u16le(b: &[u8], o: usize) -> u16 {
    b.get(o..o + 2).map_or(0, |x| u16::from_le_bytes([x[0], x[1]]))
}
fn u32le(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4).map_or(0, |x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}
fn u64le(b: &[u8], o: usize) -> u64 {
    b.get(o..o + 8).map_or(0, |x| u64::from_le_bytes([x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]]))
}

/// FILETIME (100 ns desde 1601) a segundos desde 1970.
fn filetime_to_epoch(t: u64) -> u64 {
    t.saturating_sub(116_444_736_000_000_000) / 10_000_000
}

/// Deshace la corrección de fin de sector («update sequence») de un registro: NTFS guarda en los dos
/// últimos bytes de cada tramo de 512 un número de control, y los bytes de verdad, aparte.
/// `false` si el registro no es un registro de archivo o está a medio escribir.
pub(super) fn fixup(rec: &mut [u8]) -> bool {
    if rec.len() < 48 || &rec[0..4] != b"FILE" {
        return false;
    }
    let off = u16le(rec, 4) as usize;
    let count = u16le(rec, 6) as usize;
    if count < 2 || off + count * 2 > rec.len() || (count - 1) * 512 > rec.len() {
        return false;
    }
    let check = [rec[off], rec[off + 1]];
    for i in 1..count {
        let end = i * 512;
        if rec[end - 2..end] != check {
            return false;
        }
        rec[end - 2] = rec[off + i * 2];
        rec[end - 1] = rec[off + i * 2 + 1];
    }
    true
}

/// Lo que se saca de un registro de la MFT.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Record {
    pub in_use: bool,
    pub is_dir: bool,
    /// Registro base, si este es una extensión (atributos que no cupieron en el base).
    pub base: u64,
    /// Carpeta, nombre y espacio de nombres del mejor nombre (el largo antes que el 8.3).
    pub name: Option<(u64, String, u8)>,
    /// Tamaño del contenido principal (el tramo que empieza al principio, si está repartido).
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub attrs: Option<u32>,
}

/// El valor de un atributo residente (guardado dentro del propio registro).
fn resident_value(a: &[u8]) -> Option<&[u8]> {
    let len = u32le(a, 0x10) as usize;
    let off = u16le(a, 0x14) as usize;
    a.get(off..off.checked_add(len)?)
}

/// Lee un registro ya corregido con `fixup`.
pub(super) fn parse(rec: &[u8]) -> Record {
    let flags = u16le(rec, 0x16);
    let mut r = Record { in_use: flags & 1 != 0, is_dir: flags & 2 != 0, base: u64le(rec, 0x20) & MASK48, ..Default::default() };
    if !r.in_use {
        return r;
    }
    let used = (u32le(rec, 0x18) as usize).min(rec.len());
    let mut off = u16le(rec, 0x14) as usize;
    while off + 16 <= used {
        let ty = u32le(rec, off);
        let len = u32le(rec, off + 4) as usize;
        if ty == ATTR_END || len < 16 || off + len > used {
            break;
        }
        let a = &rec[off..off + len];
        let non_resident = a[8] != 0;
        let unnamed = a[9] == 0;
        match ty {
            ATTR_STANDARD_INFORMATION if !non_resident => {
                if let Some(v) = resident_value(a).filter(|v| v.len() >= 0x24) {
                    r.modified = Some(filetime_to_epoch(u64le(v, 8)));
                    r.attrs = Some(u32le(v, 0x20));
                }
            }
            ATTR_FILE_NAME if !non_resident => {
                if let Some(v) = resident_value(a).filter(|v| v.len() >= 0x42) {
                    let n = v[0x40] as usize;
                    let ns = v[0x41];
                    let better = r.name.as_ref().is_none_or(|(_, _, old)| *old == NS_DOS && ns != NS_DOS);
                    if better {
                        if let Some(raw) = v.get(0x42..0x42 + n * 2) {
                            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                            r.name = Some((u64le(v, 0) & MASK48, String::from_utf16_lossy(&units), ns));
                        }
                    }
                }
            }
            ATTR_DATA if unnamed => {
                if non_resident {
                    // El tamaño real solo vale en el tramo que empieza en el principio (VCN 0).
                    if a.len() >= 0x38 && u64le(a, 0x10) == 0 {
                        r.size = Some(u64le(a, 0x30));
                    }
                } else {
                    r.size = Some(u64::from(u32le(a, 0x10)));
                }
            }
            _ => {}
        }
        off += len;
    }
    r
}

/// Los tramos de un atributo no residente: (primer clúster, cuántos). `None` en el primer clúster es un
/// hueco (tramo disperso). `None` del todo si la lista está dañada.
pub(super) fn decode_runs(mut p: &[u8]) -> Option<Vec<(Option<u64>, u64)>> {
    let mut out = Vec::new();
    let mut lcn: i64 = 0;
    while let Some(&h) = p.first() {
        if h == 0 {
            break;
        }
        let (ls, os) = ((h & 0x0F) as usize, (h >> 4) as usize);
        if ls == 0 || ls > 8 || os > 8 || p.len() < 1 + ls + os {
            return None;
        }
        let mut len: u64 = 0;
        for (i, b) in p[1..1 + ls].iter().enumerate() {
            len |= u64::from(*b) << (8 * i);
        }
        if os == 0 {
            out.push((None, len));
        } else {
            let mut delta: i64 = 0;
            for (i, b) in p[1 + ls..1 + ls + os].iter().enumerate() {
                delta |= i64::from(*b) << (8 * i);
            }
            // Extiende el signo del último byte.
            if os < 8 && p[ls + os] & 0x80 != 0 {
                delta |= -1i64 << (8 * os);
            }
            lcn = lcn.checked_add(delta)?;
            if lcn < 0 {
                return None;
            }
            out.push((Some(lcn as u64), len));
        }
        p = &p[1 + ls + os..];
    }
    Some(out)
}

/// Los tramos del contenido principal de un registro (el de la propia MFT, que está en el registro 0).
pub(super) fn data_runs(rec: &[u8]) -> Option<Vec<(Option<u64>, u64)>> {
    let used = (u32le(rec, 0x18) as usize).min(rec.len());
    let mut off = u16le(rec, 0x14) as usize;
    while off + 16 <= used {
        let ty = u32le(rec, off);
        let len = u32le(rec, off + 4) as usize;
        if ty == ATTR_END || len < 16 || off + len > used {
            return None;
        }
        let a = &rec[off..off + len];
        if ty == ATTR_DATA && a[9] == 0 && a[8] != 0 && u64le(a, 0x10) == 0 {
            let mp = u16le(a, 0x20) as usize;
            return decode_runs(a.get(mp..)?);
        }
        off += len;
    }
    None
}

// ---------- La tabla en memoria ----------

const F_IN_USE: u8 = 1;
const F_DIR: u8 = 2;
const F_NAMED: u8 = 4;
/// Con «reparse» (enlace, unión, OneDrive…): no se cuenta ni se entra, como en el recorrido normal.
const F_SKIP: u8 = 8;

/// Un registro por archivo o carpeta, en columnas para que millones quepan en poca memoria.
#[derive(Default)]
pub(super) struct Table {
    parent: Vec<u32>,
    size: Vec<u64>,
    modified: Vec<u64>,
    flags: Vec<u8>,
    ns: Vec<u8>,
    name_at: Vec<(u32, u16)>,
    names: String,
}

impl Table {
    pub(super) fn with_len(n: usize) -> Table {
        Table { parent: vec![0; n], size: vec![0; n], modified: vec![0; n], flags: vec![0; n], ns: vec![0; n], name_at: vec![(0, 0); n], names: String::new() }
    }

    fn name(&self, i: usize) -> &str {
        let (at, len) = self.name_at[i];
        self.names.get(at as usize..at as usize + len as usize).unwrap_or("")
    }

    /// Guarda lo leído del registro `idx` (o de su base, si es una extensión).
    pub(super) fn put(&mut self, idx: usize, r: Record) {
        let n = self.flags.len();
        let i = if r.base != 0 { r.base as usize } else { idx };
        if i >= n {
            return;
        }
        if r.base == 0 {
            if !r.in_use {
                return;
            }
            self.flags[i] |= F_IN_USE | if r.is_dir { F_DIR } else { 0 };
        }
        if let Some(a) = r.attrs {
            if a & REPARSE != 0 {
                self.flags[i] |= F_SKIP;
            }
        }
        if let Some(m) = r.modified {
            self.modified[i] = m;
        }
        if let Some(s) = r.size {
            self.size[i] = s;
        }
        if let Some((parent, name, ns)) = r.name {
            let take = self.flags[i] & F_NAMED == 0 || (self.ns[i] == NS_DOS && ns != NS_DOS);
            if take && name.len() <= usize::from(u16::MAX) && parent <= u64::from(u32::MAX) {
                let at = self.names.len() as u32;
                self.names.push_str(&name);
                self.name_at[i] = (at, name.len() as u16);
                self.parent[i] = parent as u32;
                self.ns[i] = ns;
                self.flags[i] |= F_NAMED;
            }
        }
    }

    /// ¿Es `target` una carpeta en uso de este volumen? Si no, el número de registro no es de esta MFT.
    pub(super) fn is_folder(&self, target: u32) -> bool {
        let t = target as usize;
        t < self.flags.len() && self.flags[t] & (F_IN_USE | F_DIR) == F_IN_USE | F_DIR
    }

    /// Monta el árbol de la carpeta `target` (su número de registro) como lo haría el recorrido normal.
    pub(super) fn build(&self, target: u32, root_path: &Path, c: &Counters) -> (Node, Tops) {
        let n = self.flags.len();
        let target = target as usize;
        let root_name = root_path.file_name().map_or_else(|| root_path.display().to_string(), |x| x.to_string_lossy().into_owned());
        let empty = || (Node { name: root_name.clone(), size: 0, files: 0, kinds: [0; KINDS.len()], modified: 0, children: vec![] }, Tops::default());
        if target >= n || self.flags[target] & (F_IN_USE | F_DIR) != F_IN_USE | F_DIR {
            return empty();
        }

        // ¿Cuelga cada carpeta de `target`? 0 sin saber, 1 dentro, 2 fuera.
        let mut inside = vec![0u8; n];
        inside[target] = 1;
        let mut chain: Vec<usize> = Vec::new();
        let is_dir = |i: usize| self.flags[i] & (F_IN_USE | F_DIR | F_NAMED) == F_IN_USE | F_DIR | F_NAMED;
        let mut resolve = |start: usize, inside: &mut Vec<u8>| -> bool {
            chain.clear();
            let mut i = start;
            let verdict = loop {
                if inside[i] != 0 {
                    break inside[i];
                }
                // Archivos internos de NTFS, carpetas rotas, bucles o demasiada profundidad: fuera. También lo que
                // el recorrido normal no cuenta: enlaces y carpetas de la nube, y System Volume Information
                // (copias de Windows que ni se pueden mandar a la papelera).
                if (i as u32) < FIRST_USER_RECORD
                    || !is_dir(i)
                    || chain.len() > 4096
                    || self.flags[i] & F_SKIP != 0
                    || (self.parent[i] == ROOT_RECORD && self.name(i).eq_ignore_ascii_case("System Volume Information"))
                {
                    break 2;
                }
                chain.push(i);
                let p = self.parent[i] as usize;
                if p >= n || p == i {
                    break 2;
                }
                i = p;
            };
            for &d in &chain {
                inside[d] = verdict;
            }
            verdict == 1
        };

        // Carpetas de dentro, en orden compacto.
        let mut dir_id = vec![u32::MAX; n];
        let mut dirs: Vec<usize> = vec![target];
        dir_id[target] = 0;
        // `i` es el número de registro: indexa varias columnas a la vez.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            if i != target && is_dir(i) && resolve(i, &mut inside) {
                dir_id[i] = dirs.len() as u32;
                dirs.push(i);
            }
        }

        #[derive(Clone, Default)]
        struct Agg {
            size: u64,
            files: u64,
            kinds: [u64; KINDS.len()],
            modified: u64,
        }
        let mut agg = vec![Agg::default(); dirs.len()];
        // Los más grandes: (tamaño, registro). Los caminos se montan solo para los que se quedan.
        let mut all: Vec<(u64, u32)> = Vec::new();
        let mut by_kind: Vec<Vec<(u64, u32)>> = vec![Vec::new(); KINDS.len()];
        let mut files = 0u64;
        let mut bytes = 0u64;
        for i in 0..n {
            let f = self.flags[i];
            if f & (F_IN_USE | F_DIR | F_NAMED | F_SKIP) != F_IN_USE | F_NAMED || (i as u32) < FIRST_USER_RECORD {
                continue;
            }
            let p = self.parent[i] as usize;
            if p >= n || dir_id[p] == u32::MAX {
                continue;
            }
            let (len, kind) = (self.size[i], kind_of(self.name(i)));
            let a = &mut agg[dir_id[p] as usize];
            a.size += len;
            a.files += 1;
            a.kinds[kind] += len;
            a.modified = a.modified.max(self.modified[i]);
            all.push((len, i as u32));
            if len >= KIND_MIN {
                by_kind[kind].push((len, i as u32));
            }
            files += 1;
            bytes += len;
            if files.is_multiple_of(65_536) {
                c.files.store(files, Ordering::Relaxed);
                c.bytes.store(bytes, Ordering::Relaxed);
            }
        }
        c.files.store(files, Ordering::Relaxed);
        c.bytes.store(bytes, Ordering::Relaxed);

        // De las más hondas a la de arriba, cada carpeta suma lo suyo a su madre.
        let mut depth = vec![0u32; dirs.len()];
        for k in 1..dirs.len() {
            let (mut i, mut d) = (dirs[k], 0u32);
            while i != target && d < 4096 {
                i = self.parent[i] as usize;
                d += 1;
            }
            depth[k] = d;
        }
        let mut order: Vec<usize> = (1..dirs.len()).collect();
        order.sort_unstable_by_key(|&k| std::cmp::Reverse(depth[k]));
        let mut kids: Vec<Vec<u32>> = vec![Vec::new(); dirs.len()];
        for &k in &order {
            let parent = dir_id[self.parent[dirs[k]] as usize] as usize;
            let child = agg[k].clone();
            let up = &mut agg[parent];
            up.size += child.size;
            up.files += child.files;
            up.modified = up.modified.max(child.modified);
            for (mine, theirs) in up.kinds.iter_mut().zip(child.kinds) {
                *mine += theirs;
            }
            if child.size >= MIN_NODE {
                kids[parent].push(k as u32);
            }
        }

        fn node(t: &Table, k: usize, name: String, dirs: &[usize], agg: &[Agg], kids: &[Vec<u32>]) -> Node {
            let a = &agg[k];
            let mut children: Vec<Node> = kids[k].iter().map(|&c| node(t, c as usize, t.name(dirs[c as usize]).to_string(), dirs, agg, kids)).collect();
            children.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
            Node { name, size: a.size, files: a.files, kinds: a.kinds, modified: a.modified, children }
        }
        let tree = node(self, 0, root_name, &dirs, &agg, &kids);

        let path_of = |i: usize| -> PathBuf {
            let mut parts = Vec::new();
            let mut j = i;
            while j != target && parts.len() < 4096 {
                parts.push(self.name(j));
                j = self.parent[j] as usize;
            }
            let mut p = root_path.to_path_buf();
            for s in parts.iter().rev() {
                p.push(s);
            }
            p
        };
        let pick = |list: &mut Vec<(u64, u32)>, keep: usize| -> Vec<super::TopFile> {
            if list.len() > keep {
                list.select_nth_unstable_by_key(keep, |x| std::cmp::Reverse(x.0));
                list.truncate(keep);
            }
            list.iter().map(|&(len, i)| (len, path_of(i as usize), self.modified[i as usize])).collect()
        };
        let mut tops = Tops { all: pick(&mut all, TOP_FILES), ..Default::default() };
        for (mine, list) in tops.by_kind.iter_mut().zip(by_kind.iter_mut()) {
            *mine = pick(list, KIND_TOP);
        }
        (tree, tops)
    }
}

// ---------- Leer el volumen (Windows) ----------

#[cfg(windows)]
mod win {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::FileExt;
    use std::os::windows::io::AsRawHandle;

    const FSCTL_GET_NTFS_VOLUME_DATA: u32 = 0x0009_0064;
    /// Lo que se lee de una vez (y se cuenta mientras se lee el siguiente trozo).
    const CHUNK: u64 = 8 * 1024 * 1024;

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// La letra de la unidad (`C:`) si la carpeta está en un volumen NTFS con letra.
    fn ntfs_drive(path: &Path) -> Option<String> {
        use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};
        let p = wide(&path.to_string_lossy());
        let mut vol = [0u16; 512];
        // SAFETY: búferes locales con su tamaño.
        if unsafe { GetVolumePathNameW(p.as_ptr(), vol.as_mut_ptr(), vol.len() as u32) } == 0 {
            return None;
        }
        let end = vol.iter().position(|&c| c == 0).unwrap_or(vol.len());
        let v = String::from_utf16_lossy(&vol[..end]);
        let b = v.as_bytes();
        // Solo «X:\»: carpetas montadas y rutas de red siguen por el recorrido normal.
        if b.len() != 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
            return None;
        }
        let mut fs = [0u16; 32];
        let root = wide(&v);
        // SAFETY: búferes locales con su tamaño; el resto de salidas no se piden.
        let ok = unsafe {
            GetVolumeInformationW(root.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), fs.as_mut_ptr(), fs.len() as u32)
        };
        let fend = fs.iter().position(|&c| c == 0).unwrap_or(fs.len());
        (ok != 0 && String::from_utf16_lossy(&fs[..fend]) == "NTFS").then(|| v[..2].to_string())
    }

    /// El número de registro de la MFT de una carpeta y el número de serie de su volumen (si la carpeta es
    /// un enlace a otro disco, el número de serie no coincide).
    fn record_of(path: &Path) -> Option<(u32, u32)> {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        let p = wide(&path.to_string_lossy());
        // SAFETY: ruta local terminada en 0; el identificador se cierra siempre.
        unsafe {
            let h = CreateFileW(p.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, std::ptr::null(), OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, std::ptr::null_mut());
            if h == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            let ok = GetFileInformationByHandle(h, &mut info);
            CloseHandle(h);
            if ok == 0 {
                return None;
            }
            let id = ((u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)) & MASK48;
            Some((u32::try_from(id).ok()?, info.dwVolumeSerialNumber))
        }
    }

    /// ¿Tiene cabezal (disco mecánico)? Si Windows no lo sabe, se supone que sí.
    fn seek_penalty(f: &std::fs::File) -> bool {
        use windows_sys::Win32::System::IO::DeviceIoControl;
        const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
        // STORAGE_PROPERTY_QUERY: StorageDeviceSeekPenaltyProperty (7), PropertyStandardQuery (0).
        let mut query = [0u8; 12];
        query[0..4].copy_from_slice(&7u32.to_le_bytes());
        let mut out = [0u8; 12];
        let mut got = 0u32;
        // SAFETY: búferes locales con su tamaño; el identificador es del volumen abierto.
        let ok = unsafe {
            DeviceIoControl(f.as_raw_handle() as _, IOCTL_STORAGE_QUERY_PROPERTY, query.as_ptr().cast(), query.len() as u32, out.as_mut_ptr().cast(), out.len() as u32, &mut got, std::ptr::null_mut())
        };
        // DEVICE_SEEK_PENALTY_DESCRIPTOR: IncursSeekPenalty en el byte 8.
        ok == 0 || got < 9 || out[8] != 0
    }

    struct Volume {
        serial: u64,
        bytes_per_sector: u64,
        bytes_per_cluster: u64,
        record: u64,
        mft_bytes: u64,
        mft_start_lcn: u64,
    }

    fn volume_data(f: &std::fs::File) -> Option<Volume> {
        use windows_sys::Win32::System::IO::DeviceIoControl;
        let mut out = [0u8; 128];
        let mut got = 0u32;
        // SAFETY: búfer de salida local con su tamaño; el identificador es del archivo abierto.
        let ok = unsafe {
            DeviceIoControl(f.as_raw_handle() as _, FSCTL_GET_NTFS_VOLUME_DATA, std::ptr::null(), 0, out.as_mut_ptr().cast(), out.len() as u32, &mut got, std::ptr::null_mut())
        };
        if ok == 0 || got < 72 {
            return None;
        }
        let v = Volume {
            serial: u64le(&out, 0),
            bytes_per_sector: u64::from(u32le(&out, 40)),
            bytes_per_cluster: u64::from(u32le(&out, 44)),
            record: u64::from(u32le(&out, 48)),
            mft_bytes: u64le(&out, 56),
            mft_start_lcn: u64le(&out, 64),
        };
        let sane = |x: u64| x.is_power_of_two() && (512..=2 * 1024 * 1024).contains(&x);
        // Con clústeres más pequeños que un registro, un registro podría quedar partido entre dos tramos: no.
        (sane(v.bytes_per_sector) && sane(v.bytes_per_cluster) && v.record.is_power_of_two() && (1024..=65_536).contains(&v.record) && v.bytes_per_cluster >= v.record && v.mft_bytes > 0)
            .then_some(v)
    }

    fn read_exact_at(f: &std::fs::File, buf: &mut [u8], mut at: u64) -> bool {
        let mut done = 0;
        while done < buf.len() {
            match f.seek_read(&mut buf[done..], at) {
                Ok(0) | Err(_) => return false,
                Ok(k) => {
                    done += k;
                    at += k as u64;
                }
            }
        }
        true
    }

    pub fn scan(root: &Path, c: &Counters) -> Option<(Node, Tops)> {
        let drive = ntfs_drive(root)?;
        let (target, serial) = record_of(root)?;
        // Sin administrador, Windows no deja abrir el volumen: se sigue por el camino normal.
        let f = std::fs::OpenOptions::new().read(true).open(format!(r"\\.\{drive}")).ok()?;
        let v = volume_data(&f)?;
        // La carpeta es un enlace a otro disco: su número de registro no es de esta MFT.
        if v.serial as u32 != serial {
            return None;
        }
        // Una subcarpeta en un disco mecánico: recorrerla sale más a cuenta que leer toda la MFT.
        if target != ROOT_RECORD && seek_penalty(&f) {
            return None;
        }

        // El registro 0 es la propia MFT: dice en qué tramos del disco está.
        let first_len = v.record.max(v.bytes_per_sector).div_ceil(v.bytes_per_sector) * v.bytes_per_sector;
        let mut first = vec![0u8; first_len as usize];
        if !read_exact_at(&f, &mut first, v.mft_start_lcn.checked_mul(v.bytes_per_cluster)?) {
            return None;
        }
        let rec0 = &mut first[..v.record as usize];
        if !fixup(rec0) {
            return None;
        }
        let runs = data_runs(rec0)?;
        let mapped: u64 = runs.iter().map(|(_, len)| len.saturating_mul(v.bytes_per_cluster)).sum();
        // Si la MFT está tan troceada que su mapa no cabe en el registro 0, no se arriesga.
        if mapped < v.mft_bytes {
            log::info!("Análisis de espacio: la MFT de {drive} está muy fragmentada; se usa el recorrido normal");
            return None;
        }
        let records = (v.mft_bytes / v.record) as usize;
        let mut table = Table::with_len(records);
        log::info!("Análisis de espacio: leyendo la MFT de {drive} ({records} registros, {} MB)", v.mft_bytes / (1024 * 1024));

        // Un hilo lee del disco mientras este cuenta lo ya leído.
        let rec_len = v.record as usize;
        let (tx, rx) = std::sync::mpsc::sync_channel::<(usize, Vec<u8>)>(2);
        let reader = {
            let independent = c.independent;
            std::thread::Builder::new()
                .name("space-mft".into())
                .spawn(move || -> bool {
                    let mut vcn_bytes: u64 = 0;
                    for (lcn, len) in runs {
                        let run_bytes = len.saturating_mul(v.bytes_per_cluster);
                        let mut done: u64 = 0;
                        while done < run_bytes && vcn_bytes + done < v.mft_bytes {
                            if !independent && CANCEL.load(Ordering::Relaxed) {
                                return true;
                            }
                            let want = CHUNK.min(run_bytes - done).min(v.mft_bytes - (vcn_bytes + done));
                            // Lecturas en sectores enteros: los registros no cruzan un sector.
                            let size = want.div_ceil(v.bytes_per_sector) * v.bytes_per_sector;
                            let first_rec = ((vcn_bytes + done) / v.record) as usize;
                            let mut buf = vec![0u8; size as usize];
                            if let Some(lcn) = lcn {
                                let Some(at) = lcn.checked_mul(v.bytes_per_cluster).and_then(|x| x.checked_add(done)) else { return false };
                                if !read_exact_at(&f, &mut buf, at) {
                                    return false;
                                }
                            }
                            buf.truncate(want as usize);
                            if tx.send((first_rec, buf)).is_err() {
                                return true;
                            }
                            done += want;
                        }
                        vcn_bytes += run_bytes;
                        if vcn_bytes >= v.mft_bytes {
                            break;
                        }
                    }
                    true
                })
                .ok()?
        };
        let mut seen = 0u64;
        for (first_rec, mut buf) in rx {
            for (k, rec) in buf.chunks_exact_mut(rec_len).enumerate() {
                if fixup(rec) {
                    let r = parse(rec);
                    if r.in_use && r.base == 0 && !r.is_dir {
                        seen += 1;
                    }
                    table.put(first_rec + k, r);
                }
            }
            c.files.store(seen, Ordering::Relaxed);
        }
        if !reader.join().unwrap_or(false) {
            log::warn!("Análisis de espacio: no se pudo leer la MFT de {drive}; se usa el recorrido normal");
            return None;
        }
        if !c.independent && CANCEL.load(Ordering::Relaxed) {
            return Some((Node { name: String::new(), size: 0, files: 0, kinds: [0; KINDS.len()], modified: 0, children: vec![] }, Tops::default()));
        }
        if !table.is_folder(target) {
            log::warn!("Análisis de espacio: la carpeta no está en la MFT de {drive}; se usa el recorrido normal");
            return None;
        }
        Some(table.build(target, root, c))
    }
}

/// Analiza `root` leyendo la MFT. `None` si no se puede: sin administrador, no es NTFS, etc.
#[cfg(windows)]
pub(super) fn scan(root: &Path, c: &Counters) -> Option<(Node, Tops)> {
    win::scan(root, c)
}

#[cfg(not(windows))]
pub(super) fn scan(_: &Path, _: &Counters) -> Option<(Node, Tops)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un registro de 1 KB con sus atributos, como lo escribe NTFS (con la corrección de fin de sector).
    fn record(flags: u16, base: u64, attrs: &[Vec<u8>]) -> Vec<u8> {
        let mut r = vec![0u8; 1024];
        r[0..4].copy_from_slice(b"FILE");
        r[4..6].copy_from_slice(&0x30u16.to_le_bytes()); // update sequence en 0x30
        r[6..8].copy_from_slice(&3u16.to_le_bytes()); // control + 2 sectores
        r[0x14..0x16].copy_from_slice(&0x38u16.to_le_bytes());
        r[0x16..0x18].copy_from_slice(&flags.to_le_bytes());
        r[0x20..0x28].copy_from_slice(&base.to_le_bytes());
        let mut off = 0x38;
        for a in attrs {
            r[off..off + a.len()].copy_from_slice(a);
            off += a.len();
        }
        r[off..off + 4].copy_from_slice(&ATTR_END.to_le_bytes());
        r[0x18..0x1C].copy_from_slice(&((off + 8) as u32).to_le_bytes());
        // Corrección: los bytes reales al array, el número de control al final de cada sector.
        r[0x30..0x32].copy_from_slice(&[0xAB, 0xCD]);
        for i in 1..3 {
            let end = i * 512;
            let real = [r[end - 2], r[end - 1]];
            r[0x30 + i * 2..0x32 + i * 2].copy_from_slice(&real);
            r[end - 2..end].copy_from_slice(&[0xAB, 0xCD]);
        }
        r
    }

    fn resident(ty: u32, name_len: u8, value: &[u8]) -> Vec<u8> {
        let len = (0x18 + value.len()).div_ceil(8) * 8;
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&ty.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[9] = name_len;
        a[0x10..0x14].copy_from_slice(&(value.len() as u32).to_le_bytes());
        a[0x14..0x16].copy_from_slice(&0x18u16.to_le_bytes());
        a[0x18..0x18 + value.len()].copy_from_slice(value);
        a
    }

    fn std_info(modified_epoch: u64, attrs: u32) -> Vec<u8> {
        let mut v = vec![0u8; 0x48];
        let ft = modified_epoch * 10_000_000 + 116_444_736_000_000_000;
        v[8..16].copy_from_slice(&ft.to_le_bytes());
        v[0x20..0x24].copy_from_slice(&attrs.to_le_bytes());
        resident(ATTR_STANDARD_INFORMATION, 0, &v)
    }

    fn file_name(parent: u64, name: &str, ns: u8) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut v = vec![0u8; 0x42 + units.len() * 2];
        v[0..8].copy_from_slice(&parent.to_le_bytes());
        v[0x40] = units.len() as u8;
        v[0x41] = ns;
        for (i, u) in units.iter().enumerate() {
            v[0x42 + i * 2..0x44 + i * 2].copy_from_slice(&u.to_le_bytes());
        }
        resident(ATTR_FILE_NAME, 0, &v)
    }

    fn data_nonresident(size: u64, runs: &[u8]) -> Vec<u8> {
        let len = (0x40 + runs.len()).div_ceil(8) * 8;
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&ATTR_DATA.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[8] = 1;
        a[0x20..0x22].copy_from_slice(&0x40u16.to_le_bytes());
        a[0x30..0x38].copy_from_slice(&size.to_le_bytes());
        a[0x40..0x40 + runs.len()].copy_from_slice(runs);
        a
    }

    fn parsed(mut r: Vec<u8>) -> Record {
        assert!(fixup(&mut r));
        parse(&r)
    }

    #[test]
    fn fixup_restaura_los_bytes_y_rechaza_lo_roto() {
        // Los dos últimos bytes de cada sector vuelven a ser los de verdad (los guarda el array aparte).
        let mut r = record(1, 0, &[]);
        let mut real = r.clone();
        real[0x30 + 2] = 0x11;
        real[0x30 + 5] = 0x22;
        r[0x32..0x36].copy_from_slice(&real[0x32..0x36]);
        assert!(fixup(&mut r));
        assert_eq!((r[510], r[1023]), (0x11, 0x22));
        // Otra cosa que no es un registro de archivo.
        let mut bad = record(1, 0, &[]);
        bad[0..4].copy_from_slice(b"BAAD");
        assert!(!fixup(&mut bad));
        // Sector a medio escribir: el número de control no coincide.
        let mut torn = record(1, 0, &[]);
        torn[1022] = 0;
        assert!(!fixup(&mut torn));
    }

    #[test]
    fn lee_nombre_tamano_fecha_y_carpeta() {
        let r = parsed(record(1, 0, &[std_info(1_700_000_000, 0x20), file_name(0x0001_0000_0000_0040, "Informe final.docx", 1), resident(ATTR_DATA, 0, &[7u8; 300])]));
        assert!(r.in_use && !r.is_dir);
        assert_eq!(r.name, Some((0x40, "Informe final.docx".to_string(), 1)));
        assert_eq!(r.size, Some(300));
        assert_eq!(r.modified, Some(1_700_000_000));
        // Contenido fuera del registro: el tamaño real.
        let big = parsed(record(1, 0, &[file_name(5, "video.mp4", 3), data_nonresident(4_000_000_000, &[0x21, 0x10, 0x00, 0x01, 0])]));
        assert_eq!(big.size, Some(4_000_000_000));
    }

    #[test]
    fn el_nombre_largo_gana_al_8_3_y_los_flujos_con_nombre_no_cuentan() {
        let r = parsed(record(1, 0, &[file_name(5, "INFORM~1.DOC", NS_DOS), file_name(5, "Informe.docx", 1), resident(ATTR_DATA, 4, &[1u8; 50])]));
        assert_eq!(r.name.unwrap().1, "Informe.docx");
        assert_eq!(r.size, None, "un flujo con nombre (Zone.Identifier…) no es el contenido");
        let d = parsed(record(3, 0, &[file_name(5, "Documentos", 1)]));
        assert!(d.is_dir);
        let free = parsed(record(0, 0, &[file_name(5, "borrado.txt", 1)]));
        assert!(!free.in_use && free.name.is_none());
    }

    #[test]
    fn tramos_de_la_mft() {
        // 0x10 clústeres en 0x100, luego 0x20 clústeres 0x80 más atrás (desplazamiento negativo), luego un hueco.
        let runs = [0x21, 0x10, 0x00, 0x01, 0x21, 0x20, 0x80, 0xFF, 0x01, 0x05, 0];
        assert_eq!(decode_runs(&runs), Some(vec![(Some(0x100), 0x10), (Some(0x80), 0x20), (None, 5)]));
        assert_eq!(decode_runs(&[0x21, 0x10]), None, "cortado a medias");
        let r = record(1, 0, &[data_nonresident(1 << 20, &runs)]);
        let mut r2 = r.clone();
        assert!(fixup(&mut r2));
        assert_eq!(data_runs(&r2).map(|x| x.len()), Some(3));
    }

    /// Un volumen de juguete: raíz (5), Usuarios (30) → Ana (31) → vídeo y documento; Windows (32); un
    /// archivo de NTFS (2) y uno solo en la nube que no ocupan.
    fn toy() -> Table {
        let mut t = Table::with_len(40);
        let put = |t: &mut Table, i: usize, r: Vec<u8>| t.put(i, parsed(r));
        put(&mut t, 5, record(3, 0, &[file_name(5, ".", 1)]));
        put(&mut t, 2, record(1, 0, &[file_name(5, "$LogFile", 1), resident(ATTR_DATA, 0, &[0; 64])]));
        put(&mut t, 30, record(3, 0, &[file_name(5, "Usuarios", 1)]));
        put(&mut t, 31, record(3, 0, &[file_name(30, "Ana", 1)]));
        put(&mut t, 32, record(3, 0, &[file_name(5, "Windows", 1)]));
        put(&mut t, 33, record(1, 0, &[std_info(100, 0), file_name(31, "boda.mp4", 1), data_nonresident(3 << 20, &[0x11, 1, 1, 0])]));
        put(&mut t, 34, record(1, 0, &[std_info(200, 0), file_name(31, "notas.txt", 1), resident(ATTR_DATA, 0, &[1; 10])]));
        // 35 tiene sus atributos repartidos: el base solo con el nombre corto; el largo y el contenido, en 37.
        put(&mut t, 35, record(1, 0, &[std_info(300, 0), file_name(32, "KERNEL~1.DLL", NS_DOS)]));
        put(&mut t, 36, record(1, 0, &[std_info(50, REPARSE | 0x0040_0000), file_name(31, "nube.pdf", 1), data_nonresident(9 << 20, &[0x11, 1, 1, 0])]));
        put(&mut t, 37, record(1, 35, &[file_name(32, "kernel.dll", 1), data_nonresident(5 << 20, &[0x11, 1, 1, 0])]));
        // Una unión (reparse) con algo dentro, y System Volume Information: ninguna cuenta.
        put(&mut t, 24, record(3, 0, &[std_info(0, REPARSE | 0x10), file_name(5, "Datos", 1)]));
        put(&mut t, 25, record(1, 0, &[file_name(24, "dentro.iso", 1), data_nonresident(7 << 20, &[0x11, 1, 1, 0])]));
        put(&mut t, 26, record(3, 0, &[file_name(5, "System Volume Information", 1)]));
        put(&mut t, 27, record(1, 0, &[file_name(26, "{copia}", 1), data_nonresident(8 << 20, &[0x11, 1, 1, 0])]));
        t
    }

    #[test]
    fn monta_el_arbol_como_el_recorrido() {
        let t = toy();
        let (tree, tops) = t.build(5, Path::new("C:\\"), &Counters::default());
        assert_eq!(tree.files, 3, "sin el de NTFS, el de la nube, lo de la unión ni lo de System Volume Information");
        assert_eq!(tree.size, (3 << 20) + 10 + (5 << 20));
        assert_eq!(tree.children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Windows", "Usuarios"]);
        let users = &tree.children[1];
        assert_eq!((users.size, users.modified), ((3 << 20) + 10, 200));
        assert_eq!(users.children[0].name, "Ana");
        assert_eq!(tree.kinds[0], 3 << 20, "vídeo");
        assert_eq!(tops.all.iter().map(|x| x.0).max(), Some(5 << 20));
        assert!(tops.all.iter().any(|x| x.1 == Path::new("C:\\Windows\\kernel.dll")), "el nombre largo de la extensión gana al 8.3");
        assert!(t.is_folder(5) && t.is_folder(31) && !t.is_folder(33) && !t.is_folder(39));
        let video = tops.by_kind[0].first().unwrap();
        assert_eq!(video.1, PathBuf::from("C:\\Usuarios\\Ana\\boda.mp4"));
        // Solo una subcarpeta.
        let (ana, _) = t.build(31, Path::new("C:\\Usuarios\\Ana"), &Counters::default());
        assert_eq!((ana.files, ana.size, ana.name.as_str()), (2, (3 << 20) + 10, "Ana"));
    }

    #[test]
    fn bucles_y_carpetas_huerfanas_no_cuelgan_ni_cuentan() {
        let mut t = toy();
        // 38 y 39 se apuntan una a otra: no cuelgan de la raíz.
        t.put(38, parsed(record(3, 0, &[file_name(39, "a", 1)])));
        t.put(39, parsed(record(3, 0, &[file_name(38, "b", 1)])));
        let (tree, _) = t.build(5, Path::new("C:\\"), &Counters::default());
        assert_eq!(tree.files, 3);
        // Un destino que no es carpeta: vacío.
        assert_eq!(t.build(33, Path::new("C:\\x"), &Counters::default()).0.files, 0);
    }
}
