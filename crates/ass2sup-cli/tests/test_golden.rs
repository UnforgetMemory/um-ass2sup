use ass_parser::AssFile;
use color_quantizer::Quantizer;
use pgs_encoder::PgsEncoder;
use std::path::PathBuf;
use subtitle_renderer::{FontManager, RenderConfig, Renderer};

fn test_config() -> RenderConfig {
    let font_name = find_any_font();
    RenderConfig {
        width: 1920,
        height: 1080,
        script_width: 1920,
        script_height: 1080,
        default_font: font_name,
        default_font_size: 48.0,
    }
}

fn find_any_font() -> String {
    let candidates = [
        "Arial",
        "Liberation Sans",
        "DejaVu Sans",
        "Noto Sans",
        "Helvetica",
    ];
    let mut fm = FontManager::default();
    fm.load_system_fonts();
    for name in &candidates {
        if fm.query_with_fallback(name, false, false).is_some() {
            return name.to_string();
        }
    }
    "Arial".to_string()
}

fn find_fixture(name: &str) -> PathBuf {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    manifest.join("../../tests/fixtures").join(name)
}

fn run_pipeline(fixture: &str, fps: f64) -> Vec<u8> {
    let path = find_fixture(fixture);
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let config = test_config();
    let renderer = Renderer::new(config);
    let quantizer = Quantizer::new(255);
    let mut encoder = PgsEncoder::new(1920, 1080, fps);
    let mut all_bytes = Vec::new();
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let bytes = encoder.encode_frame_to_bytes(&quantized, pts_ms, frame.duration_ms);
            all_bytes.extend_from_slice(&bytes);
        }
    }
    all_bytes
}

fn parse_sup_segments(bytes: &[u8]) -> Vec<(u32, u32, u8, usize)> {
    let mut segments = Vec::new();
    let mut offset = 0;
    while offset + 13 <= bytes.len() {
        let pts = u32::from_be_bytes([
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
        ]);
        let dts = u32::from_be_bytes([
            bytes[offset + 6],
            bytes[offset + 7],
            bytes[offset + 8],
            bytes[offset + 9],
        ]);
        let seg_type = bytes[offset + 10];
        let seg_size = u16::from_be_bytes([bytes[offset + 11], bytes[offset + 12]]) as usize;
        segments.push((pts, dts, seg_type, seg_size));
        offset += 13 + seg_size;
        if offset > bytes.len() {
            break;
        }
    }
    segments
}

#[test]
fn test_golden_sup_structure() {
    let bytes = run_pipeline("simple.ass", 24.0);
    assert!(!bytes.is_empty(), "should produce output");

    let mut offset = 0;
    let mut segment_count = 0;
    while offset + 13 <= bytes.len() {
        assert_eq!(bytes[offset], b'P', "segment should start with PG");
        assert_eq!(bytes[offset + 1], b'G');
        let pts = u32::from_be_bytes([
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
        ]);
        let dts = u32::from_be_bytes([
            bytes[offset + 6],
            bytes[offset + 7],
            bytes[offset + 8],
            bytes[offset + 9],
        ]);
        let seg_type = bytes[offset + 10];
        let seg_size = u16::from_be_bytes([bytes[offset + 11], bytes[offset + 12]]) as usize;

        assert!(
            pts > 0 || segment_count == 0,
            "PTS should be non-zero for seg {}",
            segment_count
        );
        assert!(dts <= pts, "DTS should be <= PTS");
        assert!(
            seg_size > 0 || seg_type == 0x80,
            "segment size should be >0 for non-END"
        );

        offset += 13 + seg_size;
        segment_count += 1;

        if offset > bytes.len() {
            break;
        }
    }
    assert!(
        segment_count >= 3,
        "should have PCS+WDS+PDS+ODS+END at minimum"
    );
}

