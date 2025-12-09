//! Content capability validation for ACP prompt capabilities compliance
//!
//! This module provides validation of content blocks against declared prompt capabilities
//! ensuring ACP compliance and proper error reporting for capability violations.

use agent_client_protocol::{ContentBlock, PromptCapabilities};
use serde_json::{json, Value};
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during content capability validation
#[derive(Debug, Error, Clone)]
pub enum ContentCapabilityError {
    #[error("Invalid content type: agent does not support {content_type} content")]
    UnsupportedContentType {
        content_type: String,
        declared_capability: bool,
        required_capability: String,
        supported_types: Vec<String>,
    },

    #[error("Content validation failed: {reason}")]
    ValidationFailed { reason: String },

    #[error("Multiple content capability violations: {violation_count} issues")]
    MultipleViolations {
        violation_count: usize,
        violations: Vec<ContentCapabilityError>,
    },
}

impl ContentCapabilityError {
    /// Convert to ACP-compliant JSON-RPC error with structured data
    pub fn to_acp_error(&self) -> Value {
        match self {
            ContentCapabilityError::UnsupportedContentType {
                content_type,
                declared_capability,
                required_capability,
                supported_types,
            } => json!({
                "code": -32602,
                "message": format!("Invalid content type: agent does not support {} content", content_type),
                "data": {
                    "contentType": content_type,
                    "declaredCapability": declared_capability,
                    "required": required_capability,
                    "supportedTypes": supported_types
                }
            }),
            ContentCapabilityError::ValidationFailed { reason } => json!({
                "code": -32602,
                "message": format!("Content validation failed: {}", reason),
                "data": {
                    "reason": reason
                }
            }),
            ContentCapabilityError::MultipleViolations {
                violation_count,
                violations,
            } => json!({
                "code": -32602,
                "message": format!("Multiple content capability violations: {} issues", violation_count),
                "data": {
                    "violationCount": violation_count,
                    "violations": violations.iter().map(|v| v.to_acp_error()).collect::<Vec<_>>()
                }
            }),
        }
    }
}

/// Content capability validator for ACP compliance
pub struct ContentCapabilityValidator {
    prompt_capabilities: PromptCapabilities,
}

impl ContentCapabilityValidator {
    /// Create a new content capability validator
    pub fn new(prompt_capabilities: PromptCapabilities) -> Self {
        Self {
            prompt_capabilities,
        }
    }

    /// Validate a single content block against declared capabilities
    pub fn validate_content_block(
        &self,
        content: &ContentBlock,
    ) -> Result<(), ContentCapabilityError> {
        debug!(
            "Validating content block type: {:?}",
            std::mem::discriminant(content)
        );

        match content {
            // ACP requires strict content validation against declared capabilities:
            // 1. Text and ResourceLink: Always supported (baseline)
            // 2. Image: Only if promptCapabilities.image: true
            // 3. Audio: Only if promptCapabilities.audio: true
            // 4. Resource: Only if promptCapabilities.embedded_context: true
            //
            // This prevents protocol violations and ensures capability contract compliance.
            ContentBlock::Text(_) => {
                // Text content is always allowed (baseline ACP requirement)
                debug!("Text content always allowed");
                Ok(())
            }

            ContentBlock::ResourceLink(_) => {
                // Resource link content is always allowed (baseline ACP requirement)
                debug!("ResourceLink content always allowed");
                Ok(())
            }

            ContentBlock::Image(_) => {
                if self.prompt_capabilities.image {
                    debug!("Image content allowed - capability enabled");
                    Ok(())
                } else {
                    warn!("Image content blocked - capability not enabled");
                    Err(ContentCapabilityError::UnsupportedContentType {
                        content_type: "image".to_string(),
                        declared_capability: false,
                        required_capability: "promptCapabilities.image".to_string(),
                        supported_types: self.get_supported_content_types(),
                    })
                }
            }

            ContentBlock::Audio(_) => {
                if self.prompt_capabilities.audio {
                    debug!("Audio content allowed - capability enabled");
                    Ok(())
                } else {
                    warn!("Audio content blocked - capability not enabled");
                    Err(ContentCapabilityError::UnsupportedContentType {
                        content_type: "audio".to_string(),
                        declared_capability: false,
                        required_capability: "promptCapabilities.audio".to_string(),
                        supported_types: self.get_supported_content_types(),
                    })
                }
            }

            ContentBlock::Resource(_) => {
                if self.prompt_capabilities.embedded_context {
                    debug!("Resource content allowed - embedded context capability enabled");
                    Ok(())
                } else {
                    warn!("Resource content blocked - embedded context capability not enabled");
                    Err(ContentCapabilityError::UnsupportedContentType {
                        content_type: "resource".to_string(),
                        declared_capability: false,
                        required_capability: "promptCapabilities.embeddedContext".to_string(),
                        supported_types: self.get_supported_content_types(),
                    })
                }
            }
        }
    }

