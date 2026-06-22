//! Line-level lexer for ASS/SSA/SRT files.
//!
//! Splits input into lines, identifies section headers, and classifies each line.
//! Pure tokenization — no semantic understanding.
//!
//! The lexer handles:
//! - UTF-8 BOM stripping
//! - `\r\n` / `\n` normalization
//! - Section header extraction (`[Section Name]`)
//! - Comment lines (`;` or `!` prefix)
//! - Line-level span tracking (line number)

use crate::Span;

/// Classification of a single line.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `[Section Name]` — content is the name between brackets.
    SectionHeader(String),
    /// `Key: Value` pair.
    KeyValue { key: String, value: String },
    /// `Style: ...` data (raw after `Style:` prefix).
    StyleData(String),
    /// `Dialogue:` / `Comment:` / etc. line (raw after prefix).
    EventData {
        /// Event type string (e.g. "Dialogue", "Comment").
        type_name: String,
        /// Everything after the colon.
        data: String,
    },
    /// `fontname: ...` or `filename: ...` in `[Fonts]`.
    FontLine(String),
    /// Comment line (`;` or `!` prefix).
    Comment,
    /// Empty/whitespace-only line.
    Empty,
    /// Unrecognised line in current section context.
    Unknown(String),
}

/// A line together with its source position.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub token: Token,
    pub span: Span,
}

/// Lex the input into a sequence of line tokens.
///
/// Strips BOM, normalizes line endings, and classifies each line.
/// Span tracking is 1-based: line 1 is the first input line.
pub fn lex(input: &str) -> Vec<Line> {
    // Strip UTF-8 BOM
    let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let mut lines = Vec::new();
    let mut line_number: u32 = 0;

    for raw in input.lines() {
        line_number += 1;
        let span = Span::new(line_number, 1, raw.len() as u32);
        let trimmed = raw.trim();

        // Empty / whitespace-only
        if trimmed.is_empty() {
            lines.push(Line {
                token: Token::Empty,
                span,
            });
            continue;
        }

        // Comment
        if trimmed.starts_with(';') || trimmed.starts_with('!') {
            lines.push(Line {
                token: Token::Comment,
                span,
            });
            continue;
        }

        // Section header
        if trimmed.starts_with('[') {
            let content = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            // Strip inline comment after `]`
            let section_name = content.split(';').next().unwrap_or("").trim().to_string();
            lines.push(Line {
                token: Token::SectionHeader(section_name),
                span,
            });
            continue;
        }

        // Key: Value (Script Info)
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            let upper_key = key.to_uppercase();

            // Style: lines
            if upper_key == "STYLE" && !value.is_empty() {
                lines.push(Line {
                    token: Token::StyleData(value.to_string()),
                    span,
                });
                continue;
            }

            // Event lines (Dialogue, Comment, Picture, Sound, Movie, Command)
            if matches!(
                upper_key.as_str(),
                "DIALOGUE" | "COMMENT" | "PICTURE" | "SOUND" | "MOVIE" | "COMMAND"
            ) {
                lines.push(Line {
                    token: Token::EventData {
                        type_name: key.to_string(),
                        data: value.to_string(),
                    },
                    span,
                });
                continue;
            }

            // Font lines (fontname:, filename:)
            if upper_key.starts_with("FONTNAME") || upper_key.starts_with("FILENAME") {
                lines.push(Line {
                    token: Token::FontLine(trimmed.to_string()),
                    span,
                });
                continue;
            }

            // Generic KeyValue
            lines.push(Line {
                token: Token::KeyValue {
                    key: key.to_string(),
                    value: value.to_string(),
                },
                span,
            });
            continue;
        }

        // Fallback: unrecognised
        lines.push(Line {
            token: Token::Unknown(trimmed.to_string()),
            span,
        });
    }

    lines
}

/// Filter lines belonging to a specific section context.
pub struct SectionIter<'a> {
    lines: &'a [Line],
    pos: usize,
}

impl<'a> SectionIter<'a> {
    pub fn new(lines: &'a [Line]) -> Self {
        Self { lines, pos: 0 }
    }

