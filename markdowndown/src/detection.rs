//! URL type detection and classification module.
//!
//! This module provides intelligent URL type detection to route different URL types
//! to appropriate handlers. It supports detection of Google Docs, Office 365,
//! GitHub Issues, and generic HTML URLs.
//!
//! # Examples
//!
//! ## Basic URL Detection
//!
//! ```rust
//! use markdowndown::detection::UrlDetector;
//! use markdowndown::types::UrlType;
//!
//! let detector = UrlDetector::new();
//!
//! // Detect Google Docs URL
//! let url = "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";
//! let url_type = detector.detect_type(url)?;
//! assert_eq!(url_type, UrlType::GoogleDocs);
//!
//! // Detect GitHub Issues URL
//! let url = "https://github.com/owner/repo/issues/123";
//! let url_type = detector.detect_type(url)?;
//! assert_eq!(url_type, UrlType::GitHubIssue);
//! # Ok::<(), markdowndown::types::MarkdownError>(())
//! ```
//!
//! ## URL Normalization
//!
//! ```rust
//! use markdowndown::detection::UrlDetector;
//!
//! let detector = UrlDetector::new();
//!
//! // Normalize URL with tracking parameters
//! let url = "https://example.com/page?utm_source=test&content=important";
//! let normalized = detector.normalize_url(url)?;
//! assert_eq!(normalized, "https://example.com/page?content=important");
//! # Ok::<(), markdowndown::types::MarkdownError>(())
//! ```

use crate::types::{MarkdownError, UrlType};
use std::collections::HashSet;
use url::Url as ParsedUrl;

// GitHub URL structure constants
const GITHUB_WEB_MIN_PATH_SEGMENTS: usize = 4;
const GITHUB_WEB_RESOURCE_TYPE_INDEX: usize = 2;
const GITHUB_API_MIN_PATH_SEGMENTS: usize = 5;
const GITHUB_API_RESOURCE_TYPE_INDEX: usize = 3;
const GITHUB_RESOURCE_NUMBER_OFFSET: usize = 1;

// Wildcard pattern constants
const WILDCARD_PATTERN_PARTS: usize = 2;
const WILDCARD_PREFIX_LEN: usize = 2;

/// URL pattern configuration for different URL types.
#[derive(Debug, Clone)]
struct Pattern {
    /// Domain pattern to match (can contain wildcards)
    domain_pattern: String,
    /// Path pattern to match (optional)
    path_pattern: Option<String>,
    /// The URL type this pattern represents
    url_type: UrlType,
}

impl Pattern {
    /// Creates a new pattern configuration.
    fn new(domain_pattern: &str, path_pattern: Option<&str>, url_type: UrlType) -> Self {
        Self {
            domain_pattern: domain_pattern.to_string(),
            path_pattern: path_pattern.map(|s| s.to_string()),
            url_type,
        }
    }

    /// Checks if a URL matches this pattern.
    fn matches(&self, parsed_url: &ParsedUrl) -> bool {
        let host = match parsed_url.host_str() {
            Some(host) => host,
            None => return false,
        };

        // Check domain pattern
        if !self.matches_domain(host) {
            return false;
        }

        // Check path pattern if specified
        if let Some(ref path_pattern) = self.path_pattern {
            let path = parsed_url.path();
            if !self.matches_path(path, path_pattern) {
                return false;
            }
        }

        true
    }

    /// Checks if a domain matches the pattern (supports wildcards).
    fn matches_domain(&self, host: &str) -> bool {
        if self.domain_pattern.starts_with("*.") {
            self.matches_wildcard_subdomain(host)
        } else {
            host == self.domain_pattern
        }
    }

    /// Checks if a host matches a wildcard subdomain pattern.
    fn matches_wildcard_subdomain(&self, host: &str) -> bool {
        let base_domain = &self.domain_pattern[WILDCARD_PREFIX_LEN..];
        host == base_domain || self.is_subdomain_match(host, base_domain)
    }

