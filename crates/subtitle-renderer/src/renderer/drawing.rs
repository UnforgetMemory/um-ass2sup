pub(super) enum DrawingCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    BezierTo(f32, f32, f32, f32, f32, f32),
    Close,
}

pub(super) fn parse_drawing_level(text: &str) -> u8 {
    for tag_block in text.chars().collect::<Vec<_>>().windows(4) {
        if tag_block[0] == '\\' && tag_block[1] == 'p' {
            if let Some(d) = tag_block.get(2).and_then(|c| c.to_digit(10)) {
                return d as u8;
            }
        }
    }
    0
}

pub(super) fn parse_drawing_commands(text: &str) -> Vec<DrawingCommand> {
    let mut commands = Vec::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    let mut last_cmd: Option<&str> = None;

    while i < tokens.len() {
        let token = tokens[i];
        if token.len() == 1 {
            match token {
                "m" => {
                    if i + 2 < tokens.len() {
                        if let (Ok(x), Ok(y)) = (tokens[i + 1].parse::<f32>(), tokens[i + 2].parse::<f32>()) {
                            commands.push(DrawingCommand::MoveTo(x, y));
                            last_cmd = Some("m");
                            i += 3;
                            continue;
                        }
                    }
                }
                "l" => {
                    if i + 2 < tokens.len() {
                        if let (Ok(x), Ok(y)) = (tokens[i + 1].parse::<f32>(), tokens[i + 2].parse::<f32>()) {
                            commands.push(DrawingCommand::LineTo(x, y));
                            last_cmd = Some("l");
                            i += 3;
                            continue;
                        }
                    }
                }
                "b" => {
                    if i + 6 < tokens.len() {
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
                }
                "p" | "n" => {
                    if i + 1 < tokens.len() {
                        if tokens[i + 1] == "c" {
                            commands.push(DrawingCommand::Close);
                            last_cmd = None;
                            i += 2;
                            continue;
                        }
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
                        for _ in 0.._repeat {
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
                                            commands.push(DrawingCommand::BezierTo(nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]));
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
