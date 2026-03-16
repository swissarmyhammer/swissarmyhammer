---
position_column: done
position_ordinal: u3
title: Add Platform enum, ExecutorEntry, AneEmbedding variant to config
---
Core config struct changes in `swissarmyhammer-config/src/model.rs`:

- [ ] Add `Platform` enum (`MacosArm64`, `MacosX86_64`, `LinuxArm64`, `LinuxX86_64`, `WindowsX86_64`) with `current()` using `cfg!` macros and kebab-case serde
- [ ] Add `ExecutorEntry` struct with `executor: ModelExecutorConfig` and `platform: Option<Platform>`
- [ ] Add `AneEmbedding(EmbeddingModelConfig)` variant to `ModelExecutorConfig` enum (serde rename `ane-embedding`)
- [ ] Add `AneEmbedding` variant to `ModelExecutorType` enum
- [ ] Add `debug: bool` field (serde default) to `EmbeddingModelConfig`
- [ ] Change `ModelConfig.executor` field from `ModelExecutorConfig` to `executors: Vec<ExecutorEntry>`
- [ ] Custom `Deserialize` for `ModelConfig`: check for `executors` key (new list format), fall back to `executor` key (wrap in single-element vec with `platform: None`)
- [ ] Custom `Deserialize` for `ExecutorEntry`: extract `platform` from Value, deserialize remainder as `ModelExecutorConfig`
- [ ] Add `select_executor(&self) -> Option<&ModelExecutorConfig>` — picks first entry matching `Platform::current()` or `platform: None`
- [ ] Add `executor(&self) -> &ModelExecutorConfig` convenience (calls select_executor, expects at least one match)
- [ ] Update `executor_type()` to use `select_executor()` internally
- [ ] Update `Default for ModelConfig` to use `executors: vec![...]`
- [ ] Update convenience constructors (`claude_code()`, `llama_agent()`, etc.)
- [ ] Export new types from `lib.rs`
- [ ] Run existing tests, fix compilation errors

Files: `swissarmyhammer-config/src/model.rs`, `swissarmyhammer-config/src/lib.rs`