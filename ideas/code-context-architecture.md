# `swissarmyhammer-code-context` Architecture

A new crate that fuses the existing tree-sitter layer with a new LSP call graph layer into a single unified `code_context` MCP tool.

---

## Workspace layout

Everything lives under `.code-context/` in the project root, which is auto-gitignored on first startup.

```
{workspace_root}/
  .code-context/
    index.db        ← unified SQLite database (WAL mode)
    leader.lock     ← flock-based leader election file
    leader.sock     ← Unix socket for leader/reader RPC
    .gitignore      ← auto-generated, contains "*"
```

The existing `.treesitter-index.db` file gets migrated into `index.db` in a follow-up PR. For now they coexist and `code_context` opens both.

---

## Database schema

All tables live in `index.db`. Foreign keys with `ON DELETE CASCADE` mean deleting a row from `indexed_files` automatically removes all associated chunks, symbols, and call edges.

### `indexed_files` — ground truth file registry

```sql
CREATE TABLE indexed_files (
    file_path     TEXT PRIMARY KEY,
    content_hash  BLOB NOT NULL,      -- MD5, 16 bytes
    file_size     INTEGER NOT NULL,
    last_seen_at  INTEGER NOT NULL,   -- unixepoch(), updated every startup scan
    ts_indexed    INTEGER NOT NULL DEFAULT 0,   -- 1 = tree-sitter layer done
    lsp_indexed   INTEGER NOT NULL DEFAULT 0    -- 1 = LSP layer done
);
```

This table is the authoritative record of what belongs in the index. Both layers read from it to find work and write back to it when they finish a file.

### `ts_chunks` — tree-sitter semantic chunks

```sql
CREATE TABLE ts_chunks (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path    TEXT NOT NULL REFERENCES indexed_files(file_path) ON DELETE CASCADE,
    start_byte   INTEGER NOT NULL,
    end_byte     INTEGER NOT NULL,
    start_line   INTEGER NOT NULL,
    end_line     INTEGER NOT NULL,
    text         TEXT NOT NULL,
    symbol_path  TEXT,           -- e.g. "MyStruct::my_method"
    embedding    BLOB            -- f32 LE bytes, NULL until embedded
);
CREATE INDEX idx_ts_chunks_file ON ts_chunks(file_path);
```

### `lsp_symbols` — function/method symbols from the language server

```sql
CREATE TABLE lsp_symbols (
    id           TEXT PRIMARY KEY,   -- "lsp:{file_path}:{qualified_path}" e.g. "lsp:src/auth.rs:auth::AuthService::new"
    name         TEXT NOT NULL,
    kind         INTEGER NOT NULL,   -- LSP SymbolKind enum
    file_path    TEXT NOT NULL REFERENCES indexed_files(file_path) ON DELETE CASCADE,
    start_line   INTEGER NOT NULL,
    start_char   INTEGER NOT NULL,
    end_line     INTEGER NOT NULL,
    end_char     INTEGER NOT NULL,
    detail       TEXT
);
CREATE INDEX idx_lsp_symbols_file ON lsp_symbols(file_path);
```

### `lsp_call_edges` — directed call graph edges

```sql
CREATE TABLE lsp_call_edges (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    caller_id    TEXT NOT NULL REFERENCES lsp_symbols(id) ON DELETE CASCADE,
    callee_id    TEXT NOT NULL REFERENCES lsp_symbols(id) ON DELETE CASCADE,
    caller_file  TEXT NOT NULL,
    callee_file  TEXT NOT NULL,
    from_ranges  TEXT NOT NULL,   -- JSON: [{start, end}] call site locations
    source       TEXT NOT NULL DEFAULT 'lsp'  -- 'lsp' or 'treesitter'
);

-- Forward: "what does file F call?" — used when re-extracting F
CREATE INDEX idx_edges_caller_file ON lsp_call_edges(caller_file);

-- Reverse: "who calls into file F?" — used for 1-hop dirty propagation
CREATE INDEX idx_edges_callee_file ON lsp_call_edges(callee_file);
```

The reverse index (`idx_edges_callee_file`) is what makes efficient blast radius queries and incremental invalidation possible.

### Edge provenance

| `source` | Meaning |
|---|---|
| `lsp` | LSP call hierarchy — compiler-quality |
| `treesitter` | Heuristic: tree-sitter matched a call-site node name against a known symbol. May be wrong (shadowing, dynamic dispatch) |

No confidence scores — either the edge came from the compiler or from a heuristic. The provenance is surfaced in results so the agent knows what it's looking at, but we don't pretend to quantify certainty with a float.

### Tree-sitter call graph heuristic

