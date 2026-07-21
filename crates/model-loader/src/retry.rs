use crate::download_lock::DownloadCoordinator;
use crate::error::ModelError;
use crate::observer::{DownloadEvent, DownloadObserver};
use llama_common::retry::RetryManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::debug;

/// Progress state shared between all clones of an [`ObserverProgress`].
///
/// hf-hub clones its `Progress` handle into every parallel chunk task, so the
/// accumulated byte count lives behind a shared mutex. Events are emitted
/// while holding the lock, which serializes concurrent chunk updates and
/// guarantees observers see monotonically non-decreasing byte counts.
struct ObserverProgressState {
    downloaded_bytes: u64,
    total_bytes: u64,
}

/// Adapts a [`DownloadObserver`] callback to hf-hub's
/// [`hf_hub::api::tokio::Progress`] trait.
///
/// `init` emits the start event (0 of the total reported by the hub),
/// `update` accumulates per-chunk deltas, and `finish` forces the final event
/// to `downloaded_bytes == total_bytes` so it can never be lost.
#[derive(Clone)]
struct ObserverProgress {
    observer: DownloadObserver,
    /// Full, untruncated filename carried on every event.
    file: Arc<str>,
    state: Arc<Mutex<ObserverProgressState>>,
}

impl ObserverProgress {
    /// Create an adapter for `file` (the full filename) reporting to `observer`.
    fn new(file: &str, observer: DownloadObserver) -> Self {
        Self {
            observer,
            file: Arc::from(file),
            state: Arc::new(Mutex::new(ObserverProgressState {
                downloaded_bytes: 0,
                total_bytes: 0,
            })),
        }
    }

    /// Mutate the shared state and emit the resulting event while still
    /// holding the lock (keeps concurrent chunk updates monotonic).
    fn emit_with<F: FnOnce(&mut ObserverProgressState)>(&self, mutate: F) {
        let mut state = self.state.lock().expect("progress state lock poisoned");
        mutate(&mut state);
        (self.observer)(DownloadEvent::new(
            &self.file,
            state.downloaded_bytes,
            state.total_bytes,
        ));
    }
}

impl hf_hub::api::tokio::Progress for ObserverProgress {
    async fn init(&mut self, size: usize, _filename: &str) {
        self.emit_with(|state| state.total_bytes = size as u64);
    }

    async fn update(&mut self, size: usize) {
        self.emit_with(|state| state.downloaded_bytes += size as u64);
    }

    async fn finish(&mut self) {
        self.emit_with(|state| state.downloaded_bytes = state.total_bytes);
    }
}

/// Downloads a model file with retry logic, exponential backoff, and cross-process coordination
///
/// This function:
/// 1. Uses cross-process locking to prevent duplicate downloads
/// 2. Uses the unified retry manager for consistent behavior
/// 3. Waits for other processes if they're already downloading the same file
///
/// # Arguments
///
/// * `repo_api` - hf-hub repo handle to download through
/// * `filename` - name of the file within the repository
/// * `repo` - repository identifier (e.g. `org/repo`), used for locking and errors
/// * `retry_config` - retry/backoff behavior
/// * `observer` - optional progress callback; `None` is byte-identical to the
///   pre-observer behavior (zero callbacks, same cache-first download)
pub async fn download_with_retry(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    filename: &str,
    repo: &str,
    retry_config: &crate::types::RetryConfig,
    observer: Option<&DownloadObserver>,
) -> Result<PathBuf, ModelError> {
    // Create coordinator for cross-process download synchronization
    let coordinator = DownloadCoordinator::new()?;

    // Coordinate the download - if another process is downloading, we'll wait
    coordinator
        .coordinate_download(repo, filename, || {
            download_with_retry_internal(repo_api, filename, repo, retry_config, observer)
        })
        .await
}

/// Fetch `filename` once, observed or not.
///
/// Without an observer this is exactly `ApiRepo::get` (cache-first, then
/// download). With an observer it mirrors `get`'s cache-first contract by
/// hand — `download_with_progress` always re-downloads, so a cached file is
/// returned directly (a cache hit downloads nothing and emits nothing) and
/// only a real download streams events through [`ObserverProgress`].
async fn fetch_file(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    filename: &str,
    repo: &str,
    observer: Option<DownloadObserver>,
) -> Result<PathBuf, hf_hub::api::tokio::ApiError> {
    let Some(observer) = observer else {
        return repo_api.get(filename).await;
    };

    let cached = hf_hub::Cache::from_env()
        .repo(hf_hub::Repo::model(repo.to_string()))
        .get(filename);
    match cached {
        Some(path) => Ok(path),
        None => {
            repo_api
                .download_with_progress(filename, ObserverProgress::new(filename, observer))
                .await
        }
    }
}

