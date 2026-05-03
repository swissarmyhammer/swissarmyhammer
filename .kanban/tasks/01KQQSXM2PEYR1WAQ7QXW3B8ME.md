---
assignees:
- claude-code
position_column: todo
position_ordinal: ce80
project: spatial-nav
title: 'DESIGN: Spatial nav — keyboard-as-mouse cardinal + drill / first / last on Scope primitive'
---
## What

This is the **design spec** for a redesigned spatial focus navigation system. It defines the contract; six component tasks (listed below) implement the contract piecewise.

User's mental model: **"directional nav is keyboard-as-mouse"** — pressing an arrow key picks the visually-nearest thing in that direction, regardless of structural depth. Containers (zones) earn their keep only for the **drill** / **first** / **last** operations, where the tree shape matters.

This redesign supersedes the structural-cascade bug class — symptoms include `target=None`, `scope_chain=["engine"]`, focus collapsing to root when pressing Left from the leftmost perspective tab or Up from a board column. Two per-bug regression tasks (`01KQPW1FTYFWTDMW6ESM5ABGJQ` and `01KQQSC4D3RSA2PQT9KN1MSXZ3`) are archived; their scenarios are now acceptance criteria below.

## Contract

### The five primitive operations

| Operation | Key | Behaviour |
|---|---|---|
| `nav.{up,down,left,right}` | Arrows | **Geometric**: pick the registered scope (in the same layer) whose rect is nearest to the focused scope in direction D, scored by Android beam (`13 * major² + minor²`). **No structural filtering.** Just the visually-closest neighbor in the half-plane of D. |
| `nav.drillIn` | Enter | Focus the focused scope's first child (geometric: topmost-then-leftmost). On a leaf, no-op (return focused FQM). |
| `nav.drillOut` | Escape | Focus the parent scope. At layer root, fall through to `app.dismiss`. |
| `nav.first` | Home | Same as drillIn (focus first child). On a leaf, no-op. |
| `nav.last` | End | Focus the last child (bottommost-then-rightmost). On a leaf, no-op. |

### Invariants

- **Layer is the hard wall.** No operation crosses `layer_fq`. The inspector layer remains isolated from the main layer.
- **`navOverride` runs first** (rule 0). Per-direction redirects and walls short-circuit the cascade entirely.
- **No-silent-dropout.** Every operation returns a `FullyQualifiedMoniker` — never `None`, never `target=None` in the IPC, never collapses to engine root. "No motion possible" is signalled by returning the focused FQM (stay-put).
- **Coordinate system: viewport-relative.** All registered rects are sampled by `getBoundingClientRect()` and refreshed on ancestor scroll via `useTrackRectOnAncestorScroll`. The kernel's geometric pick is correct iff all candidate rects in the same layer were sampled in the same coordinate system. **This invariant is load-bearing — see component task #6 (coordinate consistency).**

### Why this is elegant

- **Cardinal nav is keyboard-as-mouse.** No mental model of zone hierarchies needed; the visually-nearest scope wins. Solves the cross-zone bug class because geometric distance doesn't care about structural depth.
- **Drill / first / last anchor on scope children.** That's why scopes (containers) still earn their keep — they define what "first child" and "last child" mean.
- **Five ops, one rule per op.** No iter 0 vs iter 1, no any-kind vs same-kind filter, no drill-into-natural-leaf as a post-cascade fixup.

### Virtualization

**Essential** — the app uses windowed rendering and off-viewport rows do not register `<FocusScope>`. The kernel cannot find them via geometric pick. The fix is a **scroll-on-edge** rule that lives in the React glue, not the kernel:

> When the kernel returns stay-put AND the focused scope is at the edge of a scrollable ancestor in direction D AND that ancestor can scroll further in D, scroll it by one item-height in D, wait for the virtualizer to mount the next row, then re-run nav.

This is component task #5.

### Containment tree contract (for drill / first / last)

- **First child** = the child whose rect is topmost; ties broken by leftmost.
- **Last child** = the child whose rect is bottommost; ties broken by rightmost.
- **Parent** = the focused scope's `parent_zone` (after `FocusZone` / `FocusScope` distinction is preserved at the type level — see component task #2 / #4 for whether the kernel exposes `parent_scope` more generally).
- **Children** = registered scopes whose `parent_zone` is the focused scope's FQM.

If the type-level `FocusZone` / `FocusScope` split is collapsed into one `Scope` primitive (separate follow-up, NOT in this plan), drill / first / last work on any scope's children — leaves just have zero children.

## Component tasks (links to follow as they're filed)

| # | Component | Task | Dependencies |
|---|---|---|---|
| 1 | Geometric cardinal nav (replaces `cardinal_cascade` with `geometric_pick`) | TBD | (none — can start) |
| 2 | `nav.drillIn` = focus first child | TBD | (none) |
| 3 | `nav.drillOut` = focus parent | TBD | (none) |
| 4 | `nav.first` / `nav.last` = focus first/last child | TBD | (none) |
| 5 | Scroll-on-edge React glue for virtualized regions | TBD | depends on #1 |
| 6 | Coordinate consistency: TS-side audit + kernel debug assertions | TBD | (none — can start in parallel with #1) |

Each component task **must** open with a `## Reference` section that points back to this design and restates the contract for that op, so an implementer reading only the component task has the full context.

`01KQQDXHANWGMBG872KZ3FZ86P` (drill into editor on Enter) is kept and re-pointed at component task #2.

## Files this plan touches (across all six components)

- `swissarmyhammer-focus/src/navigate.rs` — algorithm rewrite (#1, #2, #3, #4)
- `swissarmyhammer-focus/src/types.rs` — `Direction` enum changes (#4)
- `swissarmyhammer-focus/src/registry.rs` — debug assertions (#6)
- `swissarmyhammer-focus/README.md` — rewrite with new contract
- `swissarmyhammer-focus/tests/fixtures/mod.rs` — fixture extensions (#1, #6)
- `swissarmyhammer-focus/tests/` — new and revised integration tests
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — register/update IPC adapters (#6)
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` — coordinate audit (#6)
- `kanban-app/ui/src/components/app-shell.tsx` — nav command builders, scroll-on-edge (#5)
- `kanban-app/ui/src/components/focus-scope.tsx` and `focus-zone.tsx` — registration callsites (#6)

## Acceptance for the plan as a whole

Each component task carries its own acceptance criteria. The plan as a whole is done when all six are done AND:

- [ ] **The four reported cross-zone bugs all resolve via the same code path:**
  - [ ] Left from leftmost `perspective_tab:*` lands on a leaf inside `ui:left-nav`.
  - [ ] Up from `column:{id}` lands on a leaf inside `ui:perspective-bar`.
  - [ ] Down from a `perspective_tab:*` lands inside the perspective body.
  - [ ] Up from any board column header lands on the perspective bar; Up from the perspective bar lands on the navbar.
- [ ] No-silent-dropout contract holds across all five ops.
- [ ] Layer boundary respected.
- [ ] `navOverride` rule 0 still runs first.
- [ ] `cargo test -p swissarmyhammer-focus` passes.
- [ ] `pnpm -C kanban-app/ui test` passes.

## Workflow

- This task itself does NO implementation. It is the spec.
- The six component tasks are implemented in their own PRs, each landing the contract for its op.
- The component tasks can land in any order EXCEPT #5 depends on #1 (scroll-on-edge needs geometric pick to detect "stay-put" reliably).
- The follow-up "collapse `FocusZone` / `FocusScope` into one `Scope` primitive" is **not** part of this plan — file separately if motivated by the work.
