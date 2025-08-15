# SwissArmyHammer Configuration System using Figment

## Overview

This specification outlines the implementation of a comprehensive configuration system for SwissArmyHammer using the `figment` crate. The system will support multiple configuration sources with a clear precedence order, multiple file formats, and environment variable integration.

## Current State

Currently, SwissArmyHammer has:
- Custom TOML configuration parsing in `src/sah_config/`
- Limited configuration file discovery
- Basic environment variable support
- Hardcoded configuration paths and formats

## Proposed Design

### 1. Configuration Precedence Order

Configuration sources should be merged in the following order (later sources override earlier ones):

1. **Default values** (hardcoded in application)
2. **Global config file** (`~/.swissarmyhammer/` directory)
3. **Project config file** (`.swissarmyhammer/` directory in current project)
4. **Local config file** (current working directory)
5. **Environment variables** (with `SAH_` or `SWISSARMYHAMMER_` prefix)
6. **Command line arguments** (highest priority)

### 2. Configuration File Discovery

#### File Names
Support both short and long form names:
- `sah.{toml,yaml,yml,json}`
- `swissarmyhammer.{toml,yaml,yml,json}`

#### Search Locations
1. **Current Working Directory (pwd)**
   ```
   ./sah.toml
   ./sah.yaml
   ./sah.yml
   ./sah.json
   ./swissarmyhammer.toml
   ./swissarmyhammer.yaml
   ./swissarmyhammer.yml
   ./swissarmyhammer.json
   ```

2. **Project SwissArmyHammer Directory**
   ```
   ./.swissarmyhammer/sah.toml
   ./.swissarmyhammer/sah.yaml
   ./.swissarmyhammer/sah.yml
   ./.swissarmyhammer/sah.json
   ./.swissarmyhammer/swissarmyhammer.toml
   ./.swissarmyhammer/swissarmyhammer.yaml
   ./.swissarmyhammer/swissarmyhammer.yml
   ./.swissarmyhammer/swissarmyhammer.json
   ```

3. **User Home SwissArmyHammer Directory**
   ```
   ~/.swissarmyhammer/sah.toml
   ~/.swissarmyhammer/sah.yaml
   ~/.swissarmyhammer/sah.yml
   ~/.swissarmyhammer/sah.json
   ~/.swissarmyhammer/swissarmyhammer.toml
   ~/.swissarmyhammer/swissarmyhammer.yaml
   ~/.swissarmyhammer/swissarmyhammer.yml
   ~/.swissarmyhammer/swissarmyhammer.json
   ```

### 3. Configuration Schema

#### 3.1 Core Configuration Structure
```toml
# General settings
debug = false
verbose = false
color = "auto"  # "auto", "always", "never"

[paths]
# Directory paths
home_dir = "~/.swissarmyhammer"
project_dir = ".swissarmyhammer"
cache_dir = "~/.cache/swissarmyhammer"
temp_dir = "/tmp/swissarmyhammer"

# Resource paths
prompts_dir = "prompts"
workflows_dir = "workflows"
issues_dir = "issues"
memoranda_dir = "memoranda"

[search]
# Semantic search configuration
enabled = true
model = "nomic-embed-text-v1.5"
cache_dir = "/tmp/.cache/fastembed"
batch_size = 32
max_text_length = 8000

[workflow]
# Workflow execution settings
cache_dir = "~/.swissarmyhammer/workflow_cache"
parallel_execution = true
max_concurrent_actions = 4
timeout_seconds = 300

[git]
# Git integration settings
auto_commit = false
commit_template = "SwissArmyHammer: {action}"
default_branch = "main"

[mcp]
# MCP server settings
enabled = true
port = 3000
log_level = "info"

[prompts]
# Prompt system settings
template_engine = "liquid"
strict_variables = false
auto_escape = true

[security]
# Security settings
allow_shell_commands = true
allowed_commands = []
blocked_commands = ["rm -rf", "format", "del /f"]
sandbox_mode = false

# MCP Tool Configuration
# Each MCP tool can have its own configuration section
[tools.issue_create]
auto_numbering = true
default_template = "issue"
require_description = true

[tools.issue_list]
default_format = "table"
show_completed = false
max_items = 50

[tools.issue_work]
auto_checkout_branch = true
branch_prefix = "issue/"

[tools.memo_create]
auto_timestamp = true
default_tags = []

[tools.search_index]
auto_index_on_changes = false
include_patterns = ["**/*.rs", "**/*.md", "**/*.py"]
exclude_patterns = ["target/**", ".git/**"]
chunk_size = 1000

[tools.search_query]
default_limit = 10
min_similarity = 0.7
include_context = true

[tools.todo_create]
auto_expire = false
default_priority = "medium"

[tools.outline_generate]
default_format = "yaml"
max_depth = 10
include_private = false

[tools.abort_create]
require_reason = true
log_level = "error"
```

#### 3.2 Environment Variable Mapping Pattern

SwissArmyHammer uses a **consistent, hierarchical naming pattern** for environment variables that directly maps to the configuration structure:

**Pattern: `SAH_<SECTION>_<TOOL>_<FIELD>`**

