//! «No me ven en Teams»: la cámara. Qué cámaras hay y en qué estado, si Windows deja usarlas (permisos
//! del equipo, de las aplicaciones, de escritorio y de Teams nuevo) y qué programa la tiene ocupada.
//!
//! Windows no tiene «cámara predeterminada»: cada programa elige la suya y casi todos se quedan con la
//! primera. Para que Teams use la buena, lo que funciona es desactivar la que no se usa (la del portátil
//! con la base y una webcam, por ejemplo); se vuelve a activar igual de fácil. La vista previa la pinta la
//! interfaz con la propia cámara.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    pub id: String,
    pub name: String,
    /// ok | disabled | error
    pub state: String,
}

#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CamPrivacy {
    pub device: bool,
    pub apps: bool,
    pub desktop: bool,
    pub teams_new: Option<bool>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CamView {
    pub cameras: Vec<Camera>,
    pub privacy: CamPrivacy,
    /// Programas que la están usando ahora (nombre amable).
    pub in_use: Vec<String>,
    pub admin: bool,
}

const CONSENT: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam";
const TEAMS_PKG: &str = "MSTeams_8wekyb3d8bbwe";

fn user_root() -> String {
    match crate::target_user::hkcu_redirect() {
        Some(sid) => format!(r"HKU\{sid}"),
        None => "HKCU".into(),
    }
}

fn allowed(path: &str) -> Option<bool> {
    crate::tweaks::registry::read_string(path, "Value").map(|v| !v.eq_ignore_ascii_case("Deny"))
}

pub fn privacy() -> CamPrivacy {
    let u = format!(r"{}\{CONSENT}", user_root());
    let teams = format!(r"{u}\{TEAMS_PKG}");
    CamPrivacy {
        device: allowed(&format!(r"HKLM\{CONSENT}")).unwrap_or(true),
        apps: allowed(&u).unwrap_or(true),
        desktop: allowed(&format!(r"{u}\NonPackaged")).unwrap_or(true),
        teams_new: if crate::tweaks::registry::key_exists(&teams) { Some(allowed(&teams).unwrap_or(true)) } else { None },
    }
}

/// «C:#Program Files#Zoom#bin#Zoom.exe» o «MSTeams_8wekyb3d8bbwe» → «Zoom» / «Teams».
pub fn consent_app_name(key: &str) -> String {
    if key.starts_with("MSTeams") {
        return "Teams".into();
    }
    let exe = key.rsplit('#').next().unwrap_or(key);
    let label = crate::rocket::app_label(exe);
    // Paquetes de la tienda: «Microsoft.WindowsCamera_8wekyb3d8bbwe» → «Microsoft.WindowsCamera».
    label.split('_').next().unwrap_or(&label).to_string()
}

/// Quién está usando la cámara ahora: su «terminó» está a cero.
fn in_use() -> Vec<String> {
    use crate::tweaks::registry::{read_raw, subkeys};
    let u = format!(r"{}\{CONSENT}", user_root());
    let mut out = Vec::new();
    let qword = |path: &str, name: &str| read_raw(path, name).and_then(|r| r.bytes.get(..8).map(|b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8]))));
    let mut check = |path: String, key: &str| {
        let start = qword(&path, "LastUsedTimeStart").unwrap_or(0);
        if start > 0 && qword(&path, "LastUsedTimeStop") == Some(0) {
            let n = consent_app_name(key);
            if !out.contains(&n) {
                out.push(n);
            }
        }
    };
    for k in subkeys(&u) {
        if k == "NonPackaged" {
            for d in subkeys(&format!(r"{u}\NonPackaged")) {
                check(format!(r"{u}\NonPackaged\{d}"), &d);
            }
        } else {
            check(format!(r"{u}\{k}"), &k);
        }
    }
    out
}

/// La salida de PowerShell (un objeto o una lista) → cámaras.
pub fn parse_cameras(json: &str) -> Vec<Camera> {
    #[derive(Deserialize)]
    struct Raw {
        id: String,
        name: Option<String>,
        status: Option<String>,
    }
    let v: Vec<Raw> = serde_json::from_str::<Vec<Raw>>(json).or_else(|_| serde_json::from_str::<Raw>(json).map(|r| vec![r])).unwrap_or_default();
    v.into_iter()
        .map(|r| {
            let s = r.status.unwrap_or_default().to_ascii_lowercase();
            let state = if s == "ok" { "ok" } else if s.contains("error") || s.contains("degraded") { "error" } else { "disabled" };
            Camera { id: r.id, name: r.name.unwrap_or_else(|| "Cámara".into()), state: state.into() }
        })
        .collect()
}

