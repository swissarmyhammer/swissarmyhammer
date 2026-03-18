---
position_column: done
position_ordinal: ffffffffd780
title: LeaderGuard._proxy is Option but always Some
---
**election.rs:319**\n\n`_proxy: Option<ProxyHandle>` is always constructed as `Some(proxy)` in both `try_acquire_lock` and `try_promote`. The `Option` wrapper adds no value and obscures the invariant that leaders always own a proxy.\n\n**Suggestion**: Change to `_proxy: ProxyHandle` (non-optional). If a noop path is ever needed, add `ProxyHandle::noop()` instead.\n\n**Verify**: `cargo test -p swissarmyhammer-leader-election` passes.