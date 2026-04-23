---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff9080
project: spatial-nav
title: 'Virtual scrolling / transform animations: rects registered during a CSS transform animation get stale coordinates'
---
## What

Follow-up to `01KPVTKZ1VGDSBB0HPYTTAHJNH` (scroll-ancestor rect re-report, commit `33f60d132`). That fix handles **scroll events**. This task handles **CSS transform animations** — a different vector that hits every `FocusScope` inside a sliding / fading / morphing ancestor.

### Concrete repro site

`kanban-app/ui/src/components/slide-panel.tsx:37` uses Tailwind `translate-x-0` / `translate-x-full` for its open/close animation:

```tsx
className={cn(
  "fixed top-0 right-0 bottom-0 z-30 w-[380px] bg-background border-l border-border shadow-xl transition-transform duration-200",
  open ? "translate-x-0" : "translate-x-full",
)}
```

Every `<InspectorFocusBridge>` is wrapped in a `<SlidePanel>`. When the user opens an inspector:

1. Panel starts at `translate-x-full` (off-screen right).
2. Class flip triggers `transition-transform duration-200` → panel slides to `translate-x-0`.
3. Inside the panel, `InspectorFocusBridge` mounts. `<FocusLayer>` pushes. `<FocusScope>` field rows mount, their `useRectObserver` effects fire, each calls `getBoundingClientRect()` and `invoke("spatial_register", {x, y, w, h, ...})`.

