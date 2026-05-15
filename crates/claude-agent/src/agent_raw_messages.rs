//! Raw message manager for recording JSON-RPC messages

use std::path::PathBuf;

/// Global registry of RawMessageManagers keyed by root session ID
///
/// This allows subagents to look up and share their root agent's RawMessageManager
/// so all agents in a session hierarchy write to the same transcript file.
static RAW_MESSAGE_MANAGERS: once_cell::sync::Lazy<
    std::sync::Mutex<std::collections::HashMap<String, RawMessageManager>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Raw message manager for recording JSON-RPC messages across all agents
///
/// Manages centralized recording of raw JSON-RPC messages from multiple agents
/// (root and subagents) to a single file. This ensures a complete transcript
/// of all message traffic without race conditions or truncation issues.
///
/// Uses an mpsc channel to serialize writes from concurrent agents, similar
/// to how NotificationSender broadcasts notifications.
#[derive(Debug, Clone)]
pub struct RawMessageManager {
    /// Channel for sending raw JSON-RPC messages to be written
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

impl RawMessageManager {
    /// Register a RawMessageManager for a root session ID
    ///
    /// This allows subagents to look up and share the manager
    pub fn register(session_id: String, manager: RawMessageManager) {
        if let Ok(mut registry) = RAW_MESSAGE_MANAGERS.lock() {
            registry.insert(session_id, manager);
        }
    }

    /// Look up a RawMessageManager by root session ID
    ///
    /// Returns None if not found in registry
    pub fn lookup(session_id: &str) -> Option<RawMessageManager> {
        RAW_MESSAGE_MANAGERS
            .lock()
            .ok()
            .and_then(|registry| registry.get(session_id).cloned())
    }

    /// Create a new raw message manager with file writer task
    ///
    /// Spawns a background task that writes messages to the specified file.
    /// All messages are appended to the file in the order received.
    ///
    /// # Parameters
    ///
    /// * `path` - Path to the output file (will be created/appended to)
    ///
    /// # Returns
    ///
    /// A RawMessageManager instance that can be cloned and shared across agents
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        use std::fs::OpenOptions;
        use std::io::Write;

        // Open file in append mode so sub-agents can share the same file
        // The file is created/opened when the root agent starts
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Spawn task to write messages sequentially
        tokio::task::spawn(async move {
            while let Some(message) = receiver.recv().await {
                if let Err(e) = writeln!(file, "{}", message) {
                    tracing::warn!("Failed to write raw message to file: {}", e);
                }
                // Flush after each write to ensure data is persisted
                if let Err(e) = file.flush() {
                    tracing::warn!("Failed to flush raw message file: {}", e);
                }
            }
        });

        Ok(Self { sender })
    }

    /// Record a raw JSON-RPC message
    ///
    /// Sends the message to the writer task to be appended to the file.
    /// Non-blocking - returns immediately after queuing the message.
    ///
    /// # Arguments
    ///
    /// * `message` - The raw JSON-RPC message string to record
    pub fn record(&self, message: String) {
        if let Err(e) = self.sender.send(message) {
            tracing::warn!("Failed to send raw message to recorder: {}", e);
        }
    }
}
