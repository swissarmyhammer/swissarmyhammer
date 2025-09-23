//! Transcript recording system for llama agent sessions
//!
//! This module provides functionality to record live transcripts of messages
//! for each llama agent session into the `.swissarmyhammer/transcripts` directory.
//! The transcript is rewritten completely on each message to provide a real-time
//! record of the conversation in YAML format.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};
use ulid::Ulid;

/// Represents a single message in the transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptMessage {
    /// Unique identifier for this message
    pub message_id: String,
    /// Timestamp when the message was created
    pub timestamp: DateTime<Utc>,
    /// Role of the message sender (system, user, assistant)
    pub role: String,
    /// Content of the message
    pub content: String,
    /// Optional metadata for the message
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Complete transcript for a llama agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    /// Unique session identifier
    pub session_id: String,
    /// Timestamp when the session started
    pub session_start: DateTime<Utc>,
    /// Model being used for this session
    pub model: String,
    /// List of all messages in chronological order
    pub messages: Vec<TranscriptMessage>,
    /// Session-level metadata
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Handles transcript recording for llama agent sessions
#[derive(Debug)]
pub struct TranscriptRecorder {
    /// Base directory for transcript files
    transcript_dir: PathBuf,
    /// Current active transcript (if any)
    current_transcript: Option<Transcript>,
    /// File path for the current transcript
    current_file_path: Option<PathBuf>,
}

