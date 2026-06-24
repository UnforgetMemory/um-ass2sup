//! Telemetry initialisation — tracing subscriber setup.
//!
//! Provides a single [`init`] entry point that configures CLI-appropriate
//! log output levels, ANSI colour support, and optional source-location
//! detail for debug builds.

use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Initialises the global `tracing` subscriber with the appropriate level filter.
///
/// Level selection:
/// - `debug`: `TRACE` with targets, files, line numbers
/// - `verbose`: `DEBUG`
/// - `quiet`: `ERROR` only
/// - otherwise: `INFO`
///
/// `RUST_LOG` overrides per-module filtering while preserving the CLI default
/// level as fallback. `color` controls ANSI styling: `"always"` forces it on,
/// `"never"` forces it off, any other value (typically `"auto"`) defers to
/// whether stderr is a TTY.
///
/// Uses `try_init` so repeated calls (across tests or embedded usage) are
/// silent no-ops after the first successful init.
pub fn init(verbose: bool, quiet: bool, debug: bool, color: &str) {
    let use_color = match color {
        "always" => true,
        "never" => false,
        _ => std::io::IsTerminal::is_terminal(&std::io::stderr()),
    };

    let default_level = if debug {
        LevelFilter::TRACE
    } else if quiet {
        LevelFilter::ERROR
    } else if verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy();

    let fmt_layer = fmt::layer()
        .with_ansi(use_color)
        .with_target(debug)
        .with_file(debug)
        .with_line_number(debug)
        .with_thread_ids(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_writer(std::io::stderr);

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init();
}
