# Rules System: AI Linter for SwissArmyHammer

## Overview

The rules system will function as an AI-powered linter, applying automated checks and validations to code, documentation, and other project artifacts. Rules will be defined as markdown + liquid templates following the same structure as prompts, with both systems sharing the same underlying infrastructure for loading, rendering, and managing template-based content.

## Core Concept

**Rules and prompts are similar but distinct** - both are template-based content with metadata and hierarchical loading from multiple sources. The key differences are their **purpose, usage, and structure**:

- **Prompts**: Interactive templates for generating content or guiding user workflows
  - Have `parameters` that accept user input
  - Rendered with user-provided context variables
  
- **Rules**: Automated validation templates that check code/artifacts and report issues
  - Do NOT have `parameters` - they don't accept arguments
  - Rule template content is passed to `.check` prompt as the `{{rule}}` variable
  - Have `severity` field that prompts don't have
  - All rules check all files - LLM decides applicability

## Directory Structure

Rules will be stored in parallel to prompts with the same three-tier hierarchy:

```
builtin/rules/          # Built-in rules embedded in binary
~/rules/                # User-global rules from ~/.swissarmyhammer/rules/
.swissarmyhammer/rules/ # Project-local rules
```

## File Format

Rules follow the identical markdown + liquid format as prompts:

### Standard Rule File

```markdown
---
title: No Hardcoded Secrets
description: Detects hardcoded API keys, passwords, and tokens in code
category: security
severity: error
tags: ["security", "secrets", "credentials"]
---

Check the following {{ language }} code for hardcoded secrets.

Look for:
- API keys (e.g., API_KEY = "sk_live_...")
- Passwords in plain text
- Auth tokens
- Private keys

If this file type doesn't contain code (e.g., markdown, config files), respond with "PASS".

Report any findings with line numbers and suggestions for {{ target_path }}.
```

**Note:** Rules do NOT have `parameters` in their frontmatter. However, rule templates **are rendered** with these context variables before being passed to `.check`:
- `{{target_content}}` - The file content being checked
- `{{target_path}}` - Path to the file (e.g., `src/main.rs`)
- `{{language}}` - Detected programming language (e.g., `rust`, `python`)

**Two-stage rendering:**
1. Rule template is rendered with these variables → `rule_content`
2. `.check` prompt receives `{{rule_content}}` (the rendered rule) along with `{{target_content}}`, `{{target_path}}`, `{{language}}`

### Partial Rule Templates

```markdown
{% partial %}

Common patterns for {{ language }} code quality:

- Functions should be < 50 lines
- Cyclomatic complexity < 10
- No commented-out code blocks
```

## Shared Infrastructure

### Proposed Base Trait: `TemplateContent`

Both prompts and rules could implement a common trait (if we extract common base later):

```rust
pub trait TemplateContent {
    fn name(&self) -> &str;
    fn template(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn category(&self) -> Option<&str>;
    fn tags(&self) -> &[String];
    fn metadata(&self) -> &HashMap<String, serde_json::Value>;
    fn source(&self) -> Option<&PathBuf>;
    fn is_partial(&self) -> bool;
}
```

**Note:** `parameters()` is NOT in the base trait because rules don't have parameters. If we extract a common base, prompts would add parameters as a prompt-specific feature.

### Code Sharing Strategy: Duplicate and Specialize

After evaluation, **rules will NOT extract a common base** with prompts. Instead, we'll **duplicate the implementation pattern** for the following reasons:

1. **Different enough**: Rules lack parameters, have severity - enough differences to make generic code awkward
2. **Simpler maintenance**: Each crate is independent and can evolve separately
3. **No premature abstraction**: Extract common patterns only if we add a third similar system
4. **Proven pattern**: We already have working code in `swissarmyhammer-prompts` that we can copy

### What Rules Will Duplicate from Prompts

Rules will copy and adapt these modules from `swissarmyhammer-prompts`:

1. **`storage.rs`** - Storage backend trait and implementations
   - Copy `StorageBackend` trait
   - Copy `MemoryStorage` and `FileStorage`
   - Adapt for `Rule` instead of `Prompt`

2. **`frontmatter.rs`** - YAML frontmatter parsing
   - Copy `parse_frontmatter()` function
   - Already generic, works as-is

3. **File loading pattern** - From `prompts.rs::PromptLoader`
   - Copy directory scanning logic
   - Copy compound extension handling (`.md`, `.md.liquid`, `.liquid.md`)
   - Adapt to parse rule-specific metadata (severity, auto_fix)

4. **Resolver pattern** - From `prompt_resolver.rs`
   - Copy hierarchical loading (builtin → user → local)
   - Copy `VirtualFileSystem` integration
   - Copy source tracking pattern
   - Adapt for `Rule` type

5. **Library pattern** - From `prompts.rs::PromptLibrary`
   - Copy collection management (add/get/list/remove)
   - Copy filtering and search
   - **Remove** rendering logic (rules don't render themselves, they're rendered by `.check`)

### What Rules Will NOT Need

Rules will NOT need:

- **Rendering infrastructure**: Rules don't render themselves - they're passed to `.check` prompt
- **Partial adapter**: Rules use the `.check` prompt which handles partials via `PromptLibrary`
- **Parameter system**: Rules don't have parameters

### Implementation Summary

```
swissarmyhammer-rules/src/
├── lib.rs                  # Public API
├── rules.rs                # Rule, RuleLibrary, RuleLoader (copied pattern from prompts.rs)
├── storage.rs              # StorageBackend trait (copied from prompts)
├── frontmatter.rs          # parse_frontmatter (copied from prompts)
├── rule_resolver.rs        # RuleResolver (copied pattern from prompt_resolver.rs)
├── rule_filter.rs          # RuleFilter (similar to PromptFilter)
├── severity.rs             # Severity enum (new)
├── checker.rs              # RuleChecker with .check prompt integration (new)
└── language.rs             # Language detection (new)
```

### Why This Approach

- **Fast to implement**: Copy working code, adapt for rules
- **Independent evolution**: Rules can change without affecting prompts
- **Clear ownership**: Each crate owns its implementation
- **No premature optimization**: If we add more similar systems later (e.g., "templates", "checks"), THEN extract common base
- **Follows Rust philosophy**: Composition over inheritance, explicit over implicit

## Rule-Specific Additions

### Metadata Fields

Rules have specific frontmatter fields:

```yaml
severity: error | warning | info | hint
auto_fix: true         # Whether the rule can auto-fix issues (future)
```

