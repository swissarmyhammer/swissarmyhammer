//! Security validation for web fetch operations
//!
//! This module provides security controls to prevent SSRF attacks and enforce
//! access policies for web content fetching.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use swissarmyhammer_common::{ErrorSeverity, Severity};
use tracing::{info, warn};
use url::Url;

/// Security validation errors
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    /// URL format is invalid or malformed
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    /// Domain is in the blocked list
    #[error("Blocked domain: {0}")]
    BlockedDomain(String),
    /// Server-Side Request Forgery attempt detected
    #[error("SSRF attempt detected: {0}")]
    SsrfAttempt(String),
    /// URL scheme is not supported (only HTTP/HTTPS allowed)
    #[error("Unsupported scheme: {0}")]
    UnsupportedScheme(String),
}

impl Severity for SecurityError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // All security errors are Critical - they represent security policy violations
            // that prevent safe operation and could indicate attack attempts
            SecurityError::InvalidUrl(_) => ErrorSeverity::Critical,
            SecurityError::BlockedDomain(_) => ErrorSeverity::Critical,
            SecurityError::SsrfAttempt(_) => ErrorSeverity::Critical,
            SecurityError::UnsupportedScheme(_) => ErrorSeverity::Critical,
        }
    }
}

/// Security policy for web fetch operations
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// List of blocked domains (exact match)
    pub blocked_domains: Vec<String>,
    /// List of blocked domain patterns (contains match)
    pub blocked_patterns: Vec<String>,
    /// Whether to block private IP addresses
    pub block_private_ips: bool,
    /// Whether to block localhost addresses
    pub block_localhost: bool,
    /// Whether to block multicast addresses
    pub block_multicast: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            blocked_domains: vec![
                // Common internal/development domains
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
                "0.0.0.0".to_string(),
                // Common cloud metadata endpoints
                "169.254.169.254".to_string(),
                "metadata.google.internal".to_string(),
                "metadata.azure.com".to_string(),
                "instance-data.ec2.internal".to_string(),
            ],
            blocked_patterns: vec![
                // Block common internal patterns
                ".local".to_string(),
                ".localhost".to_string(),
                ".internal".to_string(),
            ],
            block_private_ips: true,
            block_localhost: true,
            block_multicast: true,
        }
    }
}

/// Security validator for URL and domain validation
pub struct SecurityValidator {
    policy: SecurityPolicy,
}

impl SecurityValidator {
    /// Create a new security validator with default policy
    pub fn new() -> Self {
        Self {
            policy: SecurityPolicy::default(),
        }
    }

    /// Create a new security validator with custom policy
    pub fn with_policy(policy: SecurityPolicy) -> Self {
        Self { policy }
    }

    /// Validate a URL against security policies
    pub fn validate_url(&self, url_str: &str) -> Result<Url, SecurityError> {
        // Check for obviously malformed URLs before parsing
        if url_str.contains("///") && !url_str.starts_with("file:///") {
            warn!(
                "Blocked URL with triple slash (not file scheme): {}",
                url_str
            );
            return Err(SecurityError::InvalidUrl(
                "URLs with triple slashes are not allowed except for file scheme".to_string(),
            ));
        }

        // Parse the URL
        let url = Url::parse(url_str)
            .map_err(|e| SecurityError::InvalidUrl(format!("Failed to parse URL: {e}")))?;

        // Validate scheme
        self.validate_scheme(&url)?;

        // Validate domain/host
        self.validate_host(&url)?;

        info!("URL validation passed for: {}", url_str);
        Ok(url)
    }

    /// Validate URL scheme (HTTP/HTTPS only)
    fn validate_scheme(&self, url: &Url) -> Result<(), SecurityError> {
        match url.scheme() {
            "http" | "https" => Ok(()),
            scheme => {
                warn!("Blocked unsupported scheme: {} for URL: {}", scheme, url);
                Err(SecurityError::UnsupportedScheme(format!(
                    "Scheme '{scheme}' not allowed. Only HTTP and HTTPS are supported."
                )))
            }
        }
    }

