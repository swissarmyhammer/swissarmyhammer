//! The `ToolRegistry` that stores and dispatches registered tools.

use super::tool::McpTool;
use super::validation::{
    SchemaValidator, ToolValidationError, ToolValidationReport, ToolValidationSeverity,
    ToolValidationWarning,
};
use rmcp::model::{Meta, Tool};
use std::collections::{BTreeSet, HashMap, HashSet};

/// The CLI category of a tool, the `<category>` in `sah tool <category> <name>`.
///
/// A distinct type so a category cannot be passed where a CLI name belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CliCategory<'a>(pub &'a str);

impl AsRef<str> for CliCategory<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

/// The CLI command name of a tool, the `<name>` in `sah tool <category> <name>`.
///
/// A distinct type so a CLI name cannot be passed where a category belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CliName<'a>(pub &'a str);

impl AsRef<str> for CliName<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

/// Registry for managing MCP tools
///
/// The `ToolRegistry` serves as the central repository for all MCP tools within
/// the application. It provides registration, lookup, and enumeration capabilities
/// for tools implementing the `McpTool` trait.
///
/// # Design Goals
///
/// - **Type Safety**: Tools are stored as trait objects with compile-time guarantees
/// - **Performance**: HashMap-based lookup provides O(1) tool resolution
/// - **Extensibility**: New tools can be registered dynamically at runtime
/// - **Memory Efficiency**: Tools are stored once and accessed by reference
///
/// # Usage Patterns
///
/// ## Registration
/// ```rust,ignore
/// let mut registry = ToolRegistry::new();
/// registry.register(MyTool::new());
/// registry.register(AnotherTool::new());
/// ```
///
/// ## Tool Execution
/// ```rust,ignore
/// if let Some(tool) = registry.get_tool("memo_create") {
///     let result = tool.execute(arguments, &context).await?;
///     // Handle result...
/// }
/// ```
///
/// ## MCP Integration
/// ```rust,ignore
/// // List all tools for MCP list_tools response
/// let tools = registry.list_tools();
/// ```
///
/// # Thread Safety
///
/// The registry itself is not thread-safe and should be protected by appropriate
/// synchronization when shared across threads. However, individual tools must
/// implement `Send + Sync` and can be safely called concurrently.
#[derive(Default)]
pub struct ToolRegistry {
    /// Internal storage mapping tool names to trait objects
    ///
    /// Uses HashMap for O(1) lookup performance. Tool names must be unique
    /// and are used as the primary key for tool resolution.
    tools: HashMap<String, Box<dyn McpTool>>,

