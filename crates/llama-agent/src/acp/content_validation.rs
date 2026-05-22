//! Content capability validation for ACP prompt requests.
//!
//! This module validates [`ContentBlock`]s in a `session/prompt` request
//! against the [`PromptCapabilities`] the agent advertised in its `initialize`
//! response. ACP requires that an agent reject content types it has declared
//! unsupported rather than silently mishandling them.
//!
//! llama-agent advertises `image: false`, `audio: false`, and
//! `embedded_context: false` (it only supports text content). This validator
//! enforces that contract: any image, audio, or embedded-resource block in a
//! prompt is rejected with a structured ACP error.
//!
//! This is the llama-agent counterpart to claude-agent's
//! `ContentCapabilityValidator`. Both agents now reject exactly the content
//! types they advertise as unsupported, *and* report violations in the same
//! shape: a single bad block yields one `UnsupportedContentType` error, while a
//! prompt with several bad blocks yields a `MultipleViolations` error carrying
//! every violation — so a client observes consistent behavior from either
//! agent in both the single- and multi-violation cases.

use agent_client_protocol::schema::{ContentBlock, PromptCapabilities};

/// Error produced when a prompt contains content the agent does not support.
///
/// Mirrors claude-agent's `ContentCapabilityError` shape so a client observes
/// an identical error contract from either agent:
///
/// * [`UnsupportedContentType`](ContentCapabilityError::UnsupportedContentType)
///   — a single offending content block.
/// * [`MultipleViolations`](ContentCapabilityError::MultipleViolations) — a
///   prompt with more than one offending block; carries every violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentCapabilityError {
    /// A single content block of a type the agent does not support.
    UnsupportedContentType {
        /// The unsupported content type (e.g. `"image"`).
        content_type: String,
        /// The `promptCapabilities` field that would have to be `true` to
        /// accept this content type.
        required_capability: String,
        /// Content types the agent currently accepts, given its capabilities.
        supported_types: Vec<String>,
    },

    /// A prompt with more than one offending content block.
    ///
    /// Carries every individual violation so the client can correct the
    /// request in one round trip. Each entry is an
    /// [`UnsupportedContentType`](ContentCapabilityError::UnsupportedContentType).
    MultipleViolations {
        /// Number of violations — always equal to `violations.len()`.
        violation_count: usize,
        /// The individual violations, in content-block order.
        violations: Vec<ContentCapabilityError>,
    },
}

impl ContentCapabilityError {
    /// Render the error as an ACP-compliant JSON-RPC error.
    ///
    /// Built through [`crate::acp::acp_error::invalid_params`] so the code
    /// (`-32602`, Invalid params) is named rather than a raw integer. The
    /// structured `data` shape matches claude-agent's
    /// `ContentCapabilityError::to_acp_error`, so a client sees an identical
    /// error contract from both agents:
    ///
    /// * `UnsupportedContentType` → `data` carries `contentType`,
    ///   `declaredCapability`, `required`, and `supportedTypes`.
    /// * `MultipleViolations` → `data` carries `violationCount` and a
    ///   `violations` array of the nested per-block error payloads.
    pub fn to_acp_error(&self) -> agent_client_protocol::Error {
        match self {
            ContentCapabilityError::UnsupportedContentType {
                content_type,
                required_capability,
                supported_types,
            } => crate::acp::acp_error::invalid_params(format!(
                "Invalid content type: agent does not support {} content",
                content_type
            ))
            .data(serde_json::json!({
                "contentType": content_type,
                "declaredCapability": false,
                "required": required_capability,
                "supportedTypes": supported_types,
            })),

            ContentCapabilityError::MultipleViolations {
                violation_count,
                violations,
            } => crate::acp::acp_error::invalid_params(format!(
                "Multiple content capability violations: {} issues",
                violation_count
            ))
            .data(serde_json::json!({
                "violationCount": violation_count,
                "violations": violations
                    .iter()
                    .map(ContentCapabilityError::to_acp_error_data)
                    .collect::<Vec<_>>(),
            })),
        }
    }

    /// The JSON-RPC error object as a plain [`serde_json::Value`].
    ///
    /// Used to embed each nested violation inside a `MultipleViolations`
    /// payload, matching claude-agent's `to_acp_error` recursion. The shape
    /// (`code` / `message` / `data`) mirrors a full JSON-RPC error object.
    fn to_acp_error_data(&self) -> serde_json::Value {
        let acp = self.to_acp_error();
        serde_json::json!({
            "code": i32::from(acp.code),
            "message": acp.message,
            "data": acp.data,
        })
    }
}

/// Validates prompt content blocks against advertised prompt capabilities.
///
/// Construct with the exact [`PromptCapabilities`] advertised in `initialize`
/// so the validator enforces precisely what was advertised.
pub struct ContentCapabilityValidator {
    prompt_capabilities: PromptCapabilities,
}

