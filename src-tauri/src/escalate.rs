//! Paquete de pruebas para escalar: cuando hay que pasar el problema a
//! Microsoft, al fabricante o a un compañero, un clic reúne los volcados de
//! pantallazo, los registros de eventos, el del servicio de componentes (CBS),
//! la información del sistema, los drivers, el diario de cambios y lo hecho en
//! el caso, en un zip con un resumen al principio. Sin datos personales que no
//! hagan falta: las rutas de usuario se tapan en los textos.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// «C:\Users\ana\…» → «C:\Users\[usuario]\…» en un texto.
pub fn scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let lower = text.to_ascii_lowercase();
    let mut i = 0;
    while i < text.len() {
        let rest = &lower[i..];
        let hit = ["\\users\\", "\\usuarios\\"].iter().find(|p| rest.starts_with(*p));
        if let Some(p) = hit {
            out.push_str(&text[i..i + p.len()]);
            i += p.len();
            let end = text[i..].find(['\\', ' ', '"', '\'', '\n', '\r']).map_or(text.len(), |e| i + e);
            let name = &text[i..end];
            out.push_str(if ["public", "default", "all users", "acceso público"].contains(&name.to_ascii_lowercase().as_str()) { name } else { "[usuario]" });
            i = end;
        } else {
            let ch = text[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn write_scrubbed(dir: &Path, name: &str, text: &str) {
    let _ = std::fs::write(dir.join(name), scrub(text));
}

/// Crea el paquete y lo enseña en el Explorador. `summary`: el resumen del caso (síntoma, lo probado, lo que falló).
#[tauri::command(async)]
pub fn escalation_pack(app: tauri::AppHandle, summary: String) -> Result<String, String> {
    let task = crate::task::Task::new(&app, "escalate").named("Paquete para escalar");
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let host = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "equipo".into());
    let staging = std::env::temp_dir().join(format!("adminops-escalar-{stamp}"));
    let result = (|| -> Result<PathBuf, String> {
        std::fs::create_dir_all(staging.join("volcados")).map_err(|e| e.to_string())?;
        task.step("Resumen y datos del sistema (1 de 6)");
        let mut leeme = format!("PAQUETE PARA ESCALAR · {host} · {}\n\n{}\n", chrono::Local::now().format("%d/%m/%Y %H:%M"), summary.trim());
        leeme.push_str("\nCONTENIDO\n- sistema.txt: información del sistema (systeminfo)\n- drivers.csv: drivers instalados\n- eventos-*.evtx: registros de eventos de los últimos 7 días (se abren con el Visor de eventos)\n- eventos-resumen.txt: los errores agrupados y explicados\n- cbs.log: final del registro del servicio de componentes (Windows Update, SFC)\n- volcados/: los últimos volcados de pantallazo azul\n- diario.txt: lo que AdminOps cambió en el equipo los últimos 30 días\n");
        write_scrubbed(&staging, "LEEME-resumen.txt", &leeme);
        if let Ok(t) = crate::ps::exec_opts("systeminfo.exe", &[], crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None }) {
            write_scrubbed(&staging, "sistema.txt", &t);
        }
        task.step("Drivers (2 de 6)");
        if let Ok(t) = crate::ps::exec("driverquery.exe", &["/v", "/fo", "csv"]) {
            write_scrubbed(&staging, "drivers.csv", &t);
        }
        task.step("Registros de eventos (3 de 6)");
        let week = "*[System[TimeCreated[timediff(@SystemTime) <= 604800000]]]";
        for (log, name) in [("System", "eventos-sistema.evtx"), ("Application", "eventos-aplicacion.evtx")] {
            let _ = crate::ps::exec("wevtutil.exe", &["epl", log, &staging.join(name).display().to_string(), &format!("/q:{week}"), "/ow:true"]);
        }
        if let Ok(groups) = crate::eventdigest::event_digest(7) {
            let text: String = groups.iter().filter(|g| g.weight != "noise").map(|g| format!("[{}] {} ×{} — {}\n    Qué hacer: {}\n    ({} {})\n", g.weight, g.title, g.count, g.meaning, g.todo, g.provider, g.id)).collect();
            write_scrubbed(&staging, "eventos-resumen.txt", &text);
        }
        task.step("Registro CBS (4 de 6)");
        let windir = std::env::var("windir").unwrap_or_else(|_| r"C:\Windows".into());
        if let Ok(bytes) = std::fs::read(Path::new(&windir).join(r"Logs\CBS\CBS.log")) {
            let tail = &bytes[bytes.len().saturating_sub(3 << 20)..];
            write_scrubbed(&staging, "cbs.log", &String::from_utf8_lossy(tail));
        }
        task.step("Volcados de pantallazo (5 de 6)");
        let mut dumps: Vec<PathBuf> = std::fs::read_dir(Path::new(&windir).join("Minidump")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dmp"))).collect();
        dumps.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        for d in dumps.iter().rev().take(5) {
            if let Some(n) = d.file_name() {
                let _ = std::fs::copy(d, staging.join("volcados").join(n));
            }
        }
        task.step("Diario de cambios y zip (6 de 6)");
        {
            use tauri::Manager;
            let state = app.state::<crate::tweaks::TweakState>();
            let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()).saturating_sub(30 * 86_400);
            let text: String = state.journal_since(since).iter().map(|e| format!("{} · {} · {}{}\n", chrono::DateTime::from_timestamp(e.timestamp as i64, 0).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()).unwrap_or_default(), if e.ok { "bien" } else { "falló" }, e.title, e.message.as_deref().map(|m| format!(" — {m}")).unwrap_or_default())).collect();
            write_scrubbed(&staging, "diario.txt", &text);
        }
        let out_dir = crate::paths::reports_dir(&app);
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let zip = out_dir.join(format!("Escalar-{host}-{stamp}.zip"));
        let script = format!(
            "{}{}Compress-Archive -Path (Join-Path $src '*') -DestinationPath $dst -Force\n'ok'",
            crate::ps::text_var("src", &staging.display().to_string()),
            crate::ps::text_var("dst", &zip.display().to_string())
        );
        crate::ps::powershell_opts(&script, crate::ps::Opts { timeout: Some(Duration::from_secs(180)), task: None })?;
        Ok(zip)
    })();
    let _ = std::fs::remove_dir_all(&staging);
    let zip = result?;
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", zip.display())).spawn();
    Ok(zip.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::scrub;

    #[test]
    fn tapa_los_usuarios() {
        assert_eq!(scrub(r"C:\Users\ana\Desktop\x.txt y C:\Users\Public\y"), r"C:\Users\[usuario]\Desktop\x.txt y C:\Users\Public\y");
        assert_eq!(scrub(r"D:\Usuarios\pepe"), r"D:\Usuarios\[usuario]");
        assert_eq!(scrub("sin rutas ñ"), "sin rutas ñ");
    }
}
