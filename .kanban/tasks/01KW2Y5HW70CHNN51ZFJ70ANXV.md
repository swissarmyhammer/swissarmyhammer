---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3v45xa26sb6f6smncjxbq2
  text: |-
    Picked up. Consumer map (validator-manifest `Severity` enum only; the enum lives at validators/types.rs, validator-local — confirmed NO external crate imports `swissarmyhammer_validators::Severity`).

    REMOVE (tied to the enum):
    - validators/types.rs: `Severity` enum + Display; ValidatorFrontmatter.severity + Validator::severity(); ExecutedValidator.severity + is_blocking; RuleSetManifest.severity; Rule.severity + effective_severity; RuleFrontmatter.severity; RuleResult.severity + is_blocking; ExecutedRuleSet::has_blocking_failure/blocking_failures; + module tests.
    - validators/mod.rs re-export + doc line; lib.rs re-export.
    - validators/parser.rs:411 (Rule build) + parser tests asserting severity + `severity:` test YAML.
    - validators/loader.rs `severity:` test YAML fixtures.
    - builtin/mod.rs test severity assertion.
    - review/scope.rs ValidatorWork.severity + MatchedValidator.severity + assemble/from_ruleset + import + test helper.
    - review/test_support.rs loader_with/ruleset signatures drop severity arg.
    - review/drive.rs + review/fleet.rs + review/synthesize.rs: imports, call sites, inline struct fields.
    - tools review/validators.rs: ValidatorSummary/RuleDetail/ValidatorFrontmatterView `severity: String` fields + population.
    - tools review/tests.rs `severity:` test frontmatter fixtures.
    - builtin/validators/**/*.md frontmatter `severity:` lines (64 files).

    KEEP (different concept — the review FINDING wire contract, already de-severitied by sibling card z3ax4xt; parent prompt says don't touch it): review/types.rs finding-JSON tests that assert findings carry no `severity` key, and the synthesize/fleet guards asserting the rendered output/prompt has no severity. These legitimately use the word "severity" to prove its absence. Also untouched: unrelated ErrorSeverity (common/config/web), LSP DiagnosticSeverity, ToolValidationSeverity.
  timestamp: 2026-06-27T06:08:59.818422+00:00
- actor: claude-code
  id: 01kw40nhwcp37t5pdakvavwdwb
  text: |-
    DONE — manifest `Severity` enum fully removed. Green and ready for review (left in doing).

    FILES CHANGED (code):
    - crates/swissarmyhammer-validators/src/validators/types.rs — deleted `Severity` enum + Display; removed severity from ValidatorFrontmatter, Validator::severity(), RuleSetManifest, Rule + effective_severity(), RuleFrontmatter, RuleResult + is_blocking(), ExecutedValidator + is_blocking(), ExecutedRuleSet::has_blocking_failure()/blocking_failures(); pruned/retargeted ~12 module tests.
    - crates/swissarmyhammer-validators/src/validators/parser.rs — parser no longer reads `severity:`; dropped severity asserts + YAML fixture lines.
    - crates/swissarmyhammer-validators/src/validators/loader.rs — dropped `severity:` test YAML fixtures.
    - crates/swissarmyhammer-validators/src/validators/mod.rs, lib.rs — dropped `Severity` re-export + doc example.
    - crates/swissarmyhammer-validators/src/builtin/mod.rs — removed severity assert in safety-validators test (+ doc).
    - crates/swissarmyhammer-validators/src/review/scope.rs — removed ValidatorWork.severity + MatchedValidator.severity + population; dropped Severity arg from all loader_with/ruleset call sites.
    - crates/swissarmyhammer-validators/src/review/test_support.rs — loader_with()/ruleset() drop the severity param.
    - crates/swissarmyhammer-validators/src/review/drive.rs, fleet.rs, synthesize.rs — dropped Severity imports, inline struct severity fields, and call-site args.
    - crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs — removed `severity: String` from ValidatorSummary/RuleDetail/ValidatorFrontmatterView + their population (manifest.severity / effective_severity reads).
    - crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs — dropped `severity:` from test frontmatter fixtures.
    - builtin/validators/**/*.md — removed `severity:` frontmatter from 62 files.

    GREP VERIFICATION:
    - `rg -n '\bSeverity\b' crates/swissarmyhammer-validators` → EMPTY (type fully gone).
    - `rg -n '^severity:' builtin/validators` → EMPTY.
    - `rg 'swissarmyhammer_validators::...Severity'` across crates → EMPTY (no external importer; enum was validator-local as the card stated).
    - Residual lowercase `severity` in the crate is ONLY the review FINDING wire-contract (review/types.rs finding-JSON tests; synthesize.rs/test_support.rs/fleet.rs binary-pass/fail guards) — a different, already-completed concern (sibling card z3ax4xt), explicitly out of scope per the card. Unrelated ErrorSeverity / DiagnosticSeverity / ToolValidationSeverity untouched.

    TESTS (cargo nextest, all green):
    - swissarmyhammer-validators: 282 passed, 0 skipped.
    - swissarmyhammer-common: 601 passed, 0 skipped.
    - swissarmyhammer-tools: 1412 passed, 0 skipped.
    - rdeps(swissarmyhammer-validators): 4901 passed, 2 skipped, 1 leaky (incl. swissarmyhammer-agent review_real_model_e2e, the only other test_support consumer).
    - cargo fmt clean; cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings → clean.
    - Adversarial double-check: PASS.

    Staging note: only this card's files were edited; no git add performed (leaving for /review + commit). The .kanban/* + pre-existing Cargo/heb changes in the tree are not mine.
  timestamp: 2026-06-27T07:45:52.012837+00:00
- actor: claude-code
  id: 01kw41ak04hvnkrr6349dthw2w
  text: 'Addressed both review findings (orchestrator handled directly — two doc-comment additions, too small for a full implementer pass). Note: both `loader_with` and `ruleset` already HAD doc comments, so the finding was a weak/false-positive from the running (pre-rebuild) engine; rather than dismiss, I enriched both docs to explicitly state they are shared review-test fixtures re-exported for downstream test crates, and that `ruleset`''s rule body is a fixed placeholder. `cargo check -p swissarmyhammer-validators --tests` clean. Both checklist items flipped to [x].'
  timestamp: 2026-06-27T07:57:21.284091+00:00
depends_on:
- 01KW2XZS1ZK47Q888HPZ3AX4XT
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe880
project: local-review
title: 'Review: remove legacy validator Severity (Error/Warn) — no severity anywhere'
---
Finish the job: delete the second severity concept — the `Severity { Error, Warn }` enum on the validator manifest/rules. A rule is a rule; if it fires it must be fixed. There is no advisory/blocking gradation.

## Discovery first
Grep `Severity` / `severity` across `crates/swissarmyhammer-validators` and `crates/swissarmyhammer-common` (the enum likely lives in common — see `swissarmyhammer-common/src/reporter.rs:67`, `tests/severity_integration_test.rs`). Map every consumer before deleting.

## Known surface (validators/types.rs)
- `RuleSetManifest.severity` (`:186`), default doc (`:169`), `RuleSet::severity()` (`:304-306`).
- `Rule.severity: Option<Severity>` (`:599`), `effective_severity` (`:606-608`), `RuleFrontmatter.severity` (`:626-628`).
- `ValidationOutcome.severity` + `is_blocking_failure` (`:472-485`, `:717-730`) — gated on `severity == Error`. The AVP hook-validator path that consumed this is being retired (see project description: rules run only via on-demand review), so much of this is already dead; remove it rather than carry it.
- The `MatchedValidator.severity` field in `review/scope.rs` and every `loader_with(..., Severity::Warn)` / `ruleset(...)` test helper across the review tests.

## Frontmatter / docs
- Drop `severity:` from validator frontmatter parsing and from `builtin/validators/**/VALIDATOR.md` frontmatter. Any validator that relied on `severity: warn` to "soften" rules: that softening is gone by design.

## Verify
Whole `swissarmyhammer-validators` + `swissarmyhammer-common` test suites green; `rg -w -i severity` returns nothing in the validator/review surface (tests included).

Depends on the finding-severity removal (which deletes review's read of `validator.severity`) so this enum has no remaining review consumer when it's pulled.

## Review Findings (2026-06-27 02:47)

### Nits
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:21` — Public function `loader_with` lacks a doc comment. As a test fixture exported for use by downstream crates, it should document its purpose and parameters. Add a doc comment explaining the fixture's purpose: `/// Create a test validator loader with the given name, file glob pattern, and probe names.`.
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:26` — Public function `ruleset` lacks a doc comment. As a test fixture exported for use by downstream crates, it should document its purpose and parameters. Add a doc comment explaining the fixture's purpose: `/// Create a test RuleSet with the given name, file glob pattern, and probe declarations.`.

## Review Findings (2026-06-27 02:58)

### Nits
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:725` — Hardcoded sleep duration 60 seconds for test stall is unexplained; should be a named constant. Extract as `const STALL_DURATION_SECS: u64 = 60;` at module level.