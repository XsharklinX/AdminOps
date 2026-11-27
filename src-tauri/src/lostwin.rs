//! Ventanas perdidas fuera de la pantalla: un monitor que se desconectó, una resolución que cambió o un
//! portátil que vuelve de la mesa de reuniones dejan ventanas «vivas» en coordenadas que ya no existen.
//! Se listan las que no se ven y se traen al centro del monitor principal.

use serde::Serialize;

/// Lo que tiene que verse de una ventana (en ancho y en alto) para poder agarrarla con el ratón.
const MIN_VISIBLE: i32 = 40;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LostWin {
    pub hwnd: isize,
    pub title: String,
    pub app: String,
}

/// Rectángulo en píxeles del escritorio virtual (izquierda, arriba, derecha, abajo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rc {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rc {
    pub fn w(&self) -> i32 {
        self.right - self.left
    }
    pub fn h(&self) -> i32 {
        self.bottom - self.top
    }
}

/// ¿La ventana está fuera? Cuando en ningún monitor se ven al menos 40 px de ancho y de alto
/// (o no corta ninguno). Sin monitores conocidos no se puede decir: no está perdida.
pub fn is_lost(win: &Rc, monitors: &[Rc]) -> bool {
    if monitors.is_empty() {
        return false;
    }
    !monitors.iter().any(|m| {
        let w = win.right.min(m.right) - win.left.max(m.left);
        let h = win.bottom.min(m.bottom) - win.top.max(m.top);
        w >= MIN_VISIBLE && h >= MIN_VISIBLE
    })
}

/// Dónde poner una ventana de `w`×`h` en el área de trabajo `area`: centrada y, si no cabe,
/// reducida al 90 % del área. Devuelve (x, y, ancho, alto).
pub fn centered(w: i32, h: i32, area: &Rc) -> (i32, i32, i32, i32) {
    let (aw, ah) = (area.w(), area.h());
    let (w, h) = (w.max(1), h.max(1));
    let (nw, nh) = if w > aw || h > ah { (w.min(aw * 9 / 10), h.min(ah * 9 / 10)) } else { (w, h) };
    (area.left + (aw - nw) / 2, area.top + (ah - nh) / 2, nw, nh)
}

#[cfg(windows)]
mod win {
    use super::{centered, is_lost, LostWin, Rc};
    use std::ffi::c_void;
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow,
        SetWindowPos, ShowWindow, GWL_EXSTYLE, GW_OWNER, SWP_NOACTIVATE, SWP_NOZORDER, SW_RESTORE, WS_EX_TOOLWINDOW,
    };

    fn hwnd(h: isize) -> HWND {
        HWND(h as *mut c_void)
    }

    fn rc(r: RECT) -> Rc {
        Rc { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }

    fn text_of(f: impl Fn(&mut [u16]) -> i32) -> String {
        let mut buf = [0u16; 512];
        let n = f(&mut buf).clamp(0, buf.len() as i32) as usize;
        String::from_utf16_lossy(&buf[..n])
    }

    /// Todos los monitores conectados.
    fn monitors() -> Vec<Rc> {
        unsafe extern "system" fn each(hmon: HMONITOR, _: HDC, _: *mut RECT, lparam: LPARAM) -> windows::core::BOOL {
            // SAFETY: lparam es el puntero al Vec de la llamada, vivo durante EnumDisplayMonitors.
            unsafe {
                let v = &mut *(lparam.0 as *mut Vec<Rc>);
                let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                if GetMonitorInfoW(hmon, &mut info).as_bool() {
                    v.push(rc(info.rcMonitor));
                }
            }
            true.into()
        }
        let mut v: Vec<Rc> = Vec::new();
        // SAFETY: el puntero apunta a `v`, que sigue vivo durante la llamada.
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut v as *mut Vec<Rc> as isize));
        }
        v
    }

    /// El área de trabajo (sin la barra de tareas) del monitor principal.
    fn primary_work() -> Option<Rc> {
        // SAFETY: llamada sin punteros de entrada; `info` lleva su tamaño puesto.
        unsafe {
            let m = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            (!m.is_invalid() && GetMonitorInfoW(m, &mut info).as_bool()).then(|| rc(info.rcWork))
        }
    }

    fn all_windows() -> Vec<HWND> {
        unsafe extern "system" fn each(h: HWND, lparam: LPARAM) -> windows::core::BOOL {
            // SAFETY: lparam es el puntero al Vec de la llamada, vivo durante EnumWindows.
            unsafe { (*(lparam.0 as *mut Vec<HWND>)).push(h) };
            true.into()
        }
        let mut v: Vec<HWND> = Vec::new();
        // SAFETY: el puntero apunta a `v`, que sigue vivo durante la llamada.
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut v as *mut Vec<HWND> as isize));
        }
        v
    }

    pub fn lost() -> Vec<LostWin> {
        let mons = monitors();
        if mons.is_empty() {
            return vec![];
        }
        let own = std::process::id();
        let mut sys = sysinfo::System::new();
        let mut loaded = false;
        let mut out = Vec::new();
        for h in all_windows() {
            // SAFETY: llamadas con identificadores de ventana; si una desaparece a mitad, fallan sin más.
            unsafe {
                if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() || GetWindowLongPtrW(h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
                    continue;
                }
                // Con dueño: es un cuadro de diálogo de otra ventana, no una ventana de nivel superior.
                if GetWindow(h, GW_OWNER).is_ok_and(|o| !o.is_invalid()) {
                    continue;
                }
                let title = text_of(|b| GetWindowTextW(h, b));
                if title.trim().is_empty() {
                    continue;
                }
                let class = text_of(|b| GetClassNameW(h, b));
                if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
                    continue;
                }
                // Ocultas por Windows (otro escritorio virtual, apps de la Tienda suspendidas): no están perdidas.
                let mut cloaked = 0u32;
                if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4).is_ok() && cloaked != 0 {
                    continue;
                }
                let mut pid = 0u32;
                GetWindowThreadProcessId(h, Some(&mut pid));
                if pid == own {
                    continue;
                }
                let mut r = RECT::default();
                if GetWindowRect(h, &mut r).is_err() {
                    continue;
                }
                let r = rc(r);
                if r.w() <= 0 || r.h() <= 0 || !is_lost(&r, &mons) {
                    continue;
                }
                if !loaded {
                    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                    loaded = true;
                }
                let app = sys.process(sysinfo::Pid::from_u32(pid)).map(|p| crate::rocket::app_label(&p.name().to_string_lossy())).unwrap_or_else(|| "Programa".into());
                out.push(LostWin { hwnd: h.0 as isize, title, app });
            }
        }
        out
    }

    /// Trae las ventanas (solo las que siguen perdidas) al centro del monitor principal; la última queda delante.
    pub fn bring(hwnds: &[isize]) -> Result<u32, String> {
        let area = primary_work().ok_or("Windows no dio el área del monitor principal.")?;
        let still: Vec<isize> = lost().into_iter().map(|l| l.hwnd).collect();
        let mut n = 0u32;
        let mut last = None;
        for &raw in hwnds {
            if !still.contains(&raw) {
                continue;
            }
            let h = hwnd(raw);
            // SAFETY: llamadas con un identificador de ventana; si ya no existe, fallan sin más.
            unsafe {
                if !IsWindow(Some(h)).as_bool() {
                    continue;
                }
                let mut r = RECT::default();
                if GetWindowRect(h, &mut r).is_err() {
                    continue;
                }
                let (x, y, w, hh) = centered(r.right - r.left, r.bottom - r.top, &area);
                if SetWindowPos(h, None, x, y, w, hh, SWP_NOZORDER | SWP_NOACTIVATE).is_ok() {
                    n += 1;
                    last = Some(h);
                }
            }
        }
        if let Some(h) = last {
            // SAFETY: igual que arriba.
            unsafe {
                if IsIconic(h).as_bool() {
                    let _ = ShowWindow(h, SW_RESTORE);
                }
                let _ = SetForegroundWindow(h);
            }
        }
        Ok(n)
    }
}

