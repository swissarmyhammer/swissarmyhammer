---
assignees:
- claude-code
depends_on:
- 01KMGQNHB64JZBK84A0V5QMAZY
position_column: done
position_ordinal: ffffffffff8980
title: 'Markdown merge strategy: YAML frontmatter + line-level body merge'
---
## What
Add an `.md` merge strategy to `swissarmyhammer-merge` that splits the file at the frontmatter fence (`---`), merges each part with the appropriate strategy, then reassembles.

**Algorithm:**
1. Split each of base/ours/theirs into frontmatter (between `---` fences) and body (everything after closing `---`)
2. Parse frontmatter as YAML → use the same three-way field merge from `yaml.rs` (with JSONL newest-wins for conflicts)
3. Merge the markdown body using standard line-level three-way merge (union of non-conflicting hunks, conflict markers for true conflicts)
4. Reassemble: `---\\n{merged frontmatter}\\n---\\n{merged body}`

**Body merge approach:**
- Use `diffy` crate (already a workspace dependency) for three-way diff
- Or implement a simple line-level three-way merge: diff ours vs base, diff theirs vs base, apply non-overlapping hunks, flag overlapping as conflicts
- Non-overlapping changes from both sides auto-merge (e.g., one side edits description paragraph, other side adds acceptance criteria)
- Overlapping changes → git-style conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) and exit 1

**Files to create/modify:**
- `swissarmyhammer-merge/src/md.rs` — `merge_md(base, ours, theirs, opts) -> Result<String, MergeConflict>`
- `swissarmyhammer-merge/src/frontmatter.rs` — `split_frontmatter(content) -> (Option<String>, String)` utility
- `swissarmyhammer-merge/src/lib.rs` — add `pub mod md; pub mod frontmatter;`

**Key decisions:**
- Reuse the YAML field merge from `yaml.rs` for frontmatter — don't duplicate
- Body merge is line-based, not character-based
- If a file has no frontmatter, treat entire content as body (line-level merge only)
- `MergeOpts` shared with yaml.rs (jsonl_path for newest-wins, fallback precedence)

## Acceptance Criteria
- [ ] `merge_md(base, ours, theirs, opts)` correctly splits frontmatter from body
- [ ] Frontmatter fields merged via three-way YAML field merge (same as yaml.rs)
- [ ] Body merged line-by-line — non-overlapping changes auto-merge
- [ ] Overlapping body changes produce conflict markers and return conflict
- [ ] Files without frontmatter merge as pure line-level text
- [ ] Reassembled output preserves `---` fences correctly

## Tests
- [ ] `swissarmyhammer-merge/src/md.rs` — inline `#[cfg(test)]` module
- [ ] Test: frontmatter-only changes (body identical) → clean merge
- [ ] Test: body-only changes (frontmatter identical) → clean merge
- [ ] Test: both frontmatter and body changed on different sides → clean merge
- [ ] Test: overlapping body edits → conflict markers
- [ ] Test: no frontmatter file → pure line merge
- [ ] `cargo nextest run -p swissarmyhammer-merge md` #merge-driver