    /// Checks if a host is a subdomain of a base domain.
    fn is_subdomain_match(&self, host: &str, base_domain: &str) -> bool {
        host.ends_with(base_domain)
            && host.len() > base_domain.len()
            && host.as_bytes()[host.len() - base_domain.len() - 1] == b'.'
    }

    /// Checks if a path matches the pattern.
    fn matches_path(&self, path: &str, pattern: &str) -> bool {
        if pattern.contains("*") {
            self.matches_wildcard_path(path, pattern)
        } else {
            path.starts_with(pattern)
        }
    }

    /// Checks if a path matches a wildcard pattern.
    fn matches_wildcard_path(&self, path: &str, pattern: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix("*") {
            return path.starts_with(prefix);
        }
        if let Some(suffix) = pattern.strip_prefix("*") {
            return path.ends_with(suffix);
        }
        self.matches_middle_wildcard(path, pattern)
    }

    /// Checks if a path matches a pattern with wildcard in the middle.
    fn matches_middle_wildcard(&self, path: &str, pattern: &str) -> bool {
        let parts: Vec<&str> = pattern.split('*').collect();
        parts.len() == WILDCARD_PATTERN_PARTS
            && path.starts_with(parts[0])
            && path.ends_with(parts[1])
    }
}

/// URL detector for intelligent URL type classification.
#[derive(Debug)]
pub struct UrlDetector {
    /// Configured URL patterns for detection
    patterns: Vec<Pattern>,
    /// Tracking parameters to remove during normalization
    tracking_params: HashSet<String>,
}

impl UrlDetector {
    /// Creates a new URL detector with default patterns.
    pub fn new() -> Self {
        let patterns = vec![
            // Google Docs patterns
            Pattern::new("docs.google.com", Some("/document/"), UrlType::GoogleDocs),
            Pattern::new("drive.google.com", Some("/file/"), UrlType::GoogleDocs),
            // GitHub patterns (handled separately due to complexity)
        ];

        let tracking_params = [
            "utm_source",
            "utm_medium",
            "utm_campaign",
            "utm_term",
            "utm_content",
            "ref",
            "source",
            "campaign",
            "medium",
            "term",
            "gclid",
            "fbclid",
            "msclkid",
            "_ga",
            "_gid",
            "mc_cid",
            "mc_eid",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            patterns,
            tracking_params,
        }
    }

    /// Detects the URL type for a given URL string.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL string to analyze
    ///
    /// # Returns
    ///
    /// Returns the detected `UrlType` or a `MarkdownError` if the URL is invalid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::detection::UrlDetector;
    /// use markdowndown::types::UrlType;
    ///
    /// let detector = UrlDetector::new();
    /// let url_type = detector.detect_type("https://docs.google.com/document/d/123/edit")?;
    /// assert_eq!(url_type, UrlType::GoogleDocs);
    /// # Ok::<(), markdowndown::types::MarkdownError>(())
    /// ```
    pub fn detect_type(&self, url: &str) -> Result<UrlType, MarkdownError> {
        let trimmed = url.trim();

        if crate::utils::is_local_file_path(trimmed) {
            return Ok(UrlType::LocalFile);
        }

        let parsed_url = self.parse_url(url)?;
        self.classify_web_url(&parsed_url)
    }

    /// Classifies a web URL by matching it against known patterns.
    fn classify_web_url(&self, parsed_url: &ParsedUrl) -> Result<UrlType, MarkdownError> {
        if self.is_github_issue_url(parsed_url) {
            return Ok(UrlType::GitHubIssue);
        }

        for pattern in &self.patterns {
            if pattern.matches(parsed_url) {
                return Ok(pattern.url_type.clone());
            }
        }

        Ok(UrlType::Html)
    }

