use report::format_row;

#[test]
fn formats_one_row() {
    assert_eq!(format_row("Rent", 120_000), "Rent                   1200.00\n");
    assert_eq!(format_row("Refund", -5), "Refund                   -0.05\n");
}
