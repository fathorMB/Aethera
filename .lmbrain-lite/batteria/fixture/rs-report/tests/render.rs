use report::{render_expenses, render_sales, Expense, Sale};

#[test]
fn sales_report() {
    let sales = vec![
        Sale { product: "Coffee".into(), quantity: 3, unit_cents: 250 },
        Sale { product: "Tea".into(), quantity: 1, unit_cents: 5 },
    ];
    let expected = "SALES\n\
Coffee x3                 7.50\n\
Tea x1                    0.05\n\
TOTAL                     7.55\n";
    assert_eq!(render_sales(&sales), expected);
}

#[test]
fn expenses_with_refund() {
    let expenses = vec![
        Expense { category: "Rent".into(), cents: 120_000 },
        Expense { category: "Refund".into(), cents: -123_456 },
    ];
    let expected = "EXPENSES\n\
Rent                   1200.00\n\
Refund                -1234.56\n\
TOTAL                   -34.56\n";
    assert_eq!(render_expenses(&expenses), expected);
}

#[test]
fn empty_reports() {
    assert_eq!(render_sales(&[]), "SALES\nTOTAL                     0.00\n");
    assert_eq!(render_expenses(&[]), "EXPENSES\nTOTAL                     0.00\n");
}
