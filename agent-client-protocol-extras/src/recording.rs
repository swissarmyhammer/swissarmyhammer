//! RecordingAgent - Simple proxy that records Agent method calls

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SetSessionModeRequest, SetSessionModeResponse,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    /// Task handle for notification capture
    notification_task: Option<tokio::task::JoinHandle<()>>,
}

impl<A> RecordingAgent<A> {
    pub fn new(inner: A, path: PathBuf) -> Self {
        tracing::info!("RecordingAgent: Will record to {:?}", path);
        Self {
            inner,
            path,
            calls: Arc::new(Mutex::new(Vec::new())),
            notification_buffer: Arc::new(Mutex::new(Vec::new())),
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
            tracing::info!("RecordingAgent: Starting notification capture thread");
            let mut count = 0;

            // Continuously poll for notifications until channel closes
            // No timeout - we capture for the entire lifetime of the agent
            loop {
                match receiver.try_recv() {
                    Ok(notification) => {
                        count += 1;
                        tracing::info!("RecordingAgent: Captured notification #{}", count);
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
                        tracing::info!("Notification channel closed, stopping capture");
                        break;
                    }
                }
            }

            tracing::info!(
                "RecordingAgent: Capture thread complete ({} notifications)",
                count
            );
        });

        agent
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

    fn record(&self, method: &str, req: &impl Serialize, resp: &impl Serialize) {
        let call = RecordedCall {
            method: method.to_string(),
            request: serde_json::to_value(req).unwrap_or_default(),
            response: serde_json::to_value(resp).unwrap_or_default(),
            notifications: Vec::new(), // TODO: Capture notifications
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
            "RecordingAgent: Saved {} calls to {:?} (absolute: {:?})",
            calls.len(),
            self.path,
            absolute_path
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
        let response = self.inner.ext_method(request.clone()).await?;
        self.record_with_notifications("ext_method", &request, &response);
        Ok(response)
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
