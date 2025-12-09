// TODO - remove this module -- these are useless one line wrappers around library calls

//! URL validation utilities for consistent URL validation patterns across the codebase
//!
//! This module provides reusable URL validation functions to standardize URL parsing,
//! scheme validation, and SSRF protection without forcing a common error type.
//! Each validation function returns a bool or Option, allowing callsites to use
//! their domain-specific error types.
//!
//! ## Design Decision
//!
//! These validation functions return bool or `Option<String>` rather than `Result`
//! to allow call sites to use their domain-specific error types while still
//! centralizing the validation logic. This approach:
//!
//! - Eliminates code duplication across the codebase
//! - Maintains semantic correctness of domain-specific errors
//! - Avoids breaking changes to existing error handling
//! - Provides a consistent validation pattern
//! - Centralizes SSRF protection logic
//!
//! ## Usage Examples
//!
//! ### With SessionSetupError
//!
//! ```ignore
//! use crate::url_validation;
//! use url::Url;
//!
//! fn validate_mcp_url(url_str: &str) -> Result<(), SessionSetupError> {
//!     let parsed = Url::parse(url_str).map_err(|_| SessionSetupError::InvalidUrl)?;
//!
//!     if !url_validation::is_allowed_scheme(&parsed, &["http", "https"]) {
//!         return Err(SessionSetupError::InvalidScheme);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ### With ContentSecurityError
//!
//! ```ignore
//! use crate::url_validation;
//! use url::Url;
//!
//! fn validate_uri_security(uri: &str) -> Result<(), ContentSecurityError> {
//!     let parsed = Url::parse(uri).map_err(|_| ContentSecurityError::InvalidUri)?;
//!
//!     if let Some(reason) = url_validation::validate_url_against_ssrf(&parsed) {
//!         return Err(ContentSecurityError::SsrfProtectionTriggered {
//!             target: uri.to_string(),
//!             reason,
//!         });
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

/// Check if a URL scheme is in the allowed list
///
/// This is marked inline for zero-cost abstraction.
///
/// # Examples
///
/// ```
/// use claude_agent_lib::url_validation::is_allowed_scheme;
/// use url::Url;
///
/// let url = Url::parse("https://example.com").unwrap();
/// assert!(is_allowed_scheme(&url, &["http", "https"]));
/// assert!(!is_allowed_scheme(&url, &["ftp"]));
/// ```
#[inline]
pub fn is_allowed_scheme(url: &Url, allowed_schemes: &[&str]) -> bool {
    allowed_schemes.contains(&url.scheme())
}

/// Check if an IPv4 address is private or reserved
///
/// Returns true for:
/// - Private addresses (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
/// - Loopback (127.0.0.0/8)
/// - Link-local (169.254.0.0/16)
/// - Broadcast (255.255.255.255)
/// - Documentation addresses (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24)
///
/// # Examples
///
/// ```
/// use claude_agent_lib::url_validation::is_private_ipv4;
/// use std::net::Ipv4Addr;
///
/// assert!(is_private_ipv4(&Ipv4Addr::new(192, 168, 1, 1)));
/// assert!(is_private_ipv4(&Ipv4Addr::new(127, 0, 0, 1)));
/// assert!(!is_private_ipv4(&Ipv4Addr::new(8, 8, 8, 8)));
/// ```
pub fn is_private_ipv4(ip: &Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
}

/// Check if an IPv6 address is private or reserved
///
/// Returns true for:
/// - Loopback (::1)
/// - Unspecified (::)
/// - Link-local (fe80::/10)
/// - Unique local addresses (fc00::/7)
///
/// # Examples
///
/// ```
/// use claude_agent_lib::url_validation::is_private_ipv6;
/// use std::net::Ipv6Addr;
///
/// assert!(is_private_ipv6(&Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))); // ::1
/// assert!(!is_private_ipv6(&Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)));
/// ```
pub fn is_private_ipv6(ip: &Ipv6Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified() || ((ip.segments()[0] & 0xfe00) == 0xfe00) // Link-local
        || ((ip.segments()[0] & 0xfe00) == 0xfc00) // Unique local
}

/// Check if a hostname is vulnerable to SSRF attacks
///
/// Returns true for:
/// - "localhost" (case-insensitive)
/// - "127.0.0.1" or "::1" (IP literals in hostname position)
/// - Cloud metadata service endpoints (169.254.169.254, metadata.google.internal)
///
/// # Examples
///
/// ```
/// use claude_agent_lib::url_validation::is_ssrf_vulnerable_hostname;
///
/// assert!(is_ssrf_vulnerable_hostname("localhost"));
/// assert!(is_ssrf_vulnerable_hostname("LOCALHOST"));
/// assert!(is_ssrf_vulnerable_hostname("127.0.0.1"));
/// assert!(is_ssrf_vulnerable_hostname("169.254.169.254"));
/// assert!(!is_ssrf_vulnerable_hostname("example.com"));
/// ```
pub fn is_ssrf_vulnerable_hostname(hostname: &str) -> bool {
    let hostname_lower = hostname.to_lowercase();

    // Check for localhost variants
    if hostname_lower == "localhost" || hostname_lower == "127.0.0.1" || hostname_lower == "::1" {
        return true;
    }

    // Check for metadata service endpoints (cloud providers)
    if hostname_lower == "169.254.169.254" || hostname_lower == "metadata.google.internal" {
        return true;
    }

    false
}

