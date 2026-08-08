use crate::base64_validation;
use crate::constants::sizes;
use crate::error::ToJsonRpcError;
use crate::json_rpc_codes::{INVALID_PARAMS, SERVER_ERROR};
use crate::size_validator::{SizeValidationError, SizeValidator};
use crate::url_validation;
use agent_client_protocol::schema::ContentBlock;
use base64;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, warn};
use url::Url;

/// Largest content array a [`SecurityPolicy::strict`] policy accepts.
const STRICT_MAX_CONTENT_ARRAY_LENGTH: usize = 10;

/// Requests each minute a [`SecurityPolicy::strict`] policy accepts.
const STRICT_RATE_LIMIT_REQUESTS_PER_MINUTE: u32 = 60;

/// Largest content array a [`SecurityPolicy::moderate`] policy accepts.
const MODERATE_MAX_CONTENT_ARRAY_LENGTH: usize = 50;

/// Requests each minute a [`SecurityPolicy::moderate`] policy accepts.
const MODERATE_RATE_LIMIT_REQUESTS_PER_MINUTE: u32 = 300;

/// Bytes a base64 group carries: three bytes encode as four characters.
const BASE64_DECODED_BYTES_PER_GROUP: usize = 3;

/// Characters a base64 group occupies: four characters carry three bytes.
const BASE64_ENCODED_CHARS_PER_GROUP: usize = 4;

/// Size charged to an embedded resource block, in bytes.
///
/// The real payload is not read during the array-size estimate, so every
/// resource block is charged this conservative figure.
const RESOURCE_CONTENT_SIZE_ESTIMATE: usize = 1024;

/// Size charged to a resource-link block, in bytes.
///
/// A link carries only a URI, so it costs far less than an embedded resource.
const RESOURCE_LINK_SIZE_ESTIMATE: usize = 512;

/// Base64 characters read when sniffing a content type.
///
/// Magic-number detection needs about the first 512 bytes, and 684 base64
/// characters decode to roughly that many bytes.
const CONTENT_SNIFF_SAMPLE_BASE64_CHARS: usize = 684;

/// Shortest string the repetition heuristic can judge, in characters.
const MIN_DATA_LENGTH_FOR_REPETITION_CHECK: usize = 100;

/// Characters taken from the front of a string as the repetition sample.
const REPETITION_SAMPLE_LEN: usize = 50;

/// How often the sample may repeat before the string is called repetitive.
const MAX_REPETITION_COUNT: usize = 10;

/// MIME type that declares opaque bytes, so no format check applies.
const OPAQUE_BINARY_MIME_TYPE: &str = "application/octet-stream";

/// Base64 prefix a Windows PE executable produces, from the `MZ` DOS header.
const PE_EXECUTABLE_BASE64_PREFIXES: [&str; 2] = ["TVq", "TVo"];

/// Base64 prefix an ELF executable produces, from the `\x7FELF` header.
const ELF_EXECUTABLE_BASE64_PREFIX: &str = "f0VMR";

/// Estimate the decoded size of `encoded_len` base64 characters, in bytes.
///
/// Base64 encodes three bytes as four characters, so the decoded size is
/// about three quarters of the encoded length.
fn estimated_base64_decoded_size(encoded_len: usize) -> usize {
    (encoded_len * BASE64_DECODED_BYTES_PER_GROUP) / BASE64_ENCODED_CHARS_PER_GROUP
}

