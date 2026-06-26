---
assignees:
- claude-code
depends_on:
- 01KW25YZ4MKNR09RXYR1B4S05T
position_column: todo
position_ordinal: a480
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