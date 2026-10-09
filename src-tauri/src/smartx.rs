//! SMART a fondo: la tabla completa de atributos con su umbral, la salud de los
//! NVMe, la nota de 0 a 100 y la vida que le queda a un disco.
//!
//! Este archivo es solo interpretación (no toca el disco): recibe los bytes que
//! devuelve el disco y los convierte en filas que se entienden. La lectura real
//! está en `smartio.rs`.
//!
//! Lo que enseña CrystalDiskInfo, pero explicado: cada fila dice qué mide y, si
//! no está bien, qué hacer.

use serde::Serialize;

/// Una fila de la tabla de salud: un atributo SMART (SATA) o un dato del registro
/// de salud de un NVMe. Misma forma para los dos, para pintarlas con la misma tabla.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmartRow {
    /// "05", "C5"… o "nvme-spare".
    pub id: String,
    pub name: String,
    /// Qué es, en lenguaje normal.
    pub explain: String,
    pub current: Option<u32>,
    pub worst: Option<u32>,
    pub threshold: Option<u32>,
    /// Dato bruto, ya con su unidad («12 480 h», «38 °C», «8»).
    pub raw: String,
    /// ok | warn | bad
    pub status: String,
    /// Qué hacer, si el estado no es ok.
    pub action: String,
}

// ---------- SATA / ATA ----------

pub struct AtaEntry {
    pub id: u8,
    pub current: u8,
    pub worst: u8,
    pub raw: u64,
}

/// Tabla SMART de 512 bytes: 2 de versión y 30 entradas de 12 (id, flags×2, actual, peor, raw×6, reservado).
pub fn parse_ata_table(data: &[u8]) -> Vec<AtaEntry> {
    data.get(2..)
        .unwrap_or_default()
        .chunks_exact(12)
        .take(30)
        .filter(|e| e[0] != 0)
        .map(|e| {
            let mut raw = [0u8; 8];
            raw[..6].copy_from_slice(&e[5..11]);
            AtaEntry { id: e[0], current: e[3], worst: e[4], raw: u64::from_le_bytes(raw) }
        })
        .collect()
}

/// Umbrales: 2 bytes de versión y 30 entradas de 12 (id, umbral, 10 reservados).
pub fn parse_thresholds(data: &[u8]) -> Vec<(u8, u8)> {
    data.get(2..).unwrap_or_default().chunks_exact(12).take(30).filter(|e| e[0] != 0).map(|e| (e[0], e[1])).collect()
}

