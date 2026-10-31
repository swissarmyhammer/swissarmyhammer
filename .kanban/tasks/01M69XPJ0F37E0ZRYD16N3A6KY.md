---
assignees:
- claude-code
depends_on:
- 01M69XPFH14ZS35Y9KNM3W7NA4
position_column: todo
position_ordinal: ffff8d80
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