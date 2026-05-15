---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffd980
title: Extract DisplayText primitive from TextDisplay, use it in StatusDateDisplay for consistent text rendering
---
## What

StatusDateDisplay renders its own `<span>` elements with inconsistent classes compared to TextDisplay — `text-xs text-muted-foreground` in compact mode (smaller and gray) vs TextDisplay's `truncate block` (inherited size/color), and vestigial `inline-flex items-center gap-*` wrappers in both modes left over from when the icon lived inside the display.

Extract a shared `DisplayText` component from `text-display.tsx` that both `TextDisplay` and `StatusDateDisplay` delegate to for text rendering. This ensures all field text is consistent in sizing, color, and layout.

### Approach

**New component `DisplayText`** in `kanban-app/ui/src/components/fields/displays/text-display.tsx`:

```tsx
/**
 * Shared text rendering primitive for field displays.
 *
 * Ensures consistent sizing, color, truncation, and empty-state rendering
 * across all display components. Displays that compute a string value
 * delegate here instead of rolling their own `<span>` with ad-hoc classes.
 */
export function DisplayText({ text, mode, title }: {
  text: string;
  mode: \"compact\" | \"full\";
  title?: string;
}) {
  if (!text) return <span className=\"text-muted-foreground/50\">-</span>;
  if (mode === \"compact\") return <span className=\"truncate block\" title={title}>{text}</span>;
  return <span className=\"text-sm\" title={title}>{text}</span>;
}
```

Then `TextDisplay` becomes a thin wrapper: stringifies the value, delegates to `DisplayText`. And `StatusDateDisplay` computes its phrase, then renders `<DisplayText text={phrase} mode={mode} title={parsed.timestamp} />` — removing the `inline-flex` wrappers, `text-xs`, and `text-muted-foreground` classes entirely.

### Files to modify

1. **`kanban-app/ui/src/components/fields/displays/text-display.tsx`** — Extract `DisplayText` component. Refactor `TextDisplay` to delegate to it. Export `DisplayText` alongside `TextDisplay` and `DisplayProps`.

2. **`kanban-app/ui/src/components/fields/displays/status-date-display.tsx`** — Import `DisplayText`. Replace the custom `<span>` JSX in `StatusDateDisplay` with `<DisplayText text={phrase} mode={mode} title={parsed.timestamp} />`. Remove the `inline-flex items-center gap-*` wrappers from both compact and full modes.

3. **`kanban-app/ui/src/components/fields/displays/index.ts`** — Add `DisplayText` to the barrel export.

## Acceptance Criteria

- [x] `DisplayText` is exported from `text-display.tsx` and `displays/index.ts`
- [x] `TextDisplay` delegates to `DisplayText` — no duplication of span/class logic
- [x] `StatusDateDisplay` uses `DisplayText` — no inline `text-xs`, `text-muted-foreground`, or `inline-flex` wrapper classes
- [x] Status date text in compact mode matches TextDisplay compact: same font size (inherited, not `text-xs`), same color (inherited, not `text-muted-foreground`), same truncation behavior
- [x] Status date text in full mode matches TextDisplay full: `text-sm`, no `inline-flex` wrapper
- [x] The ISO timestamp `title` tooltip is preserved on StatusDateDisplay via `DisplayText`'s `title` prop
- [x] Empty status date values still collapse (existing `isEmpty` predicate path unchanged)

## Tests

- [x] **`kanban-app/ui/src/components/fields/displays/text-display.test.tsx`** (new file) — Test `DisplayText` directly: empty text renders `-` with `text-muted-foreground/50`, compact mode renders with `truncate block`, full mode renders with `text-sm`, `title` prop is passed through as HTML attribute
- [x] **`kanban-app/ui/src/components/fields/displays/status-date-display.test.tsx`** — Update existing tests: assert no `text-xs` or `text-muted-foreground` classes in compact mode output, assert no `inline-flex` wrapper in either mode, verify `title` attribute still carries the ISO timestamp
- [x] Run `cd kanban-app/ui && npx vitest run` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #field