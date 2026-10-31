//! The `McpTool` trait, tool categories, and the default-impl macros.

use super::context::ToolContext;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::health::Doctorable;

/// Trait defining the interface for all MCP tools
///
/// The `McpTool` trait provides a standardized interface for implementing MCP tools
/// within the registry pattern. All tools must implement this trait to be usable
/// with the tool registry system.
///
/// # Design Principles
///
/// - **Stateless**: Tools should be stateless and derive all context from the `ToolContext`
/// - **Thread-Safe**: Tools must be `Send + Sync` to work in async environments
/// - **Self-Describing**: Tools provide their own schema and documentation
/// - **Error Handling**: Tools use structured error handling via `McpError`
///
/// # Implementation Guidelines
///
/// ## Tool Names
/// Tool names should follow the pattern `{domain}_{action}` (e.g., `memo_create`, `kanban`).
/// Names must be unique within the registry and should be stable across versions.
///
/// ## Descriptions
/// Use `include_str!("description.md")` to load descriptions from separate Markdown files.
/// This improves maintainability and allows for rich documentation.
///
/// ## Schemas
/// Define comprehensive JSON schemas using the `serde_json::json!` macro. Include:
/// - Parameter types and descriptions
/// - Required vs optional parameters
/// - Validation constraints
/// - Examples in the description
///
/// ## Error Handling
/// Use `McpErrorHandler::handle_error()` to convert domain errors to MCP errors:
///
/// ```rust,ignore
/// match storage.create_memo(title, content).await {
///     Ok(memo) => Ok(BaseToolImpl::create_success_response(format!("Created: {}", memo.id()))),
///     Err(e) => Err(McpErrorHandler::handle_error(e, "create memo")),
/// }
/// ```
///
/// ## Testing
/// Each tool should have comprehensive unit tests covering:
/// - Schema validation
/// - Success cases
/// - Error conditions
/// - Edge cases
///
/// # CLI Integration
///
/// The trait includes optional CLI integration methods that enable dynamic CLI command
/// generation without requiring modifications to existing tool implementations.
#[async_trait::async_trait]
pub trait McpTool:
    Doctorable + swissarmyhammer_common::lifecycle::Initializable + Send + Sync
{
    /// Get the tool's unique identifier name
    ///
    /// The name must be unique within the registry and should follow the
    /// `{domain}_{action}` pattern (e.g., `memo_create`, `kanban`).
    /// Names should be stable across versions.
    fn name(&self) -> &'static str;

    /// Get the tool's human-readable description
    ///
    /// This description is shown to users in tool listings and help text.
    /// Consider using `include_str!("description.md")` to load descriptions
    /// from separate Markdown files for better maintainability.
    fn description(&self) -> &'static str;

    /// Get the tool's JSON schema for argument validation
    ///
    /// The schema should be a valid JSON Schema object defining the structure
    /// and validation rules for the tool's arguments. Include detailed
    /// descriptions for all parameters.
    ///
    /// # Operation-based tools: return the WIRE surface here
    ///
    /// This is the model-facing schema advertised over MCP `tools/list`. For
    /// operation-based tools (those that implement [`McpTool::operations`]) it
    /// MUST be the slim wire surface — `generate_mcp_schema_wire(...)`, which
    /// drops the heavy CLI-facing keys. The full surface (with
    /// `x-operation-schemas` / `x-operation-groups` / `x-op-signatures`) belongs
    /// on the [`McpTool::schema_full`] override, NOT here. See
    /// [`swissarmyhammer_operations::generate_mcp_schema`] for the steering note.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn schema(&self) -> serde_json::Value {
    ///     serde_json::json!({
    ///         "type": "object",
    ///         "properties": {
    ///             "title": {
    ///                 "type": "string",
    ///                 "description": "The memo title",
    ///                 "minLength": 1
    ///             }
    ///         },
    ///         "required": ["title"]
    ///     })
    /// }
    /// ```
    fn schema(&self) -> serde_json::Value;

    /// Get the tool's FULL JSON schema for in-process CLI generation.
    ///
    /// Operation-based tools return a slim, model-facing schema from
    /// [`McpTool::schema`] over the MCP wire — it carries only the `op` enum,
    /// dropping the heavy `x-operation-schemas` / `x-operation-groups` /
    /// `x-forgiving-input` / `examples` keys, the per-op required-field
    /// signatures (`x-op-signatures`, full-only), and the flat per-parameter
    /// properties.
    ///
    /// The in-process CLI generator and argument extractor, however, build the
    /// noun/verb command tree and map clap matches back to JSON arguments from
    /// the schema's flat `properties`. Those consumers call `schema_full()` to
    /// obtain the complete surface.
    ///
    /// # Default
    ///
    /// Returns [`McpTool::schema`] unchanged. Tools whose `schema()` is already
    /// full (the non-operation, hand-written schemas) need not override this.
    fn schema_full(&self) -> serde_json::Value {
        self.schema()
    }

    /// Execute the tool with the given arguments and context
    ///
    /// This is the main entry point for tool execution. The method receives:
    /// - `arguments`: Validated JSON arguments from the MCP client
    /// - `context`: Access to storage backends and services
    ///
    /// # Implementation Pattern
    ///
    /// 1. Parse arguments using `BaseToolImpl::parse_arguments()`
    /// 2. Validate business logic constraints
    /// 3. Perform the operation using context storage
    /// 4. Return structured response using `BaseToolImpl::create_success_response()`
    /// 5. Handle errors using `McpErrorHandler::handle_error()`
    ///
    /// # Error Handling
    ///
    /// Always use `McpErrorHandler::handle_error()` to convert domain errors
    /// to appropriate MCP errors for consistent client experience.
    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError>;

    /// Get the CLI category for grouping related commands
    ///
    /// Returns the category name for CLI command organization. Commands are
    /// grouped into categories following a noun-based pattern (e.g., "memo", "issue").
    ///
    /// The default implementation extracts the category from the tool name using
    /// the `{category}_{action}` naming convention.
    ///
    /// # Returns
    ///
    /// * `Some(&'static str)` - The category name if the tool should appear in CLI
    /// * `None` - If the tool should not be exposed as a CLI command
    ///
    /// # Examples
    ///
    /// * `memo_create` → Some("memo")
    /// * `kanban` → Some("kanban")
    /// * `web_fetch` → Some("web")
    fn cli_category(&self) -> Option<&'static str> {
        // Extract category from tool name by taking prefix before first underscore
        let name = <Self as McpTool>::name(self);
        let prefix = name.split('_').next()?;
        CLI_CATEGORY_TABLE
            .iter()
            .find(|(known_prefix, _)| *known_prefix == prefix)
            .map(|(_, category)| *category)
    }

    /// Get the CLI command name within its category
    ///
    /// Returns the specific command name to use within the CLI category.
    /// The default implementation extracts the action from the tool name using
    /// the `{category}_{action}` naming convention.
    ///
    /// # Examples
    ///
    /// * `memo_create` → "create"
    /// * `kanban` → "kanban"
    /// * `web_fetch` → "fetch"
    fn cli_name(&self) -> &'static str {
        // Extract action from tool name by taking suffix after first underscore
        let name = <Self as McpTool>::name(self);
        name.split_once('_').map_or(name, |(_, action)| action)
    }

    /// Get brief CLI help text for the command
    ///
    /// Returns a concise description for CLI help output. The default
    /// implementation uses the first non-header line of the tool's description.
    ///
    /// # Returns
    ///
    /// * `Some(&'static str)` - Brief help text for CLI display
    /// * `None` - Use the full description or auto-generate help text
    fn cli_about(&self) -> Option<&'static str> {
        // Use first non-header line of description as brief about text
        let desc = self.description();
        for line in desc.lines() {
            let trimmed = line.trim();
            // Skip empty lines, markdown headers, and a lone wrapper tag such
            // as `<skills_instructions>` — none of them is a summary.
            if !trimmed.is_empty() && !trimmed.starts_with('#') && !is_wrapper_tag(trimmed) {
                return Some(line);
            }
        }
        // Fallback to first line if no suitable line found
        desc.lines().next()
    }

    /// Check if the tool should be hidden from CLI command generation
    ///
    /// Returns true if the tool should not be exposed as a CLI command.
    /// Useful for internal tools or tools that don't make sense in CLI context.
    ///
    /// # Default
    ///
    /// All tools are visible in CLI by default.
    fn hidden_from_cli(&self) -> bool {
        false
    }

    /// Whether `sah tool <name> ...` must print JSON instead of YAML.
    ///
    /// `sah tool` output is written for a person, and YAML reads better, so
    /// that is the default. A tool whose CLI output is consumed by a program
    /// rather than read overrides this: the program does a strict parse and a
    /// YAML rendering is not a document it can load.
    ///
    /// The tool owns this answer because the tool knows its consumer. A list
    /// kept in the CLI would drift the moment a tool moved or was renamed.
    ///
    /// # Default
    ///
    /// `false` — render YAML for a human reader.
    fn cli_output_is_json(&self) -> bool {
        false
    }

    /// Get the operations this tool supports (for operation-based CLI generation)
    ///
    /// Tools that use the operation pattern return their operations here,
    /// enabling the CLI to generate noun-verb subcommand structure.
    /// Tools that don't use operations return an empty slice and use schema-based
    /// CLI generation instead.
    ///
    /// # Default
    ///
    /// Returns an empty slice - tool uses schema-based CLI generation.
    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        &[]
    }

    /// Structural category describing the tool's relationship to a host agent.
    ///
    /// The category is metadata about *what kind of capability* a tool provides,
    /// independent of any particular host. It lets the serve boundary compose a
    /// per-client tool surface (e.g. mounting agent capabilities for a host that
    /// lacks them, or denying a native tool a [`ToolCategory::Replacement`]
    /// supersedes) instead of subtracting from a single global registry.
    ///
    /// See [`ToolCategory`] for the meaning of each variant.
    ///
    /// # Default
    ///
    /// Returns [`ToolCategory::Shared`] — most tools are domain capabilities that
    /// every host gets.
    fn category(&self) -> ToolCategory {
        ToolCategory::Shared
    }
}

