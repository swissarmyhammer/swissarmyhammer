//! CLI Context
//!
//! Shared context object that holds all storage instances and configuration
//! to avoid recreating them in each command.

use std::{rc::Rc, sync::Arc};
use swissarmyhammer_common::Result;
use swissarmyhammer_git::GitOperations;

use crate::cli::OutputFormat;
use swissarmyhammer_prompts::PromptLibrary;

/// Helper function to create error mapping closures with context
fn map_error<E: std::fmt::Display>(
    context: &str,
) -> impl FnOnce(E) -> swissarmyhammer_common::SwissArmyHammerError + '_ {
    move |e| swissarmyhammer_common::SwissArmyHammerError::Other {
        message: format!("{}: {}", context, e),
    }
}

/// Convert a JSON value to a string representation
fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

/// Get the sort order priority for a key name
fn get_key_priority(key: &str) -> u8 {
    match key {
        "name" => 0,
        "description" | "title" => 1,
        "source" => 2,
        "status" => 3,
        _ => 99,
    }
}

/// Capitalize the first letter of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Build a table from an array of JSON objects
fn build_table_from_items(array: &[serde_json::Value]) -> Result<comfy_table::Table> {
    let first =
        array
            .first()
            .ok_or_else(|| swissarmyhammer_common::SwissArmyHammerError::Other {
                message: "Array is empty".to_string(),
            })?;

    let obj =
        first
            .as_object()
            .ok_or_else(|| swissarmyhammer_common::SwissArmyHammerError::Other {
                message: "Expected object".to_string(),
            })?;

    let mut table = swissarmyhammer_doctor::new_table();

    let keys = get_sorted_keys(obj);
    let headers = create_table_headers(&keys);
    table.set_header(headers);

    add_table_rows(&mut table, array, &keys);

    Ok(table)
}

/// Get keys from an object sorted by priority
fn get_sorted_keys(obj: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut keys: Vec<_> = obj.keys().cloned().collect();
    keys.sort_by_key(|k| get_key_priority(k));
    keys
}

/// Create table headers from keys
fn create_table_headers(keys: &[String]) -> Vec<String> {
    keys.iter().map(|k| capitalize_first(k)).collect()
}

/// Add data rows to the table
fn add_table_rows(table: &mut comfy_table::Table, array: &[serde_json::Value], keys: &[String]) {
    for item in array {
        if let Some(obj) = item.as_object() {
            let row = create_table_row(obj, keys);
            table.add_row(row);
        }
    }
}

/// Create a single table row from an object
fn create_table_row(
    obj: &serde_json::Map<String, serde_json::Value>,
    keys: &[String],
) -> Vec<String> {
    keys.iter()
        .map(|k| obj.get(k).map(value_to_string).unwrap_or_default())
        .collect()
}

/// Shared CLI context containing all storage objects, configuration, and parsed arguments
#[derive(derive_builder::Builder)]
#[builder(pattern = "owned")]
pub struct CliContext {
    /// Prompt library for managing prompts
    #[allow(dead_code)]
    pub prompt_library: Arc<PromptLibrary>,

    /// Git operations (optional - None if not in a git repository)
    #[allow(dead_code)]
    pub git_operations: Option<Rc<GitOperations>>,

    /// Template context with configuration
    #[builder(setter(into))]
    pub template_context: swissarmyhammer_config::TemplateContext,

    /// Global output format setting
    #[builder(default = "OutputFormat::Table")]
    pub format: OutputFormat,

    /// Original global output format option (None if not explicitly specified)
    #[builder(default)]
    pub format_option: Option<OutputFormat>,

    /// Enable verbose output
    #[builder(default)]
    pub verbose: bool,

    /// Enable debug output
    #[builder(default)]
    pub debug: bool,

    /// Suppress output except errors
    #[builder(default)]
    pub quiet: bool,

    /// In a unit test?
    #[builder(default)]
    #[allow(dead_code)]
    pub test_mode: bool,

    /// Parsed CLI arguments
    #[builder(setter(into))]
    pub matches: clap::ArgMatches,
}

impl CliContext {
    /// Create a new CLI context with default storage implementations
    pub async fn new(
        template_context: swissarmyhammer_config::TemplateContext,
        format: OutputFormat,
        format_option: Option<OutputFormat>,
        verbose: bool,
        debug: bool,
        quiet: bool,
        matches: clap::ArgMatches,
    ) -> Result<Self> {
        CliContextBuilder::default()
            .template_context(template_context)
            .format(format)
            .format_option(format_option)
            .verbose(verbose)
            .debug(debug)
            .quiet(quiet)
            .matches(matches)
            .build_async()
            .await
    }

