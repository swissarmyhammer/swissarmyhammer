---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffee80
title: 'NIT: PerspectiveProvider auto-create effect missing dispatch in deps array'
---
**File**: kanban-app/ui/src/lib/perspective-context.tsx (useEffect for auto-create)\n\n**What**: The useEffect that auto-creates a \"Default\" perspective when none exist has `[loaded, perspectives, viewKind]` as its deps array but calls `dispatch('perspective.save', ...)`. The `dispatch` function is missing from the dependency array. ESLint exhaustive-deps would flag this.\n\n**Suggestion**: Add `dispatch` to the dependency array.\n\n**Subtasks**:\n- [ ] Add dispatch to the useEffect dependency array\n- [ ] Verify fix by running tests #review-finding