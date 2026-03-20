---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffa480
title: 'NIT: AppConfig public fields are a permanent API commitment'
---
File: kanban-app/src/state.rs lines 197-212 — `AppConfig`, `RecentBoard`, and `AppState` expose all fields as `pub`. Per Rust review guidelines, public fields are a permanent semver commitment. Changing a field name or type later is a breaking change. These structs are only used within the `kanban-app` binary crate so the impact is limited, but the pattern is still worth noting.\n\nSuggestion: for structs whose internals should not be relied upon externally, use `pub(crate)` or private fields with accessor methods. At minimum, add a `#[non_exhaustive]` attribute to prevent external construction of these types.\n\nVerification step: confirm none of these types are re-exported from a library crate and assess whether `pub(crate)` is more appropriate." #review-finding