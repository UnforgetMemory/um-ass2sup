use std::path::PathBuf;

fn find_fixture(name: &str) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../tests/fixtures").join(name)
}

fn find_any_font() -> String {
    "Arial".to_string()
}

fn render_fixture_to_sup(fixture_name: &str, font: &str) -> Result<Vec<u8>, String> {
    let fixture_path = find_fixture(fixture_name);
    let content = std::fs::read_to_string(&fixture_path)
        .map_err(|e| format!("Failed to read fixture '{}': {}", fixture_name, e))?;
    let ass = ass_parser::AssFile::parse(&content)
        .map_err(|e| format!("Failed to parse '{}': {}", fixture_name, e))?;

    let render_config = subtitle_renderer::RenderConfig {
        width: 1920,
        height: 1080,
        script_width: ass.script_info.play_res_x,
        script_height: ass.script_info.play_res_y,
        default_font: font.to_string(),
        default_font_size: 48.0,
    };
    let renderer = subtitle_renderer::Renderer::new(render_config);
    let quantizer = color_quantizer::Quantizer::new(255);
    let mut pgs_encoder = pgs_encoder::PgsEncoder::new(1920, 1080, 23.976);

    let dialogues: Vec<_> = ass.dialogue_events().collect();
    let mut all_segments = Vec::new();

    for event in &dialogues {
        let pts_ms = event.start.as_ms();
        let duration_ms = event.duration_ms();

        if let Some(frame) = renderer.render_ass(&ass, pts_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let segments = pgs_encoder.encode_frame(&quantized, pts_ms, duration_ms);
            all_segments.extend(segments);
        }
    }

    let sup_file = pgs_encoder::types::SupFile {
        segments: all_segments,
    };
    Ok(sup_file.to_bytes())
}

#[test]
fn test_golden_simple() {
    let sup_data = render_fixture_to_sup("simple.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty(), "SUP output should not be empty");
    assert_eq!(sup_data[0], b'P');
    assert_eq!(sup_data[1], b'G');
}

#[test]
fn test_golden_effects() {
    let sup_data = render_fixture_to_sup("effects.ass", &find_any_font()).expect("render failed");
    assert!(
        sup_data.len() > 100,
        "effects.ass should produce substantial output"
    );
}

#[test]
fn test_golden_karaoke() {
    let sup_data = render_fixture_to_sup("karaoke.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
    assert_eq!(sup_data[0], b'P');
    assert_eq!(sup_data[1], b'G');
}

#[test]
fn test_golden_overlapping() {
    let sup_data =
        render_fixture_to_sup("overlapping.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_complex_styles() {
    let sup_data =
        render_fixture_to_sup("complex_styles.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_effects_chain() {
    let sup_data =
        render_fixture_to_sup("effects_chain.ass", &find_any_font()).expect("render failed");
    assert!(sup_data.len() > 200);
}

#[test]
fn test_golden_karaoke_advanced() {
    let sup_data =
        render_fixture_to_sup("karaoke_advanced.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_transform_anim() {
    let sup_data =
        render_fixture_to_sup("transform_anim.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_clip_regions() {
    let sup_data =
        render_fixture_to_sup("clip_regions.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_fade_effects() {
    let sup_data =
        render_fixture_to_sup("fade_effects.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_rotation_scale() {
    let sup_data =
        render_fixture_to_sup("rotation_scale.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_unicode_text() {
    let sup_data =
        render_fixture_to_sup("unicode_text.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_position_move() {
    let sup_data =
        render_fixture_to_sup("position_move.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_color_alpha() {
    let sup_data =
        render_fixture_to_sup("color_alpha.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}

#[test]
fn test_golden_drawing_mode() {
    let sup_data =
        render_fixture_to_sup("drawing_mode.ass", &find_any_font()).expect("render failed");
    assert!(!sup_data.is_empty());
}
