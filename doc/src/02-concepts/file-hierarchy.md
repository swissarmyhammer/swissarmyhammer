# File Hierarchy

SwissArmyHammer uses a three-tier file hierarchy system that provides clear precedence rules for prompts, workflows, and configuration files.

## Hierarchy Levels

### 1. Builtin (Highest Priority)
- **Location**: Embedded in the SwissArmyHammer binary
- **Purpose**: Core prompts and workflows shipped with the application
- **Examples**: `say-hello`, `hello-world`, `implement`
- **Characteristics**: Always available, cannot be modified

### 2. User
- **Location**: `~/.swissarmyhammer/`
- **Purpose**: Personal collection shared across all projects
- **Structure**:
  ```
  ~/.swissarmyhammer/
  ├── prompts/
  ├── workflows/
  ├── memoranda/
  ├── issues/
  └── sah.toml (configuration)
  ```
- **Characteristics**: User-specific, persistent across projects

### 3. Local (Lowest Priority)
- **Location**: `./.swissarmyhammer/` (current directory and parents)
- **Purpose**: Project-specific customizations
- **Structure**:
  ```
  ./.swissarmyhammer/
  ├── prompts/
  ├── workflows/
  ├── memoranda/
  ├── issues/
  └── sah.toml (project configuration)
  ```
- **Characteristics**: Project-specific, version controlled

## Precedence Rules

When SwissArmyHammer looks for resources, it searches in this order:

1. **Local** (`./.swissarmyhammer/`)
2. **User** (`~/.swissarmyhammer/`)  
3. **Builtin** (embedded)

The first match wins, allowing local files to override user files, and user files to override builtins.

## Directory Discovery

SwissArmyHammer searches for `.swissarmyhammer/` directories by walking up the directory tree from the current working directory until it finds one or reaches the filesystem root.

## File Types

### Prompts (`prompts/`)
- Extension: `.md`
- YAML front matter required
- Support Liquid templating
- Can be nested in subdirectories

### Workflows (`workflows/`)
- Extension: `.md`
- YAML front matter required  
- State machine definitions
- Mermaid diagrams for visualization

### Configuration Files
- Names: `sah.{toml,yaml,yml,json}`
- Alternative: `swissarmyhammer.{toml,yaml,yml,json}`
- Environment variable substitution supported

### Generated Content
- `memoranda/` - Notes and documentation
- `issues/` - Project issue tracking
- `search.db` - Semantic search index (auto-generated)

## Best Practices

- **Keep builtins unchanged** - Use local/user overrides instead
- **Version control local configs** - Include `.swissarmyhammer/` in git
- **Use user level for personal tools** - Share prompts across projects
- **Organize with subdirectories** - Group related prompts logically