/// Why [`ContentSecurityValidator`] refused a content block.
///
/// The variants cover the whole security surface: policy checks, threat
/// heuristics, URI and SSRF checks, base64 and MIME type checks, and the
/// denial-of-service limits on size, array length and request rate.
#[derive(Debug, Error, Clone)]
pub enum ContentSecurityError {
    /// A policy check refused the content.
    #[error("content security validation failed: {reason} (policy: {policy_violated})")]
    SecurityValidationFailed {
        /// What the check objected to.
        reason: String,
        /// Name of the policy rule that was broken.
        policy_violated: String,
    },
    /// A threat heuristic recognised something dangerous in the content.
    #[error("suspicious content detected: {threat_type} - {details}")]
    SuspiciousContentDetected {
        /// Kind of threat the heuristic recognised.
        threat_type: String,
        /// Evidence the heuristic recorded.
        details: String,
    },
    /// A denial-of-service guard fired.
    #[error("DoS protection triggered: {protection_type} (threshold: {threshold})")]
    DoSProtectionTriggered {
        /// Which guard fired.
        protection_type: String,
        /// The limit and the observed value.
        threshold: String,
    },
    /// A URI broke a scheme, length or deny-list rule.
    #[error("URI security violation: {uri} - {reason}")]
    UriSecurityViolation {
        /// URI that was refused.
        uri: String,
        /// Why it was refused.
        reason: String,
    },
    /// Base64 content was malformed or over a limit.
    #[error("Base64 security violation: {reason}")]
    Base64SecurityViolation {
        /// Why the base64 content was refused.
        reason: String,
    },
    /// The bytes do not match the MIME type the caller declared.
    #[error("content type spoofing detected: declared {declared}, actual {actual}")]
    ContentTypeSpoofingDetected {
        /// MIME type the caller declared.
        declared: String,
        /// MIME type magic-number sniffing found.
        actual: String,
    },
    /// Text content holds a pattern sanitization refuses to pass.
    #[error("content sanitization failed: {reason}")]
    ContentSanitizationFailed {
        /// Which pattern was found.
        reason: String,
    },
    /// A URI points at a private or otherwise protected network target.
    #[error("SSRF protection triggered: {target} - {reason}")]
    SsrfProtectionTriggered {
        /// Host or address the URI names.
        target: String,
        /// Why the target is protected.
        reason: String,
    },
    /// Content is larger than the memory budget for processing.
    #[error("memory limit exceeded: {actual} > {limit} bytes")]
    MemoryLimitExceeded {
        /// Size the content holds, in bytes.
        actual: usize,
        /// Largest accepted size, in bytes.
        limit: usize,
    },
    /// The caller sent requests faster than the policy allows.
    #[error("rate limit exceeded: {operation}")]
    RateLimitExceeded {
        /// Operation that was rate limited.
        operation: String,
    },
    /// The content array holds more blocks than the policy allows.
    #[error("content array too large: {length} > {max_length}")]
    ContentArrayTooLarge {
        /// Number of blocks the array holds.
        length: usize,
        /// Largest accepted number of blocks.
        max_length: usize,
    },
    /// The content declares an encoding the validator does not support.
    #[error("invalid content encoding: {encoding}")]
    InvalidContentEncoding {
        /// Encoding the caller declared.
        encoding: String,
    },
    /// A malicious-pattern heuristic matched.
    #[error("malicious pattern detected: {pattern_type}")]
    MaliciousPatternDetected {
        /// Kind of pattern that matched.
        pattern_type: String,
    },
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
            | Self::MaliciousPatternDetected { .. } => INVALID_PARAMS,
            Self::RateLimitExceeded { .. } => SERVER_ERROR,
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

/// How aggressively a [`SecurityPolicy`] refuses content.
///
/// The level selects the whole preset — size limits, URI rules, heuristics and
/// rate limits — rather than one setting.
#[derive(Debug, Clone, PartialEq)]
pub enum SecurityLevel {
    /// Smallest limits, HTTPS only, every heuristic on. Use for untrusted
    /// input.
    Strict,
    /// Wider limits and more URI schemes, with every heuristic still on. This
    /// is the level the defaults use.
    Moderate,
    /// Widest limits, every heuristic off. Use only for trusted or debug
    /// input.
    Permissive,
}

// IMPORTANT: Do not add timeouts to content processing operations.
// Content processing should be allowed to complete regardless of size or complexity.
// Timeouts create artificial limitations and poor user experience by interrupting
// legitimate processing of large or complex content. Users cannot predict when
// operations will be artificially terminated, leading to frustration and unreliable behavior.
/// The rules a [`ContentSecurityValidator`] enforces.
///
/// A policy holds the size and rate limits, the URI allow-list and deny-lists,
/// and a switch for each heuristic. Build one with [`SecurityPolicy::strict`],
/// [`SecurityPolicy::moderate`] or [`SecurityPolicy::permissive`] rather than
/// filling the fields by hand.
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// Preset this policy came from, for reporting and comparison.
    pub level: SecurityLevel,
    /// Largest decoded base64 payload accepted, in bytes.
    pub max_base64_size: usize,
    /// Largest estimated size of one content array, in bytes.
    pub max_total_content_size: usize,
    /// Largest number of blocks accepted in one content array.
    pub max_content_array_length: usize,
    /// URI schemes the policy accepts, such as `https`.
    pub allowed_uri_schemes: HashSet<String>,
    /// Whether URIs are checked against private and local network targets.
    pub enable_ssrf_protection: bool,
    /// Whether magic-number sniffing runs on binary payloads.
    pub enable_content_sniffing: bool,
    /// Whether a payload must match the MIME type it declares.
    pub enable_format_validation: bool,
    /// Whether text content is checked for script-injection patterns.
    pub enable_content_sanitization: bool,
    /// Whether base64 payloads are checked for malicious patterns.
    pub enable_malicious_pattern_detection: bool,
    /// Regular expressions that refuse a matching URI.
    pub blocked_uri_patterns: Vec<String>,
    /// CIDR ranges that refuse a URI resolving into them.
    pub blocked_ip_ranges: Vec<String>,
    /// Longest URI accepted, in characters.
    pub max_uri_length: usize,
    /// Whether the request rate limit is enforced at all.
    pub enable_rate_limiting: bool,
    /// Requests each minute the policy accepts.
    pub rate_limit_requests_per_minute: u32,
}

