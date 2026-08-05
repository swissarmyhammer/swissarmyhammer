---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz93r1v70sx948mnttzen6q5
  text: |-
    Research complete. Facts:

    - `install.rs` is 6387 lines, 245217 bytes. Production code ends at line 3146. Five test modules follow: `applier_tests` (3148-3410), `tests` (3412-5358), `profile_tests` (5360-5797), `profile_consistency_tests` (5799-6251), `edit_redirect_tests` (6253-6387).
    - `lib.rs` declares only `pub mod install;`. No item re-exports. External callers use `mirdan::install::<item>` paths (tools crates use `deny_tool`, `allow_tool`, `register_mcp_server`, `unregister_mcp_server`, `Profile`, `Selector`, `init_profile`). A directory module with `pub use <submodule>::*;` in `mod.rs` keeps all paths stable.
    - Split plan by concern: `package.rs` (run_install family, registry download, archive, run_install_mcp, install_package, parse_package_spec, extract_zip), `deploy.rs` (skill/agent/tool/plugin/validator deployment), `profile.rs` (Selector, Profile, init/deinit, statusline, edit redirect, MCP registration), `uninstall.rs` (run_uninstall family, run_uninstall_mcp, guess_installed_type), `applier.rs` (agent-config appliers + iteration helpers + scope_is_global), `mod.rs` (shared helpers: sanitize_dir_name, rooted, validators_dir, copy_dir_recursive). Each test module becomes its own file; `applier_tests` stays inline in `applier.rs` (263 lines).
    - Largest resulting file will be `tests.rs` at about 75 KB. All files stay far under the review prompt cap (the old file rendered 567352 bytes at 245 KB, so each new file renders well under the 476042-byte budget).
    - Decision on ^927239f: the split is pure code movement and does not itself fix the cleanup items (constants, dispatch dedup, nesting, deploy_skill/deploy_agent unification). Self-review of the split will read the moved code with all validators for the first time. Findings that surface get fixed in this task; what the review does not demand stays on ^927239f. I will record the outcome on both cards after self-review.
  timestamp: 2026-08-05T14:02:57.255632+00:00
- actor: claude-code
  id: 01kz94f8awn1fw839dft7mkhaf
  text: |-
    Split landed. `install.rs` (6387 lines, 245 KB) is now `install/` with 10 files: `mod.rs` (shared helpers + re-exports), `package.rs`, `deploy.rs`, `profile.rs`, `uninstall.rs`, `applier.rs` (with inline `applier_tests`), and four test-module files (`tests.rs`, `profile_tests.rs`, `profile_consistency_tests.rs`, `edit_redirect_tests.rs`). Largest file is `tests.rs` at 72 KB — far under the cap that made the old 245 KB file render 567 KB.

    Mechanics: pure line-range movement. `mod.rs` re-exports every submodule with `pub use <submodule>::*;`, so all `mirdan::install::<item>` paths stay stable (checked callers in swissarmyhammer-tools, -skills, -agents). Cross-module internals got `pub(crate)` (deploy_* wrappers, uninstall_agent_at, for_each_detected_agent, AgentAction, scope_is_global, APPLIER_COMPONENT, read_frontmatter, install_tool_from_*, apply_statusline_at, apply_edit_redirect_at, desired_* fragments, remove_empty_dirs_up_to, uninstall_tool/plugin/validator, Selector::select). Three `include_str!("../../../builtin/...")` sites got one more `../` for the deeper path.

    Verification so far:
    - `cargo check -p mirdan --all-targets`: clean, no warnings.
    - `cargo fmt --all -- --check`: clean.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - `cargo nextest run -p mirdan`: 418/418 passed (includes the init/deinit round-trip tests in profile_tests and profile_consistency_tests).
    - `cargo nextest run --workspace`: 13533/13533 passed, 0 skipped.

    Next: `review working` self-review, which also proves no resulting file exceeds the review prompt cap.
  timestamp: 2026-08-05T14:15:37.564521+00:00