### Core Configuration Variables
```bash
# Global settings - Pattern: SAH_<FIELD>
SAH_DEBUG=true
SAH_VERBOSE=true
SAH_COLOR=always

# Section-based settings - Pattern: SAH_<SECTION>_<FIELD>
SAH_PATHS_HOME_DIR=/custom/sah/home
SAH_PATHS_CACHE_DIR=/tmp/my-sah-cache
SAH_PATHS_PROMPTS_DIR=./my-prompts

SAH_SEARCH_ENABLED=false
SAH_SEARCH_MODEL=custom-model
SAH_SEARCH_CACHE_DIR=/custom/cache

SAH_WORKFLOW_PARALLEL_EXECUTION=false
SAH_WORKFLOW_MAX_CONCURRENT_ACTIONS=8

SAH_GIT_AUTO_COMMIT=true
SAH_GIT_DEFAULT_BRANCH=develop

SAH_MCP_ENABLED=false
SAH_MCP_PORT=4000

SAH_SECURITY_SANDBOX_MODE=true
```

### Tool Configuration Variables
**Pattern: `SAH_<TOOL_NAME>_<FIELD>` or `SAH_<TOOL_CATEGORY>_<TOOL_ACTION>_<FIELD>`**

#### Issue Management Tools
```bash
# Issue creation tool - maps to [tools.issue_create]
SAH_ISSUE_CREATE_AUTO_NUMBERING=false
SAH_ISSUE_CREATE_DEFAULT_TEMPLATE=custom-issue
SAH_ISSUE_CREATE_REQUIRE_DESCRIPTION=true
SAH_ISSUE_CREATE_MAX_TITLE_LENGTH=150
SAH_ISSUE_CREATE_DEFAULT_PRIORITY=high

# Issue listing tool - maps to [tools.issue_list]  
SAH_ISSUE_LIST_DEFAULT_FORMAT=json
SAH_ISSUE_LIST_SHOW_COMPLETED=true
SAH_ISSUE_LIST_MAX_ITEMS=100

# Issue work tool - maps to [tools.issue_work]
SAH_ISSUE_WORK_AUTO_CHECKOUT_BRANCH=true
SAH_ISSUE_WORK_BRANCH_PREFIX="feature/"
```

#### Memo Management Tools
```bash
# Memo creation tool - maps to [tools.memo_create]
SAH_MEMO_CREATE_AUTO_TIMESTAMP=false
SAH_MEMO_CREATE_DEFAULT_TAGS="work,notes"
SAH_MEMO_CREATE_DEFAULT_TEMPLATE=custom-memo
SAH_MEMO_CREATE_AUTO_EDIT=true

# Memo search tool - maps to [tools.memo_search]
SAH_MEMO_SEARCH_DEFAULT_LIMIT=25
SAH_MEMO_SEARCH_INCLUDE_CONTENT=true
```

#### Search Tools
```bash
# Search indexing tool - maps to [tools.search_index]
SAH_SEARCH_INDEX_AUTO_INDEX_ON_CHANGES=true
SAH_SEARCH_INDEX_INCLUDE_PATTERNS="**/*.rs,**/*.md,**/*.py"
SAH_SEARCH_INDEX_EXCLUDE_PATTERNS="target/**,.git/**"
SAH_SEARCH_INDEX_CHUNK_SIZE=2000
SAH_SEARCH_INDEX_BATCH_SIZE=100

# Search query tool - maps to [tools.search_query]
SAH_SEARCH_QUERY_DEFAULT_LIMIT=20
SAH_SEARCH_QUERY_MIN_SIMILARITY=0.8
SAH_SEARCH_QUERY_INCLUDE_CONTEXT=true
```

#### Todo Management Tools
```bash
# Todo creation tool - maps to [tools.todo_create]
SAH_TODO_CREATE_AUTO_EXPIRE=true
SAH_TODO_CREATE_DEFAULT_PRIORITY=high
SAH_TODO_CREATE_AUTO_ASSIGN=false

# Todo show tool - maps to [tools.todo_show]
SAH_TODO_SHOW_INCLUDE_COMPLETED=false
SAH_TODO_SHOW_DEFAULT_FORMAT=table
```

#### Outline Generation Tool
```bash
# Outline generation tool - maps to [tools.outline_generate]
SAH_OUTLINE_GENERATE_DEFAULT_FORMAT=json
SAH_OUTLINE_GENERATE_MAX_DEPTH=15
SAH_OUTLINE_GENERATE_INCLUDE_PRIVATE=true
SAH_OUTLINE_GENERATE_INCLUDE_TESTS=false
```

#### Utility Tools
```bash
# Abort creation tool - maps to [tools.abort_create]
SAH_ABORT_CREATE_REQUIRE_REASON=false
SAH_ABORT_CREATE_LOG_LEVEL=warn
SAH_ABORT_CREATE_AUTO_CLEANUP=true
```

### Environment Variable Type Handling

Figment automatically handles type conversion for environment variables based on the target Rust types in your configuration structs. Our macro leverages this existing functionality:

```bash
# Figment handles these conversions automatically:
SAH_DEBUG=true                                  # -> bool
SAH_SEARCH_INDEX_CHUNK_SIZE=1500               # -> usize  
SAH_SEARCH_QUERY_MIN_SIMILARITY=0.75           # -> f64
SAH_ISSUE_CREATE_DEFAULT_TEMPLATE=my-template  # -> String

# For complex types, Figment supports various formats:
SAH_MEMO_CREATE_DEFAULT_TAGS="work,notes"     # -> Vec<String> (comma-separated)
SAH_PATHS_HOME_DIR="~/my-sah"                  # -> PathBuf (with expansion)
```

**Our Value-Add**: The `config_struct!` macro ensures type safety and provides fallback to defaults when Figment's parsing fails.

### Mapping Rules Summary

