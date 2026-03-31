---
assignees:
- claude-code
depends_on:
- 01KMGQDC1CB4FAFAHXGHQ6M2QP
position_column: done
position_ordinal: ffffffffff8a80
title: 'End-to-end test: git merge with all three drivers'
---
## What
Integration test that proves the full git workflow for all three merge drivers: `.jsonl` (union-by-id), `.yaml` (field-level), `.md` (frontmatter + body).

**Files to create/modify:**
- `swissarmyhammer-cli/tests/merge_e2e.rs` — end-to-end test

**Test scenarios (each in a temp git repo):**

**JSONL:**
1. Base: `.kanban/tasks/test.jsonl` with 2 entries
2. Branch A: append entry with id `01AAA...`
3. Branch B: append entry with id `01BBB...`
4. Merge → all 4 entries, sorted by ULID, clean
5. Conflict case: same id, different content → merge fails

**YAML:**
1. Base: `.kanban/tags/test.yaml` with `name: foo`, `color: ff0000`
2. Branch A: changes `color` to `00ff00`
3. Branch B: adds `description: bar`
4. Merge → has all three fields, clean
5. Conflict case: both sides change `color` → newest-wins via sibling `.jsonl`

**Markdown:**
1. Base: `.kanban/tasks/test.md` with frontmatter `title: X` and body
2. Branch A: changes `title` in frontmatter
3. Branch B: edits body text
4. Merge → updated title + updated body, clean
5. Conflict case: both edit same body section → conflict markers

**Build note:** Use `env!("CARGO_BIN_EXE_sah")` or `assert_cmd` pattern for binary path.

## Acceptance Criteria
- [ ] Clean merge of divergent JSONL appends via actual `git merge`
- [ ] Clean merge of non-overlapping YAML field changes via `git merge`
- [ ] Clean merge of frontmatter + body changes via `git merge`
- [ ] Conflict detection works through git for all three types
- [ ] Test is `#[ignore]` by default (requires git, slower) with CI annotation

## Tests
- [ ] `swissarmyhammer-cli/tests/merge_e2e.rs`
- [ ] `cargo nextest run -p swissarmyhammer-cli merge_e2e -- --ignored`