- actor: claude-code
  id: 01kz9rvg81fzqhdcsnqpc3kz90
  text: |-
    Work complete. Summary for the next agent:

    **Split.** `install.rs` (6387 lines) is now `crates/mirdan/src/install/`: `mod.rs` (shared helpers, re-exports), `package.rs`, `deploy.rs`, `profile.rs`, `uninstall.rs`, `applier.rs` (+inline `applier_tests`), and four test files (`tests.rs`, `profile_tests.rs`, `profile_consistency_tests.rs`, `edit_redirect_tests.rs`). `pub use <submodule>::*;` keeps every `mirdan::install::<item>` path stable.

    **Acceptance verified.**
    - `review file` ran on all 10 files. Zero prompt-cap skips. The old "567352 rendered bytes over the 476042-byte budget" state is gone.
    - `cargo nextest run -p mirdan`: 419/419.
    - `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - The init/deinit round-trips pass (`profile_tests`, `profile_consistency_tests`).

    **Self-review loop.** Four `review working` passes plus per-file passes. Every finding fixed except:
    - 15 findings on `tests.rs` that asked to refactor pre-existing tests (manual CWD save/restore → CurrentDirGuard; a pre-existing e2e asymmetry). The review skill's written rule drops findings that ask to refactor tests that already existed.
    - 2 additive test-coverage gaps → new card ^83m03kr.
    - 29 pre-existing findings in files touched only incidentally (`store.rs`, `search.rs`, `mirdan-app/commands.rs`, `deeplink.rs`) → new card ^vt9bk5j, following the same precedent that created ^927239f.

    **Findings-driven behavior changes** (each demanded by a review finding; all tested):
    - `run_install` takes `InstallMode` (Auto/ForceGit) instead of the `git` bool. Callers in dispatch.rs, search.rs, package.rs, mirdan-app updated.
    - `uninstall_skill_at` returns `NotFound` when neither symlinks nor store entries exist (matches tool/plugin/validator).
    - `remove_matching_store_entries` propagates non-NotFound read errors.
    - `run_uninstall_mcp` uses the same HOME lockfile fallback as `run_uninstall`, and reports "not found in any agent" instead of "Uninstalled ... from 0 agent(s)".
    - `uninstall_tool` returns user-visible `DeployResult`s for each MCP config it changes; `run_uninstall` forwards them.
    - `deploy_tool` errors when no agent has MCP config (matches `deploy_plugin`).
    - `deinit_profile_validators` reports removal failures as Warnings instead of swallowing them.
    - `store::copy_dir_recursive` is the one shared copy implementation (the Windows fallback and the installer both use it).

    **Subsumed ^927239f** — decision recorded on that card.

    **Design question** from this card ("escalate a file no validator could read") → new card ^cdqt4v8. Not built here, per instruction.

    **Unrelated workspace failure.** `cargo nextest run --workspace`: 13533/13534 pass. The one failure is `swissarmyhammer-skills::implement_rules_and_self_review_guidance::implement_skill_prescribes_self_review_until_clean_before_handoff`. It fails because of an uncommitted parallel edit to `builtin/skills/implement/SKILL.md` (removes the self-review prescription the guard test requires). My diff touches neither that file nor that crate. I did not touch the parallel work.
  timestamp: 2026-08-05T20:11:50.401069+00:00
- actor: claude-code
  id: 01kz9rvkfcsd1xc99rsragabd1
  text: |-
    ### implement — changed
    - evidence: 18 files — crates/mirdan/src/install.rs deleted; crates/mirdan/src/install/{mod,package,deploy,profile,uninstall,applier,tests,profile_tests,profile_consistency_tests,edit_redirect_tests}.rs created; crates/mirdan/src/{store,search,dispatch}.rs, apps/mirdan-app/src/{commands,deeplink}.rs updated. review file x10: 0 skipped. nextest -p mirdan 419/419. workspace 13533/13534 (1 failure from parallel uncommitted SKILL.md edit, outside this change). fmt + clippy -D warnings clean.
    - next: formal /review (task stays in doing)
    task: ^t1y1c37
  timestamp: 2026-08-05T20:11:53.708083+00:00
- actor: claude-code
  id: 01kz9saq4e72yw1t51zq2t3kd7
  text: |-
    ### Guard-test update rode along with this iteration

    The user edited `builtin/skills/implement/SKILL.md` in the same tree. The new stance: the implementer preloads all validator rules with `dump validators` and keeps them in mind while it codes. The implementer does not run the `review working` self-review loop. The formal `/review` step owns the review. The `/double-check` step is removed.

    The guard test `crates/swissarmyhammer-skills/tests/implement_rules_and_self_review_guidance.rs` asserted the old stance. I updated it:

    - Kept `implement_skill_prescribes_the_rules_call_before_editing` unchanged.
    - Replaced `implement_skill_prescribes_self_review_until_clean_before_handoff` with `implement_skill_leaves_review_to_the_review_step`. The new test asserts: no `{"op": "review working"}`, no `/double-check`, the preloaded-validators phrase is present, the `/review` handoff phrase is present, and the `### Leave the task in `doing` for review` step stays.
    - Rewrote the module doc comment for the new stance.

    RED→GREEN: the old test failed against the edited skill (panic on the `review working` marker). The new test passes.

    A workspace grep found no other test that asserts the old stance. `task_double_check_guidance.rs` targets the `task` skill. The mirdan `findings_are_requirements_coverage.rs` names the `double-check` agent, not the implement stance.

    Verification:
    - `cargo nextest run -p swissarmyhammer-skills` — 128 passed, 0 skipped.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'` — 8332 passed, 0 skipped.
    - `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` — clean.

    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-skills/tests/implement_rules_and_self_review_guidance.rs; nextest -p swissarmyhammer-skills 128 passed; rdeps(swissarmyhammer-skills) 8332 passed; fmt + clippy clean
    - next: formal /review of the tree
  timestamp: 2026-08-05T20:20:08.974993+00:00
- actor: claude-code
  id: 01kz9y5dp444a74r9ff534bt8k
  text: |-
    ### review — findings
    - evidence: 35 findings, 35 confirmed, 6 refuted, 0 skipped — scope `review sha HEAD~1..HEAD`. Files: apps/mirdan-app/src/commands.rs, crates/mirdan/src/dispatch.rs, crates/mirdan/src/install/applier.rs, crates/mirdan/src/install/package.rs, crates/mirdan/src/install/uninstall.rs, crates/mirdan/src/search.rs, crates/mirdan/src/store.rs
    - acceptance check: the report lists zero files as "not reviewed — would exceed the agent's prompt cap" (`skipped: 0`). The split of install.rs satisfies the prompt-cap acceptance.
    - next: correct the 35 findings in the checklist, then run /review ^t1y1c37 again
  timestamp: 2026-08-05T21:44:38.340004+00:00
- actor: claude-code
  id: 01kz9y6qxqt32r16e4g8efb8fq
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — install.rs split into 10-file install/ module; subsumed ^927239f; guard-test update for the new no-self-review skill stance rode along
    - test: green — cargo nextest --workspace, 13534/13534; fmt + clippy clean
    - commit: 236d021f8
    - review: findings — 35 confirmed, 0 skipped; acceptance (no prompt-cap skip) PASS; findings in apps/mirdan-app/src/commands.rs, crates/mirdan/src/{dispatch,search,store}.rs, crates/mirdan/src/install/{applier,package,uninstall}.rs
  timestamp: 2026-08-05T21:45:21.591064+00:00
