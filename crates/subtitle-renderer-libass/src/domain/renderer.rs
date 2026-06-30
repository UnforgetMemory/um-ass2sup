//! Rendering bridge: libass track management and frame rasterization.

use std::ffi::CString;
use std::ptr;

use libass_sys;

use crate::domain::error::AssError;
use crate::domain::frame::{AssEventInfo, AssImageData, ImageType};

/// Safe Rust wrapper around libass lifecycle.
///
/// Manages `ASS_Library`, `ASS_Renderer`, and `ASS_Track` handles with
/// correct Drop ordering (track → renderer → library).
pub struct AssRenderer {
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
        let library = unsafe { libass_sys::ass_library_init() };
        if library.is_null() {
            return Err(AssError::InitFailed);
        }

        let renderer = unsafe { libass_sys::ass_renderer_init(library) };
        if renderer.is_null() {
            unsafe { libass_sys::ass_library_done(library) };
            return Err(AssError::InitFailed);
        }

        unsafe {
            libass_sys::ass_set_frame_size(renderer, width as i32, height as i32);
            libass_sys::ass_set_storage_size(renderer, width as i32, height as i32);
            libass_sys::ass_set_extract_fonts(library, 1);
        }

        Ok(Self {
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
    pub fn load_ass(&mut self, content: &str) -> Result<(), AssError> {
        // Free any existing track
        if !self.track.is_null() {
            unsafe { libass_sys::ass_free_track(self.track) };
            self.track = ptr::null_mut();
        }

        let cstr = CString::new(content)
            .map_err(|_| AssError::Ass("ASS content contains null byte".into()))?;

        let track = unsafe {
            libass_sys::ass_read_memory(self.library, cstr.as_ptr(), content.len(), ptr::null())
        };

        if track.is_null() {
            return Err(AssError::Ass("ass_read_memory returned null".into()));
        }

        self.track = track;
        Ok(())
    }

    /// Configure font lookup.
    ///
    /// Uses fontconfig on Linux (ASS_FONTPROVIDER_FONTCONFIG=3).
    /// If `default_family` is `None`, libass uses its own fallback.
    /// If `fonts_dir` is `Some`, sets an additional fonts directory.
    pub fn configure_fonts(
        &mut self,
        default_family: Option<&str>,
        fonts_dir: Option<&str>,
    ) -> Result<(), AssError> {
        // Font directory MUST be set BEFORE ass_set_fonts
        if let Some(dir) = fonts_dir {
            let cdir = CString::new(dir)
                .map_err(|_| AssError::Config("fonts_dir contains null byte".into()))?;
            unsafe {
                libass_sys::ass_set_fonts_dir(self.library, cdir.as_ptr());
            }
        } else {
            // Fallback: set the user font dir so fontconfig finds CJK fonts
            static USER_FONTS: &str = "/home/um/.local/share/fonts";
            if let Ok(cdir) = CString::new(USER_FONTS) {
                unsafe {
                    libass_sys::ass_set_fonts_dir(self.library, cdir.as_ptr());
                }
            }
        }

        let family_cstr = default_family.and_then(|f| CString::new(f).ok());

        let family_ptr = family_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        unsafe {
            libass_sys::ass_set_fonts(
                self.renderer,
                ptr::null(),
                family_ptr,
                3, // ASS_FONTPROVIDER_FONTCONFIG
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
        unsafe { libass_sys::ass_set_hinting(self.renderer, hinting) }
    }

    /// Set font scale factor.
    pub fn set_font_scale(&self, scale: f64) {
        unsafe { libass_sys::ass_set_font_scale(self.renderer, scale) }
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
            libass_sys::ass_render_frame(
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
                unsafe { std::ffi::CStr::from_ptr(event.text) }
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
        // IMPORTANT: Correct drop order is track → renderer → library
        if !self.track.is_null() {
            unsafe { libass_sys::ass_free_track(self.track) };
        }
        if !self.renderer.is_null() {
            unsafe { libass_sys::ass_renderer_done(self.renderer) };
        }
        if !self.library.is_null() {
            unsafe { libass_sys::ass_library_done(self.library) };
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
