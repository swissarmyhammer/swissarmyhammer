---
assignees:
- claude-code
depends_on:
- 01M69XPFH14ZS35Y9KNM3W7NA4
position_column: todo
position_ordinal: ffff8f80
project: remove-files-tool
title: Decide the fate of the diagnostics leader watcher after the files tool goes
---
`crates/swissarmyhammer-diagnostics/src/watcher.rs` and `tests/leader_watcher.rs` say `files edit` only covers edits made through the tool, so a watcher catches direct writes. After the files tool is gone, every edit is a direct write.

Work:
- Read `watcher.rs` and `leader_watcher.rs` and the callers. Decide from the code: does the watcher still do a needed job (re-run LSP diagnostics when files change on disk)? Expect yes, because host Edit and Write tools do not tell sah.
- If it is still needed: keep the behavior. Fix doc comments and test names that mention `files edit` or the closed write surface. Add or keep a real-path test that a direct write on disk triggers a diagnostics refresh.
- If part of it only served the files tool (for example a follower that forwards `files edit` writes): remove that part with a test that proves the rest still works.
- Record the decision and the reason on this card as a comment.

Depends on the card that removes the files MCP tool.
Done when: no comment or test in the crate names the removed tool, and `cargo nextest run -p swissarmyhammer-diagnostics` is green. #remove-files