- actor: claude-code
  id: 01kz9yw176mc9z81qc7zgqedjh
  text: |-
    ### Findings pass — all 35 items fixed

    **apps/mirdan-app/src/commands.rs (11 items).** Added `MAX_REGISTRY_SEARCH_RESULTS` (20) and `MAX_STORE_SEARCH_DEPTH` (5). Added `format_action_result(spec, verb)` — install and uninstall use it. Added `log_and_stringify_error(spec, action, e)` — install, uninstall, and update use it. `find_store_path` now delegates to one `search_store(name, global)` helper for both scopes. `find_in_store` is flat: guard clauses replace the nested conditionals, and `skill_metadata_matches(path, name)` holds the dir-name / frontmatter-name checks.

    **crates/mirdan/src/dispatch.rs (3 items).** One generic `handle_result<T, E: Display>(result, on_ok)` prints `Error: <e>` and returns the exit code. All four sites use it: `handle_registry_result`, `handle_registry_result_msg`, `handle_deploy_result`, and the `Commands::Completion` arm.

    **crates/mirdan/src/install/applier.rs (2 items).** `AgentAction` now derives `Debug, Clone`. A `StrategyAction` enum bundles the verb, strategy dispatch, and message format for the four appliers; one `apply_strategy_action` drives it. The four public functions stay (public API stable) as one-expression wrappers. `for_each_agent_strategy` is gone — `apply_strategy_action` replaced it.

    **crates/mirdan/src/install/package.rs (2 items).** The tool check uses `eq_ignore_ascii_case("tool")`, and the non-tool rejection is `RegistryError::Validation`. TDD: two new tests in install/tests.rs went RED first — `test_install_tool_from_metadata_accepts_capitalized_tool_type` and `test_install_tool_from_metadata_non_tool_is_validation_error` — then GREEN after the fix. Both use `IsolatedTestEnvironment` + `CurrentDirGuard`.

    **crates/mirdan/src/install/uninstall.rs (1 item).** New `unregister_and_report_mcp(agents, name, global, action_verb, results)` holds the loop-and-report pattern; `run_uninstall_mcp` passes "Removed" and `uninstall_tool` passes "Unregistered".

    **crates/mirdan/src/search.rs (8 items).** Named constants for every flagged number: `POLL_IDLE_TIMEOUT_MS`, `DEBOUNCE_SHORT_QUERY_LEN`, `DEFAULT_TERMINAL_COLS/ROWS` (both use sites), `MIN/MAX_DISPLAY_RESULTS`, `LEFT_MARGIN_WIDTH` (doc reconciles the stale "4-char" comment: 4-char indent + 2-char right padding), `RESULT_SPACING_WIDTH`, `MIN_TRUNCATION_LEN`, `ELLIPSIS_WIDTH`. Whole-file cause removal also named the two debounce durations (`DEBOUNCE_SHORT_QUERY_MS` 250, `DEBOUNCE_LONG_QUERY_MS` 150).

    **crates/mirdan/src/store.rs (8 items).** One private `store_dir(global, dir_name)` backs all four public accessors (`skill/agent/tool/validators_store_dir` — public API unchanged). `store_entry_still_referenced` is flattened with a let-else and a new `is_symlink_to_target(path, canonical_target)` helper.

    **Verification.** `cargo nextest run -E 'rdeps(mirdan)'` — 3217 passed, 0 skipped. `cargo fmt --all` applied. `cargo clippy --workspace --all-targets -- -D warnings` clean. All 35 checklist items flipped to `- [x]` on the card.
  timestamp: 2026-08-05T21:56:59.238035+00:00
- actor: claude-code
  id: 01kz9yw2zdjk82dmjksx3abdkr
  text: |-
    ### implement — changed
    - evidence: 8 files — apps/mirdan-app/src/commands.rs, crates/mirdan/src/dispatch.rs, crates/mirdan/src/search.rs, crates/mirdan/src/store.rs, crates/mirdan/src/install/{applier,package,uninstall,tests}.rs; 35/35 findings checked; nextest rdeps(mirdan) 3217/3217; fmt + clippy -D warnings clean
    - next: formal /review (task stays in doing)
    task: ^t1y1c37
  timestamp: 2026-08-05T21:57:01.037052+00:00
- actor: claude-code
  id: 01kza04e8vg51586f46q6sh880
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD — 17 findings (17 confirmed, 12 refuted, 0 skipped); no file over the prompt cap. Files: apps/mirdan-app/src/commands.rs:32,35,36,37,38,39,45,48,49,50,51; crates/mirdan/src/install/package.rs:486; crates/mirdan/src/store.rs:43,202,290,812,984
    - next: correct the 17 open items in "## Review Findings (2026-08-05 17:02)", then run /review again
  timestamp: 2026-08-05T22:19:03.323535+00:00
- actor: claude-code
  id: 01kza05a6kskj6egf4p23k4tx8
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 8 files, all 35 prior findings fixed and checked
    - test: green — cargo nextest --workspace, 13536/13536; fmt + clippy clean
    - commit: 093972220
    - review: findings — 17 confirmed, 0 skipped; acceptance (no prompt-cap skip) PASS; open items in apps/mirdan-app/src/commands.rs (11 doc comments), crates/mirdan/src/install/package.rs:486 (case-sensitivity divergence), crates/mirdan/src/store.rs (5 items)
  timestamp: 2026-08-05T22:19:31.923006+00:00
