---
assignees:
- claude-code
position_column: done
position_ordinal: ffffff8b80
title: 'swissarmyhammer-leader-election/election.rs: FollowerGuard::subscribe() creates a new ZMQ context per call'
---
swissarmyhammer-leader-election/src/election.rs:450-453

```rust
pub fn subscribe(&self, topics: &[&[u8]]) -> Result<Subscriber<M>> {
    let ctx = zmq::Context::new();
    Subscriber::connected(&ctx, &self.bus_addresses.backend, topics)
}
```

Same issue exists on `LeaderGuard::subscribe()` (line 357). A new `zmq::Context` is created on every `subscribe()` call. ZMQ contexts are heavyweight OS-level objects (thread pools, file descriptors). The leader already has a context via `ProxyHandle::zmq_context()` but the follower has no stored context. Creating one-per-subscriber leaks OS resources when many subscribers are created.

Suggestion: store a `zmq::Context` on `FollowerGuard` (created at election time when the publisher is created) and reuse it for `subscribe()` calls. For `LeaderGuard`, use `_proxy.zmq_context()` instead of allocating a new one. #review-finding