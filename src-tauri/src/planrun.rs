//! El plan sigue solo después de reiniciar.
//!
//! «Preparar equipos» ejecuta una plantilla paso a paso. Cuando un paso pide
//! reiniciar (cambiar el nombre, unir al dominio, «Reiniciar el equipo»), el
//! estado del plan se guarda en disco, se crea una tarea programada de un solo uso
//! (al iniciar sesión, con los permisos más altos) que abre el .exe actual con
//! `--seguir-plan`, y se reinicia. Al volver, AdminOps se abre solo, comprueba que
//! el paso quedó bien y sigue con el siguiente. No vale una entrada de `RunOnce`:
//! AdminOps pide administrador y Windows no lanza ahí programas que piden elevación.
//!
//! El estado va a la carpeta de datos compartidos (en portable, el USB): al
//! cambiar el nombre del equipo, la carpeta propia de este equipo cambiaría con él.

use crate::tweaks::registry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Entrada de `RunOnce` de versiones anteriores: ya no se crea, solo se borra si queda.
const RUN_ONCE: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce";
const RUN_ONCE_NAME: &str = "AdminOpsSeguir";
/// Tarea programada de un solo uso que abre AdminOps al volver del reinicio.
const TASK: &str = "AdminOps seguir plan";
/// Argumento con el que arranca AdminOps tras el reinicio.
pub const RESUME_ARG: &str = "--seguir-plan";
/// Un plan guardado más viejo que esto no se retoma.
const MAX_AGE_SECS: u64 = 24 * 3600;

/// Lo que hay que comprobar al volver del reinicio.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Wait {
    /// `rename`, `domain` o `reboot`.
    pub kind: String,
    /// El nombre del equipo (rename) o el dominio (domain) esperados.
    pub expect: String,
    /// Con `domain`: el nombre nuevo del equipo, si se cambió a la vez.
    pub name: String,
}

