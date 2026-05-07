---
assignees:
- wballard
depends_on:
- 01KQW69GDFYZ1QYV9TMBD5F9RR
- 01KQW6BJZ6DTZSHKKEDP5TEG4E
- 01KQW6D6B2JXPA4PX6H94R86KB
position_column: doing
position_ordinal: '8280'
project: spatial-nav
title: 'spatial-nav redesign step 9: dev-mode dual-source verification — zero divergence under automated coverage'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Bake-in period before cutover. Run the snapshot path and the registry path side-by-side in dev builds, and prove via **automated coverage** that they produce identical results across every realistic op sequence. This is the gate before steps 10–13 delete the registry path.

No manual hour-on-the-keyboard protocol. Manual exploration finds bugs in minutes if it finds them at all; after that it just consumes time and proves nothing reproducible. The deliverable is automated tests that catch divergence today and keep catching it forever.

## Status against this card's bar (as of commit `475379968`)

This card was prematurely closed and re-opened. Commit `475379968` ("consolidate divergence harness + automated soak suite") shipped most of the deliverables; the remaining gaps are listed under "What's left to ship" below. The implementer should NOT redo what's already in.

### What's already shipped

- ✅ `swissarmyhammer-focus/src/divergence.rs` — the `compare_paths` harness with debug + release variants and 3 unit tests.
- ✅ `compare_paths` wired into `spatial_navigate`, `spatial_focus`, `spatial_focus_lost` via `kanban-app/src/commands.rs::check_navigate_divergence` / `check_focus_divergence` / `check_focus_lost_divergence`.
- ✅ `swissarmyhammer-focus/tests/spatial_nav_soak.rs` — 8 Rust integration scenarios with tracing capture asserting zero `spatial-nav snapshot/registry divergence` events: arrow nav, click focus, drag-drop, filter hide, inspector lifecycle, modal lifecycle, bulk delete, all-in-sequence.
- ✅ `kanban-app/ui/src/spatial-nav-soak.spatial.test.tsx` — 6 active browser-mode IPC-shape tests over the same scope.
- ✅ One real production bug caught and fixed during authoring (bulk-delete fixture vs `spatial_focus_lost` listener semantics).

### What's left to ship before this card closes

- ❌ **`swissarmyhammer-focus/tests/dual_path_fuzz.rs`** — `proptest`-driven test over a random sequence of `{mount, unmount, focus, navigate, drag-rect-shift, filter-mass-unmount, layer-push, layer-pop}` ops. Each op runs through both paths; assertion is `FocusChangedEvent` equality. 10,000 sequences per CI run, with proptest's shrinking turning any failure into a minimal repro. **This is the hard prerequisite for closure** — hand-written scenarios, even 8 of them, only cover the cases the author thought of. Proptest's job is to find the cases the author didn't think of.
- ❌ **Delete the 4 `it.skip` placeholders** in `kanban-app/ui/src/spatial-nav-soak.spatial.test.tsx` (currently named for "real OS drag-drop, modal Escape via real AppShell, real inspector lifecycle, sustained continuous interaction"). They are skipped because they document scenarios the automated suite cannot exercise — but a skipped test is dead code that rots and gives false confidence ("we have a test for that"). Either rewrite each as a smaller-scope automated test that *does* run, or delete it. No skips ship.
- ❌ **Drop any closing-checklist mention of a manual ≥1-hour soak.** The commit message for `475379968` includes the line *"Manual gate before step 10/cutover (NOT satisfied by this commit): the user must run pnpm tauri dev for ≥1 hour and confirm `just logs | grep \"snapshot/registry divergence\"` produces zero output."* That gate is rejected. The fuzz suite + the 8 automated scenarios + the divergence-as-CI-failure are the gate. There is no human in the gate.

## What to build (full reference, including what's already shipped — for context)

### 1. Divergence harness (`compare_paths`) — SHIPPED

Steps 6–8 each added their own divergence diagnostic. Consolidated into one dev-mode harness in `swissarmyhammer-focus/src/divergence.rs`:

