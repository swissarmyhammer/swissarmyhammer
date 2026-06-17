---
assignees:
- claude-code
position_column: todo
position_ordinal: a180
project: kv-prefix-reuse
title: Source the recurrent rollback window from the model into the session-state store
---
## What
Wire the real recurrent-state rollback window into the `SessionStateStore.max_rollback` field added by the donor-selection keystone, so production hybrid/recurrent models (Qwen3.6-35B-A3B MoE) get rollback-aware selection, while pure-attention models keep `usize::MAX`.

In `crates/llama-agent/src/queue.rs`, `RequestQueue::new` (`queue.rs:1207`) builds the store at `queue.rs:1225` and already holds `model_manager` (with `get_config()`), mirroring how it derives `model_identifier_for_strategy`. Determine whether the loaded model is recurrent/hybrid and, if so, pass the `n_rs_seq` window (`N_RS_SEQ = 64`); otherwise pass `usize::MAX`.

- `N_RS_SEQ` is currently a **function-local `const`** inside the context-params builder in `crates/llama-agent/src/model.rs:436` — promote it to a module/associated const (or accessor) so the queue and the context-params builder share ONE definition; do not duplicate the literal `64`.
- Recurrent detection: prefer a model-capability check (the llama-cpp-rs fork may expose `llama_model_is_recurrent`/arch flags — check `/Users/wballard/github/swissarmyhammer/llama-cpp-rs/llama-cpp-2/src/model.rs`; recurrent handling lives in the vendored `src/llama-memory-recurrent.cpp`). If no clean capability bit is available at store-construction time, fall back to a model-config flag derived from the model alias/arch string. Pick the most robust available signal and document the choice in a code comment.
- Only the PRODUCTION store at `queue.rs:1225` (`RequestQueue::new`) gets the model-derived window. The `#[cfg(test)] with_executor` site at `queue.rs:1347` stays `usize::MAX`.
- The store is constructed before the context in some paths; if the recurrent bit is only known post-load, thread it through `model_manager.get_config()`/model metadata rather than the live context.

## Acceptance Criteria
- [ ] For the qwen hybrid model, the production `SessionStateStore` is constructed with `max_rollback == N_RS_SEQ` (64).
- [ ] For a pure-attention model, the store is constructed with `max_rollback == usize::MAX`.
- [ ] `N_RS_SEQ` has a single shared definition used by both `model.rs` context params and the queue (no duplicated literal).
- [ ] No ACP-layer or protocol changes.

## Tests
- [ ] Unit test in `crates/llama-agent` for a pure function `recurrent_rollback_window(&ModelConfig|metadata) -> usize`: returns 64 for a recurrent/hybrid descriptor, `usize::MAX` for attention-only (no live GPU model).
- [ ] `cargo test -p llama-agent` green; `cargo build -p llama-agent` clean.

## Depends on (prose — kanban depends_on edges currently dropped by a known bug): keystone 01KVBK83218VM915ZTVZCKZ9VA (introduces the `max_rollback` field).

## Workflow
- Use `/tdd` — write the detection-helper test first (RED), then implement and wire it into `RequestQueue::new`.