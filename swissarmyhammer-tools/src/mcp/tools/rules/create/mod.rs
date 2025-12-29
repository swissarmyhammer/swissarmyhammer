//! Rule creation MCP tool that creates project-specific rules from specifications.
//!
//! This tool provides an MCP interface to create rule files in `.swissarmyhammer/rules/`
//! with minimal YAML frontmatter containing only severity and optional tags.
//!
//! sah rule ignore test_rule_with_allow

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Request structure for rule creation operations via MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRuleRequest {
    /// Rule name with optional subdirectory path (e.g., "code-quality/no-global-state")
    pub name: String,

    /// Rule checking instructions in markdown
    pub content: String,

    /// Severity level: "error", "warning", "info", or "hint"
    pub severity: String,

    /// Optional tags for filtering and organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Minimal frontmatter structure for rule files
#[derive(Debug, Serialize)]
struct RuleFrontmatter {
    severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

/// Tool for creating project-specific rules
#[derive(Clone)]
pub struct CreateRuleTool;

impl CreateRuleTool {
    /// Creates a new instance of the CreateRuleTool
    pub fn new() -> Self {
        Self
    }

    /// Validate severity string
    fn validate_severity(severity: &str) -> Result<(), McpError> {
        match severity.to_lowercase().as_str() {
            "error" | "warning" | "info" | "hint" => Ok(()),
            _ => Err(McpError::invalid_params(
                format!(
                    "Invalid severity '{}'. Must be one of: error, warning, info, hint",
                    severity
                ),
                None,
            )),
        }
    }

    /// Get the rules directory path
    fn get_rules_directory() -> PathBuf {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        current_dir.join(".swissarmyhammer").join("rules")
    }

    /// Create the rule file with frontmatter and content
    fn create_rule_file(
        name: &str,
        content: &str,
        severity: &str,
        tags: Option<Vec<String>>,
    ) -> Result<PathBuf, McpError> {
        // Get the rules directory
        let rules_dir = Self::get_rules_directory();

        // Construct the full file path
        let file_path = rules_dir.join(format!("{}.md", name));

        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                McpError::internal_error(
                    format!("Failed to create directory '{}': {}", parent.display(), e),
                    None,
                )
            })?;
        }

        // Create frontmatter
        let frontmatter = RuleFrontmatter {
            severity: severity.to_lowercase(),
            tags,
        };

        // Serialize frontmatter to YAML
        let frontmatter_yaml = serde_yaml::to_string(&frontmatter).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize frontmatter: {}", e), None)
        })?;

        // Construct the full file content
        let file_content = format!("---\n{}---\n\n{}", frontmatter_yaml, content);

        // Write the file
        fs::write(&file_path, file_content).map_err(|e| {
            McpError::internal_error(
                format!("Failed to write rule file '{}': {}", file_path.display(), e),
                None,
            )
        })?;

        tracing::info!("Created rule file: {}", file_path.display());

        Ok(file_path)
    }
}

