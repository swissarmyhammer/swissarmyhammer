use crate::base64_processor::Base64ProcessorError;
use crate::content_block_processor::ContentBlockProcessorError;
use crate::content_security_validator::ContentSecurityError;
use crate::error::{JsonRpcError, ToJsonRpcError};
use crate::mime_type_validator::MimeTypeValidationError;
use serde_json::{json, Value};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

// ACP content processing requires comprehensive error handling:
// 1. Validation errors: Clear messages for malformed content
// 2. Capability errors: Explain capability requirements
// 3. Size limit errors: Include limit information
// 4. Security errors: Generic messages to avoid information disclosure
// 5. Format errors: Suggest corrective actions
//
// All errors must include structured data for client handling.

/// Content processing error with ACP compliance
#[derive(Debug, Error)]
pub enum ContentProcessingError {
    #[error("Invalid content block structure: {0}")]
    InvalidStructure(String),

    #[error("Unsupported content type: {content_type}, supported types: {supported:?}")]
    UnsupportedContentType {
        content_type: String,
        supported: Vec<String>,
    },

    #[error("Invalid base64 data: {0}")]
    InvalidBase64(String),

    #[error("Content size exceeded: {size} > {limit}")]
    ContentSizeExceeded { size: usize, limit: usize },

    #[error("MIME type validation failed: {mime_type} does not match content format")]
    MimeTypeMismatch { mime_type: String },

    #[error("Content capability not supported: {capability}")]
    CapabilityNotSupported { capability: String },

    #[error("Security validation failed: {reason}")]
    SecurityViolation { reason: String },

    #[error("Memory pressure: insufficient memory for content processing")]
    MemoryPressure,

    #[error("Resource contention: processing queue full")]
    ResourceContention,

    #[error("Invalid URI format: {uri}")]
    InvalidUri { uri: String },

    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },

    #[error("Content validation failed: {details}")]
    ContentValidationFailed { details: String },

    #[error("Format detection failed: {reason}")]
    FormatDetectionFailed { reason: String },
}

impl ToJsonRpcError for ContentProcessingError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            Self::InvalidStructure(_)
            | Self::UnsupportedContentType { .. }
            | Self::InvalidBase64(_)
            | Self::ContentSizeExceeded { .. }
            | Self::MimeTypeMismatch { .. }
            | Self::CapabilityNotSupported { .. }
            | Self::SecurityViolation { .. }
            | Self::InvalidUri { .. }
            | Self::MissingRequiredField { .. }
            | Self::ContentValidationFailed { .. }
            | Self::FormatDetectionFailed { .. } => -32602, // Invalid params
            Self::MemoryPressure | Self::ResourceContention => -32603, // Internal error
        }
    }

    fn to_error_data(&self) -> Option<Value> {
        let data = match self {
            Self::InvalidStructure(details) => json!({
                "error": "invalid_structure",
                "details": details,
                "suggestion": "Verify content block follows ACP specification"
            }),
            Self::UnsupportedContentType {
                content_type,
                supported,
            } => json!({
                "error": "unsupported_content_type",
                "contentType": content_type,
                "supportedTypes": supported,
                "suggestion": "Use one of the supported content types"
            }),
            Self::InvalidBase64(details) => json!({
                "error": "invalid_base64_format",
                "details": details,
                "suggestion": "Ensure base64 data is properly encoded with correct padding"
            }),
            Self::ContentSizeExceeded { size, limit } => json!({
                "error": "content_size_exceeded",
                "providedSize": size,
                "maxSize": limit,
                "suggestion": "Reduce content size or split into smaller parts"
            }),
            Self::MimeTypeMismatch { mime_type } => json!({
                "error": "mime_type_mismatch",
                "mimeType": mime_type,
                "suggestion": "Ensure content data matches the declared MIME type"
            }),
            Self::CapabilityNotSupported { capability } => json!({
                "error": "capability_not_supported",
                "requiredCapability": capability,
                "declaredValue": false,
                "suggestion": "Check agent capabilities before sending content"
            }),
            Self::SecurityViolation { .. } => json!({
                "error": "security_violation",
                "suggestion": "Content failed security validation checks"
            }),
            Self::MemoryPressure => json!({
                "error": "memory_pressure",
                "suggestion": "Reduce content size or retry later"
            }),
            Self::ResourceContention => json!({
                "error": "resource_contention",
                "suggestion": "Retry request after a brief delay"
            }),
            Self::InvalidUri { uri } => json!({
                "error": "invalid_uri",
                "uri": uri,
                "suggestion": "Provide a valid URI with proper scheme"
            }),
            Self::MissingRequiredField { field } => json!({
                "error": "missing_required_field",
                "field": field,
                "suggestion": "Ensure all required fields are present"
            }),
            Self::ContentValidationFailed { details } => json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format"
            }),
            Self::FormatDetectionFailed { reason } => json!({
                "error": "format_detection_failed",
                "reason": reason,
                "suggestion": "Ensure content format matches MIME type declaration"
            }),
        };
        Some(data)
    }
}

