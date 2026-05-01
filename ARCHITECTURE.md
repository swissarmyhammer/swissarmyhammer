# Architecture

SwissArmyHammer is a monorepo producing CLI tools, MCP servers, and desktop apps for AI-assisted software engineering. The core domain is a file-backed kanban board engine with YAML-driven schema, a unified command system, and a Tauri desktop UI.

Consult the root `Cargo.toml` for current workspace members and `kanban-app/package.json` for the frontend stack. This document describes the architectural concepts and rules that don't change with every commit.

---

## 1. Rust Core

### Crate Tier Rules

Crates are organized in dependency tiers. A crate may only depend on crates in the same or lower tier. Check `Cargo.toml` for current membership — these are the *placement rules*:

- **Tier 0 — Leaves**: Zero workspace dependencies. A crate belongs here if it defines a trait, provides a utility, or solves a self-contained problem without importing any sibling crate. Core abstractions like the `Command` trait (`swissarmyhammer-commands`), the `TrackedStore` trait (`swissarmyhammer-store`), and the `Operation` proc macro (`swissarmyhammer-operations`) live here because they define interfaces that higher tiers implement.

- **Tier 1 — Foundation**: Depends only on Tier 0. A crate belongs here if it provides shared infrastructure (types, ID generation, error types, config loading) consumed broadly across the workspace but doesn't define domain semantics.

- **Tier 2 — Schema and Storage**: Depends on Tier 0-1. A crate belongs here if it defines *what* data looks like without knowing *how* it's used. The field/entity schema registry (`swissarmyhammer-fields`), perspective storage (`swissarmyhammer-perspectives`), and code-context index belong here. They declare types and storage but not business rules.

- **Tier 3 — Entity Layer**: Depends on Tier 0-2. A crate belongs here if it provides generic entity I/O — reading, writing, caching, searching entities against a schema. The `Entity` type and `EntityContext` (`swissarmyhammer-entity`) live here. They know about fields and storage but not about kanban boards, tasks, or columns.

- **Tier 4 — Application Libraries**: Depends on anything below. A crate belongs here if it implements domain logic — kanban operations, skill resolution, tree-sitter indexing, web search. These are the "engines" that CLI and UI programs wire together.

**The key structural constraint**: Application libraries have no knowledge of Tauri, React, or any specific CLI framework. They are pure domain libraries. The Tauri app and CLI tools are thin wiring layers over the engines.

### Virtual File System and Content Stacking

The `VirtualFileSystem` (`swissarmyhammer-directory`) is a foundational abstraction. It stacks three directory layers with precedence:

```
builtin/    → compiled into the binary (include_dir!)
~/.sah/     → user-level overrides
.sah/       → project-level overrides (or .kanban/, .code-context/, etc.)
```

When loading YAML definitions (commands, fields, entities, views, skills, agents), the VFS discovers files across all three layers and merges them. Project-level overrides win over user-level, which win over builtin. Partial overrides work — you can override a single keybinding in a command without restating the entire definition.

Every configurable subsystem uses this pattern: `DirectoryConfig` trait declares the directory names, and `ManagedDirectory<C>` handles discovery, precedence, and file loading.

### Content Formats

Two file formats are used pervasively:

- **Plain YAML** (`.yaml`): Structured data without a body field. Used for commands, field definitions, entity definitions without markdown content, views, perspectives, and configuration.

- **YAML Frontmatter + Markdown** (`.md`): A YAML block delimited by `---` followed by a markdown body. Used for entities with a `body_field` (tasks, skills, agents), and for skill/agent definitions (SKILL.md, AGENT.md). The frontmatter is structured metadata; the body is free-form content.

Entity storage format is determined by the entity definition's `body_field` property. If present, the entity is `.md` with the named field as the markdown body. If absent, the entity is `.yaml`.

### Change Tracking: JSONL Changelogs and Text Diffs

Every entity type has a companion `.jsonl` changelog file alongside its `.yaml` or `.md` content file. Each line is a JSON object recording one mutation — the operation, timestamp, and the changed fields. This append-only log serves three purposes:

1. **Undo/redo** — the `UndoStack` reads the changelog to reverse or replay operations across entity types.
2. **Conflict resolution** — during git merges, the JSONL log provides a per-field operation history that the custom merge driver (`swissarmyhammer-merge`) uses to resolve conflicts at the field level rather than the file level.
3. **Audit trail** — operations that return `ExecutionResult::Logged` produce a changelog entry; `Unlogged` operations (reads, queries) do not.

For markdown body content, changes are tracked as **text diffs** (via `diffy`) rather than whole-value replacement. This means the changelog records the patch, not the full body, keeping logs compact and merges meaningful even for large markdown documents.

The custom git merge driver handles three file types in `.kanban/`:
- **JSONL** files — line-level union merge (append-only logs merge cleanly)
- **YAML** files — field-level three-way merge
- **Frontmatter+Markdown** files — YAML frontmatter merged field-by-field, markdown body merged as text with conflict markers when necessary

### Liquid Templating

