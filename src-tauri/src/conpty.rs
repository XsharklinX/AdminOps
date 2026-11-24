//! Ejecutar un programa dentro de una consola virtual (ConPTY de Windows 10 1809+).
//!
//! winget solo dibuja su barra de progreso («██▒▒ 64 %») cuando cree que habla con una
//! consola; con una tubería normal calla durante toda la descarga. Dentro de una consola
//! virtual la dibuja igual que en una ventana, y aquí se lee línea a línea ya sin los
//! códigos de color y de cursor que lleva la salida de una consola.
//!
//! Si Windows no puede crear la consola virtual, el error empieza por `PTY:` y quien llama
//! se queda con la tubería de siempre (sin porcentajes, pero con las fases).

use crate::ps::{LineSplitter, Opts, Streamed};

/// Quita de la salida de una consola los códigos de escape (colores, cursor, título de la
/// ventana…) y deja el texto. Guarda su estado entre lecturas: un código puede llegar partido.
#[derive(Default)]
pub struct VtStripper {
    state: Vt,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Vt {
    #[default]
    Text,
    Esc,
    Csi,
    /// Cadena (título, DCS…) hasta BEL o ESC \
    Str,
    StrEsc,
    /// Un carácter más que no es texto (`ESC ( B`)
    Skip1,
}

impl VtStripper {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(chunk.len());
        for &b in chunk {
            self.state = match (self.state, b) {
                (Vt::Text, 0x1B) => Vt::Esc,
                (Vt::Text, 0x07 | 0x00) => Vt::Text,
                (Vt::Text, _) => {
                    out.push(b);
                    Vt::Text
                }
                (Vt::Esc, b'[') => Vt::Csi,
                (Vt::Esc, b']' | b'P' | b'_' | b'^' | b'X') => Vt::Str,
                (Vt::Esc, b'(' | b')' | b'*' | b'+' | b'#' | b'%') => Vt::Skip1,
                (Vt::Esc, _) => Vt::Text,
                (Vt::Csi, 0x40..=0x7E) => Vt::Text,
                (Vt::Csi, _) => Vt::Csi,
                (Vt::Str, 0x07) => Vt::Text,
                (Vt::Str, 0x1B) => Vt::StrEsc,
                (Vt::Str, _) => Vt::Str,
                (Vt::StrEsc, b'\\') => Vt::Text,
                (Vt::StrEsc, _) => Vt::Str,
                (Vt::Skip1, _) => Vt::Text,
            };
        }
        out
    }
}

