//! Instalar en lote con winget, una fila por programa.
//!
//! La cola vive aquí (no en la pantalla): cada programa pasa por «En cola → Preparando →
//! Descargando % → Instalando → Listo / Saltado / Falló» y cada cambio sale como evento
//! `install-item`. winget instala de uno en uno; mientras un programa espera una decisión
//! (cerrarlo, reintentar, saltarlo) la cola sigue con los demás. Al volver a la pantalla,
//! `install_snapshot` devuelve el estado de todas las filas.

use crate::apps::{invalidate_installed, CatalogApp};
use crate::software::valid_id;
use crate::task::Task;
use crate::tweaks::journal::Op;
use crate::tweaks::TweakState;
use crate::wingetout::{self, Line, Phase, Verdict};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub name: String,
    /// queued | starting | downloading | installing | done | skipped | failed
    pub state: &'static str,
    /// Descarga en curso, 0-100.
    pub percent: Option<u8>,
    /// Tamaño de la descarga, si winget lo ha dicho.
    pub size: Option<u64>,
    /// El motivo, en español (de los códigos de winget).
    pub message: String,
    /// `close-app`, `busy`, `network`… (ver `wingetout::Outcome`). Vacío si no hay.
    pub kind: &'static str,
    pub reboot: bool,
    pub code: Option<String>,
    #[serde(skip)]
    source: String,
}

#[derive(Default)]
struct Queue {
    items: Vec<Item>,
    running: bool,
}

static Q: Mutex<Queue> = Mutex::new(Queue { items: Vec::new(), running: false });

/// Un solo winget a la vez en toda la aplicación: la cola, las recetas y «Un día» pasan por aquí.
static WINGET: Mutex<()> = Mutex::new(());

pub fn winget_lock() -> std::sync::MutexGuard<'static, ()> {
    WINGET.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock() -> std::sync::MutexGuard<'static, Queue> {
    Q.lock().unwrap_or_else(|e| e.into_inner())
}

fn finished(state: &str) -> bool {
    matches!(state, "done" | "skipped" | "failed")
}

fn new_item(a: &CatalogApp) -> Item {
    Item {
        id: a.id.clone(),
        name: a.name.clone(),
        state: "queued",
        percent: None,
        size: None,
        message: String::new(),
        kind: "",
        reboot: false,
        code: None,
        source: if a.source == "msstore" { "msstore".into() } else { "winget".into() },
    }
}

/// Cambia una fila y avisa a la pantalla.
fn update(app: &tauri::AppHandle, id: &str, f: impl FnOnce(&mut Item)) {
    let item = {
        let mut q = lock();
        let Some(it) = q.items.iter_mut().find(|i| i.id == id) else { return };
        f(it);
        it.clone()
    };
    let _ = app.emit("install-item", &item);
}

/// Lo que se hace con cada línea de winget: actualizar la fila solo si algo cambia.
fn apply_line(app: &tauri::AppHandle, id: &str, line: &str) {
    match wingetout::parse_line(line) {
        Line::Progress { percent, total, .. } => {
            let changed = lock().items.iter().find(|i| i.id == id).is_some_and(|i| i.state != "downloading" || i.percent != Some(percent) || (total.is_some() && i.size != total));
            if changed {
                update(app, id, |i| {
                    // Una barra a 100 % y luego otra: no se vuelve atrás si ya se está instalando.
                    if i.state != "installing" {
                        i.state = "downloading";
                        i.percent = Some(percent);
                        if total.is_some() {
                            i.size = total;
                        }
                    }
                });
            }
        }
        Line::Phase(Phase::Downloading) => update(app, id, |i| {
            if i.state == "starting" {
                i.state = "downloading";
            }
        }),
        Line::Phase(Phase::Installing) => update(app, id, |i| {
            i.state = "installing";
            i.percent = None;
        }),
        Line::Phase(Phase::Success) | Line::Other => {}
    }
}

/// Últimas líneas útiles de la salida, para el código del instalador y lo desconocido.
fn tail(lines: &[String]) -> String {
    // El mensaje de error llega detrás de los restos del «girando» de winget (`-  \  |  mensaje`).
    let spin = |c: char| "-\\|/ ".contains(c);
    let useful: Vec<&str> = lines
        .iter()
        .map(|l| l.trim().trim_start_matches(spin))
        .filter(|l| l.len() > 3 && !l.chars().all(|c| spin(c) || "█▒░".contains(c)) && !matches!(wingetout::parse_line(l), Line::Progress { .. }))
        .collect();
    useful[useful.len().saturating_sub(4)..].join("\n")
}

