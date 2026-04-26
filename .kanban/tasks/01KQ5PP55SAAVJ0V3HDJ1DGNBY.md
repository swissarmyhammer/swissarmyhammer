---
assignees:
- claude-code
position_column: todo
position_ordinal: a080
project: spatial-nav
title: 'ARCHITECTURE FIX: collapse Focusable into FocusScope (three peers, not four)'
---
## What

The current architecture has **four React/Rust peer types** for focus: `Focusable`, `FocusZone`, `FocusLayer`, and a composite `FocusScope` that wraps either `<Focusable>` or `<FocusZone>` and adds CommandScope. This was wrong. The composite-vs-primitive split smears `showFocusBar`, click handling, and indicator rendering across two layers and is the systemic root of why the user can't see focus on anything.

The correct model is **three peer types**:

| Concept | Role | React | Rust |
|---|---|---|---|
| Layer | Modal boundary, hard nav stop | `<FocusLayer>` | `FocusLayer` |
| Zone | Navigable container | `<FocusZone>` | `FocusZone` |
| Scope | Leaf — shows focus, takes clicks, navigates | `<FocusScope>` | `FocusScope` |

`FocusScope` is the leaf. It does what the pre-spatial-nav `<FocusScope>` already did: render the focus indicator, handle clicks, route to the command scope chain. There is **no separate `<Focusable>` component** in the final state, and no composite layer.

## CRITICAL ORDERING — `<Focusable>` stays as a re-export until the end

This card **must NOT delete `<Focusable>`**. The per-component cards (column, card, navbar, etc.) need a green build during their TDD loops, and many of them currently reference `<Focusable>` from inside their own component code AND inside their tests. If `<Focusable>` disappears here, every per-component card breaks before it can TDD anything.

What this card does:
1. Move `<Focusable>`'s implementation into `<FocusScope>` so `<FocusScope>` is the leaf primitive.
2. Reduce `kanban-app/ui/src/components/focusable.tsx` to a one-line re-export: `export { FocusScope as Focusable } from "./focus-scope"`. (Type alias if needed: `export type FocusableProps = FocusScopeProps`.)
3. Add a `@deprecated` JSDoc comment on the re-export pointing at `<FocusScope>` and the eventual deletion card `01KQ5PSMYE...`.

The actual file deletion happens in card `01KQ5PSMYE...` AFTER every per-component card lands. That card runs **last** in the chain, with `depends_on` set to every per-component card + the release-blocker umbrella.

## What changes here

### React side

- **Move `<Focusable>` impl into `<FocusScope>`.** All the work currently in `<Focusable>` (mint SpatialKey, call `spatial_register_focusable` → renamed `spatial_register_scope`, render `<FocusIndicator>`, handle click → `spatial_focus`, subscribe to claim events) lives in `<FocusScope>` directly.
- **`<FocusScope>` becomes the leaf primitive** — no composite, no `kind` prop.
- **`<Focusable>` becomes a thin re-export** of `<FocusScope>` for transitional compatibility. NOT deleted in this card.
- **`<FocusZone>` keeps its current shape**; only `Focusable` references in its docstrings/types get renamed.
- **`<FocusLayer>` keeps its current shape**.
- **Drop `kind="zone"` prop on `<FocusScope>`.** Call sites that need a zone use `<FocusZone>` directly. The transitional re-export of `<Focusable>` does not accept `kind="zone"` (it forwards to `<FocusScope>` which is now leaf-only).

### Rust side

- **Rename `Focusable` → `FocusScope`** (the leaf struct).
- The internal `FocusScope` enum (sum of `Focusable | Zone`) goes away — registry stores the two struct types via an internal discriminator, but the public API exposes `FocusScope` as the leaf struct and `FocusZone` as the container struct. No public enum to navigate.
- Tauri command renames: `spatial_register_focusable` → `spatial_register_scope`. `spatial_register_zone` keeps its name. `spatial_unregister_scope` already correct.
- `FocusEventSink` / `NavStrategy` traits keep their shapes; only the type they reference changes.

### Cards to update after this lands

