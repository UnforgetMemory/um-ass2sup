fn main() {
    // Try pkg-config first (macOS Homebrew, Linux system packages)
    if let Ok(lib) = pkg_config::Config::new().probe("libass") {
        for path in &lib.link_paths {
            println!("cargo:rustc-link-search={}", path.display());
        }
        return;
    }

    // Prefer the local links/ copy (CI checkout with pre-built binary).
    let dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../links");
    if dir.join("libass.so").exists() {
        println!("cargo:rustc-link-search={}", dir.display());
    }
    // Fall back to system default linker path (libass-dev via apt, etc.)
    println!("cargo:rustc-link-lib=dylib=ass");
}
