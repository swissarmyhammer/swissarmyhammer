use crate::constants::sizes;
use thiserror::Error;

/// Unified error type for size validation failures
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SizeValidationError {
    #[error("Size exceeds limit for {field}: {actual} bytes > {limit} bytes")]
    SizeExceeded {
        field: String,
        actual: usize,
        limit: usize,
    },
}

/// Configurable size limits for various validation contexts
#[derive(Debug, Clone, PartialEq)]
pub struct SizeLimits {
    pub max_path_length: usize,
    pub max_uri_length: usize,
    pub max_base64_size: usize,
    pub max_content_size: usize,
    pub max_meta_size: usize,
}

impl Default for SizeLimits {
    fn default() -> Self {
        Self {
            max_path_length: sizes::fs::MAX_PATH_LENGTH,
            max_uri_length: sizes::uri::MAX_URI_LENGTH_EXTENDED,
            max_base64_size: sizes::content::MAX_CONTENT_MODERATE,
            max_content_size: sizes::content::MAX_RESOURCE_MODERATE,
            max_meta_size: sizes::content::MAX_META_SIZE,
        }
    }
}

impl SizeLimits {
    /// Create strict size limits for high-security contexts
    pub fn strict() -> Self {
        Self {
            max_path_length: sizes::fs::MAX_PATH_LENGTH_STRICT,
            max_uri_length: sizes::uri::MAX_URI_LENGTH,
            max_base64_size: sizes::content::MAX_CONTENT_STRICT,
            max_content_size: sizes::content::MAX_RESOURCE_STRICT,
            max_meta_size: sizes::content::MAX_META_SIZE / 10,
        }
    }

    /// Create permissive size limits for low-security contexts
    pub fn permissive() -> Self {
        Self {
            max_path_length: sizes::fs::MAX_PATH_LENGTH * 2,
            max_uri_length: sizes::uri::MAX_URI_LENGTH_EXTENDED * 2,
            max_base64_size: sizes::content::MAX_CONTENT_PERMISSIVE,
            max_content_size: sizes::content::MAX_RESOURCE_PERMISSIVE,
            max_meta_size: sizes::content::MAX_META_SIZE * 10,
        }
    }
}

/// Validator for size-related constraints across the codebase
#[derive(Debug, Clone)]
pub struct SizeValidator {
    limits: SizeLimits,
}

impl Default for SizeValidator {
    fn default() -> Self {
        Self::new(SizeLimits::default())
    }
}

impl SizeValidator {
    /// Create a new size validator with the given limits
    pub fn new(limits: SizeLimits) -> Self {
        Self { limits }
    }

    /// Validate a generic size against a specific limit
    pub fn validate_size(
        &self,
        actual: usize,
        limit: usize,
        field_name: &str,
    ) -> Result<(), SizeValidationError> {
        if actual > limit {
            return Err(SizeValidationError::SizeExceeded {
                field: field_name.to_string(),
                actual,
                limit,
            });
        }
        Ok(())
    }

    /// Validate base64 data size using estimated decoded size
    /// Base64 encoding expands data by ~4/3, so decoded size is ~3/4 of encoded size
    pub fn validate_base64_size(&self, encoded_data: &str) -> Result<(), SizeValidationError> {
        let estimated_decoded_size = (encoded_data.len() * 3) / 4;
        self.validate_size(
            estimated_decoded_size,
            self.limits.max_base64_size,
            "base64_data",
        )
    }

    /// Validate path length
    pub fn validate_path_length(&self, path: &str) -> Result<(), SizeValidationError> {
        self.validate_size(path.len(), self.limits.max_path_length, "path")
    }

    /// Validate URI length
    pub fn validate_uri_length(&self, uri: &str) -> Result<(), SizeValidationError> {
        self.validate_size(uri.len(), self.limits.max_uri_length, "uri")
    }

