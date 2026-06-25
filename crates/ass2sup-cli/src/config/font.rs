//! Font configuration — per-style fallback maps and availability checks.

use std::collections::HashMap;

use ass_core::SubtitleDocument;
use subtitle_renderer::CosmicRenderResources;
use tracing::{debug, trace, warn};

/// Per-style font fallback map: style name → ordered list of fallback names.
pub type FontMap = HashMap<String, Vec<String>>;

/// Parse `"StyleName:fallback1,fallback2"` entries into a [`FontMap`].
pub fn parse_font_map(entries: &[String]) -> Result<FontMap, String> {
    let mut map = FontMap::new();
    for entry in entries {
        let Some((style, fallbacks)) = entry.split_once(':') else {
            return Err(format!(
                "Invalid font-map entry '{entry}': expected 'StyleName:fallback1,fallback2'"
            ));
        };
        let style = style.trim();
        let fb_list: Vec<String> = fallbacks
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if style.is_empty() {
            return Err(format!("Empty style name in font-map entry '{entry}'"));
        }
        map.insert(style.to_string(), fb_list);
    }
    Ok(map)
}

/// Check that every font family used in the ASS document is available.
///
/// Returns an `Err` listing all missing fonts when `no_check` is `false`.
pub fn check_ass_fonts(
    doc: &SubtitleDocument,
    resolver: &CosmicRenderResources,
    font_map: &FontMap,
    global_fallback: &str,
    no_check: bool,
) -> Result<(), String> {
    if no_check {
        trace!("check_ass_fonts skipped (--no-check-fonts)");
        return Ok(());
    }
    debug!(
        styles = doc.styles.len(),
        global_fallback = %global_fallback,
        "checking font availability for all ASS styles"
    );

    let mut missing: Vec<String> = Vec::new();

    for style in &doc.styles {
        let primary = if style.font_name.is_empty() {
            global_fallback
        } else {
            &style.font_name
        };

        if resolver.resolve_font(primary, false, false).is_some() {
            trace!(style = ?style.name, font = %primary, "style font OK");
            continue;
        }

        // Try per-style fallback chain from --font-map
        let style_name = style.name.as_str();
        if let Some(fallbacks) = font_map.get(style_name) {
            let all_missing = fallbacks
                .iter()
                .all(|fb| resolver.resolve_font(fb, false, false).is_none());
            if !all_missing {
                trace!(
                    style = ?style.name,
                    fallbacks = ?fallbacks,
                    "at least one --font-map entry is available"
                );
                continue;
            }
        }

        // Try global fallback (--font)
        if global_fallback != primary
            && !global_fallback.is_empty()
            && global_fallback != "Arial"
            && resolver
                .resolve_font(global_fallback, false, false)
                .is_some()
        {
            debug!(
                style = ?style.name,
                primary = %primary,
                fallback = %global_fallback,
                "global --font fallback is available; using it"
            );
            continue;
        }

        let fb_chain: Vec<&str> = font_map
            .get(style_name)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        let desc = if fb_chain.is_empty() {
            format!("'{primary}' (no fallback configured)")
        } else {
            let fb_str = fb_chain.join(", ");
            format!("'{primary}' (fallbacks: {fb_str}) not installed")
        };
        warn!(style = ?style.name, "{desc}");
        missing.push(desc);
    }

    if missing.is_empty() {
        Ok(())
    } else {
        let mut msg = String::from("Font check failed — missing font(s):\n");
        for m in &missing {
            msg.push_str(&format!("  • {m}\n"));
        }
        msg.push_str(
            "Install the fonts above or re-run with --no-check-fonts to skip this check.\n",
        );
        msg.push_str(
            "Hint: for CJK subtitles install fonts-noto-cjk (Debian/Ubuntu) or embed \
             fonts via the ASS [Fonts] section.",
        );
        Err(msg)
    }
}
