//! Display resolution parsing and auto-detection.

use tracing::info;

/// Output display resolution.
#[derive(Debug, Clone, Copy)]
pub struct Resolution {
    /// Display width in pixels.
    pub width: u32,
    /// Display height in pixels.
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
        }
    }
}

impl Resolution {
    /// Parse a `WIDTHxHEIGHT` string.
    ///
    /// Both width and height must be non-zero unsigned 32-bit integers.
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid resolution format '{s}'. Expected WIDTHxHEIGHT"
            ));
        }
        let width = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("Invalid width '{}'", parts[0]))?;
        let height = parts[1]
            .parse::<u32>()
            .map_err(|_| format!("Invalid height '{}'", parts[1]))?;
        if width == 0 || height == 0 {
            return Err("Resolution dimensions must be > 0".to_string());
        }
        Ok(Self { width, height })
    }

    /// Return the user-specified resolution, falling back to the ASS script
    /// resolution (1920×1080 if that too is missing or invalid).
    pub fn from_args_or_script(
        cli_res: &Resolution,
        script_width: u32,
        script_height: u32,
    ) -> Self {
        // If the user explicitly passed -r we use it (it was already stored
        // into the default-constructed Resolution); otherwise try the script.
        if cli_res.width != 1920 || cli_res.height != 1080 {
            return *cli_res;
        }
        if script_width > 0 && script_height > 0 && script_width <= 7680 && script_height <= 4320 {
            return Self {
                width: script_width,
                height: script_height,
            };
        }
        info!(
            "Script Info resolution invalid or missing ({}x{}), falling back to 1920×1080",
            script_width, script_height
        );
        Self::default()
    }
}
