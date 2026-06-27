---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvvrxjvdkmc3qg5yyg1ep4gy
  text: 'Picked up. Research done. Plan: hashline::apply takes AnchorOp+replacement and returns whole-file content — does NOT fit the per-pair byte-range model edit/mod.rs needs (resolve_anchor returns Range<usize> for one line, must integrate with Candidate/literal both-rule and |text tie-breaker which the internal `resolve` lacks). So: add pure helper `resolve_anchor_in(content, line, hash, text: Option<&str>) -> Option<usize>` (1-based resolved line) to swissarmyhammer-hashline, unit-test it there, and refactor the internal `resolve` to delegate to it (keeps `apply` correct AND makes proximity reachable from prod via resolve_anchor). edit/mod.rs::resolve_anchor will: parse_anchor for (line,hash), extract |text suffix, call resolve_anchor_in, map 1-based line -> byte range via line_text_range. |text = tie-breaker: prefer in-window candidate whose trimmed text matches; if none match text but hash matches, fall back to nearest-hash (text is verification/fallback per design line 132, not a hard gate); nothing hashes in-window -> None (stale falls through, unchanged). Both-candidates rule already lives in resolve_pair''s (anchor,literal) match arm — proximity-resolved range flows through it unchanged. TDD: failing tests first in both crates.'
  timestamp: 2026-06-24T02:56:31.085020+00:00
