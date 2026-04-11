---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffb780
title: 'Bug: Virtual tag pills missing # prefix in display'
---
## What

Virtual tag pills (READY, BLOCKED, BLOCKING) render without the `#` prefix.
Regular tag pills show `#bug`, `#feature` etc. via `MentionPill` which renders
`{prefix}{briefSlug(slug)}`. But `VirtualTagDisplay` renders just `{slug}`
(e.g. `READY` instead of `#READY`).

### Root cause

In `kanban-app/ui/src/components/fields/displays/virtual-tag-display.tsx`,
the render at line 48 is:
```tsx
{slug}
```

It should be:
```tsx
#{slug}
```

This is a one-line fix.

### File to modify

- `kanban-app/ui/src/components/fields/displays/virtual-tag-display.tsx` —
  prepend `#` before `{slug}` in the `<span>` content

## Acceptance Criteria

- [ ] Virtual tag pills render as `#READY`, `#BLOCKED`, `#BLOCKING` (with `#` prefix)
- [ ] Visual consistency with regular `MentionPill` tags that show `#` prefix

## Tests

- [ ] `kanban-app/ui/src/components/fields/displays/virtual-tag-display.test.tsx` —
      update assertions to expect `#READY`, `#BLOCKED`, `#BLOCKING` text content
      instead of just `READY`, `BLOCKED`, `BLOCKING`
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/fields/displays/virtual-tag-display.test.tsx`

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.