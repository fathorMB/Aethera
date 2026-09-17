//! The inverse of `parse_duration`.

/// Formats seconds with the largest units first, skipping zero parts: 3725 -> "1h2m5s".
/// Zero is "0s".
pub fn format_duration(seconds: u64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }
    let mut rest = seconds;
    let mut out = String::new();
    for (unit, size) in [('d', 86_400), ('h', 3_600), ('m', 60), ('s', 1)] {
        let n = rest / size;
        rest %= size;
        if n > 0 {
            out.push_str(&n.to_string());
            out.push(unit);
        }
    }
    out
}