- actor: claude-code
  id: 01kza0sj6rbfpzzks460v155p1
  text: |-
    ### Findings pass 2 — all 17 items of "Review Findings (2026-08-05 17:02)" corrected

    **apps/mirdan-app/src/commands.rs (11 items).** Each public field of `PackageInfo` and `SearchResult` now has a doc comment. All 13 fields have documentation.

    **crates/mirdan/src/install/package.rs:486 (1 item).** The two install paths now agree. Correction at the source: `parse_package_type` (crates/mirdan/src/package_type.rs) lowercases the input before the match. The archive path and the metadata-only path both accept capitalized registry types (e.g. "Tool"). TDD sequence: the new test `test_parse_package_type_is_case_insensitive` in install/tests.rs was RED (`None` != `Some(Tool)`), then GREEN after the change. The old assertion `parse_package_type("Tool") == None` asserted the removed behavior — it moved into the new test with the opposite expectation.

    **crates/mirdan/src/store.rs (5 items).**
    - store.rs:43 — The panic is now documented, per the finding's second remedy. `store_dir` and its four public wrappers (`skill/agent/tool/validators_store_dir`) each carry a `# Panics` section for the missing-home-directory case. The public API stays stable, per the task instruction.
    - store.rs:202 — `store_entry_still_referenced` takes `agent_skill_dirs: &[&Path]`. The one production caller (`remove_agent_store_entry_if_unreferenced` in install/uninstall.rs) and the two test call sites now pass path references.
    - store.rs:290 — `remove_store_entries` takes `names: &[&str]`. The five test call sites now pass string-literal arrays. No production caller exists outside store.rs.
    - store.rs:812 — `setup_skill_structure` is a thin wrapper: `setup_store_structure(root, ".skills", ".github/copilot/skills")`.
    - store.rs:984 — One helper `create_linked_store_entry(store_dir, link_dir, name, filename, content)` holds the create-write-symlink logic. `create_skill_symlink` and `create_store_entry_with_symlink` are one-expression wrappers around it.

    **Verification.** `cargo nextest run -E 'rdeps(mirdan)'` — 3218 passed, 0 skipped. `cargo fmt --all` applied. `cargo clippy --workspace --all-targets -- -D warnings` clean. All 17 checklist items are now `- [x]` on the card.
  timestamp: 2026-08-05T22:30:35.480076+00:00
- actor: claude-code
  id: 01kza0sqe562gv6zvzx4kd5dya
  text: |-
    ### implement — changed
    - evidence: 6 files — apps/mirdan-app/src/commands.rs, crates/mirdan/src/package_type.rs, crates/mirdan/src/store.rs, crates/mirdan/src/install/uninstall.rs, crates/mirdan/src/install/tests.rs (+ cargo fmt touch-ups); 17/17 findings checked; nextest rdeps(mirdan) 3218/3218, 0 skipped; fmt applied; clippy -D warnings clean
    - next: formal /review (task stays in doing)
    task: ^t1y1c37
  timestamp: 2026-08-05T22:30:40.837623+00:00
- actor: claude-code
  id: 01kza1j21yn4417h6k4v5st68y
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (86bc987c0) — 5 findings, 5 confirmed, 0 skipped. Files: crates/mirdan/src/install/uninstall.rs:276, uninstall.rs:354, uninstall.rs:636, crates/mirdan/src/package_type.rs:201, crates/mirdan/src/store.rs:223
    - acceptance: no file in the "not reviewed — prompt cap" list. The cap problem is gone.
    - next: correct the 5 open items in the section "Review Findings (2026-08-05 17:36)", then run the review again.
  timestamp: 2026-08-05T22:43:58.142156+00:00
- actor: claude-code
  id: 01kza1jresk4vrakermj3414nf
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 6 files, all 17 prior findings fixed and checked
    - test: green — cargo nextest --workspace, 13537/13537; fmt + clippy clean
    - commit: 86bc987c0
    - review: findings — 5 confirmed, 0 skipped; acceptance (no prompt-cap skip) PASS; open: uninstall.rs:276,354,636 (path traversal guards), package_type.rs:201 (named constant), store.rs:223 (AsRef<Path> signature)
  timestamp: 2026-08-05T22:44:21.081675+00:00
- actor: claude-code
  id: 01kza288qpw04dcsk4b1eeca7v
  text: |-
    ### Findings pass 3 — all 5 items of "Review Findings (2026-08-05 17:36)" corrected

    **Traversal guards (uninstall.rs:276, :354, :636).** One shared helper removes the cause from the whole file. `safe_dir_name(name)` in `crates/mirdan/src/install/mod.rs` runs `sanitize_dir_name` and then validates the result with `store::is_safe_relative_path`. The check rejects `..` references, backslashes, absolute paths, and empty segments. It returns `RegistryError::Validation` for an unsafe name.

    Why `is_safe_relative_path` and not `is_safe_name`: sanitized names can hold multiple `/`-separated segments. URL-derived names deploy to nested store paths (see `sanitize_dir_name` doc, the nested-path comment in `uninstall_skill_at`, and `test_e2e_clone_anthropics_deploy_validator_uninstall_by_url`). `is_safe_name` rejects `/` and would break that documented contract. `is_safe_relative_path` is the documented multi-segment sibling and blocks the same `..` traversal.

    Whole-file cause removal: every path-building site in uninstall.rs now uses the helper — `uninstall_skill_at`, `uninstall_validator`, `uninstall_tool`, `uninstall_plugin`, `uninstall_agent_at` (validate-first, before agent resolution), plus `guess_installed_type` (unsafe name → Skill default → Validation error downstream) and `plugin_installed` (unsafe name → false). No raw `sanitize_dir_name` call remains in uninstall.rs.

    TDD RED→GREEN: six new tests in install/tests.rs. `test_safe_dir_name_rejects_traversal_and_accepts_nested` plus one traversal test per uninstall function (skill, validator, agent, tool, plugin). Each traversal test creates a `victim/` directory next to the store and asserts a `../victim` name and an `evil\name` name get `Validation` and the victim survives. RED run: 5 failed — the skill test showed the real vulnerability (`unwrap_err` on `Ok`: the traversal name deleted the victim and returned success). GREEN after the fix: 8/8 pass.

    **package_type.rs:201.** New `pub const MAX_PACKAGE_NAME_LENGTH: usize = 64;` with a doc comment. Whole-file cause removal: the production check in `is_valid_package_name`, the `65`-char test (`MAX_PACKAGE_NAME_LENGTH + 1`), and the boundary test all use the constant. The doc list on `is_valid_package_name` references the constant.

    **store.rs:223.** `store_entry_still_referenced` takes `agent_skill_dirs: &[impl AsRef<Path>]`; the loop calls `.as_ref()`. The production caller in uninstall.rs now passes `&all_agent_dirs` (`Vec<PathBuf>`) directly — the intermediate `Vec<&Path>` is deleted. The two store.rs test call sites compile unchanged (`&Path` implements `AsRef<Path>`).

    **Verification.** `cargo nextest run -E 'rdeps(mirdan)'` — 3224 passed, 0 skipped. `cargo fmt --all` applied; `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. All 5 checklist items are `- [x]` on the card.
  timestamp: 2026-08-05T22:56:05.878926+00:00
- actor: claude-code
  id: 01kza28e0e3verrk29g69rrpg8
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/mirdan/src/install/mod.rs (new safe_dir_name helper), crates/mirdan/src/install/uninstall.rs (7 sites use the helper), crates/mirdan/src/install/tests.rs (6 new tests, RED→GREEN), crates/mirdan/src/package_type.rs (MAX_PACKAGE_NAME_LENGTH), crates/mirdan/src/store.rs (store_entry_still_referenced takes &[impl AsRef<Path>]); 5/5 findings checked; nextest rdeps(mirdan) 3224/3224, 0 skipped; fmt applied; clippy -D warnings clean
    - next: formal /review (task stays in doing)
    task: ^t1y1c37
  timestamp: 2026-08-05T22:56:11.278904+00:00
position_column: doing
position_ordinal: '8380'
title: crates/mirdan/src/install.rs is too large for the review engine — duplication can never read it
---
# Problem

`crates/mirdan/src/install.rs` is never reviewed by the `duplication` validator. The review engine skips it every time, in its own words:

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/mirdan/src/install.rs` — 567352 rendered bytes, over the 476042-byte batch budget; not reviewed by: duplication (narrow the scope)

