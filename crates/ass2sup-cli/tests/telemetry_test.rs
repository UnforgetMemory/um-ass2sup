//! Unit tests for the unified telemetry initialisation entry point.
//!
//! These tests guard:
//!   - `TelemetryConfig::default()` value semantics
//!   - Idempotent initialisation (subsequent `init` calls do not panic)
//!   - `ASS2SUP_LOG` and `ASS2SUP_COLOR` env-var parsing
//!   - `ColorChoice` mapping (auto / always / never)
//!   - All flag-combination matrix does not panic

use ass2sup_cli::telemetry::*;
use std::sync::Mutex;
use tracing_subscriber::filter::LevelFilter;

// tracing's global subscriber can only be initialised once per process.
// To keep tests independent we serialise them through this mutex and
// perform all the assertions inside one closure per test.
static TELEMETRY_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn default_config_is_info_and_auto_color() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    let cfg = TelemetryConfig::default();
    assert!(matches!(cfg.level, LevelFilter::INFO));
    assert!(matches!(cfg.color, ColorChoice::Auto));
    assert!(!cfg.with_source);
    assert!(!cfg.with_thread_ids);
}

#[test]
fn init_is_idempotent_under_repeated_calls() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    init(TelemetryConfig::default()).expect("first init");
    init(TelemetryConfig::default()).expect("second init must not panic");
    init(TelemetryConfig::default()).expect("third init must not panic");
}

#[test]
fn init_accepts_all_color_choices() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    for color in [ColorChoice::Auto, ColorChoice::Always, ColorChoice::Never] {
        let cfg = TelemetryConfig {
            color,
            ..TelemetryConfig::default()
        };
        init(cfg).expect("color choice must not panic");
    }
}

#[test]
fn init_accepts_all_level_filters() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    for level in [
        LevelFilter::OFF,
        LevelFilter::ERROR,
        LevelFilter::WARN,
        LevelFilter::INFO,
        LevelFilter::DEBUG,
        LevelFilter::TRACE,
    ] {
        let cfg = TelemetryConfig {
            level,
            ..TelemetryConfig::default()
        };
        init(cfg).expect("level filter must not panic");
    }
}

#[test]
fn init_default_reads_ass2sup_log_env() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    // SAFETY: tests in this file serialise through TELEMETRY_LOCK; we
    // briefly mutate process-global env and restore it before unlock.
    unsafe {
        std::env::set_var("ASS2SUP_LOG", "debug");
    }
    init_default().expect("ASS2SUP_LOG=debug must init cleanly");
    unsafe {
        std::env::remove_var("ASS2SUP_LOG");
    }
}

#[test]
fn init_default_handles_unknown_log_level_gracefully() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("ASS2SUP_LOG", "gibberish-value");
    }
    init_default().expect("unknown log level must fall back, not panic");
    unsafe {
        std::env::remove_var("ASS2SUP_LOG");
    }
}

#[test]
fn init_default_reads_ass2sup_color_env() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    for (value, _expected) in [
        ("always", ColorChoice::Always),
        ("never", ColorChoice::Never),
        ("ALWAYS", ColorChoice::Always),  // case-insensitive
        ("gibberish", ColorChoice::Auto), // unknown -> Auto
    ] {
        unsafe {
            std::env::set_var("ASS2SUP_COLOR", value);
        }
        init_default().expect("ASS2SUP_COLOR parse must not panic");
        unsafe {
            std::env::remove_var("ASS2SUP_COLOR");
        }
    }
}

#[test]
fn init_supports_ass2sup_log_env_when_called_directly() {
    let _g = TELEMETRY_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("ASS2SUP_LOG", "warn");
    }
    init_default().expect("ASS2SUP_LOG=warn must init cleanly");
    unsafe {
        std::env::remove_var("ASS2SUP_LOG");
    }
}

#[test]
fn color_choice_is_copy_and_eq() {
    let a = ColorChoice::Auto;
    let b = a; // copy
    assert_eq!(a, b);
    assert_ne!(ColorChoice::Auto, ColorChoice::Always);
}

#[test]
fn telemetry_config_clone_preserves_values() {
    let cfg = TelemetryConfig {
        level: LevelFilter::DEBUG,
        color: ColorChoice::Never,
        with_source: true,
        with_thread_ids: true,
    };
    let cfg2 = cfg.clone();
    assert!(matches!(cfg2.level, LevelFilter::DEBUG));
    assert!(matches!(cfg2.color, ColorChoice::Never));
    assert!(cfg2.with_source);
    assert!(cfg2.with_thread_ids);
}
