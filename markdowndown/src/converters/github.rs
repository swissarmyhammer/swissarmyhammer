//! GitHub Issues and Pull Requests to markdown conversion with REST API integration.
//!
//! This module provides conversion of GitHub issues and pull requests to markdown format
//! by leveraging the GitHub REST API. It handles issue fetching, comment retrieval,
//! authentication, and proper markdown rendering.
//!
//! # Supported URLs
//!
//! - Issues: `https://github.com/{owner}/{repo}/issues/{number}`
//! - Pull Requests: `https://github.com/{owner}/{repo}/pull/{number}`
//!
//! # Usage Examples
//!
//! ## Basic Conversion (Public Repository)
//!
//! ```rust
//! use markdowndown::converters::GitHubConverter;
//!
//! # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
//! let converter = GitHubConverter::new();
//! let url = "https://github.com/microsoft/vscode/issues/1234";
//! let markdown = converter.convert(url).await?;
//! println!("Markdown content: {}", markdown);
//! # Ok(())
//! # }
//! ```
//!
//! ## Authenticated Conversion (Private Repository)
//!
//! ```rust
//! use markdowndown::converters::GitHubConverter;
//!
//! # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
//! let token = std::env::var("GITHUB_TOKEN").unwrap();
//! let converter = GitHubConverter::new_with_token(token);
//! let url = "https://github.com/private/repo/issues/42";
//! let markdown = converter.convert(url).await?;
//! println!("Markdown content: {}", markdown);
//! # Ok(())
//! # }
//! ```

use crate::client::HttpClient;
use crate::frontmatter::FrontmatterBuilder;
use crate::types::{Markdown, MarkdownError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use url::Url as ParsedUrl;

/// Default GitHub API base URL
const DEFAULT_GITHUB_API_BASE_URL: &str = "https://api.github.com";

/// GitHub API version header value
const GITHUB_API_VERSION: &str = "application/vnd.github.v3+json";

/// User-Agent string prefix for GitHub API requests
const USER_AGENT_PREFIX: &str = "markdowndown";

/// GitHub resource types supported for conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceType {
    /// GitHub issue
    Issue,
    /// GitHub pull request
    PullRequest,
}

impl ResourceType {
    /// Returns the API path segment for this resource type.
    pub fn api_path(&self) -> &'static str {
        match self {
            ResourceType::Issue => "issues",
            ResourceType::PullRequest => "issues", // PRs use same API endpoint as issues
        }
    }

    /// Returns the string representation for frontmatter.
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Issue => "issue",
            ResourceType::PullRequest => "pull_request",
        }
    }
}

/// Represents a parsed GitHub URL with metadata.
#[derive(Debug, Clone)]
pub struct GitHubResource {
    /// Repository owner (user or organization)
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// Issue or PR number
    pub number: u32,
    /// Type of resource (issue or pull request)
    pub resource_type: ResourceType,
    /// Original URL for reference
    pub original_url: String,
}

/// GitHub issue or pull request data from API.
#[derive(Debug, Clone, Deserialize)]
pub struct Issue {
    /// Issue ID (GitHub's internal identifier)
    pub id: u64,
    /// Issue number
    pub number: u32,
    /// Issue title
    pub title: String,
    /// Issue body content (markdown)
    #[serde(default)]
    pub body: Option<String>,
    /// Issue state (open, closed)
    pub state: String,
    /// User who created the issue
    pub user: User,
    /// Issue creation timestamp
    pub created_at: DateTime<Utc>,
    /// Issue update timestamp
    pub updated_at: DateTime<Utc>,
    /// Issue labels
    #[serde(default)]
    pub labels: Vec<Label>,
    /// Whether this is a pull request
    pub pull_request: Option<PullRequestRef>,
}

/// GitHub user information.
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    /// Username
    pub login: String,
    /// User ID
    pub id: u64,
}

/// GitHub label information.
#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    /// Label name
    pub name: String,
    /// Label color (hex)
    pub color: String,
}

/// Reference to pull request data (indicates if issue is actually a PR).
#[derive(Debug, Clone, Deserialize)]
pub struct PullRequestRef {
    /// PR URL
    pub url: String,
    /// PR HTML URL
    pub html_url: String,
}

