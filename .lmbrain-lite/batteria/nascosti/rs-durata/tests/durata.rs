use durata::{format_duration, parse_duration, DurationError};

#[test]
fn hours_and_minutes() {
    assert_eq!(parse_duration("1h30m"), Ok(5_400));
}

#[test]
fn seconds_only() {
    assert_eq!(parse_duration("45s"), Ok(45));
}

#[test]
fn digits_without_unit_are_rejected() {
    assert_eq!(parse_duration("90"), Err(DurationError::MissingUnit));
}

#[test]
fn unknown_unit() {
    assert_eq!(parse_duration("3w"), Err(DurationError::BadUnit('w')));
}

#[test]
fn empty_input() {
    assert_eq!(parse_duration("   "), Err(DurationError::Empty));
}

#[test]
fn format_round_trip() {
    assert_eq!(format_duration(3_725), "1h2m5s");
    assert_eq!(parse_duration(&format_duration(3_725)), Ok(3_725));
}

#[test]
fn hidden_all_units() {
    assert_eq!(parse_duration("2d3h4m5s"), Ok(2 * 86_400 + 3 * 3_600 + 4 * 60 + 5));
    assert_eq!(parse_duration("10m"), Ok(600));
    assert_eq!(parse_duration(" 1m1m "), Ok(120));
}

#[test]
fn hidden_errors() {
    assert_eq!(parse_duration("h"), Err(DurationError::BadNumber(String::new())));
    assert_eq!(parse_duration("1h30"), Err(DurationError::MissingUnit));
    assert_eq!(parse_duration("5x"), Err(DurationError::BadUnit('x')));
}

#[test]
fn hidden_round_trips() {
    for s in [0u64, 1, 59, 60, 61, 3_600, 86_399, 86_400, 1_000_000] {
        assert_eq!(parse_duration(&format_duration(s)), Ok(s), "{s}");
    }
}
