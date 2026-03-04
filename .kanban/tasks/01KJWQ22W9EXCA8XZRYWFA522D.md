---
position_column: done
position_ordinal: i8
title: Fix JS code injection in ValidationEngine via string interpolation
---
**Review finding: Blocker (fields crate)**

`swissarmyhammer-fields/src/validation.rs` — `run_js_validation` and `validate_entity`

The validation engine interpolates `ctx_json` (serialized entity field values) directly into a JS string via `format!()`. While `serde_json::to_string` escapes quotes, the pattern is fragile — it depends entirely on serde_json never producing output that could be reinterpreted as JS syntax-breakers.

```rust
let js_code = format!(
    r#"(function() {{
    var ctx = {ctx_json};
    {validate_body}
}})()"#,
    ...
);
```

## Fix approach
Use the QuickJS runtime's variable binding API to pass the ctx object as a bound variable rather than string-interpolating JSON into JS source code. Check if JsState supports `set()` or argument passing.

- [ ] Replace string interpolation with bound variable injection for ctx
- [ ] Add test with adversarial entity values (strings containing `}})()`, etc.)
- [ ] Verify existing validation tests still pass
- [ ] Run full test suite