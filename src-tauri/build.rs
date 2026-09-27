fn main() {
    // En release el .exe exige administrador (UAC al abrir). En debug no, para
    // que `tauri dev` pueda lanzarlo desde una terminal sin elevar.
    // ADMINOPS_ASINVOKER=1 compila la versión final sin exigir administrador,
    // solo para medir rendimiento sin UAC. Nunca para distribuir.
    let bench = std::env::var("ADMINOPS_ASINVOKER").as_deref() == Ok("1");
    println!("cargo:rerun-if-env-changed=ADMINOPS_ASINVOKER");
    let manifest = if std::env::var("PROFILE").as_deref() == Ok("release") && !bench {
        include_str!("manifests/admin.manifest")
    } else {
        include_str!("manifests/dev.manifest")
    };
    println!("cargo:rerun-if-changed=manifests");

    // El selector de archivos (tauri-plugin-dialog) importa TaskDialogIndirect,
    // que solo existe en Common Controls v6. El .exe de la app lo activa con su
    // manifiesto, pero los binarios de `cargo test` no tienen manifiesto y no
    // arrancarían (STATUS_ENTRYPOINT_NOT_FOUND). Con carga diferida, comctl32 se
    // resuelve al usarse por primera vez y los tests nunca lo usan.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:comctl32.dll");
        println!("cargo:rustc-link-lib=delayimp");
    }

    let attrs = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(manifest));
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
