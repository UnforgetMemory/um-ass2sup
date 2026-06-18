//! Smart error diagnostics (Sub-8 / Sprint 7).
//!
//! Wraps a raw `Error` with actionable suggestions and a docs link.
//! The CLI prints these instead of the bare `Display` output so users
//! know what to try next.

use crate::error::Error;

/// A single suggestion attached to a smart error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Human-readable hint.
    pub text: String,
    /// Optional concrete command or example.
    pub example: Option<String>,
}

impl Suggestion {
    /// Build a text-only suggestion.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            example: None,
        }
    }

    /// Build a suggestion with a concrete example.
    pub fn with_example(text: impl Into<String>, example: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            example: Some(example.into()),
        }
    }
}

/// A diagnostic-friendly error wrapper. Borrows the underlying error
/// to avoid requiring `Error: Clone`.
#[derive(Debug)]
pub struct SmartError<'a> {
    /// The underlying error.
    pub error: &'a Error,
    /// One or more suggested fixes.
    pub suggestions: Vec<Suggestion>,
    /// Optional link to relevant documentation.
    pub docs_link: Option<String>,
}

impl<'a> SmartError<'a> {
    /// Build a `SmartError` with no extra context.
    pub fn new(error: &'a Error) -> Self {
        Self {
            error,
            suggestions: Vec::new(),
            docs_link: None,
        }
    }

    /// Add a suggestion.
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Add a docs link.
    pub fn with_docs(mut self, link: impl Into<String>) -> Self {
        self.docs_link = Some(link.into());
        self
    }

    /// Format for human display: error + bulleted suggestions + docs.
    pub fn format_human(&self) -> String {
        let mut out = String::new();
        out.push_str("error: ");
        out.push_str(&self.error.to_string());
        out.push('\n');
        if !self.suggestions.is_empty() {
            out.push_str("\nsuggestions:\n");
            for s in &self.suggestions {
                if let Some(ex) = &s.example {
                    out.push_str(&format!("  - {} (e.g. {})\n", s.text, ex));
                } else {
                    out.push_str(&format!("  - {}\n", s.text));
                }
            }
        }
        if let Some(link) = &self.docs_link {
            out.push_str(&format!("\nsee: {}\n", link));
        }
        out
    }
}

/// Diagnose an error and attach contextual suggestions.
pub fn diagnose(error: &Error) -> SmartError<'_> {
    let mut smart = SmartError::new(error);
    match error {
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            smart = smart.with_suggestion(Suggestion::new(
                "check the file path is correct and the file exists",
            ));
            smart = smart.with_docs("https://github.com/example/ass2sup/blob/main/docs/INPUT.md");
        }
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::PermissionDenied => {
            smart = smart.with_suggestion(Suggestion::new(
                "check the file is readable; you may need chmod or sudo",
            ));
        }
        Error::Parse { file, line, .. } => {
            let loc = match line {
                Some(l) => format!("{}:{}", file.display(), l),
                None => file.display().to_string(),
            };
            smart = smart
                .with_suggestion(Suggestion::with_example(
                    "check the syntax on the offending line",
                    loc,
                ))
                .with_suggestion(Suggestion::new(
                    "if the file uses CRLF, run it through `dos2unix` first",
                ));
            smart = smart
                .with_docs("https://github.com/example/ass2sup/blob/main/docs/PARSE_ERRORS.md");
        }
        Error::Render(crate::error::RenderError::Effect { effect, .. }) => {
            smart = smart.with_suggestion(Suggestion::new(format!(
                "effect {} failed; try --no-effects or a smaller test ASS",
                effect
            )));
        }
        Error::Render(crate::error::RenderError::Event { event_idx, .. }) => {
            smart = smart.with_suggestion(Suggestion::new(format!(
                "event {} failed to render; isolate it with --max-frames 1",
                event_idx
            )));
        }
        _ => {}
    }
    smart
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ConfigError, RenderError};
    use std::path::PathBuf;

    #[test]
    fn suggestion_new_sets_text() {
        let s = Suggestion::new("try this");
        assert_eq!(s.text, "try this");
        assert!(s.example.is_none());
    }

    #[test]
    fn suggestion_with_example() {
        let s = Suggestion::with_example("run", "ass2sup foo.ass");
        assert_eq!(s.text, "run");
        assert_eq!(s.example.as_deref(), Some("ass2sup foo.ass"));
    }

    #[test]
    fn smart_error_new_has_empty_suggestions() {
        let e = Error::io("x", std::io::Error::other("x"));
        let smart = SmartError::new(&e);
        assert!(smart.suggestions.is_empty());
        assert!(smart.docs_link.is_none());
    }

    #[test]
    fn smart_error_builder() {
        let e = Error::io("x", std::io::Error::other("x"));
        let smart = SmartError::new(&e)
            .with_suggestion(Suggestion::new("hint"))
            .with_docs("https://example.com");
        assert_eq!(smart.suggestions.len(), 1);
        assert_eq!(smart.docs_link.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn format_human_includes_suggestions_and_docs() {
        let e = Error::io("x", std::io::Error::other("boom"));
        let smart = SmartError::new(&e).with_suggestion(Suggestion::new("retry"));
        let s = smart.format_human();
        assert!(s.contains("error:"));
        assert!(s.contains("suggestions:"));
        assert!(s.contains("- retry"));
    }

    #[test]
    fn diagnose_io_not_found() {
        let e = Error::io(
            "missing.ass",
            std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        );
        let smart = diagnose(&e);
        assert!(!smart.suggestions.is_empty());
        assert!(smart.docs_link.is_some());
    }

    #[test]
    fn diagnose_io_permission() {
        let e = Error::io(
            "locked.ass",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "x"),
        );
        let smart = diagnose(&e);
        assert!(!smart.suggestions.is_empty());
    }

    #[test]
    fn diagnose_parse_attaches_line() {
        let e = Error::Parse {
            file: PathBuf::from("test.ass"),
            line: Some(42),
            message: "bad syntax".to_string(),
        };
        let smart = diagnose(&e);
        assert_eq!(smart.suggestions.len(), 2);
    }

    #[test]
    fn diagnose_parse_without_line() {
        let e = Error::Parse {
            file: PathBuf::from("test.ass"),
            line: None,
            message: "x".to_string(),
        };
        let smart = diagnose(&e);
        assert_eq!(smart.suggestions.len(), 2);
    }

    #[test]
    fn diagnose_render_effect_includes_name() {
        let e = Error::Render(RenderError::Effect {
            effect: "fade".into(),
            message: "x".into(),
        });
        let smart = diagnose(&e);
        assert_eq!(smart.suggestions.len(), 1);
        assert!(smart.suggestions[0].text.contains("fade"));
    }

    #[test]
    fn diagnose_render_event_includes_index() {
        let e = Error::Render(RenderError::Event {
            event_idx: 7,
            pts_ms: 1000,
            message: "x".into(),
        });
        let smart = diagnose(&e);
        assert_eq!(smart.suggestions.len(), 1);
        assert!(smart.suggestions[0].text.contains('7'));
    }

    #[test]
    fn diagnose_config_validation_no_suggestions() {
        let e = Error::Config(ConfigError::Validation("x".into()));
        let smart = diagnose(&e);
        assert!(smart.suggestions.is_empty());
    }
}
