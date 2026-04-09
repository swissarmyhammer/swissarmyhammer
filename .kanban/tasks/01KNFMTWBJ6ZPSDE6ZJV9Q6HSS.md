---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffc480
title: 'NIT: InspectorsContainer entityStore memo is a no-op identity assignment'
---
**File**: kanban-app/ui/src/components/inspectors-container.tsx\n\n**What**: Same issue as RustEngineContainer: `const entityStore = useMemo(() => entitiesByType, [entitiesByType]);` is an identity memo that provides no benefit.\n\n**Suggestion**: Remove the useMemo and use `entitiesByType` directly.\n\n**Subtasks**:\n- [ ] Remove the redundant useMemo\n- [ ] Verify fix by running tests #review-finding