- actor: claude-code
  id: 01kvvsrr2tb0tza6v3naft0z3t
  text: |-
    Implementation landed. hashline: added pub `resolve_anchor_in` (1-based line) AND `resolve_anchor_range_in` (byte Range) sharing one private `resolve_index` over `&[&str]`; internal `resolve` (used by `apply`) now delegates to `resolve_anchor_in` -> proximity reachable from prod. edit/mod.rs::resolve_anchor parses anchor, extracts |text via split_once('|'), calls resolve_anchor_range_in; both-candidates rule unchanged (proximity range flows through resolve_pair's (anchor,literal) arm). description.md documents drift tolerance.

    double-check found a HIGH defect: my first cut mapped the resolved 1-based line back to bytes via the tools `line_text_range` which counts ONLY \n, while resolution used hashline's CR-aware split_lines — on a CR-only/mixed-ending file a VALID line-1 anchor mapped to 0..wholefile and would clobber the file. Fix: added resolve_anchor_range_in so resolution AND byte-range share ONE line model; deleted now-dead line_text_range + trim_trailing_cr in tools. Added guard tests: hashline resolve_anchor_range_in_cr_only_excludes_terminator (asserts 0..1) + crlf + stale + drifted; edit-path cascade_anchor_on_cr_only_file_replaces_single_line (a\rb\rc + line-1 anchor -> A\rb\rc, only line 1 changed).

    Green: hashline 40/40 (28 prior + 7 resolve_anchor_in + 5 range), tools edit::/files:: 232/232, tools full 1366/1366, doctest 1/1, clippy -D warnings clean, fmt clean. No prior test modified. Re-running double-check to confirm CR fix.
  timestamp: 2026-06-24T03:11:21.178567+00:00
- actor: claude-code
  id: 01kvvt00rm0kp54szch47sxed7
  text: |-
    double-check re-verification returned PASS: CR/mixed data-loss path closed (no whole-file clobber; valid CR-only line-1 anchor -> only line 1 replaced), dead code (line_text_range/trim_trailing_cr) removed cleanly with no unused-import/dead-code breakage, prior safety properties preserved (stale->None, |text can never select a non-hash-matching line), LF/CRLF unaffected. Suites green. Applied its one cosmetic nit: resolve_anchor doc comment now references resolve_anchor_range_in (doc-only). Final re-run after doc fix: clippy -D warnings clean, tools edit::/files:: 232/232, hashline 40/40. Moving to review.

    Acceptance criteria all met: drifted anchor relocates+edits; |text relocates drifted, mismatch does not mis-apply; stale falls through unchanged; resolve_anchor_in/resolve_anchor_range_in (and via them hashline::apply's `resolve`) are now reachable from production; description.md documents drift tolerance.
  timestamp: 2026-06-24T03:15:19.444299+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffde80
project: file-edit-tools
title: edit files — wire anchor drift recovery (proximity + |text relocation) into resolve_anchor
---
## What
The cross-cutting double-check found a design-intent gap: the `edit files` anchor rung resolves EXACT line N only, but `ideas/file-edit-tools.md` specifies anchor **drift recovery** — and the machinery is already built and tested in the hashline crate but never called from the edit path (dead code).

Design doc intent (`ideas/file-edit-tools.md`):
- Line 170: "`apply(content, ops)` — resolve each anchor (exact line N, **else proximity-search nearby lines for one hashing to HH**)".
- Lines 99-100, 132: the `|text` suffix "serves as verification/**fallback**" — "if line 2 drifted, the text relocates it, else reject."

Current state (`crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs`):
- `resolve_anchor` imports only `hash_line`/`parse_anchor`; it checks the EXACT line N and, on hash mismatch, returns `None` → falls through to literal interpretation of the whole `find` (which still carries the `N:HH|` prefix as literal text, so it does NOT relocate). The `|text` suffix is parsed off and discarded.
- `swissarmyhammer_hashline::apply` (with `PROXIMITY_WINDOW = 50`, symmetric nearest-wins search, `|text` not yet used) is fully implemented and unit-tested but NEVER called from production — dead code from the tool's perspective.

## Approach
Wire the edit anchor rung through the hashline crate's drift recovery so a drifted anchor relocates within ±`PROXIMITY_WINDOW`, and the optional `|text` suffix verifies/relocates:
- In `resolve_anchor` (edit/mod.rs): when the exact line N does NOT hash to `HH`, proximity-search nearby lines (reuse `swissarmyhammer_hashline::apply`/`AnchorOp`, or a thin `resolve`-style helper exposed by the hashline crate) for the nearest line hashing to `HH`; resolve to that line's span. If a `|text` suffix is present, use it as a tie-breaker/verification (prefer the proximity candidate whose text matches `|text`); if nothing in-window hashes to `HH`, return `None` (fall through to literal/near-miss — unchanged safety rule).
- Preserve existing invariants: an anchor interpretation only WINS when it resolves; the "resolving anchor AND a literal match both present → surface both as candidates" rule must still hold for the proximity-resolved case (route through the existing `Candidate`/ambiguity path, do not guess).
- Prefer reusing `swissarmyhammer_hashline::apply` over re-implementing proximity in the tools crate (removes the dead-code gap). If `apply`'s shape doesn't fit the per-pair span model, expose a small pure `resolve_anchor_in(content, line, hash, text) -> Option<usize>` from the hashline crate and call it; add its unit tests there.
- Update `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/description.md` to document that anchors tolerate small drift (now true).

## Acceptance Criteria
- [ ] A drifted anchor (correct `HH`, but the line moved a few lines from N within the window) resolves to the relocated line and edits it — exercised through the real `execute_edit` path, file mutated correctly.
- [ ] A `N:HH|text` anchor whose line drifted relocates using `|text` as verification; a `|text` that matches no in-window candidate does NOT mis-apply.
- [ ] A truly stale anchor (no line in-window hashes to `HH`) falls through to literal/near-miss exactly as today (no regression, no mis-apply).
- [ ] `swissarmyhammer_hashline::apply` (or the new `resolve_anchor_in` helper) is now reachable from the production edit path — no longer dead code.
- [ ] `edit/description.md` documents drift tolerance.

## Tests
- [ ] New edit-path tests in `edit/mod.rs`: drifted-anchor-relocates-and-edits; `|text`-relocates-drifted; stale-anchor-falls-through-to-near-miss; anchor-proximity-and-literal-both-present-surfaces-candidates.
- [ ] If a new hashline helper is added, unit-test it in `crates/swissarmyhammer-hashline`.
- [ ] `cargo nextest run -p swissarmyhammer-tools edit:: files::` and `cargo nextest run -p swissarmyhammer-hashline` green (NEVER plain `cargo test`; doctests via `--doc`).
- [ ] `cargo fmt` + `cargo clippy -p swissarmyhammer-tools -p swissarmyhammer-hashline -- -D warnings` clean.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.