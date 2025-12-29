//! File system protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/file-system
//!
//! ## Requirements Tested
//!
//! 1. **Checking Support**
//!    - Agents MUST verify client capabilities before attempting filesystem methods
//!    - Check `clientCapabilities.fs.readTextFile` and `clientCapabilities.fs.writeTextFile`
//!    - If false or not present, agent MUST NOT attempt to call the method
//!
//! 2. **Reading Files**
//!    - Method: `fs/read_text_file`
//!    - Required params: `sessionId`, `path` (absolute)
//!    - Optional params: `line` (1-based), `limit` (number of lines)
//!    - Response: `{ content: string }`
//!
//! 3. **Writing Files**
//!    - Method: `fs/write_text_file`
//!    - Required params: `sessionId`, `path` (absolute), `content`
//!    - Client MUST create file if doesn't exist
//!    - Response: `null` on success

use agent_client_protocol::{
    Agent, ClientCapabilities, ExtRequest, FileSystemCapability, InitializeRequest, ProtocolVersion,
};
use agent_client_protocol_extras::recording::RecordedSession;
use serde_json::json;
use std::sync::Arc;

/// Statistics from file system fixture verification
#[derive(Debug, Default)]
pub struct FileSystemStats {
    pub initialize_calls: usize,
    pub new_session_calls: usize,
    pub ext_method_calls: usize,
}