impl SecurityPolicy {
    /// Build the tightest policy: HTTPS only, smallest limits, every
    /// heuristic on. Use it for untrusted input.
    pub fn strict() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());

        Self {
            level: SecurityLevel::Strict,
            max_base64_size: sizes::content::MAX_CONTENT_STRICT,
            max_total_content_size: sizes::content::MAX_RESOURCE_STRICT,
            max_content_array_length: STRICT_MAX_CONTENT_ARRAY_LENGTH,
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
            rate_limit_requests_per_minute: STRICT_RATE_LIMIT_REQUESTS_PER_MINUTE,
        }
    }

    /// Build the balanced policy: `https`, `http` and `file` URIs, wider size
    /// limits, every heuristic still on. This is the level the defaults use.
    pub fn moderate() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());
        allowed_schemes.insert("http".to_string());
        allowed_schemes.insert("file".to_string());

        Self {
            level: SecurityLevel::Moderate,
            max_base64_size: sizes::content::MAX_CONTENT_MODERATE,
            max_total_content_size: sizes::content::MAX_RESOURCE_MODERATE,
            max_content_array_length: MODERATE_MAX_CONTENT_ARRAY_LENGTH,
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
            rate_limit_requests_per_minute: MODERATE_RATE_LIMIT_REQUESTS_PER_MINUTE,
        }
    }

    /// Build the loosest policy: widest limits and every heuristic off. Use it
    /// only for trusted or debug input.
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

