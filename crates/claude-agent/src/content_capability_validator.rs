//! Content capability validation for ACP prompt capabilities compliance
//!
//! This module provides validation of content blocks against declared prompt capabilities
//! ensuring ACP compliance and proper error reporting for capability violations.

use agent_client_protocol::schema::{ContentBlock, PromptCapabilities};
use serde_json::{json, Value};
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during content capability validation
#[derive(Debug, Error, Clone)]
pub enum ContentCapabilityError {
    /// The prompt carried a content type the client never declared support for.
    #[error("invalid content type: agent does not support {content_type} content")]
    UnsupportedContentType {
        /// Content type the prompt carried, such as `image` or `audio`.
        content_type: String,
        /// Whether the client declared the capability this content type needs.
        declared_capability: bool,
        /// Capability the client must declare, in ACP spelling.
        required_capability: String,
        /// Content types this client may send, given what it declared.
        supported_types: Vec<String>,
    },

    /// A content block failed validation for a reason the payload names.
    #[error("content validation failed: {reason}")]
    ValidationFailed {
        /// What the validator objected to.
        reason: String,
    },

    /// More than one content block in the same prompt failed validation.
    #[error("multiple content capability violations: {violation_count} issues")]
    MultipleViolations {
        /// How many content blocks failed.
        violation_count: usize,
        /// The failure recorded for each rejected content block.
        violations: Vec<ContentCapabilityError>,
    },
}

impl ContentCapabilityError {
    /// Render the error as an ACP-compliant JSON-RPC error.
    ///
    /// Built through [`crate::acp_error::invalid_params`] so the code (`-32602`,
    /// Invalid params) is named rather than a raw integer. The structured `data`
    /// shape is the one every SAH agent emits for this failure class, so a
    /// client sees one error contract no matter which agent answered.
    pub fn to_acp_error(&self) -> agent_client_protocol::Error {
        match self {
            ContentCapabilityError::UnsupportedContentType {
                content_type,
                declared_capability,
                required_capability,
                supported_types,
            } => crate::acp_error::invalid_params(format!(
                "invalid content type: agent does not support {} content",
                content_type
            ))
            .data(json!({
                "contentType": content_type,
                "declaredCapability": declared_capability,
                "required": required_capability,
                "supportedTypes": supported_types
            })),
            ContentCapabilityError::ValidationFailed { reason } => {
                crate::acp_error::invalid_params(format!("content validation failed: {}", reason))
                    .data(json!({ "reason": reason }))
            }
            ContentCapabilityError::MultipleViolations {
                violation_count,
                violations,
            } => crate::acp_error::invalid_params(format!(
                "multiple content capability violations: {} issues",
                violation_count
            ))
            .data(json!({
                "violationCount": violation_count,
                "violations": violations
                    .iter()
                    .map(ContentCapabilityError::to_acp_error_data)
                    .collect::<Vec<_>>()
            })),
        }
    }

    /// The JSON-RPC error object as a plain [`serde_json::Value`].
    ///
    /// Used to embed each nested violation inside a `MultipleViolations`
    /// payload. The shape (`code` / `message` / `data`) mirrors a full JSON-RPC
    /// error object.
    fn to_acp_error_data(&self) -> Value {
        let acp = self.to_acp_error();
        json!({
            "code": i32::from(acp.code),
            "message": acp.message,
            "data": acp.data,
        })
    }
}

/// Content types every ACP client accepts, whatever it declared.
const BASELINE_CONTENT_TYPES: &[&str] = &["text", "resource_link"];

/// One content type that a client must declare before it may send it.
///
/// Everything that differs between the image, audio and embedded resource
/// gates is held here as data, so the gate itself is written once.
struct OptionalCapability {
    /// Content type name reported to the caller.
    content_type: &'static str,
    /// Reads this capability out of what the client declared.
    is_declared: fn(&PromptCapabilities) -> bool,
    /// Capability the client must declare, in ACP spelling.
    required_capability: &'static str,
}

impl OptionalCapability {
    /// Image content, gated on `promptCapabilities.image`.
    const IMAGE: Self = Self {
        content_type: "image",
        is_declared: |capabilities| capabilities.image,
        required_capability: "promptCapabilities.image",
    };

    /// Audio content, gated on `promptCapabilities.audio`.
    const AUDIO: Self = Self {
        content_type: "audio",
        is_declared: |capabilities| capabilities.audio,
        required_capability: "promptCapabilities.audio",
    };

    /// Embedded resource content, gated on
    /// `promptCapabilities.embeddedContext`.
    const RESOURCE: Self = Self {
        content_type: "resource",
        is_declared: |capabilities| capabilities.embedded_context,
        required_capability: "promptCapabilities.embeddedContext",
    };
}

