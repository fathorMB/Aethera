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

/// One line per product (quantity times unit price), then a TOTAL line.
pub fn render_sales(sales: &[Sale]) -> String {
    let mut out = String::from("SALES\n");
    let mut total: i64 = 0;
    for s in sales {
        let cents = s.unit_cents * i64::from(s.quantity);
        total += cents;
        let sign = if cents < 0 { "-" } else { "" };
        let abs = cents.abs();
        let amount = format!("{}{}.{:02}", sign, abs / 100, abs % 100);
        let label = format!("{} x{}", s.product, s.quantity);
        out.push_str(&format!("{:<20}{:>10}\n", label, amount));
    }
    let sign = if total < 0 { "-" } else { "" };
    let abs = total.abs();
    let amount = format!("{}{}.{:02}", sign, abs / 100, abs % 100);
    out.push_str(&format!("{:<20}{:>10}\n", "TOTAL", amount));
    out
}

/// One line per category, in the order given, then a TOTAL line. Refunds are negative.
pub fn render_expenses(expenses: &[Expense]) -> String {
    let mut out = String::from("EXPENSES\n");
    let mut total: i64 = 0;
    for e in expenses {
        total += e.cents;
        let sign = if e.cents < 0 { "-" } else { "" };
        let abs = e.cents.abs();
        let amount = format!("{}{}.{:02}", sign, abs / 100, abs % 100);
        out.push_str(&format!("{:<20}{:>10}\n", e.category, amount));
    }
    let sign = if total < 0 { "-" } else { "" };
    let abs = total.abs();
    let amount = format!("{}{}.{:02}", sign, abs / 100, abs % 100);
    out.push_str(&format!("{:<20}{:>10}\n", "TOTAL", amount));
    out
}
