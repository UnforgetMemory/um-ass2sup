use ass_parser::timestamp::Timestamp;

#[test]
fn test_from_ms() {
    let ts = Timestamp::from_ms(1000);
    assert_eq!(ts.as_ms(), 1000);
}

#[test]
fn test_from_hms() {
    // 1 hour, 30 minutes, 15 seconds, 500 milliseconds
    let ts = Timestamp::from_hms(1, 30, 15, 500);
    assert_eq!(ts.as_ms(), 3600000 + 30 * 60000 + 15 * 1000 + 500);
}

#[test]
fn test_zero() {
    let ts = Timestamp::ZERO;
    assert_eq!(ts.as_ms(), 0);
}

#[test]
fn test_from_ass_time() {
    // ASS format: H:MM:SS.CS where CS = centiseconds
    let ts = Timestamp::from_ass_time("1:23:45.67").unwrap();
    assert_eq!(ts.as_ms(), 3600000 + 23 * 60000 + 45 * 1000 + 670);
}

#[test]
fn test_from_ass_time_zero() {
    let ts = Timestamp::from_ass_time("0:00:00.00").unwrap();
    assert_eq!(ts.as_ms(), 0);
}

#[test]
fn test_from_ass_time_invalid() {
    assert!(Timestamp::from_ass_time("invalid").is_err());
    assert!(Timestamp::from_ass_time("1:2:3").is_err());
    assert!(Timestamp::from_ass_time("a:00:00.00").is_err());
}

#[test]
fn test_as_ass_time() {
    let ts = Timestamp::from_ms(5025670); // 1:23:45.67
    assert_eq!(ts.as_ass_time(), "1:23:45.67");
}

#[test]
fn test_as_ass_time_roundtrip() {
    let inputs = vec![
        "0:00:00.00",
        "0:00:01.00",
        "0:01:00.00",
        "1:00:00.00",
        "0:00:00.10",
        "23:59:59.99",
    ];
    for input in inputs {
        let ts = Timestamp::from_ass_time(input).unwrap();
        assert_eq!(ts.as_ass_time(), input, "Roundtrip failed for: {}", input);
    }
}

#[test]
fn test_duration_ms() {
    let start = Timestamp::from_ms(1000);
    let end = Timestamp::from_ms(3500);
    assert_eq!(start.duration_ms(end), 2500);
}

#[test]
fn test_duration_saturating() {
    // Saturating subtraction - end before start should give 0
    let start = Timestamp::from_ms(5000);
    let end = Timestamp::from_ms(2000);
    assert_eq!(start.duration_ms(end), 0);
}

#[test]
fn test_display() {
    let ts = Timestamp::from_ms(1000);
    assert_eq!(format!("{}", ts), "0:00:01.00");
}

#[test]
fn test_large_values() {
    let ts = Timestamp::from_hms(100, 0, 0, 0);
    assert_eq!(ts.as_ms(), 360_000_000);
}