/// Internal download function that handles retries (called by coordinator)
/// Not public - only used internally by download_with_retry
async fn download_with_retry_internal(
    repo_api: &hf_hub::api::tokio::ApiRepo,
    filename: &str,
    repo: &str,
    retry_config: &crate::types::RetryConfig,
    observer: Option<&DownloadObserver>,
) -> Result<PathBuf, ModelError> {
    debug!("Starting download for {}/{}", repo, filename);

    let retry_manager = RetryManager::with_config(retry_config.clone().into());
    let operation_name = format!("download {}", filename);

    retry_manager
        .retry(&operation_name, || {
            // Fresh observer handle per attempt: a retried attempt restarts
            // its progress accounting from the resumed offset.
            let observer = observer.cloned();
            async move {
                match fetch_file(repo_api, filename, repo, observer).await {
                    Ok(path) => Ok(path),
                    Err(e) => {
                        // Convert HuggingFace error to ModelError for retry logic
                        if e.to_string().to_lowercase().contains("not found")
                            || e.to_string().contains("404")
                        {
                            Err(ModelError::NotFound(format!(
                                "HuggingFace resource not found: {}",
                                e
                            )))
                        } else {
                            Err(ModelError::Network(format!("HuggingFace API error: {}", e)))
                        }
                    }
                }
            }
        })
        .await
        .map_err(|e| {
            // Add additional context to the error
            match e {
                ModelError::NotFound(msg) => ModelError::LoadingFailed(format_download_error(
                    filename,
                    repo,
                    &std::io::Error::new(std::io::ErrorKind::NotFound, msg),
                    retry_config.max_retries,
                )),
                ModelError::Network(msg) => ModelError::LoadingFailed(format_download_error(
                    filename,
                    repo,
                    &std::io::Error::other(msg),
                    retry_config.max_retries,
                )),
                other => other,
            }
        })
}

/// Determines if an error is retriable based on the error message
pub fn is_retriable_error(error: &dyn std::error::Error) -> bool {
    let error_msg = error.to_string().to_lowercase();

    // Check for specific HTTP status codes or error patterns
    if error_msg.contains("500") || error_msg.contains("internal server error") {
        return true;
    }
    if error_msg.contains("502") || error_msg.contains("bad gateway") {
        return true;
    }
    if error_msg.contains("503") || error_msg.contains("service unavailable") {
        return true;
    }
    if error_msg.contains("504") || error_msg.contains("gateway timeout") {
        return true;
    }
    // 429 errors should not be retried - HuggingFace explicitly asks us to wait
    if error_msg.contains("429") || error_msg.contains("too many requests") {
        return false;
    }

    // Network-level errors are retriable
    if error_msg.contains("connection")
        || error_msg.contains("timeout")
        || error_msg.contains("network")
    {
        return true;
    }

    // Client errors (4xx) are generally not retriable
    if error_msg.contains("404") || error_msg.contains("not found") {
        return false;
    }
    if error_msg.contains("403") || error_msg.contains("forbidden") {
        return false;
    }
    if error_msg.contains("401") || error_msg.contains("unauthorized") {
        return false;
    }

    // Default to retriable for unknown errors
    true
}

