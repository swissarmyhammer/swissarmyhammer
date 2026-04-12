---
assignees:
- claude-code
position_column: todo
position_ordinal: b880
project: kanban-mcp
title: 'sah-cli: extract doctor.rs from commands/doctor/ to top-level module'
---
## What

Extract doctor functionality from `swissarmyhammer-cli/src/commands/doctor/mod.rs` into a top-level `swissarmyhammer-cli/src/doctor.rs`, matching the pattern in shelltool-cli and code-context-cli. Use `DoctorRunner` trait.

## Acceptance Criteria
- [ ] `swissarmyhammer-cli/src/doctor.rs` exists at top level
- [ ] `commands/doctor/` delegates to or is replaced by top-level module
- [ ] `sah doctor` still works
- [ ] `cargo test -p swissarmyhammer-cli` passes
