use ass2sup_cli::telemetry;

#[test]
fn test_init_idempotent() {
    // Should not panic on repeated calls
    telemetry::init(false, false, false, "auto");
    telemetry::init(true, false, true, "never");
}

#[test]
fn test_init_accepts_all_color_modes() {
    for color in ["auto", "always", "never"] {
        telemetry::init(false, false, false, color);
    }
}

#[test]
fn test_init_accepts_all_flag_combinations() {
    for debug in [false, true] {
        for verbose in [false, true] {
            for quiet in [false, true] {
                telemetry::init(verbose, quiet, debug, "auto");
            }
        }
    }
}
