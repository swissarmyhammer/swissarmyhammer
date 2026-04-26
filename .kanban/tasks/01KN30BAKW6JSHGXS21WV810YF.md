---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffa280
title: Dispatch cannot clear filter/group on perspective update via MCP
---
dispatch.rs:465-469\n\nThe dispatch wiring for `(Verb::Update, Noun::Perspective)` uses `op.get_string(\"filter\")` which returns `None` for both absent and null JSON values. The `UpdatePerspective` struct supports `Some(None)` to clear a filter/group, but the dispatch path can only ever produce `Some(Some(value))` or `None` (preserve). This means an MCP caller cannot clear a filter or group -- only the internal `ClearFilterCmd`/`ClearGroupCmd` commands can do it.\n\nSuggestion: Check for explicit null in the JSON params. If the param key is present with a null value, call `with_filter(None)` / `with_group(None)`. For example:\n```rust\nif op.params.contains_key(\"filter\") {\n    let filter = op.get_string(\"filter\").map(|s| s.to_string());\n    cmd = cmd.with_filter(filter);\n}\n```\n\nVerification: Add an integration test that sets a filter, then updates with `\"filter\": null`, and asserts the filter is cleared." #review-finding