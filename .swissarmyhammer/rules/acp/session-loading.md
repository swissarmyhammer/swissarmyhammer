---
severity: error
tags:
- acp
- sessions
---

# ACP Session Loading

Session loading must match claude-agent behavior for compatibility.

## Requirements

- Advertise `load_session: true` in initialize capabilities
- Load session from SessionManager storage
- Stream ALL historical messages via session/update notifications
- Maintain chronological order of conversation history
- Include all message types (user, assistant, tool)
- Return LoadSessionResponse only after history replay completes
- Handle missing sessions with appropriate error
- Preserve tool call context and results

## Verification

Test session loading with:
- Empty session (no messages)
- Session with multiple messages
- Session with tool calls and results
- Non-existent session (error case)