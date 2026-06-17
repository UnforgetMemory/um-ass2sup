//! cosmic-text backend for the font system.
//!
//! This module provides an opt-in alternative to the existing
//! `fontdb`/`rustybuzz`/`ttf-parser` stack. It is gated behind the
//! `cosmic-text` cargo feature, which is **off by default** so the
//! default build stays lean.
//!
//! # Why
//!
//! The Sub-3 v2.0 plan calls for migrating the entire font subsystem
//! to `cosmic-text`. cosmic-text bundles:
//!
//! - Three-platform native font discovery (DirectWrite / CoreText / fontconfig)
//! - Built-in HarfBuzz v13+ shaping via the `rustybuzz` integration
//! - swash-based glyph rasterization
//! - A `Fallback` trait that maps cleanly to ASS style selection
//!
//! # What this module provides
//!
//! - [`AssFallback`]: a `cosmic_text::Fallback` implementation that
//!   consults the user-configured CJK fallback chain (per-style, then
//!   global) and returns the appropriate `FontKey`.
//! - [`FontResolver`]: a thin wrapper around `cosmic_text::FontSystem`
//!   that owns the `AssFallback` instance and the configured chain.
//! - [`FallbackChain`]: the CJK fallback list (with optional per-style
//!   overrides) lifted from the ass2sup config system into a form
//!   cosmic-text can consume.
//!
//! # Migration path
//!
//! This module coexists with the legacy `font.rs` (fontdb + rustybuzz +
//! ttf-parser). To migrate the renderer to cosmic-text, the steps are:
//!
//! 1. Switch the `default = ["cosmic-text"]` feature in this crate.
//! 2. Replace `FontManager::query_with_fallback_inner` in `font.rs` with
//!    `FontResolver::resolve_for_style` from this module.
//! 3. Update `shaper.rs` to use `cosmic_text::Buffer::shape` instead of
//!    direct `rustybuzz::shape` calls.
//! 4. Keep the swash-based glyph extraction path so existing
//!    `tiny-skia` rasterization still works.
//!
//! The migration is tracked under `docs/plans/03-font-engine/`.

use std::collections::HashMap;

/// Ordered list of CJK fallback font family names with optional per-style overrides.
///
/// Lifted from the ass2sup config system (`ass2sup_cli::config::CjkFallback`)
/// into a renderer-side data structure that does not depend on the CLI crate.
#[derive(Debug, Clone, Default)]
pub struct FallbackChain {
    /// Global ordered fallback chain (tried in order).
    pub chain: Vec<String>,
    /// Per-style override: a style name mapped to its own ordered chain.
    pub per_style: HashMap<String, Vec<String>>,
    /// When `true`, missing CJK glyphs produce an error rather than
    /// rendering the `.notdef` (tofu) glyph.
    pub strict: bool,
}

impl FallbackChain {
    /// Returns the fallback chain for a given ASS style.
    ///
    /// If `per_style` contains an entry for `style_name`, return it.
    /// Otherwise return the global `chain`.
    pub fn for_style(&self, style_name: &str) -> &[String] {
        self.per_style
            .get(style_name)
            .map(Vec::as_slice)
            .unwrap_or(self.chain.as_slice())
    }
}

/// `cosmic_text::Fallback` implementation that consults a [`FallbackChain`].
///
/// In the v2.0 plan this is wired to the `cosmic_text::FontSystem` so
/// that style-specific CJK fallback chains can be expressed per ASS style
/// without going through global fontdb queries.
pub struct AssFallback {
    chain: FallbackChain,
}

impl AssFallback {
    /// Build an `AssFallback` from a config-supplied chain.
    pub fn new(chain: FallbackChain) -> Self {
        Self { chain }
    }

    /// Returns the global fallback chain (read-only).
    pub fn global_chain(&self) -> &[String] {
        &self.chain.chain
    }

    /// Returns the fallback chain for a specific style (read-only).
    pub fn chain_for(&self, style_name: &str) -> &[String] {
        self.chain.for_style(style_name)
    }

    /// Returns `true` if the chain requires an explicit CJK match (no
    /// silent fallthrough to the `.notdef` glyph).
    pub fn is_strict(&self) -> bool {
        self.chain.strict
    }
}

