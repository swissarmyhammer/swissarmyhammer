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

impl<A: Agent> Drop for RecordingAgent<A> {
    fn drop(&mut self) {
        if let Err(e) = self.save() {
            tracing::error!("Failed to save recording: {}", e);
        }
    }
}

impl<A: Agent> Agent for RecordingAgent<A> {
    fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<InitializeResponse>> + Send>> {
        // Just forward for now - recording will be added later
        self.inner.initialize(request)
    }

    fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<AuthenticateResponse>> + Send>> {
        self.inner.authenticate(request)
    }

    fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<NewSessionResponse>> + Send>> {
        self.inner.new_session(request)
    }

    fn prompt(
        &self,
        request: PromptRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<PromptResponse>> + Send>> {
        self.inner.prompt(request)
    }

    fn cancel(
        &self,
        request: CancelNotification,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<()>> + Send>> {
        self.inner.cancel(request)
    }

    fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<LoadSessionResponse>> + Send>> {
        self.inner.load_session(request)
    }

    fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<SetSessionModeResponse>> + Send>> {
        self.inner.set_session_mode(request)
    }

    fn ext_method(
        &self,
        request: ExtRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<ExtResponse>> + Send>> {
        self.inner.ext_method(request)
    }

    fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<()>> + Send>> {
        self.inner.ext_notification(notification)
    }
}

impl<A: Agent + crate::AgentWithFixture> crate::AgentWithFixture for RecordingAgent<A> {
    fn agent_type(&self) -> &'static str {
        self.inner.agent_type()
    }
}
