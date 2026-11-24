//! Lo que winget cuenta mientras instala: el progreso de la descarga línea a línea y
//! el código de salida, traducido a una frase en español.
//!
//! Todo es lógica pura (sin lanzar nada) para poder probarla con salidas reales.
//! Los códigos son los `APPINSTALLER_CLI_ERROR_*` de winget (0x8A15xxxx) y los códigos
//! que devuelven los propios instaladores (MSI: 1602, 1618, 1603…).

/// Qué dice una línea de la salida de winget.
#[derive(Debug, PartialEq, Clone)]
pub enum Line {
    /// Barra de progreso de la descarga: `  ████▒▒  34%` o `  ████▒▒  1.00 MB / 4.00 MB`.
    Progress { percent: u8, done: Option<u64>, total: Option<u64> },
    Phase(Phase),
    Other,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Phase {
    Downloading,
    Installing,
    Success,
}

/// Los bloques con los que winget dibuja la barra (U+2580–U+259F: «█», «▍», «▒»…) y los cuadrados.
fn is_bar(c: char) -> bool {
    ('\u{2580}'..='\u{259F}').contains(&c) || c == '■' || c == '□'
}

fn unit(u: &str) -> Option<u64> {
    match u.to_ascii_uppercase().as_str() {
        "B" => Some(1),
        "KB" | "KIB" => Some(1024),
        "MB" | "MIB" => Some(1024 * 1024),
        "GB" | "GIB" => Some(1024 * 1024 * 1024),
        _ => None,
    }
}

/// `52.0 MB` o `52,0 MB` → bytes.
fn size(text: &str) -> Option<u64> {
    let t = text.trim();
    let cut = t.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, u) = t.split_at(cut);
    let n: f64 = num.trim().replace(',', ".").parse().ok()?;
    let mult = unit(u.trim())?;
    (n >= 0.0).then_some((n * mult as f64) as u64)
}

fn progress(line: &str) -> Option<Line> {
    let rest: String = line.chars().filter(|c| !is_bar(*c)).collect();
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    if let Some(p) = rest.strip_suffix('%') {
        let n: u32 = p.trim().parse().ok()?;
        return (n <= 100).then_some(Line::Progress { percent: n as u8, done: None, total: None });
    }
    let (a, b) = rest.split_once('/')?;
    let (done, total) = (size(a)?, size(b)?);
    if total == 0 {
        return None;
    }
    Some(Line::Progress { percent: ((done.min(total) * 100) / total) as u8, done: Some(done), total: Some(total) })
}

/// Interpreta una línea (ya sin saltos). Entiende winget en inglés y en español.
pub fn parse_line(line: &str) -> Line {
    if let Some(p) = progress(line) {
        return p;
    }
    let l = line.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| l.contains(w));
    let t = l.trim_start();
    if t.starts_with("downloading ") || t.starts_with("descargando ") {
        Line::Phase(Phase::Downloading)
    } else if has(&["successfully installed", "instalado correctamente", "se instaló correctamente"]) {
        Line::Phase(Phase::Success)
    } else if has(&[
        "verified installer hash",
        "hash del instalador",
        "starting package install",
        "iniciando la instalación",
        "iniciando instalación",
        "installing",
        "instalando",
    ]) {
        Line::Phase(Phase::Installing)
    } else {
        Line::Other
    }
}

/// Qué pasó con un programa.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Verdict {
    /// Instalado.
    Done,
    /// No hacía falta (ya estaba, hay una versión más nueva) o se canceló.
    Skipped,
    Failed,
}

/// El motivo de un resultado de winget.
#[derive(Debug, PartialEq, Clone)]
pub struct Outcome {
    pub verdict: Verdict,
    /// Frase en español para la fila.
    pub message: String,
    /// `close-app`: hay que cerrar el programa (la interfaz ofrece «Cerrar y seguir»).
    /// `busy`, `network`, `space`, `policy`, `hash`, `missing`, `cancelled`, `other`.
    pub kind: &'static str,
    /// El instalador pide (o hará falta) reiniciar.
    pub reboot: bool,
    /// Código en texto (`0x8A150101`), si hay uno que enseñar.
    pub code: Option<String>,
}

