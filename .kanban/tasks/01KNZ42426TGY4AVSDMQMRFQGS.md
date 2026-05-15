---
assignees:
- wballard
depends_on:
- 01KNZ40VH9PJ3M9TEFPGFJJRM1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9280
project: pill-via-cm6
title: 'MentionWidget: replace slug text with clipped display name'
---
## What

Add a CM6 `WidgetType` that visually replaces mention slug text with the entity's clipped display name, while keeping the underlying slug text in the document unchanged. When the cursor is inside or adjacent to the mention range, the widget steps aside and the raw slug text is shown (answer 1b from planning).

**Files to modify / create:**
- `kanban-app/ui/src/lib/cm-mention-widget.ts` (new) — export `MentionWidget extends WidgetType` with fields `{ prefix, slug, displayName, color }`. `toDOM()` returns a `<span>` with the pill styling (inline-flex, rounded-full, color-mix background/border/text) and inner text `${prefix}${clipDisplayName(displayName)}`. `eq(other)` compares all four fields. `ignoreEvent()` returns false so click events propagate (context menu / focus will be handled by a containing React FocusScope via DOM events).
- `kanban-app/ui/src/lib/cm-mention-widget.ts` — export `clipDisplayName(name: string, maxChars = 24): string` — truncates with "…" at 24 chars (tune as needed). Add a small pure test.
- `kanban-app/ui/src/lib/cm-mention-decorations.ts` — change `decorateLine` to emit `Decoration.replace({ widget, inclusive: false })` with a `MentionWidget` built from `metaMap.get(slug)`. Entities not found in metaMap → fall back to `Decoration.mark` with muted styling (answer 3: "stale entity, raw slug muted"). Add an `atomicRanges` extension so cursor skips over the widget as a single unit.
- `kanban-app/ui/src/lib/cm-mention-decorations.ts` — selection-aware build: trigger `buildDecorations` on `update.selectionSet` as well. For any mention range that contains the selection head OR is adjacent to it (±1 char), emit the fallback `Decoration.mark` instead of the widget so the user can edit characters.
- `kanban-app/ui/src/lib/cm-mention-decorations.ts` — update `buildMentionTheme` so both the widget's DOM class AND the fallback mark share the same pill styling (one source of truth for pill CSS).

**Behavior summary:**
- Read-only viewer (no selection): every mention renders as a widget showing `$Display Name (clipped)`.
- Editable viewer, cursor elsewhere: same as above.
- Editable viewer, cursor inside/adjacent to a mention: that mention degrades to raw `$slug` with mark styling; cursor navigates character-by-character.
- Stale slug (no metaMap entry): raw slug with muted mark styling.

**CSS details:** Widget DOM uses the same class the baseTheme already styles (`.cm-tag-pill` etc.). The widget's text content is the full pill label; there is no separate `::before` / pseudo-element for the prefix — it's just part of the string.

**Out of scope for this card:** The actual consumers (MarkdownDisplay, BadgeDisplay) still render their existing pill paths. This card only changes what happens inside editors that already use `useMentionExtensions()`.

## Acceptance Criteria
- [ ] New file `lib/cm-mention-widget.ts` exports `MentionWidget` and `clipDisplayName`
- [ ] CM6 editors using mention extensions render widgets showing display names (clipped) instead of slug text
- [ ] Moving the cursor into a mention range reverts it to raw `$slug` display
- [ ] Moving the cursor away re-applies the widget
- [ ] `atomicRanges` makes left/right arrow skip over a widget in one step when cursor is outside
- [ ] Missing/stale slugs render as muted raw-slug marks (no widget)
- [ ] All existing CM6 mention tests still pass (may need updates for the new widget shape)

## Tests
- [ ] `kanban-app/ui/src/lib/cm-mention-widget.test.ts` (new) — unit test `clipDisplayName`: short name unchanged, long name clipped with "…", edge cases (exactly 24, 25 chars)
- [ ] `kanban-app/ui/src/lib/cm-mention-widget.test.ts` — instantiate `MentionWidget`, call `toDOM()`, assert the returned element's `textContent`, class name, and inline style carry the right color
- [ ] `kanban-app/ui/src/lib/cm-mention-decorations.test.ts` — (extending the file from the prior card) test the full widget pipeline: mount an EditorView with a known mention, assert the widget DOM is produced; move the selection head into the mention range, assert the widget is replaced with a mark decoration; move selection away, assert widget returns
- [ ] `kanban-app/ui/src/lib/cm-mention-decorations.test.ts` — assert stale slug (not in metaMap) renders as muted mark, not widget
- [ ] Run: `bun test cm-mention-widget cm-mention-decorations use-mention-extensions` — all pass
- [ ] Smoke: `bun run dev`, open a card with mentions in the description editor — confirm pills show full names when cursor is elsewhere and collapse to slugs when cursor is on them

## Workflow
- Use `/tdd` — start with `clipDisplayName` unit tests (trivial, fast green). Then `MentionWidget.toDOM()` test. Then the integration test that mounts an EditorView and asserts widget vs mark behavior driven by selection. Each test gets its own red → green step.