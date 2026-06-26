---
assignees:
- claude-code
depends_on:
- 01KW263EW64TVAPZD7CYCBMNVV
- 01KW260M8QZ8T37A8RZGDDVZ81
position_column: todo
position_ordinal: af80
project: expect
title: expect doctor op + register diagnostics into sah doctor
---
## What
Surface the static diagnostics two ways: the scoped `expect doctor [scope]` trait verb, and as a provider inside the shared `sah doctor` framework. Per `ideas/expect.md` §"The tool is doctorable".

- Implement `Doctorable` for `ExpectTool` (replace `impl_default_doctorable!`), mirroring how `review` hand-implements `Doctorable` to surface validator lint (`crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs:348-402`). `run_health_checks()` loads all `*.expect.md` (via the loader) and maps each `FieldDiagnostic` to a `HealthCheck` (`crates/swissarmyhammer-common/src/health.rs`: ok/warning/error + `fix` hint), category `"expect"`.
- Ensure `collect_all_health_checks()` in `crates/swissarmyhammer-tools/src/health_registry.rs` picks it up (it iterates registered tools' `run_health_checks()`; confirm `expect` is registered there from the skeleton task).
- Wire `expect doctor [scope]` as a top-level trait verb (nounless, like kanban-cli's hand-written `doctor`) that runs `diagnose` over the resolved scope and renders the structured + human output. It rolls up under `sah doctor`.
- Map `FieldDiagnostic.status` → `HealthStatus`/`CheckStatus` (Error⇒exit 2, Warning⇒exit 1) so a malformed spec fails `expect doctor` appropriately.

## Acceptance Criteria
- [ ] `expect doctor` over a scope returns per-spec diagnostics and a non-zero exit on any Error spec; clean specs exit 0.
- [ ] `sah doctor` output includes an "expect" category with the spec diagnostics (a malformed spec shows up there).
- [ ] A spec with a missing pinned `model:` shows as a Warning (exit 1), not an Error.
- [ ] No system driven, no model consulted.

## Tests
- [ ] Test asserting `collect_all_health_checks()` includes expect-category checks when a malformed spec is present in a fixture repo (mirror the review validators assertion in `health_registry.rs` tests).
- [ ] Test `expect doctor` exit codes: clean⇒0, warning-only⇒1, error⇒2.
- [ ] `cargo nextest run -p swissarmyhammer-tools expect_doctor` passes.

## Workflow
- Use `/tdd`.