/// Las palabras que winget lleva en cada instalación (el id y el origen ya van validados).
fn install_args(id: &str, source: &str) -> [String; 10] {
    ["install", "--id", id, "--exact", "--silent", "--source", source, "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity"].map(String::from)
}

/// Lanza winget y entrega su salida línea a línea. Primero en una consola virtual, que es la
/// única forma de que winget dibuje el porcentaje de la descarga; si Windows no la puede crear,
/// con una tubería normal (se ven las fases pero no el porcentaje).
fn run_winget(task: &Task, args: &[String], on_line: &mut dyn FnMut(&str)) -> Result<crate::ps::Streamed, String> {
    let timeout = Some(Duration::from_secs(30 * 60));
    // Dentro de cmd: winget es un alias de la tienda y solo hereda la consola virtual de quien lo lanza.
    let line = format!("cmd.exe /D /C winget.exe {}", args.join(" "));
    match crate::conpty::run(&line, task.opts(timeout), on_line) {
        Err(e) if e.starts_with("PTY:") => {
            log::info!("[install-apps] sin consola virtual ({e}): se usa una tubería");
            let mut cmd = crate::ps::hidden("winget");
            cmd.args(args);
            crate::ps::stream(cmd, None, task.opts(timeout), on_line)
        }
        other => other,
    }
}

fn install_one(app: &tauri::AppHandle, task: &Task, id: &str, source: &str) -> (Option<i64>, String, Option<String>) {
    let mut lines: Vec<String> = Vec::new();
    let r = run_winget(task, &install_args(id, source), &mut |l| {
        apply_line(app, id, l);
        lines.push(l.to_string());
        if lines.len() > 200 {
            lines.drain(..100);
        }
    });
    match r {
        Ok(s) => {
            let code = s.code.map(|c| c as i64);
            let text = tail(&lines);
            if wingetout::winget_not_found(code, &text) {
                return (code, text, Some(wingetout::WINGET_MISSING.into()));
            }
            (code, text, None)
        }
        Err(e) if e.contains("No se pudo iniciar") => (None, String::new(), Some(wingetout::WINGET_MISSING.into())),
        Err(e) => (None, tail(&lines), Some(crate::escalate::scrub(&e))),
    }
}

fn worker(app: tauri::AppHandle) {
    let task = Task::new(&app, "install-apps").named("Instalación de programas");
    loop {
        let next = {
            let mut q = lock();
            let total = q.items.len();
            let done = q.items.iter().filter(|i| finished(i.state)).count();
            match q.items.iter_mut().find(|i| i.state == "queued") {
                Some(it) => {
                    it.state = "starting";
                    Some((it.clone(), done, total))
                }
                None => {
                    q.running = false;
                    None
                }
            }
        };
        let Some((item, done, total)) = next else { break };
        let _ = app.emit("install-item", &item);
        if task.cancelled() {
            update(&app, &item.id, |i| {
                i.state = "skipped";
                i.kind = "cancelled";
                i.message = crate::ps::CANCELLED_MSG.into();
            });
            continue;
        }
        task.step(format!("{}/{total} · Instalando {}…", done + 1, item.name));
        let (code, text, error) = {
            let _winget = winget_lock();
            install_one(&app, &task, &item.id, &item.source)
        };
        let (verdict, message, kind, reboot, code_text) = match (code, error) {
            (_, Some(e)) if e == crate::ps::CANCELLED_MSG => (Verdict::Skipped, "Cancelado por el usuario.".to_string(), "cancelled", false, None),
            (_, Some(e)) => (Verdict::Failed, e, "other", false, None),
            (Some(c), None) => {
                let o = wingetout::interpret(c, &text);
                (o.verdict, o.message, o.kind, o.reboot, o.code)
            }
            (None, None) => (Verdict::Failed, "winget terminó sin dar un código de salida".to_string(), "other", false, None),
        };
        let journal: Result<(), String> = match verdict {
            Verdict::Failed => Err(message.clone()),
            _ => Ok(()),
        };
        if kind != "cancelled" {
            app.state::<TweakState>().record(Op::Run, &format!("Instalar {}", item.name), &journal);
        }
        // «Hay que cerrar {programa}»: con el nombre que tiene en el catálogo.
        let message = if kind == "close-app" { format!("Hay que cerrar {} para poder instalarlo", item.name) } else { message };
        update(&app, &item.id, |i| {
            i.state = match verdict {
                Verdict::Done => "done",
                Verdict::Skipped => "skipped",
                Verdict::Failed => "failed",
            };
            i.percent = None;
            i.message = message;
            i.kind = kind;
            i.reboot = reboot;
            i.code = code_text;
        });
    }
    invalidate_installed();
    let _ = app.emit("install-finished", ());
}

