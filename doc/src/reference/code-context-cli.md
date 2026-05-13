# Command-Line Help for `code-context`

This document contains the help content for the `code-context` command-line program.

## Installation

```bash
brew install swissarmyhammer/tap/code-context-cli
```

**Command Overview:**

* [`code-context`↴](#code-context)
* [`code-context serve`↴](#code-context-serve)
* [`code-context init`↴](#code-context-init)
* [`code-context deinit`↴](#code-context-deinit)
* [`code-context doctor`↴](#code-context-doctor)
* [`code-context skill`↴](#code-context-skill)
* [`code-context get`↴](#code-context-get)
* [`code-context get symbol`↴](#code-context-get-symbol)
* [`code-context get callgraph`↴](#code-context-get-callgraph)
* [`code-context get blastradius`↴](#code-context-get-blastradius)
* [`code-context get status`↴](#code-context-get-status)
* [`code-context get definition`↴](#code-context-get-definition)
* [`code-context get type-definition`↴](#code-context-get-type-definition)
* [`code-context get hover`↴](#code-context-get-hover)
* [`code-context get references`↴](#code-context-get-references)
* [`code-context get implementations`↴](#code-context-get-implementations)
* [`code-context get code-actions`↴](#code-context-get-code-actions)
* [`code-context get inbound-calls`↴](#code-context-get-inbound-calls)
* [`code-context get rename-edits`↴](#code-context-get-rename-edits)
* [`code-context get diagnostics`↴](#code-context-get-diagnostics)
* [`code-context search`↴](#code-context-search)
* [`code-context search symbol`↴](#code-context-search-symbol)
* [`code-context search code`↴](#code-context-search-code)
* [`code-context search workspace-symbol`↴](#code-context-search-workspace-symbol)
* [`code-context list`↴](#code-context-list)
* [`code-context list symbols`↴](#code-context-list-symbols)
* [`code-context grep`↴](#code-context-grep)
* [`code-context grep code`↴](#code-context-grep-code)
* [`code-context query`↴](#code-context-query)
* [`code-context query ast`↴](#code-context-query-ast)
* [`code-context find`↴](#code-context-find)
* [`code-context find duplicates`↴](#code-context-find-duplicates)
* [`code-context rebuild`↴](#code-context-rebuild)
* [`code-context rebuild index`↴](#code-context-rebuild-index)
* [`code-context clear`↴](#code-context-clear)
* [`code-context clear status`↴](#code-context-clear-status)
* [`code-context lsp`↴](#code-context-lsp)
* [`code-context lsp status`↴](#code-context-lsp-status)
* [`code-context detect`↴](#code-context-detect)
* [`code-context detect projects`↴](#code-context-detect-projects)

## `code-context`

code-context - Structural code intelligence for AI agents

Provides indexed code navigation, symbol lookup, call graph traversal, blast radius analysis, and semantic search. Exposes these capabilities as MCP tools for AI coding agents.

**Usage:** `code-context [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `serve` — Run MCP server over stdio, exposing code-context tools
* `init` — Install code-context MCP server into Claude Code settings
* `deinit` — Remove code-context from Claude Code settings
* `doctor` — Diagnose code-context configuration and setup
* `skill` — Deploy code-context skill to agent .skills/ directories
* `get` — Get a resource (symbol, callgraph, blast radius, status, etc.)
* `search` — Search for symbols, code, or workspace symbols
* `list` — List resources (symbols in a file)
* `grep` — Regex search across stored code chunks
* `query` — Execute tree-sitter queries against parsed ASTs
* `find` — Find duplicated code
* `rebuild` — Trigger re-indexing
* `clear` — Wipe index data
* `lsp` — LSP server management
* `detect` — Detect project types and languages

###### **Options:**

* `-d`, `--debug` — Enable debug output to stderr
* `-j`, `--json` — Output results as JSON (for operation commands)
* `--no-progress` — Disable interactive progress bars for long-running operations.

   `indicatif` auto-degrades to plain output on non-TTY stdout, but some environments (CI runners, recording wrappers) still benefit from a hard switch. With this flag set the dispatcher installs a no-op renderer and the tool emits no progress chrome.



## `code-context serve`

Run MCP server over stdio, exposing code-context tools

**Usage:** `code-context serve`



## `code-context init`

Install code-context MCP server into Claude Code settings

**Usage:** `code-context init [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to install the server configuration

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `code-context deinit`

Remove code-context from Claude Code settings

**Usage:** `code-context deinit [TARGET]`

###### **Arguments:**

* `<TARGET>` — Where to remove the server configuration from

  Default value: `project`

  Possible values:
  - `project`:
    Project-level settings (.claude/settings.json)
  - `local`:
    Local project settings, not committed (.claude/settings.local.json)
  - `user`:
    User-level settings (~/.claude/settings.json)




## `code-context doctor`

Diagnose code-context configuration and setup

**Usage:** `code-context doctor [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show detailed output including fix suggestions



## `code-context skill`

Deploy code-context skill to agent .skills/ directories

**Usage:** `code-context skill`



## `code-context get`

Get a resource (symbol, callgraph, blast radius, status, etc.)

**Usage:** `code-context get <COMMAND>`

###### **Subcommands:**

* `symbol` — Look up symbol locations and source text with fuzzy matching
* `callgraph` — Traverse call graph from a starting symbol
* `blastradius` — Analyze blast radius of changes to a file or symbol
* `status` — Health report with file counts, indexing progress, chunk/edge counts
* `definition` — Go to definition with layered resolution (live LSP, LSP index, tree-sitter)
* `type-definition` — Go to type definition (live LSP only)
* `hover` — Get hover information (type signature, docs)
* `references` — Find all references to a symbol
* `implementations` — Find implementations of a trait/interface
* `code-actions` — Get code actions (quickfixes, refactors) for a range (live LSP only)
* `inbound-calls` — Find all callers of a function at a given position
* `rename-edits` — Preview rename edits without applying them (live LSP only)
* `diagnostics` — Get errors and warnings for a file (live LSP only)



## `code-context get symbol`

Look up symbol locations and source text with fuzzy matching

**Usage:** `code-context get symbol [OPTIONS] --query <QUERY>`

###### **Options:**

* `--query <QUERY>` — Symbol name or qualified path to search for
* `--max-results <MAX_RESULTS>` — Maximum number of results to return



## `code-context get callgraph`

Traverse call graph from a starting symbol

**Usage:** `code-context get callgraph [OPTIONS] --symbol <SYMBOL>`

###### **Options:**

* `--symbol <SYMBOL>` — Symbol identifier (name or file:line:char locator)
* `--direction <DIRECTION>` — Traversal direction: inbound, outbound, or both
* `--max-depth <MAX_DEPTH>` — Maximum traversal depth (1-5)



## `code-context get blastradius`

Analyze blast radius of changes to a file or symbol

**Usage:** `code-context get blastradius [OPTIONS] --file-path <FILE_PATH>`

###### **Options:**

* `--file-path <FILE_PATH>` — File path to analyze
* `--symbol <SYMBOL>` — Optional symbol name to narrow the starting set
* `--max-hops <MAX_HOPS>` — Maximum number of hops to follow (1-10)



## `code-context get status`

Health report with file counts, indexing progress, chunk/edge counts

**Usage:** `code-context get status`



## `code-context get definition`

Go to definition with layered resolution (live LSP, LSP index, tree-sitter)

**Usage:** `code-context get definition --file-path <FILE_PATH> --line <LINE> --character <CHARACTER>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file containing the symbol
* `--line <LINE>` — Zero-based line number of the symbol
* `--character <CHARACTER>` — Zero-based character offset within the line



## `code-context get type-definition`

Go to type definition (live LSP only)

**Usage:** `code-context get type-definition --file-path <FILE_PATH> --line <LINE> --character <CHARACTER>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file containing the symbol
* `--line <LINE>` — Zero-based line number of the symbol
* `--character <CHARACTER>` — Zero-based character offset within the line



## `code-context get hover`

Get hover information (type signature, docs)

**Usage:** `code-context get hover --file-path <FILE_PATH> --line <LINE> --character <CHARACTER>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file containing the symbol
* `--line <LINE>` — Zero-based line number of the symbol
* `--character <CHARACTER>` — Zero-based character offset within the line



## `code-context get references`

Find all references to a symbol

**Usage:** `code-context get references [OPTIONS] --file-path <FILE_PATH> --line <LINE> --character <CHARACTER>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file containing the symbol
* `--line <LINE>` — Zero-based line number of the symbol
* `--character <CHARACTER>` — Zero-based character offset within the line
* `--include-declaration <INCLUDE_DECLARATION>` — Whether to include the declaration itself in results

  Possible values: `true`, `false`

* `--max-results <MAX_RESULTS>` — Maximum number of references to return



## `code-context get implementations`

Find implementations of a trait/interface

**Usage:** `code-context get implementations [OPTIONS] --file-path <FILE_PATH> --line <LINE> --character <CHARACTER>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file containing the trait/interface symbol
* `--line <LINE>` — Zero-based line number of the symbol
* `--character <CHARACTER>` — Zero-based character offset within the line
* `--max-results <MAX_RESULTS>` — Maximum number of implementation locations to return



## `code-context get code-actions`

Get code actions (quickfixes, refactors) for a range (live LSP only)

**Usage:** `code-context get code-actions [OPTIONS] --file-path <FILE_PATH> --start-line <START_LINE> --start-character <START_CHARACTER> --end-line <END_LINE> --end-character <END_CHARACTER>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file to get code actions for
* `--start-line <START_LINE>` — Zero-based start line of the range
* `--start-character <START_CHARACTER>` — Zero-based start character offset
* `--end-line <END_LINE>` — Zero-based end line of the range
* `--end-character <END_CHARACTER>` — Zero-based end character offset
* `--filter-kind <FILTER_KIND>` — Filter for code action kinds (e.g. quickfix, refactor, source)



## `code-context get inbound-calls`

Find all callers of a function at a given position

**Usage:** `code-context get inbound-calls [OPTIONS] --file-path <FILE_PATH> --line <LINE> --character <CHARACTER>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file containing the target symbol
* `--line <LINE>` — Zero-based line number of the target symbol
* `--character <CHARACTER>` — Zero-based character offset within the line
* `--depth <DEPTH>` — Recursive depth for caller traversal (1-5)



## `code-context get rename-edits`

Preview rename edits without applying them (live LSP only)

**Usage:** `code-context get rename-edits --file-path <FILE_PATH> --line <LINE> --character <CHARACTER> --new-name <NEW_NAME>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file containing the symbol to rename
* `--line <LINE>` — Zero-based line number of the symbol
* `--character <CHARACTER>` — Zero-based character offset within the line
* `--new-name <NEW_NAME>` — The new name for the symbol



## `code-context get diagnostics`

Get errors and warnings for a file (live LSP only)

**Usage:** `code-context get diagnostics [OPTIONS] --file-path <FILE_PATH>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file to get diagnostics for
* `--severity-filter <SEVERITY_FILTER>` — Only return diagnostics at or above this severity (error, warning, info, hint)



## `code-context search`

Search for symbols, code, or workspace symbols

**Usage:** `code-context search <COMMAND>`

###### **Subcommands:**

* `symbol` — Fuzzy search across all indexed symbols
* `code` — Semantic similarity search across code chunks using embeddings
* `workspace-symbol` — Live workspace symbol search with layered resolution



## `code-context search symbol`

Fuzzy search across all indexed symbols

**Usage:** `code-context search symbol [OPTIONS] --query <QUERY>`

###### **Options:**

* `--query <QUERY>` — Text to fuzzy-match against symbol names
* `--kind <KIND>` — Filter by symbol kind (function, method, struct, class, etc.)
* `--max-results <MAX_RESULTS>` — Maximum number of results to return



## `code-context search code`

Semantic similarity search across code chunks using embeddings

**Usage:** `code-context search code [OPTIONS] --query <QUERY>`

###### **Options:**

* `--query <QUERY>` — Natural language query for semantically similar code
* `--top-k <TOP_K>` — Maximum number of results to return
* `--min-similarity <MIN_SIMILARITY>` — Minimum cosine similarity threshold (0.0-1.0)
* `--file-pattern <FILE_PATTERN>` — Only search chunks from files matching this path pattern
* `--language <LANGUAGE>` — Only search chunks from files with these extensions



## `code-context search workspace-symbol`

Live workspace symbol search with layered resolution

**Usage:** `code-context search workspace-symbol [OPTIONS] --query <QUERY>`

###### **Options:**

* `--query <QUERY>` — Symbol name or text to search for across the workspace
* `--max-results <MAX_RESULTS>` — Maximum number of results to return



## `code-context list`

List resources (symbols in a file)

**Usage:** `code-context list <COMMAND>`

###### **Subcommands:**

* `symbols` — List all symbols in a specific file, sorted by start line



## `code-context list symbols`

List all symbols in a specific file, sorted by start line

**Usage:** `code-context list symbols --file-path <FILE_PATH>`

###### **Options:**

* `--file-path <FILE_PATH>` — Path to the file to list symbols from



## `code-context grep`

Regex search across stored code chunks

**Usage:** `code-context grep <COMMAND>`

###### **Subcommands:**

* `code` — Regex search across stored code chunks



## `code-context grep code`

Regex search across stored code chunks

**Usage:** `code-context grep code [OPTIONS] --pattern <PATTERN>`

###### **Options:**

* `--pattern <PATTERN>` — Regex pattern to search for
* `--language <LANGUAGE>` — Only search chunks from files with these extensions (e.g. rs, py)
* `--files <FILES>` — Only search chunks from these specific file paths
* `--max-results <MAX_RESULTS>` — Maximum number of matching chunks to return



## `code-context query`

Execute tree-sitter queries against parsed ASTs

**Usage:** `code-context query <COMMAND>`

###### **Subcommands:**

* `ast` — Execute tree-sitter S-expression queries against parsed ASTs



## `code-context query ast`

Execute tree-sitter S-expression queries against parsed ASTs

**Usage:** `code-context query ast [OPTIONS] --query <QUERY> --language <LANGUAGE>`

###### **Options:**

* `--query <QUERY>` — Tree-sitter S-expression query pattern
* `--language <LANGUAGE>` — Language to parse files as (e.g. rust, python, typescript)
* `--files <FILES>` — File paths to query against
* `--max-results <MAX_RESULTS>` — Maximum number of matches to return



## `code-context find`

Find duplicated code

**Usage:** `code-context find <COMMAND>`

###### **Subcommands:**

* `duplicates` — Find code in a file that is duplicated elsewhere in the codebase



## `code-context find duplicates`

Find code in a file that is duplicated elsewhere in the codebase

**Usage:** `code-context find duplicates [OPTIONS] --file-path <FILE_PATH>`

###### **Options:**

* `--file-path <FILE_PATH>` — File to check for duplicated code
* `--min-similarity <MIN_SIMILARITY>` — Minimum cosine similarity to report as duplicate (0.0-1.0)
* `--max-per-chunk <MAX_PER_CHUNK>` — Maximum duplicates to show per source chunk
* `--min-chunk-bytes <MIN_CHUNK_BYTES>` — Minimum chunk size in bytes to consider



## `code-context rebuild`

Trigger re-indexing

**Usage:** `code-context rebuild <COMMAND>`

###### **Subcommands:**

* `index` — Mark files for re-indexing by resetting indexed flags



## `code-context rebuild index`

Mark files for re-indexing by resetting indexed flags

**Usage:** `code-context rebuild index [OPTIONS]`

###### **Options:**

* `--layer <LAYER>` — Which indexing layer to reset: treesitter, lsp, or both



## `code-context clear`

Wipe index data

**Usage:** `code-context clear <COMMAND>`

###### **Subcommands:**

* `status` — Wipe all index data and return stats about what was cleared



## `code-context clear status`

Wipe all index data and return stats about what was cleared

**Usage:** `code-context clear status`



## `code-context lsp`

LSP server management

**Usage:** `code-context lsp <COMMAND>`

###### **Subcommands:**

* `status` — Show detected languages, their LSP servers, and install status



## `code-context lsp status`

Show detected languages, their LSP servers, and install status

**Usage:** `code-context lsp status`



## `code-context detect`

Detect project types and languages

**Usage:** `code-context detect <COMMAND>`

###### **Subcommands:**

* `projects` — Detect project types in the workspace and return language-specific guidelines



## `code-context detect projects`

Detect project types in the workspace and return language-specific guidelines

**Usage:** `code-context detect projects [OPTIONS]`

###### **Options:**

* `--path <PATH>` — Root path to search for projects
* `--max-depth <MAX_DEPTH>` — Maximum directory depth to search
* `--include-guidelines <INCLUDE_GUIDELINES>` — Include language-specific guidelines in output

  Possible values: `true`, `false`




