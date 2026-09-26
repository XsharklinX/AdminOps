fn main() {
    // En release el .exe exige administrador (UAC al abrir). En debug no, para
    // que `tauri dev` pueda lanzarlo desde una terminal sin elevar.
    let manifest = if std::env::var("PROFILE").as_deref() == Ok("release") {
        include_str!("manifests/admin.manifest")
    } else {
        include_str!("manifests/dev.manifest")
    };
    println!("cargo:rerun-if-changed=manifests");

    let attrs = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(manifest));
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
