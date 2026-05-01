---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffea80
project: spatial-nav
title: Card fields must render through &lt;Field withIcon /&gt; — match inspector so the FocusZone wraps icon + content uniformly
---
## What

A Field is a Zone with editor/display nested inside. Card and inspector must render fields the same way — the field's `<FocusZone>` wraps **both** the icon and the content. Today they don't, and the inconsistency is visible as different debug-border boxes:

- **Inspector** (correct) — uses `<Field withIcon />`. The icon renders **inside** the field's `<FocusZone>`. The debug border, the focus indicator, and the click-to-focus surface all encompass icon + content.
- **Card** (wrong) — `entity-card.tsx:236` `CardField` renders `<CardFieldIcon>` as a sibling of `<Field>` inside an outer flex wrapper. The icon is **outside** the field's `<FocusZone>`. The debug border encompasses only the edit area; clicking the icon does not focus the field zone.

Both surfaces should produce identical structure. The fix is mechanical: migrate `CardField` to the same `<Field withIcon />` pattern the inspector already uses (`entity-inspector.tsx:325`) and delete the now-redundant `CardFieldIcon` component.

## Where the bug lives

### Card path — `kanban-app/ui/src/components/entity-card.tsx`

Lines 236–294 (`CardField`):

```jsx
const overrideFn = getDisplayIconOverride(field.display ?? "");
const overrideResult = overrideFn ? overrideFn(entity.fields[field.name]) : null;
const resolvedIcon = overrideResult ?? fieldIcon(field);
const tooltipOverrideFn = getDisplayTooltipOverride(field.display ?? "");
const tooltipOverrideResult = tooltipOverrideFn ? tooltipOverrideFn(entity.fields[field.name]) : null;
const hasIcon = !!resolvedIcon;
return (
  <div className={hasIcon ? "flex items-start gap-1.5" : ""}>
    <CardFieldIcon field={field} icon={resolvedIcon} tooltipOverride={tooltipOverrideResult} />
    <div className="flex-1 min-w-0">
      <Field
        fieldDef={field}
        entityType={entity.entity_type}
        entityId={entity.id}
        mode="compact"
        editing={editing}
        onEdit={onEdit}
        onDone={onDone}
        onCancel={onCancel}
        showFocusBar
      />
    </div>
  </div>
);
```

The icon is computed locally and rendered as a sibling of `<Field>`. The flex wrapper isn't a `<FocusZone>` — only the inner `<Field>` is.

`CardFieldIcon` (lines 296–) is a tooltip-wrapped icon badge. Its `tooltipOverride` parameter takes a pre-computed value-dependent tooltip string — same data flow `<FieldIconBadge>` already handles inside `<Field>` via `resolveFieldIconAndTip` (`field.tsx:339`).

### Inspector path — already correct

`entity-inspector.tsx:325` uses `<Field withIcon />`. The docstring at `:248–254` documents the new contract: "the icon — when the field has one — now renders inside that `<FocusZone>` via `<Field withIcon />`, so a click on the icon dispatches `spatial_focus` for the field zone."

### Field's `withIcon` already does the work

`field.tsx:565–569`:

```ts
if (withIcon) {
  const { Icon, tip } = resolveFieldIconAndTip(fieldDef, value);
  ...
  {Icon && <FieldIconBadge Icon={Icon} tip={tip} />}
  ...
}
```

`resolveFieldIconAndTip` (line 339) already implements the same icon-priority chain `CardField` reimplements locally:
1. Display registry's `iconOverride(value)`.
2. Static `field.icon` from YAML.
3. Tooltip: display registry's `tooltipOverride(value)` → static `field.description` → humanised field name.

So the migration is a true simplification — no new logic needed in Field, only deletion in the card path.

## Approach

`kanban-app/ui/src/components/entity-card.tsx`:

1. Replace the `CardField` body with a single `<Field withIcon ... />`. Drop the local icon-resolution block, the local tooltip-resolution block, the `hasIcon` flex wrapper, and the `<CardFieldIcon>` render.
2. Delete the `CardFieldIcon` function (lines 296–end of that helper) and any imports that become unused (`CardFieldIcon`, `getDisplayIconOverride`, `getDisplayTooltipOverride`, `fieldIcon` if only used here).
3. Verify `mode="compact"` continues to be passed — the card's compact mode is still relevant; `withIcon` is independent of mode.

The compact-mode question is worth a sentence: when `withIcon=true && mode="compact"`, does `<Field>` lay the icon out the way card cells expect? `<Field>` renders `[icon, content]` in a horizontal flex row regardless of mode (see `field.tsx:565`). The card cells' `flex items-start gap-1.5` previously imposed a tighter gap (`1.5` = 6 px) than the inspector's `gap-2` (8 px). If `<FieldIconBadge>` inside Field uses a fixed gap, the card may render slightly differently than today. Pin via test:

