use ass_parser::AssFile;
use color_quantizer::Quantizer;
use pgs_encoder::PgsEncoder;
use std::path::PathBuf;
use subtitle_renderer::{FontManager, FrameCache, RenderConfig, Renderer};

fn test_config() -> RenderConfig {
    RenderConfig {
        width: 1920,
        height: 1080,
        script_width: 1920,
        script_height: 1080,
        default_font: "Arial".to_string(),
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

#[test]
fn test_simple_ass_end_to_end() {
    let path = find_fixture("simple.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let quantizer = Quantizer::new(255);
    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    assert!(!dialogues.is_empty(), "should have dialogue events");
    let mut total_frames = 0;
    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let segments = encoder.encode_frame(&quantized, pts_ms, frame.duration_ms);
            assert!(!segments.is_empty(), "should produce PGS segments");
            total_frames += 1;
        }
    }
    assert!(total_frames > 0, "should render at least one frame");
}

#[test]
fn test_effects_chain_end_to_end() {
    let path = find_fixture("effects_chain.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    assert!(dialogues.len() >= 5, "effects_chain should have 5+ events");
    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        let _frame = renderer.render_ass(&ass, pts_ms);
    }
}

#[test]
fn test_karaoke_ass_end_to_end() {
    let path = find_fixture("karaoke.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    assert!(!dialogues.is_empty(), "karaoke should have dialogue events");
    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        let _frame = renderer.render_ass(&ass, pts_ms);
    }
}

#[test]
fn test_frame_cache_end_to_end() {
    let path = find_fixture("simple.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let cache = FrameCache::new(64);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    if !dialogues.is_empty() {
        let pts_ms = dialogues[0].start.as_ms();
        let frame1 = renderer.render_ass_cached(&ass, pts_ms, &cache, 0);
        assert!(frame1.is_some(), "first render should succeed");
        let frame2 = renderer.render_ass_cached(&ass, pts_ms, &cache, 0);
        assert!(frame2.is_some(), "cached render should succeed");
        assert_eq!(cache.len(), 1, "cache should have 1 entry");
    }
}

#[test]
fn test_sup_binary_output_valid() {
    let path = find_fixture("simple.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let quantizer = Quantizer::new(255);
    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let bytes = encoder.encode_frame_to_bytes(&quantized, pts_ms, frame.duration_ms);
            assert!(bytes.len() >= 14, "SUP output should have at least header");
            assert_eq!(bytes[0], b'P', "SUP magic byte 1");
            assert_eq!(bytes[1], b'G', "SUP magic byte 2");
        }
    }
}

#[test]
fn test_overlay_events_render_separately() {
    let path = find_fixture("multi_style.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let quantizer = Quantizer::new(255);
    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    let mut rendered_count = 0;
    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let segments = encoder.encode_frame(&quantized, pts_ms, frame.duration_ms);
            assert!(!segments.is_empty());
            rendered_count += 1;
        }
    }
    assert!(rendered_count >= 3, "multi_style should render 3+ frames");
}

#[test]
fn test_complex_styles_end_to_end() {
    let path = find_fixture("complex_styles.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let quantizer = Quantizer::new(255);
    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    assert!(dialogues.len() >= 4, "complex_styles should have 4+ events");
    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let segments = encoder.encode_frame(&quantized, pts_ms, frame.duration_ms);
            assert!(!segments.is_empty());
        }
    }
}

#[test]
fn test_validation_before_render() {
    let path = find_fixture("overlapping.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let report = subtitle_validator::validate(&ass);
    if !report.is_valid {
        for finding in report.errors() {
            eprintln!("Validation error: {}", finding.message);
        }
    }
}

#[test]
fn test_quantizer_pipeline_consistency() {
    let path = find_fixture("simple.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    if let Some(event) = dialogues.first() {
        let pts_ms = event.start.as_ms();
        if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
            let quantizer1 = Quantizer::new(255);
            let quantizer2 = Quantizer::new(255);
            let q1 = quantizer1.quantize(&frame.bitmap, frame.width, frame.height);
            let q2 = quantizer2.quantize(&frame.bitmap, frame.width, frame.height);
            assert_eq!(q1.width, q2.width);
            assert_eq!(q1.height, q2.height);
            assert_eq!(q1.indices.len(), q2.indices.len());
        }
    }
}

#[test]
fn test_full_pipeline_ass_to_sup_bytes() {
    let path = find_fixture("simple.ass");
    let content = std::fs::read_to_string(&path).unwrap();
    let ass = AssFile::parse(&content).unwrap();
    let font_name = find_any_font();
    let mut config = test_config();
    config.default_font = font_name;
    let renderer = Renderer::new(config);
    let quantizer = Quantizer::new(255);
    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
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
    assert!(!all_bytes.is_empty(), "should produce SUP bytes");
    assert!(all_bytes.len() >= 14, "SUP output should have header");
}
