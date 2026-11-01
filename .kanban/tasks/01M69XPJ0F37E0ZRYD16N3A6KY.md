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
- actor: claude-code
  id: 01m6agwzysgm6w0p4vczdys7y5
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD: 0 findings (7 pairs attempted, 0 failed). Hand check: git grep -i hashline/edit-match/edit_match outside .kanban/.skills finds nothing. Cargo.lock names neither crate. Both crate directories are gone.
    - next: none. Card moved to done.
  timestamp: 2026-11-01T03:01:57.849275+00:00
- actor: claude-code
  id: 01m6agx9x15zx6ab35hz4977ag
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — both crates deleted, 19 files
    - test: green — cargo nextest, 13481 passed
    - commit: c137a166c
    - review: clean — 0 findings, card is done
  timestamp: 2026-11-01T03:02:08.033668+00:00
depends_on:
- 01M69XPFH14ZS35Y9KNM3W7NA4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffd180
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