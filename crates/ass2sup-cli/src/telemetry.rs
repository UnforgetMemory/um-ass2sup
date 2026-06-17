//! Telemetry / logging initialisation entry points for the ass2sup CLI.
//!
//! Two functions are exposed:
//!
//! - [`init`] takes an explicit [`TelemetryConfig`] and installs a
//!   `tracing_subscriber` registry. Repeated calls are safe (the
//!   underlying `try_init` is idempotent and surfaces no error after
//!   the first successful install).
//! - [`init_default`] reads the `ASS2SUP_LOG` and `ASS2SUP_COLOR`
//!   environment variables and delegates to [`init`]. It is the
//!   convenience used by the `main` entry point.
//!
//! The configuration surface is intentionally small: callers needing
//! fine-grained control (e.g. emitting JSON to a sidecar collector)
//! can drop down to `tracing_subscriber` directly; this module covers
//! the 99% case.
//!
//! [`init`]: crate::telemetry::init
//! [`TelemetryConfig`]: crate::telemetry::TelemetryConfig
//! [`init_default`]: crate::telemetry::init_default

use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

use crate::error::Result;

/// Telemetry configuration. Cheap to construct; pass by value.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Minimum level emitted.
    pub level: LevelFilter,
    /// ANSI colour policy.
    pub color: ColorChoice,
    /// When `true`, log records include target / file / line number.
    pub with_source: bool,
    /// When `true`, log records include the OS thread id.
    pub with_thread_ids: bool,
}

/// Color output policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    /// Defer to TTY detection on stderr.
    Auto,
    /// Force ANSI colour on.
    Always,
    /// Force ANSI colour off.
    Never,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            level: LevelFilter::INFO,
            color: ColorChoice::Auto,
            with_source: false,
            with_thread_ids: false,
        }
    }
}

/// Initialise the global tracing subscriber from an explicit config.
///
/// Repeated calls are silent no-ops after the first successful install
/// (the underlying `try_init` returns a benign error that we discard).
pub fn init(config: TelemetryConfig) -> Result<()> {
    let use_color = resolve_color(config.color);

    let env_filter = EnvFilter::builder()
        .with_default_directive(config.level.into())
        .from_env_lossy();

    let fmt_layer = fmt::layer()
        .with_ansi(use_color)
        .with_target(config.with_source)
        .with_file(config.with_source)
        .with_line_number(config.with_source)
        .with_thread_ids(config.with_thread_ids)
        .with_timer(fmt::time::uptime())
        .with_writer(std::io::stderr);

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init();

    Ok(())
}

/// Initialise telemetry from `ASS2SUP_LOG` and `ASS2SUP_COLOR` env vars,
/// falling back to [`TelemetryConfig::default`] for any missing / invalid value.
///
/// Unknown log-level values cause a one-line warning on stderr and a
/// graceful fallback to the default level; same shape as
/// `parse_level` so the two stay in sync.
pub fn init_default() -> Result<()> {
    let mut config = TelemetryConfig::default();

    if let Ok(level) = std::env::var("ASS2SUP_LOG") {
        if let Some(parsed) = parse_level(&level) {
            config.level = parsed;
        } else {
            eprintln!(
                "warning: ASS2SUP_LOG={level:?} is not a recognised level; \
                 using {:?}",
                config.level
            );
        }
    }

    if let Ok(color) = std::env::var("ASS2SUP_COLOR") {
        config.color = match color.to_lowercase().as_str() {
            "always" => ColorChoice::Always,
            "never" => ColorChoice::Never,
            _ => ColorChoice::Auto,
        };
    }

    init(config)
}

/// Parse a textual level name into a [`LevelFilter`].
///
/// Used by tests and by callers wanting to derive a [`TelemetryConfig`]
/// from a free-form string. Returns `None` for unknown names.
pub fn parse_level(name: &str) -> Option<LevelFilter> {
    match name.to_lowercase().as_str() {
        "off" => Some(LevelFilter::OFF),
        "error" => Some(LevelFilter::ERROR),
        "warn" | "warning" => Some(LevelFilter::WARN),
        "info" => Some(LevelFilter::INFO),
        "debug" => Some(LevelFilter::DEBUG),
        "trace" => Some(LevelFilter::TRACE),
        _ => None,
    }
}

fn resolve_color(choice: ColorChoice) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => std::io::IsTerminal::is_terminal(&std::io::stderr()),
    }
}
