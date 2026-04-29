//! RecordingAgent - Simple proxy that records Agent method calls

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SetSessionModeRequest, SetSessionModeResponse,
};
use model_context_protocol_extras::McpNotification;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use swissarmyhammer_common::Pretty;

/// Recorded method call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedCall {
    pub method: String,
    pub request: serde_json::Value,
    pub response: serde_json::Value,
    /// Notifications sent during this method call
    #[serde(default)]
    pub notifications: Vec<serde_json::Value>,
}

/// Recorded session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    pub calls: Vec<RecordedCall>,
}

/// RecordingAgent wraps any Agent and records all method calls
pub struct RecordingAgent<A> {
    inner: A,
    path: PathBuf,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    /// Buffer for notifications received during current method call
    pub(crate) notification_buffer: Arc<Mutex<Vec<serde_json::Value>>>,
    /// Set of seen notification keys for deduplication
    seen_notifications: Arc<Mutex<HashSet<u64>>>,
    /// Task handle for notification capture (unused but kept for future)
    #[allow(dead_code)]
    notification_task: Option<tokio::task::JoinHandle<()>>,
}

impl<A> RecordingAgent<A> {
    pub fn new(inner: A, path: PathBuf) -> Self {
        tracing::info!("RecordingAgent: Will record to {}", Pretty(&path));
        Self {
            inner,
            path,
            calls: Arc::new(Mutex::new(Vec::new())),
            notification_buffer: Arc::new(Mutex::new(Vec::new())),
            seen_notifications: Arc::new(Mutex::new(HashSet::new())),
            notification_task: None,
        }
    }

    /// Create with notification receiver
    ///
    /// Spawns a task to consume notifications from receiver and buffer them.
    pub fn with_notifications(
        inner: A,
        path: PathBuf,
        mut receiver: tokio::sync::broadcast::Receiver<agent_client_protocol::SessionNotification>,
    ) -> Self {
        let agent = Self::new(inner, path);
        let buffer = Arc::clone(&agent.notification_buffer);

        // Spawn thread to capture notifications until channel closes
        std::thread::spawn(move || {
            tracing::info!("RecordingAgent: Starting ACP notification capture thread");
            let mut count = 0;

            // Continuously poll for notifications until channel closes
            // No timeout - we capture for the entire lifetime of the agent
            loop {
                match receiver.try_recv() {
                    Ok(notification) => {
                        count += 1;
                        tracing::trace!("RecordingAgent: Captured ACP notification #{}", count);
                        if let Ok(json) = serde_json::to_value(&notification) {
                            buffer.lock().unwrap().push(json);
                        }
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                        // No message yet, sleep briefly and continue
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            "Receiver lagged by {}, some notifications may be lost",
                            skipped
                        );
                        // Continue capturing even if we lagged
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                        tracing::info!("ACP notification channel closed, stopping capture");
                        break;
                    }
                }
            }

            tracing::info!(
                "RecordingAgent: ACP capture thread complete ({} notifications)",
                count
            );
        });

        agent
    }

    /// Add MCP notification source for capture
    ///
    /// Spawns a thread to consume MCP notifications and convert them to JSON.
    /// Deduplicates notifications using their dedup_key.
    pub fn add_mcp_source(&self, mut receiver: tokio::sync::broadcast::Receiver<McpNotification>) {
        let buffer = Arc::clone(&self.notification_buffer);
        let seen = Arc::clone(&self.seen_notifications);

        std::thread::spawn(move || {
            tracing::info!("RecordingAgent: Starting MCP notification capture thread");
            let mut count = 0;
            let mut deduped = 0;

            loop {
                match receiver.try_recv() {
                    Ok(notification) => {
                        let key = notification.dedup_key();

                        // Check for duplicate
                        let mut seen_set = seen.lock().unwrap();
                        if seen_set.contains(&key) {
                            deduped += 1;
                            tracing::debug!(
                                "RecordingAgent: Deduped MCP notification (key={})",
                                key
                            );
                            continue;
                        }
                        seen_set.insert(key);
                        drop(seen_set);

                        count += 1;
                        tracing::info!("RecordingAgent: Captured MCP notification #{}", count);

                        // Convert MCP notification to JSON
                        if let Ok(json) = serde_json::to_value(&notification) {
                            buffer.lock().unwrap().push(json);
                        }
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            "MCP receiver lagged by {}, some notifications may be lost",
                            skipped
                        );
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                        tracing::info!("MCP notification channel closed, stopping capture");
                        break;
                    }
                }
            }

            tracing::info!(
                "RecordingAgent: MCP capture thread complete ({} captured, {} deduped)",
                count,
                deduped
            );
        });
    }

    /// Get access to notification buffer for external capture
    pub fn notification_buffer(&self) -> Arc<Mutex<Vec<serde_json::Value>>> {
        Arc::clone(&self.notification_buffer)
    }

    /// Record a method call.
    ///
    /// Note: despite earlier naming, this method does NOT attach the captured
    /// notifications inline. Notifications arrive asynchronously after the
    /// method response future resolves; attaching them here would race and
    /// mis-bucket them onto the next call. Instead, `notifications` is left
    /// empty and the per-prompt `flush_now`/`Drop` path routes buffered
    /// notifications to the correct call by `sessionId`.
    fn record_call(&self, method: &str, req: &impl Serialize, resp: &impl Serialize) {
        // Don't take notifications here - they arrive asynchronously after method completes.
        // Drop time routes buffered notifications to the appropriate call by `sessionId`,
        // which avoids the race where the response future resolves before the notification
        // stream has finished draining (notifications belonging to call N would otherwise
        // be mis-attributed to call N+1's bucket).
        let call = RecordedCall {
            method: method.to_string(),
            request: serde_json::to_value(req).unwrap_or_default(),
            response: serde_json::to_value(resp).unwrap_or_default(),
            notifications: Vec::new(), // Filled in Drop
        };
        self.calls.lock().unwrap().push(call);
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let calls = self.calls.lock().unwrap();
        let session = RecordedSession {
            calls: calls.clone(),
        };

        // Parent-directory creation is handled by `atomic_write`, no need to
        // duplicate it here.
        let json = serde_json::to_string_pretty(&session)?;
        atomic_write(&self.path, json.as_bytes())?;

        let absolute_path = std::fs::canonicalize(&self.path).unwrap_or_else(|_| self.path.clone());

        tracing::info!(
            "RecordingAgent: Saved {} calls to {} (absolute: {})",
            calls.len(),
            Pretty(&self.path),
            Pretty(&absolute_path)
        );
        Ok(())
    }

    /// Drain currently-buffered notifications, route them to their owning prompt
    /// calls by `sessionId`, and atomically persist the recording to disk.
    ///
    /// This is the durability primitive used both by [`Drop`] and by per-call
    /// flushes invoked from inside [`Agent::prompt`]. Calling it mid-stream is
    /// safe: any notifications still in flight (not yet observed by the capture
    /// thread) simply remain in the buffer and are picked up by the next flush
    /// or by the final `Drop`. Errors are logged but never propagated — a
    /// failed flush must not break the wrapped `Agent` call.
    fn flush_now(&self) {
        // Snapshot whatever notifications have arrived since the last flush.
        // Notifications still racing in the capture thread will be picked up
        // by a later flush; we deliberately do NOT sleep here because this
        // runs in the hot path between prompt calls.
        let notifications = std::mem::take(&mut *self.notification_buffer.lock().unwrap());
        if !notifications.is_empty() {
            let mut calls = self.calls.lock().unwrap();
            distribute_notifications_by_session(&mut calls, notifications);
        }

        if let Err(e) = self.save() {
            tracing::error!("RecordingAgent: mid-stream flush failed: {}", e);
        }
    }
}