/// GitHub comment data from API.
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    /// Comment ID
    pub id: u64,
    /// Comment body content (markdown)
    #[serde(default)]
    pub body: Option<String>,
    /// User who created the comment
    pub user: User,
    /// Comment creation timestamp
    pub created_at: DateTime<Utc>,
    /// Comment update timestamp
    pub updated_at: DateTime<Utc>,
}

/// GitHub reaction data.
#[derive(Debug, Clone, Deserialize)]
pub struct Reaction {
    /// Reaction content (emoji shortcode)
    pub content: String,
    /// User who made the reaction
    pub user: User,
}

/// Aggregated reaction counts.
#[derive(Debug, Clone, Default)]
pub struct ReactionCounts {
    /// Reaction emoji to count mapping
    pub counts: HashMap<String, u32>,
}

impl ReactionCounts {
    /// Creates reaction counts from a list of reactions.
    pub fn from_reactions(reactions: &[Reaction]) -> Self {
        let mut counts = HashMap::new();
        for reaction in reactions {
            *counts.entry(reaction.content.clone()).or_insert(0) += 1;
        }
        Self { counts }
    }

    /// Formats reaction counts as a string for display.
    pub fn format(&self) -> String {
        if self.counts.is_empty() {
            return String::new();
        }

        let formatted: Vec<String> = self
            .counts
            .iter()
            .map(|(emoji, count)| {
                let display_emoji = match emoji.as_str() {
                    "+1" => "👍",
                    "-1" => "👎",
                    "laugh" => "😄",
                    "confused" => "😕",
                    "heart" => "❤️",
                    "hooray" => "🎉",
                    "rocket" => "🚀",
                    "eyes" => "👀",
                    _ => emoji,
                };
                format!("{display_emoji} {count}")
            })
            .collect();

        formatted.join(" | ")
    }
}

/// GitHub to markdown converter with REST API integration and authentication.
///
/// This converter handles GitHub issues and pull requests by fetching data
/// via the GitHub REST API and rendering it as markdown with complete
/// metadata and comment history.
#[derive(Debug, Clone)]
pub struct GitHubConverter {
    /// HTTP client for making requests to GitHub API
    client: HttpClient,
    /// Optional GitHub personal access token for authentication
    auth_token: Option<String>,
    /// Base URL for GitHub API (allows for GitHub Enterprise)
    api_base_url: String,
}

impl GitHubConverter {
    /// Creates a new GitHub converter without authentication.
    ///
    /// This converter can only access public repositories and is subject
    /// to lower rate limits (60 requests per hour per IP).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GitHubConverter;
    ///
    /// let converter = GitHubConverter::new();
    /// ```
    pub fn new() -> Self {
        Self {
            client: HttpClient::new(),
            auth_token: None,
            api_base_url: DEFAULT_GITHUB_API_BASE_URL.to_string(),
        }
    }

    /// Creates a new GitHub converter with authentication token.
    ///
    /// This converter can access private repositories and has higher
    /// rate limits (5000 requests per hour per token).
    ///
    /// # Arguments
    ///
    /// * `token` - GitHub personal access token
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GitHubConverter;
    ///
    /// let token = "ghp_xxxxxxxxxxxxxxxxxxxx".to_string();
    /// let converter = GitHubConverter::new_with_token(token);
    /// ```
    pub fn new_with_token(token: String) -> Self {
        Self {
            client: HttpClient::new(),
            auth_token: Some(token),
            api_base_url: DEFAULT_GITHUB_API_BASE_URL.to_string(),
        }
    }

    /// Creates a GitHub converter with custom API base URL (primarily for testing).
    ///
    /// # Arguments
    ///
    /// * `token` - Optional GitHub personal access token
    /// * `api_base_url` - Custom API base URL (e.g., for testing with mock servers)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GitHubConverter;
    ///
    /// // Create converter with mock server URL for testing
    /// let converter = GitHubConverter::new_with_config(
    ///     Some("token".to_string()),
    ///     "http://localhost:8080".to_string()
    /// );
    /// ```
    pub fn new_with_config(token: Option<String>, api_base_url: String) -> Self {
        Self {
            client: HttpClient::new(),
            auth_token: token,
            api_base_url,
        }
    }

