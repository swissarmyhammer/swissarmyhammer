# CLI Progress Output Redesign

## Problem

`sah init` and `sah deinit` produce unstructured, inconsistent output. Two layers
print independently with no visual hierarchy:

1. **sah components** (`swissarmyhammer-cli/src/commands/install/components/mod.rs`) — ~15 `println!` calls
2. **mirdan library** (`mirdan-cli/src/install.rs`) — ~50 `println!` calls in deploy/undeploy functions

Current output looks like:
```
sah MCP server installed for Claude Code (/path/to/.mcp.json)
Bash tool denied in /path/to/.claude/settings.json (use shell tool instead)
Project structure initialized at /path/to/.swissarmyhammer
Installed 12 builtin skills (lockfile updated)
Installed 7 builtin agents (lockfile updated)
Statusline installed in /path/to/.claude/settings.json
```

No timing, no visual structure, no consistent formatting.

## Goal

Cargo/bun-style output with right-aligned verbs, consistent coloring, and timing:

```
  sah init

   Detecting  coding agents...
      Found   Claude Code, GitHub Copilot, Zed
  Registering MCP server for 3 agents
  Configuring permissions (deny Bash)
  Configuring statusline
  Scaffolding .swissarmyhammer/
  Scaffolding .prompts/
   Deploying  12 builtin skills
   Deploying  7 builtin agents
     Writing  mirdan-lock.json

   Finished   sah init in 0.08s
```

Zero `println!` in component or library code. All output flows through a reporter.

## Design: Event Channel + Renderer

### InitEvent enum

```rust
enum InitEvent {
    StepStarted { verb: String, message: String },
    StepCompleted { verb: String, message: String },
    Warning { message: String },
    Error { message: String },
}
```

### InitReporter trait

Passed to components (and eventually mirdan) so they emit events instead of printing:

```rust
trait InitReporter: Send + Sync {
    fn emit(&self, event: InitEvent);
}
```

### Two implementations

- **CliReporter** — renders cargo-style to stderr with `console` crate coloring
- **NullReporter** — discards events (for tests, library consumers who don't care)

### Renderer details

- Green right-aligned verb, 12-char wide field (the cargo convention)
- `console::style()` for colors (already a transitive dep via `indicatif`)
- Yellow for warnings, red for errors
- `Instant::now()` at top, elapsed printed at "Finished" line
- Future: swap in `indicatif` spinners for slow steps without touching component code

## The mirdan Problem

mirdan's library functions (`deploy_skill_to_agents`, `register_mcp_server`,
`unregister_mcp_server`, etc.) contain ~50 hardcoded `println!` calls. Both
`mirdan-cli` and `sah` call these functions, so both get mirdan's raw output
mixed into their own.

### Recommended approach: structured return values

mirdan functions should return structured results instead of printing:

```rust
// In mirdan library
struct DeployResult {
    action: DeployAction,  // Stored, Linked, Registered, Removed, etc.
    target: String,        // path or agent name
    detail: Option<String>,
}

enum DeployAction {
    Stored,
    Linked,
    Registered,
    Removed,
    Skipped,
}

// deploy_skill_to_agents returns Vec<DeployResult> instead of printing
```

- **mirdan-cli** formats `DeployResult` for its own CLI (preserving current output style)
- **sah** formats through its `InitReporter` channel (cargo-style)
- Tests can assert on structured results instead of capturing stdout

### Migration path

1. Add `DeployResult` / `DeployAction` types to mirdan's library
2. Replace `println!` in `mirdan-cli/src/install.rs` with pushing to a results vec
3. Have deploy/undeploy functions return `Vec<DeployResult>`
4. `mirdan-cli` main formats results (preserves current behavior)
5. sah components map `DeployResult` → `InitEvent` and emit through reporter

### Scope

~50 println calls in mirdan to convert. The functions affected:

- `deploy_skill_to_agents()` — stored, linked
- `deploy_agent_to_agents()` — stored, linked
- `deploy_tool_to_agents()` — stored, linked, registered
- `deploy_plugin_to_agents()` — deployed, registered
- `uninstall_skill()` — removed link, removed store
- `uninstall_agent()` — removed link, removed store
- `uninstall_tool()` — unregistered, removed store
- `uninstall_plugin()` — removed
- `register_mcp_server()` / `unregister_mcp_server()`
- `install_from_registry()`, `install_from_local_path()`, `install_from_git()`

## sah-side Changes

### Lifecycle trait update

Thread reporter through `InitScope` → `InitContext`:

```rust
struct InitContext {
    scope: InitScope,
    reporter: Arc<dyn InitReporter>,
}
```

Or pass reporter separately to `Initializable::init(&self, scope, reporter)`.

### Components to update

All in `swissarmyhammer-cli/src/commands/install/components/mod.rs`:

| Component | println calls to remove |
|---|---|
| McpRegistration | 4 (init) + 4 (deinit) |
| ClaudeLocalScope | 4 (init) + 4 (deinit) |
| DenyBash | 1 (init) + 1 (deinit) |
| ProjectStructure | 1 (init) + 2 (deinit) |
| SkillDeployment | 1 (init) + via mirdan (deinit) |
| AgentDeployment | 1 (init) + via mirdan (deinit) |
| LockfileCleanup | 1 (deinit) |

Plus `init.rs` and `deinit.rs` orchestrators add header/footer/timing.

### Orchestrator (`init.rs` / `deinit.rs`)

```rust
pub fn install(target: InstallTarget) -> Result<(), String> {
    let reporter = Arc::new(CliReporter::new());
    reporter.emit(InitEvent::header("sah init"));

    let start = Instant::now();
    let context = InitContext { scope: target.into(), reporter: reporter.clone() };

    // ... run registry ...

    reporter.emit(InitEvent::finished("sah init", start.elapsed()));
}
```

## Inspiration

- **cargo** — green right-aligned verb prefix, the gold standard for Rust CLIs
- **bun** — minimal, fast, checkmarks + timing
- **pnpm** — clean progress, color-coded phases
- **create-t3-app / Astro CLI** — step-by-step with spinners

## Crates

- `console` — terminal colors, styling (already a transitive dep via indicatif)
- `indicatif` — already a workspace dependency, use for spinners on slow steps later

## Phasing

1. **Phase 1**: Add `InitReporter` trait + `CliReporter` to `swissarmyhammer-common`
2. **Phase 2**: Thread reporter through sah components, remove all `println!` from components
3. **Phase 3**: Add `DeployResult` to mirdan, refactor mirdan functions to return structured data
4. **Phase 4**: Update `mirdan-cli` to format from structured results
5. **Phase 5**: sah components consume mirdan's structured results through reporter

Phase 1-2 can ship independently. Phase 3-5 is the bigger mirdan refactor.
