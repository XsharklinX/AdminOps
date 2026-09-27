//! Ejecución de procesos del sistema sin ventana, con límite de tiempo y
//! cancelación.
//!
//! Todo lo que AdminOps lanza (PowerShell, sc.exe, winget…) pasa por aquí, así
//! que ninguna operación puede dejar la interfaz esperando para siempre: al
//! superar su límite, o si el usuario pulsa Cancelar, se termina el árbol de
//! procesos completo (p. ej. PowerShell *y* el sfc.exe que lanzó).

use base64::Engine;
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
    let pids = RUNNING.lock().unwrap().get(task).cloned().unwrap_or_default();
    if pids.is_empty() {
        return false;
    }
    CANCELLED.lock().unwrap().insert(task.to_string());
    log::warn!("Cancelando tarea {task} (pids {pids:?})");
    for pid in pids {
        kill_tree(pid);
    }
    true
}

/// Marca una tarea como terminada (limpia el estado de cancelación).
pub fn finish_task(task: &str) {
    CANCELLED.lock().unwrap().remove(task);
}

pub fn is_cancelled(task: &str) -> bool {
    CANCELLED.lock().unwrap().contains(task)
}

pub const CANCELLED_MSG: &str = "Cancelado por el usuario.";

fn summary(s: &str) -> String {
    let line = s.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("");
    line.chars().take(90).collect()
}

fn run(mut cmd: Command, label: &str, detail: &str, opts: Opts) -> Result<String, String> {
    if let Some(t) = opts.task {
        if is_cancelled(t) {
            return Err(CANCELLED_MSG.into());
        }
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("No se pudo iniciar {label}: {e}"))?;
    let pid = child.id();
    if let Some(t) = opts.task {
        RUNNING.lock().unwrap().entry(t.to_string()).or_default().push(pid);
    }

    // Leer en hilos aparte: si la salida llena el buffer del pipe y nadie la
    // lee, el proceso se bloquea y parecería colgado.
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
        if let Some(v) = RUNNING.lock().unwrap().get_mut(t) {
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

/// Ejecuta un script de PowerShell y devuelve su salida estándar.
///
/// El script va como `-EncodedCommand` (UTF-16LE en base64) para no tener que
/// escapar comillas. Cualquier error no controlado sale con código ≠ 0.
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
    let utf16: Vec<u8> = full.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let mut cmd = hidden("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded]);
    run(cmd, "PowerShell", &summary(script), opts)
}

/// Línea de PowerShell que define `$var` con un texto cualquiera. Va en base64,
/// así que comillas, `$()` o saltos de línea del texto nunca se interpretan.
pub fn text_var(var: &str, value: &str) -> String {
    format!(
        "${var} = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}'))\n",
        base64::engine::general_purpose::STANDARD.encode(value.as_bytes())
    )
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
pub fn clean_error(s: &str) -> String {
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
        assert!(start.elapsed() < Duration::from_secs(10));
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