/// Error context for debugging and correlation
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub correlation_id: String,
    pub processing_stage: String,
    pub content_type: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self {
            correlation_id: Uuid::new_v4().to_string(),
            processing_stage: "unknown".to_string(),
            content_type: None,
            metadata: HashMap::new(),
        }
    }
}

// TODO: use the To/From trait system for type conversions

/// Convert ContentSecurityError to ACP-compliant JSON-RPC error
pub fn convert_content_security_error_to_acp(
    error: ContentSecurityError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let mut json_rpc_error = error.to_json_rpc_error();

    // Add correlation context if available
    if let Some(ctx) = context {
        if let Some(ref mut data) = json_rpc_error.data {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("correlationId".to_string(), json!(ctx.correlation_id));
                obj.insert("stage".to_string(), json!(ctx.processing_stage));
                if let Some(content_type) = ctx.content_type {
                    obj.insert("contentType".to_string(), json!(content_type));
                }
            }
        }
    }

    json_rpc_error
}

/// Convert Base64ProcessorError to ACP-compliant JSON-RPC error
pub fn convert_base64_error_to_acp(
    error: Base64ProcessorError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let mut json_rpc_error = error.to_json_rpc_error();

    // Add correlation context if available
    if let Some(ctx) = context {
        if let Some(ref mut data) = json_rpc_error.data {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("correlationId".to_string(), json!(ctx.correlation_id));
                obj.insert("stage".to_string(), json!(ctx.processing_stage));
                if let Some(content_type) = ctx.content_type {
                    obj.insert("contentType".to_string(), json!(content_type));
                }
            }
        }
    }

    json_rpc_error
}

/// Convert MimeTypeValidationError to ACP-compliant JSON-RPC error
pub fn convert_mime_type_error_to_acp(
    error: MimeTypeValidationError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let mut json_rpc_error = error.to_json_rpc_error();

    // Add correlation context if available
    if let Some(ctx) = context {
        if let Some(ref mut data) = json_rpc_error.data {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("correlationId".to_string(), json!(ctx.correlation_id));
                obj.insert("stage".to_string(), json!(ctx.processing_stage));
                if let Some(content_type) = ctx.content_type {
                    obj.insert("contentType".to_string(), json!(content_type));
                }
            }
        }
    }

    json_rpc_error
}

