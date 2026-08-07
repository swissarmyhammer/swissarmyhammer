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
- actor: claude-code
  id: 01kzcvax3p0v74tpt4vaef2wxq
  text: |
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (c8228f6f8) — 14 confirmed, 23 refuted, 36 attempted, 0 failed. 11 recorded in the 2026-08-06 19:31 section.
    - dropped: 3 findings at crates/swissarmyhammer-validators/src/validators/types.rs:1616, :1823, :1844. They ask to replace literal timeouts with a named constant in pre-existing test code. The commit changed only one doc comment line in that file (types.rs:343). The review skill test-refactor exception drops them.
    - no over-cap or batch-budget findings in this pass.
    - next: fix the 11 open findings, then re-review.
  timestamp: 2026-08-07T00:52:55.542675+00:00
- actor: claude-code
  id: 01kzcvbgd3xa7na1wqr6p29hdq
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 14 files; ManagedDirectory::from_git_root is gone from the validators crate, the rule load takes Option<&Path>, the doctor workspace is tool state
    - test: green — cargo nextest run --workspace 13654 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: c8228f6f8
    - review: findings — apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:42, :138, :289, :317, :417, :503; crates/swissarmyhammer-validators/src/validators/loader.rs:672, :711, :868, :1184; crates/swissarmyhammer-validators/src/validators/mod.rs:49
  timestamp: 2026-08-07T00:53:15.299799+00:00
- actor: claude-code
  id: 01kzcwqe4bqd9gq055e4dam54h
  text: |-
    All 11 findings of the 2026-08-06 19:31 section are done. Each cause was removed from the WHOLE file, not only the named line.

    **checks.rs:42, :138, :289, :317, :417 — accept generics, not concrete types.** Every function in `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` that took `&mut Vec<Check>` now takes `&mut impl Extend<Check>` — the five named ones plus `check_binary_permissions` (both cfg variants), `check_claude_config`, `check_install_stack_with`, and `check_review_engine`. `Extend` has no `push`, so all 14 `checks.push(x)` calls became `checks.extend([x])`. Callers are unchanged: `Vec<Check>` implements `Extend<Check>`.

    **checks.rs:503 — one shared workspace-root resolution.** New `swissarmyhammer_common::utils::find_workspace_root() -> PathBuf` (git root, else current directory, else `.`), re-exported from `utils`. `doctor_workspace_root()` now delegates to it. In `install/components/mod.rs` the private `workspace_root() -> Result<PathBuf, String>` is DELETED and `with_workspace_root` calls the shared function directly.

    The finding's stated remedy returns `PathBuf`, so resolution can no longer fail. That made `with_workspace_root`'s `Err` arm unreachable — dead code — so the arm and the `component` parameter went with it, and the three call sites now read `with_workspace_root(|root| ...)`. Nothing asserted on the deleted `"failed to get current directory"` message. A cwd that cannot be read now falls back to `.`, which names the same directory every later filesystem call resolves against, so the real failure still surfaces with its own context.

    **loader.rs:672, :711, :868, :1184 — the inverse case.** All four tests now pin the process working directory (shared `CurrentDirGuard`, already `#[serial_test::serial(cwd)]`) to a SECOND project that carries its own conflicting `.validators`, while the supplied workspace root names the first:
    - precedence test: cwd project holds `cwd-only` + `shared: CWD version`; asserts `cwd-only` is absent and `shared` is the supplied workspace's `Project version`.
    - tool-rule test: cwd project holds `tooled` (running `cwd-runner`) + `cwd-tooled`; asserts `cwd-tooled` is absent and the loaded run is `project-runner`.
    - failure-collection test: cwd project holds its own valid AND malformed sets; asserts `cwd-good-one` is absent and failures stay at exactly 1 — a cwd-derived layer would collect 2.
    - `directories` test: cwd project has a `.validators` directory for the whole test, so the two empty-case assertions and the named case all prove the working directory contributes nothing.

    `swissarmyhammer-common` is now a dev-dependency of `swissarmyhammer-validators` for `CurrentDirGuard` — no cycle, since common already sits below validators through `swissarmyhammer-directory`, and no local guard copy was reintroduced.

    **mod.rs:49 — the example propagates.** The module example is now `fn main() -> Result<(), AvpError>` using `?`, no `.unwrap()` anywhere in the file.

    **Not vacuous — RED proved.** The four loader tests pass on the fixed code, so `existing_project_validators_dir` was temporarily re-pointed at `std::env::current_dir()` (the original defect) and all four failed; the injection was then reverted and all 22 loader tests pass again.

    Verification: `cargo nextest run --workspace` 13656 passed / 0 failed / 0 skipped. `cargo test --workspace --doc` 0 failed. `cargo clippy --workspace --all-targets` clean. `cargo fmt --all --check` clean. `cargo doc --workspace --no-deps` adds no warning in any touched file.
  timestamp: 2026-08-07T01:17:14.763541+00:00
