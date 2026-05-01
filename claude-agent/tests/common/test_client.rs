//! Minimal Client peer implementation for testing tool execution.
//!
//! This module provides a [`TestClient`] that mirrors the `AgentRequest` surface
//! a real ACP client would handle (filesystem reads/writes, session
//! notifications, permission prompts). It maintains an in-memory filesystem and
//! exposes inherent `async` methods matching each request type, letting tests
//! drive client-side behaviour directly without an actual transport.
//!
//! # ACP 0.11
//!
//! In ACP 0.10 this struct implemented the `Client` trait. ACP 0.11 removed
//! that trait — `agent_client_protocol::Client` is now a unit [`Role`] marker —
//! and replaced trait dispatch with the typed builder/handler runtime. The
//! inherent methods on `TestClient` are equivalent to the bodies that would
//! live inside `Client.builder().on_receive_request_from(Agent, ...)` callbacks
//! in a fully wired connection. Tests that need a real peer connection can
//! call those methods from within a `Client.builder()` closure.
//!
//! [`Role`]: agent_client_protocol::Role

use agent_client_protocol::schema::{
    ReadTextFileRequest, ReadTextFileResponse, RequestPermissionRequest, RequestPermissionResponse,
    SessionNotification, WriteTextFileRequest, WriteTextFileResponse,
};
use agent_client_protocol::Error as AcpError;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Minimal client-side handler for tool execution tests.
///
/// `TestClient` keeps an in-memory filesystem and provides inherent async
/// methods matching every request the agent can send back to the client. Tests
/// can call those methods directly to simulate client behaviour, or wrap them
/// in a `Client.builder().on_receive_request_from(Agent, ...)` callback when a
/// real `ConnectionTo` peer is required.
///
/// # Thread Safety
///
/// `TestClient` uses `Arc<RwLock<>>` for the file storage, making it safe to
/// share across async tasks while allowing concurrent reads and exclusive
/// writes.
///
/// # Example
///
/// ```no_run
/// use common::test_client::TestClient;
///
/// let client = TestClient::new();
/// client.add_file("/test/example.txt", "Hello, world!");
///
/// // Drive the inherent methods directly, or wire them up inside a
/// // `Client.builder().on_receive_request_from(Agent, ...)` callback.
/// ```
#[derive(Clone)]
pub struct TestClient {
    /// In-memory filesystem (path -> content)
    files: Arc<RwLock<HashMap<PathBuf, String>>>,
}

impl TestClient {
    /// Create a new TestClient with an empty filesystem
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Pre-populate a file for testing
    ///
    /// # Arguments
    ///
    /// * `path` - File path (will be converted to PathBuf)
    /// * `content` - File content
    ///
    /// # Example
    ///
    /// ```
    /// let client = TestClient::new();
    /// client.add_file("/test/Cargo.toml", "[package]\nname = \"test\"");
    /// ```
    pub fn add_file(&self, path: impl Into<PathBuf>, content: impl Into<String>) {
        let mut files = self
            .files
            .write()
            .expect("TestClient lock poisoned - a test panic occurred while holding the lock");
        files.insert(path.into(), content.into());
    }

    /// Get file content for assertions
    ///
    /// Returns None if the file doesn't exist.
    ///
    /// # Example
    ///
    /// ```
    /// let client = TestClient::new();
    /// client.add_file("/test/file.txt", "content");
    /// assert_eq!(client.get_file("/test/file.txt"), Some("content".to_string()));
    /// ```
    pub fn get_file(&self, path: impl Into<PathBuf>) -> Option<String> {
        let files = self
            .files
            .read()
            .expect("TestClient lock poisoned - a test panic occurred while holding the lock");
        files.get(&path.into()).cloned()
    }

    /// List all files in the in-memory filesystem
    ///
    /// Returns a vector of paths for all files currently stored.
    pub fn list_files(&self) -> Vec<PathBuf> {
        let files = self
            .files
            .read()
            .expect("TestClient lock poisoned - a test panic occurred while holding the lock");
        files.keys().cloned().collect()
    }
}

impl Default for TestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TestClient {
    /// Read a text file from the in-memory filesystem.
    ///
    /// Supports the optional `line` (1-indexed) and `limit` parameters of
    /// [`ReadTextFileRequest`] for partial reads. Returns
    /// [`AcpError::invalid_params`] if the path does not exist.
    pub async fn read_text_file(
        &self,
        request: ReadTextFileRequest,
    ) -> Result<ReadTextFileResponse, AcpError> {
        let files = self
            .files
            .read()
            .expect("TestClient lock poisoned - a test panic occurred while holding the lock");

        let content = files.get(&request.path).ok_or_else(|| {
            AcpError::invalid_params().data(format!("File not found: {:?}", request.path))
        })?;

        // Handle line/limit parameters
        let lines: Vec<&str> = content.lines().collect();
        let start = request.line.map(|l| (l - 1) as usize).unwrap_or(0);
        let end = request
            .limit
            .map(|l| start + l as usize)
            .unwrap_or(lines.len())
            .min(lines.len());

        let result_lines = if start < lines.len() {
            lines[start..end].join("\n")
        } else {
            String::new()
        };

        Ok(ReadTextFileResponse::new(result_lines))
    }

    /// Write a text file to the in-memory filesystem.
    ///
    /// Creates or overwrites the file at the specified path.
    pub async fn write_text_file(
        &self,
        request: WriteTextFileRequest,
    ) -> Result<WriteTextFileResponse, AcpError> {
        let mut files = self
            .files
            .write()
            .expect("TestClient lock poisoned - a test panic occurred while holding the lock");
        files.insert(request.path, request.content);

        Ok(WriteTextFileResponse::new())
    }

    /// Handle session notifications.
    ///
    /// `TestClient` ignores notifications since the existing tests only verify
    /// that they're sent, not that they're processed.
    pub async fn session_notification(
        &self,
        _notification: SessionNotification,
    ) -> Result<(), AcpError> {
        // Test client doesn't need to handle notifications
        Ok(())
    }

    /// Handle permission requests.
    ///
    /// `TestClient` auto-approves every permission to keep tests deterministic.
    pub async fn request_permission(
        &self,
        _request: RequestPermissionRequest,
    ) -> Result<RequestPermissionResponse, AcpError> {
        // Auto-approve all permissions in tests
        use agent_client_protocol::schema::{
            PermissionOptionId, RequestPermissionOutcome, SelectedPermissionOutcome,
        };
        let selected = SelectedPermissionOutcome::new(PermissionOptionId::new("allow"));
        Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Selected(selected),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::SessionId;

    fn test_session_id() -> SessionId {
        SessionId::new("test-session")
    }

    #[tokio::test]
    async fn test_read_existing_file() {
        let client = TestClient::new();
        client.add_file("/test/file.txt", "Hello, world!");

        let request = ReadTextFileRequest::new(test_session_id(), PathBuf::from("/test/file.txt"));

        let response = client.read_text_file(request).await.unwrap();
        assert_eq!(response.content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let client = TestClient::new();

        let request =
            ReadTextFileRequest::new(test_session_id(), PathBuf::from("/nonexistent.txt"));

        let result = client.read_text_file(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_with_line_offset() {
        let client = TestClient::new();
        client.add_file("/test/multi.txt", "line1\nline2\nline3\nline4");

        let request = ReadTextFileRequest::new(test_session_id(), PathBuf::from("/test/multi.txt"))
            .line(2) // Start at line 2 (0-indexed becomes line 1)
            .limit(2); // Read 2 lines

        let response = client.read_text_file(request).await.unwrap();
        assert_eq!(response.content, "line2\nline3");
    }

    #[tokio::test]
    async fn test_write_new_file() {
        let client = TestClient::new();

        let request = WriteTextFileRequest::new(
            test_session_id(),
            PathBuf::from("/test/new.txt"),
            "New content".to_string(),
        );

        let result = client.write_text_file(request).await;
        assert!(result.is_ok());

        // Verify file was created
        assert_eq!(
            client.get_file("/test/new.txt"),
            Some("New content".to_string())
        );
    }

    #[tokio::test]
    async fn test_write_overwrites_existing() {
        let client = TestClient::new();
        client.add_file("/test/existing.txt", "Old content");

        let request = WriteTextFileRequest::new(
            test_session_id(),
            PathBuf::from("/test/existing.txt"),
            "New content".to_string(),
        );

        client.write_text_file(request).await.unwrap();

        // Verify file was overwritten
        assert_eq!(
            client.get_file("/test/existing.txt"),
            Some("New content".to_string())
        );
    }

    #[tokio::test]
    async fn test_session_notification_succeeds() {
        let client = TestClient::new();

        let text_content = agent_client_protocol::schema::TextContent::new("test".to_string());
        let content_block = agent_client_protocol::schema::ContentBlock::Text(text_content);
        let content_chunk = agent_client_protocol::schema::ContentChunk::new(content_block);
        let notification = agent_client_protocol::schema::SessionNotification::new(
            test_session_id(),
            agent_client_protocol::schema::SessionUpdate::AgentMessageChunk(content_chunk),
        );

        let result = client.session_notification(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_request_permission_auto_approves() {
        let client = TestClient::new();

        let fields = agent_client_protocol::schema::ToolCallUpdateFields::new()
            .title("test-tool".to_string());
        let tool_call_update = agent_client_protocol::schema::ToolCallUpdate::new(
            agent_client_protocol::schema::ToolCallId::new("tool-1"),
            fields,
        );

        let request = RequestPermissionRequest::new(test_session_id(), tool_call_update, vec![]);

        let response = client.request_permission(request).await.unwrap();
        match response.outcome {
            agent_client_protocol::schema::RequestPermissionOutcome::Selected(selected) => {
                assert_eq!(selected.option_id.0.as_ref(), "allow");
            }
            _ => panic!("Expected Selected outcome with 'allow'"),
        }
    }

    #[tokio::test]
    async fn test_list_files() {
        let client = TestClient::new();
        client.add_file("/test/file1.txt", "content1");
        client.add_file("/test/file2.txt", "content2");

        let files = client.list_files();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&PathBuf::from("/test/file1.txt")));
        assert!(files.contains(&PathBuf::from("/test/file2.txt")));
    }
}