    /// Normalizes a URL by cleaning and validating it.
    ///
    /// This method:
    /// - Trims whitespace
    /// - Ensures HTTPS scheme where possible
    /// - Removes tracking parameters
    /// - Validates URL structure
    ///
    /// # Arguments
    ///
    /// * `url` - The URL string to normalize
    ///
    /// # Returns
    ///
    /// Returns the normalized URL string or a `MarkdownError` if invalid.
    pub fn normalize_url(&self, url: &str) -> Result<String, MarkdownError> {
        let trimmed = url.trim();

        if crate::utils::is_local_file_path(trimmed) {
            return Ok(trimmed.to_string());
        }

        let mut parsed_url = self.parse_url(trimmed)?;
        self.remove_tracking_params(&mut parsed_url);
        Ok(parsed_url.to_string())
    }

    /// Removes tracking parameters from a parsed URL.
    fn remove_tracking_params(&self, parsed_url: &mut ParsedUrl) {
        let query_pairs = self.get_non_tracking_params(parsed_url);
        self.set_cleaned_query(parsed_url, query_pairs);
    }

    /// Extracts query parameters that are not tracking parameters.
    fn get_non_tracking_params(&self, parsed_url: &ParsedUrl) -> Vec<(String, String)> {
        parsed_url
            .query_pairs()
            .filter(|(key, _)| !self.tracking_params.contains(key.as_ref()))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Sets the cleaned query string on a parsed URL.
    fn set_cleaned_query(&self, parsed_url: &mut ParsedUrl, query_pairs: Vec<(String, String)>) {
        parsed_url.set_query(None);
        if !query_pairs.is_empty() {
            let query_string = self.build_query_string(&query_pairs);
            parsed_url.set_query(Some(&query_string));
        }
    }

    /// Builds a query string from key-value pairs.
    fn build_query_string(&self, pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| {
                if v.is_empty() {
                    k.clone()
                } else {
                    format!("{k}={v}")
                }
            })
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Validates a URL without normalization.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL string to validate
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if valid, or a `MarkdownError` if invalid.
    pub fn validate_url(&self, url: &str) -> Result<(), MarkdownError> {
        let trimmed = url.trim();

        // Allow local file paths
        if crate::utils::is_local_file_path(trimmed) {
            return Ok(());
        }

        // Basic validation - must be HTTP or HTTPS for web URLs
        if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
            let context = crate::types::ErrorContext::new(url, "URL validation", "validate_url");
            return Err(MarkdownError::ValidationError {
                kind: crate::types::ValidationErrorKind::InvalidUrl,
                context,
            });
        }

        ParsedUrl::parse(trimmed).map_err(|_parse_error| {
            let context = crate::types::ErrorContext::new(url, "URL parsing", "validate_url");
            MarkdownError::ValidationError {
                kind: crate::types::ValidationErrorKind::InvalidUrl,
                context,
            }
        })?;
        Ok(())
    }

    /// Parses a URL string into a parsed URL, handling common issues.
    fn parse_url(&self, url: &str) -> Result<ParsedUrl, MarkdownError> {
        let trimmed = url.trim();

        // Basic validation - must be HTTP or HTTPS
        if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
            let context =
                crate::types::ErrorContext::new(url, "URL parsing", "UrlDetector::parse_url");
            return Err(MarkdownError::ValidationError {
                kind: crate::types::ValidationErrorKind::InvalidUrl,
                context,
            });
        }