/// (nombre, qué es)
fn ata_info(id: u8) -> (&'static str, &'static str) {
    match id {
        1 => ("Tasa de errores de lectura", "Cuántas veces falla la lectura de la superficie. En muchos discos el número en bruto no significa nada; fíjate en el valor y el umbral."),
        2 => ("Rendimiento", "Nota que da el fabricante a la velocidad del disco. Si baja mucho, el disco se está volviendo lento."),
        3 => ("Tiempo de arranque del motor", "Lo que tarda el disco en alcanzar su velocidad al encender. Si crece, el motor se está cansando."),
        4 => ("Arranques y paradas", "Cuántas veces se ha puesto en marcha el motor. Es un contador de uso, no de fallo."),
        5 => ("Sectores reasignados", "Sectores dañados que el disco ya ha apartado y sustituido por otros de reserva. Alguno es normal; que suba con los días es mala señal."),
        7 => ("Errores de búsqueda", "Fallos al mover el cabezal a la posición correcta. En discos mecánicos con cabezal defectuoso sube."),
        8 => ("Rendimiento de búsqueda", "Nota del fabricante a la velocidad de movimiento del cabezal."),
        9 => ("Horas encendido", "Tiempo total que ha estado encendido. Sirve para saber su edad real."),
        10 => ("Reintentos de giro", "Veces que el motor no consiguió arrancar a la primera. Si es mayor que 0 hay un problema mecánico o de alimentación."),
        11 => ("Reintentos de calibración", "Veces que el disco tuvo que recalibrarse. Si sube, hay un problema mecánico."),
        12 => ("Ciclos de encendido", "Veces que se ha encendido el disco entero."),
        170 => ("Bloques de reserva", "Reserva que le queda al SSD para sustituir celdas gastadas."),
        171 => ("Fallos de programación", "Veces que el SSD no pudo escribir una celda."),
        172 => ("Fallos de borrado", "Veces que el SSD no pudo borrar un bloque."),
        173 => ("Desgaste de celdas", "Cuánto se han gastado las celdas del SSD respecto a la media."),
        174 => ("Apagados inesperados", "Veces que se cortó la luz con el disco encendido. Pueden dañar datos."),
        175 => ("Fallos del condensador", "Estado de la protección contra cortes de luz del SSD."),
        177 => ("Desgaste (nivelación)", "Vida restante del SSD según el fabricante: 100 es nuevo, 0 es agotado."),
        179 => ("Bloques de reserva usados", "Bloques de reserva que el SSD ya ha tenido que usar."),
        180 => ("Bloques de reserva libres", "Bloques de reserva que aún le quedan al SSD."),
        181 => ("Fallos de programación", "Veces que el SSD no pudo escribir."),
        182 => ("Fallos de borrado", "Veces que el SSD no pudo borrar."),
        183 => ("Fallos de enlace SATA", "Veces que la conexión SATA se cortó. Suele ser el cable o el puerto."),
        184 => ("Errores de extremo a extremo", "El dato se estropeó por dentro del propio disco, entre su memoria y la superficie."),
        187 => ("Errores no corregibles", "Errores de lectura que el disco no pudo corregir. Tiene que ser 0."),
        188 => ("Tiempos de espera", "Órdenes que el disco no contestó a tiempo. Valores altos sueltos pueden venir de un cable o de la alimentación."),
        189 => ("Cabezal fuera de altura", "El cabezal ha volado demasiado alto o bajo. Es un dato del fabricante."),
        190 => ("Temperatura del aire", "Temperatura del aire que entra en el disco."),
        191 => ("Golpes", "Veces que el disco notó un golpe o vibración."),
        192 => ("Apagados inesperados", "Veces que el disco se apagó sin avisar. Muchos pueden dañar datos."),
        193 => ("Ciclos de aparcado", "Veces que el cabezal se ha aparcado. Los portátiles lo hacen mucho; hay un máximo por diseño."),
        194 => ("Temperatura", "Temperatura actual del disco."),
        195 => ("Errores corregidos por hardware", "Errores de lectura que el disco corrigió solo. Números altos son normales en algunos fabricantes."),
        196 => ("Eventos de reasignación", "Veces que el disco intentó apartar un sector dañado."),
        197 => ("Sectores pendientes", "Sectores que el disco no puede leer y espera reescribir. Tienen que ser 0: si no, hay datos en riesgo."),
        198 => ("Sectores no corregibles", "Sectores que no se pudieron leer ni siquiera tras reintentar. Tienen que ser 0."),
        199 => ("Errores de conexión (CRC)", "Datos que se estropearon por el camino entre el disco y el equipo: cable, puerto o caja USB."),
        200 => ("Errores de escritura", "Fallos al escribir en la superficie."),
        201 => ("Errores de lectura sin corregir", "Lecturas que dieron error y no se pudieron corregir."),
        202 => ("Vida restante", "Porcentaje de vida que queda según el fabricante."),
        206 => ("Altura de vuelo del cabezal", "Margen de altura del cabezal sobre el plato."),
        231 => ("Vida restante del SSD", "Porcentaje de vida del SSD según el fabricante: 100 es nuevo."),
        232 => ("Reserva disponible", "Porcentaje de celdas de reserva que quedan."),
        233 => ("Desgaste del medio", "Desgaste de las celdas flash. 100 es nuevo; baja con el uso."),
        234 => ("Escritura acumulada", "Datos totales que ha escrito el SSD."),
        241 => ("Total escrito", "Datos totales que el equipo ha escrito en el disco."),
        242 => ("Total leído", "Datos totales que el equipo ha leído del disco."),
        245 => ("Ciclos de borrado", "Veces que se han borrado las celdas flash."),
        246 => ("Total escrito por el equipo", "Sectores escritos por el equipo desde que es nuevo."),
        _ => ("Atributo del fabricante", "Dato propio del fabricante: no tiene una interpretación general."),
    }
}

/// Qué ids se miran por su dato bruto (tienen que ser 0).
fn is_critical_counter(id: u8) -> bool {
    matches!(id, 5 | 10 | 11 | 183 | 184 | 187 | 196 | 197 | 198 | 199 | 200 | 201)
}

fn is_wear(id: u8) -> bool {
    matches!(id, 173 | 177 | 202 | 231 | 233)
}

fn is_temp(id: u8) -> bool {
    matches!(id, 190 | 194)
}

