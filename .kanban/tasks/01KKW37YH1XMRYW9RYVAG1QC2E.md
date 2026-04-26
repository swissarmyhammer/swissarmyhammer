---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffc480
title: resolve_handle silently falls back to uncanonicalised path on canonicalize failure
---
kanban-app/src/commands.rs:28-30\n\n```rust\nlet canonical = PathBuf::from(&bp)\n    .canonicalize()\n    .unwrap_or_else(|_| PathBuf::from(&bp));\n```\n\nIf `canonicalize` fails (e.g. path does not exist yet, or is a symlink in an unusual state), the code silently falls back to the raw path. The boards map is keyed by canonical paths (set during `open_board`). A raw path will never match a canonical key, so the lookup returns `None` and the caller gets "Board not open" with no indication that the path canonicalization failed.\n\nThis is the same silent-fallback pattern used in `set_active_board` and `open_board`. It is an existing pattern, not introduced here, but the new `resolve_handle` helper centralises it without comment about the failure case.\n\nSuggestion: Add an inline comment explaining why the fallback is safe (or document in the function's doc comment that paths must be pre-opened and thus should already be canonical)."