| Configuration Level | Env Var Pattern | Example |
|---------------------|-----------------|---------|
| Global | `SAH_<FIELD>` | `SAH_DEBUG` → `debug` |
| Core Section | `SAH_<SECTION>_<FIELD>` | `SAH_SEARCH_ENABLED` → `search.enabled` |
| Tool Config | `SAH_<TOOL>_<FIELD>` | `SAH_ISSUE_CREATE_AUTO_NUMBERING` → `tools.issue_create.auto_numbering` |
| Nested Tool | `SAH_<CATEGORY>_<ACTION>_<FIELD>` | `SAH_SEARCH_INDEX_CHUNK_SIZE` → `tools.search_index.chunk_size` |

### Environment Variable Best Practices

**Consistency Rules:**
1. **Always use `SAH_` prefix** - shorter and more convenient
2. **Use SCREAMING_SNAKE_CASE** - standard environment variable convention  
3. **Follow the hierarchical pattern** - makes variables predictable and discoverable
4. **Match configuration structure** - env var names should directly map to config paths
5. **Use descriptive field names** - avoid abbreviations when possible

**Examples of Good Environment Variable Names:**
```bash
# Good: Clear hierarchy and descriptive names
SAH_ISSUE_CREATE_AUTO_NUMBERING=true
SAH_SEARCH_INDEX_CHUNK_SIZE=1000
SAH_MEMO_CREATE_DEFAULT_TEMPLATE=note

# Avoid: Inconsistent patterns or unclear abbreviations  
SAH_ISSUE_AUTONUMBER=true      # Missing hierarchy
SAH_SEARCH_CHUNK=1000          # Abbreviated field name
ISSUE_CREATE_AUTO=true         # Missing SAH_ prefix
```

### Environment Variable to Config Mapping in Practice

The `config_struct!` macro automatically maps environment variables to configuration fields:

```rust
config_struct! {
    pub struct IssueCreateConfig {
        // TOML: [tools.issue_create] auto_numbering = true
        // ENV:  SAH_ISSUE_CREATE_AUTO_NUMBERING=true
        pub auto_numbering: bool = true,
        env = "SAH_ISSUE_CREATE_AUTO_NUMBERING",
        
        // TOML: [tools.issue_create] default_template = "issue"  
        // ENV:  SAH_ISSUE_CREATE_DEFAULT_TEMPLATE=custom-issue
        pub default_template: String = "issue".to_string(),
        env = "SAH_ISSUE_CREATE_DEFAULT_TEMPLATE",
        
        // TOML: [tools.issue_create] max_title_length = 200
        // ENV:  SAH_ISSUE_CREATE_MAX_TITLE_LENGTH=150
        pub max_title_length: usize = 200,
        env = "SAH_ISSUE_CREATE_MAX_TITLE_LENGTH",
        validate = |&x| x >= 10 && x <= 500,
    }
}
```

**Configuration Resolution Order:**
```
1. Macro default        → auto_numbering = true
2. TOML config file     → auto_numbering = false  (overrides default)
3. Environment variable → SAH_ISSUE_CREATE_AUTO_NUMBERING=true (overrides TOML)
4. Runtime argument     → --auto-numbering false (overrides env var)
```

**Complete Mapping Example:**

| Configuration Path | TOML Section | Environment Variable | Macro Field |
|-------------------|--------------|---------------------|-------------|
| `tools.issue_create.auto_numbering` | `[tools.issue_create]`<br>`auto_numbering = true` | `SAH_ISSUE_CREATE_AUTO_NUMBERING=true` | `pub auto_numbering: bool = true` |
| `tools.search_index.chunk_size` | `[tools.search_index]`<br>`chunk_size = 1000` | `SAH_SEARCH_INDEX_CHUNK_SIZE=2000` | `pub chunk_size: usize = 1000` |
| `tools.memo_create.default_tags` | `[tools.memo_create]`<br>`default_tags = ["work"]` | `SAH_MEMO_CREATE_DEFAULT_TAGS="work,notes"` | `pub default_tags: Vec<String> = vec![]` |

### 4. Architecture: New Separate Configuration Crate

#### 4.1 New Crate Creation Requirements

**IMPORTANT: We are creating a completely new, standalone crate specifically for configuration:**

The configuration system **MUST** be implemented as its own separate crate called `swissarmyhammer-config`. This is not a refactoring of existing code - this is creating a brand new crate from scratch.

**Key Requirements:**
- **New standalone crate**: `swissarmyhammer-config` will be created as a new directory at the repository root
- **Lower-level dependency**: This config crate will be a dependency of other crates, not the other way around
- **No existing code migration**: We are not moving existing configuration code - we are building fresh
- **Shared across all crates**: The config crate will be used by `swissarmyhammer`, `swissarmyhammer-cli`, and `swissarmyhammer-tools`

**New Crate Structure (to be created):**
```
swissarmyhammer-config/           # <- NEW CRATE DIRECTORY
├── Cargo.toml                    # <- NEW cargo manifest
├── README.md                     # <- NEW crate documentation
├── src/
│   ├── lib.rs                    # <- NEW main library entry point
│   ├── core.rs                   # <- NEW core configuration types
│   ├── tools/                    # <- NEW tool-specific configurations
│   │   ├── mod.rs
│   │   ├── issues.rs             # <- NEW issue tool configurations
│   │   ├── memos.rs              # <- NEW memo tool configurations
│   │   ├── search.rs             # <- NEW search tool configurations
│   │   └── todos.rs              # <- NEW todo tool configurations
│   ├── macros.rs                 # <- NEW configuration generation macros
│   ├── figment_ext.rs            # <- NEW figment extensions and helpers
│   ├── validation.rs             # <- NEW configuration validation
│   └── error.rs                  # <- NEW error types
├── tests/                        # <- NEW integration tests
│   ├── config_loading.rs
│   ├── env_vars.rs
│   └── tool_configs.rs
└── examples/                     # <- NEW usage examples
    ├── basic_config.rs
    └── tool_config.rs
```

