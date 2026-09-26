//! Apps preinstaladas (paquetes Appx/MSIX): listar, quitar y reinstalar.
//!
//! Solo se puede quitar lo que aparece en el listado y no está protegido; el
//! nombre se valida antes de meterlo en ningún script.

use super::journal::{entry, Backup, Op};
use super::{ensure_restore_point, TweakState};
use crate::ps;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::State;

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Advice {
    Remove,
    Optional,
    Keep,
}

#[derive(Deserialize, Clone, Debug)]
struct KnownApp {
    package: String,
    name: String,
    description: String,
    advice: Advice,
    store_id: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct KnownFile {
    app: Vec<KnownApp>,
}

fn known() -> &'static [KnownApp] {
    static KNOWN: OnceLock<Vec<KnownApp>> = OnceLock::new();
    KNOWN.get_or_init(|| {
        toml::from_str::<KnownFile>(include_str!("../../tweaks/bloatware.toml"))
            .expect("bloatware.toml inválido")
            .app
    })
}

fn find_known(package: &str) -> Option<&'static KnownApp> {
    known().iter().find(|k| k.package.eq_ignore_ascii_case(package))
}

/// Paquetes que nunca se ofrecen para quitar: runtimes, códecs, idiomas, la
/// Store/winget y componentes del shell. Se comparan como prefijo.
const PROTECTED: &[&str] = &[
    "Microsoft.WindowsStore",
    "Microsoft.StorePurchaseApp",
    "Microsoft.DesktopAppInstaller",
    "Microsoft.Winget.",
    "Microsoft.SecHealthUI",
    "Microsoft.VCLibs",
    "Microsoft.UI.Xaml",
    "Microsoft.NET.",
    "Microsoft.WindowsAppRuntime",
    "MicrosoftCorporationII.WinAppRuntime",
    "Microsoft.Services.Store",
    "Microsoft.GamingServices",
    "Microsoft.LanguageExperiencePack",
    "Microsoft.Ink.",
    "Microsoft.MicrosoftEdge",
    "Microsoft.OneDriveSync",
    "Microsoft.ApplicationCompatibilityEnhancements",
    "Microsoft.WindowsTerminal",
    "Microsoft.Windows.",
    "MicrosoftWindows.",
    "Windows.",
    "NearbyShare",
];

/// Heurística adicional para extensiones de códecs/imagen (HEVC, AV1, WebP…).
fn is_protected(name: &str) -> bool {
    if find_known(name).is_some() {
        return false; // lo conocido se evalúa por su `advice`, no por prefijo
    }
    let lower = name.to_ascii_lowercase();
    PROTECTED.iter().any(|p| lower.starts_with(&p.to_ascii_lowercase()))
        || lower.ends_with("extension")
        || lower.ends_with("extensions")
        // GUIDs = apps de sistema sin nombre legible
        || (name.len() == 36 && name.chars().filter(|c| *c == '-').count() == 4)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() < 128 && name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

#[derive(Deserialize)]
struct RawPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Publisher")]
    publisher: String,
    #[serde(rename = "Signature")]
    signature: String,
    #[serde(rename = "Provisioned")]
    provisioned: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    package: String,
    name: String,
    description: Option<String>,
    note: Option<String>,
    /// `None` = app no catalogada ("Otras apps").
    advice: Option<Advice>,
    version: Option<String>,
    publisher: Option<String>,
    installed: bool,
    /// Se instala también para cuentas nuevas (paquete aprovisionado).
    provisioned: bool,
    reinstallable: bool,
}

fn installed_packages() -> Result<Vec<RawPackage>, String> {
    let admin = crate::elevation::is_elevated();
    let script = format!(
        r#"
$admin = ${admin}
$all = if ($admin) {{ Get-AppxPackage -AllUsers }} else {{ Get-AppxPackage }}
$prov = if ($admin) {{ @(Get-AppxProvisionedPackage -Online | ForEach-Object DisplayName) }} else {{ @() }}
$r = @($all | Where-Object {{ -not $_.IsFramework -and -not $_.NonRemovable -and $_.SignatureKind -ne 'System' }} |
  Group-Object Name | ForEach-Object {{
    $p = $_.Group[0]
    [pscustomobject]@{{
      Name = $p.Name; Version = "$($p.Version)"; Signature = "$($p.SignatureKind)"
      Publisher = ($p.Publisher -replace '^CN=([^,]+).*', '$1')
      Provisioned = $prov -contains $p.Name
    }}
  }})
ConvertTo-Json -InputObject $r -Compress
"#
    );
    let out = ps::powershell(&script)?;
    if out.is_empty() {
        return Ok(vec![]);
    }
    serde_json::from_str(&out).map_err(|e| format!("Respuesta inesperada de Get-AppxPackage: {e}"))
}

#[tauri::command(async)]
pub fn list_apps() -> Result<Vec<AppView>, String> {
    let installed = installed_packages()?;
    let mut out: Vec<AppView> = installed
        .iter()
        .filter(|p| !is_protected(&p.name))
        .map(|p| {
            let k = find_known(&p.name);
            AppView {
                package: p.name.clone(),
                name: k.map_or_else(|| p.name.clone(), |k| k.name.clone()),
                description: k.map(|k| k.description.clone()),
                note: k.and_then(|k| k.note.clone()),
                advice: k.map(|k| k.advice),
                version: Some(p.version.clone()),
                publisher: Some(p.publisher.clone()).filter(|s| !s.is_empty()).or(Some(p.signature.clone())),
                installed: true,
                provisioned: p.provisioned,
                reinstallable: k.is_some_and(|k| k.store_id.is_some()),
            }
        })
        .collect();

    // Apps conocidas que no están: se listan para poder reinstalarlas.
    for k in known() {
        if k.store_id.is_some() && !installed.iter().any(|p| p.name.eq_ignore_ascii_case(&k.package)) {
            out.push(AppView {
                package: k.package.clone(),
                name: k.name.clone(),
                description: Some(k.description.clone()),
                note: k.note.clone(),
                advice: Some(k.advice),
                version: None,
                publisher: None,
                installed: false,
                provisioned: false,
                reinstallable: true,
            });
        }
    }
    Ok(out)
}

