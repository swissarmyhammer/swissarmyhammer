---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffdb80
title: Fix field icon vertical alignment — icon sits 1px too high in inspector
---
## What

The field icon in the inspector's `FieldIconTooltip` appears slightly too high relative to the first line of text. The root cause is a fixed `mt-0.5` (2px) margin that doesn't match the text's line-height.

**Inspector** (`entity-inspector.tsx:484`): parent flex is `items-start`, icon span is `mt-0.5 shrink-0`, icon is 14px (`size={14}`). Adjacent display text is `text-sm` (14px font / 20px line-height per Tailwind). Correct centering: `(20 - 14) / 2 = 3px`. Current margin is 2px — **1px too high**.

**Card** (`entity-card.tsx:280`): icon span is `mt-0.5 shrink-0`, icon is 12px (`h-3 w-3`). Adjacent text is `text-xs` (12px / 16px line-height). Correct centering: `(16 - 12) / 2 = 2px`. Current margin is 2px — **already correct**.

**Approach**: Replace the `mt-0.5` hack in the inspector's `FieldIconTooltip` with a line-height-matched wrapper. Instead of a bare `<span className="mt-0.5 ...">`, use `<span className="h-5 inline-flex items-center shrink-0 text-muted-foreground">`. The `h-5` (20px) matches `text-sm`'s line-height, and `inline-flex items-center` centers the 14px icon within that 20px box. Combined with the parent's `items-start`, this keeps the icon aligned with the first line even for multi-line content — no magic pixel offsets.

The card's `CardFieldIcon` (`entity-card.tsx:280`) is already correct at `mt-0.5` but could be hardened the same way: `h-4 inline-flex items-center` (16px = `text-xs` line-height). Both changes in one pass.

### Files to modify

1. **`kanban-app/ui/src/components/entity-inspector.tsx`** — In `FieldIconTooltip` (line 484), change `<span className="mt-0.5 shrink-0 text-muted-foreground">` to `<span className="h-5 inline-flex items-center shrink-0 text-muted-foreground">`.

2. **`kanban-app/ui/src/components/entity-card.tsx`** — In `CardFieldIcon` (line 280), change `<span ... className="mt-0.5 shrink-0 text-muted-foreground/50">` to `<span ... className="h-4 inline-flex items-center shrink-0 text-muted-foreground/50">`.

## Acceptance Criteria

- [x] Inspector field icon is vertically centered with the first line of the adjacent display text (no visible misalignment)
- [x] Card field icon remains correctly aligned (no regression)
- [x] Multi-line field content keeps the icon aligned with the first line (not the middle of the block)
- [x] Tooltip hover target and tooltip text continue working

## Tests

- [x] **`kanban-app/ui/src/components/entity-inspector.test.tsx`** — Add a test that `FieldIconTooltip` renders an icon wrapper with `h-5` and `items-center` classes (structural assertion, ensures the alignment approach isn't regressed)
- [x] **`kanban-app/ui/src/components/entity-card.test.tsx`** — Add a test that `CardFieldIcon` renders with `h-4` and `items-center` classes
- [x] Run `cd kanban-app/ui && npx vitest run` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #field