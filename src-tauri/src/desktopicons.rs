//! Los iconos del Escritorio vuelven a su sitio al conectar o quitar la base o un monitor.
//!
//! Windows recoloca los iconos cada vez que cambian las pantallas y el usuario los vuelve a ordenar a mano. Aquí se
//! guarda dónde está cada icono para cada combinación de pantallas (la misma «firma» que usa el cohete) y, al volver a
//! esa combinación, se reponen. Se le pregunta al propio Escritorio (el Explorador del usuario) por COM: es lo mismo que
//! arrastrarlos. Con «Organizar iconos automáticamente» Windows decide y esto no hace nada.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Una lectura o reposición en marcha: si el Explorador está colgado, no se acumulan más.
static BUSY: AtomicBool = AtomicBool::new(false);
/// Hasta cuándo no se guarda (segundos Unix): tras un fallo al reponer, lo que hay no es lo bueno.
static NO_SAVE_UNTIL: AtomicU64 = AtomicU64::new(0);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// La clave lleva el equipo: en portable el archivo va en la memoria USB y otro PC con las mismas pantallas no
/// puede recibir los iconos de este.
fn full_key(key: &str) -> String {
    format!("{}|{key}", crate::paths::host())
}

type Layout = HashMap<String, (i32, i32)>;

fn file(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("escritorio-iconos.json")
}

/// Guarda dónde están ahora los iconos, para esta combinación de pantallas.
pub fn save(app: &tauri::AppHandle, key: &str) {
    if key.is_empty() || now() < NO_SAVE_UNTIL.load(Ordering::SeqCst) || BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let (app, key) = (app.clone(), full_key(key));
    // En su propio hilo: una llamada al Explorador colgado no para al vigilante.
    std::thread::spawn(move || {
        save_now(&app, &key);
        BUSY.store(false, Ordering::SeqCst);
    });
}

fn save_now(app: &tauri::AppHandle, key: &str) {
    let Ok(now) = read_positions() else { return };
    if now.is_empty() {
        return;
    }
    let path = file(app);
    let mut all: HashMap<String, Layout> = crate::paths::read_json(&path);
    if all.get(key) == Some(&now) {
        return;
    }
    // Unas pocas combinaciones por equipo (en portable pasan muchos equipos por la misma memoria).
    let host = key.split('|').next().unwrap_or_default().to_string();
    if all.keys().filter(|k| k.starts_with(&format!("{host}|"))).count() > 12 {
        all.retain(|k, _| !k.starts_with(&format!("{host}|")));
    }
    if all.len() > 200 {
        all.clear();
    }
    all.insert(key.to_string(), now);
    let _ = crate::paths::write_json(&path, &all);
}

/// Repone los iconos como estaban con esta combinación de pantallas (si se guardó alguna vez). Devuelve cuántos movió.
pub fn restore(app: &tauri::AppHandle, key: &str) -> usize {
    let all: HashMap<String, Layout> = crate::paths::read_json(&file(app));
    let Some(saved) = all.get(&full_key(key)) else { return 0 };
    if BUSY.swap(true, Ordering::SeqCst) {
        // Ocupado guardando: lo de ahora es lo que dejó Windows; que no se guarde encima.
        NO_SAVE_UNTIL.store(now() + 600, Ordering::SeqCst);
        return 0;
    }
    let r = place(saved);
    BUSY.store(false, Ordering::SeqCst);
    match r {
        Ok(n) => n,
        Err(e) => {
            // Lo que hay ahora es lo que dejó Windows: no se guarda encima de lo bueno en un buen rato.
            NO_SAVE_UNTIL.store(now() + 600, Ordering::SeqCst);
            log::info!("Iconos del Escritorio: no se pudieron reponer ({e})");
            0
        }
    }
}

#[cfg(windows)]
fn with_com<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    // SAFETY: inicia y cierra COM en este hilo alrededor de `f`.
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let r = f();
        if init.is_ok() {
            CoUninitialize();
        }
        r
    }
}