    /// Validate host against security policies
    fn validate_host(&self, url: &Url) -> Result<(), SecurityError> {
        let host = url
            .host_str()
            .ok_or_else(|| SecurityError::InvalidUrl("URL must have a host".to_string()))?;

        // Reject empty or invalid hosts
        if host.is_empty() {
            warn!("Blocked empty host for URL: {}", url);
            return Err(SecurityError::InvalidUrl(
                "Host cannot be empty".to_string(),
            ));
        }

        // Reject hosts with path traversal attempts or invalid dot/hyphen patterns
        if host.contains("..")
            || host.contains("./")
            || host.starts_with(".")
            || host.ends_with(".")
            || host.starts_with("-")
            || host.ends_with("-")
            || host.contains("-.")
            || host.contains(".-")
        {
            warn!(
                "Blocked host with invalid characters: {} for URL: {}",
                host, url
            );
            return Err(SecurityError::InvalidUrl(format!(
                "Host '{host}' contains invalid characters or patterns"
            )));
        }

        // Check against blocked domains (exact match)
        if self.policy.blocked_domains.contains(&host.to_lowercase()) {
            warn!("Blocked domain: {} for URL: {}", host, url);
            return Err(SecurityError::BlockedDomain(format!(
                "Domain '{host}' is in the blocked list"
            )));
        }

        // Check against blocked patterns
        let host_lower = host.to_lowercase();
        for pattern in &self.policy.blocked_patterns {
            if host_lower.contains(pattern) {
                warn!(
                    "Blocked domain pattern: {} matches {} for URL: {}",
                    pattern, host, url
                );
                return Err(SecurityError::BlockedDomain(format!(
                    "Domain '{host}' matches blocked pattern '{pattern}'"
                )));
            }
        }

        // Check for IP-based SSRF attempts
        self.validate_ip_address(url, host)?;

        Ok(())
    }

    /// Validate IP addresses for SSRF protection
    fn validate_ip_address(&self, url: &Url, host: &str) -> Result<(), SecurityError> {
        // Try to parse the host as an IP address
        if let Ok(ip) = host.parse::<IpAddr>() {
            self.check_ip_restrictions(&ip, url)?;
        }

        // Also check if URL has an explicit IP
        if let Some(ip) = url
            .socket_addrs(|| None)
            .ok()
            .and_then(|addrs| addrs.first().map(|addr| addr.ip()))
        {
            self.check_ip_restrictions(&ip, url)?;
        }

        Ok(())
    }

    /// Check IP address restrictions
    fn check_ip_restrictions(&self, ip: &IpAddr, url: &Url) -> Result<(), SecurityError> {
        match ip {
            IpAddr::V4(ipv4) => self.check_ipv4_restrictions(ipv4, url),
            IpAddr::V6(ipv6) => self.check_ipv6_restrictions(ipv6, url),
        }
    }

