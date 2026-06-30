//! Native rendering backend (swash + tiny-skia).
//!
//! Delegates to [`ConversionPipeline::render_and_quantize`] for the actual
//! render + quantise loop.  This module only handles renderer initialisation
//! and font loading.

use ass_core::SubtitleDocument;
use color_quantizer::QuantizedFrame;
use tracing::{debug, info, warn};

use crate::cli::args::Args;
use crate::config::Config;
use crate::error::CliError;
use crate::pipeline::convert::ConversionPipeline;

/// Render and quantize using the native Rust rendering stack.
pub fn render_and_quantize(
    doc: &SubtitleDocument,
    config: &Config,
    args: &Args,
) -> Result<Vec<QuantizedFrame>, CliError> {
    let mut renderer = create_native_renderer(doc, config)?;

    renderer.set_font_map(config.font.font_map.clone());

    let font_check_result = super::super::super::config::font::check_ass_fonts_with_fn(
        doc,
        |family| renderer.font_available(family),
        &config.font.font_map,
        &config.font.default_font,
        config.font.no_check,
    );
    if let Err(e) = font_check_result {
        if !args.force {
            return Err(CliError::Conversion(e));
        }
        warn!("{e}");
    }

    Ok(ConversionPipeline::render_and_quantize(
        doc,
        &mut renderer,
        config,
        args,
    ))
}

/// Create and initialise a native [`Renderer`] from document metadata + config.
fn create_native_renderer(
    doc: &SubtitleDocument,
    config: &Config,
) -> Result<subtitle_renderer::Renderer, CliError> {
    let render_cfg = subtitle_renderer::RenderConfig {
        width: config.resolution.width,
        height: config.resolution.height,
        script_width: doc.metadata.play_res_x,
        script_height: doc.metadata.play_res_y,
        default_font: config.font.default_font.clone(),
        default_font_size: config.font.default_font_size,
        vsfilter_compat: config.font.vsfilter_compat,
    };

    let renderer = subtitle_renderer::Renderer::new(render_cfg);

    let system_count = renderer.load_system_fonts();
    debug!(system_count, "loaded system fonts");

    for dir in &config.font.font_dirs {
        let added = renderer.load_user_fonts_dir(dir);
        if added > 0 {
            info!("Loaded {added} font face(s) from {}", dir.display());
        } else {
            warn!("No font files found in --font-dir: {}", dir.display());
        }
    }

    Ok(renderer)
}
