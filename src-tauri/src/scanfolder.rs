//! Escanear a carpeta (Impresoras → Escáneres): el asistente que deja este equipo listo para que
//! una RICOH o una EPSON de la red le mande los escaneos a una carpeta compartida.
//!
//! Qué hace (pide administrador local), todo en una sola entrada del diario con su «Deshacer»:
//! 1. La carpeta `C:\Escaneos` (si existe se reutiliza y no se toca lo que hay dentro).
//! 2. El usuario local `escaner`, con una contraseña que se puede dictar («Mesa-Verde-42»), que nunca
//!    caduca, que él no puede cambiar y oculto de la pantalla de inicio de sesión.
//! 3. El recurso compartido `Escaneos`: cambio solo para `escaner` (y control total para Administradores);
//!    en NTFS, modificar para `escaner` y para el usuario del cliente (el de `target_user`).
//! 4. El cortafuegos: una regla propia (`FIREWALL_RULE`), TCP 445 de entrada, solo desde la IP de la
//!    impresora y solo en el perfil de red actual. Las reglas de «Compartir archivos e impresoras» de
//!    Windows no se tocan: la de SMB cubre a la vez las redes privadas y las públicas. Con la red Pública
//!    no se toca nada: se explica y se ofrece ponerla privada como paso aparte.
//! 5. Que la impresora llegue: ping, misma red o ruta, y el servidor de archivos escuchando en el 445.
//!
//! Luego se vigila la carpeta (cada 2 s, hasta 10 min) y, si no llega nada, se dice por qué mirando lo
//! que Windows registra de verdad (registros de SMBServer, sesiones y la configuración del servidor).
//! La contraseña no se guarda en ningún sitio; si se pierde, «Contraseña nueva» la cambia.

use crate::tweaks::journal::{Backup, Op};
use crate::tweaks::TweakState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Emitter, State};

pub const SHARE: &str = "Escaneos";
pub const USER: &str = "escaner";
const TITLE: &str = "Escanear a carpeta";
/// Marca en la descripción del usuario: lo creó AdminOps.
const USER_DESCRIPTION: &str = "Escanear a carpeta (AdminOps)";
/// Cuánto se vigila la carpeta.
const WATCH_MAX: Duration = Duration::from_secs(600);
const WATCH_EVERY: Duration = Duration::from_secs(2);
/// La regla propia de AdminOps en el cortafuegos: solo el 445 de entrada, solo desde la impresora y solo
/// en la red actual. No se tocan las reglas de «Compartir archivos e impresoras» de Windows porque la de
/// SMB suele cubrir a la vez las redes privadas y las públicas.
pub const FIREWALL_RULE: &str = "AdminOps-Escaneos-SMB";

fn folder_path() -> String {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    format!("{drive}\\Escaneos")
}

// ---------- Contraseña que se puede dictar (puro, con pruebas) ----------

/// Palabras españolas comunes de 4 a 6 letras, sin tildes ni eñes y sin nada feo. Ninguna lleva
/// tres letras seguidas de «escaner»: Windows no admite contraseñas con parte del nombre del usuario.
pub const WORDS: &[&str] = &[
    "mesa", "verde", "azul", "rojo", "negro", "blanco", "gris", "rosa", "plata", "dorado", "luna", "cielo", "lago", "isla", "monte", "valle", "campo", "bosque", "selva", "playa", "arena",
    "piedra", "nube", "lluvia", "viento", "fuego", "tierra", "flor", "rosal", "pino", "cedro", "palma", "hoja", "rama", "brisa", "nieve", "hielo", "trigo", "huerto", "costa", "puerto", "barco",
    "vela", "perla", "coral", "silla", "libro", "pluma", "mapa", "reloj", "llave", "puerta", "techo", "suelo", "pared", "cama", "taza", "plato", "vaso", "jarra", "cesto", "caja", "cinta", "hilo",
    "aguja", "tela", "lana", "nudo", "cofre", "farol", "tambor", "flauta", "piano", "arpa", "coro", "nota", "ritmo", "tono", "gato", "oso", "lobo", "tigre", "cebra", "mono", "cisne", "garza",
    "grillo", "abeja", "rana", "trucha", "foca", "nutria", "conejo", "venado", "oveja", "toro", "potro", "gallo", "paloma", "pavo", "kiwi", "cereza", "miel", "queso", "arroz", "sopa", "salsa",
    "dulce", "pastel", "jugo", "norte", "este", "oeste", "calma", "brillo", "claro", "fuerte", "firme", "veloz", "ligero", "suave", "alegre", "feliz", "bueno", "nuevo", "grande", "libre", "justo",
    "sabio", "noble", "bravo", "tenaz", "sereno", "amable", "genial", "cobre", "hierro", "acero", "bronce", "roble", "nogal", "olivo", "laurel", "menta", "salvia", "romero", "lunes", "martes",
    "jueves", "fresno", "tibio", "sutil", "ancla", "remo", "faro", "ruta", "meta", "plan", "idea", "sello", "regla", "lupa", "ficha", "nodo", "red", "ficus", 
    "lirio", "tinta", "papel", "marco", "punto", "radio", "cable", "clave", "lienzo", "pincel", "paleta", "cuerda", "rueda", "banco", "motor", "tecla", "dalia", "violeta", "cuarzo", "ambar", "jade", "topacio", "zafiro", "cumbre", "colina", "pradera", "arroyo", "fuente", "cabo",
    "duna", "oasis", "aurora", "sol", "alba", "ocaso", "marea", "ola", "bruma", "rocas", "cueva", "ladera", "sendero", "camino", "puente",
];

const DIGIT_WORDS: [&str; 10] = ["cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve"];

fn capital(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// «Mesa-Verde-42» y cómo se dicta («Mesa, guion, Verde, guion, cuatro, dos»). `w1`, `w2` son índices de `WORDS`.
pub fn compose_password(w1: usize, w2: usize, digits: u32) -> (String, String) {
    let a = capital(WORDS[w1 % WORDS.len()]);
    let b = capital(WORDS[w2 % WORDS.len()]);
    let d = digits % 100;
    let password = format!("{a}-{b}-{d:02}");
    let speech = format!("{a}, guion, {b}, guion, {}, {}", DIGIT_WORDS[(d / 10) as usize], DIGIT_WORDS[(d % 10) as usize]);
    (password, speech)
}

/// Un número al azar menor que `n` (sin sesgo), con el generador del sistema.
fn random_below(n: u32) -> u32 {
    if n <= 1 {
        return 0;
    }
    let zone = u32::MAX - (u32::MAX % n);
    loop {
        let mut b = [0u8; 4];
        fill_random(&mut b);
        let v = u32::from_le_bytes(b);
        if v < zone {
            return v % n;
        }
    }
}

#[cfg(windows)]
fn fill_random(buf: &mut [u8]) {
    use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
    // SAFETY: el búfer es nuestro y mide lo que se dice.
    let r = unsafe { BCryptGenRandom(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if r != 0 {
        // No debería pasar: sin azar de calidad no se genera contraseña.
        panic!("el generador de números aleatorios del sistema falló");
    }
}

#[cfg(not(windows))]
fn fill_random(buf: &mut [u8]) {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((t >> ((i % 8) * 8)) & 0xFF) as u8;
    }
}

/// La contraseña más larga que puede salir con este formato (dos palabras más largas, dos guiones y dos cifras).
pub fn max_password_len() -> u32 {
    let longest = WORDS.iter().map(|w| w.len()).max().unwrap_or(0);
    // La segunda palabra no puede ser la misma que la primera: con la más larga repetida saldría una menos.
    let second = WORDS.iter().map(|w| w.len()).filter(|&l| l < longest || WORDS.iter().filter(|w| w.len() == longest).count() > 1).max().unwrap_or(longest);
    (longest + second + 4) as u32
}

/// Una contraseña nueva que cumple la directiva local: se sortea hasta que valga (con un tope). Si el mínimo de la
/// directiva no se puede alcanzar con este formato, un error que lo explica.
pub fn new_valid_password(min: Option<u32>, complexity: Option<bool>) -> Result<(String, String), String> {
    if let Some(m) = min {
        if m > max_password_len() {
            return Err(format!("La directiva de contraseñas de este equipo pide al menos {m} caracteres y la contraseña que genera AdminOps, con dos palabras y dos cifras, no pasa de {}. Baja el mínimo en la directiva local de contraseñas o crea el usuario a mano.", max_password_len()));
        }
    }
    // El largo mínimo se cumple al elegir las palabras (no a ciegas); lo que queda por sortear es la complejidad.
    let mut last = None;
    for _ in 0..400 {
        let (p, s) = new_password_min(min.unwrap_or(0));
        match policy_problem(&p, min, complexity) {
            None => return Ok((p, s)),
            Some(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| "No se pudo generar una contraseña que cumpla la directiva de este equipo.".into()))
}

/// Una contraseña nueva sin mínimo: dos palabras, guion y dos cifras (10 a 99). Solo para las pruebas.
#[cfg(test)]
pub fn new_password() -> (String, String) {
    new_password_min(0)
}

/// Una contraseña nueva de al menos `min` caracteres, si el formato lo permite: se elige la primera palabra entre las que,
/// con la segunda más larga posible, llegan al mínimo, y la segunda entre las que llegan con esa primera.
pub fn new_password_min(min: u32) -> (String, String) {
    let need = (min as usize).saturating_sub(4);
    let n = WORDS.len();
    let longest_other = |i: usize| WORDS.iter().filter(|w| **w != WORDS[i]).map(|w| w.len()).max().unwrap_or(0);
    let mut firsts: Vec<usize> = (0..n).filter(|&i| WORDS[i].len() + longest_other(i) >= need).collect();
    if firsts.is_empty() {
        firsts = (0..n).collect();
    }
    let w1 = firsts[random_below(firsts.len() as u32) as usize];
    let mut seconds: Vec<usize> = (0..n).filter(|&j| WORDS[j] != WORDS[w1] && WORDS[w1].len() + WORDS[j].len() >= need).collect();
    if seconds.is_empty() {
        seconds = (0..n).filter(|&j| WORDS[j] != WORDS[w1]).collect();
    }
    let w2 = seconds[random_below(seconds.len() as u32) as usize];
    compose_password(w1, w2, 10 + random_below(90))
}

// ---------- Directiva local de contraseñas (puro, con pruebas) ----------

/// Texto de un archivo de `secedit` (UTF-16 con marca de orden, o texto normal).
pub fn decode_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&u);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Largo mínimo y complejidad de las contraseñas, según un `secedit /export`.
pub fn parse_policy(text: &str) -> (Option<u32>, Option<bool>) {
    let mut min = None;
    let mut complex = None;
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        match k.trim() {
            "MinimumPasswordLength" => min = v.parse().ok(),
            "PasswordComplexity" => complex = v.parse::<u32>().ok().map(|n| n != 0),
            _ => {}
        }
    }
    (min, complex)
}

/// ¿Cumple esta contraseña la directiva? Texto de lo que falla, si falla.
pub fn policy_problem(password: &str, min: Option<u32>, complexity: Option<bool>) -> Option<String> {
    if let Some(m) = min {
        if (password.chars().count() as u32) < m {
            return Some(format!("La directiva de contraseñas de este equipo pide al menos {m} caracteres y la contraseña que se genera tiene {}. Baja el mínimo en la directiva local de contraseñas o crea el usuario a mano.", password.chars().count()));
        }
    }
    if complexity == Some(true) {
        let kinds = [password.chars().any(|c| c.is_ascii_uppercase()), password.chars().any(|c| c.is_ascii_lowercase()), password.chars().any(|c| c.is_ascii_digit()), password.chars().any(|c| !c.is_ascii_alphanumeric())];
        if kinds.iter().filter(|k| **k).count() < 3 || password.to_lowercase().contains(USER) {
            return Some("La directiva de contraseñas de este equipo pide una complejidad que la contraseña generada no cumple.".into());
        }
    }
    None
}

// ---------- PDF: cuántas páginas (puro, con pruebas) ----------

/// Cuenta los objetos `/Type /Page` que no son `/Pages`, leyendo por trozos. `None` si no sale ninguno
/// (un PDF con los objetos comprimidos o una imagen).
pub fn count_pdf_pages<R: Read>(mut r: R) -> Option<u32> {
    /// Bytes que se guardan de un trozo al siguiente: caben `/Type`, los espacios y `/Page` con lo que sigue.
    const KEEP: usize = 48;
    let mut count = 0u32;
    let mut hay: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = r.read(&mut buf).ok()?;
        total += n as u64;
        if total > 512 * 1024 * 1024 {
            return None;
        }
        let eof = n == 0;
        hay.extend_from_slice(&buf[..n]);
        // Las coincidencias que empiezan en la cola se dejan para el trozo siguiente.
        let limit = if eof { hay.len() } else { hay.len().saturating_sub(KEEP) };
        count += scan_pages(&hay, limit, eof);
        if eof {
            break;
        }
        let from = hay.len().saturating_sub(KEEP);
        hay.drain(..from);
    }
    (count > 0).then_some(count)
}

/// Las páginas cuyo `/Type` empieza antes de `limit`. Al final del archivo, lo que falte cuenta como separador.
fn scan_pages(hay: &[u8], limit: usize, eof: bool) -> u32 {
    let mut count = 0;
    let mut i = 0;
    while i < limit {
        let Some(p) = find(&hay[i..], b"/Type") else { break };
        let start = i + p;
        if start >= limit {
            break;
        }
        let mut j = start + 5;
        while j < hay.len() && hay[j].is_ascii_whitespace() {
            j += 1;
        }
        if hay.len() >= j + 5 && &hay[j..j + 5] == b"/Page" {
            let next = hay.get(j + 5).copied();
            let ends = match next {
                Some(c) => c != b's' && !c.is_ascii_alphanumeric(),
                None => eof,
            };
            if ends {
                count += 1;
            }
        }
        i = start + 5;
    }
    count
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ---------- Eventos de SMB y diagnóstico (puro, con pruebas) ----------

/// Lo que dice un evento de SMBServer sobre una dirección.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmbEvent {
    pub id: u32,
    pub user: Option<String>,
}

