use crate::base64_validation;
use crate::constants::sizes;
use crate::content_security_validator::{ContentSecurityError, ContentSecurityValidator};
use crate::error::ToJsonRpcError;
use crate::json_rpc_codes::{INTERNAL_ERROR, INVALID_PARAMS};
use crate::mime_type_validator::{MimeTypeValidationError, MimeTypeValidator};
use crate::size_validator::{SizeValidationError, SizeValidator};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use std::collections::HashSet;
use thiserror::Error;

/// Shortest buffer the suspicious-pattern heuristics can judge, in bytes.
///
/// A shorter buffer holds too little evidence, so it is always accepted.
const MIN_HEURISTIC_DATA_LEN: usize = 16;

/// Divisor that fixes the share of null bytes treated as suspicious.
///
/// More than `len / 2` null bytes is over 50% of the buffer, which suggests
/// corrupted data or a padding attack rather than real content.
const NULL_BYTE_THRESHOLD_RATIO: usize = 2;

/// Bytes needed to read the DOS/Windows `MZ` magic signature.
const DOS_HEADER_MIN_SIZE: usize = 2;

/// Bytes needed to read the Linux ELF `\x7fELF` magic signature.
const ELF_HEADER_MIN_SIZE: usize = 4;

/// Bytes needed to read the truncated Mach-O magic signature.
const MACHO_PARTIAL_HEADER_MIN_SIZE: usize = 3;

/// Bytes needed to read the full Mach-O magic signature.
const MACHO_FULL_HEADER_MIN_SIZE: usize = 4;

/// Magic signatures that mark an embedded executable, with the byte count each
/// signature needs and the platform it identifies.
const EXECUTABLE_SIGNATURES: &[(&[u8], usize, &str)] = &[
    (b"MZ", DOS_HEADER_MIN_SIZE, "DOS/Windows executable"),
    (b"\x7fELF", ELF_HEADER_MIN_SIZE, "Linux ELF executable"),
    (
        b"\xfe\xed\xfa",
        MACHO_PARTIAL_HEADER_MIN_SIZE,
        "Mach-O binary (partial)",
    ),
    (
        b"\xcf\xfa\xed\xfe",
        MACHO_FULL_HEADER_MIN_SIZE,
        "Mach-O binary",
    ),
];

