---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffa480
title: Normalize compact field-display heights for virtualized grid rows
---
## What

Visual regression discovered while testing the `DataTable` row-virtualization work (task `01KQ1XWCWHT7C4T71T02V99AGP`). The virtualizer assumes a fixed `ROW_HEIGHT = 32px` per row, but compact-mode field displays render at different heights depending on whether the cell is populated or empty. With the virtualizer reserving exactly 32px per row, taller populated cells overflow their reserved slot — causing rows to render at visibly inconsistent heights and the absolute scroll position to drift from the actual rendered positions.

Two confirmed offenders, both surfaced by the user:

1. **`tags` field** (`kanban-app/ui/src/components/fields/displays/badge-list-display.tsx`) — empty state renders `<span className="text-muted-foreground/50">{field.placeholder ?? "-"}</span>` (~16-18px content height); populated state renders CM6-rendered `MentionView` pills which are visibly taller. The YAML `placeholder: "Add tags"` is wired up correctly — the issue is purely the height mismatch between the plain `<span>` and the CM6 widget pipeline.

2. **`assignees` field** (`kanban-app/ui/src/components/fields/displays/avatar-display.tsx`):
   - **Bug A**: hardcodes `<span className="text-muted-foreground/50">-</span>` for the empty state and **ignores `field.placeholder` entirely** — unlike `BadgeListDisplay` and `BadgeDisplay` which honor it. Inconsistent with the established placeholder convention.
   - **Bug B**: `swissarmyhammer-kanban/builtin/definitions/assignees.yaml` has no `placeholder` field — needs one added (e.g. `placeholder: "Assign"`).
   - **Bug C**: populated state renders `<Avatar size="md" />` which is `w-7 h-7` = 28px (see `kanban-app/ui/src/components/avatar.tsx:24`). Plus row `py-1.5` padding (6px × 2 = 12px), the populated cell is ~40px tall, while the empty `<span>` is ~18px. Hence the height divergence.

The right fix is to normalize compact-mode display heights at the display-component level so populated and empty variants render the same height. This is correct independent of virtualization (the variable heights would have caused subtle row-jitter before too — virtualization just made it loud), and it preserves the perf win of fixed-height virtualization without falling back to `measureElement`.

