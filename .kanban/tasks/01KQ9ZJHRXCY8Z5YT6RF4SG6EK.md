---
assignees:
- claude-code
depends_on:
- 01KQ9X3A9NMRYK50GWP4S4ZMJ4
- 01KQ9XBAG5P9W3JREQYNGAYM8Y
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffdb80
project: spatial-nav
title: Inspector field navigation must work end-to-end — icon inside the focus zone, vertical nav, Enter drills in to pills or edit mode
---
## What

Inspector field rows must behave as a single focusable, navigable surface. Today three things are wrong:

1. **The field icon is outside the field's `<FocusZone>`**, so clicking the icon does not focus the field, and the focus bar paints between the icon and the field content rather than to the left of the icon.
2. **Up / Down arrow nav between sibling fields is unreliable** (likely the rects-on-scroll bug `01KQ9XBAG5P9W3JREQYNGAYM8Y` plus inspector panel scrolling).
3. **Enter on a pill / badge-list field doesn't drill into the pills.** `01KQ9X3A9NMRYK50GWP4S4ZMJ4` introduces a scope-level `field.edit` bound to Enter that always enters edit mode — but for fields with spatial children (pills), Enter should drill in to focus the first pill, then arrow-nav among pills.

This ticket fixes all three so a user clicking, arrow-keying, and pressing Enter on inspector fields gets the experience the project promised.

## Where the bugs live

### Bug 1 — Icon outside the focus zone

`kanban-app/ui/src/components/entity-inspector.tsx:325–333` (`FieldRow`):

```jsx
<div className="flex items-start gap-2">
  {Icon && <FieldIconTooltip Icon={Icon} tip={tip} />}
  <div className="flex-1 min-w-0">{content}</div>
</div>
```

The icon sits as a sibling of `<FieldContent>` (which renders `<Field>`). `<Field>` (`kanban-app/ui/src/components/fields/field.tsx:410`) wraps its contents in `<Inspectable><FocusZone moniker="field:...">…</FocusZone></Inspectable>` — so the focus zone covers only the content branch, not the icon. Consequences:

- A click on `<FieldIconTooltip>`'s icon does not bubble to any `<FocusZone>` / `<FocusScope>`, so spatial focus does not move to the field.
- The `<FocusIndicator>` paints `-left-2` from the field zone's left edge — between the icon and the content. The user wants the bar to the LEFT of the icon.

**Fix shape**: keep the `<FocusZone>` wrap **inside `<Field>`** (single seam — every `<Field>` callsite participates in the spatial graph the same way) and **make the icon an optional part of `<Field>` itself**. The icon, when shown, renders as the leftmost child inside the existing `<FocusZone>` so:

- The icon, the content, and the indicator all share one containing block.
- Click on icon bubbles to the zone's click handler → focus moves to the field zone.
- The indicator at `-left-2` paints to the left of the icon — exactly what the user wants.

This is deliberately **not** lifting the wrap up to FieldRow. Lifting up means every callsite needs to be updated to provide its own zone, and the codebase has many `<Field>` callsites (inspector, card cells, grid cells, navbar percent-complete, possibly others). Missing one creates a non-spatial inspector field that we won't notice until a user reports it. Concentrating the wrap in `<Field>` keeps the contract single-seamed.

### Bug 2 — Vertical nav between fields

The inspector panel is a `<FocusZone moniker="panel:type:id">` containing a stack of field zones. Beam search "down" from one field zone should pick the next field zone by rect. This requires accurate rects on every field zone, which today are stale when the inspector panel scrolls (the inspector body is scrollable — see `slide-panel.tsx:45` `overflow-y-auto`).

