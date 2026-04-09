//! CLI definition for the code-context command-line interface.
//!
//! This module is self-contained -- it only depends on `clap` and `std` so that
//! `build.rs` can compile it independently via `#[path = "src/cli.rs"]` to
//! generate documentation, man pages, and shell completions at build time.

use clap::{Parser, Subcommand, ValueEnum};

/// Target location for install/uninstall operations.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum InstallTarget {
    /// Project-level settings (.claude/settings.json)
    Project,
    /// Local project settings, not committed (.claude/settings.local.json)
    Local,
    /// User-level settings (~/.claude/settings.json)
    User,
}

impl std::fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallTarget::Project => write!(f, "project"),
            InstallTarget::Local => write!(f, "local"),
            InstallTarget::User => write!(f, "user"),
        }
    }
}

/// code-context - Structural code intelligence for AI agents
///
/// Provides indexed code navigation, symbol lookup, call graph traversal,
/// blast radius analysis, and semantic search. Exposes these capabilities
/// as MCP tools for AI coding agents.
#[derive(Parser, Debug)]
#[command(name = "code-context")]
#[command(version)]
#[command(about = "Structural code intelligence for AI coding agents")]
pub struct Cli {
    /// Enable debug output to stderr
    #[arg(short, long, global = true)]
    pub debug: bool,