    /// Get the prompt library - returns a new library with all prompts loaded
    /// This reloads prompts to ensure we have the latest version
    pub fn get_prompt_library(&self) -> Result<swissarmyhammer_prompts::PromptLibrary> {
        let mut library = swissarmyhammer_prompts::PromptLibrary::new();
        let mut resolver = swissarmyhammer::PromptResolver::new();

        resolver
            .load_all_prompts(&mut library)
            .map_err(map_error("Failed to load prompts"))?;

        Ok(library)
    }

    /// Render a prompt with parameters, merging with template context
    pub fn render_prompt(
        &self,
        prompt_name: &str,
        parameters: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let library = self.get_prompt_library()?;

        // Create a template context with CLI arguments having highest precedence
        let mut final_context = self.template_context.clone();
        for (key, value) in parameters {
            final_context.set(key.clone(), value.clone());
        }

        // Render the prompt with the merged context
        library
            .render(prompt_name, &final_context)
            .map_err(map_error(&format!(
                "Failed to render prompt '{}'",
                prompt_name
            )))
    }

    /// Display items using the configured output format
    pub fn display<T>(&self, items: Vec<T>) -> Result<()>
    where
        T: serde::Serialize,
    {
        let format = self.format_option.unwrap_or(self.format);
        match format {
            OutputFormat::Table => self.display_as_table(items),
            OutputFormat::Json => self.display_as_json(items),
            OutputFormat::Yaml => self.display_as_yaml(items),
        }
    }

    /// Display items as a table
    fn display_as_table<T>(&self, items: Vec<T>) -> Result<()>
    where
        T: serde::Serialize,
    {
        if items.is_empty() {
            println!("No items to display");
            return Ok(());
        }

        let json_items =
            serde_json::to_value(&items).map_err(map_error("Failed to convert to JSON"))?;

        let array = json_items.as_array().ok_or_else(|| {
            swissarmyhammer_common::SwissArmyHammerError::Other {
                message: "Expected array".to_string(),
            }
        })?;

        if array.is_empty() {
            println!("No items to display");
            return Ok(());
        }

        let table = build_table_from_items(array)?;
        println!("{table}");
        Ok(())
    }

    /// Display items as JSON
    fn display_as_json<T>(&self, items: Vec<T>) -> Result<()>
    where
        T: serde::Serialize,
    {
        let json = serde_json::to_string_pretty(&items)
            .map_err(map_error("Failed to serialize to JSON"))?;
        println!("{}", json);
        Ok(())
    }

    /// Display items as YAML
    fn display_as_yaml<T>(&self, items: Vec<T>) -> Result<()>
    where
        T: serde::Serialize,
    {
        let yaml =
            serde_yaml_ng::to_string(&items).map_err(map_error("Failed to serialize to YAML"))?;
        println!("{}", yaml);
        Ok(())
    }

    /// Display different types based on verbose flag using display rows enum
    pub fn display_prompts(
        &self,
        rows: crate::commands::prompt::display::DisplayRows,
    ) -> Result<()> {
        use crate::commands::prompt::display::DisplayRows;

        match rows {
            DisplayRows::Standard(items) => self.display(items),
            DisplayRows::Verbose(items) => self.display(items),
        }
    }
}

impl CliContextBuilder {
    /// Validate that a required builder field is present
    fn require_field<T>(field: Option<T>, field_name: &str) -> Result<T> {
        field.ok_or_else(|| swissarmyhammer_common::SwissArmyHammerError::Other {
            message: format!("{} is required", field_name),
        })
    }

