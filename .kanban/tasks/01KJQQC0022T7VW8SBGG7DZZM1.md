---
title: Atomic write uses .tmp extension -- collision risk with concurrent writes
position:
  column: done
  ordinal: d7
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/io.rs` line 81

`write_entity` uses `path.with_extension("tmp")` for the temp file. If two concurrent writes to the same entity happen, they will both write to the same `.tmp` file, causing a data race. The existing `swissarmyhammer-fields/src/context.rs` (line 364) uses a ULID-based temp filename (`.tmp_{ulid}`) which avoids this issue.

Additionally, if the write fails after creating the temp file but before the rename, the `.tmp` file is never cleaned up.

**Suggestion:** Use a unique temp filename (e.g. `.tmp_{ulid}` or `tempfile` crate) instead of a fixed `.tmp` extension. Consider adding cleanup-on-error for the temp file.

- [ ] Replace `path.with_extension("tmp")` with a unique temp filename (e.g. using ULID or random suffix)
- [ ] Add cleanup of the temp file if `fs::rename` fails
- [ ] Verify with tests #warning