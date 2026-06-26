---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2dzgg18tzevkr8d91mqee5
  text: |-
    Implemented `crates/swissarmyhammer-expect/src/spec.rs`: `Expectation`, `Frontmatter` (deny_unknown_fields, all 11 closed keys with documented defaults), `Criterion`, plus `ReliabilityPolicy` (parses `pass^N`), `Isolation` (default shared), `Setup` (string|list), and a `duration_str` serde module for `30s`/`5m` timeouts. Added `serde_yaml_ng` dep; wired module + re-exports into lib.rs. Reuses the existing `Surface` and `VerdictTier` enums from types.rs. Wrote a small frontmatter splitter + single-pass section walker rather than depending on templating (matches the prevailing per-crate pattern; validators has its own too). Criteria are collected from any GFM checklist item in the body so the no-`## Then`-header / criteria-list case parses.

    TDD: 13 new spec tests covering the coupon worked example (description, surface cli, reliability pass^3, 2 Given, 2 When, 4 Then unchecked, Notes block), unknown-key rejection (names `surfce`), default application, no-GWT, zero criteria, outside-repo-root, missing frontmatter, timeout/reliability forms, ReliabilityPolicy round-trip, setup string|list.

    Verified: `cargo nextest run -p swissarmyhammer-expect` = 21 passed; `cargo clippy -p swissarmyhammer-expect --all-targets -- -D warnings` clean; `cargo test --doc` 1 passed; double-check agent verdict PASS. Left in `doing` for /review.
  timestamp: 2026-06-26T17:00:00.897497+00:00
- actor: claude-code
  id: 01kw2eyy1sc6kmv8h8zgmpxh3t
  text: |-
    Addressed all 6 review findings in crates/swissarmyhammer-expect/src/spec.rs:
    1. ReliabilityPolicy.required is now private with a `pub fn required(&self) -> u32` getter; the ≥1 invariant stays enforced in Deserialize and can't be bypassed by external construction. Updated 3 in-crate test usages to call required().
    2. Hoisted the parse_bullet check out of the match (Notes captured first, then bullet-parse + small inner match for Given/When) — routing now caps at 3 nesting levels.
    3-5. Added module-level consts MILLIS_PER_HOUR/MINUTE/SECOND (u128) in duration_str, used by both serialize() and parse_duration().
    6. Added const CHECKBOX_MARKER_LEN: usize = 3, used in both spots in parse_criterion.

    Verification: cargo nextest run -p swissarmyhammer-expect = 21 passed, 0 failed; cargo test --doc = 1 passed; cargo fmt applied; cargo clippy --all-targets -D warnings clean. Left in doing for review.
  timestamp: 2026-06-26T17:17:10.585509+00:00
- actor: claude-code
  id: 01kw2farv01fv9yrfprf8yd5yn
  text: 'Cleared the open review nit (2026-06-26 12:18): added module-level `const MILLIS_PER_MILLIS: u128 = 1;` in the `duration_str` module and used it in the `ms` branch of `parse_duration` (was bare literal `1`), matching the existing MILLIS_PER_SECOND/MINUTE/HOUR pattern. Verified: cargo nextest run -p swissarmyhammer-expect = 21 passed; cargo test --doc -p swissarmyhammer-expect = 1 passed; cargo fmt applied; cargo clippy -p swissarmyhammer-expect --all-targets -- -D warnings clean.'
  timestamp: 2026-06-26T17:23:38.464500+00:00
depends_on:
- 01KW25YZ4MKNR09RXYR1B4S05T
position_column: doing
position_ordinal: '8280'
project: expect
title: Parse *.expect.md files (frontmatter + intent body + criteria)
---
## What
Parse a `*.expect.md` expectation file into a typed `Expectation`. Closest template: `crates/swissarmyhammer-validators/src/validators/types.rs` (RuleSet/Rule frontmatter+body model) and `loader.rs`.

- New `crates/swissarmyhammer-expect/src/spec.rs`:
  - `Expectation { path: String /* repo-relative, no ext = identity */, frontmatter: Frontmatter, intent: String /* whole body */, criteria: Vec<Criterion>, given: Vec<String>, when: Vec<String>, notes: Option<String> }`.
  - `Frontmatter` — the closed enumeration from `ideas/expect.md` §"Frontmatter Reference": `description` (required), `surface` (required, the `Surface` enum), `model` (Option), `reliability` (default `pass^1`), `repeat` (Option), `tiers` (default all three), `similarity_threshold` (Option), `timeout` (default 60s), `tags` (default []), `setup` (Option string|list), `isolation` (default `shared`). **Reject unknown keys** (serde `deny_unknown_fields`) so a typo fails loudly.
  - `Criterion { text: String, checked: bool }` parsed from the `## Then` GFM checklist (`- [ ]` items).
