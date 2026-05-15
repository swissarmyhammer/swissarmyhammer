---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: cc80
project: spatial-nav
title: Up from board column collapses focus to engine root instead of reaching perspective bar
---
## What

Reported behavior: with focus on the column-name area in a board column (focused FQM segment is `column:{id}`, e.g. `column:todo`), pressing `ArrowUp` (`nav.up`) leaves the user with no visible focus indicator anywhere on screen. The expected destination — the visibly-above perspective bar (`ui:perspective-bar`, its tabs, or the filter editor) — is never reached.

Sample log:
```
2026-05-03 15:41:45.985295-0500   command  args=Some(Object {"scope_chain": Array [
  String("column:todo"),
  String("ui:board"),
  String("board:board"),
  String("view:01JMVIEW0000000000BOARD0"),
  String("ui:perspective"),
  String("perspective:01KNAGCS70G61X2JW682RWYEV5"),
  String("perspective:01KNAGCS70G61X2JW682RWYEV5"),
  String("board:board"),
  String("store:/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/.kanban"),
  String("mode:normal"),
  String("window:board-01kqdzgz26ejbrdg2h9nxce6te"),
  String("engine"),
]}) board_path=None cmd=ui.setFocus scope_chain=Some(["engine"]) target=None
```

The result `scope_chain=Some(["engine"])` and `target=None` indicates focus collapsed to the engine root — the same broken outcome reported for `Left` from the leftmost perspective tab in task `01KQPW1FTYFWTDMW6ESM5ABGJQ`.

User clarified the expected behaviour: "what i'd expect is that i go up to some part of the perspective bar". Specifically, `Up` from `column:{id}` should land on the visibly-above `ui:perspective-bar` zone (or one of its leaves — `perspective_tab:{id}` or `filter_editor:{id}`).

### Same root cause as `01KQPW1FTYFWTDMW6ESM5ABGJQ`

Both bugs share the symptom: cross-zone cardinal nav from a leaf/zone whose `parent_zone` has no in-direction peer at iter 0 returns `target=None` instead of returning either:

- The visibly-adjacent peer zone via iter 1 (peer-zone search).
- The drill-out fallback to the parent zone.

Per `swissarmyhammer-focus/README.md`'s cascade contract:

1. Iter 0 — any-kind in-zone peer of `column:todo` inside `ui:board` searching `Up`: no in-zone peer above the column header within the board zone → miss.
2. Iter 1 — same-kind peer-zone search at `ui:board`'s level: peer zones at the layer root include `ui:navbar` (top), `ui:perspective-bar` (just above the board), `ui:left-nav` (left). `ui:perspective-bar` is the closest geometrically-above peer → should win the beam score.
3. Drill-out fallback — if iter 1 also misses, return `ui:board`'s parent zone (`ui:perspective` per the chain).

None of those should produce `["engine"]`. Whatever fixes `01KQPW1FTYFWTDMW6ESM5ABGJQ` (cross-zone iter-1 beam scoring or drill-into-leaf-on-cross-zone) almost certainly fixes this too — the symptom and shape match.

### What this task adds

Treat this as an **additional regression case** for the same fix. The work itself rolls into `01KQPW1FTYFWTDMW6ESM5ABGJQ`'s implementation, but this task contributes a second, distinct regression test that pins:

- Direction `Up` (the original task pinned `Left`).
- Starting FQM kind = zone (`column:{id}`) — the original was a leaf (`perspective_tab:{id}`).
- Expected destination is `ui:perspective-bar` (or a leaf inside it), not `ui:left-nav`.

Two regression tests with different direction × kind × destination combinations dramatically reduce the chance the fix is over-tuned to one shape.

### Files to read first

