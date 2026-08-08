use crate::error::ToJsonRpcError;
use crate::json_rpc_codes::INVALID_PARAMS;
use serde_json::{json, Value};
use std::collections::HashSet;
use thiserror::Error;

/// Reason recorded on [`MimeTypeValidationError::SecurityBlocked`].
const SECURITY_BLOCKED_REASON: &str = "MIME type blocked for security reasons";

/// Format name reported when magic-byte detection recognises nothing.
const UNKNOWN_FORMAT: &str = "unknown";

/// MIME type to expected image format, as magic-byte detection names it.
const IMAGE_MIME_FORMATS: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpeg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
];

/// MIME type to expected audio format, as magic-byte detection names it.
const AUDIO_MIME_FORMATS: &[(&str, &str)] = &[
    ("audio/wav", "wav"),
    ("audio/mp3", "mp3"),
    ("audio/mpeg", "mp3"),
    ("audio/ogg", "ogg"),
    ("audio/aac", "aac"),
];

/// A per-category magic-byte check, as [`MimeTypeValidator`] passes it around.
type FormatValidator = fn(&MimeTypeValidator, &[u8], &str) -> Result<(), MimeTypeValidationError>;

/// Why [`MimeTypeValidator`] rejected a MIME type or a payload.
///
/// The variants cover the three gates the validator runs: the security
/// deny-list, the per-category allow-list, and the magic-byte format check.
#[derive(Debug, Error, Clone)]
pub enum MimeTypeValidationError {
    /// The MIME type is off the allow-list for this content category.
    #[error("unsupported MIME type for {content_type}: {mime_type}")]
    UnsupportedMimeType {
        /// Content category being validated, such as `image` or `audio`.
        content_type: String,
        /// MIME type the caller declared.
        mime_type: String,
        /// Every MIME type the category accepts.
        allowed_types: Vec<String>,
        /// Advice for the caller, when the validator can offer any.
        suggestion: Option<String>,
    },
    /// The MIME type is on the policy's security deny-list.
    #[error("MIME type blocked for security reasons: {mime_type}")]
    SecurityBlocked {
        /// MIME type the caller declared.
        mime_type: String,
        /// Why the deny-list holds this MIME type.
        reason: String,
        /// Content categories the caller may use instead.
        allowed_categories: Vec<String>,
    },
    /// The magic bytes of the payload disagree with the declared MIME type.
    #[error("MIME type format validation failed: expected {expected}, detected {detected}")]
    FormatMismatch {
        /// Format the declared MIME type calls for.
        expected: String,
        /// Format the magic bytes point to, or `unknown`.
        detected: String,
        /// MIME type the caller declared.
        mime_type: String,
    },
    /// The MIME type is not written as `type/subtype`.
    #[error("invalid MIME type format: {mime_type}")]
    InvalidFormat {
        /// MIME type the caller declared.
        mime_type: String,
    },
    /// Content-level validation failed. The payload names the fault.
    #[error("content validation failed: {details}")]
    ContentValidation {
        /// What the validator objected to.
        details: String,
    },
}

impl ToJsonRpcError for MimeTypeValidationError {
    fn to_json_rpc_code(&self) -> i32 {
        // Every MIME type validation error is a caller mistake.
        INVALID_PARAMS
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

/// How aggressively MIME types and binary contents should be validated.
///
/// Used by [`MimeTypePolicy`] to pick defaults for allow-lists, format
/// matching, and security filtering.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationLevel {
    /// Only a small curated set of MIME types is accepted and binary payloads
    /// must match their declared format exactly.
    Strict,
    /// A broader set of text/document MIME types is allowed, but magic-byte
    /// validation and security filtering are still active.
    Moderate,
    /// Most MIME types are accepted without magic-byte validation or security
    /// filtering. Suitable for trusted/debug-only contexts.
    Permissive,
}

/// The MIME type rules a [`MimeTypeValidator`] enforces.
///
/// A policy holds one allow-list for each content category, a deny-list that
/// overrides them, and two switches that turn magic-byte matching and security
/// filtering on or off. Build one with [`MimeTypePolicy::strict`],
/// [`MimeTypePolicy::moderate`] or [`MimeTypePolicy::permissive`].
#[derive(Debug, Clone)]
pub struct MimeTypePolicy {
    /// How aggressively this policy validates, for reporting and comparison.
    pub validation_level: ValidationLevel,
    /// MIME types accepted for image content.
    pub allowed_image_types: HashSet<String>,
    /// MIME types accepted for audio content.
    pub allowed_audio_types: HashSet<String>,
    /// MIME types accepted for embedded resource content.
    pub allowed_resource_types: HashSet<String>,
    /// MIME types refused outright, whatever the allow-lists say.
    pub blocked_types: HashSet<String>,
    /// Whether payload magic bytes must match the declared MIME type.
    pub require_format_validation: bool,
    /// Whether the deny-list is consulted at all.
    pub enable_security_filtering: bool,
}

impl Default for MimeTypePolicy {
    fn default() -> Self {
        Self::moderate()
    }
}

impl MimeTypePolicy {
    /// Build a [`MimeTypePolicy`] tuned for untrusted input — only core
    /// image/audio/resource types are allowed and payloads must pass
    /// magic-byte validation.
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