**The mount-time `report()` fires during step 3, while the panel is still mid-animation.** The rect `getBoundingClientRect()` returns reflects the panel's current (mid-slide) transform, not its final position. `ResizeObserver` doesn't fire when the transform completes (size didn't change). There is no scroll during the animation, so `01KPVTKZ1VGDSBB0HPYTTAHJNH`'s scroll listener doesn't trigger either. **Rust ends up with inspector-field rects at stale mid-animation x-coordinates forever.**

### Fix applied

Extended `useRectObserver` in `kanban-app/ui/src/components/focus-scope.tsx`:

1. Added a `POSITIONAL_TRANSITION_PROPS` allowlist (`transform`, `translate`, `left`, `top`, `right`, `bottom`).
2. Added a `transitionend` listener on `document` that filters by `propertyName` and triggers a RAF-throttled `report()`.
3. Added a RAF-deferred re-report one frame after the initial `report()` call — catches layout-settle races even when no CSS transition fires. **Cost**: this fires unconditionally on every scope mount, adding one extra `spatial_register` IPC round-trip per scope per mount, independent of whether an animated ancestor exists. For a first-load app with 100–200 scopes that is 100–200 extra Tauri IPCs on cold start. The trade-off is deliberate — the hook cannot cheaply tell at mount time whether an animated ancestor is in flight, so the deferred re-report happens unconditionally rather than gating behind a brittle heuristic. If this shows up in startup traces, a follow-up can gate on "did `ResizeObserver`'s initial tick report a different rect than mount-time?".
4. Unified the RAF token across the scroll listener, the transition listener, and the mount-deferred re-report — any signal within one frame coalesces into a single `spatial_register` invocation.
5. Cleanup tears down the listener, cancels the pending RAF, and preserves the existing ResizeObserver + scroll listener teardown.

### Files modified

- `kanban-app/ui/src/components/focus-scope.tsx` — extended `useRectObserver` as above.

### Files added

- `kanban-app/ui/src/test/spatial-nav-transition-rect.test.tsx` — chromium browser test. Mounts a FocusScope inside a `SlidePanel`-shaped animated ancestor; asserts a second `spatial_register` invoke lands after the real browser fires `transitionend` with the settled post-animation x. Also includes a regression guard verifying opacity transitions don't spam the registration path.

### Out of scope (unchanged)

- Changing SlidePanel's animation to not use transform.
- `MutationObserver`-based transform detection.
- Inspector-specific fix. The mechanism is general — any consumer (modals, drawers, animated sidebars) benefits automatically.

## Acceptance Criteria

- [x] `useRectObserver` attaches a `transitionend` listener that re-runs `report()` when a transform-class property finishes animating on any ancestor
- [x] `useRectObserver` also re-runs `report()` once on the frame after initial mount (RAF-deferred) to catch layout-settle races
- [x] Cleanup removes the `transitionend` listener and cancels the RAF on unmount
- [x] Browser test: mount a FocusScope inside an animated ancestor, confirm a second `spatial_register` invoke fires with post-animation coordinates — fails on HEAD, passes after the fix
- [x] Existing tests (scroll re-report, ResizeObserver, mount/unmount contracts) still green
- [x] `cd kanban-app/ui && npm test` — all green (1422 tests / 133 files)
- [x] No new permanent `console.warn` traces in production code

## Tests

- [x] `cd kanban-app/ui && npm test -- spatial-nav-transition-rect` — passes (2/2)
- [x] `cd kanban-app/ui && npm test -- focus-scope` — existing tests green (49/49 in focus-scope.test.tsx + 9/9 in focus-scope-scroll.node.test.ts)
- [x] `cd kanban-app/ui && npm test -- spatial-nav-virtual-scroll` — previous scroll-listener tests still green (2/2, regression guard)
- [x] `cd kanban-app/ui && npm test` — full suite green (1422/1422)

## Workflow

- Used `/tdd`. Wrote the failing transition-rect browser test first. Confirmed it failed on HEAD (only mount-time + ResizeObserver-initial invokes, no transition-driven one). Implemented the fix. Confirmed test now passes.
- Did NOT change SlidePanel, entity-inspector, or any consumer. The fix is general in `useRectObserver`.
- RAF throttling from the scroll-listener fix applies here too — one shared RAF token coalesces any combination of scroll / transitionend / mount-deferred signals per frame.
- Both listeners (scroll + transitionend) live in the same `useEffect`, share the same RAF token, share the same cleanup — no forked code paths.

## Review Findings (2026-04-22 07:23)

TDD trace re-verified: with `focus-scope.tsx` reverted via `git stash`, `spatial-nav-transition-rect.test.tsx` fails test 1 with `expected 2 to be greater than 2`; restoring the fix makes it pass. All 58 focus-scope tests, both virtual-scroll regression guards, and the full 1422/133 UI suite green on the as-committed tree. Design, cleanup symmetry, RAF coalescing, and `POSITIONAL_TRANSITION_PROPS` filter all read cleanly. Two nits, no blockers or warnings.

### Nits
- [x] `kanban-app/ui/src/test/spatial-nav-transition-rect.test.tsx:260-291` — The opacity-regression test waits only 2 guard frames (~32ms) after dispatching the fake `transitionend`, while the harness's real `transition: transform 100ms` is still in flight. On a slow CI runner (e.g. 30Hz frame cap or system load) the real transition's `transitionend` could land inside the guard window and trip `xsAfter.length === xsBefore.length`. Consider either dispatching the fake opacity event into a non-animating harness, or bumping the pre-capture settle to match the transition's completion time.
  - Fixed: the opacity test now waits for the real `transition: transform` to fully settle (via the same `waitFor("panel transition to settle", ...)` helper test 1 uses) before snapshotting `xsBefore`. After the real transitionend has fired and flushed, the fake opacity event is dispatched into a quiescent harness — the post-dispatch guard window can no longer collide with the real transform transitionend.
- [x] `kanban-app/ui/src/components/focus-scope.tsx:285` — The unconditional `scheduleReport()` on mount adds one extra `spatial_register` invoke per scope on every mount, regardless of whether an animated ancestor exists. The inline comment acknowledges this as intentional ("without any heuristic about which consumers need it"), which is defensible — but worth naming in the task description alongside the +1-IPC-per-scope cost: for 100-200 scopes on first app load that is 100-200 extra Tauri IPC round-trips. If this shows up in startup traces, a follow-up can gate the deferred re-report on `ResizeObserver`'s initial tick's rect differing from mount-time's.
  - Fixed: the "Fix applied" section above now explicitly names the +1-IPC-per-scope-per-mount cost (100–200 extra Tauri IPCs on cold start for a typical app) and records the deliberate trade-off plus the possible follow-up heuristic.