When no LSP is available (unsupported language, server not installed, server failed), `get callgraph` falls back to a tree-sitter heuristic to produce `source: 'treesitter'` edges. The algorithm:

1. For each chunk in `ts_chunks`, parse with tree-sitter and walk the AST for `call_expression` (or language-equivalent) nodes.
2. Extract the callee name from each call node (e.g., `foo()` → `"foo"`, `self.bar()` → `"bar"`, `MyStruct::new()` → `"MyStruct::new"`).
3. Look up each callee name against all known `symbol_path` values in `ts_chunks`.
4. If a match is found, insert an edge from the calling chunk's symbol to the matched symbol.

**Known limitations** (which is why these edges are tagged `treesitter`, not `lsp`):
- Name collisions: `foo()` in file A might match a different `foo` than the one actually called.
- Dynamic dispatch: `trait_object.method()` can't be resolved without type info.
- Qualified paths: `crate::module::func()` requires understanding module structure that tree-sitter doesn't have.
- Cross-language calls: not attempted.

Despite these limitations, the heuristic is useful — in most codebases, function names are unique enough that simple name matching produces a usable (if noisy) call graph. The agent sees `source: 'treesitter'` and knows to treat it as approximate.

### Next-step hints in tool responses

Every tool response includes a short `hint` field suggesting the logical next operation. This teaches the agent the workflow without requiring the skill document to be loaded:

- `grep code` → *"Use `get symbol` to see the full function, or `get callgraph` to see callers."*
- `find symbol` → *"Use `get symbol` for source text, or `get callgraph` for call relationships."*
- `get callgraph` → *"Use `get blastradius` for full impact analysis."*
- `git get diff` → *"Review modified symbols with `get symbol`, or check callers with `get callgraph`."*

Hints are cheap to implement (static string per operation) and help the agent chain operations effectively.

---

## Leader election

Uses the existing `swissarmyhammer-leader-election` crate pointed at `.code-context/leader.lock`.

**Leader** — the first process to acquire the lock. Responsible for:
- Running startup cleanup
- Writing to the database
- Running background indexers (TS and LSP)
- Holding the file watcher

**Readers** — every other process. Open the database read-only and query whatever the leader has indexed so far. Readers never run watchers or write anything.

This is the same pattern as the existing tree-sitter `Workspace` — the leader/reader split is well-understood and proven in that codebase.

---

## Startup: stale entry cleanup

The watcher only fires for changes that happen while the process is running. If files are deleted, renamed, or modified while the process is stopped, the database has stale entries that will never be cleaned up by the watcher alone.

The leader runs a file-set diff immediately after acquiring the lock, before starting any background indexers:

1. **Walk the filesystem** — collect every file path + MD5 hash, respecting `.gitignore` (using the `ignore` crate). Hashing is parallelized via `rayon` since this is the slow step on large repos.

2. **Query the database** — fetch all `(file_path, content_hash)` rows from `indexed_files`.

3. **Delete stale entries** — files in the DB but not on disk. The `ON DELETE CASCADE` foreign keys clean up all associated chunks, symbols, and call edges automatically.

4. **Mark changed files dirty** — files whose MD5 no longer matches. This means deleting their `ts_chunks` and `lsp_symbols` rows and clearing their `ts_indexed`/`lsp_indexed` flags so background indexers pick them up.

5. **Upsert the current file set** — insert new files, update `last_seen_at` for survivors.

This whole pass happens synchronously before the watcher starts. The indexers then see a clean, accurate `indexed_files` table with no stale entries and correct dirty flags.

One optimization worth adding later: skip re-hashing files whose `last_seen_at` matches their filesystem `mtime`. That would reduce the startup hash-everything cost to near zero on subsequent runs.

---

## Shared file watcher

The existing `WorkspaceWatcher` accepts exactly one `WorkspaceWatcherCallback`. Rather than running multiple watchers (wasteful, and causes duplicate inotify/FSEvents subscriptions), we run one watcher with a fanout callback that broadcasts to multiple handlers.

```
WorkspaceWatcher
    └── FanoutWatcherCallback
            ├── TsWatcherHandler   → triggers tree-sitter re-index
            └── LspWatcherHandler  → notifies LSP server + marks dirty
```

When any file changes, `FanoutWatcherCallback` also writes directly to the `indexed_files` table — clearing `ts_indexed`/`lsp_indexed` flags for the changed paths. This means dirty state is always durable in the DB, not only in-memory. If the process crashes mid-reindex, the next startup will see the dirty flags and pick up where it left off.

Adding a new consumer (e.g., a future community detection layer) is just adding another handler to the fanout.

---

## LSP layer: incremental invalidation

