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
- actor: claude-code
  id: 01kw2gc34es1aqqench07vv3f2
  text: |-
    Resolved review findings. NIT (config.rs hardcoded 0.80) FIXED: extracted module-level `const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.80;` and used it in EmbedderConfig's Default impl.

    Both WARNINGS (config.rs public fields; spec.rs Expectation/Frontmatter/Criterion public fields) consciously WAIVED, no code change. Rationale: public fields are the idiomatic, correct design for plain serde config/data DTOs in this workspace — adding getters/encapsulation would fight the DTO pattern with no real benefit. Additionally, the spec.rs structs were already reviewed and APPROVED to `done` under ^qj11rxw, so re-encapsulating them now is out of scope and re-litigates approved code. Checkboxes flipped to [x] with explicit "(accepted: ...)" notes.
  timestamp: 2026-06-26T17:41:50.350176+00:00
- actor: claude-code
  id: 01kw2gn1mpxx8s63n0tep1sdsz
  text: |-
    Cleared the two open 12:42 review findings in config.rs:
    1. WARNING — default_config_matches_the_documented_defaults test now asserts against DEFAULT_SIMILARITY_THRESHOLD instead of the literal 0.80.
    2. NIT — extracted `const DEFAULT_ESCALATE_BELOW_CONFIDENCE: f32 = 0.6;`, used it in ApprovalConfig::default(), and pointed both 0.6 test assertions (default + partial-merge tests) at the const.
    Verified: cargo nextest run -p swissarmyhammer-expect = 31 passed; cargo test --doc = 1 passed; cargo fmt applied; cargo clippy --all-targets -D warnings clean. Both checkboxes flipped; task left in doing for review.
  timestamp: 2026-06-26T17:46:43.734239+00:00
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

## Review Findings (2026-06-26 12:33)

### Warnings
- [x] `crates/swissarmyhammer-expect/src/config.rs:27` — Public struct fields are a permanent API commitment. Future changes like validation, computed fields, or type modifications would require breaking changes. All config structs have public fields. Make struct fields private and provide accessor methods or builder patterns. While serde typically requires public fields, you can use private fields with appropriate serde attributes (`#[serde(skip)]`, custom accessors, or builder patterns) to maintain forward compatibility. (accepted: idiomatic public serde DTO; spec.rs already approved in ^qj11rxw)
- [x] `crates/swissarmyhammer-expect/src/spec.rs:38` — Public struct fields are a permanent API commitment. The Expectation, Frontmatter, and Criterion structs expose fields publicly, preventing future validation or computed fields without breaking changes. Make struct fields private and provide accessor methods. Though serde works well with public fields, using private fields with serde attributes preserves the ability to add validation or computed behavior later without breaking the public API. (accepted: idiomatic public serde DTO; spec.rs already approved in ^qj11rxw)

### Nits
- [x] `crates/swissarmyhammer-expect/src/config.rs:81` — Hardcoded similarity threshold `0.80` in Default impl should be a named constant — it configures the Tier-2 cosine cutoff behavior and is documented as the default. Define `const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.80;` at module level and use it in the Default impl.

## Review Findings (2026-06-26 12:42)

### Warnings
- [x] `crates/swissarmyhammer-expect/src/config.rs:215` — Test hardcodes `0.80` where it should reference the `DEFAULT_SIMILARITY_THRESHOLD` constant (defined at line 26). After extracting a named constant, all occurrences of the literal must be replaced so changes propagate everywhere and remain synchronized. Replace `0.80` with `DEFAULT_SIMILARITY_THRESHOLD` on line 215: `assert_eq!(config.embedder.similarity_threshold, DEFAULT_SIMILARITY_THRESHOLD);`.

### Nits
- [x] `crates/swissarmyhammer-expect/src/config.rs:111` — The default value 0.6 for escalate_below_confidence should be a named constant (e.g., DEFAULT_ESCALATE_BELOW_CONFIDENCE), following the pattern established by DEFAULT_SIMILARITY_THRESHOLD. Define 'const DEFAULT_ESCALATE_BELOW_CONFIDENCE: f32 = 0.6;' at the top of the file with other constants (after line 22), then use it in ApprovalConfig::default() on line 111.