/// Applies a [`SecurityPolicy`] to ACP content blocks.
///
/// The validator checks a block's size, its URI, its base64 payload and the
/// MIME type it declares, and refuses anything the policy forbids. Build one
/// from a preset with [`ContentSecurityValidator::strict`],
/// [`ContentSecurityValidator::moderate`] or
/// [`ContentSecurityValidator::permissive`], or from a policy of your own with
/// [`ContentSecurityValidator::new`].
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
    /// Build a validator from `policy`, compiling its URI deny-list patterns.
    ///
    /// # Errors
    ///
    /// Returns [`ContentSecurityError::SecurityValidationFailed`] when a
    /// pattern in `blocked_uri_patterns` is not a valid regular expression.
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

    /// Build a validator on [`SecurityPolicy::strict`].
    ///
    /// # Errors
    ///
    /// Returns the error [`ContentSecurityValidator::new`] reports.
    pub fn strict() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::strict())
    }

    /// Build a validator on [`SecurityPolicy::moderate`].
    ///
    /// # Errors
    ///
    /// Returns the error [`ContentSecurityValidator::new`] reports.
    pub fn moderate() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::moderate())
    }

    /// Build a validator on [`SecurityPolicy::permissive`].
    ///
    /// # Errors
    ///
    /// Returns the error [`ContentSecurityValidator::new`] reports.
    pub fn permissive() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::permissive())
    }

    /// The policy this validator enforces.
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
                    total_estimated_size += estimated_base64_decoded_size(image.data.len());
                }
                ContentBlock::Audio(audio) => {
                    total_estimated_size += estimated_base64_decoded_size(audio.data.len());
                }
                ContentBlock::Resource(_) => {
                    total_estimated_size += RESOURCE_CONTENT_SIZE_ESTIMATE;
                }
                ContentBlock::ResourceLink(_) => {
                    total_estimated_size += RESOURCE_LINK_SIZE_ESTIMATE;
                }
                _ => {
                    // Unknown content type - charge the conservative estimate
                    total_estimated_size += RESOURCE_CONTENT_SIZE_ESTIMATE;
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
        let estimated_decoded_size = estimated_base64_decoded_size(data.len());
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
        text_content: &agent_client_protocol::schema::TextContent,
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
        resource_content: &agent_client_protocol::schema::EmbeddedResource,
    ) -> Result<(), ContentSecurityError> {
        use agent_client_protocol::schema::EmbeddedResourceResource;

        match &resource_content.resource {
            EmbeddedResourceResource::TextResourceContents(text_resource) => {
                self.validate_text_resource(text_resource)
            }
            EmbeddedResourceResource::BlobResourceContents(blob_resource) => {
                self.validate_blob_resource(blob_resource)
            }
            // Unknown or unsupported resource type - reject for security
            _ => Err(ContentSecurityError::SecurityValidationFailed {
                reason: "Unsupported resource type".to_string(),
                policy_violated: "resource_type_allowlist".to_string(),
            }),
        }
    }

    /// Validate the URI and the text of a text resource.
    ///
    /// An empty URI is skipped, and the text is checked only when the policy
    /// sets [`SecurityPolicy::enable_content_sanitization`].
    fn validate_text_resource(
        &self,
        text_resource: &agent_client_protocol::schema::TextResourceContents,
    ) -> Result<(), ContentSecurityError> {
        if !text_resource.uri.is_empty() {
            self.validate_uri_security(&text_resource.uri)?;
        }

        if !text_resource.text.is_empty() && self.policy.enable_content_sanitization {
            self.validate_text_content_safety(&text_resource.text)?;
        }

        Ok(())
    }

    /// Validate the URI, the base64 payload and the MIME type of a blob
    /// resource.
    ///
    /// An empty URI is skipped, and an empty blob ends the check.
    fn validate_blob_resource(
        &self,
        blob_resource: &agent_client_protocol::schema::BlobResourceContents,
    ) -> Result<(), ContentSecurityError> {
        if !blob_resource.uri.is_empty() {
            self.validate_uri_security(&blob_resource.uri)?;
        }

        if blob_resource.blob.is_empty() {
            return Ok(());
        }

        self.validate_base64_security(&blob_resource.blob, "resource")?;
        self.validate_blob_mime_type_consistency(blob_resource)
    }

    /// Match a blob's bytes against the MIME type it declares.
    ///
    /// Returns `Ok(())` when the policy does not set
    /// [`SecurityPolicy::enable_format_validation`], when the blob declares no
    /// MIME type, or when it declares [`OPAQUE_BINARY_MIME_TYPE`], which names
    /// no format to match.
    fn validate_blob_mime_type_consistency(
        &self,
        blob_resource: &agent_client_protocol::schema::BlobResourceContents,
    ) -> Result<(), ContentSecurityError> {
        if !self.policy.enable_format_validation {
            return Ok(());
        }

        let Some(mime_type) = blob_resource.mime_type.as_deref() else {
            return Ok(());
        };

        if mime_type.eq_ignore_ascii_case(OPAQUE_BINARY_MIME_TYPE) {
            return Ok(());
        }

        self.validate_content_type_consistency(&blob_resource.blob, mime_type)
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
        let sample_size = std::cmp::min(base64_data.len(), CONTENT_SNIFF_SAMPLE_BASE64_CHARS);
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
        if PE_EXECUTABLE_BASE64_PREFIXES
            .iter()
            .any(|prefix| data.starts_with(prefix))
        {
            return Some("potential_pe_executable".to_string());
        }

        if data.starts_with(ELF_EXECUTABLE_BASE64_PREFIX) {
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
        if data.len() < MIN_DATA_LENGTH_FOR_REPETITION_CHECK {
            return false;
        }

        // Sample check: does the head of the string repeat too often?
        let sample = &data[0..REPETITION_SAMPLE_LEN];
        data.matches(sample).count() > MAX_REPETITION_COUNT
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
    use agent_client_protocol::schema::TextContent;

    /// The eight-byte PNG signature, base64 encoded. Content sniffing reports
    /// this as `image/png`.
    const PNG_SIGNATURE_BASE64: &str = "iVBORw0KGgo=";

    fn create_test_validator() -> ContentSecurityValidator {
        ContentSecurityValidator::moderate().unwrap()
    }

    #[test]
    fn test_blob_mime_type_consistency_skips_uppercase_opaque_binary() {
        use agent_client_protocol::schema::BlobResourceContents;

        let validator = create_test_validator();

        // RFC 2045 makes MIME types case-insensitive, so a blob declared as
        // `Application/Octet-Stream` must skip the format check exactly as the
        // lowercase spelling does. Without that, the PNG payload below reads as
        // a spoofed content type.
        let blob_resource = BlobResourceContents::new(PNG_SIGNATURE_BASE64, "")
            .mime_type("Application/Octet-Stream");

        assert!(validator
            .validate_blob_mime_type_consistency(&blob_resource)
            .is_ok());
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

        let safe_text = TextContent::new("This is safe text content");

        let dangerous_text = TextContent::new("<script>alert('xss')</script>");

        assert!(validator.validate_text_security(&safe_text).is_ok());
        assert!(validator.validate_text_security(&dangerous_text).is_err());
    }

    #[test]
    fn test_content_blocks_security_validation() {
        let validator = create_test_validator();

        let safe_content = vec![ContentBlock::Text(TextContent::new("Hello"))];

        let too_many_content = vec![ContentBlock::Text(TextContent::new("test")); 100]; // Exceeds moderate policy limit

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

        // Test executable detection: each signature prefix plus filler.
        let pe_executable_base64 = format!("{}QAAMAAAAEAAAA", PE_EXECUTABLE_BASE64_PREFIXES[0]);
        let elf_executable_base64 = format!("{}gIBAQAAAAA", ELF_EXECUTABLE_BASE64_PREFIX);

        if validator.policy.enable_malicious_pattern_detection {
            assert!(validator
                .detect_malicious_base64_patterns(&pe_executable_base64)
                .is_some());
            assert!(validator
                .detect_malicious_base64_patterns(&elf_executable_base64)
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
        use agent_client_protocol::schema::{EmbeddedResourceResource, TextResourceContents};

        let validator = create_test_validator();

        let text_resource =
            TextResourceContents::new("Sample text content", "https://example.com/data.json");
        let embedded = agent_client_protocol::schema::EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(text_resource),
        );
        let content = ContentBlock::Resource(embedded);

        let result = validator.validate_content_security(&content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_resource_content_with_invalid_uri() {
        use agent_client_protocol::schema::{EmbeddedResourceResource, TextResourceContents};

        let validator = create_test_validator();

        let text_resource =
            TextResourceContents::new("Sample text content", "http://localhost/secret");
        let embedded = agent_client_protocol::schema::EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(text_resource),
        );
        let content = ContentBlock::Resource(embedded);

        let result = validator.validate_content_security(&content);
        // Should fail due to SSRF protection (localhost)
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_resource_content_with_blob() {
        use agent_client_protocol::schema::{BlobResourceContents, EmbeddedResourceResource};

        let validator = create_test_validator();

        let blob_resource =
            BlobResourceContents::new("SGVsbG8gV29ybGQ=", "").mime_type("text/plain");
        let embedded = agent_client_protocol::schema::EmbeddedResource::new(
            EmbeddedResourceResource::BlobResourceContents(blob_resource),
        );
        let content = ContentBlock::Resource(embedded);

        let result = validator.validate_content_security(&content);
        assert!(result.is_ok());
    }
}
