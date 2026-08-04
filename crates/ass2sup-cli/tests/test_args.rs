use ass2sup_cli::cli::args::Args;
use ass2sup_cli::config::font::parse_font_map;
use ass2sup_cli::config::resolution::Resolution;
use clap::Parser;

#[test]
fn test_args_default() {
    // Minimal invocation: just an input file.
    let args = Args::parse_from(["ass2sup", "input.ass"]);
    assert_eq!(args.input, [std::path::PathBuf::from("input.ass")]);
    assert_eq!(args.fps, 23.976);
    assert_eq!(args.max_colors, 255);
    assert_eq!(args.dither, "floyd-steinberg");
    assert_eq!(args.font, "Arial");
    assert_eq!(args.color, "auto");
    assert_eq!(args.log_level, "info");
    assert_eq!(args.format, "sup");
    assert_eq!(args.overlap, "off");
    assert!(!args.validate);
    assert!(!args.parallel);
    assert!(!args.quiet);
    assert!(args.resolution.is_none());
    assert!(args.output.is_none());
}

#[test]
fn test_fps_rejects_infinite_and_nan() {
    // NaN / ±inf fps must fail at parse time: they would make the
    // frame-timeline loops in the pipeline spin forever (see W1).
    for bad in ["inf", "+inf", "-inf", "infinity", "nan", "NaN"] {
        let result = Args::try_parse_from(["ass2sup", "input.ass", "--check", "-f", bad]);
        assert!(result.is_err(), "fps '{bad}' must be rejected");
    }
}

#[test]
fn test_fps_rejects_non_positive() {
    for bad in ["0", "0.0", "-1", "-23.976"] {
        let result = Args::try_parse_from(["ass2sup", "input.ass", "-f", bad]);
        assert!(result.is_err(), "fps '{bad}' must be rejected");
    }
}

#[test]
fn test_fps_accepts_valid_values() {
    for good in ["23.976", "25", "29.97", "23.976"] {
        let result = Args::try_parse_from(["ass2sup", "input.ass", "-f", good]);
        assert!(
            result.is_ok(),
            "fps '{good}' must be accepted, got: {:?}",
            result.err()
        );
    }
    let args = Args::parse_from(["ass2sup", "input.ass", "-f", "29.97"]);
    assert!((args.fps - 29.97).abs() < 1e-9);
}

#[test]
fn test_enum_args_reject_invalid_values() {
    // Unknown values for enumerated string args must fail at parse time
    // instead of silently falling back to a default.
    for (flag, value) in [
        ("--overlap", "bogus"),
        ("--dither", "bogus"),
        ("--color-space", "bogus"),
        ("--tonemap", "bogus"),
        ("--format", "bogus"),
        ("--log-level", "bogus"),
    ] {
        let result = Args::try_parse_from(["ass2sup", "input.ass", flag, value]);
        assert!(result.is_err(), "{flag} '{value}' must be rejected");
    }
}

#[test]
fn test_enum_args_accept_valid_values() {
    let args = Args::parse_from([
        "ass2sup",
        "input.ass",
        "--overlap",
        "strict",
        "--dither",
        "ordered",
        "--color-space",
        "bt709",
        "--tonemap",
        "hable",
        "--format",
        "srt",
        "--log-level",
        "debug",
    ]);
    assert_eq!(args.overlap, "strict");
    assert_eq!(args.dither, "ordered");
    assert_eq!(args.color_space, "bt709");
    assert_eq!(args.tonemap.as_deref(), Some("hable"));
    assert_eq!(args.format, "srt");
    assert_eq!(args.log_level, "debug");
}

#[test]
fn test_removed_flags_are_unknown() {
    // --quantizer and --parallel-frames no longer exist: passing them must
    // produce a clap parse error rather than being silently accepted.
    for flag in ["--quantizer", "--parallel-frames"] {
        let result = Args::try_parse_from(["ass2sup", "input.ass", flag, "median-cut"]);
        assert!(
            result.is_err(),
            "{flag} must be rejected as an unknown argument"
        );
    }
}

#[cfg(all(feature = "native-backend", feature = "libass-backend"))]
#[test]
fn test_backend_rejects_unknown_value_when_both_backends_compiled() {
    // Only meaningful in dual-backend builds, where --backend actually
    // selects a renderer. Single-backend builds accept any value (no-op).
    for value in ["bogus", "cairo", ""] {
        let result = Args::try_parse_from(["ass2sup", "input.ass", "--backend", value]);
        assert!(result.is_err(), "--backend '{value}' must be rejected");
    }
    let args = Args::parse_from(["ass2sup", "input.ass", "--backend", "libass"]);
    assert_eq!(args.backend.as_deref(), Some("libass"));
}

#[test]
fn test_resolution_parse() {
    let res = Resolution::parse("1920x1080").unwrap();
    assert_eq!(res.width, 1920);
    assert_eq!(res.height, 1080);

    let res = Resolution::parse("1280x720").unwrap();
    assert_eq!(res.width, 1280);
    assert_eq!(res.height, 720);

    let res = Resolution::parse("3840x2160").unwrap();
    assert_eq!(res.width, 3840);
    assert_eq!(res.height, 2160);
}

#[test]
fn test_resolution_parse_invalid() {
    // Non-numeric input
    assert!(Resolution::parse("abc").is_err());
    // Missing height
    assert!(Resolution::parse("1920").is_err());
    // Extra separator
    assert!(Resolution::parse("1920x1080x").is_err());
    // Negative values (u32 won't parse)
    assert!(Resolution::parse("-1920x1080").is_err());
    // Zero dimensions
    assert!(Resolution::parse("0x1080").is_err());
    assert!(Resolution::parse("1920x0").is_err());
    // Empty string
    assert!(Resolution::parse("").is_err());
}

#[test]
fn test_font_map_parse() {
    let entries = vec![
        "Style1:Arial,Noto Sans".to_string(),
        "Style2:Times New Roman".to_string(),
    ];
    let map = parse_font_map(&entries).unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(
        map.get("Style1").unwrap(),
        &vec!["Arial".to_string(), "Noto Sans".to_string()]
    );
    assert_eq!(
        map.get("Style2").unwrap(),
        &vec!["Times New Roman".to_string()]
    );
}

#[test]
fn test_font_map_invalid() {
    // Missing colon separator
    assert!(parse_font_map(&["JustAStyle".to_string()]).is_err());
    // Empty style name before colon
    assert!(parse_font_map(&[":Arial".to_string()]).is_err());
    // Empty entry string
    assert!(parse_font_map(&["".to_string()]).is_err());
}
