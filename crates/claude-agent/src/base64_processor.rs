use crate::base64_validation;
use crate::constants::sizes;
use crate::content_security_validator::{ContentSecurityError, ContentSecurityValidator};
use crate::error::ToJsonRpcError;
use crate::mime_type_validator::{MimeTypeValidationError, MimeTypeValidator};
use crate::size_validator::{SizeValidationError, SizeValidator};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum Base64ProcessorError {
    #[error("Invalid base64 format: {0}")]
    InvalidBase64(String),
    #[error("Data exceeds maximum size limit of {limit} bytes (actual: {actual})")]
    SizeExceeded { limit: usize, actual: usize },
    #[error("Unsupported image format: {0}")]
    UnsupportedImageFormat(String),
    #[error("Unsupported audio format: {0}")]
    UnsupportedAudioFormat(String),
    #[error("Format validation failed: expected {expected}, but data appears to be {actual}")]
    FormatMismatch { expected: String, actual: String },
    #[error("MIME type not allowed: {0}")]
    MimeTypeNotAllowed(String),
    #[error("Memory allocation failed: insufficient memory for processing")]
    MemoryAllocationFailed,
    #[error("Capability not supported: {capability}")]
    CapabilityNotSupported { capability: String },
    #[error("Security validation failed")]
    SecurityValidationFailed,
    #[error("Enhanced security validation failed: {0}")]
    EnhancedSecurityValidationFailed(#[from] ContentSecurityError),
    #[error("Content validation failed: {details}")]
    ContentValidationFailed { details: String },
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
            | Self::ContentValidationFailed { .. } => -32602, // Invalid params
            Self::MemoryAllocationFailed
            | Self::EnhancedSecurityValidationFailed(_)
            | Self::MimeTypeValidationFailed(_) => -32603, // Internal error
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
// operations will be artificially terminated, leading to frustration and unreliable behavior.
#[derive(Clone)]
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
        if data.len() < 16 {
            return false;
        }

        // Check for excessive null bytes (possible data corruption or attack)
        let null_count = data.iter().filter(|&&b| b == 0).count();
        if null_count > data.len() / 2 {
            return true;
        }

        // Check for patterns that might indicate embedded executables
        if data.len() >= 2 && data.starts_with(b"MZ") {
            return true; // DOS/Windows executable
        }
        if data.len() >= 4 && data.starts_with(b"\x7fELF") {
            return true; // Linux ELF executable
        }
        if data.len() >= 3 && data.starts_with(b"\xfe\xed\xfa") {
            return true; // Mach-O binary (partial)
        }
        if data.len() >= 4 && data.starts_with(b"\xcf\xfa\xed\xfe") {
            return true; // Mach-O binary
        }

        false
    }

    pub fn decode_image_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support
        self.validate_capability("image")?;

        // Enhanced security validation if available
        if let Some(ref validator) = self.content_security_validator {
            validator
                .validate_base64_security(data, "image")
                .map_err(|_e| Base64ProcessorError::SecurityValidationFailed)?;
        }

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

    pub fn decode_audio_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support
        self.validate_capability("audio")?;

        // Enhanced security validation if available
        if let Some(ref validator) = self.content_security_validator {
            validator
                .validate_base64_security(data, "audio")
                .map_err(|_e| Base64ProcessorError::SecurityValidationFailed)?;
        }

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

        // Enhanced security validation if available
        if let Some(ref validator) = self.content_security_validator {
            validator
                .validate_base64_security(data, "blob")
                .map_err(|_e| Base64ProcessorError::SecurityValidationFailed)?;
        }

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
