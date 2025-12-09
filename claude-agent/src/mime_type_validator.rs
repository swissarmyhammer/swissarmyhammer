use crate::error::ToJsonRpcError;
use serde_json::{json, Value};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum MimeTypeValidationError {
    #[error("Unsupported MIME type for {content_type}: {mime_type}")]
    UnsupportedMimeType {
        content_type: String,
        mime_type: String,
        allowed_types: Vec<String>,
        suggestion: Option<String>,
    },
    #[error("MIME type blocked for security reasons: {mime_type}")]
    SecurityBlocked {
        mime_type: String,
        reason: String,
        allowed_categories: Vec<String>,
    },
    #[error("MIME type format validation failed: expected {expected}, detected {detected}")]
    FormatMismatch {
        expected: String,
        detected: String,
        mime_type: String,
    },
    #[error("Invalid MIME type format: {mime_type}")]
    InvalidFormat { mime_type: String },
    #[error("Content validation failed: {details}")]
    ContentValidation { details: String },
}

impl ToJsonRpcError for MimeTypeValidationError {
    fn to_json_rpc_code(&self) -> i32 {
        -32602 // All MIME type validation errors are invalid params
    }

    fn to_error_data(&self) -> Option<Value> {
        let data = match self {
            Self::UnsupportedMimeType {
                content_type,
                mime_type,
                allowed_types,
                suggestion,
            } => json!({
                "error": "unsupported_mime_type",
                "contentType": content_type,
                "providedMimeType": mime_type,
                "allowedTypes": allowed_types,
                "suggestion": suggestion.as_ref().unwrap_or(&format!("Use one of the supported {} MIME types", content_type))
            }),
            Self::SecurityBlocked {
                mime_type,
                reason,
                allowed_categories,
            } => json!({
                "error": "mime_type_security_blocked",
                "providedMimeType": mime_type,
                "reason": reason,
                "allowedCategories": allowed_categories,
                "suggestion": "Use a MIME type from allowed categories"
            }),
            Self::FormatMismatch {
                expected,
                detected,
                mime_type,
            } => json!({
                "error": "mime_type_format_mismatch",
                "declaredMimeType": mime_type,
                "expectedFormat": expected,
                "detectedFormat": detected,
                "suggestion": "Ensure content data matches the declared MIME type"
            }),
            Self::InvalidFormat { mime_type } => json!({
                "error": "invalid_mime_type_format",
                "providedMimeType": mime_type,
                "suggestion": "Provide a valid MIME type in format 'type/subtype'"
            }),
            Self::ContentValidation { details } => json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format"
            }),
        };
        Some(data)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationLevel {
    Strict,
    Moderate,
    Permissive,
}

#[derive(Debug, Clone)]
pub struct MimeTypePolicy {
    pub validation_level: ValidationLevel,
    pub allowed_image_types: HashSet<String>,
    pub allowed_audio_types: HashSet<String>,
    pub allowed_resource_types: HashSet<String>,
    pub blocked_types: HashSet<String>,
    pub require_format_validation: bool,
    pub enable_security_filtering: bool,
}

impl Default for MimeTypePolicy {
    fn default() -> Self {
        Self::moderate()
    }
}

impl MimeTypePolicy {
    pub fn strict() -> Self {
        let mut allowed_image_types = HashSet::new();
        allowed_image_types.insert("image/png".to_string());
        allowed_image_types.insert("image/jpeg".to_string());
        allowed_image_types.insert("image/gif".to_string());
        allowed_image_types.insert("image/webp".to_string());

        let mut allowed_audio_types = HashSet::new();
        allowed_audio_types.insert("audio/wav".to_string());
        allowed_audio_types.insert("audio/mp3".to_string());
        allowed_audio_types.insert("audio/mpeg".to_string());
        allowed_audio_types.insert("audio/ogg".to_string());
        allowed_audio_types.insert("audio/aac".to_string());

        let mut allowed_resource_types = HashSet::new();
        allowed_resource_types.insert("text/plain".to_string());
        allowed_resource_types.insert("application/json".to_string());

        let mut blocked_types = HashSet::new();
        blocked_types.insert("application/x-executable".to_string());
        blocked_types.insert("application/x-msdownload".to_string());
        blocked_types.insert("application/x-msdos-program".to_string());
        blocked_types.insert("text/html".to_string());
        blocked_types.insert("application/javascript".to_string());

        Self {
            validation_level: ValidationLevel::Strict,
            allowed_image_types,
            allowed_audio_types,
            allowed_resource_types,
            blocked_types,
            require_format_validation: true,
            enable_security_filtering: true,
        }
    }

