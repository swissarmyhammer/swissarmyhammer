# Agent System: Per-Workflow System Prompts via ACP Modes

## Overview

Extend workflows to behave more like agents by allowing each workflow to define its own system prompt. System prompts are exposed as **ACP Modes**, providing a standard interface for agent personas across claude-agent, llama-agent, and future agent implementations.

## Architecture: Modes as Agent Personas

### Core Model

```
┌─────────────────────────────────────────────────────────────┐
│  ACP Mode System                                             │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  ModeRegistry (swissarmyhammer-modes)                   │ │
│  │                                                          │ │
│  │  builtin/modes/        → embedded at compile time       │ │
│  │  ~/.swissarmyhammer/   → user overrides                 │ │
│  │  .swissarmyhammer/     → project overrides              │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Mode                                                    │ │
│  │  - id: "planner"                                        │ │
│  │  - name: "Planner"                                      │ │
│  │  - description: "Architecture specialist"               │ │
│  │  - system_prompt: "..." (embedded content)              │ │
│  │  - prompt: ".system/planner" (OR reference to prompt)   │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  ACP SessionMode (agent-client-protocol)                │ │
│  │  - id: SessionModeId                                    │ │
│  │  - name: String                                         │ │
│  │  - description: Option<String>                          │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Key Principles

1. **Modes are the ACP interface** - Clients see and select modes via ACP protocol
2. **Modes can embed or reference prompts** - Flexibility for simple vs complex personas
3. **ModeRegistry handles the override stack** - builtin → user → project
4. **Setting a mode configures the agent** - System prompt is applied when mode is activated

## Mode Types

### Embedded Modes

Simple modes with content directly in the mode file:

```markdown
---
name: Explore
description: Fast agent for codebase exploration
---

You are a codebase exploration specialist optimized for quick discovery.

Your primary capabilities:
- Finding files by patterns
- Searching code for keywords
- Answering questions about codebase structure
```

### Prompt-Referencing Modes

Modes that reference prompts from the prompts system (supports Liquid templating and partials):

```markdown
---
name: Planner
description: Architecture and planning specialist
prompt: .system/planner
---
```

When a mode has `prompt:` set, the caller should load and render that prompt through the prompts system to get the final system prompt content.

## Available Modes

### Built-in Modes (10 total)

| ID | Name | Type | Purpose |
|----|------|------|---------|
| `general-purpose` | General Purpose | Embedded | Default general-purpose agent |
| `Explore` | Explore | Embedded | Codebase exploration |
| `Plan` | Plan | Embedded | Implementation planning |
| `default` | Default | Prompt-ref | General-purpose with coding standards |
| `planner` | Planner | Prompt-ref | Architecture and planning |
| `implementer` | Implementer | Prompt-ref | Code implementation with TDD |
| `reviewer` | Reviewer | Prompt-ref | Code review specialist |
| `tester` | Tester | Prompt-ref | Test writing and execution |
| `committer` | Committer | Prompt-ref | Git commit specialist |
| `rule-checker` | Rule Checker | Prompt-ref | Code quality analysis |

### System Prompts Location

Prompt-referencing modes use prompts from `builtin/prompts/.system/`:

```
builtin/prompts/
├── .system/                      # System prompts for modes
│   ├── default.md               # General-purpose coding assistant
│   ├── planner.md               # Planning specialist
│   ├── implementer.md           # Implementation specialist
│   ├── reviewer.md              # Code review specialist
│   ├── tester.md                # Test writing specialist
│   ├── committer.md             # Git commit specialist
│   └── rule-checker.md          # Rule checking agent
├── _partials/                    # Shared content blocks
│   ├── coding-standards.md
│   ├── test-driven-development.md
│   ├── tool_use.md.liquid
│   └── git-practices.md
└── ...
```

## Implementation Details

### Mode Struct (swissarmyhammer-modes)

```rust
pub struct Mode {
    /// Unique identifier (e.g., "planner")
    id: String,

    /// Human-readable name
    name: String,

    /// Description of when this mode should be used
    description: String,

    /// Embedded system prompt (used if `prompt` is None)
    system_prompt: String,

