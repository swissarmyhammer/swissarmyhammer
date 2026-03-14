# Subagent Implementation: Concrete Design for Llama-Agent

## 1. Parallelization Strategy

### The Actual Execution Model

Use **tokio::task::JoinSet** for efficient parallel subagent execution:

```rust
// In parent agent's tool handler
use tokio::task::JoinSet;

pub async fn parallelize_subagents(
    subagent_configs: Vec<SubagentConfig>,
    parent_session: &Session,
    agent_server: Arc<AgentServer>,
) -> Result<Vec<SubagentResult>> {
    let mut join_set = JoinSet::new();

    // Spawn each subagent as a concurrent task
    for (idx, config) in subagent_configs.into_iter().enumerate() {
        let agent_server = Arc::clone(&agent_server);
        let parent_session_id = parent_session.id.clone();

        join_set.spawn(async move {
            // Create subagent session
            let subagent_session = Session {
                parent_session_id: Some(parent_session_id.clone()),
                agent_role: Some(config.role.clone()),
                tool_restrictions: config.tool_restrictions.clone(),
                session_mode: config.session_mode.clone(),
                ..Default::default()
            };

            let subagent_id = agent_server
                .session_manager
                .create(subagent_session)
                .await?;

            // Send initial prompt to subagent
            agent_server
                .queue_generation_request(GenerationRequest {
                    session_id: subagent_id.clone(),
                    messages: vec![Message {
                        role: MessageRole::User,
                        content: config.task_description,
                        ..Default::default()
                    }],
                    max_tokens: config.max_tokens,
                })
                .await?;

            // Wait for completion
            let result = agent_server
                .wait_for_session_completion(&subagent_id, Duration::from_secs(300))
                .await?;

            Ok::<SubagentResult, SubagentError>(SubagentResult {
                subagent_id,
                role: config.role,
                output: result,
                execution_time: /* elapsed time */,
            })
        });
    }

    // Collect results as they complete (not in order!)
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(subagent_result)) => results.push(subagent_result),
            Ok(Err(e)) => {
                // Handle per-subagent failure
                tracing::error!("Subagent failed: {}", e);
                // Continue with other subagents
            }
            Err(e) => tracing::error!("Task join error: {}", e),
        }
    }

    Ok(results)
}
```

### Key Points

1. **JoinSet** returns results in completion order (not spawn order)
2. **Per-task error handling**: one subagent failure doesn't abort others
3. **Bounded concurrency**: RequestQueue already serializes model access
4. **Timeout per subagent**: prevent hanging operations
5. **Results collection**: gather all results before parent continues

### Bounded Concurrency Control

Use semaphore to limit concurrent active subagents:

```rust
pub struct SubagentExecutor {
    agent_server: Arc<AgentServer>,
    max_concurrent_subagents: usize,
    semaphore: Arc<Semaphore>,
}

impl SubagentExecutor {
    pub fn new(agent_server: Arc<AgentServer>, max_concurrent: usize) -> Self {
        Self {
            agent_server,
            max_concurrent_subagents: max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn spawn_subagent(&self, config: SubagentConfig) -> Result<SubagentHandle> {
        // Acquire permit (blocks if at max)
        let permit = self.semaphore.acquire().await?;

        // Spawn task
        let handle = self.agent_server.session_manager.create(/* ... */).await?;

        // Permit is held until dropped
        tokio::spawn(async move {
            let _guard = permit;
            // ... execute subagent ...
            // permit auto-drops when future completes
        });

        Ok(SubagentHandle { session_id: handle })
    }
}
```

---

## 2. Tool Integration: Expose as MCP Tool

### The spawn_subagent MCP Tool

Define as a standard tool in swissarmyhammer-tools:

```rust
// swissarmyhammer-tools/src/mcp/tools/agent/spawn_subagent.rs

pub struct SpawnSubagentTool;

#[async_trait]
impl Operation for SpawnSubagentTool {
    fn verb(&self) -> &'static str { "spawn" }
    fn noun(&self) -> &'static str { "subagent" }
    fn description(&self) -> &'static str {
        "Spawn a subagent to work on a specific task in parallel"
    }
    fn parameters(&self) -> &'static [ParamMeta] { &SPAWN_SUBAGENT_PARAMS }
}

pub async fn spawn_subagent(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let req: SpawnSubagentRequest = BaseToolImpl::parse_arguments(arguments)?;

    // Load agent definition from registry
    let agent_def = context.agent_library.get(&req.agent_type)?;

    // Create subagent session
    let session = Session {
        parent_session_id: Some(context.current_session_id.clone()),
        agent_role: Some(agent_def.role.clone()),
        tool_restrictions: agent_def.tool_restrictions.clone(),
        system_prompt: req.system_prompt.unwrap_or(agent_def.system_prompt),
        ..Default::default()
    };

    let subagent_id = context.agent_server.session_manager.create(session).await?;

    Ok(CallToolResult::Text {
        content: serde_json::json!({
            "subagent_id": subagent_id.to_string(),
            "agent_type": req.agent_type,
            "status": "spawned",
            "message": "Subagent ready for input. Send messages to this session ID."
        }).to_string(),
    })
}
```

