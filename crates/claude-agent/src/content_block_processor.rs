//! The stage that turns an ACP content block into something a language model can
//! read.
//!
//! A prompt arrives as a list of
//! [`agent_client_protocol::schema::ContentBlock`] values of mixed kinds — text,
//! image, audio, embedded resource, resource link. A model consumes text, so
//! every kind reduces to one [`ProcessedContent`] carrying a text
//! representation, with the decoded bytes beside it for the binary kinds. The
//! reduction is uniform, so what a prompt means does not depend on which kinds
//! it happened to hold.
//!
//! This module is the top of the content pipeline and owns the order the checks
//! below it run in: declared capability, then block structure, then size, then
//! decoding through [`crate::base64_processor`], then policy through
//! [`crate::content_security_validator`]. A caller gets that order by calling
//! [`ContentBlockProcessor::process_content_block`], and cannot get it wrong.
//!
//! Batch processing adds a second behaviour, and it is optional. With recovery
//! switched off the first bad block fails the whole prompt. With it on, a bad
//! block is retried with exponential backoff, replaced by a placeholder, and
//! counted in the returned [`ContentProcessingSummary`] — so one broken
//! attachment does not discard the rest of the message. If every block fails,
//! the first error is returned instead of an empty summary.

use crate::base64_processor::{Base64Processor, Base64ProcessorError};
use crate::content_security_validator::{ContentSecurityError, ContentSecurityValidator};
use crate::error::ToJsonRpcError;
use crate::json_rpc_codes::{INTERNAL_ERROR, INVALID_PARAMS};
use crate::size_validator::{SizeLimits, SizeValidationError, SizeValidator};
use crate::url_validation;
use agent_client_protocol::schema::{ContentBlock, TextContent};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, warn};
use url::Url;

/// How many extra attempts a failed content block gets during batch recovery.
const MAX_RETRIES: u32 = 3;

/// Milliseconds in a second, the first backoff step.
const MS_PER_SECOND: u64 = 1000;

/// Longest a retry waits, in milliseconds.
const MAX_BACKOFF_MS: u64 = 10_000;

/// Factor the backoff grows by on each further attempt.
const BACKOFF_BASE: u64 = 2;

/// MIME type used when a blob resource declares none.
const DEFAULT_BLOB_MIME_TYPE: &str = "text/plain";

/// A [`Base64Processor`] decode method, as the shared media path passes it
/// around.
type MediaDecoder = fn(&Base64Processor, &str, &str) -> Result<Vec<u8>, Base64ProcessorError>;

/// The limits and switches that decide how a [`ContentBlockProcessor`] treats
/// a content block.
///
/// These settings are named fields rather than positional arguments, so a call
/// site says which switch it sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentValidationConfig {
    /// Largest decoded resource accepted, in bytes.
    pub max_resource_size: usize,
    /// Whether URIs on content blocks are parsed and checked.
    pub enable_uri_validation: bool,
    /// Whether a content kind must be declared before it is processed.
    pub enable_capability_validation: bool,
    /// Which content kinds the agent declares, by capability name.
    pub supported_capabilities: HashMap<String, bool>,
    /// Whether a failed block is retried and replaced instead of aborting the
    /// batch.
    pub enable_batch_recovery: bool,
}

impl Default for ContentValidationConfig {
    fn default() -> Self {
        let mut supported_capabilities = HashMap::new();
        supported_capabilities.insert("text".to_string(), true);
        supported_capabilities.insert("image".to_string(), true);
        supported_capabilities.insert("audio".to_string(), false); // Disabled by default
        supported_capabilities.insert("resource".to_string(), true);
        supported_capabilities.insert("resource_link".to_string(), true);

        Self {
            max_resource_size: SizeLimits::default().max_content_size,
            enable_uri_validation: true,
            enable_capability_validation: true,
            supported_capabilities,
            enable_batch_recovery: true,
        }
    }
}

impl ContentValidationConfig {
    /// The default settings with the resource size cap and the URI switch set.
    ///
    /// [`ContentBlockProcessor::new`] and
    /// [`ContentBlockProcessor::with_enhanced_security`] both take just these
    /// two settings, and both build their configuration here.
    #[must_use]
    pub fn with_resource_limit(max_resource_size: usize, enable_uri_validation: bool) -> Self {
        Self {
            max_resource_size,
            enable_uri_validation,
            ..Default::default()
        }
    }
}

/// Configuration struct for enhanced security settings
#[derive(Debug, Clone)]
pub struct EnhancedSecurityConfig {
    /// Limits and switches applied to every block.
    pub validation: ContentValidationConfig,
    /// Security validator applied to every block.
    pub content_security_validator: ContentSecurityValidator,
}

