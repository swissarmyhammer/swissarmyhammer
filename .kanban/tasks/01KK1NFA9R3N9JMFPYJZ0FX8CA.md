---
position_column: done
position_ordinal: ffff9980
title: 'W1: RwLock::read/write().unwrap() in UIState can panic on poisoned lock'
---
In `swissarmyhammer-commands/src/ui_state.rs`, every method on `UIState` calls `.unwrap()` on the `RwLock` read/write result (e.g. line 74: `self.inner.write().unwrap()`). If any thread panics while holding the lock, the lock becomes poisoned and all subsequent calls will panic.\n\nSince `UIState` is shared across async tasks via `Arc`, a poisoned lock will bring down the entire application. Consider using `.expect(\"UIState lock poisoned\")` at minimum for debuggability, or better yet handle the poisoned case gracefully (e.g. `unwrap_or_else(|e| e.into_inner())` to recover the data). #review-finding #warning