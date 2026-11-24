//! El registro de acciones: para cada cosa que AdminOps cambia en el equipo desde
//! un botón, su **comando equivalente** («Ver el comando») y **qué mide** (el antes
//! y el después de cada arreglo).
//!
//! Dónde vive cada cosa, para no duplicar nada:
//! - Las acciones y ajustes del catálogo (`src-tauri/tweaks/*.toml`) ya son datos: el
//!   script que se enseña es el mismo texto que se ejecuta (sale del catálogo, no está
//!   copiado a mano) y su medida es el campo `measure` del propio ajuste. Los ajustes que
//!   AdminOps hace con la API de registro o de servicios se enseñan con su equivalente
//!   en PowerShell y se marcan `aproximado`.
//! - Las pocas acciones que no están en el catálogo (reparar la red, vaciar la cola de una
//!   impresora, quitar del inicio, punto de restauración) construyen su comando a partir
//!   de las mismas constantes que usa el código que las ejecuta.

use crate::tweaks::model::{Kind, RegKind, Startup, Tweak};
use crate::tweaks::TweakState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ActionCommand {
    pub id: String,
    pub name: String,
    /// El PowerShell, con comentarios en español.
    pub script: String,
    /// AdminOps lo hace con funciones de Windows: esto es lo equivalente, no lo mismo.
    pub approximate: bool,
    pub admin: bool,
    /// Qué mide al ejecutarse (ver `measure.rs`), si mide algo.
    pub measure: Option<String>,
    /// Nombre propuesto para «Guardar como .ps1» (sin tildes ni espacios).
    pub file_name: String,
}

/// Un id de instancia de ejemplo para las pruebas de las acciones sobre dispositivos.
#[cfg(test)]
const DEV_ID: &str = r"USB\VID_046D&PID_C52B\5&2A";

/// Acciones que no son del catálogo (para las pruebas: cada una tiene que poder construirse).
#[cfg(test)]
const EXTRA_IDS: &[&str] = &["net.repair-quick", "net.repair-deep", "printer.clear-queue", "startup.disable", "restore-point", "device.scan", "device.restart", "device.enable", "display.native", "display.extend", "display.clone", "display.restart-driver", "kbd.only-spanish", "unsaved.copy", "scan.setup", "scan.password", "scan.private", "scan.signing", "scan.shortcut", "onedrive.pause"];

fn ps_path(path: &str) -> String {
    path.replacen('\\', ":\\", 1)
}

fn ps_lit(s: &str) -> String {
    crate::ps::ps_literal(s)
}

/// Texto que va en un comentario o título del script: sin saltos de línea ni controles.
fn ps_com(s: &str) -> String {
    crate::ps::ps_comment(s)
}

/// «Renovar la IP» → «Renovar_la_IP.ps1».
pub fn file_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'Á' => 'a',
            'é' | 'è' | 'ë' | 'É' => 'e',
            'í' | 'ì' | 'ï' | 'Í' => 'i',
            'ó' | 'ò' | 'ö' | 'Ó' => 'o',
            'ú' | 'ù' | 'ü' | 'Ú' => 'u',
            'ñ' | 'Ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_');
    format!("{}.ps1", if out.is_empty() { "AdminOps" } else { out })
}

/// Las variables `$User*` del catálogo, con lo que valen en la cuenta que lo ejecuta
/// (sin rutas con el nombre del usuario: `$env:USERPROFILE`, `$env:TEMP`).
fn user_prelude(body: &str) -> String {
    let vars: [(&str, &str); 6] = [
        ("$UserSid", "$UserSid = ([Security.Principal.WindowsIdentity]::GetCurrent()).User.Value"),
        ("$UserHive", "$UserHive = 'HKCU:'"),
        ("$UserProfile", "$UserProfile = $env:USERPROFILE"),
        ("$UserAppData", "$UserAppData = $env:APPDATA"),
        ("$UserLocalAppData", "$UserLocalAppData = $env:LOCALAPPDATA"),
        ("$UserTemp", "$UserTemp = $env:TEMP"),
    ];
    let lines: Vec<&str> = vars.iter().filter(|(v, _)| body.contains(v)).map(|(_, l)| *l).collect();
    if lines.is_empty() {
        String::new()
    } else {
        format!("# Carpetas del usuario con la sesión abierta\n{}\n\n", lines.join("\n"))
    }
}

