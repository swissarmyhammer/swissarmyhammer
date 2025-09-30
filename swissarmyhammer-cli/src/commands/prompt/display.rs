//! Display objects for prompt command output
//!
//! Provides clean display objects with `Tabled` and `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use serde::{Deserialize, Serialize};
use tabled::Tabled;

// Emoji constants for source display consistency across all listing commands
const BUILTIN_EMOJI: &str = "📦 Built-in";
const PROJECT_EMOJI: &str = "📁 Project";
const USER_EMOJI: &str = "👤 User";

/// Convert FileSource to emoji representation for consistent display across all listing commands.
/// This ensures all three table displays (prompt, flow, agent) use the same emoji mapping:
/// - 📦 Built-in: System-provided built-in items
/// - 📁 Project: Project-specific items from .swissarmyhammer directory  
/// - 👤 User: User-specific items from user's home directory
fn file_source_to_emoji(source: Option<&swissarmyhammer::FileSource>) -> &'static str {
    match source {
        Some(swissarmyhammer::FileSource::Builtin) => BUILTIN_EMOJI,
        Some(swissarmyhammer::FileSource::Local) => PROJECT_EMOJI,
        Some(swissarmyhammer::FileSource::User) => USER_EMOJI,
        Some(swissarmyhammer::FileSource::Dynamic) | None => BUILTIN_EMOJI, // Default fallback
    }
}

/// Basic prompt information for standard list output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct PromptRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Source")]
    pub source: String,
}

/// Detailed prompt information for verbose list output  
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
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
            source: prompt
                .source
                .as_ref()
                .map(|s| s.display().to_string())
                .unwrap_or("Unknown".to_string()),
        }
    }
}