    /// Build the CliContext with async initialization of storage components
    pub async fn build_async(self) -> Result<CliContext> {
        let mut prompt_library = PromptLibrary::new();

        // Add default prompt sources
        #[allow(deprecated)]
        if let Ok(home_dir) = swissarmyhammer_common::utils::paths::get_swissarmyhammer_dir() {
            let prompts_dir = home_dir.join("prompts");
            if prompts_dir.exists() {
                if let Err(e) = prompt_library.add_directory(&prompts_dir) {
                    eprintln!(
                        "Warning: Failed to load prompts from {:?}: {}",
                        prompts_dir, e
                    );
                }
            }
        }

        // Initialize git operations - make it optional when not in a git repository
        let git_operations = match GitOperations::new() {
            Ok(ops) => {
                tracing::debug!("Git operations initialized successfully");
                Some(Rc::new(ops))
            }
            Err(e) => {
                tracing::warn!("Git operations not available: {}", e);
                None
            }
        };

        Ok(CliContext {
            prompt_library: Arc::new(prompt_library),
            git_operations,
            template_context: Self::require_field(self.template_context, "template_context")?,
            format: self.format.unwrap_or(OutputFormat::Table),
            format_option: self.format_option.unwrap_or_default(),
            verbose: self.verbose.unwrap_or_default(),
            debug: self.debug.unwrap_or_default(),
            quiet: self.quiet.unwrap_or_default(),
            test_mode: self.quiet.unwrap_or_default(),
            matches: Self::require_field(self.matches, "matches")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    #[derive(Serialize, Debug, Clone)]
    struct TestRow {
        status: String,
        name: String,
        message: String,
    }

    /// Test data builder for creating TestRow instances
    fn create_test_row(status: &str, name: &str, message: &str) -> TestRow {
        TestRow {
            status: status.to_string(),
            name: name.to_string(),
            message: message.to_string(),
        }
    }

    /// Helper function to render a table with comfy-table
    fn render_table(test_rows: &[TestRow]) -> String {
        let mut table = swissarmyhammer_doctor::new_table();
        table.set_header(vec!["Status", "Name", "Message"]);

        for row in test_rows {
            table.add_row(vec![&row.status, &row.name, &row.message]);
        }

        table.to_string()
    }

    /// Comprehensive table verification helper
    ///
    /// Validates multiple aspects of table rendering in a single function
    fn verify_table_output(
        table: &str,
        expected_emojis: &[&str],
        min_lines: Option<usize>,
        additional_content: &[&str],
    ) {
        // Verify emojis are present
        for emoji in expected_emojis {
            assert!(
                table.contains(emoji),
                "Table should contain emoji: {}",
                emoji
            );
        }

        // Verify minimum line count if specified
        if let Some(min) = min_lines {
            let lines: Vec<&str> = table.lines().collect();
            assert!(
                lines.len() >= min,
                "Table should have at least {} lines, got {}",
                min,
                lines.len()
            );
        }

        // Verify additional content strings
        for content in additional_content {
            assert!(table.contains(content), "Table should contain: {}", content);
        }

        // Verify table separators in data rows
        let lines: Vec<&str> = table.lines().collect();
        let data_rows: Vec<&str> = lines
            .iter()
            .filter(|line| expected_emojis.iter().any(|emoji| line.contains(emoji)))
            .copied()
            .collect();

        assert_eq!(
            data_rows.len(),
            expected_emojis.len(),
            "Should have {} data rows",
            expected_emojis.len()
        );

        for row in &data_rows {
            assert!(
                row.contains('│'),
                "Row should contain column separators: {}",
                row
            );
        }
    }

    /// Test that table rendering with emoji characters produces properly aligned output
    #[test]
    fn test_table_alignment_with_emojis() {
        let test_rows = vec![
            create_test_row("✓", "Check One", "Everything is working"),
            create_test_row("⚠️", "Check Two", "Warning message"),
            create_test_row("✗", "Check Three", "Error occurred"),
        ];

        let table = render_table(&test_rows);

        verify_table_output(&table, &["✓", "⚠️", "✗"], Some(7), &[]);
    }

    /// Test that table rendering handles long text correctly
    #[test]
    fn test_table_with_long_content() {
        let test_rows = vec![
            create_test_row("✓", "Short Name", "Short message"),
            create_test_row(
                "⚠️",
                "Very Long Name That Might Cause Issues",
                "This is a very long message that contains a lot of text and might cause alignment issues if not handled properly"
            ),
        ];

        let table = render_table(&test_rows);

        verify_table_output(
            &table,
            &["✓", "⚠️"],
            None,
            &["Very Long Name", "very long message"],
        );
    }

    /// Test that empty table is handled gracefully
    #[test]
    fn test_empty_table() {
        let test_rows: Vec<TestRow> = vec![];
        let table = render_table(&test_rows);

        assert!(!table.is_empty(), "Empty table should produce some output");
    }

    /// Test table with special characters
    #[test]
    fn test_table_with_special_characters() {
        let test_rows = vec![
            create_test_row("✓", "Test with → arrow", "Contains • bullet"),
            create_test_row("⚠️", "Test with © symbol", "Contains ™ trademark"),
        ];

        let table = render_table(&test_rows);

        verify_table_output(&table, &["✓", "⚠️"], None, &["→", "•", "©", "™"]);
    }

    // --- Tests for private helper functions ---

    #[test]
    fn test_value_to_string_string() {
        let val = serde_json::Value::String("hello".to_string());
        assert_eq!(super::value_to_string(&val), "hello");
    }

    #[test]
    fn test_value_to_string_number() {
        let val = serde_json::json!(42);
        assert_eq!(super::value_to_string(&val), "42");
    }

    #[test]
    fn test_value_to_string_float() {
        let val = serde_json::json!(2.72);
        assert_eq!(super::value_to_string(&val), "2.72");
    }

    #[test]
    fn test_value_to_string_bool() {
        assert_eq!(
            super::value_to_string(&serde_json::Value::Bool(true)),
            "true"
        );
        assert_eq!(
            super::value_to_string(&serde_json::Value::Bool(false)),
            "false"
        );
    }

    #[test]
    fn test_value_to_string_null() {
        assert_eq!(super::value_to_string(&serde_json::Value::Null), "null");
    }

    #[test]
    fn test_value_to_string_array() {
        let val = serde_json::json!([1, 2, 3]);
        // Arrays fall through to the catch-all which uses .to_string()
        assert_eq!(super::value_to_string(&val), "[1,2,3]");
    }

    #[test]
    fn test_value_to_string_object() {
        let val = serde_json::json!({"key": "val"});
        let result = super::value_to_string(&val);
        assert!(result.contains("key"));
        assert!(result.contains("val"));
    }

    #[test]
    fn test_get_key_priority_known_keys() {
        assert_eq!(super::get_key_priority("name"), 0);
        assert_eq!(super::get_key_priority("description"), 1);
        assert_eq!(super::get_key_priority("title"), 1);
        assert_eq!(super::get_key_priority("source"), 2);
        assert_eq!(super::get_key_priority("status"), 3);
    }

    #[test]
    fn test_get_key_priority_unknown_keys() {
        assert_eq!(super::get_key_priority("foo"), 99);
        assert_eq!(super::get_key_priority("bar"), 99);
        assert_eq!(super::get_key_priority(""), 99);
    }

    #[test]
    fn test_capitalize_first_normal() {
        assert_eq!(super::capitalize_first("hello"), "Hello");
        assert_eq!(super::capitalize_first("world"), "World");
    }

    #[test]
    fn test_capitalize_first_already_capitalized() {
        assert_eq!(super::capitalize_first("Hello"), "Hello");
    }

    #[test]
    fn test_capitalize_first_empty() {
        assert_eq!(super::capitalize_first(""), "");
    }

    #[test]
    fn test_capitalize_first_single_char() {
        assert_eq!(super::capitalize_first("a"), "A");
    }

    #[test]
    fn test_get_sorted_keys() {
        let mut map = serde_json::Map::new();
        map.insert("status".to_string(), serde_json::Value::String("ok".into()));
        map.insert("name".to_string(), serde_json::Value::String("test".into()));
        map.insert("foo".to_string(), serde_json::Value::String("bar".into()));
        map.insert(
            "description".to_string(),
            serde_json::Value::String("desc".into()),
        );

        let keys = super::get_sorted_keys(&map);
        // name(0), description(1), status(3), foo(99)
        assert_eq!(keys[0], "name");
        assert_eq!(keys[1], "description");
        assert_eq!(keys[2], "status");
        assert_eq!(keys[3], "foo");
    }

    #[test]
    fn test_create_table_headers() {
        let keys = vec!["name".to_string(), "status".to_string()];
        let headers = super::create_table_headers(&keys);
        assert_eq!(headers, vec!["Name", "Status"]);
    }

    #[test]
    fn test_create_table_row() {
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), serde_json::Value::String("test".into()));
        map.insert("count".to_string(), serde_json::json!(42));

        let keys = vec![
            "name".to_string(),
            "count".to_string(),
            "missing".to_string(),
        ];
        let row = super::create_table_row(&map, &keys);
        assert_eq!(row, vec!["test", "42", ""]);
    }

    #[test]
    fn test_build_table_from_items_success() {
        let items = vec![
            serde_json::json!({"name": "Alice", "status": "active"}),
            serde_json::json!({"name": "Bob", "status": "inactive"}),
        ];
        let result = super::build_table_from_items(&items);
        assert!(result.is_ok());
        let table = result.unwrap().to_string();
        assert!(table.contains("Alice"));
        assert!(table.contains("Bob"));
    }

    #[test]
    fn test_build_table_from_items_empty() {
        let items: Vec<serde_json::Value> = vec![];
        let result = super::build_table_from_items(&items);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_table_from_items_non_object() {
        let items = vec![serde_json::json!("just a string")];
        let result = super::build_table_from_items(&items);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_table_rows_skips_non_objects() {
        let mut table = swissarmyhammer_doctor::new_table();
        table.set_header(vec!["Name"]);
        let items = vec![
            serde_json::json!({"name": "Alice"}),
            serde_json::json!("not an object"),
            serde_json::json!({"name": "Bob"}),
        ];
        let keys = vec!["name".to_string()];
        super::add_table_rows(&mut table, &items, &keys);
        let output = table.to_string();
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
    }

    #[test]
    fn test_map_error_creates_error_with_context() {
        let mapper = super::map_error::<String>("Loading config");
        let err = mapper("file not found".to_string());
        let err_msg = format!("{}", err);
        assert!(err_msg.contains("Loading config"));
        assert!(err_msg.contains("file not found"));
    }

    #[test]
    fn test_cli_context_builder_require_field_some() {
        let result =
            super::CliContextBuilder::require_field(Some("value".to_string()), "test_field");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "value");
    }

    #[test]
    fn test_cli_context_builder_require_field_none() {
        let result = super::CliContextBuilder::require_field::<String>(None, "test_field");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("test_field"));
        assert!(err_msg.contains("required"));
    }

    #[tokio::test]
    async fn test_cli_context_new() {
        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContext::new(
            template_context,
            super::OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await;
        assert!(ctx.is_ok());
        let ctx = ctx.unwrap();
        assert!(!ctx.verbose);
        assert!(!ctx.debug);
        assert!(!ctx.quiet);
    }

    #[tokio::test]
    async fn test_cli_context_builder_missing_required_fields() {
        // Missing template_context and matches should fail
        let result = super::CliContextBuilder::default().build_async().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cli_context_get_prompt_library() {
        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContext::new(
            template_context,
            super::OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await
        .unwrap();

        let library = ctx.get_prompt_library();
        assert!(library.is_ok());
    }

    #[tokio::test]
    async fn test_cli_context_render_prompt_nonexistent() {
        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContext::new(
            template_context,
            super::OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await
        .unwrap();

        let params = std::collections::HashMap::new();
        let result = ctx.render_prompt("nonexistent-prompt", &params);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cli_context_display_formats() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct Item {
            name: String,
            value: i32,
        }

        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        // Test with format_option overriding format
        let ctx = super::CliContextBuilder::default()
            .template_context(template_context.clone())
            .matches(matches)
            .format(super::OutputFormat::Table)
            .format_option(Some(super::OutputFormat::Json))
            .build_async()
            .await
            .unwrap();

        let items = vec![
            Item {
                name: "a".into(),
                value: 1,
            },
            Item {
                name: "b".into(),
                value: 2,
            },
        ];

        // display uses format_option when set
        let result = ctx.display(items);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_context_display_empty_items() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct Item {
            name: String,
        }

        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContext::new(
            template_context,
            super::OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await
        .unwrap();

        let items: Vec<Item> = vec![];
        let result = ctx.display(items);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_context_display_yaml() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct Item {
            name: String,
        }

        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContextBuilder::default()
            .template_context(template_context)
            .matches(matches)
            .format(super::OutputFormat::Yaml)
            .build_async()
            .await
            .unwrap();

        let items = vec![Item {
            name: "test".into(),
        }];
        let result = ctx.display(items);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_context_display_table_with_items() {
        use serde::Serialize;

        #[derive(Serialize)]
        struct Item {
            name: String,
            status: String,
        }

        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContext::new(
            template_context,
            super::OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await
        .unwrap();

        let items = vec![Item {
            name: "test".into(),
            status: "ok".into(),
        }];
        let result = ctx.display(items);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_context_display_prompts() {
        use crate::commands::prompt::display::{DisplayRows, PromptRow};

        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContextBuilder::default()
            .template_context(template_context)
            .matches(matches)
            .format_option(Some(super::OutputFormat::Json))
            .build_async()
            .await
            .unwrap();

        // Test Standard variant
        let rows = DisplayRows::Standard(vec![PromptRow {
            name: "test".into(),
            title: "Test Prompt".into(),
            source: "builtin".into(),
        }]);
        let result = ctx.display_prompts(rows);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cli_context_display_prompts_verbose() {
        use crate::commands::prompt::display::{DisplayRows, VerbosePromptRow};

        let template_context = swissarmyhammer_config::TemplateContext::new();
        let matches = clap::Command::new("test").get_matches_from(vec!["test"]);

        let ctx = super::CliContextBuilder::default()
            .template_context(template_context)
            .matches(matches)
            .format_option(Some(super::OutputFormat::Json))
            .build_async()
            .await
            .unwrap();

        // Test Verbose variant
        let rows = DisplayRows::Verbose(vec![VerbosePromptRow {
            name: "test".into(),
            title: "Test Prompt".into(),
            description: "A test prompt".into(),
            source: "builtin".into(),
            category: "test".into(),
        }]);
        let result = ctx.display_prompts(rows);
        assert!(result.is_ok());
    }
}
