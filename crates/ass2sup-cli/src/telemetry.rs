//! Telemetry initialisation — dual-layer tracing subscriber.
//!
//! **stdout** — user-facing output (simplified, no timestamps).
//! **stderr** — diagnostic tracing (timestamps, targets, optional file/line).
//!
//! Level mapping:
//!
//! | CLI flags     | stdout filter | stderr filter |
//! |---------------|---------------|---------------|
//! | (default)     | `INFO`        | `WARN`        |
//! | `--verbose`   | `INFO`        | `DEBUG`       |
//! | `--debug`     | `INFO`        | `TRACE` (+ file/line) |
//! | `--quiet`     | `ERROR`       | `ERROR`       |

use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    fmt,
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
};

/// Initialises the global `tracing` subscriber with dual output layers.
///
/// - **stdout** receives user-facing messages at `INFO` level (or `ERROR` in quiet mode).
/// - **stderr** receives full diagnostic output with timestamps, controlled by
///   `--verbose`/`--debug`/`--quiet`.
///
/// `RUST_LOG` overrides the stderr filter while preserving the CLI defaults as fallback.
pub fn init(verbose: bool, quiet: bool, debug: bool, color: &str) {
    let use_color = match color {
        "always" => true,
        "never" => false,
        _ => std::io::IsTerminal::is_terminal(&std::io::stdout()),
    };

    // stderr filter: diagnostic depth controlled by --verbose/--debug/--quiet
    let diag_level = if debug {
        LevelFilter::TRACE
    } else if verbose {
        LevelFilter::DEBUG
    } else if quiet {
        LevelFilter::ERROR
    } else {
        LevelFilter::WARN
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(diag_level.into())
        .from_env_lossy();

    // stdout layer: user-facing messages, simplified (no timer/target/file)
    let user_level = if quiet {
        LevelFilter::ERROR
    } else {
        LevelFilter::INFO
    };
    let user_filter = EnvFilter::builder()
        .with_default_directive(user_level.into())
        .from_env_lossy();

    let user_layer = fmt::layer()
        .with_ansi(use_color)
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .with_thread_ids(false)
        .without_time()
        .with_writer(std::io::stdout)
        .event_format(
            fmt::format()
                .with_level(false)
                .with_target(false)
                .with_file(false)
                .with_line_number(false)
                .without_time(),
        )
        .with_filter(user_filter);

    // stderr layer: diagnostic tracing with full context
    let diag_layer = fmt::layer()
        .with_ansi(use_color)
        .with_target(debug)
        .with_file(debug)
        .with_line_number(debug)
        .with_thread_ids(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_writer(std::io::stderr)
        .with_filter(env_filter);

    let _ = tracing_subscriber::registry()
        .with(user_layer)
        .with(diag_layer)
        .try_init();
}
