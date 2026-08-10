---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzpfhm5j5mv1r8b4y0hyq6y8
  text: |-
    Research: the review engine takes its file set from `scope_review` (crates/swissarmyhammer-validators/src/review/scope.rs). Both consumers read that one work-list — the fan-out pairs and `plan_tool_rules` — so a file that leaves the work-list can reach neither. `RuleSet.base_path` already names each loaded set's directory in all three layers, so the store answers "what is a fixture" with no path pattern.

    Related discovery: commit f8c47217a (^f0wna3d) added a `.tmpl` suffix to the SHIPPED fixtures, which stops a builtin fixture matching a rule by extension. That does not answer this card: a user or project set is free to store `fixtures/<rule>.fail.rs` under a real extension (`find_fixture` accepts any extension), and such a file is ordinary source to the engine. The store-derived exclusion covers both.
  timestamp: 2026-08-10T18:39:17.170588+00:00
- actor: claude-code
  id: 01kzpfj1cn950qfmy61k3eyacd
  text: |-
    Design landed:
    - `FIXTURES_DIR_NAME` and `RuleSet::fixtures_dir()` moved to `validators/types.rs`, beside the set they describe. doctor, tool_health, the shipped tests and test_support now read the one constant.
    - `ValidatorLoader::fixture_dirs()` names every loaded set's `fixtures/` directory across builtin, user and project.
    - New `review/scope/fixtures.rs`: `ExcludedFile { path, reason }` plus `split_validator_fixtures`, called from `scope_review` right after scope resolution. A path is compared per component against each fixture root; both sides resolve through `canonicalize` when they exist (a temporary root reaches a process as `/var/...` and resolves to `/private/var/...`) and lexically when they do not.
    - `WorkList` carries `excluded()`; `run_review` hands it to `synthesize`, which renders one note per file with its reason and folds the paths into `counts.skipped_files`. It is NOT a finding and NOT counted by `counts.skipped` — a fixture is deliberate data, not a coverage failure.

    What did not work: the first e2e shape committed the validator set IN the reviewed change, which put the set's `.md` files in scope and moved the batch boundaries. The set is standing configuration now, so it lands in the baseline commit and the change is the fixture plus the planted source.

    Also learned: `CLAIM_DUP` fails in three `review_e2e` tests before this change (^bh5ncd0), so a new test must not assert it.
  timestamp: 2026-08-10T18:39:30.709160+00:00
- actor: claude-code
  id: 01kzpfjhhzwa4e2exezhwfbnxn
  text: |-
    ### implement — changed
    - evidence: 13 files. New `crates/swissarmyhammer-validators/src/review/scope/fixtures.rs`; edited `validators/{types,loader}.rs`, `doctor.rs`, `lib.rs`, `review/{mod,test_support,tool_health}.rs`, `review/scope.rs`, `review/scope/{batch,resolve,tests}.rs`, `review/synthesize.rs`, `review/tool_rules/tests/shipped.rs`, `swissarmyhammer-tools/src/mcp/tools/review/review_op/{response,tests}.rs`, `swissarmyhammer-tools/tests/integration/{review_e2e,review_fixture}.rs`.
    - acceptance 1: `review::scope::fixtures::tests::a_changed_builtin_fixture_leaves_the_scope_and_source_stays` runs the REAL builtin loader against the REAL repository root over `builtin/validators/code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl` plus a source file: the fixture leaves the scope with reason "validator fixture", the source stays. `review_e2e_sha_excludes_a_validator_fixture_and_still_reviews_the_source` drives `review sha` through the registered production tool and asserts `counts.skipped_files == [the fixture]`, `counts.skipped == 0`, and zero checklist findings naming it. RED proved for both by short-circuiting the split (fixture stayed in the scope / `skipped_files` came back empty).
    - acceptance 2 (proved, not asserted): a throwaway run copied the shipped `code-hygiene` set to a temporary directory and ran `check_review_engine_with` twice. Healthy: `missing-docs-rust usable=true` — doctor ran the shipped fixture pair. Then the rule's `run` script was broken: `usable=false fallback=true detail="fixtures failed: the fail fixture missing-docs-rust.fail.rs.tmpl produced no findings; at least one is required"`. The gate moved to doctor; it did not vanish. The probe was removed after the run; `doctor::` tests: 21 passed.
    - acceptance 3: `review_e2e_sha_excludes_a_validator_fixture_and_still_reviews_the_source` — real temp git repo, real on-disk index, registered production `review` tool, real project validator set at `<repo>/.validators/`. HOME is isolated by `IsolatedTestEnvironment`, so no test reads `~/.validators`.
    - commands: `cargo nextest run --workspace --no-fail-fast` → 14033 run, 14029 passed, 4 failed, 0 skipped. The 4 are the known pre-existing failures (^bh5ncd0): three `review_e2e` tests on `report_has_claim(markdown, CLAIM_DUP)` ("item 1 duplication", "duplication via sha", "a confirmed finding must land on the task") and the `review_progress_stdio` timeout. The failure text is the same as before this change. `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` zero warnings.
    - next: ready for /review.
  timestamp: 2026-08-10T18:39:47.263207+00:00
position_column: doing
position_ordinal: '8480'
title: 'review scope: exclude validator-set fixture files from review pairs and tool runs'
---
The review engine reviews validator fixture files as ordinary changed source, so every missing-docs tool rule fires on the fail fixture built to make it fire. This blocked ^f0wna3d: six eslint findings asked to document `missing-docs-typescript.fail.ts`, and documenting it breaks the fixture contract in `builtin/validators/README.md` (the fail fixture must hold undocumented items). Every future fixture edit re-raises the same findings.

Fix — derive the exclusion from the validator store, not from a user glob:
- The loader knows every validator set root across all three layers (builtin, user `~/.validators/`, project `./.validators/`). A changed file under any set's `fixtures/` directory leaves the review work-list: no LLM (validator, file) pair, and never an argument to a tool rule's `run` script.
- Report each excluded file in `skipped_files` with the reason "validator fixture", and log it. No silent truncation.
- This does not conflict with the no-path-based-test-exclusion rule. That rule forbids user path globs for TEST code, because tests live inline in source. A fixture directory is not test code: the README contract defines its files as intentionally failing data, and doctor is their gate — doctor runs every tool rule against them on each health check. The exclusion comes from the store structure, a single source of truth.

Acceptance:
- A `review sha` over a commit that touches `builtin/validators/code-hygiene/fixtures/*.fail.*` reports zero findings about fixture files and lists them in `skipped_files`.
- Doctor still runs the fixtures and still fails a broken tool rule — the gate moves, it does not vanish.
- A production-path test covers the scenario: a changed fail fixture plus a changed source file; the source file is reviewed, the fixture is skipped with the reason.

#tool-validators