---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb780
title: BoardHandle::open error handling uses .map_err(|e| format!()) -- loses error chain
---
kanban-app/src/state.rs -- BoardHandle::open()\n\nThis is application code (Tauri), so `anyhow` is the recommended error type. Currently all errors are mapped to `String` via `format!(\"{e}\")`, which discards the error chain. The Rust review guidelines require `.context()` on every `?` in app code.\n\nMultiple `.map_err(|e| format!(\"...: {e}\"))` calls lose the source error for debugging.\n\nSuggestion: Switch the return type from `Result<Self, String>` to `anyhow::Result<Self>` and use `.context(\"what we were doing\")` instead of `.map_err(format)`. #review-finding