---
position_column: done
position_ordinal: e0
title: delete_entity_files uses sync path.exists() -- TOCTOU race and sync in async
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/io.rs` lines 124-137

`delete_entity_files` calls `path.exists()` (sync `std::fs`) followed by `fs::remove_file` (async `tokio::fs`). This has two issues:

1. **TOCTOU race**: The file could be deleted between the `exists()` check and the `remove_file` call, causing an unexpected error. Or the file could be created between the check and the delete.
2. **Sync I/O in async context**: `path.exists()` calls `std::fs::metadata()` synchronously, blocking the Tokio thread pool. In practice this is unlikely to be noticeable for metadata calls, but it is an anti-pattern.

**Suggestion:** Replace with a try-delete pattern: call `fs::remove_file` directly and match on `ErrorKind::NotFound` to ignore it. This is both race-free and fully async. Example:
```rust
match fs::remove_file(path).await {
    Ok(()) => {},
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
    Err(e) => return Err(e.into()),
}
```

- [ ] Replace `exists()` + `remove_file()` with try-delete-then-ignore-NotFound for both data and log files
- [ ] Verify with tests #warning