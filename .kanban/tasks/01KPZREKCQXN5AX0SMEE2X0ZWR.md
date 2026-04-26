---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9980
title: Tag ui-state-changed events with the UIStateChange variant; frontend ignores scope_chain echoes
---
## What

`kanban-app/src/commands.rs::emit_ui_state_change_if_needed` broadcasts the full UIState JSON on every command that returns a `UIStateChange`. Because `ui.setFocus` returns `UIStateChange::ScopeChain(...)` and fires on every arrow key, every keystroke fans out a `ui-state-changed` event with the entire UIState payload to every open window. Every window's `UIStateProvider` does `setState(payload)`, so every `useUIState()` consumer (`AppShell`, `PerspectiveProvider`, `useAppMode`, palette, etc.) re-renders on every focus move. That cascade is one of the dominant per-keystroke costs on the 2000-row board.

The backend already knows, at emit time, exactly *which* variant changed — `UIStateChange` is enumerated and pattern-matched in the same function. Today it throws that discriminator away and sends a headless snapshot, forcing the frontend to either blindly apply it (status quo — the bug) or deep-compare to reconstruct what the backend already knew (rejected as wasteful re-derivation of existing info). The fix is to carry the discriminator on the wire and let the frontend ignore events it doesn't care about.

**The one consumer of scope_chain-via-UIState** — audited via `Grep "scope_chain" kanban-app/ui/src`:
- `kanban-app/ui/src/components/command-palette.tsx` reads it from `useUIState()` and uses it to refetch `list_commands_for_scope` when the palette is open and focus changes.
- No other consumer reads scope_chain from `UIStateSnapshot`. Other references write it (via `ui.setFocus`) or read it from the `context-menu-command` event payload, which is a different path.

That single consumer migrates to the frontend-authoritative source (`FocusedScopeContext` + `scopeChainFromScope`), which removes the last dependency on scope-chain-via-UIState.

**Wire format change**

The `ui-state-changed` event payload becomes a wrapper:
```json
{
  "kind": "scope_chain",
  "state": { /* full UIState snapshot — same shape as today's payload */ }
}
```

`kind` values — one per `UIStateChange` variant plus the two result-shape cases the current code already handles:
```
scope_chain | palette_open | keymap_mode | inspector_stack
| active_view | active_perspective | app_mode
| board_switch | board_close
```

**Backend** — `kanban-app/src/commands.rs::emit_ui_state_change_if_needed`

```rust
fn emit_ui_state_change_if_needed(app: &AppHandle, state: &AppState, result: &Value) {
    let kind = if let Ok(change) = serde_json::from_value::<UIStateChange>(result.clone()) {
        Some(match change {
            UIStateChange::ScopeChain(_) => "scope_chain",
            UIStateChange::PaletteOpen(_) => "palette_open",
            UIStateChange::KeymapMode(_) => "keymap_mode",
            UIStateChange::InspectorStack(_) => "inspector_stack",
            UIStateChange::ActiveView(_) => "active_view",
            UIStateChange::ActivePerspective(_) => "active_perspective",
            UIStateChange::AppMode(_) => "app_mode",
        })
    } else if result.get("BoardSwitch").is_some() {
        Some("board_switch")
    } else if result.get("BoardClose").is_some() {
        Some("board_close")
    } else {
        None
    };
    if let Some(kind) = kind {
        let _ = app.emit(
            "ui-state-changed",
            serde_json::json!({ "kind": kind, "state": state.ui_state.to_json() }),
        );
    }
}
```

No semantic change — backend is just telling the truth about *which* change it already knew it made. No UI-specific logic added to the backend. The "act on this or not" decision lives on the frontend.

**Frontend** — `kanban-app/ui/src/lib/ui-state-context.tsx`

```ts
interface UIStateChangedEvent {
  kind:
    | "scope_chain" | "palette_open" | "keymap_mode"
    | "inspector_stack" | "active_view" | "active_perspective" | "app_mode"
    | "board_switch" | "board_close";
  state: UIStateSnapshot;
}

listen<UIStateChangedEvent>("ui-state-changed", (event) => {
  // scope_chain is frontend-authoritative — the frontend drove this change via
  // ui.setFocus, and no useUIState() consumer reads scope_chain from here.
  // Skipping the setState keeps the prev reference so focus moves don't
  // cascade re-renders through every useUIState() subtree.
  if (event.payload.kind === "scope_chain") return;
  setState(event.payload.state);
});
```

One string compare. No diff helper, no deep-equal. Extensible: if `keymap_mode` or another slice later turns out to be write-only, adding it to the skip-list is a one-line change.

**Migrate the last scope_chain consumer** — `kanban-app/ui/src/components/command-palette.tsx`

```ts
// Before:
const { keymap_mode: mode, scope_chain: scopeChain } = useUIState();

// After:
const { keymap_mode: mode } = useUIState();
const focusedScope = useContext(FocusedScopeContext);
const scopeChain = useMemo(
  () => scopeChainFromScope(focusedScope),
  [focusedScope],
);
```

Palette semantics are preserved: focus change while palette is open still invalidates `useEffect([open, scopeChain])` and refetches commands for the new chain — just via `FocusedScopeContext` instead of via a round-trip through the backend.

