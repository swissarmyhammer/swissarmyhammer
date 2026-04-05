---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe480
title: 'NIT: VirtualTagCommand fields could use &str returns instead of String'
---
**File:** swissarmyhammer-kanban/src/virtual_tags.rs (VirtualTagStrategy trait)\n\n**What:** The `commands()` method on `VirtualTagStrategy` returns `Vec<VirtualTagCommand>` where `VirtualTagCommand` owns all its strings. Since the strategies are static singletons with compile-time-known values, every call to `commands()` allocates new Strings for id, name, etc. Similarly, `slug()`, `color()`, and `description()` correctly return `&str` but `commands()` does not follow the same pattern.\n\n**Suggestion:** This is minor since `commands()` is only called during metadata serialization (not on every evaluate). No change needed now, but if `commands()` ends up on a hot path, consider returning `&[VirtualTagCommand]` with a `LazyLock` static or `Cow<'static, str>` fields.\n\n**Verification:** No test needed -- style observation." #review-finding