/// Frase cuando winget no existe en el equipo.
pub const WINGET_MISSING: &str = "winget no está disponible en este equipo (se instala con «Instalador de aplicación» desde Microsoft Store).";

/// ¿La salida dice que el comando no existe? (cmd devuelve 9009 y «no se reconoce como un comando…»).
pub fn winget_not_found(code: Option<i64>, text: &str) -> bool {
    let l = text.to_lowercase();
    code == Some(9009) || l.contains("no se reconoce") || l.contains("is not recognized as an internal or external command")
}

fn hex(code: i64) -> String {
    let c = code as i32 as u32;
    if c >= 0x1_0000 {
        format!("0x{c:08X}")
    } else {
        format!("0x{c:X}")
    }
}

/// «Installer failed with exit code: 1603» / «Código de salida del instalador: 1603»: el código del instalador.
fn installer_code(text: &str) -> Option<i64> {
    for l in text.lines() {
        let low = l.to_lowercase();
        if (low.contains("exit code") || low.contains("código de salida")) && !low.contains("0x8a15") {
            let digits: String = l.chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<Vec<_>>().into_iter().rev().collect();
            if let Ok(n) = digits.trim_matches('.').parse::<i64>() {
                return Some(n);
            }
            let after = l.rsplit(':').next().unwrap_or("").trim().trim_end_matches('.');
            if let Ok(n) = after.parse::<i64>() {
                return Some(n);
            }
        }
    }
    None
}

fn out(verdict: Verdict, kind: &'static str, message: &str, code: i64) -> Outcome {
    Outcome { verdict, message: message.into(), kind, reboot: false, code: (code != 0).then(|| hex(code)) }
}

