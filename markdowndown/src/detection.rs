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
            // Wildcard subdomain matching
            let base_domain = &self.domain_pattern[2..];
            host == base_domain || host.ends_with(&format!(".{base_domain}"))
        } else {
            // Exact domain matching
            host == self.domain_pattern
        }
    }

    /// Checks if a path matches the pattern.
    fn matches_path(&self, path: &str, pattern: &str) -> bool {
        if pattern.contains("*") {
            // Simple wildcard matching
            if let Some(prefix) = pattern.strip_suffix("*") {
                path.starts_with(prefix)
            } else if let Some(suffix) = pattern.strip_prefix("*") {
                path.ends_with(suffix)
            } else {
                // Pattern contains * in the middle - basic implementation
                let parts: Vec<&str> = pattern.split('*').collect();
                if parts.len() == 2 {
                    path.starts_with(parts[0]) && path.ends_with(parts[1])
                } else {
                    false
                }
            }
        } else {
            // Exact path matching
            path.starts_with(pattern)
        }
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

        // Check for local file paths first (before trying to parse as URL)
        if crate::utils::is_local_file_path(trimmed) {
            return Ok(UrlType::LocalFile);
        }

        // Try to parse as URL for web-based sources
        let parsed_url = self.parse_url(url)?;

        // Special handling for GitHub issues (more complex pattern)
        if self.is_github_issue_url(&parsed_url) {
            return Ok(UrlType::GitHubIssue);
        }

        // Check each pattern to find a match
        for pattern in &self.patterns {
            if pattern.matches(&parsed_url) {
                return Ok(pattern.url_type.clone());
            }
        }

        // Default to HTML for any other HTTP/HTTPS URLs
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

        // Handle local file paths separately (no URL parsing needed)
        if crate::utils::is_local_file_path(trimmed) {
            return Ok(trimmed.to_string());
        }

        // Parse as URL for web-based sources
        let mut parsed_url = self.parse_url(trimmed)?;

        // Remove tracking parameters
        let query_pairs: Vec<(String, String)> = parsed_url
            .query_pairs()
            .filter(|(key, _)| !self.tracking_params.contains(key.as_ref()))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        // Clear existing query and rebuild without tracking params
        parsed_url.set_query(None);
        if !query_pairs.is_empty() {
            let query_string = query_pairs
                .iter()
                .map(|(k, v)| {
                    if v.is_empty() {
                        k.clone()
                    } else {
                        format!("{k}={v}")
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            parsed_url.set_query(Some(&query_string));
        }

        Ok(parsed_url.to_string())
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

    /// Checks if a URL matches a GitHub issue or pull request pattern.
    fn is_github_issue_url(&self, parsed_url: &ParsedUrl) -> bool {
        let host = parsed_url.host_str();
        if host != Some("github.com") && host != Some("api.github.com") {
            return false;
        }

        let path = parsed_url.path();
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match host {
            Some("github.com") => {
                // GitHub issue/PR URLs have the pattern: /{owner}/{repo}/issues/{number} or /{owner}/{repo}/pull/{number}
                // Need exactly 4 or more segments: owner, repo, "issues"/"pull", number
                if path_segments.len() >= 4 {
                    if let (Some(resource_segment), Some(number_segment)) =
                        (path_segments.get(2), path_segments.get(3))
                    {
                        if (*resource_segment == "issues" || *resource_segment == "pull")
                            && number_segment.parse::<u32>().is_ok()
                        {
                            return true;
                        }
                    }
                }
            }
            Some("api.github.com") => {
                // GitHub API URLs have the pattern: /repos/{owner}/{repo}/issues/{number} or /repos/{owner}/{repo}/pulls/{number}
                // Need exactly 5 or more segments: "repos", owner, repo, "issues"/"pulls", number
                if path_segments.len() >= 5 {
                    if let (Some(repos_segment), Some(resource_segment), Some(number_segment)) = (
                        path_segments.first(),
                        path_segments.get(3),
                        path_segments.get(4),
                    ) {
                        if *repos_segment == "repos"
                            && (*resource_segment == "issues" || *resource_segment == "pulls")
                            && number_segment.parse::<u32>().is_ok()
                        {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }

        false
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