- Parse the markdown: split YAML frontmatter, extract `## Given` / `## When` / `## Then` / `## Notes` sections (G/W/T optional; ≥1 `Then` criterion is the one hard content requirement, but *parsing* tolerates 0 — `doctor` is what enforces ≥1). Reuse the project's existing frontmatter/markdown split helper if one exists in `swissarmyhammer-templating` or validators; otherwise a small `serde_yaml` + section splitter.
- Identity: derive `path` (repo-relative, `.expect.md` stripped) from an input file path + repo root.

## Acceptance Criteria
- [ ] The worked example in `ideas/expect.md` (the coupon spec, lines ~110-144) parses into an `Expectation` with description, `surface: cli`, `reliability: pass^3`, 2 Given, 2 When, 4 Then criteria, and a Notes block.
- [ ] An unknown frontmatter key (`surfce:`) produces a parse error naming the bad key (not silently ignored).
- [ ] Defaults applied when omitted: `reliability=pass^1`, `isolation=shared`, `timeout=60s`, `tiers=[deterministic,tolerance,judgment]`, `tags=[]`.
- [ ] A spec with G/W/T omitted but ≥1 criterion still parses.

## Tests
- [ ] `crates/swissarmyhammer-expect/src/spec.rs` unit tests covering: the coupon example fixture, unknown-key rejection, default application, and the no-GWT case. Put fixtures inline or under `crates/swissarmyhammer-expect/tests/fixtures/`.
- [ ] `cargo nextest run -p swissarmyhammer-expect spec` passes.

## Workflow
- Use `/tdd` — write the failing parse tests (coupon fixture + unknown-key) first.

## Review Findings (2026-06-26 12:00)

### Warnings
- [x] `crates/swissarmyhammer-expect/src/spec.rs:79` — ReliabilityPolicy::required is public but has a documented invariant (≥ 1) that is validated only during deserialization, not at the type level. Direct construction can violate it: `ReliabilityPolicy { required: 0 }` bypasses all validation. Make `required` private and expose it via a public getter: `pub fn required(&self) -> u32 { self.required }`. The custom Serialize/Deserialize implementations will work unchanged since serde respects private fields.
- [x] `crates/swissarmyhammer-expect/src/spec.rs:260` — Function nesting depth reaches 4 levels, exceeding the 3-level threshold. The nested `if let Some(item) = parse_bullet(line)` expressions inside the `match current` arms create code paths with 4-level nesting depth. Restructure to reduce nesting: move the bullet parsing check outside the match statement. Check `if let Some(item) = parse_bullet(line)` before routing: `if let Some(item) = parse_bullet(line) { match current { Section::Given => given.push(item), Section::When => when.push(item), _ => {} } }` to constrain max nesting to 3 levels.
- [x] `crates/swissarmyhammer-expect/src/spec.rs:388` — Hardcoded literal `3_600_000` (milliseconds per hour) is duplicated within the duration_str module. Appears at lines 388, 389 in serialize() and line 426 in parse_duration(). Define `const MILLIS_PER_HOUR: u128 = 3_600_000;` at module level and reuse it.
- [x] `crates/swissarmyhammer-expect/src/spec.rs:390` — Hardcoded literal `60_000` (milliseconds per minute) is duplicated within the duration_str module. Appears at lines 390, 391 in serialize() and line 424 in parse_duration(). Define `const MILLIS_PER_MINUTE: u128 = 60_000;` at module level and reuse it.
- [x] `crates/swissarmyhammer-expect/src/spec.rs:392` — Hardcoded literal `1_000` (milliseconds per second) is duplicated within the duration_str module. Appears at lines 392, 393 in serialize() and line 422 in parse_duration(). Define `const MILLIS_PER_SECOND: u128 = 1_000;` at module level and reuse it.

### Nits
- [x] `crates/swissarmyhammer-expect/src/spec.rs:349` — Hardcoded literal `3` represents the GFM checkbox marker length (`[ ]` or `[x]`). Should be a named constant for clarity and to prevent duplication. Define `const CHECKBOX_MARKER_LEN: usize = 3;` and use it in both places.

## Review Findings (2026-06-26 12:18)

### Nits
- [x] `crates/swissarmyhammer-expect/src/spec.rs:501` — The literal `1` for millisecond conversion breaks the pattern established by named constants (`MILLIS_PER_SECOND`, `MILLIS_PER_MINUTE`, `MILLIS_PER_HOUR`). Inconsistent use of magic numbers vs. named constants for unit conversion factors. Define `const MILLIS_PER_MILLIS: u128 = 1;` at line 449 and use it on line 501: `(value, MILLIS_PER_MILLIS)`.