/// Every optional content capability, in the order the validator reports them.
const OPTIONAL_CAPABILITIES: &[OptionalCapability] = &[
    OptionalCapability::IMAGE,
    OptionalCapability::AUDIO,
    OptionalCapability::RESOURCE,
];

/// Content capability validator for ACP compliance
#[derive(Debug)]
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

        // ACP requires strict content validation against declared capabilities:
        // 1. Text and ResourceLink: Always supported (baseline)
        // 2. Image: Only if promptCapabilities.image: true
        // 3. Audio: Only if promptCapabilities.audio: true
        // 4. Resource: Only if promptCapabilities.embedded_context: true
        //
        // This prevents protocol violations and ensures capability contract compliance.
        match content {
            ContentBlock::Text(_) | ContentBlock::ResourceLink(_) => {
                debug!("baseline content type always allowed");
                Ok(())
            }
            ContentBlock::Image(_) => self.check_optional_capability(&OptionalCapability::IMAGE),
            ContentBlock::Audio(_) => self.check_optional_capability(&OptionalCapability::AUDIO),
            ContentBlock::Resource(_) => {
                self.check_optional_capability(&OptionalCapability::RESOURCE)
            }
            _ => {
                warn!("unknown content block type blocked");
                Err(ContentCapabilityError::UnsupportedContentType {
                    content_type: "unknown".to_string(),
                    declared_capability: false,
                    required_capability: "none".to_string(),
                    supported_types: self.supported_content_types(),
                })
            }
        }
    }

    /// Allow content whose capability the client declared, and reject it when
    /// the client did not.
    fn check_optional_capability(
        &self,
        capability: &OptionalCapability,
    ) -> Result<(), ContentCapabilityError> {
        if (capability.is_declared)(&self.prompt_capabilities) {
            debug!(
                content_type = capability.content_type,
                "content allowed, capability declared"
            );
            return Ok(());
        }

        warn!(
            content_type = capability.content_type,
            "content blocked, capability not declared"
        );
        Err(ContentCapabilityError::UnsupportedContentType {
            content_type: capability.content_type.to_string(),
            declared_capability: false,
            required_capability: capability.required_capability.to_string(),
            supported_types: self.supported_content_types(),
        })
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

    /// The content types this client may send: the baseline types, plus every
    /// optional capability it declared.
    fn supported_content_types(&self) -> Vec<String> {
        BASELINE_CONTENT_TYPES
            .iter()
            .map(|name| (*name).to_string())
            .chain(
                OPTIONAL_CAPABILITIES
                    .iter()
                    .filter(|capability| (capability.is_declared)(&self.prompt_capabilities))
                    .map(|capability| capability.content_type.to_string()),
            )
            .collect()
    }

    /// Get the underlying prompt capabilities
    pub fn prompt_capabilities(&self) -> &PromptCapabilities {
        &self.prompt_capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::EmbeddedResource;

    fn create_test_capabilities(
        image_enabled: bool,
        audio_enabled: bool,
        embedded_context_enabled: bool,
    ) -> PromptCapabilities {
        PromptCapabilities::new()
            .image(image_enabled)
            .audio(audio_enabled)
            .embedded_context(embedded_context_enabled)
    }

    // Helper functions to create test content blocks
    mod content_blocks {
        use agent_client_protocol::schema::{
            AudioContent, ContentBlock, ImageContent, ResourceLink, TextContent,
        };

        pub fn text(content: &str) -> ContentBlock {
            ContentBlock::Text(TextContent::new(content))
        }

        pub fn image(mime_type: &str, data: &str) -> ContentBlock {
            ContentBlock::Image(ImageContent::new(data, mime_type))
        }

        pub fn image_png() -> ContentBlock {
            const VALID_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
            image("image/png", VALID_PNG_BASE64)
        }

        pub fn audio(mime_type: &str, data: &str) -> ContentBlock {
            ContentBlock::Audio(AudioContent::new(data, mime_type))
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
            ContentBlock::ResourceLink(
                ResourceLink::new(name, uri)
                    .description(description)
                    .mime_type(mime_type)
                    .title(title)
                    .size(size_bytes as i64),
            )
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
        let embedded_resource =
            EmbeddedResource::new(serde_json::from_value(resource_data).unwrap());
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
        let embedded_resource =
            EmbeddedResource::new(serde_json::from_value(resource_data).unwrap());
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
        let embedded_resource =
            EmbeddedResource::new(serde_json::from_value(resource_data).unwrap());
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
    fn test_supported_content_types() {
        let capabilities = create_test_capabilities(true, false, true);
        let validator = ContentCapabilityValidator::new(capabilities);
        let supported = validator.supported_content_types();

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
        assert_eq!(
            acp_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(acp_error.message.contains("image content"));
        let data = acp_error.data.expect("error data present");
        assert_eq!(data["contentType"], "image");
        assert_eq!(data["declaredCapability"], false);
    }
}