    pub fn moderate() -> Self {
        let mut policy = Self::strict();
        policy.validation_level = ValidationLevel::Moderate;

        // Add more resource types for moderate policy
        policy
            .allowed_resource_types
            .insert("text/html".to_string());
        policy.allowed_resource_types.insert("text/css".to_string());
        policy
            .allowed_resource_types
            .insert("text/x-python".to_string());
        policy
            .allowed_resource_types
            .insert("text/x-rust".to_string());
        policy
            .allowed_resource_types
            .insert("application/xml".to_string());
        policy
            .allowed_resource_types
            .insert("text/markdown".to_string());
        policy
            .allowed_resource_types
            .insert("application/pdf".to_string());

        // Remove text/html from blocked types for moderate policy
        policy.blocked_types.remove("text/html");

        policy
    }

    pub fn permissive() -> Self {
        let mut policy = Self::moderate();
        policy.validation_level = ValidationLevel::Permissive;
        policy.require_format_validation = false;
        policy.enable_security_filtering = false;

        // Add more permissive resource types
        policy
            .allowed_resource_types
            .insert("application/javascript".to_string());
        policy
            .allowed_resource_types
            .insert("application/octet-stream".to_string());

        // Clear blocked types for permissive policy
        policy.blocked_types.clear();

        policy
    }
}

#[derive(Clone)]
pub struct MimeTypeValidator {
    policy: MimeTypePolicy,
}

impl Default for MimeTypeValidator {
    fn default() -> Self {
        Self::new(MimeTypePolicy::default())
    }
}

impl MimeTypeValidator {
    pub fn new(policy: MimeTypePolicy) -> Self {
        Self { policy }
    }

    pub fn strict() -> Self {
        Self::new(MimeTypePolicy::strict())
    }

    pub fn moderate() -> Self {
        Self::new(MimeTypePolicy::moderate())
    }

    pub fn permissive() -> Self {
        Self::new(MimeTypePolicy::permissive())
    }

    // ACP requires comprehensive MIME type validation and security:
    // 1. Image: Validate against supported image formats
    // 2. Audio: Validate against supported audio formats
    // 3. Resources: Allow flexible MIME types with security filtering
    // 4. Security: Block dangerous MIME types and validate format matching
    // 5. Format validation: Ensure declared MIME type matches actual content
    //
    // MIME type validation prevents security issues and ensures proper content handling.

    pub fn validate_image_mime_type(
        &self,
        mime_type: &str,
        data: Option<&[u8]>,
    ) -> Result<(), MimeTypeValidationError> {
        // Check security blocking first
        if self.policy.enable_security_filtering && self.is_mime_type_blocked(mime_type) {
            return Err(MimeTypeValidationError::SecurityBlocked {
                mime_type: mime_type.to_string(),
                reason: "MIME type blocked for security reasons".to_string(),
                allowed_categories: vec!["image".to_string()],
            });
        }

        // Check if MIME type is allowed for images
        if !self.policy.allowed_image_types.contains(mime_type) {
            return Err(MimeTypeValidationError::UnsupportedMimeType {
                content_type: "image".to_string(),
                mime_type: mime_type.to_string(),
                allowed_types: self.policy.allowed_image_types.iter().cloned().collect(),
                suggestion: self.suggest_alternative_mime_type(mime_type, "image"),
            });
        }

        // Validate actual format matches declared MIME type
        if self.policy.require_format_validation {
            if let Some(data) = data {
                self.validate_image_format_matches_mime(data, mime_type)?;
            }
        }

        Ok(())
    }

    pub fn validate_audio_mime_type(
        &self,
        mime_type: &str,
        data: Option<&[u8]>,
    ) -> Result<(), MimeTypeValidationError> {
        // Check security blocking first
        if self.policy.enable_security_filtering && self.is_mime_type_blocked(mime_type) {
            return Err(MimeTypeValidationError::SecurityBlocked {
                mime_type: mime_type.to_string(),
                reason: "MIME type blocked for security reasons".to_string(),
                allowed_categories: vec!["audio".to_string()],
            });
        }

        // Check if MIME type is allowed for audio
        if !self.policy.allowed_audio_types.contains(mime_type) {
            return Err(MimeTypeValidationError::UnsupportedMimeType {
                content_type: "audio".to_string(),
                mime_type: mime_type.to_string(),
                allowed_types: self.policy.allowed_audio_types.iter().cloned().collect(),
                suggestion: self.suggest_alternative_mime_type(mime_type, "audio"),
            });
        }

        // Validate actual format matches declared MIME type
        if self.policy.require_format_validation {
            if let Some(data) = data {
                self.validate_audio_format_matches_mime(data, mime_type)?;
            }
        }

        Ok(())
    }

