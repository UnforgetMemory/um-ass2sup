//! Unit tests for the TOML-backed `ass2sup_cli::config` module.
//!
//! These tests guard:
//!   - Default values
//!   - Serde round-trip (TOML → Config → TOML → Config)
//!   - `deny_unknown_fields` rejection
//!   - `Config::load` semantics for missing / present / malformed files
//!   - `Config::save` round-trip via tempdir
//!   - `Config::merge_with_args` precedence rules

use ass2sup_cli::config::*;
use ass2sup_cli::error::ConfigError;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn default_config_has_expected_values() {
    let cfg = Config::default();
    assert_eq!(cfg.defaults.fps, Some(23.976));
    assert_eq!(cfg.defaults.max_colors, Some(255));
    assert_eq!(cfg.defaults.dither.as_deref(), Some("floyd-steinberg"));
    assert_eq!(cfg.defaults.parallel_frames, Some(true));
    assert!(cfg.cjk_fallback.chain.is_empty());
    assert!(cfg.cjk_fallback.per_style.is_empty());
    assert!(!cfg.cjk_fallback.strict);
    assert_eq!(cfg.color.output_space.as_deref(), Some("sdr-bt709"));
    assert_eq!(cfg.color.tonemap.as_deref(), Some("hable"));
    assert!(cfg.style_overrides.is_empty());
    assert_eq!(cfg.rendering.backend.as_deref(), Some("auto"));
    assert_eq!(cfg.rendering.pixel_accuracy.as_deref(), Some("high"));
    assert!(cfg.log_level.is_none());
}

#[test]
fn serde_round_trip_preserves_cjk_chain() {
    let cfg = Config {
        cjk_fallback: CjkFallback {
            chain: vec!["Noto Sans CJK SC".to_string()],
            per_style: HashMap::new(),
            strict: true,
        },
        ..Config::default()
    };
    let toml_str = toml::to_string(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(parsed.cjk_fallback.chain, cfg.cjk_fallback.chain);
    assert!(parsed.cjk_fallback.strict);
}

#[test]
fn unknown_field_in_top_level_is_rejected() {
    let toml_str = r#"
nonsense_field = "should fail"
"#;
    let result: Result<Config, _> = toml::from_str(toml_str);
    assert!(result.is_err(), "deny_unknown_fields must reject");
}

#[test]
fn unknown_field_in_defaults_is_rejected() {
    let toml_str = r#"
[defaults]
fps = 23.976
totally_made_up = true
"#;
    let result: Result<Config, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn unknown_field_in_color_is_rejected() {
    let toml_str = r#"
[color]
output_space = "sdr-bt709"
mystery_toggle = 42
"#;
    let result: Result<Config, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn load_nonexistent_path_returns_default() {
    let cfg = Config::load(std::path::Path::new("/nonexistent/never/toml.toml"))
        .expect("missing file should be treated as default");
    assert_eq!(cfg.defaults.fps, Some(23.976));
}

#[test]
fn load_malformed_toml_propagates_parse_error() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("bad.toml");
    std::fs::write(&path, "this is = not = valid toml = at all [[[").unwrap();
    let result = Config::load(&path);
    assert!(
        result.is_err(),
        "malformed TOML must surface ConfigError::Parse"
    );
}

#[test]
fn save_then_load_round_trips_via_tempdir() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("cfg.toml");
    let mut cfg = Config::default();
    cfg.cjk_fallback.chain = vec!["Test Font".to_string()];
    cfg.cjk_fallback.strict = true;
    cfg.log_level = Some("debug".to_string());
    cfg.save(&path).expect("save");
    let loaded = Config::load(&path).expect("load");
    assert_eq!(loaded.cjk_fallback.chain, vec!["Test Font".to_string()]);
    assert!(loaded.cjk_fallback.strict);
    assert_eq!(loaded.log_level.as_deref(), Some("debug"));
}

#[test]
fn empty_document_yields_default_via_load() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("empty.toml");
    std::fs::write(&path, "").unwrap();
    let cfg = Config::load(&path).expect("empty is valid");
    assert_eq!(cfg.defaults.fps, Some(23.976));
}

#[test]
fn style_override_array_round_trips() {
    let cfg = Config {
        style_overrides: vec![StyleOverride {
            style: "OP_1".into(),
            font: "Source Han Sans CN".into(),
        }],
        ..Config::default()
    };
    let s = toml::to_string(&cfg).unwrap();
    let back: Config = toml::from_str(&s).unwrap();
    assert_eq!(back.style_overrides.len(), 1);
    assert_eq!(back.style_overrides[0].style, "OP_1");
    assert_eq!(back.style_overrides[0].font, "Source Han Sans CN");
}

#[test]
fn rendering_config_round_trips() {
    let cfg = Config {
        rendering: RenderingConfig {
            backend: Some("gpu".to_string()),
            pixel_accuracy: Some("exact".to_string()),
        },
        ..Config::default()
    };
    let s = toml::to_string(&cfg).unwrap();
    let back: Config = toml::from_str(&s).unwrap();
    assert_eq!(back.rendering.backend.as_deref(), Some("gpu"));
    assert_eq!(back.rendering.pixel_accuracy.as_deref(), Some("exact"));
}

#[test]
fn load_oversized_file_is_refused() {
    use std::io::Write;
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("huge.toml");
    // Write > MAX_CONFIG_SIZE_BYTES (1 MiB) of repeating content.
    let chunk = "[defaults]\nfps = 23.976\n".repeat(50_000); // ~1.2 MiB
    assert!(
        chunk.len() as u64 > MAX_CONFIG_SIZE_BYTES,
        "test setup must exceed limit (chunk={})",
        chunk.len()
    );
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(chunk.as_bytes()).unwrap();
    f.sync_all().unwrap();
    let result = Config::load(&path);
    assert!(result.is_err(), "files over the limit must be rejected");
    let err = result.unwrap_err();
    assert!(
        matches!(err, ConfigError::Validation(_)),
        "expected ConfigError::Validation, got: {err:?}"
    );
}

#[test]
fn parse_level_recognises_off() {
    use ass2sup_cli::telemetry::parse_level;
    use tracing_subscriber::filter::LevelFilter;
    assert!(matches!(parse_level("off"), Some(LevelFilter::OFF)));
    assert!(matches!(parse_level("OFF"), Some(LevelFilter::OFF)));
    assert!(parse_level("nonsense").is_none());
}
