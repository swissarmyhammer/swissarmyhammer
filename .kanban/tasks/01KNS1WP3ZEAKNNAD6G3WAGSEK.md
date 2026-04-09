---
assignees:
- claude-code
depends_on:
- 01KNS1TQR2C3TYG1G8STEYZPA5
position_column: todo
position_ordinal: '8380'
project: code-context-cli
title: Implement CLI definition (clap structure) for code-context
---
## What
Create `code-context-cli/src/cli.rs` with the full clap CLI definition, mirroring `shelltool-cli/src/cli.rs`.

The CLI must be self-contained (only `clap` and `std` deps) so `build.rs` can compile it via `#[path = "src/cli.rs"]`.

### Structs and enums to create:

**`InstallTarget`** (same as shelltool — `Project`, `Local`, `User` with `ValueEnum`)

**`Cli`**:
- `#[command(name = "code-context")]`
- `#[command(about = "Code intelligence MCP server for AI agents")]`
- `debug: bool` flag (global)
- `json: bool` flag — output as JSON (global, for operation commands)
- `command: Commands`

**`Commands`** enum:
- `Serve` — run MCP server over stdio
- `Init { target: InstallTarget }` — install into agent settings
- `Deinit { target: InstallTarget }` — remove from agent settings
- `Doctor { verbose: bool }` — diagnose setup
- `Skill` — deploy code-context skill to agent .skills/ directories

**Operation subcommands** — using nested clap subcommand groups matching the `verb noun` pattern:
- `Get(GetCommands)` — with sub-enum `GetCommands`: Symbol, Callgraph, Blastradius, Status, Definition, TypeDefinition, Hover, References, Implementations, CodeActions, InboundCalls, RenameEdits, Diagnostics
- `Search(SearchCommands)` — Symbol, Code, WorkspaceSymbol
- `List(ListCommands)` — Symbols
- `Grep(GrepCommands)` — Code
- `Query(QueryCommands)` — Ast
- `Find(FindCommands)` — Duplicates
- `Build(BuildCommands)` — Status
- `Clear(ClearCommands)` — Status
- `Lsp(LspCommands)` — Status
- `Detect(DetectCommands)` — Projects

Each operation variant carries its own named fields matching the MCP parameter names:
- `GetSymbol { query: String, max_results: Option<u64> }`
- `SearchSymbol { query: String, kind: Option<String>, max_results: Option<u64> }`
- `ListSymbols { file_path: String }`
- `GrepCode { pattern: String, language: Option<Vec<String>>, max_results: Option<u64> }`
- `GetCallgraph { symbol: String, direction: Option<String>, max_depth: Option<u64> }`
- `GetBlastradius { file_path: String, symbol: Option<String>, max_hops: Option<u64> }`
- `GetStatus` (no args)
- `BuildStatus { layer: Option<String> }`
- `ClearStatus` (no args)
- `LspStatus` (no args)
- `DetectProjects { path: Option<String>, max_depth: Option<u64>, include_guidelines: Option<bool> }`
- `SearchCode { query: String, top_k: Option<u64>, min_similarity: Option<f64> }`
- `FindDuplicates { file_path: String, min_similarity: Option<f64>, max_per_chunk: Option<u64> }`
- `QueryAst { query: String, language: String, max_results: Option<u64> }`
- `SearchWorkspaceSymbol { query: String, max_results: Option<u64> }`
- `GetDefinition { file_path: String, line: u64, character: u64 }`
- `GetTypeDefinition { file_path: String, line: u64, character: u64 }`
- `GetHover { file_path: String, line: u64, character: u64 }`
- `GetReferences { file_path: String, line: u64, character: u64, include_declaration: Option<bool> }`
- `GetImplementations { file_path: String, line: u64, character: u64 }`
- `GetCodeActions { file_path: String, start_line: u64, start_character: u64, end_line: u64, end_character: u64 }`
- `GetInboundCalls { file_path: String, line: u64, character: u64, depth: Option<u64> }`
- `GetRenameEdits { file_path: String, line: u64, character: u64, new_name: String }`
- `GetDiagnostics { file_path: String, severity_filter: Option<String> }`

## Acceptance Criteria
- [ ] `cargo check -p code-context-cli` passes
- [ ] `code-context --help` shows all top-level commands
- [ ] `code-context get --help` shows all `get` subcommands
- [ ] `code-context get symbol --query foo` parses correctly

## Tests
- [ ] No tests needed for cli.rs (clap's own derive tests are sufficient)
- [ ] `cargo check -p code-context-cli` must pass clean

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.