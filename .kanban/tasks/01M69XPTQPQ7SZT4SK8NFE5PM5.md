---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6ahjysfvjb84retm5z3cp3j
  text: |-
    Decision: KEEP the diagnostics leader watcher. Reason, by evidence: (1) start_diagnostics_watcher_with_notifier is called in swissarmyhammer-tools/src/mcp/server/code_context.rs, so the watcher is live. (2) The watcher does not use the files tool. It watches the disk and runs didChange plus pull for each changed file. Host Edit and Write tools, formatters and git checkout still write to disk without telling sah, so the job is still needed. (3) The real-path test leader_watcher::watcher_redreport_on_direct_disk_write writes a type error to disk and checks the re-report. It passes with real rust-analyzer. No follower-only forwarding code for the files tool exists in the crate, so nothing is removed.
    Changes: comments only. Removed `files edit`, closed write surface and inline-on-edit fold-in wording from watcher.rs, diagnose.rs, config.rs, language.rs, lib.rs, tests/leader_watcher.rs, and the same fold-in wording in swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs. The remaining word fold-in in diagnose.rs means folding broken dependents into a report, not the files tool.
    Checks: cargo check --workspace --tests ok. cargo nextest run -p swissarmyhammer-diagnostics: 96 passed, 0 failed. clippy -D warnings ok for diagnostics and tools. fmt ok.
    Review: `review file` on the 7 changed files gave findings only on code lines I did not change (watcher.rs logging/AsRef, diagnose.rs logging and test helpers, leader_watcher.rs test helpers, tools diagnostics mod.rs error handling). I changed comment lines only, so none of the findings sit on lines I changed. I did not fix them. They need a separate card if wanted.
  timestamp: 2026-11-01T03:13:57.551354+00:00
- actor: claude-code
  id: 01m6ahk0zpn3hawfdde8zgedcz
  text: |-
    ### implement — changed
    - evidence: 7 files (comments only): crates/swissarmyhammer-diagnostics/src/{watcher,diagnose,config,language,lib}.rs, crates/swissarmyhammer-diagnostics/tests/leader_watcher.rs, crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs. nextest 96/96 pass, clippy and fmt clean.
    - next: /review. Review lists 40 findings on code lines I did not change.
  timestamp: 2026-11-01T03:13:59.798193+00:00
- actor: claude-code
  id: 01m6ahw797gxrhjzwysn7yayta
  text: |-
    ### review - clean
    - evidence: review sha HEAD~1..HEAD, 0 findings (7 files reviewed). Hand-check git grep for "files edit" and "closed write surface" found hits only in ideas/diagnostic.md. None are in the diagnostics crate or the tools diagnostics module.
    - next: none
  timestamp: 2026-11-01T03:19:01.159804+00:00
- actor: claude-code
  id: 01m6ahwh3bqvqr8hg4np4c8fc1
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — watcher kept, 7 files of comments fixed
    - test: green — cargo nextest, 13481 passed
    - commit: ebda61c19
    - review: clean — 0 findings, card is done
  timestamp: 2026-11-01T03:19:11.211279+00:00
depends_on:
- 01M69XPFH14ZS35Y9KNM3W7NA4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffd280
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