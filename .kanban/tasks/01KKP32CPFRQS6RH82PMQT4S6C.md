---
position_column: done
position_ordinal: z00
title: 'Update AVP spec: triggers.md and schema.md'
---
## What
Update the AVP specification at `/Users/wballard/github/agentvalidatorprotocol/spec` to document all new hook event types.

**`docs/core-concepts/triggers.md`:**

1. Update lifecycle diagram to show new events:
   - Add Elicitation/ElicitationResult in MCP interaction flow
   - Add WorktreeCreate/WorktreeRemove
   - Add PostCompact after PreCompact
   - Add InstructionsLoaded, ConfigChange in session flow
   - Add TeammateIdle, TaskCompleted in agent team flow

2. Add new trigger sections under appropriate categories:

   **MCP Interaction Triggers** (new category):
   - `Elicitation` — MCP server requests user input. Matcher: MCP server name. Decision: allow/deny.
   - `ElicitationResult` — User responds to elicitation. Matcher: MCP server name. Decision: allow/block.

   **Session Lifecycle Triggers** (add to existing):
   - `InstructionsLoaded` — CLAUDE.md/rules loaded. Observe-only (command hooks only).
   - `ConfigChange` — Config files change. Matcher: source type (user_settings, project_settings, local_settings, policy_settings, skills). Decision: allow/block.
   - `PostCompact` — After context compaction. Observe-only (command hooks only).
   - `WorktreeCreate` — Worktree created. Decision: allow/deny. (command hooks only)
   - `WorktreeRemove` — Worktree removed. Observe-only (command hooks only).

   **Agent Team Triggers** (new category):
   - `TeammateIdle` — Agent teammate goes idle. Decision: allow/block. No matcher support.
   - `TaskCompleted` — Task marked complete. Decision: allow/block. No matcher support.

3. Update Notification trigger matchers table to note `elicitation_dialog` is legacy — prefer the first-class `Elicitation` trigger.

4. Update triggerMatcher table with new matchers.

**`docs/reference/schema.md`:**

1. Add all new trigger values to the Trigger Values section under appropriate categories
2. Add new triggerMatcher values table entries

**Files:**
- `/Users/wballard/github/agentvalidatorprotocol/spec/docs/core-concepts/triggers.md`
- `/Users/wballard/github/agentvalidatorprotocol/spec/docs/reference/schema.md`

## Acceptance Criteria
- [ ] All 9 new triggers documented in triggers.md with use cases, matchers, decision control
- [ ] Each trigger notes whether it's command-hooks-only or supports all handler types
- [ ] Lifecycle diagram updated to show new event flow
- [ ] schema.md Trigger Values section lists all new triggers under appropriate categories
- [ ] triggerMatcher tables updated with new trigger-specific matchers
- [ ] Notification section updated to note `elicitation_dialog` is legacy
- [ ] No broken markdown links

## Tests
- [ ] Manual review: all 9 new triggers documented with consistent format matching existing triggers
- [ ] Verify trigger names in spec match the PascalCase strings in `HookType` enum exactly
- [ ] Verify matcher values in spec match the `matcher_value()` semantics from Card 5