/// Structural category of an [`McpTool`] relative to a host agent.
///
/// Reported by [`McpTool::category`]. The category is composition metadata: the
/// serve boundary reads it to build the tool surface for a given client, rather
/// than registering everything and subtracting afterwards.
///
/// - [`ToolCategory::Shared`] — a domain capability every host gets (kanban, git,
///   code context, question, ralph). The default.
/// - [`ToolCategory::Agent`] — a base agent capability that off-the-shelf agents
///   (like Claude Code) already provide natively: file read/write/edit, glob,
///   grep, web, skills, subagent delegation. Mounted for hosts that lack it.
/// - [`ToolCategory::Replacement`] — an agent capability that also supersedes a
///   named native host tool. `native` is the host tool it replaces (e.g. a
///   `shell` tool replaces `Bash`). A replacement is an agent capability *and*
///   signals that the host's native tool of that name should defer to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    /// Domain capability available to every host. The default.
    #[default]
    Shared,
    /// Base agent capability mounted for hosts that lack it natively.
    Agent,
    /// Agent capability that also supersedes a named native host tool.
    Replacement {
        /// The native host tool this capability replaces (e.g. `"Bash"`).
        native: &'static str,
    },
}

/// Shared body of [`impl_default_doctorable!`] and [`impl_empty_initializable!`].
///
/// Both traits expose the same two identity methods: `name()` delegates to the
/// tool's [`McpTool::name`] and `category()` is `"tools"`. Each public macro
/// passes its trait path here so the body is written once.
#[doc(hidden)]
#[macro_export]
macro_rules! impl_tool_identity {
    ($trait_path:path, $tool_type:ty) => {
        impl $trait_path for $tool_type {
            fn name(&self) -> &str {
                <Self as $crate::mcp::tool_registry::McpTool>::name(self)
            }

            fn category(&self) -> &str {
                "tools"
            }
        }
    };
}

