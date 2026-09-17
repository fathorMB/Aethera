//! Plain-text reports for the monthly accounts. Amounts are integer cents.

pub struct Sale {
    pub product: String,
    pub quantity: u32,
    pub unit_cents: i64,
}

pub struct Expense {
    pub category: String,
    pub cents: i64,
}

/// One report line: the label padded to 20 columns, the amount right-aligned in 10, a newline.
pub fn format_row(label: &str, cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    let amount = format!("{}{}.{:02}", sign, abs / 100, abs % 100);
    format!("{:<20}{:>10}\n", label, amount)
}

/// One line per product (quantity times unit price), then a TOTAL line.
pub fn render_sales(sales: &[Sale]) -> String {
    let mut out = String::from("SALES\n");
    let mut total: i64 = 0;
    for s in sales {
        let cents = s.unit_cents * i64::from(s.quantity);
        total += cents;
        out.push_str(&format_row(&format!("{} x{}", s.product, s.quantity), cents));
    }
    out.push_str(&format_row("TOTAL", total));
    out
}

/// One line per category, in the order given, then a TOTAL line. Refunds are negative.
pub fn render_expenses(expenses: &[Expense]) -> String {
    let mut out = String::from("EXPENSES\n");
    let mut total: i64 = 0;
    for e in expenses {
        total += e.cents;
        out.push_str(&format_row(&e.category, e.cents));
    }
    out.push_str(&format_row("TOTAL", total));
    out
}
