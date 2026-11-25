fn main() {
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attributes).expect("tauri build");
    let build_env = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../config/build.env");
    println!("cargo:rerun-if-changed={}", build_env.display());
    for line in std::fs::read_to_string(&build_env).unwrap_or_default().lines() {
        if let Some((key, value)) = line.trim().split_once('=') {
            if key.starts_with("ZIP_") && !value.is_empty() && std::env::var(key).is_err() {
                println!("cargo:rustc-env={key}={value}");
            }
        }
    }
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
}
