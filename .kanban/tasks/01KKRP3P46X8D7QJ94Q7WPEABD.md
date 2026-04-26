---
assignees:
- claude-code
position_column: done
position_ordinal: ffffff8e80
title: 'swissarmyhammer-leader-election/bus.rs: Subscriber ZMQ thread leaks after Subscriber is dropped'
---
swissarmyhammer-leader-election/src/bus.rs:153-157

The `Subscriber` stores `_thread: JoinHandle<()>` but does not implement `Drop`. The comment explains the thread exits within 100ms (rcvtimeo) after the `Receiver` is dropped because `tx.send()` fails. However, there is no guarantee the thread exits in bounded time: if ZMQ `recv_multipart` never returns `EAGAIN` (e.g. on a busy bus), the thread may block longer than 100ms. The context reference keeps the ZMQ context alive beyond the `Subscriber` lifetime. 

This is low risk for current usage but becomes a leak in high-throughput scenarios.

Suggestion: implement `Drop` for `Subscriber` that drops the receiver first, then either joins with a timeout or documents the bounded-exit guarantee explicitly in the type doc. #review-finding