The rects-on-scroll fix in `01KQ9XBAG5P9W3JREQYNGAYM8Y` (this ticket's dependency) addresses the registration side. Once that lands, vertical nav should work for inspector fields. This ticket pins the contract via tests so the inspector-specific case is verified, not assumed.

### Bug 3 — Enter on pill fields must drill in, not edit

`01KQ9X3A9NMRYK50GWP4S4ZMJ4` introduces `field.edit` bound to Enter at the field-zone scope level. That shadows the global `nav.drillIn` for every focused field zone. For fields whose display renders pills (`badge-list-display.tsx` → `MentionView` → each pill is a `<FocusScope>` leaf inside the field zone — see `mention-view.tsx:263–272`), the user wants Enter to drill in to the first pill, then arrow-nav among pills (in-zone beam search), then Escape to drill back to the field zone.

The current `field.edit` always calls `onEdit`; for a pill field, `onEdit` opens an editor where applicable, which is not what the user wants — they want to navigate the existing pills.

Fix: change `field.edit`'s execute closure to **drill in first, then edit only on null kernel result**:

```ts
{
  id: "field.edit",
  name: "Edit Field",
  keys: { vim: "Enter", cua: "Enter" },
  execute: async () => {
    // Pills (or any spatial children) win — drill into them.
    const moniker = await actions.drillIn(fieldZoneKey);
    if (moniker !== null) {
      setFocus(moniker);
      return;
    }
    // No spatial children — fall through to edit mode.
    onEdit?.();
  },
}
```

This unifies the two behaviours: a field with pills drills in (kernel returns the first pill), a field without pills enters edit mode (kernel returns null, fallback fires). The same scope-level command serves both — no per-display branching at registration time.

For non-editable fields with no pills (`onEdit` undefined, no spatial children), Enter is a no-op — consistent with "leaves with no editor have nothing to drill into".

## Approach

### 1. Move icon rendering into `<Field>`

`kanban-app/ui/src/components/fields/field.tsx`:

- Add an optional `withIcon?: boolean` prop (defaults to `false` — backwards-compatible for every existing callsite that doesn't opt in). Wire through `FieldProps` and pass it into the render.
- When `withIcon === true`, resolve the icon inside `<Field>` itself: read `fieldDef.icon` for the static lucide name and call the display registry's `iconOverride` (using the value Field already subscribes to via `useFieldValue`). Mirror the inspector's existing `HelpCircle` fallback when the static name doesn't resolve. Likewise compute the tooltip via the existing `getDisplayTooltipOverride` + `field.description` chain.
- Render the icon as the leftmost child inside the existing `<FocusZone>` — alongside the editor or display content in a horizontal flex row. Use the same `<FieldIconTooltip>` component the inspector currently uses (move it from `entity-inspector.tsx` into a shared location, e.g. `kanban-app/ui/src/components/fields/field-icon-badge.tsx`, so both the legacy `entity-inspector.tsx` callers — if any remain after this PR — and the new in-`Field` rendering share one source).
- Keep `<Field>`'s existing `<Inspectable>` + `<FocusZone>` wrap structure exactly as today. Only the inner content shape changes: it becomes `[icon?, content]` in a flex row instead of just `[content]`.

### 2. Update `<FieldRow>` in `entity-inspector.tsx` to use the new prop

`kanban-app/ui/src/components/entity-inspector.tsx`:

- Remove the outer `<div className="flex items-start gap-2">` icon-and-content wrap.
- Remove the icon resolution + `<FieldIconTooltip>` rendering — those move to `<Field>`.
- The row becomes essentially `<Field withIcon ... />`. The `data-testid={\`field-row-${field.name}\`}` attribute moves to whatever wrapper remains (the field zone div itself, via the `<Field>` passthrough HTML attrs, or a thin div around `<Field>` if attribute placement matters for tests).

### 3. Extend `field.edit` to drill into pills first

`kanban-app/ui/src/components/fields/field.tsx` (where `01KQ9X3A9NMRYK50GWP4S4ZMJ4` will register the scope-level `field.edit`):

- Change the `execute` closure to call `actions.drillIn(fieldZoneKey)` first.
- On non-null result: `setFocus(moniker)` and return.
- On null result: fall through to `onEdit?.()`.

The closure needs access to the field zone's `SpatialKey`. The existing `<FocusZone>` mints that key in a ref (`focus-zone.tsx:367`); thread it through to the command's execute via the same closure-capture pattern `nav.drillIn` uses in `app-shell.tsx:333`. The `field.edit` registration happens inside the body of `<FocusZone>` itself — the simplest path is to have the FocusZone build the scope-level commands array and forward the key + drill action automatically, so `<Field>` only has to declare "I want field-edit semantics on Enter" and pass `onEdit`.

If that turns out to be too much primitive churn, an acceptable fallback: read the field zone's key via context (the same way `useParentZoneKey` does for descendants) — Field is the consumer of its own `<FocusZone>`, so it's a small change.

### 4. Verify vertical nav

`01KQ9XBAG5P9W3JREQYNGAYM8Y` brings inspector field rects into sync with scroll. This ticket adds the inspector-specific test coverage so the contract is observable.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

### Click + focus bar (Bug 1)

- [x] Clicking a field row's **icon** inside the inspector focuses the field zone. `useFocusedScope()` reports the field's moniker, the `<FocusIndicator>` renders, and `data-focused="true"` flips on the field zone's wrapper.
- [x] Clicking a field row's **content** (label, value, pill area) inside the inspector focuses the field zone. Same assertions.
- [x] The rendered `<FocusIndicator>` element is positioned to the **left of the icon**, not between the icon and the content. Asserted via DOM order: the indicator's host is the field-zone wrapper, the icon is the first content child inside that wrapper, and the indicator's `-left-2` offset places it before the icon's left edge.
- [x] No regression: every existing `<Field>` callsite (card cells, grid cells, navbar percent-complete, etc.) continues to render exactly as today — no icon appears unless the callsite opts in via `withIcon`. The default value of `withIcon` is `false`, and the existing callsites do not pass it.

### Up / Down nav (Bug 2)

- [x] `nav.down` from a focused field zone in the inspector lands on the next field zone in document order.
- [x] `nav.up` from a focused field zone lands on the previous field zone.
- [x] After scrolling the inspector panel body, `nav.down` and `nav.up` continue to pick the correct sibling. (Regression guard against the rects-on-scroll bug; depends on `01KQ9XBAG5P9W3JREQYNGAYM8Y`.)
- [x] `nav.down` from the last visible field zone scrolls the inspector body to bring the next field into view, then settles focus on that field. (Mirrors the column-view pattern.)

### Enter drill-in semantics (Bug 3)

- [x] Enter on a focused field zone whose display renders **pills** drills into the first pill — `useFocusedScope()` reports the first pill's moniker, and the field stays in display mode (does not enter edit mode).
- [x] After drilling into pills, `nav.right` from the first pill lands on the second pill; `nav.left` walks back; arrow nav works in both directions across all pills in the field.
- [x] Escape from a focused pill drills out to the field zone (existing drill-out chain).
- [x] Enter on a focused field zone whose display has **no spatial children** and is editable enters edit mode — `editing` flips to `true`, `<FieldEditor>` mounts, DOM focus lands on the editor input. (Regression guard for the existing `field.edit` from `01KQ9X3A9NMRYK50GWP4S4ZMJ4`.)
- [x] Enter on a focused field zone whose display has no spatial children AND is non-editable is a no-op — no edit mode, no inspector dispatch, no error.

### Side effects guarded

- [x] Double-click on the field zone (icon or content) still dispatches `ui.inspect` for the field's moniker (regression guard for the inspector-on-dblclick contract). Implicit through the unchanged `<Inspectable>` wrap inside `<Field>`; covered by existing `inspectable.spatial.test.tsx`.
- [x] Space on the focused field zone dispatches `ui.inspect` (depends on `01KQ9XJ4XGKVW24EZSQCA6K3E2` Space-on-Inspectable; if not landed yet, this assertion can be deferred to that ticket). Covered by existing `inspector-field.space-inspect.browser.test.tsx`.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/fields/field.with-icon.browser.test.tsx` (new file)

Mounts `<Field withIcon ... />` inside the production provider stack against the per-test backend.

- [x] `field_with_icon_renders_icon_inside_focus_zone` — render `<Field withIcon ... />` for a field def with `icon` set, query the rendered tree, assert the icon element is a descendant of the `[data-moniker="field:..."]` zone wrapper (NOT a sibling).
- [x] `field_without_with_icon_prop_renders_no_icon` — render the same field with `withIcon={false}` (or omitted); assert no icon element appears inside the zone. (Regression guard for non-inspector callers.)
- [x] `clicking_icon_inside_field_focuses_field_zone` — render `<Field withIcon ... />`, click the icon, assert `useFocusedScope()` reports the field moniker.
- [x] `clicking_content_inside_field_focuses_field_zone` — same with click on the content area.
- [x] `focus_indicator_paints_to_left_of_icon` — focus the field zone via `spatial_focus(key)`, assert the rendered `[data-testid="focus-indicator"]` is a sibling of the icon inside the zone wrapper (the indicator's `-left-2` places it visually to the left).
- [x] `field_icon_uses_static_yaml_icon_when_no_override` — assert the rendered icon matches the lucide component for the field def's `icon` name.
- [x] `field_icon_uses_display_registry_iconOverride_when_provided` — register a display with an `iconOverride(value)` that returns a different icon for a known value; render the field with that value; assert the override's icon renders.
- [x] `field_icon_falls_back_to_HelpCircle_for_unknown_icon_name` — set `field.icon` to a name that does not resolve to a lucide component; assert `HelpCircle` renders. (Mirrors the inspector's legacy fallback.)

Test command: `bun run test:browser fields/field.with-icon.browser.test.tsx` — all eight pass.

### Frontend — `kanban-app/ui/src/components/entity-inspector.field-vertical-nav.browser.test.tsx` (new file)

- [x] `down_from_first_field_lands_on_second_field` — focus the first inspector field, fire `keydown { key: "ArrowDown" }`, assert `useFocusedScope()` reports the second field's moniker.
- [x] `up_from_last_field_lands_on_previous_field` — symmetric.
- [x] `down_after_scroll_picks_next_field_in_content_order` — render an inspector with enough fields to require panel scrolling; scroll the inspector body; press ArrowDown; assert the next field gains focus. (Depends on `01KQ9XBAG5P9W3JREQYNGAYM8Y`.)
- [x] `down_at_last_visible_field_scrolls_to_bring_next_field_into_view` — focus the last on-screen field, ArrowDown, assert the panel scrolled and focus landed on the now-visible next field.

Test command: `bun run test:browser entity-inspector.field-vertical-nav.browser.test.tsx` — all four pass.

### Frontend — `kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx` (new file)

Builds an inspector for a task with both pill fields (e.g. `tags`) and editable scalar fields (e.g. `name`).

- [x] `enter_on_pill_field_drills_into_first_pill` — focus the tags field zone, fire `keydown { key: "Enter" }`, assert `useFocusedScope()` reports the first pill's moniker (e.g. `tag:<id>`) and `editing` on the field is still `false`.
- [x] `right_from_first_pill_lands_on_second_pill` — after drilling in, fire ArrowRight, assert focus moves to the next pill.
- [x] `escape_from_pill_drills_back_to_field_zone` — fire Escape on the focused pill, assert focus returns to the field zone moniker (existing drill-out chain).
- [x] `enter_on_editable_scalar_field_enters_edit_mode` — focus the name field, Enter, assert `editing` flipped to `true` and `document.activeElement` is the editor input. (Regression guard for `01KQ9X3A9NMRYK50GWP4S4ZMJ4`.)
- [x] `enter_on_non_editable_field_with_no_pills_is_noop` — focus a `editor: "none"` field with no display children, Enter, assert nothing changes — no edit, no dispatch, no focus move.
- [x] `enter_on_pill_field_with_zero_pills_falls_through_to_edit_or_noop` — when the field is editable but the value is empty (no pills rendered), Enter falls through to edit mode (if `onEdit` is wired) or is a no-op (otherwise). Pin the contract under test so future implementers don't accidentally invert it.

Test command: `bun run test:browser entity-inspector.field-enter-drill.browser.test.tsx` — all six pass.

### Rust kernel — augment `swissarmyhammer-focus/tests/drill.rs`

- [x] `drill_in_field_zone_with_pill_children_returns_first_pill_moniker` — register a field zone with three pill scopes as children at horizontally-progressing rects; assert `drill_in(fieldZoneKey)` returns the leftmost pill's moniker.
- [x] `drill_in_field_zone_with_no_children_returns_none` — register a field zone with no spatial children; assert `drill_in` returns None.

Test command: `cargo test -p swissarmyhammer-focus --test drill drill_in_field_zone` — both pass.

## Workflow

- Use `/tdd` — write the click-icon, indicator-position, and Enter-on-pill failing tests first, watch them fail, then move icon resolution into `<Field>` and update `field.edit`'s execute closure.
- Single ticket — three observable failures of the same surface concern (inspector field interaction). The fix concentrates on `<Field>` (icon + drill-in semantics) plus a one-callsite update in `entity-inspector.tsx`. No other callsite changes — every existing `<Field>` consumer continues to work because `withIcon` defaults to `false`.
- Land after `01KQ9X3A9NMRYK50GWP4S4ZMJ4` (introduces `field.edit`) and `01KQ9XBAG5P9W3JREQYNGAYM8Y` (rects-on-scroll). The Enter-drill-into-pills logic extends `field.edit`; the vertical-nav-after-scroll case relies on the rects fix.