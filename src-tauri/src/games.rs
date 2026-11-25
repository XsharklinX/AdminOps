//! ¿Es un juego? Muchos juegos dejan en el Visor de eventos un «error de
//! aplicación» (1000) al salir con Alt+F4, o un «dejó de responder» (1002)
//! cuando se cierran a mano mientras cargan. Eso no es una avería del equipo:
//! los avisos, el diagnóstico y la grabación automática los tratan aparte.
//!
//! Se reconoce un juego por dónde está instalado (las bibliotecas de Steam,
//! Epic, GOG, Xbox…) o porque Windows ya lo tiene apuntado como juego (la
//! Barra de juegos guarda la lista en `HKCU\System\GameConfigStore\Children`).

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Trozos de ruta de las bibliotecas de juegos (en minúsculas, con `\`).
const LIBRARY_MARKERS: &[&str] = &[
    "\\steamapps\\common\\",
    "\\epic games\\",
    "\\gog galaxy\\games\\",
    "\\gog games\\",
    "\\xboxgames\\",
    "\\riot games\\",
    "\\ea games\\",
    "\\ubisoft game launcher\\games\\",
    "\\rockstar games\\",
    "\\amazon games\\library\\",
    "\\itch\\apps\\",
];

fn norm(path: &str) -> String {
    path.trim().replace('/', "\\").to_lowercase()
}

/// Por la ruta: está dentro de una biblioteca de juegos (y no es el lanzador de la tienda).
pub fn in_game_library(path: &str) -> bool {
    let p = norm(path);
    !p.is_empty() && !p.contains("\\launcher\\") && LIBRARY_MARKERS.iter().any(|m| p.contains(m))
}

/// El primer dato de un evento que parece la ruta de un .exe (`C:\…\juego.exe`).
pub fn exe_path_in<'a>(values: impl IntoIterator<Item = &'a str>) -> Option<String> {
    values.into_iter().map(str::trim).find(|v| {
        let b = v.as_bytes();
        b.len() > 7 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') && v.to_lowercase().ends_with(".exe")
    }).map(str::to_string)
}

/// Los juegos que la Barra de juegos de Windows tiene apuntados: rutas completas y nombres de archivo.
#[derive(Default)]
struct Known {
    paths: HashSet<String>,
    files: HashSet<String>,
}

static KNOWN: Mutex<Option<(Instant, std::sync::Arc<Known>)>> = Mutex::new(None);
const KNOWN_TTL: Duration = Duration::from_secs(10 * 60);

fn known() -> std::sync::Arc<Known> {
    let mut g = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, k)) = g.as_ref() {
        if at.elapsed() < KNOWN_TTL {
            return k.clone();
        }
    }
    let k = std::sync::Arc::new(read_known());
    *g = Some((Instant::now(), k.clone()));
    k
}

#[cfg(windows)]
fn read_known() -> Known {
    use winreg::enums::HKEY_CURRENT_USER;
    let mut k = Known::default();
    let Ok(children) = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey("System\\GameConfigStore\\Children") else { return k };
    for name in children.enum_keys().flatten().take(2000) {
        let Ok(child) = children.open_subkey(&name) else { continue };
        let Ok(path) = child.get_value::<String, _>("MatchedExeFullPath") else { continue };
        let p = norm(&path);
        // Lo que está dentro de Windows no es un juego aunque la Barra de juegos lo haya apuntado.
        if p.is_empty() || p.contains("\\windows\\") {
            continue;
        }
        if let Some(file) = p.rsplit('\\').next() {
            k.files.insert(file.to_string());
        }
        k.paths.insert(p);
    }
    k
}

#[cfg(not(windows))]
fn read_known() -> Known {
    Known::default()
}

/// ¿Es un juego? Con la ruta, por la ruta (biblioteca o lista de Windows); sin
/// ella, solo si el nombre del archivo está en la lista de Windows.
pub fn is_game(exe: &str, path: Option<&str>) -> bool {
    if let Some(p) = path.filter(|p| !p.trim().is_empty()) {
        return in_game_library(p) || known().paths.contains(&norm(p));
    }
    let file = norm(exe);
    let file = file.rsplit('\\').next().unwrap_or(&file);
    !file.is_empty() && known().files.contains(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_las_bibliotecas_de_juegos() {
        assert!(in_game_library(r"D:\SteamLibrary\steamapps\common\Hades\Hades.exe"));
        assert!(in_game_library(r"C:\Program Files\Epic Games\Fortnite\FortniteClient-Win64-Shipping.exe"));
        assert!(in_game_library("C:/XboxGames/Forza/Content/forza.exe"));
        assert!(in_game_library(r"C:\Riot Games\VALORANT\live\VALORANT.exe"));
        assert!(!in_game_library(r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE"));
        assert!(!in_game_library(r"C:\Program Files\Steam\steam.exe"), "el lanzador de Steam no es un juego");
        assert!(!in_game_library(r"C:\Program Files (x86)\Epic Games\Launcher\Portal\Binaries\Win64\EpicGamesLauncher.exe"));
        assert!(!in_game_library(r"C:\Program Files\Rockstar Games\Launcher\Launcher.exe"));
        assert!(!in_game_library(""));
    }

    #[test]
    fn saca_la_ruta_del_exe_de_los_datos_del_evento() {
        // Evento 1000: nombre, versión…, y en el 11.º dato la ruta.
        let v = ["Hades.exe", "1.0", "5f1a", "ntdll.dll", "10.0", "abc", "c0000005", "0x1234", "1a2b", "01d9", r"D:\SteamLibrary\steamapps\common\Hades\Hades.exe", r"C:\Windows\SYSTEM32\ntdll.dll"];
        assert_eq!(exe_path_in(v).as_deref(), Some(r"D:\SteamLibrary\steamapps\common\Hades\Hades.exe"));
        assert_eq!(exe_path_in(["EXCEL.EXE", "16.0", "x.exe", r"\\servidor\x.exe"]), None);
    }

    #[test]
    fn con_ruta_de_biblioteca_es_juego() {
        assert!(is_game("Hades.exe", Some(r"D:\SteamLibrary\steamapps\common\Hades\Hades.exe")));
        assert!(!is_game("EXCEL.EXE", Some(r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE")));
    }
}
