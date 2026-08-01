use std::ffi::c_void;
use std::sync::OnceLock;

use libloading::Library;

/// Opaque handle to an [`ASS_Library`] instance.
#[repr(C)]
pub struct ASS_Library {
    _p: [u8; 0],
}

/// Opaque handle to an [`ASS_Renderer`] instance.
#[repr(C)]
pub struct ASS_Renderer {
    _p: [u8; 0],
}

/// Opaque handle to an [`ASS_Style`] instance.
#[repr(C)]
pub struct ASS_Style {
    _p: [u8; 0],
}

/// Image type produced by libass rendering.
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum ImageType {
    Character = 0,
    Outline = 1,
    Shadow = 2,
}

/// A rendered image bitmap returned by [`ass_render_frame`].
#[repr(C)]
pub struct ASS_Image {
    pub w: i32,
    pub h: i32,
    pub stride: i32,
    pub bitmap: *mut u8,
    pub color: u32,
    pub dst_x: i32,
    pub dst_y: i32,
    pub next: *mut ASS_Image,
    pub image_type: ImageType,
}

/// An event (subtitle line) inside an [`ASS_Track`].
#[repr(C)]
pub struct ASS_Event {
    pub start: i64,
    pub duration: i64,
    pub read_order: i32,
    pub layer: i32,
    pub style: i32,
    pub name: *mut i8,
    pub margin_l: i32,
    pub margin_r: i32,
    pub margin_v: i32,
    pub effect: *mut i8,
    pub text: *mut i8,
    pub render_priv: *mut c_void,
}

/// Parsed ASS subtitle track.
#[repr(C)]
pub struct ASS_Track {
    pub n_styles: i32,
    pub max_styles: i32,
    pub n_events: i32,
    pub max_events: i32,
    pub styles: *mut c_void,
    pub events: *mut ASS_Event,
    pub style_format: *mut i8,
    pub event_format: *mut i8,
    pub track_type: i32,
    pub play_res_x: i32,
    pub play_res_y: i32,
    pub timer: f64,
    pub wrap_style: i32,
    pub scaled_border_and_shadow: i32,
    pub kerning: i32,
    pub language: *mut i8,
    pub ycbcr_matrix: i32,
    pub default_style: i32,
    pub name: *mut i8,
    pub library: *mut c_void,
    pub parser_priv: *mut c_void,
    pub layout_res_x: i32,
    pub layout_res_y: i32,
}

// ---------------------------------------------------------------------------
// Runtime loader — libass is loaded via libloading so the binary starts
// without the native library present.  Only --backend libass path triggers
// loading, producing a clear error when the library is unavailable.
// ---------------------------------------------------------------------------

/// Error returned when libass cannot be loaded at runtime.
#[derive(Debug)]
pub struct LoadingError(pub String);

/// Runtime-loaded function pointers for the libass C API (v0.17).
///
/// Created once via [`Libass::global()`] and cached for the lifetime of the
/// process.  All function pointers use the same signatures as the original
/// `extern "C"` block.
pub struct Libass {
    #[allow(dead_code)]
    _lib: Library,
    pub ass_library_init: unsafe extern "C" fn() -> *mut ASS_Library,
    pub ass_library_done: unsafe extern "C" fn(*mut ASS_Library),
    pub ass_renderer_init: unsafe extern "C" fn(*mut ASS_Library) -> *mut ASS_Renderer,
    pub ass_renderer_done: unsafe extern "C" fn(*mut ASS_Renderer),
    pub ass_set_frame_size: unsafe extern "C" fn(*mut ASS_Renderer, i32, i32),
    pub ass_set_storage_size: unsafe extern "C" fn(*mut ASS_Renderer, i32, i32),
    pub ass_set_fonts:
        unsafe extern "C" fn(*mut ASS_Renderer, *const i8, *const i8, i32, *const i8, i32),
    pub ass_set_fonts_dir: unsafe extern "C" fn(*mut ASS_Library, *const i8),
    pub ass_set_hinting: unsafe extern "C" fn(*mut ASS_Renderer, i32),
    pub ass_set_font_scale: unsafe extern "C" fn(*mut ASS_Renderer, f64),
    pub ass_set_cache_limits: unsafe extern "C" fn(*mut ASS_Renderer, i32, i32),
    pub ass_render_frame:
        unsafe extern "C" fn(*mut ASS_Renderer, *mut ASS_Track, i64, *mut i32) -> *mut ASS_Image,
    pub ass_read_memory:
        unsafe extern "C" fn(*mut ASS_Library, *const i8, usize, *const i8) -> *mut ASS_Track,
    pub ass_free_track: unsafe extern "C" fn(*mut ASS_Track),
    pub ass_library_version: unsafe extern "C" fn() -> i32,
    pub ass_set_extract_fonts: unsafe extern "C" fn(*mut ASS_Library, i32),
    pub ass_set_message_cb: unsafe extern "C" fn(
        *mut ASS_Library,
        Option<unsafe extern "C" fn(i32, *const i8, *mut i8, *mut i8)>,
        *mut i8,
    ),
    pub ass_alloc_event: unsafe extern "C" fn(*mut ASS_Track) -> i32,
    pub ass_step_sub: unsafe extern "C" fn(*mut ASS_Track, i64, i32) -> i64,
    pub ass_flush_events: unsafe extern "C" fn(*mut ASS_Track),
    pub ass_add_font: unsafe extern "C" fn(*mut ASS_Library, *const i8, *const i8, i32),
}