- `swissarmyhammer-focus/src/navigate.rs` — `BeamNavStrategy::next` cascade.
- `swissarmyhammer-focus/tests/fixtures/mod.rs` — realistic-app fixture. Currently registers `ui:navbar` and `ui:perspective-bar`; needs `ui:left-nav` (per `01KQPW1FTYFWTDMW6ESM5ABGJQ`) and column zones inside `ui:board` for this test to land.
- `swissarmyhammer-focus/tests/perspective_bar_arrow_nav.rs` — sibling integration test where `01KQPW1FTYFWTDMW6ESM5ABGJQ`'s regression lives.
- `kanban-app/ui/src/components/column-view.tsx:547` — the column-name `<Field>` is the sole spatial-nav registration for the column header (`field:column:<id>.name`). Confirm whether the user's focused FQM is the column zone (`column:{id}`) or the inner name field (`field:column:{id}.name`); the log shows `column:todo` so it's the column zone level.

### Dependency

This task **depends on `01KQPW1FTYFWTDMW6ESM5ABGJQ`** — both bugs share the underlying cascade fix. Implement that task first; this task adds the Up regression and verifies the fix generalizes.

If `01KQPW1FTYFWTDMW6ESM5ABGJQ`'s fix already lands a generalised cross-zone behaviour (drill-into-leaf-on-cross-zone, or correct iter-1 peer-zone scoring), the only thing this task contributes is the additional regression test plus any fixture changes needed to model `column:{id}` zones inside `ui:board`.

## Acceptance Criteria

- [ ] In a layout with `ui:perspective-bar` above `ui:board`, and `column:{id}` zones inside `ui:board`, `BeamNavStrategy::next(registry, column:todo, segment, Direction::Up)` returns a `FullyQualifiedMoniker` whose path lies within `ui:perspective-bar` (the zone itself or a leaf inside it). It MUST NOT return the focused FQM, the layer root, `engine`, or `ui:board`.
- [ ] In the running app, pressing `ArrowUp` while focus is on a column header (`column:{id}`) moves visible focus into the perspective bar — the user observes a `<FocusIndicator>` paint on a tab leaf, the filter editor leaf, or another visible target inside `ui:perspective-bar`. The `ui.setFocus` IPC carries a non-`None` `target` and a `scope_chain` containing at least one `ui:perspective-bar`-anchored frame above `engine`.
- [ ] No regression: existing `perspective_bar_arrow_nav.rs` tests stay green, plus the new `01KQPW1FTYFWTDMW6ESM5ABGJQ` regression for `Left` from leftmost perspective tab.

## Tests

- [ ] If `01KQPW1FTYFWTDMW6ESM5ABGJQ`'s fixture work did not already include column zones inside `ui:board`, extend `swissarmyhammer-focus/tests/fixtures/mod.rs` `RealisticApp` to register at least one `column:{id}` zone inside `ui:board` with the production geometry (sized box near the top of the board area). Expose an accessor like `column_todo_fq()`.
- [ ] Add `#[test] fn column_up_from_top_of_board_lands_in_perspective_bar()` in a new file `swissarmyhammer-focus/tests/board_column_arrow_nav.rs` (or extend `perspective_bar_arrow_nav.rs` if scope-creep is acceptable). Assert `nav(column_todo_fq(), Up)` returns either `perspective_bar_fq()` itself or one of its leaves. Explicitly assert it does NOT return `ui:board`, `engine`, the layer root, or the focused FQM.
- [ ] Add a defensive regression `#[test] fn column_up_never_collapses_to_layer_root()` asserting the result is not `app.layer_root_fq()` and not `app.engine_root_fq()`.
- [ ] Run `cargo test -p swissarmyhammer-focus board_column_arrow_nav perspective_bar_arrow_nav` and confirm both new and existing tests pass.
- [ ] Run the full `cargo test -p swissarmyhammer-focus` to catch any cross-test regressions.

## Workflow

- Schedule **after** `01KQPW1FTYFWTDMW6ESM5ABGJQ`'s cascade fix lands.
- Use `/tdd` — extend the realistic-app fixture with column zones if not already done by the parent fix, write the failing `column_up_from_top_of_board_lands_in_perspective_bar` regression (RED), confirm the parent fix already turns it GREEN. If RED persists, the parent fix needs additional generalisation — file a follow-up against the parent task rather than diverging here.