/// Everything that can go wrong while decoding and checking base64 content.
///
/// The variants split into three groups: format faults in the encoded text,
/// policy faults such as a disallowed MIME type or an unsupported capability,
/// and validation faults raised by the MIME type and security validators.
#[derive(Debug, Error, Clone)]
pub enum Base64ProcessorError {
    /// The text is not valid base64. The payload names the fault.
    #[error("invalid base64 format: {0}")]
    InvalidBase64(String),
    /// The decoded content is larger than the configured size limit.
    #[error("data exceeds maximum size limit of {limit} bytes (actual: {actual})")]
    SizeExceeded {
        /// Largest accepted size, in bytes.
        limit: usize,
        /// Size the content actually holds, in bytes.
        actual: usize,
    },
    /// The declared image format is not one the processor accepts.
    #[error("unsupported image format: {0}")]
    UnsupportedImageFormat(String),
    /// The declared audio format is not one the processor accepts.
    #[error("unsupported audio format: {0}")]
    UnsupportedAudioFormat(String),
    /// The magic bytes of the decoded content disagree with the declared format.
    #[error("format validation failed: expected {expected}, but data appears to be {actual}")]
    FormatMismatch {
        /// Format the declared MIME type calls for.
        expected: String,
        /// Format the magic bytes point to.
        actual: String,
    },
    /// The MIME type is not on the allow-list for this content kind.
    #[error("MIME type not allowed: {0}")]
    MimeTypeNotAllowed(String),
    /// The decoded content is larger than the memory budget for processing.
    #[error("memory allocation failed: insufficient memory for processing")]
    MemoryAllocationFailed,
    /// The agent does not declare the capability this content kind needs.
    #[error("capability not supported: {capability}")]
    CapabilityNotSupported {
        /// Name of the capability the content needs.
        capability: String,
    },
    /// A built-in security heuristic rejected the decoded content.
    #[error("security validation failed")]
    SecurityValidationFailed,
    /// The optional [`ContentSecurityValidator`] rejected the content.
    #[error("enhanced security validation failed: {0}")]
    EnhancedSecurityValidationFailed(#[from] ContentSecurityError),
    /// Content-level validation failed. The payload names the fault.
    #[error("content validation failed: {details}")]
    ContentValidationFailed {
        /// What the validator objected to.
        details: String,
    },
    /// The [`MimeTypeValidator`] rejected the declared MIME type.
    #[error("MIME type validation failed: {0}")]
    MimeTypeValidationFailed(#[from] MimeTypeValidationError),
}

impl ToJsonRpcError for Base64ProcessorError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            Self::InvalidBase64(_)
            | Self::SizeExceeded { .. }
            | Self::MimeTypeNotAllowed(_)
            | Self::FormatMismatch { .. }
            | Self::UnsupportedImageFormat(_)
            | Self::UnsupportedAudioFormat(_)
            | Self::CapabilityNotSupported { .. }
            | Self::SecurityValidationFailed
            | Self::ContentValidationFailed { .. } => INVALID_PARAMS,
            Self::MemoryAllocationFailed
            | Self::EnhancedSecurityValidationFailed(_)
            | Self::MimeTypeValidationFailed(_) => INTERNAL_ERROR,
        }
    }

    fn to_error_data(&self) -> Option<Value> {
        let data = match self {
            Self::InvalidBase64(details) => json!({
                "error": "invalid_base64_format",
                "details": details,
                "suggestion": "Ensure base64 data is properly encoded with correct padding"
            }),
            Self::SizeExceeded { limit, actual } => json!({
                "error": "content_size_exceeded",
                "providedSize": actual,
                "maxSize": limit,
                "suggestion": "Reduce content size or split into smaller parts"
            }),
            Self::UnsupportedImageFormat(format) => json!({
                "error": "unsupported_image_format",
                "format": format,
                "supportedFormats": ["png", "jpeg", "gif", "webp"],
                "suggestion": "Convert image to a supported format"
            }),
            Self::UnsupportedAudioFormat(format) => json!({
                "error": "unsupported_audio_format",
                "format": format,
                "supportedFormats": ["wav", "mp3", "mpeg", "ogg", "aac"],
                "suggestion": "Convert audio to a supported format"
            }),
            Self::FormatMismatch { expected, actual } => json!({
                "error": "format_mismatch",
                "expectedFormat": expected,
                "actualFormat": actual,
                "suggestion": "Ensure content data matches the declared MIME type"
            }),
            Self::MimeTypeNotAllowed(mime_type) => json!({
                "error": "mime_type_not_allowed",
                "mimeType": mime_type,
                "suggestion": "Use an allowed MIME type"
            }),
            Self::MemoryAllocationFailed => json!({
                "error": "memory_allocation_failed",
                "suggestion": "Reduce content size or retry later"
            }),
            Self::CapabilityNotSupported { capability } => json!({
                "error": "capability_not_supported",
                "requiredCapability": capability,
                "suggestion": "Check agent capabilities before sending content"
            }),
            Self::SecurityValidationFailed => json!({
                "error": "security_validation_failed",
                "suggestion": "Content failed security validation checks"
            }),
            Self::ContentValidationFailed { details } => json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format"
            }),
            Self::EnhancedSecurityValidationFailed(security_error) => {
                return security_error.to_error_data();
            }
            Self::MimeTypeValidationFailed(mime_error) => {
                return mime_error.to_error_data();
            }
        };
        Some(data)
    }
}

