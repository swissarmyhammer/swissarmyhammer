use crate::base64_validation;
use crate::constants::sizes;
use crate::error::ToJsonRpcError;
use crate::size_validator::{SizeValidationError, SizeValidator};
use crate::url_validation;
use agent_client_protocol::ContentBlock;
use base64;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, warn};
use url::Url;

#[derive(Debug, Error, Clone)]
pub enum ContentSecurityError {
    #[error("Content security validation failed: {reason} (policy: {policy_violated})")]
    SecurityValidationFailed {
        reason: String,
        policy_violated: String,
    },
    #[error("Suspicious content detected: {threat_type} - {details}")]
    SuspiciousContentDetected {
        threat_type: String,
        details: String,
    },
    #[error("DoS protection triggered: {protection_type} (threshold: {threshold})")]
    DoSProtectionTriggered {
        protection_type: String,
        threshold: String,
    },
    #[error("URI security violation: {uri} - {reason}")]
    UriSecurityViolation { uri: String, reason: String },
    #[error("Base64 security violation: {reason}")]
    Base64SecurityViolation { reason: String },
    #[error("Content type spoofing detected: declared {declared}, actual {actual}")]
    ContentTypeSpoofingDetected { declared: String, actual: String },
    #[error("Content sanitization failed: {reason}")]
    ContentSanitizationFailed { reason: String },
    #[error("SSRF protection triggered: {target} - {reason}")]
    SsrfProtectionTriggered { target: String, reason: String },
    #[error("Memory limit exceeded: {actual} > {limit} bytes")]
    MemoryLimitExceeded { actual: usize, limit: usize },
    #[error("Rate limit exceeded: {operation}")]
    RateLimitExceeded { operation: String },
    #[error("Content array too large: {length} > {max_length}")]
    ContentArrayTooLarge { length: usize, max_length: usize },
    #[error("Invalid content encoding: {encoding}")]
    InvalidContentEncoding { encoding: String },
    #[error("Malicious pattern detected: {pattern_type}")]
    MaliciousPatternDetected { pattern_type: String },
}

impl ToJsonRpcError for ContentSecurityError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            Self::SecurityValidationFailed { .. }
            | Self::SuspiciousContentDetected { .. }
            | Self::DoSProtectionTriggered { .. }
            | Self::UriSecurityViolation { .. }
            | Self::Base64SecurityViolation { .. }
            | Self::ContentTypeSpoofingDetected { .. }
            | Self::ContentSanitizationFailed { .. }
            | Self::SsrfProtectionTriggered { .. }
            | Self::MemoryLimitExceeded { .. }
            | Self::ContentArrayTooLarge { .. }
            | Self::InvalidContentEncoding { .. }
            | Self::MaliciousPatternDetected { .. } => -32602, // Invalid params
            Self::RateLimitExceeded { .. } => -32000, // Server error
        }
    }

    fn to_error_data(&self) -> Option<Value> {
        let data = match self {
            Self::SecurityValidationFailed {
                reason,
                policy_violated,
            } => json!({
                "error": "security_validation_failed",
                "details": reason,
                "policyViolated": policy_violated,
                "suggestion": "Review content security policies and ensure compliance"
            }),
            Self::SuspiciousContentDetected {
                threat_type,
                details,
            } => json!({
                "error": "suspicious_content_detected",
                "threatType": threat_type,
                "details": details,
                "suggestion": "Remove suspicious content or use a lower security level"
            }),
            Self::DoSProtectionTriggered {
                protection_type,
                threshold,
            } => json!({
                "error": "dos_protection_triggered",
                "protectionType": protection_type,
                "threshold": threshold,
                "suggestion": "Reduce content size or processing complexity"
            }),
            Self::UriSecurityViolation { uri, reason } => json!({
                "error": "uri_security_violation",
                "uri": uri,
                "details": reason,
                "suggestion": "Use allowed URI schemes and avoid private/local addresses"
            }),
            Self::Base64SecurityViolation { reason } => json!({
                "error": "base64_security_violation",
                "details": reason,
                "suggestion": "Ensure base64 data is valid and within size limits"
            }),
            Self::ContentTypeSpoofingDetected { declared, actual } => json!({
                "error": "content_type_spoofing_detected",
                "declaredType": declared,
                "actualType": actual,
                "suggestion": "Ensure declared MIME type matches actual content format"
            }),
            Self::ContentSanitizationFailed { reason } => json!({
                "error": "content_sanitization_failed",
                "details": reason,
                "suggestion": "Remove potentially dangerous content patterns"
            }),
            Self::SsrfProtectionTriggered { target, reason } => json!({
                "error": "ssrf_protection_triggered",
                "target": target,
                "details": reason,
                "suggestion": "Avoid accessing private networks or sensitive endpoints"
            }),
            Self::MemoryLimitExceeded { actual, limit } => json!({
                "error": "memory_limit_exceeded",
                "actualBytes": actual,
                "limitBytes": limit,
                "suggestion": "Reduce content size or increase memory limits"
            }),
            Self::RateLimitExceeded { operation } => json!({
                "error": "rate_limit_exceeded",
                "operation": operation,
                "suggestion": "Reduce request frequency or wait before retrying"
            }),
            Self::ContentArrayTooLarge { length, max_length } => json!({
                "error": "content_array_too_large",
                "arrayLength": length,
                "maxLength": max_length,
                "suggestion": "Reduce the number of content blocks in the array"
            }),
            Self::InvalidContentEncoding { encoding } => json!({
                "error": "invalid_content_encoding",
                "encoding": encoding,
                "suggestion": "Use supported content encoding formats"
            }),
            Self::MaliciousPatternDetected { pattern_type } => json!({
                "error": "malicious_pattern_detected",
                "patternType": pattern_type,
                "suggestion": "Remove or sanitize detected malicious patterns"
            }),
        };
        Some(data)
    }
}

