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
