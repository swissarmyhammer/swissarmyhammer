---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff080
project: ai-panel
title: Initialize the board folder as a SwissArmyHammer workspace (inline sah init)
---
## What
When a board folder is opened in the kanban app it should be a full SwissArmyHammer workspace — skills, prompts, the SAH directory — so the in-process agent has the same toolset and skills `sah` provides. Run the `sah init` workspace-setup logic **in-process** (Rust), never by shelling out to `sah init`.

- `sah init` is composable `Initializable` components: `apps/swissarmyhammer-cli/src/commands/install/init.rs` builds an `swissarmyhammer_common::lifecycle::InitRegistry`, registers components via `swissarmyhammer_cli::commands::registry::register_all`, and runs `run_all_init(&scope, &reporter)`.
- In the kanban-app, on board open (`AppState::open_board` / `BoardHandle::open`, `apps/kanban-app/src/state.rs`), run the same init for the board folder at project scope: build an `InitRegistry`, register the components, run init rooted at the board directory. Must be idempotent — safe to run on every open.
- Scope decision: include the workspace + skills components; EXCLUDE the Claude-Code `.claude/settings.json` bits (`install_deny_bash`, `install_statusline`).

## Acceptance Criteria
- [x] Opening a board folder makes it a SAH workspace (SAH directory + skills present) via in-process init — no `sah` subprocess.
- [x] Init is idempotent across repeated board opens.
- [x] The chosen way to reach the init logic (CLI-as-library vs extracted crate) is documented in the task.
- [x] `cargo build -p kanban-app` is clean.

## Tests
- [x] Integration test: open a board in a fresh temp dir; assert the SAH workspace layout and skills are created; open again, assert idempotent (no error, no duplication).
- [x] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the workspace-created + idempotency tests first.

## Implementation Notes (2026-05-16) — Option B chosen (per user decision)

The prior agent's "BLOCKED" section is superseded. The user reviewed the A/B/C
options and **directed Option B**: extract the init registry + workspace/skills
components into a new lightweight library crate AND rework them to accept an
explicit root `&Path` instead of relying on process CWD / git-root detection.
No process-CWD mutation anywhere.

### New crate: `swissarmyhammer-workspace-init`
`crates/swissarmyhammer-workspace-init/` — a lightweight library (deps:
`swissarmyhammer-common`, `-config`, `-prompts`, `-skills` only; no heavy CLI
dependency tree). Public API:
- `run_workspace_init(root: &Path, scope, reporter) -> Vec<InitResult>` — builds a fresh `InitRegistry`, registers the components, runs them.
- `register_workspace_init(&mut InitRegistry, root: &Path)` — registers the components into a caller-owned registry.
- Re-exports the lifecycle vocabulary (`InitRegistry`, `InitResult`, `InitScope`, `InitStatus`, `Initializable`).

### What was extracted and rerooted
Two root-explicit `Initializable` components:
- `ProjectStructure` (priority 20) — creates `<root>/.sah/` (+ `workflows/`) and `<root>/.prompts/` via `SwissarmyhammerDirectory::from_custom_root(root)`. No git-root detection, no `current_dir()`.
- `SkillDeployment` (priority 30) — renders the builtin skills through the Liquid prompt engine and writes them to a board-local `<root>/.sah/skills/<name>/SKILL.md` (plus bundled resources). It does NOT use mirdan agent-detection or a CWD-relative store — the workspace is self-contained for an in-process agent. Idempotent: each skill dir is recreated from scratch per run. Frontmatter formatting is shared via `swissarmyhammer_skills::deploy::format_skill_md`.

The Claude-Code `.claude/settings.json` components (`deny-bash`, `statusline`) are intentionally EXCLUDED — out of scope.

### CLI migration (init logic not forked)
`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs`:
`ProjectStructure::init` now resolves the root (git root, else CWD) and
delegates the actual `.sah/` + `.prompts/` creation to
`swissarmyhammer_workspace_init::ProjectStructure::new(root).init(...)`. The
CLI keeps its own `deinit` (directory removal) and its mirdan-based
`skill::SkillDeployment` (which targets detected coding-agent dirs — a distinct
target from the workspace-local `.sah/skills/`, so not a fork). The `.sah/`
workspace-structure logic now lives in exactly one place. CLI builds clean;
all 451 CLI lib tests pass.

