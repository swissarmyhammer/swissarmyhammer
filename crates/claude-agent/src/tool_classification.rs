// TODO - this looks to be only used from testing and is a useless reformatting fo existing data

//! Tool classification and title generation for ACP compliance
//!
//! This module provides functionality to classify tools by kind and generate
//! human-readable titles based on tool names and parameters.
//!
//! Each known tool name and each known operation is declared once, in a table
//! that holds its kind and, where it has one, its title. A tool name that has
//! no title row gets its name humanized as the title.

use crate::tool_types::{ToolCallReport, ToolKind};

/// Name suffix that marks an operation-based tool (for example `mcp__sah__treesitter`).
/// Such a tool picks its behavior from the `op` argument.
const OP_TOOL_SUFFIX: &str = "__treesitter";

/// Prefix that marks a tool served by an MCP server.
const MCP_TOOL_PREFIX: &str = "mcp__";

/// Title for a tool that has no name, or none after its MCP prefix.
const UNKNOWN_TOOL_TITLE: &str = "Unknown tool";

/// Title for an operation that has no row in [`OPS`].
const UNKNOWN_OP_TITLE: &str = "Code operation";

/// Argument keys that can hold a file path, tried in order.
const PATH_KEYS: &[&str] = &["path", "file_path"];

/// Placeholder in a title template that stands for the argument value.
const VALUE_PLACEHOLDER: &str = "{}";

/// How an argument value is shown in a title.
#[derive(Debug, Clone, Copy)]
enum ValueStyle {
    /// Show the final path component only.
    FileName,
    /// Show the value as it is.
    Verbatim,
}

/// The two texts of a title.
#[derive(Debug, Clone, Copy)]
struct TitleText {
    /// Title when a value is present; [`VALUE_PLACEHOLDER`] marks the value.
    template: &'static str,
    /// Title when no key holds a value.
    fallback: &'static str,
}

/// The title of a tool or operation, built from one argument.
#[derive(Debug, Clone, Copy)]
struct TitleSpec {
    /// Argument keys that can hold the value, tried in order.
    keys: &'static [&'static str],
    style: ValueStyle,
    text: TitleText,
}

impl TitleSpec {
    /// Build the title from the arguments of a call.
    fn render(&self, arguments: &serde_json::Value) -> String {
        let value = self
            .keys
            .iter()
            .find_map(|key| str_arg(arguments, key))
            .map(|value| match self.style {
                ValueStyle::FileName => file_name_or_path(value),
                ValueStyle::Verbatim => value,
            });
        match value {
            Some(value) => self.text.template.replace(VALUE_PLACEHOLDER, value),
            None => self.text.fallback.to_string(),
        }
    }
}

/// A group of exact tool names that share one kind and, optionally, one title.
struct ToolSpec {
    names: &'static [&'static str],
    kind: ToolKind,
    title: Option<TitleSpec>,
}

/// One operation of an operation-based tool.
struct OpSpec {
    name: &'static str,
    kind: ToolKind,
    title: TitleSpec,
}

/// Title built from a file path argument, shown as its file name.
const fn file_title(text: TitleText) -> TitleSpec {
    TitleSpec {
        keys: PATH_KEYS,
        style: ValueStyle::FileName,
        text,
    }
}

const READ_TITLE: TitleSpec = file_title(TitleText {
    template: "Reading {}",
    fallback: "Reading file",
});

const WRITE_TITLE: TitleSpec = file_title(TitleText {
    template: "Writing to {}",
    fallback: "Writing file",
});

const DELETE_TITLE: TitleSpec = file_title(TitleText {
    template: "Deleting {}",
    fallback: "Deleting file",
});

const SEARCH_TITLE: TitleSpec = TitleSpec {
    keys: &["pattern"],
    style: ValueStyle::Verbatim,
    text: TitleText {
        template: "Searching for '{}'",
        fallback: "Searching files",
    },
};

const TERMINAL_TITLE: TitleSpec = TitleSpec {
    keys: &["command"],
    style: ValueStyle::Verbatim,
    text: TitleText {
        template: "Running {}",
        fallback: "Creating terminal session",
    },
};