**Note:** No `applies_to` field - all rules are checked against all files. The LLM decides if a rule is applicable to a given file.

### Rule Structure

```rust
pub struct Rule {
    // Shared fields (similar to Prompt but NO parameters)
    pub name: String,
    pub template: String,              // The rule content (checking instructions)
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<PathBuf>,
    pub metadata: HashMap<String, serde_json::Value>,
    
    // Rule-specific fields
    pub severity: Severity,
    pub auto_fix: bool,                // Future: whether rule can auto-fix
}

pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}
```

**Important:** 
- `Rule` does NOT have a `parameters: Vec<Parameter>` field because rules don't take parameters
- `Rule` does NOT have an `applies_to` field - all rules check all files, LLM decides applicability
- The rule template is passed as-is to the `.check` prompt

## Implementation Strategy (Logical Dependency Order)

### Phase 1: Add `.check` Builtin Prompt (No Dependencies)

**Why first:** This requires no new crates and can be tested independently.

1. Create `builtin/prompts/.check.md` in `swissarmyhammer-prompts`
2. Define parameters: `rule`, `target_content`, `target_path`, `language`
3. Design prompt template for rule checking
4. Test prompt renders correctly with mock data via `sah prompt test .check`
5. **Deliverable:** Working `.check` prompt that can be manually tested

### Phase 2: Create Agent Crate (Depends on: prompts)

**Why second:** Needed by rules for LLM execution, depends on prompts for rendering.

1. Create `swissarmyhammer-agent` crate
2. Add dependency on `swissarmyhammer-prompts` (for prompt rendering)
3. Define `Agent` trait for LLM invocation
4. Implement basic `OpenAIAgent` or `AnthropicAgent`
5. Add `AgentConfig` for model, temperature, etc.
6. Test basic prompt execution: load prompt → render → execute → get response
7. **Deliverable:** Working agent that can execute prompts via LLM

**Important simplifications:**
- **No timeouts needed** - Let LLM requests run as long as they need
- **No security features needed** - Agent is internal infrastructure, not exposed to untrusted input
- **No rate limiting** - Simple, straightforward execution
- **No retries** - If it fails, it fails (can be added later if needed)

### Phase 3: Create Rules Crate Structure (Depends on: prompts, agent)

**Why third:** Rules need both prompts (for `.check`) and agent (for execution).

**Copy and adapt from `swissarmyhammer-prompts`:**

1. Create `swissarmyhammer-rules` crate
2. Add dependencies:
   ```toml
   swissarmyhammer-prompts = { path = "../swissarmyhammer-prompts" }
   swissarmyhammer-agent = { path = "../swissarmyhammer-agent" }
   ```
3. **Copy `storage.rs`** → Adapt `StorageBackend` for `Rule` type
4. **Copy `frontmatter.rs`** → Use as-is (already generic)
5. Create `Severity` enum (Error/Warning/Info/Hint)
6. Define `Rule` struct with fields: name, template, description, category, tags, severity, auto_fix
7. **Deliverable:** Basic rule data structures

### Phase 4: Implement Rule Loading (Depends on: Phase 3)

**Why fourth:** Need Rule struct before we can load rules.

1. **Copy loading logic from `prompts.rs`** → Create `RuleLoader`
   - Directory scanning with `.md`, `.md.liquid` extensions
   - Frontmatter parsing using copied `frontmatter.rs`
   - Parse rule-specific metadata: `severity`, `auto_fix`
2. **Copy resolver pattern from `prompt_resolver.rs`** → Create `RuleResolver`
   - Hierarchical loading (builtin → user → local)
   - VirtualFileSystem integration
   - Source tracking with `FileSource`
