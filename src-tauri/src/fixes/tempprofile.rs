//! «Iniciaste sesión con un perfil temporal»: de la guía al arreglo. Windows
//! deja el perfil bueno renombrado como «<SID>.bak» en el registro y carga uno
//! vacío. Aquí se localiza, se comprueba que la carpeta del usuario está entera,
//! se guarda una copia de la lista de perfiles y se corrige. Lo que hoy son diez
//! minutos de regedit.

use crate::troubleshoot::{finding, fix_confirm, Finding};
use crate::tweaks::registry;
use std::path::Path;

const LIST: &str = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    /// Nombre de la subclave (con «.bak» si lo lleva).
    pub key: String,
    pub path: String,
    pub state: u32,
    pub ref_count: u32,
    /// La carpeta existe y tiene su NTUSER.DAT.
    pub folder_ok: bool,
}

impl Entry {
    pub fn sid(&self) -> &str {
        self.key.trim_end_matches(".bak")
    }
    pub fn is_bak(&self) -> bool {
        self.key.ends_with(".bak")
    }
    pub fn is_temp(&self) -> bool {
        let p = self.path.to_ascii_uppercase();
        p.ends_with("\\TEMP") || p.contains("\\TEMP.") || p.contains("\\TEMP\\")
    }
    pub fn user(&self) -> String {
        self.path.rsplit('\\').next().unwrap_or(&self.path).to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Hay un «.bak» con el perfil bueno: se pone en su sitio.
    RestoreBak { sid: String, user: String, path: String, replaces_temp: bool },
    /// El perfil bueno está, pero su carpeta falta o está rota: no se toca solo.
    FolderBroken { sid: String, path: String },
}

/// Qué hay que hacer con la lista de perfiles.
pub fn plan(entries: &[Entry]) -> Vec<Plan> {
    let mut out = Vec::new();
    for bak in entries.iter().filter(|e| e.is_bak() && e.sid().starts_with("S-1-5-21-")) {
        let main = entries.iter().find(|e| e.key == bak.sid());
        if !bak.folder_ok {
            out.push(Plan::FolderBroken { sid: bak.sid().to_string(), path: bak.path.clone() });
            continue;
        }
        // Si la principal apunta a una carpeta buena y no temporal, el .bak es un resto viejo.
        if let Some(m) = main {
            if m.folder_ok && !m.is_temp() && m.path.eq_ignore_ascii_case(&bak.path) {
                continue;
            }
            if m.folder_ok && !m.is_temp() {
                continue;
            }
        }
        out.push(Plan::RestoreBak { sid: bak.sid().to_string(), user: bak.user(), path: bak.path.clone(), replaces_temp: main.is_some() });
    }
    out
}

fn folder_ok(path: &str) -> bool {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let p = path.replace("%SystemDrive%", &drive);
    Path::new(&p).join("NTUSER.DAT").metadata().map(|m| m.len() > 0).unwrap_or(false)
}

fn entries() -> Vec<Entry> {
    registry::subkeys(LIST)
        .into_iter()
        .filter(|k| k.starts_with("S-1-5-21-"))
        .map(|k| {
            let base = format!(r"{LIST}\{k}");
            let path = registry::read_string(&base, "ProfileImagePath").unwrap_or_default();
            Entry { folder_ok: folder_ok(&path), state: registry::read_u32(&base, "State").unwrap_or(0), ref_count: registry::read_u32(&base, "RefCount").unwrap_or(0), path, key: k }
        })
        .collect()
}

pub fn findings(entries: &[Entry], current_sid: Option<&str>) -> Vec<Finding> {
    let mut out = Vec::new();
    let current_temp = current_sid.and_then(|sid| entries.iter().find(|e| e.key == sid)).is_some_and(|e| e.is_temp());
    if current_temp {
        out.push(finding("bad", "Esta sesión usa un perfil temporal", "Lo que se guarde en el Escritorio o en Documentos se perderá al cerrar sesión. Más abajo, si se puede, el arreglo."));
    }
    for p in plan(entries) {
        match p {
            Plan::RestoreBak { sid, user, path, replaces_temp } => out.push(
                finding("bad", format!("El perfil de «{user}» está apartado como copia (.bak)"), format!("Windows marcó el perfil bueno ({path}) como copia {}y carga uno vacío. La carpeta del usuario está entera: se puede arreglar. Se guarda antes una copia de la lista de perfiles.", if replaces_temp { "y creó otro temporal " } else { "" }))
                    .fixes(vec![fix_confirm(format!("prof.fix:{sid}"), "Arreglar el perfil", true, "Se guarda una copia de la lista de perfiles y se pone el perfil bueno en su sitio. Después hay que cerrar sesión (o reiniciar) y entrar de nuevo con ese usuario.")]),
            ),
            Plan::FolderBroken { path, .. } => out.push(finding("warn", "El perfil apartado no tiene su carpeta entera", format!("Falta {path}\\NTUSER.DAT o está vacío: no se arregla solo porque podría perderse configuración. Copia los datos de esa carpeta (Datos → Copiar datos) a un perfil nuevo."))),
        }
    }
    for e in entries.iter().filter(|e| !e.is_bak() && e.state != 0 && !e.is_temp()) {
        out.push(finding("info", format!("Perfil de «{}» con marcas de estado", e.user()), format!("Estado {:#x}: Windows lo marcó al fallar una carga anterior. Si el usuario entra bien, no hace falta hacer nada.", e.state)));
    }
    if !out.iter().any(|f| f.level == "bad" || f.level == "warn") {
        out.insert(0, finding("ok", "Los perfiles de usuario están bien", format!("{} perfil(es) en la lista, ninguno apartado ni temporal.", entries.iter().filter(|e| !e.is_bak()).count())));
    }
    out
}

pub fn check() -> Result<Vec<Finding>, String> {
    let sid = crate::target_user::get().map(|t| t.sid.clone());
    Ok(findings(&entries(), sid.as_deref()))
}

fn repair(sid: &str) -> Result<String, String> {
    if !crate::elevation::is_elevated() {
        return Err("Requiere ejecutar AdminOps como administrador.".into());
    }
    if !sid.starts_with("S-1-5-21-") || !sid.chars().all(|c| c.is_ascii_digit() || c == '-' || c == 'S') {
        return Err("Identificador de usuario no válido.".into());
    }
    let all = entries();
    let Some(Plan::RestoreBak { path, .. }) = plan(&all).into_iter().find(|p| matches!(p, Plan::RestoreBak { sid: s, .. } if s == sid)) else {
        return Err("Ese perfil ya no necesita arreglo (vuelve a comprobar).".into());
    };
    let copy = super::reg_backup(LIST, "lista-de-perfiles")?;
    let main = format!(r"{LIST}\{sid}");
    let bak = format!(r"{LIST}\{sid}.bak");
    if registry::key_exists(&main) {
        registry::delete_tree(&main)?;
    }
    crate::ps::exec("reg.exe", &["copy", &bak, &main, "/s", "/f"])?;
    registry::delete_tree(&bak)?;
    use crate::tweaks::model::{RegData, RegKind};
    registry::write(&main, "State", RegKind::Dword, &RegData::Int(0))?;
    registry::write(&main, "RefCount", RegKind::Dword, &RegData::Int(0))?;
    log::info!("Perfil {sid} restaurado ({path}); copia de la lista en {}", copy.display());
    Ok(format!("Perfil arreglado ({path}). Cierra sesión o reinicia y entra de nuevo con ese usuario. Copia de la lista de perfiles en «copias-registro»."))
}

pub fn run(kind: &str, arg: &str) -> Option<Result<String, String>> {
    match kind {
        "prof.fix" => Some(repair(arg)),
        _ => None,
    }
}

pub fn title(kind: &str) -> Option<&'static str> {
    (kind == "prof.fix").then_some("Arreglar un perfil de usuario temporal")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(key: &str, path: &str, ok: bool) -> Entry {
        Entry { key: key.into(), path: path.into(), folder_ok: ok, ..Default::default() }
    }

