---
title: Consider extracting shared execute() scaffolding in command modules
position:
  column: todo
  ordinal: d6
---
Many command modules (`task/add.rs`, `task/mv.rs`, `task/complete.rs`, etc.) repeat nearly identical `execute()` patterns for opening context, calling entity_context, and building JSON responses. A future pass could extract the shared scaffolding into a helper or macro.

- [ ] Evaluate common patterns across command execute() impls
- [ ] Extract shared scaffolding if beneficial
- [ ] Verify tests still pass