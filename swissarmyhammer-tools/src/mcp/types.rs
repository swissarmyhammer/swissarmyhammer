//! Request and response types for MCP operations, along with constants

// IssueName is used via full path and re-exported below
use serde::Deserialize;
use std::collections::HashMap;

/// Request structure for getting a prompt
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPromptRequest {
    /// Name of the prompt to retrieve
    pub name: String,
    /// Optional arguments for template rendering
    #[serde(default)]
    pub arguments: HashMap<String, String>,
}

/// Request structure for listing prompts
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListPromptsRequest {
    /// Optional filter by category
    pub category: Option<String>,
}

/// Request to create a new issue
///
/// # Examples
///
/// Create a named issue (will create file like `000123_feature_name.md`):
/// ```ignore
/// CreateIssueRequest {
///     name: Some(swissarmyhammer::issues::IssueName("feature_name".to_string())),
///     content: "# Implement new feature\n\nDetails...".to_string(),
/// }
/// ```
///
/// Create a nameless issue (will create file like `000123.md`):
/// ```ignore
/// CreateIssueRequest {
///     name: None,
///     content: "# Quick fix needed\n\nDetails...".to_string(),
/// }
/// ```
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateIssueRequest {
    /// Name of the issue (will be used in filename) - optional
    /// When `Some(name)`, creates files like `000123_name.md`
    /// When `None`, creates files like `000123.md`
    pub name: Option<swissarmyhammer::issues::IssueName>,
    /// Markdown content of the issue
    pub content: String,
}

/// Request to mark an issue as complete
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MarkCompleteRequest {
    /// Issue name to mark as complete
    pub name: swissarmyhammer::issues::IssueName,
}

/// Request to check if all issues are complete
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AllCompleteRequest {
    // No parameters needed
}

/// Request to update an issue
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateIssueRequest {
    /// Issue name to update
    pub name: swissarmyhammer::issues::IssueName,
    /// New markdown content for the issue
    pub content: String,
    /// If true, append to existing content instead of replacing
    #[serde(default)]
    pub append: bool,
}

/// Request to work on an issue
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorkIssueRequest {
    /// Issue name to work on
    pub name: swissarmyhammer::issues::IssueName,
}

/// Request to merge an issue
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MergeIssueRequest {
    /// Issue name to merge
    pub name: swissarmyhammer::issues::IssueName,
    /// Whether to delete the branch after merging (default: false)
    #[serde(default)]
    pub delete_branch: bool,
}

// Re-export IssueName for convenience
pub use swissarmyhammer::issues::IssueName;

/// Request to create a new todo item
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateTodoRequest {
    /// Name of the todo list file
    pub todo_list: String,
    /// Brief description of the task
    pub task: String,
    /// Optional additional context or implementation notes
    pub context: Option<String>,
}

/// Request to show a todo item
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ShowTodoRequest {
    /// Name of the todo list file
    pub todo_list: String,
    /// Either a specific ULID or "next" to show the next incomplete item
    pub item: String,
}

/// Request to mark a todo item as complete
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MarkCompleteTodoRequest {
    /// Name of the todo list file
    pub todo_list: String,
    /// ULID of the todo item to mark as complete
    pub id: String,
}

/// Request to fetch web content
///
/// # Examples
///
/// Basic web fetch:
/// ```ignore
/// WebFetchRequest {
///     url: "https://docs.rust-lang.org/book/ch04-01-what-is-ownership.html".to_string(),
///     timeout: None,
///     follow_redirects: None,
///     max_content_length: None,
///     user_agent: None,
/// }
/// ```
///
/// Advanced web fetch with custom settings:
/// ```ignore
/// WebFetchRequest {
///     url: "https://api.github.com/docs/rest/repos".to_string(),
///     timeout: Some(45),
///     follow_redirects: Some(true),
///     max_content_length: Some(2097152),
///     user_agent: Some("SwissArmyHammer-DocProcessor/1.0".to_string()),
/// }
/// ```
#[derive(Debug, schemars::JsonSchema)]
pub struct WebFetchRequest {
    /// The URL to fetch content from (must be a valid HTTP/HTTPS URL)
    pub url: String,
    /// Request timeout in seconds (optional, defaults to 30 seconds)
    /// Minimum: 5, Maximum: 120
    pub timeout: Option<u32>,
    /// Whether to follow HTTP redirects (optional, defaults to true)
    pub follow_redirects: Option<bool>,
    /// Maximum content length in bytes (optional, defaults to 1MB)
    /// Minimum: 1024, Maximum: 10485760 (10MB)
    pub max_content_length: Option<u32>,
    /// Custom User-Agent header (optional, defaults to "SwissArmyHammer-Bot/1.0")
    pub user_agent: Option<String>,
}

impl<'de> Deserialize<'de> for WebFetchRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        
        #[derive(Deserialize)]
        struct WebFetchRequestHelper {
            url: String,
            timeout: Option<u32>,
            follow_redirects: Option<bool>,
            max_content_length: Option<u32>,
            user_agent: Option<String>,
        }
        
        let helper = WebFetchRequestHelper::deserialize(deserializer)?;
        
        // Validate timeout range
        const MIN_TIMEOUT_SECONDS: u32 = 5;
        const MAX_TIMEOUT_SECONDS: u32 = 120;
        
        let timeout = helper.timeout.map(|timeout| {
            if timeout < MIN_TIMEOUT_SECONDS || timeout > MAX_TIMEOUT_SECONDS {
                return Err(Error::custom(format!(
                    "Timeout must be between {MIN_TIMEOUT_SECONDS} and {MAX_TIMEOUT_SECONDS} seconds"
                )));
            }
            Ok(timeout)
        }).transpose()?;
        
        // Validate and clamp max_content_length
        const MIN_CONTENT_LENGTH_BYTES: u32 = 1024;
        const MAX_CONTENT_LENGTH_BYTES: u32 = 10_485_760;
        
        let max_content_length = helper.max_content_length.map(|length| {
            if length < MIN_CONTENT_LENGTH_BYTES || length > MAX_CONTENT_LENGTH_BYTES {
                return Err(Error::custom(format!(
                    "Maximum content length must be between {MIN_CONTENT_LENGTH_BYTES} and {MAX_CONTENT_LENGTH_BYTES} bytes"
                )));
            }
            Ok(length)
        }).transpose()?;
        
        // Keep user_agent as-is for now, validation will handle empty strings
        let user_agent = helper.user_agent;
        
        Ok(WebFetchRequest {
            url: helper.url,
            timeout,
            follow_redirects: helper.follow_redirects,
            max_content_length,
            user_agent,
        })
    }
}