All YAML and frontmatter+markdown files loaded through the VFS support Liquid template syntax. This enables shared partials, conditional content, and variable interpolation across any configuration or content file — commands, field definitions, entity definitions, skills, agents, and any future YAML-driven schema. Template resolution happens at load time; partials are discovered from the same VFS directory stack, so a project-level partial can override a builtin one just like any other file.

### Key Abstractions

#### Context Objects (Blackboard Pattern)

Every subsystem exposes a **Context** struct that bundles its I/O primitives, configuration, and indexes into a single value. Prefer passing a context object over long argument lists.

**Why contexts:**

- **One argument instead of many.** A function that needs storage, field definitions, and validation takes one `&KanbanContext` instead of three separate parameters. When requirements grow the signature stays stable.
- **Blackboard pattern.** Higher-level contexts compose lower-level ones as fields. A consumer receives the top-level context and reaches through it to whatever layer it needs. No need to thread individual pieces through the call stack.
- **Clear ownership.** Each context owns its resources (paths, indexes, caches, locks). Callers don't manage lifetimes of the internals.

**Context hierarchy:**

```
CliContext                     # CLI flags, output format, prompt library
  └─ TemplateContext           # config key/value pairs

KanbanContext                  # .kanban/ root, file locking
  ├─ FieldsContext (Arc)       # field definitions, entity templates, name/ID indexes
  ├─ EntityContext (Arc)       # entity I/O, changelogs, undo stack, validation, compute
  │    └─ FieldsContext (Arc)  # shared — same instance as KanbanContext.fields
  └─ ViewsContext (RwLock)     # view definitions, CRUD, disk persistence

CommandContext                 # scope chain, target, args, UI state
  └─ extensions: HashMap<TypeId, Arc<dyn Any>>
       └─ KanbanContext (Arc)  # injected as a typed extension
```

Contexts at the bottom of the hierarchy (FieldsContext, ViewsContext) are self-contained. Contexts higher up compose them via `Arc` so the same instance is shared without copying.

**Conventions:**

- **Create with `open()` or a builder, not bare constructors.** Most contexts have an async `open()` that loads definitions from disk and builds indexes. Use `new()` only for lightweight/partial initialization (e.g., tests).
- **Compose via `Arc` fields.** When a higher-level context needs a lower-level one, store it as `Arc<T>` so it can be shared across contexts without lifetime gymnastics.
- **Use `with_*` builder methods for optional capabilities.** Attach engines, registries, or configuration after construction rather than requiring everything upfront.
- **Extensions for cross-cutting services.** `CommandContext` uses a `TypeId`-keyed extension map so domain contexts (KanbanContext) can be injected without the command framework knowing about them.

**Anti-patterns to avoid:**

- **Long argument lists.** If a function takes more than 2-3 related parameters, bundle them into a context or introduce a new one.
- **Passing internals instead of the context.** Don't destructure a context to pass its fields individually — pass the context and let the callee reach in.
- **Cloning instead of sharing.** Use `Arc` to share contexts. Cloning a FieldsContext with its indexes is wasteful.
- **Logic in the context.** Contexts provide *access*, not behavior. Business logic belongs in commands and helpers that receive the context.

#### Command

The `Command` trait (`swissarmyhammer-commands`) is the interface for all state-mutating operations in the UI:

```rust
#[async_trait]
pub trait Command: Send + Sync {
    fn available(&self, ctx: &CommandContext) -> bool;
    async fn execute(&self, ctx: &CommandContext) -> Result<Value>;
}
```

Commands are registered by string ID (e.g. `"task.add"`) in a flat map. They receive a `CommandContext` containing the scope chain, target moniker, explicit args, UIState, and domain-specific extensions (KanbanContext, EntityContext, StoreContext). The Command trait is defined separately from the kanban engine — it knows nothing about tasks or boards.

Commands are **defined and run in Rust**, but **exposed for invocation** through four surfaces: native menu bar, context menus, the command palette, and occasionally direct button clicks. The YAML command definition controls which surfaces a command appears on via `menu`, `context_menu`, `keys`, and `visible` properties.

#### Operation

The `Operation` trait (`swissarmyhammer-operations`) is for MCP tool definitions. Operations are structs where fields ARE parameters:

```rust
#[operation(verb = "add", noun = "task", description = "Create a new task")]
pub struct AddTask {
    #[param(alias = "name")]
    pub title: String,
    pub column: Option<String>,
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> { ... }
}
```

`ExecutionResult` distinguishes `Logged` (produces an audit trail entry) from `Unlogged` (read-only). The `#[operation]` proc macro generates MCP JSON schema and CLI argument parsing from the struct definition.

#### TrackedStore

Three-layer storage architecture (`swissarmyhammer-store`):

```
TrackedStore (trait)     — domain-specific store (e.g. PerspectiveStore)
  └─ StoreHandle         — adds undo/cache layer
       └─ StoreContext    — coordinates multiple stores with shared UndoStack
```

Each entity type gets its own `TrackedStore` implementation. The `StoreContext` wraps them all with a shared `UndoStack` for cross-entity undo/redo. JSONL changelogs track every mutation for audit and conflict resolution.

#### Entity

A generic bag of fields (`swissarmyhammer-entity`):

```rust
pub struct Entity {
    pub entity_type: String,
    pub id: String,
    pub fields: HashMap<String, Value>,
}
```

Entity IDs come from filenames, not file contents. This makes git diffs clean and merges tractable.

#### UIState

Per-window state tracked in the Rust backend (`swissarmyhammer-commands`) and synced to the frontend via events:

- `keymap_mode`: cua / vim / emacs
- `windows`: per-window state (board_path, inspector_stack, active_view_id, active_perspective_id, geometry)
- `open_boards`, `recent_boards`
- Transient: scope_chain, drag_session, clipboard state, undo/redo availability

Thread-safe via internal `RwLock`. Auto-persists to YAML on every mutation (when loaded from a file path).

### YAML-Driven Schema

Everything declarative is defined in YAML loaded through the VFS. The YAML files are the single source of truth; Rust code and the frontend interpret them at runtime.

**What's defined in YAML**: commands, field definitions, entity definitions, view definitions, perspectives, LSP server specs.

**Command YAML example:**

```yaml
- id: task.add
  name: New Task
  scope: "entity:column"
  undoable: true
  keys:
    cua: Mod+N
    vim: a
  menu:
    path: [Edit]
    group: 1
  context_menu: true
  params:
    - name: column
      from: scope_chain
      entity_type: column
```

The `params[].from` field declares how parameters are resolved: `scope_chain` extracts from the moniker hierarchy, `args` from explicit arguments, `target` from the target moniker.

**Entity YAML example:**

```yaml
name: task
icon: check-square
body_field: body
search_display_field: title
mention_prefix: "^"
commands:
  - id: ui.inspect
    context_menu: true
fields:
  - title
  - tags
  - assignees
```

**Field YAML example:**

```yaml
name: title
type:
  kind: markdown
  single_line: true
editor: markdown
display: text
sort: alphanumeric
width: 300
section: header
```

### Computed Fields and Pseudo-Field Dependencies

Fields with `type.kind: computed` declare a `derive` function that runs after an entity is read. Most derivations only consume other fields on the same entity, but some need inputs that never live in `entity.fields` on disk — the JSONL changelog, filesystem metadata, etc. These inputs are modeled as **pseudo-fields**: reserved names prefixed with `_` that are lazily injected before derivation and stripped immediately after, so they never persist and never reach callers.

**How a computed field opts in.** Declare the pseudo-field in `depends_on` in the field YAML:

```yaml
name: change_count
type:
  kind: computed
  derive: count-changelog
  depends_on:
    - _changelog
```

At read time the entity layer notices the `_`-prefixed dependency, sources the value, writes it into `entity.fields` under that reserved name, runs the derivation, and then removes the reserved key before the entity is returned.

**Currently supported pseudo-fields.** Both are defined and injected by `EntityContext::inject_compute_dependencies` in `swissarmyhammer-entity/src/context.rs`:

- **`_changelog`** — the entity's JSONL changelog as a JSON array. Missing log file resolves to an empty array (`[]`), not an error — a newly created entity with no recorded mutations is a normal state, not a failure.
- **`_file_created`** — RFC 3339 timestamp from the entity file's `Metadata::created()`, falling back to `Metadata::modified()` on platforms or filesystems that don't support btime. Resolves to `Value::Null` when the file is missing or cannot be stat'd — this is a backstop signal, never the primary one, so derivations that depend on it must tolerate `null`.

Both are memoized through `EntityCache` when an `EntityCache` is attached, so list/read calls on a steady-state board don't re-read every task's changelog or re-stat every entity file. The cache invalidates on the mutation paths that would move these values.

**Adding a new pseudo-field.** This is a three-point change in `swissarmyhammer-entity/src/context.rs`:

1. Add a `want_<name>` branch in `inject_compute_dependencies` that reads the new source and inserts the value under `_<name>`.
2. Add `entity.fields.remove("_<name>")` to the strip block in `derive_compute_fields` (immediately after the engine's per-field loop) so the pseudo-field never leaks into persisted output.
3. Update the list in this section of `ARCHITECTURE.md` and in the docstring on `apply_compute_with_query`.

If the new pseudo-field is expensive (disk I/O, syscall), extend `EntityCache::get_or_load_compute_inputs` to memoize it alongside `_changelog` and `_file_created`. Uncached pseudo-fields are fine for cheap, in-memory derivations but cost real time under the `buffer_unordered` fan-out in batch list operations.

### Patterns

- **YAML-as-Schema**: YAML is the single source of truth. Rust provides implementations; the frontend interprets metadata. Adding a new field type means adding a YAML file — the UI renders it automatically.
- **VFS Content Stacking**: All YAML configuration uses the three-layer VFS (builtin → user → project). Any definition can be overridden at the project level without forking defaults.
- **File-Per-Entity Storage**: One file per entity, ID from filename. Git diffs are per-entity, merge conflicts are per-entity, and the `.kanban/` directory IS the board.
- **ULID Identifiers**: All entity IDs use ULIDs, generated monotonically. Time-ordered, collision-free, sort correctly as strings.
- **Fractional Indexing**: Task ordering uses the `Ordinal` type. Inserting between items computes a midpoint string — no renumbering needed.
- **Computed Fields**: Fields with `kind: computed` declare a `derive` function name. The `ComputeEngine` runs these when entities are read, injecting computed values.
- **Atomic File Writes**: Entity writes use temp-file-then-rename. No partial writes visible to watchers or other processes.
- **Leader Election**: Multi-process coordination uses OS file locks and ZMQ pub/sub. The leader owns write access; followers read and receive updates via the bus.

### Computed Fields and Pseudo-Field Dependencies

Computed fields (those with `kind: computed` in their YAML definition) are derived at read time by the `ComputeEngine`. Some derivations need data that isn't stored in the entity's own fields — for example, the JSONL changelog or the file's filesystem creation time. These are supplied through **pseudo-fields**: reserved `_`-prefixed names that `EntityContext` injects into `entity.fields` before derivation and strips out afterward so they are never persisted or returned to callers.

#### How a field opts in

A computed field declares its pseudo-field dependencies in its YAML definition via `depends_on`:

```yaml
name: created
type:
  kind: computed
  derive: derive-created
  depends_on:
    - _changelog
    - _file_created
```

The entity layer checks whether *any* computed field for the entity type declares a given dependency before loading it. If no field in the type needs `_changelog`, the changelog is never read — the injection is lazy per-dependency, not per-entity.

#### Supported pseudo-fields

| Name | Source | Value | Error / missing semantics |
|------|--------|-------|---------------------------|
| `_changelog` | The entity's `.jsonl` changelog file | `Value::Array` of serialized `ChangeEntry` objects | Empty array (`[]`) when the changelog file is missing or unreadable |
| `_file_created` | `Metadata::created()` on the entity's source file, falling back to `Metadata::modified()` when the platform/filesystem doesn't expose btime | `Value::String` — RFC 3339 timestamp | `Value::Null` when the file is missing or cannot be stat'd |

Both values are memoized in the `EntityCache` when one is attached, so repeated reads (e.g. listing 2000 tasks) don't re-read every changelog or re-stat every file.

#### Current consumers

- **`created`** (`derive-created`) — depends on `_changelog` and `_file_created`. Uses the earliest changelog timestamp, falling back to the file creation time.
- **`updated`** (`derive-updated`) — depends on `_changelog`. Uses the latest changelog timestamp.
- **`started`** (`derive-started`) — depends on `_changelog`. Scans changelog for the first move into an active column.
- **`completed`** (`derive-completed`) — depends on `_changelog`. Scans changelog for the move into the terminal column.

#### Adding a new pseudo-field

1. Add a branch in `EntityContext::inject_compute_dependencies` (`swissarmyhammer-entity/src/context.rs`) that checks `any_field_depends_on(owned_defs, "_name")` and inserts the value into `entity.fields`.
2. Add a corresponding `entity.fields.remove("_name")` in the strip block at the end of `EntityContext::derive_compute_fields`.
3. If an `EntityCache` is attached, add a cached loader path in `EntityCache::get_or_load_compute_inputs` alongside the existing `_changelog` / `_file_created` loaders.
4. Update this section with the new name, source, value format, and error semantics.

### Practices

1. **No feature flags.** The Cargo workspace says explicitly: "NEVER add features or feature flags." The only exception is `test-support` for test utilities.
2. **Entity IDs from filenames.** Never store an entity's ID inside the file. The filename IS the ID.
3. **ULID for all new IDs.** Never use UUIDs, auto-increment, or random strings.
4. **Builtin YAML is compiled in.** Changing a builtin YAML file requires recompilation.
5. **Command registration is centralized.** All command impls are registered in one `register_commands()` function. Don't scatter registration.
6. **CWD isolation via RAII.** Use `CurrentDirGuard` / `serial_test` for tests that touch the working directory. Never add production APIs to fix test environment problems.
7. **`.skills/` is generated.** Never edit files there directly. The source of truth is `builtin/skills/`.

---

## 2. MCP Architecture

### The McpTool Trait Hierarchy

Every MCP tool must satisfy three trait bounds:

```rust
pub trait McpTool: Doctorable + Initializable + Send + Sync { ... }
```

This means every tool is simultaneously health-checkable, lifecycle-managed, and MCP-callable.

#### Doctorable (health checks)

Defined in `swissarmyhammer-common`. Every component that can be diagnosed implements:

- `name()` / `category()` — identification and grouping
- `run_health_checks()` — returns a list of checks, each `Ok` / `Warning` / `Error` with an optional fix suggestion
- `is_applicable()` — skip checks that don't apply to the current environment

#### Initializable (lifecycle)

Defined in `swissarmyhammer-common`. Every component with setup/teardown needs implements:

- `init(scope)` / `deinit(scope)` — one-time project setup/teardown
- `start()` / `stop()` — runtime lifecycle, called when an MCP client connects/disconnects
- `priority()` — ordering (lower runs first for init, reverse for deinit)
- `is_applicable(scope)` — scope-aware filtering

Three scopes: `Project` (`.sah/`, `.skills/`), `Local` (Claude Code settings), `User` (global config).

#### McpTool (the tool itself)

Defined in `swissarmyhammer-tools`. The core tool interface:

- `name()` — unique identifier, conventionally `{category}_{action}`
- `description()` — typically loaded via `include_str!("description.md")`
- `schema()` — JSON Schema for argument validation
- `execute(arguments, context)` — the actual tool logic
- `operations()` — for operation-based tools, returns the verb/noun operation list

Two convenience macros reduce boilerplate: `impl_empty_doctorable!` and `impl_empty_initializable!` for tools that don't need health checks or lifecycle.

### Tool Registration and Discovery

Tools are registered into a `ToolRegistry` at server startup. Each tool category has a `register_*_tools()` function. The registry powers:

- **MCP `list_tools`** — returns all enabled tools with schemas
- **MCP `call_tool`** — dispatches to the tool by name
- **CLI subcommands** — the same tools are exposed via dynamic clap generation
- **Doctor** — iterates all tools calling `Doctorable::run_health_checks()`
- **Init/Deinit** — iterates all tools calling `Initializable::init()` / `deinit()`

### Transport

The MCP server supports two transports via the `rmcp` crate:

- **Stdio** (default) — standard input/output, the standard mode for Claude Code integration
- **HTTP** — Streamable HTTP transport for remote or multi-client scenarios

### Operation-Based Tools

Tools that handle multiple verbs on the same noun (like the kanban tool handling "add task", "list tasks", "move task") use the `Operation` trait for forgiving input parsing. The tool's `operations()` method returns the operation list, enabling:

- Automatic JSON schema generation from operation structs
- Dynamic CLI noun-verb subcommand generation
- Verb/noun routing from a single MCP tool entry point

### Patterns

- **McpTool = Doctorable + Initializable + Tool**: Every MCP tool is simultaneously health-checkable, lifecycle-managed, and callable. Enforced by the supertrait bound.
- **Operation = Struct + Execute**: Operations are structs with `#[operation]` proc macro. Fields are parameters. Generates MCP JSON schema and CLI args from the same type.
- **Dynamic CLI from Schema**: CLI programs generate clap command trees from the operation schema. Adding an `#[operation]` struct automatically adds a CLI subcommand.

### Practices

1. **Every tool implements all three traits.** Use `impl_empty_doctorable!` / `impl_empty_initializable!` if a tool has no health checks or lifecycle, but never skip the traits.
2. **init/deinit runs `Initializable` components in priority order.** Don't add setup logic outside the `Initializable` trait.
3. **Doctor collects from the tool registry.** Don't add health checks outside the `Doctorable` trait.

---

## 3. Agents, LLMs, and Embeddings

### Agent Architecture

The workspace has a dual-agent architecture: agents can run via the Claude CLI (cloud) or via local LLM inference (llama.cpp). Both backends speak ACP (Agent Communication Protocol) 0.11 — they construct an `agent_client_protocol::Agent` via its builder, register typed handlers for incoming requests and notifications, and connect to a transport with `connect_with(...)`. This makes the two backends interchangeable from a consumer's perspective: each one is a process that talks ACP over JSON-RPC 2.0 / stdio.

```
swissarmyhammer-agent (facade)
├── create_agent(ModelConfig) dispatches to:
│
├── claude-agent    — wraps the Claude CLI as a child process
│                     translates Claude API responses to/from ACP
│
└── llama-agent     — local LLM inference via llama.cpp
                      full ACP protocol implementation (JSON-RPC 2.0)
                      MCP client for tool calls
                      chat template engine
```

Consumers call `create_agent(ModelConfig)` and receive an `AcpAgentHandle` that carries an in-process ACP client plus a `SessionNotification` broadcast receiver. They never touch the underlying implementation. `ModelConfig` determines the backend — the `executor_type` field selects `ClaudeCode` or `LlamaAgent`.

### ACP (Agent Communication Protocol)

ACP is the protocol that makes agents interoperable with editors (Zed, JetBrains, etc.) and with each other. It defines:

- **Sessions** — stateful conversations with an agent
- **Prompt turns** — request/response pairs with streaming content blocks
- **Capabilities** — filesystem access, terminal execution, plans, slash commands
- **Permissions** — configurable policies (AlwaysAsk, AutoApproveReads, RuleBased)

The `llama-agent` crate contains the most complete ACP implementation, serving as a full protocol handler over JSON-RPC 2.0 / stdio. It builds its server via `agent_client_protocol::Agent.builder().on_receive_request(...).on_receive_notification(...).connect_with(transport, bridge)` — a single typed handler keyed on `ClientRequest` covers every ACP method (`initialize`, `authenticate`, `session/*`, plus extension channels), and the SDK demuxes by method name. The `acp-conformance` crate provides a protocol conformance test suite that validates any ACP backend against the spec.

### Subagent Metadata (not LLM inference)

The agent MCP tool (`swissarmyhammer-agents`) does NOT run LLM inference. It provides **metadata** — agent definitions loaded from AGENT.md files via the VFS — so the host agent (Claude Code or a local LLM) can adopt the persona, instructions, and tool configuration of a specialized subagent. Operations: `list agent`, `use agent`, `search agent`. Agent instructions are rendered through Liquid templates at load time.

### Embedding Architecture

Text embeddings power semantic search across code context and entities. Two backends implement the sealed `TextEmbedder` trait:

- **llama-embedding** — CPU/GPU embedding via llama.cpp with GGUF models
- **ane-embedding** — Apple Neural Engine embedding via CoreML (macOS only)

The `swissarmyhammer-embedding` facade selects the best backend for the current platform automatically. It handles long-text chunking with overlap and mean-pooling transparently. Model resolution goes through `model-loader`, which handles HuggingFace repo downloads and local path resolution with caching.

### Ralph: Persistent Agent Loop

Ralph prevents an autonomous agent from stopping while work remains. Used by skills like `finish` and `test-loop`.

1. A skill calls `set ralph` with an instruction and max_iterations (default 50)
2. Ralph writes a `.ralph/<session_id>.md` file with the instruction and iteration counter
3. When the agent's Stop hook fires, `check ralph` returns `"decision": "block"` if iterations remain
4. Each check increments the counter — the hard ceiling prevents infinite loops
5. When work is done, `clear ralph` releases the block

The iteration counter persists across `set ralph` calls so a skill cannot reset the safety cap.

### Patterns

- **Dual-Backend, Single Protocol**: Cloud and local LLM agents are interchangeable because both speak ACP. Consumers go through `swissarmyhammer-agent::create_agent` and never know which backend runs.
- **ACP as Protocol**: Agent interop uses the Agent Communication Protocol, not ad-hoc APIs. In ACP 0.11, backends are built by registering handlers on `Agent.builder()` and wiring them to a transport with `connect_with(...)` — there is no `impl Agent for MyBackend` trait to implement. The conformance test suite validates any new backend.
- **Metadata, Not Inference**: The subagent system provides persona/instructions to the host agent. It does not spawn LLM processes.
- **Platform-Aware Embedding**: The embedding facade selects the best backend (ANE on Apple Silicon, llama.cpp elsewhere) automatically.

### Practices

1. **New agent backends must build their server via `agent_client_protocol::Agent.builder()`.** In ACP 0.11 `Agent` is a unit struct, not a trait — register typed handlers with `.on_receive_request(...)` and `.on_receive_notification(...)`, then call `.connect_with(transport, bridge)` to run the dispatch loop. Don't create separate interfaces and don't try to `impl Agent for ...` (that pattern is the 0.10 contract and no longer exists).
2. **New agent backends must pass `acp-conformance`.** The conformance suite is the contract, not individual test cases.
3. **The `TextEmbedder` trait is sealed.** Only workspace crates can implement it. Don't expose it for external implementation.
4. **Ralph's iteration counter is a safety cap, not a feature.** Skills should `clear ralph` when done, not rely on hitting the ceiling.

---

## 4. Command Line Programs

The workspace produces several binaries. The specifics change — check `Cargo.toml` for the current list. The architectural patterns are stable:

### Dynamic CLI from Schema

CLI programs generate their clap command trees from the operation/command schema. Adding an `#[operation]` struct automatically adds a CLI subcommand. The CLI always matches the available operations without manual synchronization.

### init/deinit Pattern

CLI programs that integrate with Claude Code provide `init [project|local|user]` and `deinit` subcommands. These run all `Initializable` components in priority order for the given scope — registering the MCP server, creating project structure, deploying builtin skills/agents, and configuring Claude Code settings.

### Doctor Pattern

Every CLI has a `doctor` subcommand that runs all `Doctorable` health checks. System-level checks (PATH, file permissions, LSP servers) combine with per-tool checks from the `McpTool` registry for a unified diagnostic report.

### Dual-Mode Tauri Apps

Tauri apps operate in GUI mode by default but fall back to CLI mode when invoked with arguments. They share domain libraries with their CLI counterparts — the GUI and CLI are two views of the same engine.

### Practices

1. **The kanban board is the single source of truth for task tracking.** Not markdown files, not built-in task tools.
2. **Verify before claiming.** Always run tests, check logs, read output. Never guess, never ask the user to verify.

---

## 5. UI Programs

### Core Principle: State in Rust, Presentation in React

The frontend is a **presentation and command dispatch layer**. All application state lives in the Rust backend. React reads state (via Tauri queries and events), renders it, and dispatches commands back to Rust. React never computes domain logic, never owns authoritative state, and never constructs command objects.

### kanban-app

The kanban desktop app is a Tauri 2 application with a React frontend.

#### Rust Backend

The Tauri backend is a thin wiring layer:

- **AppState** holds the command registry, command impls, open boards (each a `BoardHandle` wrapping KanbanContext + StoreContext + EntityCache + SearchIndex), UIState, and native menu.
- **`dispatch_command`** is the single mutation entry point. All other `#[tauri::command]` functions are read-only queries.
- **File watcher** monitors `.kanban/` directories for external changes. SHA-256 content hashing avoids double-firing on its own writes.
- **Menu** is built dynamically from the CommandsRegistry and rebuilds on keymap changes, board switches, and focus changes.

#### Frontend

React 19 + Vite + TypeScript + Tailwind CSS 4 + Radix UI + dnd-kit + CodeMirror 6.

##### Container / View Separation

The frontend has two kinds of components with distinct roles:

**Containers** own state, provide context, and manage command scopes. A Container:
- Lives in one file
- Owns a `CommandScopeProvider` with a moniker
- Uses React context (providers, hooks) to expose state to descendants
- Wraps `children` — never renders domain-specific UI directly
- Handles event listeners, data fetching, and state management

**Views** (presenters) display data and dispatch user interactions. A View:
- Takes props or reads from context — never manages state
- Renders UI: layout, styles, interactive elements
- Dispatches commands via `useDispatchCommand` — never calls the backend directly
- Has no `CommandScopeProvider` of its own (it lives inside one from a Container)

This separation means Containers are testable for state management without rendering, and Views are testable for rendering without backend wiring.

##### Container Architecture

Each level of the scope hierarchy uses a Container component. The authoritative container tree is defined in `kanban-app/ui/src/App.tsx`, where each container has inline comments explaining its placement and dependencies. Refer to App.tsx for the current hierarchy rather than duplicating it here.

`AppModeContainer` is the first container inside the window because the interaction mode governs the entire UI surface — which keybindings are active, whether the toolbar shows a search field, which commands are available. It wraps NavBar, the content area, and everything else. Mode transitions (normal → command → search) are command-driven through Rust UIState.

##### Command Invocation Surfaces

Commands are defined and run in Rust but invoked from the frontend through four surfaces:

1. **Native menu bar** — built dynamically from CommandsRegistry YAML. Commands with `menu.path` appear in the macOS menu. Rebuilds on keymap/board/focus changes.
2. **Context menus** — commands with `context_menu: true` appear in right-click menus. The scope chain at the click point determines which commands are available and what parameters they receive.
3. **Command palette** — all commands with `visible: true` (default) appear in the Cmd+K palette. Fuzzy-searched by name.
4. **Buttons** — occasional direct button clicks dispatch commands via `useDispatchCommand`. This is the exception, not the norm.

All four surfaces use the same dispatch path: `useDispatchCommand` → scope chain resolution → Rust backend.

##### The Scope Chain and Monikers

A **moniker** is a `"type:id"` string that names a specific entity or scope boundary — for example `"window:main"`, `"board:01ABC"`, `"column:todo"`, or `"task:01XYZ"`. Monikers are the universal way to reference entities across the command system, inspector stack, context menus, and scope chain.

The **scope chain** is an ordered list of monikers representing the current context, from innermost (most specific) to outermost (least specific). It is built by walking the React component tree from the focused element to the root, collecting each `CommandScopeProvider`'s moniker along the way.

For example, when a user right-clicks a task card in the "To Do" column of the main window, the scope chain is:

```
["task:01XYZ", "column:todo", "view:board", "board:01ABC", "engine", "mode:normal", "window:main"]
  ↑ innermost                                                                      outermost ↑
```

The key innovation: **a command knows _where_ in the app it is being invoked**, not just what arguments it was given. The same `"ui.inspect"` command behaves differently when invoked from a task card (inspects the task) versus from a column header (inspects the column) — because the scope chain tells it the context. Commands don't need explicit arguments for information that's implicit in the user's focus. The scope chain, an explicit target moniker, and explicit params combine to give every command full situational awareness.

Commands use the scope chain in three ways:

1. **Availability**: A command with `scope: "entity:column"` is only available when a column moniker is in the chain. The `task.add` command requires a column context to know where to create the task.

2. **Parameter resolution**: A command parameter with `from: scope_chain` and `entity_type: column` resolves to `"todo"` by finding the nearest column moniker. No explicit argument is needed — the context provides it.

3. **Command resolution**: Commands resolve by walking the chain innermost to outermost. If a scope registers `"task.delete"` and `available` returns true, it runs. If `available` returns false, the command is **blocked** — parent scopes are not searched. If the command is not registered at all, the search continues outward. If no scope handles it, the command dispatches to the Rust backend.

`CommandScopeProvider` components form a linked list via React context. Each scope has:
- A `Map<string, CommandDef>` of registered commands
- A `parent: CommandScope | null` pointer
- An optional `moniker` string

The container tree directly produces the scope chain:

```tsx
<RustEngineContainer moniker="engine">              // outermost
  <WindowContainer moniker="window:main">
    <AppModeContainer moniker="mode:normal">        // wraps everything incl. toolbar
      <BoardContainer moniker="board:01ABC">
        <PerspectiveContainer moniker="perspective:p1">
          <ViewContainer moniker="view:board">
            <ColumnView moniker="column:todo">
              <TaskCard moniker="task:01XYZ">       // innermost — focused element
                {/* right-click here builds the chain above */}
              </TaskCard>
            </ColumnView>
          </ViewContainer>
        </PerspectiveContainer>
      </BoardContainer>
    </AppModeContainer>
  </WindowContainer>
</RustEngineContainer>
```

##### Command Flow: React to Rust

```
useDispatchCommand("task.add")
  │
  ├── Resolve scope chain from React context
  ├── Check for frontend execute handler
  │     ├── Found → call execute(), done
  │     └── Not found ↓
  │
  invoke("dispatch_command", {
    cmd: "task.add",
    scopeChain: ["task:abc", "column:todo", "window:main"],
    args: {...},
    boardPath: "/path/to/.kanban"
  })
  │
  ├── Prefix rewrite (view.switch:* → ui.view.set)
  ├── Look up CommandDef + Command impl
  ├── Build CommandContext with extensions
  ├── Check available(), execute()
  ├── Handle side effects (board switch, window create, quit, etc.)
  └── flush_and_emit() → entity events
```

##### Entity Flow: Rust to React

```
Command execution
  │
  flush_and_emit()
  │
  ├── StoreContext.flush_all() → detect changed files
  ├── Emit Tauri events:
  │     entity-created / entity-removed / entity-field-changed
  │     ui-state-changed (if UIState mutated)
  │
  React event listeners
  │
  ├── Structural types (column, swimlane) → full refresh
  ├── Entity types (task, tag) → re-fetch via get_entity
  └── EntityStoreProvider diffs by field → re-render only changed fields
```

Events are **signals to re-fetch**, not data carriers. The backend is always the source of truth.

##### Field-Level Subscriptions

The `EntityStoreProvider` holds all entities keyed by type. The `FieldSubscriptions` class manages per-field subscriptions via `useSyncExternalStore`. When entities change, it diffs old vs new by field value (deep equality), notifying only subscribers whose specific field changed. A task card whose `title` changes does not re-render if only `ordinal` was updated.

##### CodeMirror 6 as the Editor

CodeMirror 6 is the standard editor component for all text editing in the UI. **`TextEditor` (`fields/text-editor.tsx`) is the single shared CM6 component.** All editing contexts — field editing, inline rename, filter expressions — use `TextEditor` extended with props. Never create alternative editor components or use CM6 directly.

`TextEditor` provides:

- **Keymap consistency** — CUA, Vim, and Emacs keymap modes read from UIState and applied via `keymapExtension(mode)`. Every editor instance shares the same keymap config.
- **Smart rendering** — syntax highlighting, mention pills, markdown preview via `extraExtensions`
- **Behavioral modes via props:**
  - `singleLine` — Enter always commits (no newlines, even in vim insert mode), Escape commits immediately in vim (no two-escape normal-mode dance), blur commits. Use for inline rename, short inputs.
  - `popup` — auto-enters vim insert mode on mount. Use for quick-capture and inline editing where the user expects to type immediately.
  - `extraExtensions` — grammar/language support, autocomplete, mention decorations. Use to specialize editing behavior without forking the component.

When a new editing context needs different behavior, add a prop to `TextEditor` — do not duplicate CM6 setup.

### Patterns

- **Single Dispatch Path**: ALL state mutations route through `dispatch_command`. This gives every mutation undo/redo, event emission, UIState persistence, and audit logging for free.
- **Container / View Separation**: Containers own state and scope. Views take props and render. Never mix the two roles in one component.
- **Events as Signals**: Backend events trigger re-fetch, not state push. The frontend never trusts event payloads as authoritative data.
- **Scope Chain = Situational Awareness**: Commands know where they're invoked via the moniker chain. The same command behaves differently based on context.
- **SHA-256 Deduplication in Watcher**: Content hashing distinguishes external changes from the app's own writes, preventing event feedback loops.

### Practices

1. **State in Rust, presentation in React.** React never owns authoritative state. It reads from the backend, renders, and dispatches commands back.
2. **Commands in Rust.** All command logic lives in Rust. Never construct command objects or compute command results client-side.
3. **`useDispatchCommand` is the only dispatch mechanism.** Every command execution — keybindings, palette, context menus, drag-and-drop, programmatic triggers — must flow through `useDispatchCommand`. This is what gives us busy tracking (`inflightCount`), scope chain resolution, and client-side command handling. When commands originate from Rust (native menus, context menus), emit a Tauri event back to the frontend and let it dispatch — never call `dispatch_command_internal` directly from the Rust side. Direct `invoke()` is allowed only for read-only queries (`get_entity`, `list_views`, etc.).
4. **No new `#[tauri::command]` mutations.** All state changes go through `dispatch_command`.
5. **UI interprets Field metadata.** Never hardcode field-specific rendering logic in React. The YAML field properties drive rendering.
6. **No module-level dispatch functions.** Trace to the owning component and use the hook there.
7. **Container components own scope boundaries.** One file per container, wraps children only, no presentation logic.
8. **Views are pure presenters.** They receive data via props or context and dispatch commands via hooks. They never manage state or own scope providers.
9. **Events trigger re-fetch, not state push.** Always re-read from the backend.
10. **CodeMirror 6 is the only text editor.** Don't introduce alternative editor components.
11. **TDD for new containers and commands.** Write tests first (RED), implement (GREEN), refactor. Every container gets a `.test.tsx` file.
12. **`console.warn` for frontend instrumentation.** Check OS unified log, never ask the user to look at the browser console.