/// Atomically write `bytes` to `path` by writing to a sibling temp file and
/// renaming. This guarantees that a process kill mid-write cannot leave the
/// recording file half-written or corrupt; readers see either the previous
/// good contents or the new ones.
///
/// The temp file lives next to the destination so that `rename` stays on the
/// same filesystem (rename across filesystems would copy + unlink and lose the
/// atomicity guarantee).
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "recording path has no parent directory",
        )
    })?;
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "recording path has no file name",
        )
    })?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(file_name);
    tmp_name.push(".tmp");
    let tmp_path = parent.join(&tmp_name);

    // Write to the temp file. On any failure, remove the temp file so we don't
    // leave a `.recording.json.tmp` orphan on disk if the process exits before
    // the next successful flush would have overwritten it.
    let write_result = (|| -> std::io::Result<()> {
        let mut tmp = std::fs::File::create(&tmp_path)?;
        tmp.write_all(bytes)?;
        tmp.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    // NOTE: We deliberately do not fsync the parent directory after the rename.
    // POSIX-strict durability would open `parent` and `sync_all()` it so the new
    // directory entry survives a kernel crash. The actual failure mode for this
    // recorder is SIGKILL of the user-space process (Stop-hook timeout), not a
    // kernel panic — and the rename itself is atomic at the kernel level, so
    // SIGKILL between rename and a missing dir-fsync still leaves a readable
    // recording. The diagnostic use case does not justify the extra fsync.
    std::fs::rename(&tmp_path, path)
}

impl<A> Drop for RecordingAgent<A> {
    fn drop(&mut self) {
        // Give capture thread time to receive all async notifications still in
        // flight. Per-prompt flushes (see `flush_now` invoked from `prompt()`)
        // already persisted every prior prompt's request/response and any
        // notifications that had arrived by then; this final settle window
        // exists only to catch the tail of the *last* prompt's notification
        // stream, which has no subsequent flush to fall back on.
        //
        // Do not remove without also rethinking the per-prompt flush contract:
        // for any prompt N < last, durability is provided by `flush_now` at the
        // end of `Agent::prompt`; only prompt `last` relies on this sleep.
        std::thread::sleep(std::time::Duration::from_secs(2));

        let notification_count = self.notification_buffer.lock().unwrap().len();
        tracing::info!(
            "RecordingAgent Drop: {} buffered notifications to distribute",
            notification_count
        );

        self.flush_now();
    }
}

/// Extract the `sessionId` field from a JSON value, if present at the top level.
///
/// Returns `None` for notifications/calls that don't carry a session id (e.g. the
/// initialize call). The field is named `sessionId` because both ACP
/// `SessionNotification` and ACP requests serialize with that camelCase key.
fn extract_session_id(value: &serde_json::Value) -> Option<&str> {
    value.get("sessionId").and_then(|v| v.as_str())
}

/// Distribute buffered notifications to their matching prompt calls by `sessionId`.
///
/// Streaming notifications arrive on a separate channel from prompt responses. The
/// response future for prompt N can resolve while N's notifications are still
/// in flight, which means a naïve "append all buffered notifs to the last prompt"
/// strategy mis-buckets call N's tail onto call N+1 (and so on). Routing by
/// `sessionId` is reliable because each notification carries the id of the session
/// it belongs to.
///
/// Routing rules:
/// - For each notification with a `sessionId`, append it to the *last* prompt call
///   whose request has the same `sessionId`. The "last" choice ensures that if a
///   single session has multiple prompt calls (rare), trailing notifications go
///   to the most recent call rather than retroactively into an earlier bucket.
/// - Notifications without a `sessionId`, or whose session has no matching prompt
///   call, are appended to the last prompt call as a fallback so they are not
///   silently dropped.
/// - If there are no prompt calls at all, the notifications are logged and
///   discarded (there is nowhere to attach them in the recording schema).
fn distribute_notifications_by_session(
    calls: &mut [RecordedCall],
    notifications: Vec<serde_json::Value>,
) {
    if notifications.is_empty() {
        return;
    }

    // Build an index: sessionId -> index of the *last* prompt call with that session.
    let mut last_prompt_for_session: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut last_prompt_idx: Option<usize> = None;
    for (idx, call) in calls.iter().enumerate() {
        if call.method != "prompt" {
            continue;
        }
        last_prompt_idx = Some(idx);
        if let Some(sid) = extract_session_id(&call.request) {
            last_prompt_for_session.insert(sid.to_string(), idx);
        }
    }

    let Some(fallback_idx) = last_prompt_idx else {
        tracing::warn!(
            "No prompt call found to attach {} notifications",
            notifications.len()
        );
        return;
    };

    // Tally for logging.
    let mut routed_by_session = 0usize;
    let mut routed_to_fallback = 0usize;

    for notification in notifications {
        let target_idx = extract_session_id(&notification)
            .and_then(|sid| last_prompt_for_session.get(sid).copied())
            .inspect(|_| {
                routed_by_session += 1;
            })
            .unwrap_or_else(|| {
                routed_to_fallback += 1;
                fallback_idx
            });
        calls[target_idx].notifications.push(notification);
    }

    tracing::info!(
        "Distributed notifications: {} routed by sessionId, {} routed to fallback (last prompt)",
        routed_by_session,
        routed_to_fallback
    );
}

#[async_trait::async_trait(?Send)]
impl<A: Agent> Agent for RecordingAgent<A> {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        let response = self.inner.initialize(request.clone()).await?;
        self.record_call("initialize", &request, &response);
        Ok(response)
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        self.inner.authenticate(request).await
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        let response = self.inner.new_session(request.clone()).await?;
        self.record_call("new_session", &request, &response);
        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> agent_client_protocol::Result<PromptResponse> {
        let response = self.inner.prompt(request.clone()).await?;
        self.record_call("prompt", &request, &response);
        // Persist after every prompt so the recording is durable across
        // mid-flight termination. If the *next* prompt deadlocks and the
        // process is killed, the on-disk file already contains every prior
        // prompt's request, response, and any notifications that landed in
        // the buffer before this flush. See task 01KQAFT5H1CYQ8YDNAM4J0HD1Q.
        self.flush_now();
        Ok(response)
    }

    async fn cancel(&self, request: CancelNotification) -> agent_client_protocol::Result<()> {
        self.inner.cancel(request.clone()).await?;
        self.record_call("cancel", &request, &());
        Ok(())
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        let response = self.inner.load_session(request.clone()).await?;
        self.record_call("load_session", &request, &response);
        Ok(response)
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        let response = self.inner.set_session_mode(request.clone()).await?;
        self.record_call("set_session_mode", &request, &response);
        Ok(response)
    }

    async fn ext_method(&self, request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        let result = self.inner.ext_method(request.clone()).await;
        match &result {
            Ok(response) => {
                self.record_call("ext_method", &request, response);
            }
            Err(e) => {
                // Record error responses too (for capability check tests that expect errors)
                self.record_call(
                    "ext_method",
                    &request,
                    &serde_json::json!({
                        "error": {
                            "code": e.code,
                            "message": &e.message,
                            "data": &e.data
                        }
                    }),
                );
            }
        }
        result
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> agent_client_protocol::Result<()> {
        self.inner.ext_notification(notification.clone()).await?;
        self.record_call("ext_notification", &notification, &());
        Ok(())
    }
}

impl<A: Agent + crate::AgentWithFixture> crate::AgentWithFixture for RecordingAgent<A> {
    fn agent_type(&self) -> &'static str {
        self.inner.agent_type()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock,
        ExtNotification, ExtRequest, ExtResponse, InitializeRequest, InitializeResponse,
        LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
        PromptRequest, PromptResponse, SessionId, SetSessionModeRequest, SetSessionModeResponse,
        StopReason, TextContent,
    };
    use serde_json::value::RawValue;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // -- Mock agent for recording tests --

    struct MockAgent {
        prompt_called: Arc<AtomicBool>,
    }

    impl MockAgent {
        fn new() -> (Self, Arc<AtomicBool>) {
            let called = Arc::new(AtomicBool::new(false));
            (
                Self {
                    prompt_called: called.clone(),
                },
                called,
            )
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Agent for MockAgent {
        async fn initialize(
            &self,
            _request: InitializeRequest,
        ) -> agent_client_protocol::Result<InitializeResponse> {
            Ok(InitializeResponse::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
        }

        async fn authenticate(
            &self,
            _request: AuthenticateRequest,
        ) -> agent_client_protocol::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::new())
        }

        async fn new_session(
            &self,
            _request: NewSessionRequest,
        ) -> agent_client_protocol::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new("test-session"))
        }

        async fn prompt(
            &self,
            _request: PromptRequest,
        ) -> agent_client_protocol::Result<PromptResponse> {
            self.prompt_called.store(true, Ordering::SeqCst);
            Ok(PromptResponse::new(StopReason::EndTurn))
        }

        async fn cancel(&self, _request: CancelNotification) -> agent_client_protocol::Result<()> {
            Ok(())
        }

        async fn load_session(
            &self,
            _request: LoadSessionRequest,
        ) -> agent_client_protocol::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }

        async fn set_session_mode(
            &self,
            _request: SetSessionModeRequest,
        ) -> agent_client_protocol::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }

        async fn ext_method(
            &self,
            _request: ExtRequest,
        ) -> agent_client_protocol::Result<ExtResponse> {
            Err(agent_client_protocol::Error::method_not_found())
        }

        async fn ext_notification(
            &self,
            _notification: ExtNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    fn make_prompt_request() -> PromptRequest {
        PromptRequest::new(
            SessionId::from("test-session"),
            vec![ContentBlock::Text(TextContent::new("hello"))],
        )
    }

    fn make_ext_request() -> ExtRequest {
        let raw = RawValue::from_string("{}".to_string()).unwrap();
        ExtRequest::new("custom/method", Arc::from(raw))
    }

    fn make_ext_notification() -> ExtNotification {
        let raw = RawValue::from_string("{}".to_string()).unwrap();
        ExtNotification::new("custom/notify", Arc::from(raw))
    }

    fn make_mcp_progress_notification(
        token: &str,
    ) -> model_context_protocol_extras::McpNotification {
        use rmcp::model::{NumberOrString, ProgressNotificationParam, ProgressToken};
        model_context_protocol_extras::McpNotification::Progress(ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String(token.to_string().into())),
            progress: 50.0,
            total: Some(100.0),
            message: Some("halfway".into()),
        })
    }

    #[test]
    fn test_recorded_call_serialization_roundtrip() {
        let call = RecordedCall {
            method: "prompt".to_string(),
            request: serde_json::json!({"prompt": "hello"}),
            response: serde_json::json!({"stop_reason": "EndTurn"}),
            notifications: vec![serde_json::json!({"type": "chunk"})],
        };

        let json = serde_json::to_string(&call).unwrap();
        let deserialized: RecordedCall = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.method, "prompt");
        assert_eq!(deserialized.notifications.len(), 1);
    }

    #[test]
    fn test_recorded_call_default_notifications() {
        let json = r#"{"method":"init","request":{},"response":{}}"#;
        let call: RecordedCall = serde_json::from_str(json).unwrap();
        assert!(call.notifications.is_empty());
    }

    #[test]
    fn test_recorded_session_serialization_roundtrip() {
        let session = RecordedSession {
            calls: vec![RecordedCall {
                method: "initialize".to_string(),
                request: serde_json::json!({}),
                response: serde_json::json!({}),
                notifications: vec![],
            }],
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: RecordedSession = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.calls.len(), 1);
        assert_eq!(deserialized.calls[0].method, "initialize");
    }

    #[test]
    fn test_recording_agent_new_creates_empty_state() {
        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, PathBuf::from("/tmp/test_rec_new.json"));

        let buffer = agent.notification_buffer();
        assert!(buffer.lock().unwrap().is_empty());
        {
            let calls = agent.calls.lock().unwrap();
            assert!(calls.is_empty());
        }

        std::mem::forget(agent);
    }

    #[test]
    fn test_notification_buffer_returns_shared_ref() {
        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, PathBuf::from("/tmp/test_rec_buf.json"));

        let buf1 = agent.notification_buffer();
        let buf2 = agent.notification_buffer();

        buf1.lock().unwrap().push(serde_json::json!("test"));
        assert_eq!(buf2.lock().unwrap().len(), 1);

        std::mem::forget(agent);
    }

    #[test]
    fn test_record_call_stores_call() {
        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, PathBuf::from("/tmp/test_rec_record.json"));

        agent.record_call(
            "initialize",
            &serde_json::json!({"protocol_version": "2024-11-05"}),
            &serde_json::json!({"agent_info": null}),
        );

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "initialize");
            assert!(calls[0].notifications.is_empty());
        }

        std::mem::forget(agent);
    }

    #[test]
    fn test_save_creates_file_and_writes_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path.clone());
        agent.record_call("initialize", &"req", &"resp");

        agent.save().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let session: RecordedSession = serde_json::from_str(&content).unwrap();
        assert_eq!(session.calls.len(), 1);
        assert_eq!(session.calls[0].method, "initialize");

        std::mem::forget(agent);
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deep/recording.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path.clone());
        agent.save().unwrap();

        assert!(path.exists());

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_initialize_delegates_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let _response = agent
            .initialize(InitializeRequest::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
            .await
            .unwrap();

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "initialize");
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_authenticate_delegates_without_recording() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let _response = agent
            .authenticate(AuthenticateRequest::new("test-method"))
            .await
            .unwrap();

        {
            let calls = agent.calls.lock().unwrap();
            assert!(calls.is_empty());
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_new_session_delegates_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let response = agent
            .new_session(NewSessionRequest::new("/tmp"))
            .await
            .unwrap();

        assert_eq!(response.session_id.to_string(), "test-session");

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "new_session");
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_prompt_delegates_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, called) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let response = agent.prompt(make_prompt_request()).await.unwrap();

        assert!(called.load(Ordering::SeqCst));
        assert_eq!(response.stop_reason, StopReason::EndTurn);

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "prompt");
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_cancel_delegates_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        agent
            .cancel(CancelNotification::new("test-session"))
            .await
            .unwrap();

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "cancel");
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_load_session_delegates_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let _response = agent
            .load_session(LoadSessionRequest::new("test-session", "/tmp"))
            .await
            .unwrap();

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "load_session");
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_set_session_mode_delegates_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let _response = agent
            .set_session_mode(SetSessionModeRequest::new("test-session", "plan"))
            .await
            .unwrap();

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "set_session_mode");
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_ext_method_records_error_response() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let result = agent.ext_method(make_ext_request()).await;
        assert!(result.is_err());

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "ext_method");
            assert!(calls[0].response.get("error").is_some());
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_recording_agent_ext_notification_delegates_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        agent
            .ext_notification(make_ext_notification())
            .await
            .unwrap();

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "ext_notification");
        }

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_with_notifications_captures_acp_notifications() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (tx, rx) = tokio::sync::broadcast::channel(16);
        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::with_notifications(mock, path, rx);

        let notification = agent_client_protocol::SessionNotification::new(
            SessionId::from("s1"),
            agent_client_protocol::SessionUpdate::AgentMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::Text(TextContent::new(
                    "hello",
                ))),
            ),
        );
        tx.send(notification).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        {
            let buffer = agent.notification_buffer.lock().unwrap();
            assert_eq!(buffer.len(), 1);
        }

        drop(tx);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_add_mcp_source_captures_and_deduplicates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let (tx, rx) =
            tokio::sync::broadcast::channel::<model_context_protocol_extras::McpNotification>(16);
        agent.add_mcp_source(rx);

        let notif = make_mcp_progress_notification("t1");
        tx.send(notif.clone()).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        {
            let buffer = agent.notification_buffer.lock().unwrap();
            assert_eq!(buffer.len(), 1);
        }

        // Send duplicate — should be deduped
        tx.send(notif).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        {
            let buffer = agent.notification_buffer.lock().unwrap();
            assert_eq!(buffer.len(), 1, "Duplicate notification should be deduped");
        }

        drop(tx);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_with_notifications_handles_lagged_receiver() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (tx, rx) = tokio::sync::broadcast::channel(1);
        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::with_notifications(mock, path, rx);

        for i in 0..5 {
            let notification = agent_client_protocol::SessionNotification::new(
                SessionId::from("s1"),
                agent_client_protocol::SessionUpdate::AgentMessageChunk(
                    agent_client_protocol::ContentChunk::new(ContentBlock::Text(TextContent::new(
                        format!("msg {}", i),
                    ))),
                ),
            );
            let _ = tx.send(notification);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        drop(tx);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_add_mcp_source_handles_lagged_receiver() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, path);

        let (tx, rx) =
            tokio::sync::broadcast::channel::<model_context_protocol_extras::McpNotification>(1);
        agent.add_mcp_source(rx);

        for i in 0..5 {
            let notif = make_mcp_progress_notification(&format!("t{}", i));
            let _ = tx.send(notif);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        drop(tx);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        std::mem::forget(agent);
    }

    #[tokio::test]
    async fn test_ext_method_records_success_response() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        struct SuccessExtAgent;

        #[async_trait::async_trait(?Send)]
        impl Agent for SuccessExtAgent {
            async fn initialize(
                &self,
                _r: InitializeRequest,
            ) -> agent_client_protocol::Result<InitializeResponse> {
                Ok(InitializeResponse::new(
                    agent_client_protocol::ProtocolVersion::LATEST,
                ))
            }
            async fn authenticate(
                &self,
                _r: AuthenticateRequest,
            ) -> agent_client_protocol::Result<AuthenticateResponse> {
                Ok(AuthenticateResponse::new())
            }
            async fn new_session(
                &self,
                _r: NewSessionRequest,
            ) -> agent_client_protocol::Result<NewSessionResponse> {
                Ok(NewSessionResponse::new("s1"))
            }
            async fn prompt(
                &self,
                _r: PromptRequest,
            ) -> agent_client_protocol::Result<PromptResponse> {
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
            async fn cancel(&self, _r: CancelNotification) -> agent_client_protocol::Result<()> {
                Ok(())
            }
            async fn load_session(
                &self,
                _r: LoadSessionRequest,
            ) -> agent_client_protocol::Result<LoadSessionResponse> {
                Ok(LoadSessionResponse::new())
            }
            async fn set_session_mode(
                &self,
                _r: SetSessionModeRequest,
            ) -> agent_client_protocol::Result<SetSessionModeResponse> {
                Ok(SetSessionModeResponse::new())
            }
            async fn ext_method(
                &self,
                _r: ExtRequest,
            ) -> agent_client_protocol::Result<ExtResponse> {
                let raw = RawValue::from_string(r#"{"status":"ok"}"#.to_string()).unwrap();
                Ok(ExtResponse::new(Arc::from(raw)))
            }
            async fn ext_notification(
                &self,
                _n: ExtNotification,
            ) -> agent_client_protocol::Result<()> {
                Ok(())
            }
        }

        let agent = RecordingAgent::new(SuccessExtAgent, path);

        let result = agent.ext_method(make_ext_request()).await;
        assert!(result.is_ok());

        {
            let calls = agent.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, "ext_method");
        }

        std::mem::forget(agent);
    }

    // -- Tests for sessionId-based notification routing --

    /// Build a `prompt` RecordedCall with a given session id, used by routing tests.
    fn prompt_call_for(session_id: &str) -> RecordedCall {
        RecordedCall {
            method: "prompt".to_string(),
            request: serde_json::json!({ "sessionId": session_id }),
            response: serde_json::json!({ "stopReason": "end_turn" }),
            notifications: Vec::new(),
        }
    }

    /// Build a notification JSON object tagged with the given session id and a marker
    /// payload so each notification can be uniquely identified in assertions.
    fn notification_for(session_id: &str, marker: &str) -> serde_json::Value {
        serde_json::json!({
            "sessionId": session_id,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": { "type": "text", "text": marker }
            }
        })
    }

    fn marker_of(notification: &serde_json::Value) -> &str {
        notification
            .pointer("/update/content/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    #[test]
    fn distribute_routes_notifications_by_session_id() {
        // Two prompt calls, each on its own session — exactly the bug shape:
        //   CALL2: sessionId=A (rule 1)
        //   CALL4: sessionId=B (rule 2)
        // Notifications for A arrive AFTER A's response future already resolved
        // (they're in the buffer when the recorder finally drains). The fix must
        // route them to A by sessionId, not append them to the last call (B).
        let mut calls = vec![
            RecordedCall {
                method: "initialize".to_string(),
                request: serde_json::json!({}),
                response: serde_json::json!({}),
                notifications: Vec::new(),
            },
            prompt_call_for("session-A"),
            RecordedCall {
                method: "new_session".to_string(),
                request: serde_json::json!({}),
                response: serde_json::json!({ "sessionId": "session-B" }),
                notifications: Vec::new(),
            },
            prompt_call_for("session-B"),
        ];

        // Interleaved arrival order — A's notifications arrive late, even after B's.
        let buffered = vec![
            notification_for("session-B", "B-1"),
            notification_for("session-A", "A-1"),
            notification_for("session-B", "B-2"),
            notification_for("session-A", "A-2"),
            notification_for("session-A", "A-3"),
        ];

        distribute_notifications_by_session(&mut calls, buffered);

        // Call index 1 is the prompt for session-A, index 3 is the prompt for session-B.
        let a_markers: Vec<&str> = calls[1].notifications.iter().map(marker_of).collect();
        let b_markers: Vec<&str> = calls[3].notifications.iter().map(marker_of).collect();

        assert_eq!(
            a_markers,
            vec!["A-1", "A-2", "A-3"],
            "session-A notifications must land in session-A's prompt call only, in arrival order"
        );
        assert_eq!(
            b_markers,
            vec!["B-1", "B-2"],
            "session-B notifications must land in session-B's prompt call only, in arrival order"
        );

        // Non-prompt calls must remain untouched.
        assert!(calls[0].notifications.is_empty());
        assert!(calls[2].notifications.is_empty());
    }

    #[test]
    fn distribute_falls_back_to_last_prompt_when_session_unknown() {
        // A notification whose sessionId matches no prompt call falls back to the
        // most recent prompt call, so it isn't silently dropped.
        let mut calls = vec![prompt_call_for("session-A"), prompt_call_for("session-B")];

        let buffered = vec![
            notification_for("session-A", "A-1"),
            notification_for("session-unknown", "stray"),
            notification_for("session-B", "B-1"),
        ];

        distribute_notifications_by_session(&mut calls, buffered);

        assert_eq!(
            calls[0].notifications.len(),
            1,
            "A's bucket has only its own"
        );
        assert_eq!(marker_of(&calls[0].notifications[0]), "A-1");

        assert_eq!(
            calls[1].notifications.len(),
            2,
            "B's bucket gets its own + the stray fallback"
        );
        let b_markers: Vec<&str> = calls[1].notifications.iter().map(marker_of).collect();
        assert_eq!(b_markers, vec!["stray", "B-1"]);
    }

    #[test]
    fn distribute_routes_repeated_session_to_last_prompt_for_that_session() {
        // If one session has multiple prompts, all routed-by-session notifications
        // land in the *last* prompt for that session. This is the least-bad choice
        // when temporal info isn't available, and matches the documented behaviour.
        let mut calls = vec![
            prompt_call_for("session-A"),
            prompt_call_for("session-B"),
            prompt_call_for("session-A"), // second prompt on session-A
        ];

        let buffered = vec![
            notification_for("session-A", "A-1"),
            notification_for("session-A", "A-2"),
            notification_for("session-B", "B-1"),
        ];

        distribute_notifications_by_session(&mut calls, buffered);

        assert!(
            calls[0].notifications.is_empty(),
            "first prompt for session-A receives nothing — last-wins routing"
        );
        let last_a: Vec<&str> = calls[2].notifications.iter().map(marker_of).collect();
        assert_eq!(last_a, vec!["A-1", "A-2"]);

        let b: Vec<&str> = calls[1].notifications.iter().map(marker_of).collect();
        assert_eq!(b, vec!["B-1"]);
    }

    #[test]
    fn distribute_handles_empty_inputs() {
        let mut calls = vec![prompt_call_for("session-A")];
        distribute_notifications_by_session(&mut calls, Vec::new());
        assert!(calls[0].notifications.is_empty());

        // No prompt calls at all — must not panic; notifications are dropped.
        let mut empty: Vec<RecordedCall> = vec![RecordedCall {
            method: "initialize".to_string(),
            request: serde_json::json!({}),
            response: serde_json::json!({}),
            notifications: Vec::new(),
        }];
        distribute_notifications_by_session(
            &mut empty,
            vec![notification_for("session-A", "orphan")],
        );
        assert!(empty[0].notifications.is_empty());
    }

    #[test]
    fn distribute_is_resilient_to_notifications_without_session_id() {
        // Some captured items may not have a top-level sessionId (e.g. MCP notifs
        // converted to JSON). They must fall back to the last prompt call.
        let mut calls = vec![prompt_call_for("session-A"), prompt_call_for("session-B")];

        let no_sid = serde_json::json!({ "kind": "mcp_progress", "value": 42 });
        let buffered = vec![no_sid.clone(), notification_for("session-A", "A-1")];

        distribute_notifications_by_session(&mut calls, buffered);

        assert_eq!(calls[0].notifications.len(), 1);
        assert_eq!(marker_of(&calls[0].notifications[0]), "A-1");
        assert_eq!(calls[1].notifications.len(), 1);
        assert_eq!(calls[1].notifications[0], no_sid);
    }

    /// End-to-end regression: simulate the bug scenario through a real `RecordingAgent`
    /// with overlapping prompt calls. Notifications for prompt 1 are pushed into the
    /// buffer AFTER prompt 2 has already returned — exactly the off-by-one race the
    /// task describes. After Drop, each call's bucket must contain only its own
    /// session's notifications.
    #[tokio::test]
    async fn overlapping_prompt_streams_bucket_correctly_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overlap.json");

        struct DualSessionAgent;

        #[async_trait::async_trait(?Send)]
        impl Agent for DualSessionAgent {
            async fn initialize(
                &self,
                _r: InitializeRequest,
            ) -> agent_client_protocol::Result<InitializeResponse> {
                Ok(InitializeResponse::new(
                    agent_client_protocol::ProtocolVersion::LATEST,
                ))
            }
            async fn authenticate(
                &self,
                _r: AuthenticateRequest,
            ) -> agent_client_protocol::Result<AuthenticateResponse> {
                Ok(AuthenticateResponse::new())
            }
            async fn new_session(
                &self,
                _r: NewSessionRequest,
            ) -> agent_client_protocol::Result<NewSessionResponse> {
                Ok(NewSessionResponse::new("ignored"))
            }
            async fn prompt(
                &self,
                _r: PromptRequest,
            ) -> agent_client_protocol::Result<PromptResponse> {
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
            async fn cancel(&self, _r: CancelNotification) -> agent_client_protocol::Result<()> {
                Ok(())
            }
            async fn load_session(
                &self,
                _r: LoadSessionRequest,
            ) -> agent_client_protocol::Result<LoadSessionResponse> {
                Ok(LoadSessionResponse::new())
            }
            async fn set_session_mode(
                &self,
                _r: SetSessionModeRequest,
            ) -> agent_client_protocol::Result<SetSessionModeResponse> {
                Ok(SetSessionModeResponse::new())
            }
            async fn ext_method(
                &self,
                _r: ExtRequest,
            ) -> agent_client_protocol::Result<ExtResponse> {
                Err(agent_client_protocol::Error::method_not_found())
            }
            async fn ext_notification(
                &self,
                _n: ExtNotification,
            ) -> agent_client_protocol::Result<()> {
                Ok(())
            }
        }

        let agent = RecordingAgent::new(DualSessionAgent, path.clone());

        // Two prompt calls on different sessions, just like the bug recording.
        let prompt_a = PromptRequest::new(
            SessionId::from("session-A"),
            vec![ContentBlock::Text(TextContent::new("rule 1 input"))],
        );
        let prompt_b = PromptRequest::new(
            SessionId::from("session-B"),
            vec![ContentBlock::Text(TextContent::new("rule 2 input"))],
        );

        let _ = agent.prompt(prompt_a).await.unwrap();
        let _ = agent.prompt(prompt_b).await.unwrap();

        // Now simulate the race: notifications for BOTH sessions arrive AFTER both
        // prompt response futures already resolved. They are interleaved in
        // arrival order, mirroring how token-by-token chunks would stream in.
        {
            let mut buf = agent.notification_buffer.lock().unwrap();
            buf.push(notification_for("session-A", "rule-1-tok-1"));
            buf.push(notification_for("session-B", "rule-2-tok-1"));
            buf.push(notification_for("session-A", "rule-1-tok-2"));
            buf.push(notification_for("session-B", "rule-2-tok-2"));
            buf.push(notification_for("session-A", "rule-1-tok-3"));
        }

        // Dropping triggers distribution + save. We'd rather not pay the 2-second
        // settle sleep, but the assertions read from the saved file so we have to.
        drop(agent);

        let json = std::fs::read_to_string(&path).unwrap();
        let session: RecordedSession = serde_json::from_str(&json).unwrap();

        let prompt_calls: Vec<&RecordedCall> = session
            .calls
            .iter()
            .filter(|c| c.method == "prompt")
            .collect();
        assert_eq!(prompt_calls.len(), 2);

        let call_a = prompt_calls[0];
        let call_b = prompt_calls[1];
        assert_eq!(extract_session_id(&call_a.request), Some("session-A"));
        assert_eq!(extract_session_id(&call_b.request), Some("session-B"));

        let a_markers: Vec<&str> = call_a.notifications.iter().map(marker_of).collect();
        let b_markers: Vec<&str> = call_b.notifications.iter().map(marker_of).collect();

        assert_eq!(
            a_markers,
            vec!["rule-1-tok-1", "rule-1-tok-2", "rule-1-tok-3"],
            "session-A's prompt call must contain only session-A's notifications, in arrival order"
        );
        assert_eq!(
            b_markers,
            vec!["rule-2-tok-1", "rule-2-tok-2"],
            "session-B's prompt call must contain only session-B's notifications, in arrival order"
        );
    }

    /// Minimal Agent that returns trivial responses; used by durability tests
    /// where we only care about the *recording* behaviour, not the response
    /// contents.
    struct TrivialAgent;

    #[async_trait::async_trait(?Send)]
    impl Agent for TrivialAgent {
        async fn initialize(
            &self,
            _r: InitializeRequest,
        ) -> agent_client_protocol::Result<InitializeResponse> {
            Ok(InitializeResponse::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
        }
        async fn authenticate(
            &self,
            _r: AuthenticateRequest,
        ) -> agent_client_protocol::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::new())
        }
        async fn new_session(
            &self,
            _r: NewSessionRequest,
        ) -> agent_client_protocol::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new("ignored"))
        }
        async fn prompt(&self, _r: PromptRequest) -> agent_client_protocol::Result<PromptResponse> {
            Ok(PromptResponse::new(StopReason::EndTurn))
        }
        async fn cancel(&self, _r: CancelNotification) -> agent_client_protocol::Result<()> {
            Ok(())
        }
        async fn load_session(
            &self,
            _r: LoadSessionRequest,
        ) -> agent_client_protocol::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }
        async fn set_session_mode(
            &self,
            _r: SetSessionModeRequest,
        ) -> agent_client_protocol::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }
        async fn ext_method(&self, _r: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
            Err(agent_client_protocol::Error::method_not_found())
        }
        async fn ext_notification(&self, _n: ExtNotification) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    /// Mid-flight termination simulation: drive several prompt calls, then
    /// `mem::forget` the agent so [`Drop`] never runs. The file on disk must
    /// still be a valid recording containing every prompt completed before the
    /// "kill". This is the regression test for task 01KQAFT5H1CYQ8YDNAM4J0HD1Q.
    #[tokio::test]
    async fn mid_flight_termination_preserves_completed_prompts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("midflight.json");

        let agent = RecordingAgent::new(TrivialAgent, path.clone());

        // Simulate 3 of 11 rules completing before the parent process is killed.
        for i in 0..3 {
            let req = PromptRequest::new(
                SessionId::from(format!("rule-{}", i)),
                vec![ContentBlock::Text(TextContent::new(format!(
                    "rule {} input",
                    i
                )))],
            );
            let _ = agent.prompt(req).await.unwrap();
        }

        // Abnormal termination: Drop never runs, so durability must come from
        // the per-prompt flush invoked inside `prompt()`.
        std::mem::forget(agent);

        let json = std::fs::read_to_string(&path).expect("recording file must exist on disk");
        let session: RecordedSession =
            serde_json::from_str(&json).expect("recording must be valid JSON in the legacy schema");

        let prompt_calls: Vec<&RecordedCall> = session
            .calls
            .iter()
            .filter(|c| c.method == "prompt")
            .collect();
        assert_eq!(
            prompt_calls.len(),
            3,
            "all 3 completed prompts must be durable on disk after mem::forget"
        );

        for (i, call) in prompt_calls.iter().enumerate() {
            assert_eq!(
                extract_session_id(&call.request),
                Some(format!("rule-{}", i).as_str())
            );
        }
    }

    /// The on-disk schema must remain `{"calls": [...]}` so existing fixtures
    /// keep loading. This regression test reads back what the per-prompt flush
    /// wrote and asserts the top-level shape, not just successful deserialization.
    #[tokio::test]
    async fn on_disk_schema_is_legacy_calls_object() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schema.json");

        let agent = RecordingAgent::new(TrivialAgent, path.clone());
        let req = PromptRequest::new(
            SessionId::from("only-session"),
            vec![ContentBlock::Text(TextContent::new("hi"))],
        );
        let _ = agent.prompt(req).await.unwrap();
        std::mem::forget(agent);

        let json = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            value.get("calls").is_some(),
            "on-disk recording must have a top-level `calls` array"
        );
        assert!(value.get("calls").unwrap().is_array());
    }

    /// Each prompt-call flush should overwrite the file in place, not append.
    /// We assert a strictly monotonic call count over successive flushes.
    #[tokio::test]
    async fn per_prompt_flush_replaces_file_contents_each_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("monotonic.json");

        let agent = RecordingAgent::new(TrivialAgent, path.clone());

        for i in 0..4 {
            let req = PromptRequest::new(
                SessionId::from(format!("rule-{}", i)),
                vec![ContentBlock::Text(TextContent::new(format!("input-{}", i)))],
            );
            let _ = agent.prompt(req).await.unwrap();

            // Read after every flush — must always parse and contain exactly i+1
            // prompt calls.
            let json = std::fs::read_to_string(&path).unwrap();
            let session: RecordedSession = serde_json::from_str(&json).unwrap();
            let prompt_count = session
                .calls
                .iter()
                .filter(|c| c.method == "prompt")
                .count();
            assert_eq!(
                prompt_count,
                i + 1,
                "after {} prompts, on-disk file must contain {} prompt calls",
                i + 1,
                i + 1
            );
        }

        std::mem::forget(agent);
    }

    /// `atomic_write` must produce the destination file with the exact bytes
    /// requested, and must not leave a stray temp file behind on success.
    #[test]
    fn atomic_write_lands_full_contents_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.json");

        atomic_write(&path, br#"{"calls":[]}"#).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), br#"{"calls":[]}"#);

        // Temp file must have been renamed away, not left behind.
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries.len(), 1, "only the destination file should remain");
    }
}
