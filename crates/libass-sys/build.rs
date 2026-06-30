fn main() {
    // Link libass via the local links/ copy that has a proper libass.so symlink.
    // This approach propagates correctly to dependents (unlike -l:libass.so.9).
    let dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../links");
    println!("cargo:rustc-link-search={}", dir.display());
    println!("cargo:rustc-link-lib=dylib=ass");
}
