---
severity: error
tags:
- acp
- streaming
---

# ACP Streaming Architecture

Streaming must follow proven claude-agent patterns for reliability.

## Requirements

- Use tokio broadcast channel for session/update notifications
- Separate concurrent channels for requests and notifications
- Proper shutdown coordination via CancellationToken
- Handle backpressure in broadcast channel
- Send notifications in correct order
- Never block notification sending
- Clean up resources on session close
- Handle client disconnection gracefully

## Verification

Test with:
- Multiple concurrent sessions
- High-frequency token generation
- Client disconnect during streaming
- Graceful shutdown while streaming