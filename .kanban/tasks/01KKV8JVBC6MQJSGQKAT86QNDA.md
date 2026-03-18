---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffa80
title: 'nit: getEntityIcon is exported but unused'
---
**entity-icon.tsx:41-43**\n\n`getEntityIcon()` is exported but not imported anywhere. It duplicates the logic in `EntityIcon` and was likely intended for cases where callers need the raw icon component. Consider removing it until there's an actual consumer — dead exports add confusion.\n\n- [ ] Remove `getEntityIcon` or add a consumer\n- [ ] Verify `npx tsc --noEmit` passes