---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: 'WARNING: default_virtual_tag_registry is called fresh on every command invocation'
---
**File:** swissarmyhammer-kanban/src/task/list.rs, next.rs, kanban-app/src/commands.rs\n\n**What:** Each of `ListTasks::execute`, `NextTask::execute`, and the Tauri `list_entities` / `read_entity` commands calls `default_virtual_tag_registry()` to create a new registry on every invocation. The function allocates a new `HashMap`, boxes three strategy structs, and pushes three slug strings.\n\n**Why this matters:** The registry is immutable once built -- it never changes at runtime. Recreating it on every request is wasteful. For a CLI tool this is negligible, but the Tauri app calls `list_entities` on every UI render (board view, drag, filter change), making this a hot path.\n\n**Suggestion:** Store the registry once on `KanbanContext` (or use `LazyLock`/`OnceLock` for a static singleton) and pass `&VirtualTagRegistry` into enrichment functions. This also creates a natural extension point for user-defined virtual tags.\n\n**Verification:** No behavioral test needed -- this is a performance/architecture concern." #review-finding