---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffd980
title: 'JSONL merge library: new swissarmyhammer-merge crate'
---
## What
Create a new crate `swissarmyhammer-merge` containing the core JSONL merge logic, independent of git or CLI. This keeps merge strategies isolated and extensible for future file types.

**Algorithm:**
1. Read three inputs (base/ours/theirs) as `&str`
2. Parse each line, extract the `"id"` field (first key, ULID string)
3. Collect all lines into a `BTreeMap<String, String>` keyed by id (ULID sorts lexicographically = chronological)
4. If same id appears in ours AND theirs with different content → return conflict error
5. Otherwise, write deduplicated lines sorted by id key
6. Return `Ok(merged_content)` or `Err(conflict_details)`

**Files to create:**
- `swissarmyhammer-merge/Cargo.toml` — new crate, deps: `serde_json` (extract id field only)
- `swissarmyhammer-merge/src/lib.rs` — re-exports, error types
- `swissarmyhammer-merge/src/jsonl.rs` — `merge_jsonl(base, ours, theirs) -> Result<String, MergeConflict>`
- `Cargo.toml` (workspace) — add `swissarmyhammer-merge` to workspace members

**Key decisions:**
- Use `serde_json::Value` only to extract `id`, then keep the original line bytes (preserves formatting)
- BTreeMap gives us sorted-by-ULID output for free
- Lines that fail to parse (empty, malformed) are preserved at the end with a warning
- Crate is pure logic, no filesystem I/O — callers pass strings in, get strings out

## Acceptance Criteria
- [ ] New `swissarmyhammer-merge` crate exists in workspace
- [ ] `merge_jsonl(base, ours, theirs) -> Result<String, MergeConflict>` works for disjoint appends
- [ ] Duplicate ids with identical content are deduplicated
- [ ] Duplicate ids with differing content return `Err`
- [ ] Output is sorted by ULID id
- [ ] Empty inputs handled gracefully

## Tests
- [ ] `swissarmyhammer-merge/src/jsonl.rs` — inline `#[cfg(test)]` module
- [ ] Test: two branches append different entries → union
- [ ] Test: overlapping identical entries → deduplicated
- [ ] Test: same id, different content → conflict error
- [ ] Test: empty base, ours, theirs combinations
- [ ] `cargo nextest run -p swissarmyhammer-merge` #merge-driver