fn cameras() -> Vec<Camera> {
    let ps = "@(Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue | Where-Object { $_.Class -in 'Camera','Image' -and $_.FriendlyName -notmatch 'scan|escáner|printer|impresora' }) | ForEach-Object { [pscustomobject]@{ id=$_.InstanceId; name=$_.FriendlyName; status=\"$($_.Status)\" } } | ConvertTo-Json -Compress";
    crate::ps::powershell_opts(ps, crate::ps::Opts { timeout: Some(std::time::Duration::from_secs(45)), task: None }).map(|s| parse_cameras(s.trim())).unwrap_or_default()
}

#[tauri::command(async)]
pub fn cam_view() -> CamView {
    CamView { cameras: cameras(), privacy: privacy(), in_use: in_use(), admin: crate::elevation::is_elevated() }
}

/// Da los permisos de cámara que falten (el del equipo necesita administrador).
#[tauri::command(async)]
pub fn cam_allow(tweaks: tauri::State<'_, crate::tweaks::TweakState>) -> Result<Vec<String>, String> {
    use crate::tweaks::model::{RegData, RegKind};
    use crate::tweaks::registry::write;
    let u = format!(r"{}\{CONSENT}", user_root());
    let p = privacy();
    let allow = |path: &str| write(path, "Value", RegKind::String, &RegData::Str("Allow".into()));
    let mut done = Vec::new();
    if !p.device {
        if !crate::elevation::is_elevated() {
            return Err("La cámara está apagada para todo el equipo: hace falta abrir AdminOps como administrador.".into());
        }
        allow(&format!(r"HKLM\{CONSENT}"))?;
        done.push("Cámara encendida para el equipo".to_string());
    }
    if !p.apps {
        allow(&u)?;
        done.push("Las aplicaciones pueden usar la cámara".to_string());
    }
    if !p.desktop {
        allow(&format!(r"{u}\NonPackaged"))?;
        done.push("Las aplicaciones de escritorio pueden usar la cámara".to_string());
    }
    if p.teams_new == Some(false) {
        allow(&format!(r"{u}\{TEAMS_PKG}"))?;
        done.push("Teams puede usar la cámara".to_string());
    }
    if !done.is_empty() {
        tweaks.record(crate::tweaks::journal::Op::Run, "Permisos de cámara", &Ok::<(), String>(()));
    }
    Ok(done)
}

/// Activa o desactiva una cámara (para que Teams y compañía usen la otra).
#[tauri::command(async)]
pub fn cam_enable(tweaks: tauri::State<'_, crate::tweaks::TweakState>, id: String, enable: bool) -> Result<(), String> {
    if !crate::elevation::is_elevated() {
        return Err("Hace falta abrir AdminOps como administrador.".into());
    }
    // Solo cámaras de verdad: el id tiene que ser uno de los que se listan.
    let cam = cameras().into_iter().find(|c| c.id == id).ok_or("Esa cámara ya no está conectada.")?;
    let verb = if enable { "/enable-device" } else { "/disable-device" };
    let r = crate::ps::exec("pnputil", &[verb, &id]).map(|_| ());
    let title = format!("{} la cámara «{}»", if enable { "Activar" } else { "Desactivar" }, cam.name);
    tweaks.record(crate::tweaks::journal::Op::Run, &title, &r);
    r.map_err(|e| format!("Windows no dejó cambiar la cámara: {}", crate::ps::clean_error(&e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camaras_de_powershell() {
        let one = r#"{"id":"USB\\VID_046D&PID_082D\\8F2B","name":"Logitech C920","status":"OK"}"#;
        assert_eq!(parse_cameras(one), vec![Camera { id: "USB\\VID_046D&PID_082D\\8F2B".into(), name: "Logitech C920".into(), state: "ok".into() }]);
        let many = r#"[{"id":"a","name":"Integrated Camera","status":"Error"},{"id":"b","name":"C920","status":"Unknown"}]"#;
        let v = parse_cameras(many);
        assert_eq!(v[0].state, "error");
        assert_eq!(v[1].state, "disabled");
        assert!(parse_cameras("").is_empty());
    }

    #[test]
    fn nombres_de_quien_la_usa() {
        assert_eq!(consent_app_name("C:#Program Files#Zoom#bin#Zoom.exe"), "Zoom");
        assert_eq!(consent_app_name("MSTeams_8wekyb3d8bbwe"), "Teams");
        assert_eq!(consent_app_name("Microsoft.WindowsCamera_8wekyb3d8bbwe"), "Microsoft.WindowsCamera");
    }
}