#[cfg(windows)]
fn find_lost() -> Vec<LostWin> {
    win::lost()
}

#[cfg(not(windows))]
fn find_lost() -> Vec<LostWin> {
    vec![]
}

/// Las ventanas visibles que quedaron fuera de todos los monitores.
#[tauri::command(async)]
pub fn lost_windows() -> Vec<LostWin> {
    find_lost()
}

/// Trae esas ventanas al centro del monitor principal. Solo mueve las que de verdad siguen perdidas.
#[tauri::command(async)]
pub fn bring_windows(tweaks: tauri::State<'_, crate::tweaks::TweakState>, hwnds: Vec<isize>) -> Result<u32, String> {
    #[cfg(windows)]
    let r = win::bring(&hwnds);
    #[cfg(not(windows))]
    let r: Result<u32, String> = {
        let _ = &hwnds;
        Ok(0)
    };
    if let Ok(n) = &r {
        if *n > 0 {
            let title = if *n == 1 { "Traer 1 ventana perdida a la pantalla".to_string() } else { format!("Traer {n} ventanas perdidas a la pantalla") };
            tweaks.record(crate::tweaks::journal::Op::Run, &title, &Ok::<(), String>(()));
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(l: i32, t: i32, rr: i32, b: i32) -> Rc {
        Rc { left: l, top: t, right: rr, bottom: b }
    }

    #[test]
    fn dentro_y_fuera() {
        let mons = [r(0, 0, 1920, 1080)];
        assert!(!is_lost(&r(100, 100, 900, 700), &mons));
        // Mitad fuera por la derecha: se ve y se puede agarrar.
        assert!(!is_lost(&r(1500, 100, 2300, 700), &mons));
        // Monitor que ya no está (a la derecha) o minimizada a -32000.
        assert!(is_lost(&r(2000, 100, 2800, 700), &mons));
        assert!(is_lost(&r(-32000, -32000, -31800, -31900), &mons));
        // Se ven solo 20 px por el borde: perdida.
        assert!(is_lost(&r(1900, 100, 2700, 700), &mons));
        // Se ve el ancho pero solo 10 px de alto.
        assert!(is_lost(&r(100, 1070, 900, 1500), &mons));
    }

    #[test]
    fn con_dos_monitores() {
        let mons = [r(0, 0, 1920, 1080), r(-1280, 0, 0, 1024)];
        assert!(!is_lost(&r(-1200, 50, -400, 600), &mons));
        assert!(is_lost(&r(-3000, 50, -2200, 600), &mons));
        // Sin monitores conocidos no se afirma nada.
        assert!(!is_lost(&r(5000, 5000, 6000, 6000), &[]));
    }

    #[test]
    fn centrada_y_ajustada() {
        let area = r(0, 0, 1920, 1040);
        assert_eq!(centered(800, 600, &area), (560, 220, 800, 600));
        // No cabe: al 90 % del área.
        assert_eq!(centered(3000, 2000, &area), (96, 52, 1728, 936));
        // Área que no empieza en el origen.
        assert_eq!(centered(100, 100, &r(100, 50, 300, 250)), (150, 100, 100, 100));
    }
}
