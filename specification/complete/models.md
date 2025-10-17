# Agent Management API and CLI Specification

## Overview

Add a new top-level `sah agent` command to manage built-in agent configurations. The command will discover agent configurations from embedded resources and allow users to easily switch between them.

## Requirements

### CLI Interface

```bash
# List all available built-in agents
sah agent list

# Use a specific agent configuration
sah agent use <agent_name>
```

### Built-in Agent Compilation

Built-in agents need to be compiled into the binary as embedded resources, similar to prompts and workflows:

#### Build Script Integration

Add to `swissarmyhammer-config/build.rs`:

```rust
fn generate_builtin_agents(out_dir: &str) {
    let dest_path = Path::new(&out_dir).join("builtin_agents.rs");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let builtin_dir = Path::new(&manifest_dir).join("../builtin/agents");

    let mut code = String::new();
    code.push_str("// Auto-generated builtin agents - do not edit manually\n");
    code.push_str("/// Get all built-in agents as a vector of (name, content) tuples\n");
    code.push_str("pub fn get_builtin_agents() -> Vec<(&'static str, &'static str)> {\n");
    code.push_str("    vec![\n");

    if builtin_dir.exists() {
        for entry in fs::read_dir(&builtin_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let name = path.file_stem().unwrap().to_str().unwrap();
                code.push_str(&format!(
                    "        (\"{}\", include_str!(\"{}\")),\n",
                    name,
                    path.display()
                ));
            }
        }
    }
    
    code.push_str("    ]\n");
    code.push_str("}\n");
    
    fs::write(&dest_path, code).unwrap();
}
```

### Agent Discovery Hierarchy

Following the same pattern as prompts and workflows, agents are discovered in this order:

1. **User agents** (highest precedence) - `.swissarmyhammer/agents/`
2. **Project agents** - `agents/` directory in project root
3. **Built-in agents** (lowest precedence) - compiled into binary from `builtin/agents/`

#### Built-in Agent Discovery

- Agents compiled into binary from `builtin/agents/` directory at build time
- Use file stem (filename without extension) as agent name
- Example: `builtin/agents/qwen-coder.yaml` → agent name `qwen-coder`
- Access via `include!(concat!(env!("OUT_DIR"), "/builtin_agents.rs"));`

#### User Agent Discovery

- Scan `.swissarmyhammer/agents/` for `.yaml` files
- Allow users to override built-in agents with same name
- Enable custom agent configurations without modifying project files

### Configuration Management

- Check for existing config in order:
  1. `.swissarmyhammer/sah.yaml`
  2. `.swissarmyhammer/sah.toml`
- If neither exists, create `.swissarmyhammer/sah.yaml`
- Replace the `agent:` section in the config file with the selected built-in agent configuration

### API Design (swissarmyhammer-config)

