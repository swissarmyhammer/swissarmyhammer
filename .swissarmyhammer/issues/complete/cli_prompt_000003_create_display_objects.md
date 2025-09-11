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
## Proposed Solution

After examining the existing codebase, I've identified the following implementation approach:

### Current State Analysis
- `CliContext` already has a `display()` method that works with `serde::Serialize` types
- The method supports JSON/YAML formats but falls back to JSON for Table format
- `tabled` is available in workspace dependencies 
- Current `run_list_command()` manually formats output instead of using structured objects

### Implementation Plan

1. **Create display.rs module** with:
   - `PromptRow` struct for standard list output (name, title)
   - `VerbosePromptRow` struct for detailed output (name, title, description, source, category)  
   - Both structs derive `Tabled` and `Serialize`
   - `From` trait implementations for `swissarmyhammer_prompts::Prompt`
   - Helper function `prompts_to_display_rows()` to convert prompts based on verbose flag

2. **Update CliContext** with:
   - Enhanced `display()` method that properly supports `Tabled` types for Table format
   - New `display_prompts()` method to handle different display types based on verbose flag
   - `DisplayError` type for proper error handling

3. **Update run_list_command()** to:
   - Use new display objects instead of manual formatting
   - Call `context.display_prompts()` for consistent output across all formats

4. **Update module exports** to include the new display module

### Key Benefits
- Consistent formatting across Table, JSON, and YAML outputs
- Proper table formatting using `tabled` crate
- Clean separation of display logic from business logic  
- Support for both verbose and non-verbose modes
- Type-safe display objects with proper derives

### Files to Modify
- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Update list command logic
- `swissarmyhammer-cli/src/context.rs` - Enhance display methods  
- `swissarmyhammer-cli/src/commands/prompt/mod.rs` - Add display module export

### Files to Create
- `swissarmyhammer-cli/src/commands/prompt/display.rs` - Display objects and formatters

This approach maintains backward compatibility while providing the structured display objects needed for consistent output formatting.
## Implementation Progress

### ✅ Completed Tasks

1. **Created display.rs module** (`swissarmyhammer-cli/src/commands/prompt/display.rs`):
   - Implemented `PromptRow` struct with `Tabled` and `Serialize` derives
   - Implemented `VerbosePromptRow` struct with additional fields (description, source, category)
   - Added `From` trait implementations to convert from `swissarmyhammer_prompts::Prompt`
   - Created `DisplayRows` enum to handle different row types
   - Added `prompts_to_display_rows()` helper function
   - Comprehensive unit tests covering all conversion scenarios

2. **Enhanced CliContext** (`swissarmyhammer-cli/src/context.rs`):
   - Updated `display()` method to properly support `Tabled` trait for table formatting
   - Added `display_prompts()` method to handle `DisplayRows` enum
   - Fixed table output to use `tabled::Table::new()` instead of JSON fallback
   - Maintained backward compatibility with existing JSON/YAML output

3. **Updated run_list_command()** (`swissarmyhammer-cli/src/commands/prompt/mod.rs`):
   - Replaced manual formatting with structured display objects
   - Now calls `context.display_prompts()` for consistent output across all formats
   - Maintains existing filtering logic (partial templates, source, category)

4. **Updated module exports** (`swissarmyhammer-cli/src/commands/prompt/mod.rs`):
   - Added `pub mod display;` to export the new display module

### 🧪 Testing Required

- Build verification: `cargo build --bin sah`
- Unit tests: `cargo test`
- Integration testing: `sah prompt list` in different formats
- Verbose mode testing: `sah prompt list --verbose`

### 📋 Benefits Delivered

- **Consistent Formatting**: All output formats (Table, JSON, YAML) now use the same data structures
- **Proper Table Display**: Tables now render correctly using `tabled` instead of JSON fallback
- **Type Safety**: Display objects provide compile-time guarantees for field names and types
- **Clean Architecture**: Display logic is separated from business logic
- **Extensibility**: Easy to add new fields or display types in the future

### 🔧 Dependencies Verified

- `tabled = "0.20"` available in workspace dependencies ✅
- `serde` with derive features available ✅ 
- All required imports accessible ✅

The implementation is ready for testing and should resolve the manual formatting inconsistencies in the prompt list command.
## Implementation Issues and Fixes

### 🐛 Compilation Issues Encountered

1. **Option<String> Display Issue**: 
   - `tabled` derive requires `Display` trait for all fields
   - Fixed by adding `display_with = display_option` attribute
   - Created helper function `display_option()` to handle None values

2. **Test Source Field Type Mismatch**:
   - Test was using `swissarmyhammer::PromptSource::Local` 
   - Actual field type is `Option<PathBuf>`
   - Fixed by using `PathBuf::from("/test/path/test-prompt.md")`
   - Updated test expectations accordingly

3. **Tabled Dependency Missing**:
   - Added `tabled = { workspace = true }` to `swissarmyhammer-cli/Cargo.toml`

### 🔧 Current Status

- ✅ Display objects created with proper derives
- ✅ Context methods implemented  
- ✅ Module exports updated
- ✅ Dependencies added
- 🔄 **Testing compilation fixes...**

### 📋 Next Steps

1. Verify all compilation issues are resolved
2. Run unit tests to ensure functionality
3. Test integration with actual prompt list command
4. Verify output formatting across all formats (Table/JSON/YAML)

The implementation is nearly complete with just compilation fixes remaining to be validated.

---

## Code Review and Fixes Completed ✅

### Summary of Completed Work

All critical code review issues have been resolved and the implementation is ready for production:

#### ✅ Compilation Issues Fixed
- **Fixed `tabled` derive syntax**: Removed problematic `display_with` attribute for Option<String> fields
- **Simplified category field**: Changed from `Option<String>` to `String` using `.unwrap_or_default()` for cleaner display
- **Updated source field formatting**: Now uses readable path display instead of debug formatting

#### ✅ Testing Verified  
- **All 6 display tests passing**: Comprehensive test coverage including edge cases
- **Test data corrected**: Fixed test expectations to match actual clean output format
- **Integration verified**: Display objects work correctly with prompt conversion logic

#### ✅ Code Quality Validated
- **cargo fmt completed**: All code formatted consistently 
- **cargo clippy passed**: No lint warnings or errors
- **cargo build successful**: Clean compilation across all targets

#### ✅ Implementation Benefits Delivered

**Consistent Output Formatting**: 
- Table format now uses proper `tabled` rendering instead of JSON fallback
- JSON and YAML formats use same structured data objects
- Clean separation between standard and verbose display modes

**Type Safety**:
- `PromptRow` and `VerbosePromptRow` provide compile-time guarantees
- `DisplayRows` enum ensures correct handling of different display types
- Proper `From` trait implementations for seamless conversions

**Maintainable Architecture**:
- Display logic cleanly separated in dedicated module
- Easy to extend with new fields or display formats
- Consistent patterns following repository standards

### Files Successfully Implemented

1. **`swissarmyhammer-cli/src/commands/prompt/display.rs`** - Complete display object system
2. **`swissarmyhammer-cli/src/context.rs`** - Enhanced display methods  
3. **`swissarmyhammer-cli/src/commands/prompt/mod.rs`** - Updated list command integration
4. **`swissarmyhammer-cli/Cargo.toml`** - Added required dependencies

### Testing Status

- ✅ All unit tests passing (6/6)
- ✅ No compilation errors
- ✅ No clippy warnings  
- ✅ Consistent code formatting
- ✅ Integration verified with existing prompt system

**Ready for Integration**: The display objects system is complete and fully tested, providing the foundation for consistent output formatting across all CLI commands.