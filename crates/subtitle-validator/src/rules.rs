use std::collections::HashMap;

use ass_core::{Event, EventType, OverrideTag, ScriptMetadata, Style, SubtitleDocument};

use crate::report::{
    OverlapConfig, OverlapSeverity, OverlapWarning, RuleId, Severity, ValidationFinding,
    ValidationReport, ValidationStage,
};

/// Runs all validation stages (encoding, structure, style, event, semantic)
/// plus overlap detection against the parsed ASS file, returning a
/// [`ValidationReport`] describing all findings and statistics.
pub fn validate(ass: &SubtitleDocument, overlap_config: &OverlapConfig) -> ValidationReport {
    let mut report = ValidationReport::new();

    // Stage 1: Encoding checks
    validate_encoding(&ass.metadata, &mut report);

    // Stage 2: Structure checks
    validate_structure(ass, &mut report);

    // Stage 3: Style checks
    validate_styles(&ass.styles, &mut report);

    // Stage 4: Event checks
    validate_events(&ass.events, &ass.styles, &mut report);

    // Stage 5: Semantic checks
    validate_semantics(ass, &mut report);

    // Overlap detection (separate from rules)
    detect_overlaps(&ass.events, overlap_config, &mut report);

    report.stats.total_events = ass.events.len();
    report.stats.total_styles = ass.styles.len();
    report.stats.karaoke_events = ass.events.iter().filter(|e| !e.karaoke.is_empty()).count();
    report.stats.effect_events = ass.events.iter().filter(|e| has_effects(e)).count();

    report
}

// ─────────────────────── Stage 1: Encoding ───────────────────────

fn validate_encoding(info: &ScriptMetadata, report: &mut ValidationReport) {
    // V001: Check script type
    if info.script_type != "v4.00" && info.script_type != "v4.00+" {
        report.add_finding(ValidationFinding {
            rule_id: RuleId::V001,
            stage: ValidationStage::Encoding,
            severity: Severity::Warning,
            line: None,
            column: None,
            message: format!(
                "Unrecognized ScriptType: '{}', expected v4.00+",
                info.script_type
            ),
            suggestion: Some("Set ScriptType to v4.00+ for ASS v4+".to_string()),
        });
    }

    // V002: Check play resolution
    if info.play_res_x == 0 || info.play_res_y == 0 {
        report.add_finding(ValidationFinding {
            rule_id: RuleId::V002,
            stage: ValidationStage::Encoding,
            severity: Severity::Error,
            line: None,
            column: None,
            message: format!(
                "Invalid PlayResX/PlayResY: {}x{}",
                info.play_res_x, info.play_res_y
            ),
            suggestion: Some("Set valid resolution, e.g. 1920x1080".to_string()),
        });
    }
}

// ─────────────────────── Stage 2: Structure ───────────────────────

fn validate_structure(ass: &SubtitleDocument, report: &mut ValidationReport) {
    // V003: No events
    if ass.events.is_empty() {
        report.add_finding(ValidationFinding {
            rule_id: RuleId::V003,
            stage: ValidationStage::Structure,
            severity: Severity::Error,
            line: None,
            column: None,
            message: "No subtitle events found".to_string(),
            suggestion: None,
        });
    }

    // V004: No styles
    if ass.styles.is_empty() {
        report.add_finding(ValidationFinding {
            rule_id: RuleId::V004,
            stage: ValidationStage::Structure,
            severity: Severity::Warning,
            line: None,
            column: None,
            message: "No styles defined, will use default".to_string(),
            suggestion: Some("Define at least one style in [V4+ Styles]".to_string()),
        });
    }

    // V005: Duplicate style names
    let mut style_names: HashMap<String, usize> = HashMap::new();
    for (i, style) in ass.styles.iter().enumerate() {
        if let Some(&prev_idx) = style_names.get(style.name.as_str()) {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V005,
                stage: ValidationStage::Structure,
                severity: Severity::Warning,
                line: None,
                column: None,
                message: format!(
                    "Duplicate style '{}' (first at index {}, again at {})",
                    style.name, prev_idx, i
                ),
                suggestion: Some("Rename or remove duplicate style".to_string()),
            });
        }
        style_names.insert(style.name.0.clone(), i);
    }
}

