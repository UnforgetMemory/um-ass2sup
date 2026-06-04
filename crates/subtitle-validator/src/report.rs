use std::fmt;

/// Severity level of a validation finding.
///
/// Severity is ordered: `Error` > `Warning` > `Info`. This ordering is used for
/// filtering and prioritisation in validation reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Hard error that prevents conversion to SUP/PGS.
    ///
    /// Errors indicate problems like missing `[Script Info]` headers, invalid
    /// encoding, or malformed ASS override tags that cannot be safely ignored.
    Error,
    /// Warning that may cause visual issues during rendering.
    ///
    /// Warnings flag potential problems such as overlapping subtitles, unused
    /// styles, or timing that may clip at certain framerates.
    Warning,
    /// Informational note — no action required.
    ///
    /// Info-level findings provide context about file properties or suggestions
    /// for optimisation (e.g., "3 unused styles detected").
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARN "),
            Severity::Info => write!(f, "INFO "),
        }
    }
}

/// The validation stage where a finding was detected.
///
/// ASS subtitle files pass through multiple validation stages in order.
/// Each stage checks a different aspect of the file's correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationStage {
    /// Byte-level encoding checks (UTF-8 validity, BOM detection).
    Encoding,
    /// Structural checks — required sections (`[Script Info]`, `[V4+ Styles]`,
    /// `[Events]`), header fields, and section ordering.
    Structure,
    /// Style validation — font references, colour formats, margin values,
    /// alignment codes, and scale factors.
    Style,
    /// Event validation — dialogue line parsing, override tag syntax, timing
    /// format, and layer/index values.
    Event,
    /// Semantic checks — cross-referencing styles, detecting duplicate events,
    /// and verifying consistency across the file.
    Semantic,
}

impl fmt::Display for ValidationStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationStage::Encoding => write!(f, "encoding"),
            ValidationStage::Structure => write!(f, "structure"),
            ValidationStage::Style => write!(f, "style"),
            ValidationStage::Event => write!(f, "event"),
            ValidationStage::Semantic => write!(f, "semantic"),
        }
    }
}

/// Unique validation rule identifier.
///
/// Each rule is a numeric ID in the range `V001`–`V015`. The prefix `V` is
/// added by the [`Display`](std::fmt::Display) implementation. Constants are provided for all
/// defined rules:
///
/// | Constant | Checks |
/// |----------|--------|
/// | [`RuleId::V001`] | UTF-8 encoding validity |
/// | [`RuleId::V002`] | BOM presence |
/// | [`RuleId::V003`] | Required `[Script Info]` section |
/// | [`RuleId::V004`] | Required `[V4+ Styles]` section |
/// | [`RuleId::V005`] | Required `[Events]` section |
/// | [`RuleId::V006`] | Script info header fields |
/// | [`RuleId::V007`] | Style format validation |
/// | [`RuleId::V008`] | Colour format (`&HBBGGRR&` or `#RRGGBB`) |
/// | [`RuleId::V009`] | Dialogue line format |
/// | [`RuleId::V010`] | Override tag syntax |
/// | [`RuleId::V011`] | Timing format (`H:MM:SS.CC`) |
/// | [`RuleId::V012`] | Style reference validity |
/// | [`RuleId::V013`] | Overlap detection |
/// | [`RuleId::V014`] | Karaoke override tag consistency |
/// | [`RuleId::V015`] | Semantic consistency |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleId(pub u16);

