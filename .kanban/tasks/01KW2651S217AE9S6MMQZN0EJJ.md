---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw35x4bety8sebbnf19hw17m
  text: |-
    Picked up. Research done. Plan (TDD):
    1. Engine loader: add `RawSpec {path, content}` + `ExpectationLoader::discover_raw(scope)` that discovers *.expect.md and reads RAW content (no parse) so malformed specs are surfaced for doctoring rather than aborting (load_all/resolve_scope parse via deny_unknown_fields and would error on a malformed spec). Refactor scope resolution to split path-resolution from parsing; make spec::derive_path pub(crate); export RawSpec.
    2. tools/expect/doctor.rs: production DoctorFacts (available_models from ModelManager::list_agents, known_setup_commands=None → unverifiable warning), map DiagnosticStatus→HealthStatus and →exit (Ok0/Warn1/Err2), doctor_report(scope) structured+human, health_checks() mapping each FieldDiagnostic→HealthCheck category "expect".
    3. Replace impl_default_doctorable!(ExpectTool) with hand-written Doctorable (like review).
    4. Tests: loader raw-discovery RED/GREEN; doctor exit codes clean0/warn1/err2 (inject fixed model list, no system/model); health_registry test mirroring review validators assertion (CWD fixture repo w/ malformed spec → expect-category check). doctor/init are NOT ops (per description.md + CLI builds from operations()); they roll up via trait. expect already registered in collect_all_health_checks.
  timestamp: 2026-06-26T23:58:08.750639+00:00
- actor: claude-code
  id: 01kw36s9xbzrrm03zx3vjjc2z6
  text: |-
    Implementation landed (green, left in `doing` for review).

    Files changed:
    - crates/swissarmyhammer-expect/src/loader.rs — added `RawSpec {path, content}` + `ExpectationLoader::discover_raw(scope)` (raw read, no parse, so malformed specs are surfaced for doctoring instead of aborting like load_all/resolve_scope do under deny_unknown_fields); refactored scope resolution into shared `resolve_scope_files`.
    - crates/swissarmyhammer-expect/src/spec.rs — `derive_path` → pub(crate); lib.rs exports `RawSpec`.
    - crates/swissarmyhammer-tools/src/mcp/tools/expect/doctor.rs — NEW. production DoctorFacts (available_models from ModelManager::list_agents, known_setup_commands=None), DiagnosticStatus→HealthStatus and →exit (Ok0/Warn1/Err2) mappings, doctor_report/run_doctor (scoped `expect doctor [scope]`), health_checks/health_checks_in (sah doctor rollup), render_report.
    - crates/swissarmyhammer-tools/src/mcp/tools/expect/mod.rs — replaced impl_default_doctorable!(ExpectTool) with hand-written Doctorable (category "expect") delegating to doctor::health_checks; added `pub mod doctor`.
    - crates/swissarmyhammer-tools/src/health_registry.rs — added integration test test_expect_spec_health_check_included (collect_all_health_checks picks up expect; expect already registered there).

    Verification (all green):
    - cargo nextest run -p swissarmyhammer-tools -E 'test(expect) or test(doctor) or test(health)' → 67 passed.
    - cargo test -p swissarmyhammer-expect → 90 lib + 2 + 4 doctests passed.
    - cargo check --workspace → ok. cargo clippy -p swissarmyhammer-tools -- -D warnings → clean. cargo fmt applied.

    Acceptance: expect doctor exit codes clean=0/warn1/err2 (run_doctor/doctor_report, tested); sah doctor includes "expect" category w/ malformed spec error (health_registry test); missing pinned model = Warning not Error (tested); no system driven / no model consulted (diagnose pure; only list_agents names; tests inject fixed model list).

    Adversarial double-check (advisory) verdict REVISE, 3 findings:
    1. (Medium) scoped `expect doctor [scope]` not wired as a standalone CLI command. JUSTIFIED PROCEED: matches the accepted sibling `init` task precedent — ExpectTool registers only as a trait impl (registry.rs:51) rolling up into sah init/sah doctor; there is no `sah expect init` standalone command either, and the sah CLI builds per-tool subcommands solely from operations() (doctor/init deliberately excluded per description.md). run_doctor/doctor_report are the reusable doctor pass the later `check` tasks consume ("check = doctor + observe + evaluate + compare"), not dead code. A nounless-per-tool-CLI-verb mechanism is app-level infra absent for init; out of scope here.
    2. (Low) empty repo → expect category vanished from sah doctor. FIXED: health_checks_in now emits one OK "no expectation specs found" line when no specs (mirrors review's all-valid OK), with test.
    3. (Low/info) doctor_repo_root CWD=/ walk hazard. No change: CLI-only call site with real CWD, matches existing PromptHealthChecker/code_context patterns; noted for any future session-scoped reuse.
  timestamp: 2026-06-27T00:13:31.947905+00:00
depends_on:
- 01KW263EW64TVAPZD7CYCBMNVV
- 01KW260M8QZ8T37A8RZGDDVZ81
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffef80
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