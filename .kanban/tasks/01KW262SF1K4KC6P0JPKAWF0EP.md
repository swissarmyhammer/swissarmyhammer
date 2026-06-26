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
- actor: claude-code
  id: 01kw2tz0k6kk4ng4kxe725td5q
  text: 'Re-picked from review to resolve 2 maintainability findings. Plan: (A) derive the config.toml header surface list from surfaces::catalog() instead of the hardcoded literal; (B) PREFERRED path is viable — both kanban and swissarmyhammer-tools(expect) already depend on swissarmyhammer-common, so extracting a shared ensure_gitignore_entries(dir, &[entries]) into common/src/fs_utils.rs and delegating from both is a clean drop-in. Kanban keeps its own REQUIRED_GITIGNORE_ENTRIES constant (domain data) as a thin wrapper; expect likewise. TDD on the shared helper.'
  timestamp: 2026-06-26T20:46:56.102338+00:00
- actor: claude-code
  id: 01kw2vh9r2czk967kfm84hp5me
  text: |-
    Both review findings resolved (PREFERRED path for the duplication finding).

    FINDING B (gitignore duplication, blocker + warning): extracted the canonical reconciler into swissarmyhammer-common — `swissarmyhammer_common::fs_utils::ensure_gitignore_entries(dir: &Path, entries: &[&str])` with 4 new TDD tests (create-missing, append-without-clobber, idempotent, trimmed-line dedup). kanban's `ensure_gitignore_entries(kanban_root)` and expect's `ensure_gitignore(expect_dir)` now both delegate, each keeping its own REQUIRED_GITIGNORE_ENTRIES. Verified clean drop-in: `cargo nextest run -p swissarmyhammer-kanban` = 1537 passed.

    FINDING A (hardcoded surface list): added `use swissarmyhammer_expect::{surfaces, Surface}`; new `catalog_surface_names()` derives the config.toml header "one of:" list from `surfaces::catalog()` (byte-identical to the old literal, but now drift-proof). New test `expect_init_config_header_lists_every_catalog_surface` guards it. Also moved the now-test-only GITIGNORE_FILE const into the test module to keep clippy -D warnings clean.

    Verification: expect tests 27 passed; kanban 1537 passed; common gitignore tests pass; `cargo check --workspace` clean; `cargo clippy -p swissarmyhammer-tools -- -D warnings` clean; `cargo fmt` applied. double-check agent: PASS. Leaving in `doing` for /review.
  timestamp: 2026-06-26T20:56:55.298616+00:00
- actor: claude-code
  id: 01kw2w89s2sfht1bxkjbb7tyjv
  text: |-
    Cleared the two open warnings from the 2026-06-26 15:57 review.

    1. fs_utils.rs ensure_gitignore_entries doc comment: already present and comprehensive in HEAD (commit b65e12088) — covers reconcile/append-only-missing, idempotency, reads+preserves existing content, rewrites only when changed, params (dir/entries), and io::Result error condition. No code change needed; finding satisfied by committed code.

    2. expect/init.rs expect_paths test helper: refactored to derive subdir paths from SCAFFOLD_SUBDIRS (fixed paths in the vec, then `for subdir in SCAFFOLD_SUBDIRS { paths.push(expect_dir.join(subdir)); }`) instead of hardcoding "expectations"/"goldens"/"received". Eliminates the drift risk.

    Verify: cargo nextest -p swissarmyhammer-tools -E 'test(expect)' = 27 passed; -p swissarmyhammer-common -E 'test(gitignore)' = 8 passed; cargo fmt applied; clippy -p swissarmyhammer-common -p swissarmyhammer-tools -D warnings clean. Left in doing for /review.
  timestamp: 2026-06-26T21:09:28.994181+00:00
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
- [x] `expect init` (and `sah init`) create the full `.expect/` tree; re-running does not overwrite `config.toml`/`example.expect.md`.
- [x] `.expect/.gitignore` ignores `received/` but not `goldens/` (no blanket `*`).
- [x] `config.toml` is written with surface defaults derived from the detected project type(s).
- [x] Running in a non-git / read-only CWD does not panic (derive root safely; mirror gui-cwd-readonly guidance).

## Tests
- [x] Init test in a `tempfile` repo: asserts every scaffolded path exists, `.gitignore` content (received ignored, goldens kept), and idempotency on second run.
- [x] Test that detected `ProjectType` ⇒ expected `surface` default in `config.toml`.
- [x] `cargo nextest run -p swissarmyhammer-tools expect_init` passes.

## Workflow
- Use `/tdd`.

## Review Findings (2026-06-26 15:33)