impl From<SizeValidationError> for Base64ProcessorError {
    fn from(error: SizeValidationError) -> Self {
        match error {
            SizeValidationError::SizeExceeded { actual, limit, .. } => {
                Base64ProcessorError::SizeExceeded { limit, actual }
            }
        }
    }
}

// IMPORTANT: Do not add timeouts to content processing operations.
// Content processing should be allowed to complete regardless of size or complexity.
// Timeouts create artificial limitations and poor user experience by interrupting
// legitimate processing of large or complex content. Users cannot predict when
/// Decodes base64 image, audio and blob payloads, and checks each one.
///
/// A decode runs four gates in order: the capability the content kind needs,
/// the optional [`ContentSecurityValidator`], the base64 format and size
/// limits, and finally the declared MIME type against the decoded bytes. Build
/// one with [`Base64Processor::new`] for size limits alone, or with
/// [`Base64Processor::with_enhanced_security`] to add the security validator.
#[derive(Clone, Debug)]
pub struct Base64Processor {
    allowed_blob_mime_types: HashSet<String>,
    max_memory_usage: usize,
    enable_capability_validation: bool,
    enable_security_validation: bool,
    supported_capabilities: HashSet<String>,
    content_security_validator: Option<ContentSecurityValidator>,
    mime_type_validator: MimeTypeValidator,
    size_validator: SizeValidator,
}

impl Default for Base64Processor {
    fn default() -> Self {
        let mut allowed_blob_mime_types = HashSet::new();
        // Image types
        allowed_blob_mime_types.insert("image/png".to_string());
        allowed_blob_mime_types.insert("image/jpeg".to_string());
        allowed_blob_mime_types.insert("image/gif".to_string());
        allowed_blob_mime_types.insert("image/webp".to_string());
        // Audio types
        allowed_blob_mime_types.insert("audio/wav".to_string());
        allowed_blob_mime_types.insert("audio/mp3".to_string());
        allowed_blob_mime_types.insert("audio/mpeg".to_string());
        allowed_blob_mime_types.insert("audio/ogg".to_string());
        allowed_blob_mime_types.insert("audio/aac".to_string());
        // Other types
        allowed_blob_mime_types.insert("application/pdf".to_string());
        allowed_blob_mime_types.insert("text/plain".to_string());

        let mut supported_capabilities = HashSet::new();
        supported_capabilities.insert("image".to_string());
        supported_capabilities.insert("audio".to_string());
        supported_capabilities.insert("text".to_string());

        let size_validator = SizeValidator::default();

        Self {
            allowed_blob_mime_types,
            max_memory_usage: sizes::memory::MAX_BASE64_MEMORY,
            enable_capability_validation: true,
            enable_security_validation: true,
            supported_capabilities,
            content_security_validator: None,
            mime_type_validator: MimeTypeValidator::moderate(),
            size_validator,
        }
    }
}

