---
assignees:
- claude-code
depends_on:
- 01KMGQCDYS8CFHVK5BR517MER4
position_column: done
position_ordinal: ffffffffff8780
title: 'YAML merge strategy: three-way field merge with newest-wins from JSONL'
---
## What
Add a YAML merge strategy to `swissarmyhammer-merge` that does field-level three-way merging with changelog-aware conflict resolution. This handles pure `.yaml` entity files (tags, columns, actors, boards, swimlanes, views).

**Core algorithm:**
1. Parse base, ours, theirs as YAML mappings (`serde_yaml::Value::Mapping`)
2. For each field, compare ours and theirs against base:
   - Only ours changed → take ours
   - Only theirs changed → take theirs  
   - Neither changed → keep as-is
   - Field added only in one side → take the addition
   - Field removed only in one side → take the removal
   - **Both changed same field** → resolve via JSONL changelog (see below)
3. Output: merged YAML mapping serialized back to string

**Newest-wins conflict resolution:**
- Caller passes an optional JSONL changelog path (sibling file derived by the CLI driver)
- Read the changelog, find the most recent `ChangeEntry` touching each conflicting field
- Compare timestamps → take the field value from whichever side has the newer change
- Fallback if no JSONL or field not found: configurable precedence (default: theirs-wins)

**Files to create/modify:**
- `swissarmyhammer-merge/src/yaml.rs` — `merge_yaml(base, ours, theirs, opts) -> Result<String, MergeConflict>`
- `swissarmyhammer-merge/src/yaml.rs` — `MergeOpts { jsonl_path: Option<PathBuf>, fallback_precedence: Precedence }`
- `swissarmyhammer-merge/src/lib.rs` — add `pub mod yaml;`

**Key decisions:**
- Use `serde_yaml` for parsing, preserve field order where possible
- Only top-level field merge (not deep recursive) — our YAML entities are flat
- The JSONL reading is optional — works without it (just falls back to precedence)
- `serde_json` to parse JSONL changelog entries (only needs `timestamp` + `changes[].0` field name)
- The MD card (separate) reuses this for frontmatter merging

## Acceptance Criteria
- [ ] `merge_yaml(base, ours, theirs, opts)` auto-merges non-conflicting field changes
- [ ] Conflicting fields resolved by JSONL changelog timestamps when available
- [ ] Falls back to configurable precedence when no JSONL
- [ ] Handles field additions and removals from both sides

## Tests
- [ ] `swissarmyhammer-merge/src/yaml.rs` — inline `#[cfg(test)]` module
- [ ] Test: non-overlapping field changes auto-merge
- [ ] Test: conflicting field with JSONL → newest wins
- [ ] Test: conflicting field without JSONL → fallback precedence
- [ ] Test: field addition on one side only
- [ ] Test: field removal on one side only
- [ ] `cargo nextest run -p swissarmyhammer-merge yaml`