// ─────────────────────── Stage 3: Style ───────────────────────

fn validate_styles(styles: &[Style], report: &mut ValidationReport) {
    for style in styles.iter() {
        // V006: Font size zero or negative
        if style.font_size <= 0.0 {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V006,
                stage: ValidationStage::Style,
                severity: Severity::Warning,
                line: None,
                column: None,
                message: format!(
                    "Style '{}' has invalid font size: {}",
                    style.name, style.font_size
                ),
                suggestion: Some("Set font_size > 0".to_string()),
            });
        }

        // V007: Scale 0
        if style.scale_x <= 0.0 || style.scale_y <= 0.0 {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V007,
                stage: ValidationStage::Style,
                severity: Severity::Warning,
                line: None,
                column: None,
                message: format!(
                    "Style '{}' has invalid scale: {}x{}",
                    style.name, style.scale_x, style.scale_y
                ),
                suggestion: Some("Scale values should be > 0 (1.0 = 100%)".to_string()),
            });
        }

        // V008: Alignment out of range
        if style.alignment.to_u8() < 1 || style.alignment.to_u8() > 11 {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V008,
                stage: ValidationStage::Style,
                severity: Severity::Error,
                line: None,
                column: None,
                message: format!(
                    "Style '{}' has invalid alignment: {}",
                    style.name,
                    style.alignment.to_u8()
                ),
                suggestion: Some("Alignment must be 1-11 (numpad layout)".to_string()),
            });
        }
    }
}

// ─────────────────────── Stage 4: Event ───────────────────────

fn validate_events(events: &[Event], styles: &[Style], report: &mut ValidationReport) {
    let style_names: Vec<&str> = styles.iter().map(|s| s.name.as_str()).collect();

    for (i, event) in events.iter().enumerate() {
        // Only validate Dialogue events
        if event.event_type != EventType::Dialogue {
            continue;
        }

        // V009: Reference to non-existent style
        if !style_names.is_empty() && !style_names.contains(&event.style.as_str()) {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V009,
                stage: ValidationStage::Event,
                severity: Severity::Warning,
                line: None,
                column: None,
                message: format!("Event #{} references undefined style '{}'", i, event.style),
                suggestion: Some(format!("Available styles: {:?}", style_names)),
            });
        }

        // V010: Empty text
        if event.text_raw.trim().is_empty() {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V010,
                stage: ValidationStage::Event,
                severity: Severity::Info,
                line: None,
                column: None,
                message: format!("Event #{} has empty text", i),
                suggestion: None,
            });
        }

        // V011: Start time >= end time
        if event.start_ms >= event.end_ms {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V011,
                stage: ValidationStage::Event,
                severity: Severity::Error,
                line: None,
                column: None,
                message: format!(
                    "Event #{}: start time ({}) >= end time ({})",
                    i,
                    ass_time(event.start_ms),
                    ass_time(event.end_ms)
                ),
                suggestion: Some("Ensure end time is after start time".to_string()),
            });
        }

        // V012: Extremely long duration (>30s)
        let duration = event.end_ms.saturating_sub(event.start_ms);
        if duration > 30000 {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V012,
                stage: ValidationStage::Event,
                severity: Severity::Warning,
                line: None,
                column: None,
                message: format!(
                    "Event #{}: very long duration ({:.1}s)",
                    i,
                    duration as f64 / 1000.0
                ),
                suggestion: Some("Consider splitting long events".to_string()),
            });
        }

        // V013: Unmatched override block braces
        let open = event.text_raw.chars().filter(|&c| c == '{').count();
        let close = event.text_raw.chars().filter(|&c| c == '}').count();
        if open != close {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V013,
                stage: ValidationStage::Event,
                severity: Severity::Error,
                line: None,
                column: None,
                message: format!(
                    "Event #{}: unmatched braces ({} '{{' vs {} '}}')",
                    i, open, close
                ),
                suggestion: Some("Check override tag blocks are properly closed".to_string()),
            });
        }
    }
}