**Relationship to Existing Crates:**
```
swissarmyhammer-config/           # NEW: Low-level config crate (no dependencies on other SAH crates)
    ↑ (dependency)
    ├── swissarmyhammer/          # EXISTING: Will depend on swissarmyhammer-config
    ├── swissarmyhammer-cli/      # EXISTING: Will depend on swissarmyhammer-config  
    └── swissarmyhammer-tools/    # EXISTING: Will depend on swissarmyhammer-config
```

**Implementation Phases:**
1. **Phase 1**: Create new `swissarmyhammer-config` crate with basic structure
2. **Phase 2**: Implement `config_struct!` macro and core configuration types
3. **Phase 3**: Add tool-specific configuration support
4. **Phase 4**: Integration into existing crates via dependency updates
5. **Phase 5**: Deprecate existing configuration approaches (later phase)

#### 4.2 Configuration Generation Macro

A declarative macro system to generate configuration structs with automatic:
- Serde serialization/deserialization
- Default value handling
- Environment variable mapping
- Validation rules
- Documentation generation

**Macro Definition:**
```rust
/// Generate a configuration struct with automatic figment integration and robust defaulting
#[macro_export]
macro_rules! config_struct {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field:ident: $ty:ty = $default:expr
                $(, env = $env_var:literal)?
                $(, validate = $validator:expr)?
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $field_vis $field: $ty,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $default,)*
                }
            }
        }

        impl $name {
            /// Load configuration using Figment with fail-safe defaults
            pub fn load_from_figment(figment: &Figment) -> Result<Self, ConfigError> {
                // Let Figment handle all the heavy lifting:
                // - File format parsing (TOML, YAML, JSON)
                // - Environment variable parsing and type conversion
                // - Configuration source merging and precedence
                // - Serde deserialization
                
                match figment.extract::<Self>() {
                    Ok(config) => {
                        // Figment successfully loaded and parsed the config
                        config.validate()?;
                        Ok(config)
                    },
                    Err(_) => {
                        // Figment extraction failed (missing files, parse errors, etc.)
                        // Fall back to defaults for fail-safe behavior
                        let default_config = Self::default();
                        default_config.validate()?;
                        Ok(default_config)
                    }
                }
            }
            
            /// Validate configuration values with detailed error reporting
            pub fn validate(&self) -> Result<(), ConfigError> {
                $(
                    $(
                        if !($validator)(&self.$field) {
                            return Err(ConfigError::ValidationFailed {
                                field: format!("{}::{}", stringify!($name), stringify!($field)),
                                value: format!("{:?}", self.$field),
                                constraint: stringify!($validator).to_string(),
                            });
                        }
                    )?
                )*
                Ok(())
            }
            
            /// Get field defaults for documentation/help generation
            pub fn field_defaults() -> std::collections::HashMap<&'static str, String> {
                let mut defaults = std::collections::HashMap::new();
                $(
                    defaults.insert(stringify!($field), format!("{:?}", $default));
                )*
                defaults
            }
            
            /// Get environment variable mappings for documentation
            pub fn env_mappings() -> std::collections::HashMap<&'static str, &'static str> {
                let mut mappings = std::collections::HashMap::new();
                $(
                    $(
                        mappings.insert(stringify!($field), $env_var);
                    )?
                )*
                mappings
            }
        }
        
        impl ToolConfig for $name {
            fn load_from_global(global: &SwissArmyHammerConfig) -> Result<Self, ConfigError> {
                // Create a figment with the same sources as global config
                // but scoped to this tool's configuration section
                let figment = global.create_tool_figment(stringify!($name));
                Self::load_from_figment(&figment)
            }
        }
    };
}
```

