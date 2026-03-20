---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8180'
title: Add slugify utility for mention slugs
---
## What
Create a `slugify()` utility function that converts display field values with spaces into hyphenated, lowercase slugs suitable for mention text. Task titles like "Fix Login Bug" become "fix-login-bug".

**New file:** `kanban-app/ui/src/lib/slugify.ts`

```typescript
/** Convert a display name to a mention-safe slug: lowercase, spaces→hyphens, strip non-word chars. */
export function slugify(str: string): string {
  return str.toLowerCase().replace(/\s+/g, "-").replace(/[^\w-]/g, "");
}
```

## Acceptance Criteria
- [ ] `slugify("Fix Login Bug")` returns `"fix-login-bug"`
- [ ] `slugify("my-tag")` returns `"my-tag"` (idempotent for existing slugs)
- [ ] `slugify("Hello World! (v2)")` returns `"hello-world-v2"`

## Tests
- [ ] Unit test in `kanban-app/ui/src/lib/__tests__/slugify.test.ts`
- [ ] Test edge cases: empty string, already-slugified, special chars, multiple spaces