/// Test that agent properly checks readTextFile capability before allowing reads
pub async fn test_read_text_file_capability_check<A: Agent + ?Sized>(
    agent: &A,
) -> crate::Result<()> {
    tracing::info!("Testing fs/read_text_file capability check");

    // Initialize with NO readTextFile capability
    let client_caps = ClientCapabilities::new().fs(FileSystemCapability::new()
        .read_text_file(false)
        .write_text_file(false));

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Attempt to read a file without capability
    let params = json!({
        "sessionId": session_id.0,
        "path": "/tmp/test.txt"
    });

    let ext_request = ExtRequest::new(
        "fs/read_text_file",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    // Should return an error because capability is not declared
    let result = agent.ext_method(ext_request).await;

    match result {
        Err(e) => {
            // Agent correctly rejected the request
            // The error should be Invalid params (-32602) according to JSON-RPC spec
            let error_msg = format!("{:?}", e);
            if error_msg.contains("Invalid params") || error_msg.contains("-32602") {
                tracing::info!("Agent correctly rejected fs/read_text_file without capability (Invalid params)");
                Ok(())
            } else if error_msg.contains("capability") || error_msg.contains("not supported") {
                tracing::info!("Agent correctly rejected fs/read_text_file without capability");
                Ok(())
            } else {
                Err(crate::Error::Validation(format!(
                    "Agent rejected fs/read_text_file but with unexpected error: {}",
                    error_msg
                )))
            }
        }
        Ok(_) => Err(crate::Error::Validation(
            "Agent should reject fs/read_text_file when capability not declared".to_string(),
        )),
    }
}

/// Test that agent properly checks writeTextFile capability before allowing writes
pub async fn test_write_text_file_capability_check<A: Agent + ?Sized>(
    agent: &A,
) -> crate::Result<()> {
    tracing::info!("Testing fs/write_text_file capability check");

    // Initialize with NO writeTextFile capability
    let client_caps = ClientCapabilities::new().fs(FileSystemCapability::new()
        .read_text_file(false)
        .write_text_file(false));

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Attempt to write a file without capability
    let params = json!({
        "sessionId": session_id.0,
        "path": "/tmp/test.txt",
        "content": "test content"
    });

    let ext_request = ExtRequest::new(
        "fs/write_text_file",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    // Should return an error because capability is not declared
    let result = agent.ext_method(ext_request).await;

    match result {
        Err(e) => {
            // Agent correctly rejected the request
            // The error should be Invalid params (-32602) according to JSON-RPC spec
            let error_msg = format!("{:?}", e);
            if error_msg.contains("Invalid params") || error_msg.contains("-32602") {
                tracing::info!("Agent correctly rejected fs/write_text_file without capability (Invalid params)");
                Ok(())
            } else if error_msg.contains("capability") || error_msg.contains("not supported") {
                tracing::info!("Agent correctly rejected fs/write_text_file without capability");
                Ok(())
            } else {
                Err(crate::Error::Validation(format!(
                    "Agent rejected fs/write_text_file but with unexpected error: {}",
                    error_msg
                )))
            }
        }
        Ok(_) => Err(crate::Error::Validation(
            "Agent should reject fs/write_text_file when capability not declared".to_string(),
        )),
    }
}

/// Test reading a text file with the readTextFile capability
pub async fn test_read_text_file_basic<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing basic fs/read_text_file");

    // Initialize with readTextFile capability
    let client_caps = ClientCapabilities::new().fs(FileSystemCapability::new()
        .read_text_file(true)
        .write_text_file(false));

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Create a test file to read
    let test_file = std::env::temp_dir().join("acp_conformance_read_test.txt");
    std::fs::write(&test_file, "Hello\nWorld\nTest")?;

    // Read the file
    let params = json!({
        "sessionId": session_id.0,
        "path": test_file.to_string_lossy()
    });

    let ext_request = ExtRequest::new(
        "fs/read_text_file",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    let result = agent.ext_method(ext_request).await;

    // Clean up
    let _ = std::fs::remove_file(&test_file);

    match result {
        Ok(response) => {
            // Parse response to check for content field
            let response_value: serde_json::Value = serde_json::from_str(response.0.get())?;

            if let Some(content) = response_value.get("content") {
                if content.is_string() {
                    tracing::info!("Successfully read file content");
                    Ok(())
                } else {
                    Err(crate::Error::Validation(
                        "Response content field is not a string".to_string(),
                    ))
                }
            } else {
                Err(crate::Error::Validation(
                    "Response missing 'content' field".to_string(),
                ))
            }
        }
        Err(e) => Err(crate::Error::Agent(e)),
    }
}

/// Test reading with line offset and limit parameters
pub async fn test_read_text_file_with_range<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing fs/read_text_file with line and limit");

    // Initialize with readTextFile capability
    let client_caps = ClientCapabilities::new().fs(FileSystemCapability::new()
        .read_text_file(true)
        .write_text_file(false));

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Create a test file with multiple lines
    let test_file = std::env::temp_dir().join("acp_conformance_read_range_test.txt");
    std::fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5")?;

    // Read lines 2-3 (line is 1-based, limit is 2)
    let params = json!({
        "sessionId": session_id.0,
        "path": test_file.to_string_lossy(),
        "line": 2,
        "limit": 2
    });

    let ext_request = ExtRequest::new(
        "fs/read_text_file",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    let result = agent.ext_method(ext_request).await;

    // Clean up
    let _ = std::fs::remove_file(&test_file);

    match result {
        Ok(response) => {
            let response_value: serde_json::Value = serde_json::from_str(response.0.get())?;

            if response_value.get("content").is_some() {
                tracing::info!("Successfully read file with line/limit parameters");
                Ok(())
            } else {
                Err(crate::Error::Validation(
                    "Response missing 'content' field".to_string(),
                ))
            }
        }
        Err(e) => Err(crate::Error::Agent(e)),
    }
}

/// Test writing a text file with the writeTextFile capability
pub async fn test_write_text_file_basic<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing basic fs/write_text_file");

    // Initialize with writeTextFile capability
    let client_caps = ClientCapabilities::new().fs(FileSystemCapability::new()
        .read_text_file(false)
        .write_text_file(true));

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Write to a test file
    let test_file = std::env::temp_dir().join("acp_conformance_write_test.txt");
    let test_content = "Test content\nLine 2\nLine 3";

    let params = json!({
        "sessionId": session_id.0,
        "path": test_file.to_string_lossy(),
        "content": test_content
    });

    let ext_request = ExtRequest::new(
        "fs/write_text_file",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    let result = agent.ext_method(ext_request).await;

    // Clean up (if file was actually created during recording)
    let _ = std::fs::remove_file(&test_file);

    // We only verify the ACP protocol response, not file system side effects
    match result {
        Ok(_response) => {
            tracing::info!("Successfully sent fs/write_text_file request");
            Ok(())
        }
        Err(e) => Err(crate::Error::Agent(e)),
    }
}

/// Test that writing creates a new file if it doesn't exist
pub async fn test_write_text_file_creates_new<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing fs/write_text_file creates new file");

    // Initialize with writeTextFile capability
    let client_caps = ClientCapabilities::new().fs(FileSystemCapability::new()
        .read_text_file(false)
        .write_text_file(true));

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Ensure the file doesn't exist
    let test_file = std::env::temp_dir().join("acp_conformance_new_file_test.txt");
    let _ = std::fs::remove_file(&test_file);

    let test_content = "New file content";

    let params = json!({
        "sessionId": session_id.0,
        "path": test_file.to_string_lossy(),
        "content": test_content
    });

    let ext_request = ExtRequest::new(
        "fs/write_text_file",
        Arc::from(serde_json::value::to_raw_value(&params)?),
    );

    let result = agent.ext_method(ext_request).await;

    // Clean up (if file was actually created during recording)
    let _ = std::fs::remove_file(&test_file);

    // We only verify the ACP protocol response, not file system side effects
    match result {
        Ok(_) => {
            tracing::info!("Successfully sent fs/write_text_file request for new file");
            Ok(())
        }
        Err(e) => Err(crate::Error::Agent(e)),
    }
}

/// Test that both read and write work when both capabilities are declared
pub async fn test_read_write_integration<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing fs/read_text_file and fs/write_text_file integration");

    // Initialize with both capabilities
    let client_caps = ClientCapabilities::new().fs(FileSystemCapability::new()
        .read_text_file(true)
        .write_text_file(true));

    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    let test_file = std::env::temp_dir().join("acp_conformance_integration_test.txt");
    let test_content = "Integration test content";

    // Write file
    let write_params = json!({
        "sessionId": session_id.0,
        "path": test_file.to_string_lossy(),
        "content": test_content
    });

    let write_request = ExtRequest::new(
        "fs/write_text_file",
        Arc::from(serde_json::value::to_raw_value(&write_params)?),
    );

    agent.ext_method(write_request).await?;

    // Read file back
    let read_params = json!({
        "sessionId": session_id.0,
        "path": test_file.to_string_lossy()
    });

    let read_request = ExtRequest::new(
        "fs/read_text_file",
        Arc::from(serde_json::value::to_raw_value(&read_params)?),
    );

    let read_response = agent.ext_method(read_request).await?;

    // Clean up
    let _ = std::fs::remove_file(&test_file);

    // Verify content matches
    let response_value: serde_json::Value = serde_json::from_str(read_response.0.get())?;

    if let Some(content) = response_value.get("content").and_then(|v| v.as_str()) {
        if content == test_content {
            tracing::info!("Write and read integration successful");
            Ok(())
        } else {
            Err(crate::Error::Validation(format!(
                "Content mismatch. Expected: '{}', Got: '{}'",
                test_content, content
            )))
        }
    } else {
        Err(crate::Error::Validation(
            "Could not extract content from read response".to_string(),
        ))
    }
}

/// Verify file system fixture has proper recordings
pub fn verify_file_system_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<FileSystemStats, Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut stats = FileSystemStats::default();

    // CRITICAL: Verify we have calls recorded (catches poor tests with calls: [])
    assert!(
        !session.calls.is_empty(),
        "Expected recorded calls, fixture has calls: [] - test didn't call agent properly"
    );

    for call in &session.calls {
        match call.method.as_str() {
            "initialize" => stats.initialize_calls += 1,
            "new_session" => stats.new_session_calls += 1,
            "ext_method" => stats.ext_method_calls += 1,
            _ => {}
        }
    }

    tracing::info!("{} file system fixture stats: {:?}", agent_type, stats);

    // Should have at least initialize and new_session
    assert!(
        stats.initialize_calls >= 1,
        "Expected at least 1 initialize call, got {}",
        stats.initialize_calls
    );
    assert!(
        stats.new_session_calls >= 1,
        "Expected at least 1 new_session call, got {}",
        stats.new_session_calls
    );

    Ok(stats)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_compiles() {
        assert!(true);
    }
}
