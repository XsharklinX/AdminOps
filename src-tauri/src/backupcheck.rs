//! Comprobar que las copias de seguridad sirven. Que exista una copia no basta:
//! tiene que ser reciente, estar en otro disco, cubrir lo importante y poder
//! leerse. Se comprueba cada cosa por separado y se dice cuál falla.
//!
//! - **Reciente**: el archivo más nuevo de la copia.
//! - **En otro disco**: una copia en el mismo disco no protege de que el disco falle.
//! - **Cubre lo importante**: de los archivos más recientes de cada carpeta de origen
//!   (Documentos, Escritorio, Outlook…), cuántos están en la copia con el mismo nombre y tamaño.
//! - **Se puede restaurar**: se lee de verdad un archivo de la copia y, si está también
//!   en el origen, se comprueba que es idéntico.
//!
//! Solo lee. No usa más que `std`, así que se prueba con carpetas de verdad.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DAY: u64 = 86_400;
/// Archivos de origen que se miran por carpeta (los más recientes).
const SAMPLE: usize = 40;
/// Tope de archivos que se recorren y de tiempo, para no colgarse en una copia enorme.
const MAX_FILES: usize = 150_000;
const BUDGET: Duration = Duration::from_secs(25);

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SetCfg {
    pub id: String,
    pub name: String,
    /// Carpeta o unidad donde está la copia.
    pub dest: String,
    /// Carpetas que debería proteger.
    pub sources: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Check {
    pub id: String,
    pub label: String,
    /// ok | warn | bad
    pub level: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Cover {
    pub source: String,
    pub found: u32,
    pub total: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Status {
    pub id: String,
    pub name: String,
    pub dest: String,
    pub checked: u64,
    /// ok | warn | bad
    pub level: String,
    pub newest: Option<u64>,
    pub coverage: Vec<Cover>,
    pub checks: Vec<Check>,
}

struct File {
    path: PathBuf,
    name: String,
    mtime: u64,
    size: u64,
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Archivos sin interés para decir si hay copia: temporales y de sistema.
fn noise(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("~$") || n.starts_with('.') || n == "desktop.ini" || n == "thumbs.db" || n.ends_with(".tmp") || n.ends_with(".lnk")
}

fn walk(dir: &Path, depth: usize, start: Instant, out: &mut Vec<File>) {
    if depth > 8 || out.len() >= MAX_FILES || start.elapsed() > BUDGET {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        if meta.is_dir() {
            if !name.starts_with('$') && !name.eq_ignore_ascii_case("System Volume Information") {
                walk(&e.path(), depth + 1, start, out);
            }
        } else if meta.is_file() && !noise(&name) {
            out.push(File { path: e.path(), name, mtime: meta.modified().map_or(0, secs), size: meta.len() });
        }
        if out.len() >= MAX_FILES {
            return;
        }
    }
}

fn files(dir: &Path) -> Vec<File> {
    let mut v = Vec::new();
    walk(dir, 0, Instant::now(), &mut v);
    v
}

fn hash_file(p: &Path) -> Option<u64> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).ok()?;
    let mut h = DefaultHasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            return Some(h.finish());
        }
        h.write(&buf[..n]);
    }
}

fn check(id: &str, label: &str, level: &str, text: impl Into<String>) -> Check {
    Check { id: id.into(), label: label.into(), level: level.into(), text: text.into() }
}

fn worst(checks: &[Check]) -> &'static str {
    if checks.iter().any(|c| c.level == "bad") {
        "bad"
    } else if checks.iter().any(|c| c.level == "warn") {
        "warn"
    } else {
        "ok"
    }
}

/// Texto de «hace N días».
pub fn ago(days: u64) -> String {
    match days {
        0 => "hoy".into(),
        1 => "ayer".into(),
        n => format!("hace {n} días"),
    }
}

