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
                        tracing::info!("RecordingAgent: Captured ACP notification #{}", count);
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

    /// Record a method call with captured notifications
    fn record_with_notifications(&self, method: &str, req: &impl Serialize, resp: &impl Serialize) {
        // Don't take notifications here - they arrive asynchronously after method completes
        // Drop will associate all buffered notifications with the appropriate method
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

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&session)?;
        std::fs::write(&self.path, json)?;

        let absolute_path = std::fs::canonicalize(&self.path).unwrap_or_else(|_| self.path.clone());

        tracing::info!(
            "RecordingAgent: Saved {} calls to {} (absolute: {})",
            calls.len(),
            Pretty(&self.path),
            Pretty(&absolute_path)
        );
        Ok(())
    }
}

impl<A> Drop for RecordingAgent<A> {
    fn drop(&mut self) {
        // Give capture thread time to receive all async notifications
        // This ensures notifications sent during Agent method calls are fully captured
        std::thread::sleep(std::time::Duration::from_secs(2));

        let notification_count = self.notification_buffer.lock().unwrap().len();
        tracing::info!(
            "RecordingAgent Drop: {} buffered notifications to distribute",
            notification_count
        );

        // Associate all captured notifications with the prompt method call
        // (notifications are generated during prompt execution)
        if notification_count > 0 {
            let notifications = std::mem::take(&mut *self.notification_buffer.lock().unwrap());
            let mut calls = self.calls.lock().unwrap();

            // Find the last prompt call and add notifications to it
            if let Some(call) = calls.iter_mut().rev().find(|c| c.method == "prompt") {
                tracing::info!(
                    "Adding {} notifications to prompt call",
                    notifications.len()
                );
                call.notifications = notifications;
            } else {
                tracing::warn!(
                    "No prompt call found to attach {} notifications",
                    notifications.len()
                );
            }
        }

        if let Err(e) = self.save() {
            tracing::error!("Failed to save recording: {}", e);
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<A: Agent> Agent for RecordingAgent<A> {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        let response = self.inner.initialize(request.clone()).await?;
        self.record_with_notifications("initialize", &request, &response);
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
        self.record_with_notifications("new_session", &request, &response);
        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> agent_client_protocol::Result<PromptResponse> {
        let response = self.inner.prompt(request.clone()).await?;
        self.record_with_notifications("prompt", &request, &response);
        Ok(response)
    }

    async fn cancel(&self, request: CancelNotification) -> agent_client_protocol::Result<()> {
        self.inner.cancel(request.clone()).await?;
        self.record_with_notifications("cancel", &request, &());
        Ok(())
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        let response = self.inner.load_session(request.clone()).await?;
        self.record_with_notifications("load_session", &request, &response);
        Ok(response)
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        let response = self.inner.set_session_mode(request.clone()).await?;
        self.record_with_notifications("set_session_mode", &request, &response);
        Ok(response)
    }

    async fn ext_method(&self, request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        let result = self.inner.ext_method(request.clone()).await;
        match &result {
            Ok(response) => {
                self.record_with_notifications("ext_method", &request, response);
            }
            Err(e) => {
                // Record error responses too (for capability check tests that expect errors)
                self.record_with_notifications(
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
        self.record_with_notifications("ext_notification", &notification, &());
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
    fn test_record_with_notifications_stores_call() {
        let (mock, _) = MockAgent::new();
        let agent = RecordingAgent::new(mock, PathBuf::from("/tmp/test_rec_record.json"));

        agent.record_with_notifications(
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
        agent.record_with_notifications("initialize", &"req", &"resp");

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
}