impl ContentCapabilityValidator {
    /// Create a validator for the given advertised prompt capabilities.
    pub fn new(prompt_capabilities: PromptCapabilities) -> Self {
        Self {
            prompt_capabilities,
        }
    }

    /// Validate a single content block.
    ///
    /// Text and resource-link blocks are always allowed (ACP baseline). Image,
    /// audio, and embedded-resource blocks are allowed only when the matching
    /// capability is advertised; otherwise an
    /// [`UnsupportedContentType`](ContentCapabilityError::UnsupportedContentType)
    /// error is returned.
    pub fn validate_content_block(
        &self,
        content: &ContentBlock,
    ) -> Result<(), ContentCapabilityError> {
        match content {
            // Text and resource links are baseline ACP content — always allowed.
            ContentBlock::Text(_) | ContentBlock::ResourceLink(_) => Ok(()),

            ContentBlock::Image(_) => self.require(
                self.prompt_capabilities.image,
                "image",
                "promptCapabilities.image",
            ),

            ContentBlock::Audio(_) => self.require(
                self.prompt_capabilities.audio,
                "audio",
                "promptCapabilities.audio",
            ),

            ContentBlock::Resource(_) => self.require(
                self.prompt_capabilities.embedded_context,
                "resource",
                "promptCapabilities.embeddedContext",
            ),

            // `ContentBlock` is `#[non_exhaustive]`; any future variant is
            // rejected rather than silently mishandled.
            _ => Err(ContentCapabilityError::UnsupportedContentType {
                content_type: "unknown".to_string(),
                required_capability: "none".to_string(),
                supported_types: self.supported_content_types(),
            }),
        }
    }

    /// Validate every content block in a prompt.
    ///
    /// Collects *all* violations rather than stopping at the first, mirroring
    /// claude-agent's `ContentCapabilityValidator::validate_content_blocks`:
    ///
    /// * no violations → `Ok(())`.
    /// * exactly one violation → that single
    ///   [`UnsupportedContentType`](ContentCapabilityError::UnsupportedContentType).
    /// * more than one → a
    ///   [`MultipleViolations`](ContentCapabilityError::MultipleViolations)
    ///   carrying every violation.
    pub fn validate_content_blocks(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<(), ContentCapabilityError> {
        let violations: Vec<ContentCapabilityError> = content_blocks
            .iter()
            .filter_map(|block| self.validate_content_block(block).err())
            .collect();

        match violations.len() {
            0 => Ok(()),
            // Exactly one bad block — report it directly, byte-identical to a
            // single-block rejection.
            1 => Err(violations.into_iter().next().expect("len checked == 1")),
            count => Err(ContentCapabilityError::MultipleViolations {
                violation_count: count,
                violations,
            }),
        }
    }

    /// Allow the content type when `enabled`, otherwise build a capability error.
    fn require(
        &self,
        enabled: bool,
        content_type: &str,
        required_capability: &str,
    ) -> Result<(), ContentCapabilityError> {
        if enabled {
            Ok(())
        } else {
            Err(ContentCapabilityError::UnsupportedContentType {
                content_type: content_type.to_string(),
                required_capability: required_capability.to_string(),
                supported_types: self.supported_content_types(),
            })
        }
    }

    /// List the content types currently accepted, given the capabilities.
    fn supported_content_types(&self) -> Vec<String> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::{AudioContent, ImageContent, ResourceLink, TextContent};

    /// Capabilities matching what llama-agent actually advertises: text only.
    fn text_only_capabilities() -> PromptCapabilities {
        PromptCapabilities::new()
            .image(false)
            .audio(false)
            .embedded_context(false)
    }

    fn text_block() -> ContentBlock {
        ContentBlock::Text(TextContent::new("hello"))
    }

    fn image_block() -> ContentBlock {
        const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        ContentBlock::Image(ImageContent::new(PNG, "image/png"))
    }

    fn audio_block() -> ContentBlock {
        const WAV: &str = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";
        ContentBlock::Audio(AudioContent::new(WAV, "audio/wav"))
    }

    fn resource_block() -> ContentBlock {
        let resource = serde_json::json!({
            "uri": "https://example.com/resource",
            "mimeType": "text/plain",
            "text": "embedded content"
        });
        ContentBlock::Resource(agent_client_protocol::schema::EmbeddedResource::new(
            serde_json::from_value(resource).unwrap(),
        ))
    }

    /// Extract the fields of an `UnsupportedContentType` error, panicking if the
    /// error is any other variant.
    fn expect_unsupported(err: &ContentCapabilityError) -> (&str, &str, &[String]) {
        match err {
            ContentCapabilityError::UnsupportedContentType {
                content_type,
                required_capability,
                supported_types,
            } => (content_type, required_capability, supported_types),
            other => panic!("expected UnsupportedContentType, got {other:?}"),
        }
    }

    #[test]
    fn text_content_always_allowed() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        assert!(validator.validate_content_block(&text_block()).is_ok());
    }

