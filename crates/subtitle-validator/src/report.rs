use std::fmt;

/// Severity of a validation finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Hard error that prevents conversion
    Error,
    /// Warning that may cause visual issues
    Warning,
    /// Informational note
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

/// Validation stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationStage {
    Encoding,
    Structure,
    Style,
    Event,
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

/// Unique rule identifier (V001..V099)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleId(pub u16);

impl RuleId {
    pub const V001: RuleId = RuleId(1);
    pub const V002: RuleId = RuleId(2);
    pub const V003: RuleId = RuleId(3);
    pub const V004: RuleId = RuleId(4);
    pub const V005: RuleId = RuleId(5);
    pub const V006: RuleId = RuleId(6);
    pub const V007: RuleId = RuleId(7);
    pub const V008: RuleId = RuleId(8);
    pub const V009: RuleId = RuleId(9);
    pub const V010: RuleId = RuleId(10);
    pub const V011: RuleId = RuleId(11);
    pub const V012: RuleId = RuleId(12);
    pub const V013: RuleId = RuleId(13);
    pub const V014: RuleId = RuleId(14);
    pub const V015: RuleId = RuleId(15);
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V{:03}", self.0)
    }
}

/// A single validation finding
#[derive(Debug, Clone)]
pub struct ValidationFinding {
    pub rule_id: RuleId,
    pub stage: ValidationStage,
    pub severity: Severity,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
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

/// Overlap severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OverlapSeverity {
    /// Full visual overlap at same position — always bad
    Critical,
    /// Significant overlap (>200ms) at same position
    High,
    /// Partial overlap at same position
    Medium,
    /// Overlap at different positions — may be intentional
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

/// Describes an overlap between two subtitle events
#[derive(Debug, Clone)]
pub struct OverlapWarning {
    /// Index of first event
    pub event_a_idx: usize,
    /// Index of second event
    pub event_b_idx: usize,
    /// Start time of overlap (ms)
    pub overlap_start: u64,
    /// Duration of overlap (ms)
    pub overlap_duration: u64,
    /// Whether events visually overlap (same approximate screen position)
    pub visual_overlap: bool,
    /// Severity of the overlap
    pub severity: OverlapSeverity,
    /// Whether one or both events are karaoke (which tolerates overlaps)
    pub karaoke_involved: bool,
    /// Human-readable suggestion
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
            if self.karaoke_involved { " (karaoke)" } else { "" }
        )
    }
}

/// Overlap detection configuration
#[derive(Debug, Clone)]
pub struct OverlapConfig {
    /// Strict mode: warn on ALL overlaps
    pub strict: bool,
    /// Minimum overlap duration to report (ms)
    pub min_duration_ms: u64,
    /// Whether to check visual (positional) overlap
    pub check_visual: bool,
    /// Whether to ignore karaoke-related overlaps
    pub ignore_karaoke: bool,
    /// Position proximity threshold for "same position" (pixels)
    pub position_threshold: f64,
    /// Maximum allowed simultaneous events at same position
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
    /// Strict mode: reports all overlaps
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

    /// Lenient mode: only critical overlaps
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

/// Validation statistics
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    pub total_events: usize,
    pub total_styles: usize,
    pub total_errors: usize,
    pub total_warnings: usize,
    pub total_infos: usize,
    pub total_overlaps: usize,
    pub critical_overlaps: usize,
    pub karaoke_events: usize,
    pub effect_events: usize,
}

/// The complete validation report
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub findings: Vec<ValidationFinding>,
    pub overlaps: Vec<OverlapWarning>,
    pub stats: ValidationStats,
    pub is_valid: bool,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            findings: Vec::new(),
            overlaps: Vec::new(),
            stats: ValidationStats::default(),
            is_valid: true,
        }
    }

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

    pub fn add_overlap(&mut self, overlap: OverlapWarning) {
        if overlap.severity == OverlapSeverity::Critical || overlap.severity == OverlapSeverity::High {
            self.stats.critical_overlaps += 1;
        }
        self.stats.total_overlaps += 1;
        self.overlaps.push(overlap);
    }

    pub fn errors(&self) -> Vec<&ValidationFinding> {
        self.findings.iter().filter(|f| f.severity == Severity::Error).collect()
    }

    pub fn warnings(&self) -> Vec<&ValidationFinding> {
        self.findings.iter().filter(|f| f.severity == Severity::Warning).collect()
    }

    /// Summary line
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

    /// Print all findings in a formatted way
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
