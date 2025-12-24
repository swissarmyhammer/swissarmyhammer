//! PlaybackAgent - Replays recorded Agent method calls

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SetSessionModeRequest, SetSessionModeResponse,
};
use crate::recording::RecordedSession;
use std::path::PathBuf;
use std::sync::Mutex;

/// PlaybackAgent replays recorded method calls
pub struct PlaybackAgent {
    session: RecordedSession,
    current_call: Mutex<usize>,
    agent_type: &'static str,
}

impl PlaybackAgent {
    pub fn new(path: PathBuf, agent_type: &'static str) -> Self {
        tracing::info!("PlaybackAgent: Loading from {:?}", path);

        let session = std::fs::read_to_string(&path)
            .and_then(|content| serde_json::from_str(&content).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load fixture from {:?}: {}, using empty", path, e);
                RecordedSession { calls: vec![] }
            });

        tracing::info!("PlaybackAgent: Loaded {} calls", session.calls.len());

        Self {
            session,
            current_call: Mutex::new(0),
            agent_type,
        }
    }

    fn get_next_call(&self, method: &str) -> agent_client_protocol::Result<serde_json::Value> {
        let mut index = self.current_call.lock().unwrap();

        if *index >= self.session.calls.len() {
            tracing::error!("PlaybackAgent: No more recorded calls (requested {}, have {})", *index + 1, self.session.calls.len());
            return Err(agent_client_protocol::Error::internal_error());
        }

        let call = &self.session.calls[*index];
        if call.method != method {
            tracing::warn!("PlaybackAgent: Method mismatch - expected {}, got {}", method, call.method);
        }

        *index += 1;
        Ok(call.response.clone())
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for PlaybackAgent {
    async fn initialize(&self, _request: InitializeRequest) -> agent_client_protocol::Result<InitializeResponse> {
        let response_json = self.get_next_call("initialize")?;
        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize initialize response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn authenticate(&self, _request: AuthenticateRequest) -> agent_client_protocol::Result<AuthenticateResponse> {
        let response_json = self.get_next_call("authenticate")?;
        serde_json::from_value(response_json).map_err(|_| agent_client_protocol::Error::internal_error())
    }

    async fn new_session(&self, _request: NewSessionRequest) -> agent_client_protocol::Result<NewSessionResponse> {
        let response_json = self.get_next_call("new_session")?;
        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize new_session response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn prompt(&self, _request: PromptRequest) -> agent_client_protocol::Result<PromptResponse> {
        let response_json = self.get_next_call("prompt")?;
        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize prompt response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn cancel(&self, _request: CancelNotification) -> agent_client_protocol::Result<()> {
        Ok(())
    }

    async fn load_session(&self, _request: LoadSessionRequest) -> agent_client_protocol::Result<LoadSessionResponse> {
        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn set_session_mode(&self, _request: SetSessionModeRequest) -> agent_client_protocol::Result<SetSessionModeResponse> {
        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn ext_method(&self, _request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn ext_notification(&self, _notification: ExtNotification) -> agent_client_protocol::Result<()> {
        Ok(())
    }
}

impl crate::AgentWithFixture for PlaybackAgent {
    fn agent_type(&self) -> &'static str {
        self.agent_type
    }
}