impl Base64Processor {
    /// Build a processor that accepts base64 payloads up to `max_size` bytes.
    ///
    /// Every other setting keeps its [`Default`] value: capability and security
    /// validation are on, and the MIME type policy is
    /// [`MimeTypeValidator::moderate`].
    pub fn new(max_size: usize) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_base64_size: max_size,
            ..Default::default()
        });

        Self {
            size_validator,
            ..Default::default()
        }
    }

    /// Build a processor with every limit and switch set explicitly.
    ///
    /// `max_size` caps the base64 payload, `max_memory_usage` caps the decoded
    /// bytes, the two flags turn capability and security validation on or off,
    /// and `supported_capabilities` names the content kinds the agent declares.
    /// No [`ContentSecurityValidator`] is attached.
    pub fn new_with_config(
        max_size: usize,
        max_memory_usage: usize,
        enable_capability_validation: bool,
        enable_security_validation: bool,
        supported_capabilities: HashSet<String>,
    ) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_base64_size: max_size,
            ..Default::default()
        });

        Self {
            max_memory_usage,
            enable_capability_validation,
            enable_security_validation,
            supported_capabilities,
            content_security_validator: None,
            mime_type_validator: MimeTypeValidator::moderate(),
            size_validator,
            ..Default::default()
        }
    }

    /// Build a processor that also runs `content_security_validator` on the
    /// raw base64 text before decoding.
    ///
    /// `max_size` caps the base64 payload. Every other setting keeps its
    /// [`Default`] value.
    pub fn with_enhanced_security(
        max_size: usize,
        content_security_validator: ContentSecurityValidator,
    ) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_base64_size: max_size,
            ..Default::default()
        });

        Self {
            content_security_validator: Some(content_security_validator),
            mime_type_validator: MimeTypeValidator::moderate(),
            size_validator,
            ..Default::default()
        }
    }

    /// Build a processor with every limit and switch set explicitly, plus a
    /// [`ContentSecurityValidator`] that runs on the raw base64 text.
    ///
    /// This is [`Base64Processor::new_with_config`] with the security validator
    /// attached.
    pub fn with_enhanced_security_config(
        max_size: usize,
        max_memory_usage: usize,
        enable_capability_validation: bool,
        enable_security_validation: bool,
        supported_capabilities: HashSet<String>,
        content_security_validator: ContentSecurityValidator,
    ) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_base64_size: max_size,
            ..Default::default()
        });

        Self {
            max_memory_usage,
            enable_capability_validation,
            enable_security_validation,
            supported_capabilities,
            content_security_validator: Some(content_security_validator),
            mime_type_validator: MimeTypeValidator::moderate(),
            size_validator,
            ..Default::default()
        }
    }

    /// Check if a capability is supported
    fn validate_capability(&self, capability: &str) -> Result<(), Base64ProcessorError> {
        if !self.enable_capability_validation {
            return Ok(());
        }

        if !self.supported_capabilities.contains(capability) {
            return Err(Base64ProcessorError::CapabilityNotSupported {
                capability: capability.to_string(),
            });
        }
        Ok(())
    }

    /// Perform security validation on content
    fn perform_security_validation(&self, data: &[u8]) -> Result<(), Base64ProcessorError> {
        if !self.enable_security_validation {
            return Ok(());
        }

        // Check for potentially malicious patterns (basic security checks)
        if data.len() > self.max_memory_usage {
            return Err(Base64ProcessorError::MemoryAllocationFailed);
        }

        // Check for suspicious patterns in binary data
        if self.contains_suspicious_patterns(data) {
            return Err(Base64ProcessorError::SecurityValidationFailed);
        }

        Ok(())
    }

    /// Check for suspicious patterns in binary data
    fn contains_suspicious_patterns(&self, data: &[u8]) -> bool {
        // Basic heuristic checks for potentially malicious content
        if data.len() < MIN_HEURISTIC_DATA_LEN {
            return false;
        }

        // Check for excessive null bytes (possible data corruption or attack)
        let null_count = data.iter().filter(|&&b| b == 0).count();
        if null_count > data.len() / NULL_BYTE_THRESHOLD_RATIO {
            return true;
        }

        // Check for patterns that might indicate embedded executables
        EXECUTABLE_SIGNATURES
            .iter()
            .any(|(magic, min_size, _platform)| {
                data.len() >= *min_size && data.starts_with(magic)
            })
    }

    /// Run the optional enhanced security validator over the raw base64 text.
    ///
    /// `content_type` names the content kind for the validator's own limits.
    /// Returns `Ok(())` when no validator is attached.
    fn validate_enhanced_security(
        &self,
        data: &str,
        content_type: &str,
    ) -> Result<(), Base64ProcessorError> {
        let Some(validator) = self.content_security_validator.as_ref() else {
            return Ok(());
        };
        validator
            .validate_base64_security(data, content_type)
            .map_err(|_e| Base64ProcessorError::SecurityValidationFailed)
    }

    /// Decode base64 `data` as an image of `mime_type`.
    ///
    /// # Errors
    ///
    /// Returns [`Base64ProcessorError::CapabilityNotSupported`] when the agent
    /// does not declare the `image` capability,
    /// [`Base64ProcessorError::SecurityValidationFailed`] when a security check
    /// rejects the content, [`Base64ProcessorError::InvalidBase64`] when the
    /// text is malformed, [`Base64ProcessorError::SizeExceeded`] when the
    /// payload is over the limit, and
    /// [`Base64ProcessorError::MimeTypeValidationFailed`] when the decoded
    /// bytes disagree with `mime_type`.
    pub fn decode_image_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support
        self.validate_capability("image")?;

        self.validate_enhanced_security(data, "image")?;

        // Validate base64 format and size limits
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        // Perform base64 decoding
        let decoded = general_purpose::STANDARD
            .decode(data)
            .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))?;

        // Use centralized MIME type validator with format validation
        self.mime_type_validator
            .validate_image_mime_type(mime_type, Some(&decoded))?;

        // Security validation
        self.perform_security_validation(&decoded)?;

        Ok(decoded)
    }

    /// Decode base64 `data` as audio of `mime_type`.
    ///
    /// # Errors
    ///
    /// Returns [`Base64ProcessorError::CapabilityNotSupported`] when the agent
    /// does not declare the `audio` capability,
    /// [`Base64ProcessorError::SecurityValidationFailed`] when a security check
    /// rejects the content, [`Base64ProcessorError::InvalidBase64`] when the
    /// text is malformed, [`Base64ProcessorError::SizeExceeded`] when the
    /// payload is over the limit, and
    /// [`Base64ProcessorError::MimeTypeValidationFailed`] when the decoded
    /// bytes disagree with `mime_type`.
    pub fn decode_audio_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support
        self.validate_capability("audio")?;

        self.validate_enhanced_security(data, "audio")?;

        // Validate base64 format and size limits
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        // Perform base64 decoding
        let decoded = general_purpose::STANDARD
            .decode(data)
            .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))?;

        // Use centralized MIME type validator with format validation
        self.mime_type_validator
            .validate_audio_mime_type(mime_type, Some(&decoded))?;

        // Security validation
        self.perform_security_validation(&decoded)?;

        Ok(decoded)
    }

    /// Decode base64 `data` as an arbitrary blob of `mime_type`.
    ///
    /// The capability checked follows `mime_type`: `image` for `image/*`,
    /// `audio` for `audio/*`, and `text` for everything else. Unlike the image
    /// and audio paths, the decoded bytes are not matched against magic bytes;
    /// only the MIME type allow-list applies.
    ///
    /// # Errors
    ///
    /// Returns [`Base64ProcessorError::CapabilityNotSupported`] when the agent
    /// does not declare the capability the MIME type implies,
    /// [`Base64ProcessorError::MimeTypeNotAllowed`] when `mime_type` is off the
    /// allow-list, [`Base64ProcessorError::SecurityValidationFailed`] when a
    /// security check rejects the content,
    /// [`Base64ProcessorError::InvalidBase64`] when the text is malformed, and
    /// [`Base64ProcessorError::SizeExceeded`] when the payload is over the
    /// limit.
    pub fn decode_blob_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support (general capability for blob data)
        let capability = if mime_type.starts_with("image/") {
            "image"
        } else if mime_type.starts_with("audio/") {
            "audio"
        } else {
            "text" // Default for other blob types like PDF, text
        };
        self.validate_capability(capability)?;

        self.validate_enhanced_security(data, "blob")?;

        // Validate MIME type and base64 format
        self.validate_mime_type(mime_type, &self.allowed_blob_mime_types)?;
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        // Perform base64 decoding
        let decoded = general_purpose::STANDARD
            .decode(data)
            .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))?;

        // Security validation
        self.perform_security_validation(&decoded)?;

        Ok(decoded)
    }

    fn validate_base64_format(&self, data: &str) -> Result<(), Base64ProcessorError> {
        base64_validation::validate_base64_format(data).map_err(|e| match e {
            base64_validation::Base64ValidationError::EmptyData => {
                Base64ProcessorError::InvalidBase64("Empty base64 data".to_string())
            }
            base64_validation::Base64ValidationError::InvalidCharacters => {
                Base64ProcessorError::InvalidBase64("Contains invalid characters".to_string())
            }
            base64_validation::Base64ValidationError::InvalidPadding => {
                Base64ProcessorError::InvalidBase64("Invalid base64 padding".to_string())
            }
        })
    }

    fn check_size_limits(&self, data: &str) -> Result<(), Base64ProcessorError> {
        self.size_validator.validate_base64_size(data)?;
        Ok(())
    }

    fn validate_mime_type(
        &self,
        mime_type: &str,
        allowed_types: &HashSet<String>,
    ) -> Result<(), Base64ProcessorError> {
        if !allowed_types.contains(mime_type) {
            return Err(Base64ProcessorError::MimeTypeNotAllowed(
                mime_type.to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_base64_format() {
        let processor = Base64Processor::default();

        // Valid base64
        assert!(processor.validate_base64_format("SGVsbG8gV29ybGQ=").is_ok());

        // Empty string
        assert!(processor.validate_base64_format("").is_err());

        // Invalid characters
        assert!(processor.validate_base64_format("Hello!").is_err());

        // Invalid padding
        assert!(processor.validate_base64_format("SGVsbG8").is_err());
    }

    #[test]
    fn test_check_size_limits() {
        let processor = Base64Processor::new(100); // 100 bytes limit

        // Small data (should pass)
        assert!(processor.check_size_limits("SGVsbG8=").is_ok()); // "Hello"

        // Large data (should fail)
        let large_data = "A".repeat(200); // Much larger than 100 bytes when decoded
        assert!(processor.check_size_limits(&large_data).is_err());
    }

    #[test]
    fn test_validate_png_format() {
        let validator = MimeTypeValidator::default();

        // Valid PNG header
        let png_header = b"\x89PNG\r\n\x1a\n";
        assert!(validator
            .validate_image_mime_type("image/png", Some(png_header))
            .is_ok());

        // Invalid PNG header
        let invalid_header = b"NOTPNG\x00\x00";
        assert!(validator
            .validate_image_mime_type("image/png", Some(invalid_header))
            .is_err());
    }

    #[test]
    fn test_validate_jpeg_format() {
        let validator = MimeTypeValidator::default();

        // Valid JPEG header (SOI marker)
        let jpeg_header = b"\xFF\xD8\xFF\xE0";

        let result = validator.validate_image_mime_type("image/jpeg", Some(jpeg_header));
        if let Err(e) = result {
            panic!("JPEG validation should have succeeded but got error: {}", e);
        }

        // Invalid JPEG header
        let invalid_header = b"NOTJPEG\x00";
        let result2 = validator.validate_image_mime_type("image/jpeg", Some(invalid_header));
        if result2.is_ok() {
            panic!("Invalid JPEG header should have been rejected");
        }
    }

    #[test]
    fn test_decode_image_data() {
        let processor = Base64Processor::default();

        // This is a 1x1 PNG in base64
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let result = processor.decode_image_data(png_data, "image/png");
        assert!(result.is_ok());

        // Test with wrong MIME type
        let result = processor.decode_image_data(png_data, "image/jpeg");
        assert!(result.is_err());
    }

    #[test]
    fn test_mime_type_validation() {
        let processor = Base64Processor::default();

        // Test allowed blob MIME type (image)
        assert!(processor
            .validate_mime_type("image/png", &processor.allowed_blob_mime_types)
            .is_ok());

        // Test disallowed MIME type
        assert!(processor
            .validate_mime_type("image/bmp", &processor.allowed_blob_mime_types)
            .is_err());
    }
}
