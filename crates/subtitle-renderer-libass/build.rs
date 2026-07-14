fn main() {
    // Ass2sup-core also links against libass via libass-sys.
    // We need to ensure the test targets find libass.so.
    let dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../links");
    if dir.join("libass.so").exists() {
        println!("cargo:rustc-link-search={}", dir.display());
    }
    println!("cargo:rustc-link-lib=dylib=ass");
}
