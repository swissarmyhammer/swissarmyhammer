---
position_column: done
position_ordinal: ffffffa880
title: '[warning] `ElectionError::SocketError` variant is defined but never constructed'
---
**File:** `swissarmyhammer-leader-election/src/error.rs` line 22\n**Severity:** warning\n\n`ElectionError::SocketError(io::Error)` appears in the error enum and is tested in `test_all_error_variants_display`, but there is no site in `election.rs` that ever constructs it. Socket file removal uses `let _ = fs::remove_file(...)` (errors silently discarded). This is dead code in the public API of a library crate.\n\nEither:\n1. Remove the variant if socket errors are intentionally swallowed (the current design), or\n2. Actually propagate socket creation/removal errors through it.\n\nA dead public error variant is a semver hazard and misleads callers about what errors they should handle." #review-finding