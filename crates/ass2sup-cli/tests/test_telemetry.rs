use ass2sup_cli::telemetry;

#[test]
fn test_init_idempotent() {
    // Should not panic on repeated calls
    telemetry::init(false, false, false, "auto", None);
    telemetry::init(true, false, true, "never", None);
}

#[test]
fn test_init_accepts_all_color_modes() {
    for color in ["auto", "always", "never"] {
        telemetry::init(false, false, false, color, None);
    }
}

#[test]
fn test_init_accepts_all_flag_combinations() {
    for debug in [false, true] {
        for verbose in [false, true] {
            for quiet in [false, true] {
                telemetry::init(verbose, quiet, debug, "auto", None);
            }
        }
    }
}

#[test]
fn test_init_with_log_file_writes_file() {
    // tracing_subscriber is process-global; parallel tests race on the first
    // try_init. Isolate by running the CLI binary itself (separate process).
    let dir = std::env::temp_dir().join(format!("ass2sup-telemetry-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log_path = dir.join("cli.log");
    let log_str = log_path.to_string_lossy().to_string();

    let exe = env!("CARGO_BIN_EXE_ass2sup");
    // Use --check (validation only) so run() executes and telemetry inits,
    // while still exiting quickly. Needs a tiny ASS file.
    let ass = dir.join("t.ass");
    std::fs::write(
        &ass,
        "[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,Hello\n",
    )
    .unwrap();
    let out = std::process::Command::new(exe)
        .args(["--check", ass.to_str().unwrap(), "--log-file", &log_str])
        .output()
        .expect("run ass2sup --check");
    assert!(out.status.success(), "--check should exit 0");

    // File layer is synchronous; the process has exited so the file must exist.
    let content = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        !content.is_empty(),
        "log file should capture diagnostics, got: {content:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_init_invalid_log_file_no_panic() {
    // A path in a nonexistent directory should not panic.
    telemetry::init(
        false,
        false,
        false,
        "auto",
        Some("/nonexistent-dir-xyz/log.log"),
    );
}
