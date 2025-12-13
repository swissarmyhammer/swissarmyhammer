//! Test recording utilities for capturing Claude process interactions
//!
//! This module provides helpers for recording and playing back Claude process
//! interactions, enabling tests to run without spawning real Claude binaries.
//!
//! # Recording a Real Claude Session
//!
//! Wrap ClaudeProcess I/O calls with ClaudeRecorder to capture interactions:
//!
//! ```rust,ignore
//! let recorder = ClaudeRecorder::new();
//! let mut process = ClaudeProcess::spawn(session_id)?;
//!
//! // Record each write
//! let input = r#"{"type":"user","message":{...}}"#;
//! process.write_line(input).await?;
//! recorder.record_input(input);
//!
//! // Record each read
//! while let Some(output) = process.read_line().await? {
//!     recorder.record_output(&output);
//! }
//!
//! // Save to fixture
//! recorder.save_to_file("tests/fixtures/my_test.json")?;
//! ```
//!
//! # Playback in Tests
//!
//! Use RecordedClaudeBackend to replay without spawning Claude:
//!
//! ```rust,ignore
//! let mut backend = RecordedClaudeBackend::from_file("fixtures/my_test.json")?;
//! backend.write_line(input).await?;
//! let output = backend.read_line().await?.unwrap();
//! assert!(output.contains("expected"));
//! ```
//!
//! See `tests/test_prompt_recorded.rs` for complete examples.

use claude_agent::claude_backend::{ClaudeExchange, RecordedSession};
use std::sync::{Arc, Mutex};

/// A recording wrapper that captures all Claude I/O
///
/// This can wrap a real ClaudeProcess and record all interactions
/// to a fixture file for later playback.
pub struct ClaudeRecorder {
    exchanges: Arc<Mutex<Vec<ClaudeExchange>>>,
    current_input: Arc<Mutex<Option<String>>>,
    current_outputs: Arc<Mutex<Vec<String>>>,
}

impl ClaudeRecorder {
    /// Create a new recorder
    pub fn new() -> Self {
        Self {
            exchanges: Arc::new(Mutex::new(Vec::new())),
            current_input: Arc::new(Mutex::new(None)),
            current_outputs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record an input line sent to Claude
    pub fn record_input(&self, line: &str) {
        // If there's a previous exchange, finalize it
        if self.current_input.lock().unwrap().is_some() {
            self.finalize_exchange();
        }

        // Start new exchange
        *self.current_input.lock().unwrap() = Some(line.to_string());
        self.current_outputs.lock().unwrap().clear();
    }

    /// Record an output line from Claude
    pub fn record_output(&self, line: &str) {
        self.current_outputs.lock().unwrap().push(line.to_string());
    }

    /// Finalize the current exchange
    fn finalize_exchange(&self) {
        let input = self.current_input.lock().unwrap().take();
        let outputs = std::mem::take(&mut *self.current_outputs.lock().unwrap());

        if let Some(input) = input {
            let exchange = ClaudeExchange { input, outputs };
            self.exchanges.lock().unwrap().push(exchange);
        }
    }

    /// Get the recorded session
    pub fn into_session(self) -> RecordedSession {
        // Finalize any pending exchange
        self.finalize_exchange();

        let exchanges = Arc::try_unwrap(self.exchanges)
            .ok()
            .and_then(|mutex| mutex.into_inner().ok())
            .unwrap_or_default();

        RecordedSession { exchanges }
    }

    /// Save the recorded session to a JSON file
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        self.finalize_exchange();

        let exchanges = self.exchanges.lock().unwrap().clone();
        let session = RecordedSession { exchanges };

        let json = serde_json::to_string_pretty(&session)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        std::fs::write(path, json)
    }
}

impl Default for ClaudeRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_basic() {
        let recorder = ClaudeRecorder::new();

        recorder.record_input("input1");
        recorder.record_output("output1a");
        recorder.record_output("output1b");

        recorder.record_input("input2");
        recorder.record_output("output2a");

        let session = recorder.into_session();

        assert_eq!(session.exchanges.len(), 2);
        assert_eq!(session.exchanges[0].input, "input1");
        assert_eq!(session.exchanges[0].outputs.len(), 2);
        assert_eq!(session.exchanges[1].input, "input2");
        assert_eq!(session.exchanges[1].outputs.len(), 1);
    }

    #[test]
    fn test_recorder_save_to_file() {
        let recorder = ClaudeRecorder::new();

        recorder.record_input("test_input");
        recorder.record_output("test_output");

        let temp_path = std::env::temp_dir().join("test_recording.json");
        recorder.save_to_file(&temp_path).unwrap();

        // Verify file was created
        assert!(temp_path.exists());

        // Clean up
        std::fs::remove_file(temp_path).ok();
    }
}
