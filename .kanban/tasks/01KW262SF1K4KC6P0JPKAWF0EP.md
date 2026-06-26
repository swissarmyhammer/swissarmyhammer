---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2snjrezspwpqsaecr15ery
  text: 'Picked up. Research done: confirmed reference impls (KanbanTool Initializable, ProjectStructure), ensure_gitignore_entries pattern (explicit entries, append-missing, no blanket *), ExpectConfig (exported from swissarmyhammer_expect, deny_unknown_fields, all-default round-trip), Surface enum (serde lowercase: cli/http/browser/gui/file/db), ProjectType enum (11 variants), detect_projects(root, max_depth). All deps already present in swissarmyhammer-tools Cargo.toml. Spec §"expect init" (ideas/expect.md ~320) gives the canonical .expect/ tree + config.toml template (~967). Plan: new init.rs submodule with root-explicit scaffold_expect_dir, ProjectType->Surface mapping table, config.toml = canonical template + detected-surface header comment, Initializable impl on ExpectTool gated to Project|Local with safe root resolution (git-root else cwd, None->skipped, no panic). Register in registry.rs register_all (len 2->3).'
  timestamp: 2026-06-26T20:24:18.446484+00:00
- actor: claude-code
  id: 01kw2t5p96hbwgs5gfh0fzfkb9
  text: |-
    Implemented and green. Changes:
    - NEW crates/swissarmyhammer-tools/src/mcp/tools/expect/init.rs — Initializable impl for ExpectTool (display_name "Expectations", category "tools", priority 45, applicable Project|Local only). Root-explicit scaffold_expect_dir(root): creates .expect/{expectations,goldens,received}/, write_if_absent for README.md/example.expect.md/config.toml (idempotent, never overwrites), ensure_gitignore mirroring kanban's ensure_gitignore_entries (explicit `received/`, no blanket `*`). detected_surfaces() calls detect_projects(root, None) (errors -> default cli, no panic), exhaustive ProjectType->Surface table (Rust/Go/Python/CMake/Makefile=cli, NodeJs/Java*/CSharp/Php=http, Flutter=gui), surface names derived from Surface serde. config.toml = documented all-defaults template (round-trips to ExpectConfig::default) + detected-surface header comment. resolve_project_root() = git root else cwd, None->Skipped (no .expect() on env path). deinit preserves .expect/ (committed goldens).
    - NEW templates/{config.toml,README.md,example.expect.md}.
    - mod.rs: removed impl_empty_initializable! stub, added `mod init;`.
    - registry.rs: registered ExpectTool in register_all (priority 45, between ProjectStructure and KanbanTool); updated doc table + both count tests 2->3.

    Verification: `cargo nextest run -p swissarmyhammer-tools -E 'test(expect)'` = 26 passed (13 new init tests). RED verified: blanket-`*` gitignore breaks the keeps-goldens test. `cargo check --workspace` clean. `cargo clippy -p swissarmyhammer-tools -- -D warnings` clean. CLI registry tests pass (len 3). double-check agent: PASS. Leaving in `doing` for /review.
  timestamp: 2026-06-26T20:33:06.342322+00:00
depends_on:
- 01KW25ZW4NED0J1BD77HPK7DNX
- 01KW260M8QZ8T37A8RZGDDVZ81
position_column: doing
position_ordinal: '8280'
project: expect
title: 'expect init: scaffold .expect/ tree + surface auto-detection'
---
## What
Implement `expect init` (the `Initializable` trait verb, rolling up to `sah init`) that scaffolds the `.expect/` dot-folder. Per `ideas/expect.md` §"expect init". Idempotent, never overwrites existing files.

- Implement `Initializable` for `ExpectTool` (replacing the `impl_empty_initializable!` stub), mirroring `KanbanTool`'s impl (`crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:198-363`) and `ProjectStructure` (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`).
- Scaffold at repo root:
  ```
  .expect/
    config.toml        # written with detected surface defaults
    README.md          # what expectations are + how to write one
    example.expect.md  # one worked expectation, ready to copy
    expectations/      # repo-global specs
    goldens/           # committed
    received/          # gitignored
    .gitignore         # ignores received/, keeps goldens/
  ```
- `.gitignore`: mirror `ensure_gitignore_entries` (`crates/swissarmyhammer-kanban/src/board/init.rs:313`) — explicit `received/` entry, NOT a blanket `*`, so `goldens/` stay tracked.
- Surface auto-detection: call `detect_projects()` (`crates/swissarmyhammer-project-detection`), map detected `ProjectType`s to sensible `surface` defaults written into `config.toml` so the first `expect expectation create` has context.
- Register the component in `apps/swissarmyhammer-cli/src/commands/registry.rs` `register_all` (alongside `ProjectStructure`/`KanbanTool`) so `sah init` runs it.
- Gate filesystem work to `InitScope::Project|Local` (not User), as kanban does.

## Acceptance Criteria
- [ ] `expect init` (and `sah init`) create the full `.expect/` tree; re-running does not overwrite `config.toml`/`example.expect.md`.
- [ ] `.expect/.gitignore` ignores `received/` but not `goldens/` (no blanket `*`).
- [ ] `config.toml` is written with surface defaults derived from the detected project type(s).
- [ ] Running in a non-git / read-only CWD does not panic (derive root safely; mirror gui-cwd-readonly guidance).

## Tests
- [ ] Init test in a `tempfile` repo: asserts every scaffolded path exists, `.gitignore` content (received ignored, goldens kept), and idempotency on second run.
- [ ] Test that detected `ProjectType` ⇒ expected `surface` default in `config.toml`.
- [ ] `cargo nextest run -p swissarmyhammer-tools expect_init` passes.

## Workflow
- Use `/tdd`.