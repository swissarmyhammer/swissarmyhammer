---
title: Scaffold swissarmyhammer-fields crate
position:
  column: done
  ordinal: c5
---
Create the new `swissarmyhammer-fields` crate and add it to the workspace.

**Files to create:**
- `swissarmyhammer-fields/Cargo.toml` — workspace package fields, deps: `serde`, `serde_yaml`, `tokio`, `thiserror`, `tracing`, `ulid`
- `swissarmyhammer-fields/src/lib.rs` — crate root, module declarations
- `swissarmyhammer-fields/src/error.rs` — FieldsError enum with thiserror

**Files to modify:**
- Root `Cargo.toml` — add `swissarmyhammer-fields` to workspace members, add `swissarmyhammer-fields = { path = "swissarmyhammer-fields" }` to workspace dependencies

**Pattern:** Follow existing crate patterns (workspace = true for package fields, workspace deps, no feature flags).

**Subtasks:**
- [ ] Create Cargo.toml with workspace deps
- [ ] Create lib.rs with module declarations (error, types, context)
- [ ] Create error.rs with FieldsError
- [ ] Add to workspace Cargo.toml members and dependencies
- [ ] Verify `cargo check -p swissarmyhammer-fields` passes