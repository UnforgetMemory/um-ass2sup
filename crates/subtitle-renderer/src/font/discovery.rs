use std::path::PathBuf;

/// Discover system font directories for the current platform.
pub fn discover_system_font_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    {
        for dir in &["/usr/share/fonts", "/usr/local/share/fonts"] {
            let p = PathBuf::from(dir);
            if p.exists() {
                paths.push(p);
            }
        }
        // User fonts
        if let Some(home) = std::env::var_os("HOME") {
            for sub in &[".local/share/fonts", ".fonts"] {
                let p = PathBuf::from(&home).join(sub);
                if p.exists() {
                    paths.push(p);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        for dir in &["/System/Library/Fonts", "/Library/Fonts"] {
            paths.push(PathBuf::from(dir));
        }
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(&home).join("Library/Fonts"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(windir) = std::env::var_os("WINDIR") {
            paths.push(PathBuf::from(&windir).join("Fonts"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            paths.push(PathBuf::from(&local).join("Microsoft/Windows/Fonts"));
        }
    }

    paths
}

/// Discover user-specified font directories.
pub fn discover_user_font_paths(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter().filter(|d| d.exists()).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_font_paths_non_empty_on_linux() {
        if cfg!(target_os = "linux") {
            let paths = discover_system_font_paths();
            assert!(!paths.is_empty(), "Linux must have at least one font path");
            assert!(paths.iter().any(|p| p.to_string_lossy().contains("fonts")));
        }
    }

    #[test]
    fn user_font_paths_filters_nonexistent() {
        let dirs = vec![
            PathBuf::from("/nonexistent/path/xyz"),
            PathBuf::from("/usr/share/fonts"), // exists on Linux
        ];
        let result = discover_user_font_paths(&dirs);
        // Filter: only existing paths returned
        if cfg!(target_os = "linux") {
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].to_string_lossy(), "/usr/share/fonts");
        }
    }
}
