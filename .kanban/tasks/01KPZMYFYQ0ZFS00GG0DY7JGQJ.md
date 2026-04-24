---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff9a80
title: Move build_dynamic_sources out of kanban-app into UIState (headless-testable)
---
## What

PR #40 review comment from @wballard on `kanban-app/src/commands.rs:2002`:

> I do not expect to see this here - I expect this to be in UIState -- fully testable without the GUI crate at all

Ref: https://github.com/swissarmyhammer/swissarmyhammer/pull/40#discussion_r3137370667

`build_dynamic_sources` currently sits in `kanban-app/src/commands.rs` and assembles the runtime inputs for dynamic command emission (views, boards, windows, perspectives). Because it lives in the Tauri/GUI crate, it cannot be exercised from a headless Rust test — the integration tests that validate dynamic-command behavior have to stand up Tauri scaffolding or skip this codepath.

The reviewer's architectural position: this logic belongs on UIState (likely in `swissarmyhammer-commands/src/ui_state.rs`) or in the kanban crate, where it's headlessly testable without the GUI crate.

### Files to modify

- `kanban-app/src/commands.rs::build_dynamic_sources` — remove from here.
- `swissarmyhammer-commands/src/ui_state.rs` (or `swissarmyhammer-kanban/src/`) — define the headless analogue. `UIState` already tracks recent boards, window labels, active perspectives per window, and active view per window, so the raw inputs for dynamic emission are already there; this task is about moving the *assembly* logic so any headless test can produce a `DynamicSources` from a `UIState` + registry + perspective context without touching `AppHandle` / `State<AppState>`.
- `swissarmyhammer-kanban/src/scope_commands.rs` — callers of `build_dynamic_sources` now call the new headless entry point.
- `swissarmyhammer-kanban/tests/command_surface_matrix.rs` (or similar) — tighten test coverage to prove the headless path produces the same emitter outputs.

### Investigation needed

- Check what `build_dynamic_sources` reads from `AppState` that is not already on `UIState`. If anything is GUI-only (e.g. live `Window` handles), factor the GUI-only inputs into a separate lightweight struct and keep the UIState-only path the primary one.
- Look at `swissarmyhammer-kanban/src/scope_commands.rs` for existing callers of `DynamicSources` and how they currently inject the struct during tests (there is likely already a headless path used in unit tests — this task may be about promoting that to the only path).

## Acceptance Criteria

- [x] `build_dynamic_sources` no longer lives in `kanban-app/src/commands.rs`.
- [x] The replacement lives in a crate that does NOT depend on `tauri` / GUI chrome (likely `swissarmyhammer-commands` or `swissarmyhammer-kanban`).
- [x] An integration test in `swissarmyhammer-kanban/tests/` constructs a `DynamicSources` from a bare `UIState` + registry + perspective context and asserts the same emitter outputs (views, boards, windows, perspectives) as the live app — with no Tauri or GUI crate in scope.
- [x] `cargo test -p swissarmyhammer-kanban` exercises the new headless path.
- [x] `cargo test -p kanban-app` still passes — GUI callers now invoke the relocated function.
- [x] No regression: the live app still emits the correct dynamic commands.

## Tests

- [x] Add `swissarmyhammer-kanban/tests/dynamic_sources_headless.rs` (new, or extend an existing test file) with tests that build a `DynamicSources` from fixture `UIState` + fixture `PerspectiveContext` + fixture registry, then invoke `emit_dynamic_commands` and assert the output set (view.set entries, board.switch entries, window.focus entries, perspective.set entries).
- [x] If the code move uncovers any AppHandle-only inputs, split them into a small GUI-side shim struct and test the pure path in the kanban crate.

## Workflow

