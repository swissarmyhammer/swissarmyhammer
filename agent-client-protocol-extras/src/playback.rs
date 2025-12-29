//! PlaybackAgent - Replays recorded Agent method calls

use crate::recording::RecordedSession;
use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification,
    ExtRequest, ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SetSessionModeRequest, SetSessionModeResponse,
};
use std::path::PathBuf;
use std::sync::Mutex;

/// PlaybackAgent replays recorded method calls
pub struct PlaybackAgent {
    session: RecordedSession,
    current_call: Mutex<usize>,
    agent_type: &'static str,
    /// Channel to send notifications during playback
    notification_tx: tokio::sync::broadcast::Sender<agent_client_protocol::SessionNotification>,
}

impl PlaybackAgent {
    pub fn new(path: PathBuf, agent_type: &'static str) -> Self {
        tracing::info!("PlaybackAgent: Loading from {:?}", path);

        let session = std::fs::read_to_string(&path)
            .and_then(|content| {
                serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load fixture from {:?}: {}, using empty", path, e);
                RecordedSession { calls: vec![] }
            });

        tracing::info!("PlaybackAgent: Loaded {} calls", session.calls.len());

        let (notification_tx, _) = tokio::sync::broadcast::channel(1000);

        Self {
            session,
            current_call: Mutex::new(0),
            agent_type,
            notification_tx,
        }
    }

    /// Get notification receiver for playback
    pub fn subscribe_notifications(
        &self,
    ) -> tokio::sync::broadcast::Receiver<agent_client_protocol::SessionNotification> {
        self.notification_tx.subscribe()
    }

    fn get_next_call(
        &self,
        method: &str,
    ) -> agent_client_protocol::Result<(serde_json::Value, Vec<serde_json::Value>)> {
        let mut index = self.current_call.lock().unwrap();

        if *index >= self.session.calls.len() {
            tracing::error!(
                "PlaybackAgent: No more recorded calls (requested {}, have {})",
                *index + 1,
                self.session.calls.len()
            );
            return Err(agent_client_protocol::Error::internal_error());
        }

        let call = &self.session.calls[*index];
        if call.method != method {
            tracing::warn!(
                "PlaybackAgent: Method mismatch - expected {}, got {}",
                method,
                call.method
            );
        }

        *index += 1;
        Ok((call.response.clone(), call.notifications.clone()))
    }

    fn replay_notifications(&self, notifications: Vec<serde_json::Value>) {
        if notifications.is_empty() {
            return;
        }

        tracing::info!(
            "PlaybackAgent: Replaying {} notifications",
            notifications.len()
        );
        let tx = self.notification_tx.clone();

        tokio::spawn(async move {
            for notif_json in notifications {
                if let Ok(notification) =
                    serde_json::from_value::<agent_client_protocol::SessionNotification>(notif_json)
                {
                    let _ = tx.send(notification);
                }
            }
        });
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for PlaybackAgent {
    async fn initialize(
        &self,
        _request: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        let (response_json, notifications) = self.get_next_call("initialize")?;
        self.replay_notifications(notifications);

        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize initialize response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn authenticate(
        &self,
        _request: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        let (response_json, _notifications) = self.get_next_call("authenticate")?;
        serde_json::from_value(response_json)
            .map_err(|_| agent_client_protocol::Error::internal_error())
    }

    async fn new_session(
        &self,
        _request: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        let (response_json, notifications) = self.get_next_call("new_session")?;
        self.replay_notifications(notifications);

        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize new_session response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn prompt(
        &self,
        _request: PromptRequest,
    ) -> agent_client_protocol::Result<PromptResponse> {
        let (response_json, notifications) = self.get_next_call("prompt")?;
        self.replay_notifications(notifications);

        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize prompt response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn cancel(&self, _request: CancelNotification) -> agent_client_protocol::Result<()> {
        Ok(())
    }

    async fn load_session(
        &self,
        _request: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        let (response_json, notifications) = self.get_next_call("load_session")?;
        self.replay_notifications(notifications);

        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize load_session response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn set_session_mode(
        &self,
        _request: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        let (response_json, notifications) = self.get_next_call("set_session_mode")?;
        self.replay_notifications(notifications);

        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize set_session_mode response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn ext_method(&self, _request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        let (response_json, notifications) = self.get_next_call("ext_method")?;
        self.replay_notifications(notifications);

        // Check if response is an error
        if let Some(error) = response_json.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-32603) as i32;
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Internal error")
                .to_string();
            let mut err = agent_client_protocol::Error::new(code, message);
            if let Some(data) = error.get("data").cloned() {
                err = err.data(data);
            }
            return Err(err);
        }

        serde_json::from_value(response_json).map_err(|e| {
            tracing::error!("Failed to deserialize ext_method response: {}", e);
            agent_client_protocol::Error::internal_error()
        })
    }

    async fn ext_notification(
        &self,
        _notification: ExtNotification,
    ) -> agent_client_protocol::Result<()> {
        Ok(())
    }
}

impl crate::AgentWithFixture for PlaybackAgent {
    fn agent_type(&self) -> &'static str {
        self.agent_type
    }

    fn is_playback(&self) -> bool {
        true
    }
}