/// Macro to implement Doctorable for tools that don't have special health checks
///
/// Wires `name()` to the tool's [`McpTool::name`] and `category()` to `"tools"`,
/// and inherits the [`Doctorable::run_health_checks`] default — a single OK check
/// derived from `name()`/`category()`. As a result every tool surfaces at least
/// one OK line in `sah doctor`. Tools that need custom diagnostics should
/// implement Doctorable manually and override `run_health_checks`.
///
/// # Example
///
/// ```ignore
/// impl_default_doctorable!(MyTool);
/// ```
#[macro_export]
macro_rules! impl_default_doctorable {
    ($tool_type:ty) => {
        $crate::impl_tool_identity!(swissarmyhammer_common::health::Doctorable, $tool_type);
    };
}

/// Macro to implement Initializable for tools that don't have lifecycle operations
///
/// Provides a default implementation with empty init/deinit/start/stop.
/// Tools that need custom lifecycle should implement Initializable manually.
///
/// # Example
///
/// ```ignore
/// impl_empty_initializable!(MyTool);
/// ```
#[macro_export]
macro_rules! impl_empty_initializable {
    ($tool_type:ty) => {
        $crate::impl_tool_identity!(swissarmyhammer_common::lifecycle::Initializable, $tool_type);
    };
}

/// Rows of (tool name prefix, CLI category) read by the default
/// [`McpTool::cli_category`]. The prefix is the part of the tool name before
/// the first underscore. A new category is one new row.
const CLI_CATEGORY_TABLE: &[(&str, &str)] = &[
    ("memo", "memo"),
    ("web", "web"),
    ("outline", "outline"),
    ("notify", "notify"),
    ("kanban", "kanban"),
    ("git", "git"),
    ("cel", "cel"),
];

/// True when `line` is nothing but an XML-style tag, such as
/// `<skills_instructions>` or `</skills_instructions>`.
///
/// Some tools fence their description in a wrapper tag so a host agent can find
/// the block. The tag is markup, not prose, so it must never become a CLI
/// command summary.
pub(super) fn is_wrapper_tag(line: &str) -> bool {
    line.starts_with('<') && line.ends_with('>') && !line.contains(' ')
}