```rust
// Include generated builtin agents
include!(concat!(env!("OUT_DIR"), "/builtin_agents.rs"));

// Agent management operations
pub struct AgentManager;

impl AgentManager {
    /// List all available agents from all sources
    pub fn list_agents() -> Result<Vec<AgentInfo>, AgentError> {
        let mut agents = Vec::new();
        
        // 1. Load built-in agents (lowest precedence)
        for (name, content) in get_builtin_agents() {
            let description = parse_agent_description(content);
            agents.push(AgentInfo {
                name: name.to_string(),
                content: content.to_string(),
                source: AgentSource::Builtin,
                description,
            });
        }
        
        // 2. Load project agents (medium precedence)
        if let Ok(project_agents) = load_project_agents() {
            for agent in project_agents {
                // Replace builtin agent if same name exists
                if let Some(existing) = agents.iter_mut().find(|a| a.name == agent.name) {
                    *existing = agent;
                } else {
                    agents.push(agent);
                }
            }
        }
        
        // 3. Load user agents (highest precedence)
        if let Ok(user_agents) = load_user_agents() {
            for agent in user_agents {
                // Replace existing agent if same name exists
                if let Some(existing) = agents.iter_mut().find(|a| a.name == agent.name) {
                    *existing = agent;
                } else {
                    agents.push(agent);
                }
            }
        }
        
        Ok(agents)
    }
    
    /// Apply an agent configuration to the project config
    pub fn use_agent(agent_name: &str) -> Result<(), AgentError> {
        // Find the agent by name from all sources
        let agents = Self::list_agents()?;
        let agent = agents
            .iter()
            .find(|a| a.name == agent_name)
            .ok_or_else(|| AgentError::NotFound(agent_name.to_string()))?;
        
        // Parse the agent configuration
        let agent_config: AgentConfig = serde_yaml::from_str(&agent.content)?;
        
        // Find or create project config file
        // Replace agent section
        // Write back to disk
    }
    
    /// Load agents from .swissarmyhammer/agents/
    fn load_user_agents() -> Result<Vec<AgentInfo>, AgentError> {
        let agents_dir = Path::new(".swissarmyhammer/agents");
        Self::load_agents_from_dir(agents_dir, AgentSource::User)
    }
    
    /// Load agents from agents/ directory in project root
    fn load_project_agents() -> Result<Vec<AgentInfo>, AgentError> {
        let agents_dir = Path::new("agents");
        Self::load_agents_from_dir(agents_dir, AgentSource::Project)
    }
    
    /// Load agents from a specific directory
    fn load_agents_from_dir(dir: &Path, source: AgentSource) -> Result<Vec<AgentInfo>, AgentError> {
        let mut agents = Vec::new();
        
        if !dir.exists() {
            return Ok(agents);
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| AgentError::InvalidPath(path.clone()))?;
                
                let content = fs::read_to_string(&path)?;
                let description = parse_agent_description(&content);
                
                agents.push(AgentInfo {
                    name: name.to_string(),
                    content,
                    source,
                    description,
                });
            }
        }
        
        Ok(agents)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentSource {
    Builtin,
    Project,
    User,
}

pub struct AgentInfo {
    pub name: String,
    pub content: String,
    pub source: AgentSource,
    pub description: Option<String>,
}
```

### CLI Implementation (Thin Wrapper)

The CLI should be minimal, delegating all logic to the swissarmyhammer-config API and following the same output formatting pattern as other commands:

```rust
// In main CLI
match args.command {
    Command::Agent { subcommand } => match subcommand {
        AgentSubcommand::List { format } => {
            let agents = AgentManager::list_agents()?;
            display_agents(&agents, format.unwrap_or(OutputFormat::Table))?;
        }
        AgentSubcommand::Use { agent_name } => {
            AgentManager::use_agent(&agent_name)?;
            println!("Successfully switched to agent: {}", agent_name);
        }
    }
}

/// Display agents using consistent formatting with other commands
fn display_agents(agents: &[AgentInfo], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(agents)?;
            println!("{json}");
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(agents)?;
            print!("{yaml}");
        }
        OutputFormat::Table => {
            display_agents_table(agents)?;
        }
    }
    Ok(())
}

/// Display agents in table format following the same pattern as prompts
fn display_agents_table(agents: &[AgentInfo]) -> Result<()> {
    if agents.is_empty() {
        println!("No agents found.");
        return Ok(());
    }

    let builtin_agents: Vec<_> = agents.iter().filter(|a| matches!(a.source, AgentSource::Builtin)).collect();
    let project_agents: Vec<_> = agents.iter().filter(|a| matches!(a.source, AgentSource::Project)).collect();
    let user_agents: Vec<_> = agents.iter().filter(|a| matches!(a.source, AgentSource::User)).collect();

    let total_agents = agents.len();
    let builtin_count = builtin_agents.len();
    let project_count = project_agents.len();
    let user_count = user_agents.len();

    println!("🤖 Agents: {total_agents} total");
    println!("📦 Built-in: {builtin_count}");
    if project_count > 0 {
        println!("📁 Project: {project_count}");
    }
    if user_count > 0 {
        println!("👤 User: {user_count}");
    }
    println!();

    // Create a custom 2-line format like prompts
    let is_tty = atty::is(atty::Stream::Stdout);
    
    for agent in agents {
        let description = agent.description.as_deref().unwrap_or("");

        // First line: Name | Description (colored by source)
        let first_line = if is_tty {
            let (name_colored, desc_colored) = match &agent.source {
                AgentSource::Builtin => (
                    agent.name.green().bold().to_string(),
                    description.green().to_string(),
                ),
                AgentSource::User => (
                    agent.name.blue().bold().to_string(),
                    description.blue().to_string(),
                ),
                AgentSource::Project => (
                    agent.name.yellow().bold().to_string(),
                    description.yellow().to_string(),
                ),
            };
            if description.is_empty() {
                name_colored
            } else {
                format!("{} | {}", name_colored, desc_colored)
            }
        } else {
            if description.is_empty() {
                agent.name.clone()
            } else {
                format!("{} | {}", agent.name, description)
            }
        };

        // Second line: Source and executor info
        let executor_info = format!("source: {:?}", agent.source).to_lowercase();
        let second_line = if is_tty {
            executor_info.dimmed().to_string()
        } else {
            executor_info
        };

        println!("{}", first_line);
        println!("  {}", second_line);
        println!(); // Blank line between entries
    }

    Ok(())
}
```