    /// Validate content size
    pub fn validate_content_size(&self, size: usize) -> Result<(), SizeValidationError> {
        self.validate_size(size, self.limits.max_content_size, "content")
    }

    /// Validate metadata object size
    pub fn validate_meta_size(&self, size: usize) -> Result<(), SizeValidationError> {
        self.validate_size(size, self.limits.max_meta_size, "metadata")
    }

    /// Get the configured limits
    pub fn limits(&self) -> &SizeLimits {
        &self.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = SizeLimits::default();
        assert_eq!(limits.max_path_length, 4096);
        assert_eq!(limits.max_uri_length, 8192);
        assert_eq!(limits.max_base64_size, 10 * 1024 * 1024);
        assert_eq!(limits.max_content_size, 50 * 1024 * 1024);
        assert_eq!(limits.max_meta_size, 100_000);
    }

    #[test]
    fn test_strict_limits() {
        let limits = SizeLimits::strict();
        assert_eq!(limits.max_path_length, 1024);
        assert_eq!(limits.max_uri_length, 4096);
        assert_eq!(limits.max_base64_size, 1024 * 1024);
        assert_eq!(limits.max_content_size, 5 * 1024 * 1024);
        assert_eq!(limits.max_meta_size, 10_000);
    }

    #[test]
    fn test_permissive_limits() {
        let limits = SizeLimits::permissive();
        assert_eq!(limits.max_path_length, 8192);
        assert_eq!(limits.max_uri_length, 16384);
        assert_eq!(limits.max_base64_size, 100 * 1024 * 1024);
        assert_eq!(limits.max_content_size, 500 * 1024 * 1024);
        assert_eq!(limits.max_meta_size, 1_000_000);
    }

    #[test]
    fn test_validator_creation() {
        let validator = SizeValidator::default();
        assert_eq!(validator.limits().max_path_length, 4096);

        let custom_limits = SizeLimits {
            max_path_length: 1024,
            max_uri_length: 2048,
            max_base64_size: 1024,
            max_content_size: 2048,
            max_meta_size: 512,
        };
        let custom_validator = SizeValidator::new(custom_limits.clone());
        assert_eq!(custom_validator.limits().max_path_length, 1024);
    }

    #[test]
    fn test_validate_size_success() {
        let validator = SizeValidator::default();
        assert!(validator.validate_size(100, 200, "test_field").is_ok());
        assert!(validator.validate_size(0, 100, "test_field").is_ok());
        assert!(validator.validate_size(100, 100, "test_field").is_ok());
    }

    #[test]
    fn test_validate_size_failure() {
        let validator = SizeValidator::default();
        let result = validator.validate_size(201, 200, "test_field");
        assert!(result.is_err());

        if let Err(SizeValidationError::SizeExceeded {
            field,
            actual,
            limit,
        }) = result
        {
            assert_eq!(field, "test_field");
            assert_eq!(actual, 201);
            assert_eq!(limit, 200);
        } else {
            panic!("Expected SizeExceeded error");
        }
    }

    #[test]
    fn test_validate_base64_size() {
        let validator = SizeValidator::default();

        // Small valid base64 string: "Hello" encoded is "SGVsbG8="
        assert!(validator.validate_base64_size("SGVsbG8=").is_ok());

        // Create a large base64 string that exceeds 10MB when decoded
        // Encoded size needs to be > (10MB * 4/3) to exceed 10MB decoded
        let large_size = (10 * 1024 * 1024 * 4 / 3) + 1000;
        let large_data = "A".repeat(large_size);
        assert!(validator.validate_base64_size(&large_data).is_err());
    }

    #[test]
    fn test_validate_path_length() {
        let validator = SizeValidator::default();

        // Valid path within limit
        let short_path = "/usr/local/bin";
        assert!(validator.validate_path_length(short_path).is_ok());

        // Path exactly at limit
        let at_limit_path = "a".repeat(4096);
        assert!(validator.validate_path_length(&at_limit_path).is_ok());

        // Path exceeding limit
        let too_long_path = "a".repeat(4097);
        let result = validator.validate_path_length(&too_long_path);
        assert!(result.is_err());

        if let Err(SizeValidationError::SizeExceeded { field, .. }) = result {
            assert_eq!(field, "path");
        }
    }