    /// Creates a GitHub converter with authentication from environment variable.
    ///
    /// Looks for the GITHUB_TOKEN environment variable and uses it for authentication.
    /// Falls back to unauthenticated mode if the variable is not set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GitHubConverter;
    ///
    /// // Set GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx in environment
    /// let converter = GitHubConverter::from_env();
    /// ```
    pub fn from_env() -> Self {
        match std::env::var("GITHUB_TOKEN") {
            Ok(token) if !token.trim().is_empty() => Self::new_with_token(token),
            _ => Self::new(),
        }
    }

    /// Converts a GitHub issue or pull request URL to markdown with frontmatter.
    ///
    /// This method performs the complete conversion workflow:
    /// 1. Parse and validate the GitHub URL
    /// 2. Fetch issue/PR data from GitHub API
    /// 3. Fetch all comments and reactions
    /// 4. Render issue and comments as markdown
    /// 5. Generate frontmatter with metadata
    /// 6. Combine frontmatter with content
    ///
    /// # Arguments
    ///
    /// * `url` - The GitHub issue or pull request URL to convert
    ///
    /// # Returns
    ///
    /// Returns a `Markdown` instance containing the issue/PR content with frontmatter,
    /// or a `MarkdownError` on failure.
    ///
    /// # Errors
    ///
    /// * `MarkdownError::InvalidUrl` - If the URL format is invalid
    /// * `MarkdownError::AuthError` - If authentication is required but not provided
    /// * `MarkdownError::NetworkError` - For API errors, rate limiting, or network failures
    /// * `MarkdownError::ParseError` - If API response parsing fails
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GitHubConverter;
    ///
    /// # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
    /// let converter = GitHubConverter::new();
    /// let url = "https://github.com/microsoft/vscode/issues/1234";
    /// let markdown = converter.convert(url).await?;
    ///
    /// // The result includes frontmatter with metadata
    /// assert!(markdown.as_str().contains("source_url:"));
    /// assert!(markdown.as_str().contains("github_issue_number:"));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn convert(&self, url: &str) -> Result<Markdown, MarkdownError> {
        // Step 1: Parse and validate the GitHub URL
        let resource = self.parse_github_url(url)?;

        // Step 2-3: Fetch issue/PR data and comments from GitHub API
        let (issue, comments) = self.fetch_issue_and_comments(&resource).await?;