/// Exact tool names, with the kind and the title of each group.
static TOOLS: &[ToolSpec] = &[
    // File system read operations
    ToolSpec {
        names: &["fs_read_text_file", "fs_read"],
        kind: ToolKind::Read,
        title: Some(READ_TITLE),
    },
    ToolSpec {
        names: &["read_file"],
        kind: ToolKind::Read,
        title: None,
    },
    // File system write and modification operations
    ToolSpec {
        names: &["fs_write_text_file", "fs_write"],
        kind: ToolKind::Edit,
        title: Some(WRITE_TITLE),
    },
    ToolSpec {
        names: &["write_file", "fs_edit", "edit_file"],
        kind: ToolKind::Edit,
        title: None,
    },
    // File deletion operations
    ToolSpec {
        names: &["fs_delete"],
        kind: ToolKind::Delete,
        title: Some(DELETE_TITLE),
    },
    ToolSpec {
        names: &["delete_file", "remove_file"],
        kind: ToolKind::Delete,
        title: None,
    },
    // File move and rename operations
    ToolSpec {
        names: &["fs_move", "move_file", "rename_file"],
        kind: ToolKind::Move,
        title: None,
    },
    // Search and grep operations
    ToolSpec {
        names: &["search", "grep"],
        kind: ToolKind::Search,
        title: Some(SEARCH_TITLE),
    },
    ToolSpec {
        names: &["fs_search", "find"],
        kind: ToolKind::Search,
        title: None,
    },
    // Terminal and command execution
    ToolSpec {
        names: &["terminal_create"],
        kind: ToolKind::Execute,
        title: Some(TERMINAL_TITLE),
    },
    ToolSpec {
        names: &["terminal_write", "terminal_read", "execute", "run"],
        kind: ToolKind::Execute,
        title: None,
    },
    // External data fetching
    ToolSpec {
        names: &["fetch", "http_get", "download", "curl", "wget"],
        kind: ToolKind::Fetch,
        title: None,
    },
    // Internal reasoning and planning tools
    ToolSpec {
        names: &[
            "think",
            "reason",
            "plan",
            "analyze_approach",
            "generate_strategy",
        ],
        kind: ToolKind::Think,
        title: None,
    },
];

/// Keywords found in an MCP tool name and the kind each group has.
/// The first group with a matching keyword wins, so the order matters.
const MCP_KEYWORD_KINDS: &[(&[&str], ToolKind)] = &[
    (&["read", "get", "show", "list"], ToolKind::Read),
    (&["write", "create", "edit", "update"], ToolKind::Edit),
    (&["delete", "remove"], ToolKind::Delete),
    (&["search", "grep", "find"], ToolKind::Search),
    (&["execute", "shell", "terminal"], ToolKind::Execute),
    (&["fetch", "web", "http"], ToolKind::Fetch),
];

/// Operations of an operation-based tool, with the kind and the title of each.
static OPS: &[OpSpec] = &[
    OpSpec {
        name: "search code",
        kind: ToolKind::Search,
        title: TitleSpec {
            keys: &["query"],
            style: ValueStyle::Verbatim,
            text: TitleText {
                template: "Searching code for '{}'",
                fallback: "Searching code",
            },
        },
    },
    OpSpec {
        name: "query ast",
        kind: ToolKind::Read,
        title: file_title(TitleText {
            template: "Querying AST of {}",
            fallback: "Querying AST",
        }),
    },
];

/// Find the row of an exact tool name.
fn tool_spec(tool_name: &str) -> Option<&'static ToolSpec> {
    TOOLS.iter().find(|spec| spec.names.contains(&tool_name))
}

/// Find the row of an operation name.
fn op_spec(op: &str) -> Option<&'static OpSpec> {
    OPS.iter().find(|spec| spec.name == op)
}

