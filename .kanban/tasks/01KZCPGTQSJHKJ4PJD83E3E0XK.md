---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzcr3twnhdtqvs6sdwtfynek
  text: |-
    Research done. The process-CWD reads live in `crates/swissarmyhammer-validators/src/validators/loader.rs`, in three places that all call `ManagedDirectory::<ValidatorsConfig>::from_git_root()`:
    - `ValidatorLoader::load_all`
    - `ValidatorLoader::get_directories`
    - `ValidatorLoader::diagnostics`

    Two extra facts found while reading:
    1. `ManagedDirectory::new` CREATES the directory it resolves. So today, loading rules CREATES `<cwd git root>/.validators` in whatever repository the process CWD sits in — a stray-dotdir side effect. The user layer does NOT do this: `user_validators_dir()` is a plain `dirs::home_dir().join(ValidatorsConfig::DIR_NAME)`. The fix mirrors the user layer: `workspace_root.join(ValidatorsConfig::DIR_NAME)`, plus the `.exists()` guard that is already there. No creation.
    2. `ReviewTool::run_health_checks` (the `Doctorable` doctor surface) documents that it loads the project layer relative to the session working directory, but calls `check_validators()` with no root. `Doctorable::run_health_checks(&self)` carries no context, so the root must become tool state.

    Plan:
    - `load_rules(workspace_root: Option<&Path>)`; `load_all` / `get_directories` / `diagnostics` take the same. `None` = no project layer (fail closed), matching `workspace_project_types`'s `None` semantics.
    - `match_rules` passes its existing root down.
    - `doctor::check_review_engine(workspace_root)` passes `Some(workspace_root)`.
    - tools: `get_validator` / `check_validators` gain the root; `list_validators` / `dump_validators` pass the one they already take.
    - `review_op::run_review_request_inner` passes its `repo_path`.
    - CLI `ValidatorTools::init` passes the `with_workspace_root` root it already has.
    - `ReviewTool` gains a `workspace_root` field for the doctor surface; `collect_all_health_checks` takes the doctor workspace root and the CLI doctor supplies it.
  timestamp: 2026-08-06T23:56:38.165930+00:00
- actor: claude-code
  id: 01kzcsfg1x7j3c5t4vt5kz80dz
  text: |-
    Implementation landed, TDD RED→GREEN verified.

    RED first: `list_validators_reads_the_session_workspace_not_the_process_cwd` (crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs). Two workspaces each carry their own `.validators` set; the session names workspace A while `CurrentDirGuard` pins the process CWD to workspace B. It failed with `["... , "cwd-workspace-set", ...]` — the CWD workspace's set loaded and the session's did not. Exactly the reported defect.

    GREEN after threading the root. The project layer now resolves through two private helpers in loader.rs, `project_validators_dir` / `existing_project_validators_dir`, which join `ValidatorsConfig::DIR_NAME` onto the caller's root the way `user_validators_dir` joins it onto home.

    Side effect removed on the way: `ManagedDirectory::from_git_root()` CREATES the directory it resolves, so every rule load used to create `<cwd git root>/.validators`. The plain join does not create anything, and the `.exists()` guard that was already there decides whether the layer loads.

    The loader now has no CWD dependency at all, so its `CwdGuard` test helper became dead code and was deleted; the three tests that used it pass an explicit root instead. `ValidatorLoader::get_directories` was renamed `directories` (the Rust api-design rule bans a `get_` prefix on a getter) and gained a real test — the old one only asserted the length was at most 2.

    Doctor surface: `Doctorable::run_health_checks(&self)` carries no session, so the root became tool state. `ReviewTool::with_doctor_workspace_root` + `register_review_tool_for_workspace` pin it, `collect_all_health_checks(workspace_root)` supplies it, and the CLI resolves it once in the new `checks::doctor_workspace_root()`. That helper is now the ONE place the process working directory answers "which workspace" — correct for a CLI, which is invoked in the workspace it is asked about. No library rediscovers a workspace below it.

    To avoid a name collision with the new field, `ReviewTool::workspace_root(context)` (from ^3hwy2pd) is renamed `session_workspace_root`. Same resolution, reused as-is — no second one was added.

    Verification: `cargo nextest run --workspace` 13654 passed / 0 failed / 0 skipped. `cargo clippy --workspace --all-targets` clean. `cargo doc --workspace --no-deps` adds no warning in any touched file.
  timestamp: 2026-08-07T00:20:28.861669+00:00
- actor: claude-code
  id: 01kzcsfq2pr56tex76eqztvear
  text: |-
    ### implement — changed
    - evidence: 12 files — crates/swissarmyhammer-validators/src/validators/loader.rs, crates/swissarmyhammer-validators/src/validators/mod.rs, crates/swissarmyhammer-validators/src/validators/types.rs, crates/swissarmyhammer-validators/src/lib.rs, crates/swissarmyhammer-validators/src/doctor.rs, crates/swissarmyhammer-validators/src/builtin/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs, crates/swissarmyhammer-tools/src/health_registry.rs, apps/swissarmyhammer-cli/src/commands/doctor/{checks.rs,mod.rs} + apps/swissarmyhammer-cli/src/commands/install/components/mod.rs
    - next: ready for /review
  timestamp: 2026-08-07T00:20:36.054303+00:00
position_column: doing
position_ordinal: '8480'
title: Resolve the project validator directory from the session working dir, not the process CWD
---
Found while doing ^3hwy2pd.

`ValidatorLoader::load_all` (and `validator_directories` / the loader diagnostics) resolve the PROJECT validator layer with `ManagedDirectory::<ValidatorsConfig>::from_git_root()`, which reads the process current directory. `load_rules()` calls it, and every `review` op calls `load_rules()`.

So the `list/dump/get/check validators` ops and `match_rules` load `<cwd git root>/.validators`, not `<session working dir git root>/.validators`. A server whose process CWD differs from the session working dir loads the wrong project layer, or none.

^3hwy2pd threaded a workspace root into the same ops for PROJECT TYPE resolution. This card threads the same root into the RuleSet LOAD.

Work:
- Give `load_rules` a workspace root parameter and pass it down to the project-layer directory resolution.
- Keep the current behavior when no root is available.
- Update every caller: the `review` tool ops, `match_rules`, the doctor surface.

Acceptance:
- A `list validators` call whose session working dir names project A returns project A's validators while the process CWD sits in project B.
- No production path calls `std::env::current_dir()` to find `.validators`.

#tool-validators