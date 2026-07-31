---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kywq3jyr19badv4q0c02a18v
  text: |-
    Second confirmation from a different review round. ^qsr5rdt's review re-found this same cluster in `install.rs` — 14 findings, all pre-existing, none on code that commit authored. Two rounds independently landing on the same file is a signal the cleanup is worth doing rather than deferring again.

    Additions to the item list from that round:
    - `rooted` takes `PathBuf` where `&Path` would do.
    - Deinit count inconsistency — real lines ~2045 and ~2067 (the engine cited 1820/1840).
    - PackageType dispatch triplication is at real lines ~90, ~167, ~362, which sharpens the existing "duplicated across call sites" item.

    Line-number warning, reconfirmed and worse than this card first stated. The engine cited `install.rs:1441` for metadata rendering, but line 1441 is `fn resolve_agent_file`. Offsets ran 200+ lines in places. Grep for the symbol; treat every cited number as wrong until checked.

    Not in scope here, tracked separately: `resolved_skill_names` / `resolved_agent_names` convergence (real lines ~2151-2166) — see ^kt2p9xr.
  timestamp: 2026-07-31T18:31:10.552414+00:00
- actor: claude-code
  id: 01kywq5ea9ytqkxaxaapf33aj7
  text: 'Correction to my previous comment: the resolver-convergence card is **^m4eb4bf**, not ^kt2p9xr. I cited the id before creating the card and guessed. ^kt2p9xr does not exist.'
  timestamp: 2026-07-31T18:32:11.337167+00:00
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