/// Why [`ContentBlockProcessor`] refused or failed to process a content block.
///
/// The variants cover structural faults in the block, capability and size
/// policy, URI faults, and failures forwarded from the base64 and security
/// validators.
#[derive(Debug, Error, Clone)]
pub enum ContentBlockProcessorError {
    /// Base64 decoding or checking failed.
    #[error("Base64 processing error: {0}")]
    Base64Error(#[from] Base64ProcessorError),
    /// An embedded resource failed validation. The payload names the fault.
    #[error("resource validation error: {0}")]
    ResourceValidation(String),
    /// A resource link failed validation. The payload names the fault.
    #[error("ResourceLink validation error: {0}")]
    ResourceLinkValidation(String),
    /// The content block is of a kind the processor does not handle.
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
    /// A field the content block must carry is absent. The payload names it.
    #[error("missing required field: {0}")]
    MissingRequiredField(String),
    /// A URI on the content block cannot be parsed.
    #[error("invalid URI format: {0}")]
    InvalidUri(String),
    /// The content is larger than the configured size limit.
    #[error("content size exceeds limit: {actual} > {limit} bytes")]
    ContentSizeExceeded {
        /// Size the content holds, in bytes.
        actual: usize,
        /// Largest accepted size, in bytes.
        limit: usize,
    },
    /// An annotation on the content block is malformed.
    #[error("invalid annotation: {0}")]
    InvalidAnnotation(String),
    /// The agent does not declare the capability this content kind needs.
    #[error("capability not supported: {capability}")]
    CapabilityNotSupported {
        /// Name of the capability the content needs.
        capability: String,
    },
    /// Content-level validation failed. The payload names the fault.
    #[error("content validation failed: {details}")]
    ContentValidationFailed {
        /// What the validator objected to.
        details: String,
    },
    /// The content block does not have the shape its kind requires.
    #[error("invalid content structure: {details}")]
    InvalidContentStructure {
        /// What was wrong with the structure.
        details: String,
    },
    /// Processing needed more memory than the budget allows.
    #[error("memory allocation failed during processing")]
    MemoryAllocationFailed,
    /// Some blocks in a batch failed and recovery could not save the batch.
    #[error("batch processing partially failed: {successful}/{total} items processed")]
    PartialBatchFailure {
        /// Number of blocks processed successfully.
        successful: usize,
        /// Number of blocks in the batch.
        total: usize,
    },
    /// The content behind a resource link could not be fetched.
    #[error("resource link fetch failed: {uri}")]
    ResourceLinkFetchFailed {
        /// URI that could not be fetched.
        uri: String,
    },
    /// The content array as a whole failed validation.
    #[error("content array validation failed: {details}")]
    ContentArrayValidationFailed {
        /// What the validator objected to.
        details: String,
    },
    /// The [`ContentSecurityValidator`] refused the content.
    #[error("content security validation failed: {0}")]
    ContentSecurityValidationFailed(#[from] ContentSecurityError),
}

impl ToJsonRpcError for ContentBlockProcessorError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            Self::Base64Error(base64_error) => base64_error.to_json_rpc_code(),
            Self::ResourceValidation(_)
            | Self::ResourceLinkValidation(_)
            | Self::UnsupportedContentType(_)
            | Self::MissingRequiredField(_)
            | Self::InvalidUri(_)
            | Self::ContentSizeExceeded { .. }
            | Self::InvalidAnnotation(_)
            | Self::CapabilityNotSupported { .. }
            | Self::ContentValidationFailed { .. }
            | Self::InvalidContentStructure { .. }
            | Self::ContentArrayValidationFailed { .. } => INVALID_PARAMS,
            Self::MemoryAllocationFailed
            | Self::PartialBatchFailure { .. }
            | Self::ResourceLinkFetchFailed { .. }
            | Self::ContentSecurityValidationFailed(_) => INTERNAL_ERROR,
        }
    }

    fn to_error_data(&self) -> Option<Value> {
        let data = match self {
            Self::Base64Error(base64_error) => return base64_error.to_error_data(),
            Self::ResourceValidation(details) => json!({
                "error": "resource_validation_failed",
                "details": details,
                "suggestion": "Check resource structure and content format"
            }),
            Self::ResourceLinkValidation(details) => json!({
                "error": "resource_link_validation_failed",
                "details": details,
                "suggestion": "Verify resource link URI and metadata"
            }),
            Self::UnsupportedContentType(content_type) => json!({
                "error": "unsupported_content_type",
                "contentType": content_type,
                "supportedTypes": ["text", "image", "audio", "resource", "resource_link"],
                "suggestion": "Use one of the supported content block types"
            }),
            Self::MissingRequiredField(field) => json!({
                "error": "missing_required_field",
                "field": field,
                "suggestion": "Ensure all required fields are present in content block"
            }),
            Self::InvalidUri(uri) => json!({
                "error": "invalid_uri",
                "uri": uri,
                "suggestion": "Provide a valid URI with proper scheme (http, https, file, etc.)"
            }),
            Self::ContentSizeExceeded { actual, limit } => json!({
                "error": "content_size_exceeded",
                "providedSize": actual,
                "maxSize": limit,
                "suggestion": "Reduce content size or split into smaller parts"
            }),
            Self::InvalidAnnotation(details) => json!({
                "error": "invalid_annotation",
                "details": details,
                "suggestion": "Check annotation format and structure"
            }),
            Self::CapabilityNotSupported { capability } => json!({
                "error": "capability_not_supported",
                "requiredCapability": capability,
                "suggestion": "Check agent capabilities before sending content"
            }),
            Self::ContentValidationFailed { details } => json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format"
            }),
            Self::InvalidContentStructure { details } => json!({
                "error": "invalid_content_structure",
                "details": details,
                "suggestion": "Verify content block follows ACP specification"
            }),
            Self::MemoryAllocationFailed => json!({
                "error": "memory_allocation_failed",
                "suggestion": "Reduce content size or retry later"
            }),
            Self::PartialBatchFailure { successful, total } => json!({
                "error": "partial_batch_failure",
                "successfulItems": successful,
                "totalItems": total,
                "suggestion": "Review individual item errors for details"
            }),
            Self::ResourceLinkFetchFailed { uri } => json!({
                "error": "resource_link_fetch_failed",
                "uri": uri,
                "suggestion": "Verify resource link is accessible"
            }),
            Self::ContentArrayValidationFailed { details } => json!({
                "error": "content_array_validation_failed",
                "details": details,
                "suggestion": "Check content array structure and elements"
            }),
            Self::ContentSecurityValidationFailed(security_error) => {
                return security_error.to_error_data();
            }
        };
        Some(data)
    }
}

impl From<SizeValidationError> for ContentBlockProcessorError {
    fn from(error: SizeValidationError) -> Self {
        match error {
            SizeValidationError::SizeExceeded { actual, limit, .. } => {
                ContentBlockProcessorError::ContentSizeExceeded { actual, limit }
            }
        }
    }
}

/// One content block after [`ContentBlockProcessor`] has decoded and checked
/// it.
///
/// Every block, whatever its kind, yields a text representation a language
/// model can read. Binary kinds also carry their decoded bytes.
#[derive(Debug, Clone)]
pub struct ProcessedContent {
    /// Kind of the block, with the details that kind carries.
    pub content_type: ProcessedContentType,
    /// Text a language model reads in place of the block.
    pub text_representation: String,
    /// Decoded bytes, for the kinds that carry a payload.
    pub binary_data: Option<Vec<u8>>,
    /// Extra facts about the block, such as its MIME type and source URI.
    pub metadata: HashMap<String, String>,
    /// Size of the block as it arrived, in bytes.
    pub size_bytes: usize,
}

