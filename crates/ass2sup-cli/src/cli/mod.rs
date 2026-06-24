//! CLI driver adapter — argument parsing, input discovery, progress display.
//!
//! The [`args::Args`] struct (via clap) defines every CLI flag.
//! [`run::run`] is the top-level entry point called by `main.rs`.

pub mod args;
pub mod progress;
