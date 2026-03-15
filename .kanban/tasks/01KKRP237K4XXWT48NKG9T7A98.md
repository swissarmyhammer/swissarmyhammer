---
assignees:
- claude-code
position_column: done
position_ordinal: fffff680
title: 'heb/context.rs: HebContext::open() workspace_root parameter is unused after election setup'
---
heb/src/context.rs:28

`workspace_root` is passed to `LeaderElection::with_config` for election identity but is not stored on `HebContext`. As a result `HebContext` cannot answer "which workspace am I bound to?" and callers cannot validate they are publishing to the right bus.

Suggestion: store `workspace_root: PathBuf` on `HebContext` and expose `workspace_root(&self) -> &Path`. #review-finding