/// Ejecuta `command_line` (programa y argumentos, ya entrecomillados) dentro de una consola
/// virtual y entrega su salida línea a línea. Mismo límite de tiempo y cancelación que `ps::stream`.
#[cfg(windows)]
pub fn run(command_line: &str, opts: Opts, on_line: &mut dyn FnMut(&str)) -> Result<Streamed, String> {
    use std::ffi::c_void;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::Storage::FileSystem::ReadFile;
    use windows_sys::Win32::System::Console::{ClosePseudoConsole, CreatePseudoConsole, COORD, HPCON};
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, InitializeProcThreadAttributeList, UpdateProcThreadAttribute, WaitForSingleObject, EXTENDED_STARTUPINFO_PRESENT,
        PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };

    if let Some(t) = opts.task {
        if crate::ps::is_cancelled(t) {
            return Err(crate::ps::CANCELLED_MSG.into());
        }
    }
    // Los HANDLE son punteros: para pasarlos a otro hilo se guardan como número.
    struct Raw(usize);

    // SAFETY: llamadas de la API de consola virtual de Windows con estructuras inicializadas a
    // cero o por la propia API; cada recurso creado se libera más abajo, también si algo falla.
    unsafe {
        let (mut in_read, mut in_write, mut out_read, mut out_write): (HANDLE, HANDLE, HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        if CreatePipe(&mut in_read, &mut in_write, std::ptr::null(), 0) == 0 {
            return Err("PTY: no se pudo crear la tubería de entrada".into());
        }
        if CreatePipe(&mut out_read, &mut out_write, std::ptr::null(), 0) == 0 {
            CloseHandle(in_read);
            CloseHandle(in_write);
            return Err("PTY: no se pudo crear la tubería de salida".into());
        }
        let mut hpc: HPCON = std::mem::zeroed();
        // Ancha, para que la consola no parta las líneas largas (las direcciones de descarga).
        let hr = CreatePseudoConsole(COORD { X: 200, Y: 50 }, in_read, out_write, 0, &mut hpc);
        // La consola ya tiene su lado de las tuberías: aquí se cierran los que no se usan.
        CloseHandle(in_read);
        CloseHandle(out_write);
        if hr < 0 {
            CloseHandle(in_write);
            CloseHandle(out_read);
            return Err(format!("PTY: Windows no pudo crear la consola virtual (0x{:08X})", hr as u32));
        }

        let mut size = 0usize;
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        // En múltiplos de 8 bytes: la lista de atributos tiene que ir alineada.
        let mut attr_buf = vec![0u64; size.div_ceil(8).max(1)];
        let attrs = attr_buf.as_mut_ptr() as *mut c_void;
        let cleanup = |hpc: HPCON, in_write: HANDLE, out_read: HANDLE| {
            ClosePseudoConsole(hpc);
            CloseHandle(in_write);
            CloseHandle(out_read);
        };
        if InitializeProcThreadAttributeList(attrs, 1, 0, &mut size) == 0 || UpdateProcThreadAttribute(attrs, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize, hpc as *const c_void, std::mem::size_of::<HPCON>(), std::ptr::null_mut(), std::ptr::null()) == 0 {
            cleanup(hpc, in_write, out_read);
            return Err("PTY: no se pudo preparar el proceso".into());
        }

        let mut si: STARTUPINFOEXW = std::mem::zeroed();
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        // Con este indicador y los tres manejadores a cero, el proceso usa la consola virtual como
        // entrada y salida; sin él hereda la salida de quien lo lanza y no se ve nada aquí.
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.lpAttributeList = attrs;
        let mut wide: Vec<u16> = command_line.encode_utf16().chain(std::iter::once(0)).collect();
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let created = CreateProcessW(std::ptr::null(), wide.as_mut_ptr(), std::ptr::null(), std::ptr::null(), 0, EXTENDED_STARTUPINFO_PRESENT, std::ptr::null(), std::ptr::null(), &si.StartupInfo, &mut pi);
        DeleteProcThreadAttributeList(attrs);
        if created == 0 {
            let e = std::io::Error::last_os_error();
            cleanup(hpc, in_write, out_read);
            return Err(format!("No se pudo iniciar el proceso: {e}"));
        }
        let pid = pi.dwProcessId;
        if let Some(t) = opts.task {
            crate::ps::track(t, pid);
        }

        // Lectura en un hilo: la consola no cierra su salida hasta que se cierra ella misma.
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let reader = Raw(out_read as usize);
        let reader_thread = std::thread::spawn(move || {
            let mut strip = VtStripper::default();
            let mut split = LineSplitter::default();
            let mut buf = [0u8; 4096];
            loop {
                let mut n = 0u32;
                let ok = ReadFile(reader.0 as HANDLE, buf.as_mut_ptr(), buf.len() as u32, &mut n, std::ptr::null_mut());
                if ok == 0 || n == 0 {
                    break;
                }
                for l in split.push(&strip.push(&buf[..n as usize])) {
                    if tx.send(l).is_err() {
                        return;
                    }
                }
            }
            for l in split.finish() {
                let _ = tx.send(l);
            }
        });

        let start = Instant::now();
        let mut timed_out = false;
        loop {
            while let Ok(line) = rx.try_recv() {
                on_line(&line);
            }
            if WaitForSingleObject(pi.hProcess, 100) == WAIT_OBJECT_0 {
                break;
            }
            if opts.timeout.is_some_and(|t| start.elapsed() > t) {
                timed_out = true;
                crate::ps::kill_tree(pid);
                break;
            }
        }
        // El proceso terminó. La consola pinta su último cuadro un instante después: se espera antes
        // de cerrarla, y cerrarla es lo que hace que la lectura acabe.
        WaitForSingleObject(pi.hProcess, 5000);
        std::thread::sleep(std::time::Duration::from_millis(300));
        ClosePseudoConsole(hpc);
        let _ = reader_thread.join();
        while let Ok(line) = rx.try_recv() {
            on_line(&line);
        }
        let mut code = 0u32;
        let got = GetExitCodeProcess(pi.hProcess, &mut code);
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
        CloseHandle(in_write);
        CloseHandle(out_read);
        if let Some(t) = opts.task {
            crate::ps::untrack(t, pid);
        }
        if opts.task.is_some_and(crate::ps::is_cancelled) {
            return Err(crate::ps::CANCELLED_MSG.into());
        }
        if timed_out {
            return Err(format!("El proceso tardó más de {} s y se detuvo.", opts.timeout.map_or(0, |t| t.as_secs())));
        }
        Ok(Streamed { code: (got != 0).then_some(code as i32), stderr: String::new() })
    }
}

