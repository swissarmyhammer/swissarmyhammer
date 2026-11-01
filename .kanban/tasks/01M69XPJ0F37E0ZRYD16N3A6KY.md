---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6agm29pbwgt2fdfbm4920zc
  text: |-
    ### implement — changed
    - evidence: removed both crates, 2 workspace members, 2 workspace.dependencies lines, ideas/file-edit-tools.md (design doc for the deleted tool). Cargo.lock updated by cargo. git grep for hashline/edit-match/edit_match outside .kanban/.skills finds nothing. cargo check --workspace --tests, clippy -D warnings, fmt --check, build pass. nextest workspace: 13481 passed, 0 failed. No Rust file changed, so no review file run.
    - next: /review
  timestamp: 2026-11-01T02:57:05.334746+00:00
depends_on:
- 01M69XPFH14ZS35Y9KNM3W7NA4
position_column: doing
position_ordinal: '8480'
project: remove-files-tool
title: Delete the swissarmyhammer-hashline and swissarmyhammer-edit-match crates
---
Only the MCP files tool used these two crates (read gave hash anchors, edit matched text). Delete them with it.

Work:
- Delete `crates/swissarmyhammer-hashline/` and `crates/swissarmyhammer-edit-match/` (sources, tests, property tests).
- Remove both from the workspace `Cargo.toml` members and `[workspace.dependencies]`. Run `cargo update -w`.
- Search the whole repo (Rust, Cargo.toml, justfile, CI files, docs, builtin, standards) for `hashline`, `edit-match`, `edit_match` and fix every hit outside `.kanban/` history.
- Check docs that explain hash anchors or the edit match ladder (`doc/src`) and remove or fix them.

Depends on the card that removes the files MCP tool.
Done when: `cargo build --workspace` and `cargo nextest run --workspace` pass and a search for both names finds nothing. #remove-files