impl From<SizeValidationError> for ContentSecurityError {
    fn from(error: SizeValidationError) -> Self {
        match error {
            SizeValidationError::SizeExceeded {
                field,
                actual,
                limit,
            } => ContentSecurityError::UriSecurityViolation {
                uri: format!("{} ({} bytes)", field, actual),
                reason: format!("Size exceeds limit: {} > {}", actual, limit),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SecurityLevel {
    Strict,
    Moderate,
    Permissive,
}

// IMPORTANT: Do not add timeouts to content processing operations.
// Content processing should be allowed to complete regardless of size or complexity.
// Timeouts create artificial limitations and poor user experience by interrupting
// legitimate processing of large or complex content. Users cannot predict when
// operations will be artificially terminated, leading to frustration and unreliable behavior.
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub level: SecurityLevel,
    pub max_base64_size: usize,
    pub max_total_content_size: usize,
    pub max_content_array_length: usize,
    pub allowed_uri_schemes: HashSet<String>,
    pub enable_ssrf_protection: bool,
    pub enable_content_sniffing: bool,
    pub enable_format_validation: bool,
    pub enable_content_sanitization: bool,
    pub enable_malicious_pattern_detection: bool,
    pub blocked_uri_patterns: Vec<String>,
    pub blocked_ip_ranges: Vec<String>,
    pub max_uri_length: usize,
    pub enable_rate_limiting: bool,
    pub rate_limit_requests_per_minute: u32,
}

impl SecurityPolicy {
    pub fn strict() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());

        Self {
            level: SecurityLevel::Strict,
            max_base64_size: sizes::content::MAX_CONTENT_STRICT,
            max_total_content_size: sizes::content::MAX_RESOURCE_STRICT,
            max_content_array_length: 10,
            allowed_uri_schemes: allowed_schemes,
            enable_ssrf_protection: true,
            enable_content_sniffing: true,
            enable_format_validation: true,
            enable_content_sanitization: true,
            enable_malicious_pattern_detection: true,
            blocked_uri_patterns: vec![
                r"localhost".to_string(),
                r"127\..*".to_string(),
                r"192\.168\..*".to_string(),
                r"10\..*".to_string(),
                r"172\.(1[6-9]|2[0-9]|3[01])\..*".to_string(),
            ],
            blocked_ip_ranges: vec![
                "127.0.0.0/8".to_string(),
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
                "192.168.0.0/16".to_string(),
                "::1/128".to_string(),
            ],
            max_uri_length: sizes::uri::MAX_URI_LENGTH,
            enable_rate_limiting: true,
            rate_limit_requests_per_minute: 60,
        }
    }

    pub fn moderate() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());
        allowed_schemes.insert("http".to_string());
        allowed_schemes.insert("file".to_string());

