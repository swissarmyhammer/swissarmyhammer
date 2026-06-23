---
assignees:
- claude-code
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
- 01KVTV8TDX7RVS3Q91ZS05QB30
- 01KVTV96NPVW5RXGRX8KRPB70R
position_column: todo
position_ordinal: a580
project: file-edit-tools
title: edit files — cascade application core (anchor + literal ladder), atomic batch
---
## What
Replace the sequential `content.matches`/`replacen` apply logic in `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` with the shape-inferred cascade over the canonical `Vec<(find, replace)>` from the normalization task. Wire in `swissarmyhammer-hashline` and `swissarmyhammer-edit-match` (add both to `swissarmyhammer-tools` deps). **Scope is the apply core only** — idempotency/no-op and consumed-target detection are a separate follow-on task.

Per pair, run the cascade on `find`:
1. If `find` parses as a hashline anchor (`N:HH` or `N:HH|text`) **and resolves** (line exists and hashes to HH; `|text` is verification/relocation fallback) → replace that whole line.
2. Else literal: run `swissarmyhammer_edit_match::find_match`. On `Unique`, replace that span.
- Replace semantics follow what `find` resolved to: anchor → replace the **line**; span → replace the **span**.
- Safety rule: a structured (anchor) interpretation only **wins** when it resolves; `42:a3` is literal text if line 42 doesn't hash to `a3`. (If a resolving anchor AND a literal match both exist, that surfaces as ambiguity — handled by the ambiguity task; here just leave the hook for it.)
- **Atomic batch**: resolve all pairs first, then commit in one rewrite; any failure leaves the file byte-identical. Reuse the existing temp-write+rename / encoding + line-ending preservation (`edit_file_atomic`).
- Delete = empty `replace`. Insert = replace a line with itself plus new content (no special op).
- Keep recording the mutated path via `context.record_mutated_path` so the existing `inline_diagnostics` fold-in still fires (`crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs`).
- Update `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/description.md` to document the forgiving `find`/`replace` + hashline-anchor surface.

This supersedes the edit/replace split — one op, no separate `replace files`.

## Acceptance Criteria
- [ ] A `N:HH` anchor that resolves replaces that line; a stale anchor falls through to literal text (no mis-apply).
- [ ] A bare-string `find` is matched via the ladder and the span replaced; normalized-match preserves surrounding bytes/indentation.
- [ ] Multi-pair batch is atomic: a single failing pair leaves the file byte-identical.
- [ ] `context.record_mutated_path` is still called on success (diagnostics fold-in unaffected).
- [ ] `edit/description.md` documents the new surface.

## Tests
- [ ] Unit/integration tests in `edit/mod.rs`: anchor resolve, stale-anchor-as-literal, normalized span apply, atomic rollback on failure (file byte-identical).
- [ ] A test asserting the mutated path is recorded (diagnostics fold-in path).
- [ ] `cargo test -p swissarmyhammer-tools edit::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.