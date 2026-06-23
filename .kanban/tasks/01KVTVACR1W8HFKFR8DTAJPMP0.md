---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvv4qtkch1jb8vt4k2syw0fz
  text: |-
    Picked up. Studied sibling crates: swissarmyhammer-hashline (parse_anchor/hash_line/apply/AnchorOp) and swissarmyhammer-edit-match (find_match -> MatchOutcome {Unique{span,rung,confidence}, Ambiguous, NoMatch}). edit/mod.rs already has normalize_edit_args -> Vec<EditPair{find,replace,replace_all}> from ^krpb70r. hashline already in tools Cargo.toml; edit-match needs adding (workspace dep already declared).

    Design: replace per-pair edit_file_atomic loop with ONE batch-atomic apply: read file once (encoding+line-ending detect), resolve every pair against working string in sequence, write once. Per pair: replace_all=true -> literal global replace (legacy). replace_all=false -> try anchor (parse_anchor AND resolves via hash) -> replace whole LINE; else find_match literal -> Unique replaces span; Ambiguous/NoMatch -> clear error (seam for downstream ^0fvjsv4/^5tj0c9z). Key tension: legacy "replace first mid-line occurrence" tests (multiple_occurrences_without_replace_all) — find_match rejects non-line-aligned single-line finds as None. Evaluating which legacy tests legitimately change.
  timestamp: 2026-06-23T21:03:50.892167+00:00
- actor: claude-code
  id: 01kvv55t0dmmq23jkd0067atbc
  text: |-
    Implementation landed (TDD red->green). Changes in edit/mod.rs:
    - Added swissarmyhammer-edit-match to tools Cargo.toml (hashline already present).
    - New cascade resolution: Resolution enum {Splice{range,replacement}, GlobalLiteral{find,replace}}; resolve_pair() runs anchor->literal-substring->find_match ladder per pair; apply_all_pairs() resolves+applies all pairs to an in-memory working string; apply_resolution() splices.
    - Anchor dispatch: resolve_anchor() = parse_anchor AND hash_line(line)==expected; resolving anchor replaces whole LINE (line_text_range, terminator excluded). Stale anchor -> None -> literal. (Some,Some) anchor+literal both present -> ambiguity error (seam for ^0fvjsv4). find_match Ambiguous/NoMatch -> clear error (seams for ^0fvjsv4/^5tj0c9z).
    - Atomicity: extracted commit_content() from edit_file_atomic (temp-write+rename+encoding+line-ending+metadata). execute_edit now reads file ONCE, apply_all_pairs in memory, commit_content ONCE. Any pair failure -> file byte-identical (no commit). Legacy edit_file_atomic kept for single-pair test callers, now routes through commit_content too.
    - replace_all=true preserved as literal global replace (legacy). replace_all=false: first literal occurrence (legacy mid-line substring tests stay green).
    - context.record_mutated_path(path) still called on success.
    - Updated edit/description.md to document the find/replace + hashline anchor cascade + atomicity.

    Tests: 7 new cascade tests (anchor->whole-line, stale-anchor->literal, normalized span preserves indentation, atomic rollback byte-identical, multi-pair commit-all, records mutated path, empty replace deletes). RED confirmed 2 failed pre-impl (atomic rollback + anchor whole-line); all green after.

    cargo nextest run -p swissarmyhammer-tools edit:: files:: => 220 passed. Full -p swissarmyhammer-tools => 1353 passed 0 failed. --doc => 8 passed. clippy -D warnings clean. fmt clean.
  timestamp: 2026-06-23T21:11:29.037880+00:00
- actor: claude-code
  id: 01kvv5amp7gqx4ts8kmyzb8754
  text: 'really-done: verification command green (edit:: files:: => 220 passed; full -p swissarmyhammer-tools => 1353 passed). Ran double-check adversarial gate -> REVISE with ONE finding: description.md over-promised hashline proximity-drift recovery ("or a nearby line it drifted to") that the implementation doesn''t do — resolve_anchor resolves the EXACT line only (parse_anchor + hash_line==expected), it does not call hashline::apply''s PROXIMITY_WINDOW search. Per task scope (anchor "resolves (line exists + hash matches)"), exact-line resolution is correct; fixed description.md to drop the drift claim (option a). Re-ran edit:: files:: => 220 passed. No logic change. double-check''s other note (literal first-occurrence not routed through find_match Ambiguous) is the intentional legacy-preserving seam deferred to ^0fvjsv4 — no action.'
  timestamp: 2026-06-23T21:14:07.431830+00:00
- actor: claude-code
  id: 01kvv67aqweet6xc6vpv2k5rfk
  text: |-
    Worked all 4 review findings (style/clarity only, zero behavior change) in edit/mod.rs:
    - Warning (ambiguity error): removed trailing period -> "...Disambiguate the edit".
    - Warning (no-unique-target error): removed trailing period -> "...or a hashline anchor".
    - Warning (line_text_range nesting): extracted `fn trim_trailing_cr(bytes, start, end) -> usize` and replaced the `while`-loop body's nested if/if with a `for` + `continue` guard. Both the in-loop and final-line CR-trim sites now call the helper; nesting reduced from 4 to 2 levels, CR-trim logic de-duplicated.
    - Nit: added Debug to EditFileTool derive -> #[derive(Default, Debug)].

    Cascade logic, anchor-vs-literal dispatch, and all tests untouched. Verification:
    - cargo nextest run -p swissarmyhammer-tools edit:: files:: => 220 passed, 0 failed.
    - cargo clippy -p swissarmyhammer-tools -- -D warnings => clean (exit 0, zero warnings).
    - cargo fmt => clean.
    All 4 findings flipped to [x]. Task stays in review.
  timestamp: 2026-06-23T21:29:47.516481+00:00
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
- 01KVTV8TDX7RVS3Q91ZS05QB30
- 01KVTV96NPVW5RXGRX8KRPB70R
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd780
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

## Review Findings (2026-06-23 15:15)

### Warnings
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs:400` — Error message ends with a period. The error-handling rule specifies Display messages should be lowercase with no trailing punctuation. Remove the trailing period: `"'{}' is ambiguous: it resolves as a hashline anchor and also occurs as literal text. Disambiguate the edit"`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs:430` — Error message ends with a period. The error-handling rule specifies Display messages should be lowercase with no trailing punctuation. Remove the trailing period: `"'{}' matches {} locations; no unique target. Provide more surrounding context or a hashline anchor"`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs:1650` — Function `line_text_range` has 4 levels of nesting (while → if → if → if), exceeding the 3-level threshold. The innermost condition checking for CR characters is buried deep within the loop structure. Extract the CR trimming logic into a helper function. Replace the 4-level nested if with a call to `fn trim_trailing_cr(bytes: &[u8], start: usize, end: usize) -> usize` that returns the adjusted end position. This reduces nesting from 4 to 2 levels and improves readability.

### Nits
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs:85` — Public struct EditFileTool does not derive Debug. The trait-implementations rule requires new public types to implement all applicable traits; Debug is necessary for downstream code to debug-print instances without reimplementation. Add Debug to derive: `#[derive(Default, Debug)]`.