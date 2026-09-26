//! Ejecución de procesos del sistema sin ventana (sin parpadeo de consola).

use base64::Engine;
use std::process::{Command, Output};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `Command` que no abre ventana de consola.
pub fn hidden(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Ejecuta un script de PowerShell y devuelve su salida estándar (recortada).
///
/// El script va como `-EncodedCommand` (UTF-16LE en base64) para no tener que
/// escapar comillas. Cualquier error no controlado sale con código ≠ 0.
pub fn powershell(script: &str) -> Result<String, String> {
    let full = format!(
        "$ProgressPreference='SilentlyContinue';$ErrorActionPreference='Stop';\
         [Console]::OutputEncoding=[Text.Encoding]::UTF8;\n{script}"
    );
    let utf16: Vec<u8> = full.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);

    let out = hidden("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded])
        .output()
        .map_err(|e| format!("No se pudo iniciar PowerShell: {e}"))?;
    finish(out)
}

/// Ejecuta un programa del sistema (sc.exe, ipconfig…) sin ventana.
pub fn exec(program: &str, args: &[&str]) -> Result<String, String> {
    let out = hidden(program)
        .args(args)
        .output()
        .map_err(|e| format!("No se pudo ejecutar {program}: {e}"))?;
    finish(out)
}

fn finish(out: Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        // PowerShell serializa los errores como CLIXML en stderr; nos quedamos con el texto útil.
        let msg = if stderr.is_empty() { stdout } else { clean_clixml(&stderr) };
        Err(if msg.is_empty() { format!("Código de salida {:?}", out.status.code()) } else { msg })
    }
}

fn clean_clixml(s: &str) -> String {
    if !s.contains("#< CLIXML") {
        return s.to_string();
    }
    s.split("<S S=\"Error\">")
        .skip(1)
        .filter_map(|part| part.split("</S>").next())
        .map(|t| t.replace("_x000D__x000A_", " ").trim().to_string())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