fn tag_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = xml.find(&format!("<{tag}"))?;
    let after = &xml[open..];
    let start = after.find('>')? + 1;
    let end = after[start..].find(&format!("</{tag}>"))?;
    Some(after[start..start + end].trim())
}

/// Los pares (nombre, valor) de los `<Data Name="…">valor</Data>` de un evento.
fn data_fields(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<Data") {
        let tail = &rest[i + 5..];
        let Some(gt) = tail.find('>') else { break };
        let head = &tail[..gt];
        let name = head.find("Name=").map(|p| {
            let v = &head[p + 5..];
            let q = v.chars().next().unwrap_or('"');
            v[1..].split(q).next().unwrap_or("").to_string()
        });
        if head.ends_with('/') {
            rest = &tail[gt + 1..];
            continue;
        }
        let Some(end) = tail[gt + 1..].find("</Data>") else { break };
        let value = tail[gt + 1..gt + 1 + end].trim().to_string();
        out.push((name.unwrap_or_default(), value));
        rest = &tail[gt + 1 + end + 7..];
    }
    out
}

/// ¿Es este nombre de cliente de una sesión SMB exactamente esa IP? Vale `10.0.0.41`, `::ffff:10.0.0.41`, `[::ffff:10.0.0.41]`
/// y con puerto o zona; no vale `10.0.0.411` ni `110.0.0.41`.
pub fn client_is(name: &str, ip: &str) -> bool {
    let mut n = name.trim().trim_start_matches('[');
    if let Some(i) = n.find(']') {
        n = &n[..i];
    }
    if let Some(i) = n.find('%') {
        n = &n[..i];
    }
    let lower = n.to_ascii_lowercase();
    let n = lower.strip_prefix("::ffff:").unwrap_or(&lower);
    // Con puerto: «10.0.0.41:445» (solo para IPv4, que no lleva más «:»).
    let n = if n.matches(':').count() == 1 { n.split(':').next().unwrap_or(n) } else { n };
    n == ip
}

fn mentions_ip(text: &str, ip: &str) -> bool {
    let mut from = 0;
    while let Some(p) = text[from..].find(ip) {
        let s = from + p;
        let e = s + ip.len();
        let before_ok = s == 0 || !text.as_bytes()[s - 1].is_ascii_digit() && text.as_bytes()[s - 1] != b'.';
        let after_ok = e >= text.len() || !text.as_bytes()[e].is_ascii_digit() && text.as_bytes()[e] != b'.';
        if before_ok && after_ok {
            return true;
        }
        from = e;
    }
    false
}

/// De los eventos (XML de cada uno), los que hablan de esa dirección.
pub fn events_about(xmls: &[String], ip: &str) -> Vec<SmbEvent> {
    let mut out = Vec::new();
    for x in xmls {
        let fields = data_fields(x);
        if !fields.iter().any(|(_, v)| mentions_ip(v, ip)) {
            continue;
        }
        let Some(id) = tag_text(x, "EventID").and_then(|t| t.parse::<u32>().ok()) else { continue };
        let user = fields.iter().find(|(n, _)| n.eq_ignore_ascii_case("UserName")).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
        out.push(SmbEvent { id, user });
    }
    out
}

/// Por qué no llega el escaneo.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    /// `auth`, `smb1`, `signing`, `session`, `nothing` o `unknown` (los registros no se pudieron leer).
    pub kind: String,
    pub sentence: String,
    pub details: Vec<String>,
    /// La firma SMB es obligatoria en este equipo: se puede ofrecer quitarla.
    pub signing_required: bool,
}

/// Qué ha pasado con la impresora según lo que Windows registró (sin adivinar más de lo que se ve).
pub fn analyze(security: &[SmbEvent], audit: &[SmbEvent], sessions: u32, require_signing: bool, logs_ok: bool) -> Diagnosis {
    let auth: Vec<&SmbEvent> = security.iter().filter(|e| e.id == 551).collect();
    let smb1 = audit.iter().filter(|e| e.id == 3000).count();
    let unreadable = "No se pudieron leer los registros de SMB de Windows (hace falta administrador): puede que la impresora sí haya intentado entrar.".to_string();
    let mk = |kind: &str, sentence: &str, mut details: Vec<String>| {
        if !logs_ok && matches!(kind, "session" | "signing") {
            details.push(unreadable.clone());
        }
        Diagnosis { kind: kind.into(), sentence: sentence.into(), details, signing_required: require_signing }
    };
    if !auth.is_empty() {
        let mut d = vec!["Lo dice el registro de seguridad del servidor de archivos de este equipo.".to_string()];
        if auth.iter().all(|e| e.user.is_none()) {
            d.push("La impresora no mandó ningún usuario: en el panel de la impresora falta el usuario o la contraseña.".into());
        } else {
            d.push(format!("Revisa en la impresora el usuario «{USER}» y la contraseña: si se perdió, «Contraseña nueva» la cambia."));
        }
        return mk("auth", "La impresora llegó pero el usuario o la contraseña no coinciden.", d);
    }
    if smb1 > 0 {
        return mk(
            "smb1",
            "La impresora solo habla SMB1, que Windows tiene apagado por seguridad.",
            vec![
                "Qué se puede hacer: actualizar el firmware de la impresora para que hable SMB 2 o 3, o escanear a correo o a FTP.".into(),
                "AdminOps no activa SMB1: es un protocolo inseguro.".into(),
            ],
        );
    }
    if sessions > 0 {
        return mk("session", "Entró, pero no dejó ningún archivo: revisa la ruta en el panel.", vec!["La impresora abrió una sesión con este equipo; suele ser una ruta con una carpeta distinta o sin permiso.".into()]);
    }
    if require_signing {
        return mk(
            "signing",
            "La impresora no ha llegado a entrar y este equipo exige la firma SMB: las impresoras viejas no la saben usar, es la causa probable.",
            vec!["Si la impresora no es vieja, revisa la ruta y que esté en la misma red.".into(), "Quitar la firma obligatoria baja la seguridad de este equipo en la red: es una acción de riesgo alto y se puede deshacer.".into()],
        );
    }
    if !logs_ok {
        // Sin poder leer lo que Windows registra, no se puede decir que no lo haya intentado.
        return mk("unknown", "No se pudo comprobar si la impresora intentó entrar: leer los registros de SMB de Windows necesita administrador.", Vec::new());
    }
    mk("nothing", "La impresora no ha intentado conectar: revisa la ruta y que esté en la misma red.", Vec::new())
}

// ---------- Lo que se enseña ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub label: String,
    /// `running`, `ok`, `warn` o `error`.
    pub state: String,
    pub detail: Option<String>,
}

fn step(id: &str, label: &str, state: &str, detail: Option<String>) -> Step {
    Step { id: id.into(), label: label.into(), state: state.into(), detail }
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    pub is_admin: bool,
    pub computer: String,
    pub folder: String,
    pub folder_exists: bool,
    pub user_exists: bool,
    pub share_exists: bool,
    pub share_ok: bool,
    /// `Private`, `Public` o `DomainAuthenticated` (la red por la que se llega a la impresora).
    pub network_category: Option<String>,
    pub network_name: Option<String>,
    pub local_ip: Option<String>,
    pub same_subnet: Option<bool>,
    pub require_signing: Option<bool>,
    pub smb1_enabled: Option<bool>,
    pub password_min: Option<u32>,
    /// La contraseña más larga que genera AdminOps.
    pub password_max: u32,
    pub password_complexity: Option<bool>,
    /// Reglas del cortafuegos de compartir por SMB que están apagadas en el perfil actual.
    pub firewall_off: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    /// `\\NOMBRE-DEL-EQUIPO\Escaneos`.
    pub path: String,
    /// `\\10.0.0.23\Escaneos`, con la IP por la que llega la impresora.
    pub ip_path: Option<String>,
    pub user: String,
    pub password: String,
    pub speech: String,
    pub entry_id: u64,
    pub steps: Vec<Step>,
    /// La red es pública: el cortafuegos no se abrió.
    pub public_network: bool,
    /// Se dejó algo en la carpeta que ya existía.
    pub folder_had_files: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchStatus {
    pub running: bool,
    /// `idle`, `waiting`, `arrived` o `timeout`.
    pub state: String,
    pub started_ms: u64,
    pub file: Option<String>,
    pub pages: Option<u32>,
    pub arrived_ms: Option<u64>,
    pub diagnosis: Option<Diagnosis>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn parse_ip(s: &str) -> Result<Ipv4Addr, String> {
    s.trim().parse::<Ipv4Addr>().map_err(|_| "Escribe la IP de la impresora (por ejemplo 10.0.0.41).".to_string())
}

fn run_ps(script: &str, secs: u64) -> Result<String, String> {
    crate::ps::powershell_opts(script, crate::ps::Opts { timeout: Some(Duration::from_secs(secs)), task: None })
}

fn run_json(script: &str, secs: u64) -> Result<Value, String> {
    let out = run_ps(script, secs)?;
    let t = out.trim();
    if t.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(t).map_err(|e| format!("Respuesta no válida de Windows: {e}"))
}

/// Un valor de PowerShell que puede salir como texto suelto, lista o nada.
fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => Vec::new(),
    }
}

fn need_admin() -> Result<(), String> {
    if crate::elevation::is_elevated() {
        Ok(())
    } else {
        Err("Esto necesita administrador: reinicia AdminOps como administrador.".into())
    }
}

// ---------- Scripts (un solo sitio: lo que se ejecuta es lo que se enseña en «Ver el comando») ----------

/// La ruta de red de la impresora al equipo: dirección propia, si está en la misma red y cómo se llama la red.
fn route_script(ip: &str) -> String {
    format!(
        "{}$r = @(Find-NetRoute -RemoteIPAddress $ip -ErrorAction SilentlyContinue)\n\
         $ruta = $r | Where-Object {{ $_.NextHop }} | Select-Object -First 1\n\
         $dir = $r | Where-Object {{ $_.IPAddress }} | Select-Object -First 1\n\
         $idx = if ($ruta) {{ $ruta.InterfaceIndex }} elseif ($dir) {{ $dir.InterfaceIndex }} else {{ $null }}\n\
         $perfil = if ($idx) {{ Get-NetConnectionProfile -InterfaceIndex $idx -ErrorAction SilentlyContinue }} else {{ $null }}\n\
         [pscustomobject]@{{\n\
           localIp = if ($dir) {{ \"$($dir.IPAddress)\" }} else {{ $null }}\n\
           onLink = if ($ruta) {{ \"$($ruta.NextHop)\" -in @('0.0.0.0', '::') }} else {{ $null }}\n\
           ifIndex = $idx\n\
           category = if ($perfil) {{ \"$($perfil.NetworkCategory)\" }} else {{ $null }}\n\
           network = if ($perfil) {{ \"$($perfil.Name)\" }} else {{ $null }}\n\
         }} | ConvertTo-Json -Compress\n",
        crate::ps::text_var("ip", ip)
    )
}


/// Un permiso del recurso compartido, para devolverlo como estaba.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ShareAce {
    pub account: String,
    /// `Full`, `Change` o `Read`.
    pub right: String,
    pub deny: bool,
}

/// Lo que hay antes de tocar nada: el usuario y el recurso compartido.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Preflight {
    pub user_exists: bool,
    pub user_ours: bool,
    /// Cómo estaba el usuario si ya existía y no era de AdminOps (activado y su entrada en `UserList`).
    pub user_prev: Option<UserPrev>,
    pub share_path: Option<String>,
    /// El recurso lo creó AdminOps (lleva su marca en la descripción): al deshacer se quita, no se «restaura».
    pub share_ours: bool,
    pub share_access: Vec<ShareAce>,
    /// El usuario del equipo ya tenía una entrada explícita en la carpeta (o no se pudo saber): al deshacer no se le quita.
    pub client_had_acl: bool,
}

/// Cómo estaba un usuario `escaner` que ya existía y no era de AdminOps, para devolverlo así al deshacer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserPrev {
    pub enabled: bool,
    /// El valor que tenía en `SpecialAccounts\UserList` (oculto de la pantalla de inicio), si tenía entrada.
    pub list_value: Option<u32>,
}

/// Mira si existe el usuario y el recurso (y cómo están), sin cambiar nada. Si la consulta falla, falla (no se supone que no existían).
fn preflight_script(client_sid: Option<&str>) -> String {
    let mut s = String::new();
    s.push_str(&crate::ps::text_var("usuario", USER));
    s.push_str(&crate::ps::text_var("recurso", SHARE));
    s.push_str(&crate::ps::text_var("carpeta", &folder_path()));
    s.push_str(&crate::ps::text_var("clienteSid", client_sid.unwrap_or("")));
    s.push_str("$u = @(Get-LocalUser -ErrorAction Stop | Where-Object { $_.Name -eq $usuario }) | Select-Object -First 1\n");
    s.push_str("$sh = @(Get-SmbShare -ErrorAction Stop | Where-Object { $_.Name -eq $recurso }) | Select-Object -First 1\n");
    s.push_str("$acc = @(); if ($sh) { $acc = @(Get-SmbShareAccess -Name $recurso -ErrorAction Stop | ForEach-Object { [pscustomobject]@{ account = \"$($_.AccountName)\"; right = \"$($_.AccessRight)\"; deny = (\"$($_.AccessControlType)\" -eq 'Deny') } }) }\n");
    s.push_str("# Cómo estaba el usuario: activado y su entrada para ocultarlo de la pantalla de inicio\n");
    s.push_str("$lv = $null\n$p = Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\\SpecialAccounts\\UserList' -ErrorAction SilentlyContinue\n");
    s.push_str("if ($p -and ($p.PSObject.Properties.Name -contains $usuario)) { $lv = [int]$p.$usuario }\n");
    s.push_str("# ¿El usuario del equipo ya tenía una entrada propia (no heredada) en la carpeta? Si no se puede saber, se dice que sí\n");
    s.push_str("$tenia = $false\n");
    s.push_str("if ($clienteSid -and (Test-Path -LiteralPath $carpeta)) {\n  try { $tenia = [bool]((Get-Acl -LiteralPath $carpeta -ErrorAction Stop).Access | Where-Object { -not $_.IsInherited -and $_.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value -eq $clienteSid }) } catch { $tenia = $true }\n}\n");
    s.push_str("[pscustomobject]@{\n  userExists = [bool]$u\n  userDescription = if ($u) { \"$($u.Description)\" } else { '' }\n  userEnabled = if ($u) { [bool]$u.Enabled } else { $null }\n  userListValue = $lv\n  shareExists = [bool]$sh\n  sharePath = if ($sh) { \"$($sh.Path)\" } else { $null }\n  shareDescription = if ($sh) { \"$($sh.Description)\" } else { '' }\n  clientHadAcl = $tenia\n  access = @($acc)\n} | ConvertTo-Json -Depth 4 -Compress\n");
    s
}

