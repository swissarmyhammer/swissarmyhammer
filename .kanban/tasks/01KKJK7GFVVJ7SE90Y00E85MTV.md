---
position_column: done
position_ordinal: fffffff780
title: lib.rs crate docs still show old try_become_leader() API in example
---
swissarmyhammer-leader-election/src/lib.rs:17-38\n\nThe crate-level doc comment still shows the old `try_become_leader()` / `Err(ElectionError::LockHeld)` pattern as the primary example. The code comment on `elect()` says it is the preferred entry point for new code, but the crate docs were not updated. A new user of the crate will see the legacy API first.\n\nSuggestion: Update the crate-level example to use `elect()` and `ElectionOutcome::Leader`/`Follower` matching. The old example can remain as a secondary \"legacy\" example or be removed.",
<parameter name="tags">["review-finding"] #review-finding