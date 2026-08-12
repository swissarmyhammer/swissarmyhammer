//! Watching the prompt directories, and stopping cleanly when the server does.
//!
//! FSEvents registration is synchronous and can be slow, so it never runs on a
//! tokio worker: startup is handed to a background task whose handle the server
//! keeps. Shutdown aborts that handle and latches a stop flag, so a
//! registration still in flight cannot resurrect the watcher after shutdown.

use super::retry::retry_with_backoff;
use super::McpServer;
use crate::mcp::file_watcher::{FileWatcher, McpFileWatcherCallback};
use rmcp::RoleServer;

use swissarmyhammer_common::Result;

impl McpServer {
    /// Start watching prompt directories for file changes.
    ///
    /// When files change, the server will automatically reload prompts and
    /// send notifications to the MCP client.
    ///
    /// # Arguments
    ///
    /// * `peer` - The MCP peer connection for sending notifications
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if watching starts successfully, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if file watching cannot be initialized.
    pub async fn start_file_watching(&self, peer: rmcp::Peer<RoleServer>) -> Result<()> {
        // Create callback that handles file changes and notifications
        let callback = McpFileWatcherCallback::new(self.clone(), peer);
        self.start_file_watching_with_callback(callback).await
    }

    /// Start file watching with an explicit callback.
    ///
    /// WHY this builds the watcher off the shared `file_watcher` lock: on macOS
    /// the FSEvents `.watch()` registration inside `FileWatcher::start` blocks
    /// for seconds. If that ran while holding `self.file_watcher`, a concurrent
    /// `stop_file_watching()` (server shutdown) would block for the full
    /// registration time waiting for the lock. Instead we build the started
    /// watcher with no lock held, then acquire the lock only briefly to store
    /// it — so shutdown stays fast even mid-registration.
    ///
    /// The brief store also honors `file_watch_stopped`: if shutdown ran while
    /// this registration was in flight, the freshly-built watcher is dropped
    /// (its teardown detaches to a background thread) instead of being stored,
    /// so the watcher is never resurrected after shutdown.
    async fn start_file_watching_with_callback<C>(&self, callback: C) -> Result<()>
    where
        C: crate::mcp::file_watcher::FileWatcherCallback + Clone,
    {
        let started = retry_with_backoff(
            || {
                let callback = callback.clone();
                async move { FileWatcher::start(callback).await }
            },
            Self::is_retryable_fs_error,
            "File watcher initialization",
        )
        .await?;

        // Briefly lock to swap in the started watcher, unless shutdown has
        // already run. Dropping `started` here (when stopped) detaches its
        // FSEvents teardown to a background thread via `FileWatcher::Drop`.
        if self
            .file_watch_stopped
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            drop(started);
            return Ok(());
        }
        let mut watcher = self.file_watcher.lock().await;
        *watcher = started;
        Ok(())
    }

    /// Stop watching prompt directories for file changes.
    ///
    /// This should be called when the MCP server is shutting down.
    ///
    /// WHY it returns promptly even mid-registration: it first sets
    /// `file_watch_stopped` and aborts the in-flight startup task, so a slow
    /// FSEvents `.watch()` that is still running off-lock cannot store an
    /// active watcher afterward. Because startup never holds `file_watcher`
    /// across the slow registration, the brief lock here is uncontended, and
    /// the actual debouncer teardown is detached by `FileWatcher::stop_watching`.
    pub async fn stop_file_watching(&self) {
        self.file_watch_stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // Abort any in-flight startup task. This is a best-effort speedup; the
        // `file_watch_stopped` flag is the correctness guarantee against a late
        // store. Abort cannot interrupt a synchronous FSEvents `.watch()` call,
        // but that call runs off-lock so it does not block this shutdown.
        if let Some(handle) = self.file_watcher_task.lock().await.take() {
            handle.abort();
        }

        let mut watcher = self.file_watcher.lock().await;
        watcher.stop_watching();
    }

    /// Spawn the prompt-directory file watcher on a background task so the
    /// MCP initialize handshake can return without waiting for FSEvents
    /// debouncer construction (~400ms on macOS). Failures are logged; they
    /// never fail the handshake. The task handle is retained so
    /// `stop_file_watching()` can abort an in-flight registration.
    pub(super) fn spawn_background_file_watcher(&self, peer: rmcp::Peer<RoleServer>) {
        let server = self.clone();
        let task_slot = self.file_watcher_task.clone();
        let handle = tokio::spawn(async move {
            match server.start_file_watching(peer).await {
                Ok(_) => tracing::info!("🔍 File watching started for MCP client"),
                Err(e) => {
                    tracing::error!("✗ Failed to start file watching for MCP client: {}", e)
                }
            }
        });
        // Store the handle so shutdown can abort an in-flight startup. Spawned
        // detached so the handshake is not delayed by acquiring the slot lock.
        tokio::spawn(async move {
            *task_slot.lock().await = Some(handle);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_templating::TemplateLibrary;

    /// The promptness bound every shutdown test in this module asserts. The
    /// regression these tests guard against stalls for seconds, so one second
    /// separates a correct shutdown from a regressed one without being tight
    /// enough to fail on a loaded machine.
    const MAX_SHUTDOWN_ELAPSED: std::time::Duration = std::time::Duration::from_secs(1);

    /// Worker threads for the teardown test's own runtime. Two is the smallest
    /// count that makes it multi-threaded, which is what the test needs: one
    /// worker can be pinned inside a synchronous `.watch()` while the other
    /// still drives the shutdown.
    const TEARDOWN_WORKER_THREADS: usize = 2;

    /// Upper bound on the teardown test's runtime drop. A regressed teardown
    /// blocks for as long as the FSEvents registration takes (~4.5 s measured),
    /// so this sits well above that: it exists only to turn a hang into a
    /// measurable stall the assertion can then report.
    const TEARDOWN_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

    /// How long the teardown test works before shutting down. Long enough to
    /// let the background registration start, short enough that shutdown still
    /// lands while that registration is in flight.
    const INFLIGHT_WORK_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);

    // ---------------------------------------------------------------
    // stop_file_watching() tests
    // ---------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_stop_file_watching_is_safe_without_start() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        // Stopping without ever starting must still reach the full shutdown:
        // it latches the stop flag and leaves the watcher inactive, rather
        // than short-circuiting on the absent watcher.
        server.stop_file_watching().await;

        assert!(
            server
                .file_watch_stopped
                .load(std::sync::atomic::Ordering::SeqCst),
            "shutdown must latch the stop flag even when no watcher was started"
        );
        let watcher = server.file_watcher.lock().await;
        assert!(
            watcher.debouncer_is_none_for_test(),
            "no watcher was started, so none may be active after shutdown"
        );
    }

    /// Minimal no-op `FileWatcherCallback` for exercising the server's
    /// file-watch lifecycle without a live MCP peer.
    #[derive(Clone)]
    struct NoopWatchCallback;

    impl crate::mcp::file_watcher::FileWatcherCallback for NoopWatchCallback {
        async fn on_file_changed(&self, _paths: Vec<std::path::PathBuf>) -> Result<()> {
            Ok(())
        }
        async fn on_error(&self, _error: String) {}
    }

    /// Shutdown-promptness guard: `stop_file_watching()` must return in well
    /// under 1s even while a file-watch registration is in flight.
    ///
    /// WHY: the in-process MCP server's slow `shutdown()` was caused by
    /// `stop_file_watching()` blocking on the `file_watcher` lock that the
    /// background startup held across the slow macOS FSEvents `.watch()`
    /// registration. The fix builds the watcher off-lock, so this test spawns a
    /// startup and immediately tears down — it must complete fast. It fails
    /// (~5s) if startup regresses to holding the lock across registration.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_stop_file_watching_returns_promptly_during_inflight_registration() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        // Kick off a real off-lock registration in the background and register
        // its handle exactly like `spawn_background_file_watcher` does, so the
        // shutdown path exercises the abort branch.
        let starter = server.clone();
        let start_handle = tokio::spawn(async move {
            let _ = starter
                .start_file_watching_with_callback(NoopWatchCallback)
                .await;
        });
        *server.file_watcher_task.lock().await = Some(start_handle);

        // Tear down while the registration may still be in flight; must be fast.
        //
        // We intentionally do NOT await the background startup: aborting cannot
        // interrupt a synchronous in-flight FSEvents `.watch()`, so awaiting it
        // would re-introduce the multi-second OS registration latency into the
        // test. The `file_watch_stopped` flag set by `stop_file_watching`
        // guarantees the late store is suppressed even though the task is left
        // to drain on its own.
        let start = std::time::Instant::now();
        server.stop_file_watching().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < MAX_SHUTDOWN_ELAPSED,
            "stop_file_watching blocked for {elapsed:?}; it must not wait on an \
             in-flight registration"
        );
    }

    /// Late-store suppression guard: once `stop_file_watching()` has run, an
    /// in-flight startup that finishes afterward must NOT resurrect an active
    /// watcher.
    ///
    /// WHY: with the registration built off-lock, a startup task can complete
    /// its slow `.watch()` after shutdown. The `file_watch_stopped` flag makes
    /// the late store a no-op (dropping the freshly-built watcher) so the
    /// watcher is never active post-shutdown.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_stop_file_watching_suppresses_late_store() {
        let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

        // Shutdown first.
        server.stop_file_watching().await;

        // A start that completes afterward must not install an active watcher.
        server
            .start_file_watching_with_callback(NoopWatchCallback)
            .await
            .unwrap();

        let watcher = server.file_watcher.lock().await;
        assert!(
            watcher.debouncer_is_none_for_test(),
            "watcher must stay inactive after shutdown — late store was not suppressed"
        );
    }

    /// Runtime-teardown guard: an in-process server's full file-watch lifecycle
    /// (background-startup spawn -> brief work -> shutdown -> runtime drop) must
    /// complete in well under 1s — including the runtime teardown itself.
    ///
    /// WHY a self-contained runtime measured by `#[test]` rather than
    /// `#[tokio::test]`: the bug this guards against is a *runtime teardown*
    /// stall. The slow, synchronous macOS FSEvents `.watch()` registration used
    /// to run on a tokio worker (the background startup task). `abort()` cannot
    /// interrupt a synchronous `.watch()`, so the worker stayed pinned for the
    /// full ~4.5s registration, and dropping the runtime waited for that pinned
    /// worker. That stall is invisible to an `async fn` test (it returns before
    /// its runtime drops), so this test builds its OWN multi-thread runtime,
    /// drives the lifecycle on it, then times `drop(runtime)`. With the
    /// registration moved onto a detached `std::thread`, nothing on the runtime
    /// blocks on FSEvents, so teardown is immediate. This test fails (~4.5s) if
    /// the registration regresses back onto a tokio worker.
    ///
    /// The work directory is the crate's own checkout, which has real prompt
    /// directories, so the FSEvents registration is genuinely slow — exactly the
    /// condition that exposes the regression. Its latency is absorbed by the
    /// detached thread and must not appear in the measured teardown.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_server_lifecycle_runtime_teardown_is_fast() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(TEARDOWN_WORKER_THREADS)
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let server = McpServer::new(TemplateLibrary::default()).await.unwrap();

            // Mirror `spawn_background_file_watcher`: kick off the off-runtime
            // registration on a background task and register its handle so the
            // shutdown path exercises the real abort branch.
            let starter = server.clone();
            let start_handle = tokio::spawn(async move {
                let _ = starter
                    .start_file_watching_with_callback(NoopWatchCallback)
                    .await;
            });
            *server.file_watcher_task.lock().await = Some(start_handle);

            // Brief work, then shut down while registration may still be in
            // flight. Shutdown must not wait on the synchronous `.watch()`.
            tokio::time::sleep(INFLIGHT_WORK_PAUSE).await;
            server.stop_file_watching().await;
        });

        // The decisive measurement: dropping the runtime must not block on a
        // worker pinned inside `.watch()`. A `shutdown_timeout` bounds the drop
        // so a regression surfaces as a measurable stall rather than a hang.
        let start = std::time::Instant::now();
        runtime.shutdown_timeout(TEARDOWN_SHUTDOWN_TIMEOUT);
        let elapsed = start.elapsed();

        assert!(
            elapsed < MAX_SHUTDOWN_ELAPSED,
            "runtime teardown blocked for {elapsed:?}; the FSEvents `.watch()` \
             registration must run off the tokio runtime so an aborted startup \
             never pins a worker"
        );
    }
}