When a file changes, the LSP call graph needs to stay consistent. The invalidation strategy is deliberately bounded at one hop to keep recompute work proportional to the change.

### What happens when file F changes

1. **Remove F's symbols and outgoing edges** from the database.
2. **Re-extract F**: query the language server for its current symbols, then walk the call hierarchy to get fresh outgoing edges.
3. **Diff symbols**: compare old symbol IDs against new ones to find any that were deleted or renamed.
4. **1-hop propagation**: look up the reverse edge index to find files G that had call edges pointing into F's deleted symbols. For each such G, re-query its outgoing edges only — no need to re-extract its symbols, since G's source code hasn't changed.
5. **Stop**. No further propagation.

### Why stop at one hop

After step 4, G's edges are fresh. G's symbol set is unchanged (G's source didn't change). G's callers only care about whether G's symbols exist and where they are — which is unchanged. So there's nothing for G's callers to update.

The exception would be if G's *signature* changed in a way that breaks callers — but signature changes are caught by a file change on G itself, which starts a fresh generation-0 recompute for G, not a propagation from F.

### Two recompute paths

There are two distinct recompute jobs, not one. This is what makes the 1-hop bound work:

- **Full re-extract** — generation 0. Triggered for the directly changed file. The file watcher flips `ts_indexed = 0` and `lsp_indexed = 0` on the file row; the worker loop then re-runs symbol extraction and the call-hierarchy pass against fresh content.
- **`RefreshEdges`** — generation 1. Triggered for dependent files whose callees were renamed or deleted during generation 0. The invalidation engine emits a `RefreshEdges` action for each dependent; the worker applies it by flipping only `lsp_indexed = 0`, so the next pass re-queries outgoing call edges while leaving tree-sitter data intact. About 10x cheaper than a full re-extract since the symbol set is unchanged.

`RefreshEdges` never triggers further propagation, which closes the loop.

---

## The `code_context` MCP tool

Operations follow the `{verb} {noun}` pattern. The `op` field takes the full string, e.g. `"search code"`.

### Design decision: no BM25

We deliberately skip BM25 full-text indexing. At single-repo scale (tens of thousands of chunks, not millions), BM25 ranking adds complexity for negligible practical benefit over ripgrep, which saturates all cores and returns in milliseconds. We cover the two cases that matter:

- **Exact keyword match** → `grep code` — ripgrep against stored chunk text. Fast, precise, zero indexing overhead.
- **Conceptual/fuzzy match** → `search code` — embeddings + cosine similarity. Handles "login" ≈ "authenticate".

BM25 sits in an awkward middle ground — slower than ripgrep for exact terms, worse than embeddings for semantic meaning. The AI agent calling us can easily scan 20-50 ripgrep hits without needing statistical ranking.

### Design decision: chunks store full text

Every `ts_chunks` row keeps the full source text of its semantic block (function, struct, impl block, etc.). This means:

- `grep code` and `search code` return complete, self-contained code blocks — not line fragments requiring a follow-up file read.
- `get symbol` can return the code for a symbol without the caller knowing which file it lives in.
- Embeddings are generated from this stored text.
- The storage cost is modest — it's a copy of the source, deduplicated by content hash via `indexed_files`.

### Operation matrix

| | `code` | `ast` | `duplicates` | `callgraph` | `symbol` | `blastradius` | `status` |
|---|---|---|---|---|---|---|---|
| `search` | ✅ TS | | | | ✅ LSP | | |
| `grep` | ✅ TS | | | | | | |
| `query` | | ✅ TS | | | | | |
| `find` | | | ✅ TS | | ✅ LSP | | |
| `list` | | | | | ✅ TS/LSP | | |
| `get` | | | | ✅ LSP | ✅ TS/LSP | ✅ LSP | ✅ both |
| `build` | | | | | | | ✅ both |
| `clear` | | | | | | | ✅ both |

Note: `detect changes` lives on the **`git` tool**, not here. It uses `sem-core` for entity diffing and queries `code_context`'s call graph for fan-out. See the `git` tool section below.

**TS** = tree-sitter layer, always available. **LSP** = requires a language server. **both** = operates on both layers.

### Blocking during initial index

All query operations (`grep code`, `search code`, `get symbol`, etc.) **block until the relevant layer is fully indexed**, rather than returning partial results. During the wait, the tool returns a progress notification so the calling agent knows what's happening:

```
Indexing in progress: tree-sitter 412/847 files (49%)... waiting.
```

This avoids the subtle bug where an agent searches, gets zero results, and concludes the symbol doesn't exist — when it actually just hasn't been indexed yet. The tradeoff (latency on first call) is acceptable because:
- Tree-sitter indexing is fast (seconds, not minutes).
- LSP indexing is slower but only blocks LSP-dependent operations. Tree-sitter operations return immediately.
- Subsequent calls after indexing completes are instant.

