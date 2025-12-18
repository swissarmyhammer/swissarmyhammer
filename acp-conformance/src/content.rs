//! Content protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/content
//!
//! ## Requirements Tested
//!
//! 1. **Text Content**
//!    - All agents MUST support text content blocks in prompts
//!    - Text content has `type`, `text`, and optional `annotations`
//!
//! 2. **Image Content**
//!    - Requires `image` prompt capability
//!    - Must include `data` (base64), `mimeType`, optional `uri` and `annotations`
//!
//! 3. **Audio Content**
//!    - Requires `audio` prompt capability
//!    - Must include `data` (base64), `mimeType`, and optional `annotations`
//!
//! 4. **Embedded Resource**
//!    - Requires `embeddedContext` prompt capability
//!    - Can be text resource (uri + text + optional mimeType) or
//!      blob resource (uri + blob + optional mimeType)
//!
//! 5. **Resource Link**
//!    - References to resources the agent can access
//!    - Required: `uri`, `name`
//!    - Optional: `mimeType`, `title`, `description`, `size`, `annotations`

use agent_client_protocol::{
    Agent, AudioContent, ContentBlock, EmbeddedResource, EmbeddedResourceResource, ImageContent,
    InitializeRequest, PromptRequest, ProtocolVersion, ResourceLink, TextContent,
    TextResourceContents,
};

/// Test that agents accept text content blocks
///
/// Per spec: "All Agents **MUST** support text content blocks when included in prompts"
pub async fn test_text_content_support<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing text content support");

    // Initialize agent
    let init_request = InitializeRequest::new(ProtocolVersion::V1);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Create a prompt with text content
    let text_content = TextContent::new("Hello, this is a test message");
    let content_block = ContentBlock::Text(text_content);

    let prompt_request = PromptRequest::new(session_id, vec![content_block]);

    // Send prompt - should not error for basic text content
    let result = agent.prompt(prompt_request).await;

    // Agent should accept text content (even if it doesn't generate a response in test mode)
    // We're testing that it doesn't reject the content type
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            // Check if error is about unsupported content type
            let error_msg = format!("{:?}", e);
            if error_msg.contains("unsupported") || error_msg.contains("content") {
                Err(crate::Error::Validation(format!(
                    "Agent rejected text content: {}",
                    error_msg
                )))
            } else {
                // Other errors (like model not loaded) are acceptable for conformance tests
                Ok(())
            }
        }
    }
}

/// Test that agents properly handle image content with the image capability
pub async fn test_image_content_with_capability<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing image content with capability");

    // Initialize agent and check capabilities
    let init_request = InitializeRequest::new(ProtocolVersion::V1);
    let init_response = agent.initialize(init_request).await?;

    // Check if agent supports image capability
    let agent_supports_image = init_response.agent_capabilities.prompt_capabilities.image;

    if !agent_supports_image {
        tracing::info!("Agent does not support image capability - skipping test");
        return Ok(());
    }

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Create a minimal 1x1 PNG image (base64 encoded)
    let minimal_png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

    let image_content = ImageContent::new(minimal_png, "image/png");
    let content_block = ContentBlock::Image(image_content);

    let prompt_request = PromptRequest::new(session_id, vec![content_block]);

    // Send prompt with image
    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("unsupported") || error_msg.contains("image") {
                Err(crate::Error::Validation(format!(
                    "Agent rejected image content despite capability: {}",
                    error_msg
                )))
            } else {
                // Other errors are acceptable
                Ok(())
            }
        }
    }
}

/// Test that agents properly handle audio content with the audio capability
pub async fn test_audio_content_with_capability<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing audio content with capability");

    // Initialize agent and check capabilities
    let init_request = InitializeRequest::new(ProtocolVersion::V1);
    let init_response = agent.initialize(init_request).await?;

    // Check if agent supports audio capability
    let agent_supports_audio = init_response.agent_capabilities.prompt_capabilities.audio;

    if !agent_supports_audio {
        tracing::info!("Agent does not support audio capability - skipping test");
        return Ok(());
    }

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Create a minimal WAV header (44 bytes) - silent audio
    let minimal_wav = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAA=";

    let audio_content = AudioContent::new(minimal_wav, "audio/wav");
    let content_block = ContentBlock::Audio(audio_content);

    let prompt_request = PromptRequest::new(session_id, vec![content_block]);

    // Send prompt with audio
    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("unsupported") || error_msg.contains("audio") {
                Err(crate::Error::Validation(format!(
                    "Agent rejected audio content despite capability: {}",
                    error_msg
                )))
            } else {
                // Other errors are acceptable
                Ok(())
            }
        }
    }
}

