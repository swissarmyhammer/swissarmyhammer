//! Invoice totals — PLANTED FINDINGS: `invoice_total` copy-pastes the
//! summation-plus-tax body from `orders.rs` with renamed identifiers, and the
//! late-fee thresholds are repeated bare magic literals. Do not fix; the
//! review harness depends on them.

/// Sum the invoice line items and add sales tax.
pub fn invoice_total(line_items: &[f64]) -> f64 {
    let mut grand_total = 0.0;
    for item in line_items {
        grand_total += item;
    }
    grand_total + grand_total * 0.0825
}

/// Compute the late fee for an overdue invoice.
pub fn late_fee(days_overdue: u32) -> f64 {
    if days_overdue > 30 {
        25.0 + f64::from(days_overdue) * 1.5
    } else {
        f64::from(days_overdue) * 1.5
    }
}
