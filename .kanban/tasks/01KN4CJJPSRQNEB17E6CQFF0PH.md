---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffaf80
title: 'EXTRACT-1: Create swissarmyhammer-perspectives crate skeleton'
---
## What

Create the new `swissarmyhammer-perspectives` crate with Cargo.toml, error types, and lib.rs.

**`swissarmyhammer-perspectives/Cargo.toml`:**
- Follow `swissarmyhammer-views/Cargo.toml` as template
- Workspace deps: serde, serde_yaml_ng, serde_json, tokio, thiserror, tracing, ulid, chrono
- Dev deps: tempfile, tokio (with test-util)

**`swissarmyhammer-perspectives/src/error.rs`:**
- `PerspectiveError` enum mirroring `ViewsError` pattern:
  - `DuplicateName { item_type: String, name: String }`
  - `NotFound { resource: String, id: String }`
  - `Io(#[from] std::io::Error)`
  - `Yaml(#[from] serde_yaml_ng::Error)`
  - `Json(#[from] serde_json::Error)`
- `pub type Result<T> = std::result::Result<T, PerspectiveError>;`
- Convenience constructors: `not_found()`, `duplicate_name()`

**`swissarmyhammer-perspectives/src/lib.rs`:**
- Module declarations (types, context, changelog, error)
- Re-exports (empty for now, filled as modules are added)

**Workspace `Cargo.toml`:**
- Add `"swissarmyhammer-perspectives"` to members list

## Acceptance Criteria
- [x] `cargo check -p swissarmyhammer-perspectives` compiles
- [x] `PerspectiveError` has all 5 variants with Display/Error derives
- [x] Crate appears in workspace members
- [x] Error convenience constructors work

## Tests
- [x] `cargo check -p swissarmyhammer-perspectives` passes
- [x] Basic error construction test in error.rs