---
assignees:
- claude-code
depends_on:
- 01KVTVACR1W8HFKFR8DTAJPMP0
position_column: todo
position_ordinal: a680
project: null
title: edit files — ambiguity returns candidates (not an error) + occurrence hint
---
## What
When a `find` has multiple confident matches and `replace_all` is false, `edit files` must return the candidate spans so the model disambiguates in one follow-up — instead of failing. Implemented in `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` on top of the cascade (consumes `MatchOutcome::Ambiguous` from `swissarmyhammer-edit-match`, and the "resolving anchor AND literal match both present" case from the cascade task).

- On ambiguity, return a structured result listing each candidate: line number, current text, and a few lines of surrounding context. This is a *successful* tool result describing the choice, not an `McpError`.
- Add an `occurrence` param (1-based index) and/or a line-hint to `EDIT_FILE_PARAMS` so the model can point precisely on the retry; when supplied and it resolves to exactly one candidate, apply it.
- `replace_all: true` continues to replace every match with no ambiguity.

## Acceptance Criteria
- [ ] Two+ confident matches with `replace_all` false return candidates (line numbers + current text + context), not an error, and the file is unchanged.
- [ ] Supplying `occurrence: N` selects the Nth candidate and applies the edit.
- [ ] A resolving anchor that also has a literal match surfaces both as candidates rather than guessing.
- [ ] `replace_all: true` replaces all matches with no candidate prompt.

## Tests
- [ ] Unit tests: ambiguous match returns candidate list + file untouched; `occurrence` disambiguates and applies; anchor-vs-literal both surfaced.
- [ ] `cargo test -p swissarmyhammer-tools edit::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.