**Usage Examples with Robust Defaulting:**
```rust
use swissarmyhammer_config::config_struct;

config_struct! {
    /// Configuration for issue creation tool
    /// Each field MUST have a sensible default that allows the tool to work
    pub struct IssueCreateConfig {
        /// Enable automatic issue numbering
        /// Default: true (most users want auto-numbering)
        pub auto_numbering: bool = true,
        env = "SAH_ISSUE_CREATE_AUTO_NUMBERING",
        
        /// Default issue template to use when creating issues
        /// Default: "issue" (assumes basic issue template exists)
        pub default_template: String = "issue".to_string(),
        env = "SAH_ISSUE_CREATE_DEFAULT_TEMPLATE",
        validate = |s| !s.is_empty(),
        
        /// Require description field when creating issues
        /// Default: true (enforces good documentation practices)
        pub require_description: bool = true,
        env = "SAH_ISSUE_CREATE_REQUIRE_DESCRIPTION",
        
        /// Maximum length for issue titles
        /// Default: 200 characters (reasonable limit)
        pub max_title_length: usize = 200,
        env = "SAH_ISSUE_CREATE_MAX_TITLE_LENGTH",
        validate = |&x| x >= 10 && x <= 500,
        
        /// Default priority for new issues
        /// Default: "medium" (neutral starting point)
        pub default_priority: String = "medium".to_string(),
        env = "SAH_ISSUE_CREATE_DEFAULT_PRIORITY",
        validate = |s| ["low", "medium", "high", "critical"].contains(&s.as_str()),
    }
}

config_struct! {
    /// Configuration for search indexing tool
    /// Defaults chosen for reasonable performance and coverage
    pub struct SearchIndexConfig {
        /// Automatically index files when they change
        /// Default: false (explicit indexing to avoid performance issues)
        pub auto_index_on_changes: bool = false,
        env = "SAH_SEARCH_INDEX_AUTO_INDEX",
        
        /// File patterns to include in indexing
        /// Default: Common development file types
        pub include_patterns: Vec<String> = vec![
            "**/*.rs".to_string(), 
            "**/*.md".to_string(), 
            "**/*.py".to_string(),
            "**/*.js".to_string(),
            "**/*.ts".to_string(),
        ],
        env = "SAH_SEARCH_INDEX_INCLUDE_PATTERNS",
        validate = |v| !v.is_empty(),
        
        /// Exclude patterns to avoid indexing build artifacts
        /// Default: Common ignore patterns
        pub exclude_patterns: Vec<String> = vec![
            "target/**".to_string(),
            ".git/**".to_string(),
            "node_modules/**".to_string(),
            ".swissarmyhammer/tmp/**".to_string(),
        ],
        env = "SAH_SEARCH_INDEX_EXCLUDE_PATTERNS",
        
        /// Chunk size for text processing
        /// Default: 1000 characters (balance between context and performance)
        pub chunk_size: usize = 1000,
        env = "SAH_SEARCH_INDEX_CHUNK_SIZE",
        validate = |&x| x >= 100 && x <= 10000,
        
        /// Maximum number of files to process in a single batch
        /// Default: 50 files (prevents memory issues)
        pub batch_size: usize = 50,
        env = "SAH_SEARCH_INDEX_BATCH_SIZE",
        validate = |&x| x >= 1 && x <= 500,
    }
}

config_struct! {
    /// Configuration for memo creation tool  
    /// Optimized for note-taking workflows
    pub struct MemoCreateConfig {
        /// Automatically add timestamp to memo content
        /// Default: true (helps with organization)
        pub auto_timestamp: bool = true,
        env = "SAH_MEMO_CREATE_AUTO_TIMESTAMP",
        
        /// Default tags to apply to new memos
        /// Default: empty (user can configure as needed)
        pub default_tags: Vec<String> = vec![],
        env = "SAH_MEMO_CREATE_DEFAULT_TAGS",
        
        /// Template to use for memo creation
        /// Default: "memo" (basic memo template)
        pub default_template: String = "memo".to_string(),
        env = "SAH_MEMO_CREATE_DEFAULT_TEMPLATE",
        validate = |s| !s.is_empty(),
        
        /// Automatically open memo in editor after creation
        /// Default: false (non-intrusive behavior)
        pub auto_edit: bool = false,
        env = "SAH_MEMO_CREATE_AUTO_EDIT",
    }
}
```

**Key Principles for Tool Defaults:**

1. **Always Functional**: Every default must allow the tool to work out-of-the-box
2. **Conservative**: Choose safe defaults that won't cause issues (e.g., auto_index = false)
3. **User-Friendly**: Defaults should match common usage patterns
4. **Documented**: Comments explain why each default was chosen
5. **Validated**: Each field should have appropriate validation rules
6. **Environment Override**: Critical settings should have env var overrides

#### 4.2 Fail-Safe Configuration Behavior

The configuration system is designed to **never fail** due to missing or invalid configuration. Tools will always get a working configuration object:

**Scenario 1: No Configuration Files**
```rust
// Even with no config files at all, tools work with defaults
let config = context.get_config::<IssueCreateConfig>()?;
// config.auto_numbering = true (from default)
// config.default_template = "issue" (from default)  
// config.require_description = true (from default)
```

**Scenario 2: Partial Configuration**
```toml
# User only configures one field in sah.toml
[tools.issue_create]
auto_numbering = false
# All other fields fall back to defaults
```

```rust
let config = context.get_config::<IssueCreateConfig>()?;
// config.auto_numbering = false (from config file)
// config.default_template = "issue" (from default - not in file)
// config.require_description = true (from default - not in file)
```

**Scenario 3: Invalid Environment Variables**
```bash
# Invalid environment variable value
export SAH_ISSUE_CREATE_MAX_TITLE_LENGTH=invalid_number
```

```rust
// System logs warning but continues with default:
// WARN: Invalid value for environment variable SAH_ISSUE_CREATE_MAX_TITLE_LENGTH: 'invalid_number'. Using default: 200
let config = context.get_config::<IssueCreateConfig>()?;
// config.max_title_length = 200 (falls back to default)
```

**Scenario 4: Malformed Configuration File**
```toml
# Broken TOML syntax
[tools.issue_create
auto_numbering = tru  # missing closing bracket, typo
```

```rust
// Figment extraction fails gracefully, defaults are used
let config = context.get_config::<IssueCreateConfig>()?;
// All fields use their macro-defined defaults
```

**Scenario 5: Configuration Validation Failures**
```toml
[tools.issue_create]
max_title_length = 5000  # Exceeds validation limit of 500
```

```rust
// Validation fails, but system provides actionable error:
// ERROR: ValidationFailed { 
//   field: "IssueCreateConfig::max_title_length",
//   value: "5000", 
//   constraint: "|&x| x >= 10 && x <= 500" 
// }
// Tool execution stops with clear error message
```

**Benefits of Fail-Safe Design:**

1. **Zero Configuration Required**: Tools work immediately after installation
2. **Partial Configuration**: Users only need to configure what they want to change
3. **Graceful Degradation**: Invalid configs produce warnings, not failures
4. **Clear Error Messages**: Validation failures provide specific guidance
5. **Development Friendly**: New tools work without requiring config updates
6. **Production Safe**: Misconfigurations don't break the entire system

#### 4.3 Tool Configuration Registry

A centralized registry for all tool configurations:

