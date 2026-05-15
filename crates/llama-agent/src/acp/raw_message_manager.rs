//! Raw message recording for ACP protocol debugging
//!
//! This module provides functionality to record all JSON-RPC messages exchanged
//! during ACP sessions to a file for debugging and auditing purposes.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manager for recording raw JSON-RPC messages to a file
///
/// All messages are appended to `.acp/transcript_raw.jsonl` in the current directory.
/// This provides a complete audit trail of all protocol-level communication.
#[derive(Debug, Clone)]
pub struct RawMessageManager {
    /// Channel for sending raw JSON-RPC messages to be written
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

impl RawMessageManager {
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
    /// A RawMessageManager instance that can be cloned and shared
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        use std::fs::OpenOptions;
        use std::io::Write;

        // Open file in append mode
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let file = Arc::new(Mutex::new(file));

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Spawn task to write messages sequentially
        let file_clone = Arc::clone(&file);
        tokio::task::spawn(async move {
            while let Some(message) = receiver.recv().await {
                let mut file_guard = file_clone.lock().await;
                if let Err(e) = writeln!(*file_guard, "{}", message) {
                    tracing::warn!("Failed to write raw message to file: {}", e);
                }
                // Flush after each write to ensure data is persisted
                if let Err(e) = file_guard.flush() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_raw_message_manager() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_raw_messages.jsonl");

        // Clean up any existing test file
        let _ = fs::remove_file(&test_file);

        // Create manager
        let manager = RawMessageManager::new(test_file.clone()).unwrap();

        // Record some messages
        manager.record(r#"{"type":"init","session":"test1"}"#.to_string());
        manager.record(r#"{"type":"prompt","content":"hello"}"#.to_string());

        // Drop the manager to ensure all messages are flushed
        drop(manager);

        // Give the writer task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify the file exists before reading
        assert!(test_file.exists(), "Test file was not created");

        // Read the file and verify contents
        let contents = fs::read_to_string(&test_file).unwrap();
        assert!(contents.contains(r#"{"type":"init","session":"test1"}"#));
        assert!(contents.contains(r#"{"type":"prompt","content":"hello"}"#));

        // Clean up
        let _ = fs::remove_file(&test_file);
    }
}