#[test]
fn test_golden_multiple_fps() {
    let bytes_24 = run_pipeline("simple.ass", 24.0);
    let bytes_25 = run_pipeline("simple.ass", 25.0);
    let bytes_30 = run_pipeline("simple.ass", 30.0);

    assert!(!bytes_24.is_empty());
    assert!(!bytes_25.is_empty());
    assert!(!bytes_30.is_empty());

    if bytes_24.len() > 6 && bytes_25.len() > 6 {
        let pts_24 = u32::from_be_bytes([bytes_24[2], bytes_24[3], bytes_24[4], bytes_24[5]]);
        let pts_25 = u32::from_be_bytes([bytes_25[2], bytes_25[3], bytes_25[4], bytes_25[5]]);
        assert!(pts_24 > 0 || pts_25 > 0);
    }
}

#[test]
fn test_golden_ntsc_pts() {
    let bytes = run_pipeline("simple.ass", 23.976);
    assert!(!bytes.is_empty());

    let segments = parse_sup_segments(&bytes);
    assert!(
        !segments.is_empty(),
        "NTSC pipeline should produce segments"
    );
    let has_nonzero = segments.iter().any(|&(pts, _, _, _)| pts > 0);
    assert!(has_nonzero, "at least one segment should have PTS > 0");
}

#[test]
fn test_golden_different_resolutions() {
    let path = find_fixture("simple.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();

    for (w, h) in [(1920, 1080), (1280, 720), (720, 480)] {
        let config = RenderConfig {
            width: w,
            height: h,
            script_width: w,
            script_height: h,
            default_font: find_any_font(),
            default_font_size: 48.0,
        };
        let renderer = Renderer::new(config);
        let quantizer = Quantizer::new(255);
        let mut encoder = PgsEncoder::new(w as u16, h as u16, 24.0);
        let mut any_rendered = false;
        let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
        for event in &dialogues {
            let pts_ms = event.start.as_ms();
            if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
                assert_eq!(frame.width, w);
                assert_eq!(frame.height, h);
                let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
                let bytes = encoder.encode_frame_to_bytes(&quantized, pts_ms, frame.duration_ms);
                assert!(!bytes.is_empty());
                any_rendered = true;
            }
        }
        assert!(
            any_rendered,
            "should render at least one frame at {}x{}",
            w, h
        );
    }
}

#[test]
fn test_golden_all_fixtures_produce_output() {
    let fixtures = [
        "simple.ass",
        "effects_chain.ass",
        "karaoke_advanced.ass",
        "batch_mode.ass",
    ];

    for fixture in &fixtures {
        let bytes = run_pipeline(fixture, 24.0);
        assert!(
            !bytes.is_empty(),
            "fixture '{}' should produce SUP output",
            fixture
        );
        assert!(bytes.len() >= 13, "fixture '{}' output too small", fixture);
        assert_eq!(bytes[0], b'P', "fixture '{}' should start with PG", fixture);
        assert_eq!(bytes[1], b'G', "fixture '{}' should start with PG", fixture);
    }
}

#[test]
fn test_golden_korean_drama_island_disappeared() {
    // Korean drama "Island of the Disappeared" - Chinese subtitles
    // Tests CJK text rendering with Microsoft YaHei font
    let bytes = run_pipeline("island_disappeared.ass", 23.976);
    assert!(
        !bytes.is_empty(),
        "island_disappeared should produce SUP output"
    );
    assert!(bytes.len() >= 13, "island_disappeared output too small");
    assert_eq!(bytes[0], b'P', "island_disappeared should start with PG");
    assert_eq!(bytes[1], b'G', "island_disappeared should start with PG");

    // Verify PTS timing is present
    let segments = parse_sup_segments(&bytes);
    assert!(!segments.is_empty(), "should have valid PGS segments");
    let has_nonzero_pts = segments.iter().any(|&(pts, _, _, _)| pts > 0);
    assert!(has_nonzero_pts, "at least one segment should have PTS > 0");
}