```rust
/// Registry of all tool configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub issues: IssueConfig,
    pub memos: MemoConfig,
    pub search: SearchConfig,
    pub todos: TodoConfig,
    pub outline: OutlineConfig,
    // ... other tools
}

impl ToolsConfig {
    /// Get configuration for a specific tool
    pub fn get_tool_config<T>(&self) -> &T
    where
        T: ToolConfig,
    {
        T::from_tools_config(self)
    }
}

/// Trait for tool-specific configurations
pub trait ToolConfig {
    fn from_tools_config(tools: &ToolsConfig) -> &Self;
    fn tool_name() -> &'static str;
}

impl ToolConfig for IssueConfig {
    fn from_tools_config(tools: &ToolsConfig) -> &Self {
        &tools.issues
    }
    
    fn tool_name() -> &'static str {
        "issues"
    }
}
```

#### 4.4 Crate Dependencies

**swissarmyhammer-config/Cargo.toml:**
```toml
[package]
name = "swissarmyhammer-config"
version = "0.1.0"
edition = "2021"

[dependencies]
figment = { version = "0.10", features = ["toml", "yaml", "json", "env"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
thiserror = "1.0"
```

**Dependencies in other crates:**
```toml
# swissarmyhammer-tools/Cargo.toml
[dependencies]
swissarmyhammer-config = { path = "../swissarmyhammer-config" }

# swissarmyhammer-cli/Cargo.toml  
[dependencies]
swissarmyhammer-config = { path = "../swissarmyhammer-config" }

# swissarmyhammer/Cargo.toml
[dependencies]
swissarmyhammer-config = { path = "../swissarmyhammer-config" }
```

### 5. Implementation Details

#### 5.1 Figment Integration - Adding Value on Top

Our configuration system leverages Figment's existing capabilities and adds:

1. **Tool-specific configuration scoping** 
2. **Fail-safe default behavior**
3. **Custom validation rules**
4. **Configuration caching and hot-reloading**

```rust
use figment::{Figment, providers::{Format, Toml, Yaml, Json, Env}};
use serde::{Deserialize, Serialize};
use swissarmyhammer_config::{config_struct};

/// Main configuration that uses Figment's standard capabilities
impl SwissArmyHammerConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let figment = Self::create_figment();
        Self::load_from_figment(&figment)
    }
    
    /// Create a Figment with our standard configuration sources
    /// This leverages Figment's existing file discovery and merging
    pub fn create_figment() -> Figment {
        Figment::new()
            // Figment handles file format detection automatically
            .merge(Toml::file("~/.swissarmyhammer/sah.toml"))
            .merge(Yaml::file("~/.swissarmyhammer/sah.yaml"))
            .merge(Json::file("~/.swissarmyhammer/sah.json"))
            
            .merge(Toml::file(".swissarmyhammer/sah.toml"))
            .merge(Yaml::file(".swissarmyhammer/sah.yaml"))
            .merge(Json::file(".swissarmyhammer/sah.json"))
            
            .merge(Toml::file("sah.toml"))
            .merge(Yaml::file("sah.yaml"))
            .merge(Json::file("sah.json"))
            
            // Figment handles environment variable parsing and type conversion
            .merge(Env::prefixed("SAH_").split("_"))
    }
    
    /// Create a tool-scoped figment - this is our value-add
    /// Figment doesn't know about tool configuration scoping
    pub fn create_tool_figment(&self, tool_name: &str) -> Figment {
        // Start with the same sources as global config
        let mut figment = Self::create_figment();
        
        // Add tool-specific environment variable scoping
        // This is where we add value beyond what Figment provides
        let tool_env_prefix = format!("SAH_{}", tool_name.to_uppercase());
        figment = figment.merge(Env::prefixed(&tool_env_prefix).split("_"));
        
        figment
    }
}
```

