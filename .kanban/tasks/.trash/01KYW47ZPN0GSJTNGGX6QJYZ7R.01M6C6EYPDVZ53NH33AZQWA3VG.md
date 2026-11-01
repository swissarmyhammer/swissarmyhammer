---
assignees:
- claude-code
position_column: todo
position_ordinal: c780
title: 'skill/mod.rs: dead convert_result, duplicated operation lists, missing Debug, deep nesting'
---
Four cleanups in `crates/swissarmyhammer-tools/src/mcp/tools/skill/mod.rs`, plus two in `agent/mod.rs`. Split out of ^6xjxebg — all pre-existing; that commit only added `cli_category` to each file.

## Items

1. **Dead `convert_result`** (`skill/mod.rs` ~line 99). Nothing calls it — `execute` dispatches to `list::`, `use_op::` and `search::`, each of which builds its own response. Delete it. Confirm no caller first.

2. **Operation lists duplicated between the match arms and the error text.** `execute` matches `"list skill"`, `"use skill" | "get skill" | "load skill" | "activate skill" | "invoke skill"`, `"search skill" | "find skill" | "lookup skill"` — then the fallback arm hardcodes a different, shorter list in prose:

   ```rust
   "Unknown operation '{}'. Valid operations: 'list skill', 'use skill', 'search skill'"
   ```

   The message omits the six aliases the code accepts. Derive the message from the accepted set so the two cannot drift. This covers the findings reported at both `:160` and `:220` — one defect, two sightings.

3. **Missing `Debug` derive** on `SkillTool` and on the `agent/mod.rs` types.

4. **Deep nesting** in `skill/mod.rs::execute` — the inferred-operation branch nests a second match inside the `""` arm.

5. **`&Arc<...>` parameters** in `agent/mod.rs` (and `main.rs`, covered separately). Take `&RwLock<...>` or clone at the call site; `&Arc<T>` is a double indirection that forces the caller to own an Arc it may not need.

6. **Duplicate match arms** in `agent/mod.rs::execute` — all three `AgentOperation` arms have identical bodies (`use ...::Execute; op.execute(&ctx).await`). Collapse.

## Acceptance

- `convert_result` is gone and nothing references it.
- A test proves the unknown-operation message lists every accepted alias, and fails if an alias is added to the match without appearing in the message.
- Max nesting 3 in `execute`.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Do NOT do the XML escaping here — that is ^gt1h2sc, kept separate because it is a security fix and should not be buried in a style sweep.