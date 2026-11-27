fn main() {
    // AdminOps se abre SIN administrador (cualquier usuario del equipo, instalada o portable). El administrador
    // solo se pide al pasar a «Modo técnico» dentro de la app (elevation::relaunch_as_admin, con UAC). Desde la
    // 1.5.2; antes la versión final lo exigía al abrir.
    let manifest = include_str!("manifests/app.manifest");
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