/// Convert ContentBlockProcessorError to ACP-compliant JSON-RPC error
pub fn convert_content_block_error_to_acp(
    error: ContentBlockProcessorError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let mut json_rpc_error = error.to_json_rpc_error();

    // Add correlation context if available
    if let Some(ctx) = context {
        if let Some(ref mut data) = json_rpc_error.data {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("correlationId".to_string(), json!(ctx.correlation_id));
                obj.insert("stage".to_string(), json!(ctx.processing_stage));
                if let Some(content_type) = ctx.content_type {
                    obj.insert("contentType".to_string(), json!(content_type));
                }
            }
        }
    }

    json_rpc_error
}
/// Convert enhanced ContentProcessingError to ACP-compliant JSON-RPC error
pub fn convert_content_processing_error_to_acp(
    error: ContentProcessingError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let mut json_rpc_error = error.to_json_rpc_error();

    // Add correlation context if available
    if let Some(ctx) = context {
        if let Some(ref mut data) = json_rpc_error.data {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("correlationId".to_string(), json!(ctx.correlation_id));
                obj.insert("stage".to_string(), json!(ctx.processing_stage));
                if let Some(content_type) = ctx.content_type {
                    obj.insert("contentType".to_string(), json!(content_type));
                }
            }
        }
    }

    json_rpc_error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_error_conversion() {
        let error = Base64ProcessorError::InvalidBase64("Invalid padding".to_string());
        let context = ErrorContext {
            correlation_id: "test-123".to_string(),
            processing_stage: "validation".to_string(),
            content_type: Some("image".to_string()),
            metadata: HashMap::new(),
        };

        let json_rpc_error = convert_base64_error_to_acp(error, Some(context));

        assert_eq!(json_rpc_error.code, -32602);
        assert_eq!(
            json_rpc_error.message,
            "Invalid base64 format: Invalid padding"
        );

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "invalid_base64_format");
            assert_eq!(data["correlationId"], "test-123");
            assert_eq!(data["stage"], "validation");
            assert!(data["suggestion"].as_str().unwrap().contains("base64"));
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_size_exceeded_error_conversion() {
        let error = Base64ProcessorError::SizeExceeded {
            limit: 1024,
            actual: 2048,
        };

        let context = ErrorContext {
            correlation_id: "test-456".to_string(),
            processing_stage: "size_check".to_string(),
            content_type: None,
            metadata: HashMap::new(),
        };

        let json_rpc_error = convert_base64_error_to_acp(error, Some(context));

        assert_eq!(json_rpc_error.code, -32602);
        assert_eq!(
            json_rpc_error.message,
            "Data exceeds maximum size limit of 1024 bytes (actual: 2048)"
        );

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "content_size_exceeded");
            assert_eq!(data["providedSize"], 2048);
            assert_eq!(data["maxSize"], 1024);
            assert_eq!(data["correlationId"], "test-456");
            assert_eq!(data["stage"], "size_check");
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_content_processing_error_conversion() {
        let error = ContentProcessingError::CapabilityNotSupported {
            capability: "audio".to_string(),
        };

        let json_rpc_error = convert_content_processing_error_to_acp(error, None);

        assert_eq!(json_rpc_error.code, -32602);
        assert!(json_rpc_error.message.contains("audio"));

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "capability_not_supported");
            assert_eq!(data["requiredCapability"], "audio");
            assert_eq!(data["declaredValue"], false);
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_security_violation_no_info_disclosure() {
        let error = ContentProcessingError::SecurityViolation {
            reason: "sensitive internal details".to_string(),
        };

        let json_rpc_error = convert_content_processing_error_to_acp(error, None);

        assert_eq!(json_rpc_error.code, -32602);
        assert_eq!(
            json_rpc_error.message,
            "Security validation failed: sensitive internal details"
        );

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "security_violation");
            // Ensure sensitive reason is not included
            assert!(data.get("reason").is_none());
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_default_error_context() {
        let context = ErrorContext::default();

        assert!(!context.correlation_id.is_empty());
        assert_eq!(context.processing_stage, "unknown");
        assert!(context.content_type.is_none());
        assert!(context.metadata.is_empty());
    }

    #[test]
    fn test_content_processing_error_invalid_structure() {
        let error = ContentProcessingError::InvalidStructure("malformed json".to_string());
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
        assert!(json_rpc_error
            .message
            .contains("Invalid content block structure"));

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "invalid_structure");
            assert_eq!(data["details"], "malformed json");
        }
    }

    #[test]
    fn test_content_processing_error_unsupported_content_type() {
        let error = ContentProcessingError::UnsupportedContentType {
            content_type: "video".to_string(),
            supported: vec!["text".to_string(), "image".to_string()],
        };
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
    }

    #[test]
    fn test_content_processing_error_invalid_base64() {
        let error = ContentProcessingError::InvalidBase64("bad padding".to_string());
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
        assert!(json_rpc_error.message.contains("Invalid base64"));
    }

    #[test]
    fn test_content_processing_error_content_size_exceeded() {
        let error = ContentProcessingError::ContentSizeExceeded {
            size: 5000,
            limit: 4096,
        };
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["providedSize"], 5000);
            assert_eq!(data["maxSize"], 4096);
        }
    }

    #[test]
    fn test_content_processing_error_mime_type_mismatch() {
        let error = ContentProcessingError::MimeTypeMismatch {
            mime_type: "image/png".to_string(),
        };
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
    }

    #[test]
    fn test_content_processing_error_memory_pressure() {
        let error = ContentProcessingError::MemoryPressure;
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32603);
        assert!(json_rpc_error.message.contains("memory"));
    }

    #[test]
    fn test_content_processing_error_resource_contention() {
        let error = ContentProcessingError::ResourceContention;
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32603);
    }

    #[test]
    fn test_content_processing_error_invalid_uri() {
        let error = ContentProcessingError::InvalidUri {
            uri: "not a uri".to_string(),
        };
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
    }

    #[test]
    fn test_content_processing_error_missing_required_field() {
        let error = ContentProcessingError::MissingRequiredField {
            field: "data".to_string(),
        };
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
    }

    #[test]
    fn test_content_processing_error_content_validation_failed() {
        let error = ContentProcessingError::ContentValidationFailed {
            details: "invalid format".to_string(),
        };
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
    }

    #[test]
    fn test_content_processing_error_format_detection_failed() {
        let error = ContentProcessingError::FormatDetectionFailed {
            reason: "unknown format".to_string(),
        };
        let json_rpc_error = error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, -32602);
    }

    #[test]
    fn test_convert_content_security_error_with_context() {
        use crate::content_security_validator::ContentSecurityError;

        let error = ContentSecurityError::SecurityValidationFailed {
            reason: "blocked".to_string(),
            policy_violated: "strict".to_string(),
        };

        let context = ErrorContext {
            correlation_id: "test-sec-001".to_string(),
            processing_stage: "security_scan".to_string(),
            content_type: Some("image".to_string()),
            metadata: HashMap::new(),
        };

        let json_rpc_error = convert_content_security_error_to_acp(error, Some(context));

        assert_eq!(json_rpc_error.code, -32602);
        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["correlationId"], "test-sec-001");
            assert_eq!(data["stage"], "security_scan");
            assert_eq!(data["contentType"], "image");
        }
    }

    #[test]
    fn test_convert_mime_type_error_with_context() {
        let error = MimeTypeValidationError::UnsupportedMimeType {
            content_type: "image".to_string(),
            mime_type: "image/bmp".to_string(),
            allowed_types: vec!["image/png".to_string()],
            suggestion: Some("use PNG".to_string()),
        };

        let context = ErrorContext {
            correlation_id: "test-mime-001".to_string(),
            processing_stage: "mime_validation".to_string(),
            content_type: Some("image".to_string()),
            metadata: HashMap::new(),
        };

        let json_rpc_error = convert_mime_type_error_to_acp(error, Some(context));

        assert_eq!(json_rpc_error.code, -32602);
        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["correlationId"], "test-mime-001");
        }
    }

    #[test]
    fn test_convert_content_block_error_with_context() {
        let error = ContentBlockProcessorError::UnsupportedContentType("invalid".to_string());

        let context = ErrorContext {
            correlation_id: "test-cb-001".to_string(),
            processing_stage: "block_processing".to_string(),
            content_type: None,
            metadata: HashMap::new(),
        };

        let json_rpc_error = convert_content_block_error_to_acp(error, Some(context));

        assert_eq!(json_rpc_error.code, -32602);
        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["correlationId"], "test-cb-001");
        }
    }

    #[test]
    fn test_error_context_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("request_id".to_string(), "req-123".to_string());
        metadata.insert("user_id".to_string(), "user-456".to_string());

        let context = ErrorContext {
            correlation_id: "corr-789".to_string(),
            processing_stage: "validation".to_string(),
            content_type: Some("audio".to_string()),
            metadata,
        };

        assert_eq!(context.correlation_id, "corr-789");
        assert_eq!(context.processing_stage, "validation");
        assert_eq!(context.content_type, Some("audio".to_string()));
        assert_eq!(context.metadata.len(), 2);
    }
}
