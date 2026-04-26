---
position_column: done
position_ordinal: ffffffff9180
title: '[warning] `WorkspaceMode` and `DbRef` are missing `Debug` for `Follower.db` and `DbRef` variants'
---
**File:** `swissarmyhammer-code-context/src/workspace.rs`\n**Severity:** warning\n\n`WorkspaceMode` has a manual `Debug` impl that produces just `\"Leader\"` or `\"Follower\"` — the workspace root and mode details are lost. `DbRef` has no `Debug` impl at all. Per the Rust review guidelines, all public types must implement `Debug`.\n\nFor `DbRef`, a minimal impl like `f.debug_struct(\"DbRef\").finish_non_exhaustive()` would suffice since `Connection` is not `Debug` either. For `WorkspaceMode`, the existing impl is acceptable but should include the mode label at minimum (which it does), though adding the workspace root would aid diagnostics.\n\n**Fix:** Add `impl fmt::Debug for DbRef<'_>` with a field-less struct form." #review-finding