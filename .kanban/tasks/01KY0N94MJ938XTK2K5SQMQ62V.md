---
assignees:
- claude-code
position_column: todo
position_ordinal: a580
title: 'model-loader: download-progress observer seam, threaded through the embedding loader'
---
## What

Model downloads are completely silent today: `swissarmyhammer_embedding::Embedder::default()` / `TextEmbedder::load()` (`crates/swissarmyhammer-embedding/src/embedder.rs`) call `model_loader::download_hf_file` / `load_huggingface_model_with_path` / `load_huggingface_model_with_path_and_folder`, which funnel into `download_with_retry` (`crates/model-loader/src/retry.rs`) → hf-hub `ApiRepo::download` with no progress hook. A first-run multi-hundred-MB-to-GB download produces zero events for the minutes it takes. This card adds the observer seam; a follow-up card (`depends_on` this one) surfaces it as MCP `notifications/progress` to keep clients alive.

hf-hub 0.4.3 (workspace `Cargo.toml`) already supports this: `ApiRepo::download_with_progress<P: hf_hub::api::tokio::Progress + Clone + Send + Sync>` and `ApiBuilder::with_endpoint` (for offline tests).

Implementation shape:

1. **`crates/model-loader`**: define `DownloadEvent { file: String, downloaded_bytes: u64, total_bytes: u64 }` and `DownloadObserver = Arc<dyn Fn(DownloadEvent) + Send + Sync>` (plain callback — no async in the observer; hf-hub's `Progress` impl adapter accumulates bytes and invokes it). Thread an `Option<DownloadObserver>` through `download_with_retry` / `download_with_retry_internal` (`src/retry.rs`, switching the inner call to `download_with_progress`), `download_multi_part_model` / `download_folder_model` (`src/multipart.rs`), and `download_hf_file` / `load_huggingface_model_with_path` / `load_huggingface_model_with_path_and_folder` (`src/huggingface.rs`); export the new types from `src/lib.rs`. `None` must be byte-identical to today's behavior. Emit an event at download start (downloaded=0, total from hf-hub's init) and per chunk update — never throttle away the final event (downloaded == total).
2. **`crates/swissarmyhammer-embedding/src/embedder.rs`**: give `Embedder` a way to attach the observer (e.g. `Embedder::with_download_observer(observer)` builder stored on the struct), forwarded into every `model_loader` download call in its load path (tokenizer + model files). Default (no observer) unchanged.

Note for tests: `download_with_retry` builds its own hf-hub `Api`; to point it at a local server either switch to `ApiBuilder::from_env()` (honors `HF_ENDPOINT`) or take an endpoint override — nextest runs each test in its own process, so setting `HF_ENDPOINT` per test is safe.

Subtasks:
- [ ] `DownloadEvent` + `DownloadObserver` types in model-loader, exported from `src/lib.rs`
- [ ] Thread `Option<DownloadObserver>` through `retry.rs` (use `download_with_progress` with an adapter implementing `hf_hub::api::tokio::Progress`), `multipart.rs`, `huggingface.rs`
- [ ] `Embedder::with_download_observer` in swissarmyhammer-embedding forwarding to all model_loader calls in the load path
- [ ] Offline integration test against a local HTTP server via `HF_ENDPOINT`/endpoint override
- [ ] `None`-observer regression: existing callers compile and behave unchanged (default param propagation, no signature break left unfixed)

## Acceptance Criteria
- [ ] Downloading a file through `download_with_retry` with an observer yields at least a start event and a final event with `downloaded_bytes == total_bytes`, with byte counts monotonically non-decreasing
- [ ] `DownloadEvent.file` carries the full filename, untruncated
- [ ] Passing `None` produces zero observer calls and the same download result as before the change (all existing call sites updated, workspace builds green)
- [ ] `Embedder::with_download_observer` forwards events for every file its load path downloads (model file(s) and tokenizer.json)
- [ ] model-loader gains no new non-dev dependencies beyond what hf-hub already provides

## Tests
- [ ] Integration test `crates/model-loader/tests/download_progress_test.rs`: serve a small fake model file from a local HTTP server (tiny hyper/axum dev-dependency or a hand-rolled tokio `TcpListener` responder), point hf-hub at it via `HF_ENDPOINT` (or the endpoint override), download with an observer, assert start/updates/final event sequence and monotonic bytes; second test asserts `None` observer downloads identically with zero events
- [ ] Unit test for the hf-hub `Progress`-trait adapter in `crates/model-loader/src/retry.rs` (init/update/finish → DownloadEvent mapping) — no network
- [ ] Run: `cargo nextest run -p model-loader` and `cargo nextest run -p swissarmyhammer-embedding` — green, under 10s per unit test (local server only, never the real HF hub in tests)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #mcp #progress #review