`get status` is the exception — it always returns immediately with current progress, even mid-index. This lets the agent check readiness without blocking.

---

### `grep code`
Ripgrep-powered keyword search across all stored chunk text. Returns complete semantic blocks (whole functions, structs, impl blocks) containing the match — not line fragments. This is the structured alternative to raw `rg`: same speed, but results are code-aware.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `pattern` | string | yes | | Regex pattern (ripgrep syntax) |
| `language` | string | | all | Filter to a specific language |
| `files` | array | | all | Limit to specific file paths |
| `max_results` | integer | | 20 | Cap on returned chunks |
| `path` | string | | cwd | Workspace root |

Implementation: compiles the pattern once with the `regex` crate (the same engine ripgrep uses internally, with SIMD-accelerated `memchr` and Aho-Corasick literal optimizations). Loads chunk text from `ts_chunks` in SQLite, then `rayon::par_iter` across chunks to test matches in parallel. Returns matching chunks with file path, line range, symbol path, and highlighted match positions. No subprocess, no file I/O — the text is already in the DB.

---

### `search code`
Semantic similarity search across all indexed chunks using embeddings.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `query` | string | yes | | Text or code snippet to search for |
| `top_k` | integer | | 10 | Max results to return |
| `min_similarity` | number | | 0.7 | Cosine similarity threshold 0.0–1.0 |
| `path` | string | | cwd | Workspace root |

---

### `query ast`
Execute a tree-sitter S-expression query against the parsed AST.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `query` | string | yes | | S-expression pattern, e.g. `(function_item name: (identifier) @name)` |
| `language` | string | | all | Filter to a specific language, e.g. `rust`, `typescript` |
| `files` | array | | all | Limit to specific file paths |
| `path` | string | | cwd | Workspace root |

---

### `find duplicates`
Detect clusters of semantically similar code chunks across the workspace.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `min_similarity` | number | | 0.85 | Cosine similarity threshold 0.0–1.0 |
| `min_chunk_bytes` | integer | | 100 | Ignore chunks smaller than this |
| `file` | string | | all | Limit to duplicates of chunks in a specific file |
| `path` | string | | cwd | Workspace root |

---

### `get callgraph`
Incoming or outgoing call edges for a symbol. Backed by LSP call hierarchy; falls back to tree-sitter heuristic if LSP is not indexed.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `symbol` | string | yes | | Symbol name or `file:line:char` position |
| `direction` | string | yes | | `inbound`, `outbound`, or `both` — no default, must be explicit |
| `depth` | integer | | 1 | How many hops to traverse, 1–5 |
| `path` | string | | cwd | Workspace root |

---

### `find symbol`
Look up the definition location of a symbol by name.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `symbol` | string | yes | | Symbol name to find |
| `path` | string | | cwd | Workspace root |

---

### `search symbol`
Fuzzy/prefix search across all symbols in the workspace. Equivalent to `workspace/symbol` in LSP.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `query` | string | yes | | Name prefix or fuzzy pattern |
| `kind` | string | | all | Filter by symbol kind: `function`, `method`, `struct`, `class`, `interface` |
| `path` | string | | cwd | Workspace root |

---

### `list symbol`
All symbols defined in a specific file. Uses the tree-sitter layer (fast, no LSP needed); enriches with LSP type detail if available.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `file` | string | yes | | File path relative to workspace root |
| `path` | string | | cwd | Workspace root |

---

### `get symbol`
Return the full source text of a symbol by name. The agent doesn't need to know which file the symbol lives in — just ask for it.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `symbol` | string | yes | | Symbol name — supports fuzzy matching (see below) |
| `path` | string | | cwd | Workspace root |

**Fuzzy matching strategy**: The `symbol` parameter is matched against all known symbol paths (e.g. `MyStruct::authenticate`, `handle_request`, `AuthService.login`) using a multi-tier resolution:

