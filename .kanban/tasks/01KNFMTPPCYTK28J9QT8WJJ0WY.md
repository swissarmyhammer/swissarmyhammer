---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffc580
title: 'NIT: RustEngineContainer entityStore memo is a no-op identity assignment'
---
**File**: kanban-app/ui/src/components/rust-engine-container.tsx\n\n**What**: `const entityStore = useMemo(() => entitiesByType, [entitiesByType]);` creates a memo that returns its input unchanged. `useMemo(() => x, [x])` always returns `x` and adds overhead without providing referential stability.\n\n**Suggestion**: Remove the `useMemo` and pass `entitiesByType` directly to `EntityStoreProvider`.\n\n**Subtasks**:\n- [ ] Remove the redundant useMemo\n- [ ] Verify fix by running tests #review-finding