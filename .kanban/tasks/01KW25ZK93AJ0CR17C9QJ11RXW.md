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