        Self {
            level: SecurityLevel::Moderate,
            max_base64_size: sizes::content::MAX_CONTENT_MODERATE,
            max_total_content_size: sizes::content::MAX_RESOURCE_MODERATE,
            max_content_array_length: 50,
            allowed_uri_schemes: allowed_schemes,
            enable_ssrf_protection: true,
            enable_content_sniffing: true,
            enable_format_validation: true,
            enable_content_sanitization: true,
            enable_malicious_pattern_detection: true,
            blocked_uri_patterns: vec![r"127\.0\.0\.1".to_string(), r"localhost".to_string()],
            blocked_ip_ranges: vec!["127.0.0.0/8".to_string(), "::1/128".to_string()],
            max_uri_length: sizes::uri::MAX_URI_LENGTH,
            enable_rate_limiting: true,
            rate_limit_requests_per_minute: 300,
        }
    }

    pub fn permissive() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());
        allowed_schemes.insert("http".to_string());
        allowed_schemes.insert("file".to_string());
        allowed_schemes.insert("data".to_string());
        allowed_schemes.insert("ftp".to_string());

        Self {
            level: SecurityLevel::Permissive,
            max_base64_size: sizes::content::MAX_CONTENT_PERMISSIVE,
            max_total_content_size: sizes::content::MAX_RESOURCE_PERMISSIVE,
            max_content_array_length: sizes::messages::MAX_CONTENT_ARRAY_LENGTH,
            allowed_uri_schemes: allowed_schemes,
            enable_ssrf_protection: false,
            enable_content_sniffing: false,
            enable_format_validation: false,
            enable_content_sanitization: false,
            enable_malicious_pattern_detection: false,
            blocked_uri_patterns: vec![],
            blocked_ip_ranges: vec![],
            max_uri_length: sizes::uri::MAX_URI_LENGTH_EXTENDED,
            enable_rate_limiting: false,
            rate_limit_requests_per_minute: 0,
        }
    }
}

#[derive(Debug)]
pub struct ContentSecurityValidator {
    policy: SecurityPolicy,
    blocked_uri_regexes: Vec<Regex>,
    processing_stats: HashMap<String, u32>,
    last_rate_limit_reset: Instant,
    size_validator: SizeValidator,
}

impl Clone for ContentSecurityValidator {
    fn clone(&self) -> Self {
        // Recreate regex patterns from the policy
        let mut blocked_uri_regexes = Vec::new();
        for pattern in &self.policy.blocked_uri_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                blocked_uri_regexes.push(regex);
            }
        }

        Self {
            policy: self.policy.clone(),
            blocked_uri_regexes,
            processing_stats: self.processing_stats.clone(),
            last_rate_limit_reset: self.last_rate_limit_reset,
            size_validator: self.size_validator.clone(),
        }
    }
}

