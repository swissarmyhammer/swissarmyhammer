---
position_column: done
position_ordinal: ffe180
title: 'W2: Missing Debug impl on UIState and CommandContext (public types)'
---
Per Rust review guidelines, public types must implement all applicable traits. `UIState` (ui_state.rs) and `CommandContext` (context.rs) in swissarmyhammer-commands are public but have no `Debug` impl. `UIState` wraps `RwLock<UIStateInner>` where `UIStateInner` is private and also not Debug. `CommandContext` stores `Arc<dyn Any + Send + Sync>` in extensions which complicates auto-derive but a manual impl is feasible.\n\nFile: swissarmyhammer-commands/src/ui_state.rs, swissarmyhammer-commands/src/context.rs #review-finding #warning