### kanban-app wiring
`apps/kanban-app/src/state.rs`: new `ensure_sah_workspace(&Path)` helper is
called at the top of `BoardHandle::open` (reached from `AppState::open_board`).
It roots `run_workspace_init` at the board folder (the parent of the `.kanban`
directory), at `InitScope::Project`, on every board open. Idempotent; errors
are logged and swallowed so a board still opens on a filesystem hiccup. The
process CWD is never touched.

### Tests added
`apps/kanban-app/tests/workspace_init.rs` — 3 integration tests (written first,
TDD): workspace + skills created on open; repeated open is idempotent (no
duplication); init does not mutate process CWD. Plus 10 unit tests + 1 doctest
in the new crate.

### Verification (all green)
- `cargo build -p kanban-app` — clean
- `cargo test -p kanban-app` — 126 tests pass (incl. 3 new integration tests)
- `cargo test -p swissarmyhammer-workspace-init` — 11 tests pass
- `cargo test -p swissarmyhammer-cli --lib` — 451 tests pass
- `cargo clippy --workspace --all-targets -- -D warnings` — clean, zero warnings

## Review Findings (2026-05-16 21:05)

Reviewed the `swissarmyhammer-workspace-init` crate, the CLI migration, and the
kanban-app wiring. The Option-B approach is sound: components are genuinely
root-explicit (no `current_dir()` / git-root anywhere in the new crate), init
is genuinely idempotent (`from_custom_root` + `create_dir_all` no-ops; `write_skill`
clears the skill dir before recreating), the CLI delegates rather than forks
`.sah/` structure logic, and the Claude-Code `deny-bash`/`statusline` bits are
correctly excluded. Build, the new crate's 11 tests, the 3 kanban-app
integration tests, and clippy were all re-run locally and pass.

### Warnings
- [x] `apps/kanban-app/tests/workspace_init.rs:30-123` — All three integration tests call `run_workspace_init` directly; none exercise the actual production wiring (`ensure_sah_workspace` invoked inside `BoardHandle::open`, including the `kanban_path.parent()` board-folder math). If `ensure_sah_workspace` were removed from `BoardHandle::open`, or the `.parent()` path logic regressed, every test in this file would still pass. The acceptance criterion "opening a board folder makes it a SAH workspace" is verified only at the library layer. Add at least one test that opens a board via `AppState::open_board` (or `BoardHandle::open`) against a temp board dir and asserts `<board>/.sah/skills/plan/SKILL.md` exists — `state.rs` already has a `create_board_at` helper and `open_board`-based tests to model it on.

#### Resolution (2026-05-16)
`kanban-app` is a binary-only crate (`[[bin]]`, no `lib.rs`); `state.rs` is a
private `mod` and `AppState` / `BoardHandle` are `pub(crate)`, so an external
integration test under `tests/` structurally cannot reach `open_board`. That is
exactly why the existing `tests/workspace_init.rs` tests call the public
`run_workspace_init` dependency API. The production-wiring test must therefore
live in the in-crate `state.rs::tests` module — the only place that can drive
`AppState::open_board` → `BoardHandle::open` → `ensure_sah_workspace` (and the
finding itself points there: "`state.rs` already has a `create_board_at` helper
and `open_board`-based tests to model it on").

Added `state::tests::test_open_board_creates_sah_workspace_at_board_folder`: it
calls `AppState::open_board(tmp.path(), None)` against a fresh temp board folder
and asserts `<board>/.sah/skills/plan/SKILL.md` is a file, plus that `.sah/` is
NOT nested inside `.kanban/`. This drives the real production path with no
direct `run_workspace_init` call. Proven to genuinely fail on regression:
commenting out `ensure_sah_workspace` in `BoardHandle::open` → test fails
(`.sah/` missing); changing the `kanban_path.parent()` math to root at
`kanban_path` itself → test fails (`.sah/` lands inside `.kanban/`).
`cargo build -p kanban-app`, `cargo test -p kanban-app` (lib unit tests 102→103,
all integration binaries green), and `cargo clippy -p kanban-app --all-targets
-- -D warnings` all clean.