fn reg_kind(k: RegKind) -> &'static str {
    match k {
        RegKind::Dword => "DWord",
        RegKind::Qword => "QWord",
        RegKind::String => "String",
        RegKind::Expand => "ExpandString",
    }
}

fn service_lines(s: &crate::tweaks::model::ServiceAction) -> Vec<String> {
    let name = ps_lit(&s.name);
    let what = match s.startup {
        Startup::Automatic => "automático",
        Startup::Delayed => "automático (retrasado)",
        Startup::Manual => "manual",
        Startup::Disabled => "deshabilitado",
    };
    let mut v = vec![format!("# Servicio {}: arranque {what}{}", ps_com(&s.name), if s.stop { " y detenerlo ahora" } else { "" })];
    v.push(match s.startup {
        Startup::Automatic => format!("Set-Service -Name {name} -StartupType Automatic"),
        Startup::Manual => format!("Set-Service -Name {name} -StartupType Manual"),
        Startup::Disabled => format!("Set-Service -Name {name} -StartupType Disabled"),
        Startup::Delayed => format!("sc.exe config {name} start= delayed-auto | Out-Null"),
    });
    if s.stop {
        v.push(format!("Stop-Service -Name {name} -Force -ErrorAction SilentlyContinue"));
    }
    v
}

/// El PowerShell de un ajuste o tarea del catálogo y si es solo aproximado.
pub fn tweak_script(t: &Tweak) -> (String, bool) {
    let mut out = vec![format!("# {}", ps_com(&t.name)), format!("# {}", ps_com(&t.description))];
    if let Some(n) = &t.note {
        out.push(format!("# Ojo: {}", ps_com(n)));
    }
    out.push(String::new());
    // Registro y servicios los hace AdminOps con la API de Windows: lo equivalente en PowerShell.
    let approximate = !t.registry.is_empty() || !t.service.is_empty();
    for r in &t.registry {
        let data = match &r.value {
            crate::tweaks::model::RegData::Int(n) => n.to_string(),
            crate::tweaks::model::RegData::Str(s) => ps_lit(s),
        };
        out.push(format!("# Registro: {}\\{}", ps_com(&r.path), ps_com(&r.name)));
        out.push(format!("$key = {}", ps_lit(&ps_path(&r.path))));
        out.push("if (-not (Test-Path $key)) { New-Item -Path $key -Force | Out-Null }".into());
        out.push(format!("New-ItemProperty -Path $key -Name {} -PropertyType {} -Value {data} -Force | Out-Null", ps_lit(&r.name), reg_kind(r.kind)));
        out.push(String::new());
    }
    for s in &t.service {
        out.extend(service_lines(s));
        out.push(String::new());
    }
    let script = t.script.as_ref();
    let body = if t.kind == Kind::Action { script.and_then(|s| s.run.as_deref()) } else { script.and_then(|s| s.apply.as_deref()) };
    let mut head = out.join("\n");
    while head.contains("\n\n\n") {
        head = head.replace("\n\n\n", "\n\n");
    }
    let mut text = head.trim_end().to_string();
    if let Some(body) = body {
        let body = body.trim_end();
        text.push_str("\n\n");
        text.push_str(&user_prelude(body));
        text.push_str(body);
    }
    (text.trim_end().to_string() + "\n", approximate)
}

