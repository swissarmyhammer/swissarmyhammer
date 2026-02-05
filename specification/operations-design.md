# Operations Design

## Overview

Operations are structs where the fields ARE the parameters. No duplication.
Attribute macros add metadata. Operations live in domain crates (e.g., `swissarmyhammer-kanban`), not the tools/MCP layer.

## Example Operation

```rust
// In swissarmyhammer-kanban/src/operations/task.rs

#[operation(verb = "add", noun = "task", description = "Create a new task")]
#[derive(Debug, Deserialize)]
pub struct AddTask {
    /// The task title
    #[param(short = 't', alias = "name")]
    pub title: String,

    /// Optional description
    #[param(alias = "desc")]
    pub description: Option<String>,

    /// Target column
    #[param(short = 'c')]
    pub column: Option<String>,
}

impl AddTask {
    pub async fn execute<C: Context>(&self, ctx: &C) -> Result<Value, OperationError> {
        // Use self.title, self.description, self.column directly
        // Call into existing kanban domain logic
    }
}
```

## Key Principles

1. **Struct fields ARE parameters** - no separate parameter definitions
2. **`#[operation(...)]`** - struct-level attribute for verb, noun, description
3. **`#[param(...)]`** - field-level attribute for CLI extras (short, alias)
4. **Doc comments** on fields become parameter descriptions
5. **Type inference** - `Option<T>` = optional, `T` = required
6. **Generic execute** - `execute<C: Context>(&self, ctx: &C)` works with any context
7. **Domain location** - Operations live in domain crates (swissarmyhammer-kanban), not MCP/tools layer

## CLI Generation

The CLI layer reads the operation metadata to generate:
- `sah tool kanban task add --title "My task"`
- Noun-verb structure: `<tool> <noun> <verb> [params]`
- Short flags from `#[param(short = 'x')]`
- Aliases from `#[param(alias = "name")]`
- Help text from doc comments

## Context Trait

```rust
pub trait Context: Send + Sync {
    fn working_directory(&self) -> &Path;
    // Other context methods as needed
}
```

## Crate Structure

- `swissarmyhammer-operations` - Core traits (Operation, Context) and derive macros
- `swissarmyhammer-kanban/src/operations/` - Kanban-specific operations (AddTask, MoveTask, etc.)
- `swissarmyhammer-tools` - MCP layer, calls into operations
- `swissarmyhammer-cli` - CLI layer, generates commands from operation metadata
