---
assignees:
- claude-code
position_column: todo
position_ordinal: a580
project: local-review
title: 'Review: delete dead .hashes incremental tracking'
---
`.validators/.hashes/` skip-hash only fires for `Scope::Working`, but `/finish` commits each checkpoint and reviews `sha HEAD~1..HEAD` — so the working scope is never used and the hash store is never written (the "empty after 54 sessions" report; structural, not a bug). A prior task (`01KVJV852P5SXCYD7NB99C5G4J`) already burned a cycle chasing a phantom path-normalization bug inside this dead module.

## Remove
- `crates/swissarmyhammer-validators/src/review/tracking.rs` — delete the module (~1000 lines): `TrackingEntry`, `record_baseline_if_working`, `subtract_unchanged`, `ensure_gitignore`, `rel_key`/`clean_relative`, all of it.
- `crates/swissarmyhammer-validators/src/review/scope.rs`: drop the `use_tracking`/`rules_hash` params and the subtract-unchanged filter from `resolve_working` (`:540-597`); `scope_review` loses the `use_tracking` arg.
- `crates/swissarmyhammer-validators/src/review/synthesize.rs::run_review` + `drive.rs::run_review_over_agent`: drop the `use_tracking` param and the baseline-record tail.
- `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`: drop `ReviewRequest.force` and the `use_tracking = !force` plumbing.
- Review tool/skill: remove the `force`/`--force`/`all` escape hatch surface and its docs.
- Delete the tracking + incremental-tracking tests in scope.rs/drive.rs/tracking.rs.

## Verify
`cargo test -p swissarmyhammer-validators` green; no `.validators/.hashes/` or `.gitignore` writing remains anywhere.