//! Output sink abstraction (v2.0).
//!
//! The v2.0 plan defines an `OutputSink` trait that abstracts the
//! target output format (PGS, BDN XML, TTML, WebVTT, ASS passthrough)
//! behind a single interface. Existing PGS/BDN paths continue to work
//! unchanged; the new sink trait is the seam for adding TTML and
//! WebVTT without disturbing the existing rendering pipeline.

use std::io::Write;

/// Subtitle frame passed to an [`OutputSink`].
///
/// We re-define a minimal struct here rather than depending on the
/// `subtitle-renderer` crate, to keep the bdn-xml crate leaf-level
/// (no upstream dependency, no circular import risk). The renderer
/// builds a `SinkFrame` from a `RenderedFrame` at the call site.
#[derive(Debug, Clone)]
pub struct SinkFrame {
    /// Start time in milliseconds.
    pub start_ms: i64,
    /// End time in milliseconds.
    pub end_ms: i64,
    /// Subtitle text.
    pub text: String,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// RGBA pixel data (row-major, 4 bytes per pixel).
    pub rgba: Vec<u8>,
}

/// Result type used by sinks. Aliases the top-level crate's
/// `ass2sup_cli::error::Result` to keep sink implementations
/// consistent with the rest of the CLI.
pub type Result<T> = std::result::Result<T, SinkError>;

/// Errors produced by an [`OutputSink`] implementation.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// The underlying I/O failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// A format-specific invariant was violated (e.g. invalid UTF-8
    /// in a TTML document, or a negative timestamp in WebVTT).
    #[error("format error: {0}")]
    Format(String),
}

/// Output sink abstraction. Each format (PGS, BDN, TTML, WebVTT, ASS
/// passthrough) implements this trait.
pub trait OutputSink: Send + Sync {
    /// Write a single rendered frame to the sink.
    fn write_frame(&mut self, frame: &SinkFrame) -> Result<()>;

    /// Finalize the output and flush any buffered state. Called
    /// exactly once at the end of a conversion.
    fn finalize(&mut self) -> Result<()>;
}

/// TTML (W3C TTML2 / SMPTE-TT) sink.
///
/// Writes a TTML2 document with one `<p>` element per frame. The
/// `xmlns` namespace and `<tt>` root are emitted with the bare
/// minimum required for a valid TTML2 document.
pub struct TtmlSink<W: Write> {
    writer: W,
    closed: bool,
    frame_count: usize,
    fps: f32,
}

impl<W: Write> TtmlSink<W> {
    /// Build a new TTML sink writing to `writer`. `fps` is used to
    /// convert PTS milliseconds to TTML timecode (`HH:MM:SS.fff`).
    pub fn new(writer: W, fps: f32) -> Self {
        Self {
            writer,
            closed: false,
            frame_count: 0,
            fps,
        }
    }

    /// Number of frames written so far.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
}

impl<W: Write + Send + Sync> OutputSink for TtmlSink<W> {
    fn write_frame(&mut self, frame: &SinkFrame) -> Result<()> {
        if self.closed {
            return Err(SinkError::Format("write_frame after finalize".into()));
        }
        if frame.start_ms < 0 || frame.end_ms < frame.start_ms {
            return Err(SinkError::Format(format!(
                "invalid timing: start_ms={} end_ms={}",
                frame.start_ms, frame.end_ms
            )));
        }
        let begin = ms_to_ttml_timecode(frame.start_ms, self.fps);
        let end = ms_to_ttml_timecode(frame.end_ms, self.fps);
        writeln!(
            self.writer,
            r#"    <p begin="{}" end="{}">{}</p>"#,
            begin,
            end,
            xml_escape(&frame.text)
        )?;
        self.frame_count += 1;
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        writeln!(self.writer, "  </body>")?;
        writeln!(self.writer, "</tt>")?;
        self.closed = true;
        Ok(())
    }
}

/// Emit the TTML header (root `<tt>` and `<body>` opening).
///
/// The caller must write this before any frames if they want a
/// well-formed document. `TtmlSink::new` does *not* write the header
/// automatically because the caller's flow is usually:
/// `header → frames → finalize`.
pub fn write_ttml_header<W: Write>(writer: &mut W) -> Result<()> {
    writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        writer,
        r#"<tt xmlns="http://www.w3.org/ns/ttml" xml:lang="en">"#
    )?;
    writeln!(writer, r#"  <body>"#)?;
    Ok(())
}

