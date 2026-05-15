---
position_column: done
position_ordinal: 9f80
title: Inconsistent spacing in req() calls in dispatch.rs
---
swissarmyhammer-kanban/src/dispatch.rs:43,67,68,76,80,etc.\n\nAll `req(op,\"...\")` calls are missing a space after the comma: `req(op,\"name\")` instead of `req(op, \"name\")`. This is inconsistent with the rest of the codebase which uses standard Rust formatting.\n\nSuggestion: Run `cargo fmt` on dispatch.rs."