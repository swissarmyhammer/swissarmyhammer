---
position_column: doing
position_ordinal: a0
title: Migrate executor config consumers to select_executor()
---
Update all code that accesses `config.executor` field directly to use the new methods:

- [ ] `swissarmyhammer-agent/src/lib.rs:283` — replace `config.executor_type()` match + `config.executor` field access with `config.select_executor()` match on `&ModelExecutorConfig`. Add `AneEmbedding` to the embedding-rejected match arm.
- [ ] `swissarmyhammer-workflow/src/template_context.rs:445` — `get_llama_config()` uses `config.executor` field, change to `config.executor()` method
- [ ] `swissarmyhammer-workflow/src/actions.rs` — check for any `executor` field access
- [ ] `swissarmyhammer-tools/src/mcp/server.rs` — check for any `executor` field access
- [ ] `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs` — check for any `executor` field access
- [ ] `avp-common/src/context.rs:974` — test assertion on `config.executor`, change to `config.executor()`
- [ ] `avp-common/tests/model_config_integration.rs` — update ModelConfig construction
- [ ] Fix all compilation errors across workspace
- [ ] Run full test suite

Files: see list above (all in main swissarmyhammer repo)