---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky10e2hc0cgahz0mancrbw5c
  text: 'Batch /finish #review picked this task (first READY in scope; review column empty). Iteration 1: delegating to /implement. Successor ^1tt5pa6 unblocks when this lands. Note from ^jn2wjd5''s run: keep test gates targeted (cargo nextest -p model-loader -p swissarmyhammer-embedding); never let a gate wander into real-model suites.'
  timestamp: 2026-07-21T00:15:27.276523+00:00
- actor: claude-code
  id: 01ky10wqnfr80yjh25fk3na36z
  text: |-
    Picked up; research done. Verified hf-hub 0.4.3 source: Progress trait is async (init(size,filename)/update(delta)/finish), download_with_progress ALWAYS downloads (no cache check — get() does cache-first then download()). Design decisions:

    1. model-loader src/observer.rs: DownloadEvent (private fields + getters + constructor, Debug/Clone/PartialEq/Eq) + DownloadObserver = Arc<dyn Fn(DownloadEvent)+Send+Sync>; exported from lib.rs.
    2. retry.rs: ObserverProgress adapter (Clone; Arc<Mutex<state>>; emits under lock so concurrent chunk tasks can't invert event order → monotonic guarantee). Observer path mirrors get()'s cache-first behavior via hf_hub::Cache::from_env() pre-check (download_with_progress would otherwise re-download cached blobs every load). None path stays repo_api.get() — byte-identical.
    3. huggingface.rs: switch ApiBuilder::new() → ApiBuilder::from_env() (card-sanctioned; honors HF_ENDPOINT/HF_HOME, and makes the retry.rs cache pre-check agree with the Api's cache dir). Observer param threaded through download_hf_file / load_huggingface_model_with_path(_and_folder) / multipart fns as Option<&DownloadObserver>.
    4. loader.rs: ModelResolver gains private observer field + with_download_observer(self, obs) builder; resolve() signature unchanged → llama-agent/tests/examples unaffected.
    5. llama-embedding EmbeddingModel: with_download_observer builder (its downloads happen at load_model time via ModelResolver).
    6. Embedder: with_download_observer(name, observer) is a CONSTRUCTOR variant, not an instance builder — justification: on macOS the ANE path downloads model files + tokenizer.json during from_model_name (build_ane_model resolves eagerly), so an instance builder applied after construction could never observe them. from_model_name delegates to the same internal fn with None.

    Tests: retry.rs unit tests for the adapter mapping (no network); tests/download_progress_test.rs with a hand-rolled tokio TcpListener HTTP responder (serves metadata Range bytes=0-0 → 206 with etag/x-repo-commit/Content-Range, then chunk GETs), HF_ENDPOINT+HF_HOME set per test (nextest = process per test). Edition 2021 so env::set_var is safe-fn.
  timestamp: 2026-07-21T00:23:27.663724+00:00
- actor: claude-code
  id: 01ky11dp31s2sf5ekeryb64aar
  text: |-
    Implementation landed (TDD: tests first, watched RED via missing ObserverProgress/observer module, then GREEN).

    Changes:
    - crates/model-loader/src/observer.rs (new): DownloadEvent (private fields, constructor + file()/downloaded_bytes()/total_bytes() getters, Debug/Clone/PartialEq/Eq) + DownloadObserver type alias; exported from lib.rs.
    - crates/model-loader/src/retry.rs: ObserverProgress adapter (Clone; shared Arc<Mutex<state>>; emits under the lock so hf-hub's cloned parallel-chunk handles stay monotonic; init → start event, update → accumulate, finish → forced downloaded==total). download_with_retry/_internal gained Option<&DownloadObserver>; new fetch_file helper — None path is exactly repo_api.get (byte-identical), observer path mirrors get's cache-first contract via hf_hub::Cache::from_env() then download_with_progress (download_with_progress always re-downloads, so the pre-check prevents attached observers from re-downloading cached blobs).
    - crates/model-loader/src/huggingface.rs: observer threaded through download_hf_file / load_huggingface_model_with_path(_and_folder); ApiBuilder::new() → from_env() (card-sanctioned; honors HF_ENDPOINT/HF_HOME and keeps the cache pre-check agreeing with the Api's cache dir).
    - crates/model-loader/src/multipart.rs: both fns thread the observer per part/file.
    - crates/model-loader/src/loader.rs: ModelResolver gained private download_observer field + with_download_observer builder; resolve() signature unchanged (llama-agent/tests/examples untouched); manual Debug.
    - crates/llama-embedding/src/model.rs: EmbeddingModel.with_download_observer builder; load_model attaches it to the resolver.
    - crates/swissarmyhammer-embedding: Embedder::with_download_observer(name, observer) constructor variant (ANE path downloads during from_model_name, so a post-construction builder could never observe those); from_model_name delegates to shared build(name, None); ANE path passes observer to resolver + tokenizer.json download (TOKENIZER_FILENAME const); lib.rs re-exports DownloadEvent/DownloadObserver.

    Tests: tests/download_progress_test.rs — hand-rolled tokio TcpListener fake hub (206 + etag/x-repo-commit/content-range per Range header), HF_ENDPOINT+HF_HOME per test, #[serial] for plain-cargo-test safety; observer test asserts start(0,total)/monotonic/final==total/full filename; None test asserts identical bytes. retry.rs unit tests: adapter init/update/finish mapping, finish-forces-total, clones-share-state. observer.rs unit tests for getters/alias.

    Gates: cargo nextest run -p model-loader -p swissarmyhammer-embedding → 186/186 passed (new integration tests ~50ms each); cargo clippy -p model-loader -p swissarmyhammer-embedding --all-targets -- -D warnings clean; cargo check --workspace --all-targets clean; cargo fmt applied. double-check agent running.
  timestamp: 2026-07-21T00:32:43.105140+00:00
- actor: claude-code
  id: 01ky11ywmfk9nhpt84w4s6x380
  text: |-
    really-done complete: verification gates green (nextest -p model-loader -p swissarmyhammer-embedding 186/186; clippy -D warnings clean; cargo check --workspace --all-targets clean; fmt applied) and adversarial double-check verdict: PASS.

    Double-check independently verified against vendored hf-hub 0.4.3 source: fetch_file's Cache::from_env() pre-check is check-for-check identical to ApiRepo::get's own cache lookup given the ApiBuilder::from_env() builders in huggingface.rs (invariant documented in-code); None path literally unchanged (repo_api.get); no missed call sites of the six changed signatures anywhere in the workspace; non-macOS builds unaffected; clippy also clean for llama-embedding. Two residual notes judged non-blocking and both documented in code: (a) a hypothetical future external caller building an ApiRepo with a custom cache dir would diverge from the from_env pre-check (no such caller exists); (b) across retry attempts the adapter resets and jumps to the resumed offset — monotonicity holds within each download attempt, which is what the contract specifies.

    All subtasks/acceptance/tests checked off. Leaving in doing for /review. Successor ^1tt5pa6 (MCP notifications/progress) can build on: model_loader::{DownloadEvent, DownloadObserver}, ModelResolver::with_download_observer, EmbeddingModel::with_download_observer, Embedder::with_download_observer(name, observer) (re-exported from swissarmyhammer_embedding).
  timestamp: 2026-07-21T00:42:06.863482+00:00
position_column: doing
position_ordinal: '8280'
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
- [x] `DownloadEvent` + `DownloadObserver` types in model-loader, exported from `src/lib.rs`
- [x] Thread `Option<DownloadObserver>` through `retry.rs` (use `download_with_progress` with an adapter implementing `hf_hub::api::tokio::Progress`), `multipart.rs`, `huggingface.rs`
- [x] `Embedder::with_download_observer` in swissarmyhammer-embedding forwarding to all model_loader calls in the load path
- [x] Offline integration test against a local HTTP server via `HF_ENDPOINT`/endpoint override
- [x] `None`-observer regression: existing callers compile and behave unchanged (default param propagation, no signature break left unfixed)

## Acceptance Criteria
- [x] Downloading a file through `download_with_retry` with an observer yields at least a start event and a final event with `downloaded_bytes == total_bytes`, with byte counts monotonically non-decreasing
- [x] `DownloadEvent.file` carries the full filename, untruncated
- [x] Passing `None` produces zero observer calls and the same download result as before the change (all existing call sites updated, workspace builds green)
- [x] `Embedder::with_download_observer` forwards events for every file its load path downloads (model file(s) and tokenizer.json)
- [x] model-loader gains no new non-dev dependencies beyond what hf-hub already provides

## Tests
- [x] Integration test `crates/model-loader/tests/download_progress_test.rs`: serve a small fake model file from a local HTTP server (tiny hyper/axum dev-dependency or a hand-rolled tokio `TcpListener` responder), point hf-hub at it via `HF_ENDPOINT` (or the endpoint override), download with an observer, assert start/updates/final event sequence and monotonic bytes; second test asserts `None` observer downloads identically with zero events
- [x] Unit test for the hf-hub `Progress`-trait adapter in `crates/model-loader/src/retry.rs` (init/update/finish → DownloadEvent mapping) — no network
- [x] Run: `cargo nextest run -p model-loader` and `cargo nextest run -p swissarmyhammer-embedding` — green, under 10s per unit test (local server only, never the real HF hub in tests)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #mcp #progress #review