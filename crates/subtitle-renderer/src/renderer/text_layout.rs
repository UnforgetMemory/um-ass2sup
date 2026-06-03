use crate::shaper::Shaper;

/// Converts an ASS alignment value (1–9, numpad layout) to a normalized (x, y) position.
///
/// Alignment follows the numpad layout: 7=top-left, 8=top-center, 9=top-right,
/// 4=middle-left, 5=middle-center, 6=middle-right, 1=bottom-left, 2=bottom-center,
/// 3=bottom-right. Returns values in 0.0–1.0 range.
pub fn alignment_to_pos(alignment: u8) -> (f32, f32) {
    match alignment {
        1 => (0.0, 1.0),
        2 => (0.5, 1.0),
        3 => (1.0, 1.0),
        4 => (0.0, 0.5),
        5 => (0.5, 0.5),
        6 => (1.0, 0.5),
        7 => (0.0, 0.0),
        8 => (0.5, 0.0),
        9 => (1.0, 0.0),
        _ => (0.5, 1.0),
    }
}

/// Removes ASS override blocks (`{...}`) from text, returning plain text for rendering.
pub fn strip_override_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    for ch in text.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

pub(super) fn wrap_text(
    text: &str,
    wrap_style: u8,
    shaper: &Shaper,
    font_id: fontdb::ID,
    font_size: f32,
    spacing: f32,
    available_width: f32,
) -> Vec<String> {
    let explicit_lines: Vec<&str> = text.split('\n').collect();

    match wrap_style {
        1 => explicit_lines.into_iter().map(String::from).collect(),
        3 => {
            // Low-end wrapping: word-wrap from bottom-right (ASS q=3)
            // Uses same smart wrapping but places lines from bottom
            let mut result: Vec<String> =
                wrap_text(text, 0, shaper, font_id, font_size, spacing, available_width);
            // q=3 places lines from bottom, achieved by reversing the line order
            result.reverse();
            result
        }
        2 => explicit_lines.into_iter().map(String::from).collect(),
        _ => {
            let mut result = Vec::new();
            for line in &explicit_lines {
                if line.is_empty() {
                    result.push(String::new());
                    continue;
                }
                let words: Vec<&str> = line.split(' ').collect();

                // Phase 1: Pre-shape each word individually (O(W) instead of O(W²)).
                struct WordInfo {
                    text: String,
                    width: f32,
                }
                let word_data: Vec<WordInfo> = words
                    .iter()
                    .filter_map(|w| {
                        if w.is_empty() {
                            return None;
                        }
                        shaper.shape(w, font_id, font_size).ok().map(|shaped| WordInfo {
                            text: w.to_string(),
                            width: shaped.total_advance + spacing * shaped.glyphs.len() as f32,
                        })
                    })
                    .collect();

                if word_data.is_empty() {
                    if !line.is_empty() {
                        result.push(line.to_string());
                    }
                    continue;
                }

                // Phase 2: Shape single space to correctly measure inter-word gaps.
                let space_width = shaper
                    .shape(" ", font_id, font_size)
                    .ok()
                    .map(|s| s.total_advance + spacing * s.glyphs.len() as f32)
                    .unwrap_or(0.0);

                // Phase 3: Line breaking using cumulative word widths.
                let mut current_line = String::new();
                let mut current_width = 0.0f32;

                for (i, wi) in word_data.iter().enumerate() {
                    let gap = if current_line.is_empty() { 0.0 } else { space_width };
                    let test_width = current_width + gap + wi.width;

                    if current_width > 0.0 && test_width > available_width {
                        result.push(current_line.clone());
                        current_line = wi.text.clone();
                        current_width = wi.width;
                    } else {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                        }
                        current_line.push_str(&wi.text);
                        current_width = test_width;
                    }

                    if i == word_data.len() - 1 && !current_line.is_empty() {
                        result.push(current_line.clone());
                    }
                }
            }
            result
        }
    }
}