impl RuleId {
    /// UTF-8 encoding validity check.
    pub const V001: RuleId = RuleId(1);
    /// BOM presence check.
    pub const V002: RuleId = RuleId(2);
    /// Required `[Script Info]` section check.
    pub const V003: RuleId = RuleId(3);
    /// Required `[V4+ Styles]` section check.
    pub const V004: RuleId = RuleId(4);
    /// Required `[Events]` section check.
    pub const V005: RuleId = RuleId(5);
    /// Script info header fields check.
    pub const V006: RuleId = RuleId(6);
    /// Style format validation check.
    pub const V007: RuleId = RuleId(7);
    /// Colour format (`&HBBGGRR&` or `#RRGGBB`) check.
    pub const V008: RuleId = RuleId(8);
    /// Dialogue line format check.
    pub const V009: RuleId = RuleId(9);
    /// Override tag syntax check.
    pub const V010: RuleId = RuleId(10);
    /// Timing format (`H:MM:SS.CC`) check.
    pub const V011: RuleId = RuleId(11);
    /// Style reference validity check.
    pub const V012: RuleId = RuleId(12);
    /// Overlap detection check.
    pub const V013: RuleId = RuleId(13);
    /// Unused style detection check.
    pub const V014: RuleId = RuleId(14);
    /// Semantic consistency check.
    pub const V015: RuleId = RuleId(15);
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V{:03}", self.0)
    }
}

/// A single validation finding produced by a rule check.
///
/// Each finding captures what was detected, where in the file, and what
/// severity it carries. Findings are collected into a [`ValidationReport`].
#[derive(Debug, Clone)]
pub struct ValidationFinding {
    /// The rule that produced this finding.
    pub rule_id: RuleId,
    /// The validation stage where the finding was detected.
    pub stage: ValidationStage,
    /// Severity level (Error, Warning, or Info).
    pub severity: Severity,
    /// 1-based line number in the source file, if applicable.
    pub line: Option<u32>,
    /// 1-based column number in the source file, if applicable.
    pub column: Option<u32>,
    /// Human-readable description of the finding.
    pub message: String,
    /// Optional suggestion for fixing the issue.
    pub suggestion: Option<String>,
}

impl fmt::Display for ValidationFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} {}", self.rule_id, self.severity, self.message)?;
        if let Some(line) = self.line {
            write!(f, " (line {})", line)?;
        }
        if let Some(suggestion) = &self.suggestion {
            write!(f, "\n  → {}", suggestion)?;
        }
        Ok(())
    }
}

/// Severity level for subtitle overlap warnings.
///
/// Overlap severity is determined by both temporal duration and positional
/// proximity. Karaoke events receive reduced severity since overlapping
/// syllables are intentional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OverlapSeverity {
    /// Full visual overlap at the same screen position — always indicates a
    /// problem that will cause garbled text.
    Critical,
    /// Significant overlap (>200ms) at the same position — likely unintentional.
    High,
    /// Partial overlap at the same position — may be acceptable depending on
    /// content (e.g., karaoke transitions).
    Medium,
    /// Overlap at different screen positions — may be intentional (e.g., top and
    /// bottom subtitles displayed simultaneously).
    Low,
}

impl fmt::Display for OverlapSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OverlapSeverity::Critical => write!(f, "CRITICAL"),
            OverlapSeverity::High => write!(f, "HIGH"),
            OverlapSeverity::Medium => write!(f, "MEDIUM"),
            OverlapSeverity::Low => write!(f, "LOW"),
        }
    }
}

/// Describes an overlap between two subtitle events.
///
/// Overlaps are detected by comparing the end time of one event against the
/// start time of the next. Both temporal overlap (shared time window) and
/// visual overlap (shared screen position) are considered.
#[derive(Debug, Clone)]
pub struct OverlapWarning {
    /// Index of the earlier event in the event list.
    pub event_a_idx: usize,
    /// Index of the later event in the event list.
    pub event_b_idx: usize,
    /// Start time of the overlapping region (milliseconds from file start).
    pub overlap_start: u64,
    /// Duration of the overlap in milliseconds.
    pub overlap_duration: u64,
    /// Whether both events render at approximately the same screen position.
    pub visual_overlap: bool,
    /// Severity classification of this overlap.
    pub severity: OverlapSeverity,
    /// Whether one or both events use karaoke effects (which tolerate overlaps).
    pub karaoke_involved: bool,
    /// Human-readable suggestion for resolving the overlap.
    pub suggestion: String,
}