fn net_script(deep: bool, adapters: &str) -> String {
    use crate::troubleshoot::{restart_adapters_script, NET_ADAPTERS_TITLE, NET_DEEP, NET_FIRST, NET_RENEW};
    let line = |(title, program, args): &crate::troubleshoot::NetCmd| format!("# {title}\n{program} {}\n", args.join(" "));
    let mut out = format!("# Reparar la red{}\n\n", if deep { " a fondo" } else { "" });
    for c in NET_FIRST {
        out.push_str(&line(c));
    }
    if deep {
        for c in NET_DEEP {
            out.push_str(&line(c));
        }
    }
    out.push_str(&format!("# {NET_ADAPTERS_TITLE} (los físicos que no estén desactivados{adapters})\n"));
    out.push_str(restart_adapters_script().trim());
    out.push_str("\n\n# Espera a que vuelva la conexión antes de renovar\nStart-Sleep -Seconds 4\n");
    out.push_str(&format!("# {} (si hay IP fija, no pasa nada)\n{} {}\n", NET_RENEW.0, NET_RENEW.1, NET_RENEW.2.join(" ")));
    out
}

/// Los adaptadores físicos del equipo, para rellenar el comando («: Ethernet, Wi-Fi»).
fn adapter_names() -> String {
    let script = "(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { \"$($_.Status)\" -ne 'Disabled' -and \"$($_.Status)\" -ne 'Not Present' } | ForEach-Object { $_.Name }) -join ', '";
    crate::ps::powershell_opts(script, crate::ps::Opts { timeout: Some(std::time::Duration::from_secs(15)), task: None })
        .ok()
        .map(|s| ps_com(&s))
        .filter(|s| !s.is_empty())
        .map(|s| format!(": en este equipo, {s}"))
        .unwrap_or_default()
}

/// Construye el comando de una acción. `context` es el dato que hace falta para
/// rellenarlo (la impresora, la entrada del inicio).
pub fn build(state: &TweakState, id: &str, context: Option<&str>) -> Result<ActionCommand, String> {
    if let Ok(t) = state.find(id) {
        let (script, approximate) = tweak_script(t);
        return Ok(ActionCommand { id: t.id.clone(), name: t.name.clone(), script, approximate, admin: t.needs_admin(), measure: t.measure.clone(), file_name: file_name(&t.name) });
    }
    let make = |name: &str, script: String, approximate: bool, admin: bool, measure: Option<String>| ActionCommand {
        id: id.to_string(),
        name: name.to_string(),
        script,
        approximate,
        admin,
        measure,
        file_name: file_name(name),
    };
    match id {
        "net.repair-quick" => Ok(make("Reparar la red", net_script(false, &adapter_names()), false, true, Some("ping".into()))),
        "net.repair-deep" => Ok(make("Reparar la red a fondo", net_script(true, &adapter_names()), false, true, Some("ping".into()))),
        "printer.clear-queue" => {
            let printer = context.filter(|c| !c.trim().is_empty()).ok_or("Falta la impresora.")?;
            let script = format!(
                "# Vaciar la cola de impresión\n\n# La impresora\n{}\n# Quita todos los trabajos atascados de esa cola\n{}\n",
                crate::ps::text_var("name", printer).trim_end(),
                crate::printers::CLEAR_QUEUE_BODY
            );
            Ok(make("Vaciar la cola de impresión", script, false, false, Some(format!("cola:{printer}"))))
        }
        "device.scan" | "device.restart" | "device.enable" => {
            let (name, script) = crate::connected::action_script(id.trim_start_matches("device."), context.unwrap_or(""))?;
            // Buscar el driver usa la API de Windows Update por COM: PowerShell hace lo mismo, no es la misma llamada.
            Ok(make(name, script, id == "device.scan", true, None))
        }
        "display.native" | "display.extend" | "display.clone" | "display.restart-driver" => {
            let (name, script) = crate::displays::action_script(id.trim_start_matches("display."), context.unwrap_or(""))?;
            // Todo esto lo hace AdminOps con la API de Windows; el comando es lo equivalente.
            Ok(make(name, script, true, false, None))
        }
        "scan.setup" | "scan.password" | "scan.private" | "scan.signing" | "scan.shortcut" => {
            let (name, script) = crate::scanfolder::action_script(id.trim_start_matches("scan."), context.unwrap_or(""))?;
            // La contraseña y los usuarios se manejan con las funciones de Windows; el comando es lo equivalente.
            Ok(make(name, script, true, true, None))
        }
        "onedrive.pause" => {
            let (name, script) = crate::onedrive::action_script()?;
            Ok(make(name, script, true, false, None))
        }
        "kbd.only-spanish" => {
            let (name, script) = crate::kbdlang::action_script("only-spanish")?;
            // El atajo se quita escribiendo en el registro con la API; la lista de idiomas, con PowerShell tal cual.
            Ok(make(name, script, true, false, None))
        }
        "unsaved.copy" => {
            let (name, script) = crate::unsaved::action_script(context.unwrap_or(""))?;
            Ok(make(name, script, true, false, None))
        }
        "startup.disable" => {
            let item = context.filter(|c| !c.trim().is_empty()).ok_or("Falta el programa del inicio.")?;
            let (name, script, admin) = crate::tweaks::startup::disable_doc(item)?;
            Ok(make(&format!("Quitar del inicio: {name}"), format!("# Quitar del inicio\n\n{script}\n"), true, admin, Some("inicio".into())))
        }
        "restore-point" => {
            let script = "# Crear un punto de restauración\n\n\
                 # Windows solo deja crear uno cada 24 h; se quita ese límite un momento\n\
                 $k = 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SystemRestore'\n\
                 Set-ItemProperty $k -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord\n\
                 # La protección del sistema tiene que estar activada en la unidad de Windows\n\
                 Enable-ComputerRestore -Drive \"$env:SystemDrive\\\"\n\
                 # El punto de restauración\n\
                 Checkpoint-Computer -Description 'AdminOps: punto manual' -RestorePointType MODIFY_SETTINGS\n\
                 # Se deja el límite como estaba\n\
                 Remove-ItemProperty $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue\n"
                .to_string();
            Ok(make("Crear un punto de restauración", script, true, true, None))
        }
        _ => Err(format!("Acción desconocida: {id}")),
    }
}

