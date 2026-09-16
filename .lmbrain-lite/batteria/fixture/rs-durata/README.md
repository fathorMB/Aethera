# durata

Parse and format short human durations such as `1h30m`, `45s` or `2d4h`.

Units: `d` (days), `h` (hours), `m` (minutes), `s` (seconds). Every number must be
followed by a unit; units may appear in any order and may repeat (`1m1m` is 120 s).

Run the tests with `cargo test`.