**What Figment Provides (we don't duplicate):**
- File format parsing (TOML, YAML, JSON)
- Environment variable parsing and type conversion  
- Configuration source merging with precedence
- Serde integration for deserialization
- Path expansion and file discovery

**What We Add on Top:**
- Tool-specific configuration scoping and caching
- Fail-safe behavior with guaranteed defaults
- Custom validation rules per configuration field
- Configuration hot-reloading and change detection
- Tool context integration for MCP tools
```

#### 4.2 Configuration Validation
```rust
impl SwissArmyHammerConfig {
    pub fn validate(&self) -> Result<(), Vec<ConfigError>> {
        let mut errors = Vec::new();
        
        // Validate paths exist and are accessible
        if !self.paths.home_dir.exists() {
            errors.push(ConfigError::InvalidPath("home_dir".into()));
        }
        
        // Validate search model is supported
        if !SUPPORTED_MODELS.contains(&self.search.model.as_str()) {
            errors.push(ConfigError::UnsupportedModel(self.search.model.clone()));
        }
        
        // Validate security settings
        if self.security.sandbox_mode && self.security.allow_shell_commands {
            errors.push(ConfigError::ConflictingSettings(
                "Cannot allow shell commands in sandbox mode".into()
            ));
        }
        
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### 6. Tool Configuration Integration

#### 6.1 Tool-Specific Configuration Modules

Each MCP tool defines its own configuration in its own submodule using the config macro:

```rust
// swissarmyhammer-tools/src/mcp/tools/issues/create/config.rs
use swissarmyhammer_config::config_struct;

config_struct! {
    /// Configuration for issue creation tool
    pub struct IssueCreateConfig {
        /// Enable automatic issue numbering
        pub auto_numbering: bool = true,
        env = "SAH_ISSUE_CREATE_AUTO_NUMBERING",
        
        /// Default issue template to use
        pub default_template: String = "issue".to_string(),
        env = "SAH_ISSUE_CREATE_DEFAULT_TEMPLATE",
        
        /// Require description for new issues
        pub require_description: bool = true,
        env = "SAH_ISSUE_CREATE_REQUIRE_DESCRIPTION",
        validate = |_| true,
    }
}
```

```rust
// swissarmyhammer-tools/src/mcp/tools/search/index/config.rs
use swissarmyhammer_config::config_struct;

config_struct! {
    /// Configuration for search indexing tool
    pub struct SearchIndexConfig {
        /// Auto-index files when they change
        pub auto_index_on_changes: bool = false,
        env = "SAH_SEARCH_INDEX_AUTO_INDEX",
        
        /// File patterns to include in indexing
        pub include_patterns: Vec<String> = vec!["**/*.rs".to_string(), "**/*.md".to_string()],
        env = "SAH_SEARCH_INDEX_INCLUDE_PATTERNS",
        
        /// Chunk size for text processing
        pub chunk_size: usize = 1000,
        env = "SAH_SEARCH_INDEX_CHUNK_SIZE",
        validate = |&x| x >= 100 && x <= 10000,
    }
}
```

#### 6.2 ToolContext Integration

Each tool accesses its configuration through the `ToolContext`:

```rust
// swissarmyhammer-tools/src/mcp/tools/issues/create/mod.rs
use crate::mcp::context::ToolContext;
mod config;
use config::IssueCreateConfig;

pub struct IssueCreateTool;

impl IssueCreateTool {
    pub async fn execute(
        &self, 
        args: serde_json::Map<String, serde_json::Value>, 
        context: &ToolContext
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        // Get tool-specific configuration from context
        let config = context.get_config::<IssueCreateConfig>()?;
        
        // Use configuration values with args override
        let auto_number = args.get("auto_numbering")
            .and_then(|v| v.as_bool())
            .unwrap_or(config.auto_numbering);
            
        let template = args.get("template")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| config.default_template.clone());
            
        if config.require_description && !args.contains_key("description") {
            return Err("Description is required for issue creation".into());
        }
            
        // ... rest of tool logic using config values
    }
}
```

#### 6.3 ToolContext Implementation

The `ToolContext` provides a unified interface for configuration access:

```rust
// swissarmyhammer-tools/src/mcp/context.rs
use swissarmyhammer_config::{SwissArmyHammerConfig, ConfigError};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct ToolContext {
    /// Global configuration
    global_config: Arc<SwissArmyHammerConfig>,
    /// Cached tool configurations  
    tool_configs: RwLock<HashMap<TypeId, Box<dyn std::any::Any + Send + Sync>>>,
}

impl ToolContext {
    pub fn new(global_config: SwissArmyHammerConfig) -> Self {
        Self {
            global_config: Arc::new(global_config),
            tool_configs: RwLock::new(HashMap::new()),
        }
    }
    
    /// Get tool-specific configuration, loading and caching it if needed
    pub fn get_config<T>(&self) -> Result<Arc<T>, ConfigError>
    where
        T: ToolConfig + Clone + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        
        // Check cache first
        {
            let cache = self.tool_configs.read().unwrap();
            if let Some(cached) = cache.get(&type_id) {
                if let Some(config) = cached.downcast_ref::<Arc<T>>() {
                    return Ok(config.clone());
                }
            }
        }
        
        // Load configuration for this tool type
        let config = T::load_from_global(&self.global_config)?;
        let arc_config = Arc::new(config);
        
        // Cache for future use
        {
            let mut cache = self.tool_configs.write().unwrap();
            cache.insert(type_id, Box::new(arc_config.clone()));
        }
        
        Ok(arc_config)
    }
    
    /// Invalidate cached configurations (for hot-reloading)
    pub fn invalidate_cache(&self) {
        let mut cache = self.tool_configs.write().unwrap();
        cache.clear();
    }
}

/// Trait for tool configurations that can be loaded from global config
pub trait ToolConfig {
    fn load_from_global(global: &SwissArmyHammerConfig) -> Result<Self, ConfigError>
    where
        Self: Sized;
}
```

#### 6.2 Configuration Precedence for Tools

Tool configurations follow the same precedence as core configurations:

1. **Tool defaults** (defined in macro)
2. **Global config file** `~/.swissarmyhammer/sah.toml`
3. **Project config file** `.swissarmyhammer/sah.toml`  
4. **Local config file** `sah.toml`
5. **Environment variables** (e.g., `SAH_ISSUES_AUTO_NUMBERING=false`)
6. **Runtime arguments** (passed to MCP tool)

#### 6.4 Tool Configuration Directory Structure

Each tool has its own configuration module within its implementation:

```
swissarmyhammer-tools/src/mcp/tools/
├── issues/
│   ├── create/
│   │   ├── mod.rs              # Tool implementation
│   │   └── config.rs           # IssueCreateConfig
│   ├── list/
│   │   ├── mod.rs              # Tool implementation  
│   │   └── config.rs           # IssueListConfig
│   └── work/
│       ├── mod.rs              # Tool implementation
│       └── config.rs           # IssueWorkConfig
├── memos/
│   ├── create/
│   │   ├── mod.rs              # Tool implementation
│   │   └── config.rs           # MemoCreateConfig
│   └── search/
│       ├── mod.rs              # Tool implementation
│       └── config.rs           # MemoSearchConfig
├── search/
│   ├── index/
│   │   ├── mod.rs              # Tool implementation
│   │   └── config.rs           # SearchIndexConfig
│   └── query/
│       ├── mod.rs              # Tool implementation
│       └── config.rs           # SearchQueryConfig
└── todos/
    ├── create/
    │   ├── mod.rs              # Tool implementation
    │   └── config.rs           # TodoCreateConfig
    └── show/
        ├── mod.rs              # Tool implementation
        └── config.rs           # TodoShowConfig
```

#### 6.5 Configuration File Organization

Tool configurations are organized in the TOML configuration files using nested sections:

```toml
# Global settings
debug = false
verbose = false

# Core configurations  
[search]
enabled = true
model = "nomic-embed-text-v1.5"

[workflow]
parallel_execution = true

# Tool-specific configurations
[tools.issue_create]
auto_numbering = true
default_template = "issue"
require_description = true

[tools.issue_list]  
default_format = "table"
show_completed = false
max_items = 50

[tools.search_index]
auto_index_on_changes = false
include_patterns = ["**/*.rs", "**/*.md"]
chunk_size = 1000

[tools.search_query]
default_limit = 10
min_similarity = 0.7
include_context = true
```

#### 6.6 Dynamic Configuration Updates

Tools can respond to configuration changes:

```rust
/// Configuration watcher for hot-reloading
pub struct ConfigWatcher {
    config: Arc<RwLock<SwissArmyHammerConfig>>,
    watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    /// Reload configuration when files change
    pub fn reload_config(&self) -> Result<(), ConfigError> {
        let new_config = SwissArmyHammerConfig::load()?;
        let mut config_guard = self.config.write().unwrap();
        *config_guard = new_config;
        Ok(())
    }
}

/// Tools can subscribe to configuration changes
pub trait ConfigurableTool {
    fn on_config_changed(&mut self, new_config: &SwissArmyHammerConfig);
}
```

### 7. Configuration Management Commands

#### 5.1 CLI Configuration Commands
```bash
# View current configuration
sah config show

# View configuration with sources
sah config show --sources

# Get specific configuration value
sah config get search.model
sah config get paths.cache_dir

# Set configuration value (writes to appropriate config file)
sah config set search.model "custom-model"
sah config set debug true

# Validate configuration
sah config validate

# Show configuration file locations
sah config locations

# Generate sample configuration file
sah config init --format toml --location local
sah config init --format yaml --location project
sah config init --format json --location global
```

#### 5.2 Configuration Profiles
```bash
# Use specific configuration profile
sah --profile development workflow run test
sah --profile production search index

# Profile-specific config files
sah.development.toml
sah.production.yaml
swissarmyhammer.staging.json
```

### 6. Migration Strategy

#### 6.1 From Current System
- Migrate existing `src/sah_config/` to use Figment
- Convert current TOML parsing to Figment-based approach
- Maintain backward compatibility with existing config files
- Provide migration tool for old configuration format

#### 6.2 Gradual Rollout
1. **Phase 1**: Implement Figment-based configuration loading
2. **Phase 2**: Add environment variable support
3. **Phase 3**: Implement multi-format file support  
4. **Phase 4**: Add configuration management CLI commands
5. **Phase 5**: Deprecate old configuration system

### 7. Error Handling

#### 7.1 Configuration Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),
    
    #[error("Invalid configuration format: {0}")]
    InvalidFormat(String),
    
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Unsupported model: {0}")]
    UnsupportedModel(String),
    
    #[error("Conflicting settings: {0}")]
    ConflictingSettings(String),
}
```

#### 7.2 Graceful Degradation
- Continue with defaults if config files are missing
- Warn about invalid configuration values
- Provide helpful error messages with suggestions
- Support partial configuration loading

### 8. Testing Strategy

#### 8.1 Unit Tests
- Configuration loading from different sources
- Precedence order validation
- Environment variable parsing
- Configuration validation logic

#### 8.2 Integration Tests
- Multi-format configuration file support
- Configuration command CLI interface
- Migration from old to new configuration system
- Cross-platform path handling

#### 8.3 Configuration Test Fixtures
```
tests/
├── configs/
│   ├── valid/
│   │   ├── sah.toml
│   │   ├── sah.yaml
│   │   ├── sah.json
│   │   └── swissarmyhammer.toml
│   ├── invalid/
│   │   ├── malformed.toml
│   │   ├── missing-required.yaml
│   │   └── conflicting.json
│   └── migration/
│       ├── old-format.toml
│       └── expected-new.toml
```

### 9. Documentation Updates

#### 9.1 User Documentation
- Configuration file format examples
- Environment variable reference
- Configuration precedence explanation
- Common configuration patterns

#### 9.2 Developer Documentation
- Figment integration implementation
- Configuration schema definition
- Error handling patterns
- Testing configuration loading

### 10. Performance Considerations

#### 10.1 Configuration Caching
- Cache loaded configuration to avoid repeated file I/O
- Implement configuration change detection
- Provide configuration reload mechanisms

#### 10.2 Lazy Loading
- Load configuration sections on demand
- Avoid expensive validation until needed
- Support partial configuration updates

### 11. Success Criteria

- [ ] Figment-based configuration system implemented
- [ ] Support for TOML, YAML, and JSON formats
- [ ] Proper precedence order (pwd → .swissarmyhammer/ → ~/.swissarmyhammer/)
- [ ] Environment variable integration with SAH_ and SWISSARMYHAMMER_ prefixes
- [ ] Both `sah.*` and `swissarmyhammer.*` filename support
- [ ] Configuration management CLI commands
- [ ] Migration path from existing configuration system
- [ ] Comprehensive error handling and validation
- [ ] Complete test coverage
- [ ] Updated documentation with examples

## Conclusion

This specification provides a robust, flexible configuration system using Figment that supports multiple formats, clear precedence rules, and comprehensive environment variable integration. The implementation will significantly improve the user experience while maintaining backward compatibility with existing configurations.