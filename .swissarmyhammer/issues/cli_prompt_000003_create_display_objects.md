# Create Display Objects for Prompt Command Output

Refer to /Users/wballard/github/swissarmyhammer/ideas/cli_prompt.md

## Overview

Create clean display objects with both `Tabled` and `Serialize` derives for consistent output formatting across table, JSON, and YAML formats. This enables the CliContext display pattern.

## Current State

- Manual output formatting in `run_list_command()` 
- Inconsistent formatting across different output modes
- No structured display objects

## Goals

- Clean display objects with proper derives
- Support for verbose and non-verbose output modes
- Consistent formatting across all output types
- Integration with CliContext.display() method

## Implementation Steps

### 1. Create Display Module

**File**: `swissarmyhammer-cli/src/commands/prompt/display.rs`

```rust
use serde::Serialize;
use tabled::Tabled;

/// Basic prompt information for standard list output
#[derive(Tabled, Serialize, Debug, Clone)]
pub struct PromptRow {
    #[tabled(rename = "Name")]
    pub name: String,
    
    #[tabled(rename = "Title")]
    pub title: String,
}

/// Detailed prompt information for verbose list output  
#[derive(Tabled, Serialize, Debug, Clone)]
pub struct VerbosePromptRow {
    #[tabled(rename = "Name")]
    pub name: String,
    
    #[tabled(rename = "Title")] 
    pub title: String,
    
    #[tabled(rename = "Description")]
    pub description: String,
    
    #[tabled(rename = "Source")]
    pub source: String,
    
    #[tabled(rename = "Category")]
    pub category: Option<String>,
}

impl From<&swissarmyhammer_prompts::Prompt> for PromptRow {
    fn from(prompt: &swissarmyhammer_prompts::Prompt) -> Self {
        Self {
            name: prompt.name.clone(),
            title: prompt.metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("No title")
                .to_string(),
        }
    }
}

impl From<&swissarmyhammer_prompts::Prompt> for VerbosePromptRow {
    fn from(prompt: &swissarmyhammer_prompts::Prompt) -> Self {
        Self {
            name: prompt.name.clone(),
            title: prompt.metadata
                .get("title") 
                .and_then(|v| v.as_str())
                .unwrap_or("No title")
                .to_string(),
            description: prompt.description
                .as_deref()
                .unwrap_or("No description")
                .to_string(),
            source: prompt.source
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or("Unknown".to_string()),
            category: prompt.category.clone(),
        }
    }
}

/// Convert prompts to appropriate display format based on verbose flag
pub fn prompts_to_display_rows(
    prompts: Vec<swissarmyhammer_prompts::Prompt>,
    verbose: bool,
) -> DisplayRows {
    if verbose {
        DisplayRows::Verbose(
            prompts.iter().map(VerbosePromptRow::from).collect()
        )
    } else {
        DisplayRows::Standard(
            prompts.iter().map(PromptRow::from).collect()
        )
    }
}

#[derive(Debug)]
pub enum DisplayRows {
    Standard(Vec<PromptRow>),
    Verbose(Vec<VerbosePromptRow>),
}
```

### 2. Implement Display for CliContext

**File**: `swissarmyhammer-cli/src/context.rs`

```rust
use crate::cli::OutputFormat;

impl CliContext {
    /// Display items using the configured format
    pub fn display<T>(&self, items: Vec<T>) -> Result<(), DisplayError>
    where
        T: Tabled + Serialize,
    {
        match self.format {
            OutputFormat::Table => {
                if items.is_empty() {
                    println!("No items to display");
                } else {
                    println!("{}", Table::new(&items));
                }
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&items)
                    .map_err(|e| DisplayError::SerializationFailed(e.to_string()))?;
                println!("{}", json);
            }
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(&items)
                    .map_err(|e| DisplayError::SerializationFailed(e.to_string()))?;
                println!("{}", yaml);
            }
        }
        Ok(())
    }

    /// Display different types based on verbose flag
    pub fn display_prompts(
        &self, 
        rows: crate::commands::prompt::display::DisplayRows
    ) -> Result<(), DisplayError> {
        use crate::commands::prompt::display::DisplayRows;
        
        match rows {
            DisplayRows::Standard(items) => self.display(items),
            DisplayRows::Verbose(items) => self.display(items),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DisplayError {
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),
}
```

### 3. Update Dependencies

**File**: `swissarmyhammer-cli/Cargo.toml`

```toml
[dependencies]
# Add if not already present
tabled = "0.15"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
```

### 4. Update Module Exports

**File**: `swissarmyhammer-cli/src/commands/prompt/mod.rs`

```rust
pub mod cli;
pub mod display; 
// ... existing modules
```

## Testing Requirements

### Unit Tests

**File**: `swissarmyhammer-cli/src/commands/prompt/display.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_prompts::Prompt;

    #[test]
    fn test_prompt_row_conversion() {
        let prompt = create_test_prompt();
        let row = PromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.title, "Test Title");
    }

    #[test]
    fn test_verbose_prompt_row_conversion() {
        let prompt = create_test_prompt();
        let row = VerbosePromptRow::from(&prompt);
        assert_eq!(row.name, "test-prompt");
        assert_eq!(row.description, "Test description");
    }

    fn create_test_prompt() -> Prompt {
        // Create test prompt with all required fields
    }
}
```

### Integration Tests
- Test display formatting for different output modes
- Test verbose vs non-verbose display
- Test empty prompt lists

## Success Criteria

1. ✅ Clean display objects with Tabled + Serialize derives
2. ✅ Support for both verbose and standard output modes
3. ✅ CliContext.display() method works correctly
4. ✅ Consistent formatting across table/JSON/YAML
5. ✅ Proper handling of missing metadata
6. ✅ All tests pass

## Files Created

- `swissarmyhammer-cli/src/commands/prompt/display.rs` - Display objects and formatting

## Files Modified

- `swissarmyhammer-cli/src/context.rs` - Add display methods
- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Export display module
- `swissarmyhammer-cli/Cargo.toml` - Add dependencies if needed

## Risk Mitigation

- Comprehensive unit tests for all conversions
- Test with real prompt data to validate formatting
- Test edge cases like missing metadata

---

**Estimated Effort**: Medium (150-250 lines)
**Dependencies**: cli_prompt_000001_add_global_format_argument
**Blocks**: cli_prompt_000004_create_list_handler