        // Step 4-6: Render content and create final markdown
        self.create_markdown_document(&resource, &issue, &comments)
    }

    /// Fetches issue/PR data and comments in parallel for better performance.
    async fn fetch_issue_and_comments(
        &self,
        resource: &GitHubResource,
    ) -> Result<(Issue, Vec<Comment>), MarkdownError> {
        let issue_future = self.fetch_issue(&resource.owner, &resource.repo, resource.number);
        let comments_future = self.fetch_comments(&resource.owner, &resource.repo, resource.number);

        // Fetch both concurrently
        let (issue, comments) = tokio::try_join!(issue_future, comments_future)?;
        Ok((issue, comments))
    }

    /// Creates the final markdown document with frontmatter and content.
    fn create_markdown_document(
        &self,
        resource: &GitHubResource,
        issue: &Issue,
        comments: &[Comment],
    ) -> Result<Markdown, MarkdownError> {
        // Render issue and comments as markdown
        let content = self.render_markdown(issue, comments);

        // Generate frontmatter with metadata
        let frontmatter = self.build_frontmatter(resource, issue)?;

        // Combine frontmatter with content
        let markdown_with_frontmatter = format!("{frontmatter}\n{content}");

        Markdown::new(markdown_with_frontmatter)
    }

    /// Parses a GitHub URL and extracts resource information.
    ///
    /// # Arguments
    ///
    /// * `url` - The GitHub URL to parse
    ///
    /// # Returns
    ///
    /// Returns a `GitHubResource` with parsed information, or a `MarkdownError` if parsing fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GitHubConverter;
    ///
    /// let converter = GitHubConverter::new();
    /// let url = "https://github.com/microsoft/vscode/issues/1234";
    /// let resource = converter.parse_github_url(url)?;
    /// assert_eq!(resource.owner, "microsoft");
    /// assert_eq!(resource.repo, "vscode");
    /// assert_eq!(resource.number, 1234);
    /// # Ok::<(), markdowndown::types::MarkdownError>(())
    /// ```
    pub fn parse_github_url(&self, url: &str) -> Result<GitHubResource, MarkdownError> {
        let parsed_url = ParsedUrl::parse(url.trim()).map_err(|_| MarkdownError::InvalidUrl {
            url: url.to_string(),
        })?;

        // Check if this is a GitHub URL
        let host = parsed_url
            .host_str()
            .ok_or_else(|| MarkdownError::InvalidUrl {
                url: url.to_string(),
            })?;

        if host != "github.com" {
            return Err(MarkdownError::InvalidUrl {
                url: url.to_string(),
            });
        }

        // Parse path segments: /{owner}/{repo}/issues/{number} or /{owner}/{repo}/pull/{number}
        let path = parsed_url.path();
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if segments.len() != 4 {
            return Err(MarkdownError::InvalidUrl {
                url: url.to_string(),
            });
        }

        let owner = segments[0].to_string();
        let repo = segments[1].to_string();
        let resource_type_str = segments[2];
        let number_str = segments[3];

        // Determine resource type
        let resource_type = match resource_type_str {
            "issues" => ResourceType::Issue,
            "pull" => ResourceType::PullRequest,
            _ => {
                return Err(MarkdownError::InvalidUrl {
                    url: url.to_string(),
                })
            }
        };

        // Parse issue/PR number
        let number = number_str
            .parse::<u32>()
            .map_err(|_| MarkdownError::InvalidUrl {
                url: url.to_string(),
            })?;

        Ok(GitHubResource {
            owner,
            repo,
            number,
            resource_type,
            original_url: url.to_string(),
        })
    }

    /// Fetches issue or pull request data from GitHub API.
    pub async fn fetch_issue(
        &self,
        owner: &str,
        repo: &str,
        number: u32,
    ) -> Result<Issue, MarkdownError> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.api_base_url, owner, repo, number
        );

        let response_text = self.make_api_request(&url).await?;

        serde_json::from_str::<Issue>(&response_text).map_err(|e| MarkdownError::ParseError {
            message: format!("Failed to parse GitHub issue response: {e}"),
        })
    }

    /// Fetches all comments for an issue or pull request from GitHub API.
    pub async fn fetch_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u32,
    ) -> Result<Vec<Comment>, MarkdownError> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments",
            self.api_base_url, owner, repo, number
        );

        let response_text = self.make_api_request(&url).await?;

        serde_json::from_str::<Vec<Comment>>(&response_text).map_err(|e| {
            MarkdownError::ParseError {
                message: format!("Failed to parse GitHub comments response: {e}"),
            }
        })
    }

    /// Makes an authenticated API request to GitHub.
    async fn make_api_request(&self, url: &str) -> Result<String, MarkdownError> {
        // Create HTTP client with proper headers
        let mut headers = HashMap::new();
        headers.insert(
            "User-Agent".to_string(),
            format!("{USER_AGENT_PREFIX}/{}", env!("CARGO_PKG_VERSION")),
        );
        headers.insert("Accept".to_string(), GITHUB_API_VERSION.to_string());

        // Add authentication header if token is provided
        if let Some(ref token) = self.auth_token {
            headers.insert("Authorization".to_string(), format!("token {token}"));
        }

        // Make the request using the HttpClient with header support
        match self.client.get_text_with_headers(url, &headers).await {
            Ok(response) => Ok(response),
            Err(MarkdownError::AuthError { message }) => {
                Err(MarkdownError::AuthError {
                    message: format!("GitHub API authentication failed: {message}. Consider setting GITHUB_TOKEN environment variable.")
                })
            }
            Err(MarkdownError::NetworkError { message }) => {
                if message.contains("403") {
                    Err(MarkdownError::AuthError {
                        message: "GitHub API rate limit exceeded or access denied. Consider setting GITHUB_TOKEN environment variable.".to_string()
                    })
                } else if message.contains("404") {
                    Err(MarkdownError::NetworkError {
                        message: "GitHub issue/repository not found or not accessible.".to_string()
                    })
                } else {
                    Err(MarkdownError::NetworkError { message })
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Renders issue and comments as markdown.
    fn render_markdown(&self, issue: &Issue, comments: &[Comment]) -> String {
        let mut markdown = String::new();

        // Issue title as main heading
        markdown.push_str(&format!("# {}\n\n", issue.title));

        // Issue metadata
        markdown.push_str(&format!("**Author:** @{}  \n", issue.user.login));
        markdown.push_str(&format!(
            "**Created:** {}  \n",
            issue.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        markdown.push_str(&format!(
            "**State:** {}  \n",
            self.capitalize_first(&issue.state)
        ));

        // Labels
        if !issue.labels.is_empty() {
            let labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();
            markdown.push_str(&format!("**Labels:** {}  \n", labels.join(", ")));
        }

        markdown.push('\n');

        // Issue body
        if let Some(ref body) = issue.body {
            if !body.trim().is_empty() {
                markdown.push_str(body.trim());
                markdown.push_str("\n\n");
            }
        }

        // Comments section
        if !comments.is_empty() {
            markdown.push_str("## Comments\n\n");

            for comment in comments {
                markdown.push_str(&format!(
                    "### Comment by @{} ({})\n\n",
                    comment.user.login,
                    comment.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                ));

                if let Some(ref body) = comment.body {
                    if !body.trim().is_empty() {
                        markdown.push_str(body.trim());
                        markdown.push('\n');
                    }
                }

                // Note: Reaction fetching is not yet implemented via GitHub API
                // but the framework is in place via ReactionCounts struct
                markdown.push('\n');
            }
        }

        markdown.trim().to_string()
    }

    /// Builds frontmatter for the GitHub issue/PR.
    fn build_frontmatter(
        &self,
        resource: &GitHubResource,
        issue: &Issue,
    ) -> Result<String, MarkdownError> {
        let now = Utc::now();
        let mut builder = FrontmatterBuilder::new(resource.original_url.clone())
            .exporter(format!("markdowndown-github-{}", env!("CARGO_PKG_VERSION")))
            .download_date(now)
            .additional_field("title".to_string(), issue.title.clone())
            .additional_field("url".to_string(), resource.original_url.clone())
            .additional_field("converter".to_string(), "GitHubIssueConverter".to_string())
            .additional_field("converted_at".to_string(), now.to_rfc3339())
            .additional_field("conversion_type".to_string(), "github".to_string())
            .additional_field("github_issue_number".to_string(), issue.number.to_string())
            .additional_field(
                "github_repository".to_string(),
                format!("{}/{}", resource.owner, resource.repo),
            )
            .additional_field("github_state".to_string(), issue.state.clone())
            .additional_field("github_author".to_string(), issue.user.login.clone())
            .additional_field(
                "resource_type".to_string(),
                resource.resource_type.as_str().to_string(),
            );

        // Add labels if present
        if !issue.labels.is_empty() {
            let labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();
            builder = builder.additional_field("github_labels".to_string(), labels.join(", "));
        }

        builder.build()
    }

    /// Capitalizes the first letter of a string.
    fn capitalize_first(&self, s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}

impl Default for GitHubConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl super::Converter for GitHubConverter {
    async fn convert(&self, url: &str) -> Result<Markdown, MarkdownError> {
        self.convert(url).await
    }

    fn name(&self) -> &'static str {
        "GitHub Issue"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a test user with default values.
    fn create_test_user(login: &str, id: u64) -> User {
        User {
            login: login.to_string(),
            id,
        }
    }

    /// Creates a test issue with reasonable defaults.
    fn create_test_issue(
        number: u32,
        title: &str,
        body: Option<&str>,
        state: &str,
        user_login: &str,
        labels: Vec<Label>,
    ) -> Issue {
        Issue {
            id: 123456789, // Default test ID
            number,
            title: title.to_string(),
            body: body.map(|s| s.to_string()),
            state: state.to_string(),
            user: create_test_user(user_login, 1),
            created_at: DateTime::parse_from_rfc3339("2023-01-15T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339("2023-01-15T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            labels,
            pull_request: None,
        }
    }

    /// Creates a test comment with reasonable defaults.
    fn create_test_comment(
        id: u64,
        body: Option<&str>,
        user_login: &str,
        created_at: &str,
    ) -> Comment {
        Comment {
            id,
            body: body.map(|s| s.to_string()),
            user: create_test_user(user_login, id),
            created_at: DateTime::parse_from_rfc3339(created_at)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(created_at)
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    /// Creates a test label.
    fn create_test_label(name: &str, color: &str) -> Label {
        Label {
            name: name.to_string(),
            color: color.to_string(),
        }
    }

    #[test]
    fn test_resource_type_api_path() {
        assert_eq!(ResourceType::Issue.api_path(), "issues");
        assert_eq!(ResourceType::PullRequest.api_path(), "issues");
    }

    #[test]
    fn test_resource_type_as_str() {
        assert_eq!(ResourceType::Issue.as_str(), "issue");
        assert_eq!(ResourceType::PullRequest.as_str(), "pull_request");
    }

    #[test]
    fn test_github_converter_new() {
        let converter = GitHubConverter::new();
        assert!(converter.auth_token.is_none());
        assert_eq!(converter.api_base_url, DEFAULT_GITHUB_API_BASE_URL);
    }

    #[test]
    fn test_github_converter_with_token() {
        let token = "ghp_test_token".to_string();
        let converter = GitHubConverter::new_with_token(token.clone());
        assert_eq!(converter.auth_token, Some(token));
    }

    #[test]
    fn test_parse_github_issue_url() {
        let converter = GitHubConverter::new();
        let url = "https://github.com/microsoft/vscode/issues/12345";
        let result = converter.parse_github_url(url).unwrap();

        assert_eq!(result.owner, "microsoft");
        assert_eq!(result.repo, "vscode");
        assert_eq!(result.number, 12345);
        assert_eq!(result.resource_type, ResourceType::Issue);
        assert_eq!(result.original_url, url);
    }

    #[test]
    fn test_parse_github_pull_request_url() {
        let converter = GitHubConverter::new();
        let url = "https://github.com/rust-lang/rust/pull/98765";
        let result = converter.parse_github_url(url).unwrap();

        assert_eq!(result.owner, "rust-lang");
        assert_eq!(result.repo, "rust");
        assert_eq!(result.number, 98765);
        assert_eq!(result.resource_type, ResourceType::PullRequest);
        assert_eq!(result.original_url, url);
    }

    #[test]
    fn test_parse_invalid_github_urls() {
        let converter = GitHubConverter::new();

        let invalid_urls = [
            "https://example.com/user/repo/issues/123", // Wrong domain
            "https://github.com/user/repo",             // Missing issue/PR part
            "https://github.com/user/repo/issues",      // Missing number
            "https://github.com/user/repo/issues/abc",  // Non-numeric number
            "https://github.com/user/repo/commits/123", // Wrong resource type
            "not-a-url",                                // Invalid URL
        ];

        for url in &invalid_urls {
            let result = converter.parse_github_url(url);
            assert!(result.is_err(), "Should fail for URL: {url}");
        }
    }

    #[test]
    fn test_reaction_counts_empty() {
        let reactions = vec![];
        let counts = ReactionCounts::from_reactions(&reactions);
        assert!(counts.counts.is_empty());
        assert_eq!(counts.format(), "");
    }

    #[test]
    fn test_reaction_counts_with_reactions() {
        let user = create_test_user("test", 1);
        let reactions = vec![
            Reaction {
                content: "+1".to_string(),
                user: user.clone(),
            },
            Reaction {
                content: "+1".to_string(),
                user: user.clone(),
            },
            Reaction {
                content: "laugh".to_string(),
                user: user.clone(),
            },
        ];

        let counts = ReactionCounts::from_reactions(&reactions);
        assert_eq!(counts.counts.get("+1"), Some(&2));
        assert_eq!(counts.counts.get("laugh"), Some(&1));

        let formatted = counts.format();
        assert!(formatted.contains("👍 2"));
        assert!(formatted.contains("😄 1"));
    }

    #[test]
    fn test_capitalize_first() {
        let converter = GitHubConverter::new();
        assert_eq!(converter.capitalize_first("hello"), "Hello");
        assert_eq!(converter.capitalize_first("WORLD"), "WORLD");
        assert_eq!(converter.capitalize_first(""), "");
        assert_eq!(converter.capitalize_first("a"), "A");
    }

    #[test]
    fn test_render_markdown_basic() {
        let converter = GitHubConverter::new();

        let issue = create_test_issue(
            123,
            "Test Issue",
            Some("This is a test issue body."),
            "open",
            "testuser",
            vec![],
        );

        let comments = vec![];
        let markdown = converter.render_markdown(&issue, &comments);

        assert!(markdown.contains("# Test Issue"));
        assert!(markdown.contains("**Author:** @testuser"));
        assert!(markdown.contains("**State:** Open"));
        assert!(markdown.contains("This is a test issue body."));
    }

    #[test]
    fn test_render_markdown_with_comments() {
        let converter = GitHubConverter::new();

        let issue = create_test_issue(
            123,
            "Test Issue",
            Some("Issue body"),
            "closed",
            "author",
            vec![],
        );

        let comments = vec![
            create_test_comment(
                1,
                Some("First comment"),
                "commenter1",
                "2023-01-15T11:00:00Z",
            ),
            create_test_comment(
                2,
                Some("Second comment"),
                "commenter2",
                "2023-01-15T12:00:00Z",
            ),
        ];

        let markdown = converter.render_markdown(&issue, &comments);

        assert!(markdown.contains("## Comments"));
        assert!(markdown.contains("### Comment by @commenter1"));
        assert!(markdown.contains("First comment"));
        assert!(markdown.contains("### Comment by @commenter2"));
        assert!(markdown.contains("Second comment"));
    }

    #[test]
    fn test_render_markdown_with_labels() {
        let converter = GitHubConverter::new();

        let issue = create_test_issue(
            123,
            "Bug Report",
            Some("Found a bug"),
            "open",
            "reporter",
            vec![
                create_test_label("bug", "d73a49"),
                create_test_label("help wanted", "008672"),
            ],
        );

        let comments = vec![];
        let markdown = converter.render_markdown(&issue, &comments);

        assert!(markdown.contains("**Labels:** bug, help wanted"));
    }

    #[test]
    fn test_default_implementation() {
        let converter = GitHubConverter::default();
        assert!(converter.auth_token.is_none());
    }

    // Edge case tests
    #[test]
    fn test_parse_github_url_with_fragments() {
        let converter = GitHubConverter::new();
        let url = "https://github.com/owner/repo/issues/123#issuecomment-456";
        let result = converter.parse_github_url(url).unwrap();

        assert_eq!(result.owner, "owner");
        assert_eq!(result.repo, "repo");
        assert_eq!(result.number, 123);
        assert_eq!(result.resource_type, ResourceType::Issue);
    }

    #[test]
    fn test_parse_github_url_with_query_params() {
        let converter = GitHubConverter::new();
        let url = "https://github.com/owner/repo/pull/456?tab=files";
        let result = converter.parse_github_url(url).unwrap();

        assert_eq!(result.owner, "owner");
        assert_eq!(result.repo, "repo");
        assert_eq!(result.number, 456);
        assert_eq!(result.resource_type, ResourceType::PullRequest);
    }

    #[test]
    fn test_render_markdown_empty_body() {
        let converter = GitHubConverter::new();

        let issue = create_test_issue(123, "Empty Issue", None, "open", "user", vec![]);

        let comments = vec![];
        let markdown = converter.render_markdown(&issue, &comments);

        assert!(markdown.contains("# Empty Issue"));
        assert!(markdown.contains("**Author:** @user"));
        // Should not contain empty body content
        assert!(!markdown.contains("## Comments")); // No comments section if no comments
    }
}
