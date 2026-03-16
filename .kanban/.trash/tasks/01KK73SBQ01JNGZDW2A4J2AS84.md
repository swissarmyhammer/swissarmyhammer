---
position_column: done
position_ordinal: q7
title: '[NIT] backoff_duration is public but arguably an implementation detail'
---
File: swissarmyhammer-lsp/src/daemon.rs, line 35; also re-exported from lib.rs\n\nbackoff_duration() is a pure utility function that computes exponential backoff. It is public and re-exported, but it is only used internally by LspDaemon::restart_with_backoff(). Exposing it commits to its signature as public API.\n\nIf there is no external consumer, consider making it pub(crate) to reduce API surface. If it is intentionally public for testing, the tests already live in the same module so pub(crate) suffices. #review-finding