    /// Advance to the next section and return its header + range.
    pub fn next_section(&mut self) -> Option<(&'a str, &'a [Line])> {
        // Find next section header
        while self.pos < self.lines.len() {
            let line = &self.lines[self.pos];
            if matches!(&line.token, Token::SectionHeader(_)) {
                let header = match &line.token {
                    Token::SectionHeader(name) => name.as_str(),
                    _ => unreachable!(),
                };
                self.pos += 1;
                let start = self.pos;

                // Collect lines until next section header or end
                while self.pos < self.lines.len() {
                    let next = &self.lines[self.pos];
                    if matches!(&next.token, Token::SectionHeader(_)) {
                        break;
                    }
                    self.pos += 1;
                }

                let section_lines = &self.lines[start..self.pos];
                return Some((header, section_lines));
            }
            self.pos += 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        let lines = lex("");
        assert!(lines.is_empty());
    }

    #[test]
    fn whitespace_only() {
        let lines = lex("  \n  \n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].token, Token::Empty);
    }

    #[test]
    fn bom_stripped() {
        let input = "\u{FEFF}[Script Info]\nTitle: Test";
        let lines = lex(input);
        assert_eq!(lines.len(), 2);
        assert!(matches!(lines[0].token, Token::SectionHeader(ref n) if n == "Script Info"));
    }

    #[test]
    fn section_headers() {
        let input = "[Script Info]\n[V4+ Styles]\n[Events]\n";
        let lines = lex(input);
        let headers: Vec<&str> = lines
            .iter()
            .filter_map(|l| {
                if let Token::SectionHeader(ref n) = l.token {
                    Some(n.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(headers, vec!["Script Info", "V4+ Styles", "Events"]);
    }

    #[test]
    fn section_header_with_inline_comment() {
        let input = "[Script Info; this is a comment]";
        let lines = lex(input);
        assert!(matches!(&lines[0].token, Token::SectionHeader(n) if n == "Script Info"));
    }

    #[test]
    fn key_value_script_info() {
        let input = "[Script Info]\nTitle: Test\nPlayResX: 1920";
        let lines = lex(input);
        assert_eq!(lines.len(), 3);
        assert!(matches!(&lines[1].token, Token::KeyValue { key, .. } if key == "Title"));
        assert!(matches!(&lines[2].token, Token::KeyValue { key, .. } if key == "PlayResX"));
    }

    #[test]
    fn style_line() {
        let input = "[V4+ Styles]\nStyle: Default,Arial,48,&H00FFFFFF";
        let lines = lex(input);
        assert_eq!(lines.len(), 2);
        assert!(matches!(&lines[1].token, Token::StyleData(d) if d.starts_with("Default")));
    }

    #[test]
    fn dialogue_event() {
        let input = "[Events]\nDialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello";
        let lines = lex(input);
        assert_eq!(lines.len(), 2);
        assert!(
            matches!(&lines[1].token, Token::EventData { type_name, .. } if type_name == "Dialogue")
        );
    }

    #[test]
    fn comment_line() {
        let input = "; This is a comment\n! Another comment";
        let lines = lex(input);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].token, Token::Comment);
        assert_eq!(lines[1].token, Token::Comment);
    }

    #[test]
    fn font_line() {
        let input = "[Fonts]\nfontname: Arial, filename: arial.ttf";
        let lines = lex(input);
        assert_eq!(lines.len(), 2);
        assert!(matches!(&lines[1].token, Token::FontLine(_)));
    }

    #[test]
    fn known_event_types() {
        for et in &[
            "Dialogue", "Comment", "Picture", "Sound", "Movie", "Command",
        ] {
            let input = format!("{et}: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,test");
            let lines = lex(&input);
            assert!(matches!(&lines[0].token, Token::EventData { .. }));
        }
    }

    #[test]
    fn case_insensitive_style_detection() {
        let input = "style: Default,Arial,20";
        let lines = lex(input);
        assert!(matches!(&lines[0].token, Token::StyleData(_)));
    }

    #[test]
    fn line_number_tracking() {
        let input = "line1\nline2\n[Section]\nline3";
        let lines = lex(input);
        assert_eq!(lines[0].span, Span::new(1, 1, 5));
        assert_eq!(lines[2].span, Span::new(3, 1, 9));
    }

    #[test]
    fn section_iter_basic() {
        let input = "[A]\nx=1\n[B]\ny=2\nz=3";
        let lines = lex(input);
        let mut iter = SectionIter::new(&lines);
        let (name, content) = iter.next_section().unwrap();
        assert_eq!(name, "A");
        assert_eq!(content.len(), 1);
        let (name, content) = iter.next_section().unwrap();
        assert_eq!(name, "B");
        assert_eq!(content.len(), 2);
        assert!(iter.next_section().is_none());
    }
}