### Request/Response Types

```rust
#[derive(Serialize, Deserialize)]
pub struct SpawnSubagentRequest {
    /// Agent type to spawn (from registry)
    pub agent_type: String,

    /// Task description for the subagent
    pub task: String,

    /// Optional system prompt override
    pub system_prompt: Option<String>,

    /// Session mode: "persistent" or "ephemeral"
    pub session_mode: SessionMode,

    /// Optional tool restrictions override
    pub tool_restrictions: Option<ToolRestrictions>,

    /// Max tokens for subagent to use
    pub max_tokens: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct SubagentResult {
    pub subagent_id: String,
    pub role: AgentRole,
    pub status: "running" | "completed" | "failed",
    pub output: Option<String>,
    pub error: Option<String>,
}
```

### Tool Registration

Register in acp/commands.rs alongside skill commands:

```rust
pub async fn list_available_tools(&self) -> Vec<ToolDefinition> {
    let mut tools = vec![
        // ... existing tools ...
        ToolDefinition {
            name: "spawn_subagent".to_string(),
            description: "Spawn a subagent to work on a task in parallel".to_string(),
            parameters: SPAWN_SUBAGENT_PARAMS.to_vec(),
        },
        ToolDefinition {
            name: "wait_for_subagent".to_string(),
            description: "Wait for a subagent to complete and get results".to_string(),
            parameters: WAIT_FOR_SUBAGENT_PARAMS.to_vec(),
        },
        ToolDefinition {
            name: "list_subagent_types".to_string(),
            description: "List available agent types that can be spawned".to_string(),
            parameters: vec![],
        },
    ];

    // Add dynamically loaded agent tools
    if let Some(library) = &self.agent_library {
        for agent in library.list() {
            tools.push(ToolDefinition {
                name: format!("spawn_{}", agent.name),
                description: agent.description.clone(),
                parameters: vec![ParamMeta {
                    name: "task".to_string(),
                    description: "Task description for the agent".to_string(),
                    required: true,
                    param_type: ParamType::String,
                }],
            });
        }
    }

    tools
}
```

---

## 3. Agent Enumeration & Loading

### Agent Registry (AGENT.md Format)

Define agents in `.agents/` directory, following skill pattern:

```yaml
# .agents/code-reviewer/AGENT.md
---
name: code-reviewer
description: Reviews code for quality, security, and best practices
role: CodeReviewer
model: default  # Use same model as parent
max-tokens: 4096
max-turns: 10
tool-restrictions:
  allowed: ["read_file", "grep", "glob"]
  max-execution-time: 300
system-prompt-override: |
  You are an expert code reviewer. Your job is to:
  1. Identify security vulnerabilities
  2. Check for performance issues
  3. Ensure code style compliance
  4. Suggest improvements
---

You are a specialized code reviewer. Analyze the provided code and report findings.

Focus areas:
- Security: SQL injection, auth bypasses, data leaks
- Performance: O(n²) loops, unnecessary allocations
- Style: Naming conventions, complexity
- Best practices: Error handling, documentation
```

```yaml
# .agents/test-runner/AGENT.md
---
name: test-runner
description: Runs tests and reports failures
role: Tester
model: default
max-tokens: 2048
tool-restrictions:
  allowed: ["bash", "read_file", "glob"]
  max-execution-time: 600
session-mode: ephemeral  # Fresh session each time
---

You run tests and report results. Execute tests and summarize failures concisely.
```

```yaml
# .agents/researcher/AGENT.md
---
name: researcher
description: Researches topics and synthesizes findings
role: Researcher
model: default
max-tokens: 8192
tool-restrictions:
  allowed: ["web/fetch", "web/search", "read_file", "grep"]
  max-execution-time: 300
session-mode: persistent  # Keep learning across calls
---

You are a research assistant. Investigate topics deeply and synthesize findings.
```

### Agent Loading System