- If the test fails because the card's icon-to-content gap shifted by 2 px, expose a `gap` prop on `<Field>` (or on `<FieldIconBadge>`) so the card can tighten it. Default to whatever the inspector uses today; the card opts into the tighter spacing.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [x] `<CardField>` renders through `<Field withIcon />` — no separate `<CardFieldIcon>` rendered as a sibling, no outer flex wrapper around `<Field>`.
- [x] The `CardFieldIcon` function is deleted from `entity-card.tsx`.
- [x] In a card cell with an icon, the rendered DOM has the icon as a child of the field's `<FocusZone>` wrapper (not a sibling). Asserted by selecting `[data-segment="field:..."]` and confirming the icon element is a descendant.
- [x] Clicking the icon inside a card field focuses the field zone — `useFocusedScope()` reports the field's moniker, `<FocusIndicator>` renders inside the field zone wrapper. (Mirrors the inspector behavior pinned by `01KQ9ZJHRXCY8Z5YT6RF4SG6EK`.)
- [x] The debug overlay's dashed border around a card field encompasses the icon AND the content (proves the field zone's bounding box now includes the icon).
- [x] No regression on the inspector: existing `<Field withIcon />` usage there continues to render identically. Asserted by re-running existing inspector tests.
- [x] No regression on cards without icons: a field whose `field.icon` is unset (or whose `iconOverride` returns `null` for the current value) renders content only — no empty icon slot, no extra flex wrapper.
- [x] Visual gap between icon and content in the card matches today's layout (within the constraint that the unified `<Field withIcon />` may impose a single gap value across surfaces). The unified `gap-2` from `<Field withIcon />` was acceptable — no override prop needed.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/entity-card.field-icon-inside-zone.browser.test.tsx` (new file)

Mounts an `<EntityCard>` for a task with a known field def that has an `icon` set, inside the production provider stack against the per-test backend.

- [x] `card_field_icon_is_descendant_of_field_zone` — render the card, query `[data-segment="field:task:T1.tags"]`, assert the resolved icon element (the lucide SVG) is a descendant of that wrapper. Today: it is a sibling, NOT a descendant — the test fails before the migration.
- [x] `clicking_card_field_icon_focuses_field_zone` — click the rendered icon element, assert `useFocusedScope()` reports the field's moniker.
- [x] `clicking_card_field_content_focuses_field_zone` — click the content area, same assertion. (Regression guard.)
- [x] `focus_indicator_paints_to_left_of_icon_in_card` — `spatial_focus(fieldZoneKey)`, assert the rendered `[data-testid="focus-indicator"]` is a child of the field zone wrapper, with the icon also a child of that wrapper rendered after the indicator in DOM order.
- [x] `card_field_without_icon_renders_content_only` — render a field with no `icon` and no `iconOverride` returning a value; assert no icon element exists inside the field zone. (Regression guard for icon-less fields.)
- [x] `card_field_icon_uses_value_dependent_iconOverride` — register a display whose `iconOverride(value)` returns a non-default lucide icon for a known value; render the card; assert the override's icon is the one rendered. (Pins the override path that `CardField` previously implemented locally.)
- [x] `card_field_icon_uses_value_dependent_tooltipOverride` — register a display whose `tooltipOverride(value)` returns a non-default tooltip string; render the card; assert the rendered tooltip text matches the override. (Pins the tooltip override path that `CardField` previously implemented locally.)

Test command: `vitest run --project browser entity-card.field-icon-inside-zone.browser.test.tsx` — all seven pass.

### Frontend — augment `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts`

- [x] Add a guard that asserts `entity-card.tsx` does NOT export a `CardFieldIcon` symbol (the function is deleted as part of this migration). Source-level grep guard so a future revert that re-introduces the parallel render path is caught at lint time, not at user-report time.
- [x] Add a guard that asserts `entity-card.tsx` contains exactly zero call sites of `getDisplayIconOverride` and `getDisplayTooltipOverride` — those imports move out as part of the migration. (Same shape: source-level guard.)

Test command: `vitest run --project unit focus-architecture.guards.node.test.ts` — both new guards pass post-migration.

### Frontend — confirm regressions

