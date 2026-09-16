use report::{format_row, render_expenses, render_sales, Expense, Sale};

#[test]
fn formats_one_row() {
    assert_eq!(format_row("Rent", 120_000), "Rent                   1200.00\n");
    assert_eq!(format_row("Refund", -5), "Refund                   -0.05\n");
}

#[test]
fn hidden_rows() {
    assert_eq!(format_row("", 0), format!("{}{}\n", " ".repeat(26), "0.00"));
    assert_eq!(format_row("A label longer than twenty", 1), "A label longer than twenty      0.01\n");
    assert_eq!(format_row("x", -100), format!("x{}-1.00\n", " ".repeat(24)));
}

#[test]
fn hidden_reports_agree_with_rows() {
    let sales = vec![Sale { product: "Pen".into(), quantity: 4, unit_cents: -25 }];
    assert_eq!(render_sales(&sales), format!("SALES\n{}{}", format_row("Pen x4", -100), format_row("TOTAL", -100)));
    let expenses = vec![Expense { category: "Food".into(), cents: 999 }, Expense { category: "Gas".into(), cents: 1 }];
    assert_eq!(
        render_expenses(&expenses),
        format!("EXPENSES\n{}{}{}", format_row("Food", 999), format_row("Gas", 1), format_row("TOTAL", 1000))
    );
}