/// La vista de carpeta del Escritorio.
#[cfg(windows)]
fn desktop_view() -> Result<windows::Win32::UI::Shell::IFolderView, String> {
    use windows::core::Interface;
    use windows::Win32::System::Com::{CoCreateInstance, IServiceProvider, CLSCTX_LOCAL_SERVER};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Shell::{IFolderView, IShellBrowser, IShellWindows, SID_STopLevelBrowser, ShellWindows, SWC_DESKTOP, SWFO_NEEDDISPATCH};
    let e = |what: &'static str| move |err: windows::core::Error| format!("{what}: {err}");
    // SAFETY: llamadas COM sobre objetos vivos en este hilo.
    unsafe {
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER).map_err(e("ShellWindows"))?;
        let loc = VARIANT::from(0i32);
        let empty = VARIANT::default();
        let mut hwnd = 0i32;
        let disp = windows.FindWindowSW(&loc, &empty, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH).map_err(e("escritorio"))?;
        let provider: IServiceProvider = disp.cast().map_err(e("servicios"))?;
        let browser: IShellBrowser = provider.QueryService(&SID_STopLevelBrowser).map_err(e("navegador del escritorio"))?;
        let view = browser.QueryActiveShellView().map_err(e("vista"))?;
        view.cast::<IFolderView>().map_err(e("vista de carpeta"))
    }
}

/// Recorre los iconos del Escritorio: (nombre, posición, identificador) para cada uno.
#[cfg(windows)]
fn each_item(mut f: impl FnMut(&windows::Win32::UI::Shell::IFolderView, &str, *const windows::Win32::UI::Shell::Common::ITEMIDLIST)) -> Result<(), String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
    use windows::Win32::UI::Shell::{IEnumIDList, IShellFolder, StrRetToStrW, SHGDN_FORPARSING, SVGIO_ALLVIEW};
    let fv = desktop_view()?;
    // SAFETY: cada PIDL que da el enumerador es nuestro hasta liberarlo con CoTaskMemFree; los nombres también.
    unsafe {
        let folder: IShellFolder = fv.GetFolder().map_err(|e| format!("carpeta: {e}"))?;
        let items: IEnumIDList = fv.Items(SVGIO_ALLVIEW).map_err(|e| format!("iconos: {e}"))?;
        loop {
            let mut one = [std::ptr::null_mut::<ITEMIDLIST>()];
            let mut got = 0u32;
            if items.Next(&mut one, Some(&mut got)).is_err() || got == 0 || one[0].is_null() {
                break;
            }
            let pidl = one[0];
            let mut sr = STRRET::default();
            // La ruta completa: un acceso del Escritorio público y otro del usuario con el mismo nombre no se juntan.
            if folder.GetDisplayNameOf(pidl, SHGDN_FORPARSING, &mut sr).is_ok() {
                let mut name = windows::core::PWSTR::null();
                if StrRetToStrW(&mut sr, Some(pidl), &mut name).is_ok() && !name.is_null() {
                    let text = name.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(name.0 as _));
                    if !text.is_empty() {
                        f(&fv, &text, pidl);
                    }
                }
            }
            CoTaskMemFree(Some(pidl as _));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn read_positions() -> Result<Layout, String> {
    with_com(|| {
        let mut out = Layout::new();
        each_item(|fv, name, pidl| {
            // SAFETY: PIDL vivo durante la llamada (lo libera each_item después).
            if let Ok(p) = unsafe { fv.GetItemPosition(pidl) } {
                out.insert(name.to_string(), (p.x, p.y));
            }
        })?;
        Ok(out)
    })
}

#[cfg(windows)]
fn place(saved: &Layout) -> Result<usize, String> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Shell::SVSI_POSITIONITEM;
    with_com(|| {
        let mut moved = 0usize;
        each_item(|fv, name, pidl| {
            let Some(&(x, y)) = saved.get(name) else { return };
            // SAFETY: PIDL y punto vivos durante la llamada.
            let ok = unsafe {
                let now = fv.GetItemPosition(pidl).ok();
                if now.is_some_and(|p| p.x == x && p.y == y) {
                    return;
                }
                fv.SelectAndPositionItems(1, &pidl, Some(&POINT { x, y }), SVSI_POSITIONITEM.0 as u32).is_ok()
            };
            if ok {
                moved += 1;
            }
        })?;
        Ok(moved)
    })
}

#[cfg(not(windows))]
fn read_positions() -> Result<Layout, String> {
    Err("Solo en Windows.".into())
}

#[cfg(not(windows))]
fn place(_: &Layout) -> Result<usize, String> {
    Err("Solo en Windows.".into())
}

#[cfg(test)]
mod tests {
    /// Lee los iconos del Escritorio de verdad (no mueve nada): `cargo test lee_los_iconos -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn lee_los_iconos() {
        let p = super::read_positions().expect("leer el Escritorio");
        println!("{} iconos; p. ej. {:?}", p.len(), p.iter().next());
    }
}
