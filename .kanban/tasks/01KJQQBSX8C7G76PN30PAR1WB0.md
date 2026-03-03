---
title: 'Path traversal: entity IDs not sanitized'
position:
  column: done
  ordinal: a3
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/io.rs` lines 31-33

`entity_file_path` builds a path via `dir.join(format!("{}.{}", id, ext))`. If the caller passes an ID containing path separators (e.g. `../../etc/passwd`), the resulting path escapes the intended directory. The function `write_entity` then calls `create_dir_all` on the parent, which would create arbitrary directories.

While the kanban layer probably validates IDs before they reach this crate, the entity crate advertises itself as consumer-agnostic. A library that accepts arbitrary string IDs and writes to the filesystem has a responsibility to either validate or document the safety contract.

**Suggestion:** Add an ID validation function (or at least an assertion) in `entity_file_path` that rejects IDs containing `/`, `\`, `..`, or null bytes. Alternatively, document clearly that callers must sanitize IDs.

- [x] Add ID validation in `entity_file_path` that rejects path separators and `..`
- [x] Add a unit test with a malicious ID like `../../etc/passwd`
- [x] Verify the fix #warning