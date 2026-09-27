//! Ejecución de procesos del sistema sin ventana, con límite de tiempo y
//! cancelación.
//!
//! Todo lo que AdminOps lanza (PowerShell, sc.exe, winget…) pasa por aquí, así
//! que ninguna operación puede dejar la interfaz esperando para siempre: al
//! superar su límite, o si el usuario pulsa Cancelar, se termina el árbol de
//! procesos completo (p. ej. PowerShell *y* el sfc.exe que lanzó).

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Límite para consultas y ajustes normales. Las tareas largas declaran el suyo.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Opciones de ejecución.
#[derive(Clone, Copy)]
pub struct Opts<'a> {
    /// `None` = sin límite (solo para lo que el usuario puede cancelar).
    pub timeout: Option<Duration>,
    /// Clave con la que la UI puede cancelar esta ejecución.
    pub task: Option<&'a str>,
}

impl Default for Opts<'_> {
    fn default() -> Self {
        Opts { timeout: Some(DEFAULT_TIMEOUT), task: None }
    }
}

impl<'a> Opts<'a> {
    pub fn task(task: &'a str, timeout: Option<Duration>) -> Self {
        Opts { timeout, task: Some(task) }
    }
}

/// Procesos en curso por tarea, y tareas que el usuario canceló.
static RUNNING: LazyLock<Mutex<HashMap<String, Vec<u32>>>> = LazyLock::new(Default::default);
static CANCELLED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

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

fn kill_tree(pid: u32) {
    let _ = hidden("taskkill.exe")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Cancela todo lo que se esté ejecutando bajo `task`. Devuelve si había algo.
pub fn cancel(task: &str) -> bool {
    let pids = RUNNING.lock().unwrap_or_else(|e| e.into_inner()).get(task).cloned().unwrap_or_default();
    if pids.is_empty() {
        return false;
    }
    CANCELLED.lock().unwrap_or_else(|e| e.into_inner()).insert(task.to_string());
    log::warn!("Cancelando tarea {task} (pids {pids:?})");
    for pid in pids {
        kill_tree(pid);
    }
    true
}

/// Marca una tarea como terminada (limpia el estado de cancelación).
pub fn finish_task(task: &str) {
    CANCELLED.lock().unwrap_or_else(|e| e.into_inner()).remove(task);
}

pub fn is_cancelled(task: &str) -> bool {
    CANCELLED.lock().unwrap_or_else(|e| e.into_inner()).contains(task)
}

pub const CANCELLED_MSG: &str = "Cancelado por el usuario.";

fn summary(s: &str) -> String {
    let line = s.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("");
    line.chars().take(90).collect()
}

fn run(cmd: Command, label: &str, detail: &str, opts: Opts) -> Result<String, String> {
    run_with_input(cmd, label, detail, opts, None)
}

fn run_with_input(mut cmd: Command, label: &str, detail: &str, opts: Opts, input: Option<String>) -> Result<String, String> {
    if let Some(t) = opts.task {
        if is_cancelled(t) {
            return Err(CANCELLED_MSG.into());
        }
    }
    cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("No se pudo iniciar {label}: {e}"))?;
    let pid = child.id();
    if let Some(t) = opts.task {
        RUNNING.lock().unwrap_or_else(|e| e.into_inner()).entry(t.to_string()).or_default().push(pid);
    }

    // Leer en hilos aparte: si la salida llena el buffer del pipe y nadie la
    // lee, el proceso se bloquea y parecería colgado.
    // El script por la entrada estándar, en un hilo: si es largo y el proceso no
    // lo lee a tiempo, escribirlo aquí podría bloquear.
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let out_h = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = out_pipe.read_to_end(&mut v);
        v
    });
    let err_h = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = err_pipe.read_to_end(&mut v);
        v
    });

    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {}
            Err(_) => break None,
        }
        if opts.timeout.is_some_and(|t| start.elapsed() > t) {
            timed_out = true;
            kill_tree(pid);
            break child.wait().ok();
        }
        std::thread::sleep(Duration::from_millis(40));
    };

    if let Some(t) = opts.task {
        if let Some(v) = RUNNING.lock().unwrap_or_else(|e| e.into_inner()).get_mut(t) {
            v.retain(|p| *p != pid);
        }
    }
    let stdout = String::from_utf8_lossy(&out_h.join().unwrap_or_default()).trim().to_string();
    let stderr = String::from_utf8_lossy(&err_h.join().unwrap_or_default()).trim().to_string();
    let ms = start.elapsed().as_millis();

    if opts.task.is_some_and(is_cancelled) {
        log::warn!("{label} cancelado tras {ms} ms: {detail}");
        return Err(CANCELLED_MSG.into());
    }
    if timed_out {
        let secs = opts.timeout.map_or(0, |t| t.as_secs());
        log::error!("{label} superó {secs} s y se detuvo: {detail}");
        return Err(format!("{label} tardó más de {secs} s y se detuvo."));
    }
    match status {
        Some(s) if s.success() => {
            if ms > 3000 {
                log::info!("{label} ok en {ms} ms: {detail}");
            } else {
                log::debug!("{label} ok en {ms} ms: {detail}");
            }
            Ok(stdout)
        }
        s => {
            let raw = if stderr.is_empty() { stdout } else { stderr };
            let msg = clean_error(&raw);
            let msg = if msg.is_empty() { format!("Código de salida {:?}", s.and_then(|s| s.code())) } else { msg };
            log::warn!("{label} falló en {ms} ms: {detail} → {msg}");
            Err(msg)
        }
    }
}

