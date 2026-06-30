//! Rendering backend selection and dispatch.
//!
//! Each backend implements `render_and_quantize()` and returns a list of
//! [`QuantizedFrame`]s.  The rest of the pipeline (encode -> write) is shared.

use ass_core::SubtitleDocument;
use color_quantizer::QuantizedFrame;

use crate::cli::args::Args;
use crate::config::Config;
use crate::error::CliError;

#[cfg(feature = "libass-backend")]
mod libass;
#[cfg(feature = "native-backend")]
mod native;

/// Render and quantize subtitle frames using the selected backend.
#[allow(unused_variables)]
pub fn render_and_quantize(
    content: &str,
    doc: &SubtitleDocument,
    config: &Config,
    args: &Args,
) -> Result<Vec<QuantizedFrame>, CliError> {
    #[cfg(all(feature = "native-backend", feature = "libass-backend"))]
    {
        let use_libass = args.backend.as_deref() == Some("libass");
        if use_libass {
            libass::render_and_quantize(content, doc, config, args)
        } else {
            native::render_and_quantize(doc, config, args)
        }
    }

    #[cfg(all(feature = "native-backend", not(feature = "libass-backend")))]
    {
        native::render_and_quantize(doc, config, args)
    }

    #[cfg(all(feature = "libass-backend", not(feature = "native-backend")))]
    {
        libass::render_and_quantize(content, doc, config, args)
    }
}

/// Return the human-readable name of the active backend.
pub fn backend_name() -> &'static str {
    #[cfg(all(feature = "native-backend", feature = "libass-backend"))]
    {
        "native (libass available via --backend)"
    }
    #[cfg(all(feature = "native-backend", not(feature = "libass-backend")))]
    {
        "native"
    }
    #[cfg(all(feature = "libass-backend", not(feature = "native-backend")))]
    {
        "libass"
    }
}