/// Formats a comprehensive error message for download failures
pub fn format_download_error(
    filename: &str,
    repo: &str,
    error: &dyn std::error::Error,
    retries_attempted: u32,
) -> String {
    let base_message = format!(
        "Failed to download model file '{}' from repository '{}' after {} retries: {}",
        filename, repo, retries_attempted, error
    );

    let error_msg = error.to_string().to_lowercase();

    // Add specific guidance based on error type
    let guidance = if error_msg.contains("404") || error_msg.contains("not found") {
        "📁 File not found. Verify the filename exists in the repository. You can browse the repo at https://huggingface.co/"
    } else if error_msg.contains("403") || error_msg.contains("forbidden") {
        "🔒 Access forbidden. Check if the repository is private and if you need authentication."
    } else if error_msg.contains("429") || error_msg.contains("too many requests") {
        "⏱️ Rate limited by HuggingFace. Wait 5-10 minutes before trying again to respect their API limits."
    } else if error_msg.contains("500")
        || error_msg.contains("502")
        || error_msg.contains("503")
        || error_msg.contains("504")
    {
        "🏥 Server error on HuggingFace. This is temporary - try again in a few minutes."
    } else {
        "🌐 Network error. Check your internet connection and try again."
    };

    let additional_help = "💡 Check model file exists, is valid GGUF format, and sufficient memory is available\n🔧 You can increase retry attempts by configuring retry_config.max_retries";

    format!("{}\n{}\n{}", base_message, guidance, additional_help)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observer::{DownloadEvent, DownloadObserver};
    use crate::types::RetryConfig;
    use hf_hub::api::tokio::Progress as _;
    use std::sync::{Arc, Mutex};

    /// Build an observer that records every event into the returned sink.
    fn recording_observer() -> (DownloadObserver, Arc<Mutex<Vec<DownloadEvent>>>) {
        let events: Arc<Mutex<Vec<DownloadEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let observer: DownloadObserver = Arc::new(move |event| sink.lock().unwrap().push(event));
        (observer, events)
    }

    /// init/update/finish map to a start event (0 of total), accumulated
    /// per-chunk events, and a final event with downloaded == total.
    #[tokio::test]
    async fn observer_progress_maps_init_update_finish_to_events() {
        let (observer, events) = recording_observer();
        let mut adapter = ObserverProgress::new("full-model-name-q8_0.gguf", observer);

        adapter.init(1000, "full-model-name-q8_0.gguf").await;
        adapter.update(400).await;
        adapter.update(600).await;
        adapter.finish().await;

        let events = events.lock().unwrap();
        let bytes: Vec<(u64, u64)> = events
            .iter()
            .map(|e| (e.downloaded_bytes(), e.total_bytes()))
            .collect();
        assert_eq!(
            bytes,
            vec![(0, 1000), (400, 1000), (1000, 1000), (1000, 1000)],
            "init emits the start event, updates accumulate, finish lands on total"
        );
        for event in events.iter() {
            assert_eq!(event.file(), "full-model-name-q8_0.gguf");
        }
    }

    /// finish always forces the final event to downloaded == total, even when
    /// per-chunk updates did not account for every byte.
    #[tokio::test]
    async fn observer_progress_finish_forces_final_event_to_total() {
        let (observer, events) = recording_observer();
        let mut adapter = ObserverProgress::new("model.gguf", observer);

        adapter.init(1000, "model.gguf").await;
        adapter.update(250).await;
        adapter.finish().await;

        let events = events.lock().unwrap();
        let last = events.last().unwrap();
        assert_eq!(last.downloaded_bytes(), 1000);
        assert_eq!(last.total_bytes(), 1000);
    }

    /// hf-hub clones the progress handle into parallel chunk tasks; clones
    /// must share accumulated state so byte counts stay global and monotonic.
    #[tokio::test]
    async fn observer_progress_clones_share_accumulated_state() {
        let (observer, events) = recording_observer();
        let mut adapter = ObserverProgress::new("model.gguf", observer);
        let mut clone = adapter.clone();

        adapter.init(1000, "model.gguf").await;
        adapter.update(300).await;
        clone.update(700).await;
        clone.finish().await;

        let events = events.lock().unwrap();
        let downloaded: Vec<u64> = events.iter().map(|e| e.downloaded_bytes()).collect();
        assert_eq!(downloaded, vec![0, 300, 1000, 1000]);
    }

    #[test]
    fn test_is_retriable_error_server_errors() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Server errors should be retriable
        assert!(is_retriable_error(&TestError(
            "500 Internal Server Error".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "502 Bad Gateway".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "503 Service Unavailable".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "504 Gateway Timeout".to_string()
        )));
        assert!(!is_retriable_error(&TestError(
            "429 Too Many Requests".to_string()
        )));
    }

    #[test]
    fn test_is_retriable_error_client_errors() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Client errors should not be retriable
        assert!(!is_retriable_error(&TestError("404 Not Found".to_string())));
        assert!(!is_retriable_error(&TestError("403 Forbidden".to_string())));
        assert!(!is_retriable_error(&TestError(
            "401 Unauthorized".to_string()
        )));
    }

    #[test]
    fn test_is_retriable_error_network_errors() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Network errors should be retriable
        assert!(is_retriable_error(&TestError(
            "Connection timeout".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "Network unreachable".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "Connection refused".to_string()
        )));
    }

    #[test]
    fn test_format_download_error() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        let error = TestError("404 Not Found".to_string());
        let result = format_download_error("model.gguf", "test/repo", &error, 3);

        assert!(result.contains("model.gguf"));
        assert!(result.contains("test/repo"));
        assert!(result.contains("3 retries"));
        assert!(result.contains("📁")); // Should contain file not found guidance
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let retry_config = RetryConfig::default();
        let mut delay = retry_config.initial_delay_ms;

        // Test exponential backoff progression
        assert_eq!(delay, 1000); // Initial: 1s

        delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
        delay = delay.min(retry_config.max_delay_ms);
        assert_eq!(delay, 2000); // 2s

        delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
        delay = delay.min(retry_config.max_delay_ms);
        assert_eq!(delay, 4000); // 4s

        // Continue until we hit the max
        for _ in 0..10 {
            delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
            delay = delay.min(retry_config.max_delay_ms);
        }
        assert_eq!(delay, retry_config.max_delay_ms); // Should cap at 30s
    }

    #[test]
    fn test_download_coordinator_creation() {
        // Verify that the DownloadCoordinator can be created successfully
        // This tests the setup path of download_with_retry without requiring network access
        let coordinator = DownloadCoordinator::new();
        assert!(
            coordinator.is_ok(),
            "DownloadCoordinator should be creatable"
        );
    }

    // Note: download_with_retry and download_with_retry_internal require network access
    // to HuggingFace and are tested via integration tests. The coordinator logic
    // is tested in download_lock.rs unit tests.

    #[test]
    fn test_is_retriable_error_unknown_defaults_to_retriable() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Unknown errors default to retriable
        assert!(is_retriable_error(&TestError(
            "something weird happened".to_string()
        )));
        assert!(is_retriable_error(&TestError("".to_string())));
    }

    #[test]
    fn test_is_retriable_error_case_insensitive() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Case insensitive matching
        assert!(is_retriable_error(&TestError(
            "INTERNAL SERVER ERROR".to_string()
        )));
        assert!(is_retriable_error(&TestError("Bad Gateway".to_string())));
        assert!(is_retriable_error(&TestError(
            "SERVICE UNAVAILABLE".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "GATEWAY TIMEOUT".to_string()
        )));
        assert!(!is_retriable_error(&TestError("NOT FOUND".to_string())));
        assert!(!is_retriable_error(&TestError("FORBIDDEN".to_string())));
        assert!(!is_retriable_error(&TestError("UNAUTHORIZED".to_string())));
        assert!(!is_retriable_error(&TestError(
            "TOO MANY REQUESTS".to_string()
        )));
    }

    #[test]
    fn test_is_retriable_error_mixed_messages() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        // Connection-related errors
        assert!(is_retriable_error(&TestError(
            "Connection reset by peer".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "Request timeout after 30s".to_string()
        )));
        assert!(is_retriable_error(&TestError(
            "Network is unreachable (os error 101)".to_string()
        )));
    }

    #[test]
    fn test_format_download_error_forbidden() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        let error = TestError("403 Forbidden".to_string());
        let result = format_download_error("model.gguf", "test/repo", &error, 3);
        assert!(result.contains("model.gguf"));
        assert!(result.contains("test/repo"));
        assert!(result.contains("3 retries"));
        assert!(result.contains("🔒")); // Forbidden guidance
        assert!(result.contains("authentication"));
    }

    #[test]
    fn test_format_download_error_rate_limited() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        let error = TestError("429 Too Many Requests".to_string());
        let result = format_download_error("model.gguf", "test/repo", &error, 5);
        assert!(result.contains("5 retries"));
        assert!(result.contains("⏱️")); // Rate limit guidance
        assert!(result.contains("Rate limited"));
    }

    #[test]
    fn test_format_download_error_server_error() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        for code in ["500", "502", "503", "504"] {
            let error = TestError(format!("{} Server Error", code));
            let result = format_download_error("model.gguf", "test/repo", &error, 2);
            assert!(
                result.contains("🏥"),
                "Server error {} should contain hospital emoji",
                code
            );
            assert!(result.contains("Server error"));
        }
    }

    #[test]
    fn test_format_download_error_generic_network() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        let error = TestError("Connection refused".to_string());
        let result = format_download_error("model.gguf", "test/repo", &error, 1);
        assert!(result.contains("🌐")); // Network error guidance
        assert!(result.contains("internet connection"));
    }

    #[test]
    fn test_format_download_error_contains_help() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        let error = TestError("some error".to_string());
        let result = format_download_error("model.gguf", "test/repo", &error, 3);
        // All messages should contain the additional help text
        assert!(result.contains("💡"));
        assert!(result.contains("GGUF format"));
        assert!(result.contains("retry_config.max_retries"));
    }

    #[test]
    fn test_format_download_error_zero_retries() {
        #[derive(Debug)]
        struct TestError(String);
        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for TestError {}

        let error = TestError("error".to_string());
        let result = format_download_error("model.gguf", "test/repo", &error, 0);
        assert!(result.contains("0 retries"));
    }

    #[test]
    fn test_retry_config_custom_values() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay_ms: 500,
            backoff_multiplier: 1.5,
            max_delay_ms: 5000,
        };
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.initial_delay_ms, 500);
        assert_eq!(config.backoff_multiplier, 1.5);
        assert_eq!(config.max_delay_ms, 5000);
    }
}