// ─────────────────────── Stage 5: Semantic ───────────────────────

fn validate_semantics(ass: &SubtitleDocument, report: &mut ValidationReport) {
    // V014: Karaoke events with missing karaoke tags
    for (i, event) in ass.events.iter().enumerate() {
        if !event.karaoke.is_empty() {
            let has_k_tag = event
                .override_tags
                .iter()
                .any(|to| matches!(to.tag, OverrideTag::Karaoke { .. }));
            if !has_k_tag {
                report.add_finding(ValidationFinding {
                    rule_id: RuleId::V014,
                    stage: ValidationStage::Semantic,
                    severity: Severity::Warning,
                    line: None,
                    column: None,
                    message: format!(
                        "Event #{}: karaoke segments detected without \\k override tag",
                        i
                    ),
                    suggestion: Some(
                        "Add \\k<duration> override tag or remove karaoke segments".to_string(),
                    ),
                });
            }
        }
    }

    // V015: Check for common encoding issues (mojibake patterns)
    for (i, event) in ass.events.iter().enumerate() {
        let text = &event.text_raw;
        if text.contains('\u{00c3}') || text.contains('\u{00c2}') {
            report.add_finding(ValidationFinding {
                rule_id: RuleId::V015,
                stage: ValidationStage::Semantic,
                severity: Severity::Warning,
                line: None,
                column: None,
                message: format!("Event #{}: possible encoding issue (mojibake detected)", i),
                suggestion: Some(
                    "File may be UTF-8 but read as Latin-1. Check file encoding.".to_string(),
                ),
            });
        }
    }
}

// ─────────────────────── Overlap Detection ───────────────────────

/// Position of a subtitle event (from \\pos tag or default alignment)
#[derive(Debug, Clone, Copy)]
struct EventPosition {
    x: f64,
    y: f64,
    _explicit: bool,
}

impl EventPosition {
    fn distance_to(&self, other: &EventPosition) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Extract the effective position of an event
fn get_event_position(event: &Event, _play_res_x: u32, _play_res_y: u32) -> EventPosition {
    for to in &event.override_tags {
        match &to.tag {
            OverrideTag::Pos { x, y } => {
                return EventPosition {
                    x: *x,
                    y: *y,
                    _explicit: true,
                };
            }
            OverrideTag::Move { x1, y1, .. } => {
                return EventPosition {
                    x: *x1,
                    y: *y1,
                    _explicit: true,
                };
            }
            _ => {}
        }
    }

    // Default position based on alignment
    let alignment = effective_alignment(event).unwrap_or(2);
    let (x, y) = match alignment {
        1 => (100.0, 900.0), // Bottom-left
        2 => (540.0, 900.0), // Bottom-center
        3 => (980.0, 900.0), // Bottom-right
        4 => (100.0, 540.0), // Middle-left
        5 => (540.0, 540.0), // Middle-center
        6 => (980.0, 540.0), // Middle-right
        7 => (100.0, 180.0), // Top-left
        8 => (540.0, 180.0), // Top-center
        9 => (980.0, 180.0), // Top-right
        // ASS v4+ alignment overrides
        10 => (540.0, 900.0), // Subtitle (bottom-center)
        11 => (540.0, 180.0), // Top subtitle
        12 => (540.0, 540.0), // Mid subtitle
        _ => (540.0, 900.0),
    };

    EventPosition {
        x,
        y,
        _explicit: false,
    }
}

/// Build an interval-sorted structure and check for overlaps
pub fn detect_overlaps(events: &[Event], config: &OverlapConfig, report: &mut ValidationReport) {
    let play_res_x = 1920u32; // Default for overlap estimation
    let play_res_y = 1080u32;

    // Collect dialogue events with their time intervals
    let dialogue_events: Vec<(usize, &Event)> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.event_type == EventType::Dialogue)
        .collect();