```rust
// llama-agent/src/agents/mod.rs

pub struct AgentLibrary {
    agents: HashMap<String, AgentDefinition>,
    builtin: Vec<&'static str>,
}

pub struct AgentDefinition {
    pub name: String,
    pub description: String,
    pub role: AgentRole,
    pub model: Option<String>,
    pub max_tokens: usize,
    pub max_turns: usize,
    pub tool_restrictions: ToolRestrictions,
    pub system_prompt: String,
    pub session_mode: SessionMode,
}

impl AgentLibrary {
    pub async fn load(config_dir: &Path) -> Result<Self> {
        let mut agents = HashMap::new();

        // Load builtin agents
        for (name, def) in BUILTIN_AGENTS.iter() {
            agents.insert(name.to_string(), def.clone());
        }

        // Load from .agents/*/AGENT.md
        if config_dir.exists() {
            for entry in std::fs::read_dir(config_dir)? {
                let path = entry?.path();
                if path.is_dir() {
                    let agent_file = path.join("AGENT.md");
                    if agent_file.exists() {
                        let content = std::fs::read_to_string(&agent_file)?;
                        let agent = AgentDefinition::parse(&content)?;
                        agents.insert(agent.name.clone(), agent);
                    }
                }
            }
        }

        Ok(Self {
            agents,
            builtin: BUILTIN_AGENTS.keys().map(|s| *s).collect(),
        })
    }

    pub fn get(&self, name: &str) -> Result<&AgentDefinition> {
        self.agents.get(name)
            .ok_or_else(|| AgentError::UnknownAgent(name.to_string()))
    }

    pub fn list(&self) -> Vec<&AgentDefinition> {
        self.agents.values().collect()
    }

    pub fn names(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }
}

const BUILTIN_AGENTS: &[(&str, AgentDefinition)] = &[
    ("code-reviewer", AgentDefinition { /* ... */ }),
    ("test-runner", AgentDefinition { /* ... */ }),
    ("researcher", AgentDefinition { /* ... */ }),
];
```

### Discovery Precedence

Similar to skills, agents load in order:

1. **Builtin agents** (embedded at compile time)
2. **Project agents** (`.agents/*/AGENT.md`)
3. **User agents** (`~/.sah/agents/*/AGENT.md`)
4. **Extra search paths** (via environment variable)

---

## 4. How to Do This Better Than Claude Code

### Key Differences

**Claude Code** ❌
- Fixed set of agents built into the system
- Users can't add custom agents
- Agent definitions hardcoded in source
- No way to share or reuse agent definitions
- Limited to what Anthropic provides

**Llama-Agent** ✅ Better:

#### A. User-Defined Agents

Users can create custom agents in `.agents/` with standard AGENT.md format:

```yaml
# My custom agent
# .agents/security-auditor/AGENT.md
---
name: security-auditor
description: Audits code for OWASP top 10
role: SecurityAuditor
---

You are a security expert focusing on:
1. OWASP Top 10
2. Rust-specific vulnerabilities
3. Dependency auditing

Report findings with severity levels.
```

No code changes needed. Agent automatically available via `/spawn-subagent` tool.

#### B. Composable Agent Skills

Agents can load skills just like the main agent:

```yaml
# .agents/code-reviewer/AGENT.md
---
name: code-reviewer
skills:
  - code-quality
  - security-scanning
  - naming-conventions
tool-restrictions:
  allowed: [read_file, grep, glob]
---
```

Each skill adds its own prompts/instructions. Agents are **composable** units.

#### C. Open Agent Registry

Agents deployable via mirdan (skill package manager):

```bash
# Share agent definitions
mirdan agent publish security-auditor/

# Install from registry
mirdan agent install organization/security-auditor

# Works across all agents
mirdan agents  # See where each agent is available
```

#### D. Agent Inheritance

Agents can extend other agents:

```yaml
# .agents/strict-reviewer/AGENT.md
---
name: strict-reviewer
extends: code-reviewer
description: More strict code review
role: CodeReviewer
max-tokens: 8192  # More tokens for detailed analysis
system-prompt: |
  You are a STRICT code reviewer.
  $(base:system-prompt)  # Include base agent's prompt
  Additionally, check for:
  - Over-complex algorithms
  - Insufficient error handling
---
```

#### E. Parallel Subagent Coordination

Use the parent's perspective to orchestrate:

```text
User: "Analyze this code from security and performance angles"

Parent agent decides:
→ Spawn security-auditor in parallel
→ Spawn performance-auditor in parallel
→ Wait for both
→ Synthesize findings

Result: 50% faster than sequential review
Benefit: Anthropic's research showed 90% better quality
```

#### F. Dynamic Agent Selection

Parent agent can decide which subagents to spawn:

```text
User: "Review this PR"

Parent evaluates:
- Is this Rust code? → spawn rust-reviewer
- Does it touch auth? → spawn security-auditor
- Are tests included? → spawn test-verifier
- Configuration only? → skip performance-auditor

Dynamically spawn only necessary agents
```

#### G. Session Persistence

Some agents accumulate knowledge:

