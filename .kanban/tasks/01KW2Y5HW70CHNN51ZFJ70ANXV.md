---
assignees:
- claude-code
depends_on:
- 01KW2XZS1ZK47Q888HPZ3AX4XT
position_column: todo
position_ordinal: a780
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