impl PromptRow {
    /// Create a PromptRow with FileSource information for emoji-based source display
    pub fn from_prompt_with_source(
        prompt: &swissarmyhammer_prompts::Prompt,
        file_source: Option<&swissarmyhammer::FileSource>,
    ) -> Self {
        Self {
            name: prompt.name.clone(),
            title: prompt
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("No title")
                .to_string(),
            source: file_source_to_emoji(file_source).to_string(),
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

impl VerbosePromptRow {
    /// Create a VerbosePromptRow with FileSource information for emoji-based source display
    pub fn from_prompt_with_source(
        prompt: &swissarmyhammer_prompts::Prompt,
        file_source: Option<&swissarmyhammer::FileSource>,
    ) -> Self {
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
            source: file_source_to_emoji(file_source).to_string(),
            category: prompt.category.clone().unwrap_or_default(),
        }
    }
}

/// Convert prompts with source information to appropriate display format with emoji-based sources
pub fn prompts_to_display_rows_with_sources(
    prompts: Vec<swissarmyhammer_prompts::Prompt>,
    sources: &std::collections::HashMap<String, swissarmyhammer::FileSource>,
    verbose: bool,
) -> DisplayRows {
    if verbose {
        DisplayRows::Verbose(
            prompts
                .iter()
                .map(|prompt| {
                    let file_source = sources.get(&prompt.name);
                    VerbosePromptRow::from_prompt_with_source(prompt, file_source)
                })
                .collect(),
        )
    } else {
        DisplayRows::Standard(
            prompts
                .iter()
                .map(|prompt| {
                    let file_source = sources.get(&prompt.name);
                    PromptRow::from_prompt_with_source(prompt, file_source)
                })
                .collect(),
        )
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

    fn create_prompt_with_all_metadata() -> Prompt {
        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), serde_json::json!("Complete Title"));
        metadata.insert("author".to_string(), serde_json::json!("Test Author"));
        metadata.insert("version".to_string(), serde_json::json!("1.0.0"));

        Prompt {
            name: "complete-prompt".to_string(),
            description: Some("Full description with all fields".to_string()),
            category: Some("comprehensive".to_string()),
            tags: vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
            template: "Complete template content with {{ parameters }}".to_string(),
            parameters: vec![],
            source: Some(std::path::PathBuf::from(
                "/complete/path/complete-prompt.md",
            )),
            metadata,
        }
    }

    fn create_empty_prompt() -> Prompt {
        Prompt {
            name: "empty-prompt".to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: String::new(),
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_prompt_row_conversion() {
        let prompt = create_test_prompt();
        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "Test Title");
        assert_eq!(row.source, "/test/path/test-prompt.md");
    }

    #[test]
    fn test_prompt_row_from_prompt_with_all_metadata() {
        let prompt = create_prompt_with_all_metadata();
        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "complete-prompt");
        assert_eq!(row.title, "Complete Title");
        assert_eq!(row.source, "/complete/path/complete-prompt.md");
    }

    #[test]
    fn test_prompt_row_from_empty_prompt() {
        let prompt = create_empty_prompt();
        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "empty-prompt");
        assert_eq!(row.title, "No title");
        assert_eq!(row.source, "Unknown");
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
    fn test_verbose_prompt_row_from_complete_prompt() {
        let prompt = create_prompt_with_all_metadata();
        let row = VerbosePromptRow::from(&prompt);
        assert_eq!(row.name, "complete-prompt");
        assert_eq!(row.title, "Complete Title");
        assert_eq!(row.description, "Full description with all fields");
        assert_eq!(row.source, "/complete/path/complete-prompt.md");
        assert_eq!(row.category, "comprehensive");
    }

    #[test]
    fn test_verbose_prompt_row_from_empty_prompt() {
        let prompt = create_empty_prompt();
        let row = VerbosePromptRow::from(&prompt);
        assert_eq!(row.name, "empty-prompt");
        assert_eq!(row.title, "No title");
        assert_eq!(row.description, "No description");
        assert_eq!(row.source, "Unknown");
        assert_eq!(row.category, "");
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
        assert_eq!(row.source, "Unknown");
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
        let sources = std::collections::HashMap::new();
        let rows = prompts_to_display_rows_with_sources(prompts, &sources, false);

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
        let sources = std::collections::HashMap::new();
        let rows = prompts_to_display_rows_with_sources(prompts, &sources, true);

        match rows {
            DisplayRows::Verbose(verbose_rows) => {
                assert_eq!(verbose_rows.len(), 1);
                assert_eq!(verbose_rows[0].name, "test-prompt");
                assert_eq!(verbose_rows[0].description, "Test description");
            }
            DisplayRows::Standard(_) => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_prompts_to_display_rows_multiple_prompts() {
        let prompts = vec![
            create_test_prompt(),
            create_prompt_with_all_metadata(),
            create_empty_prompt(),
        ];

        let sources = std::collections::HashMap::new();
        let standard_rows = prompts_to_display_rows_with_sources(prompts.clone(), &sources, false);
        match standard_rows {
            DisplayRows::Standard(rows) => {
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[0].name, "test-prompt");
                assert_eq!(rows[1].name, "complete-prompt");
                assert_eq!(rows[2].name, "empty-prompt");
            }
            _ => panic!("Expected Standard rows"),
        }

        let verbose_rows = prompts_to_display_rows_with_sources(prompts, &sources, true);
        match verbose_rows {
            DisplayRows::Verbose(rows) => {
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[0].name, "test-prompt");
                assert_eq!(rows[1].name, "complete-prompt");
                assert_eq!(rows[2].name, "empty-prompt");
            }
            _ => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_prompts_to_display_rows_empty_list() {
        let prompts = vec![];
        let sources = std::collections::HashMap::new();

        let standard_rows = prompts_to_display_rows_with_sources(prompts.clone(), &sources, false);
        match standard_rows {
            DisplayRows::Standard(rows) => assert!(rows.is_empty()),
            _ => panic!("Expected Standard rows"),
        }

        let verbose_rows = prompts_to_display_rows_with_sources(prompts, &sources, true);
        match verbose_rows {
            DisplayRows::Verbose(rows) => assert!(rows.is_empty()),
            _ => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_serialization_prompt_row() {
        let row = PromptRow {
            name: "test".to_string(),
            title: "Test Title".to_string(),
            source: "Test Source".to_string(),
        };

        let json = serde_json::to_string(&row).expect("Should serialize to JSON");
        assert!(json.contains("test"));
        assert!(json.contains("Test Title"));
        assert!(json.contains("Test Source"));

        let deserialized: PromptRow =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.title, "Test Title");
        assert_eq!(deserialized.source, "Test Source");
    }

    #[test]
    fn test_serialization_verbose_prompt_row() {
        let row = VerbosePromptRow {
            name: "test".to_string(),
            title: "Test Title".to_string(),
            description: "Test Description".to_string(),
            source: "Test Source".to_string(),
            category: "Test Category".to_string(),
        };

        let json = serde_json::to_string(&row).expect("Should serialize to JSON");
        assert!(json.contains("test"));
        assert!(json.contains("Test Title"));
        assert!(json.contains("Test Description"));

        let deserialized: VerbosePromptRow =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.title, "Test Title");
        assert_eq!(deserialized.description, "Test Description");
        assert_eq!(deserialized.source, "Test Source");
        assert_eq!(deserialized.category, "Test Category");
    }

    #[test]
    fn test_metadata_edge_cases() {
        let mut metadata = HashMap::new();

        // Test with non-string JSON values
        metadata.insert("title".to_string(), serde_json::json!(123));
        metadata.insert("numeric_title".to_string(), serde_json::json!(true));
        metadata.insert("null_title".to_string(), serde_json::json!(null));

        let prompt = Prompt {
            name: "edge-case-prompt".to_string(),
            description: Some("Edge case description".to_string()),
            category: Some("edge".to_string()),
            tags: vec![],
            template: "Edge case template".to_string(),
            parameters: vec![],
            source: Some(std::path::PathBuf::from("/edge/path.md")),
            metadata,
        };

        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "edge-case-prompt");
        assert_eq!(row.title, "No title"); // Should fallback when value is not a string
        assert_eq!(row.source, "/edge/path.md");

        let verbose_row = VerbosePromptRow::from(&prompt);
        assert_eq!(verbose_row.name, "edge-case-prompt");
        assert_eq!(verbose_row.title, "No title");
    }

    #[test]
    fn test_display_rows_debug_format() {
        let prompts = vec![create_test_prompt()];
        let sources = std::collections::HashMap::new();
        let rows = prompts_to_display_rows_with_sources(prompts, &sources, false);

        let debug_str = format!("{:?}", rows);
        assert!(debug_str.contains("Standard"));
        assert!(debug_str.contains("test-prompt"));
    }

    #[test]
    fn test_prompt_row_with_source_emoji_mapping() {
        let prompt = create_test_prompt();

        // Test built-in source
        let builtin_row = PromptRow::from_prompt_with_source(
            &prompt,
            Some(&swissarmyhammer::FileSource::Builtin),
        );
        assert_eq!(builtin_row.source, "📦 Built-in");

        // Test project source
        let project_row =
            PromptRow::from_prompt_with_source(&prompt, Some(&swissarmyhammer::FileSource::Local));
        assert_eq!(project_row.source, "📁 Project");

        // Test user source
        let user_row =
            PromptRow::from_prompt_with_source(&prompt, Some(&swissarmyhammer::FileSource::User));
        assert_eq!(user_row.source, "👤 User");

        // Test dynamic source (should default to built-in)
        let dynamic_row = PromptRow::from_prompt_with_source(
            &prompt,
            Some(&swissarmyhammer::FileSource::Dynamic),
        );
        assert_eq!(dynamic_row.source, "📦 Built-in");

        // Test no source (should default to built-in)
        let no_source_row = PromptRow::from_prompt_with_source(&prompt, None);
        assert_eq!(no_source_row.source, "📦 Built-in");
    }

    #[test]
    fn test_prompts_to_display_rows_with_sources_emoji_mapping() {
        let prompts = vec![create_test_prompt()];
        let mut sources = std::collections::HashMap::new();
        sources.insert(
            "test-prompt".to_string(),
            swissarmyhammer::FileSource::Local,
        );

        // Test standard mode uses emoji mapping
        let standard_rows = prompts_to_display_rows_with_sources(prompts.clone(), &sources, false);
        match standard_rows {
            DisplayRows::Standard(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "test-prompt");
                assert_eq!(rows[0].source, "📁 Project");
            }
            _ => panic!("Expected Standard rows"),
        }

        // Test verbose mode uses emoji mapping
        let verbose_rows = prompts_to_display_rows_with_sources(prompts, &sources, true);
        match verbose_rows {
            DisplayRows::Verbose(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "test-prompt");
                assert_eq!(rows[0].source, "📁 Project");
            }
            _ => panic!("Expected Verbose rows"),
        }
    }
}