/// How a tool that has no row in [`TOOLS`] is handled.
enum ToolRoute {
    /// An MCP tool that picks its behavior from the `op` argument.
    Op,
    /// Any other MCP tool.
    Mcp,
    /// A tool that is not served by an MCP server.
    Plain,
}

/// Choose the route of a tool by its name; the one place that reads the MCP prefix and the op suffix.
fn route_of(tool: &str) -> ToolRoute {
    if !tool.starts_with(MCP_TOOL_PREFIX) {
        ToolRoute::Plain
    } else if tool.ends_with(OP_TOOL_SUFFIX) {
        ToolRoute::Op
    } else {
        ToolRoute::Mcp
    }
}

/// Read a string argument; `None` when the key is absent or not a string.
fn str_arg<'a>(arguments: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    arguments.get(key).and_then(|v| v.as_str())
}

/// Read the `op` argument of an operation-based tool; empty when absent.
fn op_of(arguments: &serde_json::Value) -> &str {
    str_arg(arguments, "op").unwrap_or("")
}

/// Reduce a path to its final component, or the whole text when it has none.
fn file_name_or_path(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// Turn a snake_case name into a sentence-case title; the unknown-tool title when no visible character is left.
fn humanize_name(name: &str) -> String {
    let spaced = name.replace('_', " ");
    let mut chars = spaced.trim().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_else(|| UNKNOWN_TOOL_TITLE.to_string())
}

impl ToolKind {
    /// Classify a tool by its name and parameters to determine the appropriate kind
    pub fn classify_tool(tool_name: &str, arguments: &serde_json::Value) -> Self {
        // ACP requires comprehensive tool call reporting with rich metadata:
        // 1. toolCallId: Unique identifier for correlation across updates
        // 2. title: Human-readable description of tool operation
        // 3. kind: Classification for UI optimization and icon selection
        // 4. status: Lifecycle state (pending, in_progress, completed, failed)
        // 5. content: Output content produced by tool execution
        // 6. locations: File paths for follow-along features
        // 7. rawInput/rawOutput: Detailed I/O data for debugging
        //
        // Complete reporting enables rich client experiences and debugging.

        if let Some(spec) = tool_spec(tool_name) {
            return spec.kind;
        }

        match route_of(tool_name) {
            // MCP tools - classify by op argument or by keyword in the name
            ToolRoute::Op => Self::classify_by_op(arguments),
            ToolRoute::Mcp => MCP_KEYWORD_KINDS
                .iter()
                .find(|(keywords, _)| keywords.iter().any(|k| tool_name.contains(k)))
                .map_or(ToolKind::Other, |(_, kind)| *kind),
            // Default fallback for unknown tools
            ToolRoute::Plain => ToolKind::Other,
        }
    }

    /// Classify an operation-based tool by inspecting the `op` argument
    fn classify_by_op(arguments: &serde_json::Value) -> Self {
        op_spec(op_of(arguments)).map_or(ToolKind::Other, |spec| spec.kind)
    }
}

/// Tool title generation system for human-readable descriptions
impl ToolCallReport {
    /// Generate a context-aware human-readable title based on tool name and parameters
    pub fn generate_title(tool_name: &str, arguments: &serde_json::Value) -> String {
        if let Some(title) = tool_spec(tool_name).and_then(|spec| spec.title) {
            return title.render(arguments);
        }

        match route_of(tool_name) {
            ToolRoute::Op => Self::generate_op_title(arguments),
            // MCP tools - generate titles based on tool name and parameters
            ToolRoute::Mcp => {
                let name = tool_name.strip_prefix(MCP_TOOL_PREFIX).unwrap_or(tool_name);
                humanize_name(name)
            }
            // Default case - convert snake_case to sentence case
            ToolRoute::Plain => humanize_name(tool_name),
        }
    }

    /// Generate a title for operation-based tools by inspecting the `op` argument
    fn generate_op_title(arguments: &serde_json::Value) -> String {
        match op_spec(op_of(arguments)) {
            Some(spec) => spec.title.render(arguments),
            None => UNKNOWN_OP_TITLE.to_string(),
        }
    }
}
