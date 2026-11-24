//! Lo que la grabación pregunta a Windows sobre monitores y ventanas: qué ventanas se pueden grabar, dónde
//! está cada una y si sigue abierta, y el nombre del programa de cada ventana.
//!
//! Del título de una ventana solo se lee el que se enseña en el selector, en vivo: nunca se guarda (puede
//! llevar nombres de clientes o de documentos). Lo que sí se guarda es el nombre del programa.

use super::input::Area;
use super::RecWindow;

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
mod stub {
    use super::*;

    pub fn list_windows() -> Vec<RecWindow> {
        Vec::new()
    }
    pub fn is_window(_: isize) -> bool {
        false
    }
    pub fn restore_window(_: isize) -> bool {
        false
    }
    pub fn window_frame(_: isize) -> Option<Area> {
        None
    }
    pub fn window_monitor_rect(_: isize) -> Option<Area> {
        None
    }
    pub fn monitor_rect(_: &str) -> Option<Area> {
        None
    }
    pub fn app_name(_: isize) -> String {
        String::new()
    }
}
#[cfg(not(windows))]
pub use stub::*;

#[cfg(windows)]
mod imp {
    use super::*;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY};
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowExW, GetClassNameW, GetWindowLongPtrW, GetWindowPlacement, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, ShowWindow,
        GWL_EXSTYLE, SW_RESTORE, WINDOWPLACEMENT, WS_EX_TOOLWINDOW,
    };

    /// Lo mínimo que mide una ventana para salir en la lista.
    const MIN_W: i32 = 120;
    const MIN_H: i32 = 80;

    fn hwnd(h: isize) -> HWND {
        HWND(h as *mut c_void)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn area_of(r: RECT) -> Area {
        Area { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top }
    }

    // ---------- Monitores ----------

    /// El monitor por su nombre GDI (`\\.\DISPLAY1`); vacío = el principal.
    pub fn monitor_handle(gdi: &str) -> Result<HMONITOR, String> {
        struct Find {
            gdi: String,
            found: Option<HMONITOR>,
        }
        unsafe extern "system" fn each(hmon: HMONITOR, _: HDC, _: *mut RECT, lparam: LPARAM) -> windows::core::BOOL {
            // SAFETY: lparam es el puntero al `Find` de la llamada, vivo durante EnumDisplayMonitors.
            unsafe {
                let find = &mut *(lparam.0 as *mut Find);
                let mut info: MONITORINFOEXW = std::mem::zeroed();
                info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
                if GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
                    let end = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
                    if String::from_utf16_lossy(&info.szDevice[..end]).eq_ignore_ascii_case(&find.gdi) {
                        find.found = Some(hmon);
                        return false.into();
                    }
                }
            }
            true.into()
        }
        if gdi.is_empty() {
            // SAFETY: llamada sin punteros de entrada.
            let m = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
            return if m.is_invalid() { Err("Windows no dio el monitor principal.".into()) } else { Ok(m) };
        }
        let mut find = Find { gdi: gdi.to_string(), found: None };
        // SAFETY: el puntero apunta a `find`, que sigue vivo durante la llamada.
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut find as *mut Find as isize));
        }
        find.found.ok_or_else(|| "Ese monitor ya no está conectado.".to_string())
    }

    fn rect_of_monitor(m: HMONITOR) -> Option<Area> {
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        // SAFETY: `info` es un MONITORINFO con su tamaño puesto.
        unsafe { GetMonitorInfoW(m, &mut info) }.as_bool().then(|| area_of(info.rcMonitor))
    }

    /// Dónde está el monitor ahora (en el escritorio virtual, píxeles físicos).
    pub fn monitor_rect(gdi: &str) -> Option<Area> {
        rect_of_monitor(monitor_handle(gdi).ok()?)
    }

    // ---------- Ventanas ----------

    pub fn is_window(h: isize) -> bool {
        // SAFETY: IsWindow acepta cualquier valor.
        unsafe { IsWindow(Some(hwnd(h))) }.as_bool()
    }

    /// Si la ventana está minimizada, la deja a la vista y al frente. Devuelve si hizo algo.
    pub fn restore_window(h: isize) -> bool {
        // SAFETY: llamadas con un identificador de ventana; si ya no existe, fallan sin más.
        unsafe {
            if !IsIconic(hwnd(h)).as_bool() {
                return false;
            }
            let _ = ShowWindow(hwnd(h), SW_RESTORE);
            let _ = SetForegroundWindow(hwnd(h));
        }
        true
    }

    /// El rectángulo que se ve de la ventana (sin la sombra), en píxeles físicos.
    pub fn window_frame(h: isize) -> Option<Area> {
        let mut r = RECT::default();
        // SAFETY: `r` mide lo que se le dice.
        let ok = unsafe { DwmGetWindowAttribute(hwnd(h), DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut c_void, std::mem::size_of::<RECT>() as u32) }.is_ok();
        if ok {
            return Some(area_of(r));
        }
        // SAFETY: igual.
        unsafe { GetWindowRect(hwnd(h), &mut r) }.is_ok().then(|| area_of(r))
    }

    /// El monitor donde está (casi toda) la ventana.
    pub fn window_monitor_rect(h: isize) -> Option<Area> {
        // SAFETY: MonitorFromWindow acepta cualquier valor.
        let m = unsafe { MonitorFromWindow(hwnd(h), MONITOR_DEFAULTTONEAREST) };
        if m.is_invalid() {
            return None;
        }
        rect_of_monitor(m)
    }

    fn text_of(f: impl Fn(&mut [u16]) -> i32) -> String {
        let mut buf = [0u16; 512];
        let n = f(&mut buf).clamp(0, buf.len() as i32) as usize;
        String::from_utf16_lossy(&buf[..n])
    }

    fn class_of(h: HWND) -> String {
        // SAFETY: el búfer mide lo que se le dice.
        text_of(|b| unsafe { GetClassNameW(h, b) })
    }

    fn pid_of(h: HWND) -> u32 {
        let mut pid = 0u32;
        // SAFETY: `pid` vive durante la llamada.
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        pid
    }

    /// La ruta del programa que tiene ese proceso, si Windows la da (sin permisos especiales).
    fn exe_path(pid: u32) -> Option<String> {
        // SAFETY: el identificador se cierra siempre; el búfer mide lo que se dice.
        unsafe {
            let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let r = QueryFullProcessImageNameW(p, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(p);
            r.ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    /// El texto «Descripción del archivo» del programa (lo que se ve en el Administrador de tareas: «Microsoft Excel»).
    fn file_description(path: &str) -> Option<String> {
        let w: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: los búferes se reservan con el tamaño que dice Windows y viven durante las llamadas.
        unsafe {
            let size = GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            GetFileVersionInfoW(PCWSTR(w.as_ptr()), None, size, data.as_mut_ptr() as *mut c_void).ok()?;
            // Idioma y página de códigos del primer bloque; si no hay, inglés de EE. UU.
            let mut lang = String::from("040904B0");
            let (mut p, mut n) = (std::ptr::null_mut::<c_void>(), 0u32);
            if VerQueryValueW(data.as_ptr() as *const c_void, PCWSTR(wide("\\VarFileInfo\\Translation").as_ptr()), &mut p, &mut n).as_bool() && n >= 4 && !p.is_null() {
                let t = std::slice::from_raw_parts(p as *const u16, 2);
                lang = format!("{:04X}{:04X}", t[0], t[1]);
            }
            for l in [lang.as_str(), "040904B0", "0C0A04B0", "0000040A"] {
                let q = wide(&format!("\\StringFileInfo\\{l}\\FileDescription"));
                let (mut p, mut n) = (std::ptr::null_mut::<c_void>(), 0u32);
                if VerQueryValueW(data.as_ptr() as *const c_void, PCWSTR(q.as_ptr()), &mut p, &mut n).as_bool() && n > 1 && !p.is_null() {
                    let s = String::from_utf16_lossy(std::slice::from_raw_parts(p as *const u16, n as usize));
                    let s = s.trim_matches(['\0', ' ']).to_string();
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
            None
        }
    }

    fn exe_stem(path: &str) -> String {
        std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string()
    }

    /// El proceso «de verdad» de una ventana: las de la Tienda cuelgan de `ApplicationFrameHost`, y el programa
    /// está en la ventana hija `Windows.UI.Core.CoreWindow`.
    fn real_pid(h: HWND) -> u32 {
        let pid = pid_of(h);
        if class_of(h) == "ApplicationFrameWindow" {
            let class = wide("Windows.UI.Core.CoreWindow");
            // SAFETY: cadena terminada en cero que vive durante la llamada.
            if let Ok(child) = unsafe { FindWindowExW(Some(h), None, PCWSTR(class.as_ptr()), PCWSTR::null()) } {
                let c = pid_of(child);
                if c != 0 && c != pid {
                    return c;
                }
            }
        }
        pid
    }

    /// El nombre del programa de una ventana: su descripción de archivo o, si no se puede leer, el .exe sin extensión.
    fn app_of(h: HWND, cache: &mut HashMap<String, String>) -> String {
        let Some(path) = exe_path(real_pid(h)) else { return "Programa".to_string() };
        cache.entry(path.clone()).or_insert_with(|| file_description(&path).unwrap_or_else(|| exe_stem(&path))).clone()
    }

    pub fn app_name(h: isize) -> String {
        app_of(hwnd(h), &mut HashMap::new())
    }

    /// Las ventanas que se pueden grabar, la de arriba primero (orden Z).
    pub fn list_windows() -> Vec<RecWindow> {
        struct Ctx {
            found: Vec<HWND>,
        }
        unsafe extern "system" fn each(h: HWND, lparam: LPARAM) -> windows::core::BOOL {
            // SAFETY: lparam es el puntero al `Ctx` de la llamada, vivo durante EnumWindows.
            unsafe { (*(lparam.0 as *mut Ctx)).found.push(h) };
            true.into()
        }
        let mut ctx = Ctx { found: Vec::new() };
        // SAFETY: el puntero apunta a `ctx`, que sigue vivo durante la llamada.
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut ctx as *mut Ctx as isize));
        }
        let own = std::process::id();
        let mut cache: HashMap<String, String> = HashMap::new();
        let mut out = Vec::new();
        for h in ctx.found {
            // SAFETY: llamadas con identificadores de ventana; si una desaparece a mitad, fallan sin más.
            unsafe {
                if !IsWindowVisible(h).as_bool() || GetWindowLongPtrW(h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
                    continue;
                }
                let title = text_of(|b| GetWindowTextW(h, b));
                if title.trim().is_empty() {
                    continue;
                }
                // Las ventanas ocultas por Windows (otro escritorio virtual, aplicaciones de la Tienda suspendidas).
                let mut cloaked = 0u32;
                if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4).is_ok() && cloaked != 0 {
                    continue;
                }
                if pid_of(h) == own || class_of(h) == "Progman" {
                    continue;
                }
                let minimized = IsIconic(h).as_bool();
                let (w, hh) = if minimized {
                    let mut pl = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
                    if GetWindowPlacement(h, &mut pl).is_err() {
                        continue;
                    }
                    (pl.rcNormalPosition.right - pl.rcNormalPosition.left, pl.rcNormalPosition.bottom - pl.rcNormalPosition.top)
                } else {
                    let mut r = RECT::default();
                    if GetWindowRect(h, &mut r).is_err() {
                        continue;
                    }
                    (r.right - r.left, r.bottom - r.top)
                };
                if w < MIN_W || hh < MIN_H {
                    continue;
                }
                out.push(RecWindow { id: h.0 as usize as u64, title, app: app_of(h, &mut cache), minimized });
            }
        }
        out
    }
}
