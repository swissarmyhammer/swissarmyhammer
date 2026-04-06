# LSP Ubertool: New Live Operations for code-context

Design for new operations that make live LSP requests through existing daemons,
enriched with indexed database context. These turn SwissArmyHammer into a
complete code intelligence layer — not just an indexer, but an interactive query
engine that combines live LSP precision with indexed breadth.

## Context

Comparison with Claude Code's LSP Tool informed this design. Claude Code has a
rich interactive LSP tool (9 operations) but is purely stateless
request/response. SwissArmyHammer already has LSP daemons running and a
JSON-RPC client, but only uses `documentSymbol` and `outgoingCalls` for batch
indexing. These new ops close the gap and go beyond.

## Shared Infrastructure Change

All existing ops take `conn: &Connection`. Live ops need an LSP client too:

```rust
/// Parameter type for ops that need live LSP access
pub struct LspContext<'a> {
    pub conn: &'a Connection,
    pub client: &'a SharedLspClient,  // Arc<Mutex<Option<LspJsonRpcClient>>>
}
```

Database-only ops keep their existing signatures. New live ops take `LspContext`
so they can query the database AND make live LSP requests.

---

## Operations

### 1. `get_definition` — Go to Definition

**File:** `ops/get_definition.rs`
**LSP method:** `textDocument/definition`

```rust
pub struct GetDefinitionOptions {
    pub file_path: String,      // absolute path
    pub line: u32,              // 0-indexed
    pub character: u32,         // 0-indexed
    pub include_source: bool,   // read source text at target (default true)
}

pub struct DefinitionResult {
    pub origins: Vec<DefinitionLocation>,
}

pub struct DefinitionLocation {
    pub file_path: String,
    pub range: LspRange,
    pub source_text: Option<String>,       // lines around the definition
    pub symbol: Option<SymbolLocation>,    // from index, if we have it
}
```

**Database enrichment:** After getting the location, look up the symbol in
`lsp_symbols` at that position to attach qualified path, kind, and detail. Read
source lines from disk for context.

---

### 2. `get_references` — Find All References

**LSP method:** `textDocument/references`

```rust
pub struct GetReferencesOptions {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
    pub include_declaration: bool,  // default true
    pub max_results: usize,         // default 100
}

pub struct ReferencesResult {
    pub references: Vec<ReferenceLocation>,
    pub total_count: usize,
    pub by_file: Vec<FileReferenceGroup>,  // grouped for readability
}

pub struct ReferenceLocation {
    pub file_path: String,
    pub range: LspRange,
    pub containing_symbol: Option<String>,  // from index: which function contains this ref
}
```

**Database enrichment:** For each reference location, query `lsp_symbols` to
find the enclosing symbol. This tells you not just *where* a thing is referenced
but *what function* references it — far more useful than raw locations.

---

### 3. `get_hover` — Type Info & Documentation

**LSP method:** `textDocument/hover`

```rust
pub struct GetHoverOptions {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
}

pub struct HoverResult {
    pub contents: String,           // markdown from LSP
    pub range: Option<LspRange>,    // symbol range the hover applies to
    pub symbol: Option<SymbolLocation>,  // from index
}
```

**Database enrichment:** Attach indexed symbol metadata if available. Simplest
op — mostly a passthrough.

---

### 4. `get_implementations` — Find Implementations of Trait/Interface

**LSP method:** `textDocument/implementation`

```rust
pub struct GetImplementationsOptions {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
    pub max_results: usize,
}

pub struct ImplementationsResult {
    pub implementations: Vec<ImplementationLocation>,
}

pub struct ImplementationLocation {
    pub file_path: String,
    pub range: LspRange,
    pub symbol: Option<SymbolLocation>,  // from index
    pub source_text: Option<String>,
}
```

**Database enrichment:** Attach indexed symbol info at each location.

---

### 5. `get_inbound_calls` — Who Calls This?

Currently SwissArmyHammer only indexes *outgoing* calls. This adds live
incoming call lookup.

**LSP methods:** `textDocument/prepareCallHierarchy` then `callHierarchy/incomingCalls`

```rust
pub struct GetInboundCallsOptions {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
    pub depth: u32,             // default 1, max 5
}

pub struct InboundCallsResult {
    pub target: String,                    // the function being called
    pub callers: Vec<InboundCallEntry>,
}

pub struct InboundCallEntry {
    pub symbol_name: String,
    pub file_path: String,
    pub range: LspRange,
    pub call_sites: Vec<LspRange>,  // where in the caller the call happens
    pub depth: u32,
    pub callers: Vec<InboundCallEntry>,  // recursive for depth > 1
}
```

**Database enrichment:** Cross-reference with existing `lsp_call_edges` to fill
gaps — the index has outbound edges that can validate/supplement the live
results.

---

### 6. `get_diagnostics` — Errors & Warnings for a File

**LSP method:** `textDocument/publishDiagnostics` (push) or `textDocument/diagnostic` (pull, LSP 3.17+)

