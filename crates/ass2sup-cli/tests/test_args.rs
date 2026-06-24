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
    assert!(!args.validate);
    assert!(!args.parallel);
    assert!(!args.verbose);
    assert!(!args.quiet);
    assert!(args.resolution.is_none());
    assert!(args.output.is_none());
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