Every card in the spatial-nav project that names `Focusable` or describes the composite `FocusScope` model needs a documentation refresh. Canonical terminology section in card `01KNQXW7HH...` is the source of truth and gets updated first. Sweep the others mechanically.

## Why the bridging-as-deprecation pattern matters

Per-component cards are blocked behind this one. They each TDD their visible-focus integration test. If `<Focusable>` disappears entirely here, every per-component card has to do a search-and-replace across its own files BEFORE it can red-green its actual test. That's lots of churn that bury the real work. With `<Focusable>` aliased to `<FocusScope>`, the per-component cards can:

- Switch their imports to `<FocusScope>` directly when convenient, OR
- Leave them alone (the alias keeps everything compiling) and let the cleanup card sweep the imports later.

Either way, the per-component card's TDD loop never breaks because of a missing primitive.

## Subtasks

- [ ] Inventory every place `<Focusable>` is imported or referenced in `kanban-app/ui/src/`
- [ ] Move `<Focusable>`'s implementation into `<FocusScope>` (focus-scope.tsx)
- [ ] Reduce `kanban-app/ui/src/components/focusable.tsx` to: `export { FocusScope as Focusable } from "./focus-scope";` plus a `@deprecated` doc comment
- [ ] Drop the `kind` prop from `<FocusScope>`; call sites that need a zone use `<FocusZone>` directly
- [ ] Rename Rust `Focusable` struct → `FocusScope` (in `swissarmyhammer-focus`)
- [ ] Drop the Rust `FocusScope` enum (was sum of `Focusable | Zone`); registry stores two distinct types internally
- [ ] Tauri command `spatial_register_focusable` → `spatial_register_scope`
- [ ] Update terminology section in card `01KNQXW7HH...` to describe three peers, not four
- [ ] Sweep every spatial-nav card for `Focusable` references; update doc references
- [ ] Run `cargo test -p swissarmyhammer-focus` + `pnpm vitest run` — all green

## Acceptance Criteria

- [ ] `<FocusScope>` is the leaf primitive — handles registration, click, claim subscription, indicator render, all in one component
- [ ] `<FocusZone>` is the container primitive (unchanged)
- [ ] `<FocusLayer>` is the layer primitive (unchanged)
- [ ] `kanban-app/ui/src/components/focusable.tsx` exists but is a thin re-export of `FocusScope` with `@deprecated` JSDoc
- [ ] Rust `Focusable` struct renamed to `FocusScope`; no public enum; `spatial_register_focusable` → `spatial_register_scope`
- [ ] All existing tests pass after the rename + collapse — if a test references the old `<Focusable>` it still works because of the re-export
- [ ] No breaking change to call sites: every existing `<Focusable>` usage continues to compile and behave identically

## Tests

- [ ] `focus-scope.test.tsx` — scope renders an indicator when its key is the focused key
- [ ] `focusable.test.tsx` (if exists) — re-export keeps existing tests green by aliasing to FocusScope
- [ ] `cargo test -p swissarmyhammer-focus` clean
- [ ] `pnpm vitest run` — all tests pass

## Workflow

Sequence:

1. Rust rename first (lowest blast-radius — just a type rename + Tauri command rename).
2. React collapse second (`<Focusable>` impl moves into `<FocusScope>`, focusable.tsx becomes a re-export, prop-shape change).
3. Per-component card TDD loops then run on top of the collapsed-but-still-aliased architecture (separate cards, not this one).
4. The actual file deletion of `focusable.tsx` happens in cleanup card `01KQ5PSMYE...` AFTER all per-component cards land.

## Blocks / blocked-by

This card **blocks** every reopened per-component card:
- `01KNQXZ81Q` Board view
- `01KQ20MX70` Column
- `01KQ20NMRQ` Card
- `01KNQXZZ9V` Grid view
- `01KQ20Q2PN` NavBar
- `01KQ20QW3K` Toolbar
- `01KPZS32YN` Perspective
- `01KNQXYC4RB` Inspector layer
- `01KNQY0P9J` Inspector field rows
- `01KPZS4RG0` Drill-in/out + Space rebind
- `01KQ5PEHWT` Release-blocker umbrella

This card is **blocked by** nothing — it is the next piece of work to start.