---
position_column: done
position_ordinal: k3
title: Change FieldDef.default from Option&lt;String&gt; to Option&lt;Value&gt;
---
**Review finding: Nit (fields crate)**

`swissarmyhammer-fields/src/types.rs` — `FieldDef.default`

The default value is always `Option<String>` regardless of field type. A number field default of `"42"` requires consumer-side parsing. A select field default has no load-time validation against options.

- [ ] Change `default: Option<String>` to `default: Option<serde_json::Value>`
- [ ] Update YAML deserialization (serde_yaml handles Value natively)
- [ ] Update EntityContext::apply_validation where defaults are inserted
- [ ] Update all tests constructing FieldDef with defaults
- [ ] Run full test suite