/// Dato bruto en palabras.
pub fn raw_text(id: u8, raw: u64) -> String {
    match id {
        9 => format!("{} h", group(raw & 0xFFFF_FFFF)),
        190 | 194 => format!("{} °C", raw & 0xFF),
        _ => group(raw),
    }
}

/// 12480 → «12 480» (como en el resto de la app).
pub fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// ok | warn | bad
pub fn ata_status(id: u8, current: u8, worst: u8, threshold: u8, raw: u64) -> &'static str {
    // El disco mismo dice que este atributo ya cayó al umbral: es el aviso oficial.
    if threshold > 0 && current > 0 && current <= threshold {
        return "bad";
    }
    let counter = if id == 5 || id == 187 || id == 197 || id == 198 { raw & 0xFFFF_FFFF } else { raw & 0xFFFF };
    let mut status = "ok";
    if is_critical_counter(id) && counter > 0 {
        let big = match id {
            5 | 196 => 100,
            197 | 198 | 187 => 20,
            _ => u64::MAX,
        };
        status = if counter >= big { "bad" } else { "warn" };
    }
    if is_temp(id) {
        let t = raw & 0xFF;
        if t >= 65 {
            return "bad";
        }
        if t >= 55 {
            status = "warn";
        }
    }
    if is_wear(id) && current > 0 {
        if current <= 10 {
            return "bad";
        }
        if current <= 30 {
            status = "warn";
        }
    }
    if status == "ok" && threshold > 0 && worst > 0 && worst <= threshold {
        status = "warn";
    }
    status
}

fn ata_action(id: u8) -> &'static str {
    match id {
        5 | 196 | 197 | 198 | 187 | 184 | 200 | 201 => "Haz una copia de lo importante ya. Si la cifra sube en los próximos días, cambia el disco.",
        10 | 11 => "Es un fallo mecánico o de alimentación: copia lo importante y revisa la fuente de alimentación.",
        183 | 199 => "Cambia el cable (o el puerto, o la caja USB). Si sigue, revisa la alimentación del equipo.",
        190 | 194 => "Revisa la ventilación del equipo o de la caja externa.",
        173 | 177 | 202 | 231 | 233 => "El SSD se acerca al final de su vida: ten copia al día y planifica cambiarlo.",
        _ => "Vuelve a mirarlo en unas semanas: si empeora, cambia el disco.",
    }
}

/// Las filas de la tabla, con su umbral (si se pudo leer).
pub fn ata_rows(table: &[AtaEntry], thresholds: &[(u8, u8)]) -> Vec<SmartRow> {
    table
        .iter()
        .map(|e| {
            let thr = thresholds.iter().find(|(id, _)| *id == e.id).map(|(_, t)| *t);
            let (name, explain) = ata_info(e.id);
            let status = ata_status(e.id, e.current, e.worst, thr.unwrap_or(0), e.raw);
            SmartRow {
                id: format!("{:02X}", e.id),
                name: name.into(),
                explain: explain.into(),
                current: Some(e.current as u32),
                worst: Some(e.worst as u32),
                threshold: thr.map(u32::from),
                raw: raw_text(e.id, e.raw),
                status: status.into(),
                action: if status == "ok" { String::new() } else { ata_action(e.id).into() },
            }
        })
        .collect()
}

// ---------- NVMe ----------

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NvmeHealth {
    pub critical_warning: u8,
    /// °C
    pub temperature: i32,
    pub available_spare: u8,
    pub spare_threshold: u8,
    pub percent_used: u8,
    /// TB (10^12 bytes)
    pub tb_read: f64,
    pub tb_written: f64,
    pub power_cycles: u64,
    pub power_on_hours: u64,
    pub unsafe_shutdowns: u64,
    pub media_errors: u64,
    pub error_log_entries: u64,
    /// Minutos pasados por encima de la temperatura de aviso.
    pub warning_temp_minutes: u32,
    pub critical_temp_minutes: u32,
}

fn le128(b: &[u8]) -> u128 {
    let mut a = [0u8; 16];
    a.copy_from_slice(&b[..16]);
    u128::from_le_bytes(a)
}

