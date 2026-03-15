---
position_column: done
position_ordinal: i5
title: Extract shared backend init pattern from llama consumers
---
**llama-agent/src/model.rs, llama-embedding/src/model.rs**

Both consumers have nearly identical `get_or_init_backend` / `get_global_backend` patterns with `OnceLock<Result<Arc<LlamaBackend>, String>>`. This is duplicated code that could live in a shared crate (e.g., `llama-common`).

- [ ] Extract the global backend singleton into `llama-common` or a new shared module
- [ ] Have both consumers use the shared version
- [ ] Verify tests pass #review-finding