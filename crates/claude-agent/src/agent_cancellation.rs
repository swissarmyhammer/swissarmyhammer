//! Cancellation management for agent sessions

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{broadcast, RwLock};

/// Cancellation state for a session
///
/// Tracks the cancellation status and metadata for operations within a session.
/// This allows immediate cancellation response and proper cleanup coordination.
#[derive(Debug, Clone)]
pub struct CancellationState {
    /// Whether the session is cancelled
    pub cancelled: bool,
    /// When the cancellation occurred
    pub cancellation_time: SystemTime,
    /// Set of operation IDs that have been cancelled
    pub cancelled_operations: HashSet<String>,
    /// Reason for cancellation (for debugging)
    pub cancellation_reason: String,
}

impl CancellationState {
    /// Create a new active (non-cancelled) state
    pub fn active() -> Self {
        Self {
            cancelled: false,
            cancellation_time: SystemTime::now(),
            cancelled_operations: HashSet::new(),
            cancellation_reason: String::new(),
        }
    }

    /// Mark as cancelled with reason
    pub fn cancel(&mut self, reason: &str) {
        self.cancelled = true;
        self.cancellation_time = SystemTime::now();
        self.cancellation_reason = reason.to_string();
    }

    /// Add a cancelled operation ID
    pub fn add_cancelled_operation(&mut self, operation_id: String) {
        self.cancelled_operations.insert(operation_id);
    }

    /// Check if operation is cancelled
    pub fn is_operation_cancelled(&self, operation_id: &str) -> bool {
        self.cancelled || self.cancelled_operations.contains(operation_id)
    }
}

/// Manager for session cancellation state
///
/// Provides thread-safe cancellation coordination across all session operations.
/// Supports immediate cancellation notification and proper cleanup coordination.
pub struct CancellationManager {
    /// Session ID -> CancellationState mapping
    cancellation_states: Arc<RwLock<HashMap<String, CancellationState>>>,
    /// Broadcast sender for immediate cancellation notifications
    cancellation_sender: broadcast::Sender<String>,
}

impl CancellationManager {
    /// Create a new cancellation manager with configurable buffer size
    pub fn new(buffer_size: usize) -> (Self, broadcast::Receiver<String>) {
        let (sender, receiver) = broadcast::channel(buffer_size);
        (
            Self {
                cancellation_states: Arc::new(RwLock::new(HashMap::new())),
                cancellation_sender: sender,
            },
            receiver,
        )
    }

    /// Check if a session is cancelled
    pub async fn is_cancelled(&self, session_id: &str) -> bool {
        let states = self.cancellation_states.read().await;
        states
            .get(session_id)
            .map(|state| state.cancelled)
            .unwrap_or(false)
    }

    /// Mark a session as cancelled
    pub async fn mark_cancelled(&self, session_id: &str, reason: &str) -> crate::Result<()> {
        {
            let mut states = self.cancellation_states.write().await;
            let state = states
                .entry(session_id.to_string())
                .or_insert_with(CancellationState::active);
            state.cancel(reason);
        }

        // Broadcast cancellation immediately
        if let Err(e) = self.cancellation_sender.send(session_id.to_string()) {
            tracing::warn!(
                "Failed to broadcast cancellation for session {}: {}",
                session_id,
                e
            );
        }

        tracing::info!("Session {} marked as cancelled: {}", session_id, reason);
        Ok(())
    }

    /// Add a cancelled operation to a session
    pub async fn add_cancelled_operation(&self, session_id: &str, operation_id: String) {
        let mut states = self.cancellation_states.write().await;
        let state = states
            .entry(session_id.to_string())
            .or_insert_with(CancellationState::active);
        state.add_cancelled_operation(operation_id);
    }

    /// Get cancellation state for debugging
    pub async fn get_cancellation_state(&self, session_id: &str) -> Option<CancellationState> {
        let states = self.cancellation_states.read().await;
        states.get(session_id).cloned()
    }

    /// Clean up cancellation state for a session (called when session ends)
    pub async fn cleanup_session(&self, session_id: &str) {
        let mut states = self.cancellation_states.write().await;
        states.remove(session_id);
    }

    /// Reset cancellation state for a new prompt turn
    ///
    /// Called at the start of each prompt to ensure cancellation from previous
    /// turns doesn't affect the new prompt.
    pub async fn reset_for_new_turn(&self, session_id: &str) {
        let mut states = self.cancellation_states.write().await;
        // Replace with fresh active state, discarding any previous cancellation
        states.insert(session_id.to_string(), CancellationState::active());
        tracing::debug!(
            "Reset cancellation state for session {} (new turn)",
            session_id
        );
    }

    /// Subscribe to cancellation notifications
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.cancellation_sender.subscribe()
    }
}
