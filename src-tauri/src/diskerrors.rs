//! Errores de disco por disco y por tipo, con su historia día a día.
//!
//! El diagnóstico contaba los sucesos 7, 11, 51, 55 y 153 de todos los discos
//! juntos: «1204 errores graves» sin decir en qué disco ni de qué tipo. Y el
//! arreglo depende del tipo: la superficie no se cura con software; la conexión
//! y la controladora, muchas veces sí. Windows dice el disco en cada suceso
//! (`\Device\Harddisk1\DR1`, «Disk 1»): aquí se reparte.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// De qué tipo es un error, que es lo que decide qué hacer.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// Bloques que no se leen (7) o fallo de hardware del disco (154).
    Surface,
    /// Operaciones que Windows tuvo que repetir (153) o error al paginar (51).
    Retry,
    /// La controladora no respondió (11) o el disco desapareció de golpe (157).
    Connection,
    /// La controladora se reinició (129 de storahci, stornvme, iaStor…).
    Controller,
    /// Sistema de archivos dañado (Ntfs 55 y 98).
    FileSystem,
}

/// Qué tipo es un suceso, por su proveedor y su número. `None`: no es de disco.
pub fn kind_of(provider: &str, id: u32) -> Option<ErrorKind> {
    let p = provider.to_ascii_lowercase();
    match (p.as_str(), id) {
        ("disk", 7) | ("disk", 154) => Some(ErrorKind::Surface),
        ("disk", 153) | ("disk", 51) => Some(ErrorKind::Retry),
        ("disk", 11) | ("disk", 157) => Some(ErrorKind::Connection),
        ("ntfs" | "microsoft-windows-ntfs", 55 | 98) => Some(ErrorKind::FileSystem),
        (_, 129) if ["storahci", "stornvme", "storport", "iastora", "iastorac", "iastoravc", "iastorv", "amdsata", "amd_sata", "nvme"].iter().any(|c| p.contains(c)) => {
            Some(ErrorKind::Controller)
        }
        _ => None,
    }
}

/// El número de disco que nombra un suceso: «\Device\Harddisk1\DR1», «Disk 1», «disco 1».
pub fn disk_in(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    for key in ["harddisk", "disk ", "disco "] {
        let mut from = 0;
        while let Some(i) = lower[from..].find(key) {
            let start = from + i + key.len();
            let digits: String = lower[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return digits.parse().ok();
            }
            from = start;
        }
    }
    None
}

/// Un suceso tal como lo devuelve el script.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawEvent {
    pub provider: String,
    pub id: u32,
    /// Día (AAAA-MM-DD, hora local).
    pub day: String,
    /// Texto donde buscar el disco (propiedades del suceso y su mensaje).
    #[serde(default)]
    pub text: String,
    /// Disco de la letra de unidad del suceso (Ntfs), si se supo.
    #[serde(default)]
    pub letter_disk: Option<u32>,
}

/// Los errores de un disco (o, con `disk` vacío, los que no se sabe de cuál son).
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiskErrors {
    pub disk: Option<u32>,
    pub total: u32,
    pub by_kind: BTreeMap<String, u32>,
    /// Recuento por día, del más antiguo al más reciente (solo días con errores).
    pub days: Vec<(String, u32)>,
    pub last_day: String,
}

const KIND_NAMES: [(ErrorKind, &str); 5] = [
    (ErrorKind::Surface, "surface"),
    (ErrorKind::Retry, "retry"),
    (ErrorKind::Connection, "connection"),
    (ErrorKind::Controller, "controller"),
    (ErrorKind::FileSystem, "fileSystem"),
];

fn name_of(k: ErrorKind) -> &'static str {
    KIND_NAMES.iter().find(|(x, _)| *x == k).map(|(_, n)| *n).unwrap_or("other")
}

/// Reparte los sucesos por disco. Los reinicios de la controladora no son de un
/// disco: van al grupo sin disco. Ordenado: más errores primero.
pub fn group(events: &[RawEvent]) -> Vec<DiskErrors> {
    // Por disco: (cuántos de cada tipo, cuántos cada día).
    type Counts = (BTreeMap<String, u32>, BTreeMap<String, u32>);
    let mut by: BTreeMap<Option<u32>, Counts> = BTreeMap::new();
    for e in events {
        let Some(kind) = kind_of(&e.provider, e.id) else { continue };
        let disk = match kind {
            ErrorKind::Controller => None,
            ErrorKind::FileSystem => e.letter_disk.or_else(|| disk_in(&e.text)),
            _ => disk_in(&e.text),
        };
        let entry = by.entry(disk).or_default();
        *entry.0.entry(name_of(kind).to_string()).or_default() += 1;
        *entry.1.entry(e.day.chars().take(10).collect()).or_default() += 1;
    }
    let mut out: Vec<DiskErrors> = by
        .into_iter()
        .map(|(disk, (kinds, days))| {
            let total = kinds.values().sum();
            let last_day = days.keys().next_back().cloned().unwrap_or_default();
            DiskErrors { disk, total, by_kind: kinds, days: days.into_iter().collect(), last_day }
        })
        .collect();
    out.sort_by(|a, b| b.total.cmp(&a.total).then(a.disk.cmp(&b.disk)));
    out
}

