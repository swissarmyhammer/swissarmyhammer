---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffc180
title: 'swissarmyhammer-leader-election/bus.rs: Subscriber::recv_timeout() silently drops channel errors'
---
swissarmyhammer-leader-election/src/bus.rs:241-243

```rust
pub fn recv_timeout(&self, timeout: Duration) -> Option<Result<M>> {
    self.receiver.recv_timeout(timeout).ok()
}
```

`recv_timeout` returns `Err(RecvTimeoutError::Disconnected)` when the internal channel is closed (ZMQ thread panicked or context was destroyed), but `.ok()` converts that to `None` — the same value returned for a normal timeout. Callers cannot distinguish "no message yet" from "subscriber is dead". In the integration test `test_leader_and_follower_hear_each_other`, a silent dead subscriber would make the test flakily pass.

Suggestion: return `Option<Result<M>>` as it is today but map `RecvTimeoutError::Disconnected` to `Some(Err(...))` so callers can detect the broken state. #review-finding