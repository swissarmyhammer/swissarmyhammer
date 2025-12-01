//! CLI Context
//!
//! Shared context object that holds all storage instances and configuration
//! to avoid recreating them in each command.

use std::{rc::Rc, sync::Arc};
use swissarmyhammer_common::Result;
use swissarmyhammer_git::GitOperations;

use crate::cli::OutputFormat;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_workflow::WorkflowStorage;

/// Helper function to create error mapping closures with context
fn map_error<E: std::fmt::Display>(
    context: &str,
) -> impl FnOnce(E) -> swissarmyhammer_common::SwissArmyHammerError + '_ {
    move |e| swissarmyhammer_common::SwissArmyHammerError::Other {
        message: format!("{}: {}", context, e),
    }
}

/// Shared CLI context containing all storage objects, configuration, and parsed arguments
#[derive(derive_builder::Builder)]
#[builder(pattern = "owned")]
pub struct CliContext {
    /// Workflow storage for loading and managing workflows
    pub workflow_storage: Arc<WorkflowStorage>,

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
        // Use explicit format option if provided, otherwise use default format
        let format = self.format_option.unwrap_or(self.format);
        match format {
            OutputFormat::Table => {
                if items.is_empty() {
                    println!("No items to display");
                } else {
                    // Convert items to JSON for dynamic table building
                    let json_items = serde_json::to_value(&items)
                        .map_err(map_error("Failed to convert to JSON"))?;

                    if let Some(array) = json_items.as_array() {
                        if let Some(first) = array.first() {
                            if let Some(obj) = first.as_object() {
                                use comfy_table::{presets::UTF8_FULL, Table};
                                let mut table = Table::new();
                                table.load_preset(UTF8_FULL);

                                // Collect keys with preferred ordering for common patterns
                                let mut keys: Vec<_> = obj.keys().collect();

                                // Apply custom ordering for common patterns: name, description/title, source, etc.
                                keys.sort_by_key(|k| {
                                    match k.as_str() {
                                        "name" => 0,
                                        "description" => 1,
                                        "title" => 1,
                                        "source" => 2,
                                        "status" => 3,
                                        _ => 99, // Everything else comes after
                                    }
                                });

                                // Add header row with capitalized names
                                let headers: Vec<String> = keys
                                    .iter()
                                    .map(|k| {
                                        // Capitalize first letter of header
                                        let mut chars = k.chars();
                                        match chars.next() {
                                            None => String::new(),
                                            Some(f) => {
                                                f.to_uppercase().collect::<String>()
                                                    + chars.as_str()
                                            }
                                        }
                                    })
                                    .collect();
                                table.set_header(headers);

                                // Add data rows using the same key order
                                for item in array {
                                    if let Some(obj) = item.as_object() {
                                        let row: Vec<String> = keys
                                            .iter()
                                            .map(|k| {
                                                obj.get(*k)
                                                    .map(|v| match v {
                                                        serde_json::Value::String(s) => s.clone(),
                                                        serde_json::Value::Number(n) => {
                                                            n.to_string()
                                                        }
                                                        serde_json::Value::Bool(b) => b.to_string(),
                                                        serde_json::Value::Null => {
                                                            "null".to_string()
                                                        }
                                                        _ => v.to_string(),
                                                    })
                                                    .unwrap_or_default()
                                            })
                                            .collect();
                                        table.add_row(row);
                                    }
                                }

                                println!("{table}");
                            }
                        }
                    }
                }
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&items)
                    .map_err(map_error("Failed to serialize to JSON"))?;
                println!("{}", json);
            }
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(&items)
                    .map_err(map_error("Failed to serialize to YAML"))?;
                println!("{}", yaml);
            }
        }
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
    /// Initialize storage with consistent error handling
    ///
    /// Generic helper to reduce error context duplication across storage initialization
    fn with_storage_error_context<T, E: std::fmt::Display>(
        storage_type: &str,
        result: std::result::Result<T, E>,
    ) -> Result<T> {
        result.map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
            message: format!("Failed to create {}: {}", storage_type, e),
        })
    }

    /// Validate that a required builder field is present
    fn require_field<T>(field: Option<T>, field_name: &str) -> Result<T> {
        field.ok_or_else(|| swissarmyhammer_common::SwissArmyHammerError::Other {
            message: format!("{} is required", field_name),
        })
    }

    /// Generic async storage initialization helper
    async fn initialize_storage<T, F, Fut, E>(storage_type: &str, initializer: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Display,
    {
        let result = initializer().await;
        Self::with_storage_error_context(storage_type, result)
    }

    /// Initialize workflow storage with error context
    async fn initialize_workflow_storage() -> Result<WorkflowStorage> {
        Self::initialize_storage("workflow storage", || async {
            tokio::task::spawn_blocking(WorkflowStorage::file_system)
                .await
                .map_err(map_error(
                    "Failed to spawn blocking task for workflow storage",
                ))?
        })
        .await
    }

    /// Build the CliContext with async initialization of storage components
    pub async fn build_async(self) -> Result<CliContext> {
        let workflow_storage = Arc::new(Self::initialize_workflow_storage().await?);

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
            workflow_storage,
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
        use comfy_table::{presets::UTF8_FULL, Table};
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
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
}