- actor: claude-code
  id: 01kzcwqks706rkctxd9dtw06bn
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/swissarmyhammer-common/src/utils/directory_utils.rs, crates/swissarmyhammer-common/src/utils/mod.rs, apps/swissarmyhammer-cli/src/commands/doctor/checks.rs, apps/swissarmyhammer-cli/src/commands/install/components/mod.rs, crates/swissarmyhammer-validators/Cargo.toml, crates/swissarmyhammer-validators/src/validators/loader.rs, crates/swissarmyhammer-validators/src/validators/mod.rs (+ Cargo.lock)
    - next: ready for /review
  timestamp: 2026-08-07T01:17:20.551630+00:00
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

## Review Findings (2026-08-06 19:31)

- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:42` — Function accepts concrete type `&mut Vec<Check>` instead of generic. Should accept `&mut impl Extend<Check>` or similar to allow callers flexibility and follow the rule 'Accept generics, not concrete types.'. Change signature to `pub fn check_installation(checks: &mut impl Extend<Check>) -> Result<()>` to accept any type implementing `Extend<Check>`.
- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:138` — Function accepts concrete type `&mut Vec<Check>` instead of generic. Should accept `&mut impl Extend<Check>` or similar. Change signature to `pub fn check_in_path(checks: &mut impl Extend<Check>) -> Result<()>`.
- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:289` — Function accepts concrete type `&mut Vec<Check>` instead of generic. Should accept `&mut impl Extend<Check>` or similar. Change signature to `pub fn check_file_permissions(checks: &mut impl Extend<Check>) -> Result<()>`.
- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:317` — Function accepts concrete type `&mut Vec<Check>` instead of generic. Should accept `&mut impl Extend<Check>` or similar. Change signature to `pub fn check_lsp_servers(checks: &mut impl Extend<Check>) -> Result<()>`.
- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:417` — Function accepts concrete type `&mut Vec<Check>` instead of generic. Should accept `&mut impl Extend<Check>` or similar. Change signature to `pub fn check_install_stack(checks: &mut impl Extend<Check>) -> Result<()>`.
- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:503` — Function `doctor_workspace_root()` reimplements the same logic as the existing private `workspace_root()` in `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs` line 340. Both find the git repository root and fall back to the current directory; only the error handling differs (Result vs PathBuf with fallback). A shared utility should be extracted instead of duplicating this pattern. Extract a shared utility function (e.g., `swissarmyhammer_common::utils::find_workspace_root()`) that returns `PathBuf`, returning the git root or falling back to current_dir. Have both `workspace_root()` in components/mod.rs and `doctor_workspace_root()` call this shared implementation, differing only in how they handle errors on the Result wrapper if needed.
- [x] `crates/swissarmyhammer-validators/src/validators/loader.rs:672` — Pre-existing test claims (via name and docstring) to verify precedence of user and project validators with a supplied workspace_root, but does not verify the paired/inverse case — that the process CWD does not interfere even when different from the supplied workspace. Extend test to create two distinct temporary projects, change process CWD to one project's directory, call `load_all(Some(other_project.path()))`, and verify that other_project's validators load (not CWD's).
- [x] `crates/swissarmyhammer-validators/src/validators/loader.rs:711` — Test is marked `#[serial_test::serial(cwd)]` and calls `loader.load_all(Some(project_root.path()))` to verify layer precedence, but does not verify the paired/inverse case — that the process CWD does not interfere even when different from the supplied workspace. Extend test to create two temporary projects with different tool rules, change process CWD to one project, call `load_all(Some(other_project.path()))`, and verify other_project's tool rules load (not CWD's).
- [x] `crates/swissarmyhammer-validators/src/validators/loader.rs:868` — Test is marked `#[serial_test::serial(cwd)]` and calls `loader.load_all(Some(project_root.path()))` to verify failure collection, but does not verify the paired/inverse case — that the process CWD does not interfere even when different from the supplied workspace. Extend test to create two temporary projects, change process CWD to one project, call `load_all(Some(other_project.path()))`, and verify the correct project's validators load (not CWD's).
- [x] `crates/swissarmyhammer-validators/src/validators/loader.rs:1184` — Test claims to verify directory resolution ignores the process CWD, but exercises only one direction: it passes an explicit workspace_root and verifies the function uses it. The test does not verify the critical inverse case — that the function still uses the supplied root even when the process CWD is a different project directory. Extend test to create two temporary project directories, change the process CWD to one of them, call `directories(Some(other_project.path()))`, and verify the other project's validators are returned despite the CWD pointing elsewhere.
- [x] `crates/swissarmyhammer-validators/src/validators/mod.rs:49` — Example code uses `.unwrap()` instead of proper error handling with `?`, teaching bad error-handling practices to users. Restructure the example to demonstrate error propagation: wrap the code in a function returning `Result<(), AvpError>` and use `?` instead of `.unwrap()`, e.g., `loader.load_all(Some(Path::new("/path/to/workspace")))?;`.