    pub fn validate_resource_mime_type(
        &self,
        mime_type: &str,
    ) -> Result<(), MimeTypeValidationError> {
        // Check security blocking first
        if self.policy.enable_security_filtering && self.is_mime_type_blocked(mime_type) {
            return Err(MimeTypeValidationError::SecurityBlocked {
                mime_type: mime_type.to_string(),
                reason: "MIME type blocked for security reasons".to_string(),
                allowed_categories: vec!["text".to_string(), "application".to_string()],
            });
        }

        // Check if MIME type is allowed for resources
        if !self.policy.allowed_resource_types.contains(mime_type) {
            return Err(MimeTypeValidationError::UnsupportedMimeType {
                content_type: "resource".to_string(),
                mime_type: mime_type.to_string(),
                allowed_types: self.policy.allowed_resource_types.iter().cloned().collect(),
                suggestion: self.suggest_alternative_mime_type(mime_type, "resource"),
            });
        }

        Ok(())
    }

    pub fn is_mime_type_secure(&self, mime_type: &str) -> bool {
        !self.is_mime_type_blocked(mime_type)
    }

    fn is_mime_type_blocked(&self, mime_type: &str) -> bool {
        self.policy.blocked_types.contains(mime_type)
    }

    fn suggest_alternative_mime_type(
        &self,
        _mime_type: &str,
        content_type: &str,
    ) -> Option<String> {
        match content_type {
            "image" => Some("Convert image to supported format like PNG or JPEG".to_string()),
            "audio" => Some("Convert audio to supported format like WAV or MP3".to_string()),
            "resource" => Some("Use plain text or JSON format".to_string()),
            _ => None,
        }
    }

