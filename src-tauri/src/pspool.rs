//! Procesos de PowerShell reutilizables para consultas.
//!
//! Arrancar powershell.exe cuesta ~300 ms; una consulta típica (detectar un
//! ajuste, leer una lista) tarda mucho menos. Aquí se mantienen hasta
//! `MAX_HOSTS` procesos vivos que reciben scripts por stdin y devuelven el
//! resultado por stdout con un protocolo de una línea.
//!
//! Solo se usa para consultas sin tarea asociada: lo que el usuario puede
//! cancelar o lo que modifica el sistema sigue yendo en procesos aislados.
//! Cada script corre en su propio ámbito (`& { }`), así que sus variables no
//! se filtran a la siguiente consulta.

use base64::Engine;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

/// 5 procesos: el diagnóstico lanza ~10 consultas a la vez (medido: 15 % más rápido que con 3).
/// Los que sobran de `KEEP_IDLE` se cierran tras `IDLE_TTL` sin uso, así que en reposo no gasta más.
const MAX_HOSTS: usize = 5;
const KEEP_IDLE: usize = 2;
const IDLE_TTL: Duration = Duration::from_secs(180);

/// Tamaño del pool (se puede forzar con ADMINOPS_PS_HOSTS para medir).
fn max_hosts() -> usize {
    std::env::var("ADMINOPS_PS_HOSTS").ok().and_then(|v| v.parse().ok()).filter(|n| (1..=8).contains(n)).unwrap_or(MAX_HOSTS)
}
const MARKER: &str = "\u{1e}ADMINOPS ";

/// Bucle del proceso anfitrión. Cada línea de entrada es un script en base64
/// (UTF-8); cada respuesta es `MARKER OK|ERR <base64>`.
const HOST_SCRIPT: &str = r#"
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$ProgressPreference = 'SilentlyContinue'
while ($true) {
  $line = [Console]::In.ReadLine()
  if ($null -eq $line) { break }
  $ok = $true
  try {
    $sb = [ScriptBlock]::Create([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($line)))
    $out = @(& { $ErrorActionPreference = 'Stop'; & $sb }) | ForEach-Object {
      if ($_ -is [string]) { $_ } else { ($_ | Out-String -Width 4096).TrimEnd() }
    }
    $text = $out -join "`n"
  } catch {
    $ok = $false
    $text = $_.Exception.Message
  }
  $b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes([string]$text))
  [Console]::Out.WriteLine("$([char]0x1e)ADMINOPS $(if ($ok) { 'OK' } else { 'ERR' }) $b64")
  [Console]::Out.Flush()
}
"#;

struct Host {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

/// Mete el proceso en un "job" que Windows cierra al terminar AdminOps (por la
/// razón que sea, también si se mata): ningún PowerShell del pool queda vivo
/// reteniendo archivos (p. ej. las DLL de sensores durante una actualización).
#[cfg(windows)]
fn kill_with_app(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    // El handle del job vive lo que vive AdminOps (nunca se cierra a mano).
    static JOB: OnceLock<usize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        job as usize
    });
    if job != 0 {
        unsafe { AssignProcessToJobObject(job as _, child.as_raw_handle() as _) };
    }
}

#[cfg(not(windows))]
fn kill_with_app(_: &std::process::Child) {}

impl Host {
    fn spawn() -> Result<Host, String> {
        let utf16: Vec<u8> = HOST_SCRIPT.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
        let mut child = crate::ps::hidden("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // stderr no se lee: si se dejara en pipe podría llenarse y bloquear el proceso.
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("No se pudo iniciar PowerShell: {e}"))?;
        kill_with_app(&child);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        log::debug!("PowerShell persistente iniciado (pid {})", child.id());
        Ok(Host { child, stdin, lines: rx })
    }

    /// Ejecuta un script. `Err(None)` = el proceso quedó inservible.
    fn run(&mut self, script: &str, timeout: Option<Duration>) -> Result<Result<String, String>, String> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(script.as_bytes());
        writeln!(self.stdin, "{b64}").and_then(|_| self.stdin.flush()).map_err(|e| e.to_string())?;
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            let wait = deadline.map_or(Duration::from_secs(3600), |d| d.saturating_duration_since(Instant::now()));
            match self.lines.recv_timeout(wait) {
                // Líneas sueltas (p. ej. Write-Host) se ignoran: solo cuenta el marcador.
                Ok(line) => {
                    let Some(rest) = line.strip_prefix(MARKER) else { continue };
                    let (status, payload) = rest.split_once(' ').unwrap_or((rest, ""));
                    let bytes = base64::engine::general_purpose::STANDARD.decode(payload).unwrap_or_default();
                    let text = String::from_utf8_lossy(&bytes).trim().to_string();
                    return Ok(if status == "OK" { Ok(text) } else { Err(text) });
                }
                Err(RecvTimeoutError::Timeout) => return Err("timeout".into()),
                Err(RecvTimeoutError::Disconnected) => return Err("el proceso terminó".into()),
            }
        }
    }

    fn kill(mut self) {
        let _ = crate::ps::hidden("taskkill.exe")
            .args(["/T", "/F", "/PID", &self.child.id().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.wait();
    }
}

struct Pool {
    /// Procesos libres y desde cuándo.
    idle: Vec<(Host, Instant)>,
    live: usize,
}

static POOL: LazyLock<(Mutex<Pool>, Condvar)> =
    LazyLock::new(|| (Mutex::new(Pool { idle: Vec::new(), live: 0 }), Condvar::new()));

fn acquire() -> Result<Host, String> {
    let (lock, cv) = &*POOL;
    let mut pool = lock.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if let Some((h, _)) = pool.idle.pop() {
            return Ok(h);
        }
        if pool.live < max_hosts() {
            pool.live += 1;
            drop(pool);
            return Host::spawn().inspect_err(|_| {
                lock.lock().unwrap_or_else(|e| e.into_inner()).live -= 1;
                cv.notify_one();
            });
        }
        pool = cv.wait(pool).unwrap_or_else(|e| e.into_inner());
    }
}