fn start_worker(app: &tauri::AppHandle) {
    let go = {
        let mut q = lock();
        let go = !q.running && q.items.iter().any(|i| i.state == "queued");
        if go {
            q.running = true;
        }
        go
    };
    if go {
        let app = app.clone();
        std::thread::spawn(move || worker(app));
    }
}

// ---------- Comandos ----------

/// Pone los programas en la cola (y empieza si no hay una instalación en marcha). Lo que
/// quedó terminado de la tanda anterior se quita; lo que sigue esperando una decisión, no.
#[tauri::command]
pub fn install_enqueue(app: tauri::AppHandle, apps: Vec<CatalogApp>) -> Result<Vec<Item>, String> {
    {
        let mut q = lock();
        if !q.running {
            q.items.retain(|i| !finished(i.state));
        }
        for a in apps {
            if !valid_id(&a.id) {
                let mut bad = new_item(&a);
                bad.state = "failed";
                bad.message = "Identificador no válido.".into();
                q.items.retain(|i| i.id != a.id);
                q.items.push(bad);
                continue;
            }
            match q.items.iter_mut().find(|i| i.id == a.id) {
                // Ya está en la cola o se está instalando: no se repite.
                Some(i) if !finished(i.state) => {}
                Some(i) => *i = new_item(&a),
                None => q.items.push(new_item(&a)),
            }
        }
    }
    start_worker(&app);
    Ok(lock().items.clone())
}

#[tauri::command]
pub fn install_snapshot() -> Vec<Item> {
    lock().items.clone()
}

/// Quita de la lista lo ya terminado (la pantalla vuelve a la selección).
#[tauri::command]
pub fn install_clear() {
    let mut q = lock();
    if !q.running {
        q.items.clear();
    } else {
        q.items.retain(|i| !finished(i.state));
    }
}

/// «Saltar»: el programa se deja sin instalar.
#[tauri::command]
pub fn install_skip(app: tauri::AppHandle, id: String) {
    update(&app, &id, |i| {
        if i.state == "failed" || i.state == "queued" {
            i.state = "skipped";
            if i.message.is_empty() {
                i.message = "Saltado".into();
            }
        }
    });
    maybe_finish(&app);
}

/// «Reintentar»: vuelve a la cola, detrás de los que faltan.
#[tauri::command]
pub fn install_retry(app: tauri::AppHandle, id: String) {
    retry(&app, &id);
}

fn retry(app: &tauri::AppHandle, id: &str) {
    update(app, id, |i| {
        if i.state == "failed" || i.state == "skipped" {
            i.state = "queued";
            i.percent = None;
            i.message = String::new();
            i.kind = "";
            i.code = None;
        }
    });
    // Detrás de los demás: así el resto de la cola no espera.
    {
        let mut q = lock();
        if let Some(pos) = q.items.iter().position(|i| i.id == id && i.state == "queued") {
            let it = q.items.remove(pos);
            q.items.push(it);
        }
    }
    start_worker(app);
}

/// Si todo terminó mientras se decidía, la pantalla se entera.
fn maybe_finish(app: &tauri::AppHandle) {
    let idle = {
        let q = lock();
        !q.running && q.items.iter().all(|i| finished(i.state))
    };
    if idle {
        let _ = app.emit("install-finished", ());
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blockers {
    /// Nombres de los procesos del programa que siguen abiertos.
    pub processes: Vec<String>,
    /// Cuántas ventanas visibles tienen (para pedir confirmación antes de cerrar).
    pub windows: usize,
}

fn blockers_of(id: &str, name: &str) -> Vec<(u32, String)> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut v: Vec<(u32, String)> = sys
        .processes()
        .values()
        .filter(|p| p.pid().as_u32() != std::process::id())
        .filter_map(|p| {
            let stem = p.name().to_string_lossy().to_string();
            wingetout::process_matches(&stem, id, name).then(|| (p.pid().as_u32(), stem.trim_end_matches(".exe").to_string()))
        })
        .collect();
    v.sort();
    v
}

#[cfg(windows)]
fn visible_windows(pids: &[u32]) -> usize {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible};
    struct Ctx<'a> {
        pids: &'a [u32],
        count: usize,
    }
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam es el puntero a `Ctx` que se pasa abajo y vive durante EnumWindows.
        let ctx = unsafe { &mut *(lparam as *mut Ctx) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if ctx.pids.contains(&pid) && unsafe { IsWindowVisible(hwnd) } != 0 {
            ctx.count += 1;
        }
        1
    }
    let mut ctx = Ctx { pids, count: 0 };
    // SAFETY: la devolución de llamada solo toca `ctx`, que sobrevive a la llamada.
    unsafe { EnumWindows(Some(each), &mut ctx as *mut Ctx as LPARAM) };
    ctx.count
}

