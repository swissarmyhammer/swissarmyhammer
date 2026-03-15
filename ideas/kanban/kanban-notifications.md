# Kanban Lifecycle Notifications

## Overview

When the kanban MCP tool modifies cards, we should send system notifications so the user has real-time awareness of progress — especially useful when an agent is running autonomously.

## Events to Notify

### Card Created
- **Trigger:** `op: "add task"`
- **Title:** `Kanban: Created`
- **Message:** card title from `tool_input.title`
- **Example:** "Kanban: Created — Fix login crash"

### Card Started
- **Trigger:** `op: "move task"` where `column` matches a "doing/in-progress" pattern
- **Title:** `Kanban: Started`
- **Message:** card title or id
- **Example:** "Kanban: Started — Refactor auth module"

### Card Completed
- **Trigger:** `op: "complete task"`
- **Title:** `Kanban: Done ✓`
- **Message:** card title or id
- **Example:** "Kanban: Done ✓ — Add unit tests for parser"

## Possible Delivery Mechanisms

### A) Claude Code PostToolUse Hook (no Rust changes)
- Hook fires after every `mcp__sah__kanban` tool call
- Shell script reads stdin JSON, checks `tool_input.op`, sends notification
- macOS: `osascript -e 'display notification "..." with title "..."'`
- Linux: `notify-send "..." "..."`
- Installed/removed by `sah init` / `sah deinit`

### B) Inside the Kanban Tool (Rust)
- After executing a lifecycle operation, call `std::process::Command` to run `osascript` / `notify-send`
- Runs in-process, no hook config needed
- Cross-platform fallback: log at info level if no notification binary available

### C) PlanSender Channel (existing dead infrastructure)
- `ToolContext.plan_sender` exists but is always `None` at runtime
- Wiring it up would let an async task consume notifications and dispatch them
- Most flexible but requires the most plumbing

## Recommendation

Start with **A (PostToolUse hook)** — it's entirely configuration/script, no Rust changes, and can be wired into `sah init` alongside the existing Bash deny rule. Switch to **B** if cross-platform or reliability is a concern.

## Open Questions

- Should notifications include a count of remaining cards? (e.g. "Done ✓ — 3 cards left")
- Do we want sound? (`osascript` supports `beep`)
- Should `move task` to non-doing columns also notify? (e.g. "Card blocked", "Card backlogged")
- Should there be a way to opt-out (flag file to disable)?
