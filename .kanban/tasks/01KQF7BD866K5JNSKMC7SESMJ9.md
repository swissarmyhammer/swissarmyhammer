---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffe780
project: spatial-nav
title: 'Diagnose & fix: "duplicate FQM registration replaces prior scope" warnings flooding the kernel log'
---
## What

Production logs show repeated `duplicate FQM registration replaces prior scope` warnings from `swissarmyhammer-focus`. The FQM-as-key invariant says paths cannot collide by construction — if they do, two React primitives composed the *same* path, which is a programmer mistake that needs root-cause investigation, not log noise to ignore.

User report:
> "this type of error is all over the log -- let's task this up -- you are the programmer so this is your mistake to figure out"

Sample log line:
```
2026-04-30 07:32:09.655461-0500 Fault kanban-app: [com.swissarmyhammer.kanban:default]
duplicate FQM registration replaces prior scope — a real duplicate FQM is a
programmer mistake (two primitives whose composed paths collide)
fq=/window/ui:perspective/ui:view/board:board/ui:board/column:done/task:01KQ2E7RPBPJ8T8KZX39N2SZ0A/field:task:01KQ2E7RPBPJ8T8KZX39N2SZ0A.project/project:spatial-nav
op="register_scope"
```

## Root cause (resolved)

**The kernel's "duplicate FQM = programmer mistake" assumption was wrong.** The placeholder→real-mount swap path in `kanban-app/ui/src/components/column-view.tsx`'s `usePlaceholderRegistration` hook intentionally re-registers the same FQM:

1. The column virtualizer (`VirtualColumn`) uses `usePlaceholderRegistration` to register a `kind: "scope"` placeholder for every off-screen task via `spatial_register_batch`. The placeholder's FQM is composed deterministically as `<columnFq>/<task.moniker>` — the same FQM the real `<EntityCard>` `<FocusScope>` will use when the card mounts.
2. On first render, the virtualizer hasn't yet measured the scroll element, so `getVirtualItems()` returns `[]`, `visibleIndices` is empty, and the placeholder hook registers placeholders for **all** tasks (including those about to mount as real cards).
3. The virtualizer measures and renders the visible window. Each visible card's `<FocusScope>` mounts and fires `spatial_register_scope` at the same FQM — the kernel sees the existing placeholder entry and emits `tracing::error!`.
4. The placeholder hook re-runs on the next render with the populated `visibleIndices` and unregisters placeholders for now-visible tasks. By then, dozens of error-level warnings have already fired.

Same pattern fires on every scroll-into-view, every React StrictMode dev-mode double-mount, and every ResizeObserver-driven rect refresh. The 50+ warnings in the production log were all this race, not a real path collision.

The architectural answer (path-as-key) is correct. The kernel's blanket warning was wrong: re-registration at the same FQM is part of the normal lifecycle, and the placeholder→real-mount swap is documented behaviour (`SpatialRegistry::apply_batch` docstring already calls this out: "the registry's per-FQM overwrite semantics handle the placeholder→real-mount rect refresh transparently").

## Fix

`swissarmyhammer-focus/src/registry.rs::register_scope` and `register_zone` now distinguish two cases:

- **Same-shape re-registration** (matching `(segment, layer_fq, parent_zone, overrides)` tuple and same kind discriminator): silent overwrite. Rect — and zones' `last_focused` — are mutable runtime state that may differ. This is the placeholder→real-mount swap path, the StrictMode double-mount path, and the ResizeObserver rect-refresh path.
- **Structural-mismatch re-registration** (different segment, layer, parent_zone, overrides, or kind flip): still emits `tracing::error!` with structured `kind_flipped` / `segment_differs` / `layer_differs` / `parent_zone_differs` / `overrides_differ` flags so genuine programmer mistakes stay visible.

The check lives in a private `warn_on_structural_mismatch` helper so both register paths share one decision point.

## Repro steps (pre-fix)

1. `npm run tauri dev` against a board with at least one column holding ≥25 tasks (the `VIRTUALIZE_THRESHOLD` in `column-view.tsx`).
2. Open the board.
3. Watch:
   ```
   log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' \
            --info --debug --last 1m | grep "duplicate FQM"
   ```
4. **Pre-fix**: dozens of `duplicate FQM registration replaces prior scope op="register_scope"` warnings in a tight burst (one per visible task, fired during the placeholder→real-mount swap on initial load).
5. **Post-fix**: zero `duplicate FQM` warnings; structural-mismatch warnings only fire if a genuine path-collision bug is introduced.

## Files changed

- `swissarmyhammer-focus/src/registry.rs` — `register_scope` and `register_zone` route through `warn_on_structural_mismatch`; docstrings updated to reflect the new contract.
- `swissarmyhammer-focus/tests/duplicate_fqm_silent_swap.rs` — new test file pinning the contract: same-shape re-registration is silent, structural-mismatch still warns. 8 tests including a 50-task burst that mirrors the production scenario.
- `swissarmyhammer-focus/tests/focus_registry.rs` — `duplicate_fq_registration_replaces_prior_entry` docstring updated to match the new contract.
- `swissarmyhammer-focus/tests/path_monikers.rs` — same: docstring + test name (`register_with_duplicate_fq_replaces`) updated.

## Acceptance criteria

- [x] Root cause identified and named: `usePlaceholderRegistration` placeholder→real-mount swap in `kanban-app/ui/src/components/column-view.tsx`, exacerbated by the kernel's overly-aggressive duplicate-FQM warning.
- [x] Fix lands and the warning no longer fires for any same-shape re-registration during normal use (verified against `log show` after the dev binary picked up the change — zero warnings since the rebuild).
- [x] Repro steps documented above.
- [x] Verified in `npm run tauri dev` against `log show` — zero `duplicate FQM` warnings post-rebuild (start "2026-04-30 12:35:30" → 0 matches; pre-fix bursts had 50+ matches per board mount).
- [x] StrictMode contract preserved: the `<FocusScope>`/`<FocusZone>` register-during-render-+-effect-cleanup pattern stays untouched. The kernel-side change makes same-shape re-registers idempotent without removing any registration sites.

## Architectural note

The original premise ("duplicates = programmer mistakes") came from card `01KQD6064G1C1RAXDFPJVT1F46` (the path-monikers refactor). The premise is correct in spirit — paths should be unique per primitive. But the kernel's *enforcement* didn't account for legitimate intentional re-registrations: virtualizer placeholders, StrictMode double effects, scroll-into-view, ResizeObserver. The fix narrows enforcement to *structural* duplicates (different metadata at the same FQM) — that's the real programmer-mistake signal.

## Cross-references

- Memory: `feedback_path_monikers.md` — path-as-key invariant.
- Parent surface: `01KQD6064G1C1RAXDFPJVT1F46`, Layer 2 (`01KQD8XM2T0FWHXANCK0KVDJH1`).
- The warning was added in `swissarmyhammer-focus/src/registry.rs::register_scope` as part of the FQM refactor.

## Workflow

- Investigation-first. Don't write code until step 1's stack trace + step 2's repro have named the offender. The architectural answer (path-as-key) is correct; the bug is in *how* React composes the path, not in the kernel.
