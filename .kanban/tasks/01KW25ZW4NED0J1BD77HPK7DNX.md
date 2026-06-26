---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2fmp24bfsb5p0cxb00at3p
  text: 'Picked up. Research done: Surface enum + ReliabilityPolicy (pass^N, custom serde) live in spec.rs/types.rs and will be reused. find_git_repository_root_from is in swissarmyhammer-directory (low-level, no tool-layer dep). toml 1.1 + tempfile in workspace deps. Plan: new config.rs with [model]/[provision]/[embedder]/[reliability]/[approval]/[agent] sections, container-level #[serde(default, deny_unknown_fields)] so absent file/sections fall back to manual Default impls; reuse ReliabilityPolicy (adding a Default=pass^1 impl) for reliability.default. TDD.'
  timestamp: 2026-06-26T17:29:03.300568+00:00
- actor: claude-code
  id: 01kw2fvz2kjagaq14qb6zmyw9m
  text: 'Done. Added crates/swissarmyhammer-expect/src/config.rs (ExpectConfig + 6 section structs, OnMissing/Granularity enums, parse/load + find_expect_dir). Reused Surface and ReliabilityPolicy (added Default=pass^1 to it), find_git_repository_root_from from swissarmyhammer-directory. Added Toml variant to error.rs; declared/exported config module; added toml + swissarmyhammer-directory deps and tempfile dev-dep. TDD: 6 RED config tests watched fail (stub parse), then GREEN. Verification: cargo nextest -p swissarmyhammer-expect = 31/31 pass (10 config); cargo test --doc = 1 pass; cargo clippy --all-targets -D warnings clean; cargo fmt applied. Adversarial double-check: PASS. Left in doing for /review.'
  timestamp: 2026-06-26T17:33:01.907331+00:00
depends_on:
- 01KW25YZ4MKNR09RXYR1B4S05T
position_column: doing
position_ordinal: '8280'
project: expect
title: .expect/config.toml schema + parsing
---
## What
Type and parse `.expect/config.toml` — the repo-level config from `ideas/expect.md` §"Config Schema".

- New `crates/swissarmyhammer-expect/src/config.rs`:
  - `ExpectConfig` with sections: `[model]` (`default: String`, `panel: Vec<String>`, `on_missing: "fallback"|"error"`), `[provision]` (`granularity: "per-check"`), `[embedder]` (`model: String`, `similarity_threshold: f32` default 0.80), `[reliability]` (`default: String` = "pass^1", `nondeterministic_surfaces: Vec<Surface>`), `[approval]` (`ci_autoapprove: bool` default false, `escalate_below_confidence: f32` default 0.6), `[agent]` (`use_case: String` default "expectations").
  - Parse with `toml` + serde; every field has a sensible default (config is optional — absent file ⇒ all defaults).
  - `ExpectConfig::load(expect_dir: &Path) -> Result<ExpectConfig, ExpectError>` reading `.expect/config.toml`, returning defaults if missing.
- Locate `.expect/` at repo root from a working dir (reuse `find_git_repository_root_from` used by code_context/review).

## Acceptance Criteria
- [ ] A full `config.toml` matching the design example parses into `ExpectConfig` with all fields populated.
- [ ] A missing `.expect/config.toml` yields `ExpectConfig::default()` (no error).
- [ ] Defaults: `embedder.similarity_threshold=0.80`, `approval.ci_autoapprove=false`, `approval.escalate_below_confidence=0.6`, `model.on_missing="fallback"`, `agent.use_case="expectations"`.
- [ ] Unknown keys in a section are rejected (typo safety), matching the spec parser's stance.

## Tests
- [ ] `crates/swissarmyhammer-expect/src/config.rs` unit tests: full-file parse, missing-file defaults, partial-file (one section) merges with defaults, unknown-key rejection.
- [ ] `cargo nextest run -p swissarmyhammer-expect config` passes.

## Workflow
- Use `/tdd`.