/// Registro de salud de un NVMe (página 2 del registro, 512 bytes).
pub fn parse_nvme_log(b: &[u8]) -> Option<NvmeHealth> {
    if b.len() < 200 {
        return None;
    }
    let kelvin = u16::from_le_bytes([b[1], b[2]]) as i32;
    // Cada «unidad de datos» son 1000 bloques de 512 bytes.
    let tb = |units: u128| units as f64 * 512_000.0 / 1e12;
    let sat = |v: u128| v.min(u64::MAX as u128) as u64;
    Some(NvmeHealth {
        critical_warning: b[0],
        temperature: if kelvin > 0 { kelvin - 273 } else { -1 },
        available_spare: b[3],
        spare_threshold: b[4],
        percent_used: b[5],
        tb_read: tb(le128(&b[32..])),
        tb_written: tb(le128(&b[48..])),
        power_cycles: sat(le128(&b[112..])),
        power_on_hours: sat(le128(&b[128..])),
        unsafe_shutdowns: sat(le128(&b[144..])),
        media_errors: sat(le128(&b[160..])),
        error_log_entries: sat(le128(&b[176..])),
        warning_temp_minutes: u32::from_le_bytes([b[192], b[193], b[194], b[195]]),
        critical_temp_minutes: u32::from_le_bytes([b[196], b[197], b[198], b[199]]),
    })
}

/// Bits del aviso crítico, en palabras.
pub fn critical_warning_text(w: u8) -> String {
    let mut parts = Vec::new();
    if w & 1 != 0 {
        parts.push("queda poca reserva");
    }
    if w & 2 != 0 {
        parts.push("temperatura fuera de límites");
    }
    if w & 4 != 0 {
        parts.push("fiabilidad degradada");
    }
    if w & 8 != 0 {
        parts.push("pasó a solo lectura");
    }
    if w & 16 != 0 {
        parts.push("falló la memoria de respaldo");
    }
    if parts.is_empty() {
        "ninguno".into()
    } else {
        parts.join(", ")
    }
}

pub fn nvme_rows(n: &NvmeHealth) -> Vec<SmartRow> {
    let row = |id: &str, name: &str, explain: &str, raw: String, status: &str, action: &str| SmartRow {
        id: id.into(),
        name: name.into(),
        explain: explain.into(),
        raw,
        status: status.into(),
        action: if status == "ok" { String::new() } else { action.into() },
        ..Default::default()
    };
    let copy = "Haz una copia de lo importante y planifica cambiarlo.";
    let mut rows = vec![
        row(
            "nvme-warn",
            "Aviso crítico",
            "El propio disco enciende un aviso si algo grave le pasa: poca reserva, temperatura, fiabilidad o solo lectura.",
            critical_warning_text(n.critical_warning),
            if n.critical_warning == 0 { "ok" } else { "bad" },
            copy,
        ),
        row(
            "nvme-temp",
            "Temperatura",
            "Temperatura actual del disco. Si pasa de unos 70 °C el disco baja la velocidad para protegerse.",
            if n.temperature >= 0 { format!("{} °C", n.temperature) } else { "—".into() },
            if n.temperature >= 80 {
                "bad"
            } else if n.temperature >= 70 {
                "warn"
            } else {
                "ok"
            },
            "Revisa la ventilación o pon un disipador en el disco.",
        ),
        row(
            "nvme-spare",
            "Reserva disponible",
            "Celdas de reserva que le quedan para sustituir las gastadas. Cuando baja del mínimo del fabricante, el disco avisa.",
            format!("{} % (mínimo {} %)", n.available_spare, n.spare_threshold),
            if n.available_spare <= n.spare_threshold {
                "bad"
            } else if n.available_spare < 50 {
                "warn"
            } else {
                "ok"
            },
            copy,
        ),
        row(
            "nvme-used",
            "Vida usada",
            "Porcentaje de la vida prevista que ya ha gastado. Puede pasar de 100 y el disco seguir funcionando, pero ya sin garantía.",
            format!("{} %", n.percent_used),
            if n.percent_used >= 100 {
                "bad"
            } else if n.percent_used >= 90 {
                "warn"
            } else {
                "ok"
            },
            copy,
        ),
        row("nvme-written", "Total escrito", "Datos totales que se han escrito en el disco desde que es nuevo.", format!("{:.1} TB", n.tb_written), "ok", ""),
        row("nvme-read", "Total leído", "Datos totales que se han leído del disco.", format!("{:.1} TB", n.tb_read), "ok", ""),
        row("nvme-hours", "Horas encendido", "Tiempo total que ha estado encendido.", format!("{} h", group(n.power_on_hours)), "ok", ""),
        row("nvme-cycles", "Ciclos de encendido", "Veces que se ha encendido el disco.", group(n.power_cycles), "ok", ""),
        row(
            "nvme-unsafe",
            "Apagados bruscos",
            "Veces que se cortó la luz o se apagó el equipo sin cerrar bien. Pueden dañar el último dato escrito.",
            group(n.unsafe_shutdowns),
            if n.unsafe_shutdowns >= 500 { "warn" } else { "ok" },
            "Apaga siempre desde Windows y, si se corta la luz a menudo, usa un SAI.",
        ),
        row(
            "nvme-media",
            "Errores de medio e integridad",
            "Veces que el disco encontró datos que no eran fiables. Tiene que ser 0.",
            group(n.media_errors),
            if n.media_errors >= 10 {
                "bad"
            } else if n.media_errors > 0 {
                "warn"
            } else {
                "ok"
            },
            copy,
        ),
        row("nvme-errlog", "Entradas del registro de errores", "Errores anotados por el disco en su registro. Pocas son normales.", group(n.error_log_entries), "ok", ""),
    ];
    if n.warning_temp_minutes > 0 || n.critical_temp_minutes > 0 {
        rows.push(row(
            "nvme-hot",
            "Tiempo demasiado caliente",
            "Minutos que el disco ha pasado por encima de su temperatura de aviso o crítica.",
            format!("{} min aviso · {} min crítica", n.warning_temp_minutes, n.critical_temp_minutes),
            if n.critical_temp_minutes > 0 { "bad" } else { "warn" },
            "Revisa la ventilación o pon un disipador en el disco.",
        ));
    }
    rows
}