impl TranscriptRecorder {
    /// Create a new transcript recorder
    ///
    /// # Arguments
    /// * `base_dir` - Base directory where transcript files will be stored
    ///
    /// # Returns
    /// A new `TranscriptRecorder` instance
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let transcript_dir = base_dir.as_ref().join("transcripts");
        Self {
            transcript_dir,
            current_transcript: None,
            current_file_path: None,
        }
    }

    /// Start a new transcript session
    ///
    /// # Arguments
    /// * `model_name` - Name/identifier of the model being used
    ///
    /// # Returns
    /// Result containing the session ID, or error if session creation failed
    pub async fn start_session(
        &mut self,
        model_name: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Ensure transcript directory exists
        fs::create_dir_all(&self.transcript_dir).await?;

        // Generate session ID and create transcript
        let session_id = Ulid::new().to_string();
        let session_start = Utc::now();

        let transcript = Transcript {
            session_id: session_id.clone(),
            session_start,
            model: model_name,
            messages: Vec::new(),
            metadata: HashMap::new(),
        };

        // Generate file path
        let timestamp_str = session_start.format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("transcript_{}_{}.yaml", timestamp_str, session_id);
        let file_path = self.transcript_dir.join(filename);

        info!(
            "Starting new transcript session: {} (file: {})",
            session_id,
            file_path.display()
        );

        self.current_transcript = Some(transcript);
        self.current_file_path = Some(file_path);

        // Write initial empty transcript
        self.write_transcript().await?;

        Ok(session_id)
    }

    /// Add a message to the current transcript
    ///
    /// # Arguments
    /// * `role` - Role of the message sender (system, user, assistant)
    /// * `content` - Content of the message
    /// * `metadata` - Optional metadata for the message
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn add_message(
        &mut self,
        role: String,
        content: String,
        metadata: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let transcript = self
            .current_transcript
            .as_mut()
            .ok_or("No active transcript session")?;

        let message = TranscriptMessage {
            message_id: Ulid::new().to_string(),
            timestamp: Utc::now(),
            role,
            content,
            metadata,
        };

        debug!(
            "Adding message to transcript (session: {}, role: {}, content_length: {})",
            transcript.session_id,
            message.role,
            message.content.len()
        );

        transcript.messages.push(message);

        // Rewrite the entire transcript file for live updates
        self.write_transcript().await?;

        Ok(())
    }

    /// Add session-level metadata
    ///
    /// # Arguments
    /// * `key` - Metadata key
    /// * `value` - Metadata value
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn add_session_metadata(
        &mut self,
        key: String,
        value: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let transcript = self
            .current_transcript
            .as_mut()
            .ok_or("No active transcript session")?;

        transcript.metadata.insert(key, value);

        // Rewrite the transcript file
        self.write_transcript().await?;

        Ok(())
    }

    /// End the current transcript session
    ///
    /// # Returns
    /// Result containing the final transcript file path, or error
    pub async fn end_session(
        &mut self,
    ) -> Result<Option<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(transcript) = &self.current_transcript {
            info!(
                "Ending transcript session: {} ({} messages)",
                transcript.session_id,
                transcript.messages.len()
            );

            // Write final transcript
            self.write_transcript().await?;

            let final_path = self.current_file_path.clone();

            // Clear current session
            self.current_transcript = None;
            self.current_file_path = None;

            Ok(final_path)
        } else {
            warn!("Attempted to end session but no active transcript");
            Ok(None)
        }
    }



    /// Write the current transcript to disk (complete rewrite for live updates)
    async fn write_transcript(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let transcript = self
            .current_transcript
            .as_ref()
            .ok_or("No active transcript to write")?;

        let file_path = self
            .current_file_path
            .as_ref()
            .ok_or("No file path for current transcript")?;

        // Serialize transcript to YAML
        let yaml_content = serde_yaml::to_string(transcript)
            .map_err(|e| format!("Failed to serialize transcript to YAML: {}", e))?;

        debug!("Serialized YAML content: {}", yaml_content);

        // Write to file atomically using a temporary file
        let temp_path = file_path.with_extension("yaml.tmp");

        fs::write(&temp_path, &yaml_content)
            .await
            .map_err(|e| format!("Failed to write transcript to temporary file: {}", e))?;

        // Atomic rename
        fs::rename(&temp_path, file_path)
            .await
            .map_err(|e| format!("Failed to rename transcript file: {}", e))?;

        debug!(
            "Transcript written to {} ({} messages, {} bytes)",
            file_path.display(),
            transcript.messages.len(),
            std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0)
        );

        Ok(())
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_transcript_serialization_debug() {
        use chrono::Utc;

        // Create a minimal transcript directly
        let messages = vec![TranscriptMessage {
            message_id: "test-id".to_string(),
            timestamp: Utc::now(),
            role: "user".to_string(),
            content: "test content".to_string(),
            metadata: None,
        }];

        let transcript = Transcript {
            session_id: "test-session".to_string(),
            session_start: Utc::now(),
            model: "test-model".to_string(),
            messages,
            metadata: HashMap::new(),
        };

        // Test serialization
        let yaml = serde_yaml::to_string(&transcript).unwrap();
        println!("Serialized YAML: {}", yaml);

        // Test deserialization
        let _deserialized: Transcript = serde_yaml::from_str(&yaml).unwrap();
    }

    #[tokio::test]
    async fn test_transcript_recorder_basic_flow() {
        let temp_dir = TempDir::new().unwrap();
        let mut recorder = TranscriptRecorder::new(temp_dir.path());

        // Start session
        println!("Starting session...");
        let session_id = recorder
            .start_session("test-model".to_string())
            .await
            .unwrap();
        println!("Session started with ID: {}", session_id);

        assert!(!session_id.is_empty());

        // Add messages
        println!("Adding system message...");
        recorder
            .add_message("system".to_string(), "System prompt".to_string(), None)
            .await
            .unwrap();

        let mut metadata = HashMap::new();
        metadata.insert(
            "tokens".to_string(),
            serde_json::Value::Number(serde_json::Number::from(150)),
        );

        println!("Adding user message...");
        recorder
            .add_message("user".to_string(), "User message".to_string(), None)
            .await
            .unwrap();

        println!("Adding assistant message...");
        recorder
            .add_message(
                "assistant".to_string(),
                "Assistant response".to_string(),
                Some(metadata),
            )
            .await
            .unwrap();

        // End session
        println!("Ending session...");
        let final_path = recorder.end_session().await.unwrap();
        assert!(final_path.is_some());
        let file_path = final_path.as_ref().unwrap();
        println!("Checking file exists: {}", file_path.display());
        assert!(file_path.exists());

        // Verify file contents
        let file_path = final_path.unwrap();
        println!("Reading file: {}", file_path.display());
        let content = match fs::read_to_string(&file_path).await {
            Ok(content) => {
                println!("Successfully read {} bytes", content.len());
                content
            }
            Err(e) => {
                println!("Failed to read file {}: {}", file_path.display(), e);
                panic!("File read failed: {}", e);
            }
        };
        println!("YAML content:\n{}", content);

        // Try parsing with better error handling
        let transcript: Transcript = match serde_yaml::from_str(&content) {
            Ok(t) => {
                println!("Successfully parsed transcript!");
                t
            }
            Err(e) => {
                println!("Failed to parse YAML: {}", e);
                println!("Content was: {}", content);
                panic!("YAML parsing failed: {}", e);
            }
        };

        assert_eq!(transcript.session_id, session_id);
        assert_eq!(transcript.model, "test-model");
        assert_eq!(transcript.messages.len(), 3);
        assert_eq!(transcript.messages[0].role, "system");
        assert_eq!(transcript.messages[1].role, "user");
        assert_eq!(transcript.messages[2].role, "assistant");
        assert!(transcript.messages[2].metadata.is_some());

        println!("Test completed successfully - all assertions passed!");
    }

    #[tokio::test]
    async fn test_transcript_recorder_session_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let mut recorder = TranscriptRecorder::new(temp_dir.path());

        recorder
            .start_session("test-model".to_string())
            .await
            .unwrap();

        // Add session metadata
        recorder
            .add_session_metadata(
                "execution_time_ms".to_string(),
                serde_json::Value::Number(serde_json::Number::from(1500)),
            )
            .await
            .unwrap();

        let final_path = recorder.end_session().await.unwrap().unwrap();

        // Verify metadata was saved
        let content = fs::read_to_string(final_path).await.unwrap();
        let transcript: Transcript = serde_yaml::from_str(&content).unwrap();

        assert!(!transcript.metadata.is_empty());
        assert_eq!(
            transcript.metadata.get("execution_time_ms").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(1500))
        );
    }

    #[tokio::test]
    async fn test_transcript_recorder_no_active_session() {
        let temp_dir = TempDir::new().unwrap();
        let mut recorder = TranscriptRecorder::new(temp_dir.path());

        // Try to add message without active session
        let result = recorder
            .add_message("user".to_string(), "Test".to_string(), None)
            .await;
        assert!(result.is_err());

        // Try to end session without active session
        let result = recorder.end_session().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_transcript_recorder_file_structure() {
        let temp_dir = TempDir::new().unwrap();
        let mut recorder = TranscriptRecorder::new(temp_dir.path());

        let session_id = recorder
            .start_session("test-model".to_string())
            .await
            .unwrap();

        let file_path = recorder.end_session().await.unwrap().unwrap();

        // Check file naming convention
        let filename = file_path.file_name().unwrap().to_str().unwrap();
        assert!(filename.starts_with("transcript_"));
        assert!(filename.ends_with(".yaml"));
        assert!(filename.contains(&session_id));

        // Check directory structure
        assert_eq!(
            file_path.parent().unwrap().file_name().unwrap(),
            "transcripts"
        );
    }
}
