---
position_column: done
position_ordinal: fffffe80
title: Add info logging to Rust command dispatch
---
Log every command invocation in the Rust command system with a readable format so we can verify all activity flows through dispatch and nothing is hard-wired in the UI.

- [ ] Find command dispatch entry point (likely `execute_command` or `dispatch_command`)
- [ ] Add `info!` log with command name, target, and args summary
- [ ] Verify logging works via `cargo test`