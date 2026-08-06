---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kza9zy1dtj0avvv83te6gmh2
  text: |-
    Research findings:

    - `ValidatorMatch` is in `crates/swissarmyhammer-validators/src/validators/types.rs` with `tools` and `files` keys. The shared helpers `matches_tools` / `matches_files` are the one match code path — `Validator::matches` and `RuleSet::matches` both call them. A new `matches_project_types` helper goes in the same set.
    - `MatchContext` carries the match inputs (`tool_name`, `file_path`, `changed_files`). It gets a `project_types` field. Prevailing pattern: a required key with no context value does not match (fail closed).
    - PROJECT_TYPE_SPECS lives in `crates/swissarmyhammer-project-detection/src/types.rs`. `detect_projects(root, None)` walks the tree; `spec_for(pt).key` gives the lowercase key ("rust", "python"). The key equals the serde name (guarded by test `spec_key_matches_serde_repr`).
    - The workspace under review = `repo_path` in `scope_review` (`crates/swissarmyhammer-validators/src/review/scope.rs`). Resolution happens once per run there and threads into `match_validators_and_files`.
    - `swissarmyhammer-validators` does not yet depend on `swissarmyhammer-project-detection`. The dep is safe: project-detection only depends on serde/thiserror — no cycle.
    - Other MatchContext constructors (`lib.rs::match_rules` hook surface, tools `engine_matched_names`) have no workspace root parameter. They pass no project types → a project_types-keyed validator does not match there (fail closed). Out of scope for this card.
    - `builtin/validators/README.md` already documents the exact AND/OR contract from the card. No README change expected.
  timestamp: 2026-08-06T01:11:21.389040+00:00
- actor: claude-code
  id: 01kzaakyqcyj0bwntc2cr49g6s
  text: |-
    Implementation landed (TDD, red observed before each green):

    - `crates/swissarmyhammer-validators/src/validators/types.rs` — `ValidatorMatch.project_types` (serde default), `MatchContext.project_types` + `with_project_types`, new `matches_project_types` helper, and a shared `matches_criteria` that both `Validator::matches` and `RuleSet::matches` now call (one match code path, no second matcher). Semantics: keys AND, values OR, absent key matches everything, required-but-unresolved context does not match (fail closed, same as tools/files). Comparison is case-insensitive against the `PROJECT_TYPE_SPECS` keys.
    - `crates/swissarmyhammer-validators/src/review/scope.rs` — `detected_project_type_keys(repo_path)` resolves the workspace's detected types once per review run via `detect_projects` + `spec_for(..).key`; threaded into `match_validators_and_files`. Detection failure warns and resolves to no types. `engine_matched_validator_names` (test-support wrapper, no workspace) passes no types.
    - `crates/swissarmyhammer-validators/Cargo.toml` — new dep `swissarmyhammer-project-detection` (no cycle; that crate only depends on serde/thiserror).
    - Test fixtures updated for the new field: `review/test_support.rs`, `review/fleet/tests.rs`.
    - `builtin/validators/README.md` unchanged — the shipped behavior matches the documented contract word for word.

    Tests (all watched RED first):
    - Regression: files-only match ignores context project types.
    - AND: `files` + `project_types` requires both.
    - OR + only-key: `project_types`-only matches all files in a matching workspace, none otherwise (including empty and unresolved context), case-insensitive.
    - RuleSet path covered separately (`test_ruleset_matches_project_types`).
    - Serde: manifest without the key parses unchanged; with the key parses into the field.
    - Wiring: `scope_review_resolves_workspace_project_types_for_matching` — real temp git repo with `Cargo.toml`; a rust-keyed validator pairs, a python-keyed one does not.

    Verification: `cargo nextest run -p swissarmyhammer-validators` 386/386 passed; `-p swissarmyhammer-tools -E 'test(validator) or test(review)'` 103/103 passed; `cargo clippy --all-targets -- -D warnings` clean on swissarmyhammer-validators (with --all-features), swissarmyhammer-tools, swissarmyhammer-agent; `cargo fmt` run.

    Note for follow-up cards: `lib.rs::match_rules` (hook surface) and the tools `list/dump validators` ops have no workspace root parameter, so they resolve no project types — a `project_types`-keyed validator does not match through them. Wiring a root into those surfaces is out of this card's scope.
  timestamp: 2026-08-06T01:22:17.452225+00:00
- actor: claude-code
  id: 01kzaam5691cx2x8drw2q4ddxr
  text: |-
    ### implement — changed
    - evidence: 6 files — crates/swissarmyhammer-validators/src/validators/types.rs, crates/swissarmyhammer-validators/src/review/scope.rs, crates/swissarmyhammer-validators/src/review/test_support.rs, crates/swissarmyhammer-validators/src/review/fleet/tests.rs, crates/swissarmyhammer-validators/Cargo.toml, Cargo.lock. cargo nextest run -p swissarmyhammer-validators: 386/386 passed; tools review/validator tests 103/103 passed; clippy -D warnings clean; fmt run.
    - next: test
  timestamp: 2026-08-06T01:22:24.073225+00:00
- actor: claude-code
  id: 01kzac5paf96saxspjvegvreqb
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (3d93fa129); engine reported 14 findings, all 14 target pre-existing test code (fleet/tests.rs, test_support.rs:274, types.rs:1394 — git blame confirms none added by this commit); the review skill's written exception drops findings that refactor existing tests; 0 findings remain
    - next: none — task moved to done
  timestamp: 2026-08-06T01:49:27.247209+00:00
- actor: claude-code
  id: 01kzac6g6r9tv9txdz4jmpze49
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 6 files (types.rs, review/scope.rs, test_support.rs, fleet/tests.rs, Cargo.toml, Cargo.lock)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3118 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 3d93fa129
    - review: clean — 0 findings (14 engine candidates dropped by the written existing-test exception); task moved to done
  timestamp: 2026-08-06T01:49:53.752671+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffaf80
title: Add project_types to ValidatorMatch with AND semantics
---
Add `project_types` as a new key on `ValidatorMatch` in `swissarmyhammer-validators/src/validators/types.rs`.

Semantics — same as the existing keys:
- The keys under `match` combine with an implicit AND. Every present key must match.
- An absent key matches everything (current ValidatorMatch behavior).
- The values inside one key combine with OR.
- So `files: ["**/*.py"]` + `project_types: [python]` = the file matches the pattern AND the workspace is a detected python project.

Work:
- Add the field to `ValidatorMatch`, serde default, so every existing manifest parses unchanged.
- Resolve detected project types from the PROJECT_TYPE_SPECS detection for the workspace under review.
- Evaluate the criterion in the one existing match code path. No second matcher.
- Update `builtin/validators/README.md` if the shipped behavior differs from the documented contract.

Acceptance:
- A match block with only `files` behaves exactly as today (regression test).
- A match block with `files` + `project_types` requires both (AND test).
- A match block with only `project_types` matches all files in a matching workspace, no files otherwise.

#tool-validators