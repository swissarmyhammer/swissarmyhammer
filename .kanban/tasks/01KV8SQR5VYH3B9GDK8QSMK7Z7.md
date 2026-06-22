---
comments:
- actor: claude-code
  id: 01kvqvr5xg8yfnr5z3gk4nabw3
  text: |-
    Implemented via TDD. Backend: gave emit_perspective_goto a scope_chain param, computed in_scope by stripping the `perspective:` prefix (mirrors emit_view_switch), set context_menu = in_scope.contains(id), added perspective_switch_caption helper ("Switch to Perspective «name»", Untitled placeholder for blank), updated caller emit_dynamic_commands + module/fn doc-comments. Files: crates/swissarmyhammer-kanban/src/scope_commands.rs.

    Tests: updated unit test blank_perspective_names_get_the_untitled_placeholder_caption for new caption/signature; added unit test in_scope_perspective_gets_a_context_menu_switch_row_siblings_dont. Added two headless integration tests in tests/dynamic_sources_headless.rs (context_menu_perspective_switch_is_scoped_to_the_perspective_in_scope, no_context_menu_perspective_switch_row_without_a_perspective_in_scope) mirroring the existing view tests. Frontend: added regression test in perspective-tab-bar.context-menu.test.tsx asserting the in-scope tab surfaces "Switch to Perspective «name»" dispatching perspective.switch with perspective:p2 in the item scope_chain (backend resolves via ResolvedFrom::Scope). Comment-only updates to nav.ts and add-create-rename.test.tsx for caption accuracy.

    RED→GREEN verified (backend new test failed to compile against old 3-arg signature, then passed). NOTE: hit the known Cargo target concurrency / stale-rlib issue — the integration test linked a stale lib until I touched src to force a rebuild; after rebuild both unit + integration pass.

    Verification (in isolated CARGO_TARGET_DIR=target/iso-qsmk7z7):
    - cargo nextest run -p swissarmyhammer-kanban: 1405 passed, 0 failed
    - cargo clippy -p swissarmyhammer-kanban -- -D warnings: clean (EXIT 0). NOTE: `--all-targets` variant fails only on PRE-EXISTING clippy lints in dependency swissarmyhammer-focus (navigate.rs/state.rs), which I did not touch.
    - cargo fmt: clean
    - cd apps/kanban-app/ui && npx tsc --noEmit: clean
    - vitest browser perspective-tab-bar.context-menu.test.tsx + add-create-rename.test.tsx: 19 passed
    - Full vitest: 10 pre-existing failures (verified identical on clean tree via git stash; e.g. spatial-nav Enter-dispatches-perspective.switch fails the same way without my changes) — unrelated to this task.
  timestamp: 2026-06-22T14:29:02.000728+00:00
- actor: claude-code
  id: 01kvqwgzp1gp0dpccpcpg2pckz
  text: |-
    Addressed all 5 Review Findings (test-file style/clarity cleanups only; core scope_commands.rs logic untouched).

    Changes:
    - perspective-tab-bar.context-menu.test.tsx:
      - Added `interface ViewBodyWithContextMenuProps { testId: string; }` and used it on `ViewBodyWithContextMenu` (was inline object type).
      - Added module-scope `const TOOLTIP_DELAY_MS = 100;` and used it for both `TooltipProvider delayDuration` occurrences.
      - Added module-scope `const CONTEXT_MENU_SYNC_DELAY_MS = 20;` and used it for all 7 `setTimeout(r, 20)` sync delays.
    - perspective-tab-bar.add-create-rename.test.tsx:
      - Added module-scope `const TOOLTIP_DELAY_MS = 100;` and used it for the `TooltipProvider delayDuration`.

    Used per-file module-scope constants (not a shared test-utils file) — there is no existing shared test-utils module for these perspective-tab-bar tests, so a shared file would be over-engineering for two literals.

    Gates (all green):
    - `npx tsc --noEmit` -> exit 0, clean.
    - `npx vitest run --project browser <both files>` -> Test Files 2 passed (2), Tests 19 passed (19), exit 0.
    - `npx prettier --write <both files>` -> both reported "unchanged" (already formatted).

    Moving back to review.
  timestamp: 2026-06-22T14:42:34.817773+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffda80
