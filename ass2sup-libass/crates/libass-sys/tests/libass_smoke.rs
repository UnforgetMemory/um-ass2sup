/// Smoke test for the libass-sys FFI bindings.
///
/// Verifies that the linked libass library is recent enough to provide
/// the expected API surface (version >= 0.17.0).
#[test]
fn libass_version_check() {
    let ver = unsafe { libass_sys::ass_library_version() };
    assert!(ver >= 0x00170000, "libass version too old: {:#x}", ver);
}
