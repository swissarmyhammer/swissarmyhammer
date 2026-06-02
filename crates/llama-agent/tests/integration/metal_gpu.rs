//! Proof that the llama-agent model load uses the Metal GPU backend on macOS.
//!
//! The kanban GUI felt CPU-slow on a 27B model, so this test removes the guess:
//! it loads the small `qwen-0.6b-test` model and asserts llama.cpp offloaded
//! every layer to the Metal GPU.
//!
//! ## Why capture stderr at the fd level
//!
//! llama.cpp / ggml print their backend logs (`ggml_metal_device_init: GPU
//! name: …`, `load_tensors: offloaded N/N layers to GPU`) straight to the C
//! `stderr` file descriptor — they do NOT flow through the `tracing` bridge an
//! in-process subscriber could capture. So the test temporarily redirects fd 2
//! to a temp file across the load, then reads it back.
//!
//! ## What fires when
//!
//! - `load_tensors: offloaded N/N layers to GPU` is printed on **every** model
//!   load — this is the definitive, per-load proof that all layers are on the
//!   GPU. On macOS the only GPU backend is Metal, so all-layers-offloaded == the
//!   model is running on Metal. This is the hard assertion.
//! - The `ggml_metal_device_init: … (Apple …)` lines come from the ggml backend
//!   singleton, which initializes once per process. Under nextest's
//!   process-per-test isolation (the CI path) they fire for this test too, so we
//!   also assert "metal" is present.
//!
//! macOS-only: Metal does not exist elsewhere.

#![cfg(target_os = "macos")]

use std::io::Write;
use std::os::unix::io::AsRawFd;

use llama_agent::model::ModelManager;
use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};
use llama_agent::types::{ModelConfig, ModelSource, RetryConfig, SessionId};
use serial_test::serial;

/// `debug = true` un-suppresses llama.cpp's native logging so the Metal/offload
/// lines are emitted to stderr (where we capture them).
fn debug_small_model_config() -> ModelConfig {
    ModelConfig {
        source: ModelSource::HuggingFace {
            repo: TEST_MODEL_REPO.to_string(),
            filename: Some(TEST_MODEL_FILE.to_string()),
            folder: None,
        },
        batch_size: 64,
        use_hf_params: true,
        retry_config: RetryConfig {
            max_retries: 2,
            initial_delay_ms: 100,
            backoff_multiplier: 1.5,
            max_delay_ms: 1000,
        },
        debug: true,
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
    }
}

/// RAII redirect of the process `stderr` fd to a temp file, restored on drop.
struct StderrCapture {
    tmp: tempfile::NamedTempFile,
    saved_fd: i32,
}

impl StderrCapture {
    fn begin() -> Self {
        let tmp = tempfile::NamedTempFile::new().expect("create temp file for stderr capture");
        let _ = std::io::stderr().flush();
        // Save the current stderr fd, then point fd 2 at the temp file.
        let saved_fd = unsafe { libc::dup(libc::STDERR_FILENO) };
        assert!(saved_fd >= 0, "dup(STDERR_FILENO) failed");
        let rc = unsafe { libc::dup2(tmp.as_file().as_raw_fd(), libc::STDERR_FILENO) };
        assert!(rc >= 0, "dup2 redirecting stderr failed");
        Self { tmp, saved_fd }
    }

    /// Restore the original stderr and return everything written while captured.
    fn finish(self) -> String {
        let _ = std::io::stderr().flush();
        unsafe {
            libc::dup2(self.saved_fd, libc::STDERR_FILENO);
            libc::close(self.saved_fd);
        }
        std::fs::read_to_string(self.tmp.path()).unwrap_or_default()
    }
}

