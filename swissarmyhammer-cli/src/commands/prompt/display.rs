//! Display objects for prompt command output
//!
//! Provides clean display objects with `Tabled` and `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use serde::Serialize;
use tabled::Tabled;

/// Basic prompt information for standard list output
#[derive(Tabled, Serialize, Debug, Clone)]
pub struct PromptRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,
}

/// Detailed prompt information for verbose list output  
#[derive(Tabled, Serialize, Debug, Clone)]
pub struct VerbosePromptRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Description")]
    pub description: String,

    #[tabled(rename = "Source")]
    pub source: String,

    #[tabled(rename = "Category")]
    pub category: String,
}

impl From<&swissarmyhammer_prompts::Prompt> for PromptRow {
    fn from(prompt: &swissarmyhammer_prompts::Prompt) -> Self {
        Self {
            name: prompt.name.clone(),
            title: prompt
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("No title")
                .to_string(),
        }
    }
}

impl From<&swissarmyhammer_prompts::Prompt> for VerbosePromptRow {
    fn from(prompt: &swissarmyhammer_prompts::Prompt) -> Self {
        Self {
            name: prompt.name.clone(),
            title: prompt
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("No title")
                .to_string(),
            description: prompt
                .description
                .as_deref()
                .unwrap_or("No description")
                .to_string(),
            source: prompt
                .source
                .as_ref()
                .map(|s| s.display().to_string())
                .unwrap_or("Unknown".to_string()),
            category: prompt.category.clone().unwrap_or_default(),
        }
    }
}

/// Convert prompts to appropriate display format based on verbose flag
pub fn prompts_to_display_rows(
    prompts: Vec<swissarmyhammer_prompts::Prompt>,
    verbose: bool,
) -> DisplayRows {
    if verbose {
        DisplayRows::Verbose(prompts.iter().map(VerbosePromptRow::from).collect())
    } else {
        DisplayRows::Standard(prompts.iter().map(PromptRow::from).collect())
    }
}

/// Enum to handle different display row types
#[derive(Debug)]
pub enum DisplayRows {
    Standard(Vec<PromptRow>),
    Verbose(Vec<VerbosePromptRow>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swissarmyhammer_prompts::Prompt;

    fn create_test_prompt() -> Prompt {
        let mut metadata = HashMap::new();
        metadata.insert(
            "title".to_string(),
            serde_json::Value::String("Test Title".to_string()),
        );

        Prompt {
            name: "test-prompt".to_string(),
            description: Some("Test description".to_string()),
            category: Some("testing".to_string()),
            tags: vec!["test".to_string()],
            template: "Test template content".to_string(),
            parameters: vec![],
            source: Some(std::path::PathBuf::from("/test/path/test-prompt.md")),
            metadata,
        }
    }

    #[test]
    fn test_prompt_row_conversion() {
        let prompt = create_test_prompt();
        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "Test Title");
    }

    #[test]
    fn test_verbose_prompt_row_conversion() {
        let prompt = create_test_prompt();
        let row = VerbosePromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "Test Title");
        assert_eq!(row.description, "Test description");
        assert_eq!(row.source, "/test/path/test-prompt.md");
        assert_eq!(row.category, "testing".to_string());
    }

    #[test]
    fn test_prompt_row_with_missing_metadata() {
        let prompt = Prompt {
            name: "test-prompt".to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: "Test template".to_string(),
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        };

        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "No title");
    }

    #[test]
    fn test_verbose_prompt_row_with_missing_metadata() {
        let prompt = Prompt {
            name: "test-prompt".to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: "Test template".to_string(),
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        };

        let row = VerbosePromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "No title");
        assert_eq!(row.description, "No description");
        assert_eq!(row.source, "Unknown");
        assert_eq!(row.category, "");
    }

    #[test]
    fn test_prompts_to_display_rows_standard() {
        let prompts = vec![create_test_prompt()];
        let rows = prompts_to_display_rows(prompts, false);

        match rows {
            DisplayRows::Standard(standard_rows) => {
                assert_eq!(standard_rows.len(), 1);
                assert_eq!(standard_rows[0].name, "test-prompt");
            }
            DisplayRows::Verbose(_) => panic!("Expected Standard rows"),
        }
    }

    #[test]
    fn test_prompts_to_display_rows_verbose() {
        let prompts = vec![create_test_prompt()];
        let rows = prompts_to_display_rows(prompts, true);

        match rows {
            DisplayRows::Verbose(verbose_rows) => {
                assert_eq!(verbose_rows.len(), 1);
                assert_eq!(verbose_rows[0].name, "test-prompt");
                assert_eq!(verbose_rows[0].description, "Test description");
            }
            DisplayRows::Standard(_) => panic!("Expected Verbose rows"),
        }
    }
}
