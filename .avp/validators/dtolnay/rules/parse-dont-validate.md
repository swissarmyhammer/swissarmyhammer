---
name: parse-dont-validate
description: Use parsing and type conversions instead of validating then proceeding with raw data
---

# Parse, Don't Validate

This rule is about the **conversion pattern**: when data crosses a boundary,
convert it into a stronger type rather than checking it and passing the original
through. If you validate with `if` statements but keep using the raw type,
the validation can drift from the usage. Parse into a type that makes the
invalid state unrepresentable.

Note: this rule is about **how you convert** data. For **what type you choose**
to represent domain concepts (enum vs String), see `no-stringly-typed`.

## What to Check

Look for patterns where code validates data but continues using the original
unvalidated type:

1. **Validate-then-use**: A function checks a condition on input, then continues
   working with the same untyped value. The check should instead be a `TryFrom`,
   `FromStr`, or constructor that returns a stronger type.

2. **Boolean guard patterns**: Functions that start with `if !is_valid(x) { return Err(...) }`
   and then proceed to use `x` as-is. The validity check should produce a new type.

3. **String parsing with manual checks**: Code that takes `&str`, runs regex or
   manual character checks, then keeps using the `&str`. Should parse into a
   newtype (e.g., `Username`, `Email`, `Slug`).

4. **Numeric range checks followed by raw use**: Code that validates `0 <= n < 256`
   then uses `n` as `usize`. Should parse into `u8` or a domain newtype.

## What Passes

- `impl TryFrom<String> for Email` that validates format during conversion
- `impl FromStr for PortNumber` that rejects out-of-range values
- Constructor functions like `fn new(raw: &str) -> Result<Self, ParseError>`
- Using `serde` deserialize to validate on parse
- Functions that accept already-parsed types (`fn send(to: Email, body: Body)`)

## What Fails

- A function that takes `&str`, checks `str.contains('@')`, then passes the `&str` along
- Code that validates a config struct's fields with `if` checks but keeps using the raw struct
- Functions with signatures like `fn process(input: String) -> Result<()>` that internally
  validate `input` but never convert it to a stronger type
- Accepting `usize` and checking it fits in `u16` range, then continuing with `usize`

## Why This Matters

Once data is parsed into a correct type, every subsequent function that accepts
that type gets the validation for free. The compiler enforces it. This is how
serde works: deserialize validates, and downstream code trusts the types.