    #[test]
    fn test_validate_uri_length() {
        let validator = SizeValidator::default();

        // Valid URI
        assert!(validator
            .validate_uri_length("https://example.com/path")
            .is_ok());

        // URI exactly at limit
        let at_limit_uri = format!("https://example.com/{}", "a".repeat(8192 - 20));
        assert!(validator.validate_uri_length(&at_limit_uri).is_ok());

        // URI exceeding limit
        let too_long_uri = "a".repeat(8193);
        let result = validator.validate_uri_length(&too_long_uri);
        assert!(result.is_err());

        if let Err(SizeValidationError::SizeExceeded { field, .. }) = result {
            assert_eq!(field, "uri");
        }
    }

    #[test]
    fn test_validate_content_size() {
        let validator = SizeValidator::default();

        // Valid content size
        assert!(validator.validate_content_size(1024).is_ok());
        assert!(validator.validate_content_size(50 * 1024 * 1024).is_ok());

        // Content exceeding limit
        let result = validator.validate_content_size(50 * 1024 * 1024 + 1);
        assert!(result.is_err());

        if let Err(SizeValidationError::SizeExceeded { field, .. }) = result {
            assert_eq!(field, "content");
        }
    }

    #[test]
    fn test_validate_meta_size() {
        let validator = SizeValidator::default();

        // Valid meta size
        assert!(validator.validate_meta_size(1000).is_ok());
        assert!(validator.validate_meta_size(100_000).is_ok());

        // Meta exceeding limit
        let result = validator.validate_meta_size(100_001);
        assert!(result.is_err());

        if let Err(SizeValidationError::SizeExceeded {
            field,
            actual,
            limit,
        }) = result
        {
            assert_eq!(field, "metadata");
            assert_eq!(actual, 100_001);
            assert_eq!(limit, 100_000);
        }
    }

    #[test]
    fn test_base64_size_estimation() {
        let validator = SizeValidator::default();

        // Test base64 size estimation formula: decoded_size = (encoded_len * 3) / 4
        // For 8 bytes encoded: "SGVsbG8=" (8 chars) should decode to ~6 bytes
        let encoded = "SGVsbG8="; // "Hello" in base64
        assert!(validator.validate_base64_size(encoded).is_ok());

        // Create encoded data that would decode to exactly the limit
        let limit = validator.limits().max_base64_size;
        let encoded_at_limit_size = (limit * 4) / 3;
        let encoded_at_limit = "A".repeat(encoded_at_limit_size);
        assert!(validator.validate_base64_size(&encoded_at_limit).is_ok());

        // One more byte should fail
        let encoded_over_limit = "A".repeat(encoded_at_limit_size + 4);
        assert!(validator.validate_base64_size(&encoded_over_limit).is_err());
    }

    #[test]
    fn test_edge_cases() {
        let validator = SizeValidator::default();

        // Zero size should always be valid
        assert!(validator.validate_size(0, 100, "test").is_ok());
        assert!(validator.validate_path_length("").is_ok());
        assert!(validator.validate_uri_length("").is_ok());
        assert!(validator.validate_content_size(0).is_ok());
        assert!(validator.validate_meta_size(0).is_ok());
        assert!(validator.validate_base64_size("").is_ok());
    }

    #[test]
    fn test_error_message_formatting() {
        let validator = SizeValidator::default();
        let result = validator.validate_size(150, 100, "test_field");

        match result {
            Err(e) => {
                let error_string = e.to_string();
                assert!(error_string.contains("test_field"));
                assert!(error_string.contains("150"));
                assert!(error_string.contains("100"));
            }
            Ok(_) => panic!("Expected an error"),
        }
    }
}
