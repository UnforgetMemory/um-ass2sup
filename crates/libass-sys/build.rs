fn main() {
    // Prefer the local links/ copy (CI checkout with pre-built binary).
    // Fall back to system libass (e.g. libass9 via apt) when links/ doesn't exist.
    let dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../links");
    if dir.join("libass.so").exists() {
        println!("cargo:rustc-link-search={}", dir.display());
    }
    println!("cargo:rustc-link-lib=dylib=ass");
}
