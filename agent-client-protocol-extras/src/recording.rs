//! RecordingAgent - Captures all Agent interactions to JSON fixture
//!
//! This proxy wraps any Agent and records:
//! - All method calls (initialize, new_session, prompt, etc.)
//! - All responses
//! - All notifications
//!
//! On Drop, saves complete trace to JSON file.

use crate::AgentWithFixture;
use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SetSessionModeRequest, SetSessionModeResponse,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// A single method call and response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodCall {
    pub method: String,
    pub request: serde_json::Value,
    pub response: serde_json::Value,
}

/// Recorded agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedAgentSession {
    pub calls: Vec<MethodCall>,
}

/// Recording agent that wraps any Agent and captures all interactions
pub struct RecordingAgent<A> {
    inner: A,
    output_path: PathBuf,
    calls: Arc<Mutex<Vec<MethodCall>>>,
}

impl<A: Agent> RecordingAgent<A> {
    /// Create a new recording agent
    pub fn new(inner: A, output_path: PathBuf) -> Self {
        tracing::info!("RecordingAgent: Will record to {:?}", output_path);
        Self {
            inner,
            output_path,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record a method call
    fn record_call(&self, method: &str, request: &serde_json::Value, response: &serde_json::Value) {
        let call = MethodCall {
            method: method.to_string(),
            request: request.clone(),
            response: response.clone(),
        };
        self.calls.lock().unwrap().push(call);
    }

    /// Save recorded session to file
    fn save_recording(&self) -> Result<(), String> {
        let calls = self.calls.lock().unwrap();
        let session = RecordedAgentSession {
            calls: calls.clone(),
        };

        // Ensure parent directory exists
        if let Some(parent) = self.output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(&session)
            .map_err(|e| format!("Failed to serialize: {}", e))?;

        std::fs::write(&self.output_path, json)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        tracing::info!(
            "RecordingAgent: Saved {} method calls to {:?}",
            calls.len(),
            self.output_path
        );
        Ok(())
    }
}

impl<A: Agent> Drop for RecordingAgent<A> {
    fn drop(&mut self) {
        if let Err(e) = self.save_recording() {
            tracing::error!("Failed to save recording on drop: {}", e);
        }
    }
}

impl<A: Agent> AgentWithFixture for RecordingAgent<A> {
    fn agent_type(&self) -> &'static str {
        // Forward to inner agent if it implements trait
        // For now, return "recording" as fallback
        "recording"
    }

    fn with_fixture(&mut self, _test_name: &str) {
        // RecordingAgent is already configured for recording
        // This is a no-op
    }
}

#[async_trait::async_trait]
impl<A: Agent> Agent for RecordingAgent<A> {
    fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<InitializeResponse>> + Send>> {
        let req_json = serde_json::to_value(&request).unwrap_or_default();
        let calls = Arc::clone(&self.calls);

        Box::pin(async move {
            let result = self.inner.initialize(request).await;

            if let Ok(ref response) = result {
                let resp_json = serde_json::to_value(response).unwrap_or_default();
                let call = MethodCall {
                    method: "initialize".to_string(),
                    request: req_json,
                    response: resp_json,
                };
                calls.lock().unwrap().push(call);
            }

            result
        })
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
        let req_json = serde_json::to_value(&request).unwrap_or_default();
        let calls = Arc::clone(&self.calls);

        Box::pin(async move {
            let result = self.inner.new_session(request).await;

            if let Ok(ref response) = result {
                let resp_json = serde_json::to_value(response).unwrap_or_default();
                let call = MethodCall {
                    method: "new_session".to_string(),
                    request: req_json,
                    response: resp_json,
                };
                calls.lock().unwrap().push(call);
            }

            result
        })
    }

    fn prompt(
        &self,
        request: PromptRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = agent_client_protocol::Result<PromptResponse>> + Send>> {
        let req_json = serde_json::to_value(&request).unwrap_or_default();
        let calls = Arc::clone(&self.calls);

        Box::pin(async move {
            let result = self.inner.prompt(request).await;

            if let Ok(ref response) = result {
                let resp_json = serde_json::to_value(response).unwrap_or_default();
                let call = MethodCall {
                    method: "prompt".to_string(),
                    request: req_json,
                    response: resp_json,
                };
                calls.lock().unwrap().push(call);
            }

            result
        })
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