const SCRIPT: &str = r#"
$since = (Get-Date).AddDays(-__DAYS__)
# Letra de unidad → disco, para los sucesos del sistema de archivos.
$letters = @{}
Get-Partition -ErrorAction SilentlyContinue | Where-Object DriveLetter | ForEach-Object { $letters["$($_.DriveLetter):".ToUpper()] = [int]$_.DiskNumber }
$out = @()
try {
  $ev = Get-WinEvent -FilterHashtable @{ LogName = 'System'; Id = 7, 11, 51, 55, 98, 129, 153, 154, 157; StartTime = $since } -ErrorAction Stop
  $out = @($ev | ForEach-Object {
    $props = ($_.Properties | ForEach-Object { "$($_.Value)" }) -join ' '
    $msg = ''
    try { $msg = $_.Message } catch {}
    $letter = $null
    if ($_.ProviderName -match 'Ntfs') {
      $m = [regex]::Match("$props $msg", '\b([A-Za-z]:)')
      if ($m.Success -and $letters.ContainsKey($m.Groups[1].Value.ToUpper())) { $letter = $letters[$m.Groups[1].Value.ToUpper()] }
    }
    [pscustomobject]@{ provider = $_.ProviderName; id = [uint32]$_.Id; day = $_.TimeCreated.ToString('yyyy-MM-dd'); text = "$props $msg"; letterDisk = $letter }
  })
} catch {
  if ($_.FullyQualifiedErrorId -notmatch 'NoMatchingEventsFound') { throw }
}
ConvertTo-Json -Depth 3 -Compress -InputObject @($out)
"#;

/// Errores de disco de los últimos `days` días, por disco y por tipo.
#[tauri::command(async)]
pub fn disk_error_history(days: Option<u32>) -> Result<Vec<DiskErrors>, String> {
    let days = days.unwrap_or(30).clamp(1, 120);
    let out = crate::pspool::query(&SCRIPT.replace("__DAYS__", &days.to_string()), Some(Duration::from_secs(60)), "Errores de disco por disco")?;
    let out = out.trim();
    if out.is_empty() {
        return Ok(vec![]);
    }
    let events: Vec<RawEvent> = serde_json::from_str(out).map_err(|e| format!("Respuesta inesperada al leer los errores de disco: {e}"))?;
    Ok(group(&events))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(provider: &str, id: u32, day: &str, text: &str) -> RawEvent {
        RawEvent { provider: provider.into(), id, day: day.into(), text: text.into(), letter_disk: None }
    }

    #[test]
    fn script_parses() {
        let errors = crate::ps::parse_errors(&SCRIPT.replace("__DAYS__", "30"));
        assert!(errors.is_empty(), "{errors}");
    }

    #[test]
    fn knows_each_kind() {
        assert_eq!(kind_of("disk", 7), Some(ErrorKind::Surface));
        assert_eq!(kind_of("disk", 153), Some(ErrorKind::Retry));
        assert_eq!(kind_of("disk", 11), Some(ErrorKind::Connection));
        assert_eq!(kind_of("Ntfs", 55), Some(ErrorKind::FileSystem));
        assert_eq!(kind_of("Microsoft-Windows-Ntfs", 98), Some(ErrorKind::FileSystem));
        assert_eq!(kind_of("storahci", 129), Some(ErrorKind::Controller));
        assert_eq!(kind_of("iaStorAC", 129), Some(ErrorKind::Controller));
        // Un 129 de otro proveedor no es de disco; tampoco un 7 que no sea de «disk».
        assert_eq!(kind_of("Microsoft-Windows-Time-Service", 129), None);
        assert_eq!(kind_of("Service Control Manager", 7), None);
    }

    #[test]
    fn finds_the_disk_in_the_event() {
        assert_eq!(disk_in(r"\Device\Harddisk1\DR1 tiene un bloque defectuoso."), Some(1));
        assert_eq!(disk_in("The IO operation at logical block address 0x1234 for Disk 2 (PDO name: \\Device\\00000035) was retried."), Some(2));
        assert_eq!(disk_in("La operación de E/S en la dirección de bloque lógico 0x10 del disco 0 se reintentó."), Some(0));
        assert_eq!(disk_in("Sin disco"), None);
        // «Disk» sin número no confunde: se sigue buscando.
        assert_eq!(disk_in("Disk error on Harddisk3"), Some(3));
    }

    #[test]
    fn groups_by_disk_kind_and_day() {
        let events = vec![
            ev("disk", 7, "2026-10-14", r"\Device\Harddisk1\DR1"),
            ev("disk", 7, "2026-10-14", r"\Device\Harddisk1\DR1"),
            ev("disk", 153, "2026-10-15", "for Disk 1 (PDO"),
            ev("disk", 153, "2026-10-12", "for Disk 2 (PDO"),
            ev("storahci", 129, "2026-10-15", r"\Device\RaidPort0"),
            RawEvent { provider: "Ntfs".into(), id: 55, day: "2026-10-16".into(), text: "D:".into(), letter_disk: Some(1) },
            ev("Service Control Manager", 7036, "2026-10-15", "nada"),
        ];
        let g = group(&events);
        assert_eq!(g[0].disk, Some(1));
        assert_eq!(g[0].total, 4);
        assert_eq!(g[0].by_kind.get("surface"), Some(&2));
        assert_eq!(g[0].by_kind.get("retry"), Some(&1));
        assert_eq!(g[0].by_kind.get("fileSystem"), Some(&1));
        assert_eq!(g[0].days, vec![("2026-10-14".into(), 2), ("2026-10-15".into(), 1), ("2026-10-16".into(), 1)]);
        assert_eq!(g[0].last_day, "2026-10-16");
        // La controladora va aparte, sin disco.
        let ctrl = g.iter().find(|d| d.disk.is_none()).unwrap();
        assert_eq!(ctrl.by_kind.get("controller"), Some(&1));
        assert_eq!(g.iter().map(|d| d.total).sum::<u32>(), 6);
    }
}
