---
position_column: done
position_ordinal: a0
title: Fix doctest failure in swissarmyhammer-kanban/src/lib.rs (line 16)
---
The doc-test in swissarmyhammer-kanban/src/lib.rs (line 16) fails to compile because `ExecutionResult` does not implement the `Try` trait, so the `?` operator cannot be applied to it. The doctest uses `InitBoard::new(...).execute(&ctx).await?` and `AddTask::new(...).execute(&ctx).await?` which both fail with error E0277. Either the doctest needs to be updated to not use `?`, or `ExecutionResult` needs to implement `Try`. #test-failure