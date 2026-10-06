//! Incrusta el instalador NSIS de AdminOps (el "clásico") en el ejecutable.
//! La versión es la de src-tauri/tauri.conf.json: compila AdminOps primero.
use std::path::PathBuf;

#[path = "src/fingerprint.rs"]
mod fingerprint;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).parent().unwrap().to_path_buf();
    let conf_path = root.join("src-tauri").join("tauri.conf.json");
    println!("cargo:rerun-if-changed={}", conf_path.display());
    let conf: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&conf_path).unwrap()).unwrap();
    let version = conf["version"].as_str().unwrap().to_string();
    println!("cargo:rustc-env=APP_VERSION={version}");

    let payload = root.join(format!("src-tauri/target/release/bundle/nsis/AdminOps_{version}_x64-setup.exe"));
    println!("cargo:rerun-if-changed={}", payload.display());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("payload.exe");
    // Huella del adminops.exe que lleva dentro: al terminar se comprueba que el
    // instalado es este (la versión no basta: dos builds pueden tener la misma).
    let exe = root.join("src-tauri/target/release/adminops.exe");
    println!("cargo:rerun-if-changed={}", exe.display());
    let hash = std::fs::read(&exe).map(|b| fingerprint::fingerprint(&b));
    println!("cargo:rustc-env=APP_EXE_SHA256={}", hash.unwrap_or_default());
    if payload.exists() {
        std::fs::copy(&payload, &out).unwrap();
    } else if std::env::var("PROFILE").as_deref() == Ok("release") {
        panic!("Falta {}: compila AdminOps antes (npm run build:release)", payload.display());
    } else {
        std::fs::write(&out, b"").unwrap(); // desarrollo: sin instalador incrustado
    }

    // El instalador escribe en Archivos de programa: exige administrador (UAC al abrirlo).
    let manifest = if std::env::var("PROFILE").as_deref() == Ok("release") {
        include_str!("../src-tauri/manifests/admin.manifest")
    } else {
        include_str!("../src-tauri/manifests/dev.manifest")
    };
    // Propiedades del ejecutable (Detalles en Windows): la misma versión, editor y
    // copyright que AdminOps. TAURI_CONFIG se mezcla con installer/tauri.conf.json.
    let merge = serde_json::json!({
        "version": version,
        "productName": "Instalador de AdminOps",
        "bundle": {
            "publisher": conf["bundle"]["publisher"],
            "copyright": conf["bundle"]["copyright"],
        },
    });
    std::env::set_var("TAURI_CONFIG", merge.to_string());

    let attrs = tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(manifest));
    tauri_build::try_build(attrs).expect("tauri-build falló");
}
