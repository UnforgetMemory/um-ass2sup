//! Rendering bridge: libass track management and frame rasterization.

use std::ffi::CString;
use std::ptr;

use libass_sys;

use crate::domain::error::AssError;
use crate::domain::frame::{AssEventInfo, AssImageData, ImageType};

/// Cast any `*const T` to `*const i8` for libass FFI.
///
/// On x86_64/macOS `c_char = i8` → T is often already i8 (no-op at runtime).
/// On aarch64/Windows `c_char = u8` → required type conversion for `CString::as_ptr()` etc.
fn cast_ptr_to_i8<T>(p: *const T) -> *const i8 {
    p as *const i8
}

/// libass log callback — third arg is a `va_list`, not a string, so we log
/// by level only.  The actual message content is lost, but level-based
/// filtering correctly suppresses INFO-level font-select noise while still
/// surfacing WARN/ERROR to the user.
extern "C" fn libass_log_callback(level: i32, _fmt: *const i8, _va: *mut i8, _data: *mut i8) {
    match level {
        0 | 1 => tracing::error!("[libass] libass error"),
        2 => tracing::warn!("[libass] libass warning"),
        3 => tracing::debug!("[libass] libass info"),
        _ => tracing::trace!("[libass] libass debug"),
    }
}

/// Safe Rust wrapper around libass lifecycle.
///
/// Manages `ASS_Library`, `ASS_Renderer`, and `ASS_Track` handles with
/// correct Drop ordering (track → renderer → library).
pub struct AssRenderer {
    libass: &'static libass_sys::Libass,
    library: *mut libass_sys::ASS_Library,
    renderer: *mut libass_sys::ASS_Renderer,
    track: *mut libass_sys::ASS_Track,
    width: u32,
    height: u32,
    fonts_configured: bool,
}

// libass handles are thread-safe (internally mutex-protected)
unsafe impl Send for AssRenderer {}
unsafe impl Sync for AssRenderer {}

impl AssRenderer {
    /// Create a new libass renderer for the given frame dimensions.
    ///
    /// Initializes `ASS_Library` and `ASS_Renderer`, configures frame size
    /// and storage size, and enables font extraction.
    pub fn new(width: u32, height: u32) -> Result<Self, AssError> {
        let libass = libass_sys::Libass::global().map_err(|_| AssError::InitFailed)?;
        let library = unsafe { (libass.ass_library_init)() };
        if library.is_null() {
            return Err(AssError::InitFailed);
        }

        let renderer = unsafe { (libass.ass_renderer_init)(library) };
        if renderer.is_null() {
            unsafe { (libass.ass_library_done)(library) };
            return Err(AssError::InitFailed);
        }

        unsafe {
            (libass.ass_set_frame_size)(renderer, width as i32, height as i32);
            (libass.ass_set_storage_size)(renderer, width as i32, height as i32);
            (libass.ass_set_extract_fonts)(library, 1);
            (libass.ass_set_message_cb)(library, Some(libass_log_callback), std::ptr::null_mut());
        }

        Ok(Self {
            libass,
            library,
            renderer,
            track: ptr::null_mut(),
            width,
            height,
            fonts_configured: false,
        })
    }

    /// Load ASS content from a string.
    ///
    /// Parses the ASS script using `ass_read_memory`. Any previously loaded
    /// track is freed first.
    #[allow(clippy::unnecessary_cast, reason = "c_char differs per platform")]
    pub fn load_ass(&mut self, content: &str) -> Result<(), AssError> {
        // Free any existing track
        if !self.track.is_null() {
            unsafe { (self.libass.ass_free_track)(self.track) };
            self.track = ptr::null_mut();
        }

        let cstr = CString::new(content)
            .map_err(|_| AssError::Ass("ASS content contains null byte".into()))?;

        let track = unsafe {
            (self.libass.ass_read_memory)(
                self.library,
                cast_ptr_to_i8(cstr.as_ptr()),
                content.len(),
                ptr::null(),
            )
        };

        if track.is_null() {
            return Err(AssError::Ass("ass_read_memory returned null".into()));
        }

        self.track = track;
        Ok(())
    }