/// Validate a URL against SSRF attacks
///
/// Checks both IP addresses and hostnames for SSRF vulnerabilities.
/// Returns None if the URL is safe, or Some(reason) if it's vulnerable.
///
/// # Examples
///
/// ```
/// use claude_agent_lib::url_validation::validate_url_against_ssrf;
/// use url::Url;
///
/// let safe_url = Url::parse("https://example.com").unwrap();
/// assert!(validate_url_against_ssrf(&safe_url).is_none());
///
/// let unsafe_url = Url::parse("http://localhost").unwrap();
/// assert!(validate_url_against_ssrf(&unsafe_url).is_some());
/// ```
pub fn validate_url_against_ssrf(url: &Url) -> Option<String> {
    if let Some(host) = url.host_str() {
        // Check if host is an IP address
        if let Ok(ip) = host.parse::<IpAddr>() {
            match ip {
                IpAddr::V4(ipv4) => {
                    if is_private_ipv4(&ipv4) {
                        return Some("Private IPv4 address".to_string());
                    }
                }
                IpAddr::V6(ipv6) => {
                    if is_private_ipv6(&ipv6) {
                        return Some("Private IPv6 address".to_string());
                    }
                }
            }
        } else if is_ssrf_vulnerable_hostname(host) {
            // Check hostname patterns
            return Some("Localhost hostname".to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_scheme() {
        let http_url = Url::parse("http://example.com").unwrap();
        let https_url = Url::parse("https://example.com").unwrap();
        let ftp_url = Url::parse("ftp://example.com").unwrap();

        assert!(is_allowed_scheme(&http_url, &["http", "https"]));
        assert!(is_allowed_scheme(&https_url, &["http", "https"]));
        assert!(!is_allowed_scheme(&ftp_url, &["http", "https"]));
        assert!(is_allowed_scheme(&ftp_url, &["ftp"]));
    }

    #[test]
    fn test_is_private_ipv4() {
        // Private addresses
        assert!(is_private_ipv4(&Ipv4Addr::new(192, 168, 1, 1)));
        assert!(is_private_ipv4(&Ipv4Addr::new(10, 0, 0, 1)));
        assert!(is_private_ipv4(&Ipv4Addr::new(172, 16, 0, 1)));

        // Loopback
        assert!(is_private_ipv4(&Ipv4Addr::new(127, 0, 0, 1)));

        // Link-local
        assert!(is_private_ipv4(&Ipv4Addr::new(169, 254, 169, 254)));

        // Broadcast
        assert!(is_private_ipv4(&Ipv4Addr::new(255, 255, 255, 255)));

        // Public addresses
        assert!(!is_private_ipv4(&Ipv4Addr::new(8, 8, 8, 8)));
        assert!(!is_private_ipv4(&Ipv4Addr::new(1, 1, 1, 1)));
    }

    #[test]
    fn test_is_private_ipv6() {
        // Loopback
        assert!(is_private_ipv6(&Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)));

        // Unspecified
        assert!(is_private_ipv6(&Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)));

        // Link-local (fe80::/10)
        assert!(is_private_ipv6(&Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)));

        // Unique local (fc00::/7)
        assert!(is_private_ipv6(&Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)));

        // Public addresses
        assert!(!is_private_ipv6(&Ipv6Addr::new(
            0x2001, 0xdb8, 0, 0, 0, 0, 0, 1
        )));
    }

    #[test]
    fn test_is_ssrf_vulnerable_hostname() {
        // Localhost variants
        assert!(is_ssrf_vulnerable_hostname("localhost"));
        assert!(is_ssrf_vulnerable_hostname("LOCALHOST"));
        assert!(is_ssrf_vulnerable_hostname("Localhost"));
        assert!(is_ssrf_vulnerable_hostname("127.0.0.1"));
        assert!(is_ssrf_vulnerable_hostname("::1"));

        // Metadata services
        assert!(is_ssrf_vulnerable_hostname("169.254.169.254"));
        assert!(is_ssrf_vulnerable_hostname("metadata.google.internal"));

        // Safe hostnames
        assert!(!is_ssrf_vulnerable_hostname("example.com"));
        assert!(!is_ssrf_vulnerable_hostname("google.com"));
        assert!(!is_ssrf_vulnerable_hostname("8.8.8.8"));
    }

    #[test]
    fn test_validate_url_against_ssrf() {
        // Safe URLs
        let safe_url = Url::parse("https://example.com").unwrap();
        assert!(validate_url_against_ssrf(&safe_url).is_none());

        let safe_ip = Url::parse("https://8.8.8.8").unwrap();
        assert!(validate_url_against_ssrf(&safe_ip).is_none());

        // Unsafe URLs - localhost
        let localhost = Url::parse("http://localhost").unwrap();
        let result = validate_url_against_ssrf(&localhost);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Localhost"));

        // Unsafe URLs - private IPv4
        let private_ip = Url::parse("http://192.168.1.1").unwrap();
        let result = validate_url_against_ssrf(&private_ip);
        assert!(result.is_some());
        assert!(result.unwrap().contains("IPv4"));

        // Unsafe URLs - loopback
        let loopback = Url::parse("http://127.0.0.1").unwrap();
        let result = validate_url_against_ssrf(&loopback);
        assert!(result.is_some());
        assert!(result.unwrap().contains("IPv4"));

        // Unsafe URLs - metadata service
        let metadata = Url::parse("http://169.254.169.254").unwrap();
        let result = validate_url_against_ssrf(&metadata);
        assert!(result.is_some());
        assert!(result.unwrap().contains("IPv4"));
    }
}
