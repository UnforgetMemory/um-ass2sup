/// Manual FFI bindings for libass v0.17.
///
/// These declarations mirror the C API of libass 0.17.x and are intended
/// for internal use by higher-level safe wrappers. All structs use
/// `#[repr(C)]` to guarantee layout compatibility with the C library.
use std::ffi::c_void;

/// Opaque handle to an [`ASS_Library`] instance.
///
/// This is a zero-sized type used only as a typed pointer target.
#[repr(C)]
pub struct ASS_Library {
    _p: [u8; 0],
}

/// Opaque handle to an [`ASS_Renderer`] instance.
///
/// This is a zero-sized type used only as a typed pointer target.
#[repr(C)]
pub struct ASS_Renderer {
    _p: [u8; 0],
}

/// Opaque handle to an [`ASS_Style`] instance.
///
/// This is a zero-sized type used only as a typed pointer target.
#[repr(C)]
pub struct ASS_Style {
    _p: [u8; 0],
}

/// Image type produced by libass rendering.
///
/// Mirrors the `enum` used by libass to classify rendered bitmaps.
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum ImageType {
    /// A filled character glyph.
    Character = 0,
    /// An outline stroke.
    Outline = 1,
    /// A drop shadow.
    Shadow = 2,
}

/// A rendered image bitmap returned by [`ass_render_frame`].
///
/// The memory backing `bitmap` is owned by libass and is valid until the
/// next call to [`ass_render_frame`] on the same track.
#[repr(C)]
pub struct ASS_Image {
    /// Bitmap width in pixels.
    pub w: i32,
    /// Bitmap height in pixels.
    pub h: i32,
    /// Stride (bytes per row) of the bitmap buffer.
    pub stride: i32,
    /// Pointer to the 8-bit alpha bitmap data.
    pub bitmap: *mut u8,
    /// Packed color in `0xAABBGGRR` format.
    pub color: u32,
    /// Destination X offset within the frame.
    pub dst_x: i32,
    /// Destination Y offset within the frame.
    pub dst_y: i32,
    /// Pointer to the next image in the linked list.
    pub next: *mut ASS_Image,
    /// Classification of this image.
    pub image_type: ImageType,
}

/// An event (subtitle line) inside an [`ASS_Track`].
///
/// Mirrors the `ASS_Event` struct from libass 0.17.
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
///
/// Mirrors the `ASS_Track` struct from libass 0.17. The layout must match
/// the C definition exactly because it is accessed directly from Rust.
#[repr(C)]
pub struct ASS_Track {
    /// Number of allocated styles.
    pub n_styles: i32,
    /// Maximum number of styles that can be stored without reallocation.
    pub max_styles: i32,
    /// Number of allocated events.
    pub n_events: i32,
    /// Maximum number of events that can be stored without reallocation.
    pub max_events: i32,
    /// Array of style definitions.
    pub styles: *mut c_void,
    /// Array of event definitions.
    pub events: *mut ASS_Event,
    /// Style format string (NUL-terminated).
    pub style_format: *mut i8,
    /// Event format string (NUL-terminated).
    pub event_format: *mut i8,
    /// Track type (`ASS_TYPE_UNKNOWN`, `ASS_TYPE_SSA`, `ASS_TYPE_ASS`).
    pub track_type: i32,
    /// Playback resolution X (from script header).
    pub play_res_x: i32,
    /// Playback resolution Y (from script header).
    pub play_res_y: i32,
    /// Timer speed multiplier.
    pub timer: f64,
    /// Wrap style (`ASS_WRAP_*` constants).
    pub wrap_style: i32,
    /// Whether border and shadow are scaled with font scale.
    pub scaled_border_and_shadow: i32,
    /// Whether kerning is enabled.
    pub kerning: i32,
    /// Language code (NUL-terminated).
    pub language: *mut i8,
    /// YCbCr matrix (`YCBCR_*` constants).
    pub ycbcr_matrix: i32,
    /// Index of the default style.
    pub default_style: i32,
    /// Track name (NUL-terminated).
    pub name: *mut i8,
    /// Back-pointer to the owning [`ASS_Library`].
    pub library: *mut c_void,
    /// Private parser data.
    pub parser_priv: *mut c_void,
    /// Layout resolution X (may differ from `play_res_x`).
    pub layout_res_x: i32,
    /// Layout resolution Y (may differ from `play_res_y`).
    pub layout_res_y: i32,
}

