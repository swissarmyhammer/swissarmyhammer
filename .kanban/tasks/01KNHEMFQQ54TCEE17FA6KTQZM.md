---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff9780
title: Store-event dedup uses String cloning for every watcher event in hot path
---
kanban-app/src/commands.rs: flush_and_emit_for_handle\n\nThe dedup logic builds a `HashSet<(String, String)>` by cloning entity_type and id for every watcher event:\n```rust\nseen.insert((entity_type.clone(), id.clone()));\n```\n\nThen for each store event, it also clones store_name and id into a `(String, String)` tuple for the `contains` check. This allocates on every event in a path that runs after every command.\n\nSuggestion: Use `HashSet<(&str, &str)>` with borrowed references for the watcher events, then borrow the store event strings for the lookup. The watcher events Vec lives for the duration of the function, so the borrows are valid.\n\nVerification: Refactor to borrowed strings, run tests, confirm no behavior change. #review-finding