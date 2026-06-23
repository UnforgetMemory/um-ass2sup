pub(crate) enum DrawingCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    BezierTo(f32, f32, f32, f32, f32, f32),
    Close,
}

/// Maximum number of times a drawing command can be repeated (DoS protection).
const MAX_DRAWING_REPEAT: usize = 10000;

pub(crate) fn parse_drawing_level(text: &str) -> u8 {
    for tag_block in text.chars().collect::<Vec<_>>().windows(4) {
        if tag_block[0] == '\\' && tag_block[1] == 'p' {
            if let Some(d) = tag_block.get(2).and_then(|c| c.to_digit(10)) {
                return d as u8;
            }
        }
    }
    0
}

pub(crate) fn parse_drawing_commands(text: &str) -> Vec<DrawingCommand> {
    let mut commands = Vec::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    let mut last_cmd: Option<&str> = None;

    while i < tokens.len() {
        let token = tokens[i];
        if token.len() == 1 {
            match token {
                "m" if i + 2 < tokens.len() => {
                    if let (Ok(x), Ok(y)) =
                        (tokens[i + 1].parse::<f32>(), tokens[i + 2].parse::<f32>())
                    {
                        commands.push(DrawingCommand::MoveTo(x, y));
                        last_cmd = Some("m");
                        i += 3;
                        continue;
                    }
                }
                "l" if i + 2 < tokens.len() => {
                    if let (Ok(x), Ok(y)) =
                        (tokens[i + 1].parse::<f32>(), tokens[i + 2].parse::<f32>())
                    {
                        commands.push(DrawingCommand::LineTo(x, y));
                        last_cmd = Some("l");
                        i += 3;
                        continue;
                    }
                }
                "b" if i + 6 < tokens.len() => {
                    let nums: Option<Vec<f32>> = (1..=6)
                        .map(|j| tokens[i + j].parse::<f32>().ok())
                        .collect::<Option<Vec<_>>>();
                    if let Some(n) = nums {
                        commands.push(DrawingCommand::BezierTo(n[0], n[1], n[2], n[3], n[4], n[5]));
                        last_cmd = Some("b");
                        i += 7;
                        continue;
                    }
                }
                "p" | "n" => {
                    if i + 1 < tokens.len() && tokens[i + 1] == "c" {
                        commands.push(DrawingCommand::Close);
                        last_cmd = None;
                        i += 2;
                        continue;
                    }
                    commands.push(DrawingCommand::Close);
                    last_cmd = None;
                    i += 1;
                    continue;
                }
                "c" => {
                    commands.push(DrawingCommand::Close);
                    last_cmd = None;
                    i += 1;
                    continue;
                }
                _ => {}
            }
        }

        if token.len() > 1 {
            if let Ok(_repeat) = token.parse::<usize>() {
                if i + 1 < tokens.len() {
                    let cmd_char = tokens[i + 1];
                    if matches!(cmd_char, "m" | "l" | "b") {
                        let args_needed = match cmd_char {
                            "m" | "l" => 2,
                            "b" => 6,
                            _ => 0,
                        };
                        for _ in 0.._repeat.min(MAX_DRAWING_REPEAT) {
                            if i + 1 + args_needed < tokens.len() {
                                match cmd_char {
                                    "m" => {
                                        let x: f32 = tokens[i + 2].parse().unwrap_or(0.0);
                                        let y: f32 = tokens[i + 3].parse().unwrap_or(0.0);
                                        commands.push(DrawingCommand::MoveTo(x, y));
                                    }
                                    "l" => {
                                        let x: f32 = tokens[i + 2].parse().unwrap_or(0.0);
                                        let y: f32 = tokens[i + 3].parse().unwrap_or(0.0);
                                        commands.push(DrawingCommand::LineTo(x, y));
                                    }
                                    "b" => {
                                        let nums: Vec<f32> = (2..=7)
                                            .filter_map(|j| tokens.get(i + j)?.parse().ok())
                                            .collect();
                                        if nums.len() == 6 {
                                            commands.push(DrawingCommand::BezierTo(
                                                nums[0], nums[1], nums[2], nums[3], nums[4],
                                                nums[5],
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        i += 2 + args_needed;
                        continue;
                    }
                }
            }
        }

        if let (Ok(x), Some("m" | "l")) = (token.parse::<f32>(), last_cmd) {
            if i + 1 < tokens.len() {
                if let Ok(y) = tokens[i + 1].parse::<f32>() {
                    if last_cmd == Some("m") {
                        commands.push(DrawingCommand::MoveTo(x, y));
                    } else {
                        commands.push(DrawingCommand::LineTo(x, y));
                    }
                    i += 2;
                    continue;
                }
            }
        }

        i += 1;
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_drawing_level ────────────────────────────────────

    #[test]
    fn test_parse_drawing_level_empty() {
        assert_eq!(parse_drawing_level(""), 0);
    }

    #[test]
    fn test_parse_drawing_level_no_p_tag() {
        assert_eq!(parse_drawing_level("m 0 0 l 100 100"), 0);
    }

    #[test]
    fn test_parse_drawing_level_p1() {
        assert_eq!(parse_drawing_level("\\p1 m 0 0 l 100 100"), 1);
    }

    #[test]
    fn test_parse_drawing_level_p2() {
        assert_eq!(parse_drawing_level("\\p2 m 0 0 l 100 100"), 2);
    }

    #[test]
    fn test_parse_drawing_level_p_in_override_block() {
        assert_eq!(parse_drawing_level("{\\p3}m 0 0 l 100 100"), 3);
    }

    // ── parse_drawing_commands ─────────────────────────────────

    #[test]
    fn test_parse_drawing_commands_empty() {
        assert!(parse_drawing_commands("").is_empty());
    }

    #[test]
    fn test_parse_drawing_commands_move_to() {
        let cmds = parse_drawing_commands("m 10 20");
        assert_eq!(cmds.len(), 1);
        match cmds[0] {
            DrawingCommand::MoveTo(x, y) => {
                assert!((x - 10.0).abs() < f32::EPSILON);
                assert!((y - 20.0).abs() < f32::EPSILON);
            }
            _ => panic!("Expected MoveTo"),
        }
    }

    #[test]
    fn test_parse_drawing_commands_line_to() {
        let cmds = parse_drawing_commands("l 30 40");
        assert_eq!(cmds.len(), 1);
        match cmds[0] {
            DrawingCommand::LineTo(x, y) => {
                assert!((x - 30.0).abs() < f32::EPSILON);
                assert!((y - 40.0).abs() < f32::EPSILON);
            }
            _ => panic!("Expected LineTo"),
        }
    }

    #[test]
    fn test_parse_drawing_commands_bezier() {
        let cmds = parse_drawing_commands("b 10 20 30 40 50 60");
        assert_eq!(cmds.len(), 1);
        match cmds[0] {
            DrawingCommand::BezierTo(x1, y1, x2, y2, x3, y3) => {
                assert!((x1 - 10.0).abs() < f32::EPSILON);
                assert!((y1 - 20.0).abs() < f32::EPSILON);
                assert!((x2 - 30.0).abs() < f32::EPSILON);
                assert!((y2 - 40.0).abs() < f32::EPSILON);
                assert!((x3 - 50.0).abs() < f32::EPSILON);
                assert!((y3 - 60.0).abs() < f32::EPSILON);
            }
            _ => panic!("Expected BezierTo"),
        }
    }

    #[test]
    fn test_parse_drawing_commands_close_p() {
        let cmds = parse_drawing_commands("p");
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], DrawingCommand::Close));
    }

    #[test]
    fn test_parse_drawing_commands_close_n() {
        let cmds = parse_drawing_commands("n");
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], DrawingCommand::Close));
    }

    #[test]
    fn test_parse_drawing_commands_close_c() {
        let cmds = parse_drawing_commands("c");
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], DrawingCommand::Close));
    }

    #[test]
    fn test_parse_drawing_commands_pc() {
        let cmds = parse_drawing_commands("p c");
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], DrawingCommand::Close));
    }

    #[test]
    fn test_parse_drawing_commands_nc() {
        let cmds = parse_drawing_commands("n c");
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], DrawingCommand::Close));
    }

    #[test]
    fn test_parse_drawing_commands_implicit_continuation_after_m() {
        // After "m", bare number pairs become implicit MoveTo
        let cmds = parse_drawing_commands("m 0 0 100 100 200 200");
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[0], DrawingCommand::MoveTo(0.0, 0.0)));
        assert!(matches!(cmds[1], DrawingCommand::MoveTo(100.0, 100.0)));
        assert!(matches!(cmds[2], DrawingCommand::MoveTo(200.0, 200.0)));
    }

    #[test]
    fn test_parse_drawing_commands_implicit_continuation_after_l() {
        // After "l", bare number pairs become implicit LineTo
        let cmds = parse_drawing_commands("m 0 0 l 100 0 100 100 0 100 c");
        assert_eq!(cmds.len(), 5);
        assert!(matches!(cmds[0], DrawingCommand::MoveTo(0.0, 0.0)));
        assert!(matches!(cmds[1], DrawingCommand::LineTo(100.0, 0.0)));
        assert!(matches!(cmds[2], DrawingCommand::LineTo(100.0, 100.0)));
        assert!(matches!(cmds[3], DrawingCommand::LineTo(0.0, 100.0)));
        assert!(matches!(cmds[4], DrawingCommand::Close));
    }

    #[test]
    fn test_parse_drawing_commands_repeat_m() {
        let cmds = parse_drawing_commands("2 m 10 20 30 40");
        assert_eq!(cmds.len(), 2);
        assert!(matches!(cmds[0], DrawingCommand::MoveTo(10.0, 20.0)));
        assert!(matches!(cmds[1], DrawingCommand::MoveTo(30.0, 40.0)));
    }

    #[test]
    fn test_parse_drawing_commands_repeat_l() {
        let cmds = parse_drawing_commands("3 l 10 20 30 40 50 60");
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[0], DrawingCommand::LineTo(10.0, 20.0)));
        assert!(matches!(cmds[1], DrawingCommand::LineTo(30.0, 40.0)));
        assert!(matches!(cmds[2], DrawingCommand::LineTo(50.0, 60.0)));
    }

    #[test]
    fn test_parse_drawing_commands_full_rectangle() {
        // Full canvas rectangle: m 0 0 l 1920 0 1920 1080 0 1080 c
        let cmds = parse_drawing_commands("m 0 0 l 1920 0 1920 1080 0 1080 c");
        assert_eq!(cmds.len(), 5);
        assert!(matches!(cmds[0], DrawingCommand::MoveTo(0.0, 0.0)));
        assert!(matches!(cmds[1], DrawingCommand::LineTo(1920.0, 0.0)));
        assert!(matches!(cmds[2], DrawingCommand::LineTo(1920.0, 1080.0)));
        assert!(matches!(cmds[3], DrawingCommand::LineTo(0.0, 1080.0)));
        assert!(matches!(cmds[4], DrawingCommand::Close));
    }

    #[test]
    fn test_parse_drawing_commands_unknown_tokens_skipped() {
        let cmds = parse_drawing_commands("x y z m 10 20");
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], DrawingCommand::MoveTo(10.0, 20.0)));
    }
}