impl fmt::Display for OverlapWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] Overlap between event #{} and #{}: {}ms at {}ms{}",
            self.severity,
            self.event_a_idx,
            self.event_b_idx,
            self.overlap_duration,
            self.overlap_start,
            if self.karaoke_involved {
                " (karaoke)"
            } else {
                ""
            }
        )
    }
}

/// Configuration for overlap detection behaviour.
///
/// Controls how overlaps are detected and classified. Three presets are provided:
///
/// - [`OverlapConfig::default`] — balanced: 100ms threshold, ignores karaoke,
///   50px position proximity
/// - [`OverlapConfig::strict`] — reports all overlaps (0ms threshold, 100px proximity)
/// - [`OverlapConfig::lenient`] — only critical overlaps (500ms threshold, 30px proximity)
#[derive(Debug, Clone)]
pub struct OverlapConfig {
    /// When `true`, report all overlaps regardless of duration.
    pub strict: bool,
    /// Minimum overlap duration (ms) to report. Overlaps shorter than this are
    /// suppressed unless `strict` is `true`.
    pub min_duration_ms: u64,
    /// When `true`, check positional overlap in addition to temporal overlap.
    pub check_visual: bool,
    /// When `true`, suppress overlap warnings for karaoke events.
    pub ignore_karaoke: bool,
    /// Maximum pixel distance between event positions to consider them "same
    /// position" for visual overlap detection.
    pub position_threshold: f64,
    /// Maximum number of simultaneous events at the same position before
    /// triggering a warning.
    pub max_simultaneous_same_pos: usize,
}

impl Default for OverlapConfig {
    fn default() -> Self {
        Self {
            strict: false,
            min_duration_ms: 100,
            check_visual: true,
            ignore_karaoke: true,
            position_threshold: 50.0,
            max_simultaneous_same_pos: 1,
        }
    }
}

impl OverlapConfig {
    /// Creates a strict configuration that reports all overlaps.
    ///
    /// In strict mode, every temporal overlap is reported regardless of duration,
    /// position proximity is widened to 100px, and karaoke overlaps are not
    /// suppressed.
    pub fn strict() -> Self {
        Self {
            strict: true,
            min_duration_ms: 0,
            check_visual: true,
            ignore_karaoke: false,
            position_threshold: 100.0,
            max_simultaneous_same_pos: 1,
        }
    }

    /// Creates a lenient configuration that only reports critical overlaps.
    ///
    /// In lenient mode, only overlaps of 500ms or longer are reported, position
    /// proximity is narrowed to 30px, karaoke overlaps are ignored, and up to
    /// 2 simultaneous events at the same position are allowed.
    pub fn lenient() -> Self {
        Self {
            strict: false,
            min_duration_ms: 500,
            check_visual: true,
            ignore_karaoke: true,
            position_threshold: 30.0,
            max_simultaneous_same_pos: 2,
        }
    }
}

/// Summary statistics for a validation run.
///
/// Counts are accumulated as findings and overlaps are added to a
/// [`ValidationReport`]. All counters start at zero via [`Default`].
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    /// Total number of dialogue events processed.
    pub total_events: usize,
    /// Total number of styles defined in the file.
    pub total_styles: usize,
    /// Number of error-severity findings.
    pub total_errors: usize,
    /// Number of warning-severity findings.
    pub total_warnings: usize,
    /// Number of info-severity findings.
    pub total_infos: usize,
    /// Total number of overlap warnings detected.
    pub total_overlaps: usize,
    /// Number of critical or high severity overlaps.
    pub critical_overlaps: usize,
    /// Number of events using karaoke effects.
    pub karaoke_events: usize,
    /// Number of events using non-karaoke effects.
    pub effect_events: usize,
}