fn release(host: Option<Host>) {
    let (lock, cv) = &*POOL;
    let mut pool = lock.lock().unwrap_or_else(|e| e.into_inner());
    match host {
        Some(h) => pool.idle.push((h, Instant::now())),
        None => pool.live -= 1,
    }
    cv.notify_one();
}

/// Cierra los procesos que llevan tiempo sin usarse (dejando `KEEP_IDLE`).
fn reap() {
    let (lock, _) = &*POOL;
    let expired: Vec<Host> = {
        let mut pool = lock.lock().unwrap_or_else(|e| e.into_inner());
        // Los más recientes quedan al final (pop los reutiliza primero).
        let keep_from = pool.idle.len().saturating_sub(KEEP_IDLE);
        let mut out = Vec::new();
        let mut i = 0;
        while i < keep_from.min(pool.idle.len()) {
            if pool.idle[i].1.elapsed() >= IDLE_TTL {
                out.push(pool.idle.remove(i).0);
                pool.live -= 1;
            } else {
                i += 1;
            }
        }
        out
    };
    for h in expired {
        h.kill();
    }
}

/// Ejecuta un script de consulta en un proceso persistente.
pub fn query(script: &str, timeout: Option<Duration>, detail: &str) -> Result<String, String> {
    let start = Instant::now();
    let mut host = acquire()?;
    match host.run(script, timeout) {
        Ok(result) => {
            release(Some(host));
            let ms = start.elapsed().as_millis();
            match &result {
                Ok(_) => log::debug!("PowerShell (pool) ok en {ms} ms: {detail}"),
                Err(e) => log::warn!("PowerShell (pool) falló en {ms} ms: {detail} → {e}"),
            }
            result.map_err(|e| crate::ps::clean_error(&e))
        }
        Err(why) => {
            host.kill();
            release(None);
            if why == "timeout" {
                let secs = timeout.map_or(0, |t| t.as_secs());
                log::error!("PowerShell (pool) superó {secs} s y se detuvo: {detail}");
                Err(format!("PowerShell tardó más de {secs} s y se detuvo."))
            } else {
                log::warn!("PowerShell (pool) se perdió ({why}): {detail}");
                Err(format!("PowerShell se cerró inesperadamente ({why})."))
            }
        }
    }
}

/// Arranca un proceso en segundo plano para que la primera consulta no espere.
pub fn warm_up() {
    std::thread::spawn(|| {
        let _ = query("1", Some(Duration::from_secs(30)), "calentamiento");
        loop {
            std::thread::sleep(Duration::from_secs(60));
            reap();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_scripts_and_isolates_variables() {
        assert_eq!(query("$x = 41; $x + 1", None, "t").unwrap(), "42");
        // La variable de la consulta anterior no existe en la siguiente.
        assert_eq!(query("\"[$x]\"", None, "t").unwrap(), "[]");
    }

    #[test]
    fn reports_errors_and_keeps_working() {
        let e = query("throw 'fallo controlado'", None, "t").unwrap_err();
        assert!(e.contains("fallo controlado"), "{e}");
        assert_eq!(query("'sigue vivo'", None, "t").unwrap(), "sigue vivo");
    }

    #[test]
    fn non_string_output_and_unicode() {
        assert_eq!(query("1 -eq 1", None, "t").unwrap(), "True");
        assert_eq!(query("'ñandú ✓'", None, "t").unwrap(), "ñandú ✓");
        assert_eq!(query("Write-Host 'ruido'; 'dato'", None, "t").unwrap(), "dato");
    }

    #[test]
    fn timeout_replaces_the_host() {
        let e = query("Start-Sleep -Seconds 20", Some(Duration::from_secs(1)), "t").unwrap_err();
        assert!(e.contains("más de 1 s"), "{e}");
        assert_eq!(query("'nuevo proceso'", None, "t").unwrap(), "nuevo proceso");
    }
}