/// Arranque de los PowerShell de un solo uso: lee el script de la entrada estándar
/// (UTF-8) y lo ejecuta. Sin `-EncodedCommand` ni base64, que los antivirus de
/// empresa (Sophos, Defender for Endpoint…) tratan como señal de malware.
pub const STDIN_BOOTSTRAP: &str = "[Console]::InputEncoding = [Text.Encoding]::UTF8; $adminopsScript = [Console]::In.ReadToEnd(); & ([ScriptBlock]::Create($adminopsScript))";

/// Ejecuta un script de PowerShell y devuelve su salida estándar.
/// Cualquier error no controlado sale con código ≠ 0.
pub fn powershell(script: &str) -> Result<String, String> {
    powershell_opts(script, Opts::default())
}

pub fn powershell_opts(script: &str, opts: Opts) -> Result<String, String> {
    // Consultas sin tarea: proceso persistente (ver pspool). Lo cancelable va
    // en un proceso propio para poder terminarlo sin afectar a nadie más.
    if opts.task.is_none() && std::env::var_os("ADMINOPS_NO_PS_POOL").is_none() {
        return crate::pspool::query(script, opts.timeout, &summary(script));
    }
    let full = format!(
        "$ProgressPreference='SilentlyContinue';$ErrorActionPreference='Stop';\
         [Console]::OutputEncoding=[Text.Encoding]::UTF8;\n{script}"
    );
    let mut cmd = hidden("powershell.exe");
    // -ExecutionPolicy Bypass (solo este proceso): con la directiva por defecto de
    // Windows ni siquiera cargan los módulos del sistema (discos, red…).
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", STDIN_BOOTSTRAP]);
    run_with_input(cmd, "PowerShell", &summary(script), opts, Some(full))
}

