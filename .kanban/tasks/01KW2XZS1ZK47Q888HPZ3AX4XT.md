---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw36dq0r4nw55tdf6q0k9twt
  text: |-
    Implemented: review is now binary pass/fail — finding-severity tier removed.

    Changes (TDD: added RED test `finding_serializes_without_a_severity_field` in types.rs, watched it fail, then made the change):
    - types.rs: deleted enum `Severity {Blocker,Warning,Nit}` + Display impl + `Finding.severity`. Finding = {file, line, validator, rule?, claim, evidence, suggestion?}. Added serialize/parse-without-severity tests.
    - synthesize.rs: `ReviewCounts` → {findings, confirmed, refuted, tasks_attempted, tasks_failed}. Removed SECTIONS grouping; renders ONE flat checklist ordered by file:line (no ### Blockers/Warnings/Nits). `findings` = deduped confirmed count. Updated tests + flat snapshot; added `renders_one_flat_findings_section_with_no_severity_grouping`.
    - review_op.rs: ReviewCountsView mirrors counts change (findings instead of blockers/warnings/nits).
    - verify.rs: dropped `- Severity:` line from verify prompt; updated test fixtures.
    - fleet.rs: deleted `## Default severity` prompt section + `severity_default()`; output contract no longer lists `severity`; moved legacy `validators::Severity` import into the test module. Updated test fixtures.
    - mod.rs: stopped re-exporting the deleted `Severity`.
    - test_support.rs / review/tests.rs / review_e2e.rs / review_real_model_e2e.rs: updated findings_json helpers + section/count assertions to flat model.
    - builtin/skills/review/SKILL.md: documented binary pass/fail, flat checklist, `{findings,confirmed,refuted}`.

    SCOPE BOUNDARY respected: legacy `validators::Severity {Error,Warn,Info}` untouched (still used in ValidatorWork/RuleSet fixtures and the manifest) — that is the separate card 01KW2Y5HW70CHNN51ZFJ70ANXV.

    Note: review_fixture.rs (integration) still emits a stray `severity` field in its simulated agent JSON — left intentionally; serde ignores unknown fields, giving robustness coverage that a real agent still emitting severity parses fine.

    Test results:
    - cargo nextest run -p swissarmyhammer-validators: 314 passed, 0 failed.
    - cargo nextest run -p swissarmyhammer-tools: 1414 passed, 2 failed (kanban::tests::test_add_column, test_delete_column) — both PASS in isolation; pre-existing environmental cwd-contention flakes, unrelated to review.
    - cargo check -p swissarmyhammer-agent --tests: clean (rdep e2e compiles).
    - cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings: clean. cargo fmt: clean. No clippy warnings in any review file.
    - Adversarial double-check: PASS.

    BLOCKER (pre-existing, not from this change): workspace-wide `cargo clippy --workspace --all-targets -- -D warnings` fails on a deprecated rmcp `notify_logging_message` call at crates/swissarmyhammer-tools/src/mcp/diagnostics_resource.rs:141 (a file not in this change). Per-crate clippy on the changed crates is clean.

    Left in `doing` for /review.
  timestamp: 2026-06-27T00:07:12.152205+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe480
project: local-review
title: 'Review: remove finding severity — binary pass/fail model'
---
Review becomes pass/fail like a test suite: a confirmed finding is a failure, full stop. No graded severity, no "nit" you may skip. "All pass or you failed."

## Remove the finding-severity tier
- Delete `Severity { Blocker, Warning, Nit }` and `Finding.severity` in `crates/swissarmyhammer-validators/src/review/types.rs:16-94`. The fan-out JSON contract drops the `severity` field; `Finding` = `{ file, line, validator, rule?, claim, evidence, suggestion? }`.
- `crates/swissarmyhammer-validators/src/review/synthesize.rs`: delete the `SECTIONS` Blockers/Warnings/Nits grouping (`:152-197`). Render ONE flat findings checklist ordered by `file:line`.
- `ReviewCounts` (synthesize.rs:74-100) → `{ findings, confirmed, refuted }`. Drop `blockers/warnings/nits`. Mirror in `ReviewCountsView` (`crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:355-389`).
- `crates/swissarmyhammer-validators/src/review/verify.rs:498` — drop the `- Severity:` line from the verify prompt.
- `crates/swissarmyhammer-validators/src/review/fleet.rs:815` — delete the `## Default severity` prompt section and `severity_default()`.

## Verdict stays binary
Any confirmed finding ⇒ task stays red (column movement is the verdict — unchanged). Zero confirmed ⇒ clean.

## Tests
Update every `Severity::{Blocker,Warning,Nit}` reference in the review unit tests; add a test asserting `Finding` parses/serializes with NO severity field, and that synthesis renders a single findings section.

NOTE: this is the FINDING severity only. The legacy `validators::Severity {Error,Warn}` on the validator manifest is a separate card.