---
title: Remove unused swissarmyhammer-directory dependency from fields crate
position:
  column: done
  ordinal: a1
---
**File:** `swissarmyhammer-fields/Cargo.toml:21`

**What:** Card 2 added `swissarmyhammer-directory` as a dependency of `swissarmyhammer-fields`, but the code never imports or uses anything from it. The VFS approach was abandoned in favor of the simpler `from_yaml_sources()` + `load_yaml_dir()` pattern that uses only `std::fs`.

**Why:** Unnecessary dependencies increase compile times and create false coupling. If `swissarmyhammer-directory` changes, `swissarmyhammer-fields` will needlessly recompile.

- [x] Remove `swissarmyhammer-directory = { path = "../swissarmyhammer-directory" }` from `swissarmyhammer-fields/Cargo.toml`
- [x] Run `cargo check --workspace` to verify nothing breaks #Warning