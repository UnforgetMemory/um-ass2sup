//! Font data resolution — the fallback chain that maps a requested
//! (family, bold, style) to concrete font bytes.

use std::collections::HashMap;
use std::sync::Arc;

use crate::context::RenderContext;
use crate::font::registry::FontRegistry;

/// Resolve the best available font bytes for `family`, following the
/// consolidated fallback chain:
///
/// exact match → suggestion → `parse_font_name` decomposition → bold upgrade →
/// font_map → first available family (last resort).
///
/// Returns an empty `Arc` when no font matches at all.
pub(crate) fn resolve_font_data_inner(
    registry: &FontRegistry,
    family: &str,
    bold: bool,
    font_map: Option<&HashMap<String, Vec<String>>>,
    style_name: &str,
    use_bold_upgrade: bool,
) -> Arc<[u8]> {
    use crate::font::types::{FontQuery, FontStyle, FontWeight};

    let weight = if bold {
        FontWeight::Bold
    } else {
        FontWeight::Normal
    };

    // Step 1: exact match
    let q = FontQuery {
        family: family.to_string(),
        weight,
        style: FontStyle::Normal,
    };
    let result = registry.query(&q);
    if let Some(id) = result.found
        && let Some(data) = registry.get_font_data_arc(id)
    {
        return data;
    }
    if let Some(sug) = result.suggestion
        && let Some(data) = registry.get_font_data_arc(sug.id)
    {
        return data;
    }

    // Step 2: parse_font_name decomposition (e.g. "MiSans Demibold" → ("MiSans", Semibold))
    if let Some((parsed_family, parsed_weight)) = parse_font_name(family) {
        let pq = FontQuery {
            family: parsed_family.to_string(),
            weight: parsed_weight,
            style: FontStyle::Normal,
        };
        let pr = registry.query(&pq);

        // Bold-upgrade: when bold requested & parsed weight < Bold, also try Bold
        if use_bold_upgrade && bold && parsed_weight < FontWeight::Bold {
            let bq = FontQuery {
                family: parsed_family.to_string(),
                weight: FontWeight::Bold,
                style: FontStyle::Normal,
            };
            let br = registry.query(&bq);
            if let Some(id) = br.found
                && let Some(data) = registry.get_font_data_arc(id)
            {
                return data;
            }
        }

        if let Some(id) = pr.found
            && let Some(data) = registry.get_font_data_arc(id)
        {
            return data;
        }
        if let Some(sug) = pr.suggestion
            && let Some(data) = registry.get_font_data_arc(sug.id)
        {
            return data;
        }
    }

    // Step 3: font_map fallback
    if let Some(fallbacks) = font_map.and_then(|m| m.get(style_name).or_else(|| m.get("Default"))) {
        for fb_name in fallbacks {
            if fb_name == family {
                continue;
            }
            let fb_query = FontQuery {
                family: fb_name.to_string(),
                weight,
                style: FontStyle::Normal,
            };
            let fb_result = registry.query(&fb_query);
            if let Some(id) = fb_result.found
                && let Some(data) = registry.get_font_data_arc(id)
            {
                return data;
            }
            if let Some(sug) = fb_result.suggestion
                && let Some(data) = registry.get_font_data_arc(sug.id)
            {
                return data;
            }
        }
    }

    // Step 4: last resort — first available font
    let families = registry.list_families();
    for fallback_family in &families {
        let q = FontQuery {
            family: fallback_family.clone(),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
        };
        if let Some(id) = registry.query(&q).found
            && let Some(data) = registry.get_font_data_arc(id)
        {
            return data;
        }
    }

    Arc::default()
}

/// Resolve font bytes for a single glyph (per-glyph wrapper used by the
/// font-data cache miss path).
pub(crate) fn resolve_glyph_font_data(
    registry: &FontRegistry,
    ctx: &RenderContext,
    _glyph_id: u16,
    font_map: &HashMap<String, Vec<String>>,
    style_name: &str,
) -> Arc<[u8]> {
    resolve_font_data_inner(
        registry,
        &ctx.font_name,
        ctx.bold,
        Some(font_map),
        style_name,
        false, // bold_upgrade already handled by parse_font_name fallback
    )
}

/// Parse a font family name to extract weight/style information.
/// For example, "MiSans Demibold" -> ("MiSans", Demibold)
pub fn parse_font_name(family: &str) -> Option<(String, crate::font::types::FontWeight)> {
    use crate::font::types::FontWeight;

    let parts: Vec<&str> = family.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // Try to find weight keyword in the last part(s)
    let weight_keywords = [
        ("Thin", FontWeight::Thin),
        ("ExtraLight", FontWeight::ExtraLight),
        ("Light", FontWeight::Light),
        ("Regular", FontWeight::Normal),
        ("Normal", FontWeight::Normal),
        ("Medium", FontWeight::Medium),
        ("Demibold", FontWeight::Semibold),
        ("SemiBold", FontWeight::Semibold),
        ("Bold", FontWeight::Bold),
        ("ExtraBold", FontWeight::ExtraBold),
        ("Black", FontWeight::Black),
        ("Heavy", FontWeight::Black),
    ];

    // Check if last part is a weight keyword
    let last = parts.last()?;
    for (keyword, weight) in &weight_keywords {
        if last.eq_ignore_ascii_case(keyword) {
            let family_part = parts[..parts.len() - 1].join(" ");
            return Some((family_part, *weight));
        }
    }

    // Check if last two parts form a weight keyword (e.g., "Extra Bold")
    if parts.len() >= 3 {
        let last_two = format!("{} {}", parts[parts.len() - 2], parts[parts.len() - 1]);
        for (keyword, weight) in &weight_keywords {
            if last_two.eq_ignore_ascii_case(keyword) {
                let family_part = parts[..parts.len() - 2].join(" ");
                return Some((family_part, *weight));
            }
        }
    }

    None
}