3. **Copy library pattern from `prompts.rs`** → Create `RuleLibrary`
   - Collection management (add/get/list/remove/search)
   - Filtering by source, category, tags, severity
   - **Exclude** rendering (rules don't render themselves)
4. Create `build.rs` to embed builtin rules (copy from prompts)
5. Add test builtin rules in `builtin/rules/`
6. **Deliverable:** Can load rules from all three tiers (builtin/user/local)

### Phase 5: Implement Rule Checking (Depends on: Phase 4, Phase 2, Phase 1)

**Why fifth:** Needs loaded rules, working agent, and `.check` prompt.

1. Create `language.rs` module
   - Use existing tree-sitter dependency (no new deps needed)
   - Implement `detect_language(path, content) -> String`
   - Use tree-sitter's language detection capabilities
2. Create `checker.rs` module
   - Implement `RuleChecker::new(agent)` - loads `PromptLibrary` for `.check`
   - Implement `check_all(rules, targets) -> Result<(), RuleError>`:
     - For each rule × target combination (all rules check all files)
     - Read file content
     - Detect language
     - Create context with: rule, target_content, target_path, language
     - Render `.check` prompt via `PromptLibrary::render()`
     - Execute via agent
     - Parse response and collect violations
3. Define `RuleViolation` struct:
   ```rust
   pub struct RuleViolation {
       pub rule_name: String,
       pub file_path: PathBuf,
       pub severity: Severity,
       pub message: String,  // Full LLM response
   }
   ```
4. Define `RuleError::Violation` variant for fail-fast behavior
4. Add unit tests for checking logic
5. **Deliverable:** Working rule checker that can check files

### Phase 6: CLI List Command (Depends on: Phase 4)

**Why sixth:** Can be built independently once rules can be loaded, doesn't need checking.

1. Create `swissarmyhammer-cli/src/commands/rule/` directory
2. **Copy `mod.rs` pattern** from `commands/prompt/mod.rs`
3. **Copy `cli.rs` pattern** from `commands/prompt/cli.rs` → Adapt for `RuleCommand` enum
4. **Copy `display.rs`** from `commands/prompt/display.rs` → Create `RuleRow` and `VerboseRuleRow`
   - Add `severity` column
   - Keep emoji-based source display (📦 Built-in, 📁 Project, 👤 User)
5. **Copy `list.rs`** from `commands/prompt/list.rs` → Adapt for rules
   - Load rules via `RuleResolver` and `RuleLibrary`
   - Filter out partials (if rules support them)
   - Convert to display rows
   - Support table/JSON/YAML formats
6. Wire up in main CLI router
7. **Deliverable:** `sah rule list` command works

### Phase 7: CLI Check Command (Depends on: Phase 5, Phase 6)

**Why seventh:** Needs working checker and display infrastructure.

1. Create `check.rs` in `commands/rule/`
2. Define `CheckCommand` struct:
   - `patterns: Vec<String>` (glob patterns)
   - `--rule`, `--severity`, `--category` filters
   - `--fail-on-violations` flag
3. Implement `execute_check_command()`:
   - Load all rules via `RuleResolver`
   - Apply filters
   - Expand glob patterns to file list
   - Create `RuleChecker` with agent
   - Run `check_all()`
   - Display violations with formatting
   - Show summary (passed/failed counts)
   - Exit with error code if `--fail-on-violations`
4. Wire up in CLI router
5. **Deliverable:** `sah rule check [glob...]` command works

### Phase 8: Integrate Agent into Workflow (Future)

**Why last:** Optional improvement, doesn't block rules functionality.

1. Update `swissarmyhammer-workflow` to depend on `swissarmyhammer-agent`
2. Implement `prompt` workflow action using agent
3. Unify LLM invocation across rules and workflows
4. **Deliverable:** Workflows can execute prompts via agent

## Dependency Graph

```
Phase 1: .check prompt
         ↓
Phase 2: Agent crate (depends on prompts for .check)
         ↓
Phase 3: Rules crate structure (depends on agent + prompts)
         ↓
Phase 4: Rule loading (depends on Phase 3)
         ↓
Phase 5: Rule checking (depends on Phase 4, 2, 1)
         ↓ ↘
Phase 6: CLI list (depends on Phase 4)
         ↓
Phase 7: CLI check (depends on Phase 5, 6)
         ↓
Phase 8: Workflow integration (depends on Phase 2, optional)
```

## Critical Path Testing

After each phase, test to ensure foundation is solid:

- **Phase 1:** `sah prompt test .check --vars rule="test rule" target_content="test code" target_path="test.rs" language="rust"`
- **Phase 2:** Agent integration test with simple prompt execution
- **Phase 4:** Load and list rules from all three tiers
- **Phase 5:** Check a single test file against a test rule
- **Phase 6:** `sah rule list` shows all rules
- **Phase 7:** `sah rule check "test/**/*.rs"` end-to-end test

## CLI Commands

### `sah rule list`

Lists all available rules with filtering and display options. **Must be consistent with `sah prompt list` implementation.**

#### Implementation Pattern

The `sah rule list` command should mirror `sah prompt list`:

```rust
// swissarmyhammer-cli/src/commands/rule/list.rs
pub async fn execute_list_command(cli_context: &CliContext) -> Result<()> {
    // Load all rules from all sources (builtin → user → local)
    let mut library = RuleLibrary::new();
    let mut resolver = RuleResolver::new();
    resolver.load_all_rules(&mut library)?;

    // Build filter (no source/category filtering for basic list)
    let filter = RuleFilter::new();

    // Get file sources for emoji display
    let mut file_sources = HashMap::new();
    let mut rule_sources = HashMap::new();
    for (name, source) in &resolver.rule_sources {
        file_sources.insert(name.clone(), source.clone());
        let rule_source: RuleSource = source.clone().into();
        rule_sources.insert(name.clone(), rule_source);
    }
    
    let all_rules = library.list_filtered(&filter, &rule_sources)?;

    // Filter out partial templates (if rules support partials)
    let rules: Vec<_> = all_rules
        .into_iter()
        .filter(|rule| !rule.is_partial())
        .collect();

    // Convert to display objects with emoji sources
    let display_rows = super::display::rules_to_display_rows_with_sources(
        rules, 
        &file_sources, 
        cli_context.verbose
    );
    cli_context.display_rules(display_rows)?;

    Ok(())
}
```

#### Display Objects

Rules should have parallel display types to prompts:

```rust
// swissarmyhammer-cli/src/commands/rule/display.rs

/// Basic rule information for standard list output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct RuleRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Severity")]
    pub severity: String,

    #[tabled(rename = "Source")]
    pub source: String,
}

/// Detailed rule information for verbose list output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct VerboseRuleRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Description")]
    pub description: String,

    #[tabled(rename = "Severity")]
    pub severity: String,

    #[tabled(rename = "Source")]
    pub source: String,

    #[tabled(rename = "Category")]
    pub category: String,
}

/// Emoji constants matching prompt display (MUST BE IDENTICAL)
const BUILTIN_EMOJI: &str = "📦 Built-in";
const PROJECT_EMOJI: &str = "📁 Project";
const USER_EMOJI: &str = "👤 User";

/// Convert FileSource to emoji (MUST MATCH prompt implementation)
fn file_source_to_emoji(source: Option<&swissarmyhammer::FileSource>) -> &'static str {
    match source {
        Some(swissarmyhammer::FileSource::Builtin) => BUILTIN_EMOJI,
        Some(swissarmyhammer::FileSource::Local) => PROJECT_EMOJI,
        Some(swissarmyhammer::FileSource::User) => USER_EMOJI,
        Some(swissarmyhammer::FileSource::Dynamic) | None => BUILTIN_EMOJI,
    }
}

/// Enum to handle different display row types
#[derive(Debug)]
pub enum DisplayRows {
    Standard(Vec<RuleRow>),
    Verbose(Vec<VerboseRuleRow>),
}
```

#### Output Format Examples

**Standard mode:**
```bash
$ sah rule list
Name                      Title                          Severity  Source
no-hardcoded-secrets      No Hardcoded Secrets          error     📦 Built-in
function-length           Function Length Limit          warning   📦 Built-in
missing-docstrings        Missing Documentation          info      👤 User
custom-naming-check       Project Naming Convention      error     📁 Project
```

**Verbose mode:**
```bash
$ sah rule list --verbose
Name                 Title                     Description                              Severity  Source        Category
no-hardcoded-secrets No Hardcoded Secrets      Detects API keys and passwords          error     📦 Built-in   security
function-length      Function Length Limit     Functions should be < 50 lines          warning   📦 Built-in   code-quality
missing-docstrings   Missing Documentation     All public functions need docs          info      👤 User       documentation
custom-naming-check  Project Naming Convention Enforces project-specific naming rules  error     📁 Project    code-quality
```

**JSON format:**
```bash
$ sah rule list --format json
[
  {
    "name": "no-hardcoded-secrets",
    "title": "No Hardcoded Secrets",
    "severity": "error",
    "source": "📦 Built-in"
  },
  ...
]
```

**YAML format:**
```bash
$ sah rule list --format yaml
- name: no-hardcoded-secrets
  title: No Hardcoded Secrets
  severity: error
  source: 📦 Built-in
...
```

#### Consistency Requirements

1. **Same emoji mapping**: Use identical emoji constants and `file_source_to_emoji()` function
2. **Same display structure**: `RuleRow` and `VerboseRuleRow` parallel to `PromptRow` and `VerbosePromptRow`
3. **Same output formats**: Support table, JSON, YAML via `cli_context.display_rules()`
4. **Same source tracking**: Use `FileSource` enum and resolver pattern
5. **Same filtering**: Support `RuleFilter` parallel to `PromptFilter`
6. **Same verbose flag**: `--verbose` flag for detailed output
7. **Same format flag**: `--format` flag for output format selection

## How Rule Checking Works

### The `.check` Prompt

Rule checking is powered by a special builtin prompt `builtin/prompts/.check.md`:

```markdown
---
title: Rule Check
description: Internal prompt for checking rules against files
parameters:
  - name: rule_content
    description: The rendered rule template content
    type: string
    required: true
  - name: target_content
    description: The file content being checked
    type: string
    required: true
  - name: target_path
    description: Path to the file being checked
    type: string
    required: true
  - name: language
    description: Detected programming language of the file
    type: string
    required: true
---

You are checking {{ language }} code against the following rule:

{{ rule_content }}

---

File: {{ target_path }}

```{{ language }}
{{ target_content }}
```

Analyze this file against the rule. If violations are found, report them with:
- Line numbers
- Severity (error/warning/info/hint)
- Description of the violation
- Suggested fix

If no violations, respond with "PASS".
```

### Context Variables Available in `.check` Prompt

When `.check` is rendered, it has access to:
- `{{rule_content}}` - The **rendered** rule template content (rule template is rendered first with context variables)
- `{{target_content}}` - The file content being checked
- `{{target_path}}` - Path to the file (e.g., `src/main.rs`)
- `{{language}}` - Detected language (e.g., `rust`, `python`, `javascript`)

### Two-Stage Rendering Process

1. **First:** Render the rule template with context variables (`{{language}}`, `{{target_path}}`, etc.) → produces `rule_content`
2. **Second:** Render `.check` prompt with `{{rule_content}}`, `{{target_content}}`, `{{target_path}}`, `{{language}}` → produces final prompt for LLM

### Language Detection

**Decision:** Use tree-sitter for language detection (already a dependency).

```toml
# swissarmyhammer-rules/Cargo.toml
[dependencies]
# ... other deps
tree-sitter = "0.20"  # Already used elsewhere in swissarmyhammer
```

Tree-sitter can detect language from:
1. File extension mapping
2. File content analysis (for ambiguous cases)
3. Existing language parsers we already have

### Check Execution Flow

```rust
// swissarmyhammer-rules/src/checker.rs

use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
use swissarmyhammer_agent::Agent;
use swissarmyhammer_config::TemplateContext;

pub struct RuleChecker {
    agent: Arc<dyn Agent>,
    prompt_library: PromptLibrary,
}

impl RuleChecker {
    pub fn new(agent: Arc<dyn Agent>) -> Result<Self> {
        // Load all prompts including the builtin .check prompt
        let mut prompt_library = PromptLibrary::new();
        let mut resolver = PromptResolver::new();
        resolver.load_all_prompts(&mut prompt_library)?;
        
        Ok(Self {
            agent,
            prompt_library,
        })
    }
    
    pub async fn check_all(&self, rules: Vec<Rule>, targets: Vec<PathBuf>) -> Result<()> {
        // Iterate every rule against every target
        // LLM decides if rule is applicable to each file
        for rule in &rules {
            for target in &targets {
                // Read target file content
                let target_content = std::fs::read_to_string(target)?;
                
                // Detect language from file extension/content
                let language = detect_language(target, &target_content)?;
                
                // STAGE 1: Render the rule template with context variables
                // This allows rules to use {{language}}, {{target_path}}, etc.
                // Use swissarmyhammer-templating for rendering
                let mut rule_context = TemplateContext::new();
                rule_context.set("target_content".to_string(), target_content.clone().into());
                rule_context.set("target_path".to_string(), target.display().to_string().into());
                rule_context.set("language".to_string(), language.clone().into());
                
                // Render using swissarmyhammer-templating
                let rendered_rule = swissarmyhammer_templating::render(&rule.template, &rule_context)?;
                
                // STAGE 2: Render the .check prompt with rendered rule content
                let mut check_context = TemplateContext::new();
                check_context.set("rule_content".to_string(), rendered_rule.into());
                check_context.set("target_content".to_string(), target_content.into());
                check_context.set("target_path".to_string(), target.display().to_string().into());
                check_context.set("language".to_string(), language.into());
                
                // Use PromptLibrary to render the .check prompt
                // This is why swissarmyhammer-rules depends on swissarmyhammer-prompts!
                let check_prompt_text = self.prompt_library.render(".check", &check_context)?;
                
                // Execute via agent (LLM)
                let result = self.agent.execute(check_prompt_text).await?;
                
                // Check result - FAIL FAST on first violation
                if result.trim() != "PASS" {
                    return Err(RuleError::Violation(RuleViolation {
                        rule_name: rule.name.clone(),
                        file_path: target.clone(),
                        severity: rule.severity.clone(),
                        message: result,
                    }));
                }
            }
        }
        
        // All checks passed
        Ok(())
    }
}

/// Detect programming language from file path and content
/// Uses tree-sitter which is already a dependency
fn detect_language(path: &Path, content: &str) -> Result<String> {
    // Use tree-sitter's language detection
    // Tree-sitter already knows about languages from file extensions
    let extension = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown");
    
    // Map extensions to language names
    // Tree-sitter supports these languages already
    let language = match extension {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" | "tsx" => "typescript",
        "go" => "go",
        "java" => "java",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "c" | "h" => "c",
        "rb" => "ruby",
        "sh" => "shell",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "md" => "markdown",
        "dart" => "dart",
        _ => "unknown",
    };
    
    Ok(language.to_string())
}
```

### Agent Trait

```rust
// swissarmyhammer-agent/src/agent.rs

#[async_trait]
pub trait Agent: Send + Sync {
    /// Execute a prompt via LLM and return the response
    /// 
    /// Simple execution: no timeouts, no retries, no security checks.
    /// If the LLM call fails, return an error.
    async fn execute(&self, prompt: String) -> Result<String>;
    
    /// Get the agent's configuration (model, temperature, etc.)
    fn config(&self) -> &AgentConfig;
}

pub struct AgentConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
    // Simple configuration - no timeout, no rate limiting, no security features
}
```

**Design Philosophy:**
- **Keep it simple** - Agent is straightforward LLM execution
- **No timeouts** - LLM requests complete when they complete
- **No security** - Agent is internal tool, not exposed to untrusted input
- **No retries** - Fail fast, let caller handle errors
- **No rate limiting** - Trust the LLM provider's rate limits

## Usage Examples

### Listing Rules

```bash
# List all rules
sah rule list

# Verbose output with descriptions
sah rule list --verbose

# JSON format
sah rule list --format json

# YAML format
sah rule list --format yaml
```

### Checking Files

```bash
# Check files matching glob pattern against all rules
sah rule check "src/**/*.rs"

# Check single file
sah rule check src/main.rs

# Check multiple glob patterns
sah rule check "src/**/*.rs" "tests/**/*.rs"

# Check all files in current directory
sah rule check "**/*"

# Check with specific rule only
sah rule check --rule no-hardcoded-secrets "src/**/*.rs"

# Check with severity filter (only errors)
sah rule check --severity error "src/**/*.rs"
```

#### CLI Definition

```rust
// swissarmyhammer-cli/src/commands/rule/cli.rs

#[derive(Debug, Clone, Parser)]
pub struct CheckCommand {
    /// Glob patterns for files to check
    /// Examples: "src/**/*.rs", "*.toml", "**/*.md"
    #[arg(required = true)]
    pub patterns: Vec<String>,
    
    /// Only run specific rule(s)
    #[arg(long, short = 'r')]
    pub rule: Option<Vec<String>>,
    
    /// Filter by severity level
    #[arg(long, short = 's', value_enum)]
    pub severity: Option<Severity>,
    
    /// Category filter
    #[arg(long, short = 'c')]
    pub category: Option<String>,
    
    // Note: No --fail-on-violations flag - always fails on violations
}
```

#### Implementation

```rust
// swissarmyhammer-cli/src/commands/rule/check.rs

pub async fn execute_check_command(
    cmd: CheckCommand,
    cli_context: &CliContext,
) -> Result<()> {
    // Phase 1: Load all rules
    let mut library = RuleLibrary::new();
    let mut resolver = RuleResolver::new();
    resolver.load_all_rules(&mut library)?;
    
    // Phase 2: Validate all rules BEFORE checking
    if !cli_context.quiet {
        println!("Validating rules...");
    }
    let rules = library.list()?;
    for rule in &rules {
        rule.validate()?;  // Fail fast if any rule is invalid
    }
    if !cli_context.quiet {
        println!("✓ All {} rules are valid\n", rules.len());
    }
    
    // Filter rules by criteria
    let mut rules = rules;
    if let Some(rule_names) = &cmd.rule {
        rules.retain(|r| rule_names.contains(&r.name));
    }
    if let Some(severity) = &cmd.severity {
        rules.retain(|r| &r.severity == severity);
    }
    if let Some(category) = &cmd.category {
        rules.retain(|r| r.category.as_ref() == Some(category));
    }
    
    // Expand glob patterns to file paths
    let mut target_files = Vec::new();
    for pattern in &cmd.patterns {
        for entry in glob::glob(pattern)? {
            let path = entry?;
            if path.is_file() {
                target_files.push(path);
            }
        }
    }
    
    if !cli_context.quiet {
        println!("Checking {} rules against {} files...\n", 
                 rules.len(), target_files.len());
    }
    
    // Phase 3: Run checks with fail-fast
    let agent = create_agent_from_config()?;
    let checker = RuleChecker::new(agent)?;
    
    // check_all now fails fast on first violation
    match checker.check_all(rules, target_files).await {
        Ok(()) => {
            if !cli_context.quiet {
                println!("✓ All checks passed");
            }
            Ok(())
        }
        Err(RuleError::Violation(violation)) => {
            // Show full LLM response and fail
            eprintln!("❌ Rule violation in {}", violation.file_path.display());
            eprintln!("Rule: {}", violation.rule_name);
            eprintln!("Severity: {:?}", violation.severity);
            eprintln!("\n{}", violation.message);  // Full LLM response
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error during checking: {}", e);
            std::process::exit(1);
        }
    }
}
```

Output:
```
Checking 15 rules against 42 files...

❌ Error: No Hardcoded Secrets (security)
   src/main.rs:42
   Found API key: API_KEY = "sk_live_abc123"
   
   Suggestion: Move to environment variable or secrets manager

⚠️  Warning: Function Too Complex (code-quality)  
   src/main.rs:108
   Function 'process_data' has cyclomatic complexity of 15 (max: 10)
   
   Suggestion: Break into smaller functions

✓ 40 files passed all checks
❌ 2 files had violations
```

### Fixing Auto-Fixable Issues (Future)

```bash
sah rule fix src/
```

## Built-in Rules (Examples)

### Security Rules
- `no-hardcoded-secrets`: Detect API keys, passwords, tokens
- `no-eval`: Detect dangerous eval() usage
- `no-sql-injection`: Detect SQL injection patterns
- `secure-random`: Require crypto-secure random generators

### Code Quality Rules
- `function-length`: Functions should be < 50 lines
- `cognitive-complexity`: Limit cognitive complexity
- `no-commented-code`: No large blocks of commented code
- `consistent-naming`: Enforce naming conventions

### Documentation Rules
- `missing-docstrings`: All public functions need docs
- `outdated-comments`: Detect comments contradicting code
- `todo-tracking`: Find and track TODO/FIXME comments

## Integration with Workflow

Rules can be integrated into workflows as checks (future work):

```yaml
# sah.toml
[workflow.pre_commit]
actions = [
  { type = "rule_check", severity = "error" },
  { type = "commit" }
]
```

This would require a workflow action type in `swissarmyhammer-workflow` that depends on `swissarmyhammer-rules`.

## Comparison with Prompts and Flows

| Aspect | Prompts | Flows | Rules |
|--------|---------|-------|-------|
| **Crate** | `swissarmyhammer-prompts` | `swissarmyhammer-workflow` | `swissarmyhammer-rules` |
| **Purpose** | Interactive content generation | Multi-step automation | Automated validation |
| **Storage** | `prompts/` subdirs | `flows/` subdirs | `rules/` subdirs |
| **Execution** | User-triggered (CLI/MCP) | User-triggered | Automated/batch |
| **Output** | Generated content | Workflow results | Pass/fail + diagnostics |
| **Metadata** | Title, category, tags, **parameters** | Steps, guards, context | Title, category, tags, **severity** (no parameters) |
| **CLI** | `sah prompt` | `sah flow` | `sah rule` |
| **Template Engine** | Liquid (templating) | Liquid (templating) | Liquid (templating) |
| **Hierarchical Loading** | builtin → user → local | builtin → user → local | builtin → user → local |
| **Partials Support** | Yes | Yes | Yes |
| **User Interaction** | High | Medium | Low (reports only) |

## Key Design Decisions

1. **Shared codebase**: Prompts and rules share 90%+ of infrastructure
2. **Type-safe specialization**: Use Rust's type system for rule-specific features
3. **Same file format**: Both use markdown + liquid for consistency
4. **Parallel hierarchies**: builtin/user/local structure for both
5. **Partial reuse**: Rules can use prompt partials and vice versa
6. **Same rendering**: Both use liquid templates with TemplateContext

## Success Criteria

### Phase 1 Complete When:
- [ ] `.check` prompt exists in `builtin/prompts/`
- [ ] Can manually test: `sah prompt test .check --vars rule="..." target_content="..." target_path="..." language="..."`
- [ ] Prompt renders correctly with all context variables

### Phase 2 Complete When:
- [ ] `swissarmyhammer-agent` crate builds
- [ ] `Agent` trait defined with simple `execute()` method
- [ ] Basic OpenAI or Anthropic agent implementation works
- [ ] Can execute a simple prompt and get response back
- [ ] No timeouts, no retries, no security features (intentionally simple)

### Phase 3-4 Complete When:
- [ ] `swissarmyhammer-rules` crate builds
- [ ] Can load rules from all three tiers (builtin/user/local)
- [ ] `sah rule list` shows all loaded rules with correct sources
- [ ] Rules have severity and other rule-specific fields
- [ ] Storage, loading, resolver patterns copied from prompts

### Phase 5 Complete When:
- [ ] Language detection works (via tree-sitter)
- [ ] `RuleChecker::check_all()` can check files against rules
- [ ] Can check a test file and get back violations
- [ ] `.check` prompt is properly loaded and rendered with context
- [ ] Agent executes the check and returns results

### Phase 6 Complete When:
- [ ] `sah rule list` command works with table/JSON/YAML output
- [ ] `sah rule list --verbose` shows detailed info
- [ ] Same emoji-based source display as prompts (📦 Built-in, 📁 Project, 👤 User)

### Phase 7 Complete When:
- [ ] `sah rule validate` validates all rules and reports issues
- [ ] `sah rule check [glob]` accepts glob patterns
- [ ] Check command validates rules before checking
- [ ] Check command filters by --rule, --severity, --category
- [ ] Check fails fast on first violation with full LLM response
- [ ] Exits with code 1 when violations found

### Phase 8 Complete When:
- [ ] `sah rule test <rule> <file>` command works
- [ ] Test command validates the rule
- [ ] Test command shows rendered .check prompt
- [ ] Test command executes REAL LLM call
- [ ] Test command shows full LLM response
- [ ] Test command parses and displays violation (if any)
- [ ] Rule authors can debug their rules effectively

### Overall Success:
- [ ] Rules are properly isolated from prompts (separate crate)
- [ ] Rules can check actual files and report real violations
- [ ] CLI commands work end-to-end
- [ ] All builtin rules work correctly
- [ ] Documentation is complete

## What's Missing / Could Be Improved

### 1. Error Handling Strategy
**Decision:** Fail-fast on rule violations, but validate first.

**Two-phase approach:**
1. **Validate phase** - Check all rules are valid before running any checks
2. **Check phase** - Run checks, fail-fast on first violation

```rust
// Error types for rules crate
pub enum RuleError {
    LoadError(String),          // Can't load rule file
    ValidationError(String),    // Rule is invalid (missing fields, etc.)
    CheckError(String),         // Error during checking
    AgentError(String),         // LLM agent failed
    LanguageDetectionError(String),
    GlobExpansionError(String),
    Violation(RuleViolation),   // Rule violation found (for fail-fast)
}

pub struct RuleViolation {
    pub rule_name: String,
    pub file_path: PathBuf,
    pub severity: Severity,
    pub message: String,  // Full LLM response
}
```

**Validation catches:**
- Missing required fields (title, description, severity)
- Invalid severity values
- Template syntax errors (invalid liquid)

**Fail-fast behavior:**
- On first rule violation, stop checking and report immediately
- Show the full LLM response
- Exit with code 1

### 2. Missing: Rule Violation Format
**What's missing:** Exact structure of violation output from LLM.

**Add specification:**
```markdown
Expected LLM response format from .check prompt:

Success (no violations):
PASS

Failure (with violations):
VIOLATION
Line: 42
Severity: error
Message: Found hardcoded API key 'sk_live_abc123'
Suggestion: Move to environment variable

VIOLATION
Line: 108
Severity: warning
Message: Function too complex (cyclomatic complexity 15)
Suggestion: Break into smaller functions
```

**Need:** Parser to extract violations from LLM response.

### 3. Performance Considerations
**Decision:** Sequential checking with fail-fast on first violation.

Since we fail-fast on the first violation, we don't need to worry about checking 1000+ files in one run. The check stops as soon as any violation is found.

**Implications:**
- No parallelism needed (checks stop at first failure)
- No batching needed (sequential execution)
- No caching needed (doesn't run long enough to matter)
- Progress indicator not needed (fails quickly)

This keeps the implementation simple and matches typical linter behavior.

### 4. Rule Testing Command
**Decision:** Add `sah rule test` that invokes agent for real testing.

```bash
# Test a specific rule against a file (executes via LLM)
sah rule test <rule-name> <file-path>

# Shows:
# 1. Rule validation (is rule well-formed?)
# 2. File reading and language detection
# 3. Rendered rule template (rule with {{language}}, {{target_path}} filled in)
# 4. Rendered .check prompt (final prompt sent to LLM)
# 5. LLM response (actual check result)
# 6. Parsed violation (if any)
```

**Implementation:**
```rust
// swissarmyhammer-cli/src/commands/rule/test.rs

pub async fn execute_test_command(
    rule_name: &str,
    file_path: &Path,
    cli_context: &CliContext,
) -> Result<()> {
    // Load the specific rule
    let mut library = RuleLibrary::new();
    let mut resolver = RuleResolver::new();
    resolver.load_all_rules(&mut library)?;
    let rule = library.get(rule_name)?;
    
    // Validate rule
    println!("1. Validating rule '{}'...", rule_name);
    rule.validate()?;
    println!("   ✓ Rule is valid\n");
    
    // Read file and detect language
    println!("2. Reading file '{}'...", file_path.display());
    let content = std::fs::read_to_string(file_path)?;
    let language = detect_language(file_path, &content)?;
    println!("   ✓ Detected language: {}\n", language);
    
    // Render rule template first using swissarmyhammer-templating
    println!("3. Rendering rule template...");
    let mut rule_context = TemplateContext::new();
    rule_context.set("target_content".to_string(), content.clone().into());
    rule_context.set("target_path".to_string(), file_path.display().to_string().into());
    rule_context.set("language".to_string(), language.clone().into());
    
    let rendered_rule = swissarmyhammer_templating::render(&rule.template, &rule_context)?;
    println!("   Rendered rule content:");
    println!("   {}", "─".repeat(60));
    println!("{}", rendered_rule);
    println!("   {}\n", "─".repeat(60));
    
    // Render .check prompt with rendered rule
    println!("4. Rendering .check prompt...");
    let mut prompt_library = PromptLibrary::new();
    let mut prompt_resolver = PromptResolver::new();
    prompt_resolver.load_all_prompts(&mut prompt_library)?;
    
    let mut check_context = TemplateContext::new();
    check_context.set("rule_content".to_string(), rendered_rule.into());
    check_context.set("target_content".to_string(), content.into());
    check_context.set("target_path".to_string(), file_path.display().to_string().into());
    check_context.set("language".to_string(), language.into());
    
    let check_prompt = prompt_library.render(".check", &check_context)?;
    println!("   Final prompt to be sent to LLM:");
    println!("   {}", "─".repeat(60));
    println!("{}", check_prompt);
    println!("   {}\n", "─".repeat(60));
    
    // Execute via agent (REAL LLM CALL)
    println!("5. Executing check via LLM agent...");
    let agent = create_agent_from_config()?;
    let response = agent.execute(check_prompt).await?;
    println!("   LLM Response:");
    println!("   {}", "─".repeat(60));
    println!("{}", response);
    println!("   {}\n", "─".repeat(60));
    
    // Parse result
    println!("6. Parsing result...");
    if response.trim() == "PASS" {
        println!("   ✓ No violations found");
        Ok(())
    } else {
        println!("   ✗ Violation detected");
        // Show parsed violation details
        Ok(())
    }
}
```

**Use case:** Rule authors can test their rules against sample files to see:
- What the LLM actually sees (the rendered prompt)
- What the LLM responds with
- Whether the rule catches what it should

**Important:** This does a REAL LLM call, so rule authors can verify their rules work correctly before deploying them.

### 5. Missing: Builtin Rules Examples
**What's missing:** Concrete list of which builtin rules to ship with.

**Recommendation - Security category:**
- `no-hardcoded-secrets` - Detect API keys, passwords
- `no-eval` - Dangerous eval() usage
- `no-sql-injection` - SQL injection patterns
- `secure-dependencies` - Known vulnerable dependencies

**Recommendation - Code Quality category:**
- `function-length` - Max function length
- `cognitive-complexity` - Complexity limits
- `no-commented-code` - Dead code detection
- `consistent-naming` - Naming conventions

**Recommendation - Documentation category:**
- `missing-docstrings` - Public functions need docs
- `outdated-comments` - Comments contradicting code

### 6. ~~Improvement: Make applies_to Optional~~
**Decision:** No `applies_to` field at all.

All rules check all files. The LLM decides if a rule is applicable. For example, a "no-hardcoded-secrets" rule can return "PASS" for markdown files.

This simplifies the rule system - no glob matching needed.

### 7. Rule Metadata Display
**Decision:** VerboseRuleRow is already specified correctly.

```rust
pub struct VerboseRuleRow {
    pub name: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub source: String,
}
```

No `applies_to` field since rules don't have that metadata.

### 8. Agent Configuration
**Decision:** Use existing sah config - NO new configuration needed.

Rules will use the **same agent configuration as workflows**. The agent is already configured for running flows, so rules simply reuse that configuration.

```rust
// In check command
let agent = create_agent_from_config()?;  // Uses existing flow agent config
```

**No new sah.toml configuration needed** - agent selection, model, temperature, etc. are all already configured for workflows.

**Future enhancement (optional):** If needed, could add rule-specific filters:
```toml
[rules.filters]
# Which rules to run by default (optional, not v1)
severity = ["error", "warning"]  # Don't run "info" or "hint" by default
categories = ["security", "code-quality"]  # Skip "documentation" rules
```

### 9. Exit Codes
**Decision:** Simple two-code system.

```
Exit Codes for `sah rule check`:
- 0: All checks passed (no violations found)
- 1: Any violations found OR errors during checking

Note: No --fail-on-violations flag needed, always fail on violations.
```

**Rationale:** Rules are checks - if they fail, the command should fail. Simple and predictable.

### 10. Git Hooks Integration
**Decision:** No automatic git hooks support.

Users can manually add rules to their git hooks if desired:
```bash
# .git/hooks/pre-commit
#!/bin/bash
sah rule check --severity error "**/*.rs"
```

**Rationale:** 
- Git hooks are project-specific
- Users should explicitly opt-in
- Not needed for v1

### 11. Caching Strategy
**Decision:** No caching needed.

Since we fail-fast on first violation, caching doesn't provide value. The check stops immediately on failure, so there's no long-running operation to optimize.

**Not implementing.**

### 12. Rule Variables/Configuration
**Decision:** Not needed for v1.

Rules should be self-contained and not require configuration. If a project needs different thresholds, they can create project-local rules in `.swissarmyhammer/rules/` with their own values.

**Not implementing configuration variables like `{{ config.max_lines }}`.**

## Open Questions (Resolved)

1. ~~Should rules and prompts share the same partial namespace?~~
   - **RESOLVED:** Yes, shared namespace. Rules can use prompt partials and vice versa.
   
2. ~~Should the base trait be extracted?~~
   - **RESOLVED:** No, duplicate the pattern. Don't extract common base until we have a third similar system.
   
3. ~~How should rule results be structured?~~
   - **RESOLVED:** Parse LLM response into `RuleViolation` struct. Need to specify expected format.
   
4. ~~Can auto-fix be expressed as a template?~~
   - **DEFERRED:** Auto-fix is Phase 9 (future work). Not needed for v1.

## Open Questions (New)

1. ~~**Error handling strategy:** Fail-fast or continue on errors?~~
   - **RESOLVED:** Fail-fast on first violation. Validate all rules first.

2. ~~**Parallel execution:** Should checks run in parallel?~~
   - **RESOLVED:** No, sequential with fail-fast. No parallelism needed.

3. **Rule testing:** Should there be a `sah rule test` command?
   - **RESOLVED:** Yes, `sah rule test <rule> <file>` executes real LLM check (Phase 8)

4. ~~**Cache strategy:** Should we cache check results?~~
   - **RESOLVED:** No caching. Fail-fast makes it unnecessary.

5. ~~**Rule configuration:** Should rules be parameterizable?~~
   - **RESOLVED:** No, not for v1. Rules should be self-contained. Projects can create custom local rules if needed.

6. ~~**Git hooks:** Should we provide git hook integration?~~
   - **RESOLVED:** No automatic integration. Users can add manually if desired.

## Crate Architecture

### New Crate: `swissarmyhammer-agent`

A new foundational crate for LLM agent invocation, used by both rules and workflows:

```
swissarmyhammer-agent/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Public API
│   ├── agent.rs                # Agent trait and implementations
│   ├── prompt_executor.rs      # Execute prompts via LLM
│   └── context.rs              # Agent execution context
└── tests/
    └── ...
```

**Purpose**: Provides simple, straightforward LLM invocation. Used by:
- `swissarmyhammer-rules` - to check rules via `.check` prompt
- `swissarmyhammer-workflow` - to execute `prompt` actions

**Design principles:**
- **Simple execution** - No timeouts, no retries, no rate limiting
- **No security features** - Internal tool, trusted input only
- **Fail fast** - If LLM call fails, return error immediately
- **Trust the provider** - Let OpenAI/Anthropic handle their own rate limits

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
swissarmyhammer-config = { path = "../swissarmyhammer-config" }
swissarmyhammer-templating = { path = "../swissarmyhammer-templating" }
swissarmyhammer-prompts = { path = "../swissarmyhammer-prompts" }

# LLM provider integrations (simple API clients)
openai-api = "0.1"
anthropic = "0.1"
# ... other providers
```

### New Crate: `swissarmyhammer-rules`

The rules crate manages rule definitions and orchestrates checking:

```
swissarmyhammer-rules/
├── Cargo.toml
├── build.rs                    # Build script to embed builtin rules
├── src/
│   ├── lib.rs                  # Public API, re-exports
│   ├── rules.rs                # Rule struct, RuleLibrary
│   ├── storage.rs              # StorageBackend implementations
│   ├── rule_resolver.rs        # Hierarchical rule loading
│   ├── rule_filter.rs          # Filtering by severity, category
│   ├── rule_partial_adapter.rs # Liquid partial support
│   ├── frontmatter.rs          # YAML frontmatter parsing (shared)
│   ├── severity.rs             # Severity enum (Error/Warning/Info/Hint)
│   └── checker.rs              # Rule checking orchestration
└── tests/
    └── ...
```

### Dependencies

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
swissarmyhammer-config = { path = "../swissarmyhammer-config" }
swissarmyhammer-templating = { path = "../swissarmyhammer-templating" }
swissarmyhammer-prompts = { path = "../swissarmyhammer-prompts" }  # CRITICAL: Needed to render .check prompt
swissarmyhammer-agent = { path = "../swissarmyhammer-agent" }      # NEW: Needed to execute prompts

# Same dependencies as prompts for consistency
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
liquid = "0.26"
walkdir = "2.3"
glob = "0.3"

# Language detection - reuse existing tree-sitter
tree-sitter = "0.20"  # Already used elsewhere in swissarmyhammer
```

**Why rules depends on prompts:**
- Rules need to render the `.check` builtin prompt with rule and file content
- The `.check` prompt is loaded via `PromptLibrary` and `PromptResolver`
- Rule templates themselves are liquid templates (similar structure to prompts)
- Both use the same rendering infrastructure (`TemplateContext`, `PromptLibrary::render()`)

### Directory Structure for Rules

Rules follow the same three-tier hierarchy:

```
builtin/rules/                  # Embedded in binary via build.rs
  ├── security/
  │   ├── no-hardcoded-secrets.md
  │   └── no-sql-injection.md
  ├── code-quality/
  │   ├── function-length.md
  │   └── cognitive-complexity.md
  └── _partials/
      └── common-patterns.md.liquid

~/.swissarmyhammer/rules/       # User-global rules
  └── my-custom-rules.md

.swissarmyhammer/rules/         # Project-local rules
  └── project-specific-rules.md
```

## Related Work

- **New crate**: `swissarmyhammer-agent` (shared LLM invocation for rules and workflows)
- **Parallel to**: `swissarmyhammer-prompts` crate (same structure and patterns)
- **Parallel to**: `swissarmyhammer-workflow` crate (same CLI command structure, will use agent)
- **Uses**: `swissarmyhammer-common` (FileSource, ValidationIssue, parameter system)
- **Uses**: `swissarmyhammer-config` (TemplateContext for rendering)
- **Uses**: `swissarmyhammer-templating` (Liquid template engine)
- **Future**: `swissarmyhammer-templates` (if we extract common base later)

## Key Insights

### Agent as Shared Infrastructure

Both `swissarmyhammer-rules` and `swissarmyhammer-workflow` need to invoke LLMs:

- **Rules**: Execute `.check` prompt with `{{rule}}` and `{{target}}` context
- **Workflows**: Execute `prompt` actions with workflow context variables

Rather than duplicate LLM invocation logic, `swissarmyhammer-agent` provides a unified interface for both use cases.

### Rules Depends on Prompts

`swissarmyhammer-rules` depends on `swissarmyhammer-prompts` because:

1. **The `.check` mechanism is a prompt**: Rules don't directly talk to LLMs - they render a prompt template that does
2. **Prompt loading infrastructure**: Rules use `PromptLibrary` and `PromptResolver` to load `.check`
3. **Rendering with context**: Rules use `PromptLibrary::render()` to render `.check` with rule and file content
4. **Consistent templating**: Both rules and prompts use Liquid templates and `TemplateContext`

**Dependency chain:**
```
swissarmyhammer-rules
  ↓ uses prompts to render .check
swissarmyhammer-prompts
  ↓ uses templating
swissarmyhammer-templating
```

This means `.check` is a **special builtin prompt** that rules use as their checking mechanism.