/// Apps instaladas marcadas como "recomendado quitar" (para el informe).
pub fn recommended_installed() -> Result<Vec<String>, String> {
    Ok(list_apps()?
        .into_iter()
        .filter(|a| a.installed && a.advice == Some(Advice::Remove))
        .map(|a| a.name)
        .collect())
}

fn remove(name: &str) -> Result<(), String> {
    let n = name.replace('\'', "''");
    ps::powershell(&format!(
        r#"
$n = '{n}'
$pk = @(Get-AppxPackage -AllUsers -Name $n)
if ($pk.Count -eq 0) {{ throw 'La app no está instalada' }}
foreach ($p in $pk) {{
  try {{ Remove-AppxPackage -Package $p.PackageFullName -AllUsers }}
  catch {{ Remove-AppxPackage -Package $p.PackageFullName }}
}}
Get-AppxProvisionedPackage -Online | Where-Object DisplayName -eq $n |
  Remove-AppxProvisionedPackage -Online -AllUsers | Out-Null
"#
    ))
    .map(|_| ())
}

/// Reinstala desde Microsoft Store con winget.
pub fn reinstall(name: &str, store_id: &str) -> Result<(), String> {
    ps::exec(
        "winget",
        &[
            "install", "--id", store_id, "--source", "msstore", "--silent",
            "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity",
        ],
    )
    .map(|_| ())
    .map_err(|e| format!("No se pudo reinstalar {name} con winget: {e}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveResult {
    package: String,
    ok: bool,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSummary {
    results: Vec<RemoveResult>,
    restore_point_created: bool,
}

#[tauri::command(async)]
pub fn remove_apps(
    packages: Vec<String>,
    skip_restore_point: bool,
    state: State<'_, TweakState>,
) -> Result<RemoveSummary, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    let installed = installed_packages()?;
    for p in &packages {
        if !valid_name(p) || is_protected(p) || !installed.iter().any(|i| &i.name == p) {
            return Err(format!("No se puede quitar el paquete: {p}"));
        }
    }
    let what = if packages.len() == 1 { packages[0].clone() } else { format!("quitar {} apps", packages.len()) };
    let restore_point_created = !skip_restore_point && ensure_restore_point(&state, &what, None)?;

    let results = packages
        .into_iter()
        .map(|p| {
            let k = find_known(&p);
            let title = format!("Quitar app: {}", k.map_or(p.as_str(), |k| k.name.as_str()));
            let mut e = entry(Op::Apply, None, &title);
            let r = remove(&p);
            match &r {
                Ok(()) => e.backups = vec![Backup::Appx { name: p.clone(), store_id: k.and_then(|k| k.store_id.clone()) }],
                Err(err) => {
                    e.ok = false;
                    e.message = Some(err.clone());
                }
            }
            state.log(e);
            RemoveResult { package: p, ok: r.is_ok(), message: r.err().unwrap_or_else(|| "Quitada".into()) }
        })
        .collect();
    Ok(RemoveSummary { results, restore_point_created })
}

/// Reinstala una app conocida (solo del catálogo, por su `store_id` verificado).
#[tauri::command(async)]
pub fn reinstall_app(package: String, state: State<'_, TweakState>) -> Result<(), String> {
    let k = find_known(&package).ok_or("Solo se pueden reinstalar apps del catálogo")?;
    let id = k.store_id.as_deref().ok_or("Esta app no se puede reinstalar automáticamente")?;
    let mut e = entry(Op::Run, None, &format!("Reinstalar app: {}", k.name));
    let r = reinstall(&k.name, id);
    if let Err(err) = &r {
        e.ok = false;
        e.message = Some(err.clone());
    }
    state.log(e);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_list_parses_and_is_consistent() {
        let k = known();
        assert!(k.len() > 20);
        for a in k {
            assert!(valid_name(&a.package), "{}", a.package);
            assert!(!is_protected(&a.package), "{} no debe estar protegido", a.package);
        }
    }

    #[test]
    fn protects_system_packages() {
        for p in ["Microsoft.WindowsStore", "Microsoft.HEVCVideoExtension", "MicrosoftWindows.CrossDevice", "Microsoft.VCLibs.140.00", "Microsoft.Winget.Source"] {
            assert!(is_protected(p), "{p}");
        }
        assert!(!is_protected("Microsoft.Windows.Photos")); // conocida
        assert!(!is_protected("SnapInc.Snapchat"));
        assert!(!valid_name("x'; Remove-Item C:\\ -Recurse"));
    }

    /// Lista las apps reales del equipo (solo lectura): `cargo test -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn list_real_apps() {
        for a in list_apps().unwrap() {
            println!("{:<9} {:<40} {:?} prov={} reinst={}", a.installed, a.package, a.advice, a.provisioned, a.reinstallable);
        }
    }
}
