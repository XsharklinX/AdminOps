//! La barra del cohete en todos los escritorios virtuales (Win+Tab).
//!
//! Windows deja cada ventana en el escritorio donde se creó. Con la API documentada (IVirtualDesktopManager) se mira si
//! la ventana del cohete está en el escritorio que se ve; si no, se lleva al de la ventana que está delante. Solo vale
//! para ventanas de este proceso, que es justo el caso.

/// Si el cohete no está en el escritorio virtual que se ve, lo lleva ahí. Se llama desde el vigilante (cada 2 s).
#[cfg(windows)]
pub fn follow(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{IVirtualDesktopManager, VirtualDesktopManager};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    thread_local! {
        static MANAGER: Option<IVirtualDesktopManager> = {
            // SAFETY: COM se inicia una vez en el hilo del vigilante, que vive lo que el proceso.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL).ok()
            }
        };
    }
    MANAGER.with(|m| {
        let Some(m) = m else { return };
        let me = HWND(hwnd as *mut core::ffi::c_void);
        // SAFETY: ventanas vivas; solo se mueve la propia.
        unsafe {
            if m.IsWindowOnCurrentVirtualDesktop(me).map(|b| b.as_bool()).unwrap_or(true) {
                return;
            }
            let front = GetForegroundWindow();
            if front.is_invalid() || front == me {
                return;
            }
            if let Ok(id) = m.GetWindowDesktopId(front) {
                let _ = m.MoveWindowToDesktop(me, &id);
            }
        }
    });
}

#[cfg(not(windows))]
pub fn follow(_: isize) {}