    #[test]
    fn bak_con_temporal() {
        let list = vec![e("S-1-5-21-1-2-3-1104", r"C:\Users\TEMP", false), e("S-1-5-21-1-2-3-1104.bak", r"C:\Users\maria.perez", true)];
        let p = plan(&list);
        assert_eq!(p, vec![Plan::RestoreBak { sid: "S-1-5-21-1-2-3-1104".into(), user: "maria.perez".into(), path: r"C:\Users\maria.perez".into(), replaces_temp: true }]);
        let f = findings(&list, Some("S-1-5-21-1-2-3-1104"));
        assert_eq!(f[0].level, "bad");
        assert!(f.iter().any(|x| x.fixes.iter().any(|y| y.id == "prof.fix:S-1-5-21-1-2-3-1104")));
    }

    #[test]
    fn solo_bak() {
        let list = vec![e("S-1-5-21-9-1001.bak", r"C:\Users\ana", true)];
        assert!(matches!(plan(&list)[0], Plan::RestoreBak { replaces_temp: false, .. }));
    }

    #[test]
    fn bak_viejo_no_se_toca() {
        let list = vec![e("S-1-5-21-9-1001", r"C:\Users\ana", true), e("S-1-5-21-9-1001.bak", r"C:\Users\ana.old", true)];
        assert!(plan(&list).is_empty());
    }

    #[test]
    fn carpeta_rota_no_se_arregla_sola() {
        let list = vec![e("S-1-5-21-9-1001", r"C:\Users\TEMP", false), e("S-1-5-21-9-1001.bak", r"C:\Users\ana", false)];
        assert!(matches!(plan(&list)[0], Plan::FolderBroken { .. }));
    }

    #[test]
    fn todo_bien() {
        let list = vec![e("S-1-5-21-9-1001", r"C:\Users\ana", true)];
        assert_eq!(findings(&list, Some("S-1-5-21-9-1001"))[0].level, "ok");
    }
}
