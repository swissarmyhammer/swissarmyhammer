---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa680
title: 'Perspective + button: create immediately with a generated name and focus inline rename — no popup; placeholder text for blank names'
---
## What

UX directive from the owner, two parts on the perspective tab bar:

1. **The + button is bad UX** ("popups and buttons, sheesh"): clicking + currently opens a popup with buttons. Instead, **+ should immediately create a perspective** with a generated name (e.g. "Untitled" / "Untitled 2" — check the codebase for an existing generated-name convention before inventing one) **and focus the name field in inline-rename mode** so the user can type the real name right away. Same interaction class as the existing tab inline-rename (ScopedPerspectiveTab's Enter-rename path — reuse that machinery, don't build new).
2. **Blank-name placeholder**: if a perspective ends up with a blank/empty name (e.g. user cleared it during rename and committed), the tab must render placeholder text (e.g. "Untitled" styled as placeholder — match how other blank-value placeholders render in the app, check Field display conventions) — never an invisible/zero-width tab.

## Design constraints

- The CREATE is durable → dispatches through the existing perspective.save command path (commands-in-rust; the ensure/save semantics just landed in card 01KTY6T1GPY94VYWANE9X41SKJ — the + create is NOT if_absent, it's an explicit new perspective).
- Inline rename arming after create: the new tab must mount, get focus, and arm rename — mind the async create→event→render loop (StoreContext command → event → UI; wait for the entity to appear rather than racing it).
- Name generation: dedupe against existing perspective names for the active view scope ("Untitled", "Untitled 2", ...).
- Blank-name placeholder is PRESENTATION (metadata-driven-ui): render-side only, the stored name stays blank unless the user types one. Decide + document whether committing an EMPTY rename is allowed (placeholder makes it survivable) or reverts to the prior name — check what the current rename commit does with empty input and keep/improve consistently.
- Remove the popup path for + entirely (no dead code left).

## Acceptance Criteria
- [x] Click + → new perspective appears immediately in the bar with a generated unique name, its name field focused in rename mode; typing + Enter commits the name
- [x] Escape during that first rename: decide + pin the semantics (keep generated name vs delete the just-created perspective — pick the least surprising; document)
- [x] No popup remains in the + flow
- [x] A blank-named perspective renders a visible placeholder in the tab (and anywhere else the name displays)
- [x] Create dispatches through the command service (no client-side store writes)

## Tests
- [x] vitest: + click dispatches the create command with a generated deduped name; on the entity appearing, rename arms (focused input)
- [x] vitest: blank name renders the placeholder; non-blank renders the name
- [x] vitest: Escape-during-first-rename pinned semantics
- [x] tsc clean; touched suites green

## Constraints
- NO whole-workspace cargo build/clippy. Never touch .kanban/actors/wballard.jsonl.
- Reuse ScopedPerspectiveTab's rename machinery and the established command dispatch — no new frameworks.

## Workflow
- /tdd — red-first per behavior.

## Resolution notes (implemented 2026-06-12)

**Escape semantics (pinned):** Escape during the first rename cancels the EDIT only — the perspective keeps its generated name ("Untitled"). Rationale: the entity was durably created on click; deleting a just-created perspective on Escape would be surprising, and no other rename in the app deletes on cancel. Pinned by `escape_keeps_the_generated_name_and_deletes_nothing` in `perspective-tab-bar.add-create-rename.test.tsx`.

**Empty-rename commit:** unchanged — `commitRename` skips when the trimmed input is empty (keeps the prior name). The blank-name placeholder covers blank names arriving from any other entry point (presentation only; stored name stays blank).

**Generated-name convention (reworked 2026-06-12, see B1/N1 below):** the frontend `generateUntitledName` and the backend `first_free_untitled_name` (perspective_commands.rs) are cross-language mirrors of ONE convention: scan for the first free "Untitled" / "Untitled N" slot by EXACT name match. Pinned by lockstep table tests on both sides.

**Create wire shape:** `perspective.save` with `{ name, view: <kind>, view_id: <active view instance> }` — explicit create, never `if_absent` (ensure stays reserved for the auto-create-Default guard). The dispatch result now carries the created entity (`{ ok, perspective: { id, … } }`, unwrapped in the plugin via `unwrapResult`); rename arms on that ID.

## Review Findings (2026-06-12 15:40)

### Blockers
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx:216` — `generateUntitledName` is count-based, not uniqueness-based, so it can generate a name that ALREADY exists among the visible perspectives, and the name-matching arrival watcher (`useArmRenameOnArrival`, line 310) then fires immediately on the PRE-EXISTING tab. **FIXED (both halves):** (1) generation is now a set-based first-free-slot scan by exact name match; (2) arming is now BY ID — the plugin's `perspective.save` unwraps the views envelope (`unwrapResult`, the `perspective.list` precedent), `createPerspective` reads `result.perspective.id` off the dispatch result, and the arrival watcher matches that id. Name matching and the "scan from the end" heuristic are deleted. Red-first test: `colliding_visible_names_never_arm_rename_on_a_preexisting_tab` (failed RED with "Untitled 2" regenerated + arming on the pre-existing tab).
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx:1402` — `AddPerspectiveCommandButton`'s render shell is the THIRD near-verbatim copy of the tab-button shell. **FIXED:** extracted shared presentational `<TabIconButton>` (`apps/kanban-app/ui/src/components/tab-icon-button.tsx`, forwardRef so it composes under `<PopoverTrigger asChild>`); `CommandButton`, `FilterFocusCommandButton`, and `AddPerspectiveCommandButton` all delegate to it, keeping their distinct press semantics. All command-button + tab-bar suites stay green.

### Warnings
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx:265` — `pendingCreateName` is never cleared on view switch. **FIXED (watcher path taken):** the name watcher is DELETED outright (obsoleted by arm-by-id). The id-based pending state is additionally bound to the dispatch-time view (`PendingCreate { id, viewId }`) and cleared when `activeViewId` changes, so a stale create can never arm rename after a view switch — pinned red-first by `pending_create_is_abandoned_when_the_view_switches_before_the_entity_lands`.
- [x] "anywhere else the name displays" — the "Go to Perspective: {name}" palette caption rendered blank for blank names. **FIXED Rust-side:** `emit_perspective_goto` (scope_commands.rs) renders `BLANK_PERSPECTIVE_NAME_PLACEHOLDER` ("Untitled") for blank/whitespace names. ONE convention with the frontend: `BLANK_NAME_PLACEHOLDER` exported from perspective-tab-bar.tsx, both literals pinned (`blank_perspective_names_get_the_untitled_placeholder_caption` Rust-side red-first; `blank_name_placeholder_literal_matches_the_rust_caption_placeholder` vitest-side), each constant's doc comment names its mirror.

### Nits
- [x] `generateUntitledName` exported with zero importers, no drift guard vs the backend mirror. **RESOLVED — option (a), documented:** the frontend keeps generating the name (the live views `save perspective` op requires an explicit `name`; backend generation only exists on the legacy Rust command path). The export is retained FOR the drift-guard test (now an importer): lockstep tables `generates_the_first_free_untitled_slot` (vitest) ↔ `first_free_untitled_name_matches_the_frontend_generator` (Rust) pin the shared first-free-slot convention. The backend's `generate_untitled_name` was refactored onto the pure `first_free_untitled_name` (fixing the same gap-collision bug there, red-first via `test_save_perspective_cmd_untitled_fallback_fills_gaps_not_counts`) and its stale `<AddPerspectiveButton>` doc reference was replaced.

## Rework notes (2026-06-12, post-review)

- `builtin/plugins/perspective-commands/commands/lifecycle.ts`: `perspective.save` now returns `unwrapResult(...)` (op payload `{ ok, perspective, entry_id }`) so the frontend can read the created id. The command-service e2e (`builtin_perspective_commands_e2e.rs`) expectations were updated for the unwrapped save payload — NOT runnable under this card's `-p swissarmyhammer-kanban`-only constraint; flagged for the next full CI run.
- De-flaked `perspective-context.test.tsx` (pre-existing cross-file flake surfaced by the verification batch, ~2/3 failure rate co-run with perspective-tab-bar.test.tsx): the Tauri `listen` mock was a single last-writer-wins slot, which under StrictMode double-mount scheduling sometimes held the disposed subscription's callback; now an array-of-callbacks with fire-all (`fireStoreChanged`), plus the refetch wait polls instead of assuming one timer hop. 5/5 green after; 179/179 across the full verification batch.
- Verification: vitest 22 files / 179 tests passed (perspective-tab-bar*, command-button*, perspective-context, spatial-nav e2e/jump-to); `tsc --noEmit` exit 0; `cargo nextest run -p swissarmyhammer-kanban` 1293/1293 passed.

## Review Findings (2026-06-12 16:44)

Iteration-2 review. All four prior findings (B1, B2, W1, W2) plus the nit verified GENUINELY fixed — arm-by-id flow read end to end (lifecycle.ts `unwrapResult` → Tauri `{result, undoable}` envelope → `createPerspective` reading `result.perspective.id` → id-matching arrival watcher; no name-matching remnants), `<TabIconButton>` extraction is render-identical (same class strings, stopPropagation, icon fill, forwardRef through Pressable), view-switch guard + test confirmed, placeholder constants cross-pinned on both sides, lockstep drift tables match verbatim (6 rows each, identical cases).

Fresh verification evidence: `cargo nextest run -p swissarmyhammer-command-service` 143/144 passed (sole failure = pre-existing carded `meta_tree_id_param_is_required_where_expected`; all 3 `builtin_perspective_commands_e2e` tests pass — the previously-unrun e2e is green). Scoped vitest 20 files / 140 tests passed (all perspective-tab-bar*, command-button*, perspective-context suites incl. the new add-create-rename file). `tsc --noEmit` exit 0. `cargo nextest run -p swissarmyhammer-kanban` 1293/1293 passed. Red-green probe: reverted `first_free_untitled_name` to count-based → `first_free_untitled_name_matches_the_frontend_generator` AND `test_save_perspective_cmd_untitled_fallback_fills_gaps_not_counts` both FAILED (re-minted "Untitled 2"); restored exactly (diffstat byte-identical) → 4/4 green. The perspective-context.test.tsx de-flake is genuine: the key assertion (`countListCalls() === listCallsBefore + 1`) is preserved exactly; only the single-timer-hop wait became a poll and the listener mock became fire-all.

Two new doc-only nits:

### Nits
- [x] `crates/swissarmyhammer-kanban/src/scope_commands.rs:411` — `BLANK_PERSPECTIVE_NAME_PLACEHOLDER` was inserted BETWEEN `emit_perspective_goto`'s pre-existing doc comment (lines 385-401, "Emit one 'Go to Perspective: <Name>' palette row…") and the function, so that whole 17-line block now misattributes to the const and `emit_perspective_goto` has no doc comment at all. Move the const (with its own doc) ABOVE the function's doc block so each item keeps its own docs. **FIXED:** const + its placeholder doc now sit above `emit_perspective_goto`'s doc block; the palette-row doc re-attaches to the function. `cargo check -p swissarmyhammer-kanban` clean.
- [x] `apps/kanban-app/ui/src/components/perspective-tab-bar.add-and-sort-migration.test.tsx:22` and `perspective-tab-bar.add-enter.spatial.test.tsx:21` — the updated header comments still describe the generated name as `"Untitled" / "Untitled N+1"`; "N+1" is the count-based convention this card just deleted. Should read `"Untitled" / "Untitled N"` (first-free-slot), matching the wording in `perspective-tab-bar.tsx` / `perspective_commands.rs`. **FIXED:** both headers now read `"Untitled" / "Untitled N" — the first free slot by exact-name match against the visible list`, matching the canonical wording on `generateUntitledName`. (The pre-migration history paragraph in the migration file keeps "N+1" — it accurately describes the deleted count-based inline logic.) `tsc --noEmit` exit 0.