/// The complete validation report for an ASS file.
///
/// A `ValidationReport` is produced by [`Validator::validate`](crate::Validator::validate)
/// and contains all findings, overlap warnings, and summary statistics from a
/// single validation run.
///
/// # Examples
///
/// ```rust,no_run
/// use ass_parser::AssFile;
/// use subtitle_validator::Validator;
///
/// let ass = AssFile::parse_file(std::path::Path::new("subtitles.ass")).unwrap();
/// let report = Validator::new().validate(&ass);
///
/// // Check validity
/// assert!(report.is_valid || !report.errors().is_empty());
///
/// // Get summary
/// println!("{}", report.summary());
///
/// // Full formatted output
/// println!("{}", report.display());
/// ```
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// All validation findings (errors, warnings, and info).
    pub findings: Vec<ValidationFinding>,
    /// All overlap warnings detected between subtitle events.
    pub overlaps: Vec<OverlapWarning>,
    /// Aggregate statistics from the validation run.
    pub stats: ValidationStats,
    /// Whether the file passed validation (no errors found).
    pub is_valid: bool,
}

impl ValidationReport {
    /// Creates an empty validation report with default statistics.
    ///
    /// The report starts with `is_valid: true`. Adding any error-severity
    /// finding will set it to `false`.
    pub fn new() -> Self {
        Self {
            findings: Vec::new(),
            overlaps: Vec::new(),
            stats: ValidationStats::default(),
            is_valid: true,
        }
    }

    /// Adds a validation finding to the report.
    ///
    /// Automatically increments the appropriate counter in [`ValidationStats`]
    /// and sets [`is_valid`](Self::is_valid) to `false` if the finding is an
    /// error.
    ///
    /// # Arguments
    ///
    /// * `finding` - The [`ValidationFinding`] to record.
    pub fn add_finding(&mut self, finding: ValidationFinding) {
        if finding.severity == Severity::Error {
            self.is_valid = false;
            self.stats.total_errors += 1;
        } else if finding.severity == Severity::Warning {
            self.stats.total_warnings += 1;
        } else {
            self.stats.total_infos += 1;
        }
        self.findings.push(finding);
    }

    /// Adds an overlap warning to the report.
    ///
    /// Increments the overlap counters in [`ValidationStats`]. Critical and high
    /// severity overlaps are counted separately in
    /// [`critical_overlaps`](ValidationStats::critical_overlaps).
    ///
    /// # Arguments
    ///
    /// * `overlap` - The [`OverlapWarning`] to record.
    pub fn add_overlap(&mut self, overlap: OverlapWarning) {
        if overlap.severity == OverlapSeverity::Critical
            || overlap.severity == OverlapSeverity::High
        {
            self.stats.critical_overlaps += 1;
        }
        self.stats.total_overlaps += 1;
        self.overlaps.push(overlap);
    }

    /// Returns all findings with `Error` severity.
    ///
    /// Errors indicate problems that prevent safe conversion to SUP/PGS format.
    pub fn errors(&self) -> Vec<&ValidationFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect()
    }

    /// Returns all findings with `Warning` severity.
    ///
    /// Warnings indicate potential issues that may affect rendering quality
    /// but do not block conversion.
    pub fn warnings(&self) -> Vec<&ValidationFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .collect()
    }

    /// Returns a one-line summary of the validation results.
    ///
    /// Format: `"Validation: N errors, N warnings, N info, N overlaps (N critical)"`
    pub fn summary(&self) -> String {
        format!(
            "Validation: {} errors, {} warnings, {} info, {} overlaps ({} critical)",
            self.stats.total_errors,
            self.stats.total_warnings,
            self.stats.total_infos,
            self.stats.total_overlaps,
            self.stats.critical_overlaps,
        )
    }

    /// Returns a formatted multi-line string with all findings and overlaps.
    ///
    /// The output includes a summary header, all validation findings, and
    /// any overlap warnings in a human-readable format suitable for terminal
    /// display.
    pub fn display(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}\n", self.summary()));
        out.push_str(&"─".repeat(70).to_string());
        out.push('\n');

        for finding in &self.findings {
            out.push_str(&format!("{}\n", finding));
        }

        if !self.overlaps.is_empty() {
            out.push_str("\n─── Overlap Warnings ───\n");
            for overlap in &self.overlaps {
                out.push_str(&format!("{}\n", overlap));
            }
        }

        out
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}