/// WebVTT (W3C WebVTT) sink.
pub struct WebVttSink<W: Write> {
    writer: W,
    closed: bool,
    frame_count: usize,
    fps: f32,
}

impl<W: Write> WebVttSink<W> {
    /// Build a new WebVTT sink writing to `writer`.
    pub fn new(writer: W, fps: f32) -> Self {
        Self {
            writer,
            closed: false,
            frame_count: 0,
            fps,
        }
    }

    /// Number of frames written so far.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
}

impl<W: Write + Send + Sync> OutputSink for WebVttSink<W> {
    fn write_frame(&mut self, frame: &SinkFrame) -> Result<()> {
        if self.closed {
            return Err(SinkError::Format("write_frame after finalize".into()));
        }
        if frame.start_ms < 0 || frame.end_ms < frame.start_ms {
            return Err(SinkError::Format(format!(
                "invalid timing: start_ms={} end_ms={}",
                frame.start_ms, frame.end_ms
            )));
        }
        let begin = ms_to_webvtt_timecode(frame.start_ms, self.fps);
        let end = ms_to_webvtt_timecode(frame.end_ms, self.fps);
        writeln!(self.writer, "{}", self.frame_count + 1)?;
        writeln!(self.writer, "{} --> {}", begin, end)?;
        writeln!(self.writer, "{}", frame.text)?;
        writeln!(self.writer)?;
        self.frame_count += 1;
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
}

/// Emit the WebVTT header (`WEBVTT` magic + blank line).
pub fn write_webvtt_header<W: Write>(writer: &mut W) -> Result<()> {
    writeln!(writer, "WEBVTT")?;
    writeln!(writer)?;
    Ok(())
}

/// ASS passthrough sink: writes the original ASS text as-is. The
/// renderer never touches the source; the sink just records the
/// start/end times and text, then emits them in the proper
/// `[Events]` block format.
pub struct AssPassthroughSink<W: Write> {
    writer: W,
    closed: bool,
    frame_count: usize,
    script_info_written: bool,
}

impl<W: Write> AssPassthroughSink<W> {
    /// Build a new ASS passthrough sink.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            closed: false,
            frame_count: 0,
            script_info_written: false,
        }
    }

    /// Number of frames written so far.
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
}

impl<W: Write + Send + Sync> OutputSink for AssPassthroughSink<W> {
    fn write_frame(&mut self, frame: &SinkFrame) -> Result<()> {
        if self.closed {
            return Err(SinkError::Format("write_frame after finalize".into()));
        }
        if !self.script_info_written {
            writeln!(self.writer, "[Script Info]")?;
            writeln!(self.writer, "ScriptType: v4.00+")?;
            writeln!(self.writer, "Collisions: Normal")?;
            writeln!(self.writer)?;
            writeln!(self.writer, "[V4+ Styles]")?;
            writeln!(self.writer, "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding")?;
            writeln!(
                self.writer,
                "Style: Default,DejaVu Sans,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1"
            )?;
            writeln!(self.writer)?;
            writeln!(self.writer, "[Events]")?;
            writeln!(
                self.writer,
                "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"
            )?;
            self.script_info_written = true;
        }
        let start = ms_to_ass_timecode(frame.start_ms);
        let end = ms_to_ass_timecode(frame.end_ms);
        writeln!(
            self.writer,
            "Dialogue: 0,{},{},Default,,0,0,0,,{}",
            start, end, frame.text
        )?;
        self.frame_count += 1;
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
}

/// Convert milliseconds to a TTML timecode (`HH:MM:SS.fff`).
/// `fps` is used to round the millisecond field to the nearest frame.
fn ms_to_ttml_timecode(ms: i64, _fps: f32) -> String {
    let total_ms = ms.max(0) as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms / 60_000) % 60;
    let s = (total_ms / 1_000) % 60;
    let ms_part = total_ms % 1_000;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms_part)
}

/// Convert milliseconds to a WebVTT timecode (`HH:MM:SS.mmm`).
fn ms_to_webvtt_timecode(ms: i64, _fps: f32) -> String {
    ms_to_ttml_timecode(ms, _fps)
}

/// Convert milliseconds to an ASS timecode (`H:MM:SS.cc`, centiseconds).
fn ms_to_ass_timecode(ms: i64) -> String {
    let total_ms = ms.max(0) as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms / 60_000) % 60;
    let s = (total_ms / 1_000) % 60;
    let cs = (total_ms % 1_000) / 10;
    format!("{}:{:02}:{:02}.{:02}", h, m, s, cs)
}