Observed on ^mawfv02: the implementer hit it on four self-review passes, and the formal `/review` of `0e63e1031~1..0e63e1031` reproduced it. `install.rs` was the largest file in that change (187 lines changed) and held the code the card was about, so the card's own subject went unreviewed for duplication.

# Why the engine's own remedy does not work

The skip message says "narrow the scope". That does not help here. The budget is per **(validator, file) pair** — one file's rendered block must fit on its own. A `review file` run limited to `crates/mirdan/src/install.rs` still renders 567352 bytes and still exceeds the 476042-byte cap. No scoping, filtering, or `batch_size` value makes a single oversized file fit.

Raising `batch_size` cannot fix it either: the batch budget is clamped down to the agent's prompt cap, so the cap is the real ceiling.

# Why this matters

The skip is reported, but it is easy to miss in a long review, and nothing fails. A file can sit permanently outside one validator's coverage while every review of it returns "clean" for that dimension. `install.rs` is the install/uninstall path for every CLI in the workspace — duplication there is exactly the defect class that rots.

This is also a silent-coverage problem in general: any file that grows past the cap drops out of a validator's reach with no gate failing.

# Fix

Split `crates/mirdan/src/install.rs` so no single file's rendered block exceeds the prompt cap. It is one file carrying several concerns — component installation, profile application, MCP config writing, skills/agents deployment, and their test modules — which is why it reached this size.

Related: ^927239f (`mirdan install.rs cleanup: constants, dispatch dedup, nesting`) already proposes cleanup in this file. Splitting it may subsume or reshape that card — read it before starting.

Consider also whether the engine should treat "a file no validator could read" as a harder signal than a warning line in the report, since today a permanently-unreviewable file is indistinguishable from a clean one at a glance.

# Acceptance