    /// Build a [`MimeTypePolicy`] that accepts a wider set of text/document
    /// resource types but still validates binary formats and applies security
    /// filtering. This is the `Default` policy.
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

    /// Build a [`MimeTypePolicy`] that accepts most MIME types without any
    /// format or security checks. Only appropriate for trusted/debug contexts.
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

/// Checks a declared MIME type, and optionally the payload behind it, against
/// a [`MimeTypePolicy`].
///
/// Each `validate_*_mime_type` method runs the same three gates: the security
/// deny-list, the allow-list for that content category, and — for image and
/// audio, when the policy asks for it — a magic-byte check that the payload
/// really is the format the MIME type declares.
#[derive(Clone, Debug)]
pub struct MimeTypeValidator {
    policy: MimeTypePolicy,
}

impl Default for MimeTypeValidator {
    fn default() -> Self {
        Self::new(MimeTypePolicy::default())
    }
}

impl MimeTypeValidator {
    /// Build a validator from an explicit [`MimeTypePolicy`].
    pub fn new(policy: MimeTypePolicy) -> Self {
        Self { policy }
    }

    /// Convenience constructor for [`MimeTypePolicy::strict`].
    pub fn strict() -> Self {
        Self::new(MimeTypePolicy::strict())
    }

    /// Convenience constructor for [`MimeTypePolicy::moderate`].
    pub fn moderate() -> Self {
        Self::new(MimeTypePolicy::moderate())
    }

    /// Convenience constructor for [`MimeTypePolicy::permissive`].
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

    /// Validate `mime_type` — and optionally `data` — for one content category.
    ///
    /// Runs the deny-list, then the `allowed_types` allow-list, then the
    /// magic-byte check when the policy asks for one, `validate_format` is
    /// supplied and `data` is present. `allowed_categories` names the
    /// categories reported on a deny-list rejection. Every public
    /// `validate_*_mime_type` method is this method with its own arguments.
    fn validate_mime_type_for_category(
        &self,
        mime_type: &str,
        data: Option<&[u8]>,
        category: &str,
        allowed_types: &HashSet<String>,
        allowed_categories: &[&str],
        validate_format: Option<FormatValidator>,
    ) -> Result<(), MimeTypeValidationError> {
        // Check security blocking first
        if self.policy.enable_security_filtering && self.is_mime_type_blocked(mime_type) {
            return Err(MimeTypeValidationError::SecurityBlocked {
                mime_type: mime_type.to_string(),
                reason: SECURITY_BLOCKED_REASON.to_string(),
                allowed_categories: allowed_categories
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect(),
            });
        }

        // Check if MIME type is allowed for this category
        if !allowed_types.contains(mime_type) {
            return Err(MimeTypeValidationError::UnsupportedMimeType {
                content_type: category.to_string(),
                mime_type: mime_type.to_string(),
                allowed_types: allowed_types.iter().cloned().collect(),
                suggestion: self.suggest_alternative_mime_type(mime_type, category),
            });
        }

        // Validate actual format matches declared MIME type
        match (self.policy.require_format_validation, validate_format, data) {
            (true, Some(validate), Some(data)) => validate(self, data, mime_type),
            _ => Ok(()),
        }
    }