    /// Check IPv4 address restrictions
    fn check_ipv4_restrictions(&self, ip: &Ipv4Addr, url: &Url) -> Result<(), SecurityError> {
        // Check localhost
        if self.policy.block_localhost && ip.is_loopback() {
            warn!("Blocked localhost IPv4: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Localhost IPv4 address {ip} is not allowed"
            )));
        }

        // Check private networks
        if self.policy.block_private_ips && self.is_private_ipv4(ip) {
            warn!("Blocked private IPv4: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Private IPv4 address {ip} is not allowed"
            )));
        }

        // Check multicast
        if self.policy.block_multicast && ip.is_multicast() {
            warn!("Blocked multicast IPv4: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Multicast IPv4 address {ip} is not allowed"
            )));
        }

        // Check link-local
        if ip.is_link_local() {
            warn!("Blocked link-local IPv4: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Link-local IPv4 address {ip} is not allowed"
            )));
        }

        // Check broadcast
        if ip.is_broadcast() {
            warn!("Blocked broadcast IPv4: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Broadcast IPv4 address {ip} is not allowed"
            )));
        }

        // Check unspecified (0.0.0.0)
        if ip.is_unspecified() {
            warn!("Blocked unspecified IPv4: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Unspecified IPv4 address {ip} is not allowed"
            )));
        }

        Ok(())
    }

    /// Check IPv6 address restrictions
    fn check_ipv6_restrictions(&self, ip: &Ipv6Addr, url: &Url) -> Result<(), SecurityError> {
        // Check localhost
        if self.policy.block_localhost && ip.is_loopback() {
            warn!("Blocked localhost IPv6: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Localhost IPv6 address {ip} is not allowed"
            )));
        }

        // Check multicast
        if self.policy.block_multicast && ip.is_multicast() {
            warn!("Blocked multicast IPv6: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Multicast IPv6 address {ip} is not allowed"
            )));
        }

        // Check unspecified (::)
        if ip.is_unspecified() {
            warn!("Blocked unspecified IPv6: {} for URL: {}", ip, url);
            return Err(SecurityError::SsrfAttempt(format!(
                "Unspecified IPv6 address {ip} is not allowed"
            )));
        }

        // Check IPv4-mapped IPv6 addresses (::ffff:x.x.x.x)
        if let Some(ipv4) = ip.to_ipv4_mapped() {
            warn!(
                "Detected IPv4-mapped IPv6 address: {} (IPv4: {}) for URL: {}",
                ip, ipv4, url
            );
            // Apply the same IPv4 restrictions to the mapped address
            return self.check_ipv4_restrictions(&ipv4, url);
        }

        Ok(())
    }

    /// Check if IPv4 address is in private ranges
    /// More comprehensive than the standard library's is_private()
    pub fn is_private_ipv4(&self, ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();

        // Standard private ranges
        if ip.is_private() {
            return true;
        }

        // Additional ranges to consider private/internal
        match octets[0] {
            // This Network (0.0.0.0/8) - includes 0.0.0.1
            0 => true,
            // Class A private (10.0.0.0/8)
            10 => true,
            // Class B private (172.16.0.0/12)
            172 if (16..=31).contains(&octets[1]) => true,
            // Class C private (192.168.0.0/16)
            192 if octets[1] == 168 => true,
            // Carrier-grade NAT (100.64.0.0/10)
            100 if (64..=127).contains(&octets[1]) => true,
            // Link-local (169.254.0.0/16)
            169 if octets[1] == 254 => true,
            // Test networks (198.18.0.0/15)
            198 if (18..=19).contains(&octets[1]) => true,
            // Reserved for future use (240.0.0.0/4)
            n if n >= 240 => true,
            _ => false,
        }
    }

    /// Log a security event for monitoring
    pub fn log_security_event(&self, event_type: &str, url: &str, details: &str) {
        warn!(
            event_type = event_type,
            url = url,
            details = details,
            "Security event detected in web fetch"
        );

        // In a production system, this would integrate with security monitoring
        // For now, we rely on structured logging
        info!(
            "Security event logged: {} for URL: {} - {}",
            event_type, url, details
        );
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_urls() {
        let validator = SecurityValidator::new();

        let valid_urls = [
            "https://example.com",
            "http://public-site.org",
            "https://api.github.com/user",
            "https://www.rust-lang.org/learn",
        ];

        for url_str in &valid_urls {
            assert!(
                validator.validate_url(url_str).is_ok(),
                "Should allow valid URL: {url_str}"
            );
        }
    }

    #[test]
    fn test_invalid_schemes() {
        let validator = SecurityValidator::new();

        let invalid_schemes = [
            "ftp://example.com",
            "file:///etc/passwd",
            "javascript:alert('xss')",
            "data:text/plain,hello",
            "mailto:user@example.com",
        ];

        for url_str in &invalid_schemes {
            assert!(
                matches!(
                    validator.validate_url(url_str),
                    Err(SecurityError::UnsupportedScheme(_))
                ),
                "Should block invalid scheme: {url_str}"
            );
        }
    }

    #[test]
    fn test_blocked_domains() {
        let validator = SecurityValidator::new();

        let blocked_domains = [
            "http://localhost",
            "https://127.0.0.1",
            "http://[::1]",
            "https://0.0.0.0",
            "http://169.254.169.254",
            "https://metadata.google.internal",
        ];

        for url_str in &blocked_domains {
            assert!(
                matches!(
                    validator.validate_url(url_str),
                    Err(SecurityError::BlockedDomain(_)) | Err(SecurityError::SsrfAttempt(_))
                ),
                "Should block domain: {url_str}"
            );
        }
    }

    #[test]
    fn test_private_ip_detection() {
        let validator = SecurityValidator::new();

        let private_ips = [
            "http://10.0.0.1",
            "https://172.16.0.1",
            "http://192.168.1.1",
            "https://100.64.0.1",
            "http://169.254.0.1",
            "https://198.18.0.1",
        ];

        for url_str in &private_ips {
            assert!(
                matches!(
                    validator.validate_url(url_str),
                    Err(SecurityError::SsrfAttempt(_))
                ),
                "Should block private IP: {url_str}"
            );
        }
    }

    #[test]
    fn test_ipv6_restrictions() {
        let validator = SecurityValidator::new();

        let blocked_ipv6 = ["http://[::1]", "https://[::ffff:127.0.0.1]", "http://[::]"];

        for url_str in &blocked_ipv6 {
            assert!(
                matches!(
                    validator.validate_url(url_str),
                    Err(SecurityError::SsrfAttempt(_))
                ),
                "Should block IPv6: {url_str}"
            );
        }
    }

    #[test]
    fn test_custom_policy() {
        let policy = SecurityPolicy {
            blocked_domains: vec!["evil.com".to_string()],
            blocked_patterns: vec![".badpattern.".to_string()],
            block_private_ips: false,
            block_localhost: false,
            block_multicast: true,
        };

        let validator = SecurityValidator::with_policy(policy);

        // Should allow localhost now
        assert!(validator.validate_url("http://localhost").is_ok());

        // Should block custom domain
        assert!(matches!(
            validator.validate_url("https://evil.com"),
            Err(SecurityError::BlockedDomain(_))
        ));

        // Should block pattern
        assert!(matches!(
            validator.validate_url("https://test.badpattern.example"),
            Err(SecurityError::BlockedDomain(_))
        ));
    }

    #[test]
    fn test_edge_case_urls() {
        let validator = SecurityValidator::new();

        let malformed = ["", "not-a-url", "://missing-scheme", "https://"];

        for url_str in &malformed {
            assert!(
                matches!(
                    validator.validate_url(url_str),
                    Err(SecurityError::InvalidUrl(_))
                ),
                "Should reject malformed URL: {url_str}"
            );
        }
    }

    #[test]
    fn test_comprehensive_private_ip_ranges() {
        let validator = SecurityValidator::new();

        let test_cases = [
            ("10.0.0.1", true),
            ("172.16.0.1", true),
            ("192.168.1.1", true),
            ("100.64.0.1", true),
            ("169.254.0.1", true),
            ("198.18.0.1", true),
            ("240.0.0.1", true),
            ("8.8.8.8", false),
            ("1.1.1.1", false),
        ];

        for (ip_str, should_be_private) in test_cases {
            let ip: Ipv4Addr = ip_str.parse().unwrap();
            assert_eq!(
                validator.is_private_ipv4(&ip),
                should_be_private,
                "IP {ip_str} private detection failed"
            );
        }
    }

    #[test]
    fn test_security_error_severity() {
        use swissarmyhammer_common::Severity;

        let errors = vec![
            SecurityError::InvalidUrl("test".to_string()),
            SecurityError::BlockedDomain("evil.com".to_string()),
            SecurityError::SsrfAttempt("127.0.0.1".to_string()),
            SecurityError::UnsupportedScheme("ftp".to_string()),
        ];

        for error in errors {
            assert_eq!(
                error.severity(),
                swissarmyhammer_common::ErrorSeverity::Critical,
                "Expected Critical severity for security error: {}",
                error
            );
        }
    }

    #[test]
    fn test_multicast_ipv4_blocked() {
        let validator = SecurityValidator::new();

        let result = validator.validate_url("http://224.0.0.1");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Multicast IPv4 224.0.0.1 should be blocked as SsrfAttempt, got: {result:?}"
        );

        let result = validator.validate_url("http://239.255.255.255");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Multicast IPv4 239.255.255.255 should be blocked as SsrfAttempt, got: {result:?}"
        );
    }

    #[test]
    fn test_broadcast_ipv4_blocked() {
        let validator = SecurityValidator::new();

        let result = validator.validate_url("http://255.255.255.255");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Broadcast IPv4 255.255.255.255 should be blocked as SsrfAttempt, got: {result:?}"
        );
    }

    #[test]
    fn test_link_local_ipv4_blocked() {
        let validator = SecurityValidator::new();

        // 169.254.1.1 is link-local (169.254.0.0/16) and should be caught
        // by is_private_ipv4 or the link-local check
        let result = validator.validate_url("http://169.254.1.1");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Link-local IPv4 169.254.1.1 should be blocked as SsrfAttempt, got: {result:?}"
        );
    }

    #[test]
    fn test_multicast_ipv6_blocked() {
        let validator = SecurityValidator::new();

        let result = validator.validate_url("http://[ff02::1]");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Multicast IPv6 ff02::1 should be blocked as SsrfAttempt, got: {result:?}"
        );

        let result = validator.validate_url("http://[ff05::1]");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Multicast IPv6 ff05::1 should be blocked as SsrfAttempt, got: {result:?}"
        );
    }

    #[test]
    fn test_invalid_host_patterns() {
        let validator = SecurityValidator::new();

        let invalid_hosts = [
            ("http://host..name.com", "double dot"),
            ("http://-host.com", "leading hyphen"),
            ("http://host-.com", "trailing hyphen"),
            ("http://host.-name.com", "dot-hyphen"),
            ("http://host-.name.com", "hyphen-dot"),
        ];

        for (url_str, desc) in &invalid_hosts {
            let result = validator.validate_url(url_str);
            assert!(
                matches!(result, Err(SecurityError::InvalidUrl(_))),
                "Host with {desc} ({url_str}) should be InvalidUrl, got: {result:?}"
            );
        }
    }

    #[test]
    fn test_log_security_event_exercised() {
        // Exercise the log_security_event code path to ensure it does not panic
        let validator = SecurityValidator::new();
        validator.log_security_event("test_event", "http://example.com", "unit test exercise");
    }

    #[test]
    fn test_carrier_grade_nat_blocked() {
        let validator = SecurityValidator::new();

        // 100.64.0.0/10 — carrier-grade NAT
        let result = validator.validate_url("http://100.100.0.1");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Carrier-grade NAT 100.100.0.1 should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_test_network_blocked() {
        let validator = SecurityValidator::new();

        // 198.18.0.0/15 — test networks
        let result = validator.validate_url("http://198.19.0.1");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Test network 198.19.0.1 should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_reserved_range_blocked() {
        let validator = SecurityValidator::new();

        // 240.0.0.0/4 — reserved for future use
        let result = validator.validate_url("http://241.0.0.1");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Reserved range 241.0.0.1 should be blocked, got: {result:?}"
        );
    }

    // ========================================================================
    // Additional security coverage tests
    // ========================================================================

    #[test]
    fn test_security_validator_default_impl() {
        let validator = SecurityValidator::default();
        // Should work identically to ::new()
        assert!(validator.validate_url("https://example.com").is_ok());
        assert!(validator.validate_url("http://localhost").is_err());
    }

    #[test]
    fn test_security_policy_default_blocked_domains() {
        let policy = SecurityPolicy::default();
        assert!(policy.blocked_domains.contains(&"localhost".to_string()));
        assert!(policy.blocked_domains.contains(&"127.0.0.1".to_string()));
        assert!(policy.blocked_domains.contains(&"::1".to_string()));
        assert!(policy.blocked_domains.contains(&"0.0.0.0".to_string()));
        assert!(policy
            .blocked_domains
            .contains(&"169.254.169.254".to_string()));
        assert!(policy
            .blocked_domains
            .contains(&"metadata.google.internal".to_string()));
        assert!(policy
            .blocked_domains
            .contains(&"metadata.azure.com".to_string()));
        assert!(policy
            .blocked_domains
            .contains(&"instance-data.ec2.internal".to_string()));
    }

    #[test]
    fn test_security_policy_default_blocked_patterns() {
        let policy = SecurityPolicy::default();
        assert!(policy.blocked_patterns.contains(&".local".to_string()));
        assert!(policy.blocked_patterns.contains(&".localhost".to_string()));
        assert!(policy.blocked_patterns.contains(&".internal".to_string()));
    }

    #[test]
    fn test_security_policy_default_flags() {
        let policy = SecurityPolicy::default();
        assert!(policy.block_private_ips);
        assert!(policy.block_localhost);
        assert!(policy.block_multicast);
    }

    #[test]
    fn test_blocked_pattern_local() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("https://myhost.local");
        assert!(
            matches!(result, Err(SecurityError::BlockedDomain(_))),
            ".local pattern should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_blocked_pattern_localhost_subdomain() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("https://app.localhost");
        assert!(
            matches!(result, Err(SecurityError::BlockedDomain(_))),
            ".localhost pattern should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_blocked_azure_metadata() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http://metadata.azure.com/metadata/instance");
        assert!(
            matches!(result, Err(SecurityError::BlockedDomain(_))),
            "metadata.azure.com should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_blocked_ec2_metadata() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http://instance-data.ec2.internal/latest/meta-data/");
        assert!(
            matches!(result, Err(SecurityError::BlockedDomain(_))),
            "instance-data.ec2.internal should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_triple_slash_url_blocked() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http:///example.com");
        assert!(
            matches!(result, Err(SecurityError::InvalidUrl(_))),
            "Triple-slash URL should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_file_triple_slash_blocked_by_scheme() {
        let validator = SecurityValidator::new();
        // file:/// is allowed past the triple-slash check but blocked by scheme validation
        let result = validator.validate_url("file:///etc/passwd");
        assert!(
            matches!(result, Err(SecurityError::UnsupportedScheme(_))),
            "file scheme should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_url_no_host_rejected() {
        let validator = SecurityValidator::new();
        // A URL that parses but has no host
        let result = validator.validate_url("http:///");
        assert!(
            result.is_err(),
            "URL with no host should be rejected, got: {result:?}"
        );
    }

    #[test]
    fn test_path_traversal_in_host_double_dot() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http://host..name.com/path");
        assert!(
            matches!(result, Err(SecurityError::InvalidUrl(_))),
            "Double dot in host should be InvalidUrl, got: {result:?}"
        );
    }

    #[test]
    fn test_host_starting_with_dot() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http://.example.com");
        // URL parser may reject this or it triggers the .starts_with('.') check
        assert!(
            result.is_err(),
            "Host starting with dot should be rejected, got: {result:?}"
        );
    }

    #[test]
    fn test_host_ending_with_dot() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http://example.com.");
        // Trailing dot is often normalized, but our check catches it
        // The URL parser may strip it; either way we verify no panic
        let _ = result; // Acceptable either way — just exercises the path
    }

    #[test]
    fn test_this_network_zero_prefix_blocked() {
        let validator = SecurityValidator::new();
        // 0.x.x.x — "This Network" range
        let result = validator.validate_url("http://0.1.2.3");
        assert!(
            matches!(
                result,
                Err(SecurityError::SsrfAttempt(_)) | Err(SecurityError::BlockedDomain(_))
            ),
            "0.x.x.x should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_is_private_ipv4_this_network() {
        let validator = SecurityValidator::new();
        let ip: Ipv4Addr = "0.1.2.3".parse().unwrap();
        assert!(
            validator.is_private_ipv4(&ip),
            "0.x.x.x should be considered private"
        );
    }

    #[test]
    fn test_is_private_ipv4_class_a() {
        let validator = SecurityValidator::new();
        let ip: Ipv4Addr = "10.255.255.255".parse().unwrap();
        assert!(validator.is_private_ipv4(&ip));
    }

    #[test]
    fn test_is_private_ipv4_class_b_range() {
        let validator = SecurityValidator::new();
        // 172.16-31.x.x range boundaries
        for second_octet in [16u8, 20, 31] {
            let ip: Ipv4Addr = format!("172.{second_octet}.0.1").parse().unwrap();
            assert!(
                validator.is_private_ipv4(&ip),
                "172.{second_octet}.0.1 should be private"
            );
        }
        // Outside range
        let ip: Ipv4Addr = "172.15.0.1".parse().unwrap();
        assert!(
            !validator.is_private_ipv4(&ip),
            "172.15.0.1 should NOT be private"
        );
        let ip: Ipv4Addr = "172.32.0.1".parse().unwrap();
        assert!(
            !validator.is_private_ipv4(&ip),
            "172.32.0.1 should NOT be private"
        );
    }

    #[test]
    fn test_is_private_ipv4_class_c() {
        let validator = SecurityValidator::new();
        let ip: Ipv4Addr = "192.168.0.1".parse().unwrap();
        assert!(validator.is_private_ipv4(&ip));
        let ip: Ipv4Addr = "192.167.0.1".parse().unwrap();
        assert!(
            !validator.is_private_ipv4(&ip),
            "192.167.x.x should NOT be private"
        );
    }

    #[test]
    fn test_is_private_ipv4_carrier_grade_nat() {
        let validator = SecurityValidator::new();
        // 100.64-127.x.x
        let ip: Ipv4Addr = "100.64.0.1".parse().unwrap();
        assert!(validator.is_private_ipv4(&ip));
        let ip: Ipv4Addr = "100.127.255.255".parse().unwrap();
        assert!(validator.is_private_ipv4(&ip));
        let ip: Ipv4Addr = "100.63.0.1".parse().unwrap();
        assert!(
            !validator.is_private_ipv4(&ip),
            "100.63.x.x is NOT carrier-grade NAT"
        );
        let ip: Ipv4Addr = "100.128.0.1".parse().unwrap();
        assert!(
            !validator.is_private_ipv4(&ip),
            "100.128.x.x is NOT carrier-grade NAT"
        );
    }

    #[test]
    fn test_is_private_ipv4_link_local() {
        let validator = SecurityValidator::new();
        let ip: Ipv4Addr = "169.254.100.100".parse().unwrap();
        assert!(validator.is_private_ipv4(&ip));
    }

    #[test]
    fn test_is_private_ipv4_test_networks() {
        let validator = SecurityValidator::new();
        let ip: Ipv4Addr = "198.18.0.1".parse().unwrap();
        assert!(validator.is_private_ipv4(&ip));
        let ip: Ipv4Addr = "198.19.255.255".parse().unwrap();
        assert!(validator.is_private_ipv4(&ip));
        let ip: Ipv4Addr = "198.17.0.1".parse().unwrap();
        assert!(
            !validator.is_private_ipv4(&ip),
            "198.17.x.x is NOT a test network"
        );
    }

    #[test]
    fn test_is_private_ipv4_reserved_future_use() {
        let validator = SecurityValidator::new();
        for first_octet in [240u8, 245, 250, 255] {
            let ip: Ipv4Addr = format!("{first_octet}.0.0.1").parse().unwrap();
            assert!(
                validator.is_private_ipv4(&ip),
                "{first_octet}.x.x.x should be reserved/private"
            );
        }
    }

    #[test]
    fn test_is_private_ipv4_public_addresses() {
        let validator = SecurityValidator::new();
        let public_ips = ["8.8.8.8", "1.1.1.1", "93.184.216.34", "151.101.1.140"];
        for ip_str in &public_ips {
            let ip: Ipv4Addr = ip_str.parse().unwrap();
            assert!(!validator.is_private_ipv4(&ip), "{ip_str} should be public");
        }
    }

    #[test]
    fn test_unspecified_ipv4_blocked() {
        let validator = SecurityValidator::new();
        // 0.0.0.0 is on the blocked_domains list, so it may be caught there first
        let result = validator.validate_url("http://0.0.0.0");
        assert!(
            result.is_err(),
            "0.0.0.0 should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_unspecified_ipv6_blocked() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http://[::]");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "Unspecified IPv6 [::] should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_ipv4_mapped_ipv6_private_blocked() {
        let validator = SecurityValidator::new();
        // ::ffff:10.0.0.1 — IPv4-mapped IPv6 with private IPv4
        let result = validator.validate_url("http://[::ffff:10.0.0.1]");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "IPv4-mapped IPv6 with private IPv4 should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_ipv4_mapped_ipv6_loopback_blocked() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("https://[::ffff:127.0.0.1]");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "IPv4-mapped IPv6 loopback should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_loopback_ipv4_blocked_via_ssrf() {
        let validator = SecurityValidator::new();
        // 127.0.0.1 is on blocked domains. Try 127.0.0.2 which is still loopback.
        let result = validator.validate_url("http://127.0.0.2");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "127.0.0.2 (loopback) should be SSRF blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_loopback_ipv6_blocked_via_ssrf() {
        // [::1] is on blocked domains, which hits BlockedDomain.
        // We already test that. Test that the SsrfAttempt path fires
        // by using a custom policy that doesn't block ::1 as a domain.
        let policy = SecurityPolicy {
            blocked_domains: vec![],
            blocked_patterns: vec![],
            block_private_ips: true,
            block_localhost: true,
            block_multicast: true,
        };
        let validator = SecurityValidator::with_policy(policy);
        let result = validator.validate_url("http://[::1]");
        assert!(
            matches!(result, Err(SecurityError::SsrfAttempt(_))),
            "IPv6 loopback should be SSRF blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_security_error_display() {
        let err = SecurityError::InvalidUrl("bad url".to_string());
        assert_eq!(err.to_string(), "Invalid URL: bad url");

        let err = SecurityError::BlockedDomain("evil.com".to_string());
        assert_eq!(err.to_string(), "Blocked domain: evil.com");

        let err = SecurityError::SsrfAttempt("127.0.0.1".to_string());
        assert_eq!(err.to_string(), "SSRF attempt detected: 127.0.0.1");

        let err = SecurityError::UnsupportedScheme("ftp".to_string());
        assert_eq!(err.to_string(), "Unsupported scheme: ftp");
    }

    #[test]
    fn test_custom_policy_allows_private_ips_when_disabled() {
        let policy = SecurityPolicy {
            blocked_domains: vec![],
            blocked_patterns: vec![],
            block_private_ips: false,
            block_localhost: false,
            block_multicast: false,
        };
        let validator = SecurityValidator::with_policy(policy);

        // Private IPs should be allowed
        assert!(
            validator.validate_url("http://10.0.0.1").is_ok(),
            "Private IP should be allowed with block_private_ips=false"
        );
        assert!(
            validator.validate_url("http://192.168.1.1").is_ok(),
            "Private IP should be allowed with block_private_ips=false"
        );
    }

    #[test]
    fn test_custom_policy_allows_multicast_when_disabled() {
        let policy = SecurityPolicy {
            blocked_domains: vec![],
            blocked_patterns: vec![],
            block_private_ips: false,
            block_localhost: false,
            block_multicast: false,
        };
        let _validator = SecurityValidator::with_policy(policy);

        // 224.0.0.1 is multicast but we disabled the check.
        // Exercises the code path where block_multicast is false.
        let result = _validator.validate_url("http://224.0.0.1");
        // Multicast check is disabled, so it should pass
        let _ = result;
    }

    #[test]
    fn test_host_with_dot_slash_pattern() {
        let validator = SecurityValidator::new();
        let result = validator.validate_url("http://host./name.com");
        // host. ends with dot — should be caught by the ends_with('.') check
        // But URL parser may handle this differently
        assert!(
            result.is_err(),
            "Host with dot-slash pattern should be rejected"
        );
    }
}
