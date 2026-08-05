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