extern "C" {
    /// Initialise a new libass library instance.
    ///
    /// Returns a pointer to an opaque [`ASS_Library`], or null on failure.
    pub fn ass_library_init() -> *mut ASS_Library;

    /// Release all resources held by a library instance.
    ///
    /// # Safety
    ///
    /// `lib` must be a valid pointer returned by [`ass_library_init`], or null.
    pub fn ass_library_done(lib: *mut ASS_Library);

    /// Initialise a renderer for a given library.
    ///
    /// Returns a pointer to an opaque [`ASS_Renderer`], or null on failure.
    ///
    /// # Safety
    ///
    /// `lib` must be a valid pointer returned by [`ass_library_init`].
    pub fn ass_renderer_init(lib: *mut ASS_Library) -> *mut ASS_Renderer;

    /// Release all resources held by a renderer.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`], or null.
    pub fn ass_renderer_done(renderer: *mut ASS_Renderer);

    /// Set the output frame size for the renderer.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`].
    pub fn ass_set_frame_size(renderer: *mut ASS_Renderer, w: i32, h: i32);

    /// Set the storage (video) size used for layout calculations.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`].
    pub fn ass_set_storage_size(renderer: *mut ASS_Renderer, w: i32, h: i32);

    /// Configure fonts for the renderer.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`].
    /// `default_font` and `default_family` must be valid C strings or null.
    /// `config` must be a valid C string or null.
    pub fn ass_set_fonts(
        renderer: *mut ASS_Renderer,
        default_font: *const i8,
        default_family: *const i8,
        dfp: i32,
        config: *const i8,
        update: i32,
    );

    /// Set an additional fonts directory for font discovery.
    ///
    /// # Safety
    ///
    /// `lib` must be a valid pointer returned by [`ass_library_init`].
    /// `fonts_dir` must be a valid C string or null.
    pub fn ass_set_fonts_dir(lib: *mut ASS_Library, fonts_dir: *const i8);

    /// Set the hinting mode for the renderer.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`].
    pub fn ass_set_hinting(renderer: *mut ASS_Renderer, hinting: i32);

    /// Set a global font scale factor.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`].
    pub fn ass_set_font_scale(renderer: *mut ASS_Renderer, font_scale: f64);

    /// Set cache limits for the renderer.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`].
    pub fn ass_set_cache_limits(renderer: *mut ASS_Renderer, glyph_max: i32, bitmap_max_size: i32);

    /// Render a single frame for `now` milliseconds.
    ///
    /// Returns a linked list of [`ASS_Image`] bitmaps, or null if nothing
    /// should be rendered. The returned images are owned by libass and are
    /// valid until the next call to this function on the same track.
    ///
    /// # Safety
    ///
    /// `renderer` must be a valid pointer returned by [`ass_renderer_init`].
    /// `track` must be a valid pointer returned by [`ass_read_memory`] or null.
    /// `detect_change` must be a valid pointer to an `i32` or null.
    pub fn ass_render_frame(
        renderer: *mut ASS_Renderer,
        track: *mut ASS_Track,
        now: i64,
        detect_change: *mut i32,
    ) -> *mut ASS_Image;

    /// Parse ASS subtitle data from memory.
    ///
    /// Returns a pointer to an [`ASS_Track`], or null on failure.
    ///
    /// # Safety
    ///
    /// `lib` must be a valid pointer returned by [`ass_library_init`].
    /// `buf` must be valid for reads of `bufsize` bytes.
    /// `codepage` must be a valid C string or null.
    pub fn ass_read_memory(
        lib: *mut ASS_Library,
        buf: *const i8,
        bufsize: usize,
        codepage: *const i8,
    ) -> *mut ASS_Track;

    /// Free all resources associated with a track.
    ///
    /// # Safety
    ///
    /// `track` must be a valid pointer returned by [`ass_read_memory`], or null.
    pub fn ass_free_track(track: *mut ASS_Track);

    /// Return the libass version as an encoded integer.
    ///
    /// The value is `0xMMmmrr` where `MM` is the major version, `mm` the
    /// minor version, and `rr` the revision.
    pub fn ass_library_version() -> i32;

    /// Enable or disable embedded font extraction.
    ///
    /// # Safety
    ///
    /// `lib` must be a valid pointer returned by [`ass_library_init`].
    pub fn ass_set_extract_fonts(lib: *mut ASS_Library, extract: i32);

    /// Add a font from raw binary data to the library's font cache.
    ///
    /// The `name` parameter is a user-chosen label (not the font family name);
    /// it is used for deduplication. `data` must be valid font data (TTF, OTF,
    /// TTC, OTC, WOFF, WOFF2).
    ///
    /// # Safety
    ///
    /// `library` must be a valid pointer returned by [`ass_library_init`].
    /// `name` must be a valid C string.
    /// `data` must be valid for reads of `data_size` bytes.
    pub fn ass_add_font(
        library: *mut ASS_Library,
        name: *const i8,
        data: *const i8,
        data_size: i32,
    );

    /// Set a log message callback.
    ///
    /// # Safety
    ///
    /// `lib` must be a valid pointer returned by [`ass_library_init`].
    /// `msg_cb` must be a valid function pointer or null.
    /// `data` is passed through to the callback and must outlive the callback.
    pub fn ass_set_message_cb(
        lib: *mut ASS_Library,
        msg_cb: Option<unsafe extern "C" fn(i32, *const i8, *mut i8, *mut i8)>,
        data: *mut i8,
    );

    /// Allocate a new event in a track.
    ///
    /// Returns the index of the newly allocated event, or a negative value on failure.
    ///
    /// # Safety
    ///
    /// `track` must be a valid pointer returned by [`ass_read_memory`].
    pub fn ass_alloc_event(track: *mut ASS_Track) -> i32;

    /// Advance the track's internal timing.
    ///
    /// Returns the timestamp of the next event after `now`, or -1.
    ///
    /// # Safety
    ///
    /// `track` must be a valid pointer returned by [`ass_read_memory`].
    pub fn ass_step_sub(track: *mut ASS_Track, now: i64, movement: i32) -> i64;

    /// Remove all events from a track.
    ///
    /// # Safety
    ///
    /// `track` must be a valid pointer returned by [`ass_read_memory`].
    pub fn ass_flush_events(track: *mut ASS_Track);
}
