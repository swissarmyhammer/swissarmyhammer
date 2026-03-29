---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffdf80
title: 'WARNING: No unit tests for the claim stack (pushClaim/updateClaim/popClaim)'
---
**File:** `kanban-app/ui/src/lib/entity-focus-context.tsx:102-166`\n\n**What:** The LIFO claim stack is the core new abstraction in this PR. It has non-trivial logic: highest-ID-wins resolution, scope registry side effects on push/pop, fallback on pop, moniker change detection in update. The only test coverage is the integration test in `inspector-focus-bridge.test.tsx` which tests mount/unmount focus claim/restore -- it does not test:\n- Two competing claims (LIFO ordering)\n- Popping a non-active claim (should not change focus)\n- updateClaim on a non-active claim (should not change focus)\n- updateClaim with a changed moniker (should unregister old moniker)\n- Monotonically increasing IDs after many push/pop cycles\n\n**Why this matters:** The claim stack replaces three bespoke focus bridge components. If its edge cases are wrong, focus will be stolen or lost in subtle ways that are hard to reproduce manually.\n\n**Suggestion:** Add a dedicated `entity-focus-context.test.tsx` that renders `EntityFocusProvider` with multiple `FocusClaim` components and asserts `focusedMoniker` after mount/unmount sequences." #review-finding