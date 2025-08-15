//! Security validation for web fetch operations
//!
//! This module provides security controls to prevent SSRF attacks and enforce
//! access policies for web content fetching.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
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
    fn is_private_ipv4(&self, ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();

        // Standard private ranges
        if ip.is_private() {
            return true;
        }

        // Additional ranges to consider private/internal
        match octets[0] {
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
            "http://[::1]", // IPv6 addresses need brackets in URLs
            "https://0.0.0.0",
            "http://169.254.169.254", // AWS metadata
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
            "https://100.64.0.1", // Carrier-grade NAT
            "http://169.254.0.1", // Link-local
            "https://198.18.0.1", // Test network
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

        let blocked_ipv6 = [
            "http://[::1]",               // Localhost
            "https://[::ffff:127.0.0.1]", // IPv4-mapped localhost
            "http://[::]",                // Unspecified
        ];

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
            block_private_ips: false, // Allow private IPs
            block_localhost: false,   // Allow localhost
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
    fn test_security_logging() {
        let validator = SecurityValidator::new();

        // This test mainly ensures the logging function doesn't panic
        validator.log_security_event(
            "SSRF_ATTEMPT",
            "http://127.0.0.1",
            "Attempted access to localhost",
        );
    }

    #[test]
    fn test_edge_case_urls() {
        let validator = SecurityValidator::new();

        // Test malformed URLs
        let malformed = [
            "",
            "not-a-url",
            "://missing-scheme",
            "https://", // Empty host
        ];

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

        // Test comprehensive private IP detection
        let test_cases = [
            ("10.0.0.1", true),    // RFC 1918
            ("172.16.0.1", true),  // RFC 1918
            ("192.168.1.1", true), // RFC 1918
            ("100.64.0.1", true),  // RFC 6598 (Carrier-grade NAT)
            ("169.254.0.1", true), // RFC 3927 (Link-local)
            ("198.18.0.1", true),  // RFC 2544 (Testing)
            ("240.0.0.1", true),   // Reserved
            ("8.8.8.8", false),    // Public DNS
            ("1.1.1.1", false),    // Public DNS
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
}
