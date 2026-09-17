//! Short human durations: `1h30m`, `45s`, `2d4h`.

mod format;

pub use format::format_duration;

/// Why a duration could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationError {
    /// The input is empty or only whitespace.
    Empty,
    /// A unit was found with no digits before it (the string holds the digits seen, possibly empty).
    BadNumber(String),
    /// A character that is neither a digit nor a known unit.
    BadUnit(char),
    /// The input ends with digits that have no unit after them.
    MissingUnit,
}

/// Seconds in one unit.
fn unit_seconds(unit: char) -> Option<u64> {
    match unit {
        'd' => Some(86_400),
        'h' => Some(3_600),
        'm' => Some(3_600),
        's' => Some(1),
        _ => None,
    }
}

/// Parses a duration into seconds. Surrounding whitespace is ignored.
pub fn parse_duration(input: &str) -> Result<u64, DurationError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(DurationError::Empty);
    }
    let mut total: u64 = 0;
    let mut digits = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let per_unit = unit_seconds(c).ok_or(DurationError::BadUnit(c))?;
        let n: u64 = digits.parse().map_err(|_| DurationError::BadNumber(digits.clone()))?;
        total += n * per_unit;
        digits.clear();
    }
    Ok(total)
}
