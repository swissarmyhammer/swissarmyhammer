---
assignees:
- claude-code
position_column: todo
position_ordinal: be80
title: 'mirdan install.rs cleanup: constants, dispatch dedup, nesting'
---
24 review findings in `crates/mirdan/src/install.rs`, split out of ^t7ebyn8.

All are pre-existing. That card's only hunk in this file was 97 lines of test at line 3523, yet every finding cites lines 143–2469 — production code the commit never touched. The file entered review scope because the card added a test to it.

## Items

- `DEFAULT_VERSION` and `FRONTMATTER_DELIMITER` should be named constants, not repeated literals.
- `PackageType` dispatch is duplicated across call sites — collapse to one table or match.
- `deploy_skill` and `deploy_agent` are near-duplicates — unify.
- Nesting depth extraction in `run_uninstall`, `remove_matching_store_entries`, `deploy_plugin`, `uninstall_tool`, `uninstall_agent_at`.
- `read_skill_frontmatter_name` should be reused rather than reimplemented at its second site.
- `uninstall_skill` needs a doc comment.

## Warning on line numbers

The review engine's cited lines track the pre-image and are offset — it cited `deploy.rs:323` for a test at 426, and `skill_loader.rs:30/37/87` for functions at 42/47/102. Grep for the symbol; do not trust the number.

## Acceptance

- No repeated version or delimiter literal.
- One dispatch site per `PackageType` decision.
- Max nesting 3 in each named function.
- `cargo nextest run -E 'rdeps(mirdan)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Fix each class across the whole file, not only at cited lines. On the previous card, patching cited lines made the review re-find the same class for eight rounds. #refactor