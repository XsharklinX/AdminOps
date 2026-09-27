//! "Probar" en la página de Atajos: simula una combinación de sistema con la
//! tecla Windows (Win+V, Win+Shift+S…) para ver qué hace.
//!
//! Solo combinaciones que empiezan por Win y teclas de una lista cerrada: la
//! interfaz no puede enviar pulsaciones arbitrarias a otras aplicaciones.

/// Código de tecla virtual para cada nombre admitido.
fn vk(name: &str) -> Option<u16> {
    let n = name.to_ascii_uppercase();
    let code = match n.as_str() {
        "WIN" => 0x5B,
        "CTRL" => 0x11,
        "SHIFT" => 0x10,
        "ALT" => 0x12,
        "TAB" => 0x09,
        "ENTER" => 0x0D,
        "SPACE" => 0x20,
        "PRTSCN" => 0x2C,
        "PAUSE" => 0x13,
        "HOME" => 0x24,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        "+" => 0xBB,
        "-" => 0xBD,
        "." => 0xBE,
        "," => 0xBC,
        s if s.len() == 1 && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) => s.as_bytes()[0] as u16,
        _ => return None,
    };
    Some(code)
}

/// Valida la combinación y devuelve sus códigos. `Win` + modificadores + una tecla final.
pub fn parse(keys: &[String]) -> Result<Vec<u16>, String> {
    if keys.len() < 2 || keys.len() > 4 || !keys[0].eq_ignore_ascii_case("Win") {
        return Err("Solo se pueden probar atajos de la tecla Windows.".into());
    }
    let codes: Vec<u16> = keys.iter().map(|k| vk(k).ok_or_else(|| format!("Tecla no admitida: {k}"))).collect::<Result<_, _>>()?;
    // Win+L bloquea la sesión: se puede, pero no por un clic accidental en "Probar".
    if codes == [0x5B, b'L' as u16] {
        return Err("Win + L bloquea el equipo: pruébalo directamente con el teclado.".into());
    }
    Ok(codes)
}

#[cfg(windows)]
fn send(codes: &[u16]) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP};
    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
    };
    // Pulsar en orden y soltar en orden inverso, como una persona.
    let mut inputs: Vec<INPUT> = codes.iter().map(|&c| key(c, false)).collect();
    inputs.extend(codes.iter().rev().map(|&c| key(c, true)));
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() { Ok(()) } else { Err("Windows no aceptó la combinación.".into()) }
}

#[cfg(not(windows))]
fn send(_: &[u16]) -> Result<(), String> {
    Err("Solo disponible en Windows.".into())
}

#[tauri::command(async)]
pub fn try_shortcut(keys: Vec<String>) -> Result<(), String> {
    let codes = parse(&keys)?;
    // Un momento para que se suelte el clic del ratón en el botón.
    std::thread::sleep(std::time::Duration::from_millis(250));
    send(&codes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todo atajo marcado con `try: true` en la página Atajos se puede simular.
    #[test]
    fn every_tryable_shortcut_parses() {
        let catalog = include_str!("../../src/lib/shortcuts.ts");
        let mut checked = 0;
        for line in catalog.lines().filter(|l| l.contains("try: true")) {
            let start = line.find("keys: [").expect("línea con keys") + 7;
            let end = start + line[start..].find(']').expect("fin de keys");
            let keys: Vec<String> = line[start..end].split(',').map(|k| k.trim().trim_matches('"').to_string()).collect();
            assert!(parse(&keys).is_ok(), "no se puede probar: {keys:?}");
            checked += 1;
        }
        assert!(checked > 30, "solo {checked} atajos probables");
    }

    fn k(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn only_windows_key_combos() {
        assert_eq!(parse(&k(&["Win", "V"])).unwrap(), vec![0x5B, b'V' as u16]);
        assert_eq!(parse(&k(&["Win", "Shift", "S"])).unwrap(), vec![0x5B, 0x10, b'S' as u16]);
        assert_eq!(parse(&k(&["Win", "."])).unwrap(), vec![0x5B, 0xBE]);
        assert!(parse(&k(&["Ctrl", "Alt", "Delete"])).is_err());
        assert!(parse(&k(&["Alt", "F4"])).is_err());
        assert!(parse(&k(&["Win", "L"])).is_err());
        assert!(parse(&k(&["Win", "F13"])).is_err());
        assert!(parse(&k(&["Win"])).is_err());
    }
}
