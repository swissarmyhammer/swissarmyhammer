---
position_column: done
position_ordinal: z00
title: HebContext.publish match arms are identical — extract helper
---
**context.rs:62-69**\n\nThe match on `ElectionOutcome::Leader` / `ElectionOutcome::Follower` calls the same `publish()` method on each guard. This could be simplified with a helper method on `ElectionOutcome` or by extracting the publisher reference.\n\n**Suggestion**: Add a `publish` method to `ElectionOutcome<M>` that delegates to whichever guard is active, or store a shared `Publisher<M>` reference.\n\n**Verify**: `cargo test -p heb` passes.