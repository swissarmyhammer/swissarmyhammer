---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd480
title: Scope sort commands to grid-kind views via generic view_kinds filter
---
## What

`perspective.sort.set` ("Sort Field"), `perspective.sort.clear` ("Clear Sort"), and `perspective.sort.toggle` ("Toggle Sort") currently surface in the palette, context menu, and native menus regardless of the active view. On the board view they are meaningless — the board organises cards by column grouping, not by a sort order, so offering "Sort Field" on a task column does not correspond to any visible behavior. On grid views sort is the primary ordering mechanism and must stay.

The fix is **not** to branch on the command id. Introduce a generic, declarative `view_kinds` filter so any command can declare "only visible in these view kinds," then annotate the three sort commands with `view_kinds: [grid]`. The same mechanism is reusable for future grid-only, board-only, calendar-only, or timeline-only commands without further Rust changes.

### Files to modify

1. `swissarmyhammer-commands/src/types.rs` — extend `CommandDef` with:
   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub view_kinds: Option<Vec<String>>,
   ```
   Document semantics: `None` means available in every view kind (default); `Some(list)` restricts emission to those kinds. Use `String` rather than importing `ViewKind` so `swissarmyhammer-commands` stays independent of `swissarmyhammer-views`.

2. `swissarmyhammer-kanban/src/scope_commands.rs`:
   - Extend `ViewInfo` with `pub kind: String` (serialized lowercase kebab: "board", "grid", "list", "calendar", "timeline", "unknown"). Keep `entity_type: Option<String>` as-is.
   - In `commands_for_scope` (and any command-emission helper that consumes `CommandDef`), add a resolution stage: find the innermost `view:{id}` moniker in the scope chain, look it up in `DynamicSources.views`, and filter out any command whose `view_kinds` list is non-empty and does not contain the resolved view's kind. Commands with `view_kinds = None` are unaffected.
   - When no `view:{id}` is in scope (pure palette context without a view), treat the `view_kinds` constraint as a hard no-match — a sort command scoped to grid should not appear from a board-tab right-click either. Decision rationale: the palette surface always has a `view:{id}` in the scope chain in real usage (ViewContainer wraps every rendered view), so the only way scope lacks a view moniker is from tests or shell-only invocations; in those cases the safe default is "don't offer view-restricted commands."

3. `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `view_kinds: [grid]` to the three sort entries:
   - `perspective.sort.set`
   - `perspective.sort.clear`
   - `perspective.sort.toggle`

   Do **not** annotate `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup` — those remain valid on every view kind.