    fn validate_image_format_matches_mime(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<(), MimeTypeValidationError> {
        let detected_format = self.detect_image_format(data);

        let expected_format = match mime_type {
            "image/png" => Some("png"),
            "image/jpeg" => Some("jpeg"),
            "image/gif" => Some("gif"),
            "image/webp" => Some("webp"),
            _ => None,
        };

        match (expected_format, detected_format.as_deref()) {
            (Some(expected), Some(detected)) => {
                if expected != detected {
                    return Err(MimeTypeValidationError::FormatMismatch {
                        expected: expected.to_string(),
                        detected: detected.to_string(),
                        mime_type: mime_type.to_string(),
                    });
                }
            }
            (Some(expected), None) => {
                // Expected a specific format but couldn't detect it - this is an error
                return Err(MimeTypeValidationError::FormatMismatch {
                    expected: expected.to_string(),
                    detected: "unknown".to_string(),
                    mime_type: mime_type.to_string(),
                });
            }
            _ => {}
        }

        Ok(())
    }

    fn validate_audio_format_matches_mime(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<(), MimeTypeValidationError> {
        let detected_format = self.detect_audio_format(data);

        let expected_format = match mime_type {
            "audio/wav" => Some("wav"),
            "audio/mp3" | "audio/mpeg" => Some("mp3"),
            "audio/ogg" => Some("ogg"),
            "audio/aac" => Some("aac"),
            _ => None,
        };

        match (expected_format, detected_format.as_deref()) {
            (Some(expected), Some(detected)) => {
                if expected != detected {
                    return Err(MimeTypeValidationError::FormatMismatch {
                        expected: expected.to_string(),
                        detected: detected.to_string(),
                        mime_type: mime_type.to_string(),
                    });
                }
            }
            (Some(expected), None) => {
                // Expected a specific format but couldn't detect it - this is an error
                return Err(MimeTypeValidationError::FormatMismatch {
                    expected: expected.to_string(),
                    detected: "unknown".to_string(),
                    mime_type: mime_type.to_string(),
                });
            }
            _ => {}
        }

        Ok(())
    }

    fn detect_image_format(&self, data: &[u8]) -> Option<String> {
        if data.len() < 2 {
            return None;
        }

        // Debug output for testing
        #[cfg(test)]
        {
            println!(
                "Detecting image format for {} bytes: {:02X?}",
                data.len(),
                &data[..data.len().min(12)]
            );
        }

        // PNG: starts with 8-byte signature
        if data.len() >= 8 && data.starts_with(b"\x89PNG\r\n\x1a\n") {
            #[cfg(test)]
            println!("Detected PNG format");
            Some("png".to_string())
        // JPEG: starts with FFD8
        } else if data.starts_with(b"\xFF\xD8") {
            #[cfg(test)]
            println!("Detected JPEG format");
            Some("jpeg".to_string())
        // GIF: starts with GIF87a or GIF89a
        } else if data.len() >= 6 && (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) {
            #[cfg(test)]
            println!("Detected GIF format");
            Some("gif".to_string())
        // WebP: RIFF....WEBP
        } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
            #[cfg(test)]
            println!("Detected WebP format");
            Some("webp".to_string())
        } else {
            #[cfg(test)]
            println!("Unknown image format");
            None
        }
    }

    fn detect_audio_format(&self, data: &[u8]) -> Option<String> {
        if data.len() < 4 {
            return None;
        }

        // WAV: RIFF....WAVE (need at least 12 bytes)
        if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
            Some("wav".to_string())
        // MP3: Frame sync bits FF Ex
        } else if data.len() >= 4 && data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 {
            Some("mp3".to_string())
        // OGG: starts with "OggS"
        } else if data.len() >= 4 && data.starts_with(b"OggS") {
            Some("ogg".to_string())
        // AAC ADTS: FF Fx
        } else if data.len() >= 7 && data[0] == 0xFF && (data[1] & 0xF0) == 0xF0 {
            Some("aac".to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mime_type_policy_levels() {
        let strict = MimeTypePolicy::strict();
        let moderate = MimeTypePolicy::moderate();
        let permissive = MimeTypePolicy::permissive();

        assert_eq!(strict.validation_level, ValidationLevel::Strict);
        assert_eq!(moderate.validation_level, ValidationLevel::Moderate);
        assert_eq!(permissive.validation_level, ValidationLevel::Permissive);

        // Strict should have fewer allowed resource types
        assert!(strict.allowed_resource_types.len() < moderate.allowed_resource_types.len());
        assert!(moderate.allowed_resource_types.len() <= permissive.allowed_resource_types.len());

        // Permissive should have no blocked types
        assert!(!strict.blocked_types.is_empty());
        assert!(permissive.blocked_types.is_empty());
    }

    #[test]
    fn test_validate_image_mime_type_allowed() {
        let validator = MimeTypeValidator::moderate();

        // Test allowed image MIME types
        assert!(validator
            .validate_image_mime_type("image/png", None)
            .is_ok());
        assert!(validator
            .validate_image_mime_type("image/jpeg", None)
            .is_ok());
        assert!(validator
            .validate_image_mime_type("image/gif", None)
            .is_ok());
        assert!(validator
            .validate_image_mime_type("image/webp", None)
            .is_ok());
    }

    #[test]
    fn test_validate_image_mime_type_disallowed() {
        let validator = MimeTypeValidator::moderate();

        // Test disallowed image MIME type
        let result = validator.validate_image_mime_type("image/tiff", None);
        assert!(result.is_err());

        if let Err(MimeTypeValidationError::UnsupportedMimeType {
            content_type,
            mime_type,
            allowed_types,
            suggestion,
        }) = result
        {
            assert_eq!(content_type, "image");
            assert_eq!(mime_type, "image/tiff");
            assert!(!allowed_types.is_empty());
            assert!(suggestion.is_some());
        } else {
            panic!("Expected UnsupportedMimeType error");
        }
    }

    #[test]
    fn test_validate_audio_mime_type_allowed() {
        let validator = MimeTypeValidator::moderate();

        // Test allowed audio MIME types
        assert!(validator
            .validate_audio_mime_type("audio/wav", None)
            .is_ok());
        assert!(validator
            .validate_audio_mime_type("audio/mp3", None)
            .is_ok());
        assert!(validator
            .validate_audio_mime_type("audio/mpeg", None)
            .is_ok());
        assert!(validator
            .validate_audio_mime_type("audio/ogg", None)
            .is_ok());
        assert!(validator
            .validate_audio_mime_type("audio/aac", None)
            .is_ok());
    }

    #[test]
    fn test_validate_resource_mime_type_allowed() {
        let validator = MimeTypeValidator::moderate();

        // Test allowed resource MIME types
        assert!(validator.validate_resource_mime_type("text/plain").is_ok());
        assert!(validator
            .validate_resource_mime_type("application/json")
            .is_ok());
        assert!(validator
            .validate_resource_mime_type("text/x-python")
            .is_ok());
        assert!(validator
            .validate_resource_mime_type("application/pdf")
            .is_ok());
    }

    #[test]
    fn test_security_blocking_strict_policy() {
        let validator = MimeTypeValidator::strict();

        // Test blocked MIME types
        let result = validator.validate_resource_mime_type("application/x-executable");
        assert!(result.is_err());

        if let Err(MimeTypeValidationError::SecurityBlocked {
            mime_type,
            reason,
            allowed_categories,
        }) = result
        {
            assert_eq!(mime_type, "application/x-executable");
            assert!(!reason.is_empty());
            assert!(!allowed_categories.is_empty());
        } else {
            panic!("Expected SecurityBlocked error");
        }
    }

    #[test]
    fn test_image_format_validation() {
        let validator = MimeTypeValidator::strict();

        // Valid PNG data
        let png_data = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        let result = validator.validate_image_mime_type("image/png", Some(png_data));
        println!("PNG validation result: {:?}", result);
        assert!(result.is_ok());

        // Invalid data for PNG MIME type
        let jpeg_data = b"\xFF\xD8\xFF\xE0\x00\x10JFIF";
        let detected_format = validator.detect_image_format(jpeg_data);
        println!("JPEG data detected as: {:?}", detected_format);
        let result = validator.validate_image_mime_type("image/png", Some(jpeg_data));
        println!("PNG validation with JPEG data result: {:?}", result);
        assert!(result.is_err());

        if let Err(MimeTypeValidationError::FormatMismatch {
            expected, detected, ..
        }) = result
        {
            assert_eq!(expected, "png");
            assert_eq!(detected, "jpeg");
        } else {
            panic!("Expected FormatMismatch error but got: {:?}", result);
        }
    }

    #[test]
    fn test_audio_format_validation() {
        let validator = MimeTypeValidator::strict();

        // Valid WAV data
        let wav_data = b"RIFF\x24\x08\x00\x00WAVE";
        let result = validator.validate_audio_mime_type("audio/wav", Some(wav_data));
        println!("WAV validation result: {:?}", result);
        assert!(result.is_ok());

        // Invalid data for WAV MIME type
        let mp3_data = b"\xFF\xFB\x90\x00";
        let detected_format = validator.detect_audio_format(mp3_data);
        println!("MP3 data detected as: {:?}", detected_format);
        let result = validator.validate_audio_mime_type("audio/wav", Some(mp3_data));
        println!("WAV validation with MP3 data result: {:?}", result);
        assert!(result.is_err());

        if let Err(MimeTypeValidationError::FormatMismatch {
            expected, detected, ..
        }) = result
        {
            assert_eq!(expected, "wav");
            assert_eq!(detected, "mp3");
        } else {
            panic!("Expected FormatMismatch error but got: {:?}", result);
        }
    }

    #[test]
    fn test_permissive_policy_allows_more() {
        let strict = MimeTypeValidator::strict();
        let permissive = MimeTypeValidator::permissive();

        // This should be blocked in strict but allowed in permissive
        assert!(strict
            .validate_resource_mime_type("application/javascript")
            .is_err());
        assert!(permissive
            .validate_resource_mime_type("application/javascript")
            .is_ok());
    }

    #[test]
    fn test_format_detection() {
        let validator = MimeTypeValidator::default();

        // Test PNG detection - use exact PNG header
        let png_data = b"\x89PNG\r\n\x1a\n";
        println!("PNG test data bytes: {:02X?}", png_data);
        let detected = validator.detect_image_format(png_data);
        println!("PNG detected: {:?}", detected);
        assert_eq!(detected, Some("png".to_string()));

        // Test JPEG detection
        let jpeg_data = b"\xFF\xD8\xFF\xE0";
        let detected = validator.detect_image_format(jpeg_data);
        assert_eq!(detected, Some("jpeg".to_string()));

        // Test WAV detection - use exact RIFF/WAVE header
        let wav_data = b"RIFF\x24\x08\x00\x00WAVE";
        println!("WAV test data bytes: {:02X?}", wav_data);
        let detected = validator.detect_audio_format(wav_data);
        println!("WAV detected: {:?}", detected);
        assert_eq!(detected, Some("wav".to_string()));

        // Test MP3 detection
        let mp3_data = b"\xFF\xFB\x90\x00";
        let detected = validator.detect_audio_format(mp3_data);
        assert_eq!(detected, Some("mp3".to_string()));
    }

    #[test]
    fn test_png_detection_basic() {
        let validator = MimeTypeValidator::default();
        let png_header = b"\x89PNG\r\n\x1a\n";
        let result = validator.detect_image_format(png_header);
        println!("Basic PNG detection: {:?}", result);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "png");
    }

    #[test]
    fn test_security_methods() {
        let validator = MimeTypeValidator::strict();

        // Test security checking
        assert!(validator.is_mime_type_secure("image/png"));
        assert!(!validator.is_mime_type_secure("application/x-executable"));
    }
}