        ParsedUrl::parse(trimmed).map_err(|parse_error| {
            let context =
                crate::types::ErrorContext::new(url, "URL parsing", "UrlDetector::parse_url")
                    .with_info(format!("Parse error: {parse_error}"));
            MarkdownError::ValidationError {
                kind: crate::types::ValidationErrorKind::InvalidUrl,
                context,
            }
        })
    }

    /// Validates GitHub path segments for issue/PR patterns.
    fn validate_github_path_segments(
        segments: &[&str],
        expected_len: usize,
        expected_pos: usize,
    ) -> bool {
        if segments.len() < expected_len {
            return false;
        }

        let Some(resource) = segments.get(expected_pos) else {
            return false;
        };
        let Some(number) = segments.get(expected_pos + GITHUB_RESOURCE_NUMBER_OFFSET) else {
            return false;
        };

        (*resource == "issues" || *resource == "pull" || *resource == "pulls")
            && number.parse::<u32>().is_ok()
    }

    /// Checks if path segments match GitHub API pattern.
    fn is_github_api_path(segments: &[&str]) -> bool {
        segments.first() == Some(&"repos")
            && Self::validate_github_path_segments(
                segments,
                GITHUB_API_MIN_PATH_SEGMENTS,
                GITHUB_API_RESOURCE_TYPE_INDEX,
            )
    }

    /// Checks if path segments match GitHub web pattern.
    fn is_github_web_path(segments: &[&str]) -> bool {
        Self::validate_github_path_segments(
            segments,
            GITHUB_WEB_MIN_PATH_SEGMENTS,
            GITHUB_WEB_RESOURCE_TYPE_INDEX,
        )
    }

    /// Checks if a URL matches a GitHub issue or pull request pattern.
    fn is_github_issue_url(&self, parsed_url: &ParsedUrl) -> bool {
        if !self.is_github_host(parsed_url) {
            return false;
        }

        let path_segments = self.extract_path_segments(parsed_url);
        self.matches_github_issue_pattern(parsed_url.host_str(), &path_segments)
    }

    /// Checks if the parsed URL is a GitHub host.
    fn is_github_host(&self, parsed_url: &ParsedUrl) -> bool {
        matches!(
            parsed_url.host_str(),
            Some("github.com") | Some("api.github.com")
        )
    }

    /// Extracts path segments from a parsed URL.
    fn extract_path_segments<'a>(&self, parsed_url: &'a ParsedUrl) -> Vec<&'a str> {
        parsed_url
            .path()
            .split('/')
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Checks if the path segments match a GitHub issue pattern.
    fn matches_github_issue_pattern(&self, host: Option<&str>, segments: &[&str]) -> bool {
        match host {
            Some("github.com") => Self::is_github_web_path(segments),
            Some("api.github.com") => Self::is_github_api_path(segments),
            _ => false,
        }
    }
}