// ---------- Autopruebas ----------

/// Estado de la autoprueba del disco, leído de la tabla SMART (byte 363).
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelfTest {
    /// idle | running | passed | failed | aborted
    pub state: String,
    /// 0-100 (de lo que queda por hacer).
    pub percent: u8,
    pub text: String,
    /// Minutos que dice el disco que tarda cada prueba.
    pub short_minutes: u32,
    pub extended_minutes: u32,
}

pub fn parse_self_test(data: &[u8]) -> SelfTest {
    let at = |i: usize| data.get(i).copied().unwrap_or(0);
    let status = at(363);
    let remaining = (status & 0x0F) as u32 * 10;
    let short_minutes = at(372) as u32;
    let ext = at(373);
    let extended_minutes = if ext == 0xFF { u16::from_le_bytes([at(375), at(376)]) as u32 } else { ext as u32 };
    let (state, text): (&str, &str) = match status >> 4 {
        0 => ("passed", "La última prueba terminó sin errores."),
        1 => ("aborted", "La última prueba se canceló."),
        2 => ("aborted", "La última prueba se interrumpió por un reinicio."),
        3 => ("failed", "La prueba falló: error grave del disco."),
        4 => ("failed", "La prueba falló: error desconocido del disco."),
        5 => ("failed", "La prueba falló: fallo eléctrico."),
        6 => ("failed", "La prueba falló: fallo del servo (mecánico)."),
        7 => ("failed", "La prueba encontró sectores que no se pueden leer."),
        8 => ("failed", "La prueba falló: daño en el manejo."),
        15 => ("running", "Prueba en marcha."),
        _ => ("idle", "Sin pruebas recientes."),
    };
    let percent = if state == "running" { 100u32.saturating_sub(remaining) as u8 } else { 100 };
    SelfTest { state: state.into(), percent, text: text.into(), short_minutes, extended_minutes }
}

// ---------- Nota de salud ----------

