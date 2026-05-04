---
assignees:
- claude-code
depends_on:
- 01KQSDP4ZJY5ERAJ68TFPVFRRE
- 01KQSEDYSJT9J8Y1N8JYX7TQ12
position_column: doing
position_ordinal: '80'
project: spatial-nav
title: 'Spatial-nav follow-up D: sweep tests + rewrite README for single FocusScope primitive'
---
## Reference

Parent: `01KQSDP4ZJY5ERAJ68TFPVFRRE`. Predecessor: sub-task C (`01KQSEDYSJT9J8Y1N8JYX7TQ12`) — JSX sweep and `focus-zone.tsx` deletion must land first.

After this task lands, the entire spatial-nav redesign is observably complete: every test file references `FocusScope` only, the README describes the single-primitive model, and `pnpm -C kanban-app/ui test` plus the kernel test suite plus typecheck plus clippy are all clean. The four cross-zone bug fixes from the redesign still work end-to-end.

## What

Test sweep across both Rust and TypeScript sides (~120 files), plus README rewrite. The implementer who attempted the single-PR collapse measured 143 source files affected; sub-tasks A/B/C handle the production-code surface, this sub-task handles everything else.

### Files to update

#### Rust integration tests (already updated in sub-task A)

- `swissarmyhammer-focus/tests/*.rs` were updated as part of sub-task A. Verify they still pass after sub-tasks B/C — they should, but a paranoid `cargo test -p swissarmyhammer-focus` rerun confirms.

#### TypeScript test files

Run `grep -rln "FocusZone\|FocusZoneContext\|useParentZoneFq\|spatial_register_zone\|<FocusZone" kanban-app/ui/src --include='*.test.*'` to confirm the complete list. Known categories:

- `kanban-app/ui/src/components/focus-zone.test.tsx` — DELETE entirely (the file under test no longer exists). Migrate any unique scenarios (e.g. context propagation tests) into `focus-scope.test.tsx` if they're not already covered.
- `kanban-app/ui/src/components/focus-scope.test.tsx` — sweep any `is_zone()` / `FocusZone` references.
- `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts` — guards that explicitly assert on the zone vs leaf distinction; reframe as "scope with children" / "scope without children" or delete obsolete guards.
- `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts` — same.
- `kanban-app/ui/src/components/inspectors-container.guards.node.test.ts` — same.
- `kanban-app/ui/src/components/board-view.guards.node.test.ts` — sweep zone references.
- All `*.spatial.test.tsx` test files — these typically assert on register IPC counts and segment shapes; replace `register_zone` with `register_scope` in mock matchers.
- All `*.spatial-nav.test.tsx` test files — same.
- All `*.browser.test.tsx` files that mock the spatial IPC — same.
- All component tests that wrap mounts in `<SpatialFocusProvider>` + the zone harness.

#### Documentation

- `swissarmyhammer-focus/README.md` — rewrite the prose distinguishing zones from leaves. New framing: "All registered things are `FocusScope`s. A scope with no children behaves as a leaf; a scope with children behaves as a container." Update every section that references zones (cardinal nav, drill in, drill out, first/last, scrolling, coordinate system) so the prose lines up with the implementation.
- `swissarmyhammer-focus/src/lib.rs` crate-root doc-comment — same prose update if not already done in sub-task A.
- Any `### Reference` / "## Architecture" section in markdown files under the repo that mentions zones explicitly — update language. Run `grep -rln "FocusZone\|focus zone" --include='*.md'` to find them.

### Out of scope for this sub-task

- Production code — sub-tasks A/B/C handle all production callsites.
- Behaviour changes — pure refactor sweep.

### Decision dependencies (already approved by user)

- The `scope-not-leaf` guard tests (in `focus-architecture.guards.node.test.ts` and similar) were enforcing the now-vacuous kind distinction. The guard files themselves can either delete those specific tests with rationale, or replace them with "no-children-of-leaf-FQM" structural assertions (which is the new equivalent invariant — leaves with children get caught by drill-in / first / last operating on children). Either approach is fine; document the choice in the commit message.

## Acceptance Criteria

- [ ] `grep -r "FocusZone\|FocusZoneContext\|useParentZoneFq" kanban-app/ui/src` returns no matches (production OR test).
- [ ] `grep -r "spatial_register_zone" swissarmyhammer-focus/src kanban-app/ui/src kanban-app/src` returns no matches.
- [ ] `grep -r "is_zone\|::Zone" swissarmyhammer-focus/src kanban-app/ui/src` returns no matches in source code (markdown task descriptions historical references excluded).
- [ ] `kanban-app/ui/src/components/focus-zone.test.tsx` is deleted.
- [ ] `swissarmyhammer-focus/README.md` is rewritten for the single-primitive model.
- [ ] `cargo test -p swissarmyhammer-focus`: zero failures.
- [ ] `cargo nextest run` (full workspace): zero failures.
- [ ] `cargo clippy --all-targets -- -D warnings` (full workspace): clean.
- [ ] `pnpm -C kanban-app/ui exec tsc --noEmit`: clean across entire codebase.
- [ ] `pnpm -C kanban-app/ui test`: zero failures (1 pre-existing skip in `entity-focus.kernel-projection.test.tsx` is fine).
- [ ] The four cross-zone regression tests in `swissarmyhammer-focus/tests/cross_zone_geometric_nav.rs` still pass — observable proof the spatial-nav redesign behaviour is preserved end-to-end.
- [ ] Drill / first / last assertions in `swissarmyhammer-focus/tests/drill.rs` still pass.
- [ ] Coordinate-consistency invariants in `swissarmyhammer-focus/tests/coordinate_invariants.rs` still pass.
- [ ] Scroll-on-edge tests in `kanban-app/ui/src/lib/scroll-on-edge.test.ts` and `column-view.virtualized-nav.browser.test.tsx` still pass.

## Tests

- [ ] Run the full test suite and confirm zero failures: `cargo nextest run && cargo clippy --all-targets -- -D warnings && pnpm -C kanban-app/ui test && pnpm -C kanban-app/ui exec tsc --noEmit`.
- [ ] Spot-check the four cross-zone regression assertions: Left from leftmost perspective_tab → leaf inside ui:left-nav; Up from column → leaf inside ui:perspective-bar; Down from perspective_tab → leaf inside perspective body; Up from column header → ui:perspective-bar.
- [ ] Spot-check drill / first / last on a zone-with-children and a scope-without-children — the unified primitive must behave identically to the pre-collapse split.

## Workflow

- Land sub-task C first — every test file imports from production code, and that surface only stabilizes after C.
- Mechanical sweep: `grep` for the deprecated identifiers, replace systematically, run typecheck after each batch.
- For guard files (`*.guards.node.test.ts`) that explicitly enforce the kind distinction, decide per-file: reframe as "has children" / "no children" structural guard, or delete with rationale.
- Rewrite the kernel README last — it depends on the final API surface settling.
- After all the sweeps, run the full test suite plus typecheck plus clippy, and verify all acceptance criteria's grep checks.
- If a guard test enforces an invariant that genuinely no longer applies (e.g. "no scope-not-leaf violation"), DELETE the test with a commit-message rationale tying it back to the parent task.
#spatial-nav-redesign