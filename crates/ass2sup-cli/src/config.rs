//! TOML-backed configuration system for the ass2sup CLI.
//!
//! The schema is intentionally flat and additive: every struct is
//! `#[serde(deny_unknown_fields)]` so adding a new key never silently
//! shadows a typo, and an unknown key produces a load error pointing
//! the user at the offending field.
//!
//! Loading semantics: a missing file is treated as [`Config::default()`]
//! (callers do not need to pre-create the config). A present but
//! malformed file produces [`ConfigError::Parse`].
//!
//! Saving: `Config::save` writes a human-readable TOML document
//! (`toml::to_string_pretty`) so the result can be hand-edited.
//!
//! CLI integration: [`Config::merge_with_args`] is the single place
//! where CLI flags override file values; downstream code reads from a
//! `Config` rather than reaching back into the `Args` struct.
//!
//! [`Config::default()`]: crate::config::Config::default()
//! [`ConfigError::Parse`]: crate::error::ConfigError::Parse
//! [`Config::merge_with_args`]: crate::config::Config::merge_with_args

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, Result};

/// Maximum config file size in bytes (1 MiB). Configs are human-edited
/// TOML and should never approach this size; refusing early prevents
/// `std::fs::read_to_string` from being weaponised into an OOM via a
/// hostile `--config /path/to/huge.toml` argument.
pub const MAX_CONFIG_SIZE_BYTES: u64 = 1024 * 1024;

/// The top-level config struct; mirrors the TOML root table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Subtitle-frame defaults (fps, palette, dithering, parallelism).
    #[serde(default)]
    pub defaults: Defaults,
    /// CJK font fallback configuration.
    #[serde(default)]
    pub cjk_fallback: CjkFallback,
    /// Output color-space / tonemapping configuration.
    #[serde(default)]
    pub color: ColorConfig,
    /// Per-style font overrides (e.g. force a particular font for `OP_1`).
    #[serde(default)]
    pub style_overrides: Vec<StyleOverride>,
    /// Renderer backend selection.
    #[serde(default)]
    pub rendering: RenderingConfig,
    /// Log level override (`trace` / `debug` / `info` / `warn` / `error`).
    #[serde(default)]
    pub log_level: Option<String>,
}

/// Default values applied to every subtitle run when no flag overrides them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    /// Frames per second (`23.976`, `25.0`, `29.97`, ...).
    pub fps: Option<f64>,
    /// Maximum number of palette entries (1..=255).
    pub max_colors: Option<u8>,
    /// Dithering method: `none` | `ordered` | `floyd-steinberg`.
    pub dither: Option<String>,
    /// Enable per-frame parallel quantisation.
    pub parallel_frames: Option<bool>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            fps: Some(23.976),
            max_colors: Some(255),
            dither: Some("floyd-steinberg".to_string()),
            parallel_frames: Some(true),
        }
    }
}

/// CJK fallback font configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CjkFallback {
    /// Ordered list of fallback font families tried in turn.
    #[serde(default)]
    pub chain: Vec<String>,
    /// Per-style override of the fallback chain.
    #[serde(default)]
    pub per_style: HashMap<String, Vec<String>>,
    /// When `true`, missing CJK glyphs produce an error instead of
    /// silently rendering the replacement `.notdef` glyph (tofu).
    #[serde(default)]
    pub strict: bool,
}

/// Output color-space and tonemapping configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorConfig {
    /// Target color space: `sdr-bt709` | `hdr-bt2020-pq` | `hdr-bt2020-hlg`.
    pub output_space: Option<String>,
    /// Tonemapping operator: `hable` | `aces` | `reinhard`.
    pub tonemap: Option<String>,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            output_space: Some("sdr-bt709".to_string()),
            tonemap: Some("hable".to_string()),
        }
    }
}

/// Force a specific font for a particular ASS style.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleOverride {
    /// ASS style name (e.g. `OP_1`, `Default`).
    pub style: String,
    /// Font family to force for events using that style.
    pub font: String,
}

/// Renderer backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderingConfig {
    /// Backend selection: `cpu` | `gpu` | `auto`.
    pub backend: Option<String>,
    /// Pixel accuracy profile: `fast` | `high` | `exact`.
    pub pixel_accuracy: Option<String>,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            backend: Some("auto".to_string()),
            pixel_accuracy: Some("high".to_string()),
        }
    }
}

