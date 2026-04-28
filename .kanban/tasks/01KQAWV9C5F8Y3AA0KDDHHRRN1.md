---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
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

- [ ] `<CardField>` renders through `<Field withIcon />` — no separate `<CardFieldIcon>` rendered as a sibling, no outer flex wrapper around `<Field>`.
- [ ] The `CardFieldIcon` function is deleted from `entity-card.tsx`.
- [ ] In a card cell with an icon, the rendered DOM has the icon as a child of the field's `<FocusZone>` wrapper (not a sibling). Asserted by selecting `[data-moniker="field:..."]` and confirming the icon element is a descendant.
- [ ] Clicking the icon inside a card field focuses the field zone — `useFocusedScope()` reports the field's moniker, `<FocusIndicator>` renders inside the field zone wrapper. (Mirrors the inspector behavior pinned by `01KQ9ZJHRXCY8Z5YT6RF4SG6EK`.)
- [ ] The debug overlay's dashed border around a card field encompasses the icon AND the content (proves the field zone's bounding box now includes the icon).
- [ ] No regression on the inspector: existing `<Field withIcon />` usage there continues to render identically. Asserted by re-running existing inspector tests.
- [ ] No regression on cards without icons: a field whose `field.icon` is unset (or whose `iconOverride` returns `null` for the current value) renders content only — no empty icon slot, no extra flex wrapper.
- [ ] Visual gap between icon and content in the card matches today's layout (within the constraint that the unified `<Field withIcon />` may impose a single gap value across surfaces). If the gap shifts visibly, expose a prop on `<Field>` (or the icon badge) so the card can tighten it.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/entity-card.field-icon-inside-zone.browser.test.tsx` (new file)

Mounts an `<EntityCard>` for a task with a known field def that has an `icon` set, inside the production provider stack against the per-test backend.

- [ ] `card_field_icon_is_descendant_of_field_zone` — render the card, query `[data-moniker="field:task:T1.<name>"]`, assert the resolved icon element (the lucide SVG) is a descendant of that wrapper. Today: it is a sibling, NOT a descendant — the test fails before the migration.
- [ ] `clicking_card_field_icon_focuses_field_zone` — click the rendered icon element, assert `useFocusedScope()` reports the field's moniker.
- [ ] `clicking_card_field_content_focuses_field_zone` — click the content area, same assertion. (Regression guard.)
- [ ] `focus_indicator_paints_to_left_of_icon_in_card` — `spatial_focus(fieldZoneKey)`, assert the rendered `[data-testid="focus-indicator"]` is a child of the field zone wrapper, with the icon also a child of that wrapper rendered after the indicator in DOM order.
- [ ] `card_field_without_icon_renders_content_only` — render a field with no `icon` and no `iconOverride` returning a value; assert no icon element exists inside the field zone. (Regression guard for icon-less fields.)
- [ ] `card_field_icon_uses_value_dependent_iconOverride` — register a display whose `iconOverride(value)` returns a non-default lucide icon for a known value; render the card; assert the override's icon is the one rendered. (Pins the override path that `CardField` previously implemented locally.)
- [ ] `card_field_icon_uses_value_dependent_tooltipOverride` — register a display whose `tooltipOverride(value)` returns a non-default tooltip string; render the card; assert the rendered tooltip text matches the override. (Pins the tooltip override path that `CardField` previously implemented locally.)

Test command: `bun run test:browser entity-card.field-icon-inside-zone.browser.test.tsx` — all seven pass.

### Frontend — augment `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts`

- [ ] Add a guard that asserts `entity-card.tsx` does NOT export a `CardFieldIcon` symbol (the function is deleted as part of this migration). Source-level grep guard so a future revert that re-introduces the parallel render path is caught at lint time, not at user-report time.
- [ ] Add a guard that asserts `entity-card.tsx` contains exactly zero call sites of `getDisplayIconOverride` and `getDisplayTooltipOverride` — those imports move out as part of the migration. (Same shape: source-level guard.)

Test command: `bun run test focus-architecture.guards.node.test.ts` — both new guards pass post-migration.

### Frontend — confirm regressions

- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` — pre-existing tests pass after the migration.
- [ ] `kanban-app/ui/src/components/entity-inspector.test.tsx` — pre-existing tests pass after the migration. (No inspector changes here, but worth confirming the shared `<Field withIcon />` path didn't drift.)

Test command: `bun run test:browser entity-card entity-inspector` — full suites pass.

## Workflow

- Use `/tdd` — write `card_field_icon_is_descendant_of_field_zone` first, watch it fail (icon is a sibling, not a descendant), migrate `CardField` to `<Field withIcon />`, watch it pass. Delete `CardFieldIcon`. Confirm regressions.
- Single ticket — one architectural inconsistency, one mechanical migration, one source-level guard so it stays fixed.
- The inspector path landed via `01KQ9ZJHRXCY8Z5YT6RF4SG6EK`; this is the parallel migration on the card surface.