// Libass is Send + Sync because libloading::Library is Send + Sync on all
// supported platforms, and function pointers are trivially Send + Sync.
unsafe impl Send for Libass {}
unsafe impl Sync for Libass {}

/// Return a list of possible library names to try.
/// The first successful load wins. On Windows, vcpkg may install the DLL
/// with a version suffix (e.g. `ass-9.dll`) rather than the bare `ass.dll`.
fn library_names() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["ass", "ass-9"]
    }
    #[cfg(target_os = "macos")]
    {
        &["libass.dylib"]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        &["libass.so"]
    }
}

impl Libass {
    /// Load the libass shared library and resolve all function pointers.
    fn load() -> Result<Self, LoadingError> {
        let names = library_names();
        let lib = (|| -> Result<_, LoadingError> {
            let mut last_err = String::from("no library names to try");
            for name in names {
                match unsafe { Library::new(name) } {
                    Ok(lib) => return Ok(lib),
                    Err(e) => last_err = format!("cannot load {name}: {e}"),
                }
            }
            Err(LoadingError(last_err))
        })()?;

        let ass_library_init: unsafe extern "C" fn() -> *mut ASS_Library = unsafe {
            *lib.get(b"ass_library_init\0")
                .map_err(|e| LoadingError(format!("cannot find ass_library_init: {e}")))?
        };
        let ass_library_done: unsafe extern "C" fn(*mut ASS_Library) = unsafe {
            *lib.get(b"ass_library_done\0")
                .map_err(|e| LoadingError(format!("cannot find ass_library_done: {e}")))?
        };
        let ass_renderer_init: unsafe extern "C" fn(*mut ASS_Library) -> *mut ASS_Renderer = unsafe {
            *lib.get(b"ass_renderer_init\0")
                .map_err(|e| LoadingError(format!("cannot find ass_renderer_init: {e}")))?
        };
        let ass_renderer_done: unsafe extern "C" fn(*mut ASS_Renderer) = unsafe {
            *lib.get(b"ass_renderer_done\0")
                .map_err(|e| LoadingError(format!("cannot find ass_renderer_done: {e}")))?
        };
        let ass_set_frame_size: unsafe extern "C" fn(*mut ASS_Renderer, i32, i32) = unsafe {
            *lib.get(b"ass_set_frame_size\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_frame_size: {e}")))?
        };
        let ass_set_storage_size: unsafe extern "C" fn(*mut ASS_Renderer, i32, i32) = unsafe {
            *lib.get(b"ass_set_storage_size\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_storage_size: {e}")))?
        };
        let ass_set_fonts: unsafe extern "C" fn(
            *mut ASS_Renderer,
            *const i8,
            *const i8,
            i32,
            *const i8,
            i32,
        ) = unsafe {
            *lib.get(b"ass_set_fonts\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_fonts: {e}")))?
        };
        let ass_set_fonts_dir: unsafe extern "C" fn(*mut ASS_Library, *const i8) = unsafe {
            *lib.get(b"ass_set_fonts_dir\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_fonts_dir: {e}")))?
        };
        let ass_set_hinting: unsafe extern "C" fn(*mut ASS_Renderer, i32) = unsafe {
            *lib.get(b"ass_set_hinting\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_hinting: {e}")))?
        };
        let ass_set_font_scale: unsafe extern "C" fn(*mut ASS_Renderer, f64) = unsafe {
            *lib.get(b"ass_set_font_scale\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_font_scale: {e}")))?
        };
        let ass_set_cache_limits: unsafe extern "C" fn(*mut ASS_Renderer, i32, i32) = unsafe {
            *lib.get(b"ass_set_cache_limits\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_cache_limits: {e}")))?
        };
        let ass_render_frame: unsafe extern "C" fn(
            *mut ASS_Renderer,
            *mut ASS_Track,
            i64,
            *mut i32,
        ) -> *mut ASS_Image = unsafe {
            *lib.get(b"ass_render_frame\0")
                .map_err(|e| LoadingError(format!("cannot find ass_render_frame: {e}")))?
        };
        let ass_read_memory: unsafe extern "C" fn(
            *mut ASS_Library,
            *const i8,
            usize,
            *const i8,
        ) -> *mut ASS_Track = unsafe {
            *lib.get(b"ass_read_memory\0")
                .map_err(|e| LoadingError(format!("cannot find ass_read_memory: {e}")))?
        };
        let ass_free_track: unsafe extern "C" fn(*mut ASS_Track) = unsafe {
            *lib.get(b"ass_free_track\0")
                .map_err(|e| LoadingError(format!("cannot find ass_free_track: {e}")))?
        };
        let ass_library_version: unsafe extern "C" fn() -> i32 = unsafe {
            *lib.get(b"ass_library_version\0")
                .map_err(|e| LoadingError(format!("cannot find ass_library_version: {e}")))?
        };
        let ass_set_extract_fonts: unsafe extern "C" fn(*mut ASS_Library, i32) = unsafe {
            *lib.get(b"ass_set_extract_fonts\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_extract_fonts: {e}")))?
        };
        let ass_set_message_cb: unsafe extern "C" fn(
            *mut ASS_Library,
            Option<unsafe extern "C" fn(i32, *const i8, *mut i8, *mut i8)>,
            *mut i8,
        ) = unsafe {
            *lib.get(b"ass_set_message_cb\0")
                .map_err(|e| LoadingError(format!("cannot find ass_set_message_cb: {e}")))?
        };
        let ass_alloc_event: unsafe extern "C" fn(*mut ASS_Track) -> i32 = unsafe {
            *lib.get(b"ass_alloc_event\0")
                .map_err(|e| LoadingError(format!("cannot find ass_alloc_event: {e}")))?
        };
        let ass_step_sub: unsafe extern "C" fn(*mut ASS_Track, i64, i32) -> i64 = unsafe {
            *lib.get(b"ass_step_sub\0")
                .map_err(|e| LoadingError(format!("cannot find ass_step_sub: {e}")))?
        };
        let ass_flush_events: unsafe extern "C" fn(*mut ASS_Track) = unsafe {
            *lib.get(b"ass_flush_events\0")
                .map_err(|e| LoadingError(format!("cannot find ass_flush_events: {e}")))?
        };
        let ass_add_font: unsafe extern "C" fn(*mut ASS_Library, *const i8, *const i8, i32) = unsafe {
            *lib.get(b"ass_add_font\0")
                .map_err(|e| LoadingError(format!("cannot find ass_add_font: {e}")))?
        };

        Ok(Self {
            _lib: lib,
            ass_library_init,
            ass_library_done,
            ass_renderer_init,
            ass_renderer_done,
            ass_set_frame_size,
            ass_set_storage_size,
            ass_set_fonts,
            ass_set_fonts_dir,
            ass_set_hinting,
            ass_set_font_scale,
            ass_set_cache_limits,
            ass_render_frame,
            ass_read_memory,
            ass_free_track,
            ass_library_version,
            ass_set_extract_fonts,
            ass_set_message_cb,
            ass_alloc_event,
            ass_step_sub,
            ass_flush_events,
            ass_add_font,
        })
    }

    /// Return a reference to the global `Libass` instance, loading the shared
    /// library on first access.
    ///
    /// This is safe to call multiple times — the library is loaded once and
    /// cached in a `OnceLock` for the process lifetime.
    pub fn global() -> Result<&'static Self, &'static LoadingError> {
        static INSTANCE: OnceLock<Result<Libass, LoadingError>> = OnceLock::new();
        match INSTANCE.get_or_init(Self::load) {
            Ok(libass) => Ok(libass),
            Err(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// CRT functions — vsnprintf is resolved at runtime (never via raw-dylib /
// static IAT imports). Linking vsnprintf into the EXE's import table is what
// made the Windows binary crash silently at startup (or on first call): the
// `#[link(kind = "raw-dylib")]` form produces `__imp_`-style references that
// do not reliably match ucrtbase.dll's export table. Loading it via libloading
// at runtime mirrors how [`Libass`] loads ass.dll, so the process always
// starts and formatting degrades gracefully if the symbol is unavailable.
// ---------------------------------------------------------------------------

/// Signature of `vsnprintf(char *s, size_t n, const char *fmt, va_list ap)`.
pub type VsnprintfFn =
    unsafe extern "C" fn(s: *mut i8, n: usize, format: *const i8, ap: *mut i8) -> i32;

/// Runtime-resolved C runtime functions needed by renderer callbacks.
///
/// Unlike [`Libass`], loading these is *best-effort*: the process must keep
/// starting even when a symbol cannot be resolved (e.g. exotic CRT layouts),
/// so callers receive `None` and degrade gracefully instead of crashing.
#[derive(Clone, Copy)]
pub struct CrtFunctions {
    /// `vsnprintf` for formatting libass's printf-style log messages.
    pub vsnprintf: VsnprintfFn,
}

impl CrtFunctions {
    fn load() -> Result<Self, LoadingError> {
        #[cfg(target_os = "windows")]
        {
            // Try (DLL, symbol) pairs in order of likelihood:
            //   1. ucrtbase.dll / "vsnprintf"   — modern UCRT (Win10 1809+)
            //   2. msvcrt.dll   / "_vsnprintf"  — legacy CRT (underscore form)
            // Some UCRT builds only export `__stdio_common_vsprintf` (an internal
            // helper) rather than `vsnprintf` itself, so we also probe that as a
            // last resort — see rust-lang raw-dylib startup-crash reports where
            // a non-exported IAT symbol aborts the process before main().
            let candidates: &[(&str, &[u8])] = &[
                ("ucrtbase.dll", b"vsnprintf\0"),
                ("msvcrt.dll", b"_vsnprintf\0"),
                ("ucrtbase.dll", b"__stdio_common_vsprintf\0"),
            ];
            let mut last_err = String::from("no CRT library names to try");
            for &(name, sym) in candidates {
                let lib = match unsafe { Library::new(name) } {
                    Ok(l) => l,
                    Err(e) => {
                        last_err = format!("cannot load {name}: {e}");
                        continue;
                    }
                };
                // SAFETY: symbol is a C function pointer with the declared signature.
                match unsafe { lib.get::<VsnprintfFn>(sym) } {
                    Ok(s) => {
                        let vsnprintf = *s;
                        // Keep the library handle alive for the process lifetime.
                        std::mem::forget(lib);
                        return Ok(Self { vsnprintf });
                    }
                    Err(e) => {
                        let sym_name =
                            String::from_utf8_lossy(sym.strip_suffix(&[0]).unwrap_or(sym));
                        last_err = format!("cannot find {sym_name} in {name}: {e}");
                    }
                }
            }
            Err(LoadingError(last_err))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let lib = unsafe { Library::new("libc.so.6") }
                .or_else(|_| unsafe { Library::new("libc.so") })
                .map_err(|e| LoadingError(format!("cannot load libc: {e}")))?;
            // SAFETY: symbol is a C function pointer with the declared signature.
            let vsnprintf: libloading::Symbol<VsnprintfFn> = unsafe {
                lib.get(b"vsnprintf\0")
                    .map_err(|e| LoadingError(format!("cannot find vsnprintf: {e}")))?
            };
            let vsnprintf = *vsnprintf;
            // Keep the library handle alive for the process lifetime.
            std::mem::forget(lib);
            Ok(Self { vsnprintf })
        }
    }

    /// Return the runtime-resolved CRT functions.
    ///
    /// Returns `None` when the symbol cannot be resolved — callers must
    /// degrade gracefully (log by level only) rather than crash.
    pub fn global() -> Option<&'static Self> {
        static INSTANCE: OnceLock<Option<CrtFunctions>> = OnceLock::new();
        INSTANCE.get_or_init(|| CrtFunctions::load().ok()).as_ref()
    }
}