project: ui-command-cleanup
title: 'Perspective ("view") tabs: no "Switch to <perspective>" in context menu or palette — mirror the view-switch fix'
---
## What

The perspective tab-bar buttons (the "view buttons" at the top of the board that switch which saved view you're looking at) have **no right-click context menu to switch to a perspective**, and the per-perspective switch rows are **not findable as "Switch to …" in the command palette**.

This is the exact symmetric gap that was already closed for **views** but never applied to **perspectives**. The fix is to make `emit_perspective_goto` mirror `emit_view_switch`.

### Background — the view path is the precedent (already done)

In `crates/swissarmyhammer-kanban/src/scope_commands.rs`:
- `emit_view_switch` (lines 312-344) emits one `view.set` row per view for the palette, and flips the row whose `view:{id}` moniker is in the right-click `scope_chain` to `context_menu: true` (card `01KV5K29FFQJTBER6HYA4J2DW6`). Caption: `"Switch to View «name»"` via `view_switch_caption` (lines 274-283).
- `emit_perspective_goto` (lines 446-477) emits one `perspective.switch` row per perspective but **hardcodes `context_menu: false` for every row** (line 468) with **no `scope_chain` parameter and no in-scope detection**, and uses the caption `"Go to Perspective: «name»"` (line 464) — so a user searching the palette for "Switch to" finds nothing.

The frontend is already fully wired (no frontend change needed for the context menu to work once the backend marks a row): the perspective tab mounts `<CommandScopeProvider moniker={moniker("perspective", perspective.id)}>` → `perspective:{id}` (`apps/kanban-app/ui/src/components/perspective-tab-bar.tsx:1107`) and the tab button has `onContextMenu={handleContextMenu}` (line 1608, `useContextMenu` at 1577). The right-click scope chain therefore already carries `perspective:{id}`; `context_menu_only` filtering in `commands_for_scope` (scope_commands.rs:669-671) drops the perspective rows today purely because they are all `context_menu: false`.

This aligns with the `ui-command-cleanup` model: the per-perspective switch commands stay **backend-defined** (in `emit_perspective_goto`); the UI only renders/dispatches them.

### Changes — all in `crates/swissarmyhammer-kanban/src/scope_commands.rs`

- [x] Give `emit_perspective_goto` a `scope_chain: &[String]` parameter and compute `in_scope: HashSet<&str>` from monikers stripped of the `perspective:` prefix — copy the pattern at `emit_view_switch` lines 318-323.
- [x] Set `context_menu: in_scope.contains(perspective.id.as_str())` instead of the hardcoded `false` (line 468), so right-clicking perspective X surfaces exactly its own "Switch to Perspective «X»" row and siblings stay palette-only — identical to the view design (lines 285-301 doc).
- [x] Rename the caption from `"Go to Perspective: «name»"` to `"Switch to Perspective «name»"` for palette findability and symmetry with `view_switch_caption`. Add a `perspective_switch_caption(name)` helper mirroring `view_switch_caption` (lines 274-283), keeping the `BLANK_PERSPECTIVE_NAME_PLACEHOLDER` ("Untitled") behavior for blank names (lines 451-458).
- [x] Update the caller `emit_dynamic_commands` (line 606) to pass `scope_chain` into `emit_perspective_goto`.
- [x] Update the existing unit test `blank_perspective_names_get_the_untitled_placeholder_caption` (lines 1460-1487) for the new caption text, and update the module doc-comment at lines 31-36 / 570-586 which describes perspective rows as "Go to Perspective" / palette-only.

## Acceptance Criteria
- [x] Right-clicking a perspective tab shows a "Switch to Perspective «name»" entry that dispatches `perspective.switch` with `args.perspective_id` for that tab, and switches the active perspective.
- [x] Sibling perspectives are NOT in the right-click menu (only the in-scope tab's own switch row is `context_menu: true`), matching view behavior.
- [x] The command palette lists "Switch to Perspective «name»" for every perspective (one `perspective.switch` row each, palette-findable under "switch").
- [x] Blank-named perspectives render the "Untitled" placeholder in both surfaces.
- [x] No regression: views still behave exactly as before; existing perspective/view/palette/context-menu tests stay green.

## Tests
- [x] **Backend unit test** in `crates/swissarmyhammer-kanban/src/scope_commands.rs` `mod tests`: add a test (mirroring `blank_perspective_names_get_the_untitled_placeholder_caption`) that calls `emit_perspective_goto` with a `scope_chain` containing `perspective:01P2` and asserts (a) every perspective gets a row with the "Switch to Perspective …" caption, (b) only the `01P2` row has `context_menu == true`, (c) all others have `context_menu == false`. Add an assertion that with an empty `scope_chain` no row is `context_menu: true` (palette-only).
- [x] **Frontend test** — extend `apps/kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx` (real `useContextMenu` → scope-chain → `list command`): seed `mockRegistry` with a `perspective.switch` row marked `context_menu: true` (and `args.perspective_id`), right-click a perspective tab, and assert the "Switch to Perspective «name»" item renders and dispatches `perspective.switch` with the correct `perspective_id`. This is the regression guard that fails before the backend fix (today no perspective row is ever `context_menu: true`).
- [x] Run `cargo test -p swissarmyhammer-kanban scope_commands` — green.
- [x] Run `cd apps/kanban-app/ui && npm test` (`tsc --noEmit && vitest run`) — both `unit` and `browser` projects green.

## Notes
- Terminology: the user's "view buttons" are the perspective tabs. True **views** (`view.set`, LeftNav `ViewButton`) already have this exact context-menu + palette behavior — this task brings perspectives to parity.
- Related but distinct: `^yrdj19h` (emit_view_switch double-definition). Not a dependency.

## Workflow
- Use `/tdd` — write the failing backend unit test (in-scope `context_menu: true`) and the failing frontend context-menu test first, watch them fail against the current hardcoded `context_menu: false`, then implement the `emit_perspective_goto` change to make them pass. #bug

## Review Findings (2026-06-22 09:32)

### Warnings
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx:425` — Component `ViewBodyWithContextMenu` uses inline object type for props instead of a named interface. Named prop interfaces are required for every component, even those with few props, as they serve as documentation. Add a named interface `interface ViewBodyWithContextMenuProps { testId: string; }` above the component definition and use it: `function ViewBodyWithContextMenu({ testId }: ViewBodyWithContextMenuProps)`.

### Nits
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.add-create-rename.test.tsx:200` — Magic number 100 configures tooltip delay (milliseconds). Should be extracted to a named constant to clarify intent and enable reuse across tests. Extract to named constant: `const TOOLTIP_DELAY_MS = 100;`.
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx:98` — Magic number 100 configures tooltip delay (milliseconds). Duplicates same value in the other test file; should be shared. Extract to named constant in a shared test utilities file: `const TOOLTIP_DELAY_MS = 100;`.
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx:190` — Magic number 20 configures test synchronization delay (milliseconds) for context menu timing. Appears multiple times; should be a named constant. Extract to named constant: `const CONTEXT_MENU_SYNC_DELAY_MS = 20;` at module scope.
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.context-menu.test.tsx:237` — Magic number 20 repeats (see line 190). Multiple occurrences should consolidate via named constant to reduce duplication. Use shared constant: `const CONTEXT_MENU_SYNC_DELAY_MS = 20;`.