```rust
pub struct GetDiagnosticsOptions {
    pub file_path: String,
    pub severity_filter: Option<Vec<DiagnosticSeverity>>,
}

pub enum DiagnosticSeverity { Error, Warning, Info, Hint }

pub struct DiagnosticsResult {
    pub diagnostics: Vec<Diagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
}

pub struct Diagnostic {
    pub range: LspRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: Option<String>,
    pub source: Option<String>,          // e.g. "rustc", "clippy"
    pub containing_symbol: Option<String>, // from index
}
```

**Implementation note:** Simplest approach is: send `didOpen`, wait briefly for
diagnostics notification, collect, return. The daemon already receives these —
just need to surface them.

**Database enrichment:** Map each diagnostic range to its enclosing symbol from
the index.

---

### 7. `workspace_symbol_live` — Live Workspace Symbol Search

**LSP method:** `workspace/symbol`

```rust
pub struct WorkspaceSymbolLiveOptions {
    pub query: String,
    pub max_results: usize,       // default 50
}

pub struct WorkspaceSymbolLiveResult {
    pub symbols: Vec<SymbolLocation>,  // reuse existing type
}
```

**Why alongside existing `search_symbol`?** The indexed search is faster and
works offline, but the live LSP query can find symbols the indexer hasn't
reached yet, or symbols in files that changed since last index. This is a
fallback/supplement, not a replacement.

---

### 8. `get_type_definition` — Go to Type

**LSP method:** `textDocument/typeDefinition`

```rust
pub struct GetTypeDefinitionOptions {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
}

pub struct TypeDefinitionResult {
    pub locations: Vec<DefinitionLocation>,  // reuse from get_definition
}
```

**Use case:** You're on a variable, you want the type's definition, not the
variable's definition. Critical for navigating generic/trait-heavy code.

---

### 9. `get_rename_edits` — Preview a Rename

**LSP methods:** `textDocument/prepareRename` (validate) then `textDocument/rename` (compute edits)

```rust
pub struct GetRenameEditsOptions {
    pub file_path: String,
    pub line: u32,
    pub character: u32,
    pub new_name: String,
}

pub struct RenameEditsResult {
    pub can_rename: bool,
    pub edits: Vec<FileEdit>,         // grouped by file
    pub files_affected: usize,
}

pub struct FileEdit {
    pub file_path: String,
    pub text_edits: Vec<TextEdit>,
}

pub struct TextEdit {
    pub range: LspRange,
    pub new_text: String,
}
```

**Note:** This only *computes* edits, doesn't apply them. The caller decides
whether to apply. Extremely powerful refactoring primitive for an AI tool.

---

### 10. `get_code_actions` — Available Fixes & Refactors

**LSP method:** `textDocument/codeAction` (optionally `codeAction/resolve`)

```rust
pub struct GetCodeActionsOptions {
    pub file_path: String,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
    pub filter_kind: Option<Vec<String>>,  // "quickfix", "refactor", "source"
}

pub struct CodeActionsResult {
    pub actions: Vec<CodeAction>,
}

pub struct CodeAction {
    pub title: String,
    pub kind: Option<String>,
    pub edits: Option<Vec<FileEdit>>,   // resolved edits if available
    pub is_preferred: bool,
}
```

**Use case:** "What can the language server suggest for this code?" —
auto-imports, extract function, inline variable, apply clippy suggestion, etc.
Turns the LSP into a refactoring engine.

---

## Capability Matrix

| Op | LSP Method | DB Enrichment | Goes Beyond Claude Code |
|---|---|---|---|
| `get_definition` | definition | symbol metadata | No |
| `get_references` | references | enclosing symbol | No |
| `get_hover` | hover | symbol metadata | No |
| `get_implementations` | implementation | symbol metadata | No |
| `get_inbound_calls` | prepareCallHierarchy + incomingCalls | cross-ref call_edges | No |
| `get_diagnostics` | publishDiagnostics | enclosing symbol | No |
| `workspace_symbol_live` | workspace/symbol | merge with index | No |
| `get_type_definition` | typeDefinition | symbol metadata | **Yes** |
| `get_rename_edits` | prepareRename + rename | — | **Yes** |
| `get_code_actions` | codeAction | — | **Yes** |

The first 7 give parity with Claude Code's LSP Tool. The last 3 go beyond it —
Claude Code doesn't expose rename, code actions, or type definition.

The consistent database enrichment (attaching indexed symbol context to live
results) is what makes this more than just an LSP passthrough — it's the
combination of live precision with indexed breadth.

---

## Implementation Priority

**Tier 1** (highest value, simplest — straightforward request/response):
- `get_definition`
- `get_references`
- `get_hover`

**Tier 2** (high value — completes the navigation story):
- `get_diagnostics`
- `get_inbound_calls`
- `get_implementations`

**Tier 3** (power features — goes beyond read-only into refactoring):
- `get_rename_edits`
- `get_code_actions`
- `get_type_definition`
- `workspace_symbol_live`