pub struct ScoreInput<'a> {
    pub media: &'a str,
    pub bus: &'a str,
    pub reallocated: u64,
    pub pending: u64,
    pub uncorrectable: u64,
    pub crc: u64,
    pub read_errors: u64,
    pub write_errors: u64,
    pub media_errors: u64,
    pub predict_failure: bool,
    /// Desgaste de un SSD en % (-1 si no se sabe).
    pub wear: i64,
    pub temperature: i64,
    pub hours: i64,
    /// Windows dice «Unhealthy».
    pub unhealthy: bool,
    /// (día, desgaste) de las últimas fotos, de la más antigua a la más reciente.
    pub wear_trend: &'a [(u32, i64)],
    pub tb_written: Option<f64>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScorePart {
    pub name: String,
    pub score: u8,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HealthScore {
    pub total: u8,
    /// Muy bueno · Bueno · Vigilar · Malo
    pub label: String,
    /// Frase para el usuario.
    pub sentence: String,
    pub parts: Vec<ScorePart>,
    /// Años que le quedan al ritmo actual (None: no se puede estimar).
    pub life_years: Option<f32>,
    pub life_text: String,
    pub tb_written: Option<f64>,
}

fn clamp(v: f64) -> u8 {
    v.round().clamp(0.0, 100.0) as u8
}

pub fn score(i: &ScoreInput) -> HealthScore {
    let ssd = i.media.eq_ignore_ascii_case("SSD") || i.bus.eq_ignore_ascii_case("NVMe");
    let part = |name: &str, s: u8, text: String| ScorePart { name: name.into(), score: s, text };

    let wear_score = if i.wear >= 0 && ssd { clamp(100.0 - i.wear as f64) } else { 100 };
    let wear_text = if i.wear >= 0 && ssd { format!("{} % de la vida gastada", i.wear) } else if ssd { "No informa del desgaste".into() } else { "Un disco mecánico no se desgasta por escritura".into() };

    let bad_sectors = i.pending * 10 + i.uncorrectable * 10 + i.reallocated * 2;
    let sect_score = clamp(100.0 - bad_sectors as f64);
    let sect_text = if bad_sectors == 0 { "Sin sectores dañados".to_string() } else { format!("{} apartados · {} pendientes · {} no corregibles", i.reallocated, i.pending, i.uncorrectable) };

    let mut err = 100.0 - (i.read_errors + i.write_errors + i.media_errors) as f64 * 10.0 - i.crc as f64 * 5.0;
    if i.predict_failure || i.unhealthy {
        err = 0.0;
    }
    let err_score = clamp(err);
    let err_text = if i.predict_failure {
        "El propio disco avisa de que va a fallar".to_string()
    } else if err_score == 100 {
        "Sin errores".to_string()
    } else {
        format!("{} de lectura/escritura · {} de conexión · {} de medio", i.read_errors + i.write_errors, i.crc, i.media_errors)
    };

    let hot = if ssd { 70.0 } else { 55.0 };
    let t = i.temperature as f64;
    let temp_score = if i.temperature < 0 || t <= 35.0 {
        100
    } else if t <= hot {
        clamp(100.0 - (t - 35.0) / (hot - 35.0) * 60.0)
    } else {
        clamp(40.0 - (t - hot) / 15.0 * 40.0)
    };
    let temp_text = if i.temperature < 0 { "No informa de la temperatura".into() } else { format!("{} °C", i.temperature) };

    let life_h = if ssd { 87_600.0 } else { 43_800.0 };
    let age_score = if i.hours <= 0 { 100 } else { clamp(100.0 - (i.hours as f64 / life_h) * 60.0).max(20) };
    let age_text = if i.hours <= 0 { "No informa de las horas".into() } else { format!("{} h encendido (≈ {:.1} años seguidos)", group(i.hours as u64), i.hours as f64 / 8760.0) };

    let parts = vec![
        part("Desgaste", wear_score, wear_text),
        part("Sectores", sect_score, sect_text),
        part("Errores", err_score, err_text),
        part("Temperatura", temp_score, temp_text),
        part("Edad", age_score, age_text),
    ];
    let weights = [0.25, 0.30, 0.25, 0.10, 0.10];
    let mut total: f64 = parts.iter().zip(weights).map(|(p, w)| p.score as f64 * w).sum();
    // Un fallo grave no se compensa con que el resto esté bien.
    if sect_score < 50 || err_score < 50 {
        total = total.min(45.0);
    }
    if i.predict_failure || i.unhealthy {
        total = total.min(20.0);
    }
    let total = clamp(total);
    let (label, sentence) = match total {
        85..=100 => ("Muy bueno", "El disco está en muy buen estado."),
        70..=84 => ("Bueno", "El disco está bien; conviene vigilar las cifras con el tiempo."),
        50..=69 => ("Vigilar", "El disco tiene señales de desgaste: ten copia de lo importante."),
        _ => ("Malo", "El disco está fallando: copia lo importante ya y cámbialo."),
    };

    let (life_years, life_text) = life(i, ssd);
    HealthScore { total, label: label.into(), sentence: sentence.into(), parts, life_years, life_text, tb_written: i.tb_written }
}

/// Años que le quedan a un SSD al ritmo de desgaste que ha llevado.
fn life(i: &ScoreInput, ssd: bool) -> (Option<f32>, String) {
    if !ssd {
        return (None, "En un disco mecánico no se puede calcular: lo que avisa es la tendencia de sectores y errores.".into());
    }
    if i.wear < 0 {
        return (None, "Este SSD no informa de su desgaste.".into());
    }
    if i.wear >= 100 {
        return (Some(0.0), "Ha agotado la vida prevista por el fabricante.".into());
    }
    // 1.º por las fotos diarias: cuánto ha subido el desgaste por día natural.
    if let (Some(first), Some(last)) = (i.wear_trend.first(), i.wear_trend.last()) {
        let days = last.0.saturating_sub(first.0) as f64;
        let up = (last.1 - first.1) as f64;
        if days >= 14.0 && up > 0.0 {
            let per_day = up / days;
            let left = (100 - i.wear) as f64 / per_day / 365.0;
            return (Some(left as f32), format!("Al ritmo de los últimos {} días (+{} % de desgaste) le quedan unos {:.1} años.", days as u32, up as i64, left));
        }
        if days >= 14.0 && up <= 0.0 && i.wear <= 5 {
            return (Some(10.0), "El desgaste casi no se mueve: le quedan más de 10 años a este ritmo.".into());
        }
    }
    // 2.º por las horas encendido: el desgaste medio por hora de toda su vida.
    if i.wear > 0 && i.hours > 0 {
        let hours_left = (100 - i.wear) as f64 * i.hours as f64 / i.wear as f64;
        // Equipo de oficina: unas 8 horas al día.
        let years = hours_left / (8.0 * 365.0);
        return (Some(years as f32), format!("Con el uso que ha tenido hasta ahora, unos {years:.1} años (suponiendo 8 h de uso al día). Será más exacto con unas semanas de fotos diarias."));
    }
    (Some(10.0), "Está casi nuevo: le quedan muchos años.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(entries: &[(u8, u8, u8, u64)]) -> Vec<u8> {
        let mut b = vec![0u8; 512];
        for (i, (id, cur, worst, raw)) in entries.iter().enumerate() {
            let o = 2 + i * 12;
            b[o] = *id;
            b[o + 3] = *cur;
            b[o + 4] = *worst;
            b[o + 5..o + 11].copy_from_slice(&raw.to_le_bytes()[..6]);
        }
        b
    }

    fn thr(entries: &[(u8, u8)]) -> Vec<u8> {
        let mut b = vec![0u8; 512];
        for (i, (id, t)) in entries.iter().enumerate() {
            let o = 2 + i * 12;
            b[o] = *id;
            b[o + 1] = *t;
        }
        b
    }

    #[test]
    fn rows_join_threshold_and_flag_pending() {
        let t = parse_ata_table(&table(&[(5, 100, 100, 0), (9, 97, 97, 12480), (197, 99, 96, 8), (199, 100, 100, 0)]));
        let th = parse_thresholds(&thr(&[(5, 10), (9, 0), (197, 0), (199, 0)]));
        let rows = ata_rows(&t, &th);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].id, "05");
        assert_eq!(rows[0].threshold, Some(10));
        assert_eq!(rows[0].status, "ok");
        assert_eq!(rows[1].raw, "12 480 h");
        assert_eq!(rows[2].id, "C5");
        assert_eq!(rows[2].status, "warn");
        assert!(!rows[2].action.is_empty());
        assert_eq!(rows[3].status, "ok");
    }

    #[test]
    fn value_at_threshold_is_bad() {
        assert_eq!(ata_status(5, 10, 10, 10, 0), "bad");
        assert_eq!(ata_status(5, 100, 100, 10, 0), "ok");
        // Cayó al umbral alguna vez pero se recuperó.
        assert_eq!(ata_status(1, 80, 6, 6, 0), "warn");
    }

    #[test]
    fn counters_escalate() {
        assert_eq!(ata_status(197, 100, 100, 0, 1), "warn");
        assert_eq!(ata_status(197, 100, 100, 0, 25), "bad");
        assert_eq!(ata_status(5, 100, 100, 36, 99), "warn");
        assert_eq!(ata_status(5, 100, 100, 36, 100), "bad");
        assert_eq!(ata_status(194, 60, 60, 0, 58), "warn");
        assert_eq!(ata_status(194, 60, 60, 0, 66), "bad");
        assert_eq!(ata_status(177, 8, 8, 0, 0), "bad");
        assert_eq!(ata_status(177, 25, 25, 0, 0), "warn");
        // El dato bruto de 9 o 12 no es un fallo aunque sea enorme.
        assert_eq!(ata_status(9, 90, 90, 0, 40_000), "ok");
    }

    #[test]
    fn nvme_log_parses_and_flags() {
        let mut b = vec![0u8; 512];
        b[1..3].copy_from_slice(&(311u16).to_le_bytes()); // 38 °C
        b[3] = 100;
        b[4] = 10;
        b[5] = 3;
        b[48..56].copy_from_slice(&(74_218_750u64).to_le_bytes()); // × 512 000 = 38 TB
        b[128..136].copy_from_slice(&(12_480u64).to_le_bytes());
        b[144..152].copy_from_slice(&(41u64).to_le_bytes());
        b[160..168].copy_from_slice(&(2u64).to_le_bytes());
        let n = parse_nvme_log(&b).unwrap();
        assert_eq!(n.temperature, 38);
        assert_eq!(n.percent_used, 3);
        assert!((n.tb_written - 38.0).abs() < 0.01);
        let rows = nvme_rows(&n);
        let media = rows.iter().find(|r| r.id == "nvme-media").unwrap();
        assert_eq!(media.status, "warn");
        assert_eq!(rows.iter().find(|r| r.id == "nvme-spare").unwrap().status, "ok");
        assert_eq!(rows.iter().find(|r| r.id == "nvme-warn").unwrap().raw, "ninguno");
        assert!(parse_nvme_log(&[0u8; 10]).is_none());
    }

    #[test]
    fn nvme_spare_below_threshold_is_bad() {
        let n = NvmeHealth { available_spare: 8, spare_threshold: 10, ..Default::default() };
        assert_eq!(nvme_rows(&n).iter().find(|r| r.id == "nvme-spare").unwrap().status, "bad");
    }

    #[test]
    fn self_test_states() {
        let mut d = vec![0u8; 512];
        d[363] = 0xF3; // en marcha, queda 30 %
        d[372] = 2;
        d[373] = 0xFF;
        d[375..377].copy_from_slice(&(120u16).to_le_bytes());
        let s = parse_self_test(&d);
        assert_eq!(s.state, "running");
        assert_eq!(s.percent, 70);
        assert_eq!(s.short_minutes, 2);
        assert_eq!(s.extended_minutes, 120);
        d[363] = 0x00;
        assert_eq!(parse_self_test(&d).state, "passed");
        d[363] = 0x70;
        assert_eq!(parse_self_test(&d).state, "failed");
        d[363] = 0x10;
        assert_eq!(parse_self_test(&d).state, "aborted");
    }

    fn input<'a>() -> ScoreInput<'a> {
        ScoreInput { media: "SSD", bus: "SATA", reallocated: 0, pending: 0, uncorrectable: 0, crc: 0, read_errors: 0, write_errors: 0, media_errors: 0, predict_failure: false, wear: 6, temperature: 38, hours: 12_480, unhealthy: false, wear_trend: &[], tb_written: None }
    }

    #[test]
    fn healthy_ssd_scores_high_and_has_life() {
        let s = score(&input());
        assert!(s.total >= 85, "{s:?}");
        assert_eq!(s.label, "Muy bueno");
        assert!(s.life_years.unwrap() > 5.0);
        assert_eq!(s.parts.len(), 5);
    }

    #[test]
    fn pending_sectors_drop_the_score() {
        let mut i = input();
        i.media = "HDD";
        i.bus = "SATA";
        i.pending = 8;
        let s = score(&i);
        assert!(s.total <= 45, "{s:?}");
        assert_eq!(s.label, "Malo");
        assert!(s.life_years.is_none());
    }

    #[test]
    fn predicted_failure_caps_at_20() {
        let mut i = input();
        i.predict_failure = true;
        assert!(score(&i).total <= 20);
    }

    #[test]
    fn life_uses_trend_when_available() {
        let trend = [(1000u32, 10i64), (1090, 19)];
        let mut i = input();
        i.wear = 19;
        i.wear_trend = &trend;
        let s = score(&i);
        // 9 puntos en 90 días = 0,1/día; quedan 81 puntos = 810 días ≈ 2,2 años.
        let y = s.life_years.unwrap();
        assert!((y - 2.2).abs() < 0.1, "{y}");
        assert!(s.life_text.contains("90 días"));
    }

    #[test]
    fn group_formats_thousands() {
        assert_eq!(group(0), "0");
        assert_eq!(group(999), "999");
        assert_eq!(group(1000), "1 000");
        assert_eq!(group(1234567), "1 234 567");
    }
}