    #[test]
    fn resource_link_always_allowed() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        let block = ContentBlock::ResourceLink(ResourceLink::new("file:///x", "x"));
        assert!(validator.validate_content_block(&block).is_ok());
    }

    #[test]
    fn image_rejected_when_capability_disabled() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        let err = validator
            .validate_content_block(&image_block())
            .expect_err("image must be rejected when image capability is false");
        let (content_type, required, supported) = expect_unsupported(&err);
        assert_eq!(content_type, "image");
        assert_eq!(required, "promptCapabilities.image");
        assert_eq!(supported, &["text", "resource_link"]);
    }

    #[test]
    fn audio_rejected_when_capability_disabled() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        let err = validator
            .validate_content_block(&audio_block())
            .expect_err("audio must be rejected when audio capability is false");
        let (content_type, _, _) = expect_unsupported(&err);
        assert_eq!(content_type, "audio");
    }

    #[test]
    fn resource_rejected_when_capability_disabled() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        let err = validator
            .validate_content_block(&resource_block())
            .expect_err("resource must be rejected when embedded_context is false");
        let (content_type, _, _) = expect_unsupported(&err);
        assert_eq!(content_type, "resource");
    }

    #[test]
    fn image_allowed_when_capability_enabled() {
        let caps = PromptCapabilities::new()
            .image(true)
            .audio(false)
            .embedded_context(false);
        let validator = ContentCapabilityValidator::new(caps);
        assert!(validator.validate_content_block(&image_block()).is_ok());
    }

    #[test]
    fn validate_blocks_single_violation_reported_directly() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        // Exactly one bad block among otherwise-acceptable text.
        let blocks = vec![text_block(), image_block(), text_block()];
        let err = validator
            .validate_content_blocks(&blocks)
            .expect_err("a prompt with image content must be rejected");
        let (content_type, _, _) = expect_unsupported(&err);
        assert_eq!(content_type, "image");
    }

    #[test]
    fn validate_blocks_multiple_violations_collected() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        // Two bad blocks — must be reported together as MultipleViolations.
        let blocks = vec![text_block(), image_block(), audio_block()];
        let err = validator
            .validate_content_blocks(&blocks)
            .expect_err("a prompt with multiple bad blocks must be rejected");
        match err {
            ContentCapabilityError::MultipleViolations {
                violation_count,
                violations,
            } => {
                assert_eq!(violation_count, 2);
                assert_eq!(violations.len(), 2);
                let (first, _, _) = expect_unsupported(&violations[0]);
                let (second, _, _) = expect_unsupported(&violations[1]);
                assert_eq!(first, "image");
                assert_eq!(second, "audio");
            }
            other => panic!("expected MultipleViolations, got {other:?}"),
        }
    }

    #[test]
    fn validate_blocks_all_text_ok() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        let blocks = vec![text_block(), text_block()];
        assert!(validator.validate_content_blocks(&blocks).is_ok());
    }

    #[test]
    fn acp_error_has_invalid_params_code_and_structured_data() {
        let err = ContentCapabilityError::UnsupportedContentType {
            content_type: "image".to_string(),
            required_capability: "promptCapabilities.image".to_string(),
            supported_types: vec!["text".to_string(), "resource_link".to_string()],
        };
        let acp = err.to_acp_error();
        assert_eq!(acp.code, agent_client_protocol::ErrorCode::from(-32602));
        let data = acp.data.expect("error must carry structured data");
        assert_eq!(data["contentType"], "image");
        assert_eq!(data["declaredCapability"], false);
        assert_eq!(data["required"], "promptCapabilities.image");
    }

    #[test]
    fn multiple_violations_acp_error_carries_every_violation() {
        let validator = ContentCapabilityValidator::new(text_only_capabilities());
        let blocks = vec![image_block(), audio_block()];
        let err = validator
            .validate_content_blocks(&blocks)
            .expect_err("multiple bad blocks must be rejected");
        let acp = err.to_acp_error();
        assert_eq!(acp.code, agent_client_protocol::ErrorCode::from(-32602));
        let data = acp.data.expect("error must carry structured data");
        assert_eq!(data["violationCount"], 2);
        let violations = data["violations"]
            .as_array()
            .expect("violations must be an array");
        assert_eq!(violations.len(), 2);
        // Each nested violation is a full JSON-RPC error object.
        assert_eq!(violations[0]["code"], -32602);
        assert_eq!(violations[0]["data"]["contentType"], "image");
        assert_eq!(violations[1]["data"]["contentType"], "audio");
    }
}
