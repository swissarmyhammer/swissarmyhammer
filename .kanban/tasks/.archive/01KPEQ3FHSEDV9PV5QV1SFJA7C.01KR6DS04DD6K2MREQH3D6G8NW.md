---
assignees: []
position_column: todo
position_ordinal: '9380'
title: 'Remove or fix the 10 #[ignore]d tests in the workspace'
---
`cargo nextest run --workspace` reports 15 skipped tests. 5 are filtered deliberately by the `default-filter` in `.config/nextest.toml` (the `qwen_embedding` / `unixcoder_` HF-model integration tests, run via `--profile embedding-models`) — those are acceptable. The remaining 10 are `#[ignore]`d in-source and must be fixed or deleted per the test-skill rule "a skipped test is either broken (fix it) or dead (delete it)".

Locations (via `rg '^\\s*#\\[ignore'`):

1. `llama-embedding/src/model.rs:655` — `#[ignore = "requires GGUF model downloaded locally"]`
2. `llama-embedding/src/model.rs:692` — same reason
3. `llama-embedding/src/model.rs:708` — same reason
4. `llama-embedding/src/model.rs:733` — same reason
   Action: these duplicate the `embedding-models` profile gate. Either keep the tests behind the existing profile filter (drop `#[ignore]`, rely on `default-filter`) or delete them outright. Do not leave both mechanisms in place.

5. `swissarmyhammer-treesitter/src/watcher.rs:586` — `#[ignore]` with note "filesystem watcher timing is inherently" flaky
6. `swissarmyhammer-treesitter/src/watcher.rs:685` — same
   Action: stabilise with deterministic polling or delete. Inherent timing flake is not a reason to keep a dead test around.

7. `swissarmyhammer-entity/src/undo_commands.rs:126` — `#[ignore = "requires StoreContext undo stack not yet on this branch"]`
8. `swissarmyhammer-entity/src/undo_commands.rs:139` — same
9. `swissarmyhammer-entity/src/undo_commands.rs:152` — same
10. `swissarmyhammer-entity/src/undo_commands.rs:178` — same
    Action: either the `StoreContext` undo stack lands (then enable these) or these are speculative and should be deleted.

Reference: test skill §3 — "Fix or delete each one — skipped tests are not acceptable." #test-failure