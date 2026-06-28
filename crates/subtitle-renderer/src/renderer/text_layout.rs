/// Converts an ASS alignment value (1–9, numpad layout) to a normalized (x, y) position.
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

pub(super) fn remap_alignment_vertical(alignment: u8, writing_mode: u8) -> u8 {
    match writing_mode {
        2 => match alignment {
            1 => 3,
            2 => 6,
            3 => 9,
            4 => 2,
            5 => 5,
            6 => 8,
            7 => 1,
            8 => 4,
            9 => 7,
            _ => alignment,
        },
        3 => match alignment {
            1 => 7,
            2 => 4,
            3 => 1,
            4 => 8,
            5 => 5,
            6 => 2,
            7 => 9,
            8 => 6,
            9 => 3,
            _ => alignment,
        },
        _ => alignment,
    }
}

/// Removes ASS override blocks (`{...}`) from text, returning plain text for rendering.
pub fn strip_override_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    for ch in text.chars() {
        match ch {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Process ASS text escape sequences after `strip_override_blocks`.
///
/// Converts:
/// - `\N` → `\n` (forced newline)
/// - `\n` → `\n` (soft newline)
/// - `\\` → `\` (escaped backslash)
/// - `\h` → non-breaking space (U+00A0)
pub fn process_ass_text_escapes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '\\' {
            match it.peek().copied() {
                Some('N') | Some('n') => {
                    it.next();
                    result.push('\n');
                }
                Some('\\') => {
                    it.next();
                    result.push('\\');
                }
                Some('h') => {
                    it.next();
                    result.push('\u{00a0}');
                }
                _ => result.push(ch),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub(super) fn wrap_text_vertical(
    text: &str,
    available_height: f32,
    line_height: f32,
) -> Vec<String> {
    if text.is_empty() || available_height <= 0.0 || line_height <= 0.0 {
        return vec![text.to_string()];
    }
    let max_lines_per_column = (available_height / line_height).floor() as usize;
    if max_lines_per_column == 0 {
        return vec![text.to_string()];
    }
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.is_empty() {
        return vec![String::new()];
    }
    let mut columns: Vec<String> = Vec::new();
    let mut current_column = String::new();
    let mut current_line_count = 0usize;
    for line in &lines {
        if current_line_count >= max_lines_per_column && !current_column.is_empty() {
            columns.push(current_column);
            current_column = String::new();
            current_line_count = 0;
        }
        if !current_column.is_empty() {
            current_column.push('\n');
        }
        current_column.push_str(line);
        current_line_count += 1;
    }
    if !current_column.is_empty() {
        columns.push(current_column);
    }
    columns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_escape_hard_newline() {
        assert_eq!(process_ass_text_escapes("line1\\Nline2"), "line1\nline2");
    }

    #[test]
    fn process_escape_soft_newline() {
        assert_eq!(process_ass_text_escapes("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn process_escape_hard_space() {
        assert_eq!(process_ass_text_escapes("a\\hb"), "a\u{00a0}b");
    }

    #[test]
    fn process_escape_backslash_preserved() {
        // \\ → single backslash, so the text "a\b" produces "a\b"
        // In Rust source, "a\\\\b" = two backslashes, "a\\b" = one backslash
        assert_eq!(process_ass_text_escapes("a\\\\b"), "a\\b");
    }

    #[test]
    fn process_escape_no_escapes() {
        assert_eq!(process_ass_text_escapes("hello world"), "hello world");
    }

    #[test]
    fn process_escape_mixed() {
        assert_eq!(
            process_ass_text_escapes("line1\\Nline2\\hmore"),
            "line1\nline2\u{00a0}more"
        );
    }

    #[test]
    fn process_escape_empty() {
        assert_eq!(process_ass_text_escapes(""), "");
    }

    #[test]
    fn strip_override_blocks_no_blocks() {
        assert_eq!(strip_override_blocks("hello world"), "hello world");
    }

    #[test]
    fn strip_override_blocks_simple() {
        assert_eq!(strip_override_blocks("{\\b1}bold"), "bold");
    }

    #[test]
    fn strip_override_blocks_nested() {
        assert_eq!(strip_override_blocks("a{\\b1{\\i1}}b"), "ab");
    }

    #[test]
    fn strip_override_blocks_escape_sequences_preserved() {
        assert_eq!(
            strip_override_blocks("{\\b1}line1\\Nline2"),
            "line1\\Nline2"
        );
    }
}