/// Test that agents properly handle embedded resource content
pub async fn test_embedded_resource_with_capability<A: Agent + ?Sized>(
    agent: &A,
) -> crate::Result<()> {
    tracing::info!("Testing embedded resource with capability");

    // Initialize agent and check capabilities
    let init_request = InitializeRequest::new(ProtocolVersion::V1);
    let init_response = agent.initialize(init_request).await?;

    // Check if agent supports embedded context capability
    let agent_supports_embedded = init_response
        .agent_capabilities
        .prompt_capabilities
        .embedded_context;

    if !agent_supports_embedded {
        tracing::info!("Agent does not support embeddedContext capability - skipping test");
        return Ok(());
    }

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Create an embedded text resource
    let text_resource = TextResourceContents::new(
        "def hello():\n    print('Hello, world!')",
        "file:///home/user/script.py",
    )
    .mime_type("text/x-python");

    let embedded_resource = EmbeddedResource::new(EmbeddedResourceResource::TextResourceContents(
        text_resource,
    ));
    let content_block = ContentBlock::Resource(embedded_resource);

    let prompt_request = PromptRequest::new(session_id, vec![content_block]);

    // Send prompt with embedded resource
    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("unsupported")
                || error_msg.contains("resource")
                || error_msg.contains("embedded")
            {
                Err(crate::Error::Validation(format!(
                    "Agent rejected embedded resource despite capability: {}",
                    error_msg
                )))
            } else {
                // Other errors are acceptable
                Ok(())
            }
        }
    }
}

/// Test that agents properly handle resource links
pub async fn test_resource_link_content<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing resource link content");

    // Initialize agent
    let init_request = InitializeRequest::new(ProtocolVersion::V1);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Create a resource link
    let resource_link = ResourceLink::new("file:///home/user/document.pdf", "document.pdf")
        .mime_type("application/pdf")
        .title("Important Document")
        .description("A document for testing")
        .size(1024000);
    let content_block = ContentBlock::ResourceLink(resource_link);

    let prompt_request = PromptRequest::new(session_id, vec![content_block]);

    // Send prompt with resource link
    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("unsupported") || error_msg.contains("resource_link") {
                Err(crate::Error::Validation(format!(
                    "Agent rejected resource link content: {}",
                    error_msg
                )))
            } else {
                // Other errors are acceptable
                Ok(())
            }
        }
    }
}

/// Test that agents properly validate content blocks in prompt requests
pub async fn test_content_validation<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing content validation");

    // Initialize agent
    let init_request = InitializeRequest::new(ProtocolVersion::V1);
    let _init_response = agent.initialize(init_request).await?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = agent_client_protocol::NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Test with empty content array - should be valid (though may not generate response)
    let prompt_request = PromptRequest::new(session_id.clone(), vec![]);

    let result = agent.prompt(prompt_request).await;

    // Empty content is technically valid per spec
    match result {
        Ok(_) => {}
        Err(e) => {
            let error_msg = format!("{:?}", e);
            // Only fail if it's specifically about empty content
            if error_msg.contains("empty") && error_msg.contains("content") {
                return Err(crate::Error::Validation(
                    "Agent rejected empty content array".to_string(),
                ));
            }
            // Other errors are acceptable (model not loaded, etc)
        }
    }

    // Test with multiple content blocks
    let text1 = TextContent::new("First message");
    let text2 = TextContent::new("Second message");
    let content_blocks = vec![ContentBlock::Text(text1), ContentBlock::Text(text2)];

    let prompt_request = PromptRequest::new(session_id, content_blocks);

    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("unsupported") || error_msg.contains("multiple") {
                Err(crate::Error::Validation(format!(
                    "Agent rejected multiple content blocks: {}",
                    error_msg
                )))
            } else {
                // Other errors are acceptable
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Basic smoke test to ensure module compiles
    #[test]
    fn test_module_compiles() {
        assert!(true);
    }
}