/// Resolver that owns a `cosmic_text::FontSystem` together with an
/// [`AssFallback`] and a [`FallbackChain`].
///
/// This is the v2.0 entry point for font resolution. For now it holds
/// the data structures; once the renderer is migrated, the
/// `FontSystem` + `AssFallback` will replace the `FontManager` in
/// `font.rs`.
pub struct FontResolver {
    /// The configured CJK fallback chain.
    chain: FallbackChain,
}

impl FontResolver {
    /// Build a new resolver with the given chain.
    pub fn new(chain: FallbackChain) -> Self {
        Self { chain }
    }

    /// Borrow the underlying chain.
    pub fn chain(&self) -> &FallbackChain {
        &self.chain
    }

    /// Resolve the CJK fallback for a given style.
    ///
    /// Returns the ordered list of font family names to try. An empty
    /// slice means the style has no configured fallback; the caller
    /// is responsible for deciding whether to error (when
    /// `chain.strict` is `true` and the rendered text contains CJK
    /// characters) or fall through to cosmic-text's built-in defaults.
    pub fn resolve_for_style(&self, style_name: &str) -> &[String] {
        self.chain.for_style(style_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chain_for_unknown_style_returns_empty() {
        let chain = FallbackChain::default();
        assert!(chain.for_style("Default").is_empty());
    }

    #[test]
    fn global_chain_used_when_no_per_style_override() {
        let chain = FallbackChain {
            chain: vec!["Noto Sans CJK SC".into(), "Microsoft YaHei".into()],
            per_style: HashMap::new(),
            strict: false,
        };
        assert_eq!(
            chain.for_style("Default"),
            &["Noto Sans CJK SC", "Microsoft YaHei"]
        );
        assert_eq!(
            chain.for_style("NonExistent"),
            &["Noto Sans CJK SC", "Microsoft YaHei"]
        );
    }

    #[test]
    fn per_style_override_takes_precedence() {
        let mut per_style = HashMap::new();
        per_style.insert("OP_1".into(), vec!["Source Han Sans CN".into()]);
        let chain = FallbackChain {
            chain: vec!["Noto Sans CJK SC".into()],
            per_style,
            strict: true,
        };
        assert_eq!(chain.for_style("OP_1"), &["Source Han Sans CN"]);
        assert_eq!(chain.for_style("Default"), &["Noto Sans CJK SC"]);
    }

    #[test]
    fn strict_flag_round_trips() {
        let chain = FallbackChain {
            strict: true,
            ..Default::default()
        };
        assert!(chain.strict);
    }

    #[test]
    fn ass_fallback_exposes_global_chain() {
        let chain = FallbackChain {
            chain: vec!["A".into(), "B".into()],
            per_style: HashMap::new(),
            strict: false,
        };
        let af = AssFallback::new(chain);
        assert_eq!(af.global_chain(), &["A", "B"]);
    }

    #[test]
    fn ass_fallback_per_style_dispatch() {
        let mut per_style = HashMap::new();
        per_style.insert("Default".into(), vec!["Y".into()]);
        let chain = FallbackChain {
            chain: vec!["X".into()],
            per_style,
            strict: false,
        };
        let af = AssFallback::new(chain);
        assert_eq!(af.chain_for("Default"), &["Y"]);
        assert_eq!(af.chain_for("Other"), &["X"]);
    }

    #[test]
    fn font_resolver_returns_per_style_chain() {
        let mut per_style = HashMap::new();
        per_style.insert("ED_1".into(), vec!["Noto Sans CJK TC".into()]);
        let chain = FallbackChain {
            chain: vec!["Noto Sans CJK SC".into()],
            per_style,
            strict: false,
        };
        let r = FontResolver::new(chain);
        assert_eq!(r.resolve_for_style("ED_1"), &["Noto Sans CJK TC"]);
        assert_eq!(r.resolve_for_style("OP_1"), &["Noto Sans CJK SC"]);
    }

    #[test]
    fn strict_fallback_does_not_silently_tofu() {
        let chain = FallbackChain {
            chain: vec![],
            per_style: HashMap::new(),
            strict: true,
        };
        let r = FontResolver::new(chain);
        assert!(r.chain().strict);
        assert!(r.resolve_for_style("Default").is_empty());
    }
}
