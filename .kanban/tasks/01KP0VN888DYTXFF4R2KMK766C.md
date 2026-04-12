---
assignees:
- claude-code
position_column: todo
position_ordinal: bc80
project: kanban-mcp
title: Extract logging.rs as standard pattern across all CLIs
---
## What

sah-cli has `logging.rs` with `FileWriterGuard` (flush+sync on every write) — the other CLIs reinvent this inline in main.rs or don't have it at all. Make `logging.rs` the standard pattern.

Options:
1. Extract `FileWriterGuard` + tracing init into a shared crate (e.g. `swissarmyhammer-common::logging`)
2. Or duplicate `logging.rs` into each CLI (same code, local module)

Either way, every CLI should have consistent file-based tracing with stderr fallback:
- **shelltool-cli**: currently inlines FileWriterGuard in main.rs → extract to logging.rs
- **code-context-cli**: needs logging.rs added
- **kanban-cli**: logging card currently puts it in main.rs → should be logging.rs instead

## Acceptance Criteria
- [ ] All four CLIs (sah, shelltool, code-context, kanban) have consistent logging setup
- [ ] FileWriterGuard pattern is in a `logging.rs` module (or shared crate), not inline in main.rs
- [ ] Each CLI logs to its tool-specific log file with stderr fallback