impl Config {
    /// Load config from `path`. A missing file yields [`Config::default`]
    /// (no error); a present file that fails to parse yields
    /// [`ConfigError::Parse`] or [`ConfigError::Read`] as appropriate.
    /// Files larger than [`MAX_CONFIG_SIZE_BYTES`] are refused with
    /// [`ConfigError::Read`] to bound the memory cost of `read_to_string`.
    pub fn load(path: &Path) -> std::result::Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX);
        if size > MAX_CONFIG_SIZE_BYTES {
            return Err(ConfigError::Validation(format!(
                "config file {} is {size} bytes, exceeds limit of {MAX_CONFIG_SIZE_BYTES} bytes",
                path.display()
            )));
        }
        let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Persist the config to `path` as pretty-printed TOML.
    pub fn save(&self, path: &Path) -> std::result::Result<(), ConfigError> {
        let content =
            toml::to_string_pretty(self).map_err(|e| ConfigError::Validation(e.to_string()))?;
        std::fs::write(path, content).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    /// Merge CLI-derived values into the config. CLI values win.
    ///
    /// Only fields actually present in the CLI are merged; `None` /
    /// `false` no-ops do not clobber config-file values. (Exception:
    /// `parallel_frames` flips to `true` when the flag is present,
    /// because clap derives `bool` flags as `false` by default.)
    pub fn merge_with_args(&mut self, args: &MergeArgs<'_>) {
        if let Some(fps) = args.fps {
            self.defaults.fps = Some(fps);
        }
        if let Some(max_colors) = args.max_colors {
            self.defaults.max_colors = Some(max_colors);
        }
        if let Some(dither) = args.dither {
            self.defaults.dither = Some(dither.to_string());
        }
        if args.parallel_frames {
            self.defaults.parallel_frames = Some(true);
        }
        if let Some(log_level) = args.log_level {
            self.log_level = Some(log_level.to_string());
        }
    }

    /// Resolve the config file path by precedence:
    /// 1. `cli_path` if supplied (`--config <PATH>`)
    /// 2. `./ass2sup.toml` in the current directory
    /// 3. `~/.config/ass2sup/config.toml`
    /// 4. `None` if none of the above exists (caller should use defaults)
    pub fn locate(cli_path: Option<&Path>) -> Option<PathBuf> {
        if let Some(p) = cli_path {
            return Some(p.to_path_buf());
        }
        let cwd_candidate = PathBuf::from("ass2sup.toml");
        if cwd_candidate.exists() {
            return Some(cwd_candidate);
        }
        if let Some(home) = std::env::var_os("HOME") {
            let user = PathBuf::from(home).join(".config/ass2sup/config.toml");
            if user.exists() {
                return Some(user);
            }
        }
        None
    }

    /// Load using the standard precedence rules. Returns [`Config::default`]
    /// if no config file is found anywhere on the search path.
    pub fn load_default(cli_path: Option<&Path>) -> std::result::Result<Self, ConfigError> {
        match Self::locate(cli_path) {
            Some(p) => Self::load(&p),
            None => Ok(Self::default()),
        }
    }
}

/// Subset of CLI args consumed by [`Config::merge_with_args`].
///
/// The CLI's `Args` struct is large; passing a thin bag of values
/// keeps `config.rs` decoupled from the clap derive struct.
#[derive(Debug, Clone, Default)]
pub struct MergeArgs<'a> {
    /// `--fps` value, if supplied.
    pub fps: Option<f64>,
    /// `--max-colors` value, if supplied.
    pub max_colors: Option<u8>,
    /// `--dither` value, if supplied.
    pub dither: Option<&'a str>,
    /// `--parallel-frames` flag.
    pub parallel_frames: bool,
    /// `--log-level` value, if supplied.
    pub log_level: Option<&'a str>,
}

/// Convenience: convert a [`ConfigError`] into the crate-wide `Error` enum.
#[doc(hidden)]
pub fn to_crate_error(e: ConfigError) -> crate::error::Error {
    crate::error::Error::Config(e)
}

/// Try to apply a config to a mutable target; never panics.
#[doc(hidden)]
pub fn try_merge_with_args(config: &mut Config, args: MergeArgs<'_>) -> Result<()> {
    config.merge_with_args(&args);
    Ok(())
}