```rust
#[cfg(debug_assertions)]
fn compare_paths<R, F1, F2>(op: &str, snapshot_path: F1, registry_path: F2) -> R
where
    R: PartialEq + std::fmt::Debug,
    F1: FnOnce() -> R,
    F2: FnOnce() -> R,
{
    let snapshot_result = snapshot_path();
    let registry_result = registry_path();
    if snapshot_result != registry_result {
        tracing::warn!(
            op = %op,
            snapshot = ?snapshot_result,
            registry = ?registry_result,
            "spatial-nav snapshot/registry divergence",
        );
    }
    snapshot_result
}
```

Wired into `spatial_navigate`, `spatial_focus`, `spatial_focus_lost`. Release builds run only the snapshot path.

### 2. Browser-mode integration coverage — MOSTLY SHIPPED

The 8 Rust + 6 TS scenarios already cover: arrow nav across directions, click focus on every scope kind, drag-drop, filter-hide, inspector lifecycle, modal lifecycle, bulk delete, all-in-sequence. The 4 skipped TS placeholders need to be either rewritten to run automated or deleted (see "What's left to ship" #2).

### 3. Property / fuzz test — NOT YET SHIPPED

`swissarmyhammer-focus/tests/dual_path_fuzz.rs` — a `proptest`-driven test that:

- Generates a random sequence of ops drawn from `{mount, unmount, focus, navigate, drag-rect-shift, filter-mass-unmount, layer-push, layer-pop}`.
- Applies each op through both the snapshot path and the registry path.
- Asserts the resulting `FocusChangedEvent` (or its absence) is identical between paths after every op.
- Shrinks any failing sequence to a minimal repro.

10,000 randomized sequences per CI run. Random op order is what catches the bugs that hand-driven sequences miss; proptest's shrinking turns a 200-op failure into a 3-op repro you can paste into a focused unit test.

### 4. CI gate — SHIPPED

The tracing capture layer in `spatial_nav_soak.rs` fails any test that produces a `spatial-nav snapshot/registry divergence` event. The fuzz suite (section 3) installs the same layer.

### 5. Bug-fix loop

Every divergence the suite finds is an architectural defect — fix in the appropriate step's code (steps 3, 4, 5 most likely), add the shrunken repro as a focused regression test, re-run. Repeat until the suite is silent across at least one full CI cycle.

## Out of scope

- Cutover (steps 10–13) starts only after this step's CI is green AND the fuzz suite is silent.
- Removing the dev-mode-only branches of `compare_paths` — that happens in step 11 when the registry path goes away.

## Acceptance criteria

- [x] `compare_paths` harness exists and is wired into `spatial_navigate`, `spatial_focus`, `spatial_focus_lost`. (commit `475379968`)
- [x] Browser-mode + Rust integration suite covers the seven scenario classes listed in section 2, each asserts zero divergence warnings. (commit `475379968`)
- [ ] `dual_path_fuzz.rs` runs 10,000 proptest sequences in CI and reports zero failures across at least one full nightly cycle.
- [ ] Zero `it.skip` placeholders remain in `kanban-app/ui/src/spatial-nav-soak.spatial.test.tsx`.
- [ ] No closing checklist references a manual ≥1-hour soak.
- [ ] Any divergence found during development has a shrunken-repro regression test added before this card closes.
- [x] CI fails on any `spatial-nav snapshot/registry divergence` tracing event during the test suite. (commit `475379968`)

## Tests

- [x] `swissarmyhammer-focus/src/divergence.rs` — the `compare_paths` harness. (shipped)
- [ ] `swissarmyhammer-focus/tests/dual_path_fuzz.rs` — proptest fuzz over op sequences. (not yet)
- [x] `swissarmyhammer-focus/tests/spatial_nav_soak.rs` — 8 integration scenarios. (shipped)
- [x] `kanban-app/ui/src/spatial-nav-soak.spatial.test.tsx` — 6 browser-mode tests. (shipped, but 4 skipped placeholders to remove or rewrite)
- [ ] CI test command: `cargo nextest run -p swissarmyhammer-focus --test dual_path_fuzz` — green over 10,000 sequences.

## Workflow

- Use `/tdd` — write `dual_path_fuzz.rs` first against a trivial 2-op set (`mount`, `focus`); let proptest run; expand the op generator one variant at a time, fixing every divergence proptest finds at the appropriate layer.
- After the fuzz test is silent, walk `spatial-nav-soak.spatial.test.tsx` and decide each `it.skip` placeholder's fate — rewrite as automated, or delete. No skips remain.

#stateless-nav