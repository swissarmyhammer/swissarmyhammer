//! RecordingAgent - Simple proxy that records Agent method calls

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SetSessionModeRequest, SetSessionModeResponse,
};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Recorded method call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedCall {
    pub method: String,
    pub request: serde_json::Value,
    pub response: serde_json::Value,
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
}

impl<A: Agent> RecordingAgent<A> {
    pub fn new(inner: A, path: PathBuf) -> Self {
        tracing::info!("RecordingAgent: Will record to {:?}", path);
        Self {
            inner,
            path,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record(&self, method: &str, req: &impl Serialize, resp: &impl Serialize) {
        let call = RecordedCall {
            method: method.to_string(),
            request: serde_json::to_value(req).unwrap_or_default(),
            response: serde_json::to_value(resp).unwrap_or_default(),
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

        tracing::info!("RecordingAgent: Saved {} calls to {:?}", calls.len(), self.path);
        Ok(())
    }
}

impl<A> Drop for RecordingAgent<A> {
    fn drop(&mut self) {
        if let Err(e) = self.save() {
            tracing::error!("Failed to save recording: {}", e);
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<A: Agent> Agent for RecordingAgent<A> {
    async fn initialize(&self, request: InitializeRequest) -> agent_client_protocol::Result<InitializeResponse> {
        // Just forward - recording will be added later
        self.inner.initialize(request).await
    }

    async fn authenticate(&self, request: AuthenticateRequest) -> agent_client_protocol::Result<AuthenticateResponse> {
        self.inner.authenticate(request).await
    }

    async fn new_session(&self, request: NewSessionRequest) -> agent_client_protocol::Result<NewSessionResponse> {
        self.inner.new_session(request).await
    }

    async fn prompt(&self, request: PromptRequest) -> agent_client_protocol::Result<PromptResponse> {
        self.inner.prompt(request).await
    }

    async fn cancel(&self, request: CancelNotification) -> agent_client_protocol::Result<()> {
        self.inner.cancel(request).await
    }

    async fn load_session(&self, request: LoadSessionRequest) -> agent_client_protocol::Result<LoadSessionResponse> {
        self.inner.load_session(request).await
    }

    async fn set_session_mode(&self, request: SetSessionModeRequest) -> agent_client_protocol::Result<SetSessionModeResponse> {
        self.inner.set_session_mode(request).await
    }

    async fn ext_method(&self, request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        self.inner.ext_method(request).await
    }

    async fn ext_notification(&self, notification: ExtNotification) -> agent_client_protocol::Result<()> {
        self.inner.ext_notification(notification).await
    }
}

impl<A: Agent + crate::AgentWithFixture> crate::AgentWithFixture for RecordingAgent<A> {
    fn agent_type(&self) -> &'static str {
        self.inner.agent_type()
    }
}