    /// Validate `mime_type` for image content, and match `data` against it.
    ///
    /// Pass `data` to have the payload's magic bytes checked against the
    /// declared MIME type; pass `None` to check the MIME type alone. The
    /// magic-byte check runs only when the policy sets
    /// [`MimeTypePolicy::require_format_validation`].
    ///
    /// # Errors
    ///
    /// Returns [`MimeTypeValidationError::SecurityBlocked`] when the deny-list
    /// holds `mime_type`, [`MimeTypeValidationError::UnsupportedMimeType`] when
    /// it is off the image allow-list, and
    /// [`MimeTypeValidationError::FormatMismatch`] when `data` is not the
    /// declared format.
    pub fn validate_image_mime_type(
        &self,
        mime_type: &str,
        data: Option<&[u8]>,
    ) -> Result<(), MimeTypeValidationError> {
        self.validate_mime_type_for_category(
            mime_type,
            data,
            "image",
            &self.policy.allowed_image_types,
            &["image"],
            Some(Self::validate_image_format_matches_mime),
        )
    }

    /// Validate `mime_type` for audio content, and match `data` against it.
    ///
    /// Pass `data` to have the payload's magic bytes checked against the
    /// declared MIME type; pass `None` to check the MIME type alone. The
    /// magic-byte check runs only when the policy sets
    /// [`MimeTypePolicy::require_format_validation`].
    ///
    /// # Errors
    ///
    /// Returns [`MimeTypeValidationError::SecurityBlocked`] when the deny-list
    /// holds `mime_type`, [`MimeTypeValidationError::UnsupportedMimeType`] when
    /// it is off the audio allow-list, and
    /// [`MimeTypeValidationError::FormatMismatch`] when `data` is not the
    /// declared format.
    pub fn validate_audio_mime_type(
        &self,
        mime_type: &str,
        data: Option<&[u8]>,
    ) -> Result<(), MimeTypeValidationError> {
        self.validate_mime_type_for_category(
            mime_type,
            data,
            "audio",
            &self.policy.allowed_audio_types,
            &["audio"],
            Some(Self::validate_audio_format_matches_mime),
        )
    }

    /// Validate `mime_type` for embedded resource content.
    ///
    /// Resources carry no binary format the validator can recognise, so only
    /// the deny-list and the resource allow-list apply.
    ///
    /// # Errors
    ///
    /// Returns [`MimeTypeValidationError::SecurityBlocked`] when the deny-list
    /// holds `mime_type`, and
    /// [`MimeTypeValidationError::UnsupportedMimeType`] when it is off the
    /// resource allow-list.
    pub fn validate_resource_mime_type(
        &self,
        mime_type: &str,
    ) -> Result<(), MimeTypeValidationError> {
        self.validate_mime_type_for_category(
            mime_type,
            None,
            "resource",
            &self.policy.allowed_resource_types,
            &["text", "application"],
            None,
        )
    }

    /// Return `true` when the MIME type is not on the policy's deny-list — the
    /// logical inverse of [`Self::is_mime_type_blocked`]. Useful as a simple
    /// security pre-check before more expensive validation.
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

    /// Match the magic bytes of `data` against the format `mime_type` declares.
    ///
    /// `mime_formats` maps a MIME type to the format name `detect` reports. A
    /// MIME type the table does not list carries no format expectation, so it
    /// passes. A payload `detect` cannot recognise is reported as
    /// [`UNKNOWN_FORMAT`] and is a mismatch.
    fn validate_format_matches_mime(
        &self,
        data: &[u8],
        mime_type: &str,
        detect: fn(&Self, &[u8]) -> Option<String>,
        mime_formats: &[(&str, &str)],
    ) -> Result<(), MimeTypeValidationError> {
        let Some(expected) = mime_formats
            .iter()
            .find(|(mime, _)| *mime == mime_type)
            .map(|(_, format)| *format)
        else {
            return Ok(());
        };

        let detected = detect(self, data);
        let detected = detected.as_deref().unwrap_or(UNKNOWN_FORMAT);
        if detected == expected {
            return Ok(());
        }

        Err(MimeTypeValidationError::FormatMismatch {
            expected: expected.to_string(),
            detected: detected.to_string(),
            mime_type: mime_type.to_string(),
        })
    }

    /// Match `data` against the image format `mime_type` declares.
    fn validate_image_format_matches_mime(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<(), MimeTypeValidationError> {
        self.validate_format_matches_mime(
            data,
            mime_type,
            Self::detect_image_format,
            IMAGE_MIME_FORMATS,
        )
    }

    /// Match `data` against the audio format `mime_type` declares.
    fn validate_audio_format_matches_mime(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<(), MimeTypeValidationError> {
        self.validate_format_matches_mime(
            data,
            mime_type,
            Self::detect_audio_format,
            AUDIO_MIME_FORMATS,
        )
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