**Files:**
- Modify: `kanban-app/ui/src/components/fields/displays/avatar-display.tsx` — add `field` to props (matching `DisplayProps`), use `field.placeholder` in the empty-state branch (mirror `BadgeListDisplay`'s `EmptyBadgeList` pattern), and ensure both populated and empty variants share a fixed compact-mode height.
- Modify: `swissarmyhammer-kanban/builtin/definitions/assignees.yaml` — add `placeholder: "Assign"` (or whichever string the team prefers — match the imperative tone of `"Add tags"`).
- Modify: `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` — wrap `EmptyBadgeList`'s compact branch (and probably the `MentionView` branch too, though `MentionView` may already be height-stable) so both render at the same fixed compact height.
- Audit (read-only): every other display in `kanban-app/ui/src/components/fields/displays/*.tsx` — `text-display`, `badge-display`, `date-display`, `markdown-display`, `number-display`, `progress-display`, `progress-ring-display`, `status-date-display`, `virtual-tag-display`, `attachment-display`, `color-swatch-display`. For each, verify the populated and empty variants render at the same height in `mode="compact"`. Fix any that diverge.
- Verify: `kanban-app/ui/src/components/data-table.tsx` `ROW_HEIGHT` constant matches the normalized cell-content height + `py-1.5` padding. Adjust the constant if the normalized height changes the math.

**Approach:**

1. Pick a target compact-mode content height. The smallest currently-wired compact display sets the floor — `Avatar size="md"` at 28px is the largest non-CM6 element. A 20–24px content height (text-line + small badge) is a sensible target; verify by measuring the actual rendered row in dev. Encode it as a shared Tailwind class (e.g. `h-6 flex items-center` on every compact-mode wrapper) or a shared `CompactCellWrapper` component. Prefer the wrapper component if more than two displays need the treatment — DRY beats copy-paste.
2. For `AvatarDisplay`: shrink to `size="sm"` (`w-5 h-5` = 20px) when in `mode="compact"` — the existing `md` size is too tall for grid cells anyway. Match the height with the empty `<span>`.
3. For `BadgeListDisplay`: `MentionView` in compact mode already renders inline pills; verify their height. If they exceed the target, constrain via the wrapper.
4. For `AvatarDisplay`'s placeholder: add `field` to `AvatarDisplayProps`, then `<span>{field.placeholder ?? "-"}</span>` mirrors `BadgeListDisplay`'s pattern. Also pass `mode` so the empty-state can use `text-sm italic` for `mode="full"` like `EmptyBadgeList` does.
5. Update `assignees.yaml` with `placeholder: "Assign"`. Verify the schema reload picks it up (no Rust changes needed — it's a string field on `FieldDef` already).
6. After the displays are normalized, re-measure `ROW_HEIGHT` against a real running grid (`cd kanban-app && bun tauri dev`) and adjust the constant in `data-table.tsx` if needed.

**Out of scope:**
- Switching the virtualizer to dynamic `measureElement` — fixed-height virtualization is the right tradeoff once content heights are uniform.
- Refactoring the display registration system or the `DisplayProps` interface beyond adding what's missing.
- Touching the editor variants (these only render in `mode="edit"` which isn't subject to row-height invariants the same way).

## Resolution Summary (2026-04-25)

Implemented a shared `CompactCellWrapper` component (`kanban-app/ui/src/components/fields/displays/compact-cell-wrapper.tsx`) — a `data-compact-cell="true"` `<div>` with `h-6 flex items-center overflow-hidden`. Every display now wraps its compact-mode output in this shell so populated and empty variants render at exactly the same pixel height (24px content + 12px row `py-1.5` padding = 36px row).

Wrapped displays: `text-display`, `date-display`, `markdown-display`, `number-display`, `progress-display`, `progress-ring-display`, `status-date-display`, `virtual-tag-display`, `attachment-display` (single + list), `color-swatch-display`, `avatar-display`, `badge-display`, `badge-list-display`. `attachment-display`'s compact branch was simplified to a one-line summary (icon + name/count) — the full drop-zone UI stayed reserved for `mode="full"`.

`AvatarDisplay` now reads `field.placeholder`, accepts `mode`, mirrors `EmptyBadgeList`'s `text-sm italic`/dash convention, and shrinks the `Avatar` to `size="sm"` (20px) in compact mode so it fits inside the 24px wrapper height.

`assignees.yaml` declares `placeholder: "Assign"`. `data-table.tsx::ROW_HEIGHT` updated 32→36 to match the new content height.

`ProgressRingDisplay` shrinks the ring from 28px to 20px in compact mode (kept at 28px in full mode) so it fits in the wrapper without overflow clipping.

## Acceptance Criteria

- [x] `assignees.yaml` declares `placeholder: "Assign"` (or agreed string).
- [x] `AvatarDisplay` reads `field.placeholder` and renders it in the empty-state span (with `-` as the legacy fallback) — matching the convention in `BadgeListDisplay`/`BadgeDisplay`.
- [x] `AvatarDisplay` accepts `mode` and uses `text-sm italic` styling for the `mode="full"` empty state, mirroring `EmptyBadgeList`.
- [x] In a grid view, every row is the **exact same pixel height** regardless of which fields are populated, empty, or contain tags/assignees. No visible jitter when scrolling. Verified structurally in `data-table.virtualized.test.tsx` (Tailwind utilities are not bundled into the test browser; structural test asserts wrapper class-name equality across populated/empty rows, which produces the same pixel height once CSS is applied). Manual `bun tauri dev` smoke deferred to reviewer (no GUI in agent environment).
- [x] In `mode="compact"`, the rendered height of every display under `kanban-app/ui/src/components/fields/displays/` is identical for both populated and empty value states — every display now wraps its compact-mode output in `CompactCellWrapper`.
- [x] `ROW_HEIGHT` in `data-table.tsx` matches the actual rendered row height — updated 32→36 (24px wrapper height + 12px `py-1.5` padding).
- [x] No new console warnings/errors during initial mount or scroll. (Pre-existing `act(…)` warnings from `SchemaProvider` mount are unchanged; no new diagnostics introduced.)

## Tests

- [x] Add a test in `kanban-app/ui/src/components/fields/displays/avatar-display.test.tsx` (create the file if needed) asserting that with `field.placeholder = "Assign"` and `value = []`, the empty-state span renders the placeholder text — not `-`. Also assert that with `value = []` and no placeholder, `-` is the fallback (regression guard).
- [x] Extend `data-table.virtualized.test.tsx` (added by the virt task): render two `DataTable`s — one with all rows populated, one with all rows empty — and assert the rendered `<tr>` heights are equal. (Implemented as structural assertion: both grids must emit `[data-compact-cell="true"]` wrappers with identical class names — Tailwind isn't bundled in browser-mode tests, so visual height equality is asserted via the class-name proxy that produces it.)
- [x] Run `cd kanban-app/ui && bun run test fields/displays` — all display tests still pass. (171/171 passing; verified via `npx vitest run --project browser src/components/fields/displays`.)
- [x] Run `cd kanban-app/ui && bun run test data-table` — virtualization tests still pass with the (possibly updated) `ROW_HEIGHT`. (4/4 passing.)
- [x] Run full UI suite: `cd kanban-app/ui && bun run test` — no new failures, no new warnings. (1382/1382 passing post-review-fixes; TypeScript build clean.)
- [ ] Manual smoke: `cd kanban-app && bun tauri dev`, open the task grid view (rows have varying tag/assignee populations), scroll. All rows render at identical height with no jitter. — DEFERRED to reviewer; no GUI available in agent environment.

## Workflow

- Use `/tdd` — write the AvatarDisplay placeholder test first (RED), wire up `field.placeholder` (GREEN), then move to the height-normalization audit. Add the height-equality test in `data-table.virtualized.test.tsx` before normalizing, so it fails with the current state and passes after the fix.

#performance #frontend #kanban-app #bug

## Review Findings (2026-04-25 05:31)

### Warnings
- [x] `kanban-app/ui/src/components/fields/displays/progress-ring-display.tsx:39` — `ProgressRingDisplay` returns `null` (no wrapper) for `total === 0` in compact mode, breaking the "every compact display emits a `[data-compact-cell="true"]` wrapper even for empty/invalid values" invariant that this task explicitly applied to `ProgressDisplay` (`progress-display.tsx:18-25`) and `StatusDateDisplay` (`status-date-display.tsx:226-230`). The `progress-ring-display.test.tsx:45-50` test pins the inconsistent behavior. In practice the row height is preserved because other cells in the same row carry the wrapper, but a row composed entirely of empty `progress-ring` cells (e.g. a single-column grid filtered to that field) would collapse below `ROW_HEIGHT`. Suggested fix: mirror `ProgressDisplay` — wrap a muted dash (or empty content) in `CompactCellWrapper` when `mode === "compact"` and `total === 0`/value is invalid; preserve `null` only for `mode === "full"`. Update the test to assert the wrapper is present.
  - **Resolved (2026-04-25 05:38):** Extracted an `EmptyProgressRingCompact` helper that emits a muted dash inside `CompactCellWrapper`. Both empty branches in `ProgressRingDisplay` (invalid value + `total === 0`) now return that helper in compact mode and `null` only in full mode. RED-first: extended `progress-ring-display.test.tsx` with three new compact-mode wrapper assertions (null value, non-object value, total=0) — initial run failed 3/9, post-fix 9/9 passing. Three sibling full-mode tests also added to lock the `null` return for inspector rows.

### Nits
- [x] `kanban-app/ui/src/components/fields/displays/attachment-display.tsx:271, :339` — `AttachmentDisplay` and `AttachmentListDisplay` hardcode `-` in their empty-state spans rather than honoring `field.placeholder`. Inconsistent with the new `AvatarDisplay`/`BadgeDisplay`/`BadgeListDisplay` placeholder convention. The task scope didn't require fixing every display's placeholder behavior, but since the wrapper was being added everywhere, this was a natural place to also adopt the convention. Leave for a follow-up if not in scope.
  - **Resolved (2026-04-25 05:38):** Added optional `field?: FieldDef` to both `AttachmentDisplayProps` and `AttachmentListDisplayProps`. Both empty compact-mode spans now render `field?.placeholder ?? "-"`, matching the `AvatarDisplay`/`BadgeDisplay`/`BadgeListDisplay` convention. The `attachment.tsx` registration adapter was updated to plumb `field` through. Full-mode behavior was intentionally left unchanged because it's an interactive drop zone with "Drop file here"/"Drop files here" text — semantically different from a passive empty placeholder, so the italic-`None` convention doesn't fit there. Added regression tests in `attachment-display.test.tsx` for both single + list variants asserting the YAML-configured placeholder surfaces in compact mode (34/34 passing).
- [x] `kanban-app/ui/src/components/fields/displays/avatar-display.tsx:33` — `EmptyAvatar` uses an inline anonymous prop type `{ mode: ...; placeholder?: ... }`. JS_TS_REVIEW.md says "Even for 2-prop components — the named interface is the documentation." Pattern matches the pre-existing `EmptyBadgeList` (badge-list-display.tsx:89), so leaving it consistent with the codebase is defensible — but a future cleanup could add `interface EmptyAvatarProps` / `interface EmptyBadgeListProps` together.
  - **Resolved (2026-04-25 05:38):** Extracted `interface EmptyAvatarProps` in `avatar-display.tsx` and `interface EmptyBadgeListProps` in `badge-list-display.tsx`, with JSDoc on both fields (`mode`, `placeholder?`). Both internal helpers now use the named interface — matches the `JS_TS_REVIEW.md` convention.
- [x] `kanban-app/ui/src/components/data-table.tsx:46-48` — The docstring on `ROW_HEIGHT` says "Keep this constant in lock-step with `COMPACT_CELL_HEIGHT_CLASS` (in `compact-cell-wrapper.tsx`)" but the lock-step is enforced only by manual review — `COMPACT_CELL_HEIGHT_CLASS` is exported but never imported here. A small improvement would be to either (a) import `COMPACT_CELL_HEIGHT_CLASS` and reference it in the comment to make the link discoverable via "find references", or (b) export a single source of truth (e.g. `COMPACT_ROW_HEIGHT_PX = 36` from `compact-cell-wrapper.tsx`) and import it in both places.
  - **Resolved (2026-04-25 05:38):** Took option (b). Added `export const COMPACT_ROW_HEIGHT_PX = 36` in `compact-cell-wrapper.tsx` with a docstring explaining the derivation (24px content + 12px padding). `data-table.tsx::ROW_HEIGHT` now imports and uses it instead of the bare `36` literal — single source of truth, picked up by "find references", changes propagate automatically.

### Final test gate (post-review-fixes, 2026-04-25 05:39)
- TypeScript build: clean (`npx tsc --noEmit` = 0 diagnostics).
- Vitest: 125 files, 1382 tests, 0 failures, 0 new warnings (5 new tests added vs. the 1377 baseline: 3 progress-ring wrapper assertions + 1 single-attachment placeholder + 1 attachment-list placeholder).