    /// Configure font lookup.
    ///
    /// Font provider selection uses `ASS_FONTPROVIDER_AUTODETECT=0` so that libass
    /// picks the platform-native provider (DirectWrite on Windows, fontconfig on
    /// Linux, CoreText on macOS).
    ///
    /// System font directories are scanned automatically based on the platform:
    ///
    /// - **Windows**: `C:\Windows\Fonts` and `%LOCALAPPDATA%\Microsoft\Windows\Fonts`
    /// - **Linux**: `/usr/share/fonts`, `/usr/local/share/fonts`, `~/.local/share/fonts`, `~/.fonts`
    /// - **macOS**: `/System/Library/Fonts`, `/Library/Fonts`, `~/Library/Fonts`
    ///
    /// In addition, all `font_dirs` provided by the user are scanned. Every font
    /// file (`.ttf`, `.otf`, `.ttc`, `.otc`, `.woff`, `.woff2`) found in any of
    /// these directories is registered with libass via [`ass_add_font`] **before**
    /// [`ass_set_fonts`] is called, so they are available to every font provider.
    /// This gives true system + user two-level font matching, regardless of the
    /// font provider in use.
    ///
    /// `font_dirs` — user-provided font directories. The first directory is also
    /// passed to [`ass_set_fonts_dir`] for embedded font extraction.
    #[allow(clippy::unnecessary_cast, reason = "c_char differs per platform")]
    pub fn configure_fonts(
        &mut self,
        default_family: Option<&str>,
        font_dirs: &[String],
    ) -> Result<(), AssError> {
        // --- 0) Build list of font directories to scan ------------------------
        let mut scan_dirs: Vec<String> = Vec::new();

        #[cfg(target_os = "windows")]
        {
            scan_dirs.push("C:\\Windows\\Fonts".to_string());
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                scan_dirs.push(format!("{}\\Microsoft\\Windows\\Fonts", local));
            }
        }

        #[cfg(target_os = "linux")]
        {
            scan_dirs.push("/usr/share/fonts".to_string());
            scan_dirs.push("/usr/local/share/fonts".to_string());
            if let Ok(home) = std::env::var("HOME") {
                scan_dirs.push(format!("{}/.local/share/fonts", home));
                scan_dirs.push(format!("{}/.fonts", home));
            }
        }

        #[cfg(target_os = "macos")]
        {
            scan_dirs.push("/System/Library/Fonts".to_string());
            scan_dirs.push("/Library/Fonts".to_string());
            if let Ok(home) = std::env::var("HOME") {
                scan_dirs.push(format!("{}/Library/Fonts", home));
            }
        }

        // Add user-provided font directories
        scan_dirs.extend(font_dirs.iter().cloned());