    /// Validate an array of content blocks against declared capabilities
    pub fn validate_content_blocks(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<(), ContentCapabilityError> {
        let mut violations = Vec::new();

        // Check each content block
        for (index, content_block) in content_blocks.iter().enumerate() {
            if let Err(violation) = self.validate_content_block(content_block) {
                warn!(
                    "Content capability validation failed for block {}: {}",
                    index, violation
                );
                violations.push(violation);
            }
        }

        // Return error if any violations occurred
        if !violations.is_empty() {
            if violations.len() == 1 {
                return Err(violations.into_iter().next().unwrap());
            } else {
                return Err(ContentCapabilityError::MultipleViolations {
                    violation_count: violations.len(),
                    violations,
                });
            }
        }

        debug!("All content blocks passed capability validation");
        Ok(())
    }

    /// Get list of currently supported content types based on capabilities
    fn get_supported_content_types(&self) -> Vec<String> {
        let mut supported = vec!["text".to_string(), "resource_link".to_string()];

        if self.prompt_capabilities.image {
            supported.push("image".to_string());
        }

        if self.prompt_capabilities.audio {
            supported.push("audio".to_string());
        }

        if self.prompt_capabilities.embedded_context {
            supported.push("resource".to_string());
        }

        supported
    }

    /// Get the underlying prompt capabilities
    pub fn prompt_capabilities(&self) -> &PromptCapabilities {
        &self.prompt_capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::EmbeddedResource;

    fn create_test_capabilities(
        image: bool,
        audio: bool,
        embedded_context: bool,
    ) -> PromptCapabilities {
        PromptCapabilities {
            image,
            audio,
            embedded_context,
            meta: None,
        }
    }

    // Helper functions to create test content blocks
    mod content_blocks {
        use agent_client_protocol::{
            AudioContent, ContentBlock, ImageContent, ResourceLink, TextContent,
        };

        pub fn text(content: &str) -> ContentBlock {
            ContentBlock::Text(TextContent {
                text: content.to_string(),
                annotations: None,
                meta: None,
            })
        }

        pub fn image(mime_type: &str, data: &str) -> ContentBlock {
            ContentBlock::Image(ImageContent {
                data: data.to_string(),
                mime_type: mime_type.to_string(),
                uri: None,
                annotations: None,
                meta: None,
            })
        }

        pub fn image_png() -> ContentBlock {
            const VALID_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
            image("image/png", VALID_PNG_BASE64)
        }

        pub fn audio(mime_type: &str, data: &str) -> ContentBlock {
            ContentBlock::Audio(AudioContent {
                data: data.to_string(),
                mime_type: mime_type.to_string(),
                annotations: None,
                meta: None,
            })
        }

        pub fn audio_wav() -> ContentBlock {
            const VALID_WAV_BASE64: &str =
                "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";
            audio("audio/wav", VALID_WAV_BASE64)
        }

        pub fn resource_link_full(
            uri: &str,
            name: &str,
            description: &str,
            mime_type: &str,
            title: &str,
            size_bytes: u64,
        ) -> ContentBlock {
            ContentBlock::ResourceLink(ResourceLink {
                uri: uri.to_string(),
                name: name.to_string(),
                description: Some(description.to_string()),
                mime_type: Some(mime_type.to_string()),
                title: Some(title.to_string()),
                size: Some(size_bytes.try_into().unwrap()),
                annotations: None,
                meta: None,
            })
        }
    }

    #[test]
    fn test_text_content_always_allowed() {
        let capabilities = create_test_capabilities(false, false, false);
        let validator = ContentCapabilityValidator::new(capabilities);
        let content = content_blocks::text("Test text content");

        assert!(validator.validate_content_block(&content).is_ok());
    }

    #[test]
    fn test_resource_link_always_allowed() {
        let capabilities = create_test_capabilities(false, false, false);
        let validator = ContentCapabilityValidator::new(capabilities);
        let content = content_blocks::resource_link_full(
            "https://example.com/resource",
            "Test resource",
            "Test resource description",
            "text/plain",
            "Test Resource",
            1024,
        );

        assert!(validator.validate_content_block(&content).is_ok());
    }

    #[test]
    fn test_image_content_allowed_when_capability_enabled() {
        let capabilities = create_test_capabilities(true, false, false);
        let validator = ContentCapabilityValidator::new(capabilities);
        let content = content_blocks::image_png();

        assert!(validator.validate_content_block(&content).is_ok());
    }

    #[test]
    fn test_image_content_blocked_when_capability_disabled() {
        let capabilities = create_test_capabilities(false, false, false);
        let validator = ContentCapabilityValidator::new(capabilities);
        let content = content_blocks::image_png();

        let result = validator.validate_content_block(&content);
        assert!(result.is_err());

        if let Err(ContentCapabilityError::UnsupportedContentType {
            content_type,
            declared_capability,
            required_capability,
            supported_types,
        }) = result
        {
            assert_eq!(content_type, "image");
            assert!(!declared_capability);
            assert_eq!(required_capability, "promptCapabilities.image");
            assert_eq!(supported_types, vec!["text", "resource_link"]);
        } else {
            panic!("Expected UnsupportedContentType error");
        }
    }

    #[test]
    fn test_audio_content_allowed_when_capability_enabled() {
        let capabilities = create_test_capabilities(false, true, false);
        let validator = ContentCapabilityValidator::new(capabilities);
        let content = content_blocks::audio_wav();

        assert!(validator.validate_content_block(&content).is_ok());
    }

    #[test]
    fn test_audio_content_blocked_when_capability_disabled() {
        let capabilities = create_test_capabilities(false, false, false);
        let validator = ContentCapabilityValidator::new(capabilities);
        let content = content_blocks::audio_wav();

        let result = validator.validate_content_block(&content);
        assert!(result.is_err());

        if let Err(ContentCapabilityError::UnsupportedContentType { content_type, .. }) = result {
            assert_eq!(content_type, "audio");
        } else {
            panic!("Expected UnsupportedContentType error");
        }
    }

    #[test]
    fn test_resource_content_allowed_when_capability_enabled() {
        let capabilities = create_test_capabilities(false, false, true);
        let validator = ContentCapabilityValidator::new(capabilities);
        let resource_data = serde_json::json!({
            "uri": "https://example.com/resource",
            "mimeType": "text/plain",
            "text": "Resource content"
        });
        let embedded_resource = EmbeddedResource {
            resource: serde_json::from_value(resource_data).unwrap(),
            annotations: None,
            meta: None,
        };
        let content = ContentBlock::Resource(embedded_resource);

        assert!(validator.validate_content_block(&content).is_ok());
    }

    #[test]
    fn test_resource_content_blocked_when_capability_disabled() {
        let capabilities = create_test_capabilities(false, false, false);
        let validator = ContentCapabilityValidator::new(capabilities);
        let resource_data = serde_json::json!({
            "uri": "https://example.com/resource",
            "mimeType": "text/plain",
            "text": "Resource content"
        });
        let embedded_resource = EmbeddedResource {
            resource: serde_json::from_value(resource_data).unwrap(),
            annotations: None,
            meta: None,
        };
        let content = ContentBlock::Resource(embedded_resource);

        let result = validator.validate_content_block(&content);
        assert!(result.is_err());

        if let Err(ContentCapabilityError::UnsupportedContentType { content_type, .. }) = result {
            assert_eq!(content_type, "resource");
        } else {
            panic!("Expected UnsupportedContentType error");
        }
    }

    #[test]
    fn test_mixed_content_blocks_validation() {
        let capabilities = create_test_capabilities(true, false, true);
        let validator = ContentCapabilityValidator::new(capabilities);

        let resource_data = serde_json::json!({
            "uri": "https://example.com/resource",
            "mimeType": "text/plain",
            "text": "Resource content"
        });
        let embedded_resource = EmbeddedResource {
            resource: serde_json::from_value(resource_data).unwrap(),
            annotations: None,
            meta: None,
        };
        let content_blocks = vec![
            content_blocks::text("Test text content"),
            content_blocks::resource_link_full(
                "https://example.com/resource",
                "Test resource",
                "Test resource description",
                "text/plain",
                "Test Resource",
                1024,
            ),
            content_blocks::image_png(),
            ContentBlock::Resource(embedded_resource),
        ];

        assert!(validator.validate_content_blocks(&content_blocks).is_ok());
    }

    #[test]
    fn test_mixed_content_blocks_with_violations() {
        let capabilities = create_test_capabilities(false, false, false);
        let validator = ContentCapabilityValidator::new(capabilities);

        let content_blocks = vec![
            content_blocks::text("Test text content"), // Should pass
            content_blocks::image_png(),               // Should fail
            content_blocks::audio_wav(),               // Should fail
        ];

        let result = validator.validate_content_blocks(&content_blocks);
        assert!(result.is_err());

        if let Err(ContentCapabilityError::MultipleViolations {
            violation_count, ..
        }) = result
        {
            assert_eq!(violation_count, 2); // Image and audio violations
        } else {
            panic!("Expected MultipleViolations error");
        }
    }

    #[test]
    fn test_get_supported_content_types() {
        let capabilities = create_test_capabilities(true, false, true);
        let validator = ContentCapabilityValidator::new(capabilities);
        let supported = validator.get_supported_content_types();

        assert!(supported.contains(&"text".to_string()));
        assert!(supported.contains(&"resource_link".to_string()));
        assert!(supported.contains(&"image".to_string()));
        assert!(!supported.contains(&"audio".to_string()));
        assert!(supported.contains(&"resource".to_string()));
    }

    #[test]
    fn test_acp_error_conversion() {
        let error = ContentCapabilityError::UnsupportedContentType {
            content_type: "image".to_string(),
            declared_capability: false,
            required_capability: "promptCapabilities.image".to_string(),
            supported_types: vec!["text".to_string(), "resource_link".to_string()],
        };

        let acp_error = error.to_acp_error();
        assert_eq!(acp_error["code"], -32602);
        assert!(acp_error["message"]
            .as_str()
            .unwrap()
            .contains("image content"));
        assert_eq!(acp_error["data"]["contentType"], "image");
        assert_eq!(acp_error["data"]["declaredCapability"], false);
    }
}