/// Declare [`ProcessedContentType`] and its counting keys from one table.
///
/// Each row gives a variant, the fields that variant carries, and the key a
/// [`ContentProcessingSummary`] counts it under. A new content kind is one new
/// row, not a variant here and a matching arm somewhere else that can drift
/// away from it.
macro_rules! processed_content_types {
    (
        $(
            $(#[$variant_doc:meta])*
            $variant:ident $({ $($(#[$field_doc:meta])* $field:ident : $field_type:ty),* $(,)? })?
                => $counting_key:literal
        ),* $(,)?
    ) => {
        /// The kind of a [`ProcessedContent`], with the details that kind carries.
        #[derive(Debug, Clone)]
        pub enum ProcessedContentType {
            $(
                $(#[$variant_doc])*
                $variant $({ $($(#[$field_doc])* $field : $field_type),* })?,
            )*
        }

        impl ProcessedContentType {
            /// The key a [`ContentProcessingSummary`] counts this kind under.
            pub fn counting_key(&self) -> &'static str {
                match self {
                    $( Self::$variant $({ $($field: _),* })? => $counting_key, )*
                }
            }
        }
    };
}

processed_content_types! {
    /// Plain text.
    Text => "text",
    /// An image decoded from base64.
    Image {
        /// MIME type the block declared.
        mime_type: String,
    } => "image",
    /// Audio decoded from base64.
    Audio {
        /// MIME type the block declared.
        mime_type: String,
    } => "audio",
    /// A resource carried inline, as text or as a base64 blob.
    EmbeddedResource {
        /// URI the resource names, when it names one.
        uri: Option<String>,
        /// MIME type the resource declared, when it declared one.
        mime_type: Option<String>,
    } => "resource",
    /// A reference to a resource held elsewhere.
    ResourceLink {
        /// URI the link names.
        uri: String,
    } => "resource_link",
}

/// Running totals gathered while a batch of content blocks is processed.
///
/// Both the strict batch path and the recovering batch path fold their results
/// in here, so the two agree on what a summary holds.
#[derive(Debug, Default)]
struct ContentAccumulator {
    text_content: String,
    has_binary_content: bool,
    processed_contents: Vec<ProcessedContent>,
    total_size: usize,
    content_type_counts: HashMap<String, usize>,
}

impl ContentAccumulator {
    /// Fold one successfully processed block into the running totals.
    ///
    /// `type_key` is the counting key for the block's kind.
    fn accumulate(&mut self, processed: ProcessedContent, type_key: &str) {
        self.text_content.push_str(&processed.text_representation);

        if processed.binary_data.is_some() {
            self.has_binary_content = true;
        }

        self.total_size += processed.size_bytes;
        *self
            .content_type_counts
            .entry(type_key.to_string())
            .or_insert(0) += 1;

        self.processed_contents.push(processed);
    }

    /// Record a placeholder that stands in for a block that failed.
    ///
    /// A placeholder contributes its text but counts toward no kind and adds
    /// no size, because no real content was processed.
    fn accumulate_fallback(&mut self, fallback: ProcessedContent) {
        self.text_content.push_str(&fallback.text_representation);
        self.processed_contents.push(fallback);
    }

    /// Turn the running totals into the summary the caller returns.
    fn into_summary(self) -> ContentProcessingSummary {
        ContentProcessingSummary {
            processed_contents: self.processed_contents,
            combined_text: self.text_content,
            has_binary_content: self.has_binary_content,
            total_size_bytes: self.total_size,
            content_type_counts: self.content_type_counts,
        }
    }
}

// IMPORTANT: Do not add timeouts to content processing operations.
// Content processing should be allowed to complete regardless of size or complexity.
// Timeouts create artificial limitations and poor user experience by interrupting
// legitimate processing of large or complex content. Users cannot predict when
/// Decodes and checks ACP content blocks, one at a time or in batches.
///
/// Each block is validated for structure, for the capability its kind needs
/// and for size, then decoded into a [`ProcessedContent`] carrying a text
/// representation and, for binary kinds, the decoded bytes. A batch either
/// fails on the first bad block or recovers from it, depending on
/// `enable_batch_recovery`.
#[derive(Debug)]
pub struct ContentBlockProcessor {
    base64_processor: Base64Processor,
    enable_uri_validation: bool,
    enable_capability_validation: bool,
    supported_capabilities: HashMap<String, bool>,
    enable_batch_recovery: bool,
    content_security_validator: Option<ContentSecurityValidator>,
    size_validator: SizeValidator,
}

impl Default for ContentBlockProcessor {
    fn default() -> Self {
        Self::from_parts(
            Base64Processor::default(),
            ContentValidationConfig::default(),
            // Default to no enhanced security validation.
            None,
        )
    }
}

impl ContentBlockProcessor {
    /// Build a processor around `base64_processor`.
    ///
    /// `max_resource_size` caps a decoded resource in bytes, and
    /// `enable_uri_validation` turns URI parsing on or off. Every other
    /// setting keeps its [`Default`] value, so batch recovery is on and no
    /// [`ContentSecurityValidator`] is attached.
    pub fn new(
        base64_processor: Base64Processor,
        max_resource_size: usize,
        enable_uri_validation: bool,
    ) -> Self {
        Self::from_parts(
            base64_processor,
            ContentValidationConfig::with_resource_limit(max_resource_size, enable_uri_validation),
            None,
        )
    }

    /// Build a processor from the full settings and the optional security
    /// validator.
    ///
    /// Every constructor, [`Default`] included, lands here, so the size
    /// validator is derived from `config` in exactly one place.
    fn from_parts(
        base64_processor: Base64Processor,
        config: ContentValidationConfig,
        content_security_validator: Option<ContentSecurityValidator>,
    ) -> Self {
        let size_validator = SizeValidator::new(SizeLimits {
            max_content_size: config.max_resource_size,
            ..Default::default()
        });

        Self {
            base64_processor,
            enable_uri_validation: config.enable_uri_validation,
            enable_capability_validation: config.enable_capability_validation,
            supported_capabilities: config.supported_capabilities,
            enable_batch_recovery: config.enable_batch_recovery,
            content_security_validator,
            size_validator,
        }
    }

    /// Build a processor with every limit and switch set explicitly.
    ///
    /// The switches arrive as one named [`ContentValidationConfig`] rather
    /// than as positional booleans. No [`ContentSecurityValidator`] is
    /// attached.
    pub fn new_with_config(
        base64_processor: Base64Processor,
        config: ContentValidationConfig,
    ) -> Self {
        Self::from_parts(base64_processor, config, None)
    }

    /// Build a processor that also runs `content_security_validator` on every
    /// block.
    ///
    /// Every setting other than `max_resource_size` and
    /// `enable_uri_validation` keeps its [`Default`] value.
    pub fn with_enhanced_security(
        base64_processor: Base64Processor,
        max_resource_size: usize,
        enable_uri_validation: bool,
        content_security_validator: ContentSecurityValidator,
    ) -> Self {
        Self::from_parts(
            base64_processor,
            ContentValidationConfig::with_resource_limit(max_resource_size, enable_uri_validation),
            Some(content_security_validator),
        )
    }

    /// Build a processor from a full [`EnhancedSecurityConfig`].
    ///
    /// This is [`ContentBlockProcessor::new_with_config`] with the security
    /// validator attached, taking its arguments as one struct.
    pub fn with_enhanced_security_config(
        base64_processor: Base64Processor,
        config: EnhancedSecurityConfig,
    ) -> Self {
        Self::from_parts(
            base64_processor,
            config.validation,
            Some(config.content_security_validator),
        )
    }

    /// Validate capability is supported
    pub fn validate_capability(&self, capability: &str) -> Result<(), ContentBlockProcessorError> {
        if !self.enable_capability_validation {
            return Ok(());
        }

        match self.supported_capabilities.get(capability) {
            Some(&true) => Ok(()),
            Some(&false) => Err(ContentBlockProcessorError::CapabilityNotSupported {
                capability: capability.to_string(),
            }),
            None => Err(ContentBlockProcessorError::CapabilityNotSupported {
                capability: capability.to_string(),
            }),
        }
    }

    /// Validate content block structure
    pub fn validate_content_block_structure(
        &self,
        content_block: &ContentBlock,
    ) -> Result<(), ContentBlockProcessorError> {
        // Enhanced security validation first if available
        if let Some(ref validator) = self.content_security_validator {
            validator.validate_content_security(content_block)?;
        }

        match content_block {
            ContentBlock::Text(text_content) => {
                if text_content.text.is_empty() {
                    return Err(ContentBlockProcessorError::InvalidContentStructure {
                        details: "Text content cannot be empty".to_string(),
                    });
                }
            }
            ContentBlock::Image(image_content) => {
                if image_content.data.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "data".to_string(),
                    ));
                }
                if image_content.mime_type.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "mime_type".to_string(),
                    ));
                }
            }
            ContentBlock::Audio(audio_content) => {
                self.validate_capability("audio")?;
                if audio_content.data.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "data".to_string(),
                    ));
                }
                if audio_content.mime_type.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "mime_type".to_string(),
                    ));
                }
            }
            ContentBlock::Resource(resource_content) => {
                self.validate_capability("resource")?;
                self.validate_resource_structure(resource_content)?;
            }
            ContentBlock::ResourceLink(resource_link) => {
                self.validate_capability("resource_link")?;
                if resource_link.uri.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "uri".to_string(),
                    ));
                }
            }
            _ => {
                // Unknown or unsupported content block type
                return Err(ContentBlockProcessorError::InvalidContentStructure {
                    details: "Unsupported content block type".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Process a ContentBlock and return structured processed content
    ///
    /// ACP requires support for all 5 ContentBlock types:
    /// 1. Text: Always supported (mandatory)
    /// 2. Image: Base64 data + MIME type validation
    /// 3. Audio: Base64 data + MIME type validation  
    /// 4. Resource: Complex nested structure with text/blob variants
    /// 5. ResourceLink: URI-based resource references with metadata
    ///
    /// Content must be validated against declared prompt capabilities.
    pub fn process_content_block(
        &self,
        content_block: &ContentBlock,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        debug!(
            "Processing content block: {:?}",
            std::mem::discriminant(content_block)
        );

        // Validate content block structure
        self.validate_content_block_structure(content_block)?;

        // Process content block
        self.process_content_block_internal(content_block)
    }

    fn process_content_block_internal(
        &self,
        content_block: &ContentBlock,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        match content_block {
            ContentBlock::Text(text_content) => {
                self.validate_capability("text")?;
                self.process_text_content(text_content)
            }
            ContentBlock::Image(image_content) => {
                self.validate_capability("image")?;
                self.process_image_content(image_content)
            }
            ContentBlock::Audio(audio_content) => self.process_audio_content(audio_content),
            ContentBlock::Resource(resource_content) => {
                self.process_embedded_resource(resource_content)
            }
            ContentBlock::ResourceLink(resource_link) => self.process_resource_link(resource_link),
            _ => {
                // Unknown or unsupported content block type
                Err(ContentBlockProcessorError::InvalidContentStructure {
                    details: "Unsupported content block type".to_string(),
                })
            }
        }
    }

    /// Describe where a payload came from, for a text representation.
    ///
    /// An empty or absent URI means the payload travelled inline.
    fn describe_source(uri: Option<&str>) -> String {
        match uri {
            Some(uri) if !uri.is_empty() => format!(" from {}", uri),
            _ => " (embedded)".to_string(),
        }
    }

    /// Describe a declared MIME type, for a text representation.
    fn describe_mime_type(mime_type: Option<&str>) -> String {
        match mime_type {
            Some(mime_type) => format!(": {}", mime_type),
            None => String::new(),
        }
    }

    /// Decode a base64 media payload and check it against the size limit.
    ///
    /// `decode` is the [`Base64Processor`] method that suits the media kind.
    fn decode_media_payload(
        &self,
        decode: MediaDecoder,
        encoded: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, ContentBlockProcessorError> {
        let decoded_data = decode(&self.base64_processor, encoded, mime_type)?;
        self.size_validator
            .validate_content_size(decoded_data.len())?;
        Ok(decoded_data)
    }

    /// The metadata every decoded payload carries: its MIME type and the size
    /// of the decoded bytes.
    fn decoded_content_metadata(mime_type: &str, decoded_size: usize) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("mime_type".to_string(), mime_type.to_string());
        metadata.insert("data_size".to_string(), decoded_size.to_string());
        metadata
    }

    /// Assemble a decoded media payload into a [`ProcessedContent`].
    ///
    /// `encoded_size` is the size of the payload as it arrived, before
    /// decoding, which is what a block reports as its size.
    fn build_media_content(
        content_type: ProcessedContentType,
        text_representation: String,
        decoded_data: Vec<u8>,
        metadata: HashMap<String, String>,
        encoded_size: usize,
    ) -> ProcessedContent {
        ProcessedContent {
            content_type,
            text_representation,
            binary_data: Some(decoded_data),
            metadata,
            size_bytes: encoded_size,
        }
    }

    /// Decode an image content block and describe it.
    fn process_image_content(
        &self,
        image_content: &agent_client_protocol::schema::ImageContent,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let decoded_data = self.decode_media_payload(
            Base64Processor::decode_image_data,
            &image_content.data,
            &image_content.mime_type,
        )?;

        let mut metadata =
            Self::decoded_content_metadata(&image_content.mime_type, decoded_data.len());

        if let Some(ref uri) = image_content.uri {
            if self.enable_uri_validation {
                self.validate_uri(uri)?;
            }
            metadata.insert("source_uri".to_string(), uri.clone());
        }

        let text_representation = format!(
            "[Image content: {} ({} bytes){}]",
            image_content.mime_type,
            decoded_data.len(),
            Self::describe_source(image_content.uri.as_deref()),
        );

        Ok(Self::build_media_content(
            ProcessedContentType::Image {
                mime_type: image_content.mime_type.clone(),
            },
            text_representation,
            decoded_data,
            metadata,
            image_content.data.len(),
        ))
    }

    /// Decode an audio content block and describe it.
    ///
    /// An audio block carries no URI, so its text representation names no
    /// source.
    fn process_audio_content(
        &self,
        audio_content: &agent_client_protocol::schema::AudioContent,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let decoded_data = self.decode_media_payload(
            Base64Processor::decode_audio_data,
            &audio_content.data,
            &audio_content.mime_type,
        )?;

        let metadata = Self::decoded_content_metadata(&audio_content.mime_type, decoded_data.len());

        let text_representation = format!(
            "[Audio content: {} ({} bytes)]",
            audio_content.mime_type,
            decoded_data.len()
        );

        Ok(Self::build_media_content(
            ProcessedContentType::Audio {
                mime_type: audio_content.mime_type.clone(),
            },
            text_representation,
            decoded_data,
            metadata,
            audio_content.data.len(),
        ))
    }

    /// Dispatch an embedded resource to the text or blob path.
    fn process_embedded_resource(
        &self,
        resource_content: &agent_client_protocol::schema::EmbeddedResource,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        use agent_client_protocol::schema::EmbeddedResourceResource;

        match &resource_content.resource {
            EmbeddedResourceResource::TextResourceContents(text_resource) => {
                self.process_text_resource(text_resource)
            }
            EmbeddedResourceResource::BlobResourceContents(blob_resource) => {
                self.process_blob_resource(blob_resource)
            }
            _ => {
                // Unknown or unsupported resource type
                Err(ContentBlockProcessorError::InvalidContentStructure {
                    details: "Unsupported resource type".to_string(),
                })
            }
        }
    }

    /// Validate an inline resource's URI, then record its URI and MIME type in
    /// its metadata.
    ///
    /// An empty URI is neither validated nor recorded. Validation runs only
    /// when the processor sets `enable_uri_validation`.
    fn validate_and_record_resource_uri(
        &self,
        uri: &str,
        mime_type: Option<&str>,
        metadata: &mut HashMap<String, String>,
    ) -> Result<(), ContentBlockProcessorError> {
        if !uri.is_empty() {
            if self.enable_uri_validation {
                self.validate_uri(uri)?;
            }
            metadata.insert("uri".to_string(), uri.to_string());
        }

        if let Some(mime_type) = mime_type {
            metadata.insert("mime_type".to_string(), mime_type.to_string());
        }

        Ok(())
    }

    /// Render the text representation of an inline resource.
    ///
    /// `resource_type` names the payload kind as the reader sees it, `Text` or
    /// `Blob`.
    fn resource_text_representation(
        resource_type: &str,
        mime_type: Option<&str>,
        uri: &str,
        size_bytes: usize,
    ) -> String {
        format!(
            "[{} Resource{}{}: {} bytes]",
            resource_type,
            Self::describe_mime_type(mime_type),
            Self::describe_source(Some(uri)),
            size_bytes
        )
    }

    /// Check and describe a text resource carried inline.
    fn process_text_resource(
        &self,
        text_resource: &agent_client_protocol::schema::TextResourceContents,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let mut metadata = HashMap::new();
        self.validate_and_record_resource_uri(
            &text_resource.uri,
            text_resource.mime_type.as_deref(),
            &mut metadata,
        )?;

        let size_bytes = text_resource.text.len();
        metadata.insert("resource_type".to_string(), "text".to_string());
        metadata.insert("data_size".to_string(), size_bytes.to_string());

        // Validate size
        self.size_validator.validate_content_size(size_bytes)?;

        let text_representation = Self::resource_text_representation(
            "Text",
            text_resource.mime_type.as_deref(),
            &text_resource.uri,
            size_bytes,
        );

        Ok(ProcessedContent {
            content_type: ProcessedContentType::EmbeddedResource {
                uri: Self::optional_uri(&text_resource.uri),
                mime_type: text_resource.mime_type.clone(),
            },
            text_representation,
            binary_data: None,
            metadata,
            size_bytes,
        })
    }

    /// Decode and describe a base64 blob resource carried inline.
    fn process_blob_resource(
        &self,
        blob_resource: &agent_client_protocol::schema::BlobResourceContents,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let mut metadata = HashMap::new();
        self.validate_and_record_resource_uri(
            &blob_resource.uri,
            blob_resource.mime_type.as_deref(),
            &mut metadata,
        )?;

        // Decode blob data. A resource with no declared MIME type is decoded
        // against a permissive one.
        let mime_type = blob_resource
            .mime_type
            .as_deref()
            .unwrap_or(DEFAULT_BLOB_MIME_TYPE);
        let decoded_data = self.decode_media_payload(
            Base64Processor::decode_blob_data,
            &blob_resource.blob,
            mime_type,
        )?;

        metadata.insert("resource_type".to_string(), "blob".to_string());
        metadata.insert("data_size".to_string(), decoded_data.len().to_string());

        let text_representation = Self::resource_text_representation(
            "Blob",
            blob_resource.mime_type.as_deref(),
            &blob_resource.uri,
            decoded_data.len(),
        );

        Ok(ProcessedContent {
            content_type: ProcessedContentType::EmbeddedResource {
                uri: Self::optional_uri(&blob_resource.uri),
                mime_type: blob_resource.mime_type.clone(),
            },
            text_representation,
            binary_data: Some(decoded_data),
            metadata,
            size_bytes: blob_resource.blob.len(),
        })
    }

    /// Describe a resource link, which carries a URI and no payload.
    fn process_resource_link(
        &self,
        resource_link: &agent_client_protocol::schema::ResourceLink,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let mut metadata = HashMap::new();

        if self.enable_uri_validation {
            self.validate_uri(&resource_link.uri)?;
        }

        metadata.insert("uri".to_string(), resource_link.uri.clone());

        let text_representation = format!("[Resource Link: {}]", resource_link.uri);

        Ok(ProcessedContent {
            content_type: ProcessedContentType::ResourceLink {
                uri: resource_link.uri.clone(),
            },
            text_representation,
            binary_data: None,
            metadata,
            // A resource link carries no content data of its own.
            size_bytes: 0,
        })
    }

    /// Turn a resource URI into `None` when the resource carries none.
    fn optional_uri(uri: &str) -> Option<String> {
        if uri.is_empty() {
            None
        } else {
            Some(uri.to_string())
        }
    }

    fn process_text_content(
        &self,
        text_content: &TextContent,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let metadata = HashMap::new();

        let content_text = text_content.text.clone();
        let size_bytes = content_text.len();

        Ok(ProcessedContent {
            content_type: ProcessedContentType::Text,
            text_representation: content_text,
            binary_data: None,
            metadata,
            size_bytes,
        })
    }

    fn validate_uri(&self, uri: &str) -> Result<(), ContentBlockProcessorError> {
        if uri.is_empty() {
            return Err(ContentBlockProcessorError::InvalidUri(
                "URI cannot be empty".to_string(),
            ));
        }

        // Parse URI
        let parsed_uri = Url::parse(uri).map_err(|_| {
            ContentBlockProcessorError::InvalidUri("Invalid URI format".to_string())
        })?;

        // Allow common schemes
        let allowed_schemes = ["file", "http", "https", "data", "ftp"];

        if !url_validation::is_allowed_scheme(&parsed_uri, &allowed_schemes) {
            warn!(
                "Potentially unsupported URI scheme: {}",
                parsed_uri.scheme()
            );
        }

        Ok(())
    }

    /// Validate resource structure
    fn validate_resource_structure(
        &self,
        resource_content: &agent_client_protocol::schema::EmbeddedResource,
    ) -> Result<(), ContentBlockProcessorError> {
        use agent_client_protocol::schema::EmbeddedResourceResource;

        match &resource_content.resource {
            EmbeddedResourceResource::TextResourceContents(text_resource) => {
                // Validate text is non-empty
                if text_resource.text.is_empty() {
                    return Err(ContentBlockProcessorError::InvalidContentStructure {
                        details: "Resource text cannot be empty".to_string(),
                    });
                }

                // Validate URI if validation is enabled
                if self.enable_uri_validation && !text_resource.uri.is_empty() {
                    self.validate_uri(&text_resource.uri)?;
                }
            }
            EmbeddedResourceResource::BlobResourceContents(blob_resource) => {
                // Validate blob is non-empty
                if blob_resource.blob.is_empty() {
                    return Err(ContentBlockProcessorError::InvalidContentStructure {
                        details: "Resource blob cannot be empty".to_string(),
                    });
                }

                // Validate URI if validation is enabled
                if self.enable_uri_validation && !blob_resource.uri.is_empty() {
                    self.validate_uri(&blob_resource.uri)?;
                }
            }
            _ => {
                // Unknown or unsupported resource type
                return Err(ContentBlockProcessorError::InvalidContentStructure {
                    details: "Unsupported resource type".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Get comprehensive content processing summary for all content blocks with enhanced error handling
    pub fn process_content_blocks(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<ContentProcessingSummary, ContentBlockProcessorError> {
        // Enhanced security validation for content arrays if available
        if let Some(ref validator) = self.content_security_validator {
            validator.validate_content_blocks_security(content_blocks)?;
        }

        if self.enable_batch_recovery {
            self.process_content_blocks_with_recovery(content_blocks)
        } else {
            self.process_content_blocks_strict(content_blocks)
        }
    }

    /// Process content blocks with strict error handling (fail on first error)
    fn process_content_blocks_strict(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<ContentProcessingSummary, ContentBlockProcessorError> {
        let mut accumulator = ContentAccumulator::default();

        for (index, content_block) in content_blocks.iter().enumerate() {
            debug!(
                "Processing content block {} of {}",
                index + 1,
                content_blocks.len()
            );

            let processed = self.process_content_block(content_block).map_err(|e| {
                error!("Failed to process content block at index {}: {}", index, e);
                e
            })?;

            let type_key = processed.content_type.counting_key().to_string();
            accumulator.accumulate(processed, &type_key);
        }

        Ok(accumulator.into_summary())
    }

    /// Process content blocks with error recovery (partial processing)
    fn process_content_blocks_with_recovery(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<ContentProcessingSummary, ContentBlockProcessorError> {
        let mut accumulator = ContentAccumulator::default();
        let mut successful_count = 0;
        let mut processing_errors = Vec::new();

        for (index, content_block) in content_blocks.iter().enumerate() {
            debug!(
                "Processing content block {} of {} (with recovery)",
                index + 1,
                content_blocks.len()
            );

            match self.process_content_block_with_retry(content_block, MAX_RETRIES) {
                Ok(processed) => {
                    successful_count += 1;
                    let type_key = processed.content_type.counting_key().to_string();
                    accumulator.accumulate(processed, &type_key);
                }
                Err(e) => {
                    error!(
                        "Failed to process content block at index {} after retries: {}",
                        index, e
                    );

                    // Add placeholder for failed content
                    accumulator.accumulate_fallback(self.create_fallback_content(index, &e));

                    // Store error for reporting
                    processing_errors.push((index, e));
                }
            }
        }

        // If every block failed, report the first failure rather than a summary
        if successful_count == 0 {
            if let Some((_index, error)) = processing_errors.into_iter().next() {
                return Err(error);
            }
        }

        if successful_count < content_blocks.len() {
            warn!(
                "Partial batch processing: {}/{} content blocks processed successfully",
                successful_count,
                content_blocks.len()
            );
        }

        Ok(accumulator.into_summary())
    }

    /// Process content block with retry logic
    fn process_content_block_with_retry(
        &self,
        content_block: &ContentBlock,
        max_retries: u32,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                Self::sleep_before_retry(attempt);
            }

            match self.process_content_block(content_block) {
                Ok(processed) => {
                    Self::log_retry_success(attempt);
                    return Ok(processed);
                }
                // Don't retry certain non-transient errors
                Err(error) if self.is_non_retryable_error(&error) => {
                    debug!("Non-retryable error encountered, not retrying: {}", error);
                    last_error = Some(error);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.expect("the retry loop runs at least once and records its error"))
    }

    /// Wait out the exponential backoff before retry number `attempt`.
    ///
    /// The wait doubles with each attempt, starting at [`MS_PER_SECOND`] and
    /// stopping at [`MAX_BACKOFF_MS`].
    fn sleep_before_retry(attempt: u32) {
        let backoff_ms = std::cmp::min(
            MS_PER_SECOND * BACKOFF_BASE.pow(attempt - 1),
            MAX_BACKOFF_MS,
        );
        debug!(
            "Retrying content block processing after {}ms (attempt {})",
            backoff_ms,
            attempt + 1
        );
        std::thread::sleep(Duration::from_millis(backoff_ms));
    }

    /// Record that a retry succeeded. The first attempt is not a retry.
    fn log_retry_success(attempt: u32) {
        if attempt > 0 {
            debug!(
                "Content block processing succeeded on attempt {}",
                attempt + 1
            );
        }
    }

    /// Check if error should not be retried
    fn is_non_retryable_error(&self, error: &ContentBlockProcessorError) -> bool {
        match error {
            ContentBlockProcessorError::CapabilityNotSupported { .. } => true,
            ContentBlockProcessorError::MissingRequiredField(_) => true,
            ContentBlockProcessorError::InvalidContentStructure { .. } => true,
            ContentBlockProcessorError::UnsupportedContentType(_) => true,
            ContentBlockProcessorError::Base64Error(base64_error) => {
                matches!(
                    base64_error,
                    crate::base64_processor::Base64ProcessorError::MimeTypeNotAllowed(_)
                        | crate::base64_processor::Base64ProcessorError::CapabilityNotSupported { .. }
                        | crate::base64_processor::Base64ProcessorError::InvalidBase64(_)
                )
            }
            _ => false, // Retry timeouts, memory issues, etc.
        }
    }

    /// Create fallback content for failed processing
    fn create_fallback_content(
        &self,
        index: usize,
        error: &ContentBlockProcessorError,
    ) -> ProcessedContent {
        let mut metadata = HashMap::new();
        metadata.insert("processing_failed".to_string(), "true".to_string());
        metadata.insert(
            "error_type".to_string(),
            format!("{:?}", std::mem::discriminant(error)),
        );
        metadata.insert("content_index".to_string(), index.to_string());

        ProcessedContent {
            content_type: ProcessedContentType::Text,
            text_representation: format!(
                "[Content processing failed at index {}: {}]",
                index, error
            ),
            binary_data: None,
            metadata,
            size_bytes: 0,
        }
    }
}

/// Summary of processing multiple content blocks
#[derive(Debug)]
pub struct ContentProcessingSummary {
    /// Every block of the batch, in order, including failure placeholders.
    pub processed_contents: Vec<ProcessedContent>,
    /// Text representations of every block, joined in order.
    pub combined_text: String,
    /// Whether any block carried decoded bytes.
    pub has_binary_content: bool,
    /// Sum of the sizes of the blocks processed successfully, in bytes.
    pub total_size_bytes: usize,
    /// How many blocks of each kind the batch processed successfully.
    pub content_type_counts: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::sizes;
    use agent_client_protocol::schema::{
        AudioContent, EmbeddedResource, ImageContent, ResourceLink,
    };

    fn create_test_processor() -> ContentBlockProcessor {
        let mut supported_capabilities = HashMap::new();
        supported_capabilities.insert("text".to_string(), true);
        supported_capabilities.insert("image".to_string(), true);
        supported_capabilities.insert("audio".to_string(), true); // Enable for testing
        supported_capabilities.insert("resource".to_string(), true);
        supported_capabilities.insert("resource_link".to_string(), true);

        ContentBlockProcessor::new_with_config(
            Base64Processor::default(),
            ContentValidationConfig {
                max_resource_size: sizes::content::MAX_RESOURCE_MODERATE,
                enable_uri_validation: true,
                enable_capability_validation: true,
                supported_capabilities,
                enable_batch_recovery: true,
            },
        )
    }

    #[test]
    fn test_process_text_content() {
        let processor = create_test_processor();
        let text_content = TextContent::new("Hello, world!".to_string());

        let result = processor.process_text_content(&text_content);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.text_representation, "Hello, world!");
        assert_eq!(processed.size_bytes, 13);
        assert!(matches!(processed.content_type, ProcessedContentType::Text));
    }

    #[test]
    fn test_process_image_content_png() {
        let processor = create_test_processor();
        // 1x1 PNG in base64
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let image_content = ImageContent::new(png_data.to_string(), "image/png".to_string());

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Image content: image/png"));
        assert!(processed.text_representation.contains("embedded"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::Image { .. }
        ));
        assert!(processed.binary_data.is_some());
        let binary_data = processed.binary_data.unwrap();
        assert!(!binary_data.is_empty());
    }

    #[test]
    fn test_process_image_content_with_uri() {
        let processor = create_test_processor();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let image_content = ImageContent::new(png_data.to_string(), "image/png".to_string())
            .uri(Some("https://example.com/image.png".to_string()));

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("from https://example.com/image.png"));
        assert_eq!(
            processed.metadata.get("source_uri"),
            Some(&"https://example.com/image.png".to_string())
        );
    }

    #[test]
    fn test_process_audio_content_wav() {
        let processor = create_test_processor();

        // Test that audio capability is supported
        println!("Testing audio capability support...");
        let capability_result = processor.validate_capability("audio");
        if let Err(e) = &capability_result {
            println!("Audio capability validation failed: {:?}", e);
        }
        assert!(
            capability_result.is_ok(),
            "Audio capability should be supported in test processor"
        );

        // Simple WAV header in base64 (RIFF header + WAVE format)
        let wav_data = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";

        let audio_content = AudioContent::new(wav_data.to_string(), "audio/wav".to_string());

        let content_block = ContentBlock::Audio(audio_content);

        // Test content block structure validation first
        println!("Testing content block structure validation...");
        let structure_result = processor.validate_content_block_structure(&content_block);
        if let Err(e) = &structure_result {
            println!("Structure validation failed: {:?}", e);
        }

        println!("Processing audio content block...");
        let result = processor.process_content_block(&content_block);

        match &result {
            Ok(_) => {
                println!("Audio processing succeeded");
            }
            Err(e) => {
                println!("Audio processing failed: {:?}", e);
                // Print the full error chain
                let mut current_error: &dyn std::error::Error = e;
                println!("Error chain:");
                println!("  - {}", current_error);
                while let Some(source) = current_error.source() {
                    println!("  - caused by: {}", source);
                    current_error = source;
                }
            }
        }

        assert!(
            result.is_ok(),
            "Expected audio processing to succeed, but got error: {:?}",
            result.err()
        );

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Audio content: audio/wav"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::Audio { .. }
        ));
        assert!(processed.binary_data.is_some());
    }

    #[test]
    fn test_process_text_resource_with_uri_and_mime() {
        use agent_client_protocol::schema::{EmbeddedResourceResource, TextResourceContents};

        let processor = create_test_processor();

        let text_resource =
            TextResourceContents::new("Test content", "file:///test.txt").mime_type("text/plain");
        let embedded_resource = EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(text_resource),
        );

        let content_block = ContentBlock::Resource(embedded_resource);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text_representation.contains("Text Resource"));
        assert!(processed.text_representation.contains("text/plain"));
        assert!(processed.text_representation.contains("file:///test.txt"));
        assert!(processed.text_representation.contains("12 bytes"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::EmbeddedResource { .. }
        ));
        assert_eq!(processed.size_bytes, 12); // "Test content" length
        assert_eq!(
            processed.metadata.get("uri"),
            Some(&"file:///test.txt".to_string())
        );
        assert_eq!(
            processed.metadata.get("mime_type"),
            Some(&"text/plain".to_string())
        );
        assert_eq!(
            processed.metadata.get("resource_type"),
            Some(&"text".to_string())
        );
        assert!(processed.binary_data.is_none()); // Text resources don't have binary data
    }

    #[test]
    fn test_process_text_resource_without_uri() {
        use agent_client_protocol::schema::{EmbeddedResourceResource, TextResourceContents};

        let processor = create_test_processor();

        let text_resource =
            TextResourceContents::new("Embedded text content", "").mime_type("text/plain");
        let embedded_resource = EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(text_resource),
        );

        let content_block = ContentBlock::Resource(embedded_resource);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text_representation.contains("Text Resource"));
        assert!(processed.text_representation.contains("(embedded)"));
        assert!(!processed.text_representation.contains("from"));
        assert_eq!(processed.size_bytes, 21); // "Embedded text content" length
        assert!(!processed.metadata.contains_key("uri"));
        if let ProcessedContentType::EmbeddedResource { uri, .. } = processed.content_type {
            assert!(uri.is_none());
        } else {
            panic!("Expected EmbeddedResource content type");
        }
    }

    #[test]
    fn test_process_text_resource_without_mime() {
        use agent_client_protocol::schema::{EmbeddedResourceResource, TextResourceContents};

        let processor = create_test_processor();

        let text_resource = TextResourceContents::new("Test", "file:///test.txt");
        let embedded_resource = EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(text_resource),
        );

        let content_block = ContentBlock::Resource(embedded_resource);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text_representation.contains("Text Resource"));
        assert!(!processed.metadata.contains_key("mime_type"));
        if let ProcessedContentType::EmbeddedResource { mime_type, .. } = processed.content_type {
            assert!(mime_type.is_none());
        } else {
            panic!("Expected EmbeddedResource content type");
        }
    }

    #[test]
    fn test_process_blob_resource_with_mime() {
        use agent_client_protocol::schema::{BlobResourceContents, EmbeddedResourceResource};

        let processor = create_test_processor();

        // Simple text encoded as base64
        let blob_data = "SGVsbG8gV29ybGQ="; // "Hello World" in base64

        // Use text/plain which is an allowed blob mime type in Base64Processor
        let blob_resource = BlobResourceContents::new(blob_data, "https://example.com/data.txt")
            .mime_type("text/plain");
        let embedded_resource = EmbeddedResource::new(
            EmbeddedResourceResource::BlobResourceContents(blob_resource),
        );

        let content_block = ContentBlock::Resource(embedded_resource);
        let result = processor.process_content_block(&content_block);
        if let Err(ref e) = result {
            tracing::error!("Error processing blob resource: {:?}", e);
        }
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text_representation.contains("Blob Resource"));
        assert!(processed.text_representation.contains("text/plain"));
        assert!(processed
            .text_representation
            .contains("https://example.com/data.txt"));
        assert!(processed.binary_data.is_some());

        let binary_data = processed.binary_data.unwrap();
        assert_eq!(binary_data, b"Hello World");
        assert_eq!(
            processed.metadata.get("resource_type"),
            Some(&"blob".to_string())
        );
        assert_eq!(
            processed.metadata.get("uri"),
            Some(&"https://example.com/data.txt".to_string())
        );
    }

    #[test]
    fn test_process_blob_resource_without_mime() {
        use agent_client_protocol::schema::{BlobResourceContents, EmbeddedResourceResource};

        let processor = create_test_processor();

        let blob_data = "SGVsbG8gV29ybGQ="; // "Hello World" in base64

        let blob_resource = BlobResourceContents::new(blob_data, "");
        let embedded_resource = EmbeddedResource::new(
            EmbeddedResourceResource::BlobResourceContents(blob_resource),
        );

        let content_block = ContentBlock::Resource(embedded_resource);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text_representation.contains("Blob Resource"));
        assert!(processed.text_representation.contains("(embedded)"));
        assert!(processed.binary_data.is_some());
        assert!(!processed.metadata.contains_key("mime_type"));
    }

    #[test]
    fn test_process_blob_resource_invalid_base64() {
        use agent_client_protocol::schema::{BlobResourceContents, EmbeddedResourceResource};

        let processor = create_test_processor();

        let blob_resource = BlobResourceContents::new("invalid-base64!@#$", "");
        let embedded_resource = EmbeddedResource::new(
            EmbeddedResourceResource::BlobResourceContents(blob_resource),
        );

        let content_block = ContentBlock::Resource(embedded_resource);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_err());

        // Should be a base64 processing error
        assert!(matches!(
            result.unwrap_err(),
            ContentBlockProcessorError::Base64Error(_)
        ));
    }

    #[test]
    fn test_process_resource_link_content() {
        let processor = create_test_processor();

        // Create a proper ResourceLink with the builder pattern.
        // ResourceLink::new takes (name, uri), so the URI goes in the second argument.
        let resource_link = ResourceLink::new("document.pdf", "https://example.com/document.pdf");

        let content_block = ContentBlock::ResourceLink(resource_link);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Resource Link: https://example.com/document.pdf"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::ResourceLink { .. }
        ));
        assert_eq!(processed.size_bytes, 0); // ResourceLink doesn't contain content data
    }

    #[test]
    fn test_validate_uri() {
        let processor = create_test_processor();

        assert!(processor.validate_uri("file:///test.txt").is_ok());
        assert!(processor.validate_uri("https://example.com").is_ok());
        assert!(processor.validate_uri("http://example.com").is_ok());
        assert!(processor
            .validate_uri("data:text/plain;base64,SGVsbG8=")
            .is_ok());

        // Error cases
        assert!(processor.validate_uri("").is_err());
        assert!(processor.validate_uri("invalid-uri").is_err());
        assert!(processor.validate_uri("just-a-path").is_err());
    }

    #[test]
    fn test_process_content_blocks_mixed() {
        let processor = create_test_processor();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let content_blocks = vec![
            ContentBlock::Text(TextContent::new("Hello")),
            ContentBlock::Image(ImageContent::new(png_data, "image/png")),
        ];

        let result = processor.process_content_blocks(&content_blocks);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert_eq!(summary.processed_contents.len(), 2);
        assert!(summary.has_binary_content);
        assert_eq!(summary.content_type_counts.get("text"), Some(&1));
        assert_eq!(summary.content_type_counts.get("image"), Some(&1));
        assert!(summary.combined_text.contains("Hello"));
        assert!(summary.combined_text.contains("[Image content:"));
        assert!(summary.total_size_bytes > 0);
    }

    #[test]
    fn test_image_format_validation_error() {
        let processor = create_test_processor();
        // Invalid base64 data
        let invalid_data = "invalid-base64-data!@#$";

        let image_content = ImageContent::new(invalid_data, "image/png");

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_err());

        // Should be a base64 processing error
        assert!(matches!(
            result.unwrap_err(),
            ContentBlockProcessorError::Base64Error(_)
        ));
    }

    #[test]
    fn test_unsupported_mime_type() {
        let processor = create_test_processor();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        // Unsupported MIME type
        let image_content = ImageContent::new(png_data, "image/bmp"); // Not in allowed list

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_err());

        // Should be a MIME type error
        assert!(matches!(
            result.unwrap_err(),
            ContentBlockProcessorError::Base64Error(_)
        ));
    }

    #[test]
    fn test_uri_validation_disabled() {
        let processor = ContentBlockProcessor::new(
            Base64Processor::default(),
            sizes::content::MAX_RESOURCE_MODERATE,
            false,
        );

        // ResourceLink::new takes (name, uri); the uri here has an invalid scheme.
        let resource_link = ResourceLink::new("test", "invalid-scheme://test");

        let content_block = ContentBlock::ResourceLink(resource_link);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok()); // Should pass with URI validation disabled
    }

    #[test]
    fn test_empty_content_blocks() {
        let processor = create_test_processor();
        let content_blocks = vec![];

        let result = processor.process_content_blocks(&content_blocks);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert_eq!(summary.processed_contents.len(), 0);
        assert!(!summary.has_binary_content);
        assert_eq!(summary.total_size_bytes, 0);
        assert!(summary.combined_text.is_empty());
        assert!(summary.content_type_counts.is_empty());
    }

    #[test]
    fn test_validate_resource_structure_with_text() {
        use agent_client_protocol::schema::{EmbeddedResourceResource, TextResourceContents};

        let processor = create_test_processor();

        let text_resource =
            TextResourceContents::new("Sample text content", "https://example.com/data.json");
        let embedded = EmbeddedResource::new(EmbeddedResourceResource::TextResourceContents(
            text_resource,
        ));
        let content_block = ContentBlock::Resource(embedded);

        let result = processor.validate_content_block_structure(&content_block);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_resource_structure_with_blob() {
        use agent_client_protocol::schema::{BlobResourceContents, EmbeddedResourceResource};

        let processor = create_test_processor();

        let blob_resource =
            BlobResourceContents::new("SGVsbG8gV29ybGQ=", "").mime_type("text/plain");
        let embedded = EmbeddedResource::new(EmbeddedResourceResource::BlobResourceContents(
            blob_resource,
        ));
        let content_block = ContentBlock::Resource(embedded);

        let result = processor.validate_content_block_structure(&content_block);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_resource_structure_empty() {
        use agent_client_protocol::schema::{EmbeddedResourceResource, TextResourceContents};

        let processor = create_test_processor();

        // Empty text should fail validation
        let text_resource = TextResourceContents::new("", "");
        let embedded = EmbeddedResource::new(EmbeddedResourceResource::TextResourceContents(
            text_resource,
        ));
        let content_block = ContentBlock::Resource(embedded);

        let result = processor.validate_content_block_structure(&content_block);
        // Empty resource should fail validation
        assert!(result.is_err());
    }
}
