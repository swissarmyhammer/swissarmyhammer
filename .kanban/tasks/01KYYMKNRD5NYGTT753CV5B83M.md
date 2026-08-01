---
assignees:
- claude-code
position_column: todo
position_ordinal: d880
title: swissarmyhammer_agent::await_collector drains on a fixed 100ms sleep and drops all content on timeout
---
# Symptom

`swissarmyhammer_agent::await_collector` (`crates/swissarmyhammer-agent/src/lib.rs`) is the same
bug class as ^8ep9cnf, on the OTHER collector — the private one behind
`swissarmyhammer_agent::execute_prompt` (the CLI/kanban AI-panel path), not
`claude_agent::collect_response_content`.

It drains like this:

```rust
tokio::time::sleep(Duration::from_millis(NOTIFICATION_COLLECTION_DELAY_MS)).await; // 100
cancel_token.cancel();
match tokio::time::timeout(Duration::from_millis(500), collector_handle).await {
    Ok(Ok(result)) => result,
    Ok(Err(e)) => { warn!(...); (String::new(), 0) }
    Err(_)      => { warn!(...); (String::new(), 0) }
}
```

Two defects:

1. The drain window is a flat 100 ms of wall clock. Anything the forwarding hops
   have not delivered by then is cut off, so the reply comes back truncated
   under load.
2. On the timeout and task-error paths it returns `String::new()` — the whole
   reply is discarded and reported as a warning, so the caller sees an EMPTY
   response rather than an error.

# Changes

^8ep9cnf added the in-band end-of-turn marker
(`agent_client_protocol_extras::turn_complete_notification` /
`is_turn_complete`), which both the claude and llama agents now emit as the
last act of every turn. This collector can drain on it:

- End the collector loop when the marker for the turn's session arrives (or the
  channel closes), exactly as `claude_agent::spawn_notification_collector` does.
- Keep a generous timeout as a hang guard only, and report a hit as an error —
  never an empty or truncated string.

Do NOT fix this by lengthening the 100 ms sleep.

# Acceptance

- A test that delivers a chunk later than the old 100 ms window still collects
  it (RED against the current code).
- No path returns `String::new()` for a drain that did not reach the end of the
  stream.
- `cargo nextest run -E 'rdeps(swissarmyhammer-agent)'` passes; `cargo fmt --all`;
  `cargo clippy --workspace --all-targets -- -D warnings` clean. #review #test-failure