/// Comprueba una copia. `same_disk`: Some(true) si la copia está en el mismo disco físico que el origen.
pub fn check_set(cfg: &SetCfg, now: u64, same_disk: Option<bool>) -> Status {
    let mut st = Status { id: cfg.id.clone(), name: cfg.name.clone(), dest: cfg.dest.clone(), checked: now, ..Default::default() };
    let dest = Path::new(&cfg.dest);
    if cfg.dest.trim().is_empty() || !dest.is_dir() {
        st.checks.push(check("exists", "Destino", "bad", "No se puede abrir el destino de la copia: ¿está conectado el disco o la carpeta de red?"));
        st.level = "bad".into();
        return st;
    }
    let backup = files(dest);
    st.checks.push(check("exists", "Destino", "ok", format!("{} archivos en la copia", backup.len())));

    // Reciente
    match backup.iter().map(|f| f.mtime).max() {
        None => st.checks.push(check("recent", "Última copia", "bad", "La carpeta de la copia está vacía.")),
        Some(t) => {
            st.newest = Some(t);
            let days = now.saturating_sub(t) / DAY;
            let (level, text) = if days <= 7 {
                ("ok", format!("El archivo más nuevo es de {}.", ago(days)))
            } else if days <= 30 {
                ("warn", format!("El archivo más nuevo es de {}: la copia está atrasada.", ago(days)))
            } else {
                ("bad", format!("El archivo más nuevo es de {}: la copia lleva más de un mes sin actualizarse.", ago(days)))
            };
            st.checks.push(check("recent", "Última copia", level, text));
        }
    }

    // En otro disco
    match same_disk {
        Some(true) => st.checks.push(check("disk", "Otro disco", "bad", "La copia está en el mismo disco que los archivos: si el disco falla, se pierden los dos.")),
        Some(false) => st.checks.push(check("disk", "Otro disco", "ok", "La copia está en otro disco.")),
        None => st.checks.push(check("disk", "Otro disco", "warn", "No se pudo saber si está en otro disco.")),
    }

    // Cubre lo importante
    let index: HashMap<(String, u64), u64> = backup.iter().map(|f| ((f.name.to_lowercase(), f.size), f.mtime)).collect();
    let mut sampled: Vec<File> = Vec::new();
    for src in &cfg.sources {
        let p = Path::new(src);
        if !p.is_dir() {
            continue;
        }
        // Dentro de la propia carpeta de la copia no cuenta (una copia dentro del origen).
        let mut all: Vec<File> = files(p).into_iter().filter(|f| !f.path.starts_with(dest)).collect();
        all.sort_by_key(|f| std::cmp::Reverse(f.mtime));
        all.truncate(SAMPLE);
        let found = all.iter().filter(|f| index.contains_key(&(f.name.to_lowercase(), f.size))).count() as u32;
        st.coverage.push(Cover { source: src.clone(), found, total: all.len() as u32 });
        sampled.extend(all);
    }
    if cfg.sources.is_empty() {
        st.checks.push(check("cover", "Qué protege", "warn", "No se ha dicho qué carpetas debería proteger esta copia."));
    }
    for c in &st.coverage {
        let name = Path::new(&c.source).file_name().map_or(c.source.clone(), |n| n.to_string_lossy().into_owned());
        let pct = (c.found * 100).checked_div(c.total).unwrap_or(100);
        let (level, text) = if c.total == 0 {
            ("ok", "Sin archivos que copiar.".to_string())
        } else if pct >= 90 {
            ("ok", format!("{} de {} archivos recientes están en la copia.", c.found, c.total))
        } else if pct >= 60 {
            ("warn", format!("Solo {} de {} archivos recientes están en la copia.", c.found, c.total))
        } else {
            ("bad", format!("Solo {} de {} archivos recientes están en la copia: casi no está protegido.", c.found, c.total))
        };
        st.checks.push(check(&format!("cover:{}", c.source), &name, level, text));
    }

    // Se puede restaurar: el archivo reciente (1 KB–5 MB) de la copia que mejor se puede comparar.
    let mut cands: Vec<&File> = backup.iter().filter(|f| (1024..=5 * 1024 * 1024).contains(&f.size)).collect();
    cands.sort_by_key(|f| std::cmp::Reverse(f.mtime));
    let pick = cands.iter().take(50).find(|f| sampled.iter().any(|s| s.name.eq_ignore_ascii_case(&f.name) && s.size == f.size)).or(cands.first());
    match pick {
        None => st.checks.push(check("restore", "Prueba de restauración", "warn", "No hay un archivo de tamaño razonable con el que probar.")),
        Some(f) => match hash_file(&f.path) {
            None => st.checks.push(check("restore", "Prueba de restauración", "bad", format!("No se pudo leer «{}» de la copia: el archivo está dañado o no se puede abrir.", f.name))),
            Some(h) => {
                let original = sampled.iter().find(|s| s.name.eq_ignore_ascii_case(&f.name) && s.size == f.size);
                match original.and_then(|o| hash_file(&o.path)) {
                    Some(oh) if oh == h => st.checks.push(check("restore", "Prueba de restauración", "ok", format!("«{}» se lee entero y es idéntico al original.", f.name))),
                    Some(_) => st.checks.push(check("restore", "Prueba de restauración", "warn", format!("«{}» se lee entero, pero no es idéntico al original (puede haberse cambiado después de la copia).", f.name))),
                    None => st.checks.push(check("restore", "Prueba de restauración", "ok", format!("«{}» se lee entero (no hay original con el que compararlo).", f.name))),
                }
            }
        },
    }
    st.level = worst(&st.checks).into();
    st
}


fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// La sección «Copias de seguridad» del informe del cliente.
pub fn html(sets: &[Status]) -> String {
    if sets.is_empty() {
        return String::new();
    }
    let mut h = String::from("<h2>Copias de seguridad</h2>");
    for s in sets {
        let cls = match s.level.as_str() {
            "bad" => "v-bad",
            "warn" => "v-warn",
            _ => "v-ok",
        };
        let title = match s.level.as_str() {
            "bad" => "Sin protección fiable",
            "warn" => "Con avisos",
            _ => "Protegido",
        };
        h.push_str(&format!("<div class=\"verdict {cls}\" style=\"margin-top:8px;break-inside:avoid\"><div class=state><div class=t>{} · {}</div><ul class=\"items one\">", esc(&s.name), title));
        for c in &s.checks {
            let li = match c.level.as_str() {
                "ok" => "y",
                "bad" => "x",
                _ => "n",
            };
            h.push_str(&format!("<li class={li}><b>{}</b>: {}</li>", esc(&c.label), esc(&c.text)));
        }
        h.push_str("</ul></div></div>");
    }
    h
}

/// Las carpetas que casi siempre merece la pena proteger, si existen.
pub fn default_sources(home: &Path, local_appdata: Option<&Path>) -> Vec<String> {
    let mut v: Vec<PathBuf> = ["Documents", "Desktop", "Pictures"].iter().map(|d| home.join(d)).collect();
    // Outlook: los .pst suelen estar en Documentos\Archivos de Outlook; los .ost no se copian (se rehacen).
    v.push(home.join("Documents").join("Outlook Files"));
    v.push(home.join("Documents").join("Archivos de Outlook"));
    if let Some(l) = local_appdata {
        v.push(l.join("Microsoft").join("Outlook"));
    }
    v.into_iter().filter(|p| p.is_dir()).map(|p| p.display().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Dir(PathBuf);
    impl Dir {
        fn new(tag: &str) -> Dir {
            let p = std::env::temp_dir().join(format!("adminops-backup-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Dir(p)
        }
        fn put(&self, rel: &str, bytes: usize, age_days: u64, now: u64) -> PathBuf {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, vec![rel.len() as u8; bytes]).unwrap();
            let t = UNIX_EPOCH + Duration::from_secs(now - age_days * DAY);
            fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
            p
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn now() -> u64 {
        secs(SystemTime::now())
    }

    fn cfg(src: &Dir, dest: &Dir) -> SetCfg {
        SetCfg { id: "a".into(), name: "Documentos".into(), dest: dest.0.display().to_string(), sources: vec![src.0.display().to_string()] }
    }

    fn level(s: &Status, id: &str) -> String {
        s.checks.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("falta {id}: {:?}", s.checks)).level.clone()
    }

    #[test]
    fn a_good_backup_passes_everything() {
        let (src, dest) = (Dir::new("s1"), Dir::new("d1"));
        let n = now();
        for i in 0..5 {
            src.put(&format!("Contratos/c{i}.docx"), 2000 + i, 1, n);
            dest.put(&format!("Contratos/c{i}.docx"), 2000 + i, 1, n);
        }
        let s = check_set(&cfg(&src, &dest), n, Some(false));
        assert_eq!(s.level, "ok", "{:?}", s.checks);
        assert_eq!(s.coverage[0], Cover { source: src.0.display().to_string(), found: 5, total: 5 });
        assert!(s.checks.iter().any(|c| c.id == "restore" && c.text.contains("idéntico")));
    }

    #[test]
    fn stale_backup_in_the_same_disk_with_missing_files_fails() {
        let (src, dest) = (Dir::new("s2"), Dir::new("d2"));
        let n = now();
        for i in 0..10 {
            src.put(&format!("f{i}.xlsx"), 3000 + i, 0, n);
        }
        // La copia solo tiene 2 de 10 y es de hace 19 días.
        dest.put("f0.xlsx", 3000, 19, n);
        dest.put("f1.xlsx", 3001, 19, n);
        let s = check_set(&cfg(&src, &dest), n, Some(true));
        assert_eq!(s.level, "bad");
        assert_eq!(level(&s, "recent"), "warn");
        assert_eq!(level(&s, "disk"), "bad");
        assert!(s.checks.iter().any(|c| c.id.starts_with("cover:") && c.level == "bad"), "{:?}", s.checks);
    }

    #[test]
    fn age_levels() {
        let (src, dest) = (Dir::new("s3"), Dir::new("d3"));
        let n = now();
        dest.put("a.txt", 2000, 40, n);
        assert_eq!(level(&check_set(&cfg(&src, &dest), n, Some(false)), "recent"), "bad");
        dest.put("b.txt", 2000, 3, n);
        assert_eq!(level(&check_set(&cfg(&src, &dest), n, Some(false)), "recent"), "ok");
    }

    #[test]
    fn missing_or_empty_destination() {
        let src = Dir::new("s4");
        let s = check_set(&SetCfg { dest: r"Z:\no\existe".into(), sources: vec![src.0.display().to_string()], ..Default::default() }, now(), None);
        assert_eq!(s.level, "bad");
        assert!(s.checks[0].text.contains("No se puede abrir"));
        let dest = Dir::new("d4");
        let e = check_set(&cfg(&src, &dest), now(), Some(false));
        assert_eq!(level(&e, "recent"), "bad");
    }

    #[test]
    fn restore_test_notices_a_changed_original() {
        let (src, dest) = (Dir::new("s5"), Dir::new("d5"));
        let n = now();
        let a = src.put("informe.docx", 4000, 0, n);
        dest.put("informe.docx", 4000, 0, n);
        fs::write(&a, vec![9u8; 4000]).unwrap(); // mismo tamaño, otro contenido
        let s = check_set(&cfg(&src, &dest), n, Some(false));
        let r = s.checks.iter().find(|c| c.id == "restore").unwrap();
        assert_eq!(r.level, "warn");
        assert!(r.text.contains("no es idéntico"));
    }

    #[test]
    fn noise_files_do_not_count_as_a_recent_backup() {
        let (src, dest) = (Dir::new("s6"), Dir::new("d6"));
        let n = now();
        dest.put("desktop.ini", 2000, 0, n);
        dest.put("~$tmp.docx", 2000, 0, n);
        dest.put("real.docx", 2000, 20, n);
        assert_eq!(level(&check_set(&cfg(&src, &dest), n, Some(false)), "recent"), "warn");
    }

    #[test]
    fn a_backup_inside_the_source_is_not_counted_as_the_original() {
        let src = Dir::new("s7");
        let n = now();
        let dest = Dir(src.0.join("copia"));
        fs::create_dir_all(&dest.0).unwrap();
        src.put("x.docx", 2000, 0, n);
        src.put("copia/x.docx", 2000, 0, n);
        let s = check_set(&SetCfg { dest: dest.0.display().to_string(), sources: vec![src.0.display().to_string()], ..Default::default() }, n, Some(true));
        assert_eq!(s.coverage[0].total, 1);
        std::mem::forget(dest);
    }

    #[test]
    fn html_lists_each_check_and_escapes() {
        let st = Status { name: "Copia <A>".into(), level: "warn".into(), checks: vec![check("recent", "Última copia", "warn", "hace 12 días")], ..Default::default() };
        let h = html(&[st]);
        assert!(h.contains("Copia &lt;A&gt; · Con avisos") && h.contains("<li class=n><b>Última copia</b>: hace 12 días</li>"));
        assert!(html(&[]).is_empty());
    }

    #[test]
    fn ago_reads_naturally() {
        assert_eq!((ago(0), ago(1), ago(19)), ("hoy".into(), "ayer".into(), "hace 19 días".into()));
    }
}
