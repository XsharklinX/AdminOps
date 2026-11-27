//! Cuando winget no funciona: en equipos recién formateados o con el Instalador de aplicaciones viejo
//! winget no está, es una versión antigua o su origen está dañado, y entonces fallan «Instalar» y
//! «Actualizar todo». Aquí se dice por qué y se repara solo:
//!
//! 1. Registrar el Instalador de aplicaciones para esta cuenta (si está en el equipo pero no en el
//!    usuario: pasa mucho con cuentas nuevas y con la cuenta que eleva AdminOps).
//! 2. Si sigue sin estar o es viejo: el módulo oficial de Microsoft `Microsoft.WinGet.Client` y su
//!    `Repair-WinGetPackageManager` (instala o actualiza winget con sus dependencias, para todos los
//!    usuarios).
//! 3. Restablecer los orígenes (`winget source reset --force`) y actualizarlos.
//!
//! Luego la interfaz vuelve a intentar lo que había fallado.

use serde::Serialize;
use std::time::Duration;
use tauri::Emitter;

pub const STEP_EVENT: &str = "winget://step";
/// Por debajo de esta versión, winget falla con los manifiestos de hoy.
const MIN_VERSION: (u32, u32) = (1, 10);

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WingetHealth {
    /// ok | missing | old | source | network
    pub state: String,
    pub version: Option<String>,
    /// La frase para el técnico.
    pub detail: String,
}

/// «v1.11.430» → (1, 11).
pub fn parse_version(s: &str) -> Option<(u32, u32)> {
    let v = s.trim().trim_start_matches(['v', 'V']);
    let mut it = v.split('.');
    let major = it.next()?.trim().parse().ok()?;
    let minor = it.next()?.trim().parse().ok()?;
    Some((major, minor))
}

/// Qué le pasa, a partir de la versión y del código de la prueba (`winget show` de un paquete conocido).
pub fn classify(version: Option<&str>, probe: Option<i64>) -> WingetHealth {
    let v = version.map(str::trim).filter(|v| !v.is_empty());
    let mk = |state: &str, detail: String| WingetHealth { state: state.into(), version: v.map(String::from), detail };
    let Some(ver) = v else {
        return mk("missing", "winget no está en este equipo (o no para esta cuenta): falta el Instalador de aplicaciones.".into());
    };
    if parse_version(ver).is_none_or(|p| p < MIN_VERSION) {
        return mk("old", format!("El Instalador de aplicaciones es viejo (winget {ver}): no entiende los paquetes de hoy."));
    }
    match probe.map(|c| c as i32 as u32) {
        Some(0) => mk("ok", format!("winget {ver} funciona.")),
        Some(0x8A15_0007 | 0x8A15_0008) => mk("network", format!("winget {ver} no llega a Internet: revisa la conexión o el proxy.")),
        Some(c) => mk("source", format!("winget {ver} está, pero su origen de paquetes falla (código 0x{c:08X}).")),
        None => mk("source", format!("winget {ver} está, pero no responde.")),
    }
}

fn version() -> Option<String> {
    crate::ps::exec_opts("winget", &["--version"], crate::ps::Opts { timeout: Some(Duration::from_secs(30)), task: None }).ok().map(|s| s.trim().to_string()).filter(|s| s.starts_with('v') || s.starts_with('V'))
}

/// Pide a winget un paquete que siempre está; su código dice si el origen funciona.
fn probe() -> Option<i64> {
    let mut cmd = crate::ps::hidden("winget");
    cmd.args(["show", "--id", "Microsoft.PowerShell", "--exact", "--source", "winget", "--accept-source-agreements", "--disable-interactivity"]);
    crate::ps::stream(cmd, None, crate::ps::Opts { timeout: Some(Duration::from_secs(90)), task: None }, &mut |_| {}).ok().and_then(|s| s.code.map(i64::from))
}

fn health() -> WingetHealth {
    let v = version();
    let p = if v.is_some() { probe() } else { None };
    classify(v.as_deref(), p)
}

