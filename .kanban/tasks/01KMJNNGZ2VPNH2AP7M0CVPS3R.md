---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffbe80
title: 'Fix failing test: AvatarDisplay > renders Avatar components for array of actor IDs'
---
Test file: `src/components/fields/displays/avatar-display.test.tsx`\n\nError: `useSchema must be used within a SchemaProvider`\n\nThe `Avatar` component (at `src/components/avatar.tsx:41`) calls `useSchema()` but the test renders `AvatarDisplay` without wrapping it in a `SchemaProvider`. The test needs a `SchemaProvider` in its render wrapper, or the `Avatar` component needs to handle the missing context gracefully. #test-failure