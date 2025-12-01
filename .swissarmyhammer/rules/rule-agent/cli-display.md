---
severity: error
tags:
- agent
- cli
- ui
---

## Rule: CLI Must Show Use Case Agent Assignments

Running `sah agent` with no arguments must display the current agent assignment for each use case.

### Requirements
1. `sah agent` (no subcommand) must show table with use case assignments
2. Table format:
   ```
   Agent Use Case Assignments:
   ┌───────────┬──────────────────┐
   │ Use Case  │ Agent            │
   ├───────────┼──────────────────┤
   │ root      │ claude-code      │
   │ rules     │ qwen-coder-flash │
   │ workflows │ claude-code      │
   └───────────┴──────────────────┘
   ```
3. Must show actual resolved agent (after fallback logic)
4. Must indicate if use case is using fallback

### Verification
Check that:
- `sah agent` shows use case table
- All three use cases are displayed
- Resolved agent names are shown
- Table formatting is clear and readable
