---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffea80
title: '[warning] Silent mutex poison in content cache stash/take'
---
avp-common/src/turn/state.rs:88-100,105-113\n\n`stash_content` and `take_content` silently swallow `PoisonError` via `.ok()` / `if let Ok`. If the mutex is poisoned (a thread panicked while holding it), content is silently dropped and no diff will ever be produced — with zero diagnostic output.\n\nAdd `tracing::warn!` on the error path so poisoned mutex is at least logged.\n\n**Verify**: grep for `content_cache.lock()` calls and confirm all have error logging. #review-finding