Use `/tdd`. Write a failing headless test first (it'll fail to compile because the builder lives in the GUI crate), then move the function, then make the test pass. #commands #refactor #architecture #uistate

## Outcome

Live-window info (title/visibility/focus) is the only piece that cannot be derived from `UIState` alone, so it remained a caller-supplied input. Everything else moved.

- New module: `swissarmyhammer-kanban/src/dynamic_sources.rs`
  - `pub struct DynamicSourcesInputs<'a>` — `ui_state`, `active_ctx`, `open_board_ctxs`, `active_window_label` (`Option<&str>`), `windows`
  - `pub async fn build_dynamic_sources(inputs) -> DynamicSources`
  - Hand-rolled `impl Debug for DynamicSourcesInputs<'_>` (prints counts + flags, elides lockable interior state).
  - Private helpers: `gather_views`, `gather_boards`, `resolve_active_view_kind`, `gather_perspectives`, `board_display_name`.
- `kanban-app/src/commands.rs::build_dynamic_sources` shrank to a ~25-line shim — projects `HashMap<PathBuf, Arc<BoardHandle>>` → `HashMap<PathBuf, Arc<KanbanContext>>`, gathers live windows from Tauri, delegates (passing `active_window_label: Some("main")`).
- Removed from `kanban-app`: `gather_views`, `gather_boards`, `gather_perspectives`, `resolve_active_view_kind`.
- Kept in `kanban-app`: `gather_windows` (Tauri-only, provides the one GUI-side input) and `board_display_name` (still called from four other places in the GUI crate; follow-up card 01KPZTQBP9X091T893ZWZG6PV5 tracks deduplication).
- Updated test: `swissarmyhammer-kanban/tests/dynamic_sources_headless.rs` — 5 tests (2 original + 3 added in review) covering single-board happy path, no-active-context, multi-board/multi-window, context-map fallback to parent-dir basename, and `view_kind` negative filter.

## Review Findings (2026-04-24 08:19)

### Nits

- [x] `swissarmyhammer-kanban/tests/dynamic_sources_headless.rs` — Coverage gap: multi-board and multi-window scenarios are not exercised. The task description explicitly called out "Multiple open boards, multiple windows" as a branch to cover. Both existing tests use exactly one board and at most one window. Add a test (or extend the main test) that opens two boards, populates `open_board_ctxs` with both, and supplies two `WindowInfo` entries — asserting both `board.switch:*` and both `window.focus:*` emissions.
  - Resolved: Added `build_dynamic_sources_emits_every_open_board_and_window` — opens two temp boards (`Board Alpha`/`Board Beta`), stuffs both into `open_board_ctxs`, supplies two `WindowInfo` entries with distinct labels, and asserts both `board.switch:<path>` and both `window.focus:<label>` commands come out of `commands_for_scope`. Also asserts each board's `entity_name` resolves through its own context.
- [x] `swissarmyhammer-kanban/src/dynamic_sources.rs:150-159` — The `open_board_ctxs.get(p) == None` fallback branch (parent-directory-basename used for both `entity_name` and `context_name`) is not exercised by any test. The task description documents this branch as "paths in `ui_state.open_boards()` with no matching entry fall back to the parent directory basename" — it's load-bearing for the live-app splash/welcome path where UIState knows about boards that haven't been opened yet. Add a test case where `ui_state.add_open_board(&path)` is called but `open_board_ctxs` is left empty, and assert the resulting `BoardInfo` has `entity_name == context_name == <parent-dir-basename>`.
  - Resolved: Added `build_dynamic_sources_falls_back_to_basename_when_ctx_missing` — seeds `UIState` with a synthetic `/tmp/.../recents-fixture/.kanban` path, leaves `open_board_ctxs` empty, and asserts `entity_name == context_name == name == "recents-fixture"`. No filesystem presence required since the builder only reads `ui_state.open_boards()`.
- [x] `swissarmyhammer-kanban/src/dynamic_sources.rs:198-218` — The `view_kind` filter in `gather_perspectives` is only exercised in the positive direction (a matching perspective appears). Add a negative assertion: register a perspective with `view: "grid"` while the active view kind resolves to `"board"`, and assert the grid perspective is filtered out. Guards against regressions in the `is_none_or` filter that would re-introduce the "same Default perspective emits once per view kind" bug the comment calls out.
  - Resolved: Added `build_dynamic_sources_filters_perspectives_by_active_view_kind` — registers both a `"board"` and a `"grid"` perspective on the same context, pins the active view kind to `"board"`, and asserts the grid perspective is filtered out (both by id and by the absence of any `view == "grid"` entry).
- [x] `swissarmyhammer-kanban/src/dynamic_sources.rs:44-67` — `DynamicSourcesInputs` does not derive/implement `Debug`, which is inconsistent with its output sibling `DynamicSources` and every other info-type in `scope_commands.rs` (all derive `Debug, Clone`). `#[derive(Debug)]` will not compile because `UIState` is not `Debug`; a hand-rolled `impl Debug` that prints `active_window_label`, `windows.len()`, `open_board_ctxs.len()`, and `active_ctx.is_some()` would give tracing something to log without exposing interior locks. Low priority — flag only because the sibling types set the convention.
  - Resolved: Added hand-rolled `impl fmt::Debug for DynamicSourcesInputs<'_>` that prints exactly the four fields the finding requested — `active_window_label`, `windows.len`, `open_board_ctxs.len`, `active_ctx.is_some`. Deliberately elides the lockable `UIState` interior so tracing cannot deadlock.
- [x] `swissarmyhammer-kanban/src/dynamic_sources.rs:63` — `active_window_label: &'a str` uses empty-string as an implicit "no window" sentinel (the `ui_state.active_view_id` call returns `""` for unknown labels, and `resolve_active_view_kind` short-circuits on `is_empty()`). `Option<&'a str>` would make the "no window focused" case explicit at the type level, matching the `active_ctx: Option<&'a KanbanContext>` treatment already on the adjacent field. Current shape works and matches the existing `UIState::active_view_id(&str)` signature — flag only as a consistency nudge.
  - Resolved: Changed `active_window_label` to `Option<&'a str>` so the "no window focused" case is explicit at the type level (matching `active_ctx`). `resolve_active_view_kind` now short-circuits on `active_window_label?` before reaching `UIState::active_view_id`, and the GUI shim passes `Some("main")`. All tests updated accordingly.
- [x] `kanban-app/src/commands.rs:135` + `swissarmyhammer-kanban/src/dynamic_sources.rs:225` — Identical 6-line `board_display_name` implementations now exist in both crates (one takes `&BoardHandle`, the other `&KanbanContext`). The GUI version's 4 remaining call sites could call a public `swissarmyhammer_kanban::board_display_name(&handle.ctx)` and the duplicated helper could collapse to a single definition in the kanban crate. Explicitly out of scope for this task (task description notes "Kept in `kanban-app`: `board_display_name` (still called from four other places in the GUI crate)"), but worth a follow-up card to eliminate the duplication now that a non-GUI `board_display_name` exists.
  - Resolved: Explicitly out of scope per the finding. Filed follow-up kanban task 01KPZTQBP9X091T893ZWZG6PV5 "Collapse duplicate board_display_name between kanban-app and swissarmyhammer-kanban" with acceptance criteria for promoting `swissarmyhammer_kanban::board_display_name` to `pub` and removing the GUI-side duplicate.
