---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvtzbtfg7cprdgr25awwxw09
  text: 'Picked up. Moved to doing. Studied sibling crate swissarmyhammer-hashline as the reference: pure IO-free crate, Cargo.toml uses `version.workspace = true` etc + `[lints] workspace = true`, doctest uses `Box<dyn std::error::Error>` with `?`, proptest in dev-deps + tests/ property file. Plan: hand-roll normalized-Levenshtein similarity (zero deps) rather than adding strsim to the workspace, since task says hand-roll is acceptable and it keeps deps minimal. Will follow /tdd: failing tests first.'
  timestamp: 2026-06-23T19:29:54.672653+00:00
- actor: claude-code
  id: 01kvtzv5y290fejhtyfsp7sya6
  text: |-
    Implementation landed. Created crates/swissarmyhammer-edit-match (zero runtime deps, proptest dev-dep). Public API: find_match(content, find) -> MatchOutcome {Unique{span,rung,confidence}|Ambiguous{candidates}|NoMatch{near}}, Rung {Exact|Normalized|Anchor|Fuzzy}, Span{range,start_line,end_line,text}, pub similarity(a,b)->f32 (normalized Levenshtein), and pub consts FUZZY_ACCEPT_THRESHOLD=0.85, FUZZY_RUNNER_UP_MARGIN=0.10, FUZZY_BOUNDARY_EPSILON=1e-4.

    Design notes for next agent:
    - Single-line `find` exact match must be LINE-ALIGNED (bounded by start/end or \n); a mid-line substring (e.g. un-indented form of an indented line) is rejected at Exact so Normalized recovers the full ORIGINAL indented line. Multi-line find = raw substring. This satisfies the acceptance criterion that dropped-indentation misses Exact but matches Normalized on original bytes.
    - FUZZY_BOUNDARY_EPSILON added because f32 makes 0.95-0.85 = 0.099999964 (an ULP below 0.10); threshold+margin comparisons use `>= CONST - EPSILON` so a candidate sitting exactly on the boundary counts as meeting it. Boundary tests assert with the same epsilon.
    - mcp files-edit tool does NOT bump mtime in a way cargo fingerprinting detects -> had to `touch` files to force recompiles. Watch for stale builds.

    Verification (all fresh, green): nextest 17/17 passed; doctest 1/1 passed; clippy --all-targets -D warnings clean; cargo fmt --check clean; cargo build -p swissarmyhammer-edit-match OK. Wired into root Cargo.toml members + [workspace.dependencies] mirroring swissarmyhammer-hashline.
  timestamp: 2026-06-23T19:38:17.922564+00:00
position_column: doing
position_ordinal: '8280'
project: file-edit-tools
title: swissarmyhammer-edit-match crate — pure literal-find ladder
---
## What
Create a new pure, IO-free crate `crates/swissarmyhammer-edit-match` (add to workspace `members`). It implements the literal-find cascade used by `edit files` when a `find` is a bare string (not a hashline anchor). A bare `find` is a *description* of a span, not a byte-exact copy.

Public API: `find_match(content: &str, find: &str) -> MatchOutcome` where
- `MatchOutcome` is one of `Unique { span: Range<usize>, rung: Rung, confidence: f32 }`, `Ambiguous { candidates: Vec<Span> }`, or `NoMatch { near: Vec<Span> }`.
- `Rung` = `Exact | Normalized | Anchor | Fuzzy`.
- `Span` carries byte range, 1-based start/end line, and the current text.

Cascade (stop at the first unique, confident match):
1. **Exact** — literal substring match (current behavior).
2. **Normalized** — match on whitespace-normalized forms (trim trailing, normalize line endings, optionally collapse indentation); return the span in the **original** content so the caller applies to original bytes.
3. **Anchor** — match unique first line and unique last line of `find`, replace the span between (tolerant of interior drift).
4. **Fuzzy** — similarity-scored (e.g. normalized Levenshtein / token ratio via `strsim`, or hand-rolled). **Pin concrete constants** (provisional is fine, but they must be named constants the tests assert against): accept a candidate as `Unique` only if its similarity ≥ `FUZZY_ACCEPT_THRESHOLD` (start at `0.85`) AND it exceeds the runner-up's similarity by ≥ `FUZZY_RUNNER_UP_MARGIN` (start at `0.10`). Otherwise return `Ambiguous` (≥2 above threshold within the margin) or `NoMatch` (none above threshold). Never apply a fuzzy match silently.

Pure: `(content, find) -> MatchOutcome`, no IO. Dependency-light.

## Acceptance Criteria
- [ ] `cargo build -p swissarmyhammer-edit-match` succeeds; no dependency on `swissarmyhammer-tools`.
- [ ] Exact match returns `Unique { rung: Exact }` with the correct byte span.
- [ ] A `find` that dropped leading indentation misses Exact but matches Normalized, and the returned span covers the **original** indented bytes.
- [ ] `FUZZY_ACCEPT_THRESHOLD` and `FUZZY_RUNNER_UP_MARGIN` are named public constants; a candidate at threshold−ε returns `NoMatch`, and a candidate above threshold but within the runner-up margin returns `Ambiguous`.
- [ ] Two equally-good matches with no tie-break return `Ambiguous` (never a silent pick).

## Tests
- [ ] Property tests in `tests/`: perturb whitespace/indentation/line-endings of a known span and assert the ladder lands on it; assert ambiguity is refused.
- [ ] Unit tests pinning the fuzzy boundary: assert exact behavior at `THRESHOLD ± ε` and at the runner-up margin boundary (deterministic against the named constants).
- [ ] Unit tests for each rung (Exact/Normalized/Anchor/Fuzzy) and for `NoMatch` near-miss spans.
- [ ] `cargo test -p swissarmyhammer-edit-match` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.