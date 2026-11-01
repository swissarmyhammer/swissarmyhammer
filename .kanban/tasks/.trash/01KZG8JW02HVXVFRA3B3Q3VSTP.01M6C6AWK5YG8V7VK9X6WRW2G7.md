---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9d80
title: 'mirdan: give Profile a builder, and fix the wrong doc comment on install_profile_mcp'
---
The review engine reports two findings on `crates/mirdan/src/install/profile.rs`, quoted word for word:

> `crates/mirdan/src/install/profile.rs:112` — Struct with 3+ optional fields does not use builder pattern. The Profile struct has 4 Option<T> fields, meeting the threshold for requiring builder pattern for ergonomic construction. Implement a builder pattern for Profile construction, e.g., `ProfileBuilder::new().with_mcp_server(...).with_skills(...).build()`, to make optional field construction more ergonomic and self-documenting.

> `crates/mirdan/src/install/profile.rs:886` — Doc comment for `install_profile_mcp` function contains unrelated text about edit-redirect fragment installation. Lines 886-890 describe applying edit-redirect to agent settings files, which is not what this function does. The function registers only the MCP server. Remove lines 886-890 from the doc comment. Keep only lines 891-896 which correctly describe registering the MCP server. Or move lines 886-890 to document a different function if one exists.

## Why this is a separate card

Both are pre-existing. Neither the `Profile` struct nor the `install_profile_mcp` doc comment appears in the ^qh5fnpd diff, which touched only an import line, one call line, and a deleted helper. They surfaced during ^qh5fnpd verification only because that used whole-file review (`review file`).

^qh5fnpd was scoped to one finding and was told not to refactor anything that finding did not name.

## A caution on the second finding

The doc comment finding cites line numbers. Line numbers move. Find the text by reading `install_profile_mcp` and locating the sentences about applying an edit-redirect fragment to agent settings files. Check whether a real function does that work — if one exists, the text belongs on it, not deleted.

## Subtasks

- [ ] Add a builder for `Profile` and move every construction site onto it.
- [ ] Correct the `install_profile_mcp` doc comment so it describes only what the function does.
- [ ] Run `cargo nextest run -p mirdan`, `cargo fmt`, and `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Verify with `{"op": "review file", "path": "crates/mirdan/src/install/profile.rs"}` that both rows are gone. #mirdan