    /// Set of tool names that are currently disabled.
    ///
    /// Disabled tools are excluded from `list_tools()` and `get_tool()` so
    /// that MCP clients cannot discover or invoke them.
    disabled_tools: HashSet<String>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            disabled_tools: HashSet::new(),
        }
    }

    /// Register a tool in the registry
    pub fn register<T: McpTool + 'static>(&mut self, tool: T) {
        let name = McpTool::name(&tool).to_string();
        self.tools.insert(name, Box::new(tool));
    }

    /// Get a tool by name.
    ///
    /// Returns `None` if the tool is not registered **or** if it is currently disabled.
    /// Callers that need to bypass the disabled filter should access `self.tools` directly
    /// (available within this module).
    pub fn get_tool(&self, name: &str) -> Option<&dyn McpTool> {
        if !self.is_tool_enabled(name) {
            return None;
        }
        self.tools.get(name).map(|tool| tool.as_ref())
    }

    /// Check if a tool matches CLI criteria
    ///
    /// Helper method that checks if a tool matches the specified category and CLI name,
    /// and is not hidden from CLI.
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool to check
    /// * `category` - The CLI category to match
    /// * `cli_name` - The CLI command name to match
    ///
    /// # Returns
    ///
    /// * `true` - If the tool matches all criteria
    /// * `false` - Otherwise
    pub(super) fn matches_cli_criteria(
        &self,
        tool: &dyn McpTool,
        category: CliCategory<'_>,
        cli_name: CliName<'_>,
    ) -> bool {
        self.is_cli_visible(tool)
            && tool.cli_category() == Some(category.as_ref())
            && tool.cli_name() == cli_name.as_ref()
    }

    /// Get a tool by category and CLI name
    ///
    /// Finds a tool that belongs to the specified category and has the specified CLI name.
    /// This is used by the dynamic CLI to map from CLI commands to actual tool names.
    ///
    /// # Arguments
    ///
    /// * `category` - The CLI category (e.g., "issue", "memo")
    /// * `cli_name` - The CLI command name (e.g., "status", "complete")
    ///
    /// # Returns
    ///
    /// * `Some(&dyn McpTool)` - The tool if found
    /// * `None` - If no tool matches the category and CLI name
    pub fn get_tool_by_cli_name(
        &self,
        category: CliCategory<'_>,
        cli_name: CliName<'_>,
    ) -> Option<&dyn McpTool> {
        self.tools
            .values()
            .find(|tool| self.matches_cli_criteria(tool.as_ref(), category, cli_name))
            .map(|tool| tool.as_ref())
    }

    /// List all registered tool names that are currently enabled.
    pub fn list_tool_names(&self) -> Vec<String> {
        self.iter_tools()
            .map(|tool| McpTool::name(tool).to_string())
            .collect()
    }

    /// Get all registered tools as Tool objects for MCP list_tools response.
    ///
    /// Disabled tools are excluded from the result. Every tool has `_meta` set
    /// with `"anthropic/alwaysLoad": true` so Claude Code loads them eagerly
    /// without requiring a ToolSearch round-trip.
    pub fn list_tools(&self) -> Vec<Tool> {
        self.list_tools_filtered(|_| true)
    }

    /// Get registered tools for the MCP list_tools response, composed for a
    /// specific connecting host.
    ///
    /// Like [`list_tools`](Self::list_tools), but additionally keeps only tools
    /// the `host` is served per the `(Host, ToolCategory)` policy in
    /// [`Host::serves`](crate::mcp::host::Host::serves). This is how the serve
    /// boundary advertises a per-client tool surface (e.g. Claude gets the
    /// `Shared` and `Replacement` tools; unknown clients get `Shared` only)
    /// without maintaining a separate registry per host.
    pub fn list_tools_for_host(&self, host: crate::mcp::host::Host) -> Vec<Tool> {
        self.list_tools_filtered(|tool| host.serves(McpTool::category(tool)))
    }

    /// Build the MCP `Tool` objects for every enabled tool that satisfies
    /// `keep`, sharing the schema/meta construction across the unfiltered and
    /// host-filtered entry points.
    ///
    /// Tools are emitted **sorted by name**. The backing store is a `HashMap`
    /// (for O(1) routing lookup), whose iteration order is non-deterministic
    /// under hash randomization — so the list is sorted here, at the emission
    /// boundary, to guarantee a stable order across repeated calls and process
    /// restarts. This keeps the rendered system+tools prefix byte-identical
    /// across sibling agent sessions, a precondition for KV prefix reuse (a
    /// non-deterministic order shrinks the shared-prefix LCP below the full
    /// prefix). Routing/dispatch is unaffected — only the rendered LIST order.
    fn list_tools_filtered<F>(&self, keep: F) -> Vec<Tool>
    where
        F: Fn(&dyn McpTool) -> bool,
    {
        let mut tools: Vec<&dyn McpTool> = self.iter_tools().filter(|tool| keep(*tool)).collect();
        tools.sort_unstable_by_key(|tool| McpTool::name(*tool));

        tools
            .into_iter()
            .map(|tool| {
                let schema = tool.schema();
                let schema_map = if let serde_json::Value::Object(map) = schema {
                    map
                } else {
                    serde_json::Map::new()
                };

                let mut mcp_tool = Tool::new(McpTool::name(tool), tool.description(), schema_map)
                    .with_title(McpTool::name(tool));
                let mut meta = Meta::new();
                meta.0.insert(
                    "anthropic/alwaysLoad".to_string(),
                    serde_json::Value::Bool(true),
                );
                mcp_tool.meta = Some(meta);
                mcp_tool
            })
            .collect()
    }

    /// Get the number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Iterate over all registered tools that are currently enabled.
    ///
    /// Returns an iterator over references to all registered, non-disabled tools.
    /// This is useful for operations that need to process all tools,
    /// such as health checking.
    ///
    /// # Returns
    ///
    /// * Iterator yielding `&dyn McpTool` for each enabled registered tool
    pub fn iter_tools(&self) -> impl Iterator<Item = &dyn McpTool> {
        self.tools
            .values()
            .filter(|tool| self.is_tool_enabled(McpTool::name(tool.as_ref())))
            .map(|tool| tool.as_ref())
    }

    /// Enable or disable a tool by name.
    ///
    /// When `enabled` is `false`, the tool is added to the disabled set and will be
    /// excluded from `list_tools()`, `get_tool()`, and `iter_tools()`.
    /// When `enabled` is `true`, the tool is removed from the disabled set.
    ///
    /// Tools that are not registered can still be added to the disabled set; they will
    /// simply be ignored if registered later while disabled.
    pub fn set_tool_enabled(&mut self, name: &str, enabled: bool) {
        if enabled {
            self.disabled_tools.remove(name);
        } else {
            self.disabled_tools.insert(name.to_string());
        }
    }

    /// Check if a tool is enabled (i.e. not in the disabled set).
    ///
    /// Returns `true` for tools that are not in the disabled set, regardless of whether
    /// they are actually registered.
    pub fn is_tool_enabled(&self, name: &str) -> bool {
        !self.disabled_tools.contains(name)
    }

    /// Enable or disable all currently registered tools at once.
    ///
    /// When `enabled` is `false`, all registered tool names are added to the disabled
    /// set.  When `enabled` is `true`, the disabled set is cleared entirely.
    pub fn set_all_enabled(&mut self, enabled: bool) {
        if enabled {
            self.disabled_tools.clear();
        } else {
            self.disabled_tools = self.tools.keys().cloned().collect();
        }
    }

    /// Return a reference to the set of disabled tool names.
    pub fn disabled_tools(&self) -> &HashSet<String> {
        &self.disabled_tools
    }

    /// True when `tool` may appear on the command line at all.
    ///
    /// A tool reaches the CLI only when it is neither hidden nor disabled.
    /// `tools.yaml` disabling is the same gate [`Self::get_tool`] applies, so
    /// honoring it here keeps the generated command tree from advertising a
    /// tool the registry would then refuse to hand back.
    ///
    /// Every CLI-facing accessor funnels through this one predicate, so the
    /// listing and the lookup can never disagree.
    fn is_cli_visible(&self, tool: &dyn McpTool) -> bool {
        !tool.hidden_from_cli() && self.is_tool_enabled(McpTool::name(tool))
    }

    /// Get unique CLI categories from all registered tools
    ///
    /// Returns a sorted list of unique category names for tools that should
    /// appear in the CLI (not hidden, not disabled, and have a valid category).
    ///
    /// # Returns
    ///
    /// * `Vec<String>` - Sorted list of unique category names
    pub fn get_cli_categories(&self) -> Vec<String> {
        self.get_cli_tools()
            .into_iter()
            .filter_map(|tool| tool.cli_category())
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Filter CLI tools using a custom predicate
    ///
    /// Internal helper method that applies common CLI filtering logic
    /// (excluding hidden and disabled tools) along with a custom predicate.
    ///
    /// # Arguments
    ///
    /// * `predicate` - Custom filter function to apply to each tool
    ///
    /// # Returns
    ///
    /// * `Vec<&dyn McpTool>` - Filtered tools matching the criteria
    fn filter_cli_tools<F>(&self, predicate: F) -> Vec<&dyn McpTool>
    where
        F: Fn(&dyn McpTool) -> bool,
    {
        self.tools
            .values()
            .filter(|tool| self.is_cli_visible(tool.as_ref()) && predicate(tool.as_ref()))
            .map(|tool| tool.as_ref())
            .collect()
    }

    /// Get all tools for a specific CLI category
    ///
    /// Returns references to all tools that belong to the specified category
    /// and are not hidden from CLI.
    ///
    /// # Arguments
    ///
    /// * `category` - The category name to filter by
    ///
    /// # Returns
    ///
    /// * `Vec<&dyn McpTool>` - Tools in the specified category
    pub fn get_tools_for_category(&self, category: &str) -> Vec<&dyn McpTool> {
        self.filter_cli_tools(|tool| tool.cli_category() == Some(category))
    }

    /// Get all tools that should appear in CLI
    ///
    /// Returns references to all tools that are not hidden from CLI
    /// and have a valid category.
    ///
    /// # Returns
    ///
    /// * `Vec<&dyn McpTool>` - All CLI-visible tools
    pub fn get_cli_tools(&self) -> Vec<&dyn McpTool> {
        self.filter_cli_tools(|tool| tool.cli_category().is_some())
    }

    /// Validate all CLI tools for schema compatibility
    ///
    /// Performs comprehensive validation of all tools that should appear in the CLI
    /// to ensure their schemas are compatible with CLI argument generation.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All CLI tools have valid schemas
    /// * `Err(Vec<ToolValidationError>)` - List of validation errors found
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// match registry.validate_cli_tools() {
    ///     Ok(()) => println!("All tools are valid"),
    ///     Err(errors) => {
    ///         for error in errors {
    ///             eprintln!("Tool validation error: {}", error);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn validate_cli_tools(&self) -> Result<(), Vec<ToolValidationError>> {
        let errors = self.collect_all_validation_errors();
        self.report_validation_errors(errors)
    }

    /// Collect validation errors from all tools
    ///
    /// Validates all tools in the registry and collects any errors found.
    ///
    /// # Returns
    ///
    /// * `Vec<ToolValidationError>` - List of all validation errors found
    fn collect_all_validation_errors(&self) -> Vec<ToolValidationError> {
        self.validation_outcomes()
            .flat_map(|(_, errors)| errors)
            .collect()
    }

    /// Validate every registered tool, pairing each tool with the errors it
    /// produced (empty when the tool is valid).
    ///
    /// Validates all tools, not just CLI tools, to catch missing category
    /// errors. This is the one per-tool validation pass that
    /// [`Self::collect_all_validation_errors`], [`Self::validate_all_tools`]
    /// and [`Self::get_tool_validation_warnings`] all build on.
    fn validation_outcomes(
        &self,
    ) -> impl Iterator<Item = (&dyn McpTool, Vec<ToolValidationError>)> {
        self.tools.values().map(|tool| {
            let tool = tool.as_ref();
            (tool, self.validate_tool(tool).err().unwrap_or_default())
        })
    }

    /// Report validation errors
    ///
    /// Converts a vector of validation errors into a Result.
    ///
    /// # Arguments
    ///
    /// * `errors` - Vector of validation errors
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If no errors were found
    /// * `Err(Vec<ToolValidationError>)` - If errors were found
    fn report_validation_errors(
        &self,
        errors: Vec<ToolValidationError>,
    ) -> Result<(), Vec<ToolValidationError>> {
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate a single tool for CLI compatibility
    ///
    /// Performs comprehensive validation of a single tool including:
    /// - Schema structure validation
    /// - Parameter type compatibility
    /// - CLI category and name validation
    /// - Required field validation
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool to validate
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Tool is valid and CLI-compatible
    /// * `Err(Vec<ToolValidationError>)` - List of validation errors found
    pub fn validate_tool(&self, tool: &dyn McpTool) -> Result<(), Vec<ToolValidationError>> {
        let mut errors = Vec::new();

        errors.extend(self.validate_schema(tool));
        errors.extend(self.validate_cli_requirements_for_tool(tool));

        self.report_validation_errors(errors)
    }

    /// Validate schema structure for a tool
    ///
    /// Checks that the tool's schema is valid and compatible with CLI generation.
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool to validate
    ///
    /// # Returns
    ///
    /// * `Vec<ToolValidationError>` - List of schema validation errors found (empty if valid)
    fn validate_schema(&self, tool: &dyn McpTool) -> Vec<ToolValidationError> {
        let tool_name = <dyn McpTool as McpTool>::name(tool);
        let schema = tool.schema();

        match SchemaValidator::validate_schema(&schema) {
            Ok(()) => Vec::new(),
            Err(validation_error) => vec![ToolValidationError::SchemaValidation {
                tool_name: tool_name.to_string(),
                error: validation_error,
            }],
        }
    }

    /// Validate CLI requirements for a tool
    ///
    /// Checks that the tool has proper CLI category and name configuration.
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool to validate
    ///
    /// # Returns
    ///
    /// * `Vec<ToolValidationError>` - List of CLI validation errors found (empty if valid)
    pub(super) fn validate_cli_requirements_for_tool(
        &self,
        tool: &dyn McpTool,
    ) -> Vec<ToolValidationError> {
        if tool.hidden_from_cli() {
            return Vec::new();
        }

        let mut errors = Vec::new();
        let tool_name = <dyn McpTool as McpTool>::name(tool);

        if tool.cli_category().is_none() {
            errors.push(ToolValidationError::MissingCliCategory {
                tool_name: tool_name.to_string(),
            });
        }

        let cli_name = tool.cli_name();
        if cli_name.is_empty() {
            errors.push(ToolValidationError::InvalidCliName {
                tool_name: tool_name.to_string(),
                cli_name: cli_name.to_string(),
                reason: "CLI name cannot be empty".to_string(),
            });
        }

        errors
    }

    /// Validate all tools with comprehensive error reporting
    ///
    /// This method validates all tools in the registry (not just CLI tools) and
    /// provides detailed error reporting suitable for development and debugging.
    ///
    /// # Returns
    ///
    /// `ToolValidationReport` containing validation results and statistics
    pub fn validate_all_tools(&self) -> ToolValidationReport {
        let mut report = ToolValidationReport::new();

        for (_, errors) in self.validation_outcomes() {
            report.total_tools += 1;

            if errors.is_empty() {
                report.valid_tools += 1;
            } else {
                report.invalid_tools += 1;
                report.errors.extend(errors);
            }
        }

        report
    }

    /// Get validation warnings for tools (non-failing validation)
    ///
    /// This method performs validation but returns warnings instead of errors,
    /// suitable for graceful degradation scenarios where invalid tools should
    /// be skipped rather than cause complete failure.
    ///
    /// # Returns
    ///
    /// `Vec<ToolValidationWarning>` - List of validation warnings
    pub fn get_tool_validation_warnings(&self) -> Vec<ToolValidationWarning> {
        self.validation_outcomes()
            .flat_map(|(tool, errors)| {
                errors.into_iter().map(move |error| ToolValidationWarning {
                    tool_name: <dyn McpTool as McpTool>::name(tool).to_string(),
                    error,
                    severity: ToolValidationSeverity::Warning,
                })
            })
            .collect()
    }
}
