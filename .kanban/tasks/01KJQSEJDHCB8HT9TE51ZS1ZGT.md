---
title: 'WARNING: apply_unified_diff silently uses diff context over actual file content'
position:
  column: todo
  ordinal: b2
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 306-310

**What:** When processing context lines (lines starting with ` `), `apply_unified_diff` copies the content from the DIFF, not from the old text. It does `result.push(line[1..].to_string())` (the diff's context line) while advancing `old_idx`, but never verifies that `old_lines[old_idx]` matches the diff's context.

**Why:** If a diff is applied to a file that has been modified since the diff was created (stale diff), the context lines will silently overwrite the actual file content with whatever the diff expected to see. In most diff tools, mismatched context is an error. Here, it causes silent data corruption.

In the current codebase, diffs are always created and applied in tight sequences, so staleness is unlikely. But this is a latent correctness issue if the changelog replay mechanism is ever used for conflict resolution or if entities are modified concurrently.

**Suggestion:** Either (a) validate that `old_lines[old_idx] == &line[1..]` and return an error on mismatch, or (b) use `old_lines[old_idx]` instead of the diff's context line (trusting the file over the diff). Option (a) is safer.

- [ ] Add context line validation or use old_lines content instead of diff content
- [ ] Add a test that detects stale diff application
- [ ] Decide on error behavior: return `Result<String>` with an error, or log a warning #warning