#[tauri::command]
pub async fn winget_check() -> Result<WingetHealth, String> {
    tauri::async_runtime::spawn_blocking(health).await.map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Step {
    id: &'static str,
    /// run | ok | fail | skip
    state: &'static str,
    text: String,
}

/// Repara lo que haga falta, paso a paso (`winget://step`), y devuelve cómo queda.
#[tauri::command]
pub async fn winget_repair(app: tauri::AppHandle) -> Result<WingetHealth, String> {
    let r = winget_repair_inner(app.clone()).await;
    crate::tweaks::record_app(&app, "Reparar winget", &r);
    r
}

async fn winget_repair_inner(app: tauri::AppHandle) -> Result<WingetHealth, String> {
    if !crate::elevation::is_elevated() {
        return Err("Para reparar winget hay que abrir AdminOps como administrador.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let step = |id: &'static str, state: &'static str, text: &str| {
            let _ = app.emit(STEP_EVENT, Step { id, state, text: text.into() });
        };
        let mut h = health();
        log::info!("winget antes de reparar: {} ({:?})", h.state, h.version);

        if h.state == "missing" || h.state == "old" {
            step("register", "run", "Registrar el Instalador de aplicaciones para esta cuenta");
            let r = crate::ps::powershell_opts(
                "Add-AppxPackage -RegisterByFamilyName -MainPackage Microsoft.DesktopAppInstaller_8wekyb3d8bbwe -ErrorAction Stop; 'ok'",
                crate::ps::Opts { timeout: Some(Duration::from_secs(120)), task: None },
            );
            h = health();
            step("register", if h.state == "missing" || h.state == "old" { "fail" } else { "ok" }, if r.is_ok() { "Instalador de aplicaciones registrado" } else { "No estaba en el equipo para registrarlo" });
        } else {
            step("register", "skip", "El Instalador de aplicaciones está");
        }

        if h.state == "missing" || h.state == "old" {
            step("update", "run", "Instalar o actualizar el Instalador de aplicaciones (módulo oficial de Microsoft)");
            let script = r#"$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
if (-not (Get-PackageProvider -ListAvailable -Name NuGet -ErrorAction SilentlyContinue | Where-Object { $_.Version -ge [version]'2.8.5.201' })) {
  Install-PackageProvider -Name NuGet -MinimumVersion 2.8.5.201 -Force -Scope AllUsers | Out-Null
}
Install-Module Microsoft.WinGet.Client -Force -AllowClobber -Repository PSGallery -Scope AllUsers -ErrorAction Stop
Import-Module Microsoft.WinGet.Client -ErrorAction Stop
Repair-WinGetPackageManager -Latest -Force -AllUsers -ErrorAction Stop | Out-Null
'ok'"#;
            let r = crate::ps::powershell_opts(script, crate::ps::Opts { timeout: Some(Duration::from_secs(15 * 60)), task: None });
            h = health();
            let ok = !(h.state == "missing" || h.state == "old");
            let text = match (&r, ok) {
                (_, true) => format!("Instalador de aplicaciones al día ({})", h.version.clone().unwrap_or_default()),
                (Err(e), false) => format!("No se pudo: {}", crate::ps::clean_error(e)),
                (Ok(_), false) => "Se instaló, pero winget sigue sin responder: reinicia la sesión y vuelve a probar".into(),
            };
            step("update", if ok { "ok" } else { "fail" }, &text);
            if !ok {
                return Ok(h);
            }
        } else {
            step("update", "skip", "La versión de winget vale");
        }

        if h.state == "network" {
            // Sin Internet (o un proxy) restablecer no arregla nada y quitaría los orígenes propios de la oficina.
            step("sources", "fail", &format!("{} Restablecer los orígenes no lo arreglaría: revisa la conexión.", h.detail));
        } else if h.state != "ok" {
            step("sources", "run", "Actualizar los orígenes");
            let opts = || crate::ps::Opts { timeout: Some(Duration::from_secs(180)), task: None };
            let _ = crate::ps::exec_opts("winget", &["source", "update", "--disable-interactivity"], opts());
            h = health();
            // Solo si actualizar no basta: el restablecimiento deja los de fábrica y quita los añadidos (un
            // repositorio privado de la empresa), así que se dice.
            if h.state != "ok" && h.state != "network" {
                step("sources", "run", "Restablecer los orígenes a los de fábrica (winget y msstore; los añadidos a mano se quitan)");
                let _ = crate::ps::exec_opts("winget", &["source", "reset", "--force", "--disable-interactivity"], opts());
                let _ = crate::ps::exec_opts("winget", &["source", "update", "--disable-interactivity"], opts());
                h = health();
            }
            step("sources", if h.state == "ok" { "ok" } else { "fail" }, &h.detail);
        } else {
            step("sources", "skip", "Los orígenes funcionan");
        }
        log::info!("winget después de reparar: {} ({:?})", h.state, h.version);
        Ok(h)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versiones() {
        assert_eq!(parse_version("v1.11.430"), Some((1, 11)));
        assert_eq!(parse_version("v1.9.25200"), Some((1, 9)));
        assert_eq!(parse_version("hola"), None);
    }

    #[test]
    fn que_le_pasa() {
        assert_eq!(classify(None, None).state, "missing");
        assert_eq!(classify(Some("v1.6.3133"), Some(0)).state, "old");
        assert_eq!(classify(Some("v1.11.430"), Some(0)).state, "ok");
        let s = classify(Some("v1.11.430"), Some(0x8A15_000F_u32 as i32 as i64));
        assert_eq!(s.state, "source");
        assert!(s.detail.contains("0x8A15000F"));
        assert_eq!(classify(Some("v1.12.1"), Some(0x8A15_0008_u32 as i32 as i64)).state, "network");
    }
}
