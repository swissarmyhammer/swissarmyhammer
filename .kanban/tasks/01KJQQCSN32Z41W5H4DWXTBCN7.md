---
title: read_entity_dir silently swallows all parse errors
position:
  column: todo
  ordinal: a8
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/io.rs` line 116

```rust
Err(_) => continue, // Skip unparseable files
```

This silently discards *all* errors when reading entity files from a directory, including I/O errors (permission denied, disk failure), not just parse errors. If a file is corrupted or has a permissions issue, the caller has no way of knowing. The comment says "skip unparseable" but the code skips everything.

**Suggestion:** At minimum, log a warning (via `tracing` or `log`). Better: distinguish between "expected" failures (parse errors) and "unexpected" failures (I/O errors). Consider collecting errors into a `Vec<EntityError>` returned alongside the entities, or returning `Result<Vec<Entity>>` with a variant that carries partial results + errors.

- [ ] At minimum, add `tracing::warn!` for skipped files
- [ ] Consider differentiating parse errors (skip) from I/O errors (propagate)
- [ ] Verify with tests #warning