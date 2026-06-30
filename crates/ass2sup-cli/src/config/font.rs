use std::collections::HashMap;

use ass_core::SubtitleDocument;
#[cfg(feature = "native-backend")]
use subtitle_renderer::font::registry::FontRegistry;
#[cfg(feature = "native-backend")]
use subtitle_renderer::font::types::{FontQuery, FontStyle, FontWeight};
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
#[cfg(feature = "native-backend")]
pub fn check_ass_fonts(
    doc: &SubtitleDocument,
    registry: &FontRegistry,
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

        if font_available(registry, primary) {
            trace!(style = ?style.name, font = %primary, "style font OK");
            continue;
        }

        let style_name = style.name.as_str();
        if let Some(fallbacks) = font_map.get(style_name) {
            let all_missing = fallbacks.iter().all(|fb| !font_available(registry, fb));
            if !all_missing {
                trace!(
                    style = ?style.name,
                    fallbacks = ?fallbacks,
                    "at least one --font-map entry is available"
                );
                continue;
            }
        }

        if global_fallback != primary
            && !global_fallback.is_empty()
            && global_fallback != "Arial"
            && font_available(registry, global_fallback)
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

#[cfg(feature = "native-backend")]
fn font_available(registry: &FontRegistry, family: &str) -> bool {
    // Try exact match first
    let q = FontQuery {
        family: family.to_string(),
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    };
    let result = registry.query(&q);
    tracing::debug!(
        family = %family,
        found = result.found.is_some(),
        candidates = result.candidates.len(),
        suggestion = result.suggestion.is_some(),
        "font_available check"
    );
    if result.found.is_some() {
        return true;
    }

    // Try parse_font_name decomposition (e.g., "MiSans Demibold"
    // → family="MiSans", weight=Semibold) matching resolve_font_data.
    if let Some((parsed_family, parsed_weight)) = subtitle_renderer::parse_font_name(family) {
        let pq = FontQuery {
            family: parsed_family.to_string(),
            weight: parsed_weight,
            style: FontStyle::Normal,
        };
        if registry.query(&pq).found.is_some() {
            return true;
        }
    }

    // Try with different weights
    for weight in [FontWeight::Bold, FontWeight::Medium, FontWeight::Semibold] {
        let q = FontQuery {
            family: family.to_string(),
            weight,
            style: FontStyle::Normal,
        };
        if registry.query(&q).found.is_some() {
            return true;
        }
    }

    // Try family-only match (any weight)
    !result.candidates.is_empty() || result.suggestion.is_some()
}

/// Check font availability using a closure instead of a direct registry reference.
///
/// This variant allows checking fonts through the Renderer's internal font registry
/// without exposing it publicly.
pub fn check_ass_fonts_with_fn(
    doc: &SubtitleDocument,
    is_available: impl Fn(&str) -> bool,
    font_map: &FontMap,
    global_fallback: &str,
    no_check: bool,
) -> Result<(), String> {
    // When --no-check-fonts is set, only skip fonts that have a font_map
    // fallback (the user has configured an explicit fallback chain).
    // Fonts with NO font_map entry are still checked — if they're missing
    // on the system, they WILL fail, forcing the user to either install
    // the font or configure --font-fallback-map.
    let full_check = !no_check;

    let mut missing: Vec<String> = Vec::new();

    for style in &doc.styles {
        let primary = if style.font_name.is_empty() {
            global_fallback
        } else {
            &style.font_name
        };

        if is_available(primary) {
            trace!(style = ?style.name, font = %primary, "style font OK");
            continue;
        }

        let style_name = style.name.as_str();
        let has_fallback = font_map.get(style_name);

        // In partial-check mode (--no-check-fonts), skip fonts that have
        // an explicit --font-fallback-map entry (trust the render-time fallback).
        if !full_check && has_fallback.is_some() {
            trace!(
                style = ?style.name,
                font = %primary,
                "skipped (has --font-map fallback, --no-check-fonts)"
            );
            continue;
        }

        let all_missing =
            has_fallback.is_none_or(|fallbacks| fallbacks.iter().all(|fb| !is_available(fb)));
        if !all_missing {
            trace!(
                style = ?style.name,
                fallbacks = ?has_fallback,
                "at least one --font-map entry is available"
            );
            continue;
        }

        if global_fallback != primary
            && !global_fallback.is_empty()
            && global_fallback != "Arial"
            && is_available(global_fallback)
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