    /// Output results as JSON (for operation commands)
    #[arg(short, long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// code-context subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run MCP server over stdio, exposing code-context tools
    Serve,
    /// Install code-context MCP server into Claude Code settings
    Init {
        /// Where to install the server configuration
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Remove code-context from Claude Code settings
    Deinit {
        /// Where to remove the server configuration from
        #[arg(value_enum, default_value_t = InstallTarget::Project)]
        target: InstallTarget,
    },
    /// Diagnose code-context configuration and setup
    Doctor {
        /// Show detailed output including fix suggestions
        #[arg(short, long)]
        verbose: bool,
    },
    /// Deploy code-context skill to agent .skills/ directories
    Skill,

    // -- Operation subcommand groups (verb-noun pattern) --
    /// Get a resource (symbol, callgraph, blast radius, status, etc.)
    Get {
        #[command(subcommand)]
        command: GetCommands,
    },
    /// Search for symbols, code, or workspace symbols
    Search {
        #[command(subcommand)]
        command: SearchCommands,
    },
    /// List resources (symbols in a file)
    List {
        #[command(subcommand)]
        command: ListCommands,
    },
    /// Regex search across stored code chunks
    Grep {
        #[command(subcommand)]
        command: GrepCommands,
    },
    /// Execute tree-sitter queries against parsed ASTs
    Query {
        #[command(subcommand)]
        command: QueryCommands,
    },
    /// Find duplicated code
    Find {
        #[command(subcommand)]
        command: FindCommands,
    },
    /// Trigger re-indexing
    Build {
        #[command(subcommand)]
        command: BuildCommands,
    },
    /// Wipe index data
    Clear {
        #[command(subcommand)]
        command: ClearCommands,
    },
    /// LSP server management
    Lsp {
        #[command(subcommand)]
        command: LspCommands,
    },
    /// Detect project types and languages
    Detect {
        #[command(subcommand)]
        command: DetectCommands,
    },
}

// ---------------------------------------------------------------------------
// Get subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context get`.
#[derive(Subcommand, Debug)]
pub enum GetCommands {
    /// Look up symbol locations and source text with fuzzy matching
    Symbol {
        /// Symbol name or qualified path to search for
        #[arg(long)]
        query: String,
        /// Maximum number of results to return
        #[arg(long)]
        max_results: Option<u64>,
    },
    /// Traverse call graph from a starting symbol
    Callgraph {
        /// Symbol identifier (name or file:line:char locator)
        #[arg(long)]
        symbol: String,
        /// Traversal direction: inbound, outbound, or both
        #[arg(long)]
        direction: Option<String>,
        /// Maximum traversal depth (1-5)
        #[arg(long)]
        max_depth: Option<u64>,
    },
    /// Analyze blast radius of changes to a file or symbol
    Blastradius {
        /// File path to analyze
        #[arg(long)]
        file_path: String,
        /// Optional symbol name to narrow the starting set
        #[arg(long)]
        symbol: Option<String>,
        /// Maximum number of hops to follow (1-10)
        #[arg(long)]
        max_hops: Option<u64>,
    },
    /// Health report with file counts, indexing progress, chunk/edge counts
    Status,
    /// Go to definition with layered resolution (live LSP, LSP index, tree-sitter)
    Definition {
        /// Path to the file containing the symbol
        #[arg(long)]
        file_path: String,
        /// Zero-based line number of the symbol
        #[arg(long)]
        line: u64,
        /// Zero-based character offset within the line
        #[arg(long)]
        character: u64,
    },
    /// Go to type definition (live LSP only)
    TypeDefinition {
        /// Path to the file containing the symbol
        #[arg(long)]
        file_path: String,
        /// Zero-based line number of the symbol
        #[arg(long)]
        line: u64,
        /// Zero-based character offset within the line
        #[arg(long)]
        character: u64,
    },
    /// Get hover information (type signature, docs)
    Hover {
        /// Path to the file containing the symbol
        #[arg(long)]
        file_path: String,
        /// Zero-based line number of the symbol
        #[arg(long)]
        line: u64,
        /// Zero-based character offset within the line
        #[arg(long)]
        character: u64,
    },
    /// Find all references to a symbol
    References {
        /// Path to the file containing the symbol
        #[arg(long)]
        file_path: String,
        /// Zero-based line number of the symbol
        #[arg(long)]
        line: u64,
        /// Zero-based character offset within the line
        #[arg(long)]
        character: u64,
        /// Whether to include the declaration itself in results
        #[arg(long)]
        include_declaration: Option<bool>,
        /// Maximum number of references to return
        #[arg(long)]
        max_results: Option<u64>,
    },
    /// Find implementations of a trait/interface
    Implementations {
        /// Path to the file containing the trait/interface symbol
        #[arg(long)]
        file_path: String,
        /// Zero-based line number of the symbol
        #[arg(long)]
        line: u64,
        /// Zero-based character offset within the line
        #[arg(long)]
        character: u64,
        /// Maximum number of implementation locations to return
        #[arg(long)]
        max_results: Option<u64>,
    },
    /// Get code actions (quickfixes, refactors) for a range (live LSP only)
    CodeActions {
        /// Path to the file to get code actions for
        #[arg(long)]
        file_path: String,
        /// Zero-based start line of the range
        #[arg(long)]
        start_line: u64,
        /// Zero-based start character offset
        #[arg(long)]
        start_character: u64,
        /// Zero-based end line of the range
        #[arg(long)]
        end_line: u64,
        /// Zero-based end character offset
        #[arg(long)]
        end_character: u64,
        /// Filter for code action kinds (e.g. quickfix, refactor, source)
        #[arg(long)]
        filter_kind: Option<Vec<String>>,
    },
    /// Find all callers of a function at a given position
    InboundCalls {
        /// Path to the file containing the target symbol
        #[arg(long)]
        file_path: String,
        /// Zero-based line number of the target symbol
        #[arg(long)]
        line: u64,
        /// Zero-based character offset within the line
        #[arg(long)]
        character: u64,
        /// Recursive depth for caller traversal (1-5)
        #[arg(long)]
        depth: Option<u64>,
    },
    /// Preview rename edits without applying them (live LSP only)
    RenameEdits {
        /// Path to the file containing the symbol to rename
        #[arg(long)]
        file_path: String,
        /// Zero-based line number of the symbol
        #[arg(long)]
        line: u64,
        /// Zero-based character offset within the line
        #[arg(long)]
        character: u64,
        /// The new name for the symbol
        #[arg(long)]
        new_name: String,
    },
    /// Get errors and warnings for a file (live LSP only)
    Diagnostics {
        /// Path to the file to get diagnostics for
        #[arg(long)]
        file_path: String,
        /// Only return diagnostics at or above this severity (error, warning, info, hint)
        #[arg(long)]
        severity_filter: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Search subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context search`.
#[derive(Subcommand, Debug)]
pub enum SearchCommands {
    /// Fuzzy search across all indexed symbols
    Symbol {
        /// Text to fuzzy-match against symbol names
        #[arg(long)]
        query: String,
        /// Filter by symbol kind (function, method, struct, class, etc.)
        #[arg(long)]
        kind: Option<String>,
        /// Maximum number of results to return
        #[arg(long)]
        max_results: Option<u64>,
    },
    /// Semantic similarity search across code chunks using embeddings
    Code {
        /// Natural language query for semantically similar code
        #[arg(long)]
        query: String,
        /// Maximum number of results to return
        #[arg(long)]
        top_k: Option<u64>,
        /// Minimum cosine similarity threshold (0.0-1.0)
        #[arg(long)]
        min_similarity: Option<f64>,
        /// Only search chunks from files matching this path pattern
        #[arg(long)]
        file_pattern: Option<String>,
        /// Only search chunks from files with these extensions
        #[arg(long)]
        language: Option<Vec<String>>,
    },
    /// Live workspace symbol search with layered resolution
    WorkspaceSymbol {
        /// Symbol name or text to search for across the workspace
        #[arg(long)]
        query: String,
        /// Maximum number of results to return
        #[arg(long)]
        max_results: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// List subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context list`.
#[derive(Subcommand, Debug)]
pub enum ListCommands {
    /// List all symbols in a specific file, sorted by start line
    Symbols {
        /// Path to the file to list symbols from
        #[arg(long)]
        file_path: String,
    },
}

// ---------------------------------------------------------------------------
// Grep subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context grep`.
#[derive(Subcommand, Debug)]
pub enum GrepCommands {
    /// Regex search across stored code chunks
    Code {
        /// Regex pattern to search for
        #[arg(long)]
        pattern: String,
        /// Only search chunks from files with these extensions (e.g. rs, py)
        #[arg(long)]
        language: Option<Vec<String>>,
        /// Only search chunks from these specific file paths
        #[arg(long)]
        files: Option<Vec<String>>,
        /// Maximum number of matching chunks to return
        #[arg(long)]
        max_results: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// Query subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context query`.
#[derive(Subcommand, Debug)]
pub enum QueryCommands {
    /// Execute tree-sitter S-expression queries against parsed ASTs
    Ast {
        /// Tree-sitter S-expression query pattern
        #[arg(long)]
        query: String,
        /// Language to parse files as (e.g. rust, python, typescript)
        #[arg(long)]
        language: String,
        /// File paths to query against
        #[arg(long)]
        files: Option<Vec<String>>,
        /// Maximum number of matches to return
        #[arg(long)]
        max_results: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// Find subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context find`.
#[derive(Subcommand, Debug)]
pub enum FindCommands {
    /// Find code in a file that is duplicated elsewhere in the codebase
    Duplicates {
        /// File to check for duplicated code
        #[arg(long)]
        file_path: String,
        /// Minimum cosine similarity to report as duplicate (0.0-1.0)
        #[arg(long)]
        min_similarity: Option<f64>,
        /// Maximum duplicates to show per source chunk
        #[arg(long)]
        max_per_chunk: Option<u64>,
        /// Minimum chunk size in bytes to consider
        #[arg(long)]
        min_chunk_bytes: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// Build subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context build`.
#[derive(Subcommand, Debug)]
pub enum BuildCommands {
    /// Mark files for re-indexing by resetting indexed flags
    Status {
        /// Which indexing layer to reset: treesitter, lsp, or both
        #[arg(long)]
        layer: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Clear subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context clear`.
#[derive(Subcommand, Debug)]
pub enum ClearCommands {
    /// Wipe all index data and return stats about what was cleared
    Status,
}

// ---------------------------------------------------------------------------
// Lsp subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context lsp`.
#[derive(Subcommand, Debug)]
pub enum LspCommands {
    /// Show detected languages, their LSP servers, and install status
    Status,
}

// ---------------------------------------------------------------------------
// Detect subcommands
// ---------------------------------------------------------------------------

/// Subcommands for `code-context detect`.
#[derive(Subcommand, Debug)]
pub enum DetectCommands {
    /// Detect project types in the workspace and return language-specific guidelines
    Projects {
        /// Root path to search for projects
        #[arg(long)]
        path: Option<String>,
        /// Maximum directory depth to search
        #[arg(long)]
        max_depth: Option<u64>,
        /// Include language-specific guidelines in output
        #[arg(long)]
        include_guidelines: Option<bool>,
    },
}

impl Cli {
    /// Parse CLI arguments, returning an error on failure instead of exiting.
    ///
    /// This is useful for testing and for `build.rs` which needs to introspect
    /// the command tree without actually running anything.
    #[allow(dead_code)]
    pub fn try_parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse args and return the `Commands` variant, panicking on failure.
    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["code-context"];
        full.extend_from_slice(args);
        Cli::try_parse_from_args(full).unwrap()
    }

    // -- Top-level help / version --

    #[test]
    fn help_displays_all_top_level_commands() {
        let err = Cli::try_parse_from_args(["code-context", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        // Every top-level command must appear
        for cmd in [
            "serve", "init", "deinit", "doctor", "skill", "get", "search", "list", "grep", "query",
            "find", "build", "clear", "lsp", "detect",
        ] {
            assert!(help.contains(cmd), "help missing command: {cmd}");
        }
    }

    #[test]
    fn get_help_displays_all_get_subcommands() {
        let err = Cli::try_parse_from_args(["code-context", "get", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        let help = err.to_string();
        for sub in [
            "symbol",
            "callgraph",
            "blastradius",
            "status",
            "definition",
            "type-definition",
            "hover",
            "references",
            "implementations",
            "code-actions",
            "inbound-calls",
            "rename-edits",
            "diagnostics",
        ] {
            assert!(help.contains(sub), "get --help missing subcommand: {sub}");
        }
    }

    // -- Global flags --

    #[test]
    fn global_debug_flag() {
        let cli = parse(&["--debug", "serve"]);
        assert!(cli.debug);
    }

    #[test]
    fn global_json_flag() {
        let cli = parse(&["--json", "serve"]);
        assert!(cli.json);
    }

    // -- Lifecycle commands --

    #[test]
    fn serve_command() {
        let cli = parse(&["serve"]);
        assert!(matches!(cli.command, Commands::Serve));
    }

    #[test]
    fn init_defaults_to_project() {
        let cli = parse(&["init"]);
        assert!(matches!(
            cli.command,
            Commands::Init {
                target: InstallTarget::Project
            }
        ));
    }

    #[test]
    fn init_user() {
        let cli = parse(&["init", "user"]);
        assert!(matches!(
            cli.command,
            Commands::Init {
                target: InstallTarget::User
            }
        ));
    }

    #[test]
    fn deinit_defaults_to_project() {
        let cli = parse(&["deinit"]);
        assert!(matches!(
            cli.command,
            Commands::Deinit {
                target: InstallTarget::Project
            }
        ));
    }

    #[test]
    fn doctor_verbose() {
        let cli = parse(&["doctor", "--verbose"]);
        assert!(matches!(cli.command, Commands::Doctor { verbose: true }));
    }

    #[test]
    fn skill_command() {
        let cli = parse(&["skill"]);
        assert!(matches!(cli.command, Commands::Skill));
    }

    // -- Get subcommands --

    #[test]
    fn get_symbol_parses() {
        let cli = parse(&["get", "symbol", "--query", "MyStruct::new"]);
        match cli.command {
            Commands::Get {
                command: GetCommands::Symbol { query, max_results },
            } => {
                assert_eq!(query, "MyStruct::new");
                assert_eq!(max_results, None);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_symbol_with_max_results() {
        let cli = parse(&["get", "symbol", "--query", "foo", "--max-results", "5"]);
        match cli.command {
            Commands::Get {
                command: GetCommands::Symbol { max_results, .. },
            } => assert_eq!(max_results, Some(5)),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_callgraph() {
        let cli = parse(&[
            "get",
            "callgraph",
            "--symbol",
            "process_request",
            "--direction",
            "inbound",
        ]);
        match cli.command {
            Commands::Get {
                command:
                    GetCommands::Callgraph {
                        symbol,
                        direction,
                        max_depth,
                    },
            } => {
                assert_eq!(symbol, "process_request");
                assert_eq!(direction.as_deref(), Some("inbound"));
                assert_eq!(max_depth, None);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_blastradius() {
        let cli = parse(&[
            "get",
            "blastradius",
            "--file-path",
            "src/server.rs",
            "--max-hops",
            "3",
        ]);
        match cli.command {
            Commands::Get {
                command:
                    GetCommands::Blastradius {
                        file_path,
                        symbol,
                        max_hops,
                    },
            } => {
                assert_eq!(file_path, "src/server.rs");
                assert!(symbol.is_none());
                assert_eq!(max_hops, Some(3));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_status() {
        let cli = parse(&["get", "status"]);
        assert!(matches!(
            cli.command,
            Commands::Get {
                command: GetCommands::Status
            }
        ));
    }

    #[test]
    fn get_definition() {
        let cli = parse(&[
            "get",
            "definition",
            "--file-path",
            "src/main.rs",
            "--line",
            "10",
            "--character",
            "5",
        ]);
        match cli.command {
            Commands::Get {
                command:
                    GetCommands::Definition {
                        file_path,
                        line,
                        character,
                    },
            } => {
                assert_eq!(file_path, "src/main.rs");
                assert_eq!(line, 10);
                assert_eq!(character, 5);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_hover() {
        let cli = parse(&[
            "get",
            "hover",
            "--file-path",
            "lib.rs",
            "--line",
            "0",
            "--character",
            "0",
        ]);
        assert!(matches!(
            cli.command,
            Commands::Get {
                command: GetCommands::Hover { .. }
            }
        ));
    }

    #[test]
    fn get_references() {
        let cli = parse(&[
            "get",
            "references",
            "--file-path",
            "f.rs",
            "--line",
            "1",
            "--character",
            "2",
            "--include-declaration",
            "false",
        ]);
        match cli.command {
            Commands::Get {
                command:
                    GetCommands::References {
                        include_declaration,
                        ..
                    },
            } => assert_eq!(include_declaration, Some(false)),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_code_actions() {
        let cli = parse(&[
            "get",
            "code-actions",
            "--file-path",
            "f.rs",
            "--start-line",
            "1",
            "--start-character",
            "0",
            "--end-line",
            "5",
            "--end-character",
            "10",
        ]);
        match cli.command {
            Commands::Get {
                command:
                    GetCommands::CodeActions {
                        start_line,
                        end_line,
                        ..
                    },
            } => {
                assert_eq!(start_line, 1);
                assert_eq!(end_line, 5);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_inbound_calls() {
        let cli = parse(&[
            "get",
            "inbound-calls",
            "--file-path",
            "f.rs",
            "--line",
            "3",
            "--character",
            "7",
            "--depth",
            "2",
        ]);
        match cli.command {
            Commands::Get {
                command: GetCommands::InboundCalls { depth, .. },
            } => assert_eq!(depth, Some(2)),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_rename_edits() {
        let cli = parse(&[
            "get",
            "rename-edits",
            "--file-path",
            "f.rs",
            "--line",
            "1",
            "--character",
            "4",
            "--new-name",
            "better_name",
        ]);
        match cli.command {
            Commands::Get {
                command: GetCommands::RenameEdits { new_name, .. },
            } => assert_eq!(new_name, "better_name"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn get_diagnostics() {
        let cli = parse(&[
            "get",
            "diagnostics",
            "--file-path",
            "f.rs",
            "--severity-filter",
            "error",
        ]);
        match cli.command {
            Commands::Get {
                command:
                    GetCommands::Diagnostics {
                        severity_filter, ..
                    },
            } => assert_eq!(severity_filter.as_deref(), Some("error")),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- Search subcommands --

    #[test]
    fn search_symbol() {
        let cli = parse(&[
            "search", "symbol", "--query", "handler", "--kind", "function",
        ]);
        match cli.command {
            Commands::Search {
                command: SearchCommands::Symbol { query, kind, .. },
            } => {
                assert_eq!(query, "handler");
                assert_eq!(kind.as_deref(), Some("function"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn search_code() {
        let cli = parse(&["search", "code", "--query", "auth handler", "--top-k", "5"]);
        match cli.command {
            Commands::Search {
                command: SearchCommands::Code { query, top_k, .. },
            } => {
                assert_eq!(query, "auth handler");
                assert_eq!(top_k, Some(5));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn search_workspace_symbol() {
        let cli = parse(&["search", "workspace-symbol", "--query", "Config"]);
        match cli.command {
            Commands::Search {
                command: SearchCommands::WorkspaceSymbol { query, .. },
            } => assert_eq!(query, "Config"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- List subcommands --

    #[test]
    fn list_symbols() {
        let cli = parse(&["list", "symbols", "--file-path", "src/main.rs"]);
        match cli.command {
            Commands::List {
                command: ListCommands::Symbols { file_path },
            } => assert_eq!(file_path, "src/main.rs"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- Grep subcommands --

    #[test]
    fn grep_code() {
        let cli = parse(&[
            "grep",
            "code",
            "--pattern",
            "TODO|FIXME",
            "--language",
            "rs",
        ]);
        match cli.command {
            Commands::Grep {
                command:
                    GrepCommands::Code {
                        pattern, language, ..
                    },
            } => {
                assert_eq!(pattern, "TODO|FIXME");
                assert_eq!(language, Some(vec!["rs".to_string()]));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- Query subcommands --

    #[test]
    fn query_ast() {
        let cli = parse(&[
            "query",
            "ast",
            "--query",
            "(function_item)",
            "--language",
            "rust",
        ]);
        match cli.command {
            Commands::Query {
                command:
                    QueryCommands::Ast {
                        query, language, ..
                    },
            } => {
                assert_eq!(query, "(function_item)");
                assert_eq!(language, "rust");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- Find subcommands --

    #[test]
    fn find_duplicates() {
        let cli = parse(&[
            "find",
            "duplicates",
            "--file-path",
            "src/handlers.rs",
            "--min-similarity",
            "0.85",
        ]);
        match cli.command {
            Commands::Find {
                command:
                    FindCommands::Duplicates {
                        file_path,
                        min_similarity,
                        ..
                    },
            } => {
                assert_eq!(file_path, "src/handlers.rs");
                assert!((min_similarity.unwrap() - 0.85).abs() < f64::EPSILON);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- Build subcommands --

    #[test]
    fn build_status() {
        let cli = parse(&["build", "status", "--layer", "treesitter"]);
        match cli.command {
            Commands::Build {
                command: BuildCommands::Status { layer },
            } => assert_eq!(layer.as_deref(), Some("treesitter")),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- Clear subcommands --

    #[test]
    fn clear_status() {
        let cli = parse(&["clear", "status"]);
        assert!(matches!(
            cli.command,
            Commands::Clear {
                command: ClearCommands::Status
            }
        ));
    }

    // -- Lsp subcommands --

    #[test]
    fn lsp_status() {
        let cli = parse(&["lsp", "status"]);
        assert!(matches!(
            cli.command,
            Commands::Lsp {
                command: LspCommands::Status
            }
        ));
    }

    // -- Detect subcommands --

    #[test]
    fn detect_projects() {
        let cli = parse(&[
            "detect",
            "projects",
            "--path",
            "/tmp",
            "--max-depth",
            "3",
            "--include-guidelines",
            "true",
        ]);
        match cli.command {
            Commands::Detect {
                command:
                    DetectCommands::Projects {
                        path,
                        max_depth,
                        include_guidelines,
                    },
            } => {
                assert_eq!(path.as_deref(), Some("/tmp"));
                assert_eq!(max_depth, Some(3));
                assert_eq!(include_guidelines, Some(true));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    // -- Install target display --

    #[test]
    fn install_target_display() {
        assert_eq!(InstallTarget::Project.to_string(), "project");
        assert_eq!(InstallTarget::Local.to_string(), "local");
        assert_eq!(InstallTarget::User.to_string(), "user");
    }
}