impl ContentSecurityValidator {
    pub fn new(policy: SecurityPolicy) -> Result<Self, ContentSecurityError> {
        let mut blocked_uri_regexes = Vec::new();
        for pattern in &policy.blocked_uri_patterns {
            match Regex::new(pattern) {
                Ok(regex) => blocked_uri_regexes.push(regex),
                Err(e) => {
                    return Err(ContentSecurityError::SecurityValidationFailed {
                        reason: format!("Invalid regex pattern '{}': {}", pattern, e),
                        policy_violated: "uri_pattern_validation".to_string(),
                    });
                }
            }
        }

        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_uri_length: policy.max_uri_length,
            ..Default::default()
        });

        Ok(Self {
            policy,
            blocked_uri_regexes,
            processing_stats: HashMap::new(),
            last_rate_limit_reset: Instant::now(),
            size_validator,
        })
    }

    pub fn strict() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::strict())
    }

    pub fn moderate() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::moderate())
    }

    pub fn permissive() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::permissive())
    }

    pub fn policy(&self) -> &SecurityPolicy {
        &self.policy
    }

    /// Perform comprehensive content security validation
    pub fn validate_content_security(
        &self,
        content: &ContentBlock,
    ) -> Result<(), ContentSecurityError> {
        debug!(
            "Starting content security validation for {:?}",
            std::mem::discriminant(content)
        );

        self.validate_content_internal(content)
    }

    fn validate_content_internal(
        &self,
        content: &ContentBlock,
    ) -> Result<(), ContentSecurityError> {
        match content {
            ContentBlock::Text(text_content) => {
                self.validate_text_security(text_content)?;
            }
            ContentBlock::Image(image_content) => {
                self.validate_base64_security(&image_content.data, "image")?;
                if let Some(ref uri) = image_content.uri {
                    self.validate_uri_security(uri)?;
                }
                if self.policy.enable_format_validation {
                    self.validate_content_type_consistency(
                        &image_content.data,
                        &image_content.mime_type,
                    )?;
                }
            }
            ContentBlock::Audio(audio_content) => {
                self.validate_base64_security(&audio_content.data, "audio")?;
                if self.policy.enable_format_validation {
                    self.validate_content_type_consistency(
                        &audio_content.data,
                        &audio_content.mime_type,
                    )?;
                }
            }
            ContentBlock::Resource(resource_content) => {
                self.validate_resource_content(resource_content)?;
            }
            ContentBlock::ResourceLink(resource_link) => {
                self.validate_uri_security(&resource_link.uri)?;
            }
            _ => {
                // Unknown or unsupported content block type - reject for security
                return Err(ContentSecurityError::SecurityValidationFailed {
                    reason: "Unsupported content type".to_string(),
                    policy_violated: "content_type_allowlist".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate array of content blocks
    pub fn validate_content_blocks_security(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<(), ContentSecurityError> {
        // Check array size limits
        if content_blocks.len() > self.policy.max_content_array_length {
            return Err(ContentSecurityError::ContentArrayTooLarge {
                length: content_blocks.len(),
                max_length: self.policy.max_content_array_length,
            });
        }

        // Calculate total content size estimate
        let mut total_estimated_size = 0;
        for content_block in content_blocks {
            match content_block {
                ContentBlock::Text(text) => {
                    total_estimated_size += text.text.len();
                }
                ContentBlock::Image(image) => {
                    // Base64 encoded size is ~4/3 of actual size
                    total_estimated_size += (image.data.len() * 3) / 4;
                }
                ContentBlock::Audio(audio) => {
                    total_estimated_size += (audio.data.len() * 3) / 4;
                }
                ContentBlock::Resource(_) => {
                    // Conservative estimate for resource content
                    total_estimated_size += 1024; // 1KB estimate
                }
                ContentBlock::ResourceLink(_) => {
                    // URI-based content has minimal memory impact
                    total_estimated_size += 512; // 512B estimate
                }
                _ => {
                    // Unknown content type - use conservative estimate
                    total_estimated_size += 1024; // 1KB estimate
                }
            }
        }

        if total_estimated_size > self.policy.max_total_content_size {
            return Err(ContentSecurityError::DoSProtectionTriggered {
                protection_type: "total_content_size".to_string(),
                threshold: format!(
                    "{} > {}",
                    total_estimated_size, self.policy.max_total_content_size
                ),
            });
        }

        // Validate each content block
        for (index, content_block) in content_blocks.iter().enumerate() {
            if let Err(e) = self.validate_content_security(content_block) {
                warn!(
                    "Content security validation failed for block {}: {}",
                    index, e
                );
                return Err(e);
            }
        }

        Ok(())
    }

    /// Validate base64 data security
    pub fn validate_base64_security(
        &self,
        data: &str,
        content_type: &str,
    ) -> Result<(), ContentSecurityError> {
        // Check size limits before processing
        let estimated_decoded_size = (data.len() * 3) / 4;
        if estimated_decoded_size > self.policy.max_base64_size {
            return Err(ContentSecurityError::Base64SecurityViolation {
                reason: format!(
                    "Base64 {} content too large: {} > {} bytes",
                    content_type, estimated_decoded_size, self.policy.max_base64_size
                ),
            });
        }

        // Validate base64 format
        if let Err(e) = base64_validation::validate_base64_format(data) {
            return Err(ContentSecurityError::Base64SecurityViolation {
                reason: format!("Invalid base64 format: {}", e),
            });
        }

        // Check for malicious patterns in base64 data if enabled
        if self.policy.enable_malicious_pattern_detection {
            if let Some(pattern_type) = self.detect_malicious_base64_patterns(data) {
                return Err(ContentSecurityError::MaliciousPatternDetected { pattern_type });
            }
        }

        Ok(())
    }

    /// Validate URI security including SSRF protection
    pub fn validate_uri_security(&self, uri: &str) -> Result<(), ContentSecurityError> {
        // Basic format validation
        if uri.is_empty() {
            return Err(ContentSecurityError::UriSecurityViolation {
                uri: uri.to_string(),
                reason: "Empty URI".to_string(),
            });
        }

        self.size_validator.validate_uri_length(uri)?;

        // Parse URI
        let parsed_uri = match Url::parse(uri) {
            Ok(url) => url,
            Err(_) => {
                return Err(ContentSecurityError::UriSecurityViolation {
                    uri: uri.to_string(),
                    reason: "Invalid URI format".to_string(),
                });
            }
        };

        // Validate scheme
        let scheme = parsed_uri.scheme();
        if !self.policy.allowed_uri_schemes.contains(scheme) {
            return Err(ContentSecurityError::UriSecurityViolation {
                uri: uri.to_string(),
                reason: format!("Disallowed URI scheme: {}", scheme),
            });
        }

        // Check blocked patterns
        for regex in &self.blocked_uri_regexes {
            if regex.is_match(uri) {
                return Err(ContentSecurityError::UriSecurityViolation {
                    uri: uri.to_string(),
                    reason: "URI matches blocked pattern".to_string(),
                });
            }
        }

        // SSRF protection
        if self.policy.enable_ssrf_protection {
            if let Some(reason) = url_validation::validate_url_against_ssrf(&parsed_uri) {
                return Err(ContentSecurityError::SsrfProtectionTriggered {
                    target: uri.to_string(),
                    reason,
                });
            }
        }

        Ok(())
    }

    /// Validate text content security
    pub fn validate_text_security(
        &self,
        text_content: &agent_client_protocol::TextContent,
    ) -> Result<(), ContentSecurityError> {
        if self.policy.enable_content_sanitization {
            self.validate_text_content_safety(&text_content.text)?;
        }

        Ok(())
    }

    /// Validate resource content security
    ///
    /// Validates embedded resource content including URI security, text content safety,
    /// base64 blob security, and content type consistency for both text and blob resources.
    ///
    /// # Arguments
    ///
    /// * `resource_content` - The embedded resource to validate
    ///
    /// # Returns
    ///
    /// * `Ok(())` if validation passes
    /// * `Err(ContentSecurityError)` if any validation check fails
    ///
    /// # Validation Checks
    ///
    /// For text resources:
    /// - URI security validation (if URI is non-empty)
    /// - Text content safety checks (if content sanitization is enabled)
    ///
    /// For blob resources:
    /// - URI security validation (if URI is non-empty)
    /// - Base64 security validation (if blob is non-empty)
    /// - Content type consistency validation (if format validation is enabled and MIME type is not application/octet-stream)
    pub fn validate_resource_content(
        &self,
        resource_content: &agent_client_protocol::EmbeddedResource,
    ) -> Result<(), ContentSecurityError> {
        use agent_client_protocol::EmbeddedResourceResource;

        match &resource_content.resource {
            EmbeddedResourceResource::TextResourceContents(text_resource) => {
                // Validate URI
                if !text_resource.uri.is_empty() {
                    self.validate_uri_security(&text_resource.uri)?;
                }

                // Validate text content
                if !text_resource.text.is_empty() && self.policy.enable_content_sanitization {
                    self.validate_text_content_safety(&text_resource.text)?;
                }
            }
            EmbeddedResourceResource::BlobResourceContents(blob_resource) => {
                // Validate URI
                if !blob_resource.uri.is_empty() {
                    self.validate_uri_security(&blob_resource.uri)?;
                }

                // Validate base64 blob data
                if !blob_resource.blob.is_empty() {
                    self.validate_base64_security(&blob_resource.blob, "resource")?;

                    // Validate content type consistency if MIME type is provided
                    if self.policy.enable_format_validation {
                        if let Some(ref mime_type) = blob_resource.mime_type {
                            if mime_type != "application/octet-stream" {
                                self.validate_content_type_consistency(
                                    &blob_resource.blob,
                                    mime_type,
                                )?;
                            }
                        }
                    }
                }
            }
            _ => {
                // Unknown or unsupported resource type - reject for security
                return Err(ContentSecurityError::SecurityValidationFailed {
                    reason: "Unsupported resource type".to_string(),
                    policy_violated: "resource_type_allowlist".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Sniff content type from binary data using magic numbers
    ///
    /// Uses the `infer` crate to detect file types by examining magic numbers
    /// at the beginning of the binary data.
    ///
    /// # Arguments
    ///
    /// * `data` - Binary data to analyze
    ///
    /// # Returns
    ///
    /// * `Some(String)` containing the detected MIME type if recognizable
    /// * `None` if the content type cannot be determined
    pub fn sniff_content_type(&self, data: &[u8]) -> Option<String> {
        infer::get(data).map(|kind| kind.mime_type().to_string())
    }

    /// Validate content type consistency to detect spoofing
    ///
    /// Compares the declared MIME type against the actual content type detected
    /// from magic numbers in the binary data. This helps prevent content type
    /// spoofing attacks where an attacker declares one type but provides another.
    ///
    /// # Arguments
    ///
    /// * `base64_data` - Base64-encoded binary data to validate
    /// * `declared_mime_type` - The MIME type claimed for this content
    ///
    /// # Returns
    ///
    /// * `Ok(())` if content sniffing is disabled, types match, or type cannot be determined
    /// * `Err(ContentSecurityError::ContentTypeSpoofingDetected)` if declared and actual types differ
    /// * `Err(ContentSecurityError::Base64SecurityViolation)` if base64 decoding fails
    ///
    /// # Implementation Details
    ///
    /// - Only validates the first 512 bytes (684 base64 characters) for efficiency
    /// - Normalizes MIME types for comparison (e.g., "image/jpg" vs "image/jpeg")
    /// - Permissive for unknown types that cannot be detected
    pub fn validate_content_type_consistency(
        &self,
        base64_data: &str,
        declared_mime_type: &str,
    ) -> Result<(), ContentSecurityError> {
        if !self.policy.enable_content_sniffing {
            return Ok(());
        }

        debug!(
            "Content type consistency validation for {}",
            declared_mime_type
        );

        // Decode a portion of the base64 data to check magic numbers
        // We only need the first 512 bytes for magic number detection
        let sample_size = std::cmp::min(base64_data.len(), 684); // 684 base64 chars = ~512 bytes
        let sample = &base64_data[..sample_size];

        // Decode the sample
        use base64::Engine;
        let decoded = match base64::engine::general_purpose::STANDARD.decode(sample) {
            Ok(data) => data,
            Err(e) => {
                return Err(ContentSecurityError::Base64SecurityViolation {
                    reason: format!("Failed to decode base64 for content sniffing: {}", e),
                });
            }
        };

        // Sniff the actual content type
        if let Some(actual_mime_type) = self.sniff_content_type(&decoded) {
            // Normalize MIME types for comparison (some variations are acceptable)
            let declared_normalized = self.normalize_mime_type(declared_mime_type);
            let actual_normalized = self.normalize_mime_type(&actual_mime_type);

            if declared_normalized != actual_normalized {
                return Err(ContentSecurityError::ContentTypeSpoofingDetected {
                    declared: declared_mime_type.to_string(),
                    actual: actual_mime_type,
                });
            }
        }
        // If we can't determine the type, we allow it (permissive for unknown types)

        Ok(())
    }

    /// Normalize MIME type for comparison
    ///
    /// Converts MIME types to a canonical form for consistent comparison.
    /// Handles common variations and aliases.
    ///
    /// # Arguments
    ///
    /// * `mime_type` - The MIME type string to normalize
    ///
    /// # Returns
    ///
    /// A normalized MIME type string in lowercase with common aliases resolved
    ///
    /// # Examples of Normalization
    ///
    /// - `"IMAGE/JPEG"` → `"image/jpeg"`
    /// - `"image/jpg"` → `"image/jpeg"`
    fn normalize_mime_type(&self, mime_type: &str) -> String {
        // Convert to lowercase and handle common variations
        let normalized = mime_type.to_lowercase();

        // Map common variations to canonical forms
        match normalized.as_str() {
            "image/jpg" => "image/jpeg".to_string(),
            "audio/x-wav" => "audio/wav".to_string(),
            _ => normalized,
        }
    }

    /// Detect malicious patterns in base64 data
    fn detect_malicious_base64_patterns(&self, data: &str) -> Option<String> {
        // Check for suspicious patterns that might indicate embedded executables or malicious content

        // Look for patterns that might decode to executable headers
        if data.starts_with("TVq") || data.starts_with("TVo") {
            return Some("potential_pe_executable".to_string());
        }

        if data.starts_with("f0VMR") {
            return Some("potential_elf_executable".to_string());
        }

        // Check for overly repetitive patterns (potential zip bombs or data corruption)
        if self.is_overly_repetitive(data) {
            return Some("repetitive_pattern".to_string());
        }

        None
    }

    /// Check if data contains overly repetitive patterns
    fn is_overly_repetitive(&self, data: &str) -> bool {
        if data.len() < 100 {
            return false;
        }

        // Sample check: if first 50 characters repeat more than 10 times
        if data.len() >= 50 {
            let sample = &data[0..50];
            let count = data.matches(sample).count();
            if count > 10 {
                return true;
            }
        }

        false
    }

    /// Validate text content for potentially dangerous content
    fn validate_text_content_safety(&self, text: &str) -> Result<(), ContentSecurityError> {
        // Check for basic script injection patterns
        let dangerous_patterns = [
            "<script",
            "javascript:",
            "onload=",
            "onerror=",
            "eval(",
            "document.cookie",
        ];

        let text_lower = text.to_lowercase();
        for pattern in &dangerous_patterns {
            if text_lower.contains(pattern) {
                return Err(ContentSecurityError::ContentSanitizationFailed {
                    reason: format!("Potentially dangerous pattern detected: {}", pattern),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::TextContent;

    fn create_test_validator() -> ContentSecurityValidator {
        ContentSecurityValidator::moderate().unwrap()
    }

    #[test]
    fn test_security_policy_levels() {
        let strict = SecurityPolicy::strict();
        let moderate = SecurityPolicy::moderate();
        let permissive = SecurityPolicy::permissive();

        assert_eq!(strict.level, SecurityLevel::Strict);
        assert_eq!(moderate.level, SecurityLevel::Moderate);
        assert_eq!(permissive.level, SecurityLevel::Permissive);

        // Strict should have tighter limits
        assert!(strict.max_base64_size < moderate.max_base64_size);
        assert!(moderate.max_base64_size < permissive.max_base64_size);
    }

    #[test]
    fn test_uri_security_validation() {
        let validator = create_test_validator();

        // Valid URIs
        assert!(validator
            .validate_uri_security("https://example.com")
            .is_ok());
        assert!(validator
            .validate_uri_security("http://example.com")
            .is_ok());
        assert!(validator
            .validate_uri_security("file:///tmp/test.txt")
            .is_ok());

        // Invalid URIs
        assert!(validator.validate_uri_security("").is_err());
        assert!(validator.validate_uri_security("invalid-uri").is_err());
        assert!(validator
            .validate_uri_security("javascript:alert(1)")
            .is_err());

        // SSRF protection
        assert!(validator.validate_uri_security("http://localhost").is_err());
        assert!(validator.validate_uri_security("http://127.0.0.1").is_err());
    }

    #[test]
    fn test_base64_security_validation() {
        let validator = create_test_validator();

        // Valid base64
        assert!(validator
            .validate_base64_security("SGVsbG8gV29ybGQ=", "test")
            .is_ok());

        // Invalid base64
        assert!(validator.validate_base64_security("", "test").is_err());
        assert!(validator
            .validate_base64_security("Invalid!@#$", "test")
            .is_err());

        // Too large (simulate by using policy with small limit)
        let strict_validator = ContentSecurityValidator::strict().unwrap();
        let large_data = "A".repeat(2 * sizes::content::MB);
        assert!(strict_validator
            .validate_base64_security(&large_data, "test")
            .is_err());
    }

    #[test]
    fn test_text_security_validation() {
        let validator = create_test_validator();

        let safe_text = TextContent {
            text: "This is safe text content".to_string(),
            annotations: None,
            meta: None,
        };

        let dangerous_text = TextContent {
            text: "<script>alert('xss')</script>".to_string(),
            annotations: None,
            meta: None,
        };

        assert!(validator.validate_text_security(&safe_text).is_ok());
        assert!(validator.validate_text_security(&dangerous_text).is_err());
    }

    #[test]
    fn test_content_blocks_security_validation() {
        let validator = create_test_validator();

        let safe_content = vec![ContentBlock::Text(TextContent {
            text: "Hello".to_string(),
            annotations: None,
            meta: None,
        })];

        let too_many_content = vec![
            ContentBlock::Text(TextContent {
                text: "test".to_string(),
                annotations: None,
                meta: None,
            });
            100
        ]; // Exceeds moderate policy limit

        assert!(validator
            .validate_content_blocks_security(&safe_content)
            .is_ok());
        assert!(validator
            .validate_content_blocks_security(&too_many_content)
            .is_err());
    }

    #[test]
    fn test_malicious_pattern_detection() {
        let validator = create_test_validator();

        // Test executable detection
        let pe_executable_base64 = "TVqQAAMAAAAEAAAA"; // PE header in base64
        let elf_executable_base64 = "f0VMRgIBAQAAAAA"; // ELF header in base64

        if validator.policy.enable_malicious_pattern_detection {
            assert!(validator
                .detect_malicious_base64_patterns(pe_executable_base64)
                .is_some());
            assert!(validator
                .detect_malicious_base64_patterns(elf_executable_base64)
                .is_some());
        }

        // Safe base64 should pass
        let safe_base64 = "SGVsbG8gV29ybGQ="; // "Hello World" in base64
        assert!(validator
            .detect_malicious_base64_patterns(safe_base64)
            .is_none());
    }

    #[test]
    fn test_ssrf_protection() {
        let validator = ContentSecurityValidator::strict().unwrap();

        // These should be blocked by SSRF protection
        assert!(validator.validate_uri_security("http://127.0.0.1").is_err());
        assert!(validator.validate_uri_security("http://localhost").is_err());
        assert!(validator
            .validate_uri_security("http://169.254.169.254")
            .is_err());
        assert!(validator.validate_uri_security("http://10.0.0.1").is_err());

        // These should be allowed
        assert!(validator
            .validate_uri_security("https://example.com")
            .is_ok());
        assert!(validator
            .validate_uri_security("https://google.com")
            .is_ok());
    }

    #[test]
    fn test_sniff_content_type_png() {
        let validator = create_test_validator();
        // 1x1 PNG in base64
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(png_data)
            .unwrap();
        let result = validator.sniff_content_type(&decoded);

        assert!(result.is_some());
        let mime_type = result.unwrap();
        assert_eq!(mime_type, "image/png");
    }

    #[test]
    fn test_sniff_content_type_jpeg() {
        let validator = create_test_validator();
        // JPEG header (FFD8FF)
        let jpeg_header = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];

        let result = validator.sniff_content_type(&jpeg_header);

        assert!(result.is_some());
        let mime_type = result.unwrap();
        assert_eq!(mime_type, "image/jpeg");
    }

    #[test]
    fn test_sniff_content_type_unknown() {
        let validator = create_test_validator();
        let unknown_data = vec![0x00, 0x01, 0x02, 0x03];

        let result = validator.sniff_content_type(&unknown_data);

        assert!(result.is_none());
    }

    #[test]
    fn test_content_type_consistency_matching() {
        let validator = create_test_validator();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let result = validator.validate_content_type_consistency(png_data, "image/png");
        assert!(result.is_ok());
    }

    #[test]
    fn test_content_type_consistency_spoofing() {
        let validator = create_test_validator();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        // Declaring as JPEG when it's actually PNG
        let result = validator.validate_content_type_consistency(png_data, "image/jpeg");
        assert!(result.is_err());

        if let Err(ContentSecurityError::ContentTypeSpoofingDetected { declared, actual }) = result
        {
            assert_eq!(declared, "image/jpeg");
            assert_eq!(actual, "image/png");
        } else {
            panic!("Expected ContentTypeSpoofingDetected error");
        }
    }

    #[test]
    fn test_content_type_consistency_disabled() {
        let mut policy = SecurityPolicy::moderate();
        policy.enable_content_sniffing = false;
        let validator = ContentSecurityValidator::new(policy).unwrap();

        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        // Should pass even with mismatched types when sniffing is disabled
        let result = validator.validate_content_type_consistency(png_data, "image/jpeg");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_resource_content_with_uri() {
        use agent_client_protocol::{EmbeddedResourceResource, TextResourceContents};

        let validator = create_test_validator();

        let text_resource = TextResourceContents {
            uri: "https://example.com/data.json".to_string(),
            text: "Sample text content".to_string(),
            mime_type: None,
            meta: None,
        };
        let embedded = agent_client_protocol::EmbeddedResource {
            resource: EmbeddedResourceResource::TextResourceContents(text_resource),
            annotations: None,
            meta: None,
        };
        let content = ContentBlock::Resource(embedded);

        let result = validator.validate_content_security(&content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_resource_content_with_invalid_uri() {
        use agent_client_protocol::{EmbeddedResourceResource, TextResourceContents};

        let validator = create_test_validator();

        let text_resource = TextResourceContents {
            uri: "http://localhost/secret".to_string(),
            text: "Sample text content".to_string(),
            mime_type: None,
            meta: None,
        };
        let embedded = agent_client_protocol::EmbeddedResource {
            resource: EmbeddedResourceResource::TextResourceContents(text_resource),
            annotations: None,
            meta: None,
        };
        let content = ContentBlock::Resource(embedded);

        let result = validator.validate_content_security(&content);
        // Should fail due to SSRF protection (localhost)
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_resource_content_with_blob() {
        use agent_client_protocol::{BlobResourceContents, EmbeddedResourceResource};

        let validator = create_test_validator();

        let blob_resource = BlobResourceContents {
            uri: "".to_string(),
            blob: "SGVsbG8gV29ybGQ=".to_string(),
            mime_type: Some("text/plain".to_string()),
            meta: None,
        };
        let embedded = agent_client_protocol::EmbeddedResource {
            resource: EmbeddedResourceResource::BlobResourceContents(blob_resource),
            annotations: None,
            meta: None,
        };
        let content = ContentBlock::Resource(embedded);

        let result = validator.validate_content_security(&content);
        assert!(result.is_ok());
    }
}