- [x] `kanban-app/ui/src/components/entity-card.test.tsx` — pre-existing tests pass after the migration. (Three tests in the `field icon tooltips` block were updated to query the new `<FieldIconBadge>` shape rather than the deleted `<CardFieldIcon>` shape — they pinned the OLD render path that no longer exists. New behaviour is asserted by the equivalent post-migration tests in the same block.)
- [x] `kanban-app/ui/src/components/entity-inspector.test.tsx` — pre-existing tests pass after the migration. (No inspector changes here, but worth confirming the shared `<Field withIcon />` path didn't drift.)

Test command: `vitest run --project browser entity-card entity-inspector` — full suites pass except for two pre-existing inspector-drill failures (`enter_on_pill_field_drills_into_first_pill`, `escape_from_pill_drills_back_to_field_zone`) unrelated to this card; verified to fail on the baseline before this migration.

## Workflow

- Use `/tdd` — write `card_field_icon_is_descendant_of_field_zone` first, watch it fail (icon is a sibling, not a descendant), migrate `CardField` to `<Field withIcon />`, watch it pass. Delete `CardFieldIcon`. Confirm regressions.
- Single ticket — one architectural inconsistency, one mechanical migration, one source-level guard so it stays fixed.
- The inspector path landed via `01KQ9ZJHRXCY8Z5YT6RF4SG6EK`; this is the parallel migration on the card surface.

## Review Findings (2026-05-01 07:30)

### Warnings
- [x] `kanban-app/ui/src/components/entity-card.test.tsx:340` — Test `renders an icon badge as a tooltip trigger inside the field zone for a field with an icon` (was `wraps the icon for a described field in a tooltip trigger labelled by the description`) lost its description-text assertion. The pre-migration test asserted `span[aria-label="Task tags"]` — verifying that the static `field.description` ("Task tags") was wired through to the tooltip trigger's accessible name. The rewrite only checks that *some* `span[data-slot="tooltip-trigger"]` exists inside the field zone; the actual description text "Task tags" is no longer asserted anywhere in this suite. The browser test #7 in `entity-card.field-icon-inside-zone.browser.test.tsx` covers the value-dependent `tooltipOverride` path, but the **static description path** (`field.description` → tooltip body) is now untested. Suggested fix: keep the rewrite's zone-descendant query, then additionally hover the trigger and assert the rendered tooltip popover text matches `"Task tags"` — the same shape test #7 uses for the override path. **Resolved 2026-05-01:** test renamed to `renders an icon badge whose tooltip body is the field's static description`; after the existing zone-descendant query it now hovers the trigger (`pointerEnter` + `focus`, 50ms wait) and asserts `document.body.textContent` includes `"Task tags"`. `TooltipProvider` in the shared `renderCard` was given `delayDuration={0}` so the popover mounts deterministically. Verified by `vitest run --project browser entity-card.test.tsx` — 21 / 21 pass.
- [x] `kanban-app/ui/src/components/entity-card.test.tsx:357` — Test `renders an icon badge for every field with an icon (e.g. progress)` (was `falls back to a humanized field name when the field has no description`) entirely lost its purpose. The pre-migration test asserted `span[aria-label="progress"]` — verifying the **humanized-name fallback** kicks in when `field.description` is empty. The rewrite only checks badge existence, which is already covered by the previous test. The humanization fallback (`field.name.replace(/_/g, " ")` — see `resolveFieldIconAndTip` in `field.tsx`) is now untested. Suggested fix: hover the progress trigger and assert the rendered tooltip text equals `"progress"` (or whatever the humanizer produces) — pin the fallback path that lives in shared field code and that previously had a card-side regression guard. **Resolved 2026-05-01:** test renamed to `falls back to the humanized field name when the field has no description (e.g. progress)`; after locating the field-zone trigger it hovers (`pointerEnter` + `focus`, 50ms wait) and asserts `document.body.textContent` includes `"progress"`, pinning the humanizer fallback in `resolveFieldIconAndTip`. Verified by the same `vitest run --project browser entity-card.test.tsx` — 21 / 21 pass.

### Nits
- [x] `kanban-app/ui/src/components/fields/field-icon-badge.tsx:39` — Pre-existing accessibility gap (NOT introduced by this card, but newly visible because the card now shares the inspector's path): the trigger `<span>` carries no `aria-label`. With the legacy `CardFieldIcon` the trigger had `aria-label={tip}`, giving screen readers an accessible name regardless of tooltip-open state; `FieldIconBadge` relies on `<TooltipContent>` only. After this migration the gap exists on **both** card and inspector surfaces. Out of scope for this card — file a follow-up to add `aria-label={tip}` (or `aria-describedby`) to `FieldIconBadge`'s trigger span so the icon has a non-visual accessible name. **Filed 2026-05-01 as follow-up task `01KQHRCKVHK7BAB96JY87KR3T2` ("FieldIconBadge trigger should carry an aria-label for screen readers") in the `spatial-nav` project.**