```yaml
# .agents/learning-reviewer/AGENT.md
---
name: learning-reviewer
session-mode: persistent
---

You are a learning code reviewer. Remember patterns from previous reviews.
If you've seen similar issues before, reference them.
```

Each project gets its own persistent subagent session.
Learns project conventions over time.

---

## 5. Complete Usage Example

### Parent Agent Spawning Multiple Subagents

```text
╔════════════════════════════════════════════════════════════════╗
║  User: "Review this PR for quality and security"              ║
╚════════════════════════════════════════════════════════════════╝

┌─ Parent Agent ──────────────────────────────────────────────────┐
│ Decision: This is Rust code touching crypto and auth           │
│                                                                  │
│ → Call /spawn-subagent tool                                    │
│   type: "security-auditor"                                    │
│   task: "Review auth.rs and crypto.rs for vulnerabilities"    │
│                                                                  │
│ → Call /spawn-subagent tool                                    │
│   type: "code-reviewer"                                        │
│   task: "Review overall code quality"                          │
│                                                                  │
│ → Call /spawn-subagent tool                                    │
│   type: "performance-auditor"                                  │
│   task: "Check for performance issues"                         │
│                                                                  │
│ [Awaits JoinSet to collect results...]                        │
│                                                                  │
│ Results:                                                        │
│ - security-auditor: "3 SQL injection risks in auth.rs"        │
│ - code-reviewer: "Cognitive complexity too high in main.rs"   │
│ - performance-auditor: "O(n²) loop in parser"                │
│                                                                  │
│ → Synthesize into consolidated report                         │
└─────────────────────────────────────────────────────────────────┘

         │                │                │
         ▼                ▼                ▼

    ┌─────────────┐ ┌─────────────┐ ┌──────────────┐
    │ Sec Audit   │ │ Code Review │ │ Performance  │
    │ Session s_1 │ │ Session s_2 │ │ Session s_3  │
    └─────────────┘ └─────────────┘ └──────────────┘
          │                │                │
          └────────────────┼────────────────┘
                           │
            ┌──────────────▼───────────────┐
            │  Shared AgentServer          │
            │  - llama.cpp model           │
            │  - RequestQueue serializes   │
            │  - Parallel sessions routed  │
            └──────────────────────────────┘
```

---

## 6. Advantages Over Claude Code

| Feature | Claude Code | Llama-Agent |
|---------|------------|------------|
| **Agent Definitions** | Hardcoded | Declarative YAML in .agents/ |
| **Custom Agents** | Can't add | Full support via AGENT.md |
| **Agent Sharing** | Not supported | Via mirdan registry |
| **Inheritance** | N/A | Agents can extend others |
| **Model Override** | Limited | Per-agent model selection |
| **Parallelization** | Built-in | Multi-session against shared model |
| **Session Modes** | Persistent only | Persistent + ephemeral |
| **Tool Isolation** | Built-in | Role-based restrictions |
| **Skills Integration** | Separate | Agents can compose skills |
| **Learning** | No | Persistent sessions accumulate knowledge |

---

## 7. Implementation Roadmap

### Phase 1: Core Parallelization
- [ ] Implement JoinSet-based parallel spawning
- [ ] Create spawn_subagent tool
- [ ] Add Session.parent_session_id
- [ ] Basic tool filtering by role

### Phase 2: Agent Registry
- [ ] Create AgentLibrary with AGENT.md parsing
- [ ] Load agents from .agents/ directory
- [ ] Add builtin agents (reviewer, tester, researcher)
- [ ] Register as MCP tools dynamically

### Phase 3: Advanced Features
- [ ] Agent inheritance (extends: field)
- [ ] Session persistence modes
- [ ] Semaphore-based bounded concurrency
- [ ] mirdan integration for agent sharing

### Phase 4: Polish
- [ ] Agent composition with skills
- [ ] Per-session agent customization
- [ ] Agent result caching
- [ ] Observability/logging improvements

---

## Key Files to Create/Modify

```
llama-agent/src/
├── acp/
│   ├── agents.rs (NEW - agent loading)
│   └── subagents.rs (NEW - spawn/execution)
├── agents/ (NEW directory)
│   ├── lib.rs
│   ├── registry.rs (AgentLibrary)
│   └── definition.rs (AgentDefinition)
└── session.rs (MODIFY - add parent_session_id, agent_role)

swissarmyhammer-tools/src/mcp/tools/
└── agent/ (NEW directory)
    ├── spawn_subagent.rs
    ├── wait_for_subagent.rs
    └── list_agents.rs

.agents/ (NEW directory - user-defined agents)
├── code-reviewer/
│   └── AGENT.md
├── test-runner/
│   └── AGENT.md
└── researcher/
    └── AGENT.md
```
