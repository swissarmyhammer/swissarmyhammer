---
assignees:
- claude-code
depends_on:
- 01KW25YZ4MKNR09RXYR1B4S05T
position_column: todo
position_ordinal: a580
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