        // --- 1) Register individual font files from all directories -----------
        tracing::info!("Registering fonts from {} director(ies)", scan_dirs.len());
        for dir_path in &scan_dirs {
            let dir = std::path::Path::new(dir_path);
            if !dir.is_dir() {
                continue;
            }
            let mut count = 0u32;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    if !matches!(
                        ext.as_str(),
                        "ttf" | "otf" | "ttc" | "otc" | "woff" | "woff2"
                    ) {
                        continue;
                    }
                    let font_data = match std::fs::read(&path) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("font")
                        .to_string();
                    if let Ok(cname) = CString::new(name.as_str()) {
                        unsafe {
                            (self.libass.ass_add_font)(
                                self.library,
                                cast_ptr_to_i8(cname.as_ptr()),
                                cast_ptr_to_i8(font_data.as_ptr()),
                                font_data.len() as i32,
                            );
                        }
                    }
                    count += 1;
                    if count.is_multiple_of(50) {
                        tracing::info!("  fonts registered: {count}");
                    }
                }
            }
            if count > 0 {
                tracing::info!("  {dir_path}: registered {count} font(s)");
            }
        }

        // --- 2) Set fonts_dir for embedded font extraction (first user dir) ------
        if let Some(dir) = font_dirs.first() {
            if let Ok(cdir) = CString::new(dir.as_str()) {
                unsafe {
                    (self.libass.ass_set_fonts_dir)(self.library, cast_ptr_to_i8(cdir.as_ptr()));
                }
            }
        }

        // --- 3) Select font provider and initialize -------------------------
        let provider: i32 = 0; // ASS_FONTPROVIDER_AUTODETECT

        let family_cstr = default_family.and_then(|f| CString::new(f).ok());

        let family_ptr = family_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        unsafe {
            (self.libass.ass_set_fonts)(
                self.renderer,
                ptr::null(),
                cast_ptr_to_i8(family_ptr),
                provider,
                ptr::null(),
                0,
            );
        }

        self.fonts_configured = true;
        Ok(())
    }

    /// Enable or disable hinting.
    ///
    /// `ASS_HINTING_LIGHT = 1`, `ASS_HINTING_NONE = 0`
    pub fn set_hinting(&self, hinting: i32) {
        unsafe { (self.libass.ass_set_hinting)(self.renderer, hinting) }
    }

    /// Set font scale factor.
    pub fn set_font_scale(&self, scale: f64) {
        unsafe { (self.libass.ass_set_font_scale)(self.renderer, scale) }
    }

    /// Render a single frame at the given timestamp.
    ///
    /// Returns `None` if no images were rendered (empty/transparent frame).
    /// The images are returned as safe `Vec<AssImageData>` — each image's
    /// alpha buffer is copied from libass's internal memory (which is only
    /// valid until the next `render_frame` call).
    pub fn render_frame(&self, timestamp_ms: i64) -> Result<Option<Vec<AssImageData>>, AssError> {
        if self.track.is_null() {
            return Err(AssError::Ass("No track loaded".into()));
        }

        let mut detect_change: i32 = 0;
        let image = unsafe {
            (self.libass.ass_render_frame)(
                self.renderer,
                self.track,
                timestamp_ms,
                &mut detect_change,
            )
        };

        if image.is_null() {
            return Ok(None);
        }

        let mut images = Vec::new();
        let mut current = image;

        while !current.is_null() {
            let img = unsafe { &*current };

            let w = img.w.max(0) as u32;
            let h = img.h.max(0) as u32;
            let stride = img.stride.max(0) as u32;

            // Copy alpha buffer from libass's internal memory
            let bitmap = if w > 0 && h > 0 && !img.bitmap.is_null() {
                let mut buf = Vec::with_capacity((stride * h) as usize);
                unsafe {
                    std::ptr::copy_nonoverlapping(img.bitmap, buf.as_mut_ptr(), buf.capacity());
                    buf.set_len(buf.capacity());
                }
                buf
            } else {
                Vec::new()
            };

            images.push(AssImageData {
                w,
                h,
                stride,
                bitmap,
                color: img.color,
                dst_x: img.dst_x.max(0) as u32,
                dst_y: img.dst_y.max(0) as u32,
                image_type: ImageType::from(img.image_type),
            });

            current = img.next;
        }

        Ok(Some(images))
    }

    /// Return parsed event metadata from the loaded track.
    ///
    /// Reads `n_events` and the `events` array from `ASS_Track`.
    /// Returns an empty vec if no track is loaded or no events exist.
    pub fn events(&self) -> Vec<AssEventInfo> {
        if self.track.is_null() {
            return Vec::new();
        }

        let track = unsafe { &*self.track };

        let n_events = track.n_events.max(0) as usize;
        if n_events == 0 || track.events.is_null() {
            return Vec::new();
        }

        let mut events = Vec::with_capacity(n_events);
        for i in 0..n_events {
            let event = unsafe { &*(track.events.add(i)) };
            let text = if !event.text.is_null() {
                unsafe { std::ffi::CStr::from_ptr(event.text as *const std::os::raw::c_char) }
                    .to_string_lossy()
                    .into_owned()
            } else {
                String::new()
            };

            events.push(AssEventInfo {
                start_ms: event.start,
                duration_ms: event.duration,
                style: event.style,
                text,
            });
        }

        events
    }

    /// Returns the PlayResX from the loaded track, or the configured width.
    pub fn play_res_x(&self) -> u32 {
        if self.track.is_null() {
            return self.width;
        }
        let track = unsafe { &*self.track };
        let res = track.play_res_x.max(0) as u32;
        if res == 0 {
            self.width
        } else {
            res
        }
    }

    /// Returns the PlayResY from the loaded track, or the configured height.
    pub fn play_res_y(&self) -> u32 {
        if self.track.is_null() {
            return self.height;
        }
        let track = unsafe { &*self.track };
        let res = track.play_res_y.max(0) as u32;
        if res == 0 {
            self.height
        } else {
            res
        }
    }

    /// Returns the number of events in the loaded track.
    pub fn num_events(&self) -> usize {
        if self.track.is_null() {
            return 0;
        }
        let track = unsafe { &*self.track };
        track.n_events.max(0) as usize
    }
}

impl Drop for AssRenderer {
    fn drop(&mut self) {
        if !self.track.is_null() {
            unsafe { (self.libass.ass_free_track)(self.track) };
        }
        if !self.renderer.is_null() {
            unsafe { (self.libass.ass_renderer_done)(self.renderer) };
        }
        if !self.library.is_null() {
            unsafe { (self.libass.ass_library_done)(self.library) };
        }
    }
}

impl std::fmt::Debug for AssRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssRenderer")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("fonts_configured", &self.fonts_configured)
            .field("track_loaded", &(!self.track.is_null()))
            .field("num_events", &self.num_events())
            .finish()
    }
}