**Optional once migration lands**: drop `scope_chain` from the frontend `UIStateSnapshot` TS type and from the Rust `UIState::to_json()` output. With no consumer, the type system then enforces "nobody reads scope_chain from UIState," and the per-keystroke IPC payload shrinks. Worth doing in the same PR.

### Files
- `kanban-app/src/commands.rs` — tag the emitted event with `kind`.
- `kanban-app/ui/src/lib/ui-state-context.tsx` — new `UIStateChangedEvent` type, discriminator-aware listener.
- `kanban-app/ui/src/components/command-palette.tsx` — switch scope-chain source to `FocusedScopeContext`.
- `kanban-app/ui/src/lib/ui-state-context.test.tsx` — regression tests for the discriminator.
- `kanban-app/ui/src/components/command-palette.test.tsx` — regression test for palette refresh on focus change.

### Subtasks
- [x] Backend: wrap the `ui-state-changed` emit payload with `{ kind, state }`; map every `UIStateChange` variant and the two board result cases.
- [x] Update any existing Rust test that asserts on the raw `ui-state-changed` payload shape (if any) to expect the wrapper.
- [x] Frontend: introduce `UIStateChangedEvent` type; update the listener in `UIStateProvider` to unwrap `.state` and early-return on `kind === "scope_chain"`.
- [x] Frontend: migrate `command-palette.tsx` to read scope chain from `FocusedScopeContext` + `scopeChainFromScope`; drop its `useUIState()` read of `scope_chain`.
- [ ] Optional hygiene: drop `scope_chain` from `UIStateSnapshot` (frontend type) and from `UIState::to_json()` (backend) once no consumer remains. (Deferred — out of parallel-worktree scope; the two types still carry scope_chain but no consumer reads it from UIState anymore. Safe to land separately.)
- [x] Add the three regression tests described under Tests.
- [ ] Manual smoke on the 2000-row swissarmyhammer board. (Deferred to reviewer.)

## Acceptance Criteria
- [x] Every `ui-state-changed` event emitted by the backend carries `{ kind, state }`. Kind is one of the nine enumerated values; `state` is the existing full UIState JSON.
- [x] `UIStateProvider` does NOT call `setState` when the event kind is `"scope_chain"`. All other kinds update state as before.
- [x] `useUIState()` returns a reference-stable value across a focus change — React Profiler shows zero re-renders of `useUIState()` consumers during arrow-key nav. (Covered by the `scope_chain events do not change useUIState() identity` regression test — `result.current` is strictly equal pre- and post-emit.)
- [x] Command palette opens with commands for the currently focused scope, and if focus moves while the palette is open, the command list refetches for the new scope — behavior identical to today, just sourced from `FocusedScopeContext`.
- [x] No regression in existing behavior for `palette_open`, `keymap_mode`, `inspector_stack`, `active_view`, `active_perspective`, `app_mode`, `board_switch`, `board_close` — each still propagates to `useUIState()` consumers when it changes.
- [x] Backend still emits `ui-state-changed` on every `ui.setFocus` (we only changed the payload shape, not suppressed the emit) — the code path in `emit_ui_state_change_if_needed` runs unconditionally when a `UIStateChange` is returned, only the payload shape changed.

## Tests
- [x] `kanban-app/ui/src/lib/ui-state-context.test.tsx` — `"scope_chain events do not change useUIState() identity"`: emit a mock payload `{ kind: "scope_chain", state: {...new scope_chain...} }`, assert `result.current` reference is strictly equal to the pre-event reference.
- [x] Same file — `"palette_open events update state"`: emit `{ kind: "palette_open", state: {...palette_open true...} }`, assert `result.current` is a new reference and `result.current.windows[label].palette_open === true`.
- [x] Same file — `"board_switch events update state"`: emit `{ kind: "board_switch", state: ... }`, assert state reference is new. Plus a loop test covering keymap_mode/inspector_stack/active_view/active_perspective/app_mode/board_close.
- [x] `kanban-app/ui/src/components/command-palette.test.tsx` — `"palette refetches commands when focused scope changes while open"`: mount palette open under FocusedScopeContext value A, assert `list_commands_for_scope` was called with A's chain; rerender with value B, assert it was re-called with B's chain.
- [x] Rust test — add to the existing `kanban-app/src/commands.rs` test module: call `ui_state_change_kind` with each `UIStateChange` variant and with `BoardSwitch`/`BoardClose`/unrelated shapes; 10 tests covering the full kind table.
- [x] Test command (frontend): `cd kanban-app/ui && npm test -- ui-state-context command-palette`. Result: 38 tests green (7 + 31).
- [x] Test command (backend): `cargo test -p kanban-app`. Result: 82 tests green.
- [ ] Manual smoke. (Deferred to reviewer.)

## Workflow
- Use `/tdd` — write the three frontend tests + one Rust test first, confirm they fail against the current unwrapped payload, then land backend wrapper + frontend discriminator + palette migration together as one PR. Keeping the changes paired in one PR prevents a wire-format mismatch between versions. #performance #events #uistate #frontend #backend