/// Lee la respuesta de `preflight_script`. Una respuesta vacía o rara es un error, no «no existe».
pub fn parse_preflight(v: &Value) -> Result<Preflight, String> {
    let user_exists = v["userExists"].as_bool().ok_or("No se pudo comprobar si el usuario «escaner» existe.")?;
    let share_exists = v["shareExists"].as_bool().ok_or("No se pudo comprobar si el recurso «Escaneos» existe.")?;
    let one = |a: &Value| ShareAce { account: a["account"].as_str().unwrap_or("").to_string(), right: a["right"].as_str().unwrap_or("").to_string(), deny: a["deny"].as_bool().unwrap_or(false) };
    let access = match &v["access"] {
        Value::Array(list) => list.iter().map(one).collect(),
        Value::Object(_) => vec![one(&v["access"])],
        _ => Vec::new(),
    };
    let user_ours = v["userDescription"].as_str().is_some_and(|d| d.contains("AdminOps"));
    // Solo importa cómo estaba un usuario que ya existía y no es de AdminOps (el nuestro se borra al deshacer).
    let user_prev = (user_exists && !user_ours).then(|| UserPrev { enabled: v["userEnabled"].as_bool().unwrap_or(true), list_value: v["userListValue"].as_u64().and_then(|n| u32::try_from(n).ok()) });
    Ok(Preflight {
        user_exists,
        user_ours,
        user_prev,
        share_path: if share_exists { Some(v["sharePath"].as_str().unwrap_or("").to_string()) } else { None },
        share_ours: share_exists && v["shareDescription"].as_str().is_some_and(|d| d.contains("AdminOps")),
        share_access: access,
        client_had_acl: v["clientHadAcl"].as_bool().unwrap_or(true),
    })
}

fn same_path(a: &str, b: &str) -> bool {
    a.trim_end_matches('\\').eq_ignore_ascii_case(b.trim_end_matches('\\'))
}

/// Si el recurso ya existe y apunta a otra carpeta, no se toca nada.
pub fn check_share_target(pre: &Preflight, folder: &str) -> Result<(), String> {
    match &pre.share_path {
        Some(p) if !same_path(p, folder) => Err(format!("Ya hay un recurso compartido «{SHARE}» que apunta a otra carpeta ({p}). No se ha cambiado nada: quítalo o cámbiale el nombre antes de preparar «Escanear a carpeta».")),
        _ => Ok(()),
    }
}

fn status_script(ip: Option<&str>) -> String {
    let mut s = String::new();
    s.push_str(&crate::ps::text_var("usuario", USER));
    s.push_str(&crate::ps::text_var("recurso", SHARE));
    s.push_str(&crate::ps::text_var("regla", FIREWALL_RULE));
    s.push_str("$u = Get-LocalUser -Name $usuario -ErrorAction SilentlyContinue\n");
    s.push_str("$sh = Get-SmbShare -Name $recurso -ErrorAction SilentlyContinue\n");
    s.push_str("$cfg = Get-SmbServerConfiguration -ErrorAction SilentlyContinue\n");
    s.push_str("$fw = Get-NetFirewallRule -Name $regla -ErrorAction SilentlyContinue\n");
    s.push_str("[pscustomobject]@{\n  userExists = [bool]$u\n  sharePath = if ($sh) { \"$($sh.Path)\" } else { $null }\n");
    s.push_str("  requireSigning = if ($cfg) { [bool]$cfg.RequireSecuritySignature } else { $null }\n  smb1 = if ($cfg) { [bool]$cfg.EnableSMB1Protocol } else { $null }\n");
    s.push_str("  rulesOff = if ($fw -and \"$($fw.Enabled)\" -eq 'True') { 0 } else { 1 }\n} | ConvertTo-Json -Compress\n");
    let _ = ip;
    s
}

/// Usuario, recurso compartido y permisos de la carpeta. `pw_line` define `$pw` (un SecureString).
pub fn user_share_script(pw_line: &str, client_sid: Option<&str>) -> String {
    let mut s = String::new();
    s.push_str(pw_line);
    s.push_str(&crate::ps::text_var("usuario", USER));
    s.push_str(&crate::ps::text_var("carpeta", &folder_path()));
    s.push_str(&crate::ps::text_var("recurso", SHARE));
    s.push_str(&crate::ps::text_var("marca", USER_DESCRIPTION));
    s.push_str(&crate::ps::text_var("clienteSid", client_sid.unwrap_or("")));
    s.push_str("# Los administradores, con el nombre que tengan en este Windows\n");
    s.push_str("$adm = ([Security.Principal.SecurityIdentifier]'S-1-5-32-544').Translate([Security.Principal.NTAccount]).Value\n");
    s.push_str("# El usuario del escáner: contraseña que no caduca y que él no puede cambiar\n");
    s.push_str("if (Get-LocalUser -Name $usuario -ErrorAction SilentlyContinue) {\n");
    s.push_str("  Set-LocalUser -Name $usuario -Password $pw -PasswordNeverExpires $true -UserMayChangePassword $false\n");
    s.push_str("  Enable-LocalUser -Name $usuario\n");
    s.push_str("} else {\n");
    s.push_str("  New-LocalUser -Name $usuario -Password $pw -FullName 'Escáner' -Description $marca -PasswordNeverExpires -UserMayNotChangePassword -AccountNeverExpires | Out-Null\n");
    s.push_str("}\n");
    s.push_str("# Oculto de la pantalla de inicio de sesión\n");
    s.push_str("$k = 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\\SpecialAccounts\\UserList'\n");
    s.push_str("if (-not (Test-Path $k)) { New-Item -Path $k -Force | Out-Null }\n");
    s.push_str("New-ItemProperty -Path $k -Name $usuario -PropertyType DWord -Value 0 -Force | Out-Null\n");
    s.push_str("# El recurso compartido: cambio solo para el usuario del escáner y control total para los administradores\n");
    s.push_str("$sh = Get-SmbShare -Name $recurso -ErrorAction SilentlyContinue\n");
    s.push_str("if ($sh -and $sh.Path.TrimEnd('\\') -ne $carpeta.TrimEnd('\\')) { throw 'Ya hay un recurso compartido «Escaneos» que apunta a otra carpeta.' }\n");
    s.push_str("if (-not $sh) {\n  New-SmbShare -Name $recurso -Path $carpeta -Description $marca -ChangeAccess $usuario -FullAccess $adm | Out-Null\n} else {\n");
    s.push_str("  Grant-SmbShareAccess -Name $recurso -AccountName $usuario -AccessRight Change -Force | Out-Null\n");
    s.push_str("  Grant-SmbShareAccess -Name $recurso -AccountName $adm -AccessRight Full -Force | Out-Null\n");
    s.push_str("  # Nadie más: se quitan los permisos de otras cuentas\n");
    s.push_str("  Get-SmbShareAccess -Name $recurso | Where-Object { $_.AccountName -ne $adm -and $_.AccountName -notlike \"*\\$usuario\" } | ForEach-Object { Revoke-SmbShareAccess -Name $recurso -AccountName $_.AccountName -Force | Out-Null }\n");
    s.push_str("}\n");
    s.push_str("# Permisos de la carpeta (NTFS): modificar para el escáner y para el usuario del equipo, para abrir y borrar los escaneos\n");
    s.push_str("& icacls.exe $carpeta /grant \"${usuario}:(OI)(CI)M\" | Out-Null\nif ($LASTEXITCODE -ne 0) { throw 'No se pudieron dar permisos al usuario en la carpeta.' }\n");
    s.push_str("if ($clienteSid) { & icacls.exe $carpeta /grant \"*${clienteSid}:(OI)(CI)M\" | Out-Null }\n");
    s
}

/// La regla propia de AdminOps: TCP 445 de entrada, solo desde la IP de la impresora y solo en el perfil
/// de la red actual (nunca en el público). Escribe siempre el nombre de la regla (creada ahora o ya
/// existente de AdminOps) para que «Deshacer» la quite.
pub fn firewall_script(profile_flag: u32, printer_ip: &str) -> String {
    let profile = match profile_flag {
        1 => "Domain",
        2 => "Private",
        _ => "Domain, Private",
    };
    let mut s = String::new();
    s.push_str(&crate::ps::text_var("regla", FIREWALL_RULE));
    s.push_str(&crate::ps::text_var("impresora", printer_ip));
    s.push_str(&crate::ps::text_var("perfil", profile));
    s.push_str("# Una regla propia: solo el puerto 445 (SMB), solo desde la impresora y solo en la red actual\n");
    s.push_str("$r = Get-NetFirewallRule -Name $regla -ErrorAction SilentlyContinue\n");
    s.push_str("if ($r) {\n  Set-NetFirewallRule -Name $regla -Enabled True -Action Allow -Profile $perfil -RemoteAddress $impresora -ErrorAction Stop\n} else {\n");
    s.push_str("  New-NetFirewallRule -Name $regla -DisplayName 'AdminOps · Escanear a carpeta (SMB)' -Direction Inbound -Protocol TCP -LocalPort 445 -RemoteAddress $impresora -Profile $perfil -Action Allow -ErrorAction Stop | Out-Null\n}\n");
    s.push_str("$regla\n");
    s
}

fn firewall_undo_script(names: &[String]) -> String {
    let mut s = String::new();
    for (i, n) in names.iter().enumerate() {
        s.push_str(&crate::ps::text_var(&format!("n{i}"), n));
        s.push_str(&format!("Remove-NetFirewallRule -Name $n{i} -ErrorAction SilentlyContinue\n"));
    }
    s
}

/// Qué hacer con el recurso al deshacer: quitarlo (lo creó AdminOps) o devolver sus permisos (ya existía).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ShareUndo {
    pub existed: bool,
    pub access: Vec<ShareAce>,
}

/// Devuelve los permisos que tenía un recurso que ya existía. Cada paso va en su `try`: uno que falle no corta a los demás
/// (ni a lo que viene después en el deshacer); lo que no se pudo se apunta en `$fallos` y se dice al final.
fn share_restore_lines(access: &[ShareAce]) -> String {
    let mut s = String::from("# El recurso ya existía: no se borra; se le devuelven los permisos que tenía\n");
    s.push_str("if (Get-SmbShare -Name $recurso -ErrorAction SilentlyContinue) {\n");
    s.push_str("  $actuales = @(); try { $actuales = @(Get-SmbShareAccess -Name $recurso -ErrorAction Stop) } catch { $fallos += \"permisos del recurso: $($_.Exception.Message)\" }\n");
    s.push_str("  foreach ($p in $actuales) { try { Revoke-SmbShareAccess -Name $recurso -AccountName $p.AccountName -Force | Out-Null } catch { $fallos += \"permisos del recurso: $($_.Exception.Message)\" } }\n");
    for (i, a) in access.iter().enumerate() {
        if !matches!(a.right.as_str(), "Full" | "Change" | "Read") || a.account.is_empty() {
            continue;
        }
        s.push_str("  ");
        s.push_str(&crate::ps::text_var(&format!("a{i}"), &a.account));
        if a.deny {
            s.push_str(&format!("  try {{ Block-SmbShareAccess -Name $recurso -AccountName $a{i} -Force | Out-Null }} catch {{ $fallos += \"permisos del recurso: $($_.Exception.Message)\" }}\n"));
        } else {
            s.push_str(&format!("  try {{ Grant-SmbShareAccess -Name $recurso -AccountName $a{i} -AccessRight {} -Force | Out-Null }} catch {{ $fallos += \"permisos del recurso: $($_.Exception.Message)\" }}\n", a.right));
        }
    }
    s.push_str("}\n");
    s
}

/// Lo que se hace al deshacer: el recurso, el usuario y lo que se abrió del cortafuegos. Todo se ejecuta siempre: si falla
/// devolver los permisos del recurso, lo demás se hace igual y el error sale al final.
fn undo_script(created_user: bool, firewall: &[String], share: &ShareUndo, extra: &UndoExtra) -> String {
    let mut s = String::new();
    s.push_str(&crate::ps::text_var("usuario", USER));
    s.push_str(&crate::ps::text_var("recurso", SHARE));
    s.push_str(&crate::ps::text_var("carpeta", &folder_path()));
    s.push_str("$fallos = @()\n");
    if share.existed {
        s.push_str(&share_restore_lines(&share.access));
    } else {
        s.push_str("try { Remove-SmbShare -Name $recurso -Force -ErrorAction SilentlyContinue } catch { }\n");
    }
    s.push_str("if (Test-Path -LiteralPath $carpeta) {\n  try { & icacls.exe $carpeta /remove $usuario | Out-Null } catch { }\n");
    // El permiso del usuario del equipo, solo si lo dio AdminOps (con un diario viejo no se sabe: no se toca).
    if let Some(sid) = extra.client_sid.as_deref().filter(|s| !s.is_empty()) {
        s.push_str(&crate::ps::text_var("clienteSid", sid));
        s.push_str("  try { & icacls.exe $carpeta /remove \"*$clienteSid\" | Out-Null } catch { }\n");
    }
    s.push_str("}\n");
    let list = "'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\\SpecialAccounts\\UserList'";
    if created_user {
        s.push_str("try { Remove-LocalUser -Name $usuario -ErrorAction SilentlyContinue } catch { }\n");
        s.push_str(&format!("try {{ Remove-ItemProperty -Path {list} -Name $usuario -ErrorAction SilentlyContinue }} catch {{ }}\n"));
    } else if let Some(prev) = &extra.user_prev {
        // Un usuario que ya existía: se deja como estaba (la contraseña no se puede devolver).
        s.push_str("# El usuario ya existía: se deja como estaba (la contraseña anterior no se puede recuperar)\n");
        if !prev.enabled {
            s.push_str("try { Disable-LocalUser -Name $usuario -ErrorAction Stop } catch { }\n");
        }
        match prev.list_value {
            None => s.push_str(&format!("try {{ Remove-ItemProperty -Path {list} -Name $usuario -ErrorAction SilentlyContinue }} catch {{ }}\n")),
            Some(v) => s.push_str(&format!("try {{ New-ItemProperty -Path {list} -Name $usuario -PropertyType DWord -Value {v} -Force -ErrorAction Stop | Out-Null }} catch {{ }}\n")),
        }
    }
    s.push_str(&firewall_undo_script(firewall));
    s.push_str("if ($fallos.Count -gt 0) { throw ('No se pudo devolver todo como estaba: ' + (($fallos | Select-Object -Unique) -join ' · ')) }\n");
    s
}

/// Lo que el diario sabe además para deshacer: cómo estaba el usuario y el permiso NTFS que dio AdminOps.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UndoExtra {
    pub user_prev: Option<UserPrev>,
    /// El SID del usuario del equipo, solo si AdminOps le dio el permiso (no lo tenía antes).
    pub client_sid: Option<String>,
}

fn profile_flag(category: &str) -> u32 {
    match category {
        "DomainAuthenticated" => 1,
        "Private" => 2,
        "Public" => 4,
        // Sin saber cuál es la red, solo las privadas y de dominio: nunca se abre para una pública.
        _ => 3,
    }
}

// ---------- Estado ----------

fn policy_now() -> (Option<u32>, Option<bool>) {
    let tmp = std::env::temp_dir().join(format!("adminops-secpol-{}.inf", std::process::id()));
    let tmp_s = tmp.to_string_lossy().into_owned();
    let r = crate::ps::exec_opts("secedit.exe", &["/export", "/cfg", &tmp_s, "/areas", "SECURITYPOLICY", "/quiet"], crate::ps::Opts { timeout: Some(Duration::from_secs(30)), task: None });
    let out = if r.is_ok() { std::fs::read(&tmp).ok().map(|b| parse_policy(&decode_text(&b))) } else { None };
    let _ = std::fs::remove_file(&tmp);
    out.unwrap_or((None, None))
}

fn local_ip_towards(ip: Ipv4Addr) -> Option<Ipv4Addr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect((ip, 9)).ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(a) if !a.is_unspecified() => Some(a),
        _ => None,
    }
}

/// Lo que se puede saber sin cambiar nada.
#[tauri::command(async)]
pub fn scanfolder_status(ip: Option<String>) -> Result<ScanStatus, String> {
    let folder = folder_path();
    let mut st = ScanStatus { is_admin: crate::elevation::is_elevated(), computer: crate::paths::host(), folder_exists: Path::new(&folder).is_dir(), folder, ..Default::default() };
    if let Ok(v) = run_json(&status_script(ip.as_deref()), 60) {
        st.user_exists = v["userExists"].as_bool().unwrap_or(false);
        let share_path = v["sharePath"].as_str().map(String::from);
        st.share_exists = share_path.is_some();
        st.share_ok = share_path.as_deref().is_some_and(|p| p.trim_end_matches('\\').eq_ignore_ascii_case(st.folder.trim_end_matches('\\')));
        st.require_signing = v["requireSigning"].as_bool();
        st.smb1_enabled = v["smb1"].as_bool();
        st.firewall_off = v["rulesOff"].as_u64().map(|n| n as u32);
    }
    if let Some(Ok(addr)) = ip.as_deref().filter(|s| !s.trim().is_empty()).map(parse_ip) {
        if let Ok(v) = run_json(&route_script(&addr.to_string()), 60) {
            st.network_category = v["category"].as_str().map(String::from);
            st.network_name = v["network"].as_str().map(String::from);
            st.same_subnet = v["onLink"].as_bool();
            st.local_ip = v["localIp"].as_str().map(String::from);
        }
        if st.local_ip.is_none() {
            st.local_ip = local_ip_towards(addr).map(|a| a.to_string());
        }
    }
    if st.is_admin {
        let (min, complex) = policy_now();
        st.password_min = min;
        st.password_max = max_password_len();
        st.password_complexity = complex;
    }
    Ok(st)
}

// ---------- Asistente ----------

fn emit_step(app: &tauri::AppHandle, s: &Step) {
    let _ = app.emit("scanfolder-step", s);
}

/// Anota el paso (sustituye al anterior con el mismo id) y avisa a la interfaz.
fn push(app: &tauri::AppHandle, steps: &mut Vec<Step>, s: Step) {
    emit_step(app, &s);
    steps.retain(|x| x.id != s.id);
    steps.push(s);
}

/// Lo que se hizo, para poder dejarlo como estaba si algo falla a medias o se deshace.
#[derive(Default, Clone)]
struct Done {
    user: bool,
    created_user: bool,
    share: bool,
    share_existed: bool,
    share_access: Vec<ShareAce>,
    folder_created: bool,
    firewall: Vec<String>,
    /// Cómo estaba el usuario, si ya existía y no era de AdminOps.
    user_prev: Option<UserPrev>,
    /// El usuario del equipo al que AdminOps dio permiso en la carpeta (no lo tenía antes).
    client_sid: Option<String>,
}

fn undo(done: &Done) -> Result<(), String> {
    let mut errors = Vec::new();
    if done.share || done.user || !done.firewall.is_empty() {
        let share = ShareUndo { existed: done.share_existed, access: done.share_access.clone() };
        let extra = UndoExtra { user_prev: done.user_prev.clone(), client_sid: done.client_sid.clone() };
        if let Err(e) = run_ps(&undo_script(done.created_user, &done.firewall, &share, &extra), 60) {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" · "))
    }
}

/// Algo falló a medias: se deja como estaba (el recurso que ya existía, con sus permisos; la carpeta solo si la creó AdminOps).
fn rollback(done: &Done) {
    let _ = undo(done);
    let _ = clean_folder(done.folder_created);
}

fn dir_has_files(p: &Path) -> bool {
    std::fs::read_dir(p).map(|mut d| d.next().is_some()).unwrap_or(false)
}

static UNDO_NOTE: Mutex<Option<String>> = Mutex::new(None);

/// Lo que se quiere decir tras deshacer (por ejemplo, que la carpeta se quedó porque tenía archivos). Lo recoge el diario.
pub fn take_undo_note() -> Option<String> {
    UNDO_NOTE.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Qué pasa con la carpeta al deshacer: solo se borra la que creó AdminOps y está vacía.
#[derive(Debug, PartialEq, Eq)]
pub enum FolderAction {
    /// No la creó AdminOps (o ya no está): se deja.
    Keep,
    /// La creó AdminOps pero tiene archivos: se deja y se dice.
    KeepWithFiles,
    Remove,
}

pub fn folder_action(created_by_adminops: bool, exists: bool, has_files: bool) -> FolderAction {
    if !created_by_adminops || !exists {
        FolderAction::Keep
    } else if has_files {
        FolderAction::KeepWithFiles
    } else {
        FolderAction::Remove
    }
}

/// Aplica `folder_action` a la carpeta de verdad y devuelve lo que hay que decir.
fn clean_folder(created: bool) -> Option<String> {
    let folder = PathBuf::from(folder_path());
    match folder_action(created, folder.is_dir(), dir_has_files(&folder)) {
        FolderAction::Keep => None,
        FolderAction::KeepWithFiles => Some("La carpeta Escaneos se queda porque tiene archivos dentro.".to_string()),
        FolderAction::Remove => std::fs::remove_dir(&folder).err().map(|e| format!("La carpeta Escaneos no se pudo quitar: {e}")),
    }
}

/// Deshace la entrada del diario: quita el recurso (o le devuelve sus permisos si ya existía), el usuario y lo que se
/// abrió del cortafuegos; la carpeta solo se borra si la creó AdminOps y está vacía.
pub fn undo_entry(created_user: bool, firewall: &[String], share_existed: bool, share_access: &[ShareAce], folder_created: bool, user_prev: Option<&UserPrev>, client_sid: Option<&str>) -> Result<(), String> {
    need_admin()?;
    let done = Done { user: true, created_user, share: true, share_existed, share_access: share_access.to_vec(), folder_created, firewall: firewall.to_vec(), user_prev: user_prev.cloned(), client_sid: client_sid.map(String::from) };
    undo(&done)?;
    *UNDO_NOTE.lock().unwrap_or_else(|e| e.into_inner()) = clean_folder(folder_created);
    Ok(())
}

/// «Escanear a carpeta»: deja este equipo listo para recibir escaneos de la impresora.
#[tauri::command(async)]
pub fn scanfolder_setup(app: tauri::AppHandle, ip: String, state: State<'_, TweakState>) -> Result<SetupResult, String> {
    need_admin()?;
    let addr = parse_ip(&ip)?;
    let ip_s = addr.to_string();
    let folder = folder_path();
    let mut steps: Vec<Step> = Vec::new();
    let mut done = Done::default();

    // 0. La contraseña tiene que valer para la directiva local.
    let (min, complex) = policy_now();
    let (password, speech) = new_valid_password(min, complex)?;
    // Y antes de tocar nada se mira lo que ya hay: el usuario y el recurso compartido (y adónde apunta).
    let client_sid = crate::target_user::get().map(|t| t.sid.clone()).filter(|s| !s.is_empty());
    let pre = parse_preflight(&run_json(&preflight_script(client_sid.as_deref()), 60)?)?;
    check_share_target(&pre, &folder)?;
    // Un recurso que dejó AdminOps en otra preparación no es «previo»: al deshacer se quita, y sus permisos no se guardan.
    done.share_existed = pre.share_path.is_some() && !pre.share_ours;
    done.share_access = if done.share_existed { pre.share_access.clone() } else { Vec::new() };
    done.user_prev = pre.user_prev.clone();
    done.client_sid = client_sid.clone().filter(|_| !pre.client_had_acl);

    // 1. La carpeta.
    push(&app, &mut steps, step("folder", "Creando la carpeta Escaneos…", "running", None));
    let existed = Path::new(&folder).is_dir();
    let had_files = existed && dir_has_files(Path::new(&folder));
    done.folder_created = !existed;
    if !existed {
        if let Err(e) = std::fs::create_dir_all(&folder) {
            let msg = format!("No se pudo crear la carpeta {folder}: {e}");
            push(&app, &mut steps, step("folder", "Carpeta Escaneos", "error", Some(msg.clone())));
            return Err(msg);
        }
    }
    push(&app, &mut steps, step("folder", if existed { "Carpeta Escaneos ya existía" } else { "Carpeta Escaneos creada" }, "ok", had_files.then(|| "Lo que había dentro no se ha tocado.".to_string())));

    // 2. Usuario, recurso compartido y permisos.
    push(&app, &mut steps, step("share", "Compartiendo la carpeta solo con el usuario «escaner»…", "running", None));
    let user_existed = pre.user_exists;
    done.created_user = !user_existed || pre.user_ours;
    let script = user_share_script(&crate::ps::secret_var("pw", &password), client_sid.as_deref());
    done.user = true;
    done.share = true;
    if let Err(e) = run_ps(&script, 120) {
        rollback(&done);
        let msg = format!("No se pudo preparar el usuario y el recurso compartido: {e}");
        push(&app, &mut steps, step("share", "Compartida solo con el usuario «escaner»", "error", Some(msg.clone())));
        return Err(msg);
    }
    let detail = format!("{} · permisos de cambio solo para «{USER}»{}", if user_existed { "Usuario con contraseña nueva" } else { "Usuario creado" }, " · oculto de la pantalla de inicio");
    push(&app, &mut steps, step("share", "Compartida solo con el usuario «escaner»", "ok", Some(detail)));

    // 3. Cortafuegos.
    push(&app, &mut steps, step("firewall", "Abriendo el cortafuegos para compartir…", "running", None));
    let route = run_json(&route_script(&ip_s), 60).unwrap_or(Value::Null);
    let category = route["category"].as_str().unwrap_or("").to_string();
    let public_network = category == "Public";
    if public_network {
        push(
            &app,
            &mut steps,
            step("firewall", "Cortafuegos sin tocar: la red es pública", "warn", Some("Windows no deja compartir archivos por una red pública. Ponla como privada (paso aparte, se puede deshacer) y vuelve a comprobar.".into())),
        );
    } else {
        match run_ps(&firewall_script(profile_flag(&category), &ip_s), 90) {
            Ok(out) => {
                done.firewall = out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
                let detail = format!("Solo el puerto 445 (SMB), solo desde {ip_s} y solo en esta red.");
                push(&app, &mut steps, step("firewall", "Cortafuegos de este equipo abierto para compartir", "ok", Some(detail)));
            }
            Err(e) => {
                rollback(&done);
                let msg = format!("No se pudo abrir el cortafuegos: {e}");
                push(&app, &mut steps, step("firewall", "Cortafuegos de este equipo", "error", Some(msg.clone())));
                return Err(msg);
            }
        }
    }

    // El diario: una sola entrada con lo necesario para deshacerlo.
    let backups = vec![Backup::ScanFolder {
        created_user: done.created_user,
        firewall: done.firewall.clone(),
        share_existed: done.share_existed,
        share_access: done.share_access.clone(),
        folder_created: done.folder_created,
        user_prev: done.user_prev.clone(),
        client_sid: done.client_sid.clone(),
    }];
    let entry_id = state.log_change(TITLE, backups, &Ok(()));

    // 4. Que la impresora llegue.
    push(&app, &mut steps, step("reach", "Comprobando que la impresora llega a este equipo…", "running", None));
    let reach = check_reach(addr, &route);
    push(&app, &mut steps, reach);

    let host = crate::paths::host();
    let local = route["localIp"].as_str().map(String::from).or_else(|| local_ip_towards(addr).map(|a| a.to_string()));
    Ok(SetupResult {
        path: format!("\\\\{host}\\{SHARE}"),
        ip_path: local.map(|l| format!("\\\\{l}\\{SHARE}")),
        user: USER.to_string(),
        password,
        speech,
        entry_id,
        steps,
        public_network,
        folder_had_files: had_files,
    })
}

/// Ping, misma red y el servidor de archivos escuchando en el 445.
fn check_reach(addr: Ipv4Addr, route: &Value) -> Step {
    let mut ping = crate::ps::hidden("ping.exe");
    ping.args(["-n", "2", "-w", "1000", &addr.to_string()]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    let ping_ok = ping.output().map(|o| String::from_utf8_lossy(&o.stdout).contains("TTL=")).unwrap_or(false);
    let local = route["localIp"].as_str().and_then(|s| s.parse::<Ipv4Addr>().ok()).or_else(|| local_ip_towards(addr));
    let on_link = route["onLink"].as_bool();
    let smb = local.is_some_and(|l| TcpStream::connect_timeout(&SocketAddr::new(IpAddr::V4(l), 445), Duration::from_millis(800)).is_ok());
    let mut parts = vec![if ping_ok { "Responde al ping".to_string() } else { "No responde al ping (algunas impresoras no lo hacen)".to_string() }];
    parts.push(match on_link {
        Some(true) => "misma red".to_string(),
        Some(false) => "se llega por el router".to_string(),
        None => "sin ruta conocida".to_string(),
    });
    parts.push(if smb { "el servidor de archivos escucha en el 445".to_string() } else { "el servidor de archivos NO escucha en el 445".to_string() });
    let state = if !smb || on_link.is_none() { "error" } else if ping_ok { "ok" } else { "warn" };
    let label = if state == "error" { "La impresora no llega a este equipo" } else { "La impresora llega a este equipo" };
    step("reach", label, state, Some(parts.join(" · ")))
}

/// «Contraseña nueva»: cambia la del usuario (y la enseña una vez).
#[tauri::command(async)]
pub fn scanfolder_new_password(state: State<'_, TweakState>) -> Result<Value, String> {
    need_admin()?;
    let (min, complex) = policy_now();
    let (password, speech) = new_valid_password(min, complex)?;
    let script = format!(
        "{}{}if (-not (Get-LocalUser -Name $usuario -ErrorAction SilentlyContinue)) {{ throw 'El usuario «escaner» no existe: configura «Escanear a carpeta» primero.' }}\nSet-LocalUser -Name $usuario -Password $pw -PasswordNeverExpires $true -UserMayChangePassword $false\n",
        crate::ps::secret_var("pw", &password),
        crate::ps::text_var("usuario", USER)
    );
    let r = run_ps(&script, 60).map(|_| ());
    state.record(Op::Run, "Escanear a carpeta: contraseña nueva", &r);
    r?;
    Ok(serde_json::json!({ "password": password, "speech": speech, "user": USER }))
}

/// Poner la red como privada (la compartición no funciona en una red pública). Con deshacer.
#[tauri::command(async)]
pub fn scanfolder_private_network(ip: String, state: State<'_, TweakState>) -> Result<String, String> {
    need_admin()?;
    let addr = parse_ip(&ip)?;
    let route = run_json(&route_script(&addr.to_string()), 60)?;
    let idx = route["ifIndex"].as_u64().ok_or("No se encontró la red por la que se llega a la impresora.")? as u32;
    let current = route["category"].as_str().unwrap_or("").to_string();
    if current != "Public" {
        return Err("La red no es pública: no hace falta cambiarla.".into());
    }
    let script = format!("$i = {idx}\nSet-NetConnectionProfile -InterfaceIndex $i -NetworkCategory Private\n");
    let r = run_ps(&script, 30).map(|_| ());
    let id = state.log_change("Poner la red como privada", vec![Backup::NetworkCategory { interface_index: idx, previous: current }], &r);
    r?;
    let _ = id;
    Ok("La red ahora es privada.".into())
}

/// Quita la firma SMB obligatoria del servidor de archivos de este equipo (riesgo alto). Con deshacer.
#[tauri::command(async)]
pub fn scanfolder_disable_signing(state: State<'_, TweakState>) -> Result<String, String> {
    need_admin()?;
    let was = run_json("[pscustomobject]@{ v = [bool](Get-SmbServerConfiguration).RequireSecuritySignature } | ConvertTo-Json -Compress", 30)?;
    if was["v"].as_bool() != Some(true) {
        return Err("La firma SMB obligatoria ya está quitada.".into());
    }
    let r = run_ps("Set-SmbServerConfiguration -RequireSecuritySignature $false -Force -Confirm:$false", 30).map(|_| ());
    state.log_change("Quitar la firma SMB obligatoria", vec![Backup::SmbSigning { previous: true }], &r);
    r?;
    Ok("Firma SMB obligatoria quitada. «Deshacer» la vuelve a poner.".into())
}

pub fn restore_signing(previous: bool) -> Result<(), String> {
    need_admin()?;
    run_ps(&format!("Set-SmbServerConfiguration -RequireSecuritySignature ${} -Force -Confirm:$false", if previous { "true" } else { "false" }), 30).map(|_| ())
}

pub fn restore_network_category(interface_index: u32, previous: &str) -> Result<(), String> {
    need_admin()?;
    if !matches!(previous, "Public" | "Private" | "DomainAuthenticated") {
        return Err("Categoría de red desconocida.".into());
    }
    // DomainAuthenticated no se puede poner a mano: solo Public y Private.
    let cat = if previous == "Public" { "Public" } else { "Private" };
    run_ps(&format!("$i = {interface_index}\nSet-NetConnectionProfile -InterfaceIndex $i -NetworkCategory {cat}\n"), 30).map(|_| ())
}

// ---------- Vigilar la carpeta ----------

struct Watch {
    stop: std::sync::Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

static WATCH: Mutex<Option<Watch>> = Mutex::new(None);
static WATCH_STATUS: Mutex<Option<WatchStatus>> = Mutex::new(None);
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

fn status_lock() -> std::sync::MutexGuard<'static, Option<WatchStatus>> {
    WATCH_STATUS.lock().unwrap_or_else(|e| e.into_inner())
}

fn audit_file() -> Option<PathBuf> {
    APP.get().map(|a| crate::paths::machine_data_dir(a).join("scanfolder_audit.json"))
}

#[derive(Serialize, Deserialize)]
struct AuditNote {
    /// Había que dejar `AuditSmb1Access` apagado al terminar.
    restore_off: bool,
}

/// Cuántas esperas necesitan ahora el registro de intentos SMB1 encendido por AdminOps. El candado se toma en cada
/// encendido y en cada restauración: así una espera nueva no se cruza con la que todavía está dejando todo como estaba
/// (ni pierde la nota ni deja el registro encendido).
static AUDIT_USERS: Mutex<u32> = Mutex::new(0);

fn audit_lock() -> std::sync::MutexGuard<'static, u32> {
    AUDIT_USERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Enciende el registro de intentos SMB1 si estaba apagado (y apunta que hay que apagarlo al acabar).
fn audit_smb1_on() -> bool {
    if !crate::elevation::is_elevated() || crate::audit::active() {
        return false;
    }
    let Some(f) = audit_file() else { return false };
    let mut users = audit_lock();
    // Ya lo encendió AdminOps (otra espera que aún no ha acabado, o una restauración que falló): se comparte, y se apaga al acabar la última.
    if f.exists() {
        *users += 1;
        return true;
    }
    let Ok(v) = run_json("[pscustomobject]@{ v = [bool](Get-SmbServerConfiguration).AuditSmb1Access } | ConvertTo-Json -Compress", 30) else { return false };
    if v["v"].as_bool() != Some(false) {
        return false;
    }
    // Primero se apunta, luego se cambia: si AdminOps muere en medio, la próxima vez se deja como estaba.
    let _ = crate::paths::write_json(&f, &AuditNote { restore_off: true });
    if run_ps("Set-SmbServerConfiguration -AuditSmb1Access $true -Force -Confirm:$false", 30).is_err() {
        let _ = std::fs::remove_file(&f);
        return false;
    }
    *users += 1;
    true
}

/// Una espera acaba: si era la última que necesitaba el registro, se deja como estaba.
fn audit_smb1_release() {
    let mut users = audit_lock();
    *users = users.saturating_sub(1);
    if *users == 0 {
        audit_smb1_restore_locked();
    }
}

/// Al abrir o cerrar AdminOps: se deja como estaba, haya las esperas que haya.
fn audit_smb1_restore() {
    let mut users = audit_lock();
    *users = 0;
    audit_smb1_restore_locked();
}

fn audit_smb1_restore_locked() {
    let Some(f) = audit_file() else { return };
    if !f.exists() {
        return;
    }
    if run_ps("Set-SmbServerConfiguration -AuditSmb1Access $false -Force -Confirm:$false", 30).is_ok() {
        let _ = std::fs::remove_file(&f);
    } else {
        log::warn!("Escanear a carpeta: no se pudo dejar como estaba el registro de intentos SMB1");
    }
}

/// Guarda la app y deja como estaba lo que quedó cambiado si AdminOps se cerró a mitad de una espera.
pub fn start(app: tauri::AppHandle) {
    let _ = APP.set(app);
    std::thread::spawn(audit_smb1_restore);
}

/// Al cerrar AdminOps: se deja de vigilar y todo como estaba.
pub fn shutdown() {
    let w = WATCH.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(mut w) = w {
        w.stop.store(true, Ordering::SeqCst);
        if let Some(h) = w.thread.take() {
            let t0 = Instant::now();
            while !h.is_finished() && t0.elapsed() < Duration::from_secs(4) {
                std::thread::sleep(Duration::from_millis(25));
            }
            if h.is_finished() {
                let _ = h.join();
            }
        }
    }
    audit_smb1_restore();
}

fn listing(folder: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(folder) else { return out };
    for e in rd.flatten().take(5000) {
        let p = e.path();
        if let Ok(m) = e.metadata() {
            if m.is_file() {
                out.push((p, m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
            } else if m.is_dir() {
                // Un nivel más: algunas impresoras dejan los escaneos en una subcarpeta por fecha.
                if let Ok(sub) = std::fs::read_dir(&p) {
                    for f in sub.flatten().take(5000) {
                        if let Ok(fm) = f.metadata() {
                            if fm.is_file() {
                                out.push((f.path(), fm.len(), fm.modified().unwrap_or(std::time::UNIX_EPOCH)));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn pages_of(p: &Path) -> Option<u32> {
    let is_pdf = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if !is_pdf {
        return None;
    }
    std::fs::File::open(p).ok().and_then(count_pdf_pages)
}

/// Empieza a esperar un escaneo de prueba.
#[tauri::command(async)]
pub fn scanfolder_watch_start(ip: String) -> Result<WatchStatus, String> {
    let addr = parse_ip(&ip)?;
    scanfolder_watch_stop();
    let folder = PathBuf::from(folder_path());
    if !folder.is_dir() {
        return Err("La carpeta Escaneos no existe: configura «Escanear a carpeta» primero.".into());
    }
    let audit_on = audit_smb1_on();
    let started = now_ms();
    *status_lock() = Some(WatchStatus { running: true, state: "waiting".into(), started_ms: started, ..Default::default() });
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("scanfolder-watch".into())
        .spawn(move || {
            let base: Vec<(PathBuf, u64)> = listing(&folder).into_iter().map(|(p, l, _)| (p, l)).collect();
            let t0 = Instant::now();
            // Archivos nuevos: (ruta, último tamaño visto, desde cuándo no cambia).
            let mut fresh: Vec<(PathBuf, u64, Instant)> = Vec::new();
            let mut result: Option<(String, Option<u32>)> = None;
            while !flag.load(Ordering::SeqCst) && t0.elapsed() < WATCH_MAX && result.is_none() {
                for (p, len, _) in listing(&folder) {
                    if base.iter().any(|(bp, bl)| *bp == p && *bl == len) {
                        continue;
                    }
                    match fresh.iter_mut().find(|f| f.0 == p) {
                        Some(f) => {
                            if f.1 != len {
                                f.1 = len;
                                f.2 = Instant::now();
                            }
                        }
                        None => fresh.push((p, len, Instant::now())),
                    }
                }
                // Quieto 2 s y con algo dentro: ya llegó.
                if let Some(f) = fresh.iter().find(|f| f.1 > 0 && f.2.elapsed() >= Duration::from_secs(2)) {
                    let name = f.0.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    result = Some((name, pages_of(&f.0)));
                    break;
                }
                let mut slept = Duration::ZERO;
                while slept < WATCH_EVERY && !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(100));
                    slept += Duration::from_millis(100);
                }
            }
            let cancelled = flag.load(Ordering::SeqCst);
            let mut st = status_lock();
            if let Some(s) = st.as_mut() {
                s.running = false;
                if let Some((file, pages)) = result {
                    s.state = "arrived".into();
                    s.file = Some(file);
                    s.pages = pages;
                    s.arrived_ms = Some(now_ms());
                } else if !cancelled {
                    s.state = "timeout".into();
                }
            }
            let timed_out = st.as_ref().is_some_and(|s| s.state == "timeout");
            drop(st);
            if timed_out {
                // Se acabó el tiempo: se dice por qué, con lo que Windows registró.
                if let Ok(d) = diagnose_inner(&addr.to_string(), started) {
                    if let Some(s) = status_lock().as_mut() {
                        s.diagnosis = Some(d);
                    }
                }
            }
            if audit_on {
                audit_smb1_release();
            }
        })
        .map_err(|e| format!("No se pudo vigilar la carpeta: {e}"))?;
    *WATCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(Watch { stop, thread: Some(thread) });
    Ok(status_lock().clone().unwrap_or_default())
}

/// Cómo va la espera.
#[tauri::command]
pub fn scanfolder_watch_status() -> WatchStatus {
    status_lock().clone().unwrap_or(WatchStatus { state: "idle".into(), ..Default::default() })
}

/// Deja de esperar (al salir de la pantalla o al empezar de nuevo).
#[tauri::command]
pub fn scanfolder_watch_stop() {
    let w = WATCH.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(mut w) = w {
        w.stop.store(true, Ordering::SeqCst);
        // No se espera: el hilo acaba solo y deja el registro SMB1 como estaba.
        let _detached = w.thread.take();
    }
    if let Some(s) = status_lock().as_mut() {
        if s.running {
            s.running = false;
            s.state = "idle".into();
        }
    }
}

/// Lo que Windows registró del servidor de archivos (cada registro dice si se pudo leer) y las sesiones de la impresora.
const DIAG_SCRIPT: &str = r#"$desde = [DateTimeOffset]::FromUnixTimeMilliseconds($desdeMs).LocalDateTime
function Eventos($registro) {
  # Primero se comprueba que el registro se puede leer: con un filtro, sin permiso también sale «no hay eventos».
  try { Get-WinEvent -LogName $registro -MaxEvents 1 -ErrorAction Stop | Out-Null }
  catch { if ($_.FullyQualifiedErrorId -notlike 'NoMatchingEventsFound*') { return [pscustomobject]@{ ok = $false; items = @() } } }
  try {
    $e = @(Get-WinEvent -FilterHashtable @{ LogName = $registro; StartTime = $desde } -MaxEvents 300 -ErrorAction Stop | ForEach-Object { $_.ToXml() })
    [pscustomobject]@{ ok = $true; items = $e }
  } catch {
    # Sin eventos no es un fallo; sin permiso para leer el registro, sí.
    [pscustomobject]@{ ok = ($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*'); items = @() }
  }
}
$sec = Eventos 'Microsoft-Windows-SMBServer/Security'
$aud = Eventos 'Microsoft-Windows-SMBServer/Audit'
$cfg = Get-SmbServerConfiguration -ErrorAction SilentlyContinue
$clientes = @(Get-SmbSession -ErrorAction SilentlyContinue | ForEach-Object { "$($_.ClientComputerName)" })
[pscustomobject]@{
  sec = @($sec.items)
  secOk = $sec.ok
  audit = @($aud.items)
  auditOk = $aud.ok
  clients = @($clientes)
  requireSigning = if ($cfg) { [bool]$cfg.RequireSecuritySignature } else { $false }
} | ConvertTo-Json -Depth 3 -Compress
"#;

fn diag_script(ip: &str, since_ms: u64) -> String {
    format!("{}$desdeMs = {since_ms}\n{DIAG_SCRIPT}", crate::ps::text_var("ip", ip))
}

fn diagnose_inner(ip: &str, since_ms: u64) -> Result<Diagnosis, String> {
    let addr = parse_ip(ip)?.to_string();
    let v = run_json(&diag_script(&addr, since_ms), 90)?;
    let sec = events_about(&strings(&v["sec"]), &addr);
    let audit = events_about(&strings(&v["audit"]), &addr);
    let sessions = strings(&v["clients"]).iter().filter(|c| client_is(c, &addr)).count() as u32;
    let logs_ok = v["secOk"].as_bool().unwrap_or(false) && v["auditOk"].as_bool().unwrap_or(false);
    Ok(analyze(&sec, &audit, sessions, v["requireSigning"].as_bool().unwrap_or(false), logs_ok))
}

/// «¿Por qué no llega?»: mira lo que Windows registró desde que se empezó a esperar.
#[tauri::command(async)]
pub fn scanfolder_diagnose(ip: String, since_ms: Option<u64>) -> Result<Diagnosis, String> {
    diagnose_inner(&ip, since_ms.unwrap_or_else(|| now_ms().saturating_sub(15 * 60_000)))
}

// ---------- Abrir la carpeta y el acceso del Escritorio ----------

#[tauri::command]
pub fn scanfolder_open_folder() -> Result<(), String> {
    let folder = folder_path();
    if !Path::new(&folder).is_dir() {
        return Err("La carpeta Escaneos no existe.".into());
    }
    std::process::Command::new("explorer.exe").arg(&folder).spawn().map(|_| ()).map_err(|e| format!("No se pudo abrir la carpeta: {e}"))
}

/// El Escritorio del usuario con la sesión abierta.
fn desktop_dir() -> Option<PathBuf> {
    let profile = crate::target_user::user_dirs()?.profile;
    let mut c = vec![profile.join("Desktop"), profile.join("OneDrive").join("Desktop"), profile.join("Escritorio"), profile.join("OneDrive").join("Escritorio")];
    if let Ok(rd) = std::fs::read_dir(&profile) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with("OneDrive") {
                c.push(e.path().join("Desktop"));
                c.push(e.path().join("Escritorio"));
            }
        }
    }
    c.into_iter().find(|p| p.is_dir())
}

fn shortcut_script(lnk: &str, folder: &str) -> String {
    format!(
        "{}{}$w = New-Object -ComObject WScript.Shell\n$a = $w.CreateShortcut($lnk)\n$a.TargetPath = $carpeta\n$a.Description = 'Escaneos de la impresora'\n$a.Save()\n",
        crate::ps::text_var("lnk", lnk),
        crate::ps::text_var("carpeta", folder)
    )
}

/// «Crear acceso en el Escritorio»: en el Escritorio del cliente, no en el de la cuenta que elevó AdminOps.
#[tauri::command(async)]
pub fn scanfolder_desktop_shortcut(state: State<'_, TweakState>) -> Result<String, String> {
    let desk = desktop_dir().ok_or("No se encontró el Escritorio del usuario.")?;
    let lnk = desk.join("Escaneos.lnk");
    let r = run_ps(&shortcut_script(&lnk.to_string_lossy(), &folder_path()), 30).map(|_| ());
    state.record(Op::Run, "Escanear a carpeta: acceso en el Escritorio", &r);
    r?;
    Ok("Acceso «Escaneos» creado en el Escritorio.".into())
}

// ---------- «Ver el comando» ----------

/// El comando equivalente de una acción de este asistente.
pub fn action_script(action: &str, context: &str) -> Result<(&'static str, String), String> {
    match action {
        "setup" => {
            let mut s = String::from("# Escanear a carpeta: carpeta, usuario, recurso compartido y cortafuegos\n\n");
            s.push_str("# La carpeta (si ya existe, no se toca lo que hay dentro)\n");
            s.push_str(&format!("New-Item -ItemType Directory -Force -Path {} | Out-Null\n\n", crate::ps::ps_literal(&folder_path())));
            s.push_str("# Contraseña que se puede dictar: dos palabras, guion y dos cifras (AdminOps genera una nueva cada vez)\n");
            s.push_str("$pw = ConvertTo-SecureString 'Mesa-Verde-42' -AsPlainText -Force\n");
            s.push_str(&user_share_script("", Some("S-1-5-21-1111111111-2222222222-3333333333-1001")));
            s.push_str("\n# Cortafuegos: una regla propia para el 445, solo desde la impresora y en la red actual (aquí, privada)\n");
            s.push_str(&firewall_script(2, "10.0.0.41"));
            Ok(("Preparar «Escanear a carpeta»", s))
        }
        "password" => Ok((
            "Contraseña nueva del usuario escaner",
            format!("# Cambiar la contraseña del usuario del escáner\n\n$pw = ConvertTo-SecureString 'Mesa-Verde-42' -AsPlainText -Force\nSet-LocalUser -Name {} -Password $pw -PasswordNeverExpires $true -UserMayChangePassword $false\n", crate::ps::ps_literal(USER)),
        )),
        "private" => Ok((
            "Poner la red como privada",
            "# La compartición de archivos no funciona en una red pública: se pone como privada\n\n$perfil = Get-NetConnectionProfile | Where-Object { $_.NetworkCategory -eq 'Public' } | Select-Object -First 1\nSet-NetConnectionProfile -InterfaceIndex $perfil.InterfaceIndex -NetworkCategory Private\n".to_string(),
        )),
        "signing" => Ok((
            "Quitar la firma SMB obligatoria",
            "# Quitar la firma SMB obligatoria del servidor de archivos (las impresoras viejas no la saben usar)\n# Ojo: baja la seguridad de este equipo en la red.\n# Para volver a ponerla: Set-SmbServerConfiguration -RequireSecuritySignature $true -Force\n\nSet-SmbServerConfiguration -RequireSecuritySignature $false -Force\n".to_string(),
        )),
        "shortcut" => {
            let lnk = if context.trim().is_empty() { "$env:USERPROFILE\\Desktop\\Escaneos.lnk" } else { context };
            let mut s = String::from("# Crear el acceso «Escaneos» en el Escritorio\n\n");
            s.push_str(&shortcut_script(lnk, &folder_path()));
            Ok(("Crear acceso en el Escritorio", s))
        }
        _ => Err(format!("Acción desconocida: scan.{action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_contrasena_tiene_el_formato_y_se_dicta() {
        let idx = |w: &str| WORDS.iter().position(|x| *x == w).unwrap();
        let (p, s) = compose_password(idx("mesa"), idx("verde"), 42);
        assert_eq!(p, "Mesa-Verde-42");
        assert_eq!(s, "Mesa, guion, Verde, guion, cuatro, dos");
        // Cifras con cero a la izquierda.
        let (p, s) = compose_password(idx("mesa"), idx("verde"), 7);
        assert_eq!(p, "Mesa-Verde-07");
        assert!(s.ends_with("cero, siete"), "{s}");
    }

    #[test]
    fn las_contrasenas_generadas_son_validas() {
        for _ in 0..300 {
            let (p, s) = new_password();
            let parts: Vec<&str> = p.split('-').collect();
            assert_eq!(parts.len(), 3, "{p}");
            for w in &parts[..2] {
                assert!(w.chars().next().unwrap().is_ascii_uppercase(), "{p}");
                assert!(w[1..].chars().all(|c| c.is_ascii_lowercase()), "{p}");
                assert!((3..=8).contains(&w.len()), "{p}");
            }
            assert_ne!(parts[0], parts[1], "{p}");
            assert_eq!(parts[2].len(), 2);
            assert!(parts[2].chars().all(|c| c.is_ascii_digit()));
            assert!(p.is_ascii(), "{p}");
            assert!(s.contains("guion") && !s.contains('-'), "{s}");
            assert_eq!(policy_problem(&p, Some(8), Some(true)), None, "{p}");
        }
    }

    #[test]
    fn la_lista_de_palabras_es_buena() {
        assert!(WORDS.len() >= 200, "hay {}", WORDS.len());
        let mut seen = std::collections::HashSet::new();
        let user = USER.to_lowercase();
        let tri: Vec<&str> = (0..=user.len() - 3).map(|i| &user[i..i + 3]).collect();
        for w in WORDS {
            assert!(w.chars().all(|c| c.is_ascii_lowercase()), "«{w}» no es ASCII en minúsculas");
            assert!((3..=8).contains(&w.len()), "«{w}» tiene {} letras", w.len());
            assert!(seen.insert(*w), "«{w}» está repetida");
            for t in &tri {
                assert!(!w.contains(t), "«{w}» lleva «{t}», parte de «{USER}»");
            }
        }
        // La mayoría, de 4 a 6 letras.
        let ok = WORDS.iter().filter(|w| (4..=6).contains(&w.len())).count();
        assert!(ok * 100 / WORDS.len() >= 90, "{ok} de {}", WORDS.len());
        // Ni una sola palabra malsonante de las conocidas.
        for feo in ["puta", "mierda", "culo", "pene", "cono", "polla", "coño", "zorra", "idiota", "burro", "mamon", "cabra", "yegua", "vaca"] {
            assert!(!WORDS.contains(&feo), "{feo}");
        }
    }

    #[test]
    fn el_azar_no_se_repite_siempre() {
        let a: std::collections::HashSet<String> = (0..40).map(|_| new_password().0).collect();
        assert!(a.len() > 30, "{}", a.len());
    }

    #[test]
    fn la_directiva_se_lee_del_archivo_de_secedit() {
        let utf16: Vec<u8> = [0xFF, 0xFE].iter().copied().chain("[System Access]\r\nMinimumPasswordAge = 0\r\nMinimumPasswordLength = 10\r\nPasswordComplexity = 1\r\n".encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
        let t = decode_text(&utf16);
        assert_eq!(parse_policy(&t), (Some(10), Some(true)));
        assert_eq!(parse_policy("PasswordComplexity = 0\nMinimumPasswordLength = 0"), (Some(0), Some(false)));
        assert_eq!(parse_policy("nada"), (None, None));
    }

    #[test]
    fn la_contrasena_se_comprueba_contra_la_directiva() {
        assert!(policy_problem("Mesa-Verde-42", Some(8), Some(true)).is_none());
        assert!(policy_problem("Mesa-Verde-42", Some(14), None).unwrap().contains("14"));
        assert!(policy_problem("mesaverde", None, Some(true)).is_some());
        assert!(policy_problem("Escaner-Verde-42", None, Some(true)).is_some());
        assert!(policy_problem("Mesa-Verde-42", None, None).is_none());
    }

    fn pdf(parts: &[&str]) -> Vec<u8> {
        format!("%PDF-1.4\n{}\n%%EOF", parts.join("\n")).into_bytes()
    }

    #[test]
    fn las_paginas_de_un_pdf_no_cuentan_el_arbol() {
        let p = pdf(&["1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj", "2 0 obj << /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >> endobj", "3 0 obj << /Type /Page /Parent 2 0 R >> endobj", "4 0 obj << /Type/Page /Parent 2 0 R >> endobj"]);
        assert_eq!(count_pdf_pages(&p[..]), Some(2));
        let tres = pdf(&["<< /Type /Page >>", "<< /Type /Page>>", "<< /Type  /Page\n>>", "<< /Type /Pages >>"]);
        assert_eq!(count_pdf_pages(&tres[..]), Some(3));
        // Un archivo que no es PDF (JPG) o sin objetos visibles: sin páginas.
        assert_eq!(count_pdf_pages(&[0xFF, 0xD8, 0xFF, 0xE0][..]), None);
        assert_eq!(count_pdf_pages(&pdf(&["<< /Type /Pages >>"])[..]), None);
        assert_eq!(count_pdf_pages(&b""[..]), None);
    }

    #[test]
    fn el_conteo_de_paginas_no_se_pierde_entre_trozos() {
        // Un PDF más grande que un trozo de lectura, con «/Type /Page» justo en el corte.
        let mut data = vec![b' '; 64 * 1024 - 8];
        data.extend_from_slice(b"<< /Type /Page >> ");
        data.extend(vec![b' '; 70_000]);
        data.extend_from_slice(b"<< /Type /Page >> << /Type /Pages >>");
        assert_eq!(count_pdf_pages(&data[..]), Some(2));
    }

    fn ev(id: u32, ip: &str, user: &str) -> String {
        format!(
            "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='Microsoft-Windows-SMBServer'/><EventID>{id}</EventID><Level>3</Level></System><EventData><Data Name='UserName'>{user}</Data><Data Name='ClientName'>\\\\{ip}</Data><Data Name='Status'>3221225581</Data></EventData></Event>"
        )
    }

    #[test]
    fn los_eventos_de_smb_se_filtran_por_la_ip_de_la_impresora() {
        let xs = vec![ev(551, "10.0.0.41", "escaner"), ev(551, "10.0.0.4", "otro"), ev(551, "10.0.0.411", "otro"), ev(3000, "10.0.0.41", "")];
        let hits = events_about(&xs, "10.0.0.41");
        assert_eq!(hits, vec![SmbEvent { id: 551, user: Some("escaner".into()) }, SmbEvent { id: 3000, user: None }]);
        assert!(events_about(&xs, "10.0.0.99").is_empty());
        assert!(events_about(&["no es xml".to_string(), String::new()], "10.0.0.41").is_empty());
        // El número de evento con atributos.
        let q = "<Event><System><EventID Qualifiers='0'>551</EventID></System><EventData><Data>10.0.0.41</Data></EventData></Event>";
        assert_eq!(events_about(&[q.to_string()], "10.0.0.41"), vec![SmbEvent { id: 551, user: None }]);
    }

    #[test]
    fn el_diagnostico_sigue_el_orden_de_causas() {
        let auth = [SmbEvent { id: 551, user: Some("escaner".into()) }];
        let smb1 = [SmbEvent { id: 3000, user: None }];
        let d = analyze(&auth, &smb1, 1, true, true);
        assert_eq!(d.kind, "auth");
        assert!(d.sentence.contains("usuario o la contraseña no coinciden"));
        let d = analyze(&[], &smb1, 1, true, true);
        assert_eq!(d.kind, "smb1");
        assert!(d.sentence.contains("SMB1"));
        assert!(d.details.iter().any(|l| l.contains("firmware")) && d.details.iter().any(|l| l.contains("FTP")));
        assert!(d.details.iter().any(|l| l.contains("no activa SMB1")));
        let d = analyze(&[], &[], 2, true, true);
        assert_eq!(d.kind, "session");
        assert!(d.sentence.contains("Entró, pero no dejó ningún archivo"));
        let d = analyze(&[], &[], 0, true, true);
        assert_eq!(d.kind, "signing");
        assert!(d.signing_required);
        let d = analyze(&[], &[], 0, false, true);
        assert_eq!(d.kind, "nothing");
        assert_eq!(d.sentence, "La impresora no ha intentado conectar: revisa la ruta y que esté en la misma red.");
        assert!(!d.signing_required);
        // Si los registros no se pudieron leer, no se dice «no ha intentado conectar».
        let ciego = analyze(&[], &[], 0, false, false);
        assert_eq!(ciego.kind, "unknown");
        assert!(ciego.sentence.contains("administrador"));
        let ciego_firma = analyze(&[], &[], 0, true, false);
        assert_eq!(ciego_firma.kind, "signing");
        assert!(ciego_firma.details.iter().any(|l| l.contains("No se pudieron leer los registros")));
        // Un fallo de autenticación que sí se vio vale aunque el otro registro no se leyera.
        assert_eq!(analyze(&auth, &[], 0, false, false).kind, "auth");
        // Sin usuario en el fallo, se dice que falta en la impresora.
        let sin = analyze(&[SmbEvent { id: 551, user: None }], &[], 0, false, true);
        assert!(sin.details.iter().any(|l| l.contains("no mandó ningún usuario")));
    }

    fn pre(user: bool, ours: bool, path: Option<&str>) -> Preflight {
        Preflight { user_exists: user, user_ours: ours, share_path: path.map(String::from), ..Default::default() }
    }

    #[test]
    fn la_comprobacion_previa_se_lee_y_no_supone_nada() {
        let v: Value = serde_json::from_str(r#"{"userExists":true,"userDescription":"Escanear a carpeta (AdminOps)","shareExists":true,"sharePath":"C:\\Escaneos","access":[{"account":"PC\\escaner","right":"Change","deny":false},{"account":"BUILTIN\\Administradores","right":"Full","deny":false}]}"#).unwrap();
        let p = parse_preflight(&v).unwrap();
        assert!(p.user_exists && p.user_ours);
        assert_eq!(p.share_path.as_deref(), Some("C:\\Escaneos"));
        assert_eq!(p.share_access.len(), 2);
        assert_eq!(p.share_access[1], ShareAce { account: "BUILTIN\\Administradores".into(), right: "Full".into(), deny: false });
        // PowerShell escribe un solo permiso como objeto, no como lista.
        let v: Value = serde_json::from_str(r#"{"userExists":false,"userDescription":"","shareExists":true,"sharePath":"C:\\Escaneos","access":{"account":"Todos","right":"Read","deny":false}}"#).unwrap();
        let p = parse_preflight(&v).unwrap();
        assert!(!p.user_exists && !p.user_ours);
        assert_eq!(p.share_access, vec![ShareAce { account: "Todos".into(), right: "Read".into(), deny: false }]);
        // Sin recurso.
        let v: Value = serde_json::from_str(r#"{"userExists":false,"userDescription":"","shareExists":false,"sharePath":null,"access":[]}"#).unwrap();
        assert_eq!(parse_preflight(&v).unwrap().share_path, None);
        // Una respuesta vacía o rara (la consulta falló) es un error, no «no existen».
        assert!(parse_preflight(&Value::Null).is_err());
        assert!(parse_preflight(&serde_json::json!({ "userExists": true })).is_err());
        assert!(parse_preflight(&serde_json::json!({ "shareExists": false })).is_err());
    }

    #[test]
    fn un_recurso_que_apunta_a_otra_carpeta_para_sin_cambiar_nada() {
        assert!(check_share_target(&pre(false, false, None), "C:\\Escaneos").is_ok());
        assert!(check_share_target(&pre(false, false, Some("C:\\Escaneos")), "C:\\Escaneos").is_ok());
        assert!(check_share_target(&pre(false, false, Some("c:\\escaneos\\")), "C:\\Escaneos").is_ok());
        let e = check_share_target(&pre(true, true, Some("D:\\Otra")), "C:\\Escaneos").unwrap_err();
        assert!(e.contains("D:\\Otra") && e.contains("No se ha cambiado nada"), "{e}");
    }

    #[test]
    fn deshacer_un_recurso_que_ya_existia_le_devuelve_los_permisos_y_no_lo_borra() {
        let share = ShareUndo { existed: true, access: vec![ShareAce { account: "PC\\Ana".into(), right: "Full".into(), deny: false }, ShareAce { account: "Todos".into(), right: "Read".into(), deny: true }, ShareAce { account: "x".into(), right: "Otro".into(), deny: false }] };
        let s = undo_script(false, &[], &share, &UndoExtra::default());
        assert!(!s.contains("Remove-SmbShare"), "{s}");
        assert!(s.contains("Revoke-SmbShareAccess") && s.contains("Grant-SmbShareAccess") && s.contains("-AccessRight Full") && s.contains("Block-SmbShareAccess"), "{s}");
        // Un derecho desconocido no se escribe.
        assert!(!s.contains("Otro"), "{s}");
        // Un recurso que creó AdminOps sí se quita.
        let s = undo_script(true, &[FIREWALL_RULE.to_string()], &ShareUndo::default(), &UndoExtra::default());
        assert!(s.contains("Remove-SmbShare") && !s.contains("Revoke-SmbShareAccess"), "{s}");
        assert!(s.contains("Remove-LocalUser") && s.contains("Remove-NetFirewallRule"), "{s}");
        // Un usuario que ya existía y no es de AdminOps se queda.
        assert!(!undo_script(false, &[], &ShareUndo::default(), &UndoExtra::default()).contains("Remove-LocalUser"));
    }

    /// Deshacer no se para a medias: la restauración del recurso va en `try`, y lo demás se hace siempre.
    #[test]
    fn deshacer_no_se_para_a_medias() {
        let share = ShareUndo { existed: true, access: vec![ShareAce { account: "PC\\Ana".into(), right: "Full".into(), deny: false }, ShareAce { account: "Todos".into(), right: "Read".into(), deny: true }] };
        let extra = UndoExtra { user_prev: None, client_sid: Some("S-1-5-21-1-2-3-1001".into()) };
        let s = undo_script(true, &[FIREWALL_RULE.to_string()], &share, &extra);
        // Solo se toca el recurso si todavía existe.
        assert!(s.contains("if (Get-SmbShare -Name $recurso -ErrorAction SilentlyContinue) {"), "{s}");
        // Cada Revoke, Grant y Block, en su try/catch.
        for cmd in ["Revoke-SmbShareAccess", "Grant-SmbShareAccess", "Block-SmbShareAccess"] {
            let l = s.lines().find(|l| l.contains(cmd)).unwrap_or_else(|| panic!("falta {cmd}\n{s}"));
            assert!(l.contains("try {") && l.contains("catch {"), "{l}");
        }
        // Lo que no depende de la restauración va después y también tolerante: icacls, usuario, regla del cortafuegos.
        let restaurar = s.find("Grant-SmbShareAccess").unwrap();
        for cmd in ["icacls.exe $carpeta /remove $usuario", "icacls.exe $carpeta /remove \"*$clienteSid\"", "Remove-LocalUser", "Remove-ItemProperty", "Remove-NetFirewallRule"] {
            let i = s.find(cmd).unwrap_or_else(|| panic!("falta {cmd}\n{s}"));
            assert!(i > restaurar, "{cmd} va antes de restaurar\n{s}");
        }
        for cmd in ["icacls.exe $carpeta /remove $usuario", "Remove-LocalUser"] {
            let l = s.lines().find(|l| l.contains(cmd)).unwrap();
            assert!(l.contains("try {") && l.contains("catch {"), "{l}");
        }
        assert!(s.contains("-ErrorAction SilentlyContinue") && s.contains("$clienteSid = 'S-1-5-21-1-2-3-1001'"), "{s}");
        // Lo que falló se dice al final, después de hacerlo todo.
        assert!(s.rfind("throw").unwrap() > s.find("Remove-NetFirewallRule").unwrap(), "{s}");
        // Sin permiso dado por AdminOps (o con un diario viejo), no se quita nada del usuario del equipo.
        assert!(!undo_script(false, &[], &share, &UndoExtra::default()).contains("clienteSid"));
    }

    /// Un usuario `escaner` que ya existía se deja como estaba: desactivado y sin entrada en la lista de ocultos.
    #[test]
    fn deshacer_devuelve_el_usuario_previo_como_estaba() {
        let off = UserPrev { enabled: false, list_value: None };
        let s = undo_script(false, &[], &ShareUndo::default(), &UndoExtra { user_prev: Some(off), client_sid: None });
        assert!(s.contains("Disable-LocalUser -Name $usuario"), "{s}");
        assert!(s.contains("Remove-ItemProperty") && !s.contains("Remove-LocalUser"), "{s}");
        // Activado y con su entrada: ni se desactiva ni se quita la entrada (se le pone su valor).
        let on = UserPrev { enabled: true, list_value: Some(1) };
        let s = undo_script(false, &[], &ShareUndo::default(), &UndoExtra { user_prev: Some(on), client_sid: None });
        assert!(!s.contains("Disable-LocalUser") && !s.contains("Remove-ItemProperty") && s.contains("-Value 1 "), "{s}");
        // Diario viejo (sin datos): no se toca nada del usuario.
        let s = undo_script(false, &[], &ShareUndo::default(), &UndoExtra::default());
        assert!(!s.contains("Disable-LocalUser") && !s.contains("UserList"), "{s}");
        // Uno que creó AdminOps se borra y no se «restaura».
        let s = undo_script(true, &[], &ShareUndo::default(), &UndoExtra { user_prev: Some(UserPrev { enabled: false, list_value: None }), client_sid: None });
        assert!(s.contains("Remove-LocalUser") && !s.contains("Disable-LocalUser"), "{s}");
    }

    /// El recurso se crea con la marca de AdminOps y, si ya está así, no se toma por «previo».
    #[test]
    fn el_recurso_de_adminops_lleva_su_marca_y_no_cuenta_como_previo() {
        let s = user_share_script("", None);
        assert!(s.contains("New-SmbShare -Name $recurso -Path $carpeta -Description $marca"), "{s}");
        let v = |d: &str| serde_json::json!({ "userExists": false, "userDescription": "", "shareExists": true, "sharePath": "C:\\Escaneos", "shareDescription": d, "access": [{ "account": "PC\\Ana", "right": "Full", "deny": false }] });
        let ours = parse_preflight(&v(USER_DESCRIPTION)).unwrap();
        assert!(ours.share_ours && ours.share_path.is_some());
        let theirs = parse_preflight(&v("Escaneos de la oficina")).unwrap();
        assert!(!theirs.share_ours);
        // Sin descripción en la respuesta (la comprobación de antes): no es nuestro.
        assert!(!parse_preflight(&serde_json::json!({ "userExists": false, "shareExists": true, "sharePath": "C:\\Escaneos" })).unwrap().share_ours);
    }

    /// De la comprobación previa salen cómo estaba el usuario y si el usuario del equipo ya tenía permiso propio.
    #[test]
    fn la_comprobacion_previa_guarda_como_estaba_el_usuario_y_el_permiso() {
        let v: Value = serde_json::from_str(r#"{"userExists":true,"userDescription":"Otro","userEnabled":false,"userListValue":null,"shareExists":false,"sharePath":null,"clientHadAcl":false,"access":[]}"#).unwrap();
        let p = parse_preflight(&v).unwrap();
        assert_eq!(p.user_prev, Some(UserPrev { enabled: false, list_value: None }));
        assert!(!p.client_had_acl);
        let v: Value = serde_json::from_str(r#"{"userExists":true,"userDescription":"","userEnabled":true,"userListValue":0,"shareExists":false,"sharePath":null,"clientHadAcl":true,"access":[]}"#).unwrap();
        let p = parse_preflight(&v).unwrap();
        assert_eq!(p.user_prev, Some(UserPrev { enabled: true, list_value: Some(0) }));
        assert!(p.client_had_acl);
        // Uno de AdminOps o que no existe: nada que devolver. Sin dato del permiso, se supone que ya lo tenía (no se quita).
        let v: Value = serde_json::from_str(r#"{"userExists":true,"userDescription":"Escanear a carpeta (AdminOps)","shareExists":false,"sharePath":null,"access":[]}"#).unwrap();
        let p = parse_preflight(&v).unwrap();
        assert_eq!(p.user_prev, None);
        assert!(p.client_had_acl);
        // El script mira las entradas propias (no heredadas) de la carpeta y la lista de usuarios ocultos.
        let s = preflight_script(Some("S-1-5-21-1-2-3-1001"));
        assert!(s.contains("$clienteSid = 'S-1-5-21-1-2-3-1001'") && s.contains("-not $_.IsInherited") && s.contains("UserList") && s.contains("$u.Enabled"), "{s}");
    }

    #[test]
    fn el_formato_viejo_del_diario_se_sigue_leyendo() {
        use crate::tweaks::journal::Backup;
        let viejo: Backup = serde_json::from_str(r#"{"kind":"scanFolder","createdUser":true,"firewall":["AdminOps-Escaneos-SMB"]}"#).unwrap();
        match viejo {
            Backup::ScanFolder { created_user, firewall, share_existed, share_access, folder_created, user_prev, client_sid } => {
                assert!(created_user && !share_existed && !folder_created && share_access.is_empty());
                // Los campos nuevos no estaban: al deshacer no se toca el usuario ni el permiso del equipo.
                assert!(user_prev.is_none() && client_sid.is_none());
                assert_eq!(firewall, vec!["AdminOps-Escaneos-SMB".to_string()]);
            }
            _ => panic!("no es ScanFolder"),
        }
        let nuevo = Backup::ScanFolder {
            created_user: false,
            firewall: vec![],
            share_existed: true,
            share_access: vec![ShareAce { account: "a".into(), right: "Full".into(), deny: false }],
            folder_created: true,
            user_prev: Some(UserPrev { enabled: false, list_value: Some(0) }),
            client_sid: Some("S-1-5-21-1-2-3-1001".into()),
        };
        let json = serde_json::to_string(&nuevo).unwrap();
        assert!(json.contains("shareExisted") && json.contains("folderCreated") && json.contains("userPrev") && json.contains("listValue") && json.contains("clientSid"), "{json}");
        match serde_json::from_str::<Backup>(&json).unwrap() {
            Backup::ScanFolder { share_existed: true, folder_created: true, user_prev: Some(u), client_sid: Some(c), .. } => {
                assert_eq!(u, UserPrev { enabled: false, list_value: Some(0) });
                assert_eq!(c, "S-1-5-21-1-2-3-1001");
            }
            otro => panic!("{otro:?}"),
        }
    }

    #[test]
    fn la_carpeta_solo_se_borra_si_la_creo_adminops_y_esta_vacia() {
        assert_eq!(folder_action(true, true, false), FolderAction::Remove);
        assert_eq!(folder_action(true, true, true), FolderAction::KeepWithFiles);
        // Si ya existía, o un diario viejo no lo sabe, no se toca.
        assert_eq!(folder_action(false, true, false), FolderAction::Keep);
        assert_eq!(folder_action(false, true, true), FolderAction::Keep);
        assert_eq!(folder_action(true, false, false), FolderAction::Keep);
    }

    #[test]
    fn la_contrasena_se_repite_hasta_cumplir_la_directiva() {
        // Un mínimo que solo cumplen las más largas: siempre sale una que vale.
        let min = max_password_len() - 3;
        for _ in 0..20 {
            let (p, _) = new_valid_password(Some(min), Some(true)).unwrap();
            assert!(p.chars().count() as u32 >= min, "{p}");
            assert_eq!(policy_problem(&p, Some(min), Some(true)), None);
        }
        assert!(new_valid_password(Some(8), Some(true)).is_ok());
        assert!(new_valid_password(None, None).is_ok());
        // Un mínimo imposible con este formato: un error claro.
        let e = new_valid_password(Some(max_password_len() + 1), None).unwrap_err();
        assert!(e.contains("pide al menos") && e.contains(&max_password_len().to_string()), "{e}");
        let largo = WORDS.iter().map(|w| w.len()).max().unwrap();
        assert!(max_password_len() as usize <= 2 * largo + 4);
        assert!(max_password_len() >= 13);
    }

    /// Con un mínimo alto (el máximo real y uno menos) siempre sale una que vale, a la primera y sin sortear a ciegas.
    #[test]
    fn la_contrasena_con_minimo_alto_siempre_sale() {
        let max = max_password_len();
        assert_eq!(max, 18, "si cambia la lista de palabras, también `passwordMax` de demo.ts");
        for min in [max - 1, max] {
            for _ in 0..300 {
                let (p, s) = new_password_min(min);
                assert!(p.chars().count() as u32 >= min, "{min}: {p}");
                let parts: Vec<&str> = p.split('-').collect();
                assert_eq!(parts.len(), 3, "{p}");
                assert_ne!(parts[0].to_lowercase(), parts[1].to_lowercase(), "{p}");
                assert!(s.starts_with(parts[0]), "{s}");
                let (v, _) = new_valid_password(Some(min), Some(true)).unwrap();
                assert!(v.chars().count() as u32 >= min, "{v}");
                assert_eq!(policy_problem(&v, Some(min), Some(true)), None);
            }
        }
        // Un mínimo bajo no limita las palabras.
        let largos: std::collections::HashSet<usize> = (0..300).map(|_| new_password_min(0).0.len()).collect();
        assert!(largos.len() > 3, "{largos:?}");
    }

    #[test]
    fn la_regla_del_cortafuegos_escribe_siempre_su_nombre() {
        let s = firewall_script(2, "10.0.0.41");
        // Fuera del if/else: sale tanto si la crea como si ya existía.
        let tras = s.rsplit("}\n").next().unwrap();
        assert_eq!(tras.trim(), "$regla", "{s}");
        let (_, doc) = action_script("setup", "").unwrap();
        assert!(doc.contains(FIREWALL_RULE));
    }

    #[test]
    fn una_sesion_se_compara_con_la_ip_exacta() {
        assert!(client_is("10.0.0.41", "10.0.0.41"));
        assert!(client_is(" 10.0.0.41 ", "10.0.0.41"));
        assert!(client_is("::ffff:10.0.0.41", "10.0.0.41"));
        assert!(client_is("::FFFF:10.0.0.41", "10.0.0.41"));
        assert!(client_is("[::ffff:10.0.0.41]", "10.0.0.41"));
        assert!(client_is("10.0.0.41:445", "10.0.0.41"));
        assert!(client_is("fe80::1%12", "fe80::1"));
        assert!(!client_is("10.0.0.411", "10.0.0.41"));
        assert!(!client_is("110.0.0.41", "10.0.0.41"));
        assert!(!client_is("10.0.0.4", "10.0.0.41"));
        assert!(!client_is("::ffff:10.0.0.4", "10.0.0.41"));
        assert!(!client_is("PC-RECEP", "10.0.0.41"));
        assert!(!client_is("", "10.0.0.41"));
    }

    #[test]
    fn la_ip_se_valida() {
        assert!(parse_ip("10.0.0.41").is_ok());
        assert!(parse_ip(" 10.0.0.41 ").is_ok());
        assert!(parse_ip("10.0.0.41; calc").is_err());
        assert!(parse_ip("impresora").is_err());
        assert!(parse_ip("").is_err());
    }

    #[test]
    fn todos_los_scripts_son_powershell_valido() {
        let mut scripts: Vec<(String, String)> = vec![
            ("estado".into(), status_script(None)),
            ("ruta".into(), route_script("10.0.0.41")),
            ("usuario y recurso".into(), user_share_script("$pw = ConvertTo-SecureString 'x' -AsPlainText -Force\n", Some("S-1-5-21-1-2-3-1001"))),
            ("usuario sin sid".into(), user_share_script("$pw = $null\n", None)),
            ("cortafuegos".into(), firewall_script(2, "10.0.0.41")),
            ("diagnóstico".into(), diag_script("10.0.0.41", 1_700_000_000_000)),
            ("deshacer".into(), undo_script(true, &["{AAAA-1}".to_string(), "x'y".to_string()], &ShareUndo::default(), &UndoExtra::default())),
            ("deshacer con recurso previo".into(), undo_script(false, &[], &ShareUndo { existed: true, access: vec![ShareAce { account: "DESKTOP\\Ana".into(), right: "Full".into(), deny: false }, ShareAce { account: "Todos".into(), right: "Read".into(), deny: true }, ShareAce { account: "mala".into(), right: "Otro".into(), deny: false }] }, &UndoExtra::default())),
            ("comprobación previa".into(), preflight_script(Some("S-1-5-21-1-2-3-1001"))),
            ("comprobación previa sin sid".into(), preflight_script(None)),
            ("deshacer con usuario previo".into(), undo_script(false, &[], &ShareUndo { existed: true, access: vec![ShareAce { account: "Todos".into(), right: "Read".into(), deny: false }] }, &UndoExtra { user_prev: Some(UserPrev { enabled: false, list_value: None }), client_sid: Some("S-1-5-21-1-2-3-1001".into()) })),
            ("deshacer con usuario previo y entrada".into(), undo_script(false, &[], &ShareUndo::default(), &UndoExtra { user_prev: Some(UserPrev { enabled: true, list_value: Some(0) }), client_sid: None })),
            ("acceso".into(), shortcut_script(r"C:\Users\Ana\Desktop\Escaneos.lnk", r"C:\Escaneos")),
        ];
        for a in ["setup", "password", "private", "signing", "shortcut"] {
            let (t, s) = action_script(a, "").unwrap();
            assert!(!t.is_empty());
            scripts.push((format!("comando {a}"), s));
        }
        for (name, s) in scripts {
            let e = crate::ps::parse_errors(&s);
            assert!(e.trim().is_empty(), "{name}: {e}\n{s}");
        }
        assert!(action_script("borrar", "").is_err());
    }

    #[test]
    fn el_grupo_de_cortafuegos_es_el_de_compartir_archivos_e_impresoras() {
        let s = firewall_script(2, "10.0.0.41");
        // Regla propia: no se encienden las de Windows (la de SMB cubre también las redes públicas).
        assert!(!s.contains("-Group"));
        assert!(s.contains("-LocalPort 445") && s.contains("Inbound") && s.contains("TCP"));
        assert!(s.contains("-RemoteAddress $impresora") && s.contains("'10.0.0.41'"));
        assert!(s.contains("'Private'") && !s.contains("Public"));
        assert!(!firewall_script(3, "10.0.0.41").contains("Public"));
        assert!(firewall_undo_script(&[FIREWALL_RULE.to_string()]).contains("Remove-NetFirewallRule"));
        assert_eq!(profile_flag("Private"), 2);
        assert_eq!(profile_flag("DomainAuthenticated"), 1);
        assert_eq!(profile_flag("Public"), 4);
        assert_eq!(profile_flag(""), 3);
    }

    #[test]
    fn la_contrasena_nunca_va_en_claro_en_el_script_del_asistente() {
        // El script real usa `secret_var` (DPAPI): la contraseña no aparece escrita.
        let line = crate::ps::secret_var("pw", "Mesa-Verde-42");
        assert!(!line.contains("Mesa-Verde-42"), "{line}");
        assert!(line.starts_with("$pw = ConvertTo-SecureString"));
    }

    /// Equipo real, solo lectura: el estado, la directiva, la configuración y los registros de SMB.
    /// `cargo test --lib smb_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn smb_real() {
        let st = scanfolder_status(Some("192.168.1.1".into())).expect("estado");
        println!("estado: {st:#?}");
        let d = diagnose_inner("192.168.1.1", now_ms().saturating_sub(24 * 3_600_000));
        println!("diagnóstico (solo lectura): {d:?}");
        let raw = run_json(
            "$cfg = Get-SmbServerConfiguration -ErrorAction SilentlyContinue\n[pscustomobject]@{ requireSigning = if ($cfg) { $cfg.RequireSecuritySignature } else { 'sin acceso' }; smb1 = if ($cfg) { $cfg.EnableSMB1Protocol } else { 'sin acceso' }; auditSmb1 = if ($cfg) { $cfg.AuditSmb1Access } else { 'sin acceso' } } | ConvertTo-Json -Compress",
            60,
        );
        println!("configuración del servidor SMB: {raw:?}");
        let logs = run_ps("foreach ($l in 'Microsoft-Windows-SMBServer/Security','Microsoft-Windows-SMBServer/Audit') { try { $e = @(Get-WinEvent -LogName $l -MaxEvents 5 -ErrorAction Stop); \"$l : $($e.Count) eventos leídos\" } catch { \"$l : $($_.Exception.Message)\" } }", 60);
        println!("registros SMB: {logs:?}");
    }
}
