use ass2sup_cli::{parse_resolution, Args};
use ass_parser::AssFile;

/// Helper to make a minimal Args with configurable resolution.
fn make_args(resolution: Option<String>) -> Args {
    Args {
        input: vec![],
        glob: None,
        recursive: false,
        max_files: None,
        output: None,
        output_dir: None,
        resolution,
        fps: 23.976,
        validate: false,
        overlap_warn: false,
        overlap_mode: "lenient".to_string(),
        quantizer: "median-cut".to_string(),
        max_colors: 255,
        dither: "floyd-steinberg".to_string(),
        font: "Arial".to_string(),
        font_size: 48.0,
        verbose: false,
        parallel: false,
        force: false,
        dry_run: false,
        parallel_frames: false,
        quiet: true,
        check: false,
        color: "never".to_string(),
        to_srt: false,
        to_bdn: false,
        no_check_fonts: true,
        font_map: vec![],
        debug: false,
        font_dir: vec![],
        config: None,
        cjk_fallback: vec![],
        log_level: None,
    }
}

#[test]
fn test_explicit_resolution_is_parsed() {
    let res = parse_resolution("1920x1080").unwrap();
    assert_eq!(res.width, 1920);
    assert_eq!(res.height, 1080);
}

#[test]
fn test_custom_resolution_is_parsed() {
    let res = parse_resolution("1280x720").unwrap();
    assert_eq!(res.width, 1280);
    assert_eq!(res.height, 720);
}

#[test]
fn test_ass_file_parses_play_res() {
    let ass_text = "[Script Info]
Title: Test
PlayResX: 1280
PlayResY: 720

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World";
    let ass = AssFile::parse(ass_text).unwrap();
    let (w, h) = ass.resolution();
    assert_eq!(w, 1280);
    assert_eq!(h, 720);
}

#[test]
fn test_args_resolution_is_optional() {
    // When user does not specify -r, resolution should be None
    let args = make_args(None);
    assert!(
        args.resolution.is_none(),
        "resolution should be None when not specified"
    );
}
