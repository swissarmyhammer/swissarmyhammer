---
position_column: done
position_ordinal: k2
title: Make load_yaml_dir async or document sync requirement
---
**Review finding: Nit (fields crate)**

`swissarmyhammer-fields/src/context.rs` — `load_yaml_dir()`

Uses `std::fs::read_dir` and `std::fs::read_to_string` (blocking I/O) while everything else uses `tokio::fs`. Called from the kanban context builder in an async context. Could block the Tokio runtime on large directories.

- [ ] Either convert to async using tokio::fs, or add doc comment explaining it's intentionally sync
- [ ] If converting to async, update callers
- [ ] Run full test suite