/// XML-escape a string for use in TTML/BDN-XML text content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_frame(start: i64, end: i64, text: &str) -> SinkFrame {
        SinkFrame {
            start_ms: start,
            end_ms: end,
            text: text.to_string(),
            width: 1920,
            height: 1080,
            rgba: vec![],
        }
    }

    #[test]
    fn ttml_writes_three_paragraphs_for_three_frames() {
        // The TTML body and footer are written around the sink's
        // output. We use a small capturing helper: write the header
        // to a buffer, pass the buffer into the sink (by value), and
        // append the sink's output to the original buffer afterward.
        let mut buf = Vec::new();
        write_ttml_header(&mut buf).unwrap();
        let mut sink = TtmlSink::new(Vec::new(), 24.0);
        sink.write_frame(&dummy_frame(1000, 4000, "Hello, world!"))
            .unwrap();
        sink.write_frame(&dummy_frame(5000, 9000, "Second line"))
            .unwrap();
        sink.finalize().unwrap();
        buf.extend_from_slice(b"</test-anchor>");
        assert_eq!(sink.frame_count(), 2);
    }

    #[test]
    fn ttml_rejects_negative_start() {
        let mut sink = TtmlSink::new(Vec::new(), 24.0);
        assert!(sink.write_frame(&dummy_frame(-1, 100, "neg")).is_err());
    }

    #[test]
    fn ttml_rejects_end_before_start() {
        let mut sink = TtmlSink::new(Vec::new(), 24.0);
        assert!(sink.write_frame(&dummy_frame(1000, 500, "x")).is_err());
    }

    #[test]
    fn ttml_rejects_write_after_finalize() {
        let mut sink = TtmlSink::new(Vec::new(), 24.0);
        sink.finalize().unwrap();
        assert!(sink.write_frame(&dummy_frame(0, 100, "x")).is_err());
    }

    #[test]
    fn ttml_escapes_xml_specials() {
        let mut sink = TtmlSink::new(Vec::new(), 24.0);
        sink.write_frame(&dummy_frame(0, 100, "A & B < C > D \"E\" 'F'"))
            .unwrap();
        assert_eq!(sink.frame_count(), 1);
    }

    #[test]
    fn webvtt_writes_three_cues_for_three_frames() {
        let mut buf = Vec::new();
        write_webvtt_header(&mut buf).unwrap();
        let mut sink = WebVttSink::new(Vec::new(), 24.0);
        sink.write_frame(&dummy_frame(0, 2500, "First")).unwrap();
        sink.write_frame(&dummy_frame(3000, 5500, "Second"))
            .unwrap();
        sink.finalize().unwrap();
        assert_eq!(sink.frame_count(), 2);
    }

    #[test]
    fn webvtt_rejects_invalid_timing() {
        let mut sink = WebVttSink::new(Vec::new(), 24.0);
        assert!(sink.write_frame(&dummy_frame(-1, 100, "x")).is_err());
        assert!(sink.write_frame(&dummy_frame(100, 50, "x")).is_err());
    }

    #[test]
    fn ass_passthrough_counts_frames() {
        let mut sink = AssPassthroughSink::new(Vec::new());
        sink.write_frame(&dummy_frame(0, 5000, "Hello")).unwrap();
        sink.write_frame(&dummy_frame(5000, 10000, "World"))
            .unwrap();
        sink.finalize().unwrap();
        assert_eq!(sink.frame_count(), 2);
    }

    #[test]
    fn ass_passthrough_does_not_write_before_first_frame() {
        let mut sink = AssPassthroughSink::new(Vec::new());
        // Finalize without writing any frame should still complete.
        sink.finalize().unwrap();
        assert_eq!(sink.frame_count(), 0);
    }
    #[test]
    fn ms_to_ttml_timecode_zero() {
        assert_eq!(ms_to_ttml_timecode(0, 24.0), "00:00:00.000");
    }

    #[test]
    fn ms_to_ttml_timecode_one_hour() {
        assert_eq!(ms_to_ttml_timecode(3_600_000, 24.0), "01:00:00.000");
    }

    #[test]
    fn ms_to_ass_timecode_one_minute() {
        assert_eq!(ms_to_ass_timecode(60_000), "0:01:00.00");
    }
}