### Blockers
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/expect/init.rs:177` — The `ensure_gitignore` function is verbatim to kanban's `ensure_gitignore_entries` (crates/swissarmyhammer-kanban/src/board/init.rs:313–332), differing only in parameter names (`expect_dir` vs `kanban_root`), variable names (`line` vs `l`), and the path constant. Two functions differing only by renamed variables are one function with arguments. This duplication inflates maintenance surface—a fix applied to one and not the other becomes a latent bug. Extract a shared `ensure_gitignore_reconcile(dir: &Path, entries: &[&str]) -> std::io::Result<()>` helper (in swissarmyhammer-common or a gitignore module). Both kanban and expect call it with their respective directory and `REQUIRED_GITIGNORE_ENTRIES`. Delete the duplicate implementations. **FIXED (preferred path)**: extracted `swissarmyhammer_common::fs_utils::ensure_gitignore_entries(dir: &Path, entries: &[&str])` (single canonical impl, 4 new tests). kanban's `ensure_gitignore_entries(kanban_root)` and expect's `ensure_gitignore(expect_dir)` both delegate to it, each keeping its own `REQUIRED_GITIGNORE_ENTRIES`. kanban suite 1537 passed (clean drop-in).

### Warnings
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/expect/init.rs:122` — The `ensure_gitignore()` function reimplements the exact algorithm already in kanban's `ensure_gitignore_entries()` (crates/swissarmyhammer-kanban/src/board/init.rs:313-332). The comment at line 124 acknowledges this: 'Mirrors the kanban board's `ensure_gitignore_entries`'. The algorithm is identical (read lines, check for missing entries, rewrite if changed); only the directory and required entries differ. This should be extracted into a shared generic utility parameterized by path and entries, then called from both places. Extract a generic `reconcile_gitignore_entries(path: &Path, entries: &[&str]) -> io::Result<()>` into a shared utility module (e.g., `swissarmyhammer-common`), then call it from both kanban and expect with their respective `REQUIRED_GITIGNORE_ENTRIES` arrays. This keeps one canonical implementation and prevents divergence if the algorithm needs fixing later. **FIXED**: same extraction as the blocker above — `swissarmyhammer_common::fs_utils::ensure_gitignore_entries`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/expect/init.rs:138` — Hardcoded list of surface names in config template comment; should be derived from the Surface enum or catalog to stay in sync when new surface variants are added. Derive the surface list dynamically from `surfaces::catalog()` (already imported in `mod.rs`). Add `use swissarmyhammer_expect::surfaces;` to the imports, create a helper function that generates the comment with the current catalog, and update the test (`expect_init_config_contents_still_parse_with_surface_header`) to verify all catalog surfaces are listed. **FIXED**: added `use swissarmyhammer_expect::{surfaces, Surface}`; new `catalog_surface_names()` helper derives the "one of:" list from `surfaces::catalog()`; `config_contents` uses it; new test `expect_init_config_header_lists_every_catalog_surface` asserts every catalog surface name appears in the header.

## Review Findings (2026-06-26 15:57)

### Warnings
- [x] `crates/swissarmyhammer-common/src/fs_utils.rs:547` — Public function `ensure_gitignore_entries` lacks doc comments. The rule requires all public items to have documentation explaining purpose, parameters, return value, and any error conditions. Add a doc comment above the function explaining: (1) that it reconciles gitignore entries (appending only those missing), (2) the intent of idempotency, (3) that it reads existing content and preserves it, (4) when it modifies the file. Example: `/// Reconcile `.gitignore` entries — append any that are missing, leaving existing content untouched.
/// 
/// Returns `Ok(())` on success or if no changes were needed (idempotent). Returns `Err` if file I/O fails.`. **FIXED**: the public `ensure_gitignore_entries` already carries a full doc comment in HEAD (commit b65e12088) covering all four points — reconcile/append-only-missing, idempotency (no-op when complete), reads+preserves existing lines without clobbering, rewrites only when changed — plus its params (`dir`, `entries`) and the `std::io::Error` return condition. Confirmed satisfied.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/expect/init.rs:227` — Subdirectory names "expectations", "goldens", "received" are hardcoded in `expect_paths` test helper, duplicating the `SCAFFOLD_SUBDIRS` constant. If directories are added or removed, both must be kept in sync — a maintenance burden and source of drift. Refactor `expect_paths` to derive subdirs from `SCAFFOLD_SUBDIRS` rather than hardcoding: collect the fixed paths (root, config, readme, example, gitignore), then append paths from the loop `for subdir in SCAFFOLD_SUBDIRS { paths.push(expect_dir.join(subdir)); }`. **FIXED**: `expect_paths` now builds the fixed paths then appends `for subdir in SCAFFOLD_SUBDIRS { paths.push(expect_dir.join(subdir)); }`, deriving all subdirs from the single `SCAFFOLD_SUBDIRS` source.