impl Default for CreateRuleTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTool for CreateRuleTool {
    fn name(&self) -> &'static str {
        "rules_create"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("rules", "create")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Rule name with optional subdirectory path (e.g., \"code-quality/no-global-state\")"
                },
                "content": {
                    "type": "string",
                    "description": "Rule checking instructions in markdown"
                },
                "severity": {
                    "type": "string",
                    "enum": ["error", "warning", "info", "hint"],
                    "description": "Severity level: \"error\", \"warning\", \"info\", or \"hint\""
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional tags for filtering and organization"
                }
            },
            "required": ["name", "content", "severity"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: CreateRuleRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::info!("Creating rule: {}", request.name);

        // Validate parameters
        if request.name.is_empty() {
            return Err(McpError::invalid_params("Rule name cannot be empty", None));
        }

        if request.content.is_empty() {
            return Err(McpError::invalid_params(
                "Rule content cannot be empty",
                None,
            ));
        }

        Self::validate_severity(&request.severity)?;

        // Create the rule file
        let file_path = Self::create_rule_file(
            &request.name,
            &request.content,
            &request.severity,
            request.tags.clone(),
        )?;

        tracing::info!(
            "Created rule '{}' at {} with severity {}",
            request.name,
            file_path.display(),
            request.severity
        );

        Ok(BaseToolImpl::create_success_response("OK"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_rule_tool_name() {
        let tool = CreateRuleTool::new();
        assert_eq!(tool.name(), "rules_create");
    }

    #[test]
    fn test_create_rule_tool_schema() {
        let tool = CreateRuleTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["name"].is_object());
        assert!(schema["properties"]["content"].is_object());
        assert!(schema["properties"]["severity"].is_object());
        assert!(schema["properties"]["tags"].is_object());
        assert_eq!(
            schema["required"],
            serde_json::json!(["name", "content", "severity"])
        );
    }

    #[test]
    fn test_validate_severity_valid() {
        assert!(CreateRuleTool::validate_severity("error").is_ok());
        assert!(CreateRuleTool::validate_severity("warning").is_ok());
        assert!(CreateRuleTool::validate_severity("info").is_ok());
        assert!(CreateRuleTool::validate_severity("hint").is_ok());
        assert!(CreateRuleTool::validate_severity("ERROR").is_ok());
        assert!(CreateRuleTool::validate_severity("Warning").is_ok());
    }

    #[test]
    fn test_validate_severity_invalid() {
        assert!(CreateRuleTool::validate_severity("invalid").is_err());
        assert!(CreateRuleTool::validate_severity("critical").is_err());
        assert!(CreateRuleTool::validate_severity("").is_err());
    }

    #[test]
    fn test_create_rule_file_basic() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result =
            CreateRuleTool::create_rule_file("test-rule", "Check for test issues", "error", None);

        assert!(result.is_ok());
        let file_path = result.unwrap();
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("---"));
        assert!(content.contains("severity: error"));
        assert!(content.contains("Check for test issues"));
        assert!(!content.contains("tags:"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_create_rule_file_with_tags() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tags = Some(vec!["maintainability".to_string(), "testing".to_string()]);
        let result =
            CreateRuleTool::create_rule_file("test-rule-tags", "Check something", "warning", tags);

        assert!(result.is_ok());
        let file_path = result.unwrap();
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("severity: warning"));
        assert!(content.contains("tags:"));
        assert!(content.contains("maintainability"));
        assert!(content.contains("testing"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_create_rule_file_with_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = CreateRuleTool::create_rule_file(
            "code-quality/cognitive-complexity",
            "Check for complex functions",
            "info",
            None,
        );

        assert!(result.is_ok());
        let file_path = result.unwrap();
        assert!(file_path.exists());
        assert!(file_path
            .to_string_lossy()
            .contains("code-quality/cognitive-complexity.md"));

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("severity: info"));
        assert!(content.contains("Check for complex functions"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_create_rule_file_nested_subdirectories() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = CreateRuleTool::create_rule_file(
            "category/subcategory/my-rule",
            "Nested rule content",
            "hint",
            None,
        );

        assert!(result.is_ok());
        let file_path = result.unwrap();
        assert!(file_path.exists());
        assert!(file_path
            .to_string_lossy()
            .contains("category/subcategory/my-rule.md"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_execute_basic_rule() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tool = CreateRuleTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert("name".to_string(), serde_json::json!("basic-rule"));
        arguments.insert(
            "content".to_string(),
            serde_json::json!("Check basic things"),
        );
        arguments.insert("severity".to_string(), serde_json::json!("error"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let file_path = CreateRuleTool::get_rules_directory().join("basic-rule.md");
        assert!(file_path.exists());

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_execute_with_tags() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tool = CreateRuleTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert("name".to_string(), serde_json::json!("tagged-rule"));
        arguments.insert("content".to_string(), serde_json::json!("Rule content"));
        arguments.insert("severity".to_string(), serde_json::json!("warning"));
        arguments.insert(
            "tags".to_string(),
            serde_json::json!(["security", "best-practices"]),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let file_path = CreateRuleTool::get_rules_directory().join("tagged-rule.md");
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("security"));
        assert!(content.contains("best-practices"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_execute_with_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tool = CreateRuleTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "name".to_string(),
            serde_json::json!("security/no-hardcoded-secrets"),
        );
        arguments.insert(
            "content".to_string(),
            serde_json::json!("Check for hardcoded API keys"),
        );
        arguments.insert("severity".to_string(), serde_json::json!("error"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let file_path = CreateRuleTool::get_rules_directory()
            .join("security")
            .join("no-hardcoded-secrets.md");
        assert!(file_path.exists());

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_execute_empty_name() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tool = CreateRuleTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert("name".to_string(), serde_json::json!(""));
        arguments.insert("content".to_string(), serde_json::json!("Content"));
        arguments.insert("severity".to_string(), serde_json::json!("error"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("name cannot be empty"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_execute_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tool = CreateRuleTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert("name".to_string(), serde_json::json!("test-rule"));
        arguments.insert("content".to_string(), serde_json::json!(""));
        arguments.insert("severity".to_string(), serde_json::json!("error"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("content cannot be empty"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_execute_invalid_severity() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let tool = CreateRuleTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert("name".to_string(), serde_json::json!("test-rule"));
        arguments.insert("content".to_string(), serde_json::json!("Content"));
        arguments.insert("severity".to_string(), serde_json::json!("critical"));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid severity"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_execute_all_severity_levels() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let severities = vec!["error", "warning", "info", "hint"];

        for severity in severities {
            let tool = CreateRuleTool::new();
            let context = crate::test_utils::create_test_context().await;

            let mut arguments = serde_json::Map::new();
            arguments.insert(
                "name".to_string(),
                serde_json::json!(format!("rule-{}", severity)),
            );
            arguments.insert("content".to_string(), serde_json::json!("Test content"));
            arguments.insert("severity".to_string(), serde_json::json!(severity));

            let result = tool.execute(arguments, &context).await;
            assert!(
                result.is_ok(),
                "Should create rule with severity: {}",
                severity
            );

            let file_path =
                CreateRuleTool::get_rules_directory().join(format!("rule-{}.md", severity));
            assert!(file_path.exists());

            let content = fs::read_to_string(&file_path).unwrap();
            assert!(content.contains(&format!("severity: {}", severity)));
        }

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_create_rule_request_parsing() {
        let json = serde_json::json!({
            "name": "test-rule",
            "content": "Test content",
            "severity": "error",
            "tags": ["tag1", "tag2"]
        });

        let request: CreateRuleRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "test-rule");
        assert_eq!(request.content, "Test content");
        assert_eq!(request.severity, "error");
        assert_eq!(
            request.tags,
            Some(vec!["tag1".to_string(), "tag2".to_string()])
        );
    }

    #[tokio::test]
    async fn test_create_rule_request_parsing_without_tags() {
        let json = serde_json::json!({
            "name": "test-rule",
            "content": "Test content",
            "severity": "warning"
        });

        let request: CreateRuleRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "test-rule");
        assert_eq!(request.content, "Test content");
        assert_eq!(request.severity, "warning");
        assert!(request.tags.is_none());
    }
}