## Implementation Notes

1. **Error Handling**: Graceful handling of missing files, invalid configs, permission errors
2. **Validation**: Validate built-in agent configs before applying
3. **Backup**: Consider backing up existing config before replacement
4. **Feedback**: Clear success/error messages for CLI users
5. **Discovery**: Recursive scanning if nested directories are needed later

## Future Enhancements

### Additional CLI Commands

```bash
# Display agent configuration details
sah agent show <agent_name>

# Show currently active agent
sah agent current

# Validate agent configuration
sah agent validate <agent_name>
```

### Enhanced API Design

```rust
impl AgentManager {
    /// Show details of a specific built-in agent
    pub fn show_agent(agent_name: &str) -> Result<AgentDetails, AgentError> {
        let agent_content = get_builtin_agents()
            .iter()
            .find(|(name, _)| *name == agent_name)
            .map(|(_, content)| *content)
            .ok_or_else(|| AgentError::NotFound(agent_name.to_string()))?;
        
        let agent_config: AgentConfig = serde_yaml::from_str(agent_content)?;
        
        Ok(AgentDetails {
            name: agent_name.to_string(),
            content: agent_content.to_string(),
            config: agent_config,
            executor_type: agent_config.executor_type(),
            description: parse_agent_description(agent_content),
        })
    }
    
    /// Get the currently active agent from project config
    pub fn get_current_agent() -> Result<Option<CurrentAgent>, AgentError> {
        // Read project config file
        // Parse agent section
        // Return current agent info
    }
    
    /// Validate a built-in agent configuration
    pub fn validate_agent(agent_name: &str) -> Result<ValidationResult, AgentError> {
        let agent_content = get_builtin_agents()
            .iter()
            .find(|(name, _)| *name == agent_name)
            .map(|(_, content)| *content)
            .ok_or_else(|| AgentError::NotFound(agent_name.to_string()))?;
        
        // Parse and validate configuration
        let validation_result = validate_agent_config(agent_content)?;
        Ok(validation_result)
    }
}

pub struct AgentDetails {
    pub name: String,
    pub content: String,
    pub config: AgentConfig,
    pub executor_type: AgentExecutorType,
    pub description: Option<String>,
}

pub struct CurrentAgent {
    pub name: Option<String>, // None if using custom config
    pub executor_type: AgentExecutorType,
    pub config_source: ConfigSource, // Builtin, Project, etc.
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
```

### Additional Features

- Agent configuration validation and linting
- Agent comparison functionality
- Export current config as new agent template
- Copy built-in agent to user directory for customization

### CLI Examples with User Agents

```bash
# List all agents (builtin, project, user)
sah agent list

# Use a user-defined agent
sah agent use my-custom-agent

# Copy built-in agent to user directory for customization
sah agent copy qwen-coder --to-user

# Show agent with source information
sah agent show qwen-coder
# Output: Agent: qwen-coder (source: user, overrides builtin)
```

### Directory Structure

```
project/
├── .swissarmyhammer/
│   ├── sah.yaml                    # Project config
│   └── agents/                     # User agents (highest precedence)
│       ├── my-custom-agent.yaml
│       └── qwen-coder.yaml         # Overrides builtin qwen-coder
├── agents/                         # Project agents (medium precedence)
│   └── team-agent.yaml
└── builtin/agents/                 # Built-in agents (compiled in, lowest precedence)
    ├── claude-code.yaml
    ├── qwen-coder.yaml
    └── qwen-coder-flash.yaml
```