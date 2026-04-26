---
position_column: done
position_ordinal: fffffffffb80
title: Publisher docstring claims Sync but type is !Sync
---
**bus.rs:57**\n\nDocstring says `Thread-safe (Send + Sync)` but `Publisher` contains `mpsc::Sender` which is `Send` but NOT `Sync`. The compile-time assertion at line 242 only checks `Send`, not `Sync`, so this is a documentation lie, not a soundness issue.\n\n**Suggestion**: Change docstring to `Thread-safe (Send)` and remove the `+ Sync` claim. Optionally add a `_assert_sync` check that would catch if Sync is ever accidentally promised.\n\n**Verify**: Docstring matches reality; no code change needed beyond comment.