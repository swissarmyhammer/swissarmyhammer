---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6by86ngjsv9tbepx3qk5xvw
  text: |-
    ### implement — stuck
    - Deleted 7 skill guidance tests in crates/swissarmyhammer-skills/tests/: finish_commit_guidance, findings_are_requirements_guidance, implement_rules_and_self_review_guidance, review_scope_repository_guidance, sah_help_guidance, skill_comment_guidance, task_double_check_guidance. Deleted tests/common/mod.rs (no remaining test uses any helper). Kept builtin_description_compliance.rs and no_committed_skill_deploy_artifacts.rs. No SKILL.md edited.
    - Other text-asserting tests found workspace-wide and deleted:
      - crates/mirdan/tests/findings_are_requirements_coverage.rs (asserts rendered skill and agent text).
      - validators crate, tool_rules/tests/shipped/: files scope_roster.rs, zero_argument.rs, temp_directory.rs, golangci_cache.rs (all scan shipped script text).
      - swift_judgment_rules.rs: 3 tests on prompt rule body text removed; the gate-behavior test kept.
      - disallowed_constructs_swift.rs: test names_only_rules_swiftlint_knows removed; script-text cross-checks removed from 2 behavior tests.
      - idioms_swift.rs: 7 tests removed (allowlist names, doctor utilities, doctor options, version floor, body tag table, body status table, plus swift_reflection module); text checks removed from 3 behavior tests.
      - shipped.rs: verify_superseded_swift_bullet now checks tool behavior only; dead helpers removed.
      - validators src/builtin/mod.rs: test_swift_casing_accepts_both_acronym_spellings and test_builtin_rulesets_carry_their_validator_md_body removed; body-text assert removed from the assertion-census test.
    - Kept (behavior, not text): mirdan deploy byte-equality tests, templating all_skills_render_test, skills tool variable-substitution tests, parser fixture tests, fixture-run shipped tests.
    - Verified: cargo nextest run -p swissarmyhammer-skills = 118 passed; validators builtin:: + shipped swift_judgment/disallowed/idioms/function_length_go = 69 passed; cargo clippy -p swissarmyhammer-skills -p swissarmyhammer-validators -p mirdan --all-targets = clean; cargo fmt --all --check = clean.
    - GAP (reason for stuck): crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs asserts on the text of builtin/_partials/project-types/swift.md and must go. I deleted it. A concurrent Claude session (nextest run for swift_guidelines_partial, cp of swift.new.md at 10:14) recreated it with new content, re-added `mod swift_guidelines_partial;` in shipped.rs, and added a swissarmyhammer-config dev-dependency to crates/swissarmyhammer-validators/Cargo.toml. I did not delete it again, to avoid destroying that session's live work. Needed: after that session ends, delete swift_guidelines_partial.rs, its `mod` line in shipped.rs, and the swissarmyhammer-config dev-dependency, then run cargo clippy for dead-code warnings.
    - Also note: nextest needs --override-version-check here (repo wants 0.9.145, installed 0.9.116).

    step: implement
    outcome: stuck
    evidence: see comment above; 19 files, 38 insertions, 4581 deletions; skills nextest 118 pass; validators subset 69 pass; clippy and fmt clean
    task: ^ga169e
  timestamp: 2026-11-01T16:14:31.088157+00:00
- actor: claude-code
  id: 01m6bzmkx2308ra7naabb6t8j7
  text: |-
    ### implement — changed
    - Deleted crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs (other session agreed; its iteration-3 edit went with it).
    - Removed `mod swift_guidelines_partial;` from shipped.rs. No helper was left unused (clippy -D warnings clean).
    - Removed the swissarmyhammer-config dev-dependency (and its comment) from crates/swissarmyhammer-validators/Cargo.toml. Nothing else in the crate used it. Cargo.lock lost one line.
    - Final workspace search for tests that read skill, validator, partial or source text: only mirdan deploy byte-equality tests (deployed file equals builtin source) and mirdan store symlink tests remain. They check deploy behavior, so they stay.
    - Ran: cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings (clean); cargo fmt --all --check (clean); cargo nextest run --override-version-check -p swissarmyhammer-validators -p swissarmyhammer-skills -p mirdan (1472 passed, 0 failed).

    step: implement
    outcome: changed
    evidence: deleted swift_guidelines_partial.rs; edited shipped.rs, validators Cargo.toml, Cargo.lock; clippy clean, fmt clean, nextest 1472 passed
    task: ^ga169e
  timestamp: 2026-11-01T16:38:46.434664+00:00
- actor: claude-code
  id: 01m6c0b445hcc0xsxcv3mjfk0c
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace --override-version-check: 13244 run, 13244 passed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings: clean. cargo fmt --all --check: one diff in crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs, fixed with rustfmt (format only); cargo fmt --all --check now exit 0. The missing_docs tests passed again after the format fix.
    - next: none
  timestamp: 2026-11-01T16:51:03.941349+00:00
- actor: claude-code
  id: 01m6c0c9rat0afqgmdxxbx3vq4
  text: |-
    ### commit — changed
    - evidence: one local commit, 24 code files plus the task files (not pushed). Subject: test: delete tests that assert on skill, validator or source text. Find the sha with git log --grep ga169e or the subject.
    - next: review
  timestamp: 2026-11-01T16:51:42.474874+00:00
position_column: doing
position_ordinal: '8680'
title: Delete tests that assert on skill text or source text
---
Goal: no test asserts on the text of a skill or on source code text. User rule: such tests must go.

Cause: card ^17103gm is stuck because `finish_skill_renders_commit_on_done_guidance` checks the words of `builtin/skills/finish/SKILL.md`. The user changed the skill on purpose and said the skill is right.

Delete these seven files in `crates/swissarmyhammer-skills/tests/`:
- `finish_commit_guidance.rs`
- `findings_are_requirements_guidance.rs`
- `implement_rules_and_self_review_guidance.rs`
- `review_scope_repository_guidance.rs`
- `sah_help_guidance.rs`
- `skill_comment_guidance.rs`
- `task_double_check_guidance.rs`

Then:
- Delete `tests/common/mod.rs` helpers that nothing uses any more (for example `rendered_builtin_instructions`). Keep helpers that other tests still use.
- Search the whole workspace for other tests that assert on skill text, validator text, or source file text (`include_str!` of `.rs` or `SKILL.md`, `read_to_string` of source). Delete each one you find. Report the list.
- Keep structural tests: `builtin_description_compliance.rs` (frontmatter limits through the API) and `no_committed_skill_deploy_artifacts.rs`. Keep tests that run validator rules against fixtures. They check behavior, not text.
- Do not edit any `SKILL.md` file.

Done when: `cargo nextest run -p swissarmyhammer-skills` passes, the workspace has no test that asserts on skill or source text, and clippy and fmt are clean. #remove-llama-cpp