    // O(n²) pairwise comparison — good enough for typical subtitle files
    // (even 10k events = 50M checks, still fast in Rust)
    for i in 0..dialogue_events.len() {
        let (idx_a, event_a) = dialogue_events[i];
        let pos_a = get_event_position(event_a, play_res_x, play_res_y);

        for &(idx_b, event_b) in &dialogue_events[i + 1..] {
            let pos_b = get_event_position(event_b, play_res_x, play_res_y);

            // Time overlap check
            let overlap_start = event_a.start_ms.max(event_b.start_ms);
            let overlap_end = event_a.end_ms.min(event_b.end_ms);

            if overlap_start >= overlap_end {
                continue; // No time overlap
            }

            let overlap_duration = overlap_end - overlap_start;

            // Skip short overlaps
            if overlap_duration < config.min_duration_ms {
                continue;
            }

            // Visual overlap check
            let visual_overlap = if config.check_visual {
                pos_a.distance_to(&pos_b) < config.position_threshold
            } else {
                false
            };

            // Skip non-visual overlaps in non-strict mode
            if !config.strict && !visual_overlap {
                continue;
            }

            // Skip karaoke overlaps if configured
            let karaoke_involved = !event_a.karaoke.is_empty() || !event_b.karaoke.is_empty();
            if config.ignore_karaoke && karaoke_involved {
                continue;
            }

            // Determine severity
            let severity = if visual_overlap
                && overlap_duration == event_a.end_ms - event_a.start_ms
                && overlap_duration == event_b.end_ms - event_b.start_ms
            {
                OverlapSeverity::Critical // Full overlap at same position
            } else if visual_overlap && overlap_duration > 200 {
                OverlapSeverity::High
            } else if visual_overlap {
                OverlapSeverity::Medium
            } else {
                OverlapSeverity::Low
            };

            let suggestion = if visual_overlap {
                format!(
                    "Events #{} and #{} overlap for {}ms at the same position. \
                     Adjust timing or move one event.",
                    idx_a, idx_b, overlap_duration
                )
            } else {
                format!(
                    "Events #{} and #{} overlap for {}ms at different positions.",
                    idx_a, idx_b, overlap_duration
                )
            };

            report.add_overlap(OverlapWarning {
                event_a_idx: idx_a,
                event_b_idx: idx_b,
                overlap_start,
                overlap_duration,
                visual_overlap,
                severity,
                karaoke_involved,
                suggestion,
            });
        }
    }
}

/// Extract effective alignment from override tags
fn effective_alignment(event: &Event) -> Option<u8> {
    for to in &event.override_tags {
        match &to.tag {
            OverrideTag::AlignmentVsfilter(a) | OverrideTag::AlignmentNumpad(a) => {
                return Some(*a);
            }
            _ => {}
        }
    }
    None
}

/// Check if event has visual effects
fn has_effects(event: &Event) -> bool {
    event.override_tags.iter().any(|to| {
        matches!(
            &to.tag,
            OverrideTag::Fade { .. }
                | OverrideTag::FadeComplex { .. }
                | OverrideTag::Transform { .. }
                | OverrideTag::Blur(_)
                | OverrideTag::GaussianBlur(_)
                | OverrideTag::Move { .. }
        )
    })
}

/// Format milliseconds as ASS time string H:MM:SS.CC
fn ass_time(ms: u64) -> String {
    let total_cs = ms / 10;
    let cs = total_cs % 100;
    let total_secs = total_cs / 100;
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = total_secs / 3600;
    format!("{}:{:02}:{:02}.{:02}", hours, mins, secs, cs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ass_core::{
        Effect, KaraokeSegment, KaraokeStyle, ScriptMetadata, SubtitleDocument, SubtitleFormat,
    };

    #[test]
    fn test_v014_non_karaoke_no_finding() {
        let ass = SubtitleDocument {
            format: SubtitleFormat::Ass,
            metadata: ScriptMetadata::default(),
            styles: vec![],
            events: vec![Event {
                event_type: EventType::Dialogue,
                layer: 0,
                start_ms: 0,
                end_ms: 5000,
                style: "Default".into(),
                actor: String::new(),
                margin_l: Some(0),
                margin_r: Some(0),
                margin_v: Some(0),
                effect: Effect::None,
                text_raw: "Hello World".into(),
                override_tags: vec![],
                karaoke: vec![],
                source_line: 0,
            }],
            fonts: vec![],
            warnings: vec![],
        };
        let mut report = ValidationReport::new();
        validate_semantics(&ass, &mut report);
        let v014: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.rule_id == RuleId::V014)
            .collect();
        assert!(v014.is_empty(), "Non-karaoke event should not trigger V014");
    }

    #[test]
    fn test_v014_parsed_karaoke_no_finding() {
        // A parsed karaoke event with {\k50} has both karaoke_segments AND
        // OverrideTag::Karaoke — no V014 should fire (consistent state)
        let ass = SubtitleDocument::parse(
            "[Script Info]\n\
             ScriptType: v4.00+\n\
             PlayResX: 1920\n\
             PlayResY: 1080\n\
             \n\
             [V4+ Styles]\n\
             Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
             OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, \
             ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, \
             Alignment, MarginL, MarginR, MarginV, Encoding\n\
             Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,\
             0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1\n\
             \n\
             [Events]\n\
             Format: Layer, Start, End, Style, Name, MarginL, MarginR, \
             MarginV, Effect, Text\n\
             Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\k50}lyrics\n",
        )
        .unwrap();
        let mut report = ValidationReport::new();
        validate_semantics(&ass, &mut report);
        let v014: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.rule_id == RuleId::V014)
            .collect();
        assert!(
            v014.is_empty(),
            "Parsed karaoke event with \\k tag should not trigger V014: {:?}",
            v014
        );
    }

    #[test]
    fn test_v014_inconsistent_karaoke_triggers_warning() {
        // Event with karaoke_segments but no OverrideTag::Karaoke => V014
        let ass = SubtitleDocument {
            format: SubtitleFormat::Ass,
            metadata: ScriptMetadata::default(),
            styles: vec![],
            events: vec![Event {
                event_type: EventType::Dialogue,
                layer: 0,
                start_ms: 0,
                end_ms: 1000,
                style: "Default".into(),
                actor: String::new(),
                margin_l: Some(0),
                margin_r: Some(0),
                margin_v: Some(0),
                effect: Effect::None,
                text_raw: "inconsistent".into(),
                override_tags: vec![],
                karaoke: vec![KaraokeSegment::new(
                    KaraokeStyle::Instant,
                    500,
                    "ly".into(),
                    0,
                )],
                source_line: 0,
            }],
            fonts: vec![],
            warnings: vec![],
        };
        let mut report = ValidationReport::new();
        validate_semantics(&ass, &mut report);
        let v014: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.rule_id == RuleId::V014)
            .collect();
        assert_eq!(v014.len(), 1, "Should trigger exactly one V014 warning");
        assert_eq!(v014[0].severity, Severity::Warning);
        assert_eq!(v014[0].stage, ValidationStage::Semantic);
        assert!(
            v014[0].message.contains("karaoke segments detected"),
            "Message should mention karaoke segments: {}",
            v014[0].message
        );
        assert!(v014[0].suggestion.is_some());
        assert!(
            v014[0]
                .suggestion
                .as_ref()
                .unwrap()
                .contains("\\k<duration>"),
            "Suggestion should mention \\k<duration>"
        );
    }
}