#[cfg(not(windows))]
pub fn run(_command_line: &str, _opts: Opts, _on_line: &mut dyn FnMut(&str)) -> Result<Streamed, String> {
    Err("PTY: solo en Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(chunks: &[&[u8]]) -> String {
        let mut s = VtStripper::default();
        let mut out = Vec::new();
        for c in chunks {
            out.extend(s.push(c));
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn removes_colours_cursor_and_title() {
        let raw = b"\x1b[?9001h\x1b[?1004h\x1b[H\x1b]0;C:\\WINDOWS\\winget.exe\x07\x1b[2J\x1b[?25l  \x1b[38;5;11m\xE2\x96\x88\xE2\x96\x88\x1b[0m  48%\x1b[K\r\n";
        assert_eq!(strip(&[raw]), "  ██  48%\r\n");
    }

    #[test]
    fn an_escape_code_split_between_reads_is_still_removed() {
        assert_eq!(strip(&[b"uno\x1b", b"[3", b"8;5;11mdos\x1b]0;tit", b"ulo\x1b", b"\\tres"]), "unodostres");
        assert_eq!(strip(&[b"a\x1b(Bb"]), "ab");
    }

    #[test]
    fn progress_survives_stripping_and_splitting() {
        let mut s = VtStripper::default();
        let mut split = LineSplitter::default();
        let raw = b"\x1b[1;1H  \x1b[32m\xE2\x96\x88\xE2\x96\x88\x1b[0m\xE2\x96\x92\xE2\x96\x92  25%\r  \x1b[32m\xE2\x96\x88\xE2\x96\x88\xE2\x96\x88\x1b[0m\xE2\x96\x92  50%\r\n";
        let lines = split.push(&s.push(raw));
        assert_eq!(lines.len(), 2);
        assert_eq!(crate::wingetout::parse_line(&lines[0]), crate::wingetout::Line::Progress { percent: 25, done: None, total: None });
        assert_eq!(crate::wingetout::parse_line(&lines[1]), crate::wingetout::Line::Progress { percent: 50, done: None, total: None });
    }

    /// Una consola virtual de verdad: `cargo test pty_runs_a_program -- --nocapture`
    #[cfg(windows)]
    #[test]
    fn pty_runs_a_program() {
        let mut lines = Vec::new();
        let r = run("cmd.exe /C \"echo uno& ping -n 2 127.0.0.1 >nul& echo dos& exit 3\"", Opts { timeout: Some(std::time::Duration::from_secs(20)), task: None }, &mut |l| lines.push(l.to_string()));
        match r {
            Ok(s) => {
                println!("{lines:?} código {:?}", s.code);
                assert!(lines.iter().any(|l| l == "uno"), "{lines:?}");
                assert!(lines.iter().any(|l| l == "dos"), "{lines:?}");
                assert_eq!(s.code, Some(3));
            }
            // Un Windows sin consola virtual no es un fallo del código.
            Err(e) if e.starts_with("PTY:") => println!("sin consola virtual: {e}"),
            Err(e) => panic!("{e}"),
        }
    }
}