#[tokio::test]
#[serial]
async fn qwen_small_model_loads_on_metal_gpu() {
    // Route any Rust-side logs (e.g. the skip warning) through tracing's test
    // writer. This is separate from the C `stderr` fd we redirect below, so it
    // does not pollute the captured llama.cpp logs.
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // This test PROVES every layer is offloaded to the Metal GPU. When the
    // environment explicitly disables GPU offload (`LLAMA_N_GPU_LAYERS=0`, e.g. a
    // deliberate local CPU run), that proof is impossible by construction — skip
    // rather than assert a GPU truth the environment forbade. With the var unset
    // (CI and normal GPU runs), `default_model_params` requests all layers and
    // the proof runs.
    if std::env::var("LLAMA_N_GPU_LAYERS").ok().as_deref() == Some("0") {
        tracing::warn!(
            "skipping Metal offload proof: LLAMA_N_GPU_LAYERS=0 disables GPU offload by request"
        );
        return;
    }

    // `#[tokio::test]` is a current-thread runtime, so the synchronous
    // `load_from_file` (and the ggml/llama stderr writes it triggers) happen on
    // this thread with fd 2 redirected.
    let manager =
        ModelManager::new(debug_small_model_config()).expect("ModelManager::new must succeed");

    let capture = StderrCapture::begin();
    let load_result = manager.load_model().await;
    // Flash attention + KV-cache type are applied when the CONTEXT is created,
    // not at model load — so create one inside the capture window too.
    let ctx_result = if load_result.is_ok() {
        manager
            .with_model(|model| {
                manager
                    .create_session_context(model, &SessionId::new())
                    .map(|_ctx| ())
            })
            .await
    } else {
        Ok(Ok(()))
    };
    let logs = capture.finish().to_lowercase();

    if let Err(e) = load_result {
        let msg = e.to_string().to_lowercase();
        if msg.contains("429")
            || msg.contains("too many requests")
            || msg.contains("rate limited")
            || msg.contains("failed to load")
            || msg.contains("loadingfailed")
        {
            tracing::warn!("skipping: model unavailable (download/rate-limit): {e}");
            return;
        }
        panic!("model load failed for a reason other than availability: {e}\nlogs:\n{logs}");
    }
    match ctx_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => panic!("context creation failed: {e}\nlogs:\n{logs}"),
        Err(e) => panic!("with_model failed: {e}\nlogs:\n{logs}"),
    }

    // Definitive per-load proof: llama.cpp reports how many layers it offloaded
    // to the GPU. Find `offloaded X/Y layers to gpu` and require X == Y > 0 —
    // every layer on the GPU. On macOS the GPU backend is Metal, so this is the
    // proof the model is NOT running on CPU.
    let offload_line = logs
        .lines()
        .find(|l| l.contains("offloaded") && l.contains("layers to gpu"))
        .unwrap_or_else(|| {
            panic!(
                "no `offloaded N/N layers to GPU` line in the load logs — the model did \
                 not offload to the GPU (running on CPU?). Captured stderr:\n{logs}"
            )
        });

    let (offloaded, total) = parse_offload_counts(offload_line).unwrap_or_else(|| {
        panic!("could not parse layer counts from offload line: {offload_line:?}")
    });
    assert!(total > 0, "model reported 0 total layers: {offload_line:?}");
    assert_eq!(
        offloaded, total,
        "not all layers were offloaded to the GPU ({offloaded}/{total}) — partial CPU \
         execution. Line: {offload_line:?}"
    );

    // Backend proof: on the CI path (nextest process-per-test) the ggml Metal
    // device initializes in-process and logs its device name.
    assert!(
        logs.contains("metal"),
        "load logs must name the Metal backend. Captured stderr:\n{logs}"
    );

    // Best-performance context params: flash attention must be enabled. llama
    // logs `llama_context: flash_attn = enabled` during context creation.
    assert!(
        logs.lines()
            .any(|l| l.contains("flash_attn") && l.contains("enabled")),
        "context must enable flash attention (expected a `flash_attn = enabled` line). \
         Captured stderr:\n{logs}"
    );

    // KV cache must be quantized to Q8_0, not the F16 default. llama logs the KV
    // cache element type per side, e.g.
    // `llama_kv_cache: ... k (q8_0): ... mib, v (q8_0): ... mib`.
    assert!(
        logs.contains("k (q8_0)") && logs.contains("v (q8_0)"),
        "KV cache K and V must both be quantized to q8_0 (not the f16 default). \
         Captured stderr:\n{logs}"
    );
}

/// Parse `(offloaded, total)` out of a llama.cpp line like
/// `load_tensors: offloaded 29/29 layers to gpu`.
fn parse_offload_counts(line: &str) -> Option<(u32, u32)> {
    let after = line.split("offloaded").nth(1)?;
    let frac = after.split_whitespace().next()?; // "29/29"
    let (a, b) = frac.split_once('/')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}
