---
position_column: done
position_ordinal: z00
title: Subscriber thread not joined on drop — silent detach
---
**bus.rs:149-152**\n\n`Subscriber` stores `_thread: Option<JoinHandle<()>>` but has no `Drop` impl. When dropped, the JoinHandle is dropped without joining, detaching the thread. The thread continues until `rcvtimeo` fires and `tx.send()` fails — up to 100ms of leaked work.\n\nSame pattern as `Publisher` (bus.rs:62-63) but Publisher's thread exits immediately when the channel Sender is dropped (mpsc::recv returns Err instantly), so it's less of an issue.\n\n**Suggestion**: Add `Drop` impl for `Subscriber` that joins the thread, similar to `ProxyHandle::drop`. For Publisher, the current behavior is acceptable since the thread exits promptly.\n\n**Verify**: `cargo test -p swissarmyhammer-leader-election` passes; integration tests still clean up within 2s.