    /// Reference to a prompt path (e.g., ".system/planner")
    /// When set, load and render this prompt for the system prompt
    prompt: Option<String>,

    /// Path to the source file
    source_path: Option<PathBuf>,
}

impl Mode {
    /// Check if this mode uses a prompt reference
    pub fn uses_prompt_reference(&self) -> bool {
        self.prompt.is_some()
    }

    /// Get the prompt reference path
    pub fn prompt(&self) -> Option<&str> {
        self.prompt.as_deref()
    }
}
```

### Mode File Formats

**Embedded content:**
```markdown
---
name: mode-name
description: Mode description
---

System prompt content here...
```

**Prompt reference:**
```markdown
---
name: mode-name
description: Mode description
prompt: .system/mode-name
---
```

### ACP Config Integration (llama-agent)

```rust
// llama-agent/src/acp/config.rs

impl AcpConfig {
    /// Load available modes from ModeRegistry
    pub fn load_modes_from_registry(&mut self) {
        let mut registry = swissarmyhammer_modes::ModeRegistry::new();

        match registry.load_all() {
            Ok(modes) => {
                self.available_modes = modes
                    .into_iter()
                    .map(|mode| {
                        let mode_id = SessionModeId::new(mode.id().to_string());
                        SessionMode::new(mode_id, mode.name().to_string())
                            .description(mode.description().to_string())
                    })
                    .collect();
            }
            Err(e) => {
                tracing::warn!("Failed to load modes from registry: {}", e);
            }
        }
    }

    /// Create config with modes from registry
    pub fn with_registry_modes() -> Self {
        let mut config = Self::default();
        config.load_modes_from_registry();
        config
    }
}
```

### Setting a Mode

When a client calls `session/set-mode`:

1. ACP server validates mode is in available_modes
2. Server looks up Mode from ModeRegistry
3. If mode has `prompt:` reference:
   - Load prompt from PromptLibrary
   - Render with session context (working_directory, etc.)
   - Apply rendered content as system prompt
4. If mode has embedded content:
   - Use system_prompt directly
5. Send CurrentModeUpdate notification to client

### Claude Agent Specifics

For `claude-agent`, modes from Claude CLI's init message are used. Our custom modes can be added alongside:

```rust
// Merge Claude CLI modes with our modes
let mut available_modes = self.get_claude_cli_modes().await;
let our_modes = ModeRegistry::new().load_all()?;
available_modes.extend(our_modes);
```

When a mode is activated, pass to Claude CLI via `--agent` flag if it's a Claude mode, or via `--append-system-prompt` if it's our mode.

## Workflow Integration

Workflows specify their mode in frontmatter:

```yaml
---
title: Planning Workflow
mode: planner           # References mode by ID
parameters:
  - name: plan_filename
    required: true
---
```

When workflow starts:
1. Look up mode by ID from ModeRegistry
2. Create ACP session
3. Set mode on session (triggers system prompt loading)
4. Execute workflow states within this session

## Override Stack

Same three-tier hierarchy as all SwissArmyHammer resources:

1. `builtin/modes/` - Embedded in binary
2. `~/.swissarmyhammer/modes/` - User overrides
3. `.swissarmyhammer/modes/` - Project overrides

A project can override the `planner` mode by creating `.swissarmyhammer/modes/planner.md`.

## Benefits

1. **Standard Interface**: Modes are the ACP way to handle agent personas
2. **Flexible Content**: Embed simple prompts or reference complex templated ones
3. **Override Stack**: Users/projects can customize modes
4. **Discovery**: Clients can query available modes via ACP
5. **Type Safety**: Mode operations are validated at the ACP level

## Success Criteria

- [x] Mode struct supports both embedded content and prompt references
- [x] ModeRegistry loads modes from builtin/user/local directories
- [x] 10 builtin modes created (3 embedded, 7 prompt-referencing)
- [x] System prompts created in `.system/` folder with shared partials
- [x] llama-agent loads modes from ModeRegistry into ACP config
- [x] Workflows can specify mode in frontmatter
- [x] Rule checker uses `rule-checker` mode
- [ ] claude-agent merges our modes with Claude CLI modes
- [ ] Mode switching loads/renders prompt references
- [ ] Documentation updated with examples