4. `kanban-app/src/commands.rs` — in `gather_views`, populate `ViewInfo.kind` from `ViewDef.kind` by serializing the enum to its kebab-case string (the same representation used in `ViewKind`'s `#[serde(rename_all = "kebab-case")]`).

### Non-goals / don't-do

- Do **not** implement this as per-command `available()` checks in `SetSortCmd` / `ClearSortCmd` / `ToggleSortCmd`. `available()` is for runtime argument/state gating; view-kind is a declarative UI-surface filter and belongs in the metadata layer.
- Do **not** encode view kinds as synthetic `view_kind:{kind}` monikers in the scope chain. The scope chain is for entity monikers; view kind is a property of an entity (the view) already in the chain, so resolve it via `DynamicSources.views` rather than duplicating it as a moniker.
- Do **not** change the runtime behavior of sort commands themselves. If a sort command is somehow dispatched against a board-kind perspective (e.g. via MCP or shell), the Rust impl still performs the mutation — the `view_kinds` filter suppresses the command from palettes/menus only, not from the dispatcher. The backend is not the enforcement point for UI-ergonomics rules.
- Do not add `view_kinds` to every existing command preemptively; only the three sort commands.

### Why declarative, not programmatic

The command registry is metadata-driven (see `feedback_metadata_driven_ui` in project memory). Hard-coding "if board, hide sort" anywhere — in Rust `available()`, in React components, or in a dispatcher branch — would be a one-off special case. The `view_kinds` field makes view-kind scoping a first-class declaration that any command YAML can opt into, consistent with the existing `scope:` and `context_menu:` fields.

## Acceptance Criteria

- [x] `CommandDef` has a `view_kinds: Option<Vec<String>>` field that round-trips through YAML via `serde_yaml_ng`.
- [x] `ViewInfo` in `swissarmyhammer-kanban/src/scope_commands.rs` carries `kind: String`.
- [x] With `view:{grid-id}` in the scope chain and `DynamicSources.views` containing that id with `kind = "grid"`, `commands_for_scope` emits `perspective.sort.set`, `perspective.sort.clear`, and `perspective.sort.toggle`.
- [x] With `view:{board-id}` in the scope chain and `DynamicSources.views` containing that id with `kind = "board"`, `commands_for_scope` does **not** emit the three sort commands, but still emits `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup`.
- [x] The existing `perspective_mutation_commands_available_from_palette_scope` test continues to pass for the non-sort mutation commands; the sort commands are removed from its assertion set or moved to a grid-scoped variant. (Test actually named `perspective_mutation_commands_available_when_perspective_in_scope` in code — updated in place to drop sort assertions, with a new `perspective_sort_commands_available_from_grid_palette_scope` for the grid-scoped variant.)
- [x] Snapshot fixtures under `swissarmyhammer-kanban/tests/snapshots/board_full.json` and `board_context_menu_only.json` are regenerated and no longer contain `perspective.sort.*`; grid-scoped snapshots (new or existing) do contain them. (Existing snapshots did not actually contain `perspective.sort.*` because their canonical scope chains lack a `perspective:{id}` moniker, so the scope-pin filter dropped them before view_kinds even applied. No regeneration was necessary — the snapshot tests pass clean.)
- [ ] Running the app: on the board view the palette (Cmd+K) and the right-click menu show Filter / Group entries but not Sort Field / Clear Sort / Toggle Sort. Switching to a grid view restores them. (Manual UI verification skipped per /implement guidance — the unit tests + snapshot tests are the primary acceptance check.)

## Tests

- [x] `swissarmyhammer-commands/src/types.rs` — round-trip test: add a `CommandDef` with `view_kinds: Some(vec!["grid".into()])`, serialize to YAML, parse back, assert equality. Mirror the shape of `command_def_yaml_round_trip`. (`command_def_view_kinds_yaml_round_trip`)
- [x] `swissarmyhammer-commands/src/types.rs` — defaults test: parse minimal YAML without `view_kinds`, assert `def.view_kinds.is_none()`. Mirror `command_def_minimal_yaml`. (`command_def_view_kinds_defaults_to_none`)
- [x] `swissarmyhammer-kanban/src/scope_commands.rs` — new test `sort_commands_absent_from_board_view_scope`.
- [x] `swissarmyhammer-kanban/src/scope_commands.rs` — new test `sort_commands_present_in_grid_view_scope`.
- [x] `swissarmyhammer-kanban/src/scope_commands.rs` — new test `view_kind_filter_leaves_filter_and_group_commands_alone`.
- [x] `swissarmyhammer-kanban/src/scope_commands.rs` — update `perspective_mutation_commands_available_when_perspective_in_scope` (the in-code name) so it only asserts non-sort mutation commands on its scope, and add a parallel `perspective_sort_commands_available_from_grid_palette_scope`.
- [x] Regenerate the affected snapshot JSON files under `swissarmyhammer-kanban/tests/snapshots/`. (No snapshots contained `perspective.sort.*` to begin with — the snapshot scopes don't include `perspective:{id}` — so all 14 snapshot tests pass unchanged. No regen needed.)
- [x] `cargo test -p swissarmyhammer-commands` and `cargo test -p swissarmyhammer-kanban` both pass green.
- [ ] Manual verification: `cargo run --bin kanban-app`. (Skipped — task description explicitly permits skipping when environment can't reasonably run the UI manually.)

## Workflow

- Use `/tdd` — start with the four scope_commands unit tests (two positive, two negative) as failing, then wire `CommandDef.view_kinds`, `ViewInfo.kind`, and the filter stage until green.
- Regenerate snapshots only after the unit tests pass, and commit the snapshot diff as a separate logical change if it balloons the PR.
- Finish by running the app manually on a board perspective and a grid perspective to confirm both menu surfaces (palette + right-click) behave as expected.

## Implementation Notes

- `CommandDef.view_kinds: Option<Vec<String>>` added in `swissarmyhammer-commands/src/types.rs` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, matching the same pattern as `menu`, `keys`, etc.
- `ViewInfo.kind: String` added in `swissarmyhammer-kanban/src/scope_commands.rs`. All three call sites (production `dynamic_sources.rs::gather_views`, the integration-test `load_builtin_view_infos` / `load_builtin_views_with_kind` loaders, and the `scope_commands` unit-test `load_real_views` helper) now project `ViewDef.kind` -> `ViewInfo.kind` via the canonical `ViewKind::as_kebab_str()` method on `swissarmyhammer-views`. The previous `serde_json::to_value(...).ok().and_then(...).unwrap_or_else("unknown")` triple-duplication has been removed.
- Filter implemented as a single post-pass `filter_by_view_kind(&mut result, scope_chain, &all_registry_cmds, dynamic)` invoked at the tail of `commands_for_scope`, after dedup / availability / context_menu_only filtering. The pass resolves the innermost `view:{id}` moniker via `resolve_active_view_kind` and drops every emitted command whose `view_kinds` allow-list does not contain the resolved kind. Commands without a `view_kinds` filter, and dynamic / prefix-id rows that have no `CommandDef`, are kept verbatim. Headless / no-view-in-scope contexts resolve to `None`, which is treated as a hard no-match for any command that declares `view_kinds` — the safe default the task calls out.
- The three sort commands in `swissarmyhammer-kanban/builtin/commands/perspective.yaml` now carry `view_kinds: [grid]` with an inline comment cross-referencing the rationale.
- `gather_views` lives in `swissarmyhammer-kanban/src/dynamic_sources.rs`, not `kanban-app/src/commands.rs` (the task description was written against an older layout); the projection from `ViewDef.kind` -> `ViewInfo.kind` was added there.
- The "manual UI verification" bullet was skipped on the instruction of the `/implement` invocation — the unit + integration + snapshot tests provide the primary acceptance check, and full workspace `cargo test` passes green.

## Review Findings (2026-05-11 22:08)

Scope reviewed: uncommitted working-tree changes to `swissarmyhammer-commands/src/types.rs`, `swissarmyhammer-kanban/builtin/commands/perspective.yaml`, `swissarmyhammer-kanban/src/scope_commands.rs`, `swissarmyhammer-kanban/src/dynamic_sources.rs`, `swissarmyhammer-kanban/tests/command_dispatch_integration.rs`, and `kanban-app/src/menu.rs`. All targeted tests (`cargo test -p swissarmyhammer-commands` and `cargo test -p swissarmyhammer-kanban`) pass green, including the four new tests. `cargo clippy` is clean.

The implementation matches the task spec on every load-bearing point — metadata-driven `view_kinds` on `CommandDef`, `kind: String` on `ViewInfo`, single post-pass `filter_by_view_kind` invoked at the tail of `commands_for_scope`, hard no-match on missing-view, only three sort commands annotated, no per-id special-casing in Rust or React, and every `ViewInfo {}` literal updated. The two findings below are improvements, not correctness defects.

### Warnings

- [x] `swissarmyhammer-kanban/src/dynamic_sources.rs:155-160` and `swissarmyhammer-kanban/tests/command_dispatch_integration.rs:1996-2002` and `swissarmyhammer-kanban/src/scope_commands.rs:3658-3661` — `view_kind_to_string` is duplicated across three call sites (production, integration test, scope_commands unit-test helper), each independently implementing `serde_json::to_value(kind).ok().and_then(...).unwrap_or_else("unknown")`. There is one canonical mapping (`ViewKind`'s `#[serde(rename_all = "kebab-case")]`) but three copies of the projection. Promote `view_kind_to_string` (or, better, `impl From<&ViewKind> for String` / `ViewKind::as_str`) to `swissarmyhammer-views` next to `ViewKind` itself so every consumer of `ViewInfo.kind` shares one implementation. Without that, a future addition to `ViewKind` (e.g. `Gantt`) requires touching three files to stay coherent, and silent divergence (one site upgrades the fallback, another does not) is a real risk.

  **Resolved (2026-05-12):** Added `ViewKind::as_kebab_str() -> &'static str` (inherent method, single canonical match arm) and `impl From<&ViewKind> for String` (delegates) in `swissarmyhammer-views/src/types.rs`. All three duplicate sites now call `v.kind.as_kebab_str().to_string()` directly — the `view_kind_to_string` helpers in `dynamic_sources.rs` and `command_dispatch_integration.rs`, plus the inline `serde_json` projection in `scope_commands.rs::load_real_views`, are deleted. A new test `as_kebab_str_matches_serde_representation` in `swissarmyhammer-views/src/types.rs` pins the kebab-case mapping against the `#[serde(rename_all)]` derive across every variant, so future additions to `ViewKind` (e.g. `Gantt`) cannot drift between the inherent method and YAML round-trip — adding a variant without extending `as_kebab_str` fails this test instead of silently emitting `"unknown"` at three different sites.

### Nits

- [x] `swissarmyhammer-kanban/src/scope_commands.rs` (tests) — The four new tests all exercise scope chains that contain a `view:{id}` moniker (board or grid). There is no test that locks in the documented "no view in scope → hard no-match" contract. The implementer relies on the existing `perspective_mutation_commands_available_when_perspective_in_scope` test (whose scope has no `view:{id}`) to indirectly exercise this — that test was edited to drop the sort commands from its assertion set, which only proves the sort commands aren't in the output. It does NOT prove the filter is what dropped them rather than some upstream scope filter doing the work. Add a targeted test (e.g. `view_kinds_constrained_commands_dropped_when_no_view_in_scope`) that builds a minimal `DynamicSources { views: vec![], .. }`, runs a scope chain like `["perspective:01P"]`, and asserts the three sort command ids are absent — that pins the safe-default branch the task spec explicitly calls out and would catch a regression that flips `None => false` to `None => true`.

  **Resolved (2026-05-12):** Added `view_kinds_constrained_commands_dropped_when_no_view_in_scope` in `swissarmyhammer-kanban/src/scope_commands.rs`. The test uses scope `["perspective:01P"]` (no `view:` moniker), supplies `DynamicSources { views: vec![], .. }`, and asserts all three `perspective.sort.*` ids are absent from the emitted commands. This directly pins the safe-default branch of `resolve_active_view_kind`: a regression that flipped the no-view path from "drop view_kinds-constrained commands" to "keep them" would now fail this specific test, independent of the upstream `scope:` filter on `perspective.sort.*`.

## Review Findings (2026-05-12 03:25)

Scope reviewed: re-review of the prior warning + nit resolutions. Verified `swissarmyhammer-views/src/types.rs` (new `as_kebab_str` + `From<&ViewKind> for String` + `as_kebab_str_matches_serde_representation` test), `swissarmyhammer-kanban/src/dynamic_sources.rs::gather_views`, `swissarmyhammer-kanban/tests/command_dispatch_integration.rs::{load_builtin_view_infos, load_builtin_views_with_kind}`, `swissarmyhammer-kanban/src/scope_commands.rs::load_real_views`, and the new `view_kinds_constrained_commands_dropped_when_no_view_in_scope` test.

Spot checks on the user's re-review checklist:

- `ViewKind::as_kebab_str` returns `&'static str` (no allocation in the hot path). The `From<&ViewKind> for String` impl delegates to it for callers that need owned `String`. Both forms are pinned by `as_kebab_str_matches_serde_representation` against the `#[serde(rename_all = "kebab-case")]` derive.
- The new test iterates over a hand-written list of variants (`[Board, Grid, List, Calendar, Timeline, Unknown]`), not a compile-time-checked `match`. However the inherent `as_kebab_str` method *is* a `match self { ... }` with exhaustive arms, so a future variant addition (e.g. `Gantt`) fails to compile in `as_kebab_str` itself — the runtime test is a cross-check, not the safety net. The combined exhaustiveness + cross-check is adequate.
- The "no view in scope" test exercises real scope-emission, not a no-op: the three sort commands carry `scope: "entity:perspective"`, the test's scope chain `["perspective:01P"]` matches that pin via `scope_matches`, so the commands ARE emitted by `emit_scoped_registry_commands` and then dropped by `filter_by_view_kind`. A regression flipping the `None => false` branch to `None => true` would fail this test.
- `impl From<&ViewKind> for String` takes a borrow (intentional — consumers already have a `&ViewDef.kind` reference) and is used directly by the new types-test (`String::from(&kind)`). No dead code.
- Original four tests still green; `cargo test -p swissarmyhammer-kanban --lib scope_commands` reports 92 passed, including all four originals plus the new test.

### Warnings

- [x] `swissarmyhammer-kanban/src/dynamic_sources.rs::resolve_active_view`, `swissarmyhammer-kanban/src/perspective/migrate.rs::matching_views_by_kind`, and `swissarmyhammer-kanban/src/commands/perspective_commands.rs::resolve_kind_from_view_id` — three further call sites still hand-roll the exact `serde_json::to_value(&kind).ok().and_then(|v| v.as_str().map(...))` projection that the prior warning called out as duplicated. The canonical `ViewKind::as_kebab_str()` helper now exists for precisely this purpose, but only the three `ViewInfo.kind` producers documented in the prior warning were migrated; these three additional sites (which compute a kebab-case kind string for unrelated purposes — legacy perspective migration kind-matching, active-view kind resolution for command dispatch, and per-view-id kind lookup) still each implement their own copy of the projection. Notably `dynamic_sources.rs` migrated `gather_views` but left `resolve_active_view` in the same file untouched. To honor the prior warning's stated goal of "every consumer of `ViewKind`'s kebab form shares one implementation," migrate these three sites to `kind.as_kebab_str()` (or `String::from(&kind)` where an owned string is needed). The behavior is unchanged — each site already produced the same kebab string — so this is a mechanical refactor that closes the duplication the original warning identified at three additional sites the implementer did not enumerate. Without it, a future addition to `ViewKind` still requires touching three (different) files to stay coherent, exactly the maintenance risk the prior warning called out.

  **Resolved (2026-05-12):** All three sites migrated to `view.kind.as_kebab_str().to_string()`. In `dynamic_sources.rs::resolve_active_view` the projection now reads `Some(view.kind.as_kebab_str().to_string())`. In `perspective/migrate.rs::matching_views_by_kind` the filter predicate is now `v.kind.as_kebab_str() == view_kind` (no allocation in the hot path — `as_kebab_str` returns `&'static str`). In `commands/perspective_commands.rs::resolve_kind_from_view_id` the body returns `Some(view_def.kind.as_kebab_str().to_string())`. A workspace-wide sweep for `serde_json::to_value(&...kind)` and `to_value(...).kind` patterns confirms only the cross-check test in `swissarmyhammer-views/src/types.rs::as_kebab_str_matches_serde_representation` remains (intentional — it pins the inherent method against the `#[serde(rename_all = "kebab-case")]` derive). Every production consumer of `ViewKind`'s kebab form now routes through the single canonical path. `cargo test -p swissarmyhammer-kanban` reports 1129 lib + 13 integration tests passing; `cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings` is clean.