- `crates/mirdan/src/install.rs` no longer appears in any review's "not reviewed — would exceed the agent's prompt cap" list.
- A `review file` run against every resulting file completes with none skipped.
- The split preserves behaviour: `cargo nextest run --workspace` green, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --all -- --check` clean.
- `sah init` / `sah deinit` and the `kanban` / `code-context` / `shelltool` equivalents still round-trip against a real isolated `$HOME`. #bug #review

## Review Findings (2026-08-05 15:26)

- [x] `apps/mirdan-app/src/commands.rs:62` — Near-verbatim `.map()` block repeated in two async command functions. Both uninstall_package and install_package map successful results to identical format strings, differing only in the verb ('Uninstalled' vs 'Installed'). Extract a helper function parameterized by the action verb: `fn format_action_result(spec: &str, verb: &str) -> String { format!("{} {}", verb, spec) }`. Call it from both `.map()` handlers to eliminate the duplicate.
- [x] `apps/mirdan-app/src/commands.rs:63` — Near-verbatim error handling block repeated in three async command functions. All three blocks log the error and convert to string identically, differing only in the log message. This logic could drift if updated in one place but not others. Extract a helper function that takes the error message as a parameter: `fn log_and_string_error(spec: &str, msg: &str) -> impl Fn(impl std::error::Error) -> String`. Call it from all three `.map_err()` sites to eliminate the repeated logic.
- [x] `apps/mirdan-app/src/commands.rs:82` — Near-verbatim error handling block repeated in three async command functions (duplicate instance). Extract shared error handling helper function (same as line 63 finding).
- [x] `apps/mirdan-app/src/commands.rs:108` — Hardcoded numeric literal `20` should be a named constant. This value limits the number of registry search results returned to the frontend and is unexplained in the code. Define a named constant `const MAX_REGISTRY_SEARCH_RESULTS: usize = 20;` at file or module scope and use it here: `client.fuzzy_search(&query, Some(MAX_REGISTRY_SEARCH_RESULTS))`.
- [x] `apps/mirdan-app/src/commands.rs:144` — Near-verbatim `.map()` block repeated in two async command functions (duplicate instance). Extract helper function (same as line 62 finding).
- [x] `apps/mirdan-app/src/commands.rs:145` — Near-verbatim error handling block repeated in three async command functions (duplicate instance). Extract shared error handling helper function (same as line 63 finding).
- [x] `apps/mirdan-app/src/commands.rs:163` — Near-verbatim code block repeated in find_store_path() function. Two identical if-blocks differ only in the store variable name (global_store vs project_store). Both call find_in_store() with the same logic and convert the result identically. Extract a helper function parameterized by the boolean for store scope: `fn search_store(name: &str, global: bool) -> Option<String> { let store = mirdan::store::skill_store_dir(global); find_in_store(&store, name, 5).map(|p| p.to_string_lossy().to_string()) }`. Call it twice from find_store_path.
- [x] `apps/mirdan-app/src/commands.rs:163` — Hardcoded numeric literal `5` should be a named constant. This value represents the maximum recursion depth for store directory traversal and appears unexplained in the code. Define a named constant `const MAX_STORE_SEARCH_DEPTH: u32 = 5;` at file or module scope and use it here: `find_in_store(&global_store, name, MAX_STORE_SEARCH_DEPTH)`.
- [x] `apps/mirdan-app/src/commands.rs:168` — Near-verbatim code block repeated in find_store_path() function (duplicate instance). Extract helper function (same as line 163 finding).
- [x] `apps/mirdan-app/src/commands.rs:168` — Hardcoded numeric literal `5` should be a named constant. This value represents the maximum recursion depth for store directory traversal and appears unexplained in the code. Use the same named constant as line 163: `find_in_store(&project_store, name, MAX_STORE_SEARCH_DEPTH)`.
- [x] `apps/mirdan-app/src/commands.rs:179` — Function exceeds both cognitive complexity gate (measured 21 vs. gate 15) and condition-nesting depth gate (measured 5 vs. gate 4). The recursive function has deeply nested conditionals checking directory structure, SKILL.md existence, and frontmatter name matching, making the control flow difficult to follow and maintain. Extract nested conditional logic into helper functions to reduce nesting. Create `skill_metadata_matches(path: &Path, name: &str) -> bool` to encapsulate the SKILL.md and frontmatter checks, reducing max nesting depth. Consider extracting the recursive directory traversal into a separate concern or using `walkdir` crate for cleaner recursion.
- [x] `crates/mirdan/src/dispatch.rs:15` — Verbatim error handling block repeated in three helper functions. All three have identical Err branch: `eprintln!("Error: {}", e); 1`. This is logic that could drift if one function is updated without updating the others. Extract a generic helper function or macro that handles the error case uniformly across all three functions. Example: `fn handle_error<T>(result: Result<T, RegistryError>, ok_fn: fn(T) -> ()) -> i32 { match result { Ok(t) => { ok_fn(t); 0 }, Err(e) => { eprintln!("Error: {}", e); 1 } } }`.
- [x] `crates/mirdan/src/dispatch.rs:29` — Verbatim error handling block repeated in three helper functions (duplicate instance). Extract shared error handling helper (same as line 15 finding).
- [x] `crates/mirdan/src/dispatch.rs:190` — Verbatim error handling block repeated in dispatch() function. The Completion command's error branch is identical to error branches in handle_registry_result(), handle_registry_result_msg(), and handle_deploy_result()—the fourth instance of this duplicated error pattern. Extract shared error handling helper or macro (same as line 15 finding) and apply consistently across all four error handling sites.
- [x] `crates/mirdan/src/install/applier.rs:73` — AgentAction is a pub(crate) struct with no derive macros for Debug or Clone. The trait-implementations rule requires new public types to implement all applicable traits; String-based value types should implement at minimum Debug (for introspection) and Clone (for ergonomic copying). Add #[derive(Debug, Clone)] above the struct definition at line 73.
- [x] `crates/mirdan/src/install/applier.rs:135` — Four public functions (register_mcp_server, unregister_mcp_server, deny_tool, allow_tool) are near-identical in structure, each calling for_each_agent_strategy with only the verb, strategy method, and message format differing. These are a single abstraction parameterized on action type. Extract a single helper function that takes the verb, strategy method selector, and message formatter as parameters, eliminating the four near-identical public wrappers. This reduces maintenance burden and prevents logic drift if one function is updated but the others are not.
- [x] `crates/mirdan/src/install/package.rs:485` — The comparison `t == "tool"` is case-sensitive. If the registry API can return package_type with different casings (e.g., "Tool", "TOOL"), packages with those casings would be incorrectly rejected with "is not a tool", contradicting the function's intent to handle tools from metadata alone. Without a visible test exercising non-canonical casings, the case-sensitivity contract is unproven. Make the comparison case-insensitive: change `t == "tool"` to `t.to_lowercase() == "tool"` or use a library helper like `t.eq_ignore_ascii_case("tool")`. Add one regression test that calls `install_tool_from_metadata` with `package_type = "Tool"` (capitalized) and confirms it succeeds or handles it as a tool.
- [x] `crates/mirdan/src/install/package.rs:488` — Applicability checks within `install_tool_from_metadata` use inconsistent error types. Line 488 returns `RegistryError::NotFound` when the package is not a tool, but line 546 returns `RegistryError::Validation` for a similar applicability failure (missing MCP configuration). Both are validation checks rejecting inapplicable inputs. The NotFound error type (used at line 381 for actual registry misses) should not be reused for input validation, breaking the invariant of error-type meaning. Change line 488 from `RegistryError::NotFound` to `RegistryError::Validation` to match line 546 and align with the established usage pattern (NotFound for registry misses, Validation for input checks).
- [x] `crates/mirdan/src/install/uninstall.rs:378` — Near-verbatim code block repeated in uninstall_tool (line 505). Both call unregister_mcp_from_agents with identical structure, differing only in the agents variable name and the message text ('Removed' vs 'Unregistered'). This is one pattern with two parameter variations — extract a shared helper that takes the action verb as an argument. Extract a helper function `unregister_and_report_mcp` that takes (agents: &[DetectedAgent], name: &str, global: bool, action_verb: &str) and handles both the agent unregistration loop and result reporting with the parameterized message.
- [x] `crates/mirdan/src/search.rs:198` — Hardcoded poll timeout of 100ms should be a named constant for maintainability. Define a constant `const POLL_IDLE_TIMEOUT_MS: u64 = 100;` and use it instead.
- [x] `crates/mirdan/src/search.rs:201` — Query length threshold of 3 characters for adaptive debounce strategy is unexplained magic number. Define a constant `const DEBOUNCE_SHORT_QUERY_LEN: usize = 3;` to document the intent.
- [x] `crates/mirdan/src/search.rs:281` — Default terminal dimensions 80 and 24 are hardcoded; should be named constants. Define constants `const DEFAULT_TERMINAL_COLS: u16 = 80;` and `const DEFAULT_TERMINAL_ROWS: u16 = 24;`.
- [x] `crates/mirdan/src/search.rs:283` — Hardcoded min result count of 2 and max of 20 for display should be named constants. Define constants `const MIN_DISPLAY_RESULTS: usize = 2;` and `const MAX_DISPLAY_RESULTS: usize = 20;`.
- [x] `crates/mirdan/src/search.rs:319` — Hardcoded left margin of 6 characters is unexplained; inline comment suggests 4-char margin but code uses 6. Define a constant `const LEFT_MARGIN_WIDTH: usize = 6;` to clarify the intended margin width and reconcile with comment.
- [x] `crates/mirdan/src/search.rs:420` — Hardcoded padding adjustment of 2 characters for name budget is unexplained. Define a constant `const RESULT_SPACING_WIDTH: usize = 2;` to document the layout padding.
- [x] `crates/mirdan/src/search.rs:479` — Hardcoded truncation threshold of 3 characters for string truncation logic is unexplained. Define a constant `const MIN_TRUNCATION_LEN: usize = 3;` to document when truncation applies.
- [x] `crates/mirdan/src/search.rs:483` — Hardcoded adjustment of 2 for ellipsis width in string truncation is unexplained. Define a constant `const ELLIPSIS_WIDTH: usize = 2;` to clarify that the 2 accounts for ".." length.
- [x] `crates/mirdan/src/store.rs:38` — Four functions (skill_store_dir, agent_store_dir, tool_store_dir, validators_store_dir) are near-identical, differing only by the directory name string. These should be extracted into a single parameterized helper function to avoid drift when the pattern changes. Extract a helper function like `fn store_dir(global: bool, name: &str) -> PathBuf` and replace all four public functions with thin wrappers that call it with the appropriate name string.
- [x] `crates/mirdan/src/store.rs:52` — agent_store_dir is near-identical to skill_store_dir (line 38), differing only by the directory name parameter. Extract shared helper function as noted in line 38 finding.
- [x] `crates/mirdan/src/store.rs:52` — Function `agent_store_dir` reimplements the identical pattern used in `skill_store_dir` (line 38), differing only in the directory name—should parameterize the common logic rather than duplicate it across four functions. Extract the common logic into a single parameterized helper (e.g., `store_dir(global: bool, name: &str) -> PathBuf`) and have all four public functions delegate to it, reducing code duplication.
- [x] `crates/mirdan/src/store.rs:66` — tool_store_dir is near-identical to skill_store_dir and agent_store_dir, differing only by the directory name parameter. Extract shared helper function as noted in line 38 finding.
- [x] `crates/mirdan/src/store.rs:66` — Function `tool_store_dir` reimplements the identical pattern from `skill_store_dir` (line 38), differing only in directory name—should share a parameterized helper instead of duplicating the implementation. Consolidate all four store directory functions (skill, agent, tool, validators) into a single parameterized helper function.
- [x] `crates/mirdan/src/store.rs:83` — validators_store_dir is near-identical to the three other store_dir functions, differing only by the directory name parameter. Extract shared helper function as noted in line 38 finding.
- [x] `crates/mirdan/src/store.rs:83` — Function `validators_store_dir` reimplements the identical pattern from `skill_store_dir` (line 38), differing only in directory name—should parameterize shared logic rather than duplicate across four functions. Refactor to use a single parameterized helper function for all four store directory accessors.
- [x] `crates/mirdan/src/store.rs:210` — Function exceeds cognitive complexity gate (24 vs. 15) and condition-nesting depth gate (6 vs. 4). The combination of nested loops (2 deep) and deeply nested conditionals with 8 branches makes control flow hard to follow and verify. Refactor to reduce nesting by extracting the symlink-checking logic into a helper function, or use early returns and guard clauses to flatten the conditional structure. For example, extract the `symlink_metadata` check into `is_symlink_to_target(path, canonical_store) -> bool`.

## Review Findings (2026-08-05 17:02)

- [x] `apps/mirdan-app/src/commands.rs:32` — Public struct field `pub name: String` in PackageInfo lacks a doc comment. All public items must have documentation. Add a doc comment above the field, e.g., `/// The name of the package.`.
- [x] `apps/mirdan-app/src/commands.rs:35` — Public struct field `pub description: String` in PackageInfo lacks a doc comment. Add a doc comment describing what the description field contains.
- [x] `apps/mirdan-app/src/commands.rs:36` — Public struct field `pub package_type: String` in PackageInfo lacks a doc comment. Add a doc comment describing the package type field.
- [x] `apps/mirdan-app/src/commands.rs:37` — Public struct field `pub version: String` in PackageInfo lacks a doc comment. Add a doc comment for the version field.
- [x] `apps/mirdan-app/src/commands.rs:38` — Public struct field `pub targets: Vec<String>` in PackageInfo lacks a doc comment. Add a doc comment explaining what targets represents.
- [x] `apps/mirdan-app/src/commands.rs:39` — Public struct field `pub store_path: Option<String>` in PackageInfo lacks a doc comment. Add a doc comment for the store_path field.
- [x] `apps/mirdan-app/src/commands.rs:45` — Public struct field `pub name: String` in SearchResult lacks a doc comment. Add a doc comment describing the package name.
- [x] `apps/mirdan-app/src/commands.rs:48` — Public struct field `pub description: String` in SearchResult lacks a doc comment. Add a doc comment for the description field.
- [x] `apps/mirdan-app/src/commands.rs:49` — Public struct field `pub author: String` in SearchResult lacks a doc comment. Add a doc comment describing the author field.
- [x] `apps/mirdan-app/src/commands.rs:50` — Public struct field `pub package_type: String` in SearchResult lacks a doc comment. Add a doc comment for the package_type field.
- [x] `apps/mirdan-app/src/commands.rs:51` — Public struct field `pub downloads: u64` in SearchResult lacks a doc comment. Add a doc comment explaining the downloads field.
- [x] `crates/mirdan/src/install/package.rs:486` — package_type is now checked case-insensitively at line 486 (`eq_ignore_ascii_case("tool")`), but the same token is handled case-sensitively at line 452 (`parse_package_type` returns `None` for capitalized forms). The diff treats the same token inconsistently across two install paths (metadata-only vs. archive), creating divergent behavior when the registry returns capitalized type strings like "Tool". Make the archive path consistent: either update `parse_package_type` to be case-insensitive, or add a case-insensitive check in `install_from_archive` (line 449-452) matching the one at line 486, so capitalized registry package_type values are accepted in both paths.
- [x] `crates/mirdan/src/store.rs:43` — Panics on expected failure mode: `dirs::home_dir().expect()` panics when the home directory is not found. This is an expected failure mode that can occur in containerized or embedded environments, not a bug. Rule: never panic on expected failure modes. Return `Result<PathBuf, RegistryError>` from `store_dir` to allow callers to handle the error gracefully, or document that this function requires a writable home directory and will panic if it's missing.
- [x] `crates/mirdan/src/store.rs:202` — Parameter `agent_skill_dirs: &[PathBuf]` uses concrete type instead of generic. Should accept borrowed paths `&[&Path]` for better API flexibility, consistent with rule guidance to use `&Path` not `&PathBuf`. Change parameter type to `agent_skill_dirs: &[&Path]` to allow callers to pass slices of references without requiring owned PathBuf allocations.
- [x] `crates/mirdan/src/store.rs:290` — Parameter `names: &[String]` uses concrete type instead of generic. Should accept borrowed strings `&[&str]` for better API flexibility, consistent with rule guidance to use `&str` not `&String`. Change parameter type to `names: &[&str]` to allow callers to pass string slices without requiring String allocation or borrowing.
- [x] `crates/mirdan/src/store.rs:812` — setup_skill_structure duplicates the logic of setup_store_structure (line 970)—both join path components from root, create directories, and return the pair. Should call the parameterized generic version. Replace setup_skill_structure with: `setup_store_structure(root, ".skills", ".github/copilot/skills")`.
- [x] `crates/mirdan/src/store.rs:984` — create_store_entry_with_symlink reimplements the logic already present in create_skill_symlink (line 822). Both create a store directory, write a metadata file, and establish a symlink—this shared capability should be consolidated into one parameterized function instead of duplicated. Extract a shared helper function with signature `fn create_store_entry_with_symlink_generic(store_dir: &Path, link_dir: &Path, name: &str, filename: &str, content: &str) -> (PathBuf, PathBuf)` and have both call it with appropriate arguments, or have one call the other with customized parameters.

## Review Findings (2026-08-05 17:36)

- [x] `crates/mirdan/src/install/uninstall.rs:276` — Path traversal vulnerability: `sanitize_dir_name(name)` does not prevent `..` sequences in package names, allowing escape from the skill store directory when constructing `flat_path`. Validate the name using `is_safe_name()` (which checks for `..`) before using it, or integrate path traversal checks into `sanitize_dir_name`. Example: add `if !is_safe_name(name) { return Err(...) }` before line 260, similar to `remove_store_entries` at store.rs:318.
- [x] `crates/mirdan/src/install/uninstall.rs:354` — Path traversal vulnerability: `uninstall_validator` constructs the target directory using `sanitize_dir_name(name)` without validating against `..` sequences. Add path traversal validation: check `if !is_safe_name(name)` before line 354, or modify `sanitize_dir_name` to call `is_safe_name` and reject unsafe inputs.
- [x] `crates/mirdan/src/install/uninstall.rs:636` — Path traversal vulnerability: `uninstall_agent_at` constructs the store path using `sanitize_dir_name(name)` without validating against `..` sequences. Add validation: check `if !is_safe_name(name)` before line 606, or modify `sanitize_dir_name` to include path traversal checks similar to the `is_safe_name` function at store.rs:258-264.
- [x] `crates/mirdan/src/package_type.rs:201` — Hardcoded numeric literal 64 used to configure test behavior (size validation boundary) should derive from a named constant rather than being a magic number. Define a constant `const MAX_PACKAGE_NAME_LENGTH: usize = 64;` and use `"a".repeat(MAX_PACKAGE_NAME_LENGTH)` instead of hardcoding 64.
- [x] `crates/mirdan/src/store.rs:223` — Function accepts `&[&Path]` but should accept `&[impl AsRef<Path>]` to follow std library conventions and avoid forcing callers to create intermediate reference vectors when they already have PathBuf collections. Change the signature to `pub fn store_entry_still_referenced(store_path: &Path, agent_skill_dirs: &[impl AsRef<Path>]) -> bool {` and update the loop to call `.as_ref()` on each element if needed.