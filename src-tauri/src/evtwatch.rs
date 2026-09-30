//! Suscripción al Visor de eventos en tiempo real.
//!
//! Antes la vigilancia de errores preguntaba a Windows cada minuto con
//! PowerShell, hubiera algo nuevo o no. Ahora Windows avisa en cuanto se
//! escribe uno de los eventos vigilados (`EvtSubscribe`), y solo entonces se
//! hace la pasada. Menos CPU con el equipo tranquilo y el aviso llega al
//! momento. La pasada periódica sigue, mucho más espaciada, para el espacio
//! libre y los dispositivos (que no dejan evento) y por si la suscripción cae.

use std::sync::mpsc::{channel, Receiver, Sender};

/// XPath de un canal: un `Select` por proveedor con sus ids.
pub fn query_xml(log: &str, sources: &[(&str, &str, &[u32])]) -> Option<String> {
    let esc = |s: &str| s.replace('&', "&amp;").replace('\'', "&apos;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;");
    let selects: Vec<String> = sources
        .iter()
        .filter(|(l, _, ids)| *l == log && !ids.is_empty())
        .map(|(_, prov, ids)| {
            let ids = ids.iter().map(|i| format!("EventID={i}")).collect::<Vec<_>>().join(" or ");
            format!("<Select Path=\"{}\">*[System[Provider[@Name='{}'] and ({ids})]]</Select>", esc(log), esc(prov))
        })
        .collect();
    (!selects.is_empty()).then(|| format!("<QueryList><Query Id=\"0\" Path=\"{}\">{}</Query></QueryList>", esc(log), selects.concat()))
}

/// Se suscribe a los canales de `sources`. Cada vez que llega un evento
/// vigilado, manda un aviso por el canal devuelto. None si Windows no dejó
/// suscribirse a ninguno (entonces se sigue preguntando cada minuto).
#[cfg(windows)]
pub fn subscribe(sources: &[(&str, &str, &[u32])]) -> Option<Receiver<()>> {
    let (tx, rx) = channel();
    let mut logs: Vec<&str> = sources.iter().map(|(l, _, _)| *l).collect();
    logs.sort_unstable();
    logs.dedup();
    let mut any = false;
    for log in logs {
        let Some(xml) = query_xml(log, sources) else { continue };
        if watch(log.to_string(), xml, tx.clone()) {
            any = true;
        }
    }
    any.then_some(rx)
}

#[cfg(not(windows))]
pub fn subscribe(_sources: &[(&str, &str, &[u32])]) -> Option<Receiver<()>> {
    None
}

/// Un hilo por canal, dormido hasta que Windows señala. Devuelve si se pudo suscribir.
#[cfg(windows)]
fn watch(log: String, xml: String, tx: Sender<()>) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::EventLog::{EvtClose, EvtNext, EvtSubscribe, EvtSubscribeToFutureEvents};
    use windows_sys::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject, INFINITE};

    let (ok_tx, ok_rx) = channel::<bool>();
    std::thread::spawn(move || {
        let wide: Vec<u16> = xml.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: todas las llamadas usan punteros y handles válidos creados aquí,
        // y cada handle se cierra una sola vez al salir del hilo.
        unsafe {
            let signal = CreateEventW(std::ptr::null(), 1, 0, std::ptr::null());
            if signal.is_null() {
                let _ = ok_tx.send(false);
                return;
            }
            let sub = EvtSubscribe(0, signal, std::ptr::null(), wide.as_ptr(), 0, std::ptr::null(), None, EvtSubscribeToFutureEvents);
            if sub == 0 {
                log::debug!("Visor de eventos: sin suscripción a {log} (error {})", GetLastError());
                CloseHandle(signal);
                let _ = ok_tx.send(false);
                return;
            }
            let _ = ok_tx.send(true);
            let mut events = [0isize; 16];
            loop {
                WaitForSingleObject(signal, INFINITE);
                // Solo importa que ha llegado algo: la pasada de siempre lo lee y lo explica.
                loop {
                    let mut n = 0u32;
                    if EvtNext(sub, events.len() as u32, events.as_mut_ptr(), 0, 0, &mut n) == 0 || n == 0 {
                        break;
                    }
                    for h in &events[..n as usize] {
                        EvtClose(*h);
                    }
                }
                ResetEvent(signal);
                if tx.send(()).is_err() {
                    break;
                }
            }
            EvtClose(sub);
            CloseHandle(signal);
        }
    });
    ok_rx.recv().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: &[(&str, &str, &[u32])] = &[("System", "disk", &[7, 51]), ("System", "Service Control Manager", &[7031]), ("Application", "Application Error", &[1000])];

    #[test]
    fn builds_one_query_per_channel() {
        let q = query_xml("System", S).unwrap();
        assert!(q.starts_with("<QueryList><Query Id=\"0\" Path=\"System\">"));
        assert!(q.contains("Provider[@Name='disk'] and (EventID=7 or EventID=51)"));
        assert!(q.contains("Provider[@Name='Service Control Manager'] and (EventID=7031)"));
        assert!(!q.contains("Application Error"), "cada canal lo suyo");
        assert!(query_xml("Security", S).is_none());
        // Un nombre con comilla no rompe el XPath.
        assert!(query_xml("System", &[("System", "O'Brien", &[1])]).unwrap().contains("O&apos;Brien"));
    }

    /// Solo lectura: se suscribe de verdad al registro del sistema.
    #[test]
    #[ignore = "usa el Visor de eventos real"]
    fn subscribes_for_real() {
        assert!(subscribe(S).is_some());
    }
}
