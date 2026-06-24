//! # subtitle-validator

#![warn(missing_docs)]
//!
//! ASS/SSA subtitle syntax validation and overlap detection for the ass2sup
//! conversion pipeline. This crate checks subtitle files for structural errors,
//! style inconsistencies, encoding problems, and temporal overlaps between events.
//!
//! ## Overview
//!
//! The validator runs a series of rules ([`report::ValidationStage`]) against a
//! parsed [`ass_core::SubtitleDocument`] and produces a [`report::ValidationReport`]
//! containing findings, overlap warnings, and summary statistics.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ass_core::SubtitleDocument;
//! use subtitle_validator::Validator;
//!
//! let ass = SubtitleDocument::parse(&std::fs::read_to_string("subtitles.ass").unwrap()).unwrap();
//! let report = Validator::new().validate(&ass);
//! if !report.is_valid {
//!     eprintln!("{}", report.display());
//! }
//! ```

/// Validation report types and statistics.
pub mod report;
/// Validation rule implementations.
pub mod rules;

use ass_core::SubtitleDocument;

pub use crate::report::{OverlapConfig, OverlapSeverity, OverlapWarning, ValidationReport};

/// ASS subtitle syntax validator and overlap detector.
///
/// `Validator` is the main entry point for running validation checks on a parsed
/// ASS/SSA subtitle file. It applies a configurable set of rules covering
/// encoding, structure, style, event, and semantic validation stages.
///
/// ## Configuration
///
/// Use [`Validator::new`] for default settings, or [`Validator::with_overlap_config`]
/// to customise overlap detection behaviour:
///
/// ```rust,no_run
/// use subtitle_validator::{Validator, OverlapConfig};
///
/// // Default: lenient overlap detection
/// let v = Validator::new();
///
/// // Strict: report all overlaps
/// let v = Validator::new().with_overlap_config(OverlapConfig::strict());
///
/// // Lenient: only critical overlaps
/// let v = Validator::new().with_overlap_config(OverlapConfig::lenient());
/// ```
pub struct Validator {
    overlap_config: OverlapConfig,
}

impl Validator {
    /// Creates a new `Validator` with default overlap detection settings.
    ///
    /// The default configuration uses [`OverlapConfig::default`], which enables
    /// visual overlap checking, ignores karaoke overlaps, and reports overlaps
    /// of 100ms or longer.
    pub fn new() -> Self {
        Self {
            overlap_config: OverlapConfig::default(),
        }
    }

    /// Configures the overlap detection behaviour for this validator.
    ///
    /// # Arguments
    ///
    /// * `config` - An [`OverlapConfig`] specifying strict/lenient mode, minimum
    ///   duration thresholds, and positional proximity settings.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use subtitle_validator::{Validator, OverlapConfig};
    ///
    /// let validator = Validator::new()
    ///     .with_overlap_config(OverlapConfig::strict());
    /// ```
    pub fn with_overlap_config(mut self, config: OverlapConfig) -> Self {
        self.overlap_config = config;
        self
    }

    /// Validates an ASS file and returns a [`ValidationReport`].
    ///
    /// Runs all configured validation rules against the parsed subtitle file,
    /// including structure checks, style validation, event validation, and
    /// overlap detection.
    ///
    /// # Arguments
    ///
    /// * `ass` - A reference to a parsed [`SubtitleDocument`] (from the `ass-parser` crate).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ass_core::SubtitleDocument;
    /// use subtitle_validator::Validator;
    ///
    /// let content = std::fs::read_to_string("subtitles.ass").unwrap();
    /// let ass = SubtitleDocument::parse(&content).unwrap();
    /// let report = Validator::new().validate(&ass);
    /// println!("{}", report.summary());
    /// ```
    pub fn validate(&self, ass: &SubtitleDocument) -> ValidationReport {
        rules::validate(ass, &self.overlap_config)
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates an ASS file using default settings (lenient overlap mode).
///
/// This is a convenience function equivalent to:
/// ```rust,no_run
/// # use ass_core::SubtitleDocument;
/// # use subtitle_validator::Validator;
/// # let ass: &SubtitleDocument = todo!();
/// Validator::new().validate(&ass);
/// ```
pub fn validate(ass: &SubtitleDocument) -> ValidationReport {
    Validator::new().validate(ass)
}

/// Validates an ASS file using strict overlap detection.
///
/// In strict mode, all overlaps are reported regardless of duration or position,
/// and karaoke overlaps are not suppressed.
///
/// Equivalent to:
/// ```rust,no_run
/// # use ass_core::SubtitleDocument;
/// # use subtitle_validator::{Validator, OverlapConfig};
/// # let ass: &SubtitleDocument = todo!();
/// Validator::new()
///     .with_overlap_config(OverlapConfig::strict())
///     .validate(&ass);
/// ```
pub fn validate_strict(ass: &SubtitleDocument) -> ValidationReport {
    Validator::new()
        .with_overlap_config(OverlapConfig::strict())
        .validate(ass)
}
