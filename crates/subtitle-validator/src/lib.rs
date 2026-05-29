pub mod report;
pub mod rules;

use ass_parser::AssFile;

use crate::report::{OverlapConfig, ValidationReport};

pub struct Validator {
    overlap_config: OverlapConfig,
}

impl Validator {
    pub fn new() -> Self {
        Self {
            overlap_config: OverlapConfig::default(),
        }
    }

    pub fn with_overlap_config(mut self, config: OverlapConfig) -> Self {
        self.overlap_config = config;
        self
    }

    pub fn validate(&self, ass: &AssFile) -> ValidationReport {
        rules::validate(ass, &self.overlap_config)
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

pub fn validate(ass: &AssFile) -> ValidationReport {
    Validator::new().validate(ass)
}

pub fn validate_strict(ass: &AssFile) -> ValidationReport {
    Validator::new()
        .with_overlap_config(OverlapConfig::strict())
        .validate(ass)
}