impl Default for UrlDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urldetector_new() {
        let detector = UrlDetector::new();
        assert!(!detector.patterns.is_empty());
        assert!(!detector.tracking_params.is_empty());
    }

    #[test]
    fn test_detect_google_docs_document() {
        let detector = UrlDetector::new();
        let url =
            "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";
        let result = detector.detect_type(url).unwrap();
        assert_eq!(result, UrlType::GoogleDocs);
    }

    #[test]
    fn test_detect_google_drive_file() {
        let detector = UrlDetector::new();
        let url = "https://drive.google.com/file/d/1234567890/view";
        let result = detector.detect_type(url).unwrap();
        assert_eq!(result, UrlType::GoogleDocs);
    }

    #[test]
    fn test_detect_github_issue() {
        let detector = UrlDetector::new();
        let url = "https://github.com/owner/repo/issues/123";
        let result = detector.detect_type(url).unwrap();
        assert_eq!(result, UrlType::GitHubIssue);
    }

    #[test]
    fn test_detect_html_fallback() {
        let detector = UrlDetector::new();
        let url = "https://example.com/article.html";
        let result = detector.detect_type(url).unwrap();
        assert_eq!(result, UrlType::Html);
    }

    #[test]
    fn test_normalize_url_removes_tracking() {
        let detector = UrlDetector::new();
        let url = "https://example.com/page?utm_source=test&content=important&utm_medium=email";
        let normalized = detector.normalize_url(url).unwrap();
        assert_eq!(normalized, "https://example.com/page?content=important");
    }

    #[test]
    fn test_normalize_url_trims_whitespace() {
        let detector = UrlDetector::new();
        let url = "  https://example.com/page  ";
        let normalized = detector.normalize_url(url).unwrap();
        assert_eq!(normalized, "https://example.com/page");
    }

    #[test]
    fn test_validate_url_valid() {
        let detector = UrlDetector::new();
        let url = "https://example.com";
        assert!(detector.validate_url(url).is_ok());
    }

    #[test]
    fn test_validate_url_invalid() {
        let detector = UrlDetector::new();
        let url = "not-a-url";
        assert!(detector.validate_url(url).is_err());
    }

    #[test]
    fn test_pattern_domain_wildcard_matching() {
        let pattern = Pattern::new("*.sharepoint.com", None, UrlType::Html);
        let url = ParsedUrl::parse("https://company.sharepoint.com/sites/team").unwrap();
        assert!(pattern.matches(&url));

        let url2 = ParsedUrl::parse("https://sharepoint.com/sites/team").unwrap();
        assert!(pattern.matches(&url2));

        let url3 = ParsedUrl::parse("https://example.com/sites/team").unwrap();
        assert!(!pattern.matches(&url3));
    }

    #[test]
    fn test_pattern_path_matching() {
        let pattern = Pattern::new("docs.google.com", Some("/document/"), UrlType::GoogleDocs);
        let url = ParsedUrl::parse("https://docs.google.com/document/d/123/edit").unwrap();
        assert!(pattern.matches(&url));

        let url2 = ParsedUrl::parse("https://docs.google.com/spreadsheets/d/123").unwrap();
        assert!(!pattern.matches(&url2));
    }

    #[test]
    fn test_github_issue_and_pr_url_detection() {
        let detector = UrlDetector::new();

        // Valid GitHub issue and pull request URLs
        let valid_urls = [
            "https://github.com/owner/repo/issues/123",
            "https://github.com/microsoft/vscode/issues/42",
            "https://github.com/rust-lang/rust/issues/12345",
            "https://github.com/owner/repo/pull/123",
            "https://github.com/microsoft/vscode/pull/456",
            "https://github.com/rust-lang/rust/pull/98765",
        ];

        for url in &valid_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::GitHubIssue, "Failed for URL: {url}");
        }

        // Invalid GitHub URLs (should fall back to HTML)
        let invalid_urls = [
            "https://github.com/owner/repo",
            "https://github.com/owner/repo/issues",
            "https://github.com/owner/repo/issues/abc",
            "https://github.com/owner/repo/pull",
            "https://github.com/owner/repo/pull/abc",
            "https://github.com/owner/repo/commits/123",
        ];

        for url in &invalid_urls {
            let result = detector.detect_type(url).unwrap();
            assert_eq!(result, UrlType::Html, "Failed for URL: {url}");
        }
    }

    #[test]
    fn test_edge_case_urls() {
        let detector = UrlDetector::new();

        // URL with query parameters
        let url = "https://docs.google.com/document/d/123/edit?usp=sharing";
        let result = detector.detect_type(url).unwrap();
        assert_eq!(result, UrlType::GoogleDocs);

        // URL with fragment (issue)
        let url = "https://github.com/owner/repo/issues/123#issuecomment-456";
        let result = detector.detect_type(url).unwrap();
        assert_eq!(result, UrlType::GitHubIssue);

        // URL with fragment (pull request)
        let url = "https://github.com/owner/repo/pull/789#pullrequestreview-123";
        let result = detector.detect_type(url).unwrap();
        assert_eq!(result, UrlType::GitHubIssue);
    }

    #[test]
    fn test_normalize_url_preserves_important_params() {
        let detector = UrlDetector::new();
        let url = "https://docs.google.com/document/d/123/edit?usp=sharing&utm_source=email";
        let normalized = detector.normalize_url(url).unwrap();
        assert!(normalized.contains("usp=sharing"));
        assert!(!normalized.contains("utm_source"));
    }

    #[test]
    fn test_invalid_url_error_handling() {
        let detector = UrlDetector::new();

        let invalid_urls = [
            "not-a-url",
            "ftp://example.com",
            "mailto:test@example.com",
            "",
            "   ",
        ];

        for url in &invalid_urls {
            let result = detector.detect_type(url);
            assert!(result.is_err(), "Should fail for URL: {url}");

            match result.unwrap_err() {
                MarkdownError::ValidationError { kind, context } => {
                    assert_eq!(kind, crate::types::ValidationErrorKind::InvalidUrl);
                    assert_eq!(context.url, *url);
                }
                _ => panic!("Expected ValidationError with InvalidUrl kind for: {url}"),
            }
        }
    }
}