/// Texto cualquiera como cadena literal de PowerShell (entre comillas simples):
/// dentro no se interpreta nada (`$()`, comillas dobles, saltos de línea…). Solo
/// hay que duplicar las comillas simples, incluidas las tipográficas, que
/// PowerShell también trata como comillas.
pub fn ps_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for c in value.chars().filter(|c| *c != '\0') {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// Línea de PowerShell que define `$var` como SecureString con una contraseña,
/// sin que aparezca legible en el script ni en el registro: es el formato de
/// `ConvertFrom-SecureString` (cifrado con DPAPI para este usuario de Windows).
/// Vacía → `$null`.
pub fn secret_var(var: &str, secret: &str) -> String {
    if secret.is_empty() {
        return format!("${var} = $null\n");
    }
    let utf16: Vec<u8> = secret.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    match crate::network::lan::dpapi(&utf16, true) {
        Ok(blob) => {
            let hex: String = blob.iter().map(|b| format!("{b:02x}")).collect();
            format!("${var} = ConvertTo-SecureString '{hex}'\n")
        }
        // Sin DPAPI (no debería pasar en Windows): literal, nunca sin contraseña.
        Err(_) => format!("${var} = ConvertTo-SecureString {} -AsPlainText -Force\n", ps_literal(secret)),
    }
}

/// Línea de PowerShell que define `$var` con un texto cualquiera, sin que nada
/// del texto se interprete (ver `ps_literal`).
pub fn text_var(var: &str, value: &str) -> String {
    format!("${var} = {}\n", ps_literal(value))
}

/// Errores de sintaxis de un script según el parser de PowerShell (vacío = correcto).
/// No lo ejecuta. Para los tests de los módulos con scripts incrustados.
#[cfg(test)]
pub fn parse_errors(script: &str) -> String {
    let probe = format!(
        "{}$e = $null; [void][System.Management.Automation.Language.Parser]::ParseInput($code, [ref]$null, [ref]$e)
         ($e | ForEach-Object {{ \"$($_.Extent.StartLineNumber): $($_.Message)\" }}) -join ' | '",
        text_var("code", script)
    );
    crate::pspool::query(&probe, None, "parse").unwrap().trim().to_string()
}

/// Ejecuta un programa del sistema (sc.exe, winget…) sin ventana.
pub fn exec(program: &str, args: &[&str]) -> Result<String, String> {
    exec_opts(program, args, Opts::default())
}

pub fn exec_opts(program: &str, args: &[&str], opts: Opts) -> Result<String, String> {
    let mut cmd = hidden(program);
    cmd.args(args);
    run(cmd, program, &args.join(" "), opts)
}

/// Deja solo el mensaje útil de un error de PowerShell: sin CLIXML, sin la
/// posición en el script ("At line:5 char:25 + ... ~~~~") ni CategoryInfo.
/// Mensajes claros para los bloqueos típicos de un equipo de empresa.
pub fn blocked_reason(raw: &str) -> Option<&'static str> {
    let l = raw.to_lowercase();
    if l.contains("malicious content") || l.contains("scriptcontainedmaliciouscontent") || l.contains("contenido malintencionado") || l.contains("software antivirus") {
        return Some("El antivirus del equipo bloqueó esta acción de AdminOps. Si confías en ella, pide a TI que añada AdminOps a las exclusiones del antivirus.");
    }
    if l.contains("language mode") || l.contains("modo de lenguaje") || l.contains("only on core types") || l.contains("solo en tipos principales") {
        return Some("PowerShell está restringido en este equipo por una directiva de la empresa (modo de lenguaje restringido): esta función de AdminOps no puede ejecutarse aquí.");
    }
    None
}

pub fn clean_error(s: &str) -> String {
    if let Some(reason) = blocked_reason(s) {
        return reason.into();
    }
    let mut text = if s.contains("#< CLIXML") {
        s.split("<S S=\"Error\">")
            .skip(1)
            .filter_map(|part| part.split("</S>").next())
            .map(|t| t.replace("_x000D__x000A_", "\n"))
            .collect::<Vec<_>>()
            .join("")
    } else {
        s.to_string()
    };
    for marker in ["At line:", "En línea:", "At char:", "+ CategoryInfo", "+ FullyQualifiedErrorId"] {
        if let Some(i) = text.find(marker) {
            text.truncate(i);
        }
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // "Get-AppxPackage : mensaje" → "mensaje"
    let text = match text.split_once(" : ") {
        Some((cmdlet, rest)) if !cmdlet.contains(' ') && cmdlet.contains('-') => rest.to_string(),
        _ => text,
    };
    text.chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_powershell_errors() {
        let raw = "Get-AppxProvisionedPackage : Another operation on app packages (.appx) is in progress. Wait for the \
                   current operation to complete and then retry the command.\r\nAt line:5 char:25\r\n+ $prov = if ($admin) \
                   { @(Get-AppxProvisionedPackage -Online | ForEach- ...\r\n+ ~~~~~~~~~\r\n    + CategoryInfo : NotSpecified";
        assert_eq!(
            clean_error(raw),
            "Another operation on app packages (.appx) is in progress. Wait for the current operation to complete and then retry the command."
        );
    }

    #[test]
    fn timeout_kills_the_process() {
        let start = Instant::now();
        let r = powershell_opts("Start-Sleep -Seconds 30", Opts { timeout: Some(Duration::from_secs(2)), task: None });
        assert!(r.unwrap_err().contains("tardó más de 2 s"));
        // Corta mucho antes de los 30 s del script. Margen amplio: con los tests en
        // paralelo puede esperar turno en el pool de PowerShell antes de empezar.
        assert!(start.elapsed() < Duration::from_secs(25));
    }

    #[test]
    fn cancel_stops_a_running_task() {
        let h = std::thread::spawn(|| powershell_opts("Start-Sleep -Seconds 30", Opts::task("test-cancel", None)));
        // Esperar a que el proceso esté registrado.
        let start = Instant::now();
        while !cancel("test-cancel") {
            assert!(start.elapsed() < Duration::from_secs(10), "la tarea nunca arrancó");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(h.join().unwrap().unwrap_err(), CANCELLED_MSG);
        finish_task("test-cancel");
    }

    #[test]
    fn large_output_does_not_deadlock() {
        let out = powershell("1..20000 | ForEach-Object { 'linea de salida bastante larga ' + $_ }").unwrap();
        assert!(out.lines().count() >= 20000);
    }
}
