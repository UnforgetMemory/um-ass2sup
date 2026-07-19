fn main() {
    // pkg-config is needed for tests that link against libass directly.
    if let Ok(lib) = pkg_config::Config::new().probe("libass") {
        for path in &lib.link_paths {
            println!("cargo:rustc-link-search={}", path.display());
        }
        return;
    }

    let dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../links");
    if dir.join("libass.so").exists() {
        println!("cargo:rustc-link-search={}", dir.display());
    }
    // libass is loaded at runtime via libloading — no compile-time link needed.
}
