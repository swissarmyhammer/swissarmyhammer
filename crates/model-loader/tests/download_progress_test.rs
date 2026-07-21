//! Offline integration tests for download progress observation.
//!
//! A hand-rolled tokio HTTP responder plays the HuggingFace hub: it answers
//! hf-hub's metadata probe (`Range: bytes=0-0` → 206 with `etag`,
//! `x-repo-commit`, and `Content-Range`) and its subsequent ranged chunk
//! GETs with slices of a fake model payload. hf-hub is pointed at it via
//! `HF_ENDPOINT`, and `HF_HOME` is redirected to a per-test temp dir so the
//! cache starts cold. The real HuggingFace hub is never contacted.

use model_loader::{download_hf_file, DownloadEvent, DownloadObserver, RetryConfig};
use serial_test::serial;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Full, deliberately long filename — events must carry it untruncated.
const MODEL_FILENAME: &str = "qwen2.5-coder-embedding-fake-model-q8_0.gguf";
const REPO: &str = "test-org/test-repo";
const ETAG: &str = "deadbeefcafe0123";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
/// Overall guard so a wedged download can never hang the suite.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Deterministic fake model payload, large enough for several stream chunks.
fn fake_payload() -> Vec<u8> {
    (0..64 * 1024).map(|i| (i % 251) as u8).collect()
}

/// Extract the byte range from a raw HTTP request, if a Range header exists.
fn parse_range(request: &str) -> Option<(usize, usize)> {
    let line = request
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("range:"))?;
    let spec = line.split('=').nth(1)?.trim();
    let (start, stop) = spec.split_once('-')?;
    Some((start.trim().parse().ok()?, stop.trim().parse().ok()?))
}

/// Serve `payload` on an ephemeral port, answering every request with a 206
/// slice per its Range header (hf-hub's metadata probe and chunk downloads
/// are both ranged GETs). Returns the bound address.
async fn spawn_fake_hub(payload: Arc<Vec<u8>>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let payload = Arc::clone(&payload);
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                loop {
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request);
                let len = payload.len();
                let (start, stop) = parse_range(&request).unwrap_or((0, len - 1));
                let stop = stop.min(len - 1);
                let body = &payload[start..=stop];
                let header = format!(
                    "HTTP/1.1 206 Partial Content\r\n\
                     etag: \"{ETAG}\"\r\n\
                     x-repo-commit: {COMMIT}\r\n\
                     content-range: bytes {start}-{stop}/{len}\r\n\
                     content-length: {}\r\n\
                     connection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(header.as_bytes()).await.ok();
                stream.write_all(body).await.ok();
                stream.shutdown().await.ok();
            });
        }
    });
    addr
}

/// Point hf-hub at the fake hub and give it a cold cache in `cache_dir`.
///
/// Safe under `cargo nextest` (one process per test); `#[serial]` guards the
/// plain `cargo test` shared-process case.
fn point_hf_at(addr: std::net::SocketAddr, cache_dir: &std::path::Path) {
    std::env::set_var("HF_ENDPOINT", format!("http://{addr}"));
    std::env::set_var("HF_HOME", cache_dir);
}

/// Downloading with an observer yields a start event (0 of total), monotonic
/// per-chunk updates, and a final event with downloaded == total, each
/// carrying the full untruncated filename.
#[tokio::test]
#[serial]
async fn observer_receives_start_updates_and_final_event() {
    let payload = Arc::new(fake_payload());
    let cache_dir = tempfile::TempDir::new().unwrap();
    let addr = spawn_fake_hub(Arc::clone(&payload)).await;
    point_hf_at(addr, cache_dir.path());

    let events: Arc<Mutex<Vec<DownloadEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let observer: DownloadObserver = Arc::new(move |event| sink.lock().unwrap().push(event));

    let path = tokio::time::timeout(
        TEST_TIMEOUT,
        download_hf_file(
            REPO,
            MODEL_FILENAME,
            &RetryConfig::default(),
            Some(&observer),
        ),
    )
    .await
    .expect("download timed out")
    .expect("download failed");

    assert_eq!(
        std::fs::read(&path).unwrap(),
        *payload,
        "downloaded bytes must match the served payload"
    );

    let events = events.lock().unwrap();
    let total = payload.len() as u64;
    assert!(
        events.len() >= 2,
        "expected at least a start and a final event, got {events:?}"
    );
    let first = &events[0];
    assert_eq!(
        first.downloaded_bytes(),
        0,
        "start event must be at 0 bytes"
    );
    assert_eq!(first.total_bytes(), total, "start event carries the total");
    for event in events.iter() {
        assert_eq!(
            event.file(),
            MODEL_FILENAME,
            "every event carries the full untruncated filename"
        );
        assert_eq!(event.total_bytes(), total);
    }
    for pair in events.windows(2) {
        assert!(
            pair[1].downloaded_bytes() >= pair[0].downloaded_bytes(),
            "byte counts must be monotonically non-decreasing: {events:?}"
        );
    }
    let last = events.last().unwrap();
    assert_eq!(
        last.downloaded_bytes(),
        total,
        "final event must reach downloaded == total"
    );
}

/// Passing `None` downloads the identical bytes with zero observer calls —
/// byte-identical to the pre-observer behavior. (With no observer attached
/// there is no callback that could fire; this asserts the download result
/// itself is unchanged.)
#[tokio::test]
#[serial]
async fn none_observer_downloads_identically_with_zero_events() {
    let payload = Arc::new(fake_payload());
    let cache_dir = tempfile::TempDir::new().unwrap();
    let addr = spawn_fake_hub(Arc::clone(&payload)).await;
    point_hf_at(addr, cache_dir.path());

    let path = tokio::time::timeout(
        TEST_TIMEOUT,
        download_hf_file(REPO, MODEL_FILENAME, &RetryConfig::default(), None),
    )
    .await
    .expect("download timed out")
    .expect("download failed");

    assert_eq!(
        std::fs::read(&path).unwrap(),
        *payload,
        "a None observer must download the identical bytes"
    );
}