/// Traduce el código de salida de winget. `text` son las últimas líneas útiles de su
/// salida (para el código del propio instalador y para lo desconocido).
pub fn interpret(code: i64, text: &str) -> Outcome {
    use Verdict::*;
    let c = code as i32 as u32;
    if winget_not_found(Some(code).filter(|c| *c == 9009), text) {
        return Outcome { verdict: Failed, message: WINGET_MISSING.into(), kind: "missing", reboot: false, code: None };
    }
    let reboot = |message: &str| Outcome { verdict: Done, message: message.into(), kind: "other", reboot: true, code: None };
    match c {
        0 => Outcome { verdict: Done, message: "Instalado".into(), kind: "other", reboot: false, code: None },
        // Ya estaba / hay una versión más nueva.
        0x8A15_0061 | 0x8A15_010D => out(Skipped, "other", "Ya estaba instalado", code),
        0x8A15_002B => out(Skipped, "other", "Ya estaba instalado y al día", code),
        0x8A15_010E => out(Skipped, "other", "Ya hay una versión más nueva instalada", code),
        // Reinicio.
        3010 | 1641 | 0x8A15_0109 | 0x8A15_010B => reboot("Instalado · pide reiniciar el equipo"),
        0x8A15_010A => reboot("Se terminará de instalar al reiniciar el equipo"),
        // Cerrar el programa.
        0x8A15_0101 | 0x8A15_0111 => out(Failed, "close-app", "Hay que cerrar el programa para poder instalarlo", code),
        0x8A15_0103 => out(Failed, "close-app", "Un archivo del programa está en uso: hay que cerrarlo", code),
        // Otro instalador.
        0x8A15_0102 | 1618 => out(Failed, "busy", "Hay otra instalación en marcha: espera a que termine y reintenta", code),
        // Sin espacio, red, memoria.
        0x8A15_0105 | 112 => out(Failed, "space", "No hay espacio suficiente en el disco", code),
        0x8A15_0106 => out(Failed, "space", "No hay memoria suficiente para instalarlo", code),
        0x8A15_0107 => out(Failed, "network", "Sin conexión a Internet", code),
        0x8A15_0008 => out(Failed, "network", "No se pudo descargar el instalador: revisa la conexión", code),
        // Paquete.
        0x8A15_0014 | 0x8A15_0017 => out(Failed, "missing", "winget no encuentra el paquete", code),
        0x8A15_0016 => out(Failed, "missing", "winget encontró varios paquetes con ese nombre", code),
        0x8A15_0010 | 0x8A15_0113 => out(Failed, "missing", "El paquete no tiene instalador para este equipo", code),
        0x8A15_0011 => out(Failed, "hash", "La descarga no coincide con su huella (hash): no se instala por seguridad", code),
        0x8A15_0104 | 0x8A15_0110 => out(Failed, "other", "Falta un componente del que depende el programa", code),
        0x8A15_0114 => out(Failed, "other", "El instalador no permite actualizar encima de la versión actual", code),
        0x8A15_0108 => out(Failed, "other", "El instalador pide contactar con el soporte del fabricante", code),
        0x8A15_0112 => out(Failed, "other", "El instalador recibió un parámetro no válido", code),
        0x8A15_0006 => out(Failed, "other", "No se pudo ejecutar el instalador", code),
        // Directivas.
        0x8A15_010F | 1625 => out(Failed, "policy", "Una directiva de este equipo bloquea la instalación", code),
        // Cancelado.
        0x8A15_010C | 0x8A15_0005 | 1602 | 1223 => out(Skipped, "cancelled", "Se canceló la instalación", code),
        1603 => out(Failed, "other", "El instalador terminó con un error grave (1603)", code),
        1638 => out(Skipped, "other", "Ya hay otra versión instalada", code),
        _ => {
            // winget envuelve el código del propio instalador: se traduce ese.
            if let Some(inner) = installer_code(text) {
                if inner != code && inner != 0 {
                    let o = interpret(inner, "");
                    if o.code.is_some() && !o.message.starts_with("winget terminó") {
                        return o;
                    }
                }
            }
            // La última línea puede traer rutas con el nombre del usuario: se limpian antes de enseñarla.
            let tail = crate::escalate::scrub(text.lines().map(str::trim).rev().find(|l| l.len() > 3).unwrap_or(""));
            let message = if tail.is_empty() { format!("winget terminó con el código {}", hex(code)) } else { format!("winget terminó con el código {}: {tail}", hex(code)) };
            Outcome { verdict: Failed, message, kind: "other", reboot: false, code: Some(hex(code)) }
        }
    }
}

/// Procesos del sistema, de la consola o del propio instalador: nunca se cierran para instalar un programa.
const NEVER_CLOSE: [&str; 14] = [
    "powershell", "pwsh", "cmd", "conhost", "explorer", "svchost", "dllhost", "rundll32", "msiexec", "winget", "adminops", "wininit", "csrss", "services",
];

/// ¿Este proceso pertenece al programa del catálogo? Solo coincidencia exacta del nombre del
/// ejecutable con el producto del id de winget (`Google.Chrome` → `chrome`; el primer trozo es el
/// fabricante y no cuenta) o con una palabra del nombre. Los procesos de `NEVER_CLOSE` y el propio
/// AdminOps no coinciden nunca.
pub fn process_matches(stem: &str, id: &str, name: &str) -> bool {
    let stem = stem.to_lowercase();
    let stem = stem.trim_end_matches(".exe");
    if stem.len() < 3 || NEVER_CLOSE.contains(&stem) {
        return false;
    }
    if let Some(own) = std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase())) {
        if own == stem {
            return false;
        }
    }
    let generic = ["microsoft", "google", "windows", "setup", "install", "installer", "app", "the", "pro", "free", "desktop"];
    let product = id.split('.').skip(1).flat_map(|seg| seg.split(['-', '_'])).map(str::to_lowercase).chain(std::iter::once(id.split('.').skip(1).collect::<String>().to_lowercase()));
    let parts = product.chain(name.split([' ', '-', '_', '.']).map(str::to_lowercase));
    let joined: String = name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    stem == joined || parts.filter(|p| p.len() >= 3 && !generic.contains(&p.as_str())).any(|p| p == stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_with_percentage() {
        assert_eq!(parse_line("  ██████████████▒▒▒▒▒▒▒▒▒▒▒▒▒▒  48%"), Line::Progress { percent: 48, done: None, total: None });
        assert_eq!(parse_line("  100%"), Line::Progress { percent: 100, done: None, total: None });
    }

    #[test]
    fn progress_with_sizes_in_english_and_spanish() {
        let en = parse_line("  ██████████████████████████████  52.0 MB / 64.0 MB");
        assert_eq!(en, Line::Progress { percent: 81, done: Some(52 * 1024 * 1024), total: Some(64 * 1024 * 1024) });
        let es = parse_line("  ███▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒  1,00 MB / 4,00 MB");
        assert_eq!(es, Line::Progress { percent: 25, done: Some(1024 * 1024), total: Some(4 * 1024 * 1024) });
        assert_eq!(parse_line("  ██  800 KB / 1.00 MB"), Line::Progress { percent: 78, done: Some(800 * 1024), total: Some(1024 * 1024) });
    }

    #[test]
    fn real_winget_download_lines_with_partial_blocks() {
        // Salida real de «winget download VideoLAN.VLC» dentro de una consola virtual.
        let total = Some(63_333_990);
        assert_eq!(parse_line("  █▍  2.00 MB / 60.4 MB"), Line::Progress { percent: 3, done: Some(2 * 1024 * 1024), total });
        assert_eq!(parse_line("  █████████████████████▊          44.0 MB / 60.4 MB"), Line::Progress { percent: 72, done: Some(44 * 1024 * 1024), total });
        assert_eq!(parse_line("  ██████████████████████████████  60.4 MB / 60.4 MB"), Line::Progress { percent: 100, done: total, total });
        for spinner in ["   |", "   /", "   -", "   \\"] {
            assert_eq!(parse_line(spinner), Line::Other, "{spinner:?}");
        }
    }

    #[test]
    fn phases_from_real_winget_lines() {
        assert_eq!(parse_line("Downloading https://dl.google.com/chrome/install/ChromeStandaloneSetup64.exe"), Line::Phase(Phase::Downloading));
        assert_eq!(parse_line("Descargando https://github.com/git-for-windows/git/releases/download/v2.47.1/Git.exe"), Line::Phase(Phase::Downloading));
        assert_eq!(parse_line("Successfully verified installer hash"), Line::Phase(Phase::Installing));
        assert_eq!(parse_line("Se verificó correctamente el hash del instalador"), Line::Phase(Phase::Installing));
        assert_eq!(parse_line("Starting package install..."), Line::Phase(Phase::Installing));
        assert_eq!(parse_line("Successfully installed"), Line::Phase(Phase::Success));
        assert_eq!(parse_line("Instalado correctamente"), Line::Phase(Phase::Success));
    }

    #[test]
    fn lines_that_say_nothing_are_other() {
        for l in ["Found Git [Git.Git] Version 2.47.1", "This application is licensed to you by its owner.", "-", "\\", "|", "/", "  ", "Microsoft is not responsible for, nor does it grant any licenses to, third-party packages."] {
            assert_eq!(parse_line(l), Line::Other, "{l:?}");
        }
        // Un porcentaje de más de 100 no es una barra.
        assert_eq!(parse_line("  250%"), Line::Other);
    }

    #[test]
    fn known_codes_in_spanish() {
        let o = interpret(0x8A15_0101u32 as i32 as i64, "");
        assert_eq!((o.verdict, o.kind), (Verdict::Failed, "close-app"));
        assert!(o.message.contains("cerrar"));
        assert_eq!(o.code.as_deref(), Some("0x8A150101"));
        assert_eq!(interpret(-1978335135, "").verdict, Verdict::Skipped); // ya instalado (0x8A150061)
        assert!(interpret(0x8A15_010E, "").message.contains("más nueva"));
        assert!(interpret(0x8A15_0105, "").message.contains("espacio"));
        assert_eq!(interpret(0x8A15_0107, "").kind, "network");
        assert_eq!(interpret(0x8A15_0014, "").kind, "missing");
        assert_eq!(interpret(0x8A15_0011, "").kind, "hash");
        assert_eq!(interpret(0x8A15_010F, "").kind, "policy");
        assert_eq!(interpret(0x8A15_0102, "").kind, "busy");
        assert_eq!(interpret(0x8A15_010C, "").kind, "cancelled");
        assert_eq!(interpret(0x8A15_010C, "").verdict, Verdict::Skipped);
    }

    #[test]
    fn reboot_codes_are_a_success() {
        for c in [3010, 1641, 0x8A15_0109] {
            let o = interpret(c, "");
            assert_eq!(o.verdict, Verdict::Done, "{c}");
            assert!(o.reboot, "{c}");
        }
        assert_eq!(interpret(0, "").verdict, Verdict::Done);
        assert!(!interpret(0, "").reboot);
    }

    #[test]
    fn winget_wraps_the_installer_code() {
        // 0x8A150049: el instalador falló con su propio código.
        let o = interpret(0x8A15_0049, "Installer failed with exit code: 1618");
        assert_eq!(o.kind, "busy");
        let o = interpret(0x8A15_0049, "El instalador falló con el código de salida: 1603");
        assert!(o.message.contains("1603"), "{}", o.message);
    }

    #[test]
    fn unknown_error_line_hides_the_user_name() {
        let o = interpret(0x8A15_0099u32 as i32 as i64, r"No se pudo escribir C:\Users\ana\AppData\Local\x.log");
        assert!(o.message.contains(r"C:\Users\[usuario]\AppData") && !o.message.contains("ana"), "{}", o.message);
    }

    #[test]
    fn winget_missing_is_a_prepared_message() {
        for (code, text) in [(9009, ""), (1, "'winget.exe' no se reconoce como un comando interno o externo"), (1, "'winget.exe' is not recognized as an internal or external command,")] {
            let o = interpret(code, text);
            assert_eq!((o.verdict, o.message.as_str()), (Verdict::Failed, WINGET_MISSING), "{code} {text}");
        }
    }

    #[test]
    fn unknown_code_shows_hex_and_winget_text() {
        let o = interpret(0x8A15_0099u32 as i32 as i64, "Algo raro ha pasado.");
        assert_eq!(o.verdict, Verdict::Failed);
        assert_eq!(o.message, "winget terminó con el código 0x8A150099: Algo raro ha pasado.");
        let o = interpret(42, "");
        assert_eq!(o.message, "winget terminó con el código 0x2A");
    }

    #[test]
    fn only_the_program_own_process_matches() {
        assert!(process_matches("chrome", "Google.Chrome", "Google Chrome"));
        assert!(process_matches("firefox.exe", "Mozilla.Firefox", "Mozilla Firefox"));
        assert!(process_matches("7zip", "7zip.7zip", "7-Zip"));
        assert!(!process_matches("explorer", "Google.Chrome", "Google Chrome"));
        assert!(!process_matches("google", "Google.Chrome", "Google Chrome"));
        assert!(!process_matches("sv", "Foo.Bar", "Bar"));
        // El fabricante (primer trozo del id) no cuenta: «Mozilla» no es un programa.
        assert!(!process_matches("mozilla", "Mozilla.Firefox", "Firefox"));
    }

    #[test]
    fn never_matches_shells_or_system_processes() {
        assert!(!process_matches("powershell.exe", "Microsoft.PowerShell", "PowerShell"));
        assert!(!process_matches("pwsh", "Microsoft.PowerShell", "PowerShell 7"));
        assert!(!process_matches("cmd", "Foo.Cmd", "Cmd"));
        assert!(!process_matches("explorer", "Foo.Explorer", "Explorer"));
        assert!(!process_matches("winget", "Microsoft.Winget", "winget"));
        assert!(!process_matches("AdminOps", "Foo.AdminOps", "AdminOps"));
        assert!(process_matches("chrome.exe", "Google.Chrome", "Google Chrome"));
    }
}