1. **Exact match** — `MyStruct::authenticate` hits directly.
2. **Suffix match** — `authenticate` matches `MyStruct::authenticate` (useful when the agent doesn't know the parent).
3. **Case-insensitive match** — `myStruct::Authenticate` still resolves.
4. **Subsequence/fuzzy match** — `auth` matches `authenticate`, `AuthService`, `handle_auth_request`. Scored by edit distance and symbol kind (functions/methods ranked above modules/files).

If multiple symbols match, all are returned with their scores, file paths, line ranges, and full source text. The agent picks the one it wants. This is intentionally generous — it's better to return 3 candidates than to fail because the agent got the casing wrong or used an abbreviation.

The symbol table is drawn from both layers: tree-sitter `symbol_path` in `ts_chunks` (always available) and `lsp_symbols` (when LSP is indexed, provides richer type info). Results are deduplicated by position.

---

### Semantic diffing

Semantic entity-level diffing lives on the **`git` tool** as `get diff`, not on `code_context`. The `git` tool handles the diff (powered by `sem-core`), and optionally queries `code_context`'s call graph for caller fan-out.

See the **`git` tool: expanded operations** section below for full specification.

---

### `get blastradius`
Aggregated impact summary for a file or symbol — how many files and symbols transitively depend on it, ranked by proximity.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `file` | string | yes | | File to analyse |
| `symbol` | string | | all symbols in file | Narrow to a specific symbol |
| `max_hops` | integer | | 3 | Max reverse traversal depth, 1–10 |
| `path` | string | | cwd | Workspace root |

---

### `get status`
Unified health report across both layers.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `path` | string | | cwd | Workspace root |

Example output:

```
| Metric              | Value                                              |
|---------------------|----------------------------------------------------|
| Mode                | Leader                                             |
| Total Files         | 847                                                |
| TS Indexed          | 847 (100%)                                         |
| LSP Indexed         | 312 (37%)                                          |
| Dirty Files         | 0                                                  |
| TS Chunks           | 9,841                                              |
| Call Edges          | 14,203                                             |
|                     |                                                    |
| LSP: rust-analyzer  | Running (pid 42301, up 4m32s)                      |
```

When things go wrong:

```
| LSP: rust-analyzer  | Failed (3 attempts, last: "crashed with SIGSEGV") |
|                     | Install: rustup component add rust-analyzer        |
```

Or when the binary is missing:

```
| LSP: rust-analyzer  | Not found                                          |
|                     | Install: rustup component add rust-analyzer        |
```

LSP indexed < 100% during startup is normal — rust-analyzer takes 30–120 seconds on a large workspace.

---

### `build status`
Trigger a full reindex of both layers. Useful after a large branch switch or bulk file operation that the watcher may have missed.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `layer` | string | | both | `ts`, `lsp`, or `both` |
| `path` | string | | cwd | Workspace root |

---

### `clear status`
Wipe the index entirely and reset to empty. The leader will begin a fresh full reindex immediately after.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `layer` | string | | both | `ts`, `lsp`, or `both` |
| `path` | string | | cwd | Workspace root |

---

## `code-context` skill

A new skill (`builtin/skills/code-context/SKILL.md`) teaches the AI agent when and how to use the `code_context` tool effectively. This is critical because the tool is only useful if the agent knows to reach for it instead of grep.

### When the skill triggers

The skill should be loaded whenever the agent is exploring code, investigating a bug, planning a refactor, or assessing blast radius. It complements the existing `Explore` agent type — where `Explore` currently relies on `Glob` and `Grep`, the skill teaches it to prefer `code_context` operations when they're available.

### Key use cases the skill describes

| Scenario | Instead of... | Use... |
|---|---|---|
| "What does this function call?" | Grepping for the function name | `get callgraph` with `direction: outbound` |
| "Who calls this function?" | Grepping for the function name | `get callgraph` with `direction: inbound` |
| "What's the impact of changing this file?" | Guessing from imports | `get blastradius` |
| "Find all structs/classes in this file" | Reading the whole file | `list symbol` |
| "Show me the authenticate function" | Reading files, guessing | `get symbol` with fuzzy match |
| "Where is FooBar defined?" | `Glob` for filenames | `find symbol` |
| "Find functions related to authentication" | `Grep` for "auth" | `search code` (semantic) or `search symbol` (name-based) |
| "Find exact keyword in code" | Raw `rg` (returns lines) | `grep code` (returns whole functions) |
| "Find code similar to this snippet" | Nothing good | `search code` with the snippet as query |
| "What changed in this file?" | `git diff` (line noise) | `git get diff` (entity-level: added/deleted/modified symbols) |
| "What breaks if I change this?" | `git diff` + grep + hope | `git get diff` with `include_callers: true` |
| "Compare two code snippets" | Nothing | `git get diff` with `left_text` + `right_text` |
| "Is the index healthy?" | Hoping | `get status` |

### Skill content outline

1. **Always check `get status` first** if unsure whether the index is ready. If LSP is unavailable, fall back to tree-sitter-only operations.
2. **Prefer structured queries over text search.** `find symbol` and `get callgraph` give precise, compiler-quality results. `Grep` gives string matches that include comments, strings, and false positives.
3. **Use `search code` for fuzzy/conceptual queries** — "error handling", "database connection pooling" — where you don't know the exact symbol name.
4. **Use `get blastradius` before refactoring** to understand how many files and symbols are affected. This replaces the manual "grep and hope" approach.
5. **Combine operations** — e.g., `find symbol` to locate a function, then `get callgraph` to understand its callers, then `get blastradius` to scope the change.

---

## Migration from `treesitter` tool

The migration is additive. The existing `treesitter` tool continues to work unchanged during the transition. The plan:

1. Fork [Ataraxy-Labs/sem](https://github.com/Ataraxy-Labs/sem) into the swissarmyhammer org. Subtree it into `vendor/sem/` so we can edit in-place and `git subtree push` corrections back to the fork for upstream PRs. Add `sem-core` as a path dependency (`vendor/sem/sem-core`) in the workspace.
2. Add `swissarmyhammer-lsp` crate — the standalone LSP supervisor with the built-in server registry. No database dependency, no tree-sitter dependency. Just process management + LSP protocol.
3. Add `swissarmyhammer-code-context` crate — the unified tool that depends on `swissarmyhammer-treesitter`, `swissarmyhammer-lsp`, and `sem-core`. Owns the unified DB schema and `CodeContextWorkspace`.
4. Register `code_context` tool alongside `treesitter` in the tool registry.
5. Add `get diff` operation to the existing `git` tool, backed by `sem-core`.
6. Migrate the TS layer to write into the unified `index.db` instead of `.treesitter-index.db` (requires adding `with_db()` to `WorkspaceBuilder`).
7. Once `code_context` is stable, remove the `treesitter` tool registration.
8. Add `builtin/skills/code-context/SKILL.md` so the agent knows when to use `code_context` instead of grep.

### Scope: Rust-only LSP to start

The `SERVERS` registry ships with only `rust-analyzer`. When `detect_projects()` finds non-Rust project types (NodeJs, Python, Go, etc.), the system logs:

```
warn!("LSP support for {project_type} is not yet available — tree-sitter features will still work")
```

This is surfaced in `get status` output as well. Adding more servers is a follow-up — each one is just a new `LspServerSpec` entry and testing.

---

## `swissarmyhammer-lsp` crate

A new standalone crate that manages LSP server lifecycles. It is consumed by `code_context` but has no dependency on tree-sitter or the unified database — it only knows how to spawn, talk to, health-check, and restart language servers.

### Design principle: zero configuration

Users never write config files, pick servers, or know what an LSP is. The system works like this:

1. `swissarmyhammer-project-detection` already detects `ProjectType::Rust`, `ProjectType::NodeJs`, etc. from marker files.
2. `swissarmyhammer-lsp` has a **built-in registry** that maps each `ProjectType` to an `LspServerSpec`. This is a Rust `const`/`static` table compiled into the binary — no YAML, no config files.
3. On startup, the leader calls `detect_projects()`, then asks the registry which servers are needed.
4. For each needed server, the supervisor checks if the binary is on `$PATH`. If it's not found, it logs a clear actionable message (e.g., "rust-analyzer not found — install with `rustup component add rust-analyzer`") and moves on. The system degrades gracefully; tree-sitter features still work.
5. If the binary is found, the server is spawned and managed automatically.

The only time the user hears about LSP is when something is wrong — and even then, the error message tells them exactly what to do.

### Built-in server registry

The registry is a static table in Rust, not a config file. Starting with rust-analyzer only:

```rust
pub struct LspServerSpec {
    /// Which project types activate this server
    pub project_types: &'static [ProjectType],
    /// Binary name (resolved via $PATH)
    pub command: &'static str,
    /// CLI arguments
    pub args: &'static [&'static str],
    /// LSP language IDs this server handles (for textDocument/didOpen)
    pub language_ids: &'static [&'static str],
    /// File extensions this server cares about
    pub file_extensions: &'static [&'static str],
    /// initializationOptions sent in the initialize request
    pub initialization_options: Option<fn() -> serde_json::Value>,
    /// How long to wait for initialized response
    pub startup_timeout: Duration,
    /// Interval between health checks
    pub health_check_interval: Duration,
    /// Install hint shown when binary is missing
    pub install_hint: &'static str,
}

pub static SERVERS: &[LspServerSpec] = &[
    LspServerSpec {
        project_types: &[ProjectType::Rust],
        command: "rust-analyzer",
        args: &[],
        language_ids: &["rust"],
        file_extensions: &["rs"],
        initialization_options: Some(|| serde_json::json!({
            "cargo": { "buildScripts": { "enable": true } },
            "procMacro": { "enable": true },
        })),
        startup_timeout: Duration::from_secs(120),
        health_check_interval: Duration::from_secs(30),
        install_hint: "Install with: rustup component add rust-analyzer",
    },
];
```

Adding a new server later (e.g., typescript-language-server, gopls, pylsp) is just adding another entry to this table and a variant to `ProjectType`. The Claude Code plugins repo has a good catalogue of LSP command lines to draw from when we expand.

### Daemon lifecycle

The leader manages each LSP server as a supervised child process:

```
LspSupervisor
  ├── LspDaemon("rust-analyzer")
  │     ├── state: Running | Starting | Failed { since, attempts }
  │     ├── child: tokio::process::Child
  │     ├── stdin/stdout: LSP JSON-RPC transport
  │     └── health: last_response_at, pending_request_count
  └── ... (future servers)
```

**Startup sequence** (leader only):

1. Call `detect_projects()` to get `Vec<DetectedProject>`.
2. Look up each `DetectedProject.project_type` in the `SERVERS` registry to find matching `LspServerSpec`s.
3. For each match, check if `command` is on `$PATH` (`which` / `tokio::process::Command::new(...).arg("--version")`).
4. If not found, log the `install_hint` at `warn!` level and skip. The `get status` operation surfaces missing servers.
5. If found, spawn the process with `tokio::process::Command`, connecting stdin/stdout as pipes.
6. Send `initialize` request with the spec's `initialization_options`, wait up to `startup_timeout` for `initialized`.
7. On success, mark as `Running` and begin feeding `textDocument/didOpen` for already-indexed files.
8. On timeout or spawn failure, mark as `Failed` and schedule a retry.

**Health checking**:

Every `health_check_interval_secs`, the supervisor sends a lightweight request (e.g., `shutdown` dry-run or a no-op `workspace/symbol` with empty query) and checks:
- Did we get a response within 5 seconds?
- Has `last_response_at` gone stale (> 2x the health interval)?
- Has the child process exited unexpectedly (`child.try_wait()`)?

If any check fails, the server is marked `Failed` and restart is triggered.

**Restart policy**:

- Exponential backoff: 1s, 2s, 4s, 8s, ... capped at 60s.
- After 5 consecutive failures, stop retrying and log an error. The `get status` operation surfaces this so the AI agent knows LSP is degraded.
- A successful `initialized` response resets the failure counter.
- `build status` with `layer: lsp` force-restarts failed servers (resets backoff).

**Shutdown**:

- On leader shutdown (guard dropped), send `shutdown` + `exit` to each running server.
- If the server doesn't exit within 5 seconds, `SIGKILL`.
- Child processes are reaped to avoid zombies.

### Integration with the watcher

When the file watcher fires, the `LspWatcherHandler` in the fanout:

1. Sends `textDocument/didChange` (or `didClose`/`didOpen` for creates/deletes) to the appropriate server based on `file_patterns`.
2. Marks the file dirty in `indexed_files`.

If the server is in `Failed` state, the handler silently drops notifications — they'll be replayed as `didOpen` calls when the server recovers and gets re-initialized.

### Reader processes and LSP

Readers never interact with LSP servers directly. They query the `lsp_symbols` and `lsp_call_edges` tables in the shared SQLite database, which the leader populates. This keeps the architecture simple — exactly one process talks to each language server.

---

## `git` tool: expanded operations

The existing `git` MCP tool gains a new `get diff` operation alongside the current `get changes`.

### Operation matrix

| | `changes` | `diff` |
|---|---|---|
| `get` | ✅ file list (existing) | ✅ entity-level semantic diff |

`get changes` returns a list of changed file paths (cheap, fast). `get diff` returns entity-level semantic diffs powered by `sem-core` — what symbols were added, deleted, modified, renamed, or moved. `get diff` is a superset: its output includes the file list plus the entity breakdown within each file.

### `get diff` — input model

Every diff has a left side and a right side. Each side can be one of four input types:

| Input type | How to specify | Example |
|---|---|---|
| Git ref | `left` / `right` as `file@ref` | `"src/main.rs@HEAD~3"`, `"lib.rs@abc1234"` |
| File path | `left` / `right` as path | `"src/main.rs"` (reads from working tree) |
| Inline text | `left_text` / `right_text` | Raw code string passed directly |
| Omitted | Leave param out | Smart default kicks in |

Separating `left`/`right` from `left_text`/`right_text` avoids ambiguity — a short string like `"main"` could be a branch name or inline code.

### `get diff` — smart defaulting

| Left | Right | Behavior |
|---|---|---|
| omitted | omitted + dirty tree | Each dirty file vs HEAD |
| omitted | omitted + clean tree | Current branch vs origin/parent branch |
| `file@ref` | omitted | file@ref vs working tree |
| `file@ref` | `file@ref` | Two git versions of same or different files |
| `file` | `file` | Two working-tree files compared structurally |
| `file@ref` | `right_text` | Git version vs proposed code |
| `file` | `right_text` | On-disk file vs proposed code |
| `left_text` | `right_text` | Pure text-vs-text structural comparison |

### `get diff` — parameters

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `left` | string | no | smart default | File path or `file@ref` for left side |
| `right` | string | no | smart default | File path or `file@ref` for right side |
| `left_text` | string | no | | Inline code for left side (use instead of `left`) |
| `right_text` | string | no | | Inline code for right side (use instead of `right`) |
| `language` | string | no | inferred | Tree-sitter language for parsing. Inferred from file extension when `left`/`right` are paths; **required** when both sides are inline text. |
| `include_callers` | boolean | no | true | Fan out via `code_context` call graph to show affected callers |
| `max_hops` | integer | no | 2 | How far to fan out for affected callers |
| `path` | string | no | cwd | Workspace root |

### Use cases

| Scenario | Parameters |
|---|---|
| "What did I change?" | `{}` (omit everything) |
| "What changed in this file?" | `{ "left": "src/main.rs@HEAD" }` |
| "Compare two commits" | `{ "left": "src/main.rs@abc123", "right": "src/main.rs@def456" }` |
| "Is this a copy of that?" | `{ "left": "src/auth.rs", "right": "src/old_auth.rs" }` |
| "How does my edit differ?" | `{ "left": "src/main.rs", "right_text": "fn main() { ... }" }` |
| "Compare two snippets" | `{ "left_text": "fn foo() {}", "right_text": "fn bar() {}" }` |

### Entity-level diff algorithm

Powered by `sem-core` from [Ataraxy-Labs/sem](https://github.com/Ataraxy-Labs/sem), forked into the swissarmyhammer org for version control. `sem-core` provides tree-sitter entity extraction (functions, structs, methods, classes) and a three-phase change detection algorithm:

1. Retrieve both versions (`git show {base}:{file}` for git mode, or read file paths / inline text for file-to-file mode).
2. `sem-core` parses both with tree-sitter → extracts entities with identity keys (`file:type:name:parent`).
3. Three-phase matching:
   - **Exact ID match** — same `type:name:parent` in both versions.
   - **Structural hash** — whitespace-insensitive content comparison to filter formatting-only changes.
   - **Fuzzy similarity** — >80% token overlap catches renames (e.g., `validate_token` → `verify_token`).
4. Classify:
   - **Added** — entity exists only in target.
   - **Deleted** — entity exists only in base.
   - **Modified** — matched entity, different content.
   - **Renamed** — fuzzy-matched with high token overlap but different name.
   - **Moved** — same identity and content, different line range. Not flagged as a change.
   - **Unchanged** — same identity, same content.
5. If `include_callers` is true, for every modified/deleted/renamed entity, query `code_context`'s `lsp_call_edges` reverse index to find callers up to `max_hops` deep.

**Why `sem-core`**: entity extraction across 16 languages and rename detection via token overlap are non-trivial to reimplement. `sem-core` is Rust, Apache-2.0/MIT dual-licensed, and already battle-tested in the Weave merge driver. Forking into swissarmyhammer gives us version control without coupling to upstream API churn.

**Why this beats `git diff`**: a function that moved within a file but didn't change content is **moved**, not a false positive. Formatting-only changes are filtered by structural hash. Renamed functions are detected via fuzzy matching. Deleted symbols surface their callers automatically via the call graph.

**Output**: list of entity-level changes (with before/after source text for modified entities), and if `include_callers` is true, the affected callers grouped by hop distance with edge provenance (`lsp` or `treesitter`).

---

## New dependencies

Two additions to `[workspace.dependencies]`:

```toml
async-lsp = { version = "0.2", features = ["client-side"] }
lsp-types = "0.97"
```

`async-lsp` provides the JSON-RPC transport and typed request/response framing over the child process's stdin/stdout. `lsp-types` gives us the full LSP protocol types. These live in `swissarmyhammer-lsp`'s `Cargo.toml`.

`sem-core` (subtree'd from our fork of [Ataraxy-Labs/sem](https://github.com/Ataraxy-Labs/sem) into `vendor/sem/`) provides entity extraction and semantic diffing. Referenced as a path dependency: `sem-core = { path = "vendor/sem/sem-core" }`.

`swissarmyhammer-project-detection` is already in the workspace. Everything else (`tokio`, `rusqlite`, `rayon`, `ignore`, `notify`, `dashmap`, `serde_json`, `thiserror`, `tracing`) is already in the workspace too.
