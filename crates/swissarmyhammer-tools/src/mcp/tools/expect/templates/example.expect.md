---
description: A valid coupon reduces the order total by its discount, exactly once
surface: cli            # cli | http | browser | gui | file | db
reliability: pass^3     # all of 3 runs must pass (default: pass^1)
model: qwen-coder-flash # named sah model for grading; omit to use the sah model default
tags: [checkout, pricing]
---

# A valid coupon reduces the total, exactly once

When a shopper applies a valid coupon to an order, the displayed total drops by
the coupon's discount amount, and applying the same coupon a second time does not
stack. The discount must come off the subtotal, not be a coincidence of some
other arithmetic.

## Given
- A freshly created cart with one $50 item (arranged per run, so `pass^3` stays independent)
- A coupon `SAVE10` worth $10 off, currently valid

## When
- The shopper applies `SAVE10`
- The shopper applies `SAVE10` again

## Then
- [ ] After the first apply, the total is $40
- [ ] The UI confirms the coupon was applied
- [ ] After the second apply, the total is still $40 (no stacking)
- [ ] An error or notice explains the coupon is already applied

## Notes
The discount must come off subtotal before tax. Don't accept a $40 total that
was reached by the wrong arithmetic (e.g. a 20% rounding coincidence) — the
reason must be the coupon.
