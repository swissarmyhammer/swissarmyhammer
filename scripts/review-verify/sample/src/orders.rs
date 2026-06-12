//! Order totals — PLANTED FINDINGS: `order_total` and `cart_total` are the
//! same body with renamed identifiers, and the tax rate / shipping numbers
//! are repeated bare magic literals. Do not fix; the review harness depends
//! on them.

/// Sum the item prices and add sales tax.
pub fn order_total(prices: &[f64]) -> f64 {
    let mut total = 0.0;
    for price in prices {
        total += price;
    }
    total + total * 0.0825
}

/// Sum the cart amounts and add sales tax.
pub fn cart_total(amounts: &[f64]) -> f64 {
    let mut sum = 0.0;
    for amount in amounts {
        sum += amount;
    }
    sum + sum * 0.0825
}

/// Compute the shipping cost for a package weight in pounds.
pub fn shipping_cost(weight: f64) -> f64 {
    if weight > 50.0 {
        weight * 1.75 + 12.5
    } else {
        weight * 1.75 + 4.99
    }
}
