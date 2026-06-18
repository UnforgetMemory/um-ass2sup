//! BDN XML subtitle format generation for Blu-ray authoring pipelines.
//!
//! This crate provides types and serialization for the **BDN (Blu-ray Disc
//! Movie) XML subtitle format**, used in professional Blu-ray authoring
//! workflows. It is part of the [`ass2sup`](https://crates.io/crates/ass2sup)
//! pipeline and can be used standalone or via `ass2sup-cli --to-bdn`.
//!
//! # Key types
//!
//! - [`BdnXml`] — Top-level BDN document with metadata (name, resolution,
//!   video format) and a list of subtitle events.
//! - [`BdnEvent`] — A single subtitle event with in/out timecodes, position,
//!   dimensions, and a reference to a PNG graphic file.
//! - [`QuantizedFrame`] — Intermediate quantized frame data (RGBA palette +
//!   indexed pixel data) that can be encoded to PNG.
//! - [`BdnError`] — Error type covering XML serialization, PNG encoding, and
//!   I/O failures.
//!
//! # Key functions
//!
//! - [`generate_xml`] — Serialize a [`BdnXml`] document to a BDN-compliant
//!   XML string.
//! - [`save_frame_png`] — Write a quantized frame to a PNG file on disk.
//! - [`generate_png`] — Encode a quantized frame as palette-indexed PNG bytes
//!   in memory.
//! - [`ms_to_timecode`] — Convert milliseconds to `HH:MM:SS:FF` timecode at a
//!   given frame rate.
//!
//! # Example
//!
//! ```no_run
//! use bdn_xml::{BdnXml, BdnEvent, generate_xml};
//!
//! let mut doc = BdnXml::new("My Movie", 1920, 1080);
//! doc.add_event(BdnEvent {
//!     index: 0,
//!     in_tc: "00:00:00:00".into(),
//!     out_tc: "00:00:03:00".into(),
//!     graphic: "00001.png".into(),
//!     x: 0,
//!     y: 0,
//!     width: 1920,
//!     height: 1080,
//!     forced: false,
//! });
//!
//! let xml = generate_xml(&doc).expect("XML generation failed");
//! ```

mod error;
pub mod sink;
mod types;
mod xml;

pub use error::BdnError;
pub use sink::{
    write_ttml_header, write_webvtt_header, AssPassthroughSink, OutputSink, Result as SinkResult,
    SinkError, TtmlSink, WebVttSink,
};
pub use types::{BdnEvent, BdnXml, QuantizedFrame};
pub use xml::{generate_png, generate_xml, ms_to_timecode, save_frame_png};
