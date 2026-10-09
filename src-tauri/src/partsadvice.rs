//! Repuestos compatibles: con lo que el equipo cuenta de sí mismo (tipo y velocidad de la memoria,
//! cuántas ranuras tiene y cuántas usa, discos puestos), dice qué se puede comprar para ampliarlo y qué
//! no encaja, para no equivocarse con la compra.
//!
//! Hay cosas que Windows no sabe: cuántas ranuras M.2 o puertos SATA libres hay, o la potencia de la
//! fuente. En esos casos lo dice claro y dice cómo averiguarlo, en vez de adivinar.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Module {
    pub capacity: u64,
    /// MHz máximos del módulo.
    pub speed: u32,
    /// MHz a los que trabaja ahora.
    pub configured: u32,
    /// Código SMBIOS: 24 DDR3, 26 DDR4, 34 DDR5…
    pub kind: u32,
    /// 8 DIMM, 12 SO-DIMM.
    pub form: u32,
    pub slot: String,
    pub part: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Disk {
    pub name: String,
    pub bus: String,
    pub media: String,
    pub size: u64,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Input {
    pub maker: String,
    pub model: String,
    pub board: String,
    pub laptop: bool,
    pub modules: Vec<Module>,
    /// Ranuras de memoria que declara la placa.
    pub slots: u32,
    /// Capacidad máxima de memoria, en bytes.
    pub max_memory: u64,
    pub disks: Vec<Disk>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// memory · storage · unknown
    pub area: String,
    /// ok · warn · info
    pub level: String,
    pub title: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Advice {
    pub model: String,
    pub items: Vec<Item>,
    /// Lo que se puede comprar, en una lista corta para copiar.
    pub shopping: Vec<String>,
    /// Texto para buscar el manual del modelo.
    pub manual_query: String,
}

pub fn ddr(kind: u32) -> Option<&'static str> {
    match kind {
        20 => Some("DDR"),
        21 => Some("DDR2"),
        24 => Some("DDR3"),
        26 => Some("DDR4"),
        34 => Some("DDR5"),
        35 => Some("LPDDR5"),
        30 => Some("LPDDR4"),
        _ => None,
    }
}

fn gb(n: u64) -> u64 {
    n / (1024 * 1024 * 1024)
}

fn item(area: &str, level: &str, title: &str, text: impl Into<String>) -> Item {
    Item { area: area.into(), level: level.into(), title: title.into(), text: text.into() }
}

/// Los tamaños comerciales de módulo, de menor a mayor.
const SIZES: [u64; 5] = [4, 8, 16, 32, 64];

/// Qué módulo comprar para añadir: el mismo tamaño que el mayor que ya hay (los pares van mejor), o 8 GB.
fn next_module(existing_gb: u64, room_gb: u64) -> Option<u64> {
    let want = if existing_gb >= 4 { existing_gb } else { 8 };
    SIZES.iter().copied().filter(|s| *s <= room_gb).rfind(|s| *s <= want.max(8)).or_else(|| SIZES.iter().copied().find(|s| *s <= room_gb))
}

pub fn advise(i: &Input) -> Advice {
    let mut a = Advice { model: [i.maker.trim(), i.model.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" "), ..Default::default() };
    a.manual_query = format!("{} manual de servicio ampliar memoria y almacenamiento", if a.model.is_empty() { i.board.trim() } else { a.model.as_str() });

    // ---- Memoria ----
    let installed = i.modules.len() as u32;
    let total_gb: u64 = i.modules.iter().map(|m| gb(m.capacity)).sum();
    if let Some(first) = i.modules.first() {
        let kind = ddr(first.kind);
        let form = if first.form == 12 { "SO-DIMM (portátil)" } else { "DIMM (sobremesa)" };
        let slowest = i.modules.iter().map(|m| if m.configured > 0 { m.configured } else { m.speed }).filter(|s| *s > 0).min().unwrap_or(0);
        let rated = i.modules.iter().map(|m| m.speed).filter(|s| *s > 0).max().unwrap_or(0);
        let kind_text = kind.map_or("tipo desconocido".to_string(), |k| k.to_string());
        let mut line = format!("{total_gb} GB en {installed} módulo(s) {kind_text} {form}");
        if slowest > 0 {
            line.push_str(&format!(", trabajando a {slowest} MHz"));
        }
        a.items.push(item("memory", "info", "Memoria instalada", format!("{line}.")));

        let free = i.slots.saturating_sub(installed);
        let max_gb = if i.max_memory > 0 { gb(i.max_memory) } else { 0 };
        let room = if max_gb > total_gb { max_gb - total_gb } else if max_gb == 0 { 64 } else { 0 };
        if i.laptop && free <= 1 && installed <= 1 && i.slots <= 2 {
            a.items.push(item("memory", "warn", "Puede llevar memoria soldada", "Muchos portátiles llevan parte de la memoria soldada a la placa y solo una ranura libre (o ninguna). Comprueba el manual antes de comprar."));
        }
        match (free, kind) {
            (0, _) => a.items.push(item("memory", "info", "Sin ranuras de memoria libres", format!("La placa declara {} ranura(s) y están todas ocupadas: para ampliar hay que cambiar un módulo por otro más grande.", i.slots.max(installed)))),
            (_, Some(k)) if room > 0 => {
                let existing = i.modules.iter().map(|m| gb(m.capacity)).max().unwrap_or(0);
                match next_module(existing, room) {
                    Some(size) => {
                        let part = format!("{size} GB {k} {} a {}{}", if first.form == 12 { "SO-DIMM" } else { "DIMM" }, if rated > 0 { rated } else { slowest }, if rated > 0 { " MHz (o más: trabajará a la velocidad del más lento)" } else { "" });
                        a.items.push(item("memory", "ok", "Se puede ampliar la memoria", format!("Hay {free} ranura(s) libre(s). Compra {part}. Máximo de la placa: {} GB.", if max_gb > 0 { max_gb.to_string() } else { "no consta".into() })));
                        a.shopping.push(format!("1 × módulo de memoria {part}"));
                    }
                    None => a.items.push(item("memory", "info", "Memoria al máximo", "La placa no admite más memoria de la que ya tiene.")),
                }
                if k == "DDR3" && total_gb < 8 {
                    a.items.push(item("memory", "info", "Memoria antigua", "Los módulos DDR3 ya casi no se fabrican: suelen salir mejor de segunda mano."));
                }
                if !matches!(k, "DDR3" | "DDR4" | "DDR5") {
                    a.items.push(item("memory", "warn", "Tipo poco común", format!("{k} es un tipo poco corriente: confirma el modelo exacto del módulo antes de comprar.")));
                }
            }
            (_, None) => a.items.push(item("memory", "warn", "No se pudo saber el tipo de memoria", "La placa no dice si es DDR3, DDR4 o DDR5. Mira la etiqueta del módulo o el manual antes de comprar.")),
            _ => a.items.push(item("memory", "info", "Memoria al máximo", "La placa no admite más memoria de la que ya tiene.")),
        }
        if a.items.iter().any(|it| it.title == "Se puede ampliar la memoria") && rated > 0 && slowest > 0 && slowest < rated {
            a.items.push(item("memory", "info", "Velocidad", format!("Los módulos son de hasta {rated} MHz pero trabajan a {slowest}: la placa o el procesador no dan más, así que no compres memoria más rápida.")));
        }
    } else {
        a.items.push(item("memory", "warn", "No se pudo leer la memoria", "Windows no ha devuelto los módulos de memoria de este equipo."));
    }

    // ---- Almacenamiento ----
    let nvme = a.items.len();
    let has_nvme = i.disks.iter().any(|d| d.bus.eq_ignore_ascii_case("NVMe"));
    let hdd_only = !i.disks.is_empty() && i.disks.iter().filter(|d| !d.bus.eq_ignore_ascii_case("USB")).all(|d| d.media.eq_ignore_ascii_case("HDD"));
    let list = i.disks.iter().filter(|d| !d.bus.eq_ignore_ascii_case("USB")).map(|d| format!("{} ({}, {} GB)", d.name.trim(), if d.bus.is_empty() { "?" } else { d.bus.as_str() }, d.size / 1_000_000_000)).collect::<Vec<_>>().join(" · ");
    if !list.is_empty() {
        a.items.push(item("storage", "info", "Discos instalados", format!("{list}.")));
    }
    if hdd_only {
        let (title, text, part) = if i.laptop {
            ("Pasar a SSD", "El disco es mecánico. Un SSD SATA de 2,5\" (7 mm) entra en casi cualquier portátil con disco de 2,5\" y es el cambio que más se nota.", "1 × SSD SATA 2,5\" 7 mm de 500 GB - 1 TB")
        } else {
            ("Pasar a SSD", "El disco es mecánico. Un SSD SATA de 2,5\" funciona en casi cualquier equipo con un puerto SATA libre (y un soporte de 3,5\" si hace falta) y es el cambio que más se nota.", "1 × SSD SATA 2,5\" de 500 GB - 1 TB (+ soporte 3,5\" si es sobremesa)")
        };
        a.items.push(item("storage", "ok", title, text));
        a.shopping.push(part.into());
    } else if has_nvme {
        a.items.push(item("storage", "info", "Ranuras M.2", "Ya hay un NVMe puesto, así que la placa tiene ranura M.2 NVMe. Windows no dice si queda otra libre: míralo en el manual o abriendo el equipo."));
    }
    if a.items.len() == nvme {
        a.items.push(item("storage", "info", "Almacenamiento", "No se pudo leer qué discos hay."));
    }

    a.items.push(item("unknown", "info", "Lo que Windows no sabe", "Cuántas ranuras M.2 y puertos SATA libres hay, la potencia de la fuente de alimentación o si algo está soldado a la placa. Para eso, el manual de servicio del modelo."));
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: u64 = 1024 * 1024 * 1024;

    fn module(gb_: u64, kind: u32, form: u32, speed: u32, conf: u32) -> Module {
        Module { capacity: gb_ * G, speed, configured: conf, kind, form, slot: "DIMM A".into(), part: String::new() }
    }

    fn sodimm_laptop() -> Input {
        Input {
            maker: "Dell".into(),
            model: "Latitude 5420".into(),
            laptop: true,
            modules: vec![module(8, 26, 12, 3200, 2933)],
            slots: 2,
            max_memory: 64 * G,
            disks: vec![Disk { name: "Samsung NVMe".into(), bus: "NVMe".into(), media: "SSD".into(), size: 512_000_000_000 }],
            ..Default::default()
        }
    }

    fn find<'a>(a: &'a Advice, title: &str) -> &'a Item {
        a.items.iter().find(|i| i.title == title).unwrap_or_else(|| panic!("falta «{title}»: {:#?}", a.items))
    }

    #[test]
    fn laptop_with_a_free_sodimm_slot() {
        let a = advise(&sodimm_laptop());
        assert_eq!(a.model, "Dell Latitude 5420");
        assert!(find(&a, "Memoria instalada").text.contains("8 GB en 1 módulo(s) DDR4 SO-DIMM (portátil), trabajando a 2933 MHz"));
        let up = find(&a, "Se puede ampliar la memoria");
        assert_eq!(up.level, "ok");
        assert!(up.text.contains("8 GB DDR4 SO-DIMM a 3200") && up.text.contains("Máximo de la placa: 64 GB"), "{}", up.text);
        assert!(find(&a, "Velocidad").text.contains("no compres memoria más rápida"));
        assert!(a.shopping[0].contains("DDR4 SO-DIMM"));
        assert!(a.items.iter().any(|i| i.title == "Ranuras M.2"));
    }

    #[test]
    fn full_slots_mean_replace_a_module() {
        let mut i = sodimm_laptop();
        i.modules.push(module(8, 26, 12, 3200, 2933));
        let a = advise(&i);
        assert!(a.items.iter().any(|x| x.title == "Sin ranuras de memoria libres"));
        assert!(a.shopping.is_empty());
    }

    #[test]
    fn desktop_ddr3_with_hdd() {
        let i = Input {
            model: "OptiPlex 3010".into(),
            maker: "Dell".into(),
            modules: vec![module(4, 24, 8, 1600, 1333)],
            slots: 4,
            max_memory: 16 * G,
            disks: vec![Disk { name: "WDC WD5000".into(), bus: "SATA".into(), media: "HDD".into(), size: 500_000_000_000 }],
            ..Default::default()
        };
        let a = advise(&i);
        let up = find(&a, "Se puede ampliar la memoria");
        assert!(up.text.contains("DDR3 DIMM"), "{}", up.text);
        assert!(a.items.iter().any(|x| x.title == "Memoria antigua"));
        assert!(find(&a, "Pasar a SSD").text.contains("2,5\""));
        assert_eq!(a.shopping.len(), 2);
    }

    #[test]
    fn memory_at_the_board_maximum() {
        let mut i = sodimm_laptop();
        i.max_memory = 8 * G;
        let a = advise(&i);
        assert!(a.items.iter().any(|x| x.title == "Memoria al máximo"));
        assert!(a.shopping.is_empty());
    }

    #[test]
    fn unknown_memory_type_asks_to_check() {
        let mut i = sodimm_laptop();
        i.modules[0].kind = 0;
        let a = advise(&i);
        assert_eq!(find(&a, "No se pudo saber el tipo de memoria").level, "warn");
    }

    #[test]
    fn no_data_is_said_plainly() {
        let a = advise(&Input::default());
        assert_eq!(find(&a, "No se pudo leer la memoria").level, "warn");
        assert!(a.items.iter().any(|x| x.title == "Lo que Windows no sabe"));
    }

    #[test]
    fn module_sizes() {
        assert_eq!(next_module(8, 56), Some(8));
        assert_eq!(next_module(16, 48), Some(16));
        assert_eq!(next_module(4, 12), Some(8));
        assert_eq!(next_module(16, 8), Some(8));
        assert_eq!(next_module(8, 0), None);
    }
}