/// El plan tal y como se quedó: la plantilla, cómo va cada paso y qué se espera.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanState {
    /// La plantilla entera (el formato lo define la interfaz).
    pub recipe: serde_json::Value,
    /// `pending`, `running`, `ok`, `error` o `skipped`, uno por paso.
    pub statuses: Vec<String>,
    pub messages: Vec<String>,
    pub stop_on_error: bool,
    /// El paso que pidió reiniciar.
    pub waiting_step: usize,
    pub wait: Wait,
    /// Nombre del equipo al guardar: así el plan no se retoma en otro equipo (USB).
    pub host_before: String,
    /// Arranque del equipo al guardar (segundos Unix), para comprobar que se reinició.
    pub boot_time: u64,
    pub saved_at: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlanLoad {
    pub state: PlanState,
    /// AdminOps se abrió con `--seguir-plan` (lo lanzó el reinicio).
    pub from_args: bool,
    /// El equipo arrancó de nuevo desde que se guardó el plan.
    pub rebooted: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanCheck {
    pub ok: bool,
    pub message: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn path(app: &tauri::AppHandle) -> PathBuf {
    crate::paths::shared_data_dir(app).join("plan-pendiente.json")
}

fn same_host(a: &str, b: &str) -> bool {
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

/// ¿Este plan es de este equipo? Sale del mismo equipo (nombre de antes) o ya con el
/// nombre nuevo que se esperaba.
pub fn belongs_here(s: &PlanState, host: &str) -> bool {
    let renamed_to = if s.wait.kind == "rename" { s.wait.expect.as_str() } else { s.wait.name.as_str() };
    same_host(&s.host_before, host) || same_host(renamed_to, host)
}

/// ¿Sigue vigente (menos de 24 h)?
pub fn is_fresh(s: &PlanState, at: u64) -> bool {
    s.saved_at <= at && at - s.saved_at < MAX_AGE_SECS
}

fn read(app: &tauri::AppHandle) -> Option<PlanState> {
    let text = std::fs::read_to_string(path(app)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write(app: &tauri::AppHandle, s: &PlanState) -> Result<(), String> {
    crate::paths::write_json(&path(app), s)
}

/// Borra la entrada de `RunOnce` que dejaban las versiones anteriores.
fn remove_legacy_run_once() {
    let _ = registry::delete(RUN_ONCE, RUN_ONCE_NAME);
}

/// El script que crea la tarea: al iniciar sesión el usuario actual, con permisos
/// altos, abre `exe` (entre comillas, por si la ruta lleva espacios) con `--seguir-plan`.
pub fn resume_task_script(exe: &str) -> String {
    format!(
        "$ErrorActionPreference = 'Stop'\n{}{}\
         $action = New-ScheduledTaskAction -Execute ('\"' + $exe + '\"') -Argument '{RESUME_ARG}'\n\
         $trigger = New-ScheduledTaskTrigger -AtLogOn -User \"$env:USERDOMAIN\\$env:USERNAME\"\n\
         $principal = New-ScheduledTaskPrincipal -UserId \"$env:USERDOMAIN\\$env:USERNAME\" -LogonType Interactive -RunLevel Highest\n\
         $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Seconds 0)\n\
         Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null\n'ok'",
        crate::ps::text_var("name", TASK),
        crate::ps::text_var("exe", exe)
    )
}

fn set_resume_task() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("No se pudo saber dónde está AdminOps: {e}"))?;
    let exe = exe.display().to_string();
    let exe = exe.strip_prefix(r"\\?\").unwrap_or(&exe);
    crate::ps::powershell(&resume_task_script(exe)).map(|_| ())
}

/// Borra la tarea de un solo uso (y la entrada de `RunOnce` de versiones anteriores).
fn remove_resume_task() {
    remove_legacy_run_once();
    let script = format!("{}Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue\n'ok'", crate::ps::text_var("name", TASK));
    let _ = crate::ps::powershell(&script);
}

fn host() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_default()
}

fn stamp(mut s: PlanState) -> PlanState {
    s.host_before = host();
    s.boot_time = sysinfo::System::boot_time();
    s.saved_at = now();
    s
}

// ---------- Comprobaciones ----------

/// El dominio escrito (`empresa.local` o `EMPRESA`) frente al que dice Windows.
pub fn domain_matches(entered: &str, actual: &str) -> bool {
    let (e, a) = (entered.trim().trim_end_matches('.').to_lowercase(), actual.trim().trim_end_matches('.').to_lowercase());
    !e.is_empty() && (e == a || a.starts_with(&format!("{e}.")) || e.starts_with(&format!("{a}.")))
}

/// El equipo arrancó después de pedir el reinicio.
pub fn rebooted(before: u64, now_boot: u64) -> bool {
    // La hora de arranque se calcula (ahora - tiempo encendido) y puede bailar un par de segundos.
    now_boot > before + 10
}

/// Comprueba el paso que pidió reiniciar. Pura: recibe lo que se ve ahora.
pub fn evaluate(wait: &Wait, boot_before: u64, current_name: &str, boot_now: u64, domain: Option<&str>) -> PlanCheck {
    let fail = |m: String| PlanCheck { ok: false, message: m };
    match wait.kind.as_str() {
        "rename" => {
            if same_host(&wait.expect, current_name) {
                PlanCheck { ok: true, message: format!("El equipo ya se llama «{current_name}».") }
            } else {
                fail(format!("El nombre del equipo sigue siendo {current_name}: el cambio no se aplicó."))
            }
        }
        "domain" => {
            let in_domain = domain.is_some_and(|d| domain_matches(&wait.expect, d));
            if !in_domain {
                return fail(match domain {
                    Some(d) => format!("El equipo está en el dominio {d}, no en {}: la unión no se aplicó.", wait.expect),
                    None => format!("El equipo no está en el dominio {}: la unión no se aplicó.", wait.expect),
                });
            }
            if !wait.name.is_empty() && !same_host(&wait.name, current_name) {
                return fail(format!("El nombre del equipo sigue siendo {current_name}: el cambio no se aplicó."));
            }
            PlanCheck { ok: true, message: format!("El equipo está en el dominio {}.", wait.expect) }
        }
        "reboot" => {
            if rebooted(boot_before, boot_now) {
                PlanCheck { ok: true, message: "El equipo se reinició.".into() }
            } else {
                fail("El equipo no se ha reiniciado desde que se pidió.".into())
            }
        }
        otro => fail(format!("No se sabe comprobar «{otro}».")),
    }
}

// ---------- Comandos ----------

/// Guarda el plan (sin reiniciar). La interfaz lo usa para dejarlo apuntado.
/// Deja también la tarea que abre AdminOps al volver. Devuelve si se pudo crear
/// (sin ella, el técnico tiene que abrir AdminOps a mano al volver).
#[tauri::command(async)]
pub fn plan_state_save(app: tauri::AppHandle, state: PlanState) -> Result<bool, String> {
    {
        let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        write(&app, &stamp(state))?;
    }
    Ok(set_resume_task().is_ok())
}

/// El plan guardado de este equipo, si lo hay y tiene menos de 24 h.
#[tauri::command(async)]
pub fn plan_state_load(app: tauri::AppHandle) -> Option<PlanLoad> {
    let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let state = read(&app)?;
    if !is_fresh(&state, now()) {
        let _ = std::fs::remove_file(path(&app));
        remove_resume_task();
        return None;
    }
    if !belongs_here(&state, &host()) {
        return None;
    }
    // Ya se abrió AdminOps: la tarea de un solo uso cumplió.
    remove_resume_task();
    let rebooted = rebooted(state.boot_time, sysinfo::System::boot_time());
    Some(PlanLoad { state, from_args: std::env::args().any(|a| a == RESUME_ARG), rebooted })
}

/// Borra el plan guardado y la tarea de un solo uso («Parar el plan», o plan terminado).
#[tauri::command(async)]
pub fn plan_state_clear(app: tauri::AppHandle) {
    {
        let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = std::fs::remove_file(path(&app));
    }
    remove_resume_task();
}

/// Comprueba el paso que pidió reiniciar (el nombre ya es el nuevo, el equipo está
/// en el dominio, el arranque es posterior).
#[tauri::command(async)]
pub fn plan_check(state: PlanState) -> PlanCheck {
    let domain = if state.wait.kind == "domain" {
        // Si no se puede leer el dominio, no se da la unión por fallida: se avisa y se puede reintentar.
        let status = match crate::domain::status() {
            Ok(s) => s,
            Err(e) => return PlanCheck { ok: false, message: format!("No se pudo comprobar el dominio: {e}") },
        };
        serde_json::to_value(status).ok().and_then(|v| {
            if v.get("partOfDomain").and_then(|p| p.as_bool()) == Some(true) {
                v.get("domain").and_then(|d| d.as_str()).map(str::to_string)
            } else {
                None
            }
        })
    } else {
        None
    };
    evaluate(&state.wait, state.boot_time, &host(), sysinfo::System::boot_time(), domain.as_deref())
}

/// Guarda el plan, deja la tarea que abre AdminOps al volver y reinicia en 5 segundos.
/// Devuelve si la tarea quedó creada (si no, hay que abrir AdminOps a mano al volver).
#[tauri::command(async)]
pub fn plan_reboot(app: tauri::AppHandle, state: PlanState) -> Result<bool, String> {
    {
        let _g = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        write(&app, &stamp(state))?;
    }
    let task = set_resume_task().is_ok();
    if let Err(e) = crate::ps::exec("shutdown.exe", &["/r", "/t", "5"]) {
        plan_state_clear(app);
        return Err(format!("No se pudo reiniciar: {e}"));
    }
    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(kind: &str, expect: &str, name: &str) -> Wait {
        Wait { kind: kind.into(), expect: expect.into(), name: name.into() }
    }

    #[test]
    fn el_estado_va_y_vuelve() {
        let s = PlanState {
            recipe: serde_json::json!({ "id": "k1", "name": "Equipo nuevo", "steps": [{ "kind": "rename", "newName": "PC-01" }, { "kind": "diagnostics" }] }),
            statuses: vec!["ok".into(), "pending".into()],
            messages: vec!["El equipo se llamará «PC-01» al reiniciar.".into(), String::new()],
            stop_on_error: true,
            waiting_step: 0,
            wait: wait("rename", "PC-01", ""),
            host_before: "PC-VIEJO".into(),
            boot_time: 1_790_000_000,
            saved_at: 1_790_000_100,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"waitingStep\":0"), "{json}");
        let otro: PlanState = serde_json::from_str(&json).unwrap();
        assert_eq!(otro, s);
    }

    #[test]
    fn un_estado_incompleto_se_lee_con_valores_por_defecto() {
        let s: PlanState = serde_json::from_str(r#"{"statuses":["ok"],"savedAt":5}"#).unwrap();
        assert_eq!(s.statuses, vec!["ok"]);
        assert!(s.wait.kind.is_empty());
        assert_eq!(s.saved_at, 5);
    }

    #[test]
    fn vigente_menos_de_24_horas() {
        let s = PlanState { saved_at: 1_000, ..Default::default() };
        assert!(is_fresh(&s, 1_000 + 23 * 3600));
        assert!(!is_fresh(&s, 1_000 + 24 * 3600));
        assert!(!is_fresh(&s, 500), "un plan del futuro no vale");
    }

    #[test]
    fn el_plan_es_de_este_equipo_antes_o_despues_de_renombrar() {
        let s = PlanState { host_before: "PC-VIEJO".into(), wait: wait("rename", "PC-NUEVO", ""), ..Default::default() };
        assert!(belongs_here(&s, "pc-viejo"));
        assert!(belongs_here(&s, "PC-NUEVO"));
        assert!(!belongs_here(&s, "OTRO-PC"));
        let d = PlanState { host_before: "PC-A".into(), wait: wait("domain", "empresa.local", "PC-B"), ..Default::default() };
        assert!(belongs_here(&d, "PC-B"));
        assert!(!belongs_here(&PlanState::default(), ""), "sin nombre no se reconoce");
    }

    #[test]
    fn rename_comprueba_el_nombre() {
        let w = wait("rename", "PC-NUEVO", "");
        assert!(evaluate(&w, 0, "pc-nuevo", 0, None).ok);
        let c = evaluate(&w, 0, "PC-VIEJO", 0, None);
        assert!(!c.ok);
        assert_eq!(c.message, "El nombre del equipo sigue siendo PC-VIEJO: el cambio no se aplicó.");
    }

    #[test]
    fn domain_comprueba_el_dominio_y_el_nombre() {
        let w = wait("domain", "empresa.local", "PC-02");
        assert!(evaluate(&w, 0, "PC-02", 0, Some("EMPRESA.LOCAL")).ok);
        assert!(!evaluate(&w, 0, "PC-02", 0, None).ok);
        assert!(!evaluate(&w, 0, "PC-02", 0, Some("otra.local")).ok);
        let sin_nombre = evaluate(&w, 0, "PC-01", 0, Some("empresa.local"));
        assert!(!sin_nombre.ok);
        assert!(sin_nombre.message.contains("PC-01"));
        assert!(domain_matches("empresa", "empresa.local"));
        assert!(!domain_matches("", "empresa.local"));
    }

    #[test]
    fn reboot_comprueba_que_el_arranque_es_posterior() {
        let w = wait("reboot", "", "");
        assert!(evaluate(&w, 1_000, "X", 2_000, None).ok);
        assert!(!evaluate(&w, 1_000, "X", 1_000, None).ok);
        // La hora de arranque baila un par de segundos entre dos lecturas: no es un reinicio.
        assert!(!rebooted(1_000, 1_002));
        assert!(rebooted(1_000, 1_100));
        assert!(!evaluate(&wait("raro", "", ""), 0, "X", 0, None).ok);
    }

    #[test]
    fn la_tarea_abre_el_exe_entre_comillas_con_seguir_plan() {
        // Con espacios en la ruta (portable en un USB con nombre): sin comillas no arrancaría.
        let script = resume_task_script(r"E:\Mi USB\AdminOps.exe");
        assert!(script.contains(r"$exe = 'E:\Mi USB\AdminOps.exe'"), "{script}");
        assert!(script.contains(r#"-Execute ('"' + $exe + '"') -Argument '--seguir-plan'"#), "{script}");
        assert!(script.contains("-AtLogOn"), "{script}");
        assert!(script.contains("-RunLevel Highest"), "{script}");
        assert!(script.contains(&format!("$name = '{TASK}'")), "{script}");
        let errores = crate::ps::parse_errors(&script);
        assert!(errores.is_empty(), "el script no compila: {errores}");
    }
}