/// El archivo `.ps1` que se guarda: cabecera con versión y fecha, `#Requires` si pide
/// administrador, y el comando.
pub fn script_file(cmd: &ActionCommand, today: &str) -> String {
    let mut s = format!("# Generado por AdminOps {} · {today}\n", env!("CARGO_PKG_VERSION"));
    if cmd.admin {
        s.push_str("#Requires -RunAsAdministrator\n");
    }
    if cmd.approximate {
        s.push_str("# AdminOps lo hace con funciones de Windows; esto es lo equivalente en PowerShell.\n");
    }
    s.push('\n');
    s.push_str(cmd.script.trim_end());
    s.push('\n');
    s
}

// ---------- Comandos ----------

#[tauri::command(async)]
pub fn action_command(id: String, context: Option<String>, state: State<'_, TweakState>) -> Result<ActionCommand, String> {
    build(&state, &id, context.as_deref())
}

/// Guarda el comando como `.ps1` donde elija el técnico. Devuelve el nombre del archivo, o `None` si cancela.
#[tauri::command(async)]
pub fn action_save_script(app: tauri::AppHandle, id: String, context: Option<String>, state: State<'_, TweakState>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let cmd = build(&state, &id, context.as_deref())?;
    let Some(out) = app.dialog().file().set_file_name(&cmd.file_name).add_filter("Script de PowerShell", &["ps1"]).blocking_save_file().and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let text = script_file(&cmd, &chrono::Local::now().format("%d/%m/%Y").to_string()).replace('\n', "\r\n");
    // Con BOM: Windows PowerShell 5.1 lee así bien las tildes.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(text.as_bytes());
    std::fs::write(&out, bytes).map_err(|e| format!("No se pudo guardar el archivo: {e}"))?;
    Ok(out.file_name().map(|n| n.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> TweakState {
        TweakState::with_data_dir(std::env::temp_dir().join("adminops-actioninfo-test"))
    }

    #[test]
    fn nombre_de_archivo_sin_tildes_ni_espacios() {
        assert_eq!(file_name("Renovar la IP"), "Renovar_la_IP.ps1");
        assert_eq!(file_name("Reiniciar Explorador de Windows"), "Reiniciar_Explorador_de_Windows.ps1");
        assert_eq!(file_name("Desactivar el escritorio remoto (NLA)"), "Desactivar_el_escritorio_remoto_NLA.ps1");
        assert_eq!(file_name("Reparación: ñandú"), "Reparacion_nandu.ps1");
        assert_eq!(file_name("¿¿??"), "AdminOps.ps1");
    }

    /// Cada ajuste y tarea del catálogo tiene su comando, y el de una tarea es el script que se ejecuta.
    #[test]
    fn todo_el_catalogo_tiene_comando() {
        let st = state();
        let mut con_medida = 0;
        let mut total = 0;
        for t in st.all() {
            let c = build(&st, &t.id, None).unwrap_or_else(|e| panic!("{}: {e}", t.id));
            assert!(c.script.contains(&format!("# {}", t.name)), "{}", t.id);
            assert!(c.script.lines().count() > 3, "{}: comando casi vacío", t.id);
            if t.kind == Kind::Action {
                let real = t.script.as_ref().and_then(|s| s.run.as_deref()).unwrap().trim_end();
                assert!(c.script.contains(real), "{}: el comando no es el script que se ejecuta", t.id);
                assert!(!c.approximate, "{}", t.id);
            }
            assert!(!c.script.contains("C:\\Users\\"), "{}: lleva una ruta con usuario", t.id);
            total += 1;
            if c.measure.is_some() {
                con_medida += 1;
            }
        }
        println!("catálogo: {total} acciones con comando, {con_medida} con medida");
        assert!(total >= 60);
        assert!(con_medida >= 10);
    }

    /// Las medidas que declara el catálogo existen.
    #[test]
    fn las_medidas_del_catalogo_son_validas() {
        let st = state();
        for t in st.all() {
            if let Some(m) = &t.measure {
                assert!(crate::measure::is_known(m), "{}: medida desconocida «{m}»", t.id);
            }
        }
    }

    #[test]
    fn un_ajuste_de_registro_se_marca_aproximado() {
        let st = state();
        let c = build(&st, "privacy.advertising-id", None).unwrap();
        assert!(c.approximate);
        assert!(c.script.contains("New-ItemProperty"), "{}", c.script);
        assert!(c.script.contains("HKCU:\\"), "{}", c.script);
    }

    #[test]
    fn un_nombre_con_saltos_de_linea_no_saca_codigo_del_comentario() {
        let st = state();
        let mut t = st.find("privacy.advertising-id").unwrap().clone();
        t.name = "Uno\nWrite-Host nombre\rWrite-Host cr\u{2028}Write-Host ls\u{0085}Write-Host nel".into();
        t.description = "Desc\r\nWrite-Host desc".into();
        t.note = Some("Nota\nWrite-Host nota".into());
        let (script, _) = tweak_script(&t);
        for l in script.lines() {
            assert!(!l.trim_start().starts_with("Write-Host"), "una línea se escapó del comentario: {l:?}");
        }
        assert!(!script.contains(['\r', '\u{2028}', '\u{2029}', '\u{0085}']), "{script:?}");
        let errores = crate::ps::parse_errors(&script);
        assert!(errores.trim().is_empty(), "{errores}");
    }

    #[test]
    fn variables_del_usuario_con_rutas_sin_nombre() {
        let st = state();
        let c = build(&st, "cleanup.user-temp", None).unwrap();
        assert!(c.script.contains("$UserTemp = $env:TEMP"), "{}", c.script);
        assert!(!c.approximate);
    }

    #[test]
    fn la_cola_lleva_la_impresora_ya_rellena() {
        let st = state();
        let c = build(&st, "printer.clear-queue", Some("HP-Recepción")).unwrap();
        assert!(c.script.contains("$name = 'HP-Recepción'"), "{}", c.script);
        assert!(c.script.contains("Remove-PrintJob"));
        assert_eq!(c.measure.as_deref(), Some("cola:HP-Recepción"));
        assert!(build(&st, "printer.clear-queue", None).is_err());
    }

    #[test]
    fn reparar_la_red_sale_de_los_pasos_reales() {
        let st = state();
        let c = build(&st, "net.repair-deep", None).unwrap();
        for p in ["ipconfig.exe /flushdns", "netsh.exe winsock reset", "netsh.exe int ip reset", "Restart-NetAdapter", "ipconfig.exe /renew"] {
            assert!(c.script.contains(p), "falta «{p}»:\n{}", c.script);
        }
        let q = build(&st, "net.repair-quick", None).unwrap();
        assert!(!q.script.contains("winsock reset"));
        assert_eq!(q.measure.as_deref(), Some("ping"));
    }

    #[test]
    fn el_archivo_lleva_cabecera_y_requires() {
        let st = state();
        let c = build(&st, "repair.ip-renew", None).unwrap();
        assert!(c.admin);
        let f = script_file(&c, "20/11/2026");
        assert!(f.starts_with("# Generado por AdminOps "), "{f}");
        assert!(f.contains(" · 20/11/2026\n#Requires -RunAsAdministrator\n"), "{f}");
        let sin = build(&st, "repair.explorer", None).unwrap();
        assert!(!script_file(&sin, "x").contains("#Requires"));
    }

    /// Las acciones que no son del catálogo también tienen comando (con su contexto).
    #[test]
    fn las_acciones_fuera_del_catalogo_tienen_comando() {
        let st = state();
        for id in EXTRA_IDS {
            let ctx = match *id {
                "printer.clear-queue" => Some("HP-Recepción"),
                "startup.disable" => continue, // necesita una entrada real del inicio
                "device.scan" | "device.restart" | "device.enable" => Some(DEV_ID),
                "display.native" => Some("1920x1080"),
                "unsaved.copy" => Some(r"C:\Users\Ana\AppData\Roaming\Microsoft\Excel\Libro1.xar"),
                _ => None,
            };
            let c = build(&st, id, ctx).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(c.script.lines().count() > 3, "{id}");
            println!("{id}: aproximado={} medida={:?}", c.approximate, c.measure);
        }
    }

    /// Lo que se enseña y se guarda como .ps1 es PowerShell que se puede ejecutar: sin errores de sintaxis.
    #[test]
    fn todos_los_comandos_son_powershell_valido() {
        let st = state();
        let mut scripts: Vec<(String, String)> = st.all().iter().map(|t| (t.id.clone(), build(&st, &t.id, None).unwrap().script)).collect();
        for (id, ctx) in [("net.repair-quick", None), ("net.repair-deep", None), ("printer.clear-queue", Some("HP 'Recepción'")), ("restore-point", None), ("device.scan", Some(DEV_ID)), ("device.restart", Some(DEV_ID)), ("device.enable", Some(DEV_ID)), ("display.native", Some("1920x1080")), ("display.extend", None), ("display.clone", None), ("display.restart-driver", None), ("scan.setup", None), ("scan.password", None), ("scan.private", None), ("scan.signing", None), ("scan.shortcut", None), ("onedrive.pause", None), ("kbd.only-spanish", None), ("unsaved.copy", Some(r"C:\Users\Ana\AppData\Roaming\Microsoft\Excel\Libro1.xar"))] {
            scripts.push((id.into(), build(&st, id, ctx).unwrap().script));
        }
        for (id, script) in scripts {
            let errors = crate::ps::parse_errors(&script);
            assert!(errors.trim().is_empty(), "{id}: {errors}");
        }
    }

    #[test]
    fn id_desconocido_da_error() {
        assert!(build(&state(), "no.existe", None).is_err());
    }
}
