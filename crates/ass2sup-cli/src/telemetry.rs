//! Telemetry initialisation — dual-layer tracing subscriber.
//!
//! **stdout** — user-facing output (simplified, no timestamps).
//! **stderr** — diagnostic tracing (timestamps, targets, optional file/line).
//!
//! Level mapping (`--log-level`):
//!
//! | `--log-level` | stdout filter | stderr filter |
//! |---------------|---------------|---------------|
//! | `error`       | `ERROR`       | `ERROR`       |
//! | `warn`        | `INFO`        | `WARN`        |
//! | `info` (default)| `INFO`      | `WARN`        |
//! | `debug`       | `INFO`        | `DEBUG`       |
//! | `trace`       | `INFO`        | `TRACE` (+ file/line) |
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
///   `--log-level` and `--quiet`.
/// - **log_file** (optional) receives the same diagnostic output as stderr,
///   written to a file with ANSI disabled, for later inspection.
///
/// `RUST_LOG` overrides the stderr filter while preserving the CLI defaults as fallback.
pub fn init(log_level: &str, quiet: bool, color: &str, log_file: Option<&str>) {
    let use_color = match color {
        "always" => true,
        "never" => false,
        _ => std::io::IsTerminal::is_terminal(&std::io::stdout()),
    };

    // stderr filter: diagnostic depth controlled by --log-level/--quiet
    let diag_level = if quiet {
        LevelFilter::ERROR
    } else {
        match log_level {
            "error" => LevelFilter::ERROR,
            "warn" => LevelFilter::WARN,
            "debug" => LevelFilter::DEBUG,
            "trace" => LevelFilter::TRACE,
            _ => LevelFilter::WARN,
        }
    };
    let debug = log_level == "trace";

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
        .with_timer(tracing_subscriber::fmt::time())
        .with_writer(std::io::stderr)
        .with_filter(env_filter.clone());

    let registry = tracing_subscriber::registry()
        .with(user_layer)
        .with(diag_layer);

    // optional file layer: full diagnostic filter (INFO+ always, regardless
    // of --verbose/--quiet which target the console), no ANSI, full timestamp.
    // A log file exists to capture what happened — it must not be silenced by
    // the console's WARN-only default.
    if let Some(path) = log_file {
        if let Ok(file) = std::fs::File::create(path) {
            let file_filter = EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy();
            let file_layer = fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_thread_ids(false)
                .with_writer(file)
                .with_filter(file_filter);
            let _ = registry.with(file_layer).try_init();
            return;
        }
        eprintln!("warning: cannot open log file '{path}', continuing without file logging");
    }
    let _ = registry.try_init();
}