#[cfg(not(windows))]
fn visible_windows(_pids: &[u32]) -> usize {
    0
}

fn item_names(id: &str) -> Option<String> {
    lock().items.iter().find(|i| i.id == id).map(|i| i.name.clone())
}

/// Qué hay abierto del programa, para decidir si «Cerrar y seguir» pide confirmación.
#[tauri::command(async)]
pub fn install_blockers(id: String) -> Blockers {
    let name = item_names(&id).unwrap_or_default();
    let found = blockers_of(&id, &name);
    let pids: Vec<u32> = found.iter().map(|(p, _)| *p).collect();
    let mut names: Vec<String> = found.into_iter().map(|(_, n)| n).collect();
    names.dedup();
    Blockers { windows: visible_windows(&pids), processes: names }
}

/// Cierra el programa (primero con cortesía, después a la fuerza) y vuelve a intentar.
#[tauri::command(async)]
pub fn install_close_and_retry(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let name = item_names(&id).ok_or("Ese programa ya no está en la cola.")?;
    let found = blockers_of(&id, &name);
    for (pid, _) in &found {
        let _ = crate::ps::exec("taskkill.exe", &["/PID", &pid.to_string()]);
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline && !blockers_of(&id, &name).is_empty() {
        std::thread::sleep(Duration::from_millis(250));
    }
    for (pid, _) in blockers_of(&id, &name) {
        let _ = crate::ps::exec("taskkill.exe", &["/F", "/PID", &pid.to_string()]);
    }
    retry(&app, &id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, state: &'static str) -> Item {
        Item { id: id.into(), name: id.into(), state, percent: None, size: None, message: String::new(), kind: "", reboot: false, code: None, source: "winget".into() }
    }

    #[test]
    fn tail_keeps_the_last_useful_lines_and_drops_bars() {
        let lines: Vec<String> = ["Found Git [Git.Git]", "  ██▒▒  48%", "-", "   -    \\    | Installer failed with exit code: 1603", "  ██  100%"].iter().map(|s| s.to_string()).collect();
        let t = tail(&lines);
        assert!(t.contains("exit code: 1603") && t.ends_with("exit code: 1603") && !t.contains("| Installer"), "{t}");
        assert!(!t.contains('%'), "{t}");
    }

    /// Usa winget y la red (sin instalar nada: el paquete no existe): `cargo test real_missing_package -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_missing_package() {
        let task = Task::detached("test-winget");
        let args = install_args("AdminOps.NoExiste.Paquete", "winget");
        let mut lines = Vec::new();
        let r = run_winget(&task, &args, &mut |l| lines.push(l.to_string())).unwrap();
        println!("líneas: {lines:#?}
código: {:?}", r.code);
        let o = wingetout::interpret(r.code.unwrap() as i64, &tail(&lines));
        println!("{o:?}");
        assert_eq!(o.kind, "missing");
    }

    /// Sin instalar nada (solo baja el instalador a una carpeta temporal): `cargo test real_download_lines -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_download_lines() {
        let dir = std::env::temp_dir().join("adminops-winget-dl-test");
        let task = Task::detached("test-winget");
        let args: Vec<String> = ["download", "--id", "VideoLAN.VLC", "--exact", "--source", "winget", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity", "--download-directory"]
            .iter()
            .map(|s| s.to_string())
            .chain(std::iter::once(format!("\"{}\"", dir.display())))
            .collect();
        let mut lines = Vec::new();
        let r = run_winget(&task, &args, &mut |l| lines.push(l.to_string())).unwrap();
        let mut last_percent = None;
        for l in &lines {
            let p = wingetout::parse_line(l);
            if let Line::Progress { percent, .. } = p {
                if last_percent.is_none_or(|x| percent >= x + 10 || percent == 100) {
                    println!("{l:?} -> {p:?}");
                    last_percent = Some(percent);
                }
            } else {
                println!("{l:?} -> {p:?}");
            }
        }
        println!("{} líneas, código {:?}", lines.len(), r.code);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finished_states() {
        assert!(finished("done") && finished("skipped") && finished("failed"));
        assert!(!finished("queued") && !finished("starting") && !finished("downloading") && !finished("installing"));
        let _ = item("x", "queued");
    }
}
