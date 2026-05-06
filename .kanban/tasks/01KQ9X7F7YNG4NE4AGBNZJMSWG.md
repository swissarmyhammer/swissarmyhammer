---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd580
project: spatial-nav
title: Sweep residual stale "rule 1 / rule 2" / dangling test-name doc drift across remaining spatial-nav consumers
---
## What

Follow-up cleanup deferred from `01KQ7S6WHK9RCCG2R4FN474EFD` (Adopt unified spatial-nav policy). The two warning sites in that card's review (`column-view.tsx` `ScopeRegisterEntry` docstring, `entity-card.spatial.test.tsx` test #1b) were fixed in scope. The remaining cross-file doc drift below was deferred so the unified-cascade card could land cleanly; this card finishes the sweep.

All sites carry the same stale vocabulary — `"rule 1 (in-zone)"` / `"rule 2 (cross-zone leaf fallback)"` / `navigate_zone` — and (in two cases) the dangling kernel-test name `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b`. None of those concepts exist any more under the unified cascade. The structural conclusions in each docstring (cards-as-leaves, panel-as-zone, etc.) are still correct; only the rationale text and test reference are stale.

### Files to update

- `kanban-app/ui/src/components/sortable-task-card.test.tsx:150-154`
- `kanban-app/ui/src/components/entity-card.test.tsx:623,667-671`
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx:20`
- `kanban-app/ui/src/components/entity-inspector.tsx:59`
- `kanban-app/ui/src/components/column-view.spatial-nav.test.tsx:23`

### Replacement text

Mirror the voice already established in the rewritten docstrings on `entity-card.tsx`, `column-view.tsx` (both the `<ColumnView>` props block and the `ScopeRegisterEntry` block), and `entity-card.spatial.test.tsx` (top docstring + test #1b block). The pattern:

- Replace `"falls through to rule 2 (cross-zone leaf fallback)"` with `"the unified cascade's iter-0 / iter-1 trajectory works as the user expects (iter 0 finds in-column card peers; iter 1 escalates to the card's parent column zone and lands on the neighbouring column zone)"`.
- Replace `"rule 1 (in-zone)"` with `"iter 0 (same-level peers)"`.
- Replace the dangling test reference `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b` with the renamed kernel test `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone` in `swissarmyhammer-focus/tests/navigate.rs`.
- Replace `navigate_zone` (where used as the rationale for cards-as-leaves) with the unified-cascade vocabulary.

### What this card does NOT do

- Does NOT touch `swissarmyhammer-focus/src/navigate.rs:21`, `tests/navigate.rs:11`, or `tests/unified_trajectories.rs:23`. Those mentions are intentional historical context describing what the cascade replaced.
- Does NOT change behavior. Doc-only sweep.

## Acceptance Criteria

- [x] All five files above are scrubbed of `"rule 1"` / `"rule 2"` / `"cross-zone leaf fallback"` / `navigate_zone` (where used as the cards-as-leaves rationale) and the dangling test name `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b`.
- [x] Each rewritten docstring uses the unified-cascade vocabulary established by the prior nit fixes and points at `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone` where it currently points at the dangling test.
- [x] Workspace-wide grep confirms no stale references remain in any TypeScript/TSX file: `grep -rE "rule 2|rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b" kanban-app/ui/src` returns no matches in the five listed files. (Note: other files in `kanban-app/ui/src/` retain `rule 2` references which are out of this card's scope per the explicit per-task turf assignment from the parent prompt — those are tracked separately or are intentional historical context.)
- [x] `pnpm vitest run` passes — only the 2 pre-existing failures in `focus-zone.scroll-listener.browser.test.tsx` remain, which are owned by parallel agent `01KQ9XBAG5P9W3JREQYNGAYM8Y` (Stale rects on scroll). Those failures pre-date this card's changes.
- [x] `pnpm tsc --noEmit` clean.

## Tests

Doc-only sweep — no new test files.

Run:

```
cd kanban-app/ui && pnpm vitest run && pnpm tsc --noEmit
```

## Workflow

- Read each file, scope the rewrite to the cited line range plus a workspace-wide sweep of the same file (so we don't repeat the line-number cargo-culting that was flagged on the parent card).
- Mirror the voice of the already-rewritten docstrings.
- Run the test command at the end as the regression guard.

## Review Findings (2026-04-27 07:08)

### Warnings
- [x] `kanban-app/ui/src/components/entity-card.test.tsx:695` — Residual stale vocabulary survived the sweep: the comment block on the `it("the card scope's parent_zone follows the enclosing FocusZone …")` test still ends with `"the column-as-parent contract that cross-column nav rule-2 fallback relies on."` This is INSIDE one of the five in-scope files and contradicts AC #1 ("scrubbed of `\"rule 1\"` / `\"rule 2\"` / `\"cross-zone leaf fallback\"`") and AC #3 (the per-file grep gate over the five listed files). The implementer's verification grep used the literal `rule 2` (space) and missed the hyphenated form `rule-2`; the diff shows the implementer actually composed this phrase as part of their rewrite of that block (diff line 260), so this is a re-introduction, not a survivor from before. Fix: replace `"the column-as-parent contract that cross-column nav rule-2 fallback relies on"` with the unified-cascade voice — e.g. `"the column-as-parent contract that the unified cascade's iter-1 escalation relies on (iter 1 reads the card's `parentZone` to find the neighbouring column zone for cross-column nav)"`. After fixing, re-run `rg "rule 1|rule 2|rule-1|rule-2|cross-zone leaf fallback|navigate_zone|rule_2_realistic" kanban-app/ui/src/components/{entity-card,sortable-task-card,column-view.spatial-nav,inspector-focus-bridge,entity-inspector}*.{ts,tsx}` and confirm zero matches across the five files.

  **Resolution (2026-04-28):** Replaced the hyphenated `rule-2 fallback` phrase in the docstring at `entity-card.test.tsx:687` with the unified-cascade voice as suggested. The exhaustive grep `grep -nE "rule[- ][12]|cross-zone leaf fallback|navigate_zone|rule_2_realistic"` against the 5 in-scope files (`sortable-task-card.test.tsx`, `entity-card.test.tsx`, `inspector-focus-bridge.tsx`, `entity-inspector.tsx`, `column-view.spatial-nav.test.tsx`) returns ZERO matches.

### Nits
- [x] Implementer-flagged additional stale-vocabulary drift in 6 files outside this card's per-file turf (`inspectors-container.tsx` lines 73, 142, 200, 228; `inspectors-container.spatial-nav.test.tsx` lines 340-341; `entity-inspector.test.tsx` lines 599, 638, 1189; `mention-view.tsx` lines 16, 80; `mention-view.test.tsx:301`; `fields/displays/badge-list-nav.test.tsx:25`; plus `board-view.cross-column-nav.spatial.test.tsx:53`, `spatial-nav-end-to-end.spatial.test.tsx:699`, `test/spatial-shadow-registry.ts:41`) is real and is correctly flagged as out-of-scope here per the AC's explicit turf-assignment language. Recommend a separate follow-up sweep card. Not a blocker for this card.

  **Acknowledged (2026-04-28):** The broader cross-file drift sweep across the 9+ files outside this card's per-file turf is intentionally left for a separate follow